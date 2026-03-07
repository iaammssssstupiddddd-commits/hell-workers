use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource,
};
use bevy::prelude::*;

use super::{
    build_mixer_destination_reservation_ops,
    build_source_reservation_ops,
    submit_assignment_with_reservation_ops,
    submit_assignment_with_source_entities,
};

pub fn issue_gather_water(
    bucket: Entity,
    tank: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::BucketTransport(BucketTransportData {
        bucket,
        source: BucketTransportSource::River,
        destination: BucketTransportDestination::Tank(tank),
        amount: 1,
        phase: BucketTransportPhase::GoingToBucket,
    });
    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        WorkType::GatherWater,
        task_pos,
        assigned_task,
        &[bucket],
        already_commanded,
    );
}

pub fn issue_haul_water_to_mixer(
    bucket: Entity,
    mixer: Entity,
    tank: Entity,
    needs_tank_fill: bool,
    mixer_already_reserved: bool,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::BucketTransport(BucketTransportData {
        bucket,
        source: BucketTransportSource::Tank {
            tank,
            needs_fill: needs_tank_fill,
        },
        destination: BucketTransportDestination::Mixer(mixer),
        amount: 0,
        phase: BucketTransportPhase::GoingToBucket,
    });
    let mut reservation_ops = build_source_reservation_ops(&[bucket]);
    if needs_tank_fill {
        reservation_ops.extend(build_source_reservation_ops(&[tank]));
    }
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        mixer,
        ResourceType::Water,
        mixer_already_reserved,
    ));
    submit_assignment_with_reservation_ops(
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
