use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::task_execution::types::GatherWaterPhase;
use bevy::prelude::*;

use super::submit_assignment;

pub fn issue_gather_water(
    tank: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::task_execution::types::AssignedTask::GatherWater(
        crate::systems::soul_ai::task_execution::types::GatherWaterData {
            bucket: ctx.task_entity,
            tank,
            phase: GatherWaterPhase::GoingToBucket,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveDestination { target: tank },
        ResourceReservationOp::ReserveSource {
            source: ctx.task_entity,
            amount: 1,
        },
    ];
    submit_assignment(
        ctx,
        queries,
        shadow,
        WorkType::GatherWater,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_haul_water_to_mixer(
    mixer: Entity,
    tank: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::task_execution::types::AssignedTask::HaulWaterToMixer(
        crate::systems::soul_ai::task_execution::types::HaulWaterToMixerData {
            bucket: ctx.task_entity,
            tank,
            mixer,
            amount: 0,
            phase: crate::systems::soul_ai::task_execution::types::HaulWaterToMixerPhase::GoingToBucket,
        },
    );
    let reservation_ops = vec![
        ResourceReservationOp::ReserveMixerDestination {
            target: mixer,
            resource_type: ResourceType::Water,
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
        WorkType::HaulWaterToMixer,
        task_pos,
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
