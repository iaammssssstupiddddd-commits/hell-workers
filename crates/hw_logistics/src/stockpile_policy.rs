use bevy::prelude::Entity;

use crate::types::ResourceType;
use crate::zone::StockpilePolicy;

/// Returns whether an item may enter a non-special stockpile owned by `stockpile_owner`.
///
/// An unowned ground item may be claimed by an owned stockpile. Owned items never cross into a
/// different owner (or an unowned stockpile). Bucket/tank storage has stricter ownership rules and
/// must not use this helper.
#[must_use]
pub fn stockpile_owner_accepts_item(
    item_owner: Option<Entity>,
    stockpile_owner: Option<Entity>,
) -> bool {
    item_owner == stockpile_owner || (item_owner.is_none() && stockpile_owner.is_some())
}

/// Which lifecycle boundary is asking whether a stockpile transfer may proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockpileTransferPhase {
    NewInbound,
    CommittedInbound { owned_reservation: usize },
    NewOutbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockpilePolicyState {
    Accepting,
    TargetReached,
    Draining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockpilePolicyRejection {
    ResourceNotAccepted,
    StoredResourceMismatch,
    PhysicalCapacityReached,
    TargetAmountReached,
    ReservedCapacityReached,
    ReservedResourceMismatch,
    NoStoredResource,
    ExportDisabled,
}

/// Scalar snapshot used by every producer, grant validator, and execution path.
///
/// `incoming_reserved` includes durable/in-flight reservations. `cycle_reserved` is the
/// caller-owned same-cycle shadow. For a committed inbound transfer, `owned_reservation`
/// identifies the part of `incoming_reserved` owned by the transfer being evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockpilePolicyInput {
    pub phase: StockpileTransferPhase,
    pub policy: StockpilePolicy,
    pub capacity: usize,
    pub stored_amount: usize,
    pub stored_resource: Option<ResourceType>,
    pub transfer_resource: ResourceType,
    pub requested_amount: usize,
    pub incoming_reserved: usize,
    /// Portion of `incoming_reserved` whose resource is not `transfer_resource`.
    pub incoming_reserved_other_resource: usize,
    pub cycle_reserved: usize,
    /// Portion of `cycle_reserved` whose resource is not `transfer_resource`.
    pub cycle_reserved_other_resource: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockpilePolicyEvaluation {
    pub state: StockpilePolicyState,
    pub physical_remaining: usize,
    pub target_remaining: usize,
    pub available_amount: usize,
    pub allowed_amount: usize,
    pub rejection: Option<StockpilePolicyRejection>,
}

/// Derives the player-facing runtime state from the durable policy and live counts.
///
/// Callers that do not own a same-cycle reservation shadow (for example the inspection UI)
/// should pass only durable `IncomingDeliveries` as `reserved_amount`. Transfer evaluators pass
/// the sum of durable and same-cycle reservations so both paths use the same state contract.
#[must_use]
pub fn derive_stockpile_policy_state(
    policy: StockpilePolicy,
    capacity: usize,
    stored_amount: usize,
    stored_resource: Option<ResourceType>,
    reserved_amount: usize,
) -> StockpilePolicyState {
    let policy = policy.normalized_for_capacity(capacity);
    let draining = stored_amount > 0
        && stored_resource.is_none_or(|stored| !policy.acceptance.accepts(stored));
    if draining {
        StockpilePolicyState::Draining
    } else if policy
        .target_amount
        .saturating_sub(stored_amount)
        .saturating_sub(reserved_amount)
        == 0
    {
        StockpilePolicyState::TargetReached
    } else {
        StockpilePolicyState::Accepting
    }
}

impl StockpilePolicyEvaluation {
    fn rejected(
        state: StockpilePolicyState,
        physical_remaining: usize,
        target_remaining: usize,
        available_amount: usize,
        rejection: StockpilePolicyRejection,
    ) -> Self {
        Self {
            state,
            physical_remaining,
            target_remaining,
            available_amount,
            allowed_amount: 0,
            rejection: Some(rejection),
        }
    }
}

pub fn evaluate_stockpile_policy(input: StockpilePolicyInput) -> StockpilePolicyEvaluation {
    let policy = input.policy.normalized_for_capacity(input.capacity);
    let physical_remaining = input.capacity.saturating_sub(input.stored_amount);
    let target_remaining = policy.target_amount.saturating_sub(input.stored_amount);
    let reserved = input.incoming_reserved.saturating_add(input.cycle_reserved);
    let state = derive_stockpile_policy_state(
        policy,
        input.capacity,
        input.stored_amount,
        input.stored_resource,
        reserved,
    );
    let draining = state == StockpilePolicyState::Draining;

    match input.phase {
        StockpileTransferPhase::NewInbound => evaluate_new_inbound(
            input,
            policy,
            state,
            physical_remaining,
            target_remaining,
            reserved,
        ),
        StockpileTransferPhase::CommittedInbound { owned_reservation } => {
            evaluate_committed_inbound(
                input,
                state,
                physical_remaining,
                target_remaining,
                owned_reservation,
            )
        }
        StockpileTransferPhase::NewOutbound => evaluate_new_outbound(
            input,
            policy,
            state,
            physical_remaining,
            target_remaining,
            draining,
        ),
    }
}

fn stored_resource_matches(input: StockpilePolicyInput) -> bool {
    input.stored_amount == 0 || input.stored_resource == Some(input.transfer_resource)
}

fn evaluate_new_inbound(
    input: StockpilePolicyInput,
    policy: StockpilePolicy,
    state: StockpilePolicyState,
    physical_remaining: usize,
    target_remaining: usize,
    reserved: usize,
) -> StockpilePolicyEvaluation {
    if !policy.acceptance.accepts(input.transfer_resource) {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::ResourceNotAccepted,
        );
    }
    if !stored_resource_matches(input) {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::StoredResourceMismatch,
        );
    }
    if input.incoming_reserved_other_resource > 0 || input.cycle_reserved_other_resource > 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::ReservedResourceMismatch,
        );
    }
    if physical_remaining == 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::PhysicalCapacityReached,
        );
    }
    if target_remaining == 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::TargetAmountReached,
        );
    }

    let available_amount = physical_remaining
        .saturating_sub(reserved)
        .min(target_remaining.saturating_sub(reserved));
    if available_amount == 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            available_amount,
            StockpilePolicyRejection::ReservedCapacityReached,
        );
    }

    StockpilePolicyEvaluation {
        state,
        physical_remaining,
        target_remaining,
        available_amount,
        allowed_amount: input.requested_amount.min(available_amount),
        rejection: None,
    }
}

