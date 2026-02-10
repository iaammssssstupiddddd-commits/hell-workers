//! Phase handlers for water gathering task

mod filling;
mod going_to_bucket;
mod going_to_tank;
mod going_to_river;
mod pouring;

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, GatherWaterData, GatherWaterPhase};
use crate::world::map::WorldMap;
use bevy::prelude::*;


pub fn assigned_task(bucket: Entity, tank: Entity, phase: GatherWaterPhase) -> AssignedTask {
    AssignedTask::GatherWater(GatherWaterData { bucket, tank, phase })
}

pub fn handle_gather_water_task(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    phase: GatherWaterPhase,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        GatherWaterPhase::GoingToBucket => {
            going_to_bucket::handle(
                ctx,
                bucket_entity,
                tank_entity,
                commands,
                world_map,
                soul_pos,
            );
        }
        GatherWaterPhase::GoingToRiver => {
            going_to_river::handle(ctx, bucket_entity, tank_entity, commands, world_map, soul_pos);
        }
        GatherWaterPhase::Filling { progress } => {
            filling::handle(
                ctx,
                bucket_entity,
                tank_entity,
                progress,
                commands,
                game_assets,
                time,
                world_map,
                soul_pos,
            );
        }
        GatherWaterPhase::GoingToTank => {
            going_to_tank::handle(ctx, bucket_entity, tank_entity, commands, world_map, soul_pos);
        }
        GatherWaterPhase::Pouring { progress } => {
            pouring::handle(
                ctx,
                bucket_entity,
                tank_entity,
                progress,
                commands,
                game_assets,
                world_map,
                soul_pos,
            );
        }
    }
}
