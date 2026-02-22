use crate::events::ResourceReservationOp;
use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use bevy::prelude::*;

use super::submit_assignment;

pub fn issue_gather_water(
    bucket: Entity,
    tank: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task =
        crate::systems::soul_ai::execute::task_execution::types::AssignedTask::GatherWater(
            crate::systems::soul_ai::execute::task_execution::types::GatherWaterData {
                bucket,
                tank,
                phase: GatherWaterPhase::GoingToBucket,
            },
        );
    let reservation_ops = vec![ResourceReservationOp::ReserveSource {
        source: bucket,
        amount: 1,
    }];

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
    bucket: Entity,
    mixer: Entity,
    tank: Entity,
    mixer_already_reserved: bool,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = crate::systems::soul_ai::execute::task_execution::types::AssignedTask::HaulWaterToMixer(
        crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerData {
            bucket,
            tank,
            mixer,
            amount: 0,
            phase: crate::systems::soul_ai::execute::task_execution::types::HaulWaterToMixerPhase::GoingToBucket,
        },
    );
    let mut reservation_ops = vec![
        ResourceReservationOp::ReserveSource {
            source: bucket,
            amount: 1,
        },
        // タンクから水を汲む作業は同時実行を1件に制限して競合を防ぐ
        ResourceReservationOp::ReserveSource {
            source: tank,
            amount: 1,
        },
    ];
    if !mixer_already_reserved {
        reservation_ops.push(ResourceReservationOp::ReserveMixerDestination {
            target: mixer,
            resource_type: ResourceType::Water,
        });
    }

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
