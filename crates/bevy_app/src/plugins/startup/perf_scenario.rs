//! 再現可能なパフォーマンス計測シナリオの構成と採取。

#[cfg(feature = "profiling")]
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
#[cfg(feature = "profiling")]
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarPolicy,
};
#[cfg(feature = "profiling")]
use crate::interface::ui::panels::task_list::{
    TaskDashboardPerfMetrics, TaskDashboardTimingMetrics,
};
#[cfg(feature = "profiling")]
use crate::systems::command::TaskArea;
#[cfg(feature = "profiling")]
use crate::systems::energy::grid_recalc::EnergyPerfMetrics;
#[cfg(feature = "profiling")]
use crate::systems::familiar_ai::FamiliarAiState;
#[cfg(feature = "profiling")]
use crate::systems::familiar_ai::perceive::resource_sync::ReservationSyncPerfMetrics;
#[cfg(feature = "profiling")]
use crate::systems::jobs::{
    Blueprint, BuildingType, ConstructionPerfMetrics, Designation, Door, DoorState, Priority, Rock,
    TargetBlueprint, TaskSlots, Tree, WorkType,
};
#[cfg(feature = "profiling")]
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
#[cfg(feature = "profiling")]
use crate::world::map::{WorldMap, WorldMapWrite};
use crate::{Render3dVisible, RenderPerfToggles};
#[cfg(feature = "profiling")]
use bevy::camera::visibility::RenderLayers;
#[cfg(feature = "profiling")]
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
#[cfg(feature = "profiling")]
use bevy::time::{Fixed, Real};
#[cfg(feature = "profiling")]
use bevy::window::PrimaryWindow;
#[cfg(feature = "profiling")]
use hw_core::constants::{LAYER_2D, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Z_MAP};
#[cfg(feature = "profiling")]
use hw_core::quality::QualitySettings;
use hw_core::quality::RttQualityPreset;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::SimulationRandomState;
#[cfg(feature = "profiling")]
use hw_core::visual_mirror::construction::BlueprintVisualState;
#[cfg(feature = "profiling")]
use hw_familiar_ai::familiar_ai::decide::resources::FamiliarDelegationPerfMetrics;
#[cfg(feature = "profiling")]
use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
};
#[cfg(feature = "profiling")]
use hw_jobs::{GatherPhase, GeneratePowerPhase, HaulPhase};
#[cfg(feature = "profiling")]
use hw_logistics::transport_request::WheelbarrowArbitrationPerfMetrics;
#[cfg(feature = "profiling")]
use hw_soul_ai::soul_ai::execute::task_execution::TaskExecutionPerfMetrics;
#[cfg(feature = "profiling")]
use hw_soul_ai::soul_ai::pathfinding::RuntimePathDeferMetrics;
#[cfg(feature = "profiling")]
use hw_soul_ai::soul_ai::update::slow_simulation::SlowSimulationPerfMetrics;
#[cfg(feature = "profiling")]
use hw_spatial::DoorPerfMetrics;
#[cfg(feature = "profiling")]
use hw_visual::visual3d::{
    Building3dVisual, FamiliarProxy3d, SoulMaskProxy3d, SoulProxy3d, SoulShadowProxy3d,
};
#[cfg(feature = "profiling")]
use hw_world::{DoorVisualHandles, RuntimePathSearchBudget, RuntimePathSearchMetrics};
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::env;
use std::fmt;
use std::path::PathBuf;

#[cfg(feature = "profiling")]
use super::perf_render_environment::{
    PerfRenderEnvironment, PerfRenderEnvironmentEvidence, PerfRenderEnvironmentState,
};
#[cfg(feature = "profiling")]
use super::rtt_setup::{Camera3dRtt, Camera3dSoulMaskRtt, RttRuntime};

#[cfg(feature = "profiling")]
mod audit_checksum;
#[cfg(feature = "profiling")]
mod audit_encoding;
#[cfg(feature = "profiling")]
mod behavior_driver;
#[cfg(feature = "profiling")]
mod capture_driver;
mod config;
#[cfg(feature = "profiling")]
#[cfg(feature = "profiling")]
mod fixture;
#[cfg(feature = "profiling")]
mod indoor_light_fixture;
#[cfg(feature = "profiling")]
mod output;
#[cfg(feature = "profiling")]
mod renderdoc_capture;
#[cfg(feature = "profiling")]
mod workload_driver;

