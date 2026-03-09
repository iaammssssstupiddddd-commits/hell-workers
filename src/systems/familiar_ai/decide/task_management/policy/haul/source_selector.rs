//! 運搬タスクのソースアイテム探索

use crate::systems::familiar_ai::decide::task_management::{
    CachedSourceItem, ReservationShadow, SourceSelectorFrameCache, validator::source_not_reserved,
};
use crate::systems::logistics::ResourceType;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};
use bevy::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

type TaskQueries<'w, 's> =
    crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries<'w, 's>;

static SOURCE_SELECTOR_CALLS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);
static SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS: AtomicU32 = AtomicU32::new(0);

fn mark_source_selector_call() {
    SOURCE_SELECTOR_CALLS.fetch_add(1, Ordering::Relaxed);
}

fn mark_cache_build_scanned_item() {
    SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS.fetch_add(1, Ordering::Relaxed);
}

fn mark_candidate_scanned_item() {
    SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS.fetch_add(1, Ordering::Relaxed);
}

/// source_selector 系の走査カウンタを読み出し、内部カウンタをリセットする。
pub(crate) fn take_source_selector_scan_snapshot() -> (u32, u32, u32) {
    (
        SOURCE_SELECTOR_CALLS.swap(0, Ordering::Relaxed),
        SOURCE_SELECTOR_CACHE_BUILD_SCANNED_ITEMS.swap(0, Ordering::Relaxed),
        SOURCE_SELECTOR_CANDIDATE_SCANNED_ITEMS.swap(0, Ordering::Relaxed),
    )
}

fn ensure_frame_cache<'w, 's>(queries: &TaskQueries<'w, 's>, shadow: &mut ReservationShadow) {
    if shadow.source_selector_cache.is_some() {
        return;
    }

    let mut cache = SourceSelectorFrameCache::default();

    // stockpile 内アイテムの探索を高速化するため、(resource_type, cell) ごとに索引化する。
    for (entity, resource_item, in_stockpile) in queries.stored_items_query.iter() {
        mark_cache_build_scanned_item();
        cache
            .by_resource_stockpile
            .entry((resource_item.0, in_stockpile.0))
            .or_insert_with(Vec::new)
            .push(entity);
    }

    shadow.source_selector_cache = Some(cache);
}

