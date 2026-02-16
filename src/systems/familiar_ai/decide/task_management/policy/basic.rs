use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

use super::super::builders::{
    issue_build, issue_collect_bone, issue_collect_sand, issue_gather, issue_refine,
};
use super::super::validator::can_reserve_source;

pub(super) fn assign_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_gather(work_type, task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if let Ok((_, bp, _)) = queries.storage.blueprints.get(ctx.task_entity) {
        if !bp.materials_complete() {
            debug!(
                "ASSIGN: Build target {:?} materials not complete",
                ctx.task_entity
            );
            return false;
        }
    }

    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_build(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_collect_sand(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_refine(task_pos, already_commanded, ctx, queries, shadow);
    true
}

pub(super) fn assign_collect_bone(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    if !can_reserve_source(ctx.task_entity, queries, shadow) {
        return false;
    }
    issue_collect_bone(task_pos, already_commanded, ctx, queries, shadow);
    true
}
