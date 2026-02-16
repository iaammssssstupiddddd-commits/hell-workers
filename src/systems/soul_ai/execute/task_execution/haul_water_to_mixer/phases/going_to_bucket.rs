//! GoingToBucket phase: Navigate to pick up bucket, or verify bucket state

use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::reservation;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::transitions::{transition_to_mixer, transition_to_tank};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    commands: &mut Commands,
    _game_assets: &Res<crate::assets::GameAssets>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    if ctx.inventory.0 == Some(bucket_entity) {
        // すでにバケツを持っている場合、バケツの状態を確認
        if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
            if res_item.0 == ResourceType::BucketWater {
                reservation::release_source(ctx, tank_entity, 1);
                transition_to_mixer(
                    commands,
                    ctx,
                    bucket_entity,
                    tank_entity,
                    mixer_entity,
                    world_map,
                    soul_pos,
                );
            } else {
                // 空ならタンクへ
                transition_to_tank(
                    commands,
                    ctx,
                    bucket_entity,
                    tank_entity,
                    mixer_entity,
                    soul_pos,
                );
            }
        } else {
            // バケツが見つからない場合は中断
            reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);
            clear_task_and_path(ctx.task, ctx.path);
        }
        return;
    }

    let Ok((bucket_transform, _, _, _, _res_item_opt, _, _)) =
        ctx.queries.designation.targets.get(bucket_entity)
    else {
        reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);
        clear_task_and_path(ctx.task, ctx.path);
        return;
    };

    let bucket_pos = bucket_transform.translation.truncate();
    update_destination_if_needed(ctx.dest, bucket_pos, ctx.path);

    if can_pickup_item(soul_pos, bucket_pos) {
        if !try_pickup_item(
            commands,
            ctx.soul_entity,
            bucket_entity,
            ctx.inventory,
            soul_pos,
            bucket_pos,
            ctx.task,
            ctx.path,
        ) {
            return;
        }
        reservation::record_picked_source(ctx, bucket_entity, 1);

        let bucket_is_water = match ctx.queries.reservation.resources.get(bucket_entity) {
            Ok(res_item) => res_item.0 == ResourceType::BucketWater,
            Err(_) => {
                reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);
                clear_task_and_path(ctx.task, ctx.path);
                return;
            }
        };

        if bucket_is_water {
            // 既に水入りなら直接ミキサーへ
            transition_to_mixer(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                world_map,
                soul_pos,
            );
        } else {
            // 空ならタンクへ
            transition_to_tank(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                soul_pos,
            );
        }
    }
}
