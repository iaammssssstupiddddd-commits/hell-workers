use crate::systems::world::zones::AreaBounds;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};

/// 削除対象と、それによって発生する孤立フラグメントを特定する
pub(crate) fn identify_removal_targets(
    world_map: &WorldMap,
    area: &AreaBounds,
) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    let mut direct_removal = Vec::new();
    let mut remaining_candidates = HashSet::new();

    // 1. 直接削除対象と、残存候補の洗い出し
    // 全てのストックパイルを確認するのは効率が悪いので、
    // 本来は「影響を受ける連結成分」だけを探索すべきだが、
    // ここでは簡易的に全ストックパイルを対象とする (数千個レベルなら問題ないはず)
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

    // 2. 残存候補の連結成分分析 (Flood Fill)
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

            // 4方向隣接
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

    // 3. 最大クラスタ以外をフラグメントとして削除対象に追加
    if clusters.is_empty() {
        return (direct_removal, Vec::new());
    }

    // 最大サイズのクラスタを見つける
    // 同点の場合はちらつき防止のために座標（クラスタ内の最小座標）をタイブレーカーとして使用する
    let max_cluster_index = clusters
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Equal => {
                let min_a = a.iter().min().unwrap();
                let min_b = b.iter().min().unwrap();
                min_a.cmp(min_b)
            }
            other => other,
        })
        .map(|(i, _)| i)
        .unwrap();

    let mut fragment_removal = Vec::new();
    for (i, cluster) in clusters.iter().enumerate() {
        if i != max_cluster_index {
            fragment_removal.extend(cluster);
        }
    }

    (direct_removal, fragment_removal)
}
