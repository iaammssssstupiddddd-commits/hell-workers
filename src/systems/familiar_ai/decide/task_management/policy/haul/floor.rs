//! 床建設向け運搬タスク

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use super::super::super::builders::{
    issue_collect_bone_with_wheelbarrow_to_floor, issue_haul_to_stockpile_with_source,
    issue_haul_with_wheelbarrow,
};
use super::super::super::validator::resolve_haul_to_floor_construction_inputs;
use super::demand;
use super::direct_collect;
use super::source_selector;
use super::wheelbarrow;

pub fn assign_haul_to_floor_construction(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((site_entity, resource_type)) =
        resolve_haul_to_floor_construction_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let site_pos = if let Ok((site_transform, _, _)) = queries.storage.floor_sites.get(site_entity)
    {
        site_transform.translation.truncate()
    } else {
        debug!(
            "ASSIGN: Floor request {:?} site {:?} not found",
            ctx.task_entity, site_entity
        );
        return false;
    };

    if resource_type == ResourceType::StasisMud {
        let remaining_needed = demand::compute_remaining_floor_mud(site_entity, queries);
        if remaining_needed == 0 {
            return false;
        }

        let max_items =
            remaining_needed.min(crate::constants::WHEELBARROW_CAPACITY as u32) as usize;
        let mut item_sources = source_selector::collect_nearby_items_for_wheelbarrow(
            resource_type,
            site_pos,
            max_items,
            queries,
            shadow,
        );
        if item_sources.is_empty() {
            item_sources = source_selector::collect_items_for_wheelbarrow_unbounded(
                resource_type,
                site_pos,
                max_items,
                queries,
                shadow,
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
            .unwrap()
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
            wheelbarrow,
            source_pos,
            crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                site_entity,
            ),
            item_entities,
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
            site_entity,
            ctx.task_entity,
            site_pos,
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

fn try_direct_bone_collect_to_floor(
    site_entity: Entity,
    task_entity: Entity,
    site_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((source_entity, source_pos)) =
        direct_collect::find_collect_bone_source(site_pos, ctx.task_area_opt, queries, shadow)
    else {
        debug!(
            "ASSIGN: Floor request {:?} has no available Bone collect source",
            task_entity
        );
        return false;
    };

    let Some(wheelbarrow) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: Floor request {:?} has no available wheelbarrow for Bone collect",
            task_entity
        );
        return false;
    };

    let remaining_needed = demand::compute_remaining_floor_bones(site_entity, queries);
    if remaining_needed == 0 {
        debug!(
            "ASSIGN: Floor request {:?} already satisfied before direct collect assignment",
            task_entity
        );
        return false;
    }
    let amount = remaining_needed.min(crate::constants::WHEELBARROW_CAPACITY as u32);

    issue_collect_bone_with_wheelbarrow_to_floor(
        wheelbarrow,
        source_entity,
        source_pos,
        site_entity,
        amount,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    info!(
        "ASSIGN: Floor request {:?} assigned direct Bone collect via wheelbarrow {:?} from {:?} to site {:?} (amount {})",
        task_entity, wheelbarrow, source_entity, site_entity, amount
    );
    true
}
