//! 統合タスクの割り当て

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::super::builders::issue_haul_to_stockpile_with_source;
use super::super::super::validator::resolve_consolidation_inputs;
use super::source_selector;

pub fn assign_consolidation_to_stockpile(
    _task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    let Some((receiver, resource_type, donor_cells)) =
        resolve_consolidation_inputs(ctx.task_entity, queries)
    else {
        return false;
    };

    let Some((source_item, source_pos)) = source_selector::find_consolidation_source_item(
        resource_type,
        &donor_cells,
        queries,
        shadow,
    ) else {
        debug!(
            "ASSIGN: Consolidation request {:?} has no available {:?} source in donor cells",
            ctx.task_entity, resource_type
        );
        return false;
    };

    issue_haul_to_stockpile_with_source(
        source_item,
        receiver,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
