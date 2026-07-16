//! パス再利用・新規探索ヘルパー

use bevy::prelude::*;
use hw_core::soul::Path;
use hw_world::{
    PathSearchCaller, PathSearchResult, PathfindingContext, RuntimePathSearchBudget, WorldMap,
    find_path_world_waypoints_with_budget,
};

use super::PathCooldown;

/// 既存パスの検証結果。
pub(super) enum ReusePathResult {
    Reused,
    NotReused,
    Deferred,
}

/// `try_reuse_existing_path` が使う探索状態。
pub(super) struct ReusePfState<'a> {
    pub entity: Entity,
    pub budget: &'a mut RuntimePathSearchBudget,
    pub world_map: &'a WorldMap,
    pub pf_context: &'a mut PathfindingContext,
}

/// 既存パスを再利用できるか検証し、障害物で部分遮断されていれば再計算する。
///
/// `Deferred` では状態を変更せず、次フレームに再試行する。
pub(super) fn try_reuse_existing_path(
    commands: &mut Commands,
    pf: ReusePfState<'_>,
    path: &mut Path,
    destination: Vec2,
    goal_grid: (i32, i32),
) -> ReusePathResult {
    let ReusePfState {
        entity,
        budget,
        world_map,
        pf_context,
    } = pf;
    // すでに有効なパスがあり、目的地も変わっていないならスキップ
    //
    // ただし、移動側が衝突で waypoint をスキップして `current_index == waypoints.len()` になっている場合、
    // パスが「残っている」扱いで再計算されず、結果的に停止してしまうことがある。
    // そのため「まだパス追従中」の場合のみスキップする。
    //
    // また、パス上に新たな障害物が追加されていないかも確認する。
    if path.current_index >= path.waypoints.len() || path.waypoints.is_empty() {
        return ReusePathResult::NotReused;
    }

    let Some(last) = path.waypoints.last() else {
        return ReusePathResult::NotReused;
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
        return ReusePathResult::NotReused;
    }

    if path.validated_obstacle_version == world_map.obstacle_version {
        return ReusePathResult::Reused;
    }

    let blocked_relative = path.waypoints[path.current_index..].iter().position(|wp| {
        let grid = WorldMap::world_to_grid(*wp);
        !world_map.is_walkable(grid.0, grid.1)
    });

    let Some(rel_idx) = blocked_relative else {
        path.validated_obstacle_version = world_map.obstacle_version;
        path.planned_destination = Some(destination);
        return ReusePathResult::Reused;
    };

    if rel_idx > 0 {
        let resume_wp = path.waypoints[path.current_index + rel_idx - 1];
        let resume_grid = WorldMap::world_to_grid(resume_wp);

        match find_path_world_waypoints_with_budget(
            world_map,
            pf_context,
            budget,
            PathSearchCaller::ActorReuse,
            resume_grid,
            goal_grid,
        ) {
            PathSearchResult::Found(mut partial_world_path) => {
                let resume_world = WorldMap::grid_to_world(resume_grid.0, resume_grid.1);
                if partial_world_path.first().copied() == Some(resume_world)
                    && !partial_world_path.is_empty()
                {
                    partial_world_path.remove(0);
                }

                let keep_len = path.current_index + rel_idx;
                path.waypoints.truncate(keep_len);
                path.waypoints.extend(partial_world_path);
                path.validated_obstacle_version = world_map.obstacle_version;
                path.planned_destination = Some(destination);
                commands.entity(entity).remove::<PathCooldown>();
                debug!("PATH: Soul {:?} reused partial path after blockage", entity);
                return ReusePathResult::Reused;
            }
            PathSearchResult::Deferred => return ReusePathResult::Deferred,
            PathSearchResult::Unreachable => {}
        }
    }

    debug!(
        "PATH: Soul {:?} path blocked by obstacle, recalculating",
        entity
    );
    ReusePathResult::NotReused
}
