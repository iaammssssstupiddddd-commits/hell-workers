//! パス再利用・新規探索ヘルパー

use bevy::prelude::*;
use hw_core::soul::Path;
use hw_world::{PathfindingContext, WorldMap, find_path_world_waypoints};

use super::PathCooldown;

/// 既存パスを再利用できるか検証し、障害物で部分遮断されていれば再計算する。
/// 再利用できた場合は true を返す。A* を呼んだ場合は pathfind_count を更新する。
pub(super) fn try_reuse_existing_path(
    commands: &mut Commands,
    entity: Entity,
    path: &mut Path,
    destination: Vec2,
    goal_grid: (i32, i32),
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    pathfind_count: &mut usize,
    phase_budget_limit: usize,
) -> bool {
    // すでに有効なパスがあり、目的地も変わっていないならスキップ
    //
    // ただし、移動側が衝突で waypoint をスキップして `current_index == waypoints.len()` になっている場合、
    // パスが「残っている」扱いで再計算されず、結果的に停止してしまうことがある。
    // そのため「まだパス追従中」の場合のみスキップする。
    //
    // また、パス上に新たな障害物が追加されていないかも確認する。
    if path.current_index >= path.waypoints.len() || path.waypoints.is_empty() {
        return false;
    }

    let Some(last) = path.waypoints.last() else {
        return false;
    };
    let goal_is_walkable = world_map.is_walkable(goal_grid.0, goal_grid.1);
    let goal_reached_by_path = if goal_is_walkable {
        last.distance_squared(destination) < 1.0
    } else {
        let last_grid = WorldMap::world_to_grid(*last);
        let dx = (last_grid.0 - goal_grid.0).abs();
        let dy = (last_grid.1 - goal_grid.1).abs();
        dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0)
    };

    if !goal_reached_by_path {
        return false;
    }

    let blocked_relative = path.waypoints[path.current_index..].iter().position(|wp| {
        let grid = WorldMap::world_to_grid(*wp);
        !world_map.is_walkable(grid.0, grid.1)
    });

    let Some(rel_idx) = blocked_relative else {
        return true;
    };

    if rel_idx > 0 && *pathfind_count < phase_budget_limit {
        let resume_wp = path.waypoints[path.current_index + rel_idx - 1];
        let resume_grid = WorldMap::world_to_grid(resume_wp);
        *pathfind_count += 1;

        if let Some(mut partial_world_path) =
            find_path_world_waypoints(world_map, pf_context, resume_grid, goal_grid)
        {
            let resume_world = WorldMap::grid_to_world(resume_grid.0, resume_grid.1);
            if partial_world_path.first().copied() == Some(resume_world)
                && !partial_world_path.is_empty()
            {
                partial_world_path.remove(0);
            }

            let keep_len = path.current_index + rel_idx;
            path.waypoints.truncate(keep_len);
            path.waypoints.extend(partial_world_path);
            commands.entity(entity).remove::<PathCooldown>();
            debug!("PATH: Soul {:?} reused partial path after blockage", entity);
            return true;
        }
    }

    debug!(
        "PATH: Soul {:?} path blocked by obstacle, recalculating",
        entity
    );
    false
}
