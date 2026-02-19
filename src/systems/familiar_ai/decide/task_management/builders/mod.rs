mod basic;
mod haul;
mod water;

pub use basic::{
    issue_build, issue_coat_wall, issue_collect_bone, issue_collect_sand, issue_frame_wall,
    issue_gather, issue_pour_floor, issue_refine, issue_reinforce_floor,
};
pub use haul::{
    issue_collect_bone_with_wheelbarrow_to_blueprint, issue_collect_bone_with_wheelbarrow_to_floor,
    issue_collect_sand_with_wheelbarrow_to_blueprint, issue_haul_to_blueprint_with_source,
    issue_haul_to_mixer, issue_haul_to_stockpile_with_source, issue_haul_with_wheelbarrow,
    issue_return_wheelbarrow,
};
pub use water::{issue_gather_water, issue_haul_water_to_mixer};

use crate::events::{ResourceReservationOp, TaskAssignmentRequest};
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use bevy::prelude::*;

pub fn submit_assignment(
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    work_type: WorkType,
    task_pos: Vec2,
    assigned_task: AssignedTask,
    reservation_ops: Vec<ResourceReservationOp>,
    already_commanded: bool,
) {
    shadow.apply_reserve_ops(&reservation_ops);
    queries.assignment_writer.write(TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}
