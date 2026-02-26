use bevy::prelude::*;

pub mod movement;
pub mod names;
pub mod observers;
pub mod spawn;

use names::{FEMALE_NAMES, MALE_NAMES};
use rand::Rng;

/// ソウルのスポーンイベント
#[derive(Message)]
pub struct DamnedSoulSpawnEvent {
    pub position: Vec2,
}

/// 性別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum Gender {
    #[default]
    Male,
    Female,
}

/// 魂のアイデンティティ（名前と性別）
#[derive(Component, Debug, Clone)]
pub struct SoulIdentity {
    pub name: String,
    pub gender: Gender,
}

impl SoulIdentity {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let gender = if rng.gen_bool(0.5) {
            Gender::Male
        } else {
            Gender::Female
        };
        let name = match gender {
            Gender::Male => MALE_NAMES[rng.gen_range(0..MALE_NAMES.len())].to_string(),
            Gender::Female => FEMALE_NAMES[rng.gen_range(0..FEMALE_NAMES.len())].to_string(),
        };
        Self { name, gender }
    }
}

/// 睡眠中の夢の質
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum DreamQuality {
    #[default]
    Awake, // 起きている
    NightTerror, // 悪夢（高ストレス時）
    NormalDream, // 普通の夢
    VividDream,  // 鮮明な夢（低ストレス＋集会中）
}

/// Soul個別の夢状態（質の追跡用）
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct DreamState {
    pub quality: DreamQuality,
}

/// グローバルDreamプール（通貨）
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct DreamPool {
    pub points: f32,
}

/// 地獄に堕ちた人間（怠惰な魂）
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct DamnedSoul {
    pub laziness: f32,   // 怠惰レベル (0.0-1.0) - 内部ステータス
    pub motivation: f32, // やる気 (0.0-1.0) - 高いほど働く
    pub fatigue: f32,    // 疲労 (0.0-1.0) - 高いほど疲れている
    pub stress: f32,     // ストレス (0.0-1.0) - 使い魔監視下で増加
    pub dream: f32,      // 夢の貯蔵量 (0.0-100.0) - 労働中に蓄積、睡眠/休憩で放出
}

impl Default for DamnedSoul {
    fn default() -> Self {
        Self {
            laziness: 0.7,   // デフォルトで怠惰
            motivation: 0.1, // デフォルトでやる気なし
            fatigue: 0.0,
            stress: 0.0, // デフォルトでストレスなし
            dream: 0.0,
        }
    }
}

/// ソウルに紐づくUI参照
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct SoulUiLinks {
    pub bar_entity: Option<Entity>,
    pub icon_entity: Option<Entity>,
}

/// ストレスによるブレイクダウン状態
/// stress >= 1.0 で付与され、stress <= 0.7 で削除される
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct StressBreakdown {
    /// 停止中（初期 1 秒）- 動けない
    pub is_frozen: bool,
    /// 1 秒間だけ停止してから解除される残り時間
    pub remaining_freeze_secs: f32,
}

/// 休憩所退出後のリクルート不可クールダウン
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct RestAreaCooldown {
    pub remaining_secs: f32,
}

/// 怠惰状態のコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct IdleState {
    pub idle_timer: f32,
    pub total_idle_time: f32, // 累計の放置時間
    pub behavior: IdleBehavior,
    pub behavior_duration: f32, // 現在の行動をどれくらい続けるか
    // 集会中のサブ行動
    pub gathering_behavior: GatheringBehavior,
    pub gathering_behavior_timer: f32,
    pub gathering_behavior_duration: f32,
    // 重なり回避が必要かどうか（初回到着時・パターン変更時に true）
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
    Wandering, // うろうろ
    Sitting,            // 座り込み
    Sleeping,           // 寝ている
    Gathering,          // 集会中
    ExhaustedGathering, // 疲労による集会移動中
    Resting,            // 休憩所で休息中
    GoingToRest,        // 休憩所へ移動中
    Escaping,           // 使い魔から逃走中
    Drifting,           // 未管理のまま漂流中（自然脱走）
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

/// 集会中のサブ行動
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Default)]
pub enum GatheringBehavior {
    #[default]
    Wandering, // うろうろ（今の動き）
    Sleeping, // 寝ている
    Standing, // 立ち尽くす
    Dancing,  // 踊り（揺れ）
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

/// アニメーション状態
#[derive(Component)]
pub struct AnimationState {
    pub is_moving: bool,
    pub facing_right: bool,
    pub bob_timer: f32,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            is_moving: false,
            facing_right: true,
            bob_timer: 0.0,
        }
    }
}

/// 会話イベント起点の一時的な表情オーバーレイ
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationExpressionKind {
    Positive,
    Negative,
    Exhausted,
    GatheringWine,
    GatheringTrump,
}

/// 会話表情の残り表示時間（秒）
#[derive(Component, Debug, Clone, Copy)]
pub struct ConversationExpression {
    pub kind: ConversationExpressionKind,
    /// 表情イベントの優先度（高いほど上書き可能）
    pub priority: u8,
    pub remaining_secs: f32,
}

pub struct DamnedSoulPlugin;

use crate::systems::GameSystemSet;

pub use spawn::spawn_damned_souls;

impl Plugin for DamnedSoulPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DamnedSoul>()
            .register_type::<SoulUiLinks>()
            .register_type::<IdleState>()
            .register_type::<StressBreakdown>()
            .register_type::<RestAreaCooldown>()
            .register_type::<DriftingState>()
            .register_type::<DreamState>()
            .register_type::<DreamPool>()
            .init_resource::<DreamPool>()
            .init_resource::<spawn::PopulationManager>()
            .add_systems(
                Update,
                (
                    spawn::population_tracking_system.in_set(GameSystemSet::Logic),
                    spawn::periodic_spawn_system
                        .in_set(GameSystemSet::Logic)
                        .after(spawn::population_tracking_system)
                        .before(spawn::soul_spawning_system),
                    spawn::soul_spawning_system.in_set(GameSystemSet::Logic),
                    movement::soul_stuck_escape_system
                        .in_set(GameSystemSet::Actor)
                        .before(movement::pathfinding_system),
                    movement::pathfinding_system.in_set(GameSystemSet::Actor),
                    movement::soul_movement.in_set(GameSystemSet::Actor),
                    movement::apply_conversation_expression_event_system
                        .in_set(GameSystemSet::Visual)
                        .after(
                            crate::systems::visual::speech::conversation::systems::process_conversation_logic,
                        ),
                    movement::update_conversation_expression_timer_system
                        .in_set(GameSystemSet::Visual),
                    movement::animation_system
                        .in_set(GameSystemSet::Visual)
                        .after(movement::apply_conversation_expression_event_system)
                        .after(movement::update_conversation_expression_timer_system),
                ),
            )
            .add_observer(observers::on_task_assigned)
            .add_observer(observers::on_task_completed)
            .add_observer(observers::on_soul_recruited)
            .add_observer(observers::on_stress_breakdown)
            .add_observer(observers::on_exhausted);
    }
}
