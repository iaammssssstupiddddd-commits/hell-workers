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
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use crate::soul_ai::execute::task_execution::AssignedTask;

use super::{PathCooldown, TASK_PATHFINDS_PHASE_LIMIT, fallback, reuse};

/// Actor phase 内の経路探索要求を保持する class 別 FIFO。
///
/// Task/idle を別々に保持することで、task phase の累積 ceiling と idle
/// reserve を維持しつつ、同 class の要求を query 順へ毎フレーム戻さない。
#[derive(Default)]
pub struct RuntimePathWorkQueue {
    active_task: VecDeque<Entity>,
    idle_or_rest: VecDeque<Entity>,
    cooling_down: VecDeque<Entity>,
    queued: HashSet<Entity>,
    cooling: HashSet<Entity>,
    continuations: HashMap<Entity, ActorPathContinuation>,
    obstacle_version: Option<u64>,
    #[cfg(feature = "profiling")]
    defer_started_at: HashMap<Entity, (PathRequestClass, u64)>,
    #[cfg(feature = "profiling")]
    defer_frame: u64,
    #[cfg(feature = "profiling")]
    defer_metrics_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathRequestClass {
    ActiveTask,
    IdleOrRest,
}

/// Capture-period wait observations for Actor path requests.
///
/// A value is the number of Update frames from the first budget deferral until
/// the latest retry. It is intentionally separate from per-core-request
/// `RuntimePathSearchMetrics`: a direct/adjacent continuation may issue more
/// than one deferred core request while still being one waiting actor.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePathDeferMetrics {
    pub active_task_max_defer_frames: u64,
    pub idle_or_rest_max_defer_frames: u64,
    pub deferred_actor_retries: u64,
    generation: u64,
}