#[cfg(feature = "profiling")]
pub(crate) use behavior_driver::{
    PerfBehaviorCapture, count_perf_behavior_fixed_tick_system, drive_perf_behavior_system,
    observe_perf_behavior_system,
};
#[cfg(feature = "profiling")]
pub(crate) use capture_driver::{drive_perf_capture_system, start_perf_capture_system};
#[cfg(feature = "profiling")]
pub(crate) use config::PerfDashboardMode;
pub use config::{
    PerfFamiliarPolicyMode, PerfOperationDialogMode, PerfRenderMode, PerfScenarioConfig,
    PerfScenarioRandomStreams, PerfScenarioSize, PerfWorkload,
};
#[cfg(feature = "profiling")]
pub(crate) use config::{
    is_fixed_step_behavior, is_fixed_step_scenario, is_not_fixed_step_audit,
    is_not_fixed_step_behavior, is_not_renderdoc_capture,
};
#[cfg(feature = "profiling")]
pub(crate) use fixture::{PerfScenarioApplied, PerfScenarioDriverState, PerfScenarioSet};
#[cfg(feature = "profiling")]
pub use fixture::{
    setup_perf_scenario_if_enabled, setup_perf_scenario_runtime_if_enabled,
    setup_perf_ui_mode_if_enabled,
};
#[cfg(feature = "profiling")]
pub(crate) use indoor_light_fixture::{
    IndoorLightFixtureState, assign_indoor_light_generator_system,
    prepare_indoor_light_soul_spa_system, seed_indoor_light_static_door_states_system,
    should_settle_indoor_light_fixture, stabilize_indoor_light_actors_system,
    validate_indoor_light_fixture_system,
};
#[cfg(feature = "profiling")]
pub(crate) use renderdoc_capture::{
    arm_renderdoc_checkpoint_system, install as install_renderdoc_capture,
    poll_renderdoc_capture_system,
};
#[cfg(feature = "profiling")]
pub(crate) use workload_driver::drive_perf_workload_system;

#[cfg(feature = "profiling")]
use audit_checksum::{
    calculate_checksum, calculate_render_inventory, calculate_scene_root_counts,
    checksum_from_audit_records, collect_audit_actor_records, latest_frame_time_ms,
};
#[cfg(feature = "profiling")]
use audit_encoding::*;
#[cfg(feature = "profiling")]
use config::{
    FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS, PERF_DETERMINISM_SCHEMA_VERSION,
    PERF_SUMMARY_SCHEMA_VERSION, PerfBehaviorCase,
};
#[cfg(feature = "profiling")]
use fixture::{PerfFixtureKind, PerfFixtureMarker};
#[cfg(feature = "profiling")]
use output::{
    PerfCaptureWriteInput, fnv1a, fnv1a_bytes, write_determinism_audit,
    write_indoor_light_fixture_sidecars, write_perf_capture, write_render_inventory,
    write_window_observation,
};

#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfCapture {
    phase: PerfCapturePhase,
    elapsed_secs: f32,
    frame_times_ms: Vec<f64>,
    fixture_wait_reported: bool,
    initial_checksum: Option<PerfScenarioChecksum>,
    initial_scene_roots: Option<PerfSceneRootCounts>,
    initial_render_inventory: Option<PerfRenderInventory>,
    initial_window: Option<PerfWindowObservation>,
    warmup_checksum: Option<PerfScenarioChecksum>,
    measure_end_checksum: Option<PerfScenarioChecksum>,
    warmup_virtual_secs: f64,
    warmup_real_secs: f64,
    measure_virtual_secs: f64,
    measure_real_secs: f64,
    fixed_update_tick: u64,
    determinism_checkpoints: Vec<PerfDeterminismCheckpoint>,
    determinism_actor_records: Vec<PerfDeterminismActorRecord>,
    #[cfg(feature = "profiling-memory")]
    memory_measurement: crate::profiling_allocator::MemoryMeasurement,
}

