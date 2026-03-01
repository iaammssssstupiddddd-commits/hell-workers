use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::{
    HaulPhase, HaulToBpPhase, HaulWithWheelbarrowPhase,
};
use bevy::prelude::*;

use super::{
    build_mixer_destination_reservation_ops, build_source_reservation_ops, build_wheelbarrow_reservation_ops,
    submit_assignment_with_spec, AssignmentSpec,
};

/// 指定のソースアイテムを使って Blueprint 運搬を割り当てる（request 方式の遅延解決用）
pub fn issue_haul_to_blueprint_with_source(
    source_item: Entity,
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulToBlueprint(
            crate::systems::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                item: source_item,
                blueprint,
                phase: HaulToBpPhase::GoingToItem,
            },
        );
    let reservation_ops = build_source_reservation_ops(&[source_item]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::Haul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

pub fn issue_haul_to_stockpile_with_source(
    source_item: Entity,
    stockpile: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Haul(
        crate::systems::soul_ai::execute::task_execution::types::HaulData {
            item: source_item,
            stockpile,
            phase: HaulPhase::GoingToItem,
        },
    );
    let reservation_ops = build_source_reservation_ops(&[source_item]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::Haul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
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
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulToMixer(
        crate::systems::soul_ai::execute::task_execution::types::HaulToMixerData {
            item: source_item,
            mixer,
            resource_type: item_type,
            phase: crate::systems::soul_ai::execute::task_execution::types::HaulToMixerPhase::GoingToItem,
        },
    );
    let mut reservation_ops = build_source_reservation_ops(&[source_item]);
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        mixer,
        item_type,
        mixer_already_reserved,
    ));
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::HaulToMixer,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

pub fn issue_haul_with_wheelbarrow(
    wheelbarrow: Entity,
    source_pos: Vec2,
    destination: crate::systems::logistics::transport_request::WheelbarrowDestination,
    items: Vec<Entity>,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos,
                destination,
                collect_source: None,
                collect_amount: 0,
                collect_resource_type: None,
                items: items.clone(),
                phase: HaulWithWheelbarrowPhase::GoingToParking,
            },
        );

    let reservation_ops = build_wheelbarrow_reservation_ops(queries, wheelbarrow, &destination, &items, &items);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

pub fn issue_return_wheelbarrow(
    wheelbarrow: Entity,
    parking_anchor: Entity,
    wheelbarrow_pos: Vec2,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos: wheelbarrow_pos,
                destination:
                    crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                        parking_anchor,
                    ),
                collect_source: None,
                collect_amount: 0,
                collect_resource_type: None,
                items: Vec::new(),
                phase: HaulWithWheelbarrowPhase::GoingToParking,
            },
        );

    let reservation_ops = build_source_reservation_ops(&[wheelbarrow]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

/// 砂ソースから直接採取して Blueprint へ猫車搬入する。
pub fn issue_collect_sand_with_wheelbarrow_to_blueprint(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    blueprint: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos,
                destination:
                    crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(
                        blueprint,
                    ),
                collect_source: Some(source_entity),
                collect_amount: haul_amount,
                collect_resource_type: Some(ResourceType::Sand),
                items: Vec::new(),
                phase: HaulWithWheelbarrowPhase::GoingToParking,
            },
        );

    let destination =
        crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(blueprint);
    let reservation_ops = build_wheelbarrow_reservation_ops(queries, wheelbarrow, &destination, &[source_entity], &[]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

/// 骨ソースから直接採取して Blueprint へ猫車搬入する。
pub fn issue_collect_bone_with_wheelbarrow_to_blueprint(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    blueprint: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos,
                destination:
                    crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(
                        blueprint,
                    ),
                collect_source: Some(source_entity),
                collect_amount: haul_amount,
                collect_resource_type: Some(ResourceType::Bone),
                items: Vec::new(),
                phase: HaulWithWheelbarrowPhase::GoingToParking,
            },
        );

    let destination =
        crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(blueprint);
    let reservation_ops = build_wheelbarrow_reservation_ops(queries, wheelbarrow, &destination, &[source_entity], &[]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}

/// 骨ソースから直接採取して FloorConstructionSite へ猫車搬入する。
pub fn issue_collect_bone_with_wheelbarrow_to_floor(
    wheelbarrow: Entity,
    source_entity: Entity,
    source_pos: Vec2,
    site_entity: Entity,
    amount: u32,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = amount.max(1);
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos,
                destination:
                    crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(
                        site_entity,
                    ),
                collect_source: Some(source_entity),
                collect_amount: haul_amount,
                collect_resource_type: Some(ResourceType::Bone),
                items: Vec::new(),
                phase: HaulWithWheelbarrowPhase::GoingToParking,
            },
        );

    let destination =
        crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(site_entity);
    let reservation_ops = build_wheelbarrow_reservation_ops(queries, wheelbarrow, &destination, &[source_entity], &[]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}
