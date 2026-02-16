use crate::constants::RESERVATION_SYNC_INTERVAL;
use crate::events::ResourceReservationOp;
use crate::events::ResourceReservationRequest;
use crate::relationships::TaskWorkers;
use crate::systems::jobs::{Designation, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{TransportRequest, TransportRequestKind};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::execute::task_execution::transport_common::lifecycle;
use bevy::prelude::*;
use std::collections::HashMap;

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

/// システム全体で共有されるリソース予約キャッシュ
///
/// タスク間（運搬、建築、採集など）でのリソース競合を防ぐために使用される。
/// Senseフェーズで再構築され、Thinkフェーズで仮予約、Actフェーズで確定更新される。
#[derive(Resource, Default, Debug)]
pub struct SharedResourceCache {
    /// ミキサー等への予約数 (Destination Reservation)
    /// (Entity, ResourceType) -> 搬送予定数
    mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,

    /// リソース/タンクからの取り出し予約数 (Source Reservation)
    /// Entity -> 取り出し予定数
    source_reservations: HashMap<Entity, usize>,

    /// このフレームで格納された数（コンポーネント未反映分）
    /// Entity -> 格納数
    frame_stored_count: HashMap<Entity, usize>,

    /// このフレームで取り出された数（コンポーネント未反映分）
    /// Entity -> 取り出し数
    frame_picked_count: HashMap<Entity, usize>,
}

impl SharedResourceCache {
    /// 外部から予約状況を再構築する（Senseフェーズ用）
    pub fn reset(
        &mut self,
        mixer_dest_reservations: HashMap<(Entity, ResourceType), usize>,
        source_reservations: HashMap<Entity, usize>,
    ) {
        self.mixer_dest_reservations = mixer_dest_reservations;
        self.source_reservations = source_reservations;
        self.frame_stored_count.clear();
        self.frame_picked_count.clear();
    }

    /// ミキサーへの予約を追加
    pub fn reserve_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        *self
            .mixer_dest_reservations
            .entry((target, resource_type))
            .or_insert(0) += 1;
    }

    /// ミキサーへの予約を解除
    pub fn release_mixer_destination(&mut self, target: Entity, resource_type: ResourceType) {
        let key = (target, resource_type);
        if let Some(count) = self.mixer_dest_reservations.get_mut(&key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.mixer_dest_reservations.remove(&key);
            }
        }
    }

    /// ミキサーへの予約数を取得
    pub fn get_mixer_destination_reservation(
        &self,
        target: Entity,
        resource_type: ResourceType,
    ) -> usize {
        self.mixer_dest_reservations
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    /// リソース取り出し予約を追加 (Source Reservation)
    pub fn reserve_source(&mut self, source: Entity, amount: usize) {
        *self.source_reservations.entry(source).or_insert(0) += amount;
    }

    /// リソース取り出し予約を解除
    pub fn release_source(&mut self, source: Entity, amount: usize) {
        if let Some(count) = self.source_reservations.get_mut(&source) {
            *count = count.saturating_sub(amount);
            if *count == 0 {
                self.source_reservations.remove(&source);
            }
        }
    }

    /// リソース取り出し予約数を取得（予約済み + このフレームで取得済み）
    pub fn get_source_reservation(&self, source: Entity) -> usize {
        let reserved = self.source_reservations.get(&source).cloned().unwrap_or(0);
        let picked = self.frame_picked_count.get(&source).cloned().unwrap_or(0);
        reserved + picked
    }

    /// 取得アクション成功を記録 (Delta Update)
    /// ソース予約を減らし、フレーム内取得数を増やす（論理在庫減少）
    pub fn record_picked_source(&mut self, source: Entity, amount: usize) {
        self.release_source(source, amount);
        *self.frame_picked_count.entry(source).or_insert(0) += amount;
    }

    /// 論理的な格納済み数（クエリ値 + フレーム内増加分）を取得
    pub fn get_logical_stored_count(&self, target: Entity, current_from_query: usize) -> usize {
        let frame_added = self.frame_stored_count.get(&target).cloned().unwrap_or(0);
        current_from_query + frame_added
    }
}

/// 予約操作をキャッシュに反映する
pub fn apply_reservation_op(cache: &mut SharedResourceCache, op: &ResourceReservationOp) {
    match *op {
        ResourceReservationOp::ReserveMixerDestination {
            target,
            resource_type,
        } => {
            cache.reserve_mixer_destination(target, resource_type);
        }
        ResourceReservationOp::ReleaseMixerDestination {
            target,
            resource_type,
        } => {
            cache.release_mixer_destination(target, resource_type);
        }
        ResourceReservationOp::ReserveSource { source, amount } => {
            cache.reserve_source(source, amount);
        }
        ResourceReservationOp::ReleaseSource { source, amount } => {
            cache.release_source(source, amount);
        }
        ResourceReservationOp::RecordPickedSource { source, amount } => {
            cache.record_picked_source(source, amount);
        }
    }
}

/// 予約更新リクエストを反映するシステム
pub fn apply_reservation_requests_system(
    mut cache: ResMut<SharedResourceCache>,
    mut requests: MessageReader<ResourceReservationRequest>,
) {
    for request in requests.read() {
        apply_reservation_op(&mut cache, &request.op);
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
    mut cache: ResMut<SharedResourceCache>,
) {
    let timer_finished = sync_timer.timer.tick(time.delta()).just_finished();
    if sync_timer.first_run_done && !timer_finished {
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
