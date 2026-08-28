//! Deterministic request intake for the exclusive finalizer transaction.

use bevy::prelude::*;
use hw_jobs::{DeconstructionCancelRequest, DeconstructionCommitRequest};

pub(super) fn drain_sorted_requests(
    world: &mut World,
) -> (
    Vec<DeconstructionCancelRequest>,
    Vec<DeconstructionCommitRequest>,
) {
    let mut cancels: Vec<_> = world
        .resource_mut::<Messages<DeconstructionCancelRequest>>()
        .drain()
        .collect();
    let mut commits: Vec<_> = world
        .resource_mut::<Messages<DeconstructionCommitRequest>>()
        .drain()
        .collect();
    cancels.sort_unstable_by_key(|request| (request.world_epoch, request.order.to_bits()));
    commits.sort_unstable_by_key(|request| {
        (
            request.world_epoch,
            request.target.to_bits(),
            request.order.to_bits(),
            request.worker.to_bits(),
            request.identity.assignment_entity.to_bits(),
            request.identity.current_target_entity.to_bits(),
            request.identity.current_work_type.stable_index(),
            request.identity.binding_stable_index(),
        )
    });
    (cancels, commits)
}
