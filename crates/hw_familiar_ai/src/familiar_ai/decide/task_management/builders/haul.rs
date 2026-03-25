//! 手運搬系 builder — 一輪車を使わない `Haul`/`HaulToBlueprint`/`HaulToMixer` タスクを生成する。

use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_jobs::WorkType;
use hw_jobs::{
    AssignedTask, HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase, HaulToMixerData,
    HaulToMixerPhase,
};

use super::{
    TaskTarget, build_mixer_destination_reservation_ops, build_source_reservation_ops,
    submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
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
        TaskTarget { work_type: WorkType::Haul, task_pos },
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
        TaskTarget { work_type: WorkType::Haul, task_pos },
        assigned_task,
        &[source_item],
        already_commanded,
    );
}

/// `issue_haul_to_mixer` のデータをまとめた構造体。
pub struct MixerHaulSpec {
    pub source_item: Entity,
    pub mixer: Entity,
    pub item_type: ResourceType,
    pub mixer_already_reserved: bool,
}

pub fn issue_haul_to_mixer(
    spec: MixerHaulSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulToMixer(HaulToMixerData {
        item: spec.source_item,
        mixer: spec.mixer,
        resource_type: spec.item_type,
        phase: HaulToMixerPhase::GoingToItem,
    });
    let mut reservation_ops = build_source_reservation_ops(&[spec.source_item]);
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        spec.mixer,
        spec.item_type,
        spec.mixer_already_reserved,
    ));
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget { work_type: WorkType::HaulToMixer, task_pos },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