fn cached_stockpile_items_by_resource(
    resource_type: ResourceType,
    stockpile: Entity,
    shadow: &ReservationShadow,
) -> &[Entity] {
    shadow
        .source_selector_cache
        .as_ref()
        .and_then(|cache| cache.by_resource_stockpile.get(&(resource_type, stockpile)))
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
        .inspect(|_| mark_candidate_scanned_item())
        .filter(|source| source_not_reserved(source.entity, queries, shadow))
        .filter(|source| extra_filter(source.entity))
        .min_by(|s1, s2| {
            let d1 = s1.pos.distance_squared(target_pos);
            let d2 = s2.pos.distance_squared(target_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|source| (source.entity, source.pos))
}

fn nearest_ground_source_with_grid<'w, 's>(
    resource_type: ResourceType,
    target_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
    owner_filter: Option<Option<Entity>>,
) -> Option<(Entity, Vec2)> {
    let max_map_tiles = hw_core::constants::MAP_WIDTH.max(hw_core::constants::MAP_HEIGHT) as f32;
    let search_radii = [
        hw_core::constants::TILE_SIZE * 10.0,
        hw_core::constants::TILE_SIZE * 20.0,
        hw_core::constants::TILE_SIZE * 40.0,
        hw_core::constants::TILE_SIZE * 80.0,
        hw_core::constants::TILE_SIZE * max_map_tiles,
    ];

    for radius in search_radii {
        let mut nearby_sources = Vec::new();
        resource_grid.get_nearby_in_radius_into(target_pos, radius, &mut nearby_sources);
        if nearby_sources.is_empty() {
            continue;
        }

        let mut candidates = Vec::new();
        for entity in nearby_sources {
            mark_candidate_scanned_item();
            let Ok((
                transform,
                _tree_opt,
                _tree_variant_opt,
                _rock_opt,
                resource_opt,
                _,
                stored_in_opt,
            )) = queries.designation.targets.get(entity)
            else {
                continue;
            };
            if stored_in_opt.is_some() {
                continue;
            }
            if !resource_opt.is_some_and(|res| res.0 == resource_type) {
                continue;
            }
            if !source_not_reserved(entity, queries, shadow) {
                continue;
            }
            if let Some(expected_owner) = owner_filter {
                let owner = queries.designation.belongs.get(entity).ok().map(|b| b.0);
                let owner_compatible = match expected_owner {
                    Some(owner_entity) => owner == Some(owner_entity),
                    None => owner.is_none(),
                };
                if !owner_compatible {
                    continue;
                }
            }

            candidates.push(CachedSourceItem {
                entity,
                pos: transform.translation.truncate(),
            });
        }

        if let Some(found) =
            find_nearest_source_item(&candidates, target_pos, queries, shadow, |_| true)
        {
            return Some(found);
        }
    }

    None
}

pub fn find_nearest_mixer_source_item<'w, 's>(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    nearest_ground_source_with_grid(item_type, mixer_pos, queries, shadow, resource_grid, None)
}

pub fn find_nearest_stockpile_source_item<'w, 's>(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    let owned_match = nearest_ground_source_with_grid(
        resource_type,
        stock_pos,
        queries,
        shadow,
        resource_grid,
        Some(item_owner),
    );
    if owned_match.is_some() || item_owner.is_none() {
        return owned_match;
    }

    // owner付きストックパイルでは、同ownerの地面資源がない場合に限り
    // owner未設定資源をフォールバック候補として扱う。
    nearest_ground_source_with_grid(
        resource_type,
        stock_pos,
        queries,
        shadow,
        resource_grid,
        Some(None),
    )
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
    let owner_compatible = owner == item_owner || (owner.is_none() && item_owner.is_some());
    if !owner_compatible {
        return None;
    }

    Some((source_item, transform.translation.truncate()))
}

pub fn find_nearest_blueprint_source_item<'w, 's>(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    nearest_ground_source_with_grid(resource_type, bp_pos, queries, shadow, resource_grid, None)
}

/// ドナーセルから未予約のアイテムを1つ検索する（統合用）。
/// 最少格納のドナーセルから優先的に選択（空にしやすくする）。
pub fn find_consolidation_source_item<'w, 's>(
    resource_type: ResourceType,
    donor_cells: &[Entity],
    queries: &TaskQueries<'w, 's>,
    shadow: &mut ReservationShadow,
) -> Option<(Entity, Vec2)> {
    mark_source_selector_call();
    ensure_frame_cache(queries, shadow);
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
        let source_item = cached_stockpile_items_by_resource(resource_type, cell, shadow)
            .iter()
            .copied()
            .inspect(|_| mark_candidate_scanned_item())
            .find(|entity| source_not_reserved(*entity, queries, shadow));
        if let Some(entity) = source_item {
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
    resource_grid: &ResourceSpatialGrid,
) -> Vec<(Entity, Vec2)> {
    collect_items_for_wheelbarrow_in_radius(
        resource_type,
        center_pos,
        max_count,
        queries,
        shadow,
        resource_grid,
        Some(hw_core::constants::TILE_SIZE * 10.0),
    )
}

pub fn collect_items_for_wheelbarrow_unbounded(
    resource_type: ResourceType,
    center_pos: Vec2,
    max_count: usize,
    queries: &TaskQueries<'_, '_>,
    shadow: &mut ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
) -> Vec<(Entity, Vec2)> {
    collect_items_for_wheelbarrow_in_radius(
        resource_type,
        center_pos,
        max_count,
        queries,
        shadow,
        resource_grid,
        None,
    )
}

fn collect_items_for_wheelbarrow_in_radius(
    resource_type: ResourceType,
    center_pos: Vec2,
    max_count: usize,
    queries: &TaskQueries<'_, '_>,
    shadow: &mut ReservationShadow,
    resource_grid: &ResourceSpatialGrid,
    search_radius: Option<f32>,
) -> Vec<(Entity, Vec2)> {
    mark_source_selector_call();
    let radius = search_radius.unwrap_or(
        hw_core::constants::TILE_SIZE
            * hw_core::constants::MAP_WIDTH.max(hw_core::constants::MAP_HEIGHT) as f32,
    );
    let search_radius_sq = radius * radius;
    let mut nearby_entities = Vec::new();
    resource_grid.get_nearby_in_radius_into(center_pos, radius, &mut nearby_entities);

    let mut items: Vec<(Entity, Vec2, f32)> = nearby_entities
        .into_iter()
        .inspect(|_| mark_candidate_scanned_item())
        .filter_map(|entity| {
            let Ok((
                transform,
                _tree_opt,
                _tree_variant_opt,
                _rock_opt,
                resource_opt,
                _,
                stored_in_opt,
            )) = queries.designation.targets.get(entity)
            else {
                return None;
            };
            if stored_in_opt.is_some() {
                return None;
            }
            if !resource_opt.is_some_and(|res| res.0 == resource_type) {
                return None;
            }
            if !source_not_reserved(entity, queries, shadow) {
                return None;
            }

            let pos = transform.translation.truncate();
            let dist_sq = pos.distance_squared(center_pos);
            if dist_sq > search_radius_sq {
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
