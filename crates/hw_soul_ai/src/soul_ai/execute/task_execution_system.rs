use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::events::publish_task_completed;
use hw_core::relationships::WorkingOn;
use hw_core::visual::SoulTaskHandles;
use hw_core::{EpochLocal, WorldEpoch};
use hw_jobs::{ActiveTaskIdentity, AssignedTask};
use hw_logistics::Wheelbarrow;
use hw_world::pathfinding::PathfindingContext;
use hw_world::{RuntimePathSearchBudget, WorldMapRead};

#[cfg(feature = "profiling")]
use crate::soul_ai::execute::task_execution::TaskExecutionPerfMetrics;
use crate::soul_ai::execute::task_execution::context::{
    TaskExecEnv, TaskExecutionContext, TaskHandlerControl, TaskQueries,
};
use crate::soul_ai::execute::task_execution::handler::dispatch::run_task_handler;
use crate::soul_ai::execute::task_execution::path_cache::TaskPathSearchProgress;
use crate::soul_ai::helpers::query_types::TaskExecutionSoulQuery;
use crate::soul_ai::helpers::work::unassign_task;
use crate::soul_ai::pathfinding::TASK_EXECUTION_PATHFINDS_PHASE_LIMIT;

#[derive(SystemParam)]
pub struct TaskExecResources<'w, 's> {
    soul_handles: Res<'w, SoulTaskHandles>,
    time: Res<'w, Time>,
    world_map: WorldMapRead<'w>,
    pf_context: Local<'s, PathfindingContext>,
    path_budget: ResMut<'w, RuntimePathSearchBudget>,
    world_epoch: Option<Res<'w, WorldEpoch>>,
    path_search_progress: Local<'s, EpochLocal<TaskPathSearchProgress>>,
    task_round_robin: Local<'s, EpochLocal<TaskExecutionRoundRobin>>,
}

