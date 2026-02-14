//! Stockpileグループ構築ユーティリティ
//!
//! ファミリア単位でStockpileセルをグループ化し、
//! 外周セル（収集距離判定用）と代表セル（anchor用）を決定する。

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
    /// 外周セルの座標（収集距離判定用）
    pub edge_positions: Vec<Vec2>,
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
    edge_cells: HashMap<(i32, i32), Vec<usize>>,
    groups_by_owner: HashMap<Entity, Vec<usize>>,
    owner_task_areas: HashMap<Entity, TaskArea>,
}

/// 収集範囲: 外周セルからの距離（タイル単位）
const EDGE_SEARCH_RADIUS_TILES: f32 = 10.0;

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

        // 外周セル判定: グリッド上で4方向隣接にグループ外セルがあるセルを外周とする
        let cell_positions: std::collections::HashSet<(i32, i32)> =
            positions.iter().map(|pos| world_to_grid(*pos)).collect();

        let edge_positions: Vec<Vec2> = positions
            .iter()
            .filter(|pos| {
                let grid = world_to_grid(**pos);
                let neighbors = [
                    (grid.0 + 1, grid.1),
                    (grid.0 - 1, grid.1),
                    (grid.0, grid.1 + 1),
                    (grid.0, grid.1 - 1),
                ];
                neighbors.iter().any(|n| !cell_positions.contains(n))
            })
            .copied()
            .collect();

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
            edge_positions,
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
    let mut edge_cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
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

        // 同一グループ内で同じ edge cell が重複登録されるのを防ぐ
        let mut unique_edge_cells = HashSet::new();
        for edge_pos in &group.edge_positions {
            let grid = world_to_grid(*edge_pos);
            if !unique_edge_cells.insert(grid) {
                continue;
            }
            edge_cells.entry(grid).or_default().push(group_idx);
        }
    }

    StockpileGroupSpatialIndex {
        edge_cells,
        groups_by_owner,
        owner_task_areas,
    }
}

/// アイテムが収集範囲内にあるかを判定し、最寄りグループを返す
///
/// 収集範囲 = TaskArea内全域 ∪ 外周セルから10タイル以内
/// 複数グループの範囲に入る場合は最寄り外周セル距離で排他決定
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
    let search_radius = EDGE_SEARCH_RADIUS_TILES * TILE_SIZE;
    let search_radius_sq = search_radius * search_radius;

    let mut candidate_group_indices = HashSet::new();
    let item_grid = world_to_grid(item_pos);
    let search_radius_tiles = EDGE_SEARCH_RADIUS_TILES.ceil() as i32;

    // 1) TaskArea 内候補を owner ごとに追加
    for (owner, area) in &index.owner_task_areas {
        if area.contains(item_pos) {
            if let Some(owner_groups) = index.groups_by_owner.get(owner) {
                candidate_group_indices.extend(owner_groups.iter().copied());
            }
        }
    }

    // 2) edge 近傍候補をセルバケットから追加
    for dy in -search_radius_tiles..=search_radius_tiles {
        for dx in -search_radius_tiles..=search_radius_tiles {
            let cell = (item_grid.0 + dx, item_grid.1 + dy);
            if let Some(cell_groups) = index.edge_cells.get(&cell) {
                candidate_group_indices.extend(cell_groups.iter().copied());
            }
        }
    }

    let mut best: Option<(usize, f32)> = None;

    for group_idx in candidate_group_indices {
        let group = &groups[group_idx];
        // 1. TaskArea内かチェック
        let in_task_area = index
            .owner_task_areas
            .get(&group.owner_familiar)
            .is_some_and(|area| area.contains(item_pos));

        // 2. 外周セルからの距離チェック
        let min_edge_dist_sq = group
            .edge_positions
            .iter()
            .map(|edge| edge.distance_squared(item_pos))
            .fold(f32::MAX, f32::min);

        let in_range = in_task_area || min_edge_dist_sq <= search_radius_sq;
        if !in_range {
            continue;
        }

        // 距離スコア: TaskArea内は外周距離、TaskArea外は外周距離
        let dist = min_edge_dist_sq;

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

fn world_to_grid(pos: Vec2) -> (i32, i32) {
    (
        (pos.x / TILE_SIZE).floor() as i32,
        (pos.y / TILE_SIZE).floor() as i32,
    )
}
