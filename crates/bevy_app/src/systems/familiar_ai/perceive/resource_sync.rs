//! root perceive system: SharedResourceCache の再構築は root の責務として残留。
//!
//! apply_reservation_op / apply_reservation_requests_system は hw_logistics に移設済み。
//! AssignedTask の実ワールド再構築を担うため、
//! hw_familiar_ai 側には置けない。

use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;
use hw_core::constants::RESERVATION_SYNC_INTERVAL;
use hw_core::events::ResourceReservationOp;
use hw_core::logistics::ResourceSourceKey;
use hw_jobs::lifecycle::{self, ReservationSignature};
use std::collections::HashMap;

use bevy::ecs::system::SystemParam;

pub use hw_logistics::SharedResourceCache;
pub use hw_logistics::apply_reservation_op;

#[derive(Resource)]
pub struct ReservationSyncTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for ReservationSyncTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(RESERVATION_SYNC_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

/// 予約を持つ active task だけの前回 signature。
///
/// `Local` では load 後にリセットできないため、load path からも初期化できる
/// root resource として保持する。
#[derive(Resource, Default, Debug)]
pub struct ReservationSignatureCache {
    active: HashMap<Entity, ReservationSignature>,
}

/// profiling capture 中にだけ収集する reservation snapshot 再構築の work counter。
///
/// 通常 build には resource 自体を登録せず、同期経路にも counter 更新を含めない。
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct ReservationSyncPerfMetrics {
    pub full_rebuilds: u64,
    /// Retained for the profiling CSV schema; pending requests no longer reserve capacity.
    pub pending_tasks_scanned: u64,
    pub assigned_tasks_scanned: u64,
}

/// 予約 snapshot 同期の resource 群。
#[derive(SystemParam)]
pub struct ReservationSyncResources<'w> {
    time: Res<'w, Time>,
    sync_timer: ResMut<'w, ReservationSyncTimer>,
    signature_cache: ResMut<'w, ReservationSignatureCache>,
    cache: ResMut<'w, SharedResourceCache>,
    #[cfg(feature = "profiling")]
    perf_metrics: Option<ResMut<'w, ReservationSyncPerfMetrics>>,
}

/// タスク状態から予約を同期するシステム (Sense Phase)
///
/// 割り当て済み`AssignedTask`の予約だけを再構築する。
/// 未割り当てrequestは需要であり容量を占有しない。pending予約を積むと、
/// 最後の空き1個をrequest自身が塞ぎ、割り当て条件を永久に満たせなくなる。
/// 同一cycleの排他はReservationShadow、確定後はpayloadの予約signatureが担う。
pub fn sync_reservations_system(
    q_souls: Query<(Entity, &AssignedTask)>,
    q_assigned_task_changed: Query<(Entity, &AssignedTask), Changed<AssignedTask>>,
    mut removed_assigned_tasks: RemovedComponents<AssignedTask>,
    mut resources: ReservationSyncResources,
) {
    resources.cache.begin_frame();

    let timer_finished = resources
        .sync_timer
        .timer
        .tick(resources.time.delta())
        .just_finished();
    let interval_due = !resources.sync_timer.first_run_done || timer_finished;

    let active_dirty = update_active_reservation_signatures(
        &mut resources.signature_cache.active,
        q_assigned_task_changed.iter(),
        removed_assigned_tasks.read(),
    );

    if resources.sync_timer.first_run_done && !interval_due && !active_dirty {
        return;
    }
    resources.sync_timer.first_run_done = true;

    let mut mixer_dest_res = HashMap::new();
    let mut source_res = HashMap::new();
    #[cfg(feature = "profiling")]
    let mut assigned_tasks_scanned = 0_u64;

    let mut active_signatures = HashMap::new();
    for (entity, task) in q_souls.iter() {
        #[cfg(feature = "profiling")]
        {
            assigned_tasks_scanned = assigned_tasks_scanned.saturating_add(1);
        }
        let signature = lifecycle::active_reservation_signature(task, |_, fallback| fallback);
        for op in signature.active_ops().iter().cloned() {
            apply_active_reservation_op(&mut mixer_dest_res, &mut source_res, op);
        }

        if !signature.is_empty() {
            active_signatures.insert(entity, signature);
        }
    }

    resources.signature_cache.active = active_signatures;
    resources
        .cache
        .replace_reservation_snapshot(mixer_dest_res, source_res);
    #[cfg(feature = "profiling")]
    if let Some(metrics) = resources.perf_metrics.as_mut() {
        metrics.full_rebuilds = metrics.full_rebuilds.saturating_add(1);
        metrics.assigned_tasks_scanned = metrics
            .assigned_tasks_scanned
            .saturating_add(assigned_tasks_scanned);
    }
}

