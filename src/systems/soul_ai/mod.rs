//! 魂（ワーカー）AI モジュール
//!
//! 魂のバイタル管理、タスク実行、仕事管理、アイドル行動を一括して管理します。

use bevy::prelude::*;

pub mod gathering;
pub mod idle;
pub mod task_execution; // タスク実行モジュール
pub mod vitals;
pub mod work;
pub mod scheduling;

use crate::systems::GameSystemSet;
use scheduling::SoulAiSystemSet;

pub struct SoulAiPlugin;

impl Plugin for SoulAiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                SoulAiSystemSet::Sense,
                SoulAiSystemSet::Think,
                SoulAiSystemSet::Act,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .register_type::<task_execution::AssignedTask>()
            .register_type::<gathering::GatheringSpot>()
            .init_resource::<work::AutoHaulCounter>()
            .init_resource::<work::auto_haul::ItemReservations>()
            .init_resource::<gathering::GatheringUpdateTimer>()
            .init_resource::<idle::escaping::EscapeDetectionTimer>()
            .add_systems(
                Update,
                (
                    // --- Sense Phase ---
                    (
                        // タイマー更新 (集会システムの間引き用)
                        gathering::tick_gathering_timer_system,
                        // バイタル更新
                        vitals::update::fatigue_update_system,
                        vitals::update::fatigue_penalty_system,
                        vitals::update::stress_system,
                        vitals::influence::supervision_stress_system,
                        vitals::influence::motivation_system,
                        // 各種メンテナンス
                        gathering::maintenance::gathering_recruitment_system,
                        gathering::maintenance::gathering_leave_system,
                        gathering::maintenance::gathering_maintenance_system,
                        gathering::maintenance::gathering_merge_system,
                        idle::escaping::escaping_detection_system,
                    )
                        .in_set(SoulAiSystemSet::Sense),
                    // --- Think Phase ---
                    (
                        // 意思決定・タスク割り当て
                        work::auto_haul::blueprint_auto_haul_system,
                        work::tank_water_request_system,
                        work::auto_haul::mud_mixer_auto_haul_system,
                        work::auto_haul::bucket_auto_haul_system,
                        work::auto_refine::mud_mixer_auto_refine_system,
                        work::auto_build::blueprint_auto_build_system
                            .after(crate::systems::familiar_ai::familiar_task_delegation_system),
                        work::task_area_auto_haul_system,
                        // アイドル・特殊行動
                        idle::behavior::idle_behavior_system,
                        idle::separation::gathering_separation_system,
                        idle::escaping::escaping_behavior_system,
                        gathering::spawn::gathering_spawn_system,
                    )
                        .in_set(SoulAiSystemSet::Think),
                    // コマンドの反映をフェーズ間で強制同期
                    bevy::ecs::schedule::ApplyDeferred
                        .after(SoulAiSystemSet::Think)
                        .before(SoulAiSystemSet::Act),
                    // --- Act Phase ---
                    (
                        // 物理的な行動・反映
                        task_execution::task_execution_system,
                        work::cleanup::cleanup_commanded_souls_system,
                        work::auto_haul::clear_item_reservations_system,
                    )
                        .in_set(SoulAiSystemSet::Act),
                ),
            )
            .add_observer(vitals::on_task_completed_motivation_bonus)
            .add_observer(vitals::on_encouraged_effect)
            .add_observer(vitals::on_soul_recruited_effect)
            .add_observer(gathering::on_participating_added)
            .add_observer(gathering::on_participating_removed);
    }
}
