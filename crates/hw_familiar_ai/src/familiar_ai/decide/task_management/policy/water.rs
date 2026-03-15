use bevy::prelude::*;
use hw_core::logistics::ResourceType;

use super::super::builders::{issue_gather_water, issue_haul_water_to_mixer};
use super::super::validator::{
    resolve_gather_water_inputs, resolve_haul_water_to_mixer_inputs, source_not_reserved,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub(super) fn assign_gather_water(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((bucket_entity, tank_entity)) = resolve_gather_water_inputs(
        ctx.task_entity,
        task_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: No suitable bucket/tank found for GatherWater task {:?}",
            ctx.task_entity
        );
        return false;
    };

    if !source_not_reserved(bucket_entity, queries, shadow) {
        return false;
    }

    issue_gather_water(
        bucket_entity,
        tank_entity,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}

pub(super) fn assign_haul_water_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((mixer_entity, tank_entity, bucket_entity)) = resolve_haul_water_to_mixer_inputs(
        ctx.task_entity,
        task_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: HaulWaterToMixer task {:?} has no TargetMixer or no available tank/bucket",
            ctx.task_entity
        );
        return false;
    };

    let bucket_is_full = queries
        .items
        .get(bucket_entity)
        .ok()
        .is_some_and(|(item, _)| item.0 == ResourceType::BucketWater)
        || queries
            .designation
            .targets
            .get(bucket_entity)
            .ok()
            .and_then(|(_, _, _, _, resource_item_opt, _, _)| resource_item_opt.map(|res| res.0))
            .is_some_and(|resource_type| resource_type == ResourceType::BucketWater);

    if !source_not_reserved(bucket_entity, queries, shadow) {
        return false;
    }
    let needs_tank_fill = !bucket_is_full;
    if needs_tank_fill && !source_not_reserved(tank_entity, queries, shadow) {
        return false;
    }
    let mixer_already_reserved = queries.reserved_for_task.get(ctx.task_entity).is_ok();

    issue_haul_water_to_mixer(
        bucket_entity,
        mixer_entity,
        tank_entity,
        needs_tank_fill,
        mixer_already_reserved,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