#[cfg(feature = "profiling")]
impl RuntimePathDeferMetrics {
    pub fn clear(&mut self) {
        self.active_task_max_defer_frames = 0;
        self.idle_or_rest_max_defer_frames = 0;
        self.deferred_actor_retries = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    fn record(&mut self, class: PathRequestClass, defer_frames: u64) {
        self.deferred_actor_retries = self.deferred_actor_retries.saturating_add(1);
        match class {
            PathRequestClass::ActiveTask => {
                self.active_task_max_defer_frames =
                    self.active_task_max_defer_frames.max(defer_frames);
            }
            PathRequestClass::IdleOrRest => {
                self.idle_or_rest_max_defer_frames =
                    self.idle_or_rest_max_defer_frames.max(defer_frames);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActorPathFingerprint {
    start_grid: (i32, i32),
    goal_grid: (i32, i32),
    destination: Vec2,
    obstacle_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActorPathStage {
    Direct,
    Adjacent,
    RestFallback,
}

#[derive(Debug, PartialEq)]
struct ActorPathContinuation {
    fingerprint: ActorPathFingerprint,
    stage: ActorPathStage,
    rest_fallback: Option<fallback::RestFallbackProgress>,
}

impl RuntimePathWorkQueue {
    fn enqueue(&mut self, entity: Entity, class: PathRequestClass) {
        if !self.queued.insert(entity) {
            return;
        }

        match class {
            PathRequestClass::ActiveTask => self.active_task.push_back(entity),
            PathRequestClass::IdleOrRest => self.idle_or_rest.push_back(entity),
        }
    }

    fn requeue_back(&mut self, entity: Entity, class: PathRequestClass) {
        self.queued.insert(entity);
        match class {
            PathRequestClass::ActiveTask => self.active_task.push_back(entity),
            PathRequestClass::IdleOrRest => self.idle_or_rest.push_back(entity),
        }
    }

    fn pop(&mut self, class: PathRequestClass) -> Option<Entity> {
        let entity = match class {
            PathRequestClass::ActiveTask => self.active_task.pop_front(),
            PathRequestClass::IdleOrRest => self.idle_or_rest.pop_front(),
        }?;
        self.queued.remove(&entity);
        Some(entity)
    }

    fn begin_cooldown(&mut self, entity: Entity) {
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
        if self.cooling.insert(entity) {
            self.cooling_down.push_back(entity);
        }
    }

    fn pop_cooldown(&mut self) -> Option<Entity> {
        while let Some(entity) = self.cooling_down.pop_front() {
            if self.cooling.remove(&entity) {
                return Some(entity);
            }
        }
        None
    }

    fn requeue_cooldown(&mut self, entity: Entity) {
        if self.cooling.insert(entity) {
            self.cooling_down.push_back(entity);
        }
    }

    fn clear_entity(&mut self, entity: Entity) {
        self.queued.remove(&entity);
        self.cooling.remove(&entity);
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
    }

    fn stage_for(&mut self, entity: Entity, fingerprint: ActorPathFingerprint) -> ActorPathStage {
        let continuation = self
            .continuations
            .entry(entity)
            .or_insert(ActorPathContinuation {
                fingerprint,
                stage: ActorPathStage::Direct,
                rest_fallback: None,
            });
        if continuation.fingerprint != fingerprint {
            *continuation = ActorPathContinuation {
                fingerprint,
                stage: ActorPathStage::Direct,
                rest_fallback: None,
            };
        }
        continuation.stage
    }

    fn advance_to_adjacent(&mut self, entity: Entity) {
        if let Some(continuation) = self.continuations.get_mut(&entity) {
            continuation.stage = ActorPathStage::Adjacent;
        }
    }

    fn begin_rest_fallback(&mut self, entity: Entity) {
        if let Some(continuation) = self.continuations.get_mut(&entity) {
            continuation.stage = ActorPathStage::RestFallback;
        }
    }

    fn rest_fallback_progress(
        &mut self,
        entity: Entity,
    ) -> &mut Option<fallback::RestFallbackProgress> {
        &mut self
            .continuations
            .get_mut(&entity)
            .expect("path continuation exists before rest fallback")
            .rest_fallback
    }

    fn finish(&mut self, entity: Entity) {
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
    }

    #[cfg(feature = "profiling")]
    fn begin_defer_metrics_frame(&mut self, metrics: &RuntimePathDeferMetrics) {
        self.defer_frame = self.defer_frame.saturating_add(1);
        if self.defer_metrics_generation != metrics.generation {
            self.defer_started_at.clear();
            self.defer_metrics_generation = metrics.generation;
        }
    }

    #[cfg(feature = "profiling")]
    fn record_deferred(
        &mut self,
        entity: Entity,
        class: PathRequestClass,
        metrics: &mut RuntimePathDeferMetrics,
    ) {
        let (tracked_class, first_defer_frame) = self
            .defer_started_at
            .entry(entity)
            .or_insert((class, self.defer_frame));
        if *tracked_class != class {
            *tracked_class = class;
            *first_defer_frame = self.defer_frame;
        }
        metrics.record(
            class,
            self.defer_frame.saturating_sub(*first_defer_frame) + 1,
        );
    }
}

/// 固定step監査と通常実行の両方で、queueへの初回投入順を全順序にする。
///
/// Changed query のarchetype順やHashSet由来の要求順を、そのままcore A* の予算
/// 競合順にしてはいけない。entity index/generation は同一world内で一意なので、
/// 各class FIFOの初回順序をここで固定する。
fn compare_entity_keys(left: Entity, right: Entity) -> Ordering {
    left.index_u32().cmp(&right.index_u32()).then_with(|| {
        left.generation()
            .to_bits()
            .cmp(&right.generation().to_bits())
    })
}

fn enqueue_requests_in_entity_order(
    work_queue: &mut RuntimePathWorkQueue,
    mut requests: Vec<(Entity, PathRequestClass)>,
    mut cooling_entities: Vec<Entity>,
) {
    requests.sort_unstable_by(|(left, _), (right, _)| compare_entity_keys(*left, *right));
    cooling_entities.sort_unstable_by(|left, right| compare_entity_keys(*left, *right));

    for entity in cooling_entities {
        work_queue.begin_cooldown(entity);
    }
    for (entity, class) in requests {
        work_queue.enqueue(entity, class);
    }
}

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

/// 1 worker のパス探索処理（cooldown 処理後に呼ぶ）。
///
/// Budget exhaustion is not an unreachable destination: `Deferred` retains
/// the existing state for a later frame.
fn process_worker_pathfinding(
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
mod tests {
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::constants::MAP_HEIGHT;
    use hw_core::events::{ResourceReservationOp, ResourceReservationRequest};
    use hw_core::relationships::WorkingOn;
    use hw_jobs::events::TaskAssignmentRequest;
    use hw_jobs::{ActiveTaskIdentity, GeneratePowerData, GeneratePowerPhase, WorkType};
    use hw_logistics::SharedResourceCache;

    #[derive(Resource, Default)]
    struct ReservationReceipts(Vec<ResourceReservationOp>);

    #[derive(Resource, Default)]
    struct BudgetClaimResults(Vec<bool>);

    fn collect_reservations(
        mut reservations: MessageReader<ResourceReservationRequest>,
        mut receipts: ResMut<ReservationReceipts>,
    ) {
        receipts
            .0
            .extend(reservations.read().map(|request| request.op.clone()));
    }

    fn claim_runtime_budget(
        mut budget: ResMut<RuntimePathSearchBudget>,
        mut results: ResMut<BudgetClaimResults>,
    ) {
        results.0.push(budget.try_claim());
    }

    #[test]
    fn preupdate_reset_restores_the_actor_budget_each_frame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(RuntimePathSearchBudget::new(1))
            .init_resource::<BudgetClaimResults>()
            .add_systems(PreUpdate, reset_runtime_path_search_budget_system)
            .add_systems(Update, claim_runtime_budget);

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<BudgetClaimResults>().0,
            vec![true, true]
        );
        assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
    }

    #[test]
    fn actor_work_queue_keeps_fifo_and_drops_entity_state_on_world_epoch_change() {
        let first = Entity::from_bits(1);
        let second = Entity::from_bits(2);
        let mut epoch = WorldEpoch::default();
        let mut local = EpochLocal::<RuntimePathWorkQueue>::default();

        let queue = local.get_mut(epoch);
        queue.enqueue(first, PathRequestClass::ActiveTask);
        queue.enqueue(second, PathRequestClass::ActiveTask);
        assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(first));
        queue.requeue_back(first, PathRequestClass::ActiveTask);
        assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(second));
        assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(first));

        queue.enqueue(first, PathRequestClass::IdleOrRest);
        queue.begin_cooldown(second);
        epoch.advance();

        let reset = local.get_mut(epoch);
        assert!(reset.active_task.is_empty());
        assert!(reset.idle_or_rest.is_empty());
        assert!(reset.cooling_down.is_empty());
        assert!(reset.continuations.is_empty());
    }

    #[test]
    fn initial_queue_admission_uses_entity_order_per_class() {
        let entity = |index| Entity::from_raw_u32(index).expect("test entity index is valid");
        let first = entity(1);
        let second = entity(2);
        let third = entity(3);
        let fourth = entity(4);
        let fifth = entity(5);
        let mut queue = RuntimePathWorkQueue::default();

        enqueue_requests_in_entity_order(
            &mut queue,
            vec![
                (fourth, PathRequestClass::IdleOrRest),
                (third, PathRequestClass::ActiveTask),
                (first, PathRequestClass::IdleOrRest),
                (second, PathRequestClass::ActiveTask),
            ],
            vec![fifth, third],
        );

        assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(second));
        assert_eq!(queue.pop(PathRequestClass::ActiveTask), Some(third));
        assert_eq!(queue.pop(PathRequestClass::IdleOrRest), Some(first));
        assert_eq!(queue.pop(PathRequestClass::IdleOrRest), Some(fourth));
        assert_eq!(queue.pop_cooldown(), Some(third));
        assert_eq!(queue.pop_cooldown(), Some(fifth));
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn defer_metrics_measure_actor_wait_and_reset_at_capture_boundary() {
        let entity = Entity::from_raw_u32(1).expect("test entity index is valid");
        let mut queue = RuntimePathWorkQueue::default();
        let mut metrics = RuntimePathDeferMetrics::default();

        queue.begin_defer_metrics_frame(&metrics);
        queue.record_deferred(entity, PathRequestClass::ActiveTask, &mut metrics);
        queue.begin_defer_metrics_frame(&metrics);
        queue.record_deferred(entity, PathRequestClass::ActiveTask, &mut metrics);

        assert_eq!(metrics.active_task_max_defer_frames, 2);
        assert_eq!(metrics.deferred_actor_retries, 2);

        metrics.clear();
        queue.begin_defer_metrics_frame(&metrics);
        queue.record_deferred(entity, PathRequestClass::IdleOrRest, &mut metrics);

        assert_eq!(metrics.active_task_max_defer_frames, 0);
        assert_eq!(metrics.idle_or_rest_max_defer_frames, 1);
        assert_eq!(metrics.deferred_actor_retries, 1);
    }

    #[test]
    fn unreachable_task_destination_unassigns_and_releases_its_reservation() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .init_resource::<RuntimePathSearchBudget>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<ReservationReceipts>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(
                Update,
                (pathfinding_system, ApplyDeferred, collect_reservations).chain(),
            );

        let target = app.world_mut().spawn_empty().id();
        let start = WorldMap::grid_to_world(10, 12);
        let soul = app
            .world_mut()
            .spawn((
                Transform::from_translation(start.extend(0.0)),
                DamnedSoul::default(),
                Destination(WorldMap::grid_to_world(-1, 12)),
                Path::default(),
                AssignedTask::GeneratePower(GeneratePowerData {
                    tile: target,
                    tile_pos: start,
                    phase: GeneratePowerPhase::Generating,
                }),
                IdleState::default(),
                ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
                WorkingOn(target),
            ))
            .id();

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert!(app.world().get::<PathCooldown>(soul).is_some());
        assert_eq!(
            app.world().resource::<ReservationReceipts>().0,
            vec![ResourceReservationOp::ReleaseSource {
                source: target,
                amount: 1,
            }]
        );
    }

    #[test]
    fn exhausted_core_budget_defers_task_pathfinding_without_unassigning() {
        let mut blocked_map = WorldMap::default();
        for y in 0..MAP_HEIGHT {
            blocked_map.add_grid_obstacle((50, y));
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(blocked_map)
            .insert_resource(RuntimePathSearchBudget::new(1))
            .init_resource::<SharedResourceCache>()
            .init_resource::<ReservationReceipts>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskAssignmentRequest>()
            .add_systems(
                Update,
                (pathfinding_system, ApplyDeferred, collect_reservations).chain(),
            );

        let target = app.world_mut().spawn_empty().id();
        let start = WorldMap::grid_to_world(25, 50);
        let destination = WorldMap::grid_to_world(75, 50);
        let soul = app
            .world_mut()
            .spawn((
                Transform::from_translation(start.extend(0.0)),
                DamnedSoul::default(),
                Destination(destination),
                Path::default(),
                AssignedTask::GeneratePower(GeneratePowerData {
                    tile: target,
                    tile_pos: destination,
                    phase: GeneratePowerPhase::Generating,
                }),
                IdleState::default(),
                ActiveTaskIdentity::new(target, target, WorkType::GeneratePower),
                WorkingOn(target),
            ))
            .id();

        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::GeneratePower(_))
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_some());
        assert!(app.world().get::<WorkingOn>(soul).is_some());
        assert_eq!(
            app.world()
                .get::<Destination>(soul)
                .map(|destination| destination.0),
            Some(destination)
        );
        assert!(app.world().get::<PathCooldown>(soul).is_none());
        assert_eq!(app.world().resource::<RuntimePathSearchBudget>().used(), 1);
        assert!(app.world().resource::<ReservationReceipts>().0.is_empty());

        // The first frame consumed direct A* and deferred its adjacent
        // fallback. After a new frame budget, the continuation must resume at
        // adjacent; retrying direct here would leave the task assigned again.
        app.world_mut()
            .resource_mut::<RuntimePathSearchBudget>()
            .reset();
        app.update();

        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<PathCooldown>(soul).is_some());
    }
}
