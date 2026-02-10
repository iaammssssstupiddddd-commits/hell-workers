//! パス探索と障害物脱出

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

/// 障害物に埋まったソウルを最寄りの歩行可能タイルへ逃がす。
/// 建築物の配置や障害物の追加で現在位置が通行不可になった場合に実行される。
pub fn soul_stuck_escape_system(
    world_map: Res<WorldMap>,
    mut query: Query<(&mut Transform, &mut Path), With<DamnedSoul>>,
) {
    for (mut transform, mut path) in query.iter_mut() {
        let current_pos = transform.translation.truncate();
        if world_map.is_walkable_world(current_pos) {
            continue;
        }
        if let Some((gx, gy)) = world_map.get_nearest_walkable_grid(current_pos) {
            let escape_pos = WorldMap::grid_to_world(gx, gy);
            transform.translation.x = escape_pos.x;
            transform.translation.y = escape_pos.y;
            path.waypoints.clear();
            path.current_index = 0;
            debug!(
                "SOUL_STUCK_ESCAPE: moved soul from {:?} to walkable {:?}",
                current_pos, escape_pos
            );
        }
    }
}

pub fn pathfinding_system(
    mut commands: Commands,
    world_map: Res<WorldMap>,
    mut pf_context: Local<PathfindingContext>,
    mut query: Query<
        (
            Entity,
            &Transform,
            &Destination,
            &mut Path,
            &mut AssignedTask,
            &IdleState,
            Option<&mut crate::systems::logistics::Inventory>,
        ),
        With<DamnedSoul>,
    >,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    for (entity, transform, destination, mut path, mut task, idle, mut inventory_opt) in
        query.iter_mut()
    {
        let current_pos = transform.translation.truncate();
        let start_grid = WorldMap::world_to_grid(current_pos);
        let goal_grid = WorldMap::world_to_grid(destination.0);

        // すでに有効なパスがあり、目的地も変わっていないならスキップ
        //
        // ただし、移動側が衝突で waypoint をスキップして `current_index == waypoints.len()` になっている場合、
        // パスが「残っている」扱いで再計算されず、結果的に停止してしまうことがある。
        // そのため「まだパス追従中」の場合のみスキップする。
        //
        // また、パス上に新たな障害物が追加されていないかも確認する。
        if path.current_index < path.waypoints.len() && !path.waypoints.is_empty() {
            if let Some(last) = path.waypoints.last() {
                if last.distance_squared(destination.0) < 1.0 {
                    // パス上に障害物がないか確認（残りの経路部分のみ）
                    let path_blocked = path.waypoints[path.current_index..].iter().any(|wp| {
                        let grid = WorldMap::world_to_grid(*wp);
                        !world_map.is_walkable(grid.0, grid.1)
                    });

                    if !path_blocked {
                        continue;
                    }

                    // パスが阻塞された場合、再計算が必要
                    debug!(
                        "PATH: Soul {:?} path blocked by obstacle, recalculating",
                        entity
                    );
                }
            }
        }

        let has_task = !matches!(*task, AssignedTask::None);
        let idle_can_move = match idle.behavior {
            IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
            _ => true,
        };

        // タスクがなく、かつアイドル移動が不要なら探索不要
        if !has_task && !idle_can_move {
            continue;
        }

        // デバッグログ: どのソウルがパス探索を行うか
        if has_task && path.waypoints.is_empty() {
            info!(
                "PATHFIND_DEBUG: Soul {:?} seeking path from {:?} to {:?}",
                entity, start_grid, goal_grid
            );
        }

        if start_grid == goal_grid {
            path.waypoints = vec![destination.0];
            path.current_index = 0;
            continue;
        }

        if let Some(grid_path) = pathfinding::find_path(
            &*world_map,
            &mut *pf_context,
            start_grid,
            goal_grid,
        )
        .or_else(|| {
            // 通常のパスが見つからない場合、ターゲットの隣接マスへのパスを試みる
            // これはターゲットが木や岩（非歩行可能）の上にある場合に有効
            debug!(
                "PATH: Soul {:?} failed find_path, trying find_path_to_adjacent",
                entity
            );
            pathfinding::find_path_to_adjacent(&*world_map, &mut *pf_context, start_grid, goal_grid)
        }) {
            path.waypoints = grid_path
                .iter()
                .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                .collect();
            path.current_index = 0;
            debug!("PATH: Soul {:?} found new path", entity);
        } else {
            debug!("PATH: Soul {:?} failed to find path", entity);
            path.waypoints.clear();

            // タスク実行中なら放棄
            if !matches!(*task, AssignedTask::None) {
                info!(
                    "PATH: Soul {:?} abandoning task due to unreachable destination",
                    entity
                );
                unassign_task(
                    &mut commands,
                    entity,
                    transform.translation.truncate(),
                    &mut task,
                    &mut path,
                    inventory_opt.as_deref_mut(),
                    None, // Dropped resource
                    &mut queries,
                    &*world_map,
                    true,
                );
            }
        }
    }
}
