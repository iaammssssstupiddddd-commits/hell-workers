use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;

use super::super::fallback;

/// Actor phase 内の経路探索要求を保持する class 別 FIFO。
///
/// Task/idle を別々に保持することで、task phase の累積 ceiling と idle
/// reserve を維持しつつ、同 class の要求を query 順へ毎フレーム戻さない。
#[derive(Default)]
pub(super) struct RuntimePathWorkQueue {
    pub(super) active_task: VecDeque<Entity>,
    pub(super) idle_or_rest: VecDeque<Entity>,
    pub(super) cooling_down: VecDeque<Entity>,
    pub(super) queued: HashSet<Entity>,
    pub(super) cooling: HashSet<Entity>,
    pub(super) continuations: HashMap<Entity, ActorPathContinuation>,
    pub(super) obstacle_version: Option<u64>,
    #[cfg(feature = "profiling")]
    pub(super) defer_started_at: HashMap<Entity, (PathRequestClass, u64)>,
    #[cfg(feature = "profiling")]
    pub(super) defer_frame: u64,
    #[cfg(feature = "profiling")]
    pub(super) defer_metrics_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PathRequestClass {
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

    pub(super) fn record(&mut self, class: PathRequestClass, defer_frames: u64) {
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
pub(super) struct ActorPathFingerprint {
    pub(super) start_grid: (i32, i32),
    pub(super) goal_grid: (i32, i32),
    pub(super) destination: Vec2,
    pub(super) obstacle_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActorPathStage {
    Direct,
    Adjacent,
    RestFallback,
}

#[derive(Debug, PartialEq)]
pub(super) struct ActorPathContinuation {
    fingerprint: ActorPathFingerprint,
    stage: ActorPathStage,
    rest_fallback: Option<fallback::RestFallbackProgress>,
}

impl RuntimePathWorkQueue {
    pub(super) fn enqueue(&mut self, entity: Entity, class: PathRequestClass) {
        if !self.queued.insert(entity) {
            return;
        }

        match class {
            PathRequestClass::ActiveTask => self.active_task.push_back(entity),
            PathRequestClass::IdleOrRest => self.idle_or_rest.push_back(entity),
        }
    }

    pub(super) fn requeue_back(&mut self, entity: Entity, class: PathRequestClass) {
        self.queued.insert(entity);
        match class {
            PathRequestClass::ActiveTask => self.active_task.push_back(entity),
            PathRequestClass::IdleOrRest => self.idle_or_rest.push_back(entity),
        }
    }

    pub(super) fn pop(&mut self, class: PathRequestClass) -> Option<Entity> {
        let entity = match class {
            PathRequestClass::ActiveTask => self.active_task.pop_front(),
            PathRequestClass::IdleOrRest => self.idle_or_rest.pop_front(),
        }?;
        self.queued.remove(&entity);
        Some(entity)
    }

    pub(super) fn begin_cooldown(&mut self, entity: Entity) {
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
        if self.cooling.insert(entity) {
            self.cooling_down.push_back(entity);
        }
    }

    pub(super) fn pop_cooldown(&mut self) -> Option<Entity> {
        while let Some(entity) = self.cooling_down.pop_front() {
            if self.cooling.remove(&entity) {
                return Some(entity);
            }
        }
        None
    }

    pub(super) fn requeue_cooldown(&mut self, entity: Entity) {
        if self.cooling.insert(entity) {
            self.cooling_down.push_back(entity);
        }
    }

    pub(super) fn clear_entity(&mut self, entity: Entity) {
        self.queued.remove(&entity);
        self.cooling.remove(&entity);
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
    }

    pub(super) fn stage_for(
        &mut self,
        entity: Entity,
        fingerprint: ActorPathFingerprint,
    ) -> ActorPathStage {
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

    pub(super) fn advance_to_adjacent(&mut self, entity: Entity) {
        if let Some(continuation) = self.continuations.get_mut(&entity) {
            continuation.stage = ActorPathStage::Adjacent;
        }
    }

    pub(super) fn begin_rest_fallback(&mut self, entity: Entity) {
        if let Some(continuation) = self.continuations.get_mut(&entity) {
            continuation.stage = ActorPathStage::RestFallback;
        }
    }

    pub(super) fn rest_fallback_progress(
        &mut self,
        entity: Entity,
    ) -> &mut Option<fallback::RestFallbackProgress> {
        &mut self
            .continuations
            .get_mut(&entity)
            .expect("path continuation exists before rest fallback")
            .rest_fallback
    }

    pub(super) fn finish(&mut self, entity: Entity) {
        self.continuations.remove(&entity);
        #[cfg(feature = "profiling")]
        self.defer_started_at.remove(&entity);
    }

    #[cfg(feature = "profiling")]
    pub(super) fn begin_defer_metrics_frame(&mut self, metrics: &RuntimePathDeferMetrics) {
        self.defer_frame = self.defer_frame.saturating_add(1);
        if self.defer_metrics_generation != metrics.generation {
            self.defer_started_at.clear();
            self.defer_metrics_generation = metrics.generation;
        }
    }

    #[cfg(feature = "profiling")]
    pub(super) fn record_deferred(
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
pub(super) fn compare_entity_keys(left: Entity, right: Entity) -> Ordering {
    left.index_u32().cmp(&right.index_u32()).then_with(|| {
        left.generation()
            .to_bits()
            .cmp(&right.generation().to_bits())
    })
}

pub(super) fn enqueue_requests_in_entity_order(
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
