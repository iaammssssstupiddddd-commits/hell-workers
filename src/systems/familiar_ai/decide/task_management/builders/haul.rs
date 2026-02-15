use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::{
    HaulPhase, HaulToBpPhase, HaulWithWheelbarrowPhase,
};
use bevy::prelude::*;


/// 指定のソースアイテムを使って Blueprint 運搬を割り当てる（request 方式の遅延解決用）
pub fn issue_haul_to_blueprint_with_source(
    source_item: Entity,
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulToBlueprint(
            crate::systems::soul_ai::execute::task_execution::types::HaulToBlueprintData {
                item: source_item,
                blueprint,
                phase: HaulToBpPhase::GoingToItem,
            },
        );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveSource {
            source: source_item,
            amount: 1,
        },
    ];
    
    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::Haul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}

pub fn issue_haul_to_stockpile_with_source(
    source_item: Entity,
    stockpile: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Haul(
        crate::systems::soul_ai::execute::task_execution::types::HaulData {
            item: source_item,
            stockpile,
            phase: HaulPhase::GoingToItem,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveSource {
            source: source_item,
            amount: 1,
        },
    ];

    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::Haul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}

pub fn issue_haul_to_mixer(
    source_item: Entity,
    mixer: Entity,
    item_type: ResourceType,
    mixer_already_reserved: bool,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulToMixer(
        crate::systems::soul_ai::execute::task_execution::types::HaulToMixerData {
            item: source_item,
            mixer,
            resource_type: item_type,
            phase: crate::systems::soul_ai::execute::task_execution::types::HaulToMixerPhase::GoingToItem,
        },
    );
    let mut reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: source_item,
        amount: 1,
    }];
    if !mixer_already_reserved {
        reservation_ops.push(ResourceReservationOp::ReserveMixerDestination {
            target: mixer,
            resource_type: item_type,
        });
    }
    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::HaulToMixer,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}

pub fn issue_haul_with_wheelbarrow(
    wheelbarrow: Entity,
    source_pos: Vec2,
    destination: crate::systems::logistics::transport_request::WheelbarrowDestination,
    items: Vec<Entity>,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    __shadow: &mut ReservationShadow,
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

    let mut reservation_ops = vec![
        // 手押し車自体をソース予約して二重使用を防止
        ResourceReservationOp::ReserveSource {
            source: wheelbarrow,
            amount: 1,
        },
    ];
    // 目的地をアイテム数分予約
    for &item in &items {
        match destination {
            crate::systems::logistics::transport_request::WheelbarrowDestination::Stockpile(_)
            | crate::systems::logistics::transport_request::WheelbarrowDestination::Blueprint(_) => {
                // Relationship で管理するため ReserveDestination は不要
            }
            crate::systems::logistics::transport_request::WheelbarrowDestination::Mixer {
                entity: target,
                resource_type,
            } => {
                let item_type = queries
                    .items
                    .get(item)
                    .ok()
                    .map(|(it, _)| it.0)
                    .unwrap_or(resource_type);
                reservation_ops.push(ResourceReservationOp::ReserveMixerDestination {
                    target,
                    resource_type: item_type,
                });
            }
        }
    }
    // 全アイテムをソース予約
    for &item in &items {
        reservation_ops.push(ResourceReservationOp::ReserveSource {
            source: item,
            amount: 1,
        });
    }

    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
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
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &mut ReservationShadow,
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

    let reservation_ops = vec![
        ResourceReservationOp::ReserveSource {
            source: wheelbarrow,
            amount: 1,
        },
        ResourceReservationOp::ReserveSource {
            source: source_entity,
            amount: 1,
        },
    ];
    // Relationship で管理するため ReserveDestination は不要

    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
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
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &mut ReservationShadow,
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

    let reservation_ops = vec![
        ResourceReservationOp::ReserveSource {
            source: wheelbarrow,
            amount: 1,
        },
        ResourceReservationOp::ReserveSource {
            source: source_entity,
            amount: 1,
        },
    ];
    // Relationship で管理するため ReserveDestination は不要

    queries.assignment_writer.write(crate::events::TaskAssignmentRequest {
        familiar_entity: ctx.fam_entity,
        worker_entity: ctx.worker_entity,
        task_entity: ctx.task_entity,
        work_type: WorkType::WheelbarrowHaul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    });
}
