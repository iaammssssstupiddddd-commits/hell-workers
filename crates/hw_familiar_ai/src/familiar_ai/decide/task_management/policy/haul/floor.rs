use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};

use super::super::super::builders::{
    WheelbarrowCollectSpec, WheelbarrowHaulSpec, issue_collect_bone_with_wheelbarrow_to_floor,
    issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_floor_construction_inputs;
use super::demand;
use super::direct_collect;
use super::source_selector;
use super::wheelbarrow;
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn assign_haul_to_floor_construction(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((site_entity, resource_type)) =
        resolve_haul_to_floor_construction_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let site_pos = match construction_sites.floor_site_pos(site_entity) {
        Some(pos) => pos,
        None => {
            debug!(
                "ASSIGN: Floor request {:?} site {:?} not found",
                ctx.task_entity, site_entity
            );
            return false;
        }
    };
    let demand_context =
        demand::DemandReadContext::new(queries, shadow, ctx.tile_site_index, ctx.incoming_snapshot);

    let remaining_needed = match resource_type {
        ResourceType::Bone => demand::compute_remaining_floor_bones(site_entity, &demand_context),
        ResourceType::StasisMud => {
            demand::compute_remaining_floor_mud(site_entity, &demand_context)
        }
        _ => 0,
    };
    if remaining_needed == 0 {
        return false;
    }

    if resource_type == ResourceType::StasisMud {
        let max_items =
            remaining_needed.min(hw_core::constants::WHEELBARROW_CAPACITY as u32) as usize;
        let mut item_sources = source_selector::collect_nearby_items_for_wheelbarrow(
            resource_type,
            site_pos,
            max_items,
            queries,
            shadow,
            ctx.resource_grid,
        );
        if item_sources.is_empty() {
            item_sources = source_selector::collect_items_for_wheelbarrow_unbounded(
                resource_type,
                site_pos,
                max_items,
                queries,
                shadow,
                ctx.resource_grid,
            );
        }
        if item_sources.is_empty() {
            debug!(
                "ASSIGN: Floor request {:?} has no available {:?} source",
                ctx.task_entity, resource_type
            );
            return false;
        }

        let source_pos = item_sources
            .iter()
            .map(|(_, pos)| *pos)
            .reduce(|a, b| a + b)
            .expect("item_sources is non-empty: checked above")
            / item_sources.len() as f32;

        let Some(wheelbarrow) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)
        else {
            debug!(
                "ASSIGN: Floor request {:?} has no available wheelbarrow for {:?}",
                ctx.task_entity, resource_type
            );
            return false;
        };

        let item_entities = item_sources.into_iter().map(|(entity, _)| entity).collect();
        issue_haul_with_wheelbarrow(
            WheelbarrowHaulSpec {
                wheelbarrow,
                source_pos,
                destination: WheelbarrowDestination::Stockpile(site_entity),
                items: item_entities,
            },
            site_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    if let Some((source_item, source_pos)) = source_selector::find_nearest_blueprint_source_item(
        resource_type,
        site_pos,
        queries,
        shadow,
        ctx.resource_grid,
    ) {
        issue_haul_to_stockpile_with_source(
            source_item,
            site_entity,
            source_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }

    if resource_type == ResourceType::Bone
        && try_direct_bone_collect_to_floor(
            FloorBoneCollectParams {
                site_entity,
                task_entity: ctx.task_entity,
                remaining_needed,
                site_pos,
            },
            already_commanded,
            ctx,
            queries,
            shadow,
        )
    {
        return true;
    }

    debug!(
        "ASSIGN: Floor request {:?} has no available {:?} source",
        ctx.task_entity, resource_type
    );
    false
}

/// `try_direct_bone_collect_to_floor` の設定パラメータをまとめた構造体。
struct FloorBoneCollectParams {
    site_entity: Entity,
    task_entity: Entity,
    remaining_needed: u32,
    site_pos: Vec2,
}

fn try_direct_bone_collect_to_floor(
    params: FloorBoneCollectParams,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_entity, source_pos)) = direct_collect::find_collect_bone_source(
        params.site_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: Floor request {:?} has no available Bone collect source",
            params.task_entity
        );
        return false;
    };

    let Some(wheelbarrow) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: Floor request {:?} has no available wheelbarrow for Bone collect",
            params.task_entity
        );
        return false;
    };

    let amount = params
        .remaining_needed
        .min(hw_core::constants::WHEELBARROW_CAPACITY as u32);

    issue_collect_bone_with_wheelbarrow_to_floor(
        WheelbarrowCollectSpec {
            wheelbarrow,
            source_entity,
            source_pos,
            destination: params.site_entity,
            amount,
        },
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    info!(
        "ASSIGN: Floor request {:?} assigned direct Bone collect via wheelbarrow {:?} from {:?} to site {:?} (amount {})",
        params.task_entity, wheelbarrow, source_entity, params.site_entity, amount
    );
    true
}
