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
    Blueprint, BuildingType, ConstructionPerfMetrics, Designation, Door, DoorPerfMetrics,
    DoorState, Priority, Rock, TaskSlots, Tree, WorkType,
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

const DEFAULT_WARMUP_SECS: f32 = 30.0;
const DEFAULT_MEASURE_SECS: f32 = 60.0;
#[cfg(feature = "profiling")]
const PERF_SUMMARY_SCHEMA_VERSION: u32 = 10;
const FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS: [u64; 4] = [1, 8, 32, 128];
const DEFAULT_FIXED_STEP_HZ: u32 = 64;
const DEFAULT_FIXED_WARMUP_TICKS: u64 = 1_920;
const DEFAULT_FIXED_AUDIT_TICKS: u64 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfWorkload {
    Gather,
    PathDoor,
    Construction,
    UiGpu,
}

impl PerfWorkload {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "gather" => Some(Self::Gather),
            "path-door" => Some(Self::PathDoor),
            "construction" => Some(Self::Construction),
            "ui-gpu" => Some(Self::UiGpu),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gather => "gather",
            Self::PathDoor => "path-door",
            Self::Construction => "construction",
            Self::UiGpu => "ui-gpu",
        }
    }

    #[cfg(feature = "profiling")]
    const fn has_automated_setup(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfScenarioSize {
    Small,
    Medium,
    Large,
}

impl PerfScenarioSize {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    const fn population(self) -> (u32, u32) {
        match self {
            Self::Small => (50, 4),
            Self::Medium => (200, 12),
            Self::Large => (500, 30),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfRenderMode {
    Cpu,
    Gpu,
}

impl PerfRenderMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "cpu" => Some(Self::Cpu),
            "gpu" => Some(Self::Gpu),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PerfClockMode {
    Realtime,
    Fixed,
}

impl PerfClockMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "realtime" => Some(Self::Realtime),
            "fixed" => Some(Self::Fixed),
            _ => None,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Fixed => "fixed",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PerfRandomStream {
    Souls,
    SoulTraits,
    Familiars,
    FamiliarVoices,
}

impl PerfRandomStream {
    const fn salt(self) -> u64 {
        match self {
            Self::Souls => 0xA2F4_0D7B_6C91_3E55,
            Self::SoulTraits => 0x9E4D_67B1_2A39_C5F0,
            Self::Familiars => 0x7B1D_53EA_C4F2_9860,
            Self::FamiliarVoices => 0xC4B8_19D2_6F30_EA57,
        }
    }
}

/// perf起動時だけ使用する、起動前に一度だけ解釈された計測条件。
#[derive(Resource, Debug, Clone)]
pub struct PerfScenarioConfig {
    enabled: bool,
    pub master_seed: u64,
    pub workload: PerfWorkload,
    pub size: PerfScenarioSize,
    pub soul_count: u32,
    pub familiar_count: u32,
    pub render_mode: PerfRenderMode,
    pub warmup_secs: f32,
    pub measure_secs: f32,
    pub output_dir: Option<PathBuf>,
    clock_mode: PerfClockMode,
    fixed_step_hz: u32,
    fixed_warmup_ticks: u64,
    fixed_audit_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfScenarioConfigError(String);

impl fmt::Display for PerfScenarioConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PerfScenarioConfigError {}

impl PerfScenarioConfig {
    pub fn try_from_process() -> Result<Self, PerfScenarioConfigError> {
        let args = env::args().collect::<Vec<_>>();
        let enabled = has_flag(&args, "--perf-scenario")
            || env::var("HW_PERF_SCENARIO").is_ok_and(|value| value == "1");

        if !enabled {
            return Ok(Self::default());
        }

        if !cfg!(feature = "profiling") {
            return Err(PerfScenarioConfigError(
                "--perf-scenario requires the profiling feature; rebuild with --features profiling"
                    .to_string(),
            ));
        }

        let workload = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-workload", "HW_PERF_WORKLOAD")?,
            "--perf-workload",
            "gather|path-door|construction|ui-gpu",
            PerfWorkload::parse,
            PerfWorkload::Gather,
        )?;
        let size = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-size", "HW_PERF_SIZE")?,
            "--perf-size",
            "small|medium|large",
            PerfScenarioSize::parse,
            PerfScenarioSize::Medium,
        )?;
        let render_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-render", "HW_PERF_RENDER")?,
            "--perf-render",
            "cpu|gpu",
            PerfRenderMode::parse,
            PerfRenderMode::Gpu,
        )?;
        let clock_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-clock", "HW_PERF_CLOCK")?,
            "--perf-clock",
            "realtime|fixed",
            PerfClockMode::parse,
            PerfClockMode::Realtime,
        )?;
        let fixed_step_hz = parse_u32_value_or_default(
            value_from_args_or_env(&args, "--perf-fixed-hz", "HW_PERF_FIXED_HZ")?,
            "--perf-fixed-hz",
            DEFAULT_FIXED_STEP_HZ,
        )?;
        let fixed_warmup_ticks = parse_u64_value_or_default(
            value_from_args_or_env(&args, "--perf-warmup-ticks", "HW_PERF_WARMUP_TICKS")?,
            "--perf-warmup-ticks",
            DEFAULT_FIXED_WARMUP_TICKS,
        )?;
        let fixed_audit_ticks = parse_u64_value_or_default(
            value_from_args_or_env(&args, "--perf-audit-ticks", "HW_PERF_AUDIT_TICKS")?,
            "--perf-audit-ticks",
            DEFAULT_FIXED_AUDIT_TICKS,
        )?;
        if matches!(clock_mode, PerfClockMode::Fixed) {
            if fixed_step_hz == 0 || fixed_warmup_ticks == 0 || fixed_audit_ticks == 0 {
                return Err(PerfScenarioConfigError(
                    "--perf-fixed-hz, --perf-warmup-ticks, and --perf-audit-ticks must be greater than 0 for --perf-clock fixed".to_string(),
                ));
            }
            if fixed_warmup_ticks <= FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[3] {
                return Err(PerfScenarioConfigError(format!(
                    "--perf-warmup-ticks must be greater than {} for --perf-clock fixed so required checkpoints remain distinct",
                    FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[3]
                )));
            }
            if fixed_warmup_ticks.checked_add(fixed_audit_ticks).is_none() {
                return Err(PerfScenarioConfigError(
                    "--perf-warmup-ticks + --perf-audit-ticks overflows u64".to_string(),
                ));
            }
        }
        let (default_souls, default_familiars) = size.population();
        let soul_count = parse_u32_value_or_default(
            value_from_args_or_env(&args, "--spawn-souls", "HW_SPAWN_SOULS")?,
            "--spawn-souls",
            default_souls,
        )?;
        let familiar_count = parse_u32_value_or_default(
            value_from_args_or_env(&args, "--spawn-familiars", "HW_SPAWN_FAMILIARS")?,
            "--spawn-familiars",
            default_familiars,
        )?;
        let master_seed = parse_u64_value_or_random(
            value_from_args_or_env(&args, "--perf-seed", "HW_PERF_SEED")?
                .or_else(|| env::var("HELL_WORKERS_WORLDGEN_SEED").ok()),
            "--perf-seed",
        )?;
        let warmup_secs = parse_duration_secs(
            value_from_args_or_env(&args, "--perf-warmup-secs", "HW_PERF_WARMUP_SECS")?,
            "--perf-warmup-secs",
            DEFAULT_WARMUP_SECS,
            true,
        )?;
        let measure_secs = parse_duration_secs(
            value_from_args_or_env(&args, "--perf-measure-secs", "HW_PERF_MEASURE_SECS")?,
            "--perf-measure-secs",
            DEFAULT_MEASURE_SECS,
            false,
        )?;
        let output_dir = value_from_args_or_env(&args, "--perf-output-dir", "HW_PERF_OUTPUT_DIR")?
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());

        Ok(Self {
            enabled,
            master_seed,
            workload,
            size,
            soul_count,
            familiar_count,
            render_mode,
            warmup_secs,
            measure_secs,
            output_dir,
            clock_mode,
            fixed_step_hz,
            fixed_warmup_ticks,
            fixed_audit_ticks,
        })
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn uses_fixed_timesteps(&self) -> bool {
        matches!(self.clock_mode, PerfClockMode::Fixed)
    }

    /// 自動 perf の CPU 条件では、計測対象外の 3D scene root を生成しない。
    ///
    /// これは起動時 fixture の生成だけに使う。通常プレイと、実行中の F8/F3
    /// 切替は既存どおり scene root を維持する。
    pub const fn omits_3d_scene_roots(&self) -> bool {
        self.enabled && matches!(self.render_mode, PerfRenderMode::Cpu)
    }

    pub const fn clock_mode_as_str(&self) -> &'static str {
        self.clock_mode.as_str()
    }

    pub const fn fixed_step_hz(&self) -> u32 {
        self.fixed_step_hz
    }

    pub const fn fixed_warmup_ticks(&self) -> u64 {
        self.fixed_warmup_ticks
    }

    pub const fn fixed_audit_ticks(&self) -> u64 {
        self.fixed_audit_ticks
    }

    #[cfg(feature = "profiling")]
    const fn fixed_audit_end_tick(&self) -> u64 {
        self.fixed_warmup_ticks + self.fixed_audit_ticks
    }

    pub fn initial_render_resources(&self) -> (Render3dVisible, RenderPerfToggles) {
        if !self.enabled {
            return (Render3dVisible::default(), RenderPerfToggles::default());
        }

        match self.render_mode {
            PerfRenderMode::Cpu => (Render3dVisible(false), RenderPerfToggles::all_disabled()),
            PerfRenderMode::Gpu => (Render3dVisible(true), RenderPerfToggles::gpu_baseline()),
        }
    }

    fn stream_seed(&self, stream: PerfRandomStream) -> u64 {
        splitmix64(self.master_seed ^ stream.salt())
    }
}

