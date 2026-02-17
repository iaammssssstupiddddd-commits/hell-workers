//! 返却タスク（バケツ・猫車）

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use bevy::prelude::*;

use super::super::super::builders::{
    issue_haul_to_stockpile_with_source, issue_return_wheelbarrow,
};
use super::super::super::validator::{
    find_bucket_return_assignment, resolve_return_bucket_tank, resolve_return_wheelbarrow,
};

/// ReturnBucket を割り当て。割り当て成功で true、対象でない場合は None、対象だが失敗は false。
pub fn assign_return_bucket(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> Option<bool> {
    let tank = resolve_return_bucket_tank(ctx.task_entity, queries)?;
    let Some((source_item, source_pos, destination_stockpile)) =
        find_bucket_return_assignment(tank, task_pos, queries, shadow)
    else {
        debug!(
            "ASSIGN: ReturnBucket request {:?} has no valid source/destination for tank {:?}",
            ctx.task_entity, tank
        );
        return Some(false);
    };

    issue_haul_to_stockpile_with_source(
        source_item,
        destination_stockpile,
        source_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    Some(true)
}

/// ReturnWheelbarrow を割り当て。割り当て成功で true、対象でない場合は None。
pub fn assign_return_wheelbarrow(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> Option<bool> {
    let (wheelbarrow, parking_anchor, wheelbarrow_pos) =
        resolve_return_wheelbarrow(ctx.task_entity, queries)?;

    issue_return_wheelbarrow(
        wheelbarrow,
        parking_anchor,
        wheelbarrow_pos,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    Some(true)
}
