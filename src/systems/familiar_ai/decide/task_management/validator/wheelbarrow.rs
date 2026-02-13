//! M6: 手押し車バッチ運搬の request resolver
//!
//! DepositToStockpile request に対して、手押し車 + 積載可能アイテムのバッチを遅延解決する。

use bevy::prelude::*;

use crate::constants::{WHEELBARROW_CAPACITY, WHEELBARROW_MIN_BATCH_SIZE};
use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;

use super::reservation::source_not_reserved;

/// ストックパイル向けの手押し車バッチを解決する。
/// Returns (wheelbarrow, items) when batch is viable.
pub fn resolve_wheelbarrow_batch_for_stockpile(
    stockpile: Entity,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    task_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec<Entity>)> {
    if !resource_type.is_loadable() {
        return None;
    }

    let wb_entity = find_nearest_wheelbarrow(task_pos, queries, shadow)?;
    let items = collect_free_items_for_stockpile(
        resource_type,
        item_owner,
        task_pos,
        stockpile,
        queries,
        shadow,
    )?;

    if items.len() < WHEELBARROW_MIN_BATCH_SIZE {
        return None;
    }

    Some((wb_entity, items))
}

/// アイテム群の重心を計算（積み込み地点として使用）
/// Designation 有無どちらのアイテムでも取得可能
pub fn compute_centroid(
    items: &[Entity],
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Vec2 {
    let mut sum = Vec2::ZERO;
    let mut count = 0;
    for &item in items {
        let pos = queries
            .designation
            .designations
            .get(item)
            .ok()
            .map(|(_, t, _, _, _, _, _, _)| t.translation.truncate())
            .or_else(|| {
                queries
                    .free_resource_items
                    .get(item)
                    .ok()
                    .map(|(_, t, _, _)| t.translation.truncate())
            });
        if let Some(p) = pos {
            sum += p;
            count += 1;
        }
    }
    if count > 0 {
        sum / count as f32
    } else {
        Vec2::ZERO
    }
}

fn find_nearest_wheelbarrow(
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

fn collect_free_items_for_stockpile(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    task_pos: Vec2,
    stockpile: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Vec<Entity>> {
    let dest_capacity = remaining_stockpile_capacity(stockpile, queries, shadow);
    let max_items = dest_capacity.min(WHEELBARROW_CAPACITY);
    if max_items < WHEELBARROW_MIN_BATCH_SIZE {
        return None;
    }

    let search_radius_sq = (crate::constants::TILE_SIZE * 10.0) * (crate::constants::TILE_SIZE * 10.0);

    let mut candidates: Vec<(Entity, f32)> = queries
        .free_resource_items
        .iter()
        .filter(|(_, _, vis, res)| {
            *vis != Visibility::Hidden && res.0 == resource_type && res.0.is_loadable()
        })
        .filter(|(e, _, _, _)| source_not_reserved(*e, queries, shadow))
        // DepositToStockpile request は地面アイテムのみ対象にする。
        .filter(|(e, _, _, _)| {
            queries
                .designation
                .targets
                .get(*e)
                .ok()
                .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none())
        })
        .filter(|(e, _, _, _)| {
            let belongs = queries.designation.belongs.get(*e).ok().map(|b| b.0);
            item_owner == belongs
        })
        .map(|(e, t, _, _)| {
            let d = t.translation.truncate().distance_squared(task_pos);
            (e, d)
        })
        .filter(|(_, d)| *d <= search_radius_sq)
        .collect();

    candidates.sort_by(|(_, d1): &(Entity, f32), (_, d2): &(Entity, f32)| {
        d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
    });

    let items: Vec<Entity> = candidates
        .into_iter()
        .take(max_items)
        .map(|(e, _)| e)
        .collect();

    if items.len() >= WHEELBARROW_MIN_BATCH_SIZE {
        Some(items)
    } else {
        None
    }
}

fn remaining_stockpile_capacity(
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
