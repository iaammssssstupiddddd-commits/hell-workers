use bevy::prelude::*;

pub mod movement;
pub mod names;
pub mod observers;
pub mod spawn;

use names::{FEMALE_NAMES, MALE_NAMES};
use rand::Rng;

// コアコンポーネントは hw_core::soul から再エクスポート
pub use hw_core::soul::{
    DamnedSoul, Destination, DreamPool, DreamQuality, DreamState, DriftEdge, DriftPhase,
    DriftingState, GatheringBehavior, IdleBehavior, IdleState, Path, RestAreaCooldown,
    StressBreakdown,
};

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

/// ソウルに紐づくUI参照
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct SoulUiLinks {
    pub bar_entity: Option<Entity>,
    pub icon_entity: Option<Entity>,
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