/// 固定 step 監査では、初期 fixture を通常の Logic ゲートより先に適用する。
///
/// 監査開始時は `Time<Virtual>` を停止したままにするため、通常の `Logic`
/// system set に置かれた spawn consumer は実行できない。この条件は、その
/// 専用経路と通常経路を相互排他的にするために使う。
#[cfg(feature = "profiling")]
pub(crate) fn is_fixed_step_audit(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| config.enabled() && config.uses_fixed_timesteps())
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_fixed_step_audit(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_fixed_step_audit(config)
}

impl Default for PerfScenarioConfig {
    fn default() -> Self {
        let (soul_count, familiar_count) = PerfScenarioSize::Medium.population();
        Self {
            enabled: false,
            master_seed: 0,
            workload: PerfWorkload::Gather,
            size: PerfScenarioSize::Medium,
            soul_count,
            familiar_count,
            render_mode: PerfRenderMode::Gpu,
            warmup_secs: DEFAULT_WARMUP_SECS,
            measure_secs: DEFAULT_MEASURE_SECS,
            output_dir: None,
            clock_mode: PerfClockMode::Realtime,
            fixed_step_hz: DEFAULT_FIXED_STEP_HZ,
            fixed_warmup_ticks: DEFAULT_FIXED_WARMUP_TICKS,
            fixed_audit_ticks: DEFAULT_FIXED_AUDIT_TICKS,
        }
    }
}

/// Soul/Familiar配置用の独立乱数列。非perf起動では参照しない。
#[derive(Resource)]
pub struct PerfScenarioRandomStreams {
    pub souls: StdRng,
    pub soul_traits: StdRng,
    pub familiars: StdRng,
    pub familiar_voices: StdRng,
}

impl FromWorld for PerfScenarioRandomStreams {
    fn from_world(world: &mut World) -> Self {
        let config = world.resource::<PerfScenarioConfig>();
        Self {
            souls: StdRng::seed_from_u64(config.stream_seed(PerfRandomStream::Souls)),
            soul_traits: StdRng::seed_from_u64(config.stream_seed(PerfRandomStream::SoulTraits)),
            familiars: StdRng::seed_from_u64(config.stream_seed(PerfRandomStream::Familiars)),
            familiar_voices: StdRng::seed_from_u64(
                config.stream_seed(PerfRandomStream::FamiliarVoices),
            ),
        }
    }
}

#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfScenarioApplied(pub(crate) bool);

/// Stable fixture identity used by fixed-step audit records. The marker avoids
/// treating allocator-dependent Entity IDs as part of the reproducibility
/// contract while still proving that the selected workload was installed.
#[cfg(feature = "profiling")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PerfFixtureMarker {
    kind: PerfFixtureKind,
    ordinal: u32,
}

#[cfg(feature = "profiling")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerfFixtureKind {
    Door,
    ConstructionSite,
    ConstructionTile,
    UiBlueprint,
}

#[cfg(feature = "profiling")]
impl PerfFixtureKind {
    const fn audit_tag(self) -> u8 {
        match self {
            Self::Door => 0,
            Self::ConstructionSite => 1,
            Self::ConstructionTile => 2,
            Self::UiBlueprint => 3,
        }
    }
}

/// Driver state intentionally holds no Entity IDs so it is world-epoch safe.
#[cfg(feature = "profiling")]
#[derive(Resource, Default)]
pub(crate) struct PerfScenarioDriverState {
    last_path_door_toggle_slot: Option<u64>,
}

#[cfg(feature = "profiling")]
type PerfSetupFamiliarQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut ActiveCommand,
        &'static mut FamiliarOperation,
    ),
>;
#[cfg(feature = "profiling")]
type PerfSetupSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static mut AssignedTask,
    ),
>;
#[cfg(feature = "profiling")]
type PerfTreeQuery<'w, 's> = Query<'w, 's, Entity, With<Tree>>;
#[cfg(feature = "profiling")]
type PerfRockQuery<'w, 's> = Query<'w, 's, Entity, With<Rock>>;

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub struct PerfWorkloadSetupParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    commands: Commands<'w, 's>,
    applied: ResMut<'w, PerfScenarioApplied>,
    q_familiars: PerfSetupFamiliarQuery<'w, 's>,
    q_souls: PerfSetupSoulQuery<'w, 's>,
    q_trees: PerfTreeQuery<'w, 's>,
    q_rocks: PerfRockQuery<'w, 's>,
    world_map: WorldMapWrite<'w>,
}

#[cfg(feature = "profiling")]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PerfScenarioSet {
    FixtureSpawn,
    FixtureApply,
    Setup,
    Apply,
    InitialCheckpoint,
    Driver,
    #[cfg(feature = "profiling")]
    Capture,
}

#[cfg(feature = "profiling")]
pub fn setup_perf_scenario_if_enabled(params: PerfWorkloadSetupParams) {
    setup_perf_workload_if_needed(params);
}

#[cfg(feature = "profiling")]
fn setup_perf_workload_if_needed(params: PerfWorkloadSetupParams) {
    let PerfWorkloadSetupParams {
        config,
        mut commands,
        mut applied,
        mut q_familiars,
        mut q_souls,
        q_trees,
        q_rocks,
        mut world_map,
    } = params;

    if applied.0 || !config.enabled() || q_familiars.is_empty() {
        return;
    }

    applied.0 = configure_perf_workload(
        &config,
        &mut commands,
        &mut q_familiars,
        &mut q_souls,
        &q_trees,
        &q_rocks,
        &mut world_map,
    );
}

#[cfg(feature = "profiling")]
pub fn setup_perf_scenario_runtime_if_enabled(params: PerfWorkloadSetupParams) {
    setup_perf_workload_if_needed(params);
}

#[cfg(feature = "profiling")]
fn configure_perf_workload(
    config: &PerfScenarioConfig,
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_souls: &mut Query<(
        Entity,
        &mut Transform,
        &mut Destination,
        &mut Path,
        &mut AssignedTask,
    )>,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
    world_map: &mut WorldMapWrite,
) -> bool {
    match config.workload {
        PerfWorkload::Gather => {
            configure_gather_baseline(commands, q_familiars, q_trees, q_rocks);
            true
        }
        PerfWorkload::PathDoor => {
            configure_path_door_fixture(commands, q_familiars, q_souls, world_map)
        }
        PerfWorkload::Construction => {
            configure_construction_fixture(commands, q_familiars, world_map, config.size)
        }
        PerfWorkload::UiGpu => {
            configure_ui_gpu_fixture(commands, q_familiars, world_map, config.size)
        }
    }
}

