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

/// 地獄に堕ちた人間（怠惰な魂）
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct DamnedSoul {
    pub laziness: f32,   // 怠惰レベル (0.0-1.0) - 内部ステータス
    pub motivation: f32, // やる気 (0.0-1.0) - 高いほど働く
    pub fatigue: f32,    // 疲労 (0.0-1.0) - 高いほど疲れている
    pub stress: f32,     // ストレス (0.0-1.0) - 使い魔監視下で増加
}

impl Default for DamnedSoul {
    fn default() -> Self {
        Self {
            laziness: 0.7,   // デフォルトで怠惰
            motivation: 0.1, // デフォルトでやる気なし
            fatigue: 0.0,
            stress: 0.0, // デフォルトでストレスなし
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
    /// 停止中（stress > 0.9）- 動けない
    pub is_frozen: bool,
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
    Escaping,           // 使い魔から逃走中
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

pub struct DamnedSoulPlugin;

use crate::systems::GameSystemSet;

pub use spawn::spawn_damned_souls;

impl Plugin for DamnedSoulPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<DamnedSoul>()
            .register_type::<SoulUiLinks>()
            .register_type::<IdleState>()
            .register_type::<StressBreakdown>()
            .add_systems(
                Update,
                (
                    spawn::soul_spawning_system.in_set(GameSystemSet::Logic),
                    movement::soul_stuck_escape_system
                        .in_set(GameSystemSet::Actor)
                        .before(movement::pathfinding_system),
                    movement::pathfinding_system.in_set(GameSystemSet::Actor),
                    movement::soul_movement.in_set(GameSystemSet::Actor),
                    movement::animation_system.in_set(GameSystemSet::Visual),
                ),
            )
            .add_observer(observers::on_task_assigned)
            .add_observer(observers::on_task_completed)
            .add_observer(observers::on_soul_recruited)
            .add_observer(observers::on_stress_breakdown)
            .add_observer(observers::on_exhausted);
    }
}
