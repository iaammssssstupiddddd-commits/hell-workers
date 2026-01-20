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
        app.register_type::<task_execution::AssignedTask>()
            .register_type::<gathering::GatheringSpot>()
            .init_resource::<work::AutoHaulCounter>()
            .add_systems(
                Update,
                (
                    // バイタル更新
                    vitals::fatigue_update_system,
                    vitals::fatigue_penalty_system,
                    vitals::stress_system,
                    vitals::supervision_stress_system,
                    vitals::motivation_system,
                    // タスク実行
                    task_execution::task_execution_system,
                    // 仕事管理
                    work::cleanup_commanded_souls_system,
                    work::blueprint_auto_haul_system,
                    work::blueprint_auto_build_system
                        .after(crate::systems::familiar_ai::familiar_ai_system), // 資材が揃った建築タスクの自動割り当て（使い魔AIの後に実行）
                    work::task_area_auto_haul_system,
                    // 動動的集会システム
                    gathering::gathering_spawn_system,
                    gathering::gathering_recruitment_system, // 新規追加: 自動参加
                    gathering::gathering_leave_system,       // 新規追加: 距離による離脱
                    gathering::gathering_maintenance_system,
                    gathering::gathering_merge_system,
                    gathering::gathering_visual_update_system,
                    // アイドル行動
                    idle::idle_behavior_system,
                    idle::idle_visual_system,
                    idle::gathering_separation_system,
                    // ビジュアル
                    vitals::familiar_hover_visualization_system,
                )
                    .chain()
                    .in_set(GameSystemSet::Logic),
            )
            // デバッグシステム (順序非依存)
            .add_systems(
                Update,
                gathering::gathering_debug_visualization_system.in_set(GameSystemSet::Logic),
            )
            .add_observer(vitals::on_task_completed_motivation_bonus)
            .add_observer(vitals::on_encouraged_effect)
            .add_observer(vitals::on_soul_recruited_effect);
    }
}
