//! 魂（ワーカー）AI モジュール
//!
//! 魂のバイタル管理、タスク実行、仕事管理、アイドル行動を一括して管理します。

use bevy::prelude::*;

pub mod gathering;
pub mod idle;
pub mod query_types;
pub mod task_execution; // タスク実行モジュール
pub mod vitals;
pub mod work;
pub mod scheduling;

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
        .register_type::<task_execution::AssignedTask>()
            .register_type::<gathering::GatheringSpot>()
            .init_resource::<work::AutoHaulCounter>()
            .init_resource::<work::auto_haul::ItemReservations>()
            .init_resource::<gathering::GatheringUpdateTimer>()
            .init_resource::<idle::escaping::EscapeDetectionTimer>()
            .add_systems(
                Update,
                (
                    // === Familiar → Soul 間の同期 ===
                    // Familiar AI の決定を Soul AI に反映
                    bevy::ecs::schedule::ApplyDeferred
                        .after(FamiliarAiSystemSet::Execute)
                        .before(SoulAiSystemSet::Perceive),

                    // === Perceive Phase ===
                    // 環境情報の読み取り、変化の検出
                    (
                        idle::escaping::escaping_detection_system,
                    )
                        .in_set(SoulAiSystemSet::Perceive),

                    // Perceive → Update 間の同期
                    bevy::ecs::schedule::ApplyDeferred
                        .after(SoulAiSystemSet::Perceive)
                        .before(SoulAiSystemSet::Update),

                    // === Update Phase ===
                    // 時間経過による内部状態の変化
                    (
                        // タイマー更新
                        gathering::tick_gathering_timer_system,
                        // バイタル更新
                        vitals::update::fatigue_update_system,
                        vitals::update::fatigue_penalty_system,
                        vitals::update::stress_system,
                        vitals::influence::supervision_stress_system,
                        vitals::influence::motivation_system,
                        // メンテナンス処理
                        gathering::maintenance::gathering_recruitment_system,
                        gathering::maintenance::gathering_leave_system,
                        gathering::maintenance::gathering_maintenance_system,
                        gathering::maintenance::gathering_merge_system,
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
                        work::auto_haul::blueprint_auto_haul_system,
                        work::tank_water_request_system,
                        work::auto_haul::mud_mixer_auto_haul_system,
                        work::auto_haul::bucket_auto_haul_system,
                        work::auto_refine::mud_mixer_auto_refine_system,
                        work::auto_build::blueprint_auto_build_system,
                        work::task_area_auto_haul_system,
                        // アイドル行動の決定
                        idle::behavior::idle_behavior_decision_system,
                        idle::separation::gathering_separation_system,
                        idle::escaping::escaping_behavior_system,
                    )
                        .in_set(SoulAiSystemSet::Decide),

                    // Decide → Execute 間の同期
                    bevy::ecs::schedule::ApplyDeferred
                        .after(SoulAiSystemSet::Decide)
                        .before(SoulAiSystemSet::Execute),

                    // === Execute Phase ===
                    // 決定された行動の実行
                    (
                        // タスク要求の適用
                        task_execution::apply_task_assignment_requests_system
                            .before(task_execution::task_execution_system),
                        task_execution::task_execution_system,
                        // アイドル行動の適用
                        idle::behavior::idle_behavior_apply_system,
                        // クリーンアップ
                        work::cleanup::cleanup_commanded_souls_system,
                        work::auto_haul::clear_item_reservations_system,
                        // 予約の確定
                        crate::systems::familiar_ai::resource_cache::apply_reservation_requests_system
                            .after(work::auto_haul::clear_item_reservations_system),
                        // エンティティ生成
                        gathering::spawn::gathering_spawn_system,
                    )
                        .in_set(SoulAiSystemSet::Execute),
                ),
            )
            .add_observer(vitals::on_task_completed_motivation_bonus)
            .add_observer(vitals::on_encouraged_effect)
            .add_observer(vitals::on_soul_recruited_effect)
            .add_observer(gathering::on_participating_added)
            .add_observer(gathering::on_participating_removed);
    }
}
