//! Wheelbarrow Arbitration System
//!
//! producer が request を出し終えた後に実行され、「どの request に
//! 手押し車を割り当てるか」を一括で決定する。
//! スコアベースの Greedy 割り当てにより、全体最適に近い手押し車配分を行う。

mod candidates;
mod collection;
mod diagnostics;
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
    ManualHaulPinnedSource, ManualTransportRequest, ReceiverPolicyTier, TransportDemand,
    TransportRequest, TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::types::{BelongsTo, ResourceItem, Wheelbarrow};
use crate::zone::{Stockpile, StockpilePolicy};

pub use diagnostics::{
    WheelbarrowArbitrationDiagnostics, WheelbarrowArbitrationHeader, WheelbarrowArbitrationOutcome,
};
pub use system::wheelbarrow_arbitration_system;

/// Returns whether this request participates in wheelbarrow arbitration.
///
/// Keep this predicate at the arbitration boundary so producers and diagnostic
/// consumers cannot drift to a `WorkType`-based approximation.
#[must_use]
pub fn is_wheelbarrow_arbitration_applicable(request: &TransportRequest) -> bool {
    match request.kind {
        crate::transport_request::TransportRequestKind::DepositToStockpile => true,
        crate::transport_request::TransportRequestKind::DeliverToBlueprint => {
            request.resource_type.requires_wheelbarrow()
        }
        crate::transport_request::TransportRequestKind::DeliverToFloorConstruction => {
            request.resource_type == crate::types::ResourceType::StasisMud
        }
        crate::transport_request::TransportRequestKind::DeliverToMixerSolid => {
            request.resource_type.requires_wheelbarrow()
        }
        _ => false,
    }
}

#[derive(Resource, Default)]
pub struct WheelbarrowArbitrationRuntime {
    initialized: bool,
    last_full_eval_secs: f64,
}

type RequestDirtyQuery<'w, 's> = Query<
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
            Added<ReceiverPolicyTier>,
            Changed<ReceiverPolicyTier>,
        )>,
    ),
>;

type FreeItemDirtyQuery<'w, 's> = Query<
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
>;

type WheelbarrowDirtyQuery<'w, 's> = Query<
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
>;

type StockpileDirtyQuery<'w, 's> = Query<
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
            Added<StockpilePolicy>,
            Changed<StockpilePolicy>,
        )>,
    ),
>;

#[derive(SystemParam)]
pub struct WheelbarrowArbitrationDirtyParams<'w, 's> {
    q_request_dirty: RequestDirtyQuery<'w, 's>,
    q_free_item_dirty: FreeItemDirtyQuery<'w, 's>,
    q_wheelbarrow_dirty: WheelbarrowDirtyQuery<'w, 's>,
    q_stockpile_dirty: StockpileDirtyQuery<'w, 's>,
    q_resource_entities: Query<'w, 's, (), With<ResourceItem>>,
    q_wheelbarrow_entities: Query<'w, 's, (), With<Wheelbarrow>>,
    removed_requests: RemovedComponents<'w, 's, TransportRequest>,
    removed_resource_items: RemovedComponents<'w, 's, ResourceItem>,
    removed_wheelbarrows: RemovedComponents<'w, 's, Wheelbarrow>,
    removed_leases: RemovedComponents<'w, 's, WheelbarrowLease>,
    removed_pinned_source: RemovedComponents<'w, 's, ManualHaulPinnedSource>,
    removed_belongs: RemovedComponents<'w, 's, BelongsTo>,
    removed_stored_in: RemovedComponents<'w, 's, StoredIn>,
    removed_designations: RemovedComponents<'w, 's, Designation>,
    removed_parked_at: RemovedComponents<'w, 's, ParkedAt>,
    removed_pushed_by: RemovedComponents<'w, 's, PushedBy>,
    removed_stored_items: RemovedComponents<'w, 's, StoredItems>,
    removed_incoming: RemovedComponents<'w, 's, IncomingDeliveries>,
    removed_stockpile_policy: RemovedComponents<'w, 's, StockpilePolicy>,
    removed_receiver_policy_tier: RemovedComponents<'w, 's, ReceiverPolicyTier>,
}