#[cfg(feature = "profiling")]
#[derive(Default)]
enum PerfCapturePhase {
    #[default]
    WaitingForScenario,
    ArmFixedAudit,
    Warmup,
    Measure,
    Flush,
    Finished,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy)]
struct PerfScenarioChecksum {
    souls: usize,
    familiars: usize,
    designations: usize,
    value: u64,
}

/// perf fixtureに生成された3D root markerの個数。
///
/// CPU条件ではSoul/Familiar用のscene rootが0、GPU条件ではfixture人口と一致
/// することをrunnerが検証する。建物rootは現時点では記録だけに留める。
#[cfg(feature = "profiling")]
#[derive(Clone, Copy)]
struct PerfSceneRootCounts {
    soul_proxy_3d: usize,
    soul_mask_proxy_3d: usize,
    soul_shadow_proxy_3d: usize,
    familiar_proxy_3d: usize,
    building_3d_visual: usize,
}

/// 実際にspawnされたcurrent rendererの構成を固定するprofiling-only inventory。
#[cfg(feature = "profiling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PerfRenderInventory {
    scene_target_count: usize,
    mask_target_count: usize,
    camera_3d_rtt_count: usize,
    camera_2d_count: usize,
    layer_2d_pass_count: usize,
    soul_proxy_3d: usize,
    soul_mask_proxy_3d: usize,
    soul_shadow_proxy_3d: usize,
    familiar_proxy_3d: usize,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Debug, PartialEq)]
struct PerfWindowObservation {
    window_present: bool,
    logical_width: Option<f32>,
    logical_height: Option<f32>,
    physical_width: Option<u32>,
    physical_height: Option<u32>,
    scale_factor: Option<f32>,
    rtt_quality: &'static str,
    scene_target_width: u32,
    scene_target_height: u32,
    mask_target_width: u32,
    mask_target_height: u32,
    target_scale_factor: f32,
    resolved_window_backend: Option<&'static str>,
    adapter_name: Option<String>,
    adapter_backend: Option<&'static str>,
    requested_present_mode: Option<&'static str>,
    effective_present_mode: Option<&'static str>,
}

#[cfg(feature = "profiling")]
impl PerfWindowObservation {
    fn capture(
        window: Option<&Window>,
        runtime: &RttRuntime,
        quality: &QualitySettings,
        environment: Option<&PerfRenderEnvironment>,
    ) -> Self {
        Self {
            window_present: window.is_some(),
            logical_width: window.map(Window::width),
            logical_height: window.map(Window::height),
            physical_width: window.map(Window::physical_width),
            physical_height: window.map(Window::physical_height),
            scale_factor: window.map(Window::scale_factor),
            rtt_quality: quality.rtt.as_str(),
            scene_target_width: runtime.viewport.width,
            scene_target_height: runtime.viewport.height,
            mask_target_width: runtime.viewport.width,
            mask_target_height: runtime.viewport.height,
            target_scale_factor: runtime.target_scale_factor,
            resolved_window_backend: environment.map(|value| value.window_backend),
            adapter_name: environment.map(|value| value.adapter_name.clone()),
            adapter_backend: environment.map(|value| value.adapter_backend),
            requested_present_mode: environment.map(|value| value.requested_present_mode),
            effective_present_mode: environment.map(|value| value.effective_present_mode),
        }
    }
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy)]
struct PerfDeterminismCheckpoint {
    checkpoint: &'static str,
    update_tick: u64,
    fixed_timestep_ns: u128,
    virtual_delta_ns: u128,
    virtual_elapsed_ns: u128,
    fixed_delta_ns: u128,
    fixed_elapsed_ns: u128,
    fixed_overstep_ns: u128,
    virtual_paused: bool,
    virtual_relative_speed_bits: u64,
    virtual_effective_speed_bits: u64,
    structural_checksum: PerfScenarioChecksum,
    checksum: PerfScenarioChecksum,
    work: PerfWorkSnapshot,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy)]
