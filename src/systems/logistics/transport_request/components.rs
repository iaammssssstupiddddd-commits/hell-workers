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
    /// DepositToStockpile: グループ内全セル（他種別では空Vec）
    pub stockpile_group: Vec<Entity>,
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

/// 猫車運搬の搬送先
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum WheelbarrowDestination {
    Stockpile(Entity),
    Blueprint(Entity),
    Mixer {
        entity: Entity,
        resource_type: ResourceType,
    },
}

impl WheelbarrowDestination {
    pub fn entity(self) -> Entity {
        match self {
            Self::Stockpile(entity) | Self::Blueprint(entity) => entity,
            Self::Mixer { entity, .. } => entity,
        }
    }
}

/// 猫車小バッチ許可制御のため、request が Pending になった時刻を保持
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct WheelbarrowPendingSince(pub f64);

/// 手押し車リース（仲裁システムによる割り当て結果）
///
/// request エンティティに付与される。仲裁システムが「どの request に手押し車を割り当てるか」を
/// 一括決定し、その結果をこのコンポーネントで保持する。
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct WheelbarrowLease {
    pub wheelbarrow: Entity,
    pub items: Vec<Entity>,
    pub source_pos: Vec2,
    pub destination: WheelbarrowDestination,
    pub lease_until: f64,
}

/// 運搬リクエストの状態
///
/// Phase 3: 実運用は Pending/Claimed のみ。TaskWorkers の有無に応じて state_machine が同期。
/// InFlight/CoolingDown/Completed は未使用のため削除し、方針A（2状態に縮退）を採用。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum TransportRequestState {
    Pending,
    Claimed,
}

impl Default for TransportRequestState {
    fn default() -> Self {
        Self::Pending
    }
}