fn evaluate_committed_inbound(
    input: StockpilePolicyInput,
    state: StockpilePolicyState,
    physical_remaining: usize,
    target_remaining: usize,
    owned_reservation: usize,
) -> StockpilePolicyEvaluation {
    if !stored_resource_matches(input) {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::StoredResourceMismatch,
        );
    }
    if physical_remaining == 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::PhysicalCapacityReached,
        );
    }

    let other_reserved = input
        .incoming_reserved
        .saturating_sub(owned_reservation)
        .saturating_add(input.cycle_reserved);
    let available_amount = physical_remaining.saturating_sub(other_reserved);
    if available_amount == 0 {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            available_amount,
            StockpilePolicyRejection::ReservedCapacityReached,
        );
    }

    StockpilePolicyEvaluation {
        state,
        physical_remaining,
        target_remaining,
        available_amount,
        allowed_amount: input.requested_amount.min(available_amount),
        rejection: None,
    }
}

fn evaluate_new_outbound(
    input: StockpilePolicyInput,
    policy: StockpilePolicy,
    state: StockpilePolicyState,
    physical_remaining: usize,
    target_remaining: usize,
    draining: bool,
) -> StockpilePolicyEvaluation {
    if input.stored_amount == 0 || input.stored_resource.is_none() {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::NoStoredResource,
        );
    }
    if !stored_resource_matches(input) {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            0,
            StockpilePolicyRejection::StoredResourceMismatch,
        );
    }
    if !policy.allow_export && !draining {
        return StockpilePolicyEvaluation::rejected(
            state,
            physical_remaining,
            target_remaining,
            input.stored_amount,
            StockpilePolicyRejection::ExportDisabled,
        );
    }

    StockpilePolicyEvaluation {
        state,
        physical_remaining,
        target_remaining,
        available_amount: input.stored_amount,
        allowed_amount: input.requested_amount.min(input.stored_amount),
        rejection: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_request::TransportPriority;
    use crate::zone::StockpileAcceptance;

    fn input(phase: StockpileTransferPhase) -> StockpilePolicyInput {
        StockpilePolicyInput {
            phase,
            policy: StockpilePolicy::for_capacity(10),
            capacity: 10,
            stored_amount: 0,
            stored_resource: None,
            transfer_resource: ResourceType::Wood,
            requested_amount: 1,
            incoming_reserved: 0,
            incoming_reserved_other_resource: 0,
            cycle_reserved: 0,
            cycle_reserved_other_resource: 0,
        }
    }

    #[test]
    fn derived_state_counts_incoming_and_ignores_an_empty_cell_resource_marker() {
        let policy = StockpilePolicy {
            target_amount: 4,
            ..StockpilePolicy::for_capacity(10)
        };

        assert_eq!(
            derive_stockpile_policy_state(policy, 10, 2, Some(ResourceType::Wood), 2,),
            StockpilePolicyState::TargetReached
        );
        assert_eq!(
            derive_stockpile_policy_state(
                StockpilePolicy {
                    acceptance: StockpileAcceptance::Only(ResourceType::Rock),
                    ..policy
                },
                10,
                0,
                Some(ResourceType::Wood),
                0,
            ),
            StockpilePolicyState::Accepting
        );
    }

    #[test]
    fn ordinary_stockpile_accepts_unowned_items_but_not_another_owners_items() {
        let owner = Entity::from_raw_u32(1).expect("valid owner");
        let other = Entity::from_raw_u32(2).expect("valid owner");

        assert!(stockpile_owner_accepts_item(None, Some(owner)));
        assert!(stockpile_owner_accepts_item(Some(owner), Some(owner)));
        assert!(stockpile_owner_accepts_item(None, None));
        assert!(!stockpile_owner_accepts_item(Some(other), Some(owner)));
        assert!(!stockpile_owner_accepts_item(Some(owner), None));
    }

    #[test]
    fn new_inbound_uses_physical_target_and_both_reservation_counts() {
        let result = evaluate_stockpile_policy(StockpilePolicyInput {
            policy: StockpilePolicy {
                target_amount: 8,
                ..StockpilePolicy::for_capacity(10)
            },
            stored_amount: 3,
            stored_resource: Some(ResourceType::Wood),
            requested_amount: 5,
            incoming_reserved: 2,
            cycle_reserved: 1,
            ..input(StockpileTransferPhase::NewInbound)
        });

        assert_eq!(result.state, StockpilePolicyState::Accepting);
        assert_eq!(result.physical_remaining, 7);
        assert_eq!(result.target_remaining, 5);
        assert_eq!(result.available_amount, 2);
        assert_eq!(result.allowed_amount, 2);
        assert_eq!(result.rejection, None);

        let capacity_probe = evaluate_stockpile_policy(StockpilePolicyInput {
            requested_amount: 0,
            ..StockpilePolicyInput {
                policy: StockpilePolicy {
                    target_amount: 8,
                    ..StockpilePolicy::for_capacity(10)
                },
                stored_amount: 3,
                stored_resource: Some(ResourceType::Wood),
                incoming_reserved: 2,
                cycle_reserved: 1,
                ..input(StockpileTransferPhase::NewInbound)
            }
        });
        assert_eq!(capacity_probe.available_amount, 2);
        assert_eq!(capacity_probe.allowed_amount, 0);
        assert_eq!(capacity_probe.rejection, None);

        let reservations_fill_target = evaluate_stockpile_policy(StockpilePolicyInput {
            policy: StockpilePolicy {
                target_amount: 5,
                ..StockpilePolicy::for_capacity(10)
            },
            stored_amount: 3,
            stored_resource: Some(ResourceType::Wood),
            incoming_reserved: 1,
            cycle_reserved: 1,
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(
            reservations_fill_target.state,
            StockpilePolicyState::TargetReached
        );
        assert_eq!(
            reservations_fill_target.rejection,
            Some(StockpilePolicyRejection::ReservedCapacityReached)
        );
    }

    #[test]
    fn new_inbound_rejects_policy_and_existing_content_mismatches() {
        let policy_rejection = evaluate_stockpile_policy(StockpilePolicyInput {
            policy: StockpilePolicy {
                acceptance: StockpileAcceptance::Only(ResourceType::Bone),
                ..StockpilePolicy::for_capacity(10)
            },
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(
            policy_rejection.rejection,
            Some(StockpilePolicyRejection::ResourceNotAccepted)
        );

        let content_rejection = evaluate_stockpile_policy(StockpilePolicyInput {
            stored_amount: 1,
            stored_resource: Some(ResourceType::Bone),
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(
            content_rejection.rejection,
            Some(StockpilePolicyRejection::StoredResourceMismatch)
        );

        let reservation_rejection = evaluate_stockpile_policy(StockpilePolicyInput {
            incoming_reserved: 1,
            incoming_reserved_other_resource: 1,
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(
            reservation_rejection.rejection,
            Some(StockpilePolicyRejection::ReservedResourceMismatch)
        );
    }

    #[test]
    fn target_zero_and_capacity_overflow_are_saturating_boundaries() {
        let target_zero = evaluate_stockpile_policy(StockpilePolicyInput {
            policy: StockpilePolicy {
                target_amount: 0,
                ..StockpilePolicy::for_capacity(10)
            },
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(target_zero.state, StockpilePolicyState::TargetReached);
        assert_eq!(
            target_zero.rejection,
            Some(StockpilePolicyRejection::TargetAmountReached)
        );

        let overflow = evaluate_stockpile_policy(StockpilePolicyInput {
            policy: StockpilePolicy {
                target_amount: usize::MAX,
                ..StockpilePolicy::for_capacity(10)
            },
            stored_amount: usize::MAX,
            stored_resource: Some(ResourceType::Wood),
            incoming_reserved: usize::MAX,
            cycle_reserved: usize::MAX,
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(overflow.physical_remaining, 0);
        assert_eq!(overflow.target_remaining, 0);
        assert_eq!(
            overflow.rejection,
            Some(StockpilePolicyRejection::PhysicalCapacityReached)
        );
    }

    #[test]
    fn committed_inbound_reclaims_its_reservation_and_grandfathers_policy() {
        let result = evaluate_stockpile_policy(StockpilePolicyInput {
            phase: StockpileTransferPhase::CommittedInbound {
                owned_reservation: 1,
            },
            policy: StockpilePolicy {
                acceptance: StockpileAcceptance::Only(ResourceType::Bone),
                inbound_priority: TransportPriority::Critical,
                target_amount: 0,
                allow_export: false,
            },
            capacity: 1,
            transfer_resource: ResourceType::Wood,
            incoming_reserved: 1,
            ..input(StockpileTransferPhase::NewInbound)
        });

        assert_eq!(result.state, StockpilePolicyState::TargetReached);
        assert_eq!(result.allowed_amount, 1);
        assert_eq!(result.rejection, None);

        let without_owned_reservation = evaluate_stockpile_policy(StockpilePolicyInput {
            phase: StockpileTransferPhase::CommittedInbound {
                owned_reservation: 0,
            },
            capacity: 1,
            incoming_reserved: 1,
            ..input(StockpileTransferPhase::NewInbound)
        });
        assert_eq!(without_owned_reservation.allowed_amount, 0);
        assert_eq!(
            without_owned_reservation.rejection,
            Some(StockpilePolicyRejection::ReservedCapacityReached)
        );
    }

    #[test]
    fn outbound_honors_export_policy_but_draining_overrides_it() {
        let blocked = evaluate_stockpile_policy(StockpilePolicyInput {
            phase: StockpileTransferPhase::NewOutbound,
            policy: StockpilePolicy {
                allow_export: false,
                ..StockpilePolicy::for_capacity(10)
            },
            stored_amount: 4,
            stored_resource: Some(ResourceType::Wood),
            requested_amount: 2,
            ..input(StockpileTransferPhase::NewOutbound)
        });
        assert_eq!(
            blocked.rejection,
            Some(StockpilePolicyRejection::ExportDisabled)
        );

        let draining = evaluate_stockpile_policy(StockpilePolicyInput {
            phase: StockpileTransferPhase::NewOutbound,
            policy: StockpilePolicy {
                acceptance: StockpileAcceptance::Only(ResourceType::Bone),
                allow_export: false,
                ..StockpilePolicy::for_capacity(10)
            },
            stored_amount: 4,
            stored_resource: Some(ResourceType::Wood),
            requested_amount: 9,
            ..input(StockpileTransferPhase::NewOutbound)
        });
        assert_eq!(draining.state, StockpilePolicyState::Draining);
        assert_eq!(draining.allowed_amount, 4);
        assert_eq!(draining.rejection, None);
    }
}
