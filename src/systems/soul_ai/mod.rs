//! 魂（ワーカー）AI モジュール
//!
//! 魂のバイタル管理、タスク実行、仕事管理、アイドル行動を一括して管理します。

use bevy::prelude::*;

pub mod gathering;
pub mod idle;
pub mod task_execution; // タスク実行モジュール
pub mod vitals;
pub mod work;

use crate::systems::GameSystemSet;

pub struct SoulAiPlugin;

impl Plugin for SoulAiPlugin {
    fn build(&self, app: &mut App) {
        app            .register_type::<task_execution::AssignedTask>()
            .register_type::<gathering::GatheringSpot>()
            .init_resource::<work::AutoHaulCounter>()
            .init_resource::<work::auto_haul::MixerWaterBucketReservations>()
            .init_resource::<gathering::GatheringUpdateTimer>()
            .init_resource::<idle::escaping::EscapeDetectionTimer>()
            .add_systems(
                Update,
                (
                    (
                        // タイマー更新 (集会システムの間引き用)
                        gathering::tick_gathering_timer_system,
                        // バイタル更新
                        vitals::update::fatigue_update_system,
                        vitals::update::fatigue_penalty_system,
                        vitals::update::stress_system,
                        vitals::influence::supervision_stress_system,
                        vitals::influence::motivation_system,
                    ),
                    (
                        // タスク実行
                        task_execution::task_execution_system,
                        // 仕事管理
                        work::cleanup::cleanup_commanded_souls_system,
                        work::auto_haul::blueprint_auto_haul_system,
                        work::tank_water_request_system,
                        work::auto_haul::mud_mixer_auto_haul_system,
                        work::auto_haul::bucket_auto_haul_system,
                        work::auto_refine::mud_mixer_auto_refine_system,
                        work::auto_build::blueprint_auto_build_system
                            .after(crate::systems::familiar_ai::familiar_ai_system),
                        work::task_area_auto_haul_system,
                    ),
                    (
                        // 動的集会システム
                        gathering::spawn::gathering_spawn_system,
                        gathering::maintenance::gathering_recruitment_system,
                        gathering::maintenance::gathering_leave_system,
                        gathering::maintenance::gathering_maintenance_system,
                        gathering::maintenance::gathering_merge_system,
                        gathering::visual::gathering_visual_update_system,
                    ),
                    (
                        // アイドル行動
                        idle::behavior::idle_behavior_system,
                        idle::visual::idle_visual_system,
                        idle::separation::gathering_separation_system,
                        // 逃走システム
                        idle::escaping::escaping_detection_system,
                        idle::escaping::escaping_behavior_system,
                    ),
                    // ビジュアル
                    vitals::visual::familiar_hover_visualization_system,
                )
                    .chain()
                    .in_set(GameSystemSet::Logic),
            )
            // デバッグシステム (順序非依存)
            .add_systems(
                Update,
                gathering::visual::gathering_debug_visualization_system
                    .in_set(GameSystemSet::Logic),
            )
            .add_observer(vitals::on_task_completed_motivation_bonus)
            .add_observer(vitals::on_encouraged_effect)
            .add_observer(vitals::on_soul_recruited_effect)
            .add_observer(gathering::on_participating_added)
            .add_observer(gathering::on_participating_removed);
    }
}
