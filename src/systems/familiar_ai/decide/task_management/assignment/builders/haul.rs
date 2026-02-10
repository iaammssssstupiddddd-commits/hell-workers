use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::{
    HaulPhase, HaulToBpPhase, HaulWithWheelbarrowPhase,
};
use bevy::prelude::*;

use super::submit_assignment;

/// アイテムエンティティに対して Blueprint 運搬を割り当てる（従来方式）
pub fn issue_haul_to_blueprint(
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    issue_haul_to_blueprint_with_source(ctx.task_entity, blueprint, task_pos, already_commanded, ctx, queries, shadow);
}

/// 指定のソースアイテムを使って Blueprint 運搬を割り当てる（request 方式の遅延解決用）
pub fn issue_haul_to_blueprint_with_source(
    source_item: Entity,
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
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
    let reservation_ops = vec![
        ResourceReservationOp::ReserveDestination { target: blueprint },
        ResourceReservationOp::ReserveSource {
            source: source_item,
            amount: 1,
        },
    ];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Haul,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_haul_to_stockpile(
    stockpile: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::Haul(
        crate::systems::soul_ai::execute::task_execution::types::HaulData {
            item: ctx.task_entity,
            stockpile,
            phase: HaulPhase::GoingToItem,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveDestination { target: stockpile },
        ResourceReservationOp::ReserveSource {
            source: ctx.task_entity,
            amount: 1,
        },
    ];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::Haul,
        task_pos,
        assigned_task,
        reservation_ops,
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
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
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
    submit_assignment(
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
    dest_stockpile: Entity,
    items: Vec<Entity>,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWithWheelbarrow(
            crate::systems::soul_ai::execute::task_execution::types::HaulWithWheelbarrowData {
                wheelbarrow,
                source_pos,
                dest_stockpile,
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
    for _ in &items {
        reservation_ops.push(ResourceReservationOp::ReserveDestination {
            target: dest_stockpile,
        });
    }
    // 全アイテムをソース予約
    for &item in &items {
        reservation_ops.push(ResourceReservationOp::ReserveSource {
            source: item,
            amount: 1,
        });
    }

    submit_assignment(
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
