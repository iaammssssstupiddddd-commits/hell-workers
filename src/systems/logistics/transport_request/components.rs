use super::kinds::TransportRequestKind;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

/// 運搬リクエストの優先度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Reflect)]
pub enum TransportPriority {
    Low = 0,
    Normal = 10,
    High = 20,
    Critical = 30,
}

impl Default for TransportPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// 運搬リクエスト本体
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportRequest {
    pub kind: TransportRequestKind,
    /// アンカーエンティティ（Stockpile, Blueprint, Mixer など）
    pub anchor: Entity,
    pub resource_type: ResourceType,
    /// リクエストを発行した Familiar
    pub issued_by: Entity,
    pub priority: TransportPriority,
}

/// 需要管理
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportDemand {
    /// 必要なスロット数
    pub desired_slots: u32,
    /// 現在運搬中のスロット数
    pub inflight: u32,
}

impl TransportDemand {
    pub fn remaining(&self) -> u32 {
        self.desired_slots.saturating_sub(self.inflight)
    }

    pub fn is_satisfied(&self) -> bool {
        self.remaining() == 0
    }
}

/// リース（ワーカーによるクレーム）
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportLease {
    pub claimed_by_worker: Entity,
    pub lease_until: f64,
    pub attempts: u32,
    pub retry_at: Option<f64>,
}

/// 運搬ポリシー
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TransportPolicy {
    pub allow_cross_area_source: bool,
    pub allow_cross_familiar_claim: bool,
    pub source_search_radius_tiles: f32,
}

impl Default for TransportPolicy {
    fn default() -> Self {
        Self {
            allow_cross_area_source: false,
            allow_cross_familiar_claim: false,
            source_search_radius_tiles: 20.0,
        }
    }
}

/// 運搬リクエストの状態
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum TransportRequestState {
    Pending,
    Claimed,
    InFlight,
    CoolingDown,
    Completed,
}

impl Default for TransportRequestState {
    fn default() -> Self {
        Self::Pending
    }
}

/// 同フレーム内の競合回避用: タスク発行済みアイテム
#[derive(Resource, Default)]
pub struct ItemReservations(pub std::collections::HashSet<Entity>);
