use super::*;

/// 1 worker のパス探索処理（cooldown 処理後に呼ぶ）。
///
/// Budget exhaustion is not an unreachable destination: `Deferred` retains
/// the existing state for a later frame.
pub(super) fn process_worker_pathfinding(
    commands: &mut Commands,
    soul: SoulPfState<'_>,
    world_pf: WorldPfCtx<'_>,
    work_queue: &mut RuntimePathWorkQueue,
    q_rest_areas: &Query<&Transform, With<hw_jobs::RestArea>>,
    queries: &mut crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> WorkerPathfindingOutcome {
    let entity = soul.entity;
    let transform = soul.transform;
    let WorldPfCtx {
        world_map,
        pf_context,
        budget,
    } = world_pf;
    let has_task = !matches!(*soul.task, AssignedTask::None);
    let current_pos = transform.translation.truncate();
    let start_grid = WorldMap::world_to_grid(current_pos);
    let goal_grid = WorldMap::world_to_grid(soul.destination.0);
    let obstacle_version = world_map.obstacle_version;

    // --- 再利用フェーズ: 既存パスが有効なら A* コストなしで続行 ---
    match reuse::try_reuse_existing_path(
        commands,
        reuse::ReusePfState {
            entity,
            budget,
            world_map,
            pf_context,
        },
        soul.path,
        soul.destination.0,
        goal_grid,
    ) {
        reuse::ReusePathResult::Reused => {
            work_queue.finish(entity);
            return WorkerPathfindingOutcome::Finished;
        }
        reuse::ReusePathResult::Deferred => return WorkerPathfindingOutcome::Deferred,
        reuse::ReusePathResult::NotReused => {}
    }

    // 同グリッド: A* 不要、1ステップで到達可能
    if start_grid == goal_grid {
        soul.path.waypoints = vec![soul.destination.0];
        soul.path.current_index = 0;
        record_path_plan(soul.path, soul.destination.0, obstacle_version);
        commands.entity(entity).remove::<PathCooldown>();
        work_queue.finish(entity);
        return WorkerPathfindingOutcome::Finished;
    }

    // --- 探索フェーズ: direct / adjacent fallback は各 core A* 枠を消費する ---
    //
    // A direct miss followed by an adjacent `Deferred` must not retry the
    // already-failed direct search next frame. The continuation is keyed by
    // the path inputs and discarded when either endpoint or topology changes.
    let fingerprint = ActorPathFingerprint {
        start_grid,
        goal_grid,
        destination: soul.destination.0,
        obstacle_version,
    };
    let stage = work_queue.stage_for(entity, fingerprint);

    if stage == ActorPathStage::Direct {
        match find_path_with_budget(
            world_map,
            pf_context,
            budget,
            PathSearchCaller::ActorNew,
            start_grid,
            goal_grid,
            PathGoalPolicy::RespectGoalWalkability,
        ) {
            PathSearchResult::Found(grid_path) => {
                soul.path.waypoints = grid_path
                    .iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                soul.path.current_index = 0;
                record_path_plan(soul.path, soul.destination.0, obstacle_version);
                commands.entity(entity).remove::<PathCooldown>();
                work_queue.finish(entity);
                debug!("PATH: Soul {:?} found new path", entity);
                return WorkerPathfindingOutcome::Finished;
            }
            PathSearchResult::Deferred => return WorkerPathfindingOutcome::Deferred,
            PathSearchResult::Unreachable => {
                work_queue.advance_to_adjacent(entity);
            }
        }
    }

    let mut run_rest_fallback = stage == ActorPathStage::RestFallback;
    if !run_rest_fallback {
        match find_path_to_adjacent_with_budget(
            world_map,
            pf_context,
            budget,
            PathSearchCaller::ActorNew,
            start_grid,
            goal_grid,
            true,
        ) {
            PathSearchResult::Found(grid_path) => {
                soul.path.waypoints = grid_path
                    .iter()
                    .map(|&(x, y)| WorldMap::grid_to_world(x, y))
                    .collect();
                soul.path.current_index = 0;
                record_path_plan(soul.path, soul.destination.0, obstacle_version);
                commands.entity(entity).remove::<PathCooldown>();
                work_queue.finish(entity);
                debug!("PATH: Soul {:?} found adjacent path", entity);
                return WorkerPathfindingOutcome::Finished;
            }
            PathSearchResult::Deferred => return WorkerPathfindingOutcome::Deferred,
            PathSearchResult::Unreachable => {
                debug!(
                    "PATH: Soul {:?} failed direct and adjacent pathfinding",
                    entity
                );
                if !has_task && soul.idle.behavior == IdleBehavior::GoingToRest {
                    work_queue.begin_rest_fallback(entity);
                    run_rest_fallback = true;
                }
            }
        }
    }

    // --- fallback フェーズ: 休憩所への代替タイルを探す（idle の GoingToRest のみ）---
    if run_rest_fallback {
        match fallback::try_rest_area_fallback_path(
            soul.destination,
            soul.path,
            soul.rest_reserved_for,
            q_rest_areas,
            fallback::SoulGridPos {
                current_pos,
                start_grid,
                goal_grid,
            },
            fallback::FallbackPfState {
                world_map,
                pf_context,
                budget,
            },
            work_queue.rest_fallback_progress(entity),
        ) {
            PathSearchResult::Found(()) => {
                commands.entity(entity).remove::<PathCooldown>();
                record_path_plan(soul.path, soul.destination.0, obstacle_version);
                work_queue.finish(entity);
                return WorkerPathfindingOutcome::Finished;
            }
            PathSearchResult::Deferred => return WorkerPathfindingOutcome::Deferred,
            PathSearchResult::Unreachable => {}
        }
    }

    // --- cleanup フェーズ: 到達不能の destination を破棄し、冷却期間を付与 ---
    fallback::cleanup_unreachable_destination(
        commands,
        fallback::SoulEntityCtx {
            entity,
            transform,
            current_pos,
            has_task,
        },
        fallback::SoulMoveState {
            idle: soul.idle,
            destination: soul.destination,
            task: soul.task,
            path: soul.path,
        },
        soul.inventory_opt,
        queries,
        world_map,
    );
    WorkerPathfindingOutcome::CoolingDown
}
