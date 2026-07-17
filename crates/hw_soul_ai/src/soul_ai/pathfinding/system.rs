use crate::soul_ai::execute::task_execution::AssignedTask;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::MAX_PATHFINDS_PER_FRAME;
use hw_core::relationships::RestAreaReservedFor;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_core::{EpochLocal, WorldEpoch};
use hw_world::{
    PathGoalPolicy, PathSearchCaller, PathSearchResult, PathfindingContext,
    RuntimePathSearchBudget, WorldMap, WorldMapRead, find_path_to_adjacent_with_budget,
    find_path_with_budget,
};

use super::{PathCooldown, TASK_PATHFINDS_PHASE_LIMIT, fallback, reuse};

mod work_queue;

#[cfg(feature = "profiling")]
pub use work_queue::RuntimePathDeferMetrics;
use work_queue::{
    ActorPathFingerprint, ActorPathStage, PathRequestClass, RuntimePathWorkQueue,
    enqueue_requests_in_entity_order,
};

/// フェーズ予算上限を返す。
/// escapeが先行して使う枠を含め、taskフェーズはidle探索用スロットを確保する。
fn phase_budget_limit(prioritize_tasks: bool) -> usize {
    if prioritize_tasks {
        TASK_PATHFINDS_PHASE_LIMIT
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
            path.planned_destination = None;
            path.validated_obstacle_version = 0;
            debug!(
                "SOUL_STUCK_ESCAPE: moved soul from {:?} to walkable {:?}",
                current_pos, escape_pos
            );
        }
    }
}

/// `process_worker_pathfinding` に渡す Soul の移動状態・アイドル情報。
struct SoulPfState<'a> {
    entity: Entity,
    transform: &'a Transform,
    destination: &'a mut Destination,
    path: &'a mut Path,
    task: &'a mut AssignedTask,
    idle: &'a mut IdleState,
    rest_reserved_for: Option<&'a RestAreaReservedFor>,
    inventory_opt: Option<&'a mut hw_logistics::Inventory>,
}

/// `process_worker_pathfinding` に渡すワールド・パス探索コンテキスト。
struct WorldPfCtx<'a> {
    world_map: &'a WorldMap,
    pf_context: &'a mut PathfindingContext,
    budget: &'a mut RuntimePathSearchBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkerPathfindingOutcome {
    Finished,
    Deferred,
    CoolingDown,
}

/// Resets the shared runtime core-A* budget before Actor pathfinding runs.
pub fn reset_runtime_path_search_budget_system(mut budget: ResMut<RuntimePathSearchBudget>) {
    budget.reset();
}

fn record_path_plan(path: &mut Path, destination: Vec2, obstacle_version: u64) {
    path.planned_destination = Some(destination);
    path.validated_obstacle_version = obstacle_version;
}

/// クールダウン中でなく、有効なパス追従中なら当フレームの探索処理を省略できる。
fn can_skip_pathfinding_tick(
    has_task: bool,
    idle_can_move: bool,
    cooldown: Option<&PathCooldown>,
    path: &Path,
    destination: Vec2,
    obstacle_version: u64,
) -> bool {
    if !has_task && !idle_can_move {
        return true;
    }
    if cooldown.is_some() {
        return false;
    }
    if path.waypoints.is_empty() || path.current_index >= path.waypoints.len() {
        return false;
    }
    if path.planned_destination != Some(destination) {
        return false;
    }
    path.validated_obstacle_version == obstacle_version
}

mod worker;

use worker::process_worker_pathfinding;

type PathfindingQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
        &'static mut IdleState,
        Option<&'static hw_core::relationships::RestingIn>,
        Option<&'static RestAreaReservedFor>,
        Option<&'static mut PathCooldown>,
        Option<&'static mut hw_logistics::Inventory>,
    ),
    With<DamnedSoul>,
>;

