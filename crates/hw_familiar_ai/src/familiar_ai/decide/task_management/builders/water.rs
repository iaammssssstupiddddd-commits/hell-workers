use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_jobs::{
    AssignedTask, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource, WorkType,
};

use super::{
    TaskTarget, build_mixer_destination_reservation_ops, build_source_reservation_ops,
    submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn issue_gather_water(
    bucket: Entity,
    tank: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
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
        TaskTarget { work_type: WorkType::GatherWater, task_pos },
        assigned_task,
        &[bucket],
        already_commanded,
    );
}

/// `issue_haul_water_to_mixer` のデータをまとめた構造体。
pub struct WaterHaulSpec {
    pub bucket: Entity,
    pub mixer: Entity,
    pub tank: Entity,
    pub needs_tank_fill: bool,
    pub mixer_already_reserved: bool,
}

pub fn issue_haul_water_to_mixer(
    spec: WaterHaulSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::BucketTransport(BucketTransportData {
        bucket: spec.bucket,
        source: BucketTransportSource::Tank {
            tank: spec.tank,
            needs_fill: spec.needs_tank_fill,
        },
        destination: BucketTransportDestination::Mixer(spec.mixer),
        amount: 0,
        phase: BucketTransportPhase::GoingToBucket,
    });
    let mut reservation_ops = build_source_reservation_ops(&[spec.bucket]);
    if spec.needs_tank_fill {
        reservation_ops.extend(build_source_reservation_ops(&[spec.tank]));
    }
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        spec.mixer,
        ResourceType::Water,
        spec.mixer_already_reserved,
    ));
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget { work_type: WorkType::HaulWaterToMixer, task_pos },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
