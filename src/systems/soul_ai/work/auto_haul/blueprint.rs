//! Blueprint auto-haul system
//!
//! Automatically creates haul tasks for materials needed by blueprints.

use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::entities::familiar::ActiveCommand;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::query_types::AutoHaulAssignedTaskQuery;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{BlueprintSpatialGrid, ResourceSpatialGrid, SpatialGridOps};
use crate::relationships::TaskWorkers;

/// 段階的検索の半径（タイル単位）
const SEARCH_RADII: [f32; 4] = [20.0, 50.0, 100.0, 200.0];

/// 設計図への自動資材運搬タスク生成システム
pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_resources: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::relationships::StoredIn>,
        ),
        (Without<Designation>, Without<TaskWorkers>),
    >,
    q_stockpiles: Query<&Transform, With<crate::systems::logistics::Stockpile>>,
    q_souls: AutoHaulAssignedTaskQuery,
    q_all_resources: Query<&ResourceItem>,
    q_reserved_items: Query<
        (&ResourceItem, &crate::systems::jobs::TargetBlueprint),
        With<Designation>,
    >,
) {
    // 1. 集計フェーズ: 各設計図への「運搬中」および「予約済み」の数を集計
    // (BlueprintEntity, ResourceType) -> Count
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    // 運搬中 (ソウルが持っている、または向かっている)
    for task in q_souls.iter() {
        let task: &AssignedTask = task;
        if let AssignedTask::HaulToBlueprint(data) = task
        {
            let item = &data.item;
            let blueprint = &data.blueprint;
            if let Ok(res_item) = q_all_resources.get(*item) {
                *in_flight.entry((*blueprint, res_item.0)).or_insert(0) += 1;
            }
        }
    }

    // 予約済み (Designation はあるが、まだソウルに割り当てられていないアイテム)
    for (res_item, target_bp) in q_reserved_items.iter() {
        *in_flight.entry((target_bp.0, res_item.0)).or_insert(0) += 1;
    }

    let mut already_assigned_this_frame = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        let (fam_entity, _active_command, task_area): (Entity, &ActiveCommand, &TaskArea) = (fam_entity, _active_command, task_area);

        // 最適化: タスクエリア内のブループリントのみを取得
        let blueprints_in_area = blueprint_grid.get_in_area(task_area.min, task_area.max);

        for bp_entity in blueprints_in_area {
            // クエリで詳細データを取得
            let Ok((_, bp_transform, blueprint, workers_opt)) = q_blueprints.get(bp_entity) else {
                continue;
            };
            let bp_pos = bp_transform.translation.truncate();

            // 既に作業員が割り当てられている場合はスキップ（建築中）
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            // 資材が揃っている場合はスキップ（建築タスクへ遷移）
            if blueprint.materials_complete() {
                continue;
            }

            // 必要な資材タイプを探す
            for (resource_type, &required) in &blueprint.required_materials {
                let delivered = *blueprint
                    .delivered_materials
                    .get(resource_type)
                    .unwrap_or(&0);
                let inflight_count = *in_flight.get(&(bp_entity, *resource_type)).unwrap_or(&0);

                // 配達済み + 運搬中 + 予約済み >= 必要数 ならこれ以上探さない
                if delivered + inflight_count as u32 >= required {
                    continue;
                }

                // 段階的に検索範囲を広げて資材を探す
                let matching_resource = find_resource_progressively(
                    bp_pos,
                    *resource_type,
                    task_area,
                    &resource_grid,
                    &q_resources,
                    &q_stockpiles,
                    &already_assigned_this_frame,
                );

                if let Some(item_entity) = matching_resource {
                    already_assigned_this_frame.insert(item_entity);
                    // 次の集計に備えてカウントアップ（同フレーム内の別使い魔が重複させないため）
                    *in_flight.entry((bp_entity, *resource_type)).or_insert(0) += 1;

                    // Designation を付与
                    commands.entity(item_entity).insert((
                        Designation {
                            work_type: WorkType::Haul,
                        },
                        IssuedBy(fam_entity),
                        TaskSlots::new(1),
                        crate::systems::jobs::TargetBlueprint(bp_entity),
                        crate::systems::jobs::Priority(0),
                    ));

                    info!(
                        "AUTO_HAUL_BP: Assigned {:?} for bp {:?} (Total expected: {})",
                        resource_type,
                        bp_entity,
                        delivered + inflight_count as u32 + 1
                    );

                    // 1つ割り当てたら、この Blueprint のこのリソースについては一旦終了して次のリソースまたは次の設計図へ
                }
            }
        }
    }
}

/// 設計図の位置から段階的に検索範囲を広げて資材を探す
///
/// 1. まず近い範囲から検索
/// 2. 見つからなければ範囲を広げる
/// 3. TaskArea内はストックパイルチェックを適用
/// 4. TaskArea外の資材も検索対象に含める（ただしストックパイル外の資材のみ）
fn find_resource_progressively(
    bp_pos: Vec2,
    resource_type: ResourceType,
    task_area: &TaskArea,
    resource_grid: &ResourceSpatialGrid,
    q_resources: &Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::relationships::StoredIn>,
        ),
        (Without<Designation>, Without<TaskWorkers>),
    >,
    q_stockpiles: &Query<&Transform, With<crate::systems::logistics::Stockpile>>,
    already_assigned: &std::collections::HashSet<Entity>,
) -> Option<Entity> {
    // 段階的に検索範囲を広げる
    for &radius_tiles in &SEARCH_RADII {
        let search_radius = TILE_SIZE * radius_tiles;
        let nearby_resources = resource_grid.get_nearby_in_radius(bp_pos, search_radius);

        // 距離でソートして近いものから優先
        let mut candidates: Vec<(Entity, f32)> = nearby_resources
            .iter()
            .filter(|&&entity| !already_assigned.contains(&entity))
            .filter_map(|&entity| {
                let Ok((_, transform, visibility, res_item, stored_in_opt)) =
                    q_resources.get(entity)
                else {
                    return None;
                };
                
                if *visibility == Visibility::Hidden {
                    return None;
                }
                if res_item.0 != resource_type {
                    return None;
                }

                let item_pos = transform.translation.truncate();

                // ストックパイル内にある場合のチェック
                if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                    if let Ok(stock_transform) = q_stockpiles.get(*stock_entity) {
                        let stock_pos = stock_transform.translation.truncate();
                        // ストックパイルがTaskArea外なら除外（他の使い魔の管轄）
                        if !task_area.contains(stock_pos) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                // ストックパイル外の資材は位置に関わらず利用可能

                let dist_sq = item_pos.distance_squared(bp_pos);
                Some((entity, dist_sq))
            })
            .collect();

        // 距離でソート
        candidates.sort_by(|(_, d1), (_, d2)| {
            d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
        });

        // 最も近い資材を返す
        if let Some((entity, _)) = candidates.first() {
            return Some(*entity);
        }
    }

    None
}
