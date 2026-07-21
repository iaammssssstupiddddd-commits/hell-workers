//! 再現可能なパフォーマンス計測シナリオの構成と採取。

#[cfg(feature = "profiling")]
use crate::entities::damned_soul::{
    DamnedSoul, Destination, GatheringBehavior, IdleBehavior, IdleState, Path,
};
#[cfg(feature = "profiling")]
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation};
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
    TaskSlots, Tree, WorkType,
};
#[cfg(feature = "profiling")]
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
#[cfg(feature = "profiling")]
use crate::world::map::{WorldMap, WorldMapWrite};
use crate::{Render3dVisible, RenderPerfToggles};
#[cfg(feature = "profiling")]
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
#[cfg(feature = "profiling")]
use bevy::time::{Fixed, Real};
#[cfg(feature = "profiling")]
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Z_MAP};
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::SimulationRandomState;
#[cfg(feature = "profiling")]
use hw_core::visual_mirror::construction::BlueprintVisualState;
#[cfg(feature = "profiling")]
use hw_familiar_ai::familiar_ai::decide::resources::FamiliarDelegationPerfMetrics;
#[cfg(feature = "profiling")]
use hw_jobs::GatherPhase;
#[cfg(feature = "profiling")]
use hw_jobs::construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
};
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
mod audit_checksum;
#[cfg(feature = "profiling")]
mod audit_encoding;
#[cfg(feature = "profiling")]
mod capture_driver;
mod config;
#[cfg(feature = "profiling")]
mod fixture;
#[cfg(feature = "profiling")]
mod output;
#[cfg(feature = "profiling")]
mod workload_driver;

#[cfg(feature = "profiling")]
pub(crate) use capture_driver::{drive_perf_capture_system, start_perf_capture_system};
pub use config::{
    PerfRenderMode, PerfScenarioConfig, PerfScenarioRandomStreams, PerfScenarioSize, PerfWorkload,
};
#[cfg(feature = "profiling")]
pub(crate) use config::{is_fixed_step_audit, is_not_fixed_step_audit};
#[cfg(feature = "profiling")]
pub(crate) use fixture::{PerfScenarioApplied, PerfScenarioDriverState, PerfScenarioSet};
#[cfg(feature = "profiling")]
pub use fixture::{setup_perf_scenario_if_enabled, setup_perf_scenario_runtime_if_enabled};
#[cfg(feature = "profiling")]
pub(crate) use workload_driver::drive_perf_workload_system;

#[cfg(feature = "profiling")]
use audit_checksum::{
    calculate_checksum, calculate_scene_root_counts, checksum_from_audit_records,
    collect_audit_actor_records, latest_frame_time_ms,
};
#[cfg(feature = "profiling")]
use audit_encoding::*;
#[cfg(feature = "profiling")]
use config::{FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS, PERF_SUMMARY_SCHEMA_VERSION};
#[cfg(feature = "profiling")]
use fixture::{PerfFixtureKind, PerfFixtureMarker};
#[cfg(feature = "profiling")]
use output::{
    PerfCaptureWriteInput, fnv1a, fnv1a_bytes, write_determinism_audit, write_perf_capture,
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
    warmup_checksum: Option<PerfScenarioChecksum>,
    measure_end_checksum: Option<PerfScenarioChecksum>,
    warmup_virtual_secs: f64,
    warmup_real_secs: f64,
    measure_virtual_secs: f64,
    measure_real_secs: f64,
    fixed_update_tick: u64,
    determinism_checkpoints: Vec<PerfDeterminismCheckpoint>,
    determinism_actor_records: Vec<PerfDeterminismActorRecord>,
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
    checksum: PerfScenarioChecksum,
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
}

#[cfg(feature = "profiling")]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PerfCaptureParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    time: ResMut<'w, Time<Virtual>>,
    fixed_time: Res<'w, Time<Fixed>>,
    real_time: Res<'w, Time<Real>>,
    diagnostics: Option<Res<'w, bevy::diagnostic::DiagnosticsStore>>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    familiar_metrics: ResMut<'w, FamiliarDelegationPerfMetrics>,
    task_execution_metrics: ResMut<'w, TaskExecutionPerfMetrics>,
    reservation_sync_metrics: ResMut<'w, ReservationSyncPerfMetrics>,
    door_metrics: ResMut<'w, DoorPerfMetrics>,
    construction_metrics: ResMut<'w, ConstructionPerfMetrics>,
    slow_simulation_metrics: ResMut<'w, SlowSimulationPerfMetrics>,
    energy_metrics: ResMut<'w, EnergyPerfMetrics>,
    runtime_path_budget: ResMut<'w, RuntimePathSearchBudget>,
    runtime_path_defer_metrics: ResMut<'w, RuntimePathDeferMetrics>,
}
