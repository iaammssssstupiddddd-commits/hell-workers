use super::common::*;
use super::context::TaskExecutionContext;
use super::types::{AssignedTask, HaulWaterToMixerPhase};
use crate::constants::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::gather_water::helpers::drop_bucket_for_auto_haul;
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
    // haul_cache removed
    _time: &Res<Time>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    match phase {
        HaulWaterToMixerPhase::GoingToBucket => {
            if ctx.inventory.0 == Some(bucket_entity) {
                // すでにバケツを持っている場合、バケツの状態を確認
                if let Ok(res_item) = ctx.queries.reservation.resources.get(bucket_entity) {
                    if res_item.0 == ResourceType::BucketWater {
                        ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                            source: tank_entity,
                            amount: 1,
                        });
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
                    ctx.queue_reservation(
                        crate::events::ResourceReservationOp::ReleaseMixerDestination {
                            target: mixer_entity,
                            resource_type: ResourceType::Water,
                        },
                    );
                    clear_task_and_path(ctx.task, ctx.path);
                }
                return;
            }

            if let Ok((bucket_transform, _, _, _res_item_opt, _, _)) =
                ctx.queries.designation.targets.get(bucket_entity)
            {
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
                    // ソース取得記録
                    ctx.queue_reservation(
                        crate::events::ResourceReservationOp::RecordPickedSource {
                            source: bucket_entity,
                            amount: 1,
                        },
                    );

                    let bucket_is_water = match ctx.queries.reservation.resources.get(bucket_entity)
                    {
                        Ok(res_item) => res_item.0 == ResourceType::BucketWater,
                        Err(_) => {
                            ctx.queue_reservation(
                                crate::events::ResourceReservationOp::ReleaseMixerDestination {
                                    target: mixer_entity,
                                    resource_type: ResourceType::Water,
                                },
                            );
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
            } else {
                ctx.queue_reservation(
                    crate::events::ResourceReservationOp::ReleaseMixerDestination {
                        target: mixer_entity,
                        resource_type: ResourceType::Water,
                    },
                );
                clear_task_and_path(ctx.task, ctx.path);
            }
        }
        HaulWaterToMixerPhase::GoingToTank => {
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
                    // タンクへ到達不能の場合、タスクを中断
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
                    *ctx.task = AssignedTask::HaulWaterToMixer(
                        crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                            bucket: bucket_entity,
                            tank: tank_entity,
                            mixer: mixer_entity,
                            amount: 0,
                            phase: HaulWaterToMixerPhase::FillingFromTank,
                        },
                    );
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
                // タンクからの取水フェーズを抜けるためロック解除
                ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
                    source: tank_entity,
                    amount: 1,
                });

                // バケツを水入りに変更
                commands.entity(bucket_entity).insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                        image: game_assets.bucket_water.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                ));

                // Mixerへ
                if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                    let (mixer_transform, _, _) = mixer_data;
                    let mixer_pos = mixer_transform.translation.truncate();

                    *ctx.task = AssignedTask::HaulWaterToMixer(
                        crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                            bucket: bucket_entity,
                            tank: tank_entity,
                            mixer: mixer_entity,
                            amount: take_amount,
                            phase: HaulWaterToMixerPhase::GoingToMixer,
                        },
                    );
                    update_destination_to_adjacent(
                        ctx.dest,
                        mixer_pos,
                        ctx.path,
                        soul_pos,
                        world_map,
                        ctx.pf_context,
                    );
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
            } else {
                // 水が尽きたら中断
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
        HaulWaterToMixerPhase::GoingToMixer => {
            if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
                let (mixer_transform, _, _) = mixer_data;
                let mixer_pos = mixer_transform.translation.truncate();

                if ctx.inventory.0 != Some(bucket_entity) {
                    // バケツを所持していないならタスクを中断
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
                    // 到達不能: バケツをドロップしてタスクをキャンセル
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
                    *ctx.task = AssignedTask::HaulWaterToMixer(
                        crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                            bucket: bucket_entity,
                            tank: tank_entity,
                            mixer: mixer_entity,
                            amount,
                            phase: HaulWaterToMixerPhase::Pouring,
                        },
                    );
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

        HaulWaterToMixerPhase::Pouring => {
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
                    // 空のバケツでは注げない
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
                // バケツが見つからない
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
                    let amount = ctx
                        .task
                        .get_amount_if_haul_water()
                        .unwrap_or(BUCKET_CAPACITY);
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

                    // 予約解除
                    ctx.queue_reservation(
                        crate::events::ResourceReservationOp::ReleaseMixerDestination {
                            target: mixer_entity,
                            resource_type: ResourceType::Water,
                        },
                    );

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
                    // Tankのバケツ置き場を探す
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
                        *ctx.task = AssignedTask::HaulWaterToMixer(
                            crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                                bucket: bucket_entity,
                                tank: tank_entity,
                                mixer: mixer_entity,
                                amount: 0,
                                phase: HaulWaterToMixerPhase::ReturningBucket,
                            },
                        );
                        update_destination_if_needed(ctx.dest, pos, ctx.path);
                    } else {
                        // 戻し先が見つからなければその場にドロップ
                        drop_item(commands, ctx.soul_entity, bucket_entity, soul_pos);
                        ctx.inventory.0 = None;
                        clear_task_and_path(ctx.task, ctx.path);
                    }
                } else {
                    // Mixerがいっぱいなら中断（あまり起こらないはず）
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
        HaulWaterToMixerPhase::ReturningBucket => {
            // 目的地（バケツ置き場）に到着したら終了
            if is_near_target(soul_pos, ctx.dest.0) {
                drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, world_map);
            }
        }
    }
}

