//! Typed outcome publication for the finalizer protocol.

use bevy::prelude::*;
use hw_jobs::{
    DeconstructionCancelOutcome, DeconstructionCommitOutcome, DeconstructionCommitRequest,
    DeconstructionCommitResult,
};

pub(super) const fn commit_outcome_base(
    request: DeconstructionCommitRequest,
) -> DeconstructionCommitOutcome {
    DeconstructionCommitOutcome {
        worker: request.worker,
        order: request.order,
        target: request.target,
        result: DeconstructionCommitResult::StaleIdentity,
    }
}

pub(super) fn write_commit_outcome(world: &mut World, outcome: DeconstructionCommitOutcome) {
    world
        .resource_mut::<Messages<DeconstructionCommitOutcome>>()
        .write(outcome);
}

pub(super) fn write_cancel_outcome(world: &mut World, outcome: DeconstructionCancelOutcome) {
    world
        .resource_mut::<Messages<DeconstructionCancelOutcome>>()
        .write(outcome);
}
