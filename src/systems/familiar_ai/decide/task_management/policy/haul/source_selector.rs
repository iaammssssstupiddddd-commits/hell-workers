//! 運搬タスクのソースアイテム探索

use crate::systems::familiar_ai::decide::task_management::{
    CachedSourceItem, ReservationShadow, SourceSelectorFrameCache, validator::source_not_reserved,
};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

type TaskQueries<'w, 's> =
    crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>;

static SOURCE_SELECTOR_CALLS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);

fn mark_source_selector_call() {
    SOURCE_SELECTOR_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn mark_scanned_item() {
    SOURCE_SELECTOR_SCANNED_ITEMS.fetch_add(1, Ordering::Relaxed);
}

/// source_selector 系の走査カウンタを読み出し、内部カウンタをリセットする。
pub(crate) fn take_source_selector_scan_snapshot() -> (u32, u32) {
    (
        SOURCE_SELECTOR_CALLS.swap(0, Ordering::Relaxed),
        SOURCE_SELECTOR_SCANNED_ITEMS.swap(0, Ordering::Relaxed),
    )
}

fn ensure_frame_cache<'w, 's>(queries: &TaskQueries<'w, 's>, shadow: &mut ReservationShadow) {
    if shadow.source_selector_cache.is_some() {
        return;
    }

    let mut cache = SourceSelectorFrameCache::default();

    // free_resource_items をフレーム内で一度だけ走査し、用途別に索引化する。
    for (entity, transform, visibility, resource_item) in queries.free_resource_items.iter() {
        mark_scanned_item();
        if *visibility == Visibility::Hidden {
            continue;
        }

        let source = CachedSourceItem {
            entity,
            pos: transform.translation.truncate(),
        };

        cache
            .by_resource
            .entry(resource_item.0)
            .or_insert_with(Vec::new)
            .push(source);

        let is_ground = queries
            .designation
            .targets
            .get(entity)
            .ok()
            .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none());
        if is_ground {
            let owner = queries.designation.belongs.get(entity).ok().map(|b| b.0);
            cache
                .by_resource_owner_ground
                .entry((resource_item.0, owner))
                .or_insert_with(Vec::new)
                .push(source);
        }
    }

    shadow.source_selector_cache = Some(cache);
}

fn cached_items_by_resource(
    resource_type: ResourceType,
    shadow: &ReservationShadow,
) -> &[CachedSourceItem] {
    shadow
        .source_selector_cache
        .as_ref()
        .and_then(|cache| cache.by_resource.get(&resource_type))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn cached_ground_items_by_resource_owner(
    resource_type: ResourceType,
    owner: Option<Entity>,
    shadow: &ReservationShadow,
) -> &[CachedSourceItem] {
    shadow
        .source_selector_cache
        .as_ref()
        .and_then(|cache| cache.by_resource_owner_ground.get(&(resource_type, owner)))
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// 共通: target_pos に最も近い未予約アイテムを検索（条件差分は extra_filter で指定）
fn find_nearest_source_item<'w, 's>(
    sources: &[CachedSourceItem],
    target_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
    extra_filter: impl Fn(Entity) -> bool,
) -> Option<(Entity, Vec2)> {
    sources
        .iter()
        .inspect(|_| mark_scanned_item())
        .filter(|source| source_not_reserved(source.entity, queries, shadow))
        .filter(|source| extra_filter(source.entity))
        .min_by(|s1, s2| {
            let d1 = s1.pos.distance_squared(target_pos);
            let d2 = s2.pos.distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|source| (source.entity, source.pos))
}

pub fn find_nearest_mixer_source_item<'w, 's>(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    ensure_frame_cache(queries, shadow);
    let sources = cached_items_by_resource(item_type, shadow);
    find_nearest_source_item(sources, mixer_pos, queries, shadow, |_| true)
}

pub fn find_nearest_stockpile_source_item<'w, 's>(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    ensure_frame_cache(queries, shadow);
    let sources = cached_ground_items_by_resource_owner(resource_type, item_owner, shadow);
    find_nearest_source_item(sources, stock_pos, queries, shadow, |_| true)
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

    let owner = queries
        .designation
        .belongs
        .get(source_item)
        .ok()
        .map(|b| b.0);
    if owner != item_owner {
        return None;
    }

    Some((source_item, transform.translation.truncate()))
}

pub fn find_nearest_blueprint_source_item<'w, 's>(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    ensure_frame_cache(queries, shadow);
    let sources = cached_items_by_resource(resource_type, shadow);
    find_nearest_source_item(sources, bp_pos, queries, shadow, |_| true)
}

/// ドナーセルから未予約のアイテムを1つ検索する（統合用）。
/// 最少格納のドナーセルから優先的に選択（空にしやすくする）。
pub fn find_consolidation_source_item<'w, 's>(
    resource_type: ResourceType,
    donor_cells: &[Entity],
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
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
            .inspect(|_| mark_scanned_item())
            .filter(|(_, res, in_stockpile)| res.0 == resource_type && in_stockpile.0 == cell)
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
    shadow: &mut ReservationShadow,
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
    shadow: &mut ReservationShadow,
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
    shadow: &mut ReservationShadow,
    search_radius: Option<f32>,
) -> Vec<(Entity, Vec2)> {
    mark_source_selector_call();
    let search_radius_sq = search_radius.map(|r| r * r);
    ensure_frame_cache(queries, shadow);
    let sources = cached_items_by_resource(resource_type, shadow);

    let mut items: Vec<(Entity, Vec2, f32)> = sources
        .iter()
        .inspect(|_| mark_scanned_item())
        .filter(|source| source_not_reserved(source.entity, queries, shadow))
        .filter_map(|source| {
            let dist_sq = source.pos.distance_squared(center_pos);
            if search_radius_sq.is_some_and(|radius_sq| dist_sq > radius_sq) {
                return None;
            }
            Some((source.entity, source.pos, dist_sq))
        })
        .collect();

    items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    items
        .into_iter()
        .take(max_count)
        .map(|(e, pos, _)| (e, pos))
        .collect()
}