struct PerfWorkSnapshot {
    delegation_cycles: u32,
    incoming_snapshot_builds: u32,
    familiars_processed: u32,
    candidate_membership_checks: u32,
    policy_disabled_rejections: u32,
    candidate_snapshot_attempts: u32,
    candidate_score_attempts: u32,
    worker_score_attempts: u32,
    source_selector_calls: u32,
    source_selector_cache_build_scanned_items: u32,
    source_selector_candidate_scanned_items: u32,
    source_selector_scanned_items: u32,
    reachable_with_cache_calls: u32,
    top_k_partition_runs: u32,
    top_k_retained_candidates: u32,
    top_k_fallback_candidates: u32,
    wheelbarrow_arbitration_rebuilds: u32,
    wheelbarrow_request_bucket_builds: u32,
    wheelbarrow_bucket_items_scanned: u32,
    wheelbarrow_candidates_after_top_k: u32,
    runtime_path_actor_new_core_searches: u64,
    runtime_path_actor_new_deferred: u64,
    runtime_path_actor_reuse_core_searches: u64,
    runtime_path_actor_reuse_deferred: u64,
    runtime_path_actor_rest_fallback_core_searches: u64,
    runtime_path_actor_rest_fallback_deferred: u64,
    runtime_path_escape_core_searches: u64,
    runtime_path_escape_deferred: u64,
    runtime_path_task_execution_core_searches: u64,
    runtime_path_task_execution_deferred: u64,
    runtime_path_bucket_transport_core_searches: u64,
    runtime_path_bucket_transport_deferred: u64,
    runtime_path_total_core_searches: u64,
    runtime_path_expanded_nodes: u64,
    runtime_path_max_expanded_nodes_per_search: u64,
    runtime_path_active_task_max_defer_frames: u64,
    runtime_path_idle_or_rest_max_defer_frames: u64,
    runtime_path_deferred_actor_retries: u64,
    dashboard_state_rebuilds: u32,
    dashboard_snapshot_rows_scanned: u32,
    dashboard_summary_rows_scanned: u32,
    dashboard_snapshot_changes: u32,
    dashboard_summary_changes: u32,
    dashboard_render_rebuilds: u32,
    dashboard_render_input_rows: u32,
    dashboard_render_visible_rows: u32,
    dashboard_render_group_headers: u32,
    dashboard_despawn_roots_requested: u32,
}

