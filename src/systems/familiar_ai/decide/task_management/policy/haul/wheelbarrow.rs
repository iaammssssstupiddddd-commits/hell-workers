//! 手押し車による一括運搬のポリシーロジック

use crate::constants::*;
use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

use super::super::super::validator::source_not_reserved;

/// タスク位置に最も近い利用可能な手押し車を検索
pub fn find_nearest_wheelbarrow(
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .wheelbarrows
        .iter()
        .filter(|(wb_entity, _)| source_not_reserved(*wb_entity, queries, shadow))
        .min_by(|(_, t1), (_, t2)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _)| e)
}

/// 指定アイテムの近くにある、手押し車に積載可能な未予約 Haul アイテムを収集
pub fn collect_nearby_haulable_items(
    primary_item: Entity,
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Vec<Entity> {
    let search_radius_sq = (TILE_SIZE * 10.0) * (TILE_SIZE * 10.0);

    let mut items: Vec<(Entity, f32)> = queries
        .designation
        .designations
        .iter()
        .filter_map(
            |(entity, transform, designation, _, _, task_workers, _, _)| {
                if designation.work_type != WorkType::Haul {
                    return None;
                }
                if task_workers.is_some_and(|tw| !tw.is_empty()) {
                    return None;
                }
                if !source_not_reserved(entity, queries, shadow) {
                    return None;
                }
                let item_type = queries.items.get(entity).ok().map(|(it, _)| it.0)?;
                if !item_type.is_loadable() {
                    return None;
                }
                let pos = transform.translation.truncate();
                let dist_sq = pos.distance_squared(task_pos);
                if dist_sq > search_radius_sq {
                    return None;
                }
                Some((entity, dist_sq))
            },
        )
        .collect();

    items.sort_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal));

    let mut result: Vec<Entity> = Vec::new();
    result.push(primary_item);
    for (entity, _) in items {
        if entity == primary_item {
            continue;
        }
        result.push(entity);
    }

    result
}

/// ストックパイルの残り容量を計算
pub fn remaining_stockpile_capacity(
    stockpile: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> usize {
    if let Ok((_, _, stock, stored)) = queries.storage.stockpiles.get(stockpile) {
        let current = stored.map(|s| s.len()).unwrap_or(0);
        let reserved = queries
            .reservation
            .resource_cache
            .get_destination_reservation(stockpile)
            + shadow.destination_reserved(stockpile);
        let used = current + reserved;
        if used >= stock.capacity {
            0
        } else {
            stock.capacity - used
        }
    } else {
        0
    }
}
