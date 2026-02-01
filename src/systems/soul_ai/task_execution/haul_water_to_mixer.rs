use bevy::prelude::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, HaulWaterToMixerPhase};
use super::common::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::world::map::WorldMap;
use crate::constants::*;
use crate::systems::soul_ai::task_execution::gather_water::helpers::drop_bucket_for_auto_haul;

pub fn handle_haul_water_to_mixer_task(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    phase: HaulWaterToMixerPhase,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    _time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        HaulWaterToMixerPhase::GoingToBucket => {
            if ctx.inventory.0 == Some(bucket_entity) {
                // すでにバケツを持っていればTankへ
                transition_to_tank(ctx, bucket_entity, tank_entity, mixer_entity);
                return;
            }

            if let Ok((bucket_transform, _, _, _, _, _)) = ctx.queries.targets.get(bucket_entity) {
                let bucket_pos = bucket_transform.translation.truncate();
                update_destination_if_needed(ctx.dest, bucket_pos, ctx.path);

                if is_near_target(soul_pos, bucket_pos) {
                    pickup_item(commands, ctx.soul_entity, bucket_entity, ctx.inventory);
                    transition_to_tank(ctx, bucket_entity, tank_entity, mixer_entity);
                }
            } else {
                haul_cache.release_mixer(mixer_entity, ResourceType::Water);
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulWaterToMixerPhase::GoingToTank => {
            if let Ok(tank_data) = ctx.queries.stockpiles.get(tank_entity) {
                let (_, tank_transform, _, _) = tank_data;
                let tank_pos = tank_transform.translation.truncate();
                
                // 2x2なので隣接位置へ
                update_destination_to_adjacent(ctx.dest, tank_pos, ctx.path, soul_pos, world_map);

                if is_near_target(soul_pos, tank_pos) {
                    *ctx.task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        mixer: mixer_entity,
                        amount: 0,
                        phase: HaulWaterToMixerPhase::FillingFromTank,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
            }

        }
        HaulWaterToMixerPhase::FillingFromTank => {
            // Tankから水を最大 BUCKET_CAPACITY 個取り出す
            let mut found_waters = Vec::new();
            for (res_entity, res_item, stored_in) in ctx.queries.resource_items.iter() {
                if res_item.0 == ResourceType::Water {
                    if let Some(stored) = stored_in {
                        if stored.0 == tank_entity {
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
                
                // バケツを水入りに変更
                commands.entity(bucket_entity).insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                        image: game_assets.bucket_water.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    }
                ));

                // Mixerへ
                if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                    let (mixer_transform, _, _) = mixer_data;
                    let mixer_pos = mixer_transform.translation.truncate();
                    
                    *ctx.task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        mixer: mixer_entity,
                        amount: take_amount,
                        phase: HaulWaterToMixerPhase::GoingToMixer,
                    });
                    update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map);
                } else {
                    abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
                }
            } else {
                // 水が尽きたら中断
                abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
            }

        }
        HaulWaterToMixerPhase::GoingToMixer => {
            if let Ok(mixer_data) = ctx.queries.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();
                
                // 到達可能かチェック
                let reachable = update_destination_to_adjacent(ctx.dest, mixer_pos, ctx.path, soul_pos, world_map);
                
                if !reachable {
                    // 到達不能: バケツをドロップしてタスクをキャンセル
                    info!("HAUL_WATER_TO_MIXER: Soul {:?} cannot reach mixer {:?}, aborting", ctx.soul_entity, mixer_entity);
                    abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
                    return;
                }

                if is_near_target(soul_pos, mixer_pos) {
                    *ctx.task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        mixer: mixer_entity,
                        amount: ctx.task.get_amount_if_haul_water().unwrap_or(0),
                        phase: HaulWaterToMixerPhase::Pouring,
                    });
                    ctx.path.waypoints.clear();
                }
            } else {
                abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
            }
        }

        HaulWaterToMixerPhase::Pouring => {
            if let Ok(mixer_data) = ctx.queries.mixers.get_mut(mixer_entity) {
                let (_, mut storage, _) = mixer_data;
                
                if storage.water < MUD_MIXER_CAPACITY {
                    let amount = ctx.task.get_amount_if_haul_water().unwrap_or(1);
                    storage.water = (storage.water + amount).min(MUD_MIXER_CAPACITY);
                    info!("TASK_EXEC: Soul {:?} poured {} water into MudMixer", ctx.soul_entity, amount);
                    
                    // 予約解除
                    haul_cache.release_mixer(mixer_entity, ResourceType::Water);

                    // バケツを空に戻す
                    commands.entity(bucket_entity).insert((
                        ResourceItem(ResourceType::BucketEmpty),
                        Sprite {
                            image: game_assets.bucket_empty.clone(),
                            custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                            ..default()
                        }
                    ));

                    // バケツを戻しに行く
                    // Tankのバケツ置き場を探す
                    let mut return_pos = None;
                    for (stock_entity, stock_transform, _, _) in ctx.queries.stockpiles.iter() {
                        if let Ok(belongs) = ctx.queries.belongs.get(stock_entity) {
                            if belongs.0 == tank_entity {
                                return_pos = Some(stock_transform.translation.truncate());
                                break;
                            }
                        }
                    }

                    if let Some(pos) = return_pos {
                        *ctx.task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
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
                    // Mixerがいっぱいなら中断（あまり起こらないはず）
                    abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
                }
            } else {
                abort_and_drop_bucket(commands, ctx, bucket_entity, mixer_entity, haul_cache, soul_pos);
            }
        }
        HaulWaterToMixerPhase::ReturningBucket => {
            // 目的地（バケツ置き場）に到着したら終了
            if ctx.soul_transform.translation.truncate().distance(ctx.dest.0) < 10.0 {
                drop_bucket_for_auto_haul(
                    commands, ctx, bucket_entity, tank_entity, haul_cache, world_map
                );
            }
        }
    }
}

fn transition_to_tank(ctx: &mut TaskExecutionContext, bucket_entity: Entity, tank_entity: Entity, mixer_entity: Entity) {
    if let Ok(tank_data) = ctx.queries.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();
        
        *ctx.task = AssignedTask::HaulWaterToMixer(crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
            bucket: bucket_entity,
            tank: tank_entity,
            mixer: mixer_entity,
            amount: 0,
            phase: HaulWaterToMixerPhase::GoingToTank,
        });
        ctx.dest.0 = tank_pos;
        ctx.path.waypoints.clear();
    } else {
        // Tankがなければ中止
        *ctx.task = AssignedTask::None;
    }
}

fn abort_and_drop_bucket(
    commands: &mut Commands, 
    ctx: &mut TaskExecutionContext, 
    bucket_entity: Entity, 
    mixer_entity: Entity,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    pos: Vec2
) {
    haul_cache.release_mixer(mixer_entity, ResourceType::Water);
    drop_item(commands, ctx.soul_entity, bucket_entity, pos);
    ctx.inventory.0 = None;
    clear_task_and_path(ctx.task, ctx.path);
}

