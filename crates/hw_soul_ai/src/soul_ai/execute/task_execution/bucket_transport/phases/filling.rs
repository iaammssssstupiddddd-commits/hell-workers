//! Filling phase: バケツに水を詰める（川から汲む or タンクから取り出す）

use crate::soul_ai::execute::task_execution::common::{
    PathSearchResult, update_bucket_destination_to_adjacent,
};
use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::transport_common::reservation;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use bevy::prelude::*;
use hw_core::constants::{BUCKET_CAPACITY, TILE_SIZE};
use hw_logistics::{ResourceItem, ResourceType};

use super::super::{abort, routing};

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: &BucketTransportData,
    progress: f32,
    commands: &mut Commands,
) -> TaskHandlerControl {
    if ctx.inventory.0 != Some(data.bucket) {
        warn!(
            "Filling: Bucket not in inventory for soul {:?}",
            ctx.soul_entity
        );
        return abort::abort_without_bucket(commands, ctx, data, ctx.env.world_map);
    }

    let soul_pos = ctx.soul_transform.translation.truncate();

    match data.source {
        BucketTransportSource::River => {
            // 時間経過で水を汲む
            let new_progress = progress + ctx.env.time.delta_secs() * 0.5;

            if new_progress >= 1.0 {
                let tank_entity = match data.destination {
                    BucketTransportDestination::Tank(tank) => tank,
                    _ => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
                };

                let Ok((tank_transform, _, _, _, _, _, _)) =
                    ctx.queries.designation.targets.get(tank_entity)
                else {
                    return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                };
                let tank_pos = tank_transform.translation.truncate();
                match routing::set_path_to_tank_boundary(
                    ctx,
                    ctx.env.world_map,
                    tank_pos,
                    data,
                    BucketTransportPhase::GoingToDestination,
                ) {
                    PathSearchResult::Found(()) => {
                        #[cfg(feature = "profiling")]
                        let bucket_before = ctx
                            .queries
                            .reservation
                            .resources
                            .get(data.bucket)
                            .ok()
                            .map(|r| r.0);
                        commands.entity(data.bucket).try_insert((
                            ResourceItem(ResourceType::BucketWater),
                            Sprite {
                                image: ctx.env.soul_handles.bucket_water.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                                ..default()
                            },
                        ));
                        commands
                            .entity(data.bucket)
                            .try_insert(hw_core::relationships::DeliveringTo(tank_entity));
                        #[cfg(feature = "profiling")]
                        if ctx.env.water_observations {
                            crate::water_observations::RiverFillReceipt {
                                worker: ctx.soul_entity,
                                assignment: ctx.task_identity().assignment_entity,
                                target: ctx.task_identity().current_target_entity,
                                tank: tank_entity,
                                bucket: data.bucket,
                                world_epoch: ctx.world_epoch,
                                bucket_before,
                            }
                            .queue(commands);
                        }
                    }
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
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
                if res_item.0 == ResourceType::Water
                    && !ctx.water_claims.taken.contains(&res_entity)
                    && let Some(stored) = stored_in
                    && stored.0 == tank
                {
                    found_waters.push(res_entity);
                    if found_waters.len() as u32 >= BUCKET_CAPACITY {
                        break;
                    }
                }
            }

            if found_waters.len() == BUCKET_CAPACITY as usize {
                let mixer_entity = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
                };

                let Ok((mixer_transform, _, _)) = ctx.queries.storage.mixers.get(mixer_entity)
                else {
                    return abort::abort_and_drop_bucket_mixer(
                        commands,
                        ctx,
                        data.bucket,
                        tank,
                        mixer_entity,
                        soul_pos,
                    );
                };
                let mixer_pos = mixer_transform.translation.truncate();
                let route = update_bucket_destination_to_adjacent(ctx, mixer_pos);
                match route {
                    PathSearchResult::Deferred => return TaskHandlerControl::Continue,
                    PathSearchResult::Unreachable => {
                        return abort::abort_and_drop_bucket_mixer(
                            commands,
                            ctx,
                            data.bucket,
                            tank,
                            mixer_entity,
                            soul_pos,
                        );
                    }
                    PathSearchResult::Found(()) => {}
                }

                let take_amount = found_waters.len() as u32;
                ctx.water_claims.buckets.insert(data.bucket);
                #[cfg(feature = "profiling")]
                let receipt = ctx.env.water_observations.then(|| {
                    crate::water_observations::TankTakeReceipt {
                        context: crate::water_observations::TransferContext {
                            worker: ctx.soul_entity,
                            assignment: ctx.task_identity().assignment_entity,
                            target: ctx.task_identity().current_target_entity,
                            tank,
                            mixer: mixer_entity,
                            bucket: data.bucket,
                            carried_before: ctx.inventory.0,
                            world_epoch: ctx.world_epoch,
                            work_type: ctx.task_identity().current_work_type,
                        },
                        bucket_before: ctx
                            .queries
                            .reservation
                            .resources
                            .get(data.bucket)
                            .ok()
                            .map(|r| r.0),
                        inputs: found_waters.clone(),
                        inputs_before: found_waters
                            .iter()
                            .filter_map(|&entity| {
                                ctx.queries.resource_items.get(entity).ok().map(
                                    |(_, _, _, item, stored, loaded)| {
                                        crate::water_observations::WaterInputObservation {
                                            entity,
                                            resource: item.0,
                                            stored_in: stored.map(|r| r.0),
                                            loaded_in: loaded.map(|r| r.0),
                                        }
                                    },
                                )
                            })
                            .collect(),
                        route_found: matches!(route, PathSearchResult::Found(())),
                    }
                });
                for water_entity in found_waters {
                    ctx.water_claims.taken.insert(water_entity);
                    commands.entity(water_entity).despawn();
                }

                // タンクからの取水フェーズを抜けるためロック解除
                reservation::release_source(ctx, tank, 1);

                // バケツを水入りに変更
                commands.entity(data.bucket).try_insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                        image: ctx.env.soul_handles.bucket_water.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                ));
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
                #[cfg(feature = "profiling")]
                if let Some(receipt) = receipt {
                    receipt.queue(commands);
                }
            } else {
                // A Boolean BucketWater always means a full bucket. Do not turn
                // 1..4 Water into a bucket that pickup later reinterprets as five.
                let mixer = match data.destination {
                    BucketTransportDestination::Mixer(m) => m,
                    _ => {
                        return abort::abort_with_bucket(commands, ctx, data, ctx.env.world_map);
                    }
                };
                return abort::abort_and_drop_bucket_mixer(
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

    TaskHandlerControl::Continue
}