#[cfg(feature = "profiling")]
fn configure_gather_baseline(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: &Query<Entity, With<Tree>>,
    q_rocks: &Query<Entity, With<Rock>>,
) {
    let area = TaskArea::from_points(Vec2::new(-1600.0, -1600.0), Vec2::new(1600.0, 1600.0));

    for (fam_entity, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::GatherResources;
        operation.max_controlled_soul = 20;
        commands.entity(fam_entity).insert(area.clone());
    }

    for tree_entity in q_trees.iter() {
        commands.entity(tree_entity).insert((
            Designation {
                work_type: WorkType::Chop,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }

    for rock_entity in q_rocks.iter() {
        commands.entity(rock_entity).insert((
            Designation {
                work_type: WorkType::Mine,
            },
            TaskSlots::new(1),
            Priority(0),
        ));
    }
}

#[cfg(feature = "profiling")]
fn configure_path_door_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_souls: &mut Query<(
        Entity,
        &mut Transform,
        &mut Destination,
        &mut Path,
        &mut AssignedTask,
    )>,
    world_map: &mut WorldMapWrite,
) -> bool {
    let Some((left_grid, door_grid, right_grid)) = find_fixture_corridor(world_map.as_ref()) else {
        error!("PERF_CAPTURE: path-door fixture could not find a free three-tile corridor");
        return false;
    };

    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let mut soul_entities = q_souls
        .iter()
        .map(|(entity, _, _, _, _)| entity)
        .collect::<Vec<_>>();
    soul_entities.sort_unstable_by_key(|entity| entity.to_bits());
    for (ordinal, soul_entity) in soul_entities.into_iter().enumerate() {
        let Ok((_, mut transform, mut destination, mut path, mut task)) =
            q_souls.get_mut(soul_entity)
        else {
            continue;
        };
        let grid = if ordinal % 2 == 0 {
            left_grid
        } else {
            right_grid
        };
        let target = if ordinal % 2 == 0 {
            right_grid
        } else {
            left_grid
        };
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        transform.translation = position.extend(transform.translation.z);
        destination.0 = WorldMap::grid_to_world(target.0, target.1);
        path.waypoints.clear();
        path.current_index = 0;
        path.planned_destination = None;
        *task = AssignedTask::None;
    }

    let door_entity = commands
        .spawn((
            Door::default(),
            Sprite {
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(
                WorldMap::grid_to_world(door_grid.0, door_grid.1).extend(Z_MAP + 0.1),
            ),
            PerfFixtureMarker {
                kind: PerfFixtureKind::Door,
                ordinal: 0,
            },
            Name::new("PerfPathDoorFixture"),
        ))
        .id();
    world_map.register_door(door_grid, door_entity, DoorState::Closed);
    true
}

#[cfg(feature = "profiling")]
fn configure_construction_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
) -> bool {
    let tile_count = match size {
        PerfScenarioSize::Small => 16,
        PerfScenarioSize::Medium => 64,
        PerfScenarioSize::Large => 128,
    };
    let mut grids = fixture_free_grids(world_map.as_ref(), tile_count);
    if grids.len() != tile_count {
        error!(
            "PERF_CAPTURE: construction fixture found only {} of {tile_count} free walkable tiles",
            grids.len()
        );
        return false;
    }
    grids.sort_unstable();
    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let world_positions = grids
        .iter()
        .map(|(gx, gy)| WorldMap::grid_to_world(*gx, *gy))
        .collect::<Vec<_>>();
    let min = world_positions
        .iter()
        .copied()
        .reduce(Vec2::min)
        .expect("non-empty construction fixture");
    let max = world_positions
        .iter()
        .copied()
        .reduce(Vec2::max)
        .expect("non-empty construction fixture");
    let position = (min + max) * 0.5;
    let area = TaskArea::from_points(
        min - Vec2::splat(TILE_SIZE * 0.5),
        max + Vec2::splat(TILE_SIZE * 0.5),
    );
    let mut site = FloorConstructionSite::new(area, position, tile_count as u32);
    site.phase = FloorConstructionPhase::Curing;
    site.tiles_reinforced = tile_count as u32;
    site.tiles_poured = tile_count as u32;
    site.curing_remaining_secs = 300.0;
    let site_entity = commands
        .spawn((
            site,
            Transform::from_translation(position.extend(Z_MAP)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::ConstructionSite,
                ordinal: 0,
            },
            Name::new("PerfConstructionSiteFixture"),
        ))
        .id();
    for (ordinal, grid) in grids.into_iter().enumerate() {
        let tile_position = WorldMap::grid_to_world(grid.0, grid.1);
        let mut tile = FloorTileBlueprint::new(site_entity, grid);
        tile.state = FloorTileState::Complete;
        commands.spawn((
            tile,
            Transform::from_translation(tile_position.extend(Z_MAP)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::ConstructionTile,
                ordinal: ordinal as u32,
            },
            Name::new("PerfConstructionTileFixture"),
        ));
    }
    true
}

#[cfg(feature = "profiling")]
fn configure_ui_gpu_fixture(
    commands: &mut Commands,
    q_familiars: &mut Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    world_map: &mut WorldMapWrite,
    size: PerfScenarioSize,
) -> bool {
    for (_, mut command, mut operation) in q_familiars.iter_mut() {
        command.command = FamiliarCommand::Idle;
        operation.max_controlled_soul = 0;
    }

    let count = match size {
        PerfScenarioSize::Small => 64,
        PerfScenarioSize::Medium => 160,
        PerfScenarioSize::Large => 320,
    };
    let mut grids = fixture_free_grids(world_map.as_ref(), count);
    if grids.len() != count {
        error!(
            "PERF_CAPTURE: ui-gpu fixture found only {} of {count} free walkable tiles",
            grids.len()
        );
        return false;
    }
    grids.sort_unstable();
    for (ordinal, grid) in grids.into_iter().enumerate() {
        let position = WorldMap::grid_to_world(grid.0, grid.1);
        commands.spawn((
            Blueprint::new(BuildingType::Wall, vec![grid]),
            BlueprintVisualState {
                progress: 0.5,
                ..default()
            },
            Sprite {
                color: Color::srgba(0.85, 0.9, 1.0, 1.0),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(position.extend(Z_MAP + 0.2)),
            PerfFixtureMarker {
                kind: PerfFixtureKind::UiBlueprint,
                ordinal: ordinal as u32,
            },
            Name::new("PerfUiGpuBlueprintFixture"),
        ));
    }
    true
}

#[cfg(feature = "profiling")]
type PerfGridPosition = (i32, i32);
#[cfg(feature = "profiling")]
type PerfFixtureCorridor = (PerfGridPosition, PerfGridPosition, PerfGridPosition);

#[cfg(feature = "profiling")]
fn find_fixture_corridor(world_map: &WorldMap) -> Option<PerfFixtureCorridor> {
    for y in 1..MAP_HEIGHT.saturating_sub(1) {
        for x in 2..MAP_WIDTH.saturating_sub(2) {
            let grids = [(x - 1, y), (x, y), (x + 1, y)];
            if grids
                .iter()
                .all(|&(gx, gy)| fixture_grid_is_free(world_map, (gx, gy)))
            {
                return Some((grids[0], grids[1], grids[2]));
            }
        }
    }
    None
}

#[cfg(feature = "profiling")]
fn fixture_free_grids(world_map: &WorldMap, count: usize) -> Vec<(i32, i32)> {
    let mut grids = Vec::with_capacity(count);
    for y in 1..MAP_HEIGHT.saturating_sub(1) {
        for x in 1..MAP_WIDTH.saturating_sub(1) {
            let grid = (x, y);
            if fixture_grid_is_free(world_map, grid) {
                grids.push(grid);
                if grids.len() == count {
                    return grids;
                }
            }
        }
    }
    grids
}

#[cfg(feature = "profiling")]
fn fixture_grid_is_free(world_map: &WorldMap, grid: (i32, i32)) -> bool {
    world_map.is_walkable(grid.0, grid.1)
        && !world_map.buildings.contains_key(&grid)
        && !world_map.doors.contains_key(&grid)
}

/// Applies the deterministic path-door interaction sequence after the initial
/// fixture checkpoint. The slot is derived from virtual time, so render-frame
/// timing and user input cannot alter the workload.
#[cfg(feature = "profiling")]
pub(crate) fn drive_perf_workload_system(
    config: Res<PerfScenarioConfig>,
    applied: Res<PerfScenarioApplied>,
    virtual_time: Res<Time<Virtual>>,
    mut state: ResMut<PerfScenarioDriverState>,
    handles: Res<DoorVisualHandles>,
    mut world_map: WorldMapWrite,
    mut q_doors: Query<(&PerfFixtureMarker, &Transform, &mut Door, &mut Sprite)>,
) {
    if !applied.0 || !config.enabled() || config.workload != PerfWorkload::PathDoor {
        return;
    }

    let toggle_slot = (virtual_time.elapsed_secs_f64() / 0.5).floor() as u64;
    if toggle_slot == 0 || state.last_path_door_toggle_slot == Some(toggle_slot) {
        return;
    }
    state.last_path_door_toggle_slot = Some(toggle_slot);
    let next_state = if toggle_slot.is_multiple_of(2) {
        DoorState::Closed
    } else {
        DoorState::Open
    };
    for (marker, transform, mut door, mut sprite) in q_doors.iter_mut() {
        if marker.kind != PerfFixtureKind::Door || door.state == next_state {
            continue;
        }
        let grid = WorldMap::world_to_grid(transform.translation.truncate());
        hw_world::apply_door_state(
            &mut door,
            &mut sprite,
            &mut world_map,
            &handles,
            grid,
            next_state,
        );
    }
}

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

/// シナリオの初期状態を、ゲーム更新より前に固定して記録する。
///
/// `Update` の末尾で初期値を採ると、初回フレームのAIや移動が入り込み、
/// 同じ seed の fixture ではなくなってしまう。そのため、シナリオ用の
/// deferred command を適用した直後にこの checkpoint を置く。
#[cfg(feature = "profiling")]
pub(crate) fn start_perf_capture_system(
    config: Res<PerfScenarioConfig>,
    applied: Res<PerfScenarioApplied>,
    checksum_queries: PerfChecksumQueries,
    mut capture: ResMut<PerfCapture>,
    virtual_time: ResMut<Time<Virtual>>,
    fixed_time: Res<Time<Fixed>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.enabled() || !matches!(capture.phase, PerfCapturePhase::WaitingForScenario) {
        return;
    }

    if !config.workload.has_automated_setup() {
        error!(
            "PERF_CAPTURE: workload '{}' has no automated setup yet; use gather",
            config.workload.as_str()
        );
        capture.phase = PerfCapturePhase::Finished;
        exit.write(AppExit::error());
        return;
    }
    if !applied.0 {
        if config.uses_fixed_timesteps() && !capture.fixture_wait_reported {
            eprintln!(
                "PERF_DETERMINISM_AUDIT: waiting for fixture setup while virtual time remains paused"
            );
            capture.fixture_wait_reported = true;
        }
        return;
    }

    let initial_checksum = calculate_checksum(&checksum_queries);
    let expected_souls = config.soul_count as usize;
    let expected_familiars = config.familiar_count as usize;
    if initial_checksum.souls != expected_souls || initial_checksum.familiars != expected_familiars
    {
        if !capture.fixture_wait_reported {
            eprintln!(
                "{}: waiting for fixture expected_souls={expected_souls} expected_familiars={expected_familiars} observed_souls={} observed_familiars={}",
                if config.uses_fixed_timesteps() {
                    "PERF_DETERMINISM_AUDIT"
                } else {
                    "PERF_CAPTURE"
                },
                initial_checksum.souls,
                initial_checksum.familiars,
            );
            capture.fixture_wait_reported = true;
        }
        return;
    }

    capture.initial_checksum = Some(initial_checksum);
    capture.initial_scene_roots = Some(calculate_scene_root_counts(&checksum_queries));
    if config.uses_fixed_timesteps() {
        if let Err(error) = record_determinism_checkpoint(
            &mut capture,
            "fixture-pre-update",
            0,
            &virtual_time,
            &fixed_time,
            &checksum_queries,
            true,
        ) {
            error!("PERF_DETERMINISM_AUDIT: invalid initial checkpoint: {error}");
            capture.phase = PerfCapturePhase::Finished;
            exit.write(AppExit::error());
            return;
        }
        capture.phase = PerfCapturePhase::ArmFixedAudit;
        eprintln!(
            "PERF_DETERMINISM_AUDIT: fixture checkpoint captured; arming fixed_hz={} warmup_ticks={} audit_ticks={}",
            config.fixed_step_hz(),
            config.fixed_warmup_ticks(),
            config.fixed_audit_ticks(),
        );
    } else {
        capture.phase = PerfCapturePhase::Warmup;
        capture.elapsed_secs = 0.0;
        eprintln!(
            "PERF_CAPTURE: phase=warmup virtual_speed=1.0 target_secs={}",
            config.warmup_secs
        );
    }
}

/// perf scenarioのwarm-up/計測/CSV出力を自動化する。
#[cfg(feature = "profiling")]
pub(crate) fn drive_perf_capture_system(
    mut params: PerfCaptureParams,
    mut capture: ResMut<PerfCapture>,
    mut exit: MessageWriter<AppExit>,
) {
    if !params.config.enabled() || matches!(capture.phase, PerfCapturePhase::Finished) {
        return;
    }

    match capture.phase {
        PerfCapturePhase::WaitingForScenario => {}
        PerfCapturePhase::ArmFixedAudit => {
            if !params.config.uses_fixed_timesteps() {
                error!("PERF_CAPTURE: fixed audit arm phase was entered with realtime clock");
                capture.phase = PerfCapturePhase::Finished;
                exit.write(AppExit::error());
                return;
            }
            params.time.unpause();
            capture.phase = PerfCapturePhase::Warmup;
            eprintln!("PERF_DETERMINISM_AUDIT: phase=warmup");
        }
        PerfCapturePhase::Warmup => {
            if params.config.uses_fixed_timesteps() {
                if let Err(error) = advance_fixed_audit_warmup(
                    &params.config,
                    &mut capture,
                    &params.time,
                    &params.fixed_time,
                    &params.checksum_queries,
                ) {
                    error!("PERF_DETERMINISM_AUDIT: invalid warmup checkpoint: {error}");
                    capture.phase = PerfCapturePhase::Finished;
                    exit.write(AppExit::error());
                }
            } else {
                capture.elapsed_secs += params.time.delta_secs();
                capture.warmup_virtual_secs += params.time.delta_secs_f64();
                capture.warmup_real_secs += params.real_time.delta_secs_f64();
                if capture.elapsed_secs >= params.config.warmup_secs {
                    capture.warmup_checksum = Some(calculate_checksum(&params.checksum_queries));
                    capture.phase = PerfCapturePhase::Measure;
                    capture.elapsed_secs = 0.0;
                    capture.frame_times_ms.clear();
                    *params.familiar_metrics = FamiliarDelegationPerfMetrics::default();
                    *params.task_execution_metrics = TaskExecutionPerfMetrics::default();
                    *params.reservation_sync_metrics = ReservationSyncPerfMetrics::default();
                    *params.door_metrics = DoorPerfMetrics::default();
                    *params.construction_metrics = ConstructionPerfMetrics::default();
                    *params.slow_simulation_metrics = SlowSimulationPerfMetrics::default();
                    *params.energy_metrics = EnergyPerfMetrics::default();
                    params.runtime_path_budget.clear_metrics();
                    params.runtime_path_defer_metrics.clear();
                    eprintln!(
                        "PERF_CAPTURE: phase=measure target_secs={}",
                        params.config.measure_secs
                    );
                }
            }
        }
        PerfCapturePhase::Measure => {
            if params.config.uses_fixed_timesteps() {
                if let Err(error) = advance_fixed_audit_measure(
                    &params.config,
                    &mut capture,
                    &params.time,
                    &params.fixed_time,
                    &params.checksum_queries,
                ) {
                    error!("PERF_DETERMINISM_AUDIT: invalid audit checkpoint: {error}");
                    capture.phase = PerfCapturePhase::Finished;
                    exit.write(AppExit::error());
                }
            } else {
                capture.elapsed_secs += params.time.delta_secs();
                capture.measure_virtual_secs += params.time.delta_secs_f64();
                capture.measure_real_secs += params.real_time.delta_secs_f64();
                if let Some(frame_time_ms) =
                    params.diagnostics.as_deref().and_then(latest_frame_time_ms)
                {
                    capture.frame_times_ms.push(frame_time_ms);
                }
                if capture.elapsed_secs >= params.config.measure_secs {
                    capture.measure_end_checksum =
                        Some(calculate_checksum(&params.checksum_queries));
                    capture.phase = PerfCapturePhase::Flush;
                }
            }
        }
        PerfCapturePhase::Flush => {
            let result = if params.config.uses_fixed_timesteps() {
                write_determinism_audit(
                    &params.config,
                    &capture.determinism_checkpoints,
                    &capture.determinism_actor_records,
                )
            } else {
                match (
                    capture.initial_checksum,
                    capture.initial_scene_roots,
                    capture.warmup_checksum,
                    capture.measure_end_checksum,
                ) {
                    (Some(initial), Some(initial_scene_roots), Some(warmup), Some(measure_end)) => {
                        write_perf_capture(PerfCaptureWriteInput {
                            config: &params.config,
                            initial_checksum: initial,
                            initial_scene_roots,
                            warmup_checksum: warmup,
                            measure_end_checksum: measure_end,
                            samples: &capture.frame_times_ms,
                            warmup_virtual_secs: capture.warmup_virtual_secs,
                            warmup_real_secs: capture.warmup_real_secs,
                            measure_virtual_secs: capture.measure_virtual_secs,
                            measure_real_secs: capture.measure_real_secs,
                            familiar_metrics: &params.familiar_metrics,
                            task_execution_metrics: &params.task_execution_metrics,
                            reservation_sync_metrics: &params.reservation_sync_metrics,
                            door_metrics: &params.door_metrics,
                            construction_metrics: &params.construction_metrics,
                            slow_simulation_metrics: &params.slow_simulation_metrics,
                            energy_metrics: &params.energy_metrics,
                            runtime_path_metrics: params.runtime_path_budget.metrics(),
                            runtime_path_defer_metrics: &params.runtime_path_defer_metrics,
                        })
                    }
                    _ => Err(std::io::Error::other(
                        "capture reached Flush without all scenario checkpoints",
                    )),
                }
            };

            capture.phase = PerfCapturePhase::Finished;
            if let Err(error) = result {
                error!("PERF_CAPTURE: failed to write CSV: {error}");
                exit.write(AppExit::error());
            } else {
                exit.write(AppExit::Success);
            }
        }
        PerfCapturePhase::Finished => {}
    }
}

#[cfg(feature = "profiling")]
fn advance_fixed_audit_warmup(
    config: &PerfScenarioConfig,
    capture: &mut PerfCapture,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> Result<(), String> {
    capture.fixed_update_tick += 1;
    let tick = capture.fixed_update_tick;
    if let Some(checkpoint) = early_checkpoint_name(tick) {
        record_determinism_checkpoint(
            capture,
            checkpoint,
            tick,
            virtual_time,
            fixed_time,
            checksum_queries,
            false,
        )?;
    }
    if tick == config.fixed_warmup_ticks() {
        record_determinism_checkpoint(
            capture,
            "post-warmup",
            tick,
            virtual_time,
            fixed_time,
            checksum_queries,
            false,
        )?;
        capture.phase = PerfCapturePhase::Measure;
        eprintln!("PERF_DETERMINISM_AUDIT: phase=audit");
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn advance_fixed_audit_measure(
    config: &PerfScenarioConfig,
    capture: &mut PerfCapture,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> Result<(), String> {
    capture.fixed_update_tick += 1;
    let tick = capture.fixed_update_tick;
    if tick == config.fixed_audit_end_tick() {
        record_determinism_checkpoint(
            capture,
            "post-audit-end",
            tick,
            virtual_time,
            fixed_time,
            checksum_queries,
            false,
        )?;
        capture.phase = PerfCapturePhase::Flush;
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn early_checkpoint_name(tick: u64) -> Option<&'static str> {
    match tick {
        1 => Some("post-update-1"),
        8 => Some("post-update-8"),
        32 => Some("post-update-32"),
        128 => Some("post-update-128"),
        _ => None,
    }
}

#[cfg(feature = "profiling")]
fn record_determinism_checkpoint(
    capture: &mut PerfCapture,
    checkpoint: &'static str,
    update_tick: u64,
    virtual_time: &Time<Virtual>,
    fixed_time: &Time<Fixed>,
    checksum_queries: &PerfChecksumQueries<'_, '_>,
    expects_paused_virtual_time: bool,
) -> Result<(), String> {
    if virtual_time.is_paused() != expects_paused_virtual_time {
        return Err(format!(
            "{checkpoint}: virtual pause state is {}, expected {}",
            virtual_time.is_paused(),
            expects_paused_virtual_time
        ));
    }
    if virtual_time.relative_speed_f64() != 1.0 {
        return Err(format!(
            "{checkpoint}: virtual relative speed is {}, expected 1.0",
            virtual_time.relative_speed_f64()
        ));
    }
    if !expects_paused_virtual_time {
        let timestep = fixed_time.timestep();
        if virtual_time.delta() != timestep {
            return Err(format!(
                "{checkpoint}: virtual delta {:?} differs from fixed timestep {:?}",
                virtual_time.delta(),
                timestep
            ));
        }
        if fixed_time.delta() != timestep {
            return Err(format!(
                "{checkpoint}: fixed delta {:?} differs from fixed timestep {:?}",
                fixed_time.delta(),
                timestep
            ));
        }
        if fixed_time.overstep() != std::time::Duration::ZERO {
            return Err(format!(
                "{checkpoint}: fixed overstep is {:?}, expected zero",
                fixed_time.overstep()
            ));
        }
    }

    let audit_records = collect_audit_actor_records(checksum_queries)?;
    let checksum = checksum_from_audit_records(&audit_records);
    capture
        .determinism_checkpoints
        .push(PerfDeterminismCheckpoint {
            checkpoint,
            update_tick,
            fixed_timestep_ns: fixed_time.timestep().as_nanos(),
            virtual_delta_ns: virtual_time.delta().as_nanos(),
            virtual_elapsed_ns: virtual_time.elapsed().as_nanos(),
            fixed_delta_ns: fixed_time.delta().as_nanos(),
            fixed_elapsed_ns: fixed_time.elapsed().as_nanos(),
            fixed_overstep_ns: fixed_time.overstep().as_nanos(),
            virtual_paused: virtual_time.is_paused(),
            virtual_relative_speed_bits: virtual_time.relative_speed_f64().to_bits(),
            virtual_effective_speed_bits: virtual_time.effective_speed_f64().to_bits(),
            checksum,
        });
    capture
        .determinism_actor_records
        .extend(
            audit_records
                .into_iter()
                .map(|record| PerfDeterminismActorRecord {
                    checkpoint,
                    update_tick,
                    actor_kind: record.actor_kind,
                    actor_key: record.actor_key,
                    record: record.record,
                }),
        );
    Ok(())
}

#[cfg(feature = "profiling")]
fn latest_frame_time_ms(diagnostics: &bevy::diagnostic::DiagnosticsStore) -> Option<f64> {
    diagnostics
        .get_measurement(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map(|measurement| measurement.value)
}

#[cfg(feature = "profiling")]
fn calculate_checksum(checksum_queries: &PerfChecksumQueries<'_, '_>) -> PerfScenarioChecksum {
    let souls = checksum_queries
        .souls
        .iter()
        .map(|(_, transform)| checksum_position(transform))
        .collect::<Vec<_>>();
    let familiars = checksum_queries
        .familiars
        .iter()
        .map(|(_, transform)| checksum_position(transform))
        .collect::<Vec<_>>();
    let designations = checksum_queries.designations.iter().count();
    let mut positions = souls.clone();
    positions.extend(familiars.iter().copied());
    positions.sort_unstable();

    let mut checksum = 0xcbf2_9ce4_8422_2325u64;
    for value in [
        souls.len() as u64,
        familiars.len() as u64,
        designations as u64,
    ] {
        checksum = fnv1a(checksum, value);
    }
    for (x, y) in positions {
        checksum = fnv1a(checksum, x as u64);
        checksum = fnv1a(checksum, y as u64);
    }

    PerfScenarioChecksum {
        souls: souls.len(),
        familiars: familiars.len(),
        designations,
        value: checksum,
    }
}

#[cfg(feature = "profiling")]
fn calculate_scene_root_counts(
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> PerfSceneRootCounts {
    PerfSceneRootCounts {
        soul_proxy_3d: checksum_queries.soul_proxy_3d.iter().count(),
        soul_mask_proxy_3d: checksum_queries.soul_mask_proxy_3d.iter().count(),
        soul_shadow_proxy_3d: checksum_queries.soul_shadow_proxy_3d.iter().count(),
        familiar_proxy_3d: checksum_queries.familiar_proxy_3d.iter().count(),
        building_3d_visual: checksum_queries.building_3d_visual.iter().count(),
    }
}

#[cfg(feature = "profiling")]
fn collect_audit_actor_records(
    checksum_queries: &PerfChecksumQueries<'_, '_>,
) -> Result<Vec<PerfAuditActorRecord>, String> {
    let mut records = Vec::new();

    for (entity, transform, soul, idle, destination, path, task, random_state) in
        checksum_queries.audit_souls.iter()
    {
        let mut record = vec![b'S'];
        write_transform(&mut record, transform, "soul transform")?;
        write_f32(&mut record, soul.laziness, "soul laziness")?;
        write_f32(&mut record, soul.motivation, "soul motivation")?;
        write_f32(&mut record, soul.fatigue, "soul fatigue")?;
        write_f32(&mut record, soul.stress, "soul stress")?;
        write_f32(&mut record, soul.dream, "soul dream")?;
        write_idle_state(&mut record, idle)?;
        write_vec2(&mut record, destination.0, "soul destination")?;
        write_path(&mut record, path, "soul path")?;
        write_assigned_task(&mut record, task, &checksum_queries.target_transforms)?;
        write_option_u64(
            &mut record,
            random_state.map(SimulationRandomState::audit_cursor),
        );
        records.push(PerfAuditActorRecord {
            actor_kind: "soul",
            actor_key: random_state
                .map(SimulationRandomState::stable_key)
                .unwrap_or_else(|| entity.to_bits()),
            record,
        });
    }

    for (
        entity,
        transform,
        familiar,
        destination,
        path,
        command,
        operation,
        ai_state,
        random_state,
    ) in checksum_queries.audit_familiars.iter()
    {
        let mut record = vec![b'F'];
        write_transform(&mut record, transform, "familiar transform")?;
        write_familiar_state(&mut record, familiar, command, operation, ai_state)?;
        write_vec2(&mut record, destination.0, "familiar destination")?;
        write_path(&mut record, path, "familiar path")?;
        write_option_u64(
            &mut record,
            random_state.map(SimulationRandomState::audit_cursor),
        );
        records.push(PerfAuditActorRecord {
            actor_kind: "familiar",
            actor_key: random_state
                .map(SimulationRandomState::stable_key)
                .unwrap_or_else(|| entity.to_bits()),
            record,
        });
    }

    for (entity, transform, designation, priority, slots) in
        checksum_queries.audit_designations.iter()
    {
        let mut record = vec![b'D'];
        write_transform(&mut record, transform, "designation transform")?;
        write_work_type(&mut record, designation.work_type);
        write_option_u32(&mut record, priority.map(|priority| priority.0));
        write_option_u32(&mut record, slots.map(|slots| slots.max));
        records.push(PerfAuditActorRecord {
            actor_kind: "designation",
            actor_key: entity.to_bits(),
            record,
        });
    }

    for (_entity, marker, transform, door, floor_site, floor_tile, blueprint) in
        checksum_queries.audit_fixtures.iter()
    {
        let mut record = vec![b'X', marker.kind.audit_tag()];
        write_u64(&mut record, marker.ordinal as u64);
        write_transform(&mut record, transform, "fixture transform")?;
        match marker.kind {
            PerfFixtureKind::Door => {
                let Some(door) = door else {
                    return Err("door fixture is missing Door".to_string());
                };
                write_door_state(&mut record, door.state);
            }
            PerfFixtureKind::ConstructionSite => {
                let Some(site) = floor_site else {
                    return Err(
                        "construction site fixture is missing FloorConstructionSite".to_string()
                    );
                };
                write_floor_phase(&mut record, site.phase);
                write_u64(&mut record, site.tiles_total as u64);
                write_u64(&mut record, site.tiles_reinforced as u64);
                write_u64(&mut record, site.tiles_poured as u64);
                write_f32(
                    &mut record,
                    site.curing_remaining_secs,
                    "fixture curing remaining secs",
                )?;
            }
            PerfFixtureKind::ConstructionTile => {
                let Some(tile) = floor_tile else {
                    return Err(
                        "construction tile fixture is missing FloorTileBlueprint".to_string()
                    );
                };
                write_grid_pos(&mut record, tile.grid_pos);
                write_floor_tile_state(&mut record, tile.state);
                write_u64(&mut record, tile.bones_delivered as u64);
                write_u64(&mut record, tile.mud_delivered as u64);
            }
            PerfFixtureKind::UiBlueprint => {
                let Some(blueprint) = blueprint else {
                    return Err("ui-gpu fixture is missing Blueprint".to_string());
                };
                write_building_type(&mut record, blueprint.kind);
                write_f32(
                    &mut record,
                    blueprint.progress,
                    "fixture blueprint progress",
                )?;
                write_u64(&mut record, blueprint.occupied_grids.len() as u64);
                for grid in &blueprint.occupied_grids {
                    write_grid_pos(&mut record, *grid);
                }
            }
        }
        records.push(PerfAuditActorRecord {
            actor_kind: "fixture",
            actor_key: ((marker.kind.audit_tag() as u64) << 32) | u64::from(marker.ordinal),
            record,
        });
    }

    records.sort_unstable_by(|left, right| {
        left.actor_kind
            .cmp(right.actor_kind)
            .then(left.actor_key.cmp(&right.actor_key))
    });
    Ok(records)
}

#[cfg(feature = "profiling")]
fn checksum_from_audit_records(records: &[PerfAuditActorRecord]) -> PerfScenarioChecksum {
    let mut payloads = records
        .iter()
        .map(|record| record.record.as_slice())
        .collect::<Vec<_>>();
    payloads.sort_unstable();
    let mut checksum = fnv1a(0xcbf2_9ce4_8422_2325u64, payloads.len() as u64);
    for record in payloads {
        checksum = fnv1a_bytes(checksum, record);
    }

    PerfScenarioChecksum {
        souls: records
            .iter()
            .filter(|record| record.actor_kind == "soul")
            .count(),
        familiars: records
            .iter()
            .filter(|record| record.actor_kind == "familiar")
            .count(),
        designations: records
            .iter()
            .filter(|record| record.actor_kind == "designation")
            .count(),
        value: checksum,
    }
}

#[cfg(feature = "profiling")]
fn checksum_position(transform: &Transform) -> (i64, i64) {
    (
        (transform.translation.x * 100.0).round() as i64,
        (transform.translation.y * 100.0).round() as i64,
    )
}

#[cfg(feature = "profiling")]
fn write_transform(record: &mut Vec<u8>, transform: &Transform, label: &str) -> Result<(), String> {
    write_f32(record, transform.translation.x, label)?;
    write_f32(record, transform.translation.y, label)?;
    write_f32(record, transform.translation.z, label)
}

#[cfg(feature = "profiling")]
fn write_vec2(record: &mut Vec<u8>, value: Vec2, label: &str) -> Result<(), String> {
    write_f32(record, value.x, label)?;
    write_f32(record, value.y, label)
}

#[cfg(feature = "profiling")]
fn write_f32(record: &mut Vec<u8>, value: f32, label: &str) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{label} contains non-finite value {value}"));
    }
    let normalized = if value == 0.0 { 0.0 } else { value };
    record.extend_from_slice(&normalized.to_bits().to_le_bytes());
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_u64(record: &mut Vec<u8>, value: u64) {
    record.extend_from_slice(&value.to_le_bytes());
}

#[cfg(feature = "profiling")]
fn write_option_u32(record: &mut Vec<u8>, value: Option<u32>) {
    match value {
        Some(value) => {
            record.push(1);
            record.extend_from_slice(&value.to_le_bytes());
        }
        None => record.push(0),
    }
}

#[cfg(feature = "profiling")]
fn write_option_u64(record: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            record.push(1);
            write_u64(record, value);
        }
        None => record.push(0),
    }
}

#[cfg(feature = "profiling")]
fn write_idle_state(record: &mut Vec<u8>, idle: &IdleState) -> Result<(), String> {
    write_f32(record, idle.idle_timer, "idle timer")?;
    write_f32(record, idle.total_idle_time, "total idle time")?;
    write_idle_behavior(record, idle.behavior);
    write_f32(record, idle.behavior_duration, "idle behavior duration")?;
    write_gathering_behavior(record, idle.gathering_behavior);
    write_f32(
        record,
        idle.gathering_behavior_timer,
        "gathering behavior timer",
    )?;
    write_f32(
        record,
        idle.gathering_behavior_duration,
        "gathering behavior duration",
    )?;
    record.push(u8::from(idle.needs_separation));
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_idle_behavior(record: &mut Vec<u8>, behavior: IdleBehavior) {
    record.push(match behavior {
        IdleBehavior::Wandering => 0,
        IdleBehavior::Sitting => 1,
        IdleBehavior::Sleeping => 2,
        IdleBehavior::Gathering => 3,
        IdleBehavior::ExhaustedGathering => 4,
        IdleBehavior::Resting => 5,
        IdleBehavior::GoingToRest => 6,
        IdleBehavior::Escaping => 7,
        IdleBehavior::Drifting => 8,
    });
}

#[cfg(feature = "profiling")]
fn write_gathering_behavior(record: &mut Vec<u8>, behavior: GatheringBehavior) {
    record.push(match behavior {
        GatheringBehavior::Wandering => 0,
        GatheringBehavior::Sleeping => 1,
        GatheringBehavior::Standing => 2,
        GatheringBehavior::Dancing => 3,
    });
}

#[cfg(feature = "profiling")]
fn write_path(record: &mut Vec<u8>, path: &Path, label: &str) -> Result<(), String> {
    write_u64(record, path.waypoints.len() as u64);
    write_u64(record, path.current_index as u64);
    for waypoint in &path.waypoints {
        write_vec2(record, *waypoint, label)?;
    }
    match path.planned_destination {
        Some(destination) => {
            record.push(1);
            write_vec2(record, destination, label)?;
        }
        None => record.push(0),
    }
    write_u64(record, path.validated_obstacle_version);
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_assigned_task(
    record: &mut Vec<u8>,
    task: &AssignedTask,
    target_transforms: &Query<&Transform>,
) -> Result<(), String> {
    match task {
        AssignedTask::None => record.push(0),
        AssignedTask::Gather(data) => {
            record.push(1);
            write_work_type(record, data.work_type);
            match data.phase {
                GatherPhase::GoingToResource => record.push(0),
                GatherPhase::Collecting { progress } => {
                    record.push(1);
                    write_f32(record, progress, "gather progress")?;
                }
                GatherPhase::Done => record.push(2),
            }
            let target = target_transforms
                .get(data.target)
                .map_err(|_| "gather task references an entity without a transform".to_string())?;
            write_transform(record, target, "gather target transform")?;
        }
        _ => {
            return Err(
                "gather determinism audit encountered an unsupported AssignedTask variant"
                    .to_string(),
            );
        }
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_familiar_state(
    record: &mut Vec<u8>,
    familiar: &Familiar,
    command: &ActiveCommand,
    operation: &FamiliarOperation,
    ai_state: &FamiliarAiState,
) -> Result<(), String> {
    record.push(match familiar.familiar_type {
        hw_core::familiar::FamiliarType::Imp => 0,
    });
    write_f32(record, familiar.command_radius, "familiar command radius")?;
    write_f32(record, familiar.efficiency, "familiar efficiency")?;
    record.extend_from_slice(&familiar.color_index.to_le_bytes());
    record.push(match command.command {
        FamiliarCommand::Idle => 0,
        FamiliarCommand::GatherResources => 1,
        FamiliarCommand::Patrol => 2,
    });
    write_f32(
        record,
        operation.fatigue_threshold,
        "familiar fatigue threshold",
    )?;
    write_u64(record, operation.max_controlled_soul as u64);
    match ai_state {
        FamiliarAiState::Idle => record.push(0),
        FamiliarAiState::SearchingTask => record.push(1),
        FamiliarAiState::Scouting { .. } => record.push(2),
        FamiliarAiState::Supervising { target, timer } => {
            record.push(3);
            record.push(u8::from(target.is_some()));
            write_f32(record, *timer, "familiar supervising timer")?;
        }
    }
    Ok(())
}

#[cfg(feature = "profiling")]
fn write_work_type(record: &mut Vec<u8>, work_type: WorkType) {
    record.push(match work_type {
        WorkType::Chop => 0,
        WorkType::Mine => 1,
        WorkType::Build => 2,
        WorkType::Move => 3,
        WorkType::Haul => 4,
        WorkType::HaulToMixer => 5,
        WorkType::GatherWater => 6,
        WorkType::CollectBone => 7,
        WorkType::Refine => 8,
        WorkType::HaulWaterToMixer => 9,
        WorkType::WheelbarrowHaul => 10,
        WorkType::ReinforceFloorTile => 11,
        WorkType::PourFloorTile => 12,
        WorkType::FrameWallTile => 13,
        WorkType::CoatWall => 14,
        WorkType::GeneratePower => 15,
    });
}

#[cfg(feature = "profiling")]
fn write_door_state(record: &mut Vec<u8>, state: DoorState) {
    record.push(match state {
        DoorState::Open => 0,
        DoorState::Closed => 1,
        DoorState::Locked => 2,
    });
}

#[cfg(feature = "profiling")]
fn write_floor_phase(record: &mut Vec<u8>, phase: FloorConstructionPhase) {
    record.push(match phase {
        FloorConstructionPhase::Reinforcing => 0,
        FloorConstructionPhase::Pouring => 1,
        FloorConstructionPhase::Curing => 2,
    });
}

#[cfg(feature = "profiling")]
fn write_floor_tile_state(record: &mut Vec<u8>, state: FloorTileState) {
    record.push(match state {
        FloorTileState::WaitingBones => 0,
        FloorTileState::ReinforcingReady => 1,
        FloorTileState::Reinforcing { .. } => 2,
        FloorTileState::ReinforcedComplete => 3,
        FloorTileState::WaitingMud => 4,
        FloorTileState::PouringReady => 5,
        FloorTileState::Pouring { .. } => 6,
        FloorTileState::Complete => 7,
    });
}

#[cfg(feature = "profiling")]
fn write_building_type(record: &mut Vec<u8>, kind: BuildingType) {
    record.push(match kind {
        BuildingType::Wall => 0,
        BuildingType::Door => 1,
        BuildingType::Floor => 2,
        BuildingType::Tank => 3,
        BuildingType::MudMixer => 4,
        BuildingType::RestArea => 5,
        BuildingType::Bridge => 6,
        BuildingType::SandPile => 7,
        BuildingType::BonePile => 8,
        BuildingType::WheelbarrowParking => 9,
        BuildingType::SoulSpa => 10,
        BuildingType::OutdoorLamp => 11,
    });
}

#[cfg(feature = "profiling")]
fn write_grid_pos(record: &mut Vec<u8>, grid: (i32, i32)) {
    record.extend_from_slice(&grid.0.to_le_bytes());
    record.extend_from_slice(&grid.1.to_le_bytes());
}

#[cfg(feature = "profiling")]
struct PerfCaptureWriteInput<'a> {
    config: &'a PerfScenarioConfig,
    initial_checksum: PerfScenarioChecksum,
    initial_scene_roots: PerfSceneRootCounts,
    warmup_checksum: PerfScenarioChecksum,
    measure_end_checksum: PerfScenarioChecksum,
    samples: &'a [f64],
    warmup_virtual_secs: f64,
    warmup_real_secs: f64,
    measure_virtual_secs: f64,
    measure_real_secs: f64,
    familiar_metrics: &'a FamiliarDelegationPerfMetrics,
    task_execution_metrics: &'a TaskExecutionPerfMetrics,
    reservation_sync_metrics: &'a ReservationSyncPerfMetrics,
    door_metrics: &'a DoorPerfMetrics,
    construction_metrics: &'a ConstructionPerfMetrics,
    slow_simulation_metrics: &'a SlowSimulationPerfMetrics,
    energy_metrics: &'a EnergyPerfMetrics,
    runtime_path_metrics: &'a RuntimePathSearchMetrics,
    runtime_path_defer_metrics: &'a RuntimePathDeferMetrics,
}

#[cfg(feature = "profiling")]
fn write_perf_capture(input: PerfCaptureWriteInput<'_>) -> std::io::Result<()> {
    let PerfCaptureWriteInput {
        config,
        initial_checksum,
        initial_scene_roots,
        warmup_checksum,
        measure_end_checksum,
        samples,
        warmup_virtual_secs,
        warmup_real_secs,
        measure_virtual_secs,
        measure_real_secs,
        familiar_metrics,
        task_execution_metrics,
        reservation_sync_metrics,
        door_metrics,
        construction_metrics,
        slow_simulation_metrics,
        energy_metrics,
        runtime_path_metrics,
        runtime_path_defer_metrics,
    } = input;

    if config.uses_fixed_timesteps() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "frame-time capture must not write fixed-step audit artifacts",
        ));
    }
    if samples.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame-time capture produced no samples",
        ));
    }

    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;

    let frames_path = directory.join("frames.csv");
    let summary_path = directory.join("summary.csv");
    let scene_roots_path = directory.join("scene_roots.csv");
    if frames_path.exists()
        || summary_path.exists()
        || scene_roots_path.exists()
        || directory.join("determinism.csv").exists()
        || directory.join("determinism_records.csv").exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("perf output already exists in {}", directory.display()),
        ));
    }
    let mut frame_csv = String::from("frame_index,frame_time_ms\n");
    for (index, frame_time_ms) in samples.iter().enumerate() {
        frame_csv.push_str(&format!("{index},{frame_time_ms:.6}\n"));
    }
    std::fs::write(&frames_path, frame_csv)?;

    let scene_roots_csv = format!(
        concat!(
            "soul_proxy_3d,soul_mask_proxy_3d,soul_shadow_proxy_3d,",
            "familiar_proxy_3d,building_3d_visual\n",
            "{},{},{},{},{}\n"
        ),
        initial_scene_roots.soul_proxy_3d,
        initial_scene_roots.soul_mask_proxy_3d,
        initial_scene_roots.soul_shadow_proxy_3d,
        initial_scene_roots.familiar_proxy_3d,
        initial_scene_roots.building_3d_visual,
    );
    std::fs::write(&scene_roots_path, scene_roots_csv)?;

    let (p50, p95, p99) = percentile_summary(samples);
    let max = samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let summary_header = concat!(
        "schema_version,seed,workload,size,render,configured_souls,configured_familiars,",
        "initial_souls,initial_familiars,initial_designations,initial_state_checksum,",
        "warmup_souls,warmup_familiars,warmup_designations,warmup_state_checksum,",
        "measure_end_souls,measure_end_familiars,measure_end_designations,measure_end_state_checksum,",
        "samples,p50_ms,p95_ms,p99_ms,max_ms,warmup_virtual_secs,warmup_real_secs,",
        "measure_virtual_secs,measure_real_secs,virtual_time_speed,delegation_latest_ms,",
        "delegation_cycles,incoming_snapshot_builds,delegation_familiars_processed,",
        "source_selector_calls,source_selector_scanned_items,",
        "reachable_with_cache_calls,task_execution_souls_queried,task_execution_idle_skips,",
        "task_execution_handler_runs,reservation_sync_full_rebuilds,",
        "reservation_sync_pending_tasks_scanned,reservation_sync_assigned_tasks_scanned,",
        "runtime_path_actor_new_core_searches,runtime_path_actor_new_deferred,",
        "runtime_path_actor_reuse_core_searches,runtime_path_actor_reuse_deferred,",
        "runtime_path_actor_rest_fallback_core_searches,runtime_path_actor_rest_fallback_deferred,",
        "runtime_path_escape_core_searches,runtime_path_escape_deferred,",
        "runtime_path_task_execution_core_searches,runtime_path_task_execution_deferred,",
        "runtime_path_bucket_transport_core_searches,runtime_path_bucket_transport_deferred,",
        "runtime_path_total_core_searches,runtime_path_expanded_nodes,",
        "runtime_path_max_expanded_nodes_per_search,runtime_path_active_task_max_defer_frames,",
        "runtime_path_idle_or_rest_max_defer_frames,runtime_path_deferred_actor_retries,",
        "door_open_souls_scanned,door_open_waypoints_scanned,door_close_souls_scanned,",
        "construction_floor_sites_considered,construction_wall_sites_considered,",
        "construction_floor_tiles_inspected,construction_wall_tiles_inspected,",
        "construction_evacuation_candidates_scanned,",
        "construction_floor_phase_elapsed_micros,construction_floor_completion_elapsed_micros,",
        "construction_wall_phase_elapsed_micros,construction_wall_completion_elapsed_micros,",
        "slow_simulation_steps,slow_simulation_souls_updated,slow_simulation_idle_decisions,",
        "slow_simulation_idle_spatial_target_lookups,slow_simulation_state_sanity_audits,",
        "energy_power_output_runs,energy_grid_recalc_runs,energy_lamp_steps,",
        "energy_lamp_candidates_scanned\n"
    );
    let summary_fields = vec![
        PERF_SUMMARY_SCHEMA_VERSION.to_string(),
        config.master_seed.to_string(),
        config.workload.as_str().to_string(),
        config.size.as_str().to_string(),
        config.render_mode.as_str().to_string(),
        config.soul_count.to_string(),
        config.familiar_count.to_string(),
        initial_checksum.souls.to_string(),
        initial_checksum.familiars.to_string(),
        initial_checksum.designations.to_string(),
        format!("{:016x}", initial_checksum.value),
        warmup_checksum.souls.to_string(),
        warmup_checksum.familiars.to_string(),
        warmup_checksum.designations.to_string(),
        format!("{:016x}", warmup_checksum.value),
        measure_end_checksum.souls.to_string(),
        measure_end_checksum.familiars.to_string(),
        measure_end_checksum.designations.to_string(),
        format!("{:016x}", measure_end_checksum.value),
        samples.len().to_string(),
        format!("{p50:.6}"),
        format!("{p95:.6}"),
        format!("{p99:.6}"),
        format!("{max:.6}"),
        format!("{warmup_virtual_secs:.6}"),
        format!("{warmup_real_secs:.6}"),
        format!("{measure_virtual_secs:.6}"),
        format!("{measure_real_secs:.6}"),
        "1.0".to_string(),
        format!("{:.6}", familiar_metrics.latest_elapsed_ms),
        familiar_metrics.delegation_cycles.to_string(),
        familiar_metrics.incoming_snapshot_builds.to_string(),
        familiar_metrics.familiars_processed.to_string(),
        familiar_metrics.source_selector_calls.to_string(),
        familiar_metrics.source_selector_scanned_items.to_string(),
        familiar_metrics.reachable_with_cache_calls.to_string(),
        task_execution_metrics.souls_queried.to_string(),
        task_execution_metrics.idle_skips.to_string(),
        task_execution_metrics.handler_runs.to_string(),
        reservation_sync_metrics.full_rebuilds.to_string(),
        reservation_sync_metrics.pending_tasks_scanned.to_string(),
        reservation_sync_metrics.assigned_tasks_scanned.to_string(),
        runtime_path_metrics.actor_new_core_searches.to_string(),
        runtime_path_metrics.actor_new_deferred.to_string(),
        runtime_path_metrics.actor_reuse_core_searches.to_string(),
        runtime_path_metrics.actor_reuse_deferred.to_string(),
        runtime_path_metrics
            .actor_rest_fallback_core_searches
            .to_string(),
        runtime_path_metrics
            .actor_rest_fallback_deferred
            .to_string(),
        runtime_path_metrics.escape_core_searches.to_string(),
        runtime_path_metrics.escape_deferred.to_string(),
        runtime_path_metrics
            .task_execution_core_searches
            .to_string(),
        runtime_path_metrics.task_execution_deferred.to_string(),
        runtime_path_metrics
            .bucket_transport_core_searches
            .to_string(),
        runtime_path_metrics.bucket_transport_deferred.to_string(),
        runtime_path_metrics.total_core_searches().to_string(),
        runtime_path_metrics.expanded_nodes.to_string(),
        runtime_path_metrics
            .max_expanded_nodes_per_search
            .to_string(),
        runtime_path_defer_metrics
            .active_task_max_defer_frames
            .to_string(),
        runtime_path_defer_metrics
            .idle_or_rest_max_defer_frames
            .to_string(),
        runtime_path_defer_metrics
            .deferred_actor_retries
            .to_string(),
        door_metrics.open_souls_scanned.to_string(),
        door_metrics.open_waypoints_scanned.to_string(),
        door_metrics.close_souls_scanned.to_string(),
        construction_metrics.floor_sites_considered.to_string(),
        construction_metrics.wall_sites_considered.to_string(),
        construction_metrics.floor_tiles_inspected.to_string(),
        construction_metrics.wall_tiles_inspected.to_string(),
        construction_metrics
            .evacuation_candidates_scanned
            .to_string(),
        construction_metrics.floor_phase_elapsed_micros.to_string(),
        construction_metrics
            .floor_completion_elapsed_micros
            .to_string(),
        construction_metrics.wall_phase_elapsed_micros.to_string(),
        construction_metrics
            .wall_completion_elapsed_micros
            .to_string(),
        slow_simulation_metrics.steps.to_string(),
        slow_simulation_metrics.souls_updated.to_string(),
        slow_simulation_metrics.idle_decisions.to_string(),
        slow_simulation_metrics
            .idle_spatial_target_lookups
            .to_string(),
        slow_simulation_metrics.state_sanity_audits.to_string(),
        energy_metrics.power_output_runs.to_string(),
        energy_metrics.grid_recalc_runs.to_string(),
        energy_metrics.lamp_steps.to_string(),
        energy_metrics.lamp_candidates_scanned.to_string(),
    ];
    let summary = format!("{summary_header}{}\n", summary_fields.join(","));
    std::fs::write(&summary_path, summary)?;
    eprintln!(
        "PERF_CAPTURE: wrote {} samples to {} (p50={p50:.3}ms p95={p95:.3}ms p99={p99:.3}ms initial_checksum={:016x} warmup_checksum={:016x})",
        samples.len(),
        directory.display(),
        initial_checksum.value,
        warmup_checksum.value,
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn perf_output_directory(config: &PerfScenarioConfig) -> PathBuf {
    config.output_dir.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "target/perf/{}-{}-{}-seed-{}",
            config.workload.as_str(),
            config.size.as_str(),
            config.render_mode.as_str(),
            config.master_seed
        ))
    })
}

