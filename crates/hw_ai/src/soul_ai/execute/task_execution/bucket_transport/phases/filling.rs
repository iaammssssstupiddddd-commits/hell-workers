//! Filling phase: バケツに水を詰める（川から汲む or タンクから取り出す）

use crate::soul_ai::execute::task_execution::common::update_destination_to_adjacent;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::soul_ai::execute::task_execution::transport_common::reservation;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use bevy::prelude::*;
use hw_core::constants::{BUCKET_CAPACITY, TILE_SIZE};
use hw_core::visual::SoulTaskHandles;
use hw_logistics::{ResourceItem, ResourceType};
use hw_world::WorldMap;

use super::super::abort;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    progress: f32,
    commands: &mut Commands,
    soul_handles: &SoulTaskHandles,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
    if ctx.inventory.0 != Some(data.bucket) {
        warn!(
            "Filling: Bucket not in inventory for soul {:?}",
            ctx.soul_entity
        );
        abort::abort_without_bucket(commands, ctx, data, world_map);
        return;
    }

    let soul_pos = ctx.soul_transform.translation.truncate();

    match data.source {
        BucketTransportSource::River => {
            // 時間経過で水を汲む
            let new_progress = progress + time.delta_secs() * 0.5;

            if new_progress >= 1.0 {
                let tank_entity = match data.destination {
                    BucketTransportDestination::Tank(tank) => tank,
                    _ => {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };

                commands.entity(data.bucket).try_insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                        image: soul_handles.bucket_water.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                ));

                if let Ok((tank_transform, _, _, _, _, _, _)) =
                    ctx.queries.designation.targets.get(tank_entity)
                {
                    let tank_pos = tank_transform.translation.truncate();
                    if super::super::routing::set_path_to_tank_boundary(
                        ctx,
                        world_map,
                        tank_pos,
                        data,
                        BucketTransportPhase::GoingToDestination,
                    )
                    .is_some()
                    {
                        commands
                            .entity(data.bucket)
                            .try_insert(hw_core::relationships::DeliveringTo(tank_entity));
                    } else {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                    }
                } else {
                    abort::abort_with_bucket(commands, ctx, data, world_map);
                }
            } else {
                *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                    phase: BucketTransportPhase::Filling {
                        progress: new_progress,
                    },
                    ..data.clone()
                });
            }
        }
        BucketTransportSource::Tank { tank, .. } => {
            // タンクから水エンティティを取り出す
            let mut found_waters = Vec::new();
            for (res_entity, _, _, res_item, stored_in, _) in ctx.queries.resource_items.iter() {
                if res_item.0 == ResourceType::Water {
                    if let Some(stored) = stored_in {
                        if stored.0 == tank {
                            found_waters.push(res_entity);
                            if found_waters.len() as u32 >= BUCKET_CAPACITY {
                                break;
                            }
                        }
                    }
                }
            }

            if !found_waters.is_empty() {
                let take_amount = found_waters.len() as u32;
                for water_entity in found_waters {
                    commands.entity(water_entity).despawn();
                }

                // タンクからの取水フェーズを抜けるためロック解除
                reservation::release_source(ctx, tank, 1);

                // バケツを水入りに変更
                commands.entity(data.bucket).try_insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                        image: soul_handles.bucket_water.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                ));

                let mixer_entity = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };

                if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                    let (mixer_transform, _, _) = mixer_data;
                    let mixer_pos = mixer_transform.translation.truncate();

                    *ctx.task = AssignedTask::BucketTransport(BucketTransportData {
                        phase: BucketTransportPhase::GoingToDestination,
                        amount: take_amount,
                        source: BucketTransportSource::Tank {
                            tank,
                            needs_fill: true,
                        },
                        ..data.clone()
                    });
                    commands
                        .entity(data.bucket)
                        .try_insert(hw_core::relationships::DeliveringTo(mixer_entity));
                    update_destination_to_adjacent(
                        ctx.dest,
                        mixer_pos,
                        ctx.path,
                        soul_pos,
                        world_map,
                        ctx.pf_context,
                    );
                } else {
                    abort::abort_and_drop_bucket_mixer(
                        commands,
                        ctx,
                        data.bucket,
                        tank,
                        mixer_entity,
                        soul_pos,
                    );
                }
            } else {
                // 水が尽きた
                let mixer = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        abort::abort_with_bucket(commands, ctx, data, world_map);
                        return;
                    }
                };
                abort::abort_and_drop_bucket_mixer(
                    commands,
                    ctx,
                    data.bucket,
                    tank,
                    mixer,
                    soul_pos,
                );
            }
        }
    }
}
