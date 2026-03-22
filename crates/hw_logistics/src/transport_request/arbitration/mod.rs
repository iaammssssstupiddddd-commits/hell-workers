//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

mod candidates;
mod collection;
mod grants;
mod lease_state;
mod metrics_update;
mod system;
mod types;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::{IncomingDeliveries, ParkedAt, PushedBy, StoredIn, StoredItems};
use hw_jobs::Designation;

use crate::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportRequest,
    TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::{BelongsTo, ReservedForTask, ResourceItem, Wheelbarrow};
use crate::zone::Stockpile;

pub use system::wheelbarrow_arbitration_system;

#[derive(Default)]
pub struct WheelbarrowArbitrationRuntime {
    initialized: bool,
    last_full_eval_secs: f64,
}

#[derive(SystemParam)]
pub struct WheelbarrowArbitrationDirtyParams<'w, 's> {
    q_request_dirty: Query<
        'w,
        's,
        (),
        (
            With<TransportRequest>,
            Or<(
                Added<TransportRequest>,
                Changed<TransportRequest>,
                Changed<TransportRequestState>,
                Changed<TransportDemand>,
                Changed<Transform>,
                Added<WheelbarrowLease>,
                Changed<WheelbarrowLease>,
                Added<WheelbarrowPendingSince>,
                Changed<WheelbarrowPendingSince>,
                Added<ManualTransportRequest>,
            )>,
        ),
    >,
    q_free_item_dirty: Query<
        'w,
        's,
        (),
        (
            With<ResourceItem>,
            Or<(
                Added<ResourceItem>,
                Changed<ResourceItem>,
                Changed<Transform>,
                Changed<Visibility>,
                Added<ReservedForTask>,
                Changed<ReservedForTask>,
                Added<ManualHaulPinnedSource>,
                Changed<ManualHaulPinnedSource>,
                Added<BelongsTo>,
                Changed<BelongsTo>,
                Added<StoredIn>,
                Changed<StoredIn>,
                Added<Designation>,
                Changed<Designation>,
            )>,
        ),
    >,
    q_wheelbarrow_dirty: Query<
        'w,
        's,
        (),
        (
            With<Wheelbarrow>,
            Or<(
                Added<Wheelbarrow>,
                Changed<Transform>,
                Added<ParkedAt>,
                Changed<ParkedAt>,
                Added<PushedBy>,
                Changed<PushedBy>,
            )>,
        ),
    >,
    q_stockpile_dirty: Query<
        'w,
        's,
        (),
        (
            With<Stockpile>,
            Or<(
                Added<Stockpile>,
                Changed<Stockpile>,
                Added<StoredItems>,
                Changed<StoredItems>,
                Added<IncomingDeliveries>,
                Changed<IncomingDeliveries>,
            )>,
        ),
    >,
    q_resource_entities: Query<'w, 's, (), With<ResourceItem>>,
    q_wheelbarrow_entities: Query<'w, 's, (), With<Wheelbarrow>>,
    removed_requests: RemovedComponents<'w, 's, TransportRequest>,
    removed_resource_items: RemovedComponents<'w, 's, ResourceItem>,
    removed_wheelbarrows: RemovedComponents<'w, 's, Wheelbarrow>,
    removed_leases: RemovedComponents<'w, 's, WheelbarrowLease>,
    removed_reserved_for_task: RemovedComponents<'w, 's, ReservedForTask>,
    removed_pinned_source: RemovedComponents<'w, 's, ManualHaulPinnedSource>,
    removed_belongs: RemovedComponents<'w, 's, BelongsTo>,
    removed_stored_in: RemovedComponents<'w, 's, StoredIn>,
    removed_designations: RemovedComponents<'w, 's, Designation>,
    removed_parked_at: RemovedComponents<'w, 's, ParkedAt>,
    removed_pushed_by: RemovedComponents<'w, 's, PushedBy>,
    removed_stored_items: RemovedComponents<'w, 's, StoredItems>,
    removed_incoming: RemovedComponents<'w, 's, IncomingDeliveries>,
}