#[cfg(feature = "profiling")]
fn expected_determinism_checkpoints(config: &PerfScenarioConfig) -> [(&'static str, u64); 7] {
    [
        ("fixture-pre-update", 0),
        ("post-update-1", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[0]),
        ("post-update-8", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[1]),
        ("post-update-32", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[2]),
        ("post-update-128", FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS[3]),
        ("post-warmup", config.fixed_warmup_ticks()),
        ("post-audit-end", config.fixed_audit_end_tick()),
    ]
}

#[cfg(feature = "profiling")]
fn write_determinism_audit(
    config: &PerfScenarioConfig,
    checkpoints: &[PerfDeterminismCheckpoint],
    actor_records: &[PerfDeterminismActorRecord],
) -> std::io::Result<()> {
    if !config.uses_fixed_timesteps() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "determinism audit requires --perf-clock fixed",
        ));
    }
    let expected = expected_determinism_checkpoints(config);
    let observed = checkpoints
        .iter()
        .map(|checkpoint| (checkpoint.checkpoint, checkpoint.update_tick))
        .collect::<Vec<_>>();
    if observed.as_slice() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("determinism checkpoints are {observed:?}; expected {expected:?}"),
        ));
    }

    let directory = perf_output_directory(config);
    std::fs::create_dir_all(&directory)?;
    let determinism_path = directory.join("determinism.csv");
    let actor_records_path = directory.join("determinism_records.csv");
    if determinism_path.exists()
        || actor_records_path.exists()
        || directory.join("frames.csv").exists()
        || directory.join("summary.csv").exists()
        || directory.join("scene_roots.csv").exists()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("perf output already exists in {}", directory.display()),
        ));
    }

    let mut csv = String::from(concat!(
        "schema_version,checkpoint,update_tick,fixed_timestep_ns,virtual_delta_ns,",
        "virtual_elapsed_ns,fixed_delta_ns,fixed_elapsed_ns,fixed_overstep_ns,virtual_paused,",
        "virtual_relative_speed_bits,virtual_effective_speed_bits,souls,familiars,designations,",
        "state_checksum\n"
    ));
    for checkpoint in checkpoints {
        csv.push_str(&format!(
            "1,{},{},{},{},{},{},{},{},{},{:016x},{:016x},{},{},{},{:016x}\n",
            checkpoint.checkpoint,
            checkpoint.update_tick,
            checkpoint.fixed_timestep_ns,
            checkpoint.virtual_delta_ns,
            checkpoint.virtual_elapsed_ns,
            checkpoint.fixed_delta_ns,
            checkpoint.fixed_elapsed_ns,
            checkpoint.fixed_overstep_ns,
            u8::from(checkpoint.virtual_paused),
            checkpoint.virtual_relative_speed_bits,
            checkpoint.virtual_effective_speed_bits,
            checkpoint.checksum.souls,
            checkpoint.checksum.familiars,
            checkpoint.checksum.designations,
            checkpoint.checksum.value,
        ));
    }
    std::fs::write(&determinism_path, csv)?;

    let mut records_csv =
        String::from("schema_version,checkpoint,update_tick,actor_kind,actor_key,record_hex\n");
    for record in actor_records {
        records_csv.push_str(&format!(
            "1,{},{},{},{},{}\n",
            record.checkpoint,
            record.update_tick,
            record.actor_kind,
            record.actor_key,
            encode_hex(&record.record),
        ));
    }
    std::fs::write(&actor_records_path, records_csv)?;
    eprintln!(
        "PERF_DETERMINISM_AUDIT: wrote {} checkpoints and {} actor records to {}",
        checkpoints.len(),
        actor_records.len(),
        directory.display(),
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(feature = "profiling")]
fn percentile_summary(samples: &[f64]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |ratio: f64| {
        let index = ((sorted.len() - 1) as f64 * ratio).round() as usize;
        sorted[index]
    };
    (percentile(0.50), percentile(0.95), percentile(0.99))
}

