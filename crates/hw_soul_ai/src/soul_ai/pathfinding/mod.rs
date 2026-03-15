//! パス探索と障害物脱出

mod fallback;
mod reuse;

use bevy::prelude::*;
use hw_core::constants::MAX_PATHFINDS_PER_FRAME;
use hw_core::relationships::RestAreaReservedFor;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_world::{PathfindingContext, WorldMap, WorldMapRead};

use crate::soul_ai::execute::task_execution::AssignedTask;

/// フェーズ予算上限を返す。
/// task フェーズは idle 探索用スロットを確保するため上限を絞る。
fn phase_budget_limit(prioritize_tasks: bool) -> usize {
    const RESERVED_IDLE_PATHFINDS_PER_FRAME: usize = 2;
    if prioritize_tasks {
        MAX_PATHFINDS_PER_FRAME.saturating_sub(RESERVED_IDLE_PATHFINDS_PER_FRAME)
    } else {
        MAX_PATHFINDS_PER_FRAME
    }
}

/// 障害物に埋まったソウルを最寄りの歩行可能タイルへ逃がす。
/// 建築物の配置や障害物の追加で現在位置が通行不可になった場合に実行される。
pub fn soul_stuck_escape_system(
    world_map: WorldMapRead,
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

/// 1 worker のパス探索処理（cooldown 処理後に呼ぶ）。
/// pathfind_count は再利用時の部分再計算・新規計算それぞれで内部更新される。
#[allow(clippy::too_many_arguments)]
fn process_worker_pathfinding(
    commands: &mut Commands,
    entity: Entity,
    transform: &Transform,
    destination: &mut Destination,
    path: &mut Path,
    task: &mut AssignedTask,
    idle: &mut IdleState,
    rest_reserved_for: Option<&RestAreaReservedFor>,
    inventory_opt: Option<&mut hw_logistics::Inventory>,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    q_rest_areas: &Query<&Transform, With<hw_jobs::RestArea>>,
    queries: &mut crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    pathfind_count: &mut usize,
    budget: usize,
) {
    let has_task = !matches!(*task, AssignedTask::None);
    let current_pos = transform.translation.truncate();
    let start_grid = WorldMap::world_to_grid(current_pos);
    let goal_grid = WorldMap::world_to_grid(destination.0);

    // --- 再利用フェーズ: 既存パスが有効なら A* コストなしで続行 ---
    if reuse::try_reuse_existing_path(
        commands,
        entity,
        path,
        destination.0,
        goal_grid,
        world_map,
        pf_context,
        pathfind_count,
        budget,
    ) {
        return;
    }

    // デバッグログ: どのソウルがパス探索を行うか
    if has_task && path.waypoints.is_empty() {
        info!(
            "PATHFIND_DEBUG: Soul {:?} seeking path from {:?} to {:?}",
            entity, start_grid, goal_grid
        );
    }

    // 同グリッド: A* 不要、1ステップで到達可能
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
        return;
    }

    // --- 探索フェーズ: 予算スロットを 1 消費して A* を実行 ---
    if *pathfind_count >= budget {
        return;
    }
    *pathfind_count += 1;

    if let Some(world_path) =
        reuse::try_find_path_world_waypoints(world_map, pf_context, start_grid, goal_grid, entity)
    {
        // デバッグ：集会中のsoulで特定位置付近の場合
        if matches!(idle.behavior, IdleBehavior::Gathering)
            && current_pos.x.abs() < 150.0
            && current_pos.y.abs() < 250.0
        {
            info!(
                "PATHFIND: {:?} found path - waypoints: {}, from {:?} to {:?}",
                entity,
                world_path.len(),
                start_grid,
                goal_grid
            );
        }
        path.waypoints = world_path;
        path.current_index = 0;
        commands.entity(entity).remove::<PathCooldown>();
        debug!("PATH: Soul {:?} found new path", entity);
    } else {
        debug!("PATH: Soul {:?} failed to find path", entity);

        // --- fallback フェーズ: 休憩所への代替タイルを探す（idle の GoingToRest のみ）---
        if !has_task
            && idle.behavior == IdleBehavior::GoingToRest
            && fallback::try_rest_area_fallback_path(
                commands,
                destination,
                path,
                rest_reserved_for,
                q_rest_areas,
                current_pos,
                start_grid,
                goal_grid,
                world_map,
                pf_context,
                entity,
            )
        {
            return;
        }

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

        // --- cleanup フェーズ: 到達不能の destination を破棄し、冷却期間を付与 ---
        fallback::cleanup_unreachable_destination(
            commands,
            entity,
            transform,
            current_pos,
            has_task,
            idle,
            destination,
            task,
            path,
            inventory_opt,
            queries,
            world_map,
        );
    }
}

pub fn pathfinding_system(
    mut commands: Commands,
    world_map: WorldMapRead,
    mut pf_context: Local<PathfindingContext>,
    mut query: Query<
        (
            Entity,
            &Transform,
            &mut Destination,
            &mut Path,
            &mut AssignedTask,
            &mut IdleState,
            Option<&hw_core::relationships::RestingIn>,
            Option<&RestAreaReservedFor>,
            Option<&mut PathCooldown>,
            Option<&mut hw_logistics::Inventory>,
        ),
        With<DamnedSoul>,
    >,
    q_rest_areas: Query<&Transform, With<hw_jobs::RestArea>>,
    mut queries: crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) {
    let mut pathfind_count = 0usize;

    // task フェーズ → idle フェーズの順に処理
    for prioritize_tasks in [true, false] {
        // task フェーズは idle 探索用に枠を確保するため上限を絞る
        let budget = phase_budget_limit(prioritize_tasks);
        if pathfind_count >= budget {
            continue;
        }

        for (
            entity,
            transform,
            mut destination,
            mut path,
            mut task,
            mut idle,
            resting_in,
            rest_reserved_for,
            mut cooldown_opt,
            mut inventory_opt,
        ) in query.iter_mut()
        {
            let has_task = !matches!(*task, AssignedTask::None);
            // task フェーズは task あり worker のみ、idle フェーズは task なし worker のみ処理
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

            // --- クールダウン処理: 残フレームを消費し、切れたらコンポーネントを除去 ---
            if let Some(cooldown) = cooldown_opt.as_mut() {
                if cooldown.remaining_frames > 0 {
                    cooldown.remaining_frames -= 1;
                    continue;
                }
                commands.entity(entity).remove::<PathCooldown>();
            }

            process_worker_pathfinding(
                &mut commands,
                entity,
                transform,
                &mut destination,
                &mut path,
                &mut task,
                &mut idle,
                rest_reserved_for,
                inventory_opt.as_deref_mut(),
                world_map.as_ref(),
                &mut pf_context,
                &q_rest_areas,
                &mut queries,
                &mut pathfind_count,
                budget,
            );
        }
    }
}
