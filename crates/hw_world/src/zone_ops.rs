//! Zone 操作に関する純粋なアルゴリズム helper。
//! `Query` / `Commands` に依存せず、`WorldMap` と domain 型のみを扱う。

use std::collections::{HashSet, VecDeque};

use hw_core::area::AreaBounds;

use crate::coords::world_to_grid;
use crate::map::WorldMap;
use crate::zones::{Site, Yard};

// ---------------------------------------------------------------------------
// Removal targets
// ---------------------------------------------------------------------------

/// 削除対象タイルと、それによって発生する孤立フラグメントを特定する。
///
/// 戻り値: `(直接削除対象, 孤立フラグメント削除対象)`
pub fn identify_removal_targets(
    world_map: &WorldMap,
    area: &AreaBounds,
) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
    let min_grid = WorldMap::world_to_grid(area.min + bevy::math::Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - bevy::math::Vec2::splat(0.1));

    let mut direct_removal = Vec::new();
    let mut remaining_candidates = HashSet::new();

    for (&grid, _) in world_map.stockpile_entries() {
        if grid.0 >= min_grid.0
            && grid.0 <= max_grid.0
            && grid.1 >= min_grid.1
            && grid.1 <= max_grid.1
        {
            direct_removal.push(grid);
        } else {
            remaining_candidates.insert(grid);
        }
    }

    if direct_removal.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // 残存候補の連結成分分析 (Flood Fill)
    let mut visited = HashSet::new();
    let mut clusters: Vec<Vec<(i32, i32)>> = Vec::new();

    for &start_node in &remaining_candidates {
        if visited.contains(&start_node) {
            continue;
        }

        let mut cluster = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(start_node);
        visited.insert(start_node);

        while let Some(current) = queue.pop_front() {
            cluster.push(current);

            let neighbors = [
                (current.0 + 1, current.1),
                (current.0 - 1, current.1),
                (current.0, current.1 + 1),
                (current.0, current.1 - 1),
            ];

            for neighbor in neighbors {
                if remaining_candidates.contains(&neighbor) && visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        clusters.push(cluster);
    }

    if clusters.is_empty() {
        return (direct_removal, Vec::new());
    }

    // 最大クラスタ以外をフラグメントとして削除対象に追加（タイブレーカー: 最小座標）
    let max_cluster_index = clusters
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Equal => {
                let min_a = a.iter().min().expect("cluster is non-empty");
                let min_b = b.iter().min().expect("cluster is non-empty");
                min_a.cmp(min_b)
            }
            other => other,
        })
        .map(|(i, _)| i)
        .expect("clusters is non-empty: checked above");

    let mut fragment_removal = Vec::new();
    for (i, cluster) in clusters.iter().enumerate() {
        if i != max_cluster_index {
            fragment_removal.extend(cluster);
        }
    }

    (direct_removal, fragment_removal)
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

/// エリアのタイル単位サイズを返す。`(幅, 高さ)`
pub fn area_tile_size(area: &AreaBounds) -> (usize, usize) {
    let min_grid = world_to_grid(area.min + bevy::math::Vec2::splat(0.1));
    let max_grid = world_to_grid(area.max - bevy::math::Vec2::splat(0.1));
    let width = max_grid.0.saturating_sub(min_grid.0).saturating_add(1) as usize;
    let height = max_grid.1.saturating_sub(min_grid.1).saturating_add(1) as usize;
    (width, height)
}

/// `area` と `site` が矩形重複しているか判定する。
pub fn rectangles_overlap_site(area: &AreaBounds, site: &Site) -> bool {
    area.min.x < site.max.x
        && area.max.x > site.min.x
        && area.min.y < site.max.y
        && area.max.y > site.min.y
}

/// `area` と `yard` が矩形重複しているか判定する。
pub fn rectangles_overlap(area: &AreaBounds, yard: &Yard) -> bool {
    area.min.x <= yard.max.x
        && area.max.x >= yard.min.x
        && area.min.y <= yard.max.y
        && area.max.y >= yard.min.y
}

/// `yard` を `drag_area` を含むように拡張した `AreaBounds` を返す。
pub fn expand_yard_area(yard: &Yard, drag_area: &AreaBounds) -> AreaBounds {
    let min = bevy::math::Vec2::new(
        yard.min.x.min(drag_area.min.x),
        yard.min.y.min(drag_area.min.y),
    );
    let max = bevy::math::Vec2::new(
        yard.max.x.max(drag_area.max.x),
        yard.max.y.max(drag_area.max.y),
    );
    AreaBounds { min, max }
}