#[cfg(feature = "profiling")]
const fn fnv1a(current: u64, value: u64) -> u64 {
    let mut hash = current;
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

#[cfg(feature = "profiling")]
fn fnv1a_bytes(mut current: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        current ^= *byte as u64;
        current = current.wrapping_mul(0x0000_0100_0000_01b3);
    }
    current
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn value_from_args_or_env(
    args: &[String],
    flag: &str,
    env_key: &str,
) -> Result<Option<String>, PerfScenarioConfigError> {
    value_from_args(args, flag).map(|value| value.or_else(|| env::var(env_key).ok()))
}

fn parse_value_or_default<T>(
    value: Option<String>,
    flag: &str,
    allowed: &str,
    parse: impl FnOnce(&str) -> Option<T>,
    default: T,
) -> Result<T, PerfScenarioConfigError> {
    match value {
        Some(value) => parse(&value).ok_or_else(|| {
            PerfScenarioConfigError(format!("{flag} must be one of {allowed}; got '{value}'"))
        }),
        None => Ok(default),
    }
}

fn parse_u32_value_or_default(
    value: Option<String>,
    flag: &str,
    default: u32,
) -> Result<u32, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(default),
    }
}

fn parse_u64_value_or_random(
    value: Option<String>,
    flag: &str,
) -> Result<u64, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(rand::random()),
    }
}