#[cfg(feature = "profiling")]
impl PerfWorkSnapshot {
    fn from_resources(
        metrics: &FamiliarDelegationPerfMetrics,
        arbitration: &WheelbarrowArbitrationPerfMetrics,
        runtime_path: &RuntimePathSearchMetrics,
        runtime_defer: &RuntimePathDeferMetrics,
        dashboard: &TaskDashboardPerfMetrics,
    ) -> Self {
        Self {
            delegation_cycles: metrics.delegation_cycles,
            incoming_snapshot_builds: metrics.incoming_snapshot_builds,
            familiars_processed: metrics.familiars_processed,
            candidate_membership_checks: metrics.candidate_membership_checks,
            policy_disabled_rejections: metrics.policy_disabled_rejections,
            candidate_snapshot_attempts: metrics.candidate_snapshot_attempts,
            candidate_score_attempts: metrics.candidate_score_attempts,
            worker_score_attempts: metrics.worker_score_attempts,
            source_selector_calls: metrics.source_selector_calls,
            source_selector_cache_build_scanned_items: metrics
                .source_selector_cache_build_scanned_items,
            source_selector_candidate_scanned_items: metrics
                .source_selector_candidate_scanned_items,
            source_selector_scanned_items: metrics.source_selector_scanned_items,
            reachable_with_cache_calls: metrics.reachable_with_cache_calls,
            top_k_partition_runs: metrics.top_k_partition_runs,
            top_k_retained_candidates: metrics.top_k_retained_candidates,
            top_k_fallback_candidates: metrics.top_k_fallback_candidates,
            wheelbarrow_arbitration_rebuilds: arbitration.rebuilds,
            wheelbarrow_request_bucket_builds: arbitration.request_bucket_builds,
            wheelbarrow_bucket_items_scanned: arbitration.bucket_items_scanned,
            wheelbarrow_candidates_after_top_k: arbitration.candidates_after_top_k,
            runtime_path_actor_new_core_searches: runtime_path.actor_new_core_searches,
            runtime_path_actor_new_deferred: runtime_path.actor_new_deferred,
            runtime_path_actor_reuse_core_searches: runtime_path.actor_reuse_core_searches,
            runtime_path_actor_reuse_deferred: runtime_path.actor_reuse_deferred,
            runtime_path_actor_rest_fallback_core_searches: runtime_path
                .actor_rest_fallback_core_searches,
            runtime_path_actor_rest_fallback_deferred: runtime_path.actor_rest_fallback_deferred,
            runtime_path_escape_core_searches: runtime_path.escape_core_searches,
            runtime_path_escape_deferred: runtime_path.escape_deferred,
            runtime_path_task_execution_core_searches: runtime_path.task_execution_core_searches,
            runtime_path_task_execution_deferred: runtime_path.task_execution_deferred,
            runtime_path_bucket_transport_core_searches: runtime_path
                .bucket_transport_core_searches,
            runtime_path_bucket_transport_deferred: runtime_path.bucket_transport_deferred,
            runtime_path_total_core_searches: runtime_path.total_core_searches(),
            runtime_path_expanded_nodes: runtime_path.expanded_nodes,
            runtime_path_max_expanded_nodes_per_search: runtime_path.max_expanded_nodes_per_search,
            runtime_path_active_task_max_defer_frames: runtime_defer.active_task_max_defer_frames,
            runtime_path_idle_or_rest_max_defer_frames: runtime_defer.idle_or_rest_max_defer_frames,
            runtime_path_deferred_actor_retries: runtime_defer.deferred_actor_retries,
            dashboard_state_rebuilds: dashboard.state_rebuilds,
            dashboard_snapshot_rows_scanned: dashboard.snapshot_rows_scanned,
            dashboard_summary_rows_scanned: dashboard.summary_rows_scanned,
            dashboard_snapshot_changes: dashboard.snapshot_changes,
            dashboard_summary_changes: dashboard.summary_changes,
            dashboard_render_rebuilds: dashboard.render_rebuilds,
            dashboard_render_input_rows: dashboard.render_input_rows,
            dashboard_render_visible_rows: dashboard.render_visible_rows,
            dashboard_render_group_headers: dashboard.render_group_headers,
            dashboard_despawn_roots_requested: dashboard.despawn_roots_requested,
        }
    }
}

/// 固定 step auditでchecksumの差分をactor単位まで追跡するための記録。
///
/// frame-time captureには出力せず、fixtureの安定keyと監査対象の直列化recordだけを残す。
#[cfg(feature = "profiling")]
struct PerfDeterminismActorRecord {
    checkpoint: &'static str,
    update_tick: u64,
    actor_kind: &'static str,
    actor_key: u64,
    record: Vec<u8>,
}

#[cfg(feature = "profiling")]
struct PerfAuditActorRecord {
    actor_kind: &'static str,
    actor_key: u64,
    record: Vec<u8>,
}

#[cfg(feature = "profiling")]
type PerfAuditSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        &'static IdleState,
        &'static Destination,
        &'static Path,
        &'static AssignedTask,
        Option<&'static SimulationRandomState>,
    ),
    With<DamnedSoul>,
>;
#[cfg(feature = "profiling")]
type PerfAuditFamiliarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Familiar,
        &'static Destination,
        &'static Path,
        &'static ActiveCommand,
        &'static FamiliarOperation,
        &'static FamiliarPolicy,
        &'static FamiliarAiState,
        Option<&'static SimulationRandomState>,
    ),
    With<Familiar>,
>;
#[cfg(feature = "profiling")]
type PerfAuditDesignationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Designation,
        Option<&'static Priority>,
        Option<&'static TaskSlots>,
    ),
>;
#[cfg(feature = "profiling")]
type PerfAuditFixtureQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PerfFixtureMarker,
        &'static Transform,
        Option<&'static Door>,
        Option<&'static FloorConstructionSite>,
        Option<&'static FloorTileBlueprint>,
        Option<&'static Blueprint>,
    ),
