use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::task_execution::types::{AssignedTask, GatherWaterPhase};
use crate::world::map::WorldMap;
use bevy::prelude::*;

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
    let q_stockpiles = &mut ctx.queries.stockpiles;
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
                    *ctx.task = AssignedTask::None;
                }
                return;
            }
            
            let Ok((bucket_transform, _, _, res_item_opt, _, _)) = q_targets.get(bucket_entity) else {
                ctx.soul.motivation -= 0.01;
                *ctx.task = AssignedTask::None;
                return;
            };

            // バケツであることを確認（任意だが安全のため）
            if let Some(res) = res_item_opt {
                if !matches!(res.0, ResourceType::BucketEmpty | ResourceType::BucketWater) {
                     *ctx.task = AssignedTask::None;
                     return;
                }
            }

            let bucket_pos = bucket_transform.translation.truncate();
            // let distance = ctx.soul_pos().distance(bucket_pos);
            let is_adjacent = {
                 let sg = WorldMap::world_to_grid(ctx.soul_pos());
                 let bg = WorldMap::world_to_grid(bucket_pos);
                 (sg.0 - bg.0).abs() <= 1 && (sg.1 - bg.1).abs() <= 1
            };

            // 1.8タイルの距離内、または隣接マスにいればピックアップ可能とする
            if crate::systems::soul_ai::task_execution::common::is_near_target(ctx.soul_pos(), bucket_pos) || is_adjacent {
                // バケツを拾う
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
                // インベントリに追加し、ワールドから消す
                ctx.inventory.0 = Some(bucket_entity);
                commands.entity(bucket_entity).insert(Visibility::Hidden);
                
                // 次のフェーズへの遷移ロジック...

                
                // 管理コンポーネントは維持する
                // commands.entity(bucket_entity).remove::<crate::systems::jobs::Designation>();
                // commands.entity(bucket_entity).remove::<crate::relationships::ManagedBy>();
                // commands.entity(bucket_entity).remove::<crate::systems::jobs::TaskSlots>();
                
                // 拾ったバケツの状態を確認
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
                        // 経路が見つからない
                         *ctx.task = AssignedTask::None;
                    }
                } else {
                    // 川が見つからない（ありえないはずだが）
                    *ctx.task = AssignedTask::None;
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
                *ctx.task = AssignedTask::None;
                return;
            }
            if ctx.inventory.0 == Some(bucket_entity) {
               commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
            }

            if ctx.soul_transform.translation.truncate().distance(ctx.dest.0) < 30.0 {
                // タンクが満タンかチェック
                let is_tank_full = if let Ok((_, _, stock, stored)) = q_stockpiles.get(tank_entity) {
                    let current = stored.map(|s| s.len()).unwrap_or(0);
                    current >= stock.capacity
                } else {
                    false
                };

                if is_tank_full {
                    *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::ReturningBucket,
                    });
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
                *ctx.task = AssignedTask::None;
                return;
            }
            
            // StoredIn関係の復元
            if ctx.inventory.0 == Some(bucket_entity) {
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
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
                        // 経路なし
                        *ctx.task = AssignedTask::None;
                    }
                } else {
                    *ctx.task = AssignedTask::None;
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
                *ctx.task = AssignedTask::None;
                return;
            }
            
            if ctx.inventory.0 == Some(bucket_entity) {
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
            }
            
            // タンクが満タンかチェック（移動中に満タンになる可能性）
            let is_tank_full = if let Ok((_, _, stock, stored)) = q_stockpiles.get(tank_entity) {
                let current = stored.map(|s| s.len()).unwrap_or(0);
                current >= stock.capacity
            } else {
                false
            };

            if is_tank_full {
                *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::ReturningBucket,
                });
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
                *ctx.task = AssignedTask::None;
                return;
            }
            
            // StoredIn関係の復元
            if ctx.inventory.0 == Some(bucket_entity) {
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
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

                 // タスク完了... ではなくバケツを戻しに行く
                 // 注：ここでバケツをドロップしてはいけない！保持したまま移動する
                 *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                     bucket: bucket_entity,
                     tank: tank_entity,
                     phase: GatherWaterPhase::ReturningBucket,
                 });
                 ctx.path.waypoints.clear(); // 目的地再計算のためクリア

                 // 搬送予約を解除
                 haul_cache.release(tank_entity);
             } else {
                 *ctx.task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                     bucket: bucket_entity,
                     tank: tank_entity,
                     phase: GatherWaterPhase::Pouring { progress: new_progress },
                 });
             }
         }
        GatherWaterPhase::ReturningBucket => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                trace!("GATHER_WATER: ReturningBucket cancelled - bucket not in inventory");
                *ctx.task = AssignedTask::None;
                return;
            }
            
            // タンクに紐付いたストレージ（置き場）を探す
            let mut best_storage = None;
            let mut min_dist_sq = f32::MAX;
            
            trace!("GATHER_WATER: Phase ReturningBucket. Searching for storage belonging to tank {:?}", tank_entity);

            for (s_entity, s_transform, stock, stored) in q_stockpiles.iter() {
                // タンクへの帰属チェック
                let belongs_to_tank = ctx.queries.belongs.get(s_entity).map(|b| b.0 == tank_entity).unwrap_or(false);
                
                let current = stored.map(|s| s.len()).unwrap_or(0);
                trace!("GATHER_WATER: Storage {:?}: belongs_to_tank={}, capacity={}/{}", s_entity, belongs_to_tank, current, stock.capacity);

                if !belongs_to_tank { continue; }
                
                // 容量チェック
                if current >= stock.capacity { 
                    trace!("GATHER_WATER: Storage {:?} is full ({}/{})", s_entity, current, stock.capacity);
                    continue; 
                }
                
                let s_pos = s_transform.translation.truncate();
                let dist_sq = soul_pos.distance_squared(s_pos);
                if dist_sq < min_dist_sq {
                    min_dist_sq = dist_sq;
                    best_storage = Some((s_entity, s_pos));
                }
            }
            
            if let Some((storage_entity, storage_pos)) = best_storage {
                let is_near = ctx.soul_pos().distance(storage_pos) < crate::constants::TILE_SIZE * 1.8;
                trace!("GATHER_WATER: Best storage is {:?} at {:?}. is_near: {}", storage_entity, storage_pos, is_near);
                
                if is_near {
                    // バケツを置く
                    trace!("GATHER_WATER: Dropping bucket {:?} into storage {:?}", bucket_entity, storage_entity);
                    commands.entity(bucket_entity).remove::<crate::relationships::StoredIn>();
                    ctx.inventory.0 = None;
                    
                    commands.entity(bucket_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(storage_pos.x, storage_pos.y, crate::constants::Z_ITEM_PICKUP),
                        crate::relationships::StoredIn(storage_entity),
                        crate::systems::logistics::InStockpile(storage_entity),
                    ));
                    
                    // タスク完了
                    commands.entity(ctx.soul_entity).remove::<crate::relationships::WorkingOn>();
                    *ctx.task = AssignedTask::None;
                    info!("GATHER_WATER: Successfully returned bucket {:?} to storage {:?}", bucket_entity, storage_entity);
                } else {
                    // 移動
                    trace!("GATHER_WATER: Moving to storage {:?} at {:?}", storage_entity, storage_pos);
                    if ctx.path.waypoints.is_empty() || ctx.dest.0.distance_squared(storage_pos) > 1.0 {
                        let start_grid = WorldMap::world_to_grid(ctx.soul_pos());
                        let target_grid = WorldMap::world_to_grid(storage_pos);
                        
                        if let Some(path) = crate::world::pathfinding::find_path(world_map, ctx.pf_context, start_grid, target_grid) {
                            ctx.dest.0 = storage_pos;
                            ctx.path.waypoints = path.iter().map(|&(x,y)| WorldMap::grid_to_world(x,y)).collect();
                            ctx.path.current_index = 0;
                        } else {
                            ctx.dest.0 = storage_pos;
                        }
                    }
                }
            } else {
                warn!("GATHER_WATER: No storage found for tank {:?}. Dropping on ground.", tank_entity);
                commands.entity(bucket_entity).remove::<crate::relationships::StoredIn>();
                ctx.inventory.0 = None;
                commands.entity(bucket_entity).insert((
                    Visibility::Visible,
                    Transform::from_xyz(ctx.soul_pos().x, ctx.soul_pos().y, crate::constants::Z_ITEM_PICKUP),
                ));
                commands.entity(ctx.soul_entity).remove::<crate::relationships::WorkingOn>();
                *ctx.task = AssignedTask::None;
            }
        }
    }
}
