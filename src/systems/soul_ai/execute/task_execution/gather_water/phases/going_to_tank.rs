//! GoingToTank phase: Navigate to storage tank

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::guards;
use super::super::helpers::{abort_task_without_item, drop_bucket_for_auto_haul};
use super::assigned_task;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    commands: &mut Commands,
    world_map: &WorldMap,
    _soul_pos: Vec2,
) {
    if !guards::has_bucket_in_inventory(ctx, bucket_entity) {
        warn!(
            "GoingToTank: Bucket not in inventory, aborting task for soul {:?}",
            ctx.soul_entity
        );
        abort_task_without_item(commands, ctx, world_map);
        return;
    }

    if guards::is_tank_full(ctx, tank_entity) {
        drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, world_map);
        return;
    }

    if ctx
        .soul_transform
        .translation
        .truncate()
        .distance(ctx.dest.0)
        < 60.0
    {
        *ctx.task = assigned_task(
            bucket_entity,
            tank_entity,
            GatherWaterPhase::Pouring { progress: 0.0 },
        );
    }
}
