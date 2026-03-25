//! root perceive system: SharedResourceCache の再構築は root の責務として残留。
//!
//! apply_reservation_op / apply_reservation_requests_system は hw_logistics に移設済み。
//! AssignedTask / Designation / TransportRequest / relationship の実ワールド再構築を担うため、
//! hw_familiar_ai 側には置けない。

use crate::systems::jobs::{Designation, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use bevy::prelude::*;
use hw_core::constants::RESERVATION_SYNC_INTERVAL;
use hw_core::events::ResourceReservationOp;
use hw_core::relationships::TaskWorkers;
use hw_jobs::lifecycle;
use std::collections::HashMap;

use bevy::ecs::system::SystemParam;

pub use hw_logistics::SharedResourceCache;
pub use hw_logistics::apply_reservation_op;

type PendingDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<Designation>,
        Without<TaskWorkers>,
        Or<(
            Added<Designation>,
            Changed<Designation>,
            Added<TransportRequest>,
            Changed<TransportRequest>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub struct DirtyCheckQueries<'w, 's> {
    q_pending_dirty: PendingDirtyQuery<'w, 's>,
    q_task_workers_added: Query<'w, 's, (), (With<Designation>, Added<TaskWorkers>)>,
    q_assigned_task_added: Query<'w, 's, (), Added<AssignedTask>>,
    q_assigned_task_changed: Query<'w, 's, (), Changed<AssignedTask>>,
}

#[derive(SystemParam)]
pub struct RemovedTrackings<'w, 's> {
    removed_designations: RemovedComponents<'w, 's, Designation>,
    removed_transport_requests: RemovedComponents<'w, 's, TransportRequest>,
    removed_task_workers: RemovedComponents<'w, 's, TaskWorkers>,
    removed_assigned_tasks: RemovedComponents<'w, 's, AssignedTask>,
}

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

/// タスク状態から予約を同期するシステム (Sense Phase)
///
/// 以下の2種類のソースから予約を再構築する:
/// 1. `AssignedTask` - 既にSoulに割り当てられているタスク
/// 2. `Designation` (Without<TaskWorkers>) - まだ割り当て待ちのタスク候補
///
/// これにより、自動発行システムが複数フレームにわたって過剰にタスクを発行することを防ぐ。
pub fn sync_reservations_system(
    time: Res<Time>,
    mut sync_timer: ResMut<ReservationSyncTimer>,
    q_souls: Query<&AssignedTask>,
    q_pending_tasks: Query<(&Designation, Option<&TransportRequest>), Without<TaskWorkers>>,
    dirty_checks: DirtyCheckQueries,
    mut removed: RemovedTrackings,
    mut cache: ResMut<SharedResourceCache>,
) {
    let DirtyCheckQueries {
        q_pending_dirty,
        q_task_workers_added,
        q_assigned_task_added,
        q_assigned_task_changed,
    } = dirty_checks;
    let timer_finished = sync_timer.timer.tick(time.delta()).just_finished();
    let interval_due = !sync_timer.first_run_done || timer_finished;
    let pending_dirty = q_pending_dirty.iter().next().is_some()
        || q_task_workers_added.iter().next().is_some()
        || removed.removed_designations.read().next().is_some()
        || removed.removed_transport_requests.read().next().is_some()
        || removed.removed_task_workers.read().next().is_some();
    let active_dirty = q_assigned_task_added.iter().next().is_some()
        || q_assigned_task_changed.iter().next().is_some()
        || removed.removed_assigned_tasks.read().next().is_some();

    if sync_timer.first_run_done && !interval_due && !pending_dirty && !active_dirty {
        return;
    }
    sync_timer.first_run_done = true;

    let mut mixer_dest_res = HashMap::new();
    let mut source_res = HashMap::new();

    // request エンティティ起点で pending 予約を再構築する。
    for (designation, transport_req) in q_pending_tasks.iter() {
        let is_transport_designation = matches!(
            designation.work_type,
            WorkType::Haul
                | WorkType::HaulToMixer
                | WorkType::GatherWater
                | WorkType::HaulWaterToMixer
                | WorkType::WheelbarrowHaul
        );
        if !is_transport_designation {
            continue;
        }
        let Some(req) = transport_req else {
            continue;
        };
        match req.kind {
            TransportRequestKind::DepositToStockpile
            | TransportRequestKind::DeliverToBlueprint
            | TransportRequestKind::DeliverToFloorConstruction
            | TransportRequestKind::DeliverToWallConstruction
            | TransportRequestKind::DeliverToProvisionalWall
            | TransportRequestKind::GatherWaterToTank
            | TransportRequestKind::ConsolidateStockpile => {
                // DeliveringTo リレーションシップを使用するため、ここでは HashMap に積まない
            }
            TransportRequestKind::DeliverToMixerSolid => {
                *mixer_dest_res
                    .entry((req.anchor, req.resource_type))
                    .or_insert(0) += 1;
            }
            TransportRequestKind::DeliverWaterToMixer => {
                *mixer_dest_res
                    .entry((req.anchor, ResourceType::Water))
                    .or_insert(0) += 1;
            }
            // ReturnBucket は返却先 BucketStorage を割り当て時に確定するため、
            // pending request 段階では destination 予約を積まない。
            TransportRequestKind::ReturnBucket
            | TransportRequestKind::ReturnWheelbarrow
            | TransportRequestKind::BatchWheelbarrow => {}
        }
    }

    for task in q_souls.iter() {
        for op in lifecycle::collect_active_reservation_ops(task, |_, fallback| fallback) {
            apply_active_reservation_op(&mut mixer_dest_res, &mut source_res, op);
        }
    }

    cache.reset(mixer_dest_res, source_res);
}

fn apply_active_reservation_op(
    mixer_dest_res: &mut HashMap<(Entity, ResourceType), usize>,
    source_res: &mut HashMap<Entity, usize>,
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
