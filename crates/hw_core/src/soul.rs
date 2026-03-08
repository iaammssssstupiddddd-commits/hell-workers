//! ソウル（魂）のコアコンポーネント定義
//!
//! UI/アニメーション/スポーン非依存の純粋なAIコンポーネント。

use bevy::prelude::*;

/// 地獄に堕ちた人間（怠惰な魂）
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct DamnedSoul {
    pub laziness: f32,   // 怠惰レベル (0.0-1.0)
    pub motivation: f32, // やる気 (0.0-1.0)
    pub fatigue: f32,    // 疲労 (0.0-1.0)
    pub stress: f32,     // ストレス (0.0-1.0)
    pub dream: f32,      // 夢の貯蔵量 (0.0-100.0)
}

impl Default for DamnedSoul {
    fn default() -> Self {
        Self {
            laziness: 0.7,
            motivation: 0.1,
            fatigue: 0.0,
            stress: 0.0,
            dream: 0.0,
        }
    }
}

/// グローバルDreamプール（通貨）
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct DreamPool {
    pub points: f32,
}

/// Soul個別の夢状態（質の追跡用）
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct DreamState {
    pub quality: DreamQuality,
}

/// 睡眠中の夢の質
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum DreamQuality {
    #[default]
    Awake,
    NightTerror,
    NormalDream,
    VividDream,
}

/// 怠惰状態のコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct IdleState {
    pub idle_timer: f32,
    pub total_idle_time: f32,
    pub behavior: IdleBehavior,
    pub behavior_duration: f32,
    pub gathering_behavior: GatheringBehavior,
    pub gathering_behavior_timer: f32,
    pub gathering_behavior_duration: f32,
    pub needs_separation: bool,
}

impl Default for IdleState {
    fn default() -> Self {
        Self {
            idle_timer: 0.0,
            total_idle_time: 0.0,
            behavior: IdleBehavior::Wandering,
            behavior_duration: 3.0,
            gathering_behavior: GatheringBehavior::Wandering,
            gathering_behavior_timer: 0.0,
            gathering_behavior_duration: 60.0,
            needs_separation: false,
        }
    }
}

/// 怠惰行動の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum IdleBehavior {
    #[default]
    Wandering,
    Sitting,
    Sleeping,
    Gathering,
    ExhaustedGathering,
    Resting,
    GoingToRest,
    Escaping,
    Drifting,
}

/// 集会中のサブ行動
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GatheringBehavior {
    #[default]
    Wandering,
    Sleeping,
    Standing,
    Dancing,
}

/// 移動先
#[derive(Component)]
pub struct Destination(pub Vec2);

/// 経路
#[derive(Component, Default)]
pub struct Path {
    pub waypoints: Vec<Vec2>,
    pub current_index: usize,
}

/// ストレスによるブレイクダウン状態
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct StressBreakdown {
    pub is_frozen: bool,
    pub remaining_freeze_secs: f32,
}

/// 休憩所退出後のリクルート不可クールダウン
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct RestAreaCooldown {
    pub remaining_secs: f32,
}

/// 漂流状態の現在フェーズ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum DriftPhase {
    #[default]
    Wandering,
    Moving,
}

/// 漂流の最終目標となるマップ端
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DriftEdge {
    North,
    South,
    East,
    West,
}

/// 漂流（自然脱走）中の実行状態
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct DriftingState {
    pub target_edge: DriftEdge,
    pub phase: DriftPhase,
    pub phase_timer: f32,
    pub phase_duration: f32,
}
