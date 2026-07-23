use bevy::prelude::*;

use super::super::super::builders::issue_haul_to_stockpile_with_source;
use super::super::super::validator::resolve_consolidation_inputs;
use super::source_selector;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn assign_consolidation_to_stockpile(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some(resolved) =
        resolve_consolidation_inputs(ctx.task_entity, queries, shadow, ctx.incoming_snapshot)
    else {
        return false;
    };

    let Some((source_item, source_pos)) = source_selector::find_consolidation_source_item(
        resolved.resource_type,
        &resolved.donor_cells,
        resolved.receiver_owner,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: Consolidation request {:?} has no available {:?} source in donor cells",
            ctx.task_entity, resolved.resource_type
        );
        return false;
    };

    issue_haul_to_stockpile_with_source(
        source_item,
        resolved.receiver,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
