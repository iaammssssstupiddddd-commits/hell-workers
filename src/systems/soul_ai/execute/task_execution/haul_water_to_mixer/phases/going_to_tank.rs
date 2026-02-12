//! GoingToTank phase: Navigate to tank to fill bucket

use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWaterToMixerData, HaulWaterToMixerPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::abort::abort_and_drop_bucket;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();

        // 2x2なので隣接位置へ
        let reachable = update_destination_to_adjacent(
            ctx.dest,
            tank_pos,
            ctx.path,
            soul_pos,
            world_map,
            ctx.pf_context,
        );

        if !reachable {
            warn!(
                "HAUL_WATER_TO_MIXER: Soul {:?} cannot reach tank {:?}, aborting",
                ctx.soul_entity, tank_entity
            );
            abort_and_drop_bucket(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                soul_pos,
            );
            return;
        }

        if is_near_target_or_dest(soul_pos, tank_pos, ctx.dest.0) {
            *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
                bucket: bucket_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: 0,
                phase: HaulWaterToMixerPhase::FillingFromTank,
            });
            ctx.path.waypoints.clear();
        }
    } else {
        warn!(
            "HAUL_WATER_TO_MIXER: Tank {:?} not found in stockpiles query, aborting",
            tank_entity
        );
        abort_and_drop_bucket(
            commands,
            ctx,
            bucket_entity,
            tank_entity,
            mixer_entity,
            soul_pos,
        );
    }
}
