mod blueprint;
mod consolidation;
mod demand;
mod direct_collect;
mod floor;
mod lease_validation;
mod mixer;
mod provisional_wall;
mod returns;
mod selector_metrics;
mod soul_spa;
mod source_selector;
mod stockpile;
mod wall;
mod wheelbarrow;

use bevy::prelude::*;

use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, CandidateRejectReason, FamiliarTaskAssignmentQueries, ReservationShadow,
    TaskAssignmentAttempt,
};

pub use mixer::assign_haul_to_mixer;
pub use source_selector::take_source_selector_scan_snapshot;

pub fn assign_haul(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    shadow: &mut ReservationShadow,
) -> TaskAssignmentAttempt {
    if blueprint::assign_haul_to_blueprint(task_pos, already_commanded, ctx, queries, shadow) {
        return TaskAssignmentAttempt::Submitted;
    }

    if let Some(ok) =
        returns::assign_return_bucket(task_pos, already_commanded, ctx, queries, shadow)
    {
        return assignment_from_haul_result(ok);
    }

    if let Some(ok) =
        returns::assign_return_wheelbarrow(task_pos, already_commanded, ctx, queries, shadow)
    {
        return assignment_from_haul_result(ok);
    }

    if provisional_wall::assign_haul_to_provisional_wall(
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    ) {
        return TaskAssignmentAttempt::Submitted;
    }

    if floor::assign_haul_to_floor_construction(
        task_pos,
        already_commanded,
        ctx,
        queries,
        construction_sites,
        shadow,
    ) {
        return TaskAssignmentAttempt::Submitted;
    }

    if wall::assign_haul_to_wall_construction(
        task_pos,
        already_commanded,
        ctx,
        queries,
        construction_sites,
        shadow,
    ) {
        return TaskAssignmentAttempt::Submitted;
    }

    if soul_spa::assign_haul_to_soul_spa(task_pos, already_commanded, ctx, queries, shadow) {
        return TaskAssignmentAttempt::Submitted;
    }

    if stockpile::assign_haul_to_stockpile(task_pos, already_commanded, ctx, queries, shadow) {
        return TaskAssignmentAttempt::Submitted;
    }

    if consolidation::assign_consolidation_to_stockpile(
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    ) {
        return TaskAssignmentAttempt::Submitted;
    }

    debug!(
        "ASSIGN: Haul task {:?} is not a valid transport request candidate",
        ctx.task_entity
    );
    TaskAssignmentAttempt::Rejected(CandidateRejectReason::MissingResourceOrSource)
}

fn assignment_from_haul_result(submitted: bool) -> TaskAssignmentAttempt {
    if submitted {
        TaskAssignmentAttempt::Submitted
    } else {
        TaskAssignmentAttempt::Rejected(CandidateRejectReason::MissingResourceOrSource)
    }
}
