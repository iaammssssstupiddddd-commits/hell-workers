use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::task_execution::types::{AssignedTask, GatherWaterPhase};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// バケツをドロップしてオートホールに任せるヘルパー関数
/// タンクが満タンになった場合や、水汲み完了後に使用
fn drop_bucket_for_auto_haul(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    let drop_grid = WorldMap::world_to_grid(soul_pos);
    let drop_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);
    
    commands.entity(bucket_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(drop_pos.x, drop_pos.y, crate::constants::Z_ITEM_PICKUP),
    ));
    commands.entity(bucket_entity).remove::<crate::relationships::StoredIn>();
    commands.entity(bucket_entity).remove::<crate::systems::logistics::InStockpile>();
    
    ctx.inventory.0 = None;
    haul_cache.release(tank_entity);
    crate::systems::soul_ai::work::unassign_task(
        commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
        None, None, &ctx.queries, haul_cache, world_map, false
    );
}

pub fn handle_gather_water_task(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    phase: GatherWaterPhase,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
    let soul_pos = ctx.soul_pos();
    let q_targets = &ctx.queries.targets;
    // let q_stockpiles = &mut ctx.queries.stockpiles; // 必要な箇所でローカルに取得する
        match phase {
        GatherWaterPhase::GoingToBucket => {
            // 既にインベントリにバケツがある場合は次のフェーズへ
            if ctx.inventory.0 == Some(bucket_entity) {
                // バケツが既にインベントリにあるので、川へ
                if let Some(river_grid) = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate()) {
                    let river_pos = WorldMap::grid_to_world(river_grid.0, river_grid.1);
                    *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::GoingToRiver,
                    });
                    ctx.dest.0 = river_pos;
                    ctx.path.waypoints = vec![river_pos];
                    ctx.path.current_index = 0;
                } else {
                    crate::systems::soul_ai::work::unassign_task(
                        commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                        Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                    );
                }
                return;
            }
            
            let Ok((bucket_transform, _, _, res_item_opt, _, stored_in_opt)) = q_targets.get(bucket_entity) else {
                crate::systems::soul_ai::work::unassign_task(
                    commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                    Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                );
                return;
            };

            // バケツであることを確認（任意だが安全のため）
            if let Some(res) = res_item_opt {
                if !matches!(res.0, ResourceType::BucketEmpty | ResourceType::BucketWater) {
                     crate::systems::soul_ai::work::unassign_task(
                        commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                        Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                    );
                     return;
                }
            }

            let bucket_pos = bucket_transform.translation.truncate();
            // let distance = ctx.soul_pos().distance(bucket_pos);
            let is_adjacent = {
                 let sg = WorldMap::world_to_grid(soul_pos);
                 let bg = WorldMap::world_to_grid(bucket_pos);
                 (sg.0 - bg.0).abs() <= 1 && (sg.1 - bg.1).abs() <= 1
            };

            // 1.8タイルの距離内、または隣接マスにいればピックアップ可能とする
            if crate::systems::soul_ai::task_execution::common::is_near_target(soul_pos, bucket_pos) || is_adjacent {
                // バケツを拾う（管理コンポーネント・StoredInの削除も含む）
                crate::systems::soul_ai::task_execution::common::pickup_item(commands, ctx.soul_entity, bucket_entity, ctx.inventory);
                
                // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                if let Some(stored_in) = stored_in_opt {
                    let q_stockpiles = &mut ctx.queries.stockpiles;
                    crate::systems::soul_ai::task_execution::common::update_stockpile_on_item_removal(stored_in.0, q_stockpiles);
                }
                let is_already_full = if let Some(res) = res_item_opt {
                    res.0 == ResourceType::BucketWater
                } else {
                    false
                };

                if is_already_full {
                    // 既に満タンならタンクへ
                    if let Ok((tank_transform, _, _, _, _, _)) = q_targets.get(tank_entity) {
                        let tank_pos = tank_transform.translation.truncate();
                        let (cx, cy) = WorldMap::world_to_grid(tank_pos);
                        let tank_grids = vec![(cx - 1, cy - 1), (cx, cy - 1), (cx - 1, cy), (cx, cy)];
                        
                        if let Some(path) = crate::world::pathfinding::find_path_to_boundary(
                            world_map,
                            ctx.pf_context,
                            WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
                            &tank_grids
                        ) {
                            *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                                bucket: bucket_entity,
                                tank: tank_entity,
                                phase: GatherWaterPhase::GoingToTank,
                            });
                            if let Some(last_grid) = path.last() {
                                ctx.dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
                            }
                            ctx.path.waypoints = path.iter().map(|&(x, y)| WorldMap::grid_to_world(x, y)).collect();
                            ctx.path.current_index = 0;
                            return;
                        }
                    }
                }

                // 次のフェーズへ：川へ
                if let Some(river_grid) = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate()) {
                    
                    // 経路探索を実行
                    if let Some(path) = crate::world::pathfinding::find_path_to_adjacent(
                        world_map,
                        ctx.pf_context,
                        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
                        river_grid
                    ) {
                        *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                            bucket: bucket_entity,
                            tank: tank_entity,
                            phase: GatherWaterPhase::GoingToRiver,
                        });
                        
                         // パスの最後の地点を目的地とする
                        if let Some(last_grid) = path.last() {
                            let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
                            ctx.dest.0 = last_pos;
                        } else {
                            // パスが空（既に隣接）なら現在地維持でフェーズ以降
                            ctx.dest.0 = ctx.soul_transform.translation.truncate();
                        }
                        
                        ctx.path.waypoints = path.iter()
                            .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                            .collect();
                        ctx.path.current_index = 0;
                    } else {
                         crate::systems::soul_ai::work::unassign_task(
                            commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                            Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                        );
                    }
                } else {
                    crate::systems::soul_ai::work::unassign_task(
                        commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                        Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                    );
                }
                return;
            } else {
                 let bucket_grid = WorldMap::world_to_grid(bucket_pos);
                 // パスがない場合のみ計算する（毎フレームリセットを防ぐ）
                    if ctx.path.waypoints.is_empty() {
                        // バケツがタンク内にめり込んでいる可能性があるため、find_path_to_boundaryを使用する
                        // (find_path_to_adjacentだとゴールが障害物の場合に失敗する)
                        if let Some(path) = crate::world::pathfinding::find_path_to_boundary(
                            world_map,
                            ctx.pf_context,
                            WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
                            &vec![bucket_grid]
                        ) {
                            if let Some(last_grid) = path.last() {
                                let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
                                ctx.dest.0 = last_pos;
                            } else {
                                ctx.dest.0 = bucket_pos;
                            }
                            ctx.path.waypoints = path.iter()
                                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                                .collect();
                            ctx.path.current_index = 0;
                        } else {
                            // 経路がない場合は直線移動
                            ctx.dest.0 = bucket_pos;
                        }
                    }
                }
        }
        GatherWaterPhase::GoingToRiver => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                // バケツを持っていないのでタスクを中断（ドロップ処理なし）
                warn!("GoingToRiver: Bucket not in inventory, aborting task for soul {:?}", ctx.soul_entity);
                crate::systems::soul_ai::work::unassign_task(
                    commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                    None, None, &ctx.queries, haul_cache, world_map, true
                );
                return;
            }

            if ctx.soul_transform.translation.truncate().distance(ctx.dest.0) < 30.0 {
                // タンクが満タンかチェック
                let is_tank_full = {
                    let q_stockpiles = &mut ctx.queries.stockpiles;
                    if let Ok((_, _, stock, Some(stored))) = q_stockpiles.get(tank_entity) {
                        stored.len() >= stock.capacity
                    } else {
                        false
                    }
                };

                if is_tank_full {
                    drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, haul_cache, world_map);
                    return;
                }

                // 水を汲むフェーズへ
                *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Filling { progress: 0.0 },
                });
            }
        }
        GatherWaterPhase::Filling { progress } => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                // バケツを持っていないのでタスクを中断（ドロップ処理なし）
                warn!("Filling: Bucket not in inventory, aborting task for soul {:?}", ctx.soul_entity);
                crate::systems::soul_ai::work::unassign_task(
                    commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                    None, None, &ctx.queries, haul_cache, world_map, true
                );
                return;
            }
            
            let new_progress = progress + time.delta_secs() * 0.5; // 2秒で満タン
            if new_progress >= 1.0 {
                // 満タン！見た目を更新
                commands.entity(bucket_entity).insert((
                    ResourceItem(ResourceType::BucketWater),
                    Sprite {
                         image: game_assets.bucket_water.clone(),
                         custom_size: Some(Vec2::splat(crate::constants::TILE_SIZE * 0.6)),
                         ..default()
                    }
                ));

                // タンクへ
                if let Ok((tank_transform, _, _, _, _, _)) = q_targets.get(tank_entity) {
                    let tank_pos = tank_transform.translation.truncate();
                    
                    // タンクの占有グリッドを計算 (2x2)
                    let (cx, cy) = WorldMap::world_to_grid(tank_pos);
                    // タンク中心座標が(cx, cy)の場合、占有領域は (cx-1, cy-1) などを基準とした2x2
                    let tank_grids = vec![
                        (cx - 1, cy - 1),
                        (cx, cy - 1),
                        (cx - 1, cy),
                        (cx, cy),
                    ];
                    
                    if let Some(path) = crate::world::pathfinding::find_path_to_boundary(
                        world_map,
                        ctx.pf_context,
                        WorldMap::world_to_grid(ctx.soul_transform.translation.truncate()),
                        &tank_grids
                    ) {
                        *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                            bucket: bucket_entity,
                            tank: tank_entity,
                            phase: GatherWaterPhase::GoingToTank,
                        });
                        
                        if let Some(last_grid) = path.last() {
                            let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
                            ctx.dest.0 = last_pos;
                        } else {
                            ctx.dest.0 = tank_pos; // フォールバック
                        }
                        
                        ctx.path.waypoints = path.iter()
                            .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                            .collect();
                        ctx.path.current_index = 0;
                    } else {
                         crate::systems::soul_ai::work::unassign_task(
                            commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                            Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                        );
                    }
                } else {
                    crate::systems::soul_ai::work::unassign_task(
                        commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                        Some(ctx.inventory), None, &ctx.queries, haul_cache, world_map, true
                    );
                }
            } else {
                *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Filling { progress: new_progress },
                });
            }
        }
        GatherWaterPhase::GoingToTank => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                // バケツを持っていないのでタスクを中断（ドロップ処理なし）
                warn!("GoingToTank: Bucket not in inventory, aborting task for soul {:?}", ctx.soul_entity);
                crate::systems::soul_ai::work::unassign_task(
                    commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                    None, None, &ctx.queries, haul_cache, world_map, true
                );
                return;
            }
            
            // タンクが満タンかチェック（移動中に満タンになる可能性）
            let is_tank_full = {
                let q_stockpiles = &mut ctx.queries.stockpiles;
                if let Ok((_, _, stock, Some(stored))) = q_stockpiles.get(tank_entity) {
                    stored.len() >= stock.capacity
                } else {
                    false
                }
            };

            if is_tank_full {
                drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, haul_cache, world_map);
                return;
            }

            if ctx.soul_transform.translation.truncate().distance(ctx.dest.0) < 60.0 { // 2x2なので少し広めに (2タイル分=64.0未満)
                *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Pouring { progress: 0.0 },
                });
            }
        }
        GatherWaterPhase::Pouring { progress } => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                // バケツを持っていないのでタスクを中断（ドロップ処理なし）
                warn!("Pouring: Bucket not in inventory, aborting task for soul {:?}", ctx.soul_entity);
                crate::systems::soul_ai::work::unassign_task(
                    commands, ctx.soul_entity, soul_pos, ctx.task, ctx.path,
                    None, None, &ctx.queries, haul_cache, world_map, true
                );
                return;
            }
            
             let new_progress = progress + time.delta_secs() * 1.0; // 1秒で注ぐ
             if new_progress >= 1.0 {
                 // 注ぎ完了！バケツを空に戻す
                 commands.entity(bucket_entity).insert(ResourceItem(ResourceType::BucketEmpty));
                 commands.entity(bucket_entity).insert(Sprite {
                      image: game_assets.bucket_empty.clone(),
                      custom_size: Some(Vec2::splat(crate::constants::TILE_SIZE * 0.6)),
                      ..default()
                 });

                 // タンクの中身を増やす
                 commands.spawn((
                     ResourceItem(ResourceType::Water),
                     crate::relationships::StoredIn(tank_entity),
                     Visibility::Hidden, // タンクの中なので見えない
                 ));

                 // バケツをその場にドロップしてタスク完了
                 // オートホールシステムがバケツをバケツ置き場に戻す
                 drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, haul_cache, world_map);
             } else {
                 *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                     bucket: bucket_entity,
                     tank: tank_entity,
                     phase: GatherWaterPhase::Pouring { progress: new_progress },
                 });
             }
         }
    }
}
