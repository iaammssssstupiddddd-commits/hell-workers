use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::task_execution::types::{HaulPhase, HaulToBpPhase};
use bevy::prelude::*;

use super::submit_assignment;

pub fn issue_haul_to_blueprint(
    blueprint: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::task_execution::types::AssignedTask::HaulToBlueprint(
        crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
            item: ctx.task_entity,
            blueprint,
            phase: HaulToBpPhase::GoingToItem,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveDestination { target: blueprint },
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

pub fn issue_haul_to_stockpile(
    stockpile: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::task_execution::types::AssignedTask::Haul(
        crate::systems::soul_ai::task_execution::types::HaulData {
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
    mixer: Entity,
    item_type: ResourceType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::task_execution::types::AssignedTask::HaulToMixer(
        crate::systems::soul_ai::task_execution::types::HaulToMixerData {
            item: ctx.task_entity,
            mixer,
            resource_type: item_type,
            phase: crate::systems::soul_ai::task_execution::types::HaulToMixerPhase::GoingToItem,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveMixerDestination {
            target: mixer,
            resource_type: item_type,
        },
        ResourceReservationOp::ReserveSource {
            source: ctx.task_entity,
            amount: 1,
        },
    ];
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
