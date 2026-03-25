use bevy::prelude::*;
use hw_core::events::ResourceReservationOp;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::events::TaskAssignmentRequest;
use hw_jobs::{AssignedTask, WorkType};

use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

#[derive(Clone, Copy)]
pub struct TaskTarget {
    pub work_type: WorkType,
    pub task_pos: Vec2,
}

pub fn submit_assignment(
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    target: TaskTarget,
    assigned_task: AssignedTask,
    reservation_ops: Vec<ResourceReservationOp>,
    already_commanded: bool,
) {
    shadow.apply_reserve_ops(&reservation_ops);
    apply_destination_shadow(queries, shadow, &assigned_task);
    queries.assignment_writer.write(TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: target.work_type,
        task_pos: target.task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}

fn apply_destination_shadow(
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    assigned_task: &AssignedTask,
) {
    match assigned_task {
        AssignedTask::Haul(data) => {
            reserve_item_destination(queries, shadow, data.stockpile, data.item);
        }
        AssignedTask::HaulToBlueprint(data) => {
            reserve_item_destination(queries, shadow, data.blueprint, data.item);
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            let Some(target) = data.destination.stockpile_or_blueprint() else {
                return;
            };

            for &item in &data.items {
                reserve_item_destination(queries, shadow, target, item);
            }

            if data.items.is_empty()
                && let Some(resource_type) = data.collect_resource_type
            {
                shadow.reserve_destination(
                    target,
                    Some(resource_type),
                    data.collect_amount.max(1) as usize,
                );
            }
        }
        _ => {}
    }
}

fn reserve_item_destination(
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    target: Entity,
    item: Entity,
) {
    let resource_type = queries
        .items
        .get(item)
        .ok()
        .map(|(item, _)| item.0)
        .or_else(|| {
            queries.designation.targets.get(item).ok().and_then(
                |(_, _, _, _, resource_item_opt, _, _)| {
                    resource_item_opt.map(|resource_item| resource_item.0)
                },
            )
        });
    shadow.reserve_destination(target, resource_type, 1);
}

pub(crate) fn submit_assignment_with_reservation_ops(
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    target: TaskTarget,
    assigned_task: AssignedTask,
    reservation_ops: Vec<ResourceReservationOp>,
    already_commanded: bool,
) {
    submit_assignment(ctx, queries, shadow, target, assigned_task, reservation_ops, already_commanded);
}

pub(crate) fn submit_assignment_with_source_entities(
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
    target: TaskTarget,
    assigned_task: AssignedTask,
    source_entities: &[Entity],
    already_commanded: bool,
) {
    let reservation_ops = build_source_reservation_ops(source_entities);
    submit_assignment_with_reservation_ops(ctx, queries, shadow, target, assigned_task, reservation_ops, already_commanded);
}

pub fn build_source_reservation_ops(sources: &[Entity]) -> Vec<ResourceReservationOp> {
    sources
        .iter()
        .copied()
        .map(|source| ResourceReservationOp::ReserveSource { source, amount: 1 })
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
    let mut reservation_ops =
        Vec::with_capacity(1 + reserved_sources.len() + destination_items.len());
    reservation_ops.push(ResourceReservationOp::ReserveSource {
        source: wheelbarrow,
        amount: 1,
    });

    for &source in reserved_sources {
        reservation_ops.push(ResourceReservationOp::ReserveSource { source, amount: 1 });
    }

    if let WheelbarrowDestination::Mixer {
        entity,
        resource_type,
    } = destination
    {
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
