//! Stockpileグループ構築ユーティリティ
//!
//! ファミリア単位でStockpileセルをグループ化し、
//! 代表セル（anchor用）を決定する。

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::constants::TILE_SIZE;
use crate::relationships::StoredItems;
use crate::systems::command::TaskArea;
use crate::systems::logistics::{BucketStorage, Stockpile};
use crate::systems::spatial::StockpileSpatialGrid;

/// ファミリアのTaskArea内にあるStockpileのグループ
pub struct StockpileGroup {
    /// グループ内の全セルエンティティ
    pub cells: Vec<Entity>,
    /// リクエスト発行者のファミリア
    pub owner_familiar: Entity,
    /// 代表セル（anchor用、重心に最も近いセル）
    pub representative: Entity,
    /// グループ全体の合算キャパシティ
    pub total_capacity: usize,
    /// グループ全体の合算格納数
    pub total_stored: usize,
}

/// StockpileGroup の探索用空間インデックス
pub struct StockpileGroupSpatialIndex {
    groups_by_owner: HashMap<Entity, Vec<usize>>,
    owner_task_areas: HashMap<Entity, TaskArea>,
}

/// 収集範囲: TaskArea外周からの距離（タイル単位）
const TASK_AREA_PERIMETER_SEARCH_RADIUS_TILES: f32 = 10.0;

/// ファミリアごとにTaskArea内のStockpileセルをグループ化する
///
/// - 各ファミリアのTaskArea内セルを1グループとする
/// - 共有セル（複数TaskAreaに含まれる）は複数グループに含まれる
/// - BucketStorageは除外(bucket_auto_haul_systemが管理)
pub fn build_stockpile_groups(
    stockpile_grid: &StockpileSpatialGrid,
    active_familiars: &[(Entity, TaskArea)],
    q_stockpiles: &Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BucketStorage>,
    )>,
) -> Vec<StockpileGroup> {
    let mut groups = Vec::new();

    for (fam_entity, area) in active_familiars {
        let stock_entities = stockpile_grid.get_in_area(area.min, area.max);

        let mut cells = Vec::new();
        let mut positions = Vec::new();
        let mut total_capacity: usize = 0;
        let mut total_stored: usize = 0;

        for stock_entity in &stock_entities {
            let Ok((entity, transform, stockpile, stored_opt, bucket_opt)) =
                q_stockpiles.get(*stock_entity)
            else {
                continue;
            };

            // BucketStorageは除外
            if bucket_opt.is_some() {
                continue;
            }

            let pos = transform.translation.truncate();
            cells.push(entity);
            positions.push(pos);
            total_capacity += stockpile.capacity;
            total_stored += stored_opt.map(|s| s.len()).unwrap_or(0);
        }

        if cells.is_empty() {
            continue;
        }

        // 代表セル = 重心に最も近いセル
        let centroid = if positions.is_empty() {
            Vec2::ZERO
        } else {
            let sum: Vec2 = positions.iter().copied().sum();
            sum / positions.len() as f32
        };

        let representative_idx = positions
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let da = a.distance_squared(centroid);
                let db = b.distance_squared(centroid);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);

        groups.push(StockpileGroup {
            representative: cells[representative_idx],
            cells,
            owner_familiar: *fam_entity,
            total_capacity,
            total_stored,
        });
    }

    groups
}

/// StockpileGroup を高速探索するための空間インデックスを構築する
pub fn build_group_spatial_index(
    groups: &[StockpileGroup],
    familiars_with_areas: &[(Entity, TaskArea)],
) -> StockpileGroupSpatialIndex {
    let mut groups_by_owner: HashMap<Entity, Vec<usize>> = HashMap::new();
    let mut owner_task_areas: HashMap<Entity, TaskArea> = HashMap::new();

    for (familiar, area) in familiars_with_areas {
        owner_task_areas.insert(*familiar, area.clone());
    }

    for (group_idx, group) in groups.iter().enumerate() {
        groups_by_owner
            .entry(group.owner_familiar)
            .or_default()
            .push(group_idx);
    }

    StockpileGroupSpatialIndex {
        groups_by_owner,
        owner_task_areas,
    }
}

