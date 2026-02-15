//! 運搬タスクのソースアイテム探索

use crate::systems::familiar_ai::decide::task_management::{
    ReservationShadow,
    validator::source_not_reserved,
};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

type TaskQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;

/// 共通: target_pos に最も近い未予約アイテムを検索（条件差分は extra_filter で指定）
fn find_nearest_source_item<'w, 's>(
    resource_type: ResourceType,
    target_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
    extra_filter: impl Fn(Entity) -> bool,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .filter(|(entity, _, _, _)| extra_filter(*entity))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(target_pos);
            let d2 = t2.translation.truncate().distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

pub fn find_nearest_mixer_source_item<'w, 's>(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    find_nearest_source_item(item_type, mixer_pos, queries, shadow, |_| true)
}

pub fn find_nearest_stockpile_source_item<'w, 's>(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    let extra_filter = |entity: Entity| {
        queries
            .designation
            .targets
            .get(entity)
            .ok()
            .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none())
            && {
                let belongs = queries.designation.belongs.get(entity).ok().map(|b| b.0);
                item_owner == belongs
            }
    };
    find_nearest_source_item(resource_type, stock_pos, queries, shadow, extra_filter)
}

pub fn find_fixed_stockpile_source_item<'w, 's>(
    source_item: Entity,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    if !source_not_reserved(source_item, queries, shadow) {
        return None;
    }

    let (transform, _, _, _, resource_opt, _, stored_in_opt) =
        queries.designation.targets.get(source_item).ok()?;
    if stored_in_opt.is_some() {
        return None;
    }
    if !resource_opt.is_some_and(|res| res.0 == resource_type) {
        return None;
    }

    let owner = queries.designation.belongs.get(source_item).ok().map(|b| b.0);
    if owner != item_owner {
        return None;
    }

    Some((source_item, transform.translation.truncate()))
}

pub fn find_nearest_blueprint_source_item<'w, 's>(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    find_nearest_source_item(resource_type, bp_pos, queries, shadow, |_| true)
}

/// ドナーセルから未予約のアイテムを1つ検索する（統合用）。
/// 最少格納のドナーセルから優先的に選択（空にしやすくする）。
pub fn find_consolidation_source_item<'w, 's>(
    resource_type: ResourceType,
    donor_cells: &[Entity],
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    // ドナーセルごとに格納数を取得してソート（最少格納優先）
    let mut donor_with_count: Vec<(Entity, usize)> = donor_cells
        .iter()
        .filter_map(|&cell| {
            let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(cell).ok()?;
            let stored = stored_opt.map(|s| s.len()).unwrap_or(0);
            if stored > 0
                && (stock.resource_type.is_none() || stock.resource_type == Some(resource_type))
            {
                Some((cell, stored))
            } else {
                None
            }
        })
        .collect();
    donor_with_count.sort_by_key(|(_, count)| *count);

    // 最少格納セルから順にアイテムを探す
    for (cell, _) in donor_with_count {
        let found = queries
            .stored_items_query
            .iter()
            .filter(|(_, res, in_stockpile)| {
                res.0 == resource_type && in_stockpile.0 == cell
            })
            .filter(|(entity, _, _)| {
                crate::systems::familiar_ai::decide::task_management::validator::source_not_reserved(
                    *entity, queries, shadow,
                )
            })
            .next();

        if let Some((entity, _, _)) = found {
            // アイテムの位置はセルの位置を使用
            let pos = queries
                .storage
                .stockpiles
                .get(cell)
                .map(|(_, t, _, _)| t.translation.truncate())
                .unwrap_or(Vec2::ZERO);
            return Some((entity, pos));
        }
    }
    None
}

/// center_pos 付近の未予約アイテムを最寄り順に最大 max_count 個収集する。
/// 探索範囲は TILE_SIZE * 10.0。
pub fn collect_nearby_items_for_wheelbarrow(
    resource_type: ResourceType,
    center_pos: Vec2,
    max_count: usize,
    queries: &TaskQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Vec<(Entity, Vec2)> {
    collect_items_for_wheelbarrow_in_radius(
        resource_type,
        center_pos,
        max_count,
        queries,
        shadow,
        Some(crate::constants::TILE_SIZE * 10.0),
    )
}

pub fn collect_items_for_wheelbarrow_unbounded(
    resource_type: ResourceType,
    center_pos: Vec2,
    max_count: usize,
    queries: &TaskQueries<'_, '_>,
    shadow: &ReservationShadow,
) -> Vec<(Entity, Vec2)> {
    collect_items_for_wheelbarrow_in_radius(
        resource_type,
        center_pos,
        max_count,
        queries,
        shadow,
        None,
    )
}

fn collect_items_for_wheelbarrow_in_radius(
    resource_type: ResourceType,
    center_pos: Vec2,
    max_count: usize,
    queries: &TaskQueries<'_, '_>,
    shadow: &ReservationShadow,
    search_radius: Option<f32>,
) -> Vec<(Entity, Vec2)> {
    let search_radius_sq = search_radius.map(|r| r * r);

    let mut items: Vec<(Entity, Vec2, f32)> = queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .filter_map(|(entity, transform, _, _)| {
            let pos = transform.translation.truncate();
            let dist_sq = pos.distance_squared(center_pos);
            if search_radius_sq.is_some_and(|radius_sq| dist_sq > radius_sq) {
                return None;
            }
            Some((entity, pos, dist_sq))
        })
        .collect();

    items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    items
        .into_iter()
        .take(max_count)
        .map(|(e, pos, _)| (e, pos))
        .collect()
}
