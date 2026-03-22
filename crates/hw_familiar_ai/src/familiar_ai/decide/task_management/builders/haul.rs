//! 手運搬系 builder — 一輪車を使わない `Haul`/`HaulToBlueprint`/`HaulToMixer` タスクを生成する。

use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_jobs::WorkType;
use hw_jobs::{
    AssignedTask, HaulData, HaulPhase, HaulToBlueprintData, HaulToBpPhase, HaulToMixerData,
    HaulToMixerPhase,
};

use super::{
    build_mixer_destination_reservation_ops, build_source_reservation_ops,
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
        WorkType::Haul,
        task_pos,
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
        WorkType::Haul,
        task_pos,
        assigned_task,
        &[source_item],
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
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulToMixer(HaulToMixerData {
        item: source_item,
        mixer,
        resource_type: item_type,
        phase: HaulToMixerPhase::GoingToItem,
    });
    let mut reservation_ops = build_source_reservation_ops(&[source_item]);
    reservation_ops.extend(build_mixer_destination_reservation_ops(
        mixer,
        item_type,
        mixer_already_reserved,
    ));
    submit_assignment_with_reservation_ops(
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
