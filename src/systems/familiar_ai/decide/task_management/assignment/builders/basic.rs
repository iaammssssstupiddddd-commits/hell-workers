use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::{BuildPhase, GatherPhase};
use bevy::prelude::*;

use super::submit_assignment;

pub fn issue_gather(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Gather(
            crate::systems::soul_ai::execute::task_execution::types::GatherData {
                target: ctx.task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        work_type,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_build(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Build(
            crate::systems::soul_ai::execute::task_execution::types::BuildData {
                blueprint: ctx.task_entity,
                phase: BuildPhase::GoingToBlueprint,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Build,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_sand(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::CollectSand(
        crate::systems::soul_ai::execute::task_execution::types::CollectSandData {
            target: ctx.task_entity,
            phase: crate::systems::soul_ai::execute::task_execution::types::CollectSandPhase::GoingToSand,
        },
    );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::CollectSand,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_refine(
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Refine(
        crate::systems::soul_ai::execute::task_execution::types::RefineData {
            mixer: ctx.task_entity,
            phase: crate::systems::soul_ai::execute::task_execution::types::RefinePhase::GoingToMixer,
        },
    );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: ctx.task_entity,
        amount: 1,
    }];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Refine,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
