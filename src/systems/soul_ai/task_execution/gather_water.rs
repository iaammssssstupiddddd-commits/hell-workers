
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
    q_targets: &Query<(
        &Transform,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&crate::systems::jobs::Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
        match phase {
        GatherWaterPhase::GoingToBucket => {
            // 既にインベントリにバケツがある場合は次のフェーズへ
            if ctx.inventory.0 == Some(bucket_entity) {
                // バケツが既にインベントリにあるので、川へ
                if let Some(river_grid) = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate()) {
                    let river_pos = WorldMap::grid_to_world(river_grid.0, river_grid.1);
                    *ctx.task = AssignedTask::GatherWater {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::GoingToRiver,
                    };
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
            if ctx.soul_transform.translation.truncate().distance(bucket_pos) < 20.0 {
                // バケツを拾う
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
                // インベントリに追加し、ワールドから消す
                ctx.inventory.0 = Some(bucket_entity);
                commands.entity(bucket_entity).insert(Visibility::Hidden);
                
                // 青いマスキング（DesignationIndicator）を削除
                // DesignationIndicatorは別のクエリで管理されているため、ここでは削除できない
                // 代わりに、Designationコンポーネントを削除することで、update_designation_indicator_systemが自動的に削除する
                commands.entity(bucket_entity).remove::<crate::systems::jobs::Designation>();
                
                // 次のフェーズへ：川へ
                if let Some(river_grid) = world_map.get_nearest_river_grid(ctx.soul_transform.translation.truncate()) {
                    let river_pos = WorldMap::grid_to_world(river_grid.0, river_grid.1);
                    *ctx.task = AssignedTask::GatherWater {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::GoingToRiver,
                    };
                    ctx.dest.0 = river_pos;
                    ctx.path.waypoints = vec![river_pos];
                    ctx.path.current_index = 0;
                } else {
                    // 川が見つからない（ありえないはずだが）
                    *ctx.task = AssignedTask::None;
                }
            } else {
                ctx.dest.0 = bucket_pos;
                // 到達チェックは movement システム側に任せる
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
                // 水を汲むフェーズへ
                *ctx.task = AssignedTask::GatherWater {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Filling { progress: 0.0 },
                };
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
                    *ctx.task = AssignedTask::GatherWater {
                        bucket: bucket_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::GoingToTank,
                    };
                    ctx.dest.0 = tank_pos;
                    ctx.path.waypoints = vec![tank_pos];
                    ctx.path.current_index = 0;
                } else {
                    *ctx.task = AssignedTask::None;
                }
            } else {
                *ctx.task = AssignedTask::GatherWater {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Filling { progress: new_progress },
                };
            }
        }
        GatherWaterPhase::GoingToTank => {
            // バケツがインベントリにあるか確認
            if ctx.inventory.0 != Some(bucket_entity) {
                *ctx.task = AssignedTask::None;
                return;
            }
            
            // StoredIn関係の復元
            if ctx.inventory.0 == Some(bucket_entity) {
                commands.entity(bucket_entity).insert(crate::relationships::StoredIn(ctx.soul_entity));
            }
            
            if ctx.soul_transform.translation.truncate().distance(ctx.dest.0) < 40.0 { // 2x2なので少し広めに
                *ctx.task = AssignedTask::GatherWater {
                    bucket: bucket_entity,
                    tank: tank_entity,
                    phase: GatherWaterPhase::Pouring { progress: 0.0 },
                };
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

                 // バケツを置く（インベントリから削除し、ワールドに表示）
                 commands.entity(bucket_entity).remove::<crate::relationships::StoredIn>();
                 ctx.inventory.0 = None;
                 // バケツの状態（空）を保持してdrop時に反映
                 // ResourceItem(ResourceType::BucketEmpty)は既に設定済み
                 commands.entity(bucket_entity).insert(Visibility::Visible);
                 // バケツを現在位置に配置
                 let drop_pos = ctx.soul_transform.translation.truncate();
                 commands.entity(bucket_entity).insert(Transform::from_xyz(
                     drop_pos.x,
                     drop_pos.y,
                     crate::constants::Z_ITEM_PICKUP,
                 ));

                 // タンクの中身を増やす
                 // Stockpile コンポーネントの StoredItems を増やす必要があるが、
                 // 面倒なので単にダミーアイテムをタンク位置にspawnしてStoredIn(tank)にする
                 commands.spawn((
                     ResourceItem(ResourceType::Water),
                     crate::relationships::StoredIn(tank_entity),
                     Visibility::Hidden, // タンクの中なので見えない
                 ));

                 // タスク完了
                 *ctx.task = AssignedTask::None;
             } else {
                 *ctx.task = AssignedTask::GatherWater {
                     bucket: bucket_entity,
                     tank: tank_entity,
                     phase: GatherWaterPhase::Pouring { progress: new_progress },
                 };
             }
        }
    }
}
