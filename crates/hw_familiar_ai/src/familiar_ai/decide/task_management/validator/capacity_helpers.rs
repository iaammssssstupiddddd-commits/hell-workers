use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_logistics::transport_request::TransportPriority;
use hw_logistics::{StockpilePolicyInput, StockpileTransferPhase, evaluate_stockpile_policy};

use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow,
};

pub(super) fn stored_items_opt_to_count(
    opt: Option<&hw_core::relationships::StoredItems>,
) -> usize {
    opt.map(|s| s.len()).unwrap_or(0)
}

/// ストックパイルのキャパシティ・タイプ検証。空きがあればその数を返す。
pub(super) fn check_stockpile_capacity(
    cell: Entity,
    resource_type: ResourceType,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
    incoming_snapshot: &IncomingDeliverySnapshot,
    expected_priority: Option<TransportPriority>,
) -> Option<usize> {
    let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(cell).ok()?;
    let policy = *queries.storage.stockpile_policies.get(cell).ok()?;
    if expected_priority.is_some_and(|expected| expected != policy.inbound_priority) {
        return None;
    }
    let stored = stored_items_opt_to_count(stored_opt);
    let incoming = incoming_snapshot.count_total(cell) as usize;
    let incoming_matching = incoming_snapshot.count_exact(cell, resource_type) as usize;
    let shadow_incoming = shadow.destination_reserved_total(cell);
    let shadow_matching = shadow.destination_reserved_resource(cell, resource_type);
    let evaluation = evaluate_stockpile_policy(StockpilePolicyInput {
        phase: StockpileTransferPhase::NewInbound,
        policy,
        capacity: stock.capacity,
        stored_amount: stored,
        stored_resource: stock.resource_type,
        transfer_resource: resource_type,
        requested_amount: 0,
        incoming_reserved: incoming,
        incoming_reserved_other_resource: incoming.saturating_sub(incoming_matching),
        cycle_reserved: shadow_incoming,
        cycle_reserved_other_resource: shadow_incoming.saturating_sub(shadow_matching),
    });

    (evaluation.available_amount > 0).then_some(evaluation.available_amount)
}
