//! パスキャッシュ検証・経路設定ヘルパー

use bevy::prelude::*;
use hw_core::soul::{Destination, Path};
use hw_world::WorldMap;

fn apply_grid_path(path: &mut Path, dest: &mut Destination, grid_path: &[(i32, i32)]) {
    if let Some(&last_grid) = grid_path.last() {
        dest.0 = WorldMap::grid_to_world(last_grid.0, last_grid.1);
    }
    path.waypoints = grid_path
        .iter()
        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
        .collect();
    path.current_index = 0;
}

/// インタラクション対象への隣接目的地を設定（岩などへの近接用）
///
/// 到達可能な隣接マスがあれば`true`を返し、なければ`false`を返す。
/// 実際の経路探索で到達可能か確認し、最も到達コストが小さい隣接マスを目的地として設定する。
pub fn update_destination_to_adjacent(
    dest: &mut Destination,
    target_pos: Vec2,
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut hw_world::PathfindingContext,
) -> bool {
    let target_grid = WorldMap::world_to_grid(target_pos);
    let start_grid = WorldMap::world_to_grid(soul_pos);

    // すでに有効なパスがあり、目的地も変わっていないならスキップ
    if !path.waypoints.is_empty()
        && path.current_index < path.waypoints.len()
        && let Some(last_wp) = path.waypoints.last()
    {
        let last_grid = WorldMap::world_to_grid(*last_wp);
        // 終点がターゲットに隣接していれば、そのパスは有効
        let dx = (last_grid.0 - target_grid.0).abs();
        let dy = (last_grid.1 - target_grid.1).abs();
        if dx <= 1 && dy <= 1 {
            // 目的地をパスの終点に更新（is_near_target_or_destで正しく判定するため）
            dest.0 = *last_wp;
            return true;
        }
    }

    // ターゲット自体がWalkableなら、そのまま直接移動を試みる
    if world_map.is_walkable(target_grid.0, target_grid.1) {
        // 直接の経路があればそれを使用
        if let Some(grid_path) = hw_world::find_path(
            world_map,
            pf_context,
            start_grid,
            target_grid,
            hw_world::PathGoalPolicy::RespectGoalWalkability,
        ) {
            apply_grid_path(path, dest, &grid_path);
            return true;
        }
    }

    // 最も到達コストが小さい隣接マスを見つける
    let directions = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    let mut best_path: Option<Vec<(i32, i32)>> = None;
    let mut best_cost = i32::MAX;

    for (dx, dy) in directions {
        let nx = target_grid.0 + dx;
        let ny = target_grid.1 + dy;

        // 隣接マスが歩行可能かチェック
        if !world_map.is_walkable(nx, ny) {
            continue;
        }

        // 開始点からこの隣接マスへの経路を探索
        if let Some(grid_path) = hw_world::find_path(
            world_map,
            pf_context,
            start_grid,
            (nx, ny),
            hw_world::PathGoalPolicy::RespectGoalWalkability,
        ) {
            // 経路コストを計算（パスの長さで近似）
            let cost = grid_path.len() as i32;
            if cost < best_cost {
                best_cost = cost;
                best_path = Some(grid_path);
            }
        }
    }

    if let Some(grid_path) = best_path {
        apply_grid_path(path, dest, &grid_path);
        true
    } else {
        // 到達不能: 近づける場所がない（完全な袋小路など）
        false
    }
}

/// 設計図への到達パスを設定（予定地の中心を一意なターゲットとする）
///
/// 到達可能な経路（または既に到着済み）がある場合は `true` を返す。
pub fn update_destination_to_blueprint(
    dest: &mut Destination,
    occupied_grids: &[(i32, i32)],
    path: &mut Path,
    soul_pos: Vec2,
    world_map: &WorldMap,
    pf_context: &mut hw_world::PathfindingContext,
) -> bool {
    use crate::soul_ai::helpers::navigation::{is_near_blueprint, update_destination_if_needed};

    let start_grid = WorldMap::world_to_grid(soul_pos);

    // 現在地がすでにゴール条件を満たしているかチェック
    if is_near_blueprint(soul_pos, occupied_grids) {
        // 到着済みなら、不要なパス（予定地内へ続くものなど）を消去して停止させる
        if !path.waypoints.is_empty() {
            path.waypoints.clear();
            path.current_index = 0;
            dest.0 = soul_pos;
        }
        return true;
    }

    // 現在のパスが既に有効（ターゲットの隣接点に向かっている）なら再計算しない
    if !path.waypoints.is_empty()
        && let Some(last_wp) = path.waypoints.last()
    {
        let last_grid = WorldMap::world_to_grid(*last_wp);

        // 終点が予定地外かつターゲットに隣接していれば、そのパスは有効
        if !occupied_grids.contains(&last_grid) {
            for &(gx, gy) in occupied_grids {
                let dx = (last_grid.0 - gx).abs();
                let dy = (last_grid.1 - gy).abs();
                if dx <= 1 && dy <= 1 {
                    return true;
                }
            }
        }
    }

    // ターゲットの中心地点を軸に「境界」までのパスを計算
    if let Some(grid_path) =
        hw_world::find_path_to_boundary(world_map, pf_context, start_grid, occupied_grids)
        && let Some(last_grid) = grid_path.last()
    {
        let last_pos = WorldMap::grid_to_world(last_grid.0, last_grid.1);
        update_destination_if_needed(dest, last_pos, path);

        path.waypoints = grid_path
            .iter()
            .map(|&(x, y)| WorldMap::grid_to_world(x, y))
            .collect();
        path.current_index = 0;
        return true;
    }

    false
}
