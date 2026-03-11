use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::{
    AssignedTask, HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase,
    HaulWithWheelbarrowData, HaulWithWheelbarrowPhase,
};
use hw_jobs::WorkType;

use crate::familiar_ai::decide::task_management::{AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow};
use super::{
    build_mixer_destination_reservation_ops, build_source_reservation_ops,
    build_wheelbarrow_reservation_ops, submit_assignment_with_reservation_ops,
    submit_assignment_with_source_entities,
};

pub fn issue_haul_to_blueprint_with_source(
    source_item: Entity,
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulToBlueprint(HaulToBlueprintData {
        item: source_item,
        blueprint,
        phase: HaulToBpPhase::GoingToItem,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        WorkType::Haul,
        task_pos,
        assigned_task,
        &[source_item],
        already_commanded,
    );
}

pub fn issue_haul_to_stockpile_with_source(
    source_item: Entity,
    stockpile: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::Haul(HaulData {
        item: source_item,
        stockpile,
        phase: HaulPhase::GoingToItem,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        WorkType::Haul,
        task_pos,
        assigned_task,
        &[source_item],
        already_commanded,
    );
}

pub fn issue_haul_to_mixer(
    source_item: Entity,
    mixer: Entity,
    item_type: ResourceType,
    mixer_already_reserved: bool,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    use hw_jobs::{HaulToMixerData, HaulToMixerPhase};
    let assigned_task = AssignedTask::HaulToMixer(HaulToMixerData {
        item: source_item,
        mixer,
        resource_type: item_type,
        phase: HaulToMixerPhase::GoingToItem,
    });
    let mut reservation_ops = build_source_reservation_ops(&[source_item]);
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        mixer,
        item_type,
        mixer_already_reserved,
    ));
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        WorkType::HaulToMixer,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

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
