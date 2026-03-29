use bevy::prelude::*;
use hw_core::constants::WHEELBARROW_CAPACITY;
use hw_core::logistics::ResourceType;

use super::super::super::builders::{
    WheelbarrowCollectSpec, issue_collect_bone_with_wheelbarrow_to_soul_spa,
};
use super::super::super::validator::resolve_haul_to_soul_spa_inputs;
use super::demand;
use super::direct_collect;
use super::wheelbarrow;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

/// Soul Spa 建設フェーズへの Bone 搬入タスクを委譲する。
pub fn assign_haul_to_soul_spa(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((site_entity, resource_type)) =
        resolve_haul_to_soul_spa_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    if resource_type != ResourceType::Bone {
        debug!(
            "ASSIGN: SoulSpa request {:?} unexpected resource type {:?}",
            ctx.task_entity, resource_type
        );
        return false;
    }

    // TransportRequest エンティティの Transform がサイト位置を保持している
    let site_pos = queries
        .designation
        .designations
        .get(ctx.task_entity)
        .ok()
        .map(|(_, t, _, _, _, _, _, _)| t.translation.truncate())
        .unwrap_or_default();

    let demand_context =
        demand::DemandReadContext::new(queries, shadow, ctx.tile_site_index, ctx.incoming_snapshot);

    let remaining_needed = demand::compute_remaining_soul_spa_bones(site_entity, &demand_context);
    if remaining_needed == 0 {
        return false;
    }

    let Some((source_entity, source_pos)) =
        direct_collect::find_collect_bone_source(site_pos, ctx.task_area_opt, queries, shadow)
    else {
        debug!(
            "ASSIGN: SoulSpa request {:?} has no available Bone collect source",
            ctx.task_entity
        );
        return false;
    };

    let Some(wb_entity) = wheelbarrow::find_nearest_wheelbarrow(source_pos, queries, shadow) else {
        debug!(
            "ASSIGN: SoulSpa request {:?} has no available wheelbarrow for Bone",
            ctx.task_entity
        );
        return false;
    };

    let amount = remaining_needed.min(WHEELBARROW_CAPACITY as u32);

    issue_collect_bone_with_wheelbarrow_to_soul_spa(
        WheelbarrowCollectSpec {
            wheelbarrow: wb_entity,
            source_entity,
            source_pos,
            destination: site_entity,
            amount,
        },
        site_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
