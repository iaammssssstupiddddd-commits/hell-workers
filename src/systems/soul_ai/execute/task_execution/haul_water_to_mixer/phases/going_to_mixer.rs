//! GoingToMixer phase: Navigate to mixer with water-filled bucket

use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWaterToMixerData, HaulWaterToMixerPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::abort::abort_and_drop_bucket;
use super::super::transitions::transition_to_tank;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
        let (mixer_transform, _, _) = mixer_data;
        let mixer_pos = mixer_transform.translation.truncate();

        if ctx.inventory.0 != Some(bucket_entity) {
            warn!(
                "HAUL_WATER_TO_MIXER: Soul {:?} has no bucket while going to mixer, aborting",
                ctx.soul_entity
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

        if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
            if res_item.0 != ResourceType::BucketWater {
                // 空バケツなら即タンクへ戻る（ミキサーへ向かわせない）
                transition_to_tank(
                    commands,
                    ctx,
                    bucket_entity,
                    tank_entity,
                    mixer_entity,
                    soul_pos,
                );
                return;
            }
        } else {
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

        let amount = ctx.task.get_amount_if_haul_water().unwrap_or(0);
        if amount == 0 {
            // 水量が不明/ゼロなら即タンクへ戻る
            transition_to_tank(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                soul_pos,
            );
            return;
        }

        // 到達可能かチェック
        let reachable = update_destination_to_adjacent(
            ctx.dest,
            mixer_pos,
            ctx.path,
            soul_pos,
            world_map,
            ctx.pf_context,
        );

        if !reachable {
            info!(
                "HAUL_WATER_TO_MIXER: Soul {:?} cannot reach mixer {:?}, aborting",
                ctx.soul_entity, mixer_entity
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

        if is_near_target_or_dest(soul_pos, mixer_pos, ctx.dest.0) {
            *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
                bucket: bucket_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount,
                phase: HaulWaterToMixerPhase::Pouring,
            });
            ctx.path.waypoints.clear();
        }
    } else {
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
