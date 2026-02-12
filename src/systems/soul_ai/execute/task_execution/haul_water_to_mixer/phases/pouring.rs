//! Pouring phase: Pour water from bucket into mixer

use crate::constants::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::transport_common::reservation;
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
    game_assets: &Res<crate::assets::GameAssets>,
    _world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    // バケツが水入りかチェック
    if ctx.inventory.0 != Some(bucket_entity) {
        warn!(
            "HAUL_WATER_TO_MIXER: Soul {:?} tried to pour without bucket, aborting",
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
    let amount = ctx.task.get_amount_if_haul_water().unwrap_or(0);
    if amount == 0 {
        warn!(
            "HAUL_WATER_TO_MIXER: Soul {:?} tried to pour with zero amount, returning to tank",
            ctx.soul_entity
        );
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
    if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
        if res_item.0 != ResourceType::BucketWater {
            warn!(
                "HAUL_WATER_TO_MIXER: Soul {:?} tried to pour with empty bucket, returning to tank",
                ctx.soul_entity
            );
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

    if let Ok(mixer_data) = ctx.queries.storage.mixers.get_mut(mixer_entity) {
        let (_, _storage, _) = mixer_data;

        let (current_count, capacity) =
            match ctx.queries.storage.stockpiles.get(mixer_entity) {
                Ok((_, _, stockpile, Some(stored_items)))
                    if stockpile.resource_type == Some(ResourceType::Water) =>
                {
                    (stored_items.len(), stockpile.capacity)
                }
                _ => (0, MUD_MIXER_CAPACITY as usize),
            };

        if current_count < capacity {
            let amount = ctx.task.get_amount_if_haul_water().unwrap_or(BUCKET_CAPACITY);
            let available = capacity.saturating_sub(current_count) as u32;
            let added = amount.min(available);

            for _ in 0..added {
                commands.spawn((
                    ResourceItem(ResourceType::Water),
                    crate::relationships::StoredIn(mixer_entity),
                    Visibility::Hidden,
                ));
            }

            info!(
                "TASK_EXEC: Soul {:?} poured {} water into MudMixer",
                ctx.soul_entity, added
            );

            reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);

            // バケツを空に戻す
            commands.entity(bucket_entity).insert((
                ResourceItem(ResourceType::BucketEmpty),
                Sprite {
                    image: game_assets.bucket_empty.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                    ..default()
                },
            ));

            // バケツを戻しに行く
            let mut return_pos = None;
            for (stock_entity, stock_transform, _, _) in
                ctx.queries.storage.stockpiles.iter()
            {
                if let Ok(belongs) = ctx.queries.designation.belongs.get(stock_entity) {
                    if belongs.0 == tank_entity {
                        return_pos = Some(stock_transform.translation.truncate());
                        break;
                    }
                }
            }

            if let Some(pos) = return_pos {
                *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    mixer: mixer_entity,
                    amount: 0,
                    phase: HaulWaterToMixerPhase::ReturningBucket,
                });
                update_destination_if_needed(ctx.dest, pos, ctx.path);
            } else {
                // 戻し先が見つからなければその場にドロップ
                drop_item(commands, ctx.soul_entity, bucket_entity, soul_pos);
                ctx.inventory.0 = None;
                clear_task_and_path(ctx.task, ctx.path);
            }
        } else {
            // Mixerがいっぱいなら中断
            abort_and_drop_bucket(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                soul_pos,
            );
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
