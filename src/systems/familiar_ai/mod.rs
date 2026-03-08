use hw_core::constants::FAMILIAR_TASK_DELEGATION_INTERVAL;
use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::FamiliarAiSystemSet;
use bevy::prelude::*;
use hw_spatial::{DesignationSpatialGrid, TransportRequestSpatialGrid};

pub mod decide;
pub mod execute;
pub mod helpers;
pub mod perceive;
pub mod update;

pub use hw_core::familiar::FamiliarAiState;
pub use helpers::query_types::FamiliarSoulQuery;

pub struct FamiliarAiPlugin;

impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        // hw_ai の FamiliarAiCorePlugin でコアシステムを登録
        app.add_plugins(hw_ai::FamiliarAiCorePlugin);

        app.configure_sets(
            Update,
            (
                FamiliarAiSystemSet::Perceive,
                FamiliarAiSystemSet::Update,
                FamiliarAiSystemSet::Decide,
                FamiliarAiSystemSet::Execute,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .register_type::<FamiliarAiState>()
        .register_type::<decide::encouragement::EncouragementCooldown>()
        .init_resource::<perceive::resource_sync::SharedResourceCache>()
        .init_resource::<perceive::resource_sync::ReservationSyncTimer>()
        .init_resource::<DesignationSpatialGrid>()
        .init_resource::<TransportRequestSpatialGrid>()
        .init_resource::<FamiliarTaskDelegationTimer>()
        .init_resource::<decide::auto_gather_for_blueprint::BlueprintAutoGatherTimer>()
        .init_resource::<decide::task_delegation::ReachabilityFrameCache>()
        .init_resource::<FamiliarDelegationPerfMetrics>()
        .add_systems(
            Update,
            (
                // === Perceive Phase ===
                (
                    perceive::resource_sync::sync_reservations_system,
                )
                    .in_set(FamiliarAiSystemSet::Perceive),
                ApplyDeferred
                    .after(FamiliarAiSystemSet::Perceive)
                    .before(FamiliarAiSystemSet::Update),
                ApplyDeferred
                    .after(FamiliarAiSystemSet::Update)
                    .before(FamiliarAiSystemSet::Decide),
                // === Decide Phase ===
                ((
                    decide::state_decision::familiar_ai_state_system,
                    decide::auto_gather_for_blueprint::blueprint_auto_gather_system,
                    ApplyDeferred,
                    decide::task_delegation::familiar_task_delegation_system,
                    decide::encouragement::encouragement_decision_system,
                )
                    .chain(),)
                    .in_set(FamiliarAiSystemSet::Decide),
                // === Execute Phase ===
                (
                    execute::max_soul_apply::handle_max_soul_changed_system,
                    execute::idle_visual_apply::familiar_idle_visual_apply_system,
                    execute::squad_apply::apply_squad_management_requests_system,
                    execute::encouragement_apply::encouragement_apply_system,
                    execute::encouragement_apply::cleanup_encouragement_cooldowns_system,
                )
                    .in_set(FamiliarAiSystemSet::Execute),
            ),
        );
    }
}

#[derive(Resource)]
pub struct FamiliarTaskDelegationTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for FamiliarTaskDelegationTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(FAMILIAR_TASK_DELEGATION_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

/// Familiar task delegation の計測値（PERF-00）
#[derive(Resource, Debug)]
pub struct FamiliarDelegationPerfMetrics {
    /// 集計ログ出力までの経過秒
    pub log_interval_secs: f32,
    /// 直近フレームの委譲システム実行時間
    pub latest_elapsed_ms: f32,
    /// source_selector 呼び出し回数（期間集計）
    pub source_selector_calls: u32,
    /// source_selector のキャッシュ構築で走査したアイテム数（期間集計）
    pub source_selector_cache_build_scanned_items: u32,
    /// source_selector の候補探索で走査したアイテム数（期間集計）
    pub source_selector_candidate_scanned_items: u32,
    /// source_selector が走査したアイテム数（期間集計）
    pub source_selector_scanned_items: u32,
    /// reachable_with_cache 呼び出し回数（期間集計）
    pub reachable_with_cache_calls: u32,
    /// 委譲対象として処理した Familiar 数（期間集計）
    pub familiars_processed: u32,
}

impl Default for FamiliarDelegationPerfMetrics {
    fn default() -> Self {
        Self {
            log_interval_secs: 0.0,
            latest_elapsed_ms: 0.0,
            source_selector_calls: 0,
            source_selector_cache_build_scanned_items: 0,
            source_selector_candidate_scanned_items: 0,
            source_selector_scanned_items: 0,
            reachable_with_cache_calls: 0,
            familiars_processed: 0,
        }
    }
}
