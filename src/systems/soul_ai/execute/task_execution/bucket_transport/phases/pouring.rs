//! Pouring phase: バケツの水をデスティネーション（タンク or ミキサー）に注ぐ

use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::common::{
    clear_task_and_path, drop_item, update_destination_if_needed,
};
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::reservation;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::{BUCKET_CAPACITY, MUD_MIXER_CAPACITY, TILE_SIZE};

use super::super::{abort, helpers};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    progress: f32,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();

    if ctx.inventory.0 != Some(data.bucket) {
        warn!(
            "Pouring: Bucket not in inventory for soul {:?}",
            ctx.soul_entity
        );
        abort::abort_without_bucket(commands, ctx, data, world_map);
        return;
    }

    match data.destination {
        BucketTransportDestination::Tank(tank_entity) => {
            let new_progress = progress + 1.0;

            if new_progress >= 1.0 {
                if !super::super::guards::tank_can_accept_full_bucket(ctx, tank_entity) {
                    helpers::drop_bucket_for_auto_haul(commands, ctx, data.bucket, world_map);
                    return;
                }

                commands
                    .entity(data.bucket)
                    .try_insert(ResourceItem(ResourceType::BucketEmpty));
                commands.entity(data.bucket).try_insert(Sprite {
                    image: game_assets.bucket_empty.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                    ..default()
                });

                for _ in 0..BUCKET_CAPACITY {
                    commands.spawn((
                        ResourceItem(ResourceType::Water),
                        crate::relationships::StoredIn(tank_entity),
                        Visibility::Hidden,
                    ));
                }

                commands
                    .entity(data.bucket)
                    .remove::<crate::relationships::DeliveringTo>();

                helpers::drop_bucket_for_auto_haul(commands, ctx, data.bucket, world_map);
            } else {
                *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                    phase: BucketTransportPhase::Pouring {
                        progress: new_progress,
                    },
                    ..data.clone()
                });
            }
        }
        BucketTransportDestination::Mixer(mixer_entity) => {
            let tank = match data.source {
                BucketTransportSource::Tank { tank, .. } => tank,
                BucketTransportSource::River => {
                    abort::abort_with_bucket(commands, ctx, data, world_map);
                    return;
                }
            };

            // バケツに水があるか確認
            if let Ok(res_item) = ctx.queries.reservation.resources.get(data.bucket) {
                if res_item.0 != ResourceType::BucketWater {
                    warn!(
                        "Pouring: Empty bucket for mixer, returning to tank for soul {:?}",
                        ctx.soul_entity
                    );
                    transition_to_tank_for_mixer(commands, ctx, data, tank, mixer_entity, soul_pos);
                    return;
                }
            } else {
                abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                );
                return;
            }

            let amount = data.amount;
            if amount == 0 {
                warn!(
                    "Pouring: Zero amount for mixer for soul {:?}, returning to tank",
                    ctx.soul_entity
                );
                transition_to_tank_for_mixer(commands, ctx, data, tank, mixer_entity, soul_pos);
                return;
            }

            // Mixer の容量確認
            let (current_count, capacity) = match ctx.queries.storage.stockpiles.get(mixer_entity) {
                Ok((_, _, stockpile, Some(stored_items)))
                    if stockpile.resource_type == Some(ResourceType::Water) =>
                {
                    (stored_items.len(), stockpile.capacity)
                }
                _ => (0, MUD_MIXER_CAPACITY as usize),
            };

            if current_count < capacity {
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
                commands
                    .entity(data.bucket)
                    .remove::<crate::relationships::DeliveringTo>();

                // バケツを空に戻す
                commands.entity(data.bucket).try_insert((
                    ResourceItem(ResourceType::BucketEmpty),
                    Sprite {
                        image: game_assets.bucket_empty.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                ));

                // バケツを戻しに行く場所を探す（タンクに紐づいた stockpile）
                let mut return_pos = None;
                for (stock_entity, stock_transform, _, _) in ctx.queries.storage.stockpiles.iter() {
                    if let Ok(belongs) = ctx.queries.designation.belongs.get(stock_entity) {
                        if belongs.0 == tank {
                            return_pos = Some(stock_transform.translation.truncate());
                            break;
                        }
                    }
                }

                if let Some(pos) = return_pos {
                    *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                        phase: BucketTransportPhase::ReturningBucket,
                        ..data.clone()
                    });
                    update_destination_if_needed(ctx.dest, pos, ctx.path);
                } else {
                    drop_item(commands, ctx.soul_entity, data.bucket, soul_pos);
                    ctx.inventory.0 = None;
                    clear_task_and_path(ctx.task, ctx.path);
                }
            } else {
                // Mixer が満杯
                abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer_entity,
                    soul_pos,
                );
            }
        }
    }
}

fn transition_to_tank_for_mixer(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    tank_entity: Entity,
    _mixer_entity: Entity,
    _soul_pos: Vec2,
) {
    if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();
        commands
            .entity(data.bucket)
            .remove::<crate::relationships::DeliveringTo>();

        *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
            phase: BucketTransportPhase::GoingToSource,
            source: BucketTransportSource::Tank {
                tank: tank_entity,
                needs_fill: true,
            },
            amount: 0,
            ..data.clone()
        });
        ctx.dest.0 = tank_pos;
        ctx.path.waypoints.clear();
    } else {
        let mixer = match data.destination {
            BucketTransportDestination::Mixer(m) => m,
            _ => return,
        };
        let soul_pos = ctx.soul_pos();
        abort::abort_and_drop_bucket_mixer(
            commands,
            ctx,
            data.bucket,
            tank_entity,
            mixer,
            soul_pos,
        );
    }
}
