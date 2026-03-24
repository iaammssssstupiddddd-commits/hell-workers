//! 一輪車運搬系 builder — `HaulWithWheelbarrow` タスクを生成するすべての関数。

use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::WorkType;
use hw_jobs::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use super::{
    build_mixer_destination_reservation_ops, build_wheelbarrow_reservation_ops,
    submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

#[allow(clippy::too_many_arguments)]
pub fn issue_haul_with_wheelbarrow(
    wheelbarrow: Entity,
    source_pos: Vec2,
    destination: WheelbarrowDestination,
    items: Vec<Entity>,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos,
        destination,
        collect_source: None,
        collect_amount: 0,
        collect_resource_type: None,
        items: items.clone(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops =
        build_wheelbarrow_reservation_ops(queries, wheelbarrow, &destination, &items, &items);
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn issue_return_wheelbarrow(
    wheelbarrow: Entity,
    parking_anchor: Entity,
    wheelbarrow_pos: Vec2,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos: wheelbarrow_pos,
        destination: WheelbarrowDestination::Stockpile(parking_anchor),
        collect_source: None,
        collect_amount: 0,
        collect_resource_type: None,
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        &[wheelbarrow],
        already_commanded,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn issue_collect_sand_with_wheelbarrow_to_blueprint(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    blueprint: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let destination = WheelbarrowDestination::Blueprint(blueprint);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos,
        destination,
        collect_source: Some(source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Sand),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        wheelbarrow,
        &destination,
        &[source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn issue_collect_sand_with_wheelbarrow_to_mixer(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    mixer_entity: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let destination = WheelbarrowDestination::Mixer {
        entity: mixer_entity,
        resource_type: ResourceType::Sand,
    };
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos,
        destination,
        collect_source: Some(source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Sand),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    // Reserve wheelbarrow + sand source, then mixer destination slots for the items we'll generate
    let mut reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        wheelbarrow,
        &destination,
        &[source_entity],
        &[],
    );
    for _ in 0..haul_amount {
        reservation_ops.extend(build_mixer_destination_reservation_ops(
            mixer_entity,
            ResourceType::Sand,
            false,
        ));
    }
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn issue_collect_bone_with_wheelbarrow_to_blueprint(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    blueprint: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let destination = WheelbarrowDestination::Blueprint(blueprint);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos,
        destination,
        collect_source: Some(source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Bone),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        wheelbarrow,
        &destination,
        &[source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn issue_collect_bone_with_wheelbarrow_to_floor(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    site_entity: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let destination = WheelbarrowDestination::Stockpile(site_entity);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow,
        source_pos,
        destination,
        collect_source: Some(source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Bone),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        wheelbarrow,
        &destination,
        &[source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