fn parse_u64_value_or_default(
    value: Option<String>,
    flag: &str,
    default: u64,
) -> Result<u64, PerfScenarioConfigError> {
    match value {
        Some(value) => value.parse().map_err(|_| {
            PerfScenarioConfigError(format!("{flag} must be an unsigned integer; got '{value}'"))
        }),
        None => Ok(default),
    }
}

fn parse_duration_secs(
    value: Option<String>,
    flag: &str,
    default: f32,
    allow_zero: bool,
) -> Result<f32, PerfScenarioConfigError> {
    let Some(value) = value else {
        return Ok(default);
    };
    let parsed = value.parse::<f32>().map_err(|_| {
        PerfScenarioConfigError(format!(
            "{flag} must be a finite number of seconds; got '{value}'"
        ))
    })?;
    let is_valid = parsed.is_finite() && (parsed >= 0.0) && (allow_zero || parsed > 0.0);
    if !is_valid {
        let constraint = if allow_zero {
            "at least 0"
        } else {
            "greater than 0"
        };
        return Err(PerfScenarioConfigError(format!(
            "{flag} must be finite and {constraint}; got '{value}'"
        )));
    }
    Ok(parsed)
}

fn value_from_args(args: &[String], flag: &str) -> Result<Option<String>, PerfScenarioConfigError> {
    let Some(index) = args.iter().position(|arg| arg == flag) else {
        return Ok(None);
    };
    args.get(index + 1)
        .cloned()
        .map(Some)
        .ok_or_else(|| PerfScenarioConfigError(format!("{flag} requires a value")))
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut mixed = value;
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_FIXED_AUDIT_TICKS, DEFAULT_FIXED_STEP_HZ, DEFAULT_FIXED_WARMUP_TICKS,
        PerfClockMode, PerfRandomStream, PerfScenarioConfig, splitmix64,
    };

    #[test]
    fn random_streams_are_stable_and_independent() {
        let config = PerfScenarioConfig {
            enabled: true,
            master_seed: 42,
            workload: super::PerfWorkload::Gather,
            size: super::PerfScenarioSize::Small,
            soul_count: 50,
            familiar_count: 4,
            render_mode: super::PerfRenderMode::Cpu,
            warmup_secs: 30.0,
            measure_secs: 60.0,
            output_dir: None,
            clock_mode: PerfClockMode::Realtime,
            fixed_step_hz: DEFAULT_FIXED_STEP_HZ,
            fixed_warmup_ticks: DEFAULT_FIXED_WARMUP_TICKS,
            fixed_audit_ticks: DEFAULT_FIXED_AUDIT_TICKS,
        };
        assert_eq!(
            config.stream_seed(PerfRandomStream::Souls),
            config.stream_seed(PerfRandomStream::Souls)
        );
        assert_ne!(
            config.stream_seed(PerfRandomStream::Souls),
            config.stream_seed(PerfRandomStream::Familiars)
        );
        assert_ne!(
            config.stream_seed(PerfRandomStream::SoulTraits),
            config.stream_seed(PerfRandomStream::FamiliarVoices)
        );
        assert!(config.omits_3d_scene_roots());
        let mut gpu_config = config.clone();
        gpu_config.render_mode = super::PerfRenderMode::Gpu;
        assert!(!gpu_config.omits_3d_scene_roots());
        assert_eq!(splitmix64(42), splitmix64(42));
    }

    #[test]
    fn duration_parser_rejects_invalid_measurement_window() {
        assert!(
            super::parse_duration_secs(Some("0".to_string()), "--perf-measure-secs", 60.0, false)
                .is_err()
        );
        assert!(
            super::parse_duration_secs(Some("NaN".to_string()), "--perf-warmup-secs", 30.0, true)
                .is_err()
        );
        assert_eq!(
            super::parse_duration_secs(Some("0".to_string()), "--perf-warmup-secs", 30.0, true)
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn fixed_clock_mode_is_explicit() {
        assert_eq!(PerfClockMode::parse("fixed"), Some(PerfClockMode::Fixed));
        assert_eq!(
            PerfClockMode::parse("realtime"),
            Some(PerfClockMode::Realtime)
        );
        assert_eq!(PerfClockMode::parse("auto"), None);
        assert_eq!(PerfClockMode::Fixed.as_str(), "fixed");
    }
}
