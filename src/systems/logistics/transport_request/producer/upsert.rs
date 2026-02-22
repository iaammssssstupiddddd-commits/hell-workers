//! Producer upsert/cleanup 共通化ヘルパー
//!
//! Phase 1: 重複 request 排除、需要ゼロ時の disable/despawn、existing 更新 / new spawn
//! の共通パターンを提供する。各 producer は需要計算に集中し、このヘルパーで
//! 実際の upsert/cleanup を行う。

use bevy::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;

use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};

/// 既存 request のループで重複を検出した際の処理
/// 戻り値: 続行するか (false = continue でスキップ)
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

/// 需要なし時の disable（Designation/TaskSlots/Priority を remove）
/// 需要がなくなったがアンカーは存在する場合に呼ぶ
#[inline]
pub fn disable_request(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .try_remove::<(Designation, TaskSlots, Priority)>();
}

/// 需要なし時に disable しつつ、inflight を維持して Demand を更新する。
#[inline]
pub fn disable_request_with_demand(commands: &mut Commands, entity: Entity, inflight: u32) {
    disable_request(commands, entity);
    commands.entity(entity).try_insert(TransportDemand {
        desired_slots: 0,
        inflight,
    });
}

/// 既存 request entity を指定内容で upsert する。
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
    commands.entity(request_entity).try_insert((
        Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: WorkType::Haul,
        },
        crate::relationships::ManagedBy(issued_by),
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

/// 新規 request entity を spawn する。
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
    commands.spawn((
        Name::new(name),
        Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
        Visibility::Hidden,
        Designation {
            work_type: WorkType::Haul,
        },
        crate::relationships::ManagedBy(issued_by),
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
