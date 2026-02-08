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

// 既存参照の互換レイヤー（M1移行中）
pub mod query_types {
    pub use super::helpers::query_types::*;
}

pub mod gathering {
    pub use super::helpers::gathering::*;

    pub mod spawn {
        pub use super::super::execute::gathering_spawn::*;
    }

    pub mod visual {
        pub use super::super::visual::gathering::*;
    }
}

pub mod idle {
    pub mod behavior {
        pub use super::super::decide::idle_behavior::*;
    }

    pub mod escaping {
        pub use super::super::perceive::escaping::*;
    }

    pub mod separation {
        pub use super::super::decide::separation::*;
    }

    pub mod visual {
        pub use super::super::visual::idle::*;
    }
}

pub mod vitals {
    pub use super::update::vitals::*;

    pub mod influence {
        pub use super::super::update::vitals_influence::*;
    }

    pub mod update {
        pub use super::super::update::vitals_update::*;
    }

    pub mod visual {
        pub use super::super::visual::vitals::*;
    }
}

pub mod work {
    pub use super::decide::work::*;

    pub mod cleanup {
        pub use super::super::execute::cleanup::*;
    }

    pub mod helpers {
        pub use super::super::helpers::work::*;
    }
}

pub mod task_execution {
    pub use super::execute::task_execution::*;
}

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
        .init_resource::<idle::escaping::EscapeBehaviorTimer>()
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
                    gathering::tick_gathering_timer_system,
                    update::gathering_tick::gathering_grace_tick_system,
                    // バイタル更新
                    vitals::update::fatigue_update_system,
                    vitals::update::fatigue_penalty_system,
                    vitals::influence::familiar_influence_unified_system,
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
                    decide::escaping::escaping_decision_system,
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
                    // タスク要求の適用
                    task_execution::apply_task_assignment_requests_system
                        .before(task_execution::task_execution_system),
                    task_execution::task_execution_system,
                    // アイドル行動の適用
                    idle::behavior::idle_behavior_apply_system,
                    execute::escaping_apply::escaping_apply_system,
                    execute::gathering_apply::gathering_apply_system,
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
