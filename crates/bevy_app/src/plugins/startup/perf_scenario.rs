//! 再現可能なパフォーマンス計測シナリオの構成と採取。

#[cfg(feature = "profiling")]
use crate::entities::damned_soul::DamnedSoul;
#[cfg(feature = "profiling")]
use crate::entities::familiar::Familiar;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand, FamiliarOperation};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, Rock, TaskSlots, Tree, WorkType};
use crate::{Render3dVisible, RenderPerfToggles};
use bevy::prelude::*;
#[cfg(feature = "profiling")]
use hw_familiar_ai::familiar_ai::decide::resources::FamiliarDelegationPerfMetrics;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::env;

const DEFAULT_WARMUP_SECS: f32 = 30.0;
const DEFAULT_MEASURE_SECS: f32 = 60.0;

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

    const fn has_automated_setup(self) -> bool {
        matches!(self, Self::Gather)
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
}

impl PerfScenarioConfig {
    pub fn from_process() -> Self {
        let args = env::args().collect::<Vec<_>>();
        let enabled = has_flag(&args, "--perf-scenario")
            || env::var("HW_PERF_SCENARIO").is_ok_and(|value| value == "1");
        let workload = value_from_args_or_env(&args, "--perf-workload", "HW_PERF_WORKLOAD")
            .as_deref()
            .and_then(PerfWorkload::parse)
            .unwrap_or(PerfWorkload::Gather);
        let size = value_from_args_or_env(&args, "--perf-size", "HW_PERF_SIZE")
            .as_deref()
            .and_then(PerfScenarioSize::parse)
            .unwrap_or(PerfScenarioSize::Medium);
        let render_mode = value_from_args_or_env(&args, "--perf-render", "HW_PERF_RENDER")
            .as_deref()
            .and_then(PerfRenderMode::parse)
            .unwrap_or(PerfRenderMode::Gpu);
        let (default_souls, default_familiars) = size.population();
        let soul_count = parse_u32_from_args_or_env(&args, "--spawn-souls", "HW_SPAWN_SOULS")
            .unwrap_or(default_souls);
        let familiar_count =
            parse_u32_from_args_or_env(&args, "--spawn-familiars", "HW_SPAWN_FAMILIARS")
                .unwrap_or(default_familiars);
        let master_seed = parse_u64_from_args_or_env(&args, "--perf-seed", "HW_PERF_SEED")
            .or_else(|| env::var("HELL_WORKERS_WORLDGEN_SEED").ok()?.parse().ok())
            .unwrap_or_else(rand::random);

        Self {
            enabled,
            master_seed,
            workload,
            size,
            soul_count,
            familiar_count,
            render_mode,
            warmup_secs: parse_f32_env("HW_PERF_WARMUP_SECS").unwrap_or(DEFAULT_WARMUP_SECS),
            measure_secs: parse_f32_env("HW_PERF_MEASURE_SECS").unwrap_or(DEFAULT_MEASURE_SECS),
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
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

impl Default for PerfScenarioConfig {
    fn default() -> Self {
        Self::from_process()
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

#[derive(Resource, Default)]
pub(crate) struct PerfScenarioApplied(pub(crate) bool);

pub fn setup_perf_scenario_if_enabled(
    config: Res<PerfScenarioConfig>,
    mut commands: Commands,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: Query<Entity, With<Tree>>,
    q_rocks: Query<Entity, With<Rock>>,
) {
    if !config.enabled() || !config.workload.has_automated_setup() {
        return;
    }

    configure_gather_baseline(&mut commands, &mut q_familiars, &q_trees, &q_rocks);
}

pub fn setup_perf_scenario_runtime_if_enabled(
    config: Res<PerfScenarioConfig>,
    mut commands: Commands,
    mut applied: ResMut<PerfScenarioApplied>,
    mut q_familiars: Query<(Entity, &mut ActiveCommand, &mut FamiliarOperation)>,
    q_trees: Query<Entity, With<Tree>>,
    q_rocks: Query<Entity, With<Rock>>,
) {
    if applied.0
        || !config.enabled()
        || !config.workload.has_automated_setup()
        || q_familiars.is_empty()
    {
        return;
    }

    configure_gather_baseline(&mut commands, &mut q_familiars, &q_trees, &q_rocks);
    applied.0 = true;
}

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
#[derive(Resource, Default)]
pub(crate) struct PerfCapture {
    phase: PerfCapturePhase,
    elapsed_secs: f32,
    frame_times_ms: Vec<f64>,
    checksum: Option<PerfScenarioChecksum>,
}

#[cfg(feature = "profiling")]
#[derive(Default)]
enum PerfCapturePhase {
    #[default]
    WaitingForScenario,
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

#[cfg(feature = "profiling")]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PerfChecksumQueries<'w, 's> {
    souls: Query<'w, 's, (Entity, &'static Transform), With<DamnedSoul>>,
    familiars: Query<'w, 's, (Entity, &'static Transform), With<Familiar>>,
    designations: Query<'w, 's, Entity, With<Designation>>,
}

#[cfg(feature = "profiling")]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PerfCaptureParams<'w, 's> {
    config: Res<'w, PerfScenarioConfig>,
    applied: Res<'w, PerfScenarioApplied>,
    time: Res<'w, Time<Virtual>>,
    diagnostics: Option<Res<'w, bevy::diagnostic::DiagnosticsStore>>,
    checksum_queries: PerfChecksumQueries<'w, 's>,
    familiar_metrics: ResMut<'w, FamiliarDelegationPerfMetrics>,
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
        PerfCapturePhase::WaitingForScenario => {
            if !params.config.workload.has_automated_setup() {
                error!(
                    "PERF_CAPTURE: workload '{}' has no automated setup yet; use gather",
                    params.config.workload.as_str()
                );
                capture.phase = PerfCapturePhase::Finished;
                exit.write(AppExit::error());
                return;
            }
            if !params.applied.0 {
                return;
            }
            capture.checksum = Some(calculate_checksum(&params.checksum_queries));
            capture.phase = PerfCapturePhase::Warmup;
            capture.elapsed_secs = 0.0;
        }
        PerfCapturePhase::Warmup => {
            capture.elapsed_secs += params.time.delta_secs();
            if capture.elapsed_secs >= params.config.warmup_secs {
                capture.phase = PerfCapturePhase::Measure;
                capture.elapsed_secs = 0.0;
                capture.frame_times_ms.clear();
                *params.familiar_metrics = FamiliarDelegationPerfMetrics::default();
            }
        }
        PerfCapturePhase::Measure => {
            capture.elapsed_secs += params.time.delta_secs();
            if let Some(frame_time_ms) =
                params.diagnostics.as_deref().and_then(latest_frame_time_ms)
            {
                capture.frame_times_ms.push(frame_time_ms);
            }
            if capture.elapsed_secs >= params.config.measure_secs {
                capture.phase = PerfCapturePhase::Flush;
            }
        }
        PerfCapturePhase::Flush => {
            if let Some(checksum) = capture.checksum
                && let Err(error) = write_perf_capture(
                    &params.config,
                    checksum,
                    &capture.frame_times_ms,
                    &params.familiar_metrics,
                )
            {
                error!("PERF_CAPTURE: failed to write CSV: {error}");
            }
            capture.phase = PerfCapturePhase::Finished;
            exit.write(AppExit::Success);
        }
        PerfCapturePhase::Finished => {}
    }
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
fn checksum_position(transform: &Transform) -> (i64, i64) {
    (
        (transform.translation.x * 100.0).round() as i64,
        (transform.translation.y * 100.0).round() as i64,
    )
}

#[cfg(feature = "profiling")]
fn write_perf_capture(
    config: &PerfScenarioConfig,
    checksum: PerfScenarioChecksum,
    samples: &[f64],
    familiar_metrics: &FamiliarDelegationPerfMetrics,
) -> std::io::Result<()> {
    let directory = format!(
        "target/perf/{}-{}-{}-seed-{}",
        config.workload.as_str(),
        config.size.as_str(),
        config.render_mode.as_str(),
        config.master_seed
    );
    std::fs::create_dir_all(&directory)?;

    let mut frame_csv = String::from("frame_index,frame_time_ms\n");
    for (index, frame_time_ms) in samples.iter().enumerate() {
        frame_csv.push_str(&format!("{index},{frame_time_ms:.6}\n"));
    }
    std::fs::write(format!("{directory}/frames.csv"), frame_csv)?;

    let (p50, p95, p99) = percentile_summary(samples);
    let summary = format!(
        "seed,workload,size,render,souls,familiars,designations,state_checksum,samples,p50_ms,p95_ms,p99_ms,delegation_latest_ms,delegation_familiars_processed,source_selector_calls,source_selector_scanned_items,reachable_with_cache_calls\n{},{},{},{},{},{},{},{:016x},{},{p50:.6},{p95:.6},{p99:.6},{:.6},{},{},{},{}\n",
        config.master_seed,
        config.workload.as_str(),
        config.size.as_str(),
        config.render_mode.as_str(),
        checksum.souls,
        checksum.familiars,
        checksum.designations,
        checksum.value,
        samples.len(),
        familiar_metrics.latest_elapsed_ms,
        familiar_metrics.familiars_processed,
        familiar_metrics.source_selector_calls,
        familiar_metrics.source_selector_scanned_items,
        familiar_metrics.reachable_with_cache_calls,
    );
    std::fs::write(format!("{directory}/summary.csv"), summary)?;
    eprintln!(
        "PERF_CAPTURE: wrote {} samples to {directory} (p50={p50:.3}ms p95={p95:.3}ms p99={p99:.3}ms checksum={:016x})",
        samples.len(),
        checksum.value
    );
    Ok(())
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn value_from_args_or_env(args: &[String], flag: &str, env_key: &str) -> Option<String> {
    value_from_args(args, flag).or_else(|| env::var(env_key).ok())
}

fn parse_u32_from_args_or_env(args: &[String], flag: &str, env_key: &str) -> Option<u32> {
    value_from_args_or_env(args, flag, env_key)?.parse().ok()
}

fn parse_u64_from_args_or_env(args: &[String], flag: &str, env_key: &str) -> Option<u64> {
    value_from_args_or_env(args, flag, env_key)?.parse().ok()
}

fn parse_f32_env(env_key: &str) -> Option<f32> {
    env::var(env_key).ok()?.parse().ok()
}

fn value_from_args(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
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
    use super::{PerfRandomStream, PerfScenarioConfig, splitmix64};

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
        assert_eq!(splitmix64(42), splitmix64(42));
    }
}