>;

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub(crate) struct PerfChecksumQueries<'w, 's> {
    indoor_light: indoor_light_fixture::IndoorLightAuditQueries<'w, 's>,
    souls: Query<'w, 's, (Entity, &'static Transform), With<DamnedSoul>>,
    familiars: Query<'w, 's, (Entity, &'static Transform), With<Familiar>>,
    designations: Query<'w, 's, Entity, With<Designation>>,
    audit_souls: PerfAuditSoulQuery<'w, 's>,
    audit_familiars: PerfAuditFamiliarQuery<'w, 's>,
    audit_designations: PerfAuditDesignationQuery<'w, 's>,
    audit_fixtures: PerfAuditFixtureQuery<'w, 's>,
    target_transforms: Query<'w, 's, &'static Transform>,
    soul_proxy_3d: Query<'w, 's, (), With<SoulProxy3d>>,
    soul_mask_proxy_3d: Query<'w, 's, (), With<SoulMaskProxy3d>>,
    soul_shadow_proxy_3d: Query<'w, 's, (), With<SoulShadowProxy3d>>,
    familiar_proxy_3d: Query<'w, 's, (), With<FamiliarProxy3d>>,
    building_3d_visual: Query<'w, 's, (), With<Building3dVisual>>,
    scene_rtt_cameras: Query<'w, 's, (), With<Camera3dRtt>>,
    mask_rtt_cameras: Query<'w, 's, (), With<Camera3dSoulMaskRtt>>,
    cameras_2d: Query<'w, 's, (&'static Camera, Option<&'static RenderLayers>), With<Camera2d>>,
}

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub(crate) struct PerfCaptureStartParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    fixed_time: Res<'w, Time<Fixed>>,
    familiar_metrics: Res<'w, FamiliarDelegationPerfMetrics>,
    arbitration_metrics: Res<'w, WheelbarrowArbitrationPerfMetrics>,
    runtime_path_budget: Res<'w, RuntimePathSearchBudget>,
    runtime_path_defer_metrics: Res<'w, RuntimePathDeferMetrics>,
    dashboard_metrics: Res<'w, TaskDashboardPerfMetrics>,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    rtt_runtime: Res<'w, RttRuntime>,
    quality: Res<'w, QualitySettings>,
    render_environment: Res<'w, PerfRenderEnvironmentEvidence>,
}

#[cfg(feature = "profiling")]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PerfCaptureParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    indoor_light_fixture: Res<'w, IndoorLightFixtureState>,
    time: ResMut<'w, Time<Virtual>>,
    fixed_time: Res<'w, Time<Fixed>>,
    real_time: Res<'w, Time<Real>>,
    diagnostics: Option<Res<'w, bevy::diagnostic::DiagnosticsStore>>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    familiar_metrics: ResMut<'w, FamiliarDelegationPerfMetrics>,
    arbitration_metrics: ResMut<'w, WheelbarrowArbitrationPerfMetrics>,
    dashboard_metrics: ResMut<'w, TaskDashboardPerfMetrics>,
    dashboard_timing_metrics: ResMut<'w, TaskDashboardTimingMetrics>,
    task_execution_metrics: ResMut<'w, TaskExecutionPerfMetrics>,
    reservation_sync_metrics: ResMut<'w, ReservationSyncPerfMetrics>,
    door_metrics: ResMut<'w, DoorPerfMetrics>,
    construction_metrics: ResMut<'w, ConstructionPerfMetrics>,
    slow_simulation_metrics: ResMut<'w, SlowSimulationPerfMetrics>,
    energy_metrics: ResMut<'w, EnergyPerfMetrics>,
    runtime_path_budget: ResMut<'w, RuntimePathSearchBudget>,
    runtime_path_defer_metrics: ResMut<'w, RuntimePathDeferMetrics>,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    rtt_runtime: Res<'w, RttRuntime>,
    quality: Res<'w, QualitySettings>,
    render_environment: Res<'w, PerfRenderEnvironmentEvidence>,
}
