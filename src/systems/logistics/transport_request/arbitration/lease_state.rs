use std::collections::HashSet;

use bevy::prelude::*;

use crate::constants::WHEELBARROW_MIN_BATCH_SIZE;
use crate::relationships::{ParkedAt, PushedBy};
use crate::systems::logistics::Wheelbarrow;
use crate::systems::logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportRequest,
    TransportRequestKind, TransportRequestState, WheelbarrowLease, WheelbarrowPendingSince,
};
use crate::systems::logistics::{ReservedForTask, ResourceItem};

pub(super) struct LeaseStateUpdate {
    pub(super) used_wheelbarrows: HashSet<Entity>,
    pub(super) cleared_requests: HashSet<Entity>,
}

pub(super) fn update_lease_state(
    commands: &mut Commands,
    q_requests: &Query<(
        Entity,
        &TransportRequest,
        &TransportRequestState,
        &TransportDemand,
        &Transform,
        Option<&WheelbarrowLease>,
        Option<&WheelbarrowPendingSince>,
        Option<&ManualTransportRequest>,
    )>,
    q_free_items: &Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::systems::jobs::Designation>,
            Without<crate::relationships::TaskWorkers>,
            Without<ReservedForTask>,
            Without<ManualHaulPinnedSource>,
        ),
    >,
    q_wheelbarrows: &Query<
        (Entity, &Transform),
        (With<Wheelbarrow>, With<ParkedAt>, Without<PushedBy>),
    >,
    now: f64,
) -> LeaseStateUpdate {
    let mut used_wheelbarrows = HashSet::new();
    let mut cleared_requests = HashSet::new();

    for (req_entity, req, state, _demand, _transform, lease_opt, pending_since_opt, _) in
        q_requests.iter()
    {
        if let Some(lease) = lease_opt {
            let min_valid_items = if req.resource_type.requires_wheelbarrow()
                && req.kind == TransportRequestKind::DeliverToBlueprint
            {
                1
            } else {
                WHEELBARROW_MIN_BATCH_SIZE
            };
            let valid_item_count = lease
                .items
                .iter()
                .filter(|item| {
                    q_free_items
                        .get(**item)
                        .ok()
                        .is_some_and(|(_, _, vis, _)| *vis != Visibility::Hidden)
                })
                .count();
            let lease_stale = q_wheelbarrows.get(lease.wheelbarrow).is_err()
                || valid_item_count < min_valid_items;

            if lease.lease_until < now || lease_stale {
                commands.entity(req_entity).remove::<WheelbarrowLease>();
                cleared_requests.insert(req_entity);
            } else {
                used_wheelbarrows.insert(lease.wheelbarrow);
            }
        }

        if *state == TransportRequestState::Pending {
            if pending_since_opt.is_none() {
                commands
                    .entity(req_entity)
                    .insert(WheelbarrowPendingSince(now));
            }
        } else if pending_since_opt.is_some() {
            commands.entity(req_entity).remove::<WheelbarrowPendingSince>();
        }
    }

    LeaseStateUpdate {
        used_wheelbarrows,
        cleared_requests,
    }
}
