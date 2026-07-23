//! Runtime adapter for phase-aware ordinary stockpile inbound evaluation.

use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::relationships::IncomingDeliveries;
use hw_logistics::{
    ResourceItem, ResourceType, StockpilePolicy, StockpilePolicyEvaluation, StockpilePolicyInput,
    StockpileTransferPhase, evaluate_stockpile_policy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InboundReservationSnapshot {
    pub incoming_reserved: usize,
    pub incoming_reserved_other_resource: usize,
    pub owned_reservation: usize,
}

/// Builds a resource-aware reservation snapshot from the live relationship set.
///
/// An entity without a readable `ResourceItem` still occupies a physical reservation and is
/// therefore counted as another resource. `owned_items` must contain only the items represented
/// by the transfer currently being evaluated.
pub fn inbound_reservation_snapshot(
    destination: Entity,
    transfer_resource: ResourceType,
    owned_items: &HashSet<Entity>,
    incoming_deliveries: &Query<(Entity, &IncomingDeliveries)>,
    resources: &Query<&ResourceItem>,
) -> InboundReservationSnapshot {
    let Ok((_, incoming)) = incoming_deliveries.get(destination) else {
        return InboundReservationSnapshot::default();
    };

    let incoming_reserved = incoming.len();
    let incoming_matching = incoming
        .iter()
        .filter(|item| {
            resources
                .get(**item)
                .is_ok_and(|resource| resource.0 == transfer_resource)
        })
        .count();
    let owned_reservation = incoming
        .iter()
        .filter(|item| owned_items.contains(item))
        .count();

    InboundReservationSnapshot {
        incoming_reserved,
        incoming_reserved_other_resource: incoming_reserved.saturating_sub(incoming_matching),
        owned_reservation,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeStockpileInboundInput {
    pub policy: StockpilePolicy,
    pub capacity: usize,
    pub stored_amount: usize,
    pub stored_resource: Option<ResourceType>,
    pub transfer_resource: ResourceType,
    pub requested_amount: usize,
    pub reservations: InboundReservationSnapshot,
    pub cycle_reserved: usize,
    pub cycle_reserved_other_resource: usize,
}

/// Evaluates a live inbound transfer. The acceptance/target policy is grandfathered only when
/// every requested item still owns a durable reservation at this destination.
pub fn evaluate_runtime_stockpile_inbound(
    input: RuntimeStockpileInboundInput,
) -> StockpilePolicyEvaluation {
    let phase = if input.requested_amount > 0
        && input.reservations.owned_reservation == input.requested_amount
    {
        StockpileTransferPhase::CommittedInbound {
            owned_reservation: input.reservations.owned_reservation,
        }
    } else {
        StockpileTransferPhase::NewInbound
    };

    evaluate_stockpile_policy(StockpilePolicyInput {
        phase,
        policy: input.policy,
        capacity: input.capacity,
        stored_amount: input.stored_amount,
        stored_resource: input.stored_resource,
        transfer_resource: input.transfer_resource,
        requested_amount: input.requested_amount,
        incoming_reserved: input.reservations.incoming_reserved,
        incoming_reserved_other_resource: input.reservations.incoming_reserved_other_resource,
        cycle_reserved: input.cycle_reserved,
        cycle_reserved_other_resource: input.cycle_reserved_other_resource,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeStockpileBatchAllowance {
    pub committed_allowed: usize,
    pub new_allowed: usize,
}

impl RuntimeStockpileBatchAllowance {
    pub fn total(self) -> usize {
        self.committed_allowed.saturating_add(self.new_allowed)
    }
}

/// Evaluates a mixed batch without demoting items that still own durable reservations.
/// Committed items consume physical space first; only the unreserved remainder is checked against
/// the current acceptance and target policy.
pub fn evaluate_runtime_stockpile_inbound_batch(
    input: RuntimeStockpileInboundInput,
) -> RuntimeStockpileBatchAllowance {
    let committed_requested = input
        .reservations
        .owned_reservation
        .min(input.requested_amount);
    let new_requested = input.requested_amount.saturating_sub(committed_requested);
    let committed_allowed = if committed_requested == 0 {
        0
    } else {
        evaluate_runtime_stockpile_inbound(RuntimeStockpileInboundInput {
            requested_amount: committed_requested,
            reservations: InboundReservationSnapshot {
                owned_reservation: committed_requested,
                ..input.reservations
            },
            ..input
        })
        .allowed_amount
    };

    let new_allowed = if new_requested == 0 {
        0
    } else {
        let stored_amount = input.stored_amount.saturating_add(committed_allowed);
        let stored_resource = if input.stored_amount == 0 && committed_allowed > 0 {
            Some(input.transfer_resource)
        } else {
            input.stored_resource
        };
        evaluate_runtime_stockpile_inbound(RuntimeStockpileInboundInput {
            stored_amount,
            stored_resource,
            requested_amount: new_requested,
            reservations: InboundReservationSnapshot {
                incoming_reserved: input
                    .reservations
                    .incoming_reserved
                    .saturating_sub(committed_requested),
                incoming_reserved_other_resource: input
                    .reservations
                    .incoming_reserved_other_resource,
                owned_reservation: 0,
            },
            ..input
        })
        .allowed_amount
    };

    RuntimeStockpileBatchAllowance {
        committed_allowed,
        new_allowed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_logistics::transport_request::TransportPriority;
    use hw_logistics::zone::StockpileAcceptance;

    fn restricted_policy() -> StockpilePolicy {
        StockpilePolicy {
            acceptance: StockpileAcceptance::Only(ResourceType::Rock),
            inbound_priority: TransportPriority::Critical,
            target_amount: 0,
            allow_export: false,
        }
    }

    fn input(owned_reservation: usize) -> RuntimeStockpileInboundInput {
        RuntimeStockpileInboundInput {
            policy: restricted_policy(),
            capacity: 10,
            stored_amount: 2,
            stored_resource: Some(ResourceType::Wood),
            transfer_resource: ResourceType::Wood,
            requested_amount: 1,
            reservations: InboundReservationSnapshot {
                incoming_reserved: owned_reservation,
                incoming_reserved_other_resource: 0,
                owned_reservation,
            },
            cycle_reserved: 0,
            cycle_reserved_other_resource: 0,
        }
    }

    #[test]
    fn committed_item_finishes_after_acceptance_and_target_change() {
        let evaluation = evaluate_runtime_stockpile_inbound(input(1));

        assert_eq!(evaluation.allowed_amount, 1);
        assert!(evaluation.rejection.is_none());
    }

    #[test]
    fn unreserved_item_obeys_current_acceptance_and_target() {
        let evaluation = evaluate_runtime_stockpile_inbound(input(0));

        assert_eq!(evaluation.allowed_amount, 0);
        assert!(evaluation.rejection.is_some());
    }

    #[test]
    fn committed_item_still_stops_at_physical_capacity() {
        let mut full = input(1);
        full.stored_amount = full.capacity;

        let evaluation = evaluate_runtime_stockpile_inbound(full);

        assert_eq!(evaluation.allowed_amount, 0);
        assert!(evaluation.rejection.is_some());
    }

    #[test]
    fn mixed_batch_preserves_committed_items_and_rechecks_only_the_unreserved_remainder() {
        let mut mixed = input(2);
        mixed.requested_amount = 3;

        let allowance = evaluate_runtime_stockpile_inbound_batch(mixed);

        assert_eq!(allowance.committed_allowed, 2);
        assert_eq!(allowance.new_allowed, 0);
        assert_eq!(allowance.total(), 2);
    }
}