type PathfindingChangedScan<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Destination,
        &'static Path,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static hw_core::relationships::RestingIn>,
        Option<&'static PathCooldown>,
    ),
    (
        With<DamnedSoul>,
        Or<(
            Changed<Destination>,
            Changed<Path>,
            Changed<AssignedTask>,
            Changed<IdleState>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub struct PathfindingResources<'w, 's> {
    world_epoch: Option<Res<'w, WorldEpoch>>,
    world_map: WorldMapRead<'w>,
    pf_context: Local<'s, PathfindingContext>,
    budget: ResMut<'w, RuntimePathSearchBudget>,
    #[cfg(feature = "profiling")]
    defer_metrics: ResMut<'w, RuntimePathDeferMetrics>,
    work_queue: Local<'s, EpochLocal<RuntimePathWorkQueue>>,
    rest_areas: Query<'w, 's, &'static Transform, With<hw_jobs::RestArea>>,
    assignment_queries:
        crate::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>,
}

fn idle_behavior_can_move(
    idle: &IdleState,
    resting_in: Option<&hw_core::relationships::RestingIn>,
) -> bool {
    match idle.behavior {
        IdleBehavior::Sitting | IdleBehavior::Sleeping => false,
        IdleBehavior::Resting => resting_in.is_none(),
        IdleBehavior::GoingToRest => true,
        _ => true,
    }
}

fn request_class_if_needed(
    task: &AssignedTask,
    idle: &IdleState,
    resting_in: Option<&hw_core::relationships::RestingIn>,
    cooldown: Option<&PathCooldown>,
    path: &Path,
    destination: Vec2,
    obstacle_version: u64,
) -> Option<PathRequestClass> {
    // A cooldown is advanced only by the queue's dedicated timer lane. It is
    // never a runnable path request, even if another component changed.
    if cooldown.is_some() {
        return None;
    }

    let has_task = !matches!(task, AssignedTask::None);
    let idle_can_move = idle_behavior_can_move(idle, resting_in);
    if can_skip_pathfinding_tick(
        has_task,
        idle_can_move,
        None,
        path,
        destination,
        obstacle_version,
    ) {
        return None;
    }

    has_task
        .then_some(PathRequestClass::ActiveTask)
        .or(Some(PathRequestClass::IdleOrRest))
}

fn collect_pathfinding_work(
    work_queue: &mut RuntimePathWorkQueue,
    obstacle_version: u64,
    query: &mut ParamSet<(PathfindingChangedScan, PathfindingQuery)>,
) {
    let topology_changed = work_queue.obstacle_version != Some(obstacle_version);
    work_queue.obstacle_version = Some(obstacle_version);

    let mut requests = Vec::new();
    let mut cooling_entities = Vec::new();

    if topology_changed {
        // Topology change is the one deliberate all-Soul invalidation. Every
        // steady-state update uses the filtered changed query below.
        for (entity, _, destination, path, task, idle, resting_in, _, cooldown, _) in
            query.p1().iter_mut()
        {
            if cooldown.is_some() {
                cooling_entities.push(entity);
                continue;
            }
            if let Some(class) = request_class_if_needed(
                &task,
                &idle,
                resting_in,
                cooldown.as_deref(),
                &path,
                destination.0,
                obstacle_version,
            ) {
                requests.push((entity, class));
            }
        }
        enqueue_requests_in_entity_order(work_queue, requests, cooling_entities);
        return;
    }

    for (entity, destination, path, task, idle, resting_in, cooldown) in query.p0().iter() {
        if cooldown.is_some() {
            cooling_entities.push(entity);
            continue;
        }
        if let Some(class) = request_class_if_needed(
            task,
            idle,
            resting_in,
            cooldown,
            path,
            destination.0,
            obstacle_version,
        ) {
            requests.push((entity, class));
        }
    }
    enqueue_requests_in_entity_order(work_queue, requests, cooling_entities);
}

fn tick_pathfinding_cooldowns(
    commands: &mut Commands,
    work_queue: &mut RuntimePathWorkQueue,
    query: &mut PathfindingQuery,
    obstacle_version: u64,
) {
    let cooling_count = work_queue.cooling_down.len();
    for _ in 0..cooling_count {
        let Some(entity) = work_queue.pop_cooldown() else {
            break;
        };
        let Ok((_, _, destination, path, task, idle, resting_in, _, cooldown_opt, _)) =
            query.get_mut(entity)
        else {
            work_queue.clear_entity(entity);
            continue;
        };

        let Some(mut cooldown) = cooldown_opt else {
            if let Some(class) = request_class_if_needed(
                &task,
                &idle,
                resting_in,
                None,
                &path,
                destination.0,
                obstacle_version,
            ) {
                work_queue.enqueue(entity, class);
            }
            continue;
        };

        if cooldown.remaining_frames > 0 {
            cooldown.remaining_frames -= 1;
            work_queue.requeue_cooldown(entity);
            continue;
        }

        commands.entity(entity).remove::<PathCooldown>();
        if let Some(class) = request_class_if_needed(
            &task,
            &idle,
            resting_in,
            None,
            &path,
            destination.0,
            obstacle_version,
        ) {
            work_queue.enqueue(entity, class);
        }
    }
}

pub fn pathfinding_system(
    mut commands: Commands,
    resources: PathfindingResources,
    mut query: ParamSet<(PathfindingChangedScan, PathfindingQuery)>,
) {
    let PathfindingResources {
        world_epoch,
        world_map,
        mut pf_context,
        mut budget,
        #[cfg(feature = "profiling")]
        mut defer_metrics,
        mut work_queue,
        rest_areas: q_rest_areas,
        assignment_queries: mut queries,
    } = resources;
    let obstacle_version = world_map.obstacle_version;
    let world_epoch = world_epoch.map_or_else(WorldEpoch::default, |epoch| *epoch);
    let work_queue = work_queue.get_mut(world_epoch);
    #[cfg(feature = "profiling")]
    work_queue.begin_defer_metrics_frame(&defer_metrics);

    collect_pathfinding_work(work_queue, obstacle_version, &mut query);

    let mut query = query.p1();
    tick_pathfinding_cooldowns(&mut commands, work_queue, &mut query, obstacle_version);

    // ActiveTask → IdleOrRest の順に queue を drain する。各 class の FIFO
    // は成功・到達不能後に次の entity へ進み、budget exhaustion のときも
    // 末尾へ戻すため、query 順の先頭が毎フレーム枠を独占しない。
    for class in [PathRequestClass::ActiveTask, PathRequestClass::IdleOrRest] {
        let prioritize_tasks = class == PathRequestClass::ActiveTask;
        budget.begin_phase(phase_budget_limit(prioritize_tasks));
        if budget.used() >= budget.phase_limit() {
            continue;
        }

        while budget.used() < budget.phase_limit() {
            let Some(entity) = work_queue.pop(class) else {
                break;
            };
            let Ok((
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
            )) = query.get_mut(entity)
            else {
                work_queue.clear_entity(entity);
                continue;
            };
            let has_task = !matches!(*task, AssignedTask::None);
            if has_task != prioritize_tasks {
                // The task changed after enqueue. Reclassify it once without
                // throwing away its continuation.
                let class = if has_task {
                    PathRequestClass::ActiveTask
                } else {
                    PathRequestClass::IdleOrRest
                };
                work_queue.enqueue(entity, class);
                continue;
            }

            let idle_can_move = idle_behavior_can_move(&idle, resting_in);

            if can_skip_pathfinding_tick(
                has_task,
                idle_can_move,
                cooldown_opt.as_deref(),
                &path,
                destination.0,
                obstacle_version,
            ) {
                work_queue.finish(entity);
                continue;
            }

            // A loaded/externally inserted cooldown may be discovered before
            // this queue has seen it. Route it through the timer lane.
            if let Some(cooldown) = cooldown_opt.as_mut() {
                if cooldown.remaining_frames > 0 {
                    work_queue.begin_cooldown(entity);
                    continue;
                }
                commands.entity(entity).remove::<PathCooldown>();
            }

            match process_worker_pathfinding(
                &mut commands,
                SoulPfState {
                    entity,
                    transform,
                    destination: &mut destination,
                    path: &mut path,
                    task: &mut task,
                    idle: &mut idle,
                    rest_reserved_for,
                    inventory_opt: inventory_opt.as_deref_mut(),
                },
                WorldPfCtx {
                    world_map: world_map.as_ref(),
                    pf_context: &mut pf_context,
                    budget: &mut budget,
                },
                work_queue,
                &q_rest_areas,
                &mut queries,
            ) {
                WorkerPathfindingOutcome::Finished => {}
                WorkerPathfindingOutcome::CoolingDown => work_queue.begin_cooldown(entity),
                WorkerPathfindingOutcome::Deferred => {
                    #[cfg(feature = "profiling")]
                    work_queue.record_deferred(entity, class, &mut defer_metrics);
                    work_queue.requeue_back(entity, class);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "system/tests.rs"]
mod tests;
