use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::builders::{issue_gather_water, issue_haul_water_to_mixer};
use super::super::validator::{
    find_best_tank_for_bucket, resolve_haul_water_to_mixer_inputs, source_not_reserved,
};

pub(super) fn assign_gather_water(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !source_not_reserved(ctx.task_entity, queries, shadow) {
        return false;
    }

    let best_tank = find_best_tank_for_bucket(
        ctx.task_entity,
        task_pos,
        ctx.task_area_opt,
        queries,
        shadow,
    );

    if let Some(tank_entity) = best_tank {
        issue_gather_water(
            tank_entity,
            task_pos,
            already_commanded,
            ctx,
            queries,
            shadow,
        );
        return true;
    }
    debug!(
        "ASSIGN: No suitable tank/mixer found for bucket {:?}",
        ctx.task_entity
    );
    false
}

pub(super) fn assign_haul_water_to_mixer(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
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

    if !source_not_reserved(bucket_entity, queries, shadow) {
        return false;
    }
    // Tankからの取水競合を避けるため、1タンク1作業のロックを確認
    if !source_not_reserved(tank_entity, queries, shadow) {
        return false;
    }
    let mixer_already_reserved = queries.reserved_for_task.get(ctx.task_entity).is_ok();

    issue_haul_water_to_mixer(
        bucket_entity,
        mixer_entity,
        tank_entity,
        mixer_already_reserved,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
