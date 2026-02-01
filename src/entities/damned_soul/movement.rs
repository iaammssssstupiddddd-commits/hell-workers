//! ソウルの移動・パス追従・アニメーションシステム

use super::*;
use crate::constants::*;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};

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
    mut haul_cache: ResMut<crate::systems::familiar_ai::haul_cache::HaulReservationCache>,
    queries: crate::systems::soul_ai::task_execution::context::TaskQueries,
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
        if path.current_index < path.waypoints.len() && !path.waypoints.is_empty() {
            if let Some(last) = path.waypoints.last() {
                if last.distance_squared(destination.0) < 1.0 {
                    continue;
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
            info!("PATHFIND_DEBUG: Soul {:?} seeking path from {:?} to {:?}", entity, start_grid, goal_grid);
        }

        if start_grid == goal_grid {
            path.waypoints = vec![destination.0];
            path.current_index = 0;
            continue;
        }

        if let Some(grid_path) =
            pathfinding::find_path(&*world_map, &mut *pf_context, start_grid, goal_grid)
                .or_else(|| {
                    // 通常のパスが見つからない場合、ターゲットの隣接マスへのパスを試みる
                    // これはターゲットが木や岩（非歩行可能）の上にある場合に有効
                    debug!("PATH: Soul {:?} failed find_path, trying find_path_to_adjacent", entity);
                    pathfinding::find_path_to_adjacent(&*world_map, &mut *pf_context, start_grid, goal_grid)
                })
        {
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
                    None,
                    &queries,
                    &mut *haul_cache,
                    &*world_map,
                    true,
                );
            }
        }
    }
}

/// 移動システム
pub fn soul_movement(
    time: Res<Time>,
    world_map: Res<WorldMap>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &mut Path,
        &mut AnimationState,
        &DamnedSoul,
        &IdleState,
        Option<&StressBreakdown>,
    )>,
) {
    for (_entity, mut transform, mut path, mut anim, soul, idle, breakdown_opt) in query.iter_mut()
    {
        if let Some(breakdown) = breakdown_opt {
            if breakdown.is_frozen {
                anim.is_moving = false;
                continue;
            }
        }

        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            // 目的地への距離が十分近い場合は到着とみなす (1.0)
            if distance > 1.0 {
                let base_speed = SOUL_SPEED_BASE;
                let motivation_bonus = soul.motivation * SOUL_SPEED_MOTIVATION_BONUS;
                let laziness_penalty = soul.laziness * SOUL_SPEED_LAZINESS_PENALTY;
                let mut speed =
                    (base_speed + motivation_bonus - laziness_penalty).max(SOUL_SPEED_MIN);

                if idle.behavior == IdleBehavior::ExhaustedGathering {
                    speed *= SOUL_SPEED_EXHAUSTED_MULTIPLIER;
                }
                if idle.behavior == IdleBehavior::Escaping {
                    speed *= ESCAPE_SPEED_MULTIPLIER;
                }

                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;

                // --- 物理衝突チェック (Global Impassability) ---
                let next_pos = current_pos + velocity;
                let mut moved = false;

                if world_map.is_walkable_world(next_pos) {
                    // 通常移動
                    transform.translation.x = next_pos.x;
                    transform.translation.y = next_pos.y;
                    moved = true;
                } else {
                    // スライディング衝突解決
                    let next_pos_x = current_pos + Vec2::new(velocity.x, 0.0);
                    if world_map.is_walkable_world(next_pos_x) {
                        transform.translation.x = next_pos_x.x;
                        moved = true;
                    } else {
                        let next_pos_y = current_pos + Vec2::new(0.0, velocity.y);
                        if world_map.is_walkable_world(next_pos_y) {
                            transform.translation.y = next_pos_y.y;
                            moved = true;
                        }
                    }

                    if !moved && move_dist > 0.01 {
                        path.current_index += 1;
                    }
                }

                anim.is_moving = moved;
                if direction.x.abs() > 0.1 {
                    anim.facing_right = direction.x > 0.0;
                }
            } else {
                path.current_index += 1;
                anim.is_moving = false;
            }
        } else {
            anim.is_moving = false;
        }
    }
}

/// アニメーションシステム
pub fn animation_system(
    time: Res<Time>,
    mut query: Query<(
        &mut Transform,
        &mut Sprite,
        &mut AnimationState,
        &DamnedSoul,
    )>,
) {
    for (mut transform, mut sprite, mut anim, soul) in query.iter_mut() {
        sprite.flip_x = !anim.facing_right;

        if anim.is_moving {
            anim.bob_timer += time.delta_secs() * ANIM_BOB_SPEED;
            let bob = (anim.bob_timer.sin() * ANIM_BOB_AMPLITUDE) + 1.0;
            transform.scale = Vec3::new(1.0, bob, 1.0);
        } else {
            let breath_speed = ANIM_BREATH_SPEED_BASE - soul.laziness;
            anim.bob_timer += time.delta_secs() * breath_speed;
            let breath = (anim.bob_timer.sin() * ANIM_BREATH_AMPLITUDE) + 1.0;
            transform.scale = Vec3::splat(breath);
        }
    }
}
