//! パス探索と障害物脱出

use crate::constants::{MAX_PATHFINDS_PER_FRAME, PATHFINDING_RETRY_COOLDOWN_FRAMES};
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

#[derive(Component, Debug, Clone, Copy)]
pub struct PathCooldown {
    remaining_frames: u8,
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
            Option<&crate::relationships::RestingIn>,
            Option<&mut PathCooldown>,
            Option<&mut crate::systems::logistics::Inventory>,
        ),
        With<DamnedSoul>,
    >,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    let mut pathfind_count = 0usize;

    for prioritize_tasks in [true, false] {
        if pathfind_count >= MAX_PATHFINDS_PER_FRAME {
            break;
        }

        for (
            entity,
            transform,
            destination,
            mut path,
            mut task,
            idle,
            resting_in,
            mut cooldown_opt,
            mut inventory_opt,
        ) in query.iter_mut()
        {
            let has_task = !matches!(*task, AssignedTask::None);
            if has_task != prioritize_tasks {
                continue;
            }

            let idle_can_move = match idle.behavior {
                IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
                IdleBehavior::Resting => resting_in.is_none(),
                IdleBehavior::GoingToRest => true,
                _ => true,
            };

            // タスクがなく、かつアイドル移動が不要なら探索不要
            if !has_task && !idle_can_move {
                continue;
            }

            if let Some(cooldown) = cooldown_opt.as_mut() {
                if cooldown.remaining_frames > 0 {
                    cooldown.remaining_frames -= 1;
                    continue;
                }
                commands.entity(entity).remove::<PathCooldown>();
            }

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
                    let goal_is_walkable = world_map.is_walkable(goal_grid.0, goal_grid.1);
                    let goal_reached_by_path = if goal_is_walkable {
                        last.distance_squared(destination.0) < 1.0
                    } else {
                        let last_grid = WorldMap::world_to_grid(*last);
                        let dx = (last_grid.0 - goal_grid.0).abs();
                        let dy = (last_grid.1 - goal_grid.1).abs();
                        dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0)
                    };

                    if goal_reached_by_path {
                        let blocked_relative = path.waypoints[path.current_index..]
                            .iter()
                            .position(|wp| {
                                let grid = WorldMap::world_to_grid(*wp);
                                !world_map.is_walkable(grid.0, grid.1)
                            });

                        if let Some(rel_idx) = blocked_relative {
                            if rel_idx > 0 && pathfind_count < MAX_PATHFINDS_PER_FRAME {
                                let resume_wp = path.waypoints[path.current_index + rel_idx - 1];
                                let resume_grid = WorldMap::world_to_grid(resume_wp);
                                pathfind_count += 1;

                                if let Some(partial_grid_path) = pathfinding::find_path(
                                    &*world_map,
                                    &mut *pf_context,
                                    resume_grid,
                                    goal_grid,
                                )
                                .or_else(|| {
                                    pathfinding::find_path_to_adjacent(
                                        &*world_map,
                                        &mut *pf_context,
                                        resume_grid,
                                        goal_grid,
                                    )
                                }) {
                                    let mut partial_world_path: Vec<Vec2> = partial_grid_path
                                        .iter()
                                        .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                                        .collect();
                                    if partial_grid_path.first().copied() == Some(resume_grid)
                                        && !partial_world_path.is_empty()
                                    {
                                        partial_world_path.remove(0);
                                    }

                                    let keep_len = path.current_index + rel_idx;
                                    path.waypoints.truncate(keep_len);
                                    path.waypoints.extend(partial_world_path);
                                    commands.entity(entity).remove::<PathCooldown>();
                                    debug!(
                                        "PATH: Soul {:?} reused partial path after blockage",
                                        entity
                                    );
                                    continue;
                                }
                            }

                            debug!(
                                "PATH: Soul {:?} path blocked by obstacle, recalculating",
                                entity
                            );
                        } else {
                            continue;
                        }
                    }
                }
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
                commands.entity(entity).remove::<PathCooldown>();
                // デバッグ：集会中のsoulで特定位置付近の場合
                if matches!(idle.behavior, IdleBehavior::Gathering)
                    && current_pos.x.abs() < 150.0
                    && current_pos.y.abs() < 250.0
                {
                    let dist = (destination.0 - current_pos).length();
                    info!(
                        "PATHFIND: {:?} same grid - pos: {:?}, dest: {:?}, dist: {:.1}",
                        entity, current_pos, destination.0, dist
                    );
                }
                continue;
            }

            if pathfind_count >= MAX_PATHFINDS_PER_FRAME {
                continue;
            }

            pathfind_count += 1;

            if let Some(grid_path) =
                pathfinding::find_path(&*world_map, &mut *pf_context, start_grid, goal_grid)
                    .or_else(|| {
                        // 通常のパスが見つからない場合、ターゲットの隣接マスへのパスを試みる
                        // これはターゲットが木や岩（非歩行可能）の上にある場合に有効
                        debug!(
                            "PATH: Soul {:?} failed find_path, trying find_path_to_adjacent",
                            entity
                        );
                        pathfinding::find_path_to_adjacent(
                            &*world_map,
                            &mut *pf_context,
                            start_grid,
                            goal_grid,
                        )
                    })
            {
                // デバッグ：集会中のsoulで特定位置付近の場合
                if matches!(idle.behavior, IdleBehavior::Gathering)
                    && current_pos.x.abs() < 150.0
                    && current_pos.y.abs() < 250.0
                {
                    info!(
                        "PATHFIND: {:?} found path - waypoints: {}, from {:?} to {:?}",
                        entity,
                        grid_path.len(),
                        start_grid,
                        goal_grid
                    );
                }
                path.waypoints = grid_path
                    .iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                path.current_index = 0;
                commands.entity(entity).remove::<PathCooldown>();
                debug!("PATH: Soul {:?} found new path", entity);
            } else {
                debug!("PATH: Soul {:?} failed to find path", entity);
                // デバッグ：集会中のsoulで特定位置付近の場合
                if matches!(idle.behavior, IdleBehavior::Gathering)
                    && current_pos.x.abs() < 150.0
                    && current_pos.y.abs() < 250.0
                {
                    warn!(
                        "PATHFIND: {:?} FAILED to find path - from grid {:?} to grid {:?}, dest: {:?}",
                        entity, start_grid, goal_grid, destination.0
                    );
                }
                path.waypoints.clear();
                commands.entity(entity).insert(PathCooldown {
                    remaining_frames: PATHFINDING_RETRY_COOLDOWN_FRAMES,
                });

                // タスク実行中なら放棄
                if has_task {
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
}
