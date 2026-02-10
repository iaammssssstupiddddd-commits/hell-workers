use crate::constants::BUCKET_CAPACITY;
use crate::systems::soul_ai::execute::task_execution::common::update_destination_to_adjacent;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWaterToMixerData, HaulWaterToMixerPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::abort::abort_and_drop_bucket;

pub(super) fn transition_to_tank(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    soul_pos: Vec2,
) {
    if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();

        *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
            bucket: bucket_entity,
            tank: tank_entity,
            mixer: mixer_entity,
            amount: 0,
            phase: HaulWaterToMixerPhase::GoingToTank,
        });
        ctx.dest.0 = tank_pos;
        ctx.path.waypoints.clear();
    } else {
        // Tankがなければバケツをドロップして中止
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

/// バケツが既に水入りの場合、直接ミキサーへ向かう
pub(super) fn transition_to_mixer(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    world_map: &WorldMap,
    soul_pos: Vec2,
) {
    if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
        let (mixer_transform, _, _) = mixer_data;
        let mixer_pos = mixer_transform.translation.truncate();

        *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
            bucket: bucket_entity,
            tank: tank_entity,
            mixer: mixer_entity,
            amount: BUCKET_CAPACITY, // 既に水入りなので満タンとみなす
            phase: HaulWaterToMixerPhase::GoingToMixer,
        });
        update_destination_to_adjacent(
            ctx.dest,
            mixer_pos,
            ctx.path,
            soul_pos,
            world_map,
            ctx.pf_context,
        );
    } else {
        // ミキサーがなければバケツをドロップして中止
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
