//! Producer upsert/cleanup 共通化ヘルパー

use bevy::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;

use hw_core::relationships::ManagedBy;
use hw_jobs::{Designation, Priority, TaskSlots, WorkType};

use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;

/// 既存 request のループで重複を検出した際の処理
#[inline]
pub fn process_duplicate_key<K: Hash + Eq>(
    commands: &mut Commands,
    entity: Entity,
    workers: usize,
    seen: &mut HashSet<K>,
    key: K,
) -> bool {
    if !seen.insert(key) {
        if workers == 0 {
            commands.entity(entity).try_despawn();
        }
        return false;
    }
    true
}

/// 需要なし時の disable
#[inline]
pub fn disable_request(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .try_remove::<(Designation, TaskSlots, Priority)>();
}

/// 需要なし時に disable しつつ、inflight を維持して Demand を更新する
#[inline]
pub fn disable_request_with_demand(commands: &mut Commands, entity: Entity, inflight: u32) {
    disable_request(commands, entity);
    commands.entity(entity).try_insert(TransportDemand {
        desired_slots: 0,
        inflight,
    });
}

#[inline]
pub fn request_state_for_workers(workers: usize) -> TransportRequestState {
    if workers == 0 {
        TransportRequestState::Pending
    } else {
        TransportRequestState::Claimed
    }
}

/// 既存 request entity を指定内容で upsert する
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn upsert_transport_request<TTarget: Component>(
    commands: &mut Commands,
    request_entity: Entity,
    key: (Entity, ResourceType),
    site_pos: Vec2,
    issued_by: Entity,
    desired_slots: u32,
    inflight: u32,
    priority: u32,
    target: TTarget,
    kind: TransportRequestKind,
) {
    upsert_transport_request_with_work_type(
        commands,
        request_entity,
        key,
        site_pos,
        issued_by,
        desired_slots,
        inflight,
        priority,
        target,
        kind,
        WorkType::Haul,
    );
}

#[allow(clippy::too_many_arguments)]
#[inline]
pub fn upsert_transport_request_with_work_type<TTarget: Component>(
    commands: &mut Commands,
    request_entity: Entity,
    key: (Entity, ResourceType),
    site_pos: Vec2,
    issued_by: Entity,
    desired_slots: u32,
    inflight: u32,
    priority: u32,
    target: TTarget,
    kind: TransportRequestKind,
    work_type: WorkType,
) {
    commands.entity(request_entity).try_insert((
        Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
        Visibility::Hidden,
        Designation { work_type },
        ManagedBy(issued_by),
        TaskSlots::new(desired_slots),
        Priority(priority),
        target,
        TransportRequest {
            kind,
            anchor: key.0,
            resource_type: key.1,
            issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots,
            inflight,
        },
        TransportPolicy::default(),
    ));
}

/// 新規 request entity を spawn する
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn spawn_transport_request<TTarget: Component>(
    commands: &mut Commands,
    name: &'static str,
    key: (Entity, ResourceType),
    site_pos: Vec2,
    issued_by: Entity,
    desired_slots: u32,
    priority: u32,
    target: TTarget,
    kind: TransportRequestKind,
) {
    spawn_transport_request_with_work_type(
        commands,
        name,
        key,
        site_pos,
        issued_by,
        desired_slots,
        priority,
        target,
        kind,
        WorkType::Haul,
    );
}

#[allow(clippy::too_many_arguments)]
#[inline]
pub fn spawn_transport_request_with_work_type<TTarget: Component>(
    commands: &mut Commands,
    name: &'static str,
    key: (Entity, ResourceType),
    site_pos: Vec2,
    issued_by: Entity,
    desired_slots: u32,
    priority: u32,
    target: TTarget,
    kind: TransportRequestKind,
    work_type: WorkType,
) {
    commands.spawn((
        Name::new(name),
        Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
        Visibility::Hidden,
        Designation { work_type },
        ManagedBy(issued_by),
        TaskSlots::new(desired_slots),
        Priority(priority),
        target,
        TransportRequest {
            kind,
            anchor: key.0,
            resource_type: key.1,
            issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots,
            inflight: 0,
        },
        TransportRequestState::Pending,
        TransportPolicy::default(),
    ));
}
