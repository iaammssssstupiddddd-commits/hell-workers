//! Phase handlers for haul water to mixer task

mod filling_from_tank;
mod going_to_bucket;
mod going_to_mixer;
mod going_to_tank;
mod pouring;
mod returning_bucket;

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerPhase;
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_water_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    phase: HaulWaterToMixerPhase,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    _time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    match phase {
        HaulWaterToMixerPhase::GoingToBucket => {
            going_to_bucket::handle(
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                commands,
                game_assets,
                world_map,
            );
        }
        HaulWaterToMixerPhase::GoingToTank => {
            going_to_tank::handle(
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                commands,
                world_map,
            );
        }
        HaulWaterToMixerPhase::FillingFromTank => {
            filling_from_tank::handle(
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                commands,
                game_assets,
                world_map,
            );
        }
        HaulWaterToMixerPhase::GoingToMixer => {
            going_to_mixer::handle(
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                commands,
                world_map,
            );
        }
        HaulWaterToMixerPhase::Pouring => {
            pouring::handle(
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                commands,
                game_assets,
                world_map,
            );
        }
        HaulWaterToMixerPhase::ReturningBucket => {
            returning_bucket::handle(
                ctx,
                bucket_entity,
                tank_entity,
                commands,
                world_map,
            );
        }
    }
}