fn update_active_reservation_signatures<'a>(
    signatures: &mut HashMap<Entity, ReservationSignature>,
    changed_tasks: impl Iterator<Item = (Entity, &'a AssignedTask)>,
    removed_assigned_tasks: impl Iterator<Item = Entity>,
) -> bool {
    let mut active_dirty = false;

    for (entity, task) in changed_tasks {
        let signature = lifecycle::active_reservation_signature(task, |_, fallback| fallback);
        if signature.is_empty() {
            active_dirty |= signatures.remove(&entity).is_some();
            continue;
        }

        if signatures.get(&entity) != Some(&signature) {
            signatures.insert(entity, signature);
            active_dirty = true;
        }
    }

    for entity in removed_assigned_tasks {
        signatures.remove(&entity);
        // assignment と despawn が同一フレームで起きると、前回 snapshot に
        // entry がないまま reservation request だけが反映済みの可能性がある。
        // removal は頻度が低いため、ここでは安全側で完全再構築する。
        active_dirty = true;
    }

    active_dirty
}

fn apply_active_reservation_op(
    mixer_dest_res: &mut HashMap<(Entity, ResourceType), usize>,
    source_res: &mut HashMap<ResourceSourceKey, usize>,
    op: ResourceReservationOp,
) {
    match op {
        ResourceReservationOp::ReserveMixerDestination {
            target,
            resource_type,
        } => {
            *mixer_dest_res.entry((target, resource_type)).or_insert(0) += 1;
        }
        ResourceReservationOp::ReserveSource { source, amount } => {
            *source_res.entry(source).or_insert(0) += amount;
        }
        ResourceReservationOp::ReleaseMixerDestination { .. }
        | ResourceReservationOp::ReleaseSource { .. }
        | ResourceReservationOp::RecordPickedSource { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::{GatherData, GatherPhase, WorkType};

    fn test_world() -> World {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(ReservationSyncTimer::default());
        world.insert_resource(ReservationSignatureCache::default());
        world.insert_resource(SharedResourceCache::default());
        #[cfg(feature = "profiling")]
        world.insert_resource(ReservationSyncPerfMetrics::default());
        world
    }

    fn gathering_task(phase: GatherPhase) -> AssignedTask {
        gathering_task_for_target(Entity::PLACEHOLDER, phase)
    }

    fn gathering_task_for_target(target: Entity, phase: GatherPhase) -> AssignedTask {
        AssignedTask::Gather(GatherData {
            target,
            work_type: WorkType::Chop,
            phase,
        })
    }

    #[test]
    fn progress_only_change_keeps_active_reservations_clean() {
        let entity = Entity::PLACEHOLDER;
        let first = gathering_task(GatherPhase::Collecting { progress: 0.1 });
        let later = gathering_task(GatherPhase::Collecting { progress: 0.9 });
        let mut signatures = HashMap::new();

        assert!(update_active_reservation_signatures(
            &mut signatures,
            [(entity, &first)].into_iter(),
            std::iter::empty(),
        ));
        assert!(!update_active_reservation_signatures(
            &mut signatures,
            [(entity, &later)].into_iter(),
            std::iter::empty(),
        ));
    }

    #[test]
    fn completion_and_removal_mark_active_reservations_dirty() {
        let entity = Entity::PLACEHOLDER;
        let collecting = gathering_task(GatherPhase::Collecting { progress: 0.5 });
        let done = gathering_task(GatherPhase::Done);
        let mut signatures = HashMap::new();

        assert!(update_active_reservation_signatures(
            &mut signatures,
            [(entity, &collecting)].into_iter(),
            std::iter::empty(),
        ));
        assert!(update_active_reservation_signatures(
            &mut signatures,
            [(entity, &done)].into_iter(),
            std::iter::empty(),
        ));
        assert!(signatures.is_empty());

        assert!(update_active_reservation_signatures(
            &mut signatures,
            [(entity, &collecting)].into_iter(),
            std::iter::empty(),
        ));
        assert!(update_active_reservation_signatures(
            &mut signatures,
            std::iter::empty(),
            [entity].into_iter(),
        ));
        assert!(signatures.is_empty());
    }

    #[test]
    fn progress_change_does_not_replace_reservation_snapshot() {
        let mut world = test_world();
        let source = world.spawn_empty().id();
        let sentinel = world.spawn_empty().id();
        let soul = world
            .spawn(gathering_task_for_target(
                source,
                GatherPhase::Collecting { progress: 0.1 },
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_reservations_system);

        schedule.run(&mut world);
        world
            .resource_mut::<SharedResourceCache>()
            .reserve_source(sentinel, 4);

        {
            let mut soul_entity = world.entity_mut(soul);
            let mut task = soul_entity.get_mut::<AssignedTask>().unwrap();
            let AssignedTask::Gather(data) = &mut *task else {
                unreachable!("test soul must keep a gathering task");
            };
            data.phase = GatherPhase::Collecting { progress: 0.9 };
        }

        schedule.run(&mut world);

        assert_eq!(
            world
                .resource::<SharedResourceCache>()
                .get_source_reservation(sentinel),
            4
        );
    }

    #[test]
    fn removed_tasks_are_consumed_without_repeating_dirty_sync() {
        let mut world = test_world();
        let first_source = world.spawn_empty().id();
        let second_source = world.spawn_empty().id();
        let sentinel = world.spawn_empty().id();
        let first_soul = world
            .spawn(gathering_task_for_target(
                first_source,
                GatherPhase::Collecting { progress: 0.1 },
            ))
            .id();
        let second_soul = world
            .spawn(gathering_task_for_target(
                second_source,
                GatherPhase::Collecting { progress: 0.1 },
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_reservations_system);

        schedule.run(&mut world);
        world.entity_mut(first_soul).remove::<AssignedTask>();
        world.entity_mut(second_soul).remove::<AssignedTask>();
        schedule.run(&mut world);

        world
            .resource_mut::<SharedResourceCache>()
            .reserve_source(sentinel, 2);
        schedule.run(&mut world);

        assert_eq!(
            world
                .resource::<SharedResourceCache>()
                .get_source_reservation(sentinel),
            2
        );
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn full_rebuild_metrics_count_only_executed_sweeps() {
        let mut world = test_world();
        let source = world.spawn_empty().id();
        world.spawn(gathering_task_for_target(
            source,
            GatherPhase::Collecting { progress: 0.1 },
        ));
        let mut schedule = Schedule::default();
        schedule.add_systems(sync_reservations_system);

        schedule.run(&mut world);
        {
            let metrics = world.resource::<ReservationSyncPerfMetrics>();
            assert_eq!(metrics.full_rebuilds, 1);
            assert_eq!(metrics.pending_tasks_scanned, 0);
            assert_eq!(metrics.assigned_tasks_scanned, 1);
        }

        schedule.run(&mut world);
        let metrics = world.resource::<ReservationSyncPerfMetrics>();
        assert_eq!(metrics.full_rebuilds, 1);
        assert_eq!(metrics.pending_tasks_scanned, 0);
        assert_eq!(metrics.assigned_tasks_scanned, 1);
    }
}