fn transition_to_tank(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    // haul_cache removed
    soul_pos: Vec2,
) {
    if let Ok(tank_data) = ctx.queries.storage.stockpiles.get(tank_entity) {
        let (_, tank_transform, _, _) = tank_data;
        let tank_pos = tank_transform.translation.truncate();

        *ctx.task = AssignedTask::HaulWaterToMixer(
            crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                bucket: bucket_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: 0,
                phase: HaulWaterToMixerPhase::GoingToTank,
            },
        );
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
fn transition_to_mixer(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    // haul_cache removed
    world_map: &WorldMap,
    soul_pos: Vec2,
) {
    if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
        let (mixer_transform, _, _) = mixer_data;
        let mixer_pos = mixer_transform.translation.truncate();

        *ctx.task = AssignedTask::HaulWaterToMixer(
            crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
                bucket: bucket_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: BUCKET_CAPACITY, // 既に水入りなので満タンとみなす
                phase: HaulWaterToMixerPhase::GoingToMixer,
            },
        );
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

fn abort_and_drop_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    // haul_cache removed
    pos: Vec2,
) {
    ctx.queue_reservation(
        crate::events::ResourceReservationOp::ReleaseMixerDestination {
            target: mixer_entity,
            resource_type: ResourceType::Water,
        },
    );
    let should_release_tank_lock = matches!(
        ctx.task,
        AssignedTask::HaulWaterToMixer(data)
            if matches!(
                data.phase,
                HaulWaterToMixerPhase::GoingToBucket
                    | HaulWaterToMixerPhase::GoingToTank
                    | HaulWaterToMixerPhase::FillingFromTank
            )
    );
    if should_release_tank_lock {
        ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
            source: tank_entity,
            amount: 1,
        });
    }

    // バケツを地面にドロップして、関連コンポーネントをクリーンアップ
    let drop_grid = WorldMap::world_to_grid(pos);
    let drop_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);
    commands.entity(bucket_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(drop_pos.x, drop_pos.y, crate::constants::Z_ITEM_PICKUP),
    ));
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::StoredIn>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::logistics::InStockpile>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::IssuedBy>();
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::TaskWorkers>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::Designation>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TaskSlots>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TargetMixer>();

    ctx.inventory.0 = None;
    clear_task_and_path(ctx.task, ctx.path);
}
