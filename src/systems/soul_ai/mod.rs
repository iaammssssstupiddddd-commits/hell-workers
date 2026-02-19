//! 魂（ワーカー）AI モジュール
//!
//! 魂のバイタル管理、タスク実行、仕事管理、アイドル行動を一括して管理します。

use bevy::prelude::*;

pub mod decide;
pub mod execute;
pub mod helpers;
pub mod perceive;
pub mod scheduling;
pub mod update;
pub mod visual;

use crate::systems::GameSystemSet;
use scheduling::{FamiliarAiSystemSet, SoulAiSystemSet};

pub struct SoulAiPlugin;

impl Plugin for SoulAiPlugin {
    fn build(&self, app: &mut App) {
        // Soul AI は Familiar AI の後に実行される
        // FamiliarAiSystemSet::Execute → ApplyDeferred → SoulAiSystemSet::Perceive
        app.configure_sets(
            Update,
            (
                SoulAiSystemSet::Perceive,
                SoulAiSystemSet::Update,
                SoulAiSystemSet::Decide,
                SoulAiSystemSet::Execute,
            )
                .chain()
                .after(FamiliarAiSystemSet::Execute)
                .in_set(GameSystemSet::Logic),
        )
        .register_type::<execute::task_execution::AssignedTask>()
        .register_type::<helpers::gathering::GatheringSpot>()
        .init_resource::<helpers::gathering::GatheringUpdateTimer>()
        .init_resource::<perceive::escaping::EscapeDetectionTimer>()
        .init_resource::<perceive::escaping::EscapeBehaviorTimer>()
        .init_resource::<decide::drifting::DriftingDecisionTimer>()
        .add_systems(
            Update,
            (
                // === Familiar → Soul 間の同期 ===
                // Familiar AI の決定を Soul AI に反映
                bevy::ecs::schedule::ApplyDeferred
                    .after(FamiliarAiSystemSet::Execute)
                    .before(SoulAiSystemSet::Perceive),
                // Perceive → Update 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Perceive)
                    .before(SoulAiSystemSet::Update),
                // === Update Phase ===
                // 時間経過による内部状態の変化
                (
                    // タイマー更新
                    helpers::gathering::tick_gathering_timer_system,
                    update::gathering_tick::gathering_grace_tick_system,
                    // バイタル更新
                    update::vitals_update::fatigue_update_system,
                    update::vitals_update::fatigue_penalty_system,
                    update::vitals_influence::familiar_influence_unified_system,
                    update::rest_area_update::rest_area_update_system,
                    update::state_sanity::ensure_rest_area_component_system,
                    update::state_sanity::clear_stale_working_on_system,
                    update::state_sanity::reconcile_rest_state_system,
                    // Dream蓄積
                    update::dream_update::dream_update_system,
                )
                    .in_set(SoulAiSystemSet::Update),
                // Update → Decide 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Update)
                    .before(SoulAiSystemSet::Decide),
                // === Decide Phase ===
                // 次の行動の選択、要求の生成
                (
                    // タスク割り当て要求
                    decide::work::auto_refine::mud_mixer_auto_refine_system,
                    decide::work::auto_build::blueprint_auto_build_system,
                    // アイドル行動の決定（先に実行）
                    decide::idle_behavior::idle_behavior_decision_system,
                    // 重なり回避（idle_behaviorの後に実行して上書きを防ぐ）
                    decide::separation::gathering_separation_system
                        .after(decide::idle_behavior::idle_behavior_decision_system),
                    decide::escaping::escaping_decision_system
                        .after(decide::idle_behavior::idle_behavior_decision_system),
                    decide::drifting::drifting_decision_system
                        .after(decide::escaping::escaping_decision_system),
                    // 集会管理の決定
                    decide::gathering_mgmt::gathering_maintenance_decision,
                    decide::gathering_mgmt::gathering_merge_decision,
                    decide::gathering_mgmt::gathering_recruitment_decision,
                    decide::gathering_mgmt::gathering_leave_decision,
                )
                    .in_set(SoulAiSystemSet::Decide),
                // Decide → Execute 間の同期
                bevy::ecs::schedule::ApplyDeferred
                    .after(SoulAiSystemSet::Decide)
                    .before(SoulAiSystemSet::Execute),
                // === Execute Phase ===
                // 決定された行動の実行
                (
                    // Designation要求の適用
                    execute::designation_apply::apply_designation_requests_system,
                    // タスク要求の適用
                    execute::task_execution::apply_task_assignment_requests_system
                        .before(execute::task_execution::task_execution_system),
                    execute::drifting::drifting_behavior_system
                        .after(execute::task_execution::apply_task_assignment_requests_system)
                        .before(execute::task_execution::task_execution_system),
                    execute::task_execution::task_execution_system,
                    // アイドル行動の適用
                    execute::idle_behavior_apply::idle_behavior_apply_system,
                    execute::escaping_apply::escaping_apply_system,
                    execute::drifting::despawn_at_edge_system
                        .after(execute::drifting::drifting_behavior_system),
                    execute::gathering_apply::gathering_apply_system,
                    // クリーンアップ
                    execute::cleanup::cleanup_commanded_souls_system,
                    // タスク要求の適用
                    crate::systems::familiar_ai::perceive::resource_sync::apply_reservation_requests_system,
                    // エンティティ生成
                    execute::gathering_spawn::gathering_spawn_system,
                )
                    .in_set(SoulAiSystemSet::Execute),
            ),
        )
        .add_observer(update::vitals::on_task_completed_motivation_bonus)
        .add_observer(update::vitals::on_encouraged_effect)
        .add_observer(update::vitals::on_soul_recruited_effect);
    }
}
