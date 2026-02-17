//! 運搬タスクの割り当てポリシー

mod blueprint;
mod consolidation;
mod demand;
mod direct_collect;
mod floor;
mod lease_validation;
mod mixer;
mod provisional_wall;
mod returns;
mod source_selector;
mod stockpile;
mod wheelbarrow;

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

pub use mixer::assign_haul_to_mixer;
pub(crate) use source_selector::take_source_selector_scan_snapshot;

pub fn assign_haul(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if blueprint::assign_haul_to_blueprint(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if let Some(ok) =
        returns::assign_return_bucket(task_pos, already_commanded, ctx, queries, shadow)
    {
        return ok;
    }

    if let Some(ok) =
        returns::assign_return_wheelbarrow(task_pos, already_commanded, ctx, queries, shadow)
    {
        return ok;
    }

    if provisional_wall::assign_haul_to_provisional_wall(
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    if floor::assign_haul_to_floor_construction(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if stockpile::assign_haul_to_stockpile(task_pos, already_commanded, ctx, queries, shadow) {
        return true;
    }

    if consolidation::assign_consolidation_to_stockpile(
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    ) {
        return true;
    }

    debug!(
        "ASSIGN: Haul task {:?} is not a valid transport request candidate",
        ctx.task_entity
    );
    false
}
