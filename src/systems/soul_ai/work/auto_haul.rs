use bevy::prelude::*;

use crate::constants::*;
use crate::entities::familiar::ActiveCommand;
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Blueprint, Designation, IssuedBy, TaskSlots, WorkType,
};
use crate::systems::logistics::{ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};

/// 指揮エリア内での自動運搬タスク生成システム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources: Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::jobs::TargetBlueprint>,
        ),
    >,
) {
    let mut already_assigned = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        let (fam_entity, _active_command, task_area): (Entity, &ActiveCommand, &TaskArea) = (fam_entity, _active_command, task_area);
        for (stock_transform, stockpile, stored_items_opt) in q_stockpiles.iter() {
            let (stock_transform, stockpile, stored_items_opt): (&Transform, &Stockpile, Option<&crate::relationships::StoredItems>) = (stock_transform, stockpile, stored_items_opt);
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            if current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            let nearest_resource = nearby_resources
                .iter()
                .filter(|&&entity| !already_assigned.contains(&entity))
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility, res_item)) = q_resources.get(entity) else {
                        return None;
                    };
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    if let Some(target_type) = stockpile.resource_type {
                        if res_item.0 != target_type {
                            return None;
                        }
                    }

                    let dist_sq = transform.translation.truncate().distance_squared(stock_pos);
                    if dist_sq < search_radius * search_radius {
                        Some((entity, dist_sq))
                    } else {
                        None
                    }
                })
                .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(entity, _)| entity);

            if let Some(item_entity) = nearest_resource {
                already_assigned.insert(item_entity);
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                    crate::systems::jobs::Priority(0),
                ));
            }
        }
    }
}

/// 設計図への自動資材運搬タスク生成システム
pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
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
    q_souls: Query<&AssignedTask>,
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
        // エリア内の Blueprint を探す
        for (bp_entity, bp_transform, blueprint, workers_opt) in q_blueprints.iter() {
            let (bp_entity, bp_transform, blueprint, workers_opt): (Entity, &Transform, &Blueprint, Option<&TaskWorkers>) = (bp_entity, bp_transform, blueprint, workers_opt);
            let bp_pos = bp_transform.translation.truncate();
            if !task_area.contains(bp_pos) {
                continue;
            }

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

                // 近くの対応する資材を探す
                let search_radius = TILE_SIZE * 20.0;
                let nearby_resources = resource_grid.get_nearby_in_radius(bp_pos, search_radius);

                // まだ Designation が付いていないものから探す
                let matching_resource = nearby_resources
                    .iter()
                    .filter(|&&entity| !already_assigned_this_frame.contains(&entity))
                    .filter_map(|&entity| {
                        let Ok((_, transform, visibility, res_item, stored_in_opt)) =
                            q_resources.get(entity)
                        else {
                            return None;
                        };
                        if *visibility == Visibility::Hidden {
                            return None;
                        }
                        if res_item.0 != *resource_type {
                            return None;
                        }

                        // ストックパイル内にある場合、そのストックパイルが主のタスクエリア内にあるかチェック
                        if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                            if let Ok(stock_transform) = q_stockpiles.get(*stock_entity) {
                                let stock_pos = stock_transform.translation.truncate();
                                if !task_area.contains(stock_pos) {
                                    return None;
                                }
                            } else {
                                // ストックパイルが見つからない（消失している）場合は地上扱いとするか除外するか
                                // 基本的には StoredIn があるなら存在するはず
                                return None;
                            }
                        }

                        let dist_sq = transform.translation.truncate().distance_squared(bp_pos);
                        if dist_sq < search_radius * search_radius {
                            Some((entity, dist_sq))
                        } else {
                            None
                        }
                    })
                    .min_by(|(_, d1), (_, d2)| {
                        d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(entity, _)| entity);

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
                    // (複数同時に探すと1フレームで一気に割り当てすぎてしまう可能性があるが、集計ロジックがあるので基本安全)
                }
            }
        }
    }
}
