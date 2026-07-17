use super::*;

mod parse;
use parse::*;

const DEFAULT_WARMUP_SECS: f32 = 30.0;
const DEFAULT_MEASURE_SECS: f32 = 60.0;
#[cfg(feature = "profiling")]
pub(super) const PERF_SUMMARY_SCHEMA_VERSION: u32 = 10;
pub(super) const FIXED_STEP_AUDIT_EARLY_UPDATE_TICKS: [u64; 4] = [1, 8, 32, 128];
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
    pub(super) const fn has_automated_setup(self) -> bool {
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
    pub(super) const fn fixed_audit_end_tick(&self) -> u64 {
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

#[cfg(test)]
mod tests;
