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
use crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries;
use crate::systems::jobs::WorkType;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::WheelbarrowDestination;
use bevy::prelude::*;

pub struct AssignmentSpec {
    pub work_type: WorkType,
    pub task_pos: Vec2,
    pub assigned_task: AssignedTask,
    pub reservation_ops: Vec<ResourceReservationOp>,
    pub already_commanded: bool,
}

pub fn submit_assignment(
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
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

pub fn submit_assignment_with_spec(
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    spec: AssignmentSpec,
) {
    submit_assignment(
        ctx,
        queries,
        shadow,
        spec.work_type,
        spec.task_pos,
        spec.assigned_task,
        spec.reservation_ops,
        spec.already_commanded,
    );
}

pub fn build_source_reservation_ops(sources: &[Entity]) -> Vec<ResourceReservationOp> {
    sources
        .iter()
        .copied()
        .map(|source| ResourceReservationOp::ReserveSource {
            source,
            amount: 1,
        })
        .collect()
}

pub fn build_mixer_destination_reservation_ops(
    mixer: Entity,
    resource_type: ResourceType,
    already_reserved: bool,
) -> Vec<ResourceReservationOp> {
    if already_reserved {
        Vec::new()
    } else {
        vec![ResourceReservationOp::ReserveMixerDestination {
            target: mixer,
            resource_type,
        }]
    }
}

pub fn build_wheelbarrow_reservation_ops(
    queries: &FamiliarTaskAssignmentQueries,
    wheelbarrow: Entity,
    destination: &WheelbarrowDestination,
    reserved_sources: &[Entity],
    destination_items: &[Entity],
) -> Vec<ResourceReservationOp> {
    let mut reservation_ops = Vec::with_capacity(1 + reserved_sources.len() + destination_items.len());
    reservation_ops.push(ResourceReservationOp::ReserveSource {
        source: wheelbarrow,
        amount: 1,
    });

    for &source in reserved_sources {
        reservation_ops.push(ResourceReservationOp::ReserveSource {
            source,
            amount: 1,
        });
    }

    if let WheelbarrowDestination::Mixer { entity, resource_type } = destination {
        for &item in destination_items {
            let item_resource_type = queries
                .items
                .get(item)
                .ok()
                .map(|(item, _)| item.0)
                .unwrap_or(*resource_type);
            reservation_ops.push(ResourceReservationOp::ReserveMixerDestination {
                target: *entity,
                resource_type: item_resource_type,
            });
        }
    }

    reservation_ops
}
