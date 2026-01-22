use bevy::prelude::*;

pub mod maintenance;
pub mod spawn;
pub mod visual;

use crate::constants::*;

// ============================================================
// 定数
// ============================================================

/// 集会発生までの基本待機時間 (秒)
pub const GATHERING_SPAWN_BASE_TIME: f32 = 10.0;
/// 近くにいるSoul1人あたりの発生時間短縮 (秒)
pub const GATHERING_SPAWN_TIME_REDUCTION_PER_SOUL: f32 = 1.5;
/// 近傍Soul検出半径
pub const GATHERING_DETECTION_RADIUS: f32 = TILE_SIZE * 5.0;
/// 集会の最大参加人数
pub const GATHERING_MAX_CAPACITY: usize = 8;
/// 集会維持に必要な最低人数
pub const GATHERING_MIN_PARTICIPANTS: usize = 2;
/// 集会消滅までの猶予時間 (秒)
pub const GATHERING_GRACE_PERIOD: f32 = 10.0;
/// 統合の初期距離 (タイル)
pub const GATHERING_MERGE_INITIAL_DISTANCE: f32 = TILE_SIZE * 2.0;
/// 統合の最大距離 (タイル)
pub const GATHERING_MERGE_MAX_DISTANCE: f32 = TILE_SIZE * 10.0;
/// 統合距離の基本拡大速度 (タイル/秒)
pub const GATHERING_MERGE_GROWTH_BASE_SPEED: f32 = TILE_SIZE * 0.3;
/// オーラの基本サイズ
pub const GATHERING_AURA_BASE_SIZE: f32 = TILE_SIZE * 3.0;
/// オーラの1人あたりサイズ増加
pub const GATHERING_AURA_SIZE_PER_PERSON: f32 = TILE_SIZE * 0.5;

/// 集会から離脱する半径 (検出半径より大きくしてチャタリングを防ぐ)
pub const GATHERING_LEAVE_RADIUS: f32 = TILE_SIZE * 7.5;

// ============================================================
// コンポーネント
// ============================================================

/// 集会の中心オブジェクトタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GatheringObjectType {
    #[default]
    Nothing, // オブジェクトなし (オーラのみ)
    CardTable, // トランプ台
    Campfire,  // 焚き火
    Barrel,    // 酒樽
}

impl GatheringObjectType {
    /// 参加人数に応じた確率テーブルでランダム選択
    pub fn random_weighted(participant_count: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let roll: f32 = rng.gen_range(0.0..1.0);

        // 人数による確率テーブル
        let (nothing, card_table, campfire) = match participant_count {
            0..=4 => (0.50, 0.80, 0.90), // Nothing 50%, CardTable 30%, Campfire 10%, Barrel 10%
            5..=6 => (0.20, 0.70, 0.90), // Nothing 20%, CardTable 50%, Campfire 20%, Barrel 10%
            _ => (0.05, 0.30, 0.70),     // Nothing 5%, CardTable 25%, Campfire 40%, Barrel 30%
        };

        if roll < nothing {
            GatheringObjectType::Nothing
        } else if roll < card_table {
            GatheringObjectType::CardTable
        } else if roll < campfire {
            GatheringObjectType::Campfire
        } else {
            GatheringObjectType::Barrel
        }
    }

    /// アセットパス (Nothing の場合は None)
    pub fn asset_path(&self) -> Option<&'static str> {
        match self {
            GatheringObjectType::Nothing => None,
            GatheringObjectType::CardTable => Some("textures/ui/card_table.png"),
            GatheringObjectType::Campfire => Some("textures/ui/campfire.png"),
            GatheringObjectType::Barrel => Some("textures/ui/barrel.png"),
        }
    }
}

/// 集会スポットコンポーネント
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct GatheringSpot {
    /// 中心座標
    pub center: Vec2,
    /// 現在の参加人数
    pub participants: usize,
    /// 最大参加人数
    pub max_capacity: usize,
    /// 消滅猶予タイマー (残り秒)
    pub grace_timer: f32,
    /// 猶予が発動中か
    pub grace_active: bool,
    /// 中心オブジェクトの種類
    pub object_type: GatheringObjectType,
    /// 生成時刻 (統合時の先着判定用)
    pub created_at: f32,
}

impl Default for GatheringSpot {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            participants: 0,
            max_capacity: GATHERING_MAX_CAPACITY,
            grace_timer: GATHERING_GRACE_PERIOD,
            grace_active: true, // 発生直後は猶予期間
            object_type: GatheringObjectType::Nothing,
            created_at: 0.0,
        }
    }
}

/// 集会スポットのビジュアル要素へのリンク
#[derive(Component, Debug)]
pub struct GatheringVisuals {
    /// オーラエンティティ
    pub aura_entity: Entity,
    /// 中心オブジェクトエンティティ (Nothing の場合は None)
    pub object_entity: Option<Entity>,
}

/// Soulが参加中の集会スポットへの参照
#[derive(Component, Debug)]
pub struct ParticipatingIn(pub Entity);

/// 集会発生の準備状態 (Soulに付与)
#[derive(Component, Debug, Default)]
pub struct GatheringReadiness {
    /// 集会発生までの累計待機時間
    pub idle_time: f32,
}

/// 集会システムの更新頻度を制御するタイマー
#[derive(Resource)]
pub struct GatheringUpdateTimer {
    pub timer: Timer,
}

impl Default for GatheringUpdateTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

// ============================================================
// ヘルパー関数
// ============================================================

/// 統合距離を計算 (時間経過で拡大、人数が少ないほど速く拡大)
pub fn calculate_merge_distance(participant_count: usize, elapsed_time: f32) -> f32 {
    let count = (participant_count.max(1)) as f32;
    let speed = GATHERING_MERGE_GROWTH_BASE_SPEED * (GATHERING_MAX_CAPACITY as f32 / count);
    let distance = GATHERING_MERGE_INITIAL_DISTANCE + elapsed_time * speed;
    distance.min(GATHERING_MERGE_MAX_DISTANCE)
}

/// オーラサイズを計算
pub fn calculate_aura_size(participant_count: usize) -> f32 {
    GATHERING_AURA_BASE_SIZE + (participant_count as f32) * GATHERING_AURA_SIZE_PER_PERSON
}

// ============================================================
// Observers (イベント駆動による参加者数更新)
// ============================================================

/// ParticipatingIn追加時に参加者数をインクリメント
pub fn on_participating_added(
    on: On<crate::events::OnGatheringParticipated>,
    mut q_spots: Query<&mut GatheringSpot>,
) {
    let event = on.event();
    if let Ok(mut spot) = q_spots.get_mut(event.spot_entity) {
        spot.participants += 1;
    }
}

/// ParticipatingIn削除時に参加者数をデクリメント
pub fn on_participating_removed(
    on: On<crate::events::OnGatheringLeft>,
    mut q_spots: Query<&mut GatheringSpot>,
) {
    let event = on.event();
    if let Ok(mut spot) = q_spots.get_mut(event.spot_entity) {
        spot.participants = spot.participants.saturating_sub(1);
    }
}

/// 集会システムのタイマーを更新するシステム
pub fn tick_gathering_timer_system(time: Res<Time>, mut timer: ResMut<GatheringUpdateTimer>) {
    timer.timer.tick(time.delta());
}
