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

/// 既存 request entity を指定内容で upsert するためのスペック。
pub struct UpsertRequestSpec<TTarget> {
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub desired_slots: u32,
    pub inflight: u32,
    pub priority: u32,
    pub target: TTarget,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
}

/// 新規 request entity を spawn するためのスペック。
pub struct SpawnRequestSpec<TTarget> {
    pub name: &'static str,
    pub key: (Entity, ResourceType),
    pub site_pos: Vec2,
    pub issued_by: Entity,
    pub desired_slots: u32,
    pub priority: u32,
    pub target: TTarget,
    pub kind: TransportRequestKind,
    pub work_type: WorkType,
}

/// 既存 request entity を指定内容で upsert する
#[inline]
pub fn upsert_transport_request<TTarget: Component>(
    commands: &mut Commands,
    request_entity: Entity,
    spec: UpsertRequestSpec<TTarget>,
) {
    commands.entity(request_entity).try_insert((
        Transform::from_xyz(spec.site_pos.x, spec.site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: spec.work_type,
        },
        ManagedBy(spec.issued_by),
        TaskSlots::new(spec.desired_slots),
        Priority(spec.priority),
        spec.target,
        TransportRequest {
            kind: spec.kind,
            anchor: spec.key.0,
            resource_type: spec.key.1,
            issued_by: spec.issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots: spec.desired_slots,
            inflight: spec.inflight,
        },
        TransportPolicy::default(),
    ));
}

/// 新規 request entity を spawn する
#[inline]
pub fn spawn_transport_request<TTarget: Component>(
    commands: &mut Commands,
    spec: SpawnRequestSpec<TTarget>,
) {
    commands.spawn((
        Name::new(spec.name),
        Transform::from_xyz(spec.site_pos.x, spec.site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: spec.work_type,
        },
        ManagedBy(spec.issued_by),
        TaskSlots::new(spec.desired_slots),
        Priority(spec.priority),
        spec.target,
        TransportRequest {
            kind: spec.kind,
            anchor: spec.key.0,
            resource_type: spec.key.1,
            issued_by: spec.issued_by,
            priority: TransportPriority::Normal,
            stockpile_group: vec![],
        },
        TransportDemand {
            desired_slots: spec.desired_slots,
            inflight: 0,
        },
        TransportRequestState::Pending,
        TransportPolicy::default(),
    ));
}