/// アイテムが収集範囲内にあるかを判定し、最寄りグループを返す
///
/// 収集範囲 = TaskArea外周から10タイル以内
/// ただし、TaskArea外側の「外周+10」領域では、他TaskArea内の位置を除外する。
/// 複数グループの範囲に入る場合は最寄りTaskArea外周距離で排他決定する。
pub fn find_nearest_group_for_item<'a>(
    item_pos: Vec2,
    groups: &'a [StockpileGroup],
    familiars_with_areas: &[(Entity, TaskArea)],
) -> Option<&'a StockpileGroup> {
    let index = build_group_spatial_index(groups, familiars_with_areas);
    find_nearest_group_for_item_indexed(item_pos, groups, &index)
}

/// 空間インデックスを使って、アイテム位置に対する最寄りグループを返す
pub fn find_nearest_group_for_item_indexed<'a>(
    item_pos: Vec2,
    groups: &'a [StockpileGroup],
    index: &StockpileGroupSpatialIndex,
) -> Option<&'a StockpileGroup> {
    let search_radius = TASK_AREA_PERIMETER_SEARCH_RADIUS_TILES * TILE_SIZE;
    let search_radius_sq = search_radius * search_radius;

    let mut candidate_group_indices = HashSet::new();
    let mut owners_containing_item = HashSet::new();
    let mut owner_perimeter_dist_sq = HashMap::new();

    // 1) TaskArea 外周 + 10 タイル以内の owner 候補を収集
    for (owner, area) in &index.owner_task_areas {
        if area.contains(item_pos) {
            owners_containing_item.insert(*owner);
        }

        let perimeter_dist_sq = distance_sq_to_task_area_perimeter(item_pos, area);
        if perimeter_dist_sq <= search_radius_sq {
            owner_perimeter_dist_sq.insert(*owner, perimeter_dist_sq);
            if let Some(owner_groups) = index.groups_by_owner.get(owner) {
                candidate_group_indices.extend(owner_groups.iter().copied());
            }
        }
    }

    let mut best: Option<(usize, f32)> = None;

    for group_idx in candidate_group_indices {
        let group = &groups[group_idx];
        let Some(&perimeter_dist_sq) = owner_perimeter_dist_sq.get(&group.owner_familiar) else {
            continue;
        };

        // 外周+10領域（TaskArea外）では、他TaskAreaに含まれる位置を除外する
        let in_owner_task_area = owners_containing_item.contains(&group.owner_familiar);
        if !in_owner_task_area && !owners_containing_item.is_empty() {
            continue;
        }

        let dist = perimeter_dist_sq;

        match &best {
            None => best = Some((group_idx, dist)),
            Some((best_idx, best_dist)) => {
                if dist < *best_dist
                    || (dist == *best_dist
                        && group.owner_familiar < groups[*best_idx].owner_familiar)
                {
                    best = Some((group_idx, dist));
                }
            }
        }
    }

    best.map(|(idx, _)| &groups[idx])
}

fn distance_sq_to_task_area_perimeter(pos: Vec2, area: &TaskArea) -> f32 {
    let inside_x = pos.x >= area.min.x && pos.x <= area.max.x;
    let inside_y = pos.y >= area.min.y && pos.y <= area.max.y;

    if inside_x && inside_y {
        let dist_to_left = pos.x - area.min.x;
        let dist_to_right = area.max.x - pos.x;
        let dist_to_bottom = pos.y - area.min.y;
        let dist_to_top = area.max.y - pos.y;
        let min_dist = dist_to_left
            .min(dist_to_right)
            .min(dist_to_bottom)
            .min(dist_to_top);
        min_dist * min_dist
    } else {
        let clamped_x = pos.x.clamp(area.min.x, area.max.x);
        let clamped_y = pos.y.clamp(area.min.y, area.max.y);
        let dx = pos.x - clamped_x;
        let dy = pos.y - clamped_y;
        dx * dx + dy * dy
    }
}
