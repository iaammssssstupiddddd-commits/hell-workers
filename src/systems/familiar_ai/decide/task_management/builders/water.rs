use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use bevy::prelude::*;

use super::{submit_assignment_with_spec, AssignmentSpec, build_source_reservation_ops, build_mixer_destination_reservation_ops};

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
    let reservation_ops = build_source_reservation_ops(&[bucket]);
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::GatherWater,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
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
    let mut reservation_ops = build_source_reservation_ops(&[bucket, tank]);
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        mixer,
        ResourceType::Water,
        mixer_already_reserved,
    ));
    submit_assignment_with_spec(
        ctx,
        queries,
        shadow,
        AssignmentSpec {
            work_type: WorkType::HaulWaterToMixer,
            task_pos,
            assigned_task,
            reservation_ops,
            already_commanded,
        },
    );
}
