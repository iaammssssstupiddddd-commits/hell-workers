//! 一輪車運搬系 builder — `HaulWithWheelbarrow` タスクを生成するすべての関数。

use bevy::prelude::*;
use hw_core::logistics::{ResourceType, WheelbarrowDestination};
use hw_jobs::WorkType;
use hw_jobs::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase};

use super::{
    TaskTarget, build_mixer_destination_reservation_ops, build_wheelbarrow_reservation_ops,
    submit_assignment_with_reservation_ops, submit_assignment_with_source_entities,
};
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

/// `issue_haul_with_wheelbarrow` の引数をまとめた構造体。
pub struct WheelbarrowHaulSpec {
    pub wheelbarrow: Entity,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    pub items: Vec<Entity>,
}

/// `issue_return_wheelbarrow` の引数をまとめた構造体。
pub struct ReturnWheelbarrowSpec {
    pub wheelbarrow: Entity,
    pub parking_anchor: Entity,
    pub wheelbarrow_pos: Vec2,
}

/// 一輪車での collect 系 builder の共通引数をまとめた構造体。
pub struct WheelbarrowCollectSpec {
    pub wheelbarrow: Entity,
    pub source_entity: Entity,
    pub source_pos: Vec2,
    /// 各 builder が独自の `WheelbarrowDestination` に変換するデスティネーションエンティティ。
    pub destination: Entity,
    pub amount: u32,
}

pub fn issue_haul_with_wheelbarrow(
    spec: WheelbarrowHaulSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination: spec.destination,
        collect_source: None,
        collect_amount: 0,
        collect_resource_type: None,
        items: spec.items.clone(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &spec.destination,
        &spec.items,
        &spec.items,
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_return_wheelbarrow(
    spec: ReturnWheelbarrowSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.wheelbarrow_pos,
        destination: WheelbarrowDestination::Stockpile(spec.parking_anchor),
        collect_source: None,
        collect_amount: 0,
        collect_resource_type: None,
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    submit_assignment_with_source_entities(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        &[spec.wheelbarrow],
        already_commanded,
    );
}

pub fn issue_collect_sand_with_wheelbarrow_to_blueprint(
    spec: WheelbarrowCollectSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = spec.amount.max(1);
    let destination = WheelbarrowDestination::Blueprint(spec.destination);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination,
        collect_source: Some(spec.source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Sand),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &destination,
        &[spec.source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_sand_with_wheelbarrow_to_mixer(
    spec: WheelbarrowCollectSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = spec.amount.max(1);
    let destination = WheelbarrowDestination::Mixer {
        entity: spec.destination,
        resource_type: ResourceType::Sand,
    };
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination,
        collect_source: Some(spec.source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Sand),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    // Reserve wheelbarrow + sand source, then mixer destination slots for the items we'll generate
    let mut reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &destination,
        &[spec.source_entity],
        &[],
    );
    for _ in 0..haul_amount {
        reservation_ops.extend(build_mixer_destination_reservation_ops(
            spec.destination,
            ResourceType::Sand,
            false,
        ));
    }
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_bone_with_wheelbarrow_to_blueprint(
    spec: WheelbarrowCollectSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = spec.amount.max(1);
    let destination = WheelbarrowDestination::Blueprint(spec.destination);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination,
        collect_source: Some(spec.source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Bone),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &destination,
        &[spec.source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

pub fn issue_collect_bone_with_wheelbarrow_to_floor(
    spec: WheelbarrowCollectSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = spec.amount.max(1);
    let destination = WheelbarrowDestination::Stockpile(spec.destination);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination,
        collect_source: Some(spec.source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Bone),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &destination,
        &[spec.source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}

/// Soul Spa 建設用の Bone 回収 → 搬入タスク（ホイールバロー使用）
pub fn issue_collect_bone_with_wheelbarrow_to_soul_spa(
    spec: WheelbarrowCollectSpec,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) {
    let haul_amount = spec.amount.max(1);
    let destination = WheelbarrowDestination::Stockpile(spec.destination);
    let assigned_task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        wheelbarrow: spec.wheelbarrow,
        source_pos: spec.source_pos,
        destination,
        collect_source: Some(spec.source_entity),
        collect_amount: haul_amount,
        collect_resource_type: Some(ResourceType::Bone),
        items: Vec::new(),
        phase: HaulWithWheelbarrowPhase::GoingToParking,
    });

    let reservation_ops = build_wheelbarrow_reservation_ops(
        queries,
        spec.wheelbarrow,
        &destination,
        &[spec.source_entity],
        &[],
    );
    submit_assignment_with_reservation_ops(
        ctx,
        queries,
        shadow,
        TaskTarget {
            work_type: WorkType::WheelbarrowHaul,
            task_pos,
        },
        assigned_task,
        reservation_ops,
        already_commanded,
    );
}