/// Active task handlers are visited from the request after the last core A*
/// claimant. This preserves the existing handler cadence while preventing the
/// query's first worker from claiming every available task-phase slot.
#[derive(Default)]
pub struct TaskExecutionRoundRobin {
    last_core_search_claimant: Option<Entity>,
    entities: Vec<Entity>,
}

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: TaskExecutionSoulQuery,
    mut queries: TaskQueries,
    mut res: TaskExecResources,
    q_wheelbarrows: Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
    q_entities: Query<Entity>,
    #[cfg(feature = "profiling")] mut perf_metrics: ResMut<TaskExecutionPerfMetrics>,
) {
    // Escape runs in Decide before task execution. Reserve two ActiveTask
    // slots for Actor-side replans later in the frame, plus the idle reserve.
    res.path_budget
        .begin_phase(TASK_EXECUTION_PATHFINDS_PHASE_LIMIT);
    let world_epoch = res
        .world_epoch
        .map_or_else(WorldEpoch::default, |epoch| *epoch);
    let path_search_progress = res.path_search_progress.get_mut(world_epoch);
    let task_round_robin = res.task_round_robin.get_mut(world_epoch);
    task_round_robin.entities.clear();
    task_round_robin
        .entities
        .extend(q_souls.iter().map(|(entity, ..)| entity));
    let task_count = task_round_robin.entities.len();
    let task_start = task_round_robin
        .last_core_search_claimant
        .and_then(|last| {
            task_round_robin
                .entities
                .iter()
                .position(|entity| *entity == last)
        })
        .map_or(0, |index| (index + 1) % task_count.max(1));

    #[cfg(feature = "profiling")]
    let mut souls_queried = 0u32;
    #[cfg(feature = "profiling")]
    let mut idle_skips = 0u32;
    #[cfg(feature = "profiling")]
    let mut handler_runs = 0u32;

    for offset in 0..task_count {
        let entity = task_round_robin.entities[(task_start + offset) % task_count];
        let Ok((
            soul_entity,
            soul_transform,
            soul,
            mut task,
            dest,
            mut path,
            mut inventory,
            breakdown_opt,
            identity_opt,
            working_on_opt,
        )) = q_souls.get_mut(entity)
        else {
            continue;
        };
        #[cfg(feature = "profiling")]
        {
            souls_queried = souls_queried.saturating_add(1);
        }

        // `&task` is an immutable reborrow of `Mut<AssignedTask>`. これを
        // TaskExecutionContext の `&mut AssignedTask` に渡す前に判定し、idle
        // Soul の5コンポーネントに不要な Changed を立てない。
        if is_idle_task(&task) {
            #[cfg(feature = "profiling")]
            {
                idle_skips = idle_skips.saturating_add(1);
            }
            continue;
        }

        if !has_consistent_task_identity(identity_opt.as_deref(), working_on_opt) {
            let reason = if identity_opt.is_some() {
                "WorkingOn target differs from ActiveTaskIdentity"
            } else {
                "ActiveTaskIdentity is missing"
            };
            warn!(
                "TASK_EXEC: Soul {:?} retryably aborting task because {}",
                soul_entity, reason
            );
            unassign_task(
                &mut commands,
                crate::soul_ai::helpers::work::SoulDropCtx {
                    soul_entity,
                    drop_pos: soul_transform.translation.truncate(),
                    inventory: Some(&mut inventory),
                    dropped_item_res: None,
                },
                &mut task,
                &mut path,
                &mut queries,
                res.world_map.as_ref(),
                false,
            );
            path_search_progress.clear_entity(soul_entity);
            continue;
        }
        let Some(identity) = identity_opt else {
            unreachable!("identity consistency check requires ActiveTaskIdentity");
        };

        if let Some(expected_item) = task.expected_item() {
            let needs_item = task.requires_item_in_inventory();
            let expected_item_alive = q_entities.get(expected_item).is_ok();
            let has_expected = inventory.0 == Some(expected_item) && expected_item_alive;
            let has_mismatch = inventory.0.is_some() && !has_expected;
            let missing_required = needs_item && !has_expected;

            if has_mismatch || missing_required {
                unassign_task(
                    &mut commands,
                    crate::soul_ai::helpers::work::SoulDropCtx {
                        soul_entity,
                        drop_pos: soul_transform.translation.truncate(),
                        inventory: Some(&mut inventory),
                        dropped_item_res: None,
                    },
                    &mut task,
                    &mut path,
                    &mut queries,
                    res.world_map.as_ref(),
                    false,
                );
                continue;
            }
        }

        let budget_used_before = res.path_budget.used();
        let completed_identity = {
            let mut ctx = TaskExecutionContext {
                soul_entity,
                soul_transform,
                soul,
                task,
                dest,
                path,
                inventory,
                identity,
                pf_context: &mut res.pf_context,
                path_budget: &mut res.path_budget,
                path_search_progress,
                queries: &mut queries,
                env: TaskExecEnv {
                    soul_handles: &res.soul_handles,
                    time: res.time.as_ref(),
                    world_map: res.world_map.as_ref(),
                    breakdown: breakdown_opt,
                },
                end_state: default(),
            };

            #[cfg(feature = "profiling")]
            {
                handler_runs = handler_runs.saturating_add(1);
            }
            let handler_control = run_task_handler(&mut ctx, &mut commands, &q_wheelbarrows);
            if handler_control == TaskHandlerControl::AlreadyEnded {
                debug!(
                    "TASK_EXEC: Soul {:?} handler attempted a duplicate terminal transition",
                    soul_entity
                );
            }

            if ctx.is_completed() {
                Some(ctx.task_identity())
            } else {
                None
            }
        };
        if res.path_budget.used() > budget_used_before {
            task_round_robin.last_core_search_claimant = Some(soul_entity);
        }

        if let Some(identity) = completed_identity {
            publish_task_completed(
                &mut commands,
                soul_entity,
                identity.assignment_entity,
                identity.current_target_entity,
                identity.current_work_type,
            );

            debug!(
                "EVENT: OnTaskCompleted triggered for Soul {:?}",
                soul_entity
            );
        }
    }

    #[cfg(feature = "profiling")]
    {
        perf_metrics.souls_queried = perf_metrics.souls_queried.saturating_add(souls_queried);
        perf_metrics.idle_skips = perf_metrics.idle_skips.saturating_add(idle_skips);
        perf_metrics.handler_runs = perf_metrics.handler_runs.saturating_add(handler_runs);
    }
}

/// `Mut<AssignedTask>` を mutable に dereference せず、idle task を判定する。
fn is_idle_task(task: &AssignedTask) -> bool {
    matches!(task, AssignedTask::None)
}

fn has_consistent_task_identity(
    identity: Option<&ActiveTaskIdentity>,
    working_on: Option<&WorkingOn>,
) -> bool {
    identity.is_some_and(|identity| identity.matches_working_on(working_on.map(|value| value.0)))
}

#[cfg(test)]
mod tests;
