use bevy::prelude::*;

/// Yard ごとに reconciler が一意化する電力網エンティティ。
/// topology / output / policy の dirty transaction で集計値を再計算する。
/// 初期状態: generation=0, consumption=0, powered=true（消費者なし＝停電ではない）
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct PowerGrid {
    /// 接続全 PowerGenerator の current_output 合計
    pub generation: f32,
    /// 接続全 PowerConsumer の demand 合計
    pub consumption: f32,
    /// 接続 consumer がすべて給電中のとき true
    pub powered: bool,
}

impl Default for PowerGrid {
    fn default() -> Self {
        Self {
            generation: 0.0,
            consumption: 0.0,
            powered: true, // 空グリッドは powered（消費者がいない＝停電ではない）
        }
    }
}

/// SoulSpaSite に付与。サイト単位の発電集計。
/// `#[require(PowerGenerator)]` で SoulSpaSite に自動付与される。
/// Default で `output_per_soul = OUTPUT_PER_SOUL` が設定される。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
pub struct PowerGenerator {
    /// 実際の出力: 占有スロット数 × output_per_soul
    pub current_output: f32,
    /// Soul 1 体あたりの発電量。通常は OUTPUT_PER_SOUL 定数と同値。
    /// フィールドとして保持する理由: 将来の上位施設（効率の良い Soul Spa 等）で
    /// 施設ごとに異なる値を設定可能にするため。
    pub output_per_soul: f32,
}

impl Default for PowerGenerator {
    fn default() -> Self {
        Self {
            current_output: 0.0,
            output_per_soul: crate::constants::OUTPUT_PER_SOUL,
        }
    }
}

/// 電力消費建物（OutdoorLamp 等）に付与。
/// `#[require(Unpowered)]` により、グリッド接続前はデフォルトで停電状態になる。
#[derive(Component, Reflect, Debug, Clone)]
#[reflect(Component)]
#[require(Unpowered, PowerConsumerPolicy)]
pub struct PowerConsumer {
    /// 稼働時の消費電力（/秒）
    pub demand: f32,
}

#[derive(Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PowerPriority {
    Low,
    #[default]
    Normal,
    High,
}

impl PowerPriority {
    pub const fn allocation_rank(self) -> u8 {
        match self {
            Self::High => 0,
            Self::Normal => 1,
            Self::Low => 2,
        }
    }
}

/// Consumerごとの永続配電方針。
#[derive(Component, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct PowerConsumerPolicy {
    pub priority: PowerPriority,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerConsumerPolicyChangeOutcome {
    pub target: Entity,
    pub status: PowerConsumerPolicyChangeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerConsumerPolicyChangeStatus {
    Applied {
        previous: PowerPriority,
        applied: PowerPriority,
    },
    StaleTarget,
    UnsupportedTarget,
    MissingPolicy,
}

#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerShedReason {
    InsufficientGeneration,
    RestoreMargin,
    LegacyGlobalDeficit,
}

/// 配電transactionから再構築されるconsumerのruntime状態。
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub enum PowerSupplyState {
    Supplied,
    Shed { reason: PowerShedReason },
    Disconnected,
    InvalidDemand,
}

/// 直近の配電 transaction が PowerGrid ごとに確定した集計値。
///
/// セーブ対象ではなく、ロード後の topology/配電再構築で再生成する。
#[derive(Component, Reflect, Debug, Clone, PartialEq)]
#[reflect(Component)]
pub struct PowerGridAllocationSummary {
    pub mode: PowerAllocationMode,
    pub generation: f32,
    pub total_demand: f32,
    pub served_demand: f32,
    pub consumer_count: usize,
    pub supplied_count: usize,
    pub shed_count: usize,
    pub invalid_count: usize,
    #[entities]
    pub shed_order: Vec<Entity>,
}

impl Default for PowerGridAllocationSummary {
    fn default() -> Self {
        Self {
            mode: PowerAllocationMode::default(),
            generation: 0.0,
            total_demand: 0.0,
            served_demand: 0.0,
            consumer_count: 0,
            supplied_count: 0,
            shed_count: 0,
            invalid_count: 0,
            shed_order: Vec::new(),
        }
    }
}

#[derive(Resource, Reflect, Debug, Default, Clone, Copy, PartialEq, Eq)]
#[reflect(Resource)]
pub enum PowerAllocationMode {
    LegacyAllOrNone,
    #[default]
    PriorityPrefix,
}

/// マーカー: この Consumer は電力供給を受けていない。
/// `#[require(Unpowered)]` によりデフォルトで付与。
/// グリッド再計算で供給が確認されると除去され、停電時に再挿入される。
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
#[reflect(Component)]
pub struct Unpowered;

/// PowerGrid エンティティ上に付与。所属する Yard への逆参照。
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct YardPowerGrid(#[entities] pub Entity);

impl Default for YardPowerGrid {
    fn default() -> Self {
        Self(Entity::PLACEHOLDER)
    }
}
