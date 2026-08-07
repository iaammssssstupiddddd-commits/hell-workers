use super::*;

mod parse;
use parse::*;

const DEFAULT_WARMUP_SECS: f32 = 30.0;
const DEFAULT_MEASURE_SECS: f32 = 60.0;
#[cfg(feature = "profiling")]
pub(super) const PERF_SUMMARY_SCHEMA_VERSION: u32 = 11;
#[cfg(feature = "profiling")]
pub(super) const PERF_DETERMINISM_SCHEMA_VERSION: u32 = 4;
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
    TaskDashboard,
    IndoorLight,
}

impl PerfWorkload {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "gather" => Some(Self::Gather),
            "path-door" => Some(Self::PathDoor),
            "construction" => Some(Self::Construction),
            "ui-gpu" => Some(Self::UiGpu),
            "task-dashboard" => Some(Self::TaskDashboard),
            "indoor-light" => Some(Self::IndoorLight),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gather => "gather",
            Self::PathDoor => "path-door",
            Self::Construction => "construction",
            Self::UiGpu => "ui-gpu",
            Self::TaskDashboard => "task-dashboard",
            Self::IndoorLight => "indoor-light",
        }
    }

    #[cfg(feature = "profiling")]
    pub(super) const fn has_automated_setup(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerfRttLightSelection {
    contract_id: &'static str,
    stage_id: &'static str,
    lane: &'static str,
}

impl PerfRttLightSelection {
    const CURRENT_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "current",
        lane: "static",
    };
    const CURRENT_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "current",
        lane: "behavior",
    };

    pub const fn contract_id(self) -> &'static str {
        self.contract_id
    }

    pub const fn stage_id(self) -> &'static str {
        self.stage_id
    }

    pub const fn lane(self) -> &'static str {
        self.lane
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PerfBehaviorCase {
    DoorStateV1,
    LoadNormalV1,
}

impl PerfBehaviorCase {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "door-state-v1" => Some(Self::DoorStateV1),
            "load-normal-v1" => Some(Self::LoadNormalV1),
            _ => None,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DoorStateV1 => "door-state-v1",
            Self::LoadNormalV1 => "load-normal-v1",
        }
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
pub enum PerfFamiliarPolicyMode {
    Baseline,
    Default,
    Disabled,
}

impl PerfFamiliarPolicyMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "baseline" => Some(Self::Baseline),
            "default" => Some(Self::Default),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Default => "default",
            Self::Disabled => "disabled",
        }
    }

    pub const fn uses_controlled_fixture(self) -> bool {
        !matches!(self, Self::Baseline)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfOperationDialogMode {
    Hidden,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfDashboardMode {
    Hidden,
    Visible,
    ActiveFilter,
}

impl PerfDashboardMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "hidden" => Some(Self::Hidden),
            "visible" => Some(Self::Visible),
            "active-filter" => Some(Self::ActiveFilter),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Visible => "visible",
            Self::ActiveFilter => "active-filter",
        }
    }
}

impl PerfOperationDialogMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "hidden" => Some(Self::Hidden),
            "open" => Some(Self::Open),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Open => "open",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PerfClockMode {
    Realtime,
    Fixed,
    FixedBehavior,
}

impl PerfClockMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "realtime" => Some(Self::Realtime),
            "fixed" => Some(Self::Fixed),
            "fixed-behavior" => Some(Self::FixedBehavior),
            _ => None,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Fixed => "fixed",
            Self::FixedBehavior => "fixed-behavior",
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
    pub familiar_policy_mode: PerfFamiliarPolicyMode,
    pub operation_dialog_mode: PerfOperationDialogMode,
    pub dashboard_mode: PerfDashboardMode,
    pub warmup_secs: f32,
    pub measure_secs: f32,
    pub output_dir: Option<PathBuf>,
    #[cfg(feature = "profiling")]
    renderdoc_capture: bool,
    rtt_light: Option<PerfRttLightSelection>,
    behavior_case: Option<PerfBehaviorCase>,
    window_width: Option<u32>,
    window_height: Option<u32>,
    window_scale_factor: Option<f32>,
    rtt_quality: Option<RttQualityPreset>,
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
            "gather|path-door|construction|ui-gpu|task-dashboard|indoor-light",
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
        let familiar_policy_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-familiar-policy", "HW_PERF_FAMILIAR_POLICY")?,
            "--perf-familiar-policy",
            "baseline|default|disabled",
            PerfFamiliarPolicyMode::parse,
            PerfFamiliarPolicyMode::Baseline,
        )?;
        let operation_dialog_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-operation-dialog", "HW_PERF_OPERATION_DIALOG")?,
            "--perf-operation-dialog",
            "hidden|open",
            PerfOperationDialogMode::parse,
            PerfOperationDialogMode::Hidden,
        )?;
        let dashboard_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-dashboard", "HW_PERF_DASHBOARD")?,
            "--perf-dashboard",
            "hidden|visible|active-filter",
            PerfDashboardMode::parse,
            PerfDashboardMode::Hidden,
        )?;
        let clock_mode = parse_value_or_default(
            value_from_args_or_env(&args, "--perf-clock", "HW_PERF_CLOCK")?,
            "--perf-clock",
            "realtime|fixed|fixed-behavior",
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
        if matches!(
            clock_mode,
            PerfClockMode::Fixed | PerfClockMode::FixedBehavior
        ) {
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
        if workload == PerfWorkload::IndoorLight
            && (soul_count, familiar_count) != (default_souls, default_familiars)
        {
            return Err(PerfScenarioConfigError(format!(
                "the indoor-light {} fixture requires exactly {default_souls} Souls and {default_familiars} Familiars",
                size.as_str()
            )));
        }
        let uses_b2_controlled_mode = familiar_policy_mode.uses_controlled_fixture()
            || matches!(operation_dialog_mode, PerfOperationDialogMode::Open);
        if uses_b2_controlled_mode
            && (!matches!(clock_mode, PerfClockMode::Fixed) || workload != PerfWorkload::Gather)
        {
            return Err(PerfScenarioConfigError(
                "--perf-familiar-policy default|disabled and --perf-operation-dialog open require the gather fixed-step audit"
                    .to_string(),
            ));
        }
        if familiar_policy_mode.uses_controlled_fixture()
            && (soul_count == 0 || familiar_count == 0)
        {
            return Err(PerfScenarioConfigError(
                "the controlled familiar policy fixture requires at least one Soul and one Familiar"
                    .to_string(),
            ));
        }
        if !matches!(dashboard_mode, PerfDashboardMode::Hidden)
            && workload != PerfWorkload::TaskDashboard
        {
            return Err(PerfScenarioConfigError(
                "--perf-dashboard visible|active-filter requires --perf-workload task-dashboard"
                    .to_string(),
            ));
        }
        if workload == PerfWorkload::TaskDashboard
            && (!matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden))
        {
            return Err(PerfScenarioConfigError(
                "the task-dashboard workload requires familiar policy baseline and operation dialog hidden"
                    .to_string(),
            ));
        }
        if workload == PerfWorkload::IndoorLight
            && (!matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden))
        {
            return Err(PerfScenarioConfigError(
                "the indoor-light workload requires familiar policy baseline, operation dialog hidden, and dashboard hidden"
                .to_string(),
            ));
        }
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
        let renderdoc_capture = has_flag(&args, "--perf-renderdoc-capture")
            || env::var("HW_PERF_RENDERDOC_CAPTURE").is_ok_and(|value| value == "1");
        let rtt_light = parse_rtt_light_selection(&args, workload)?;
        let behavior_case_value =
            value_from_args_or_env(&args, "--perf-behavior-case", "HW_PERF_BEHAVIOR_CASE")?;
        let behavior_case = match behavior_case_value {
            Some(value) => Some(PerfBehaviorCase::parse(&value).ok_or_else(|| {
                PerfScenarioConfigError(format!(
                    "--perf-behavior-case must be door-state-v1|load-normal-v1; got '{value}'"
                ))
            })?),
            None => None,
        };
        validate_behavior_lane(rtt_light, behavior_case, size, render_mode, clock_mode)?;
        let window_width = parse_optional_u32(
            value_from_args_or_env(&args, "--perf-window-width", "HW_PERF_WINDOW_WIDTH")?,
            "--perf-window-width",
        )?;
        let window_height = parse_optional_u32(
            value_from_args_or_env(&args, "--perf-window-height", "HW_PERF_WINDOW_HEIGHT")?,
            "--perf-window-height",
        )?;
        if (window_width.is_some()) != (window_height.is_some()) {
            return Err(PerfScenarioConfigError(
                "--perf-window-width and --perf-window-height must be provided together"
                    .to_string(),
            ));
        }
        if matches!(window_width, Some(0)) || matches!(window_height, Some(0)) {
            return Err(PerfScenarioConfigError(
                "--perf-window-width and --perf-window-height must be greater than 0".to_string(),
            ));
        }
        let window_scale_factor = parse_optional_positive_f32(
            value_from_args_or_env(
                &args,
                "--perf-window-scale-factor",
                "HW_PERF_WINDOW_SCALE_FACTOR",
            )?,
            "--perf-window-scale-factor",
        )?;
        let rtt_quality =
            match value_from_args_or_env(&args, "--perf-rtt-quality", "HW_PERF_RTT_QUALITY")? {
                Some(value) => Some(match value.as_str() {
                    "high" => RttQualityPreset::High,
                    "medium" => RttQualityPreset::Medium,
                    "low" => RttQualityPreset::Low,
                    _ => {
                        return Err(PerfScenarioConfigError(format!(
                            "--perf-rtt-quality must be one of high|medium|low; got '{value}'"
                        )));
                    }
                }),
                None => None,
            };
        if renderdoc_capture
            && (workload != PerfWorkload::IndoorLight
                || rtt_light != Some(PerfRttLightSelection::CURRENT_STATIC_V1)
                || size != PerfScenarioSize::Medium
                || render_mode != PerfRenderMode::Gpu
                || !matches!(clock_mode, PerfClockMode::Fixed)
                || output_dir.is_none()
                || window_width != Some(1920)
                || window_height != Some(1080)
                || window_scale_factor != Some(1.0)
                || rtt_quality != Some(RttQualityPreset::High))
        {
            return Err(PerfScenarioConfigError(
                "--perf-renderdoc-capture requires rtt-light-v1/current/static medium/gpu/fixed, an output directory, and the exact 1920x1080/scale-1/high window contract"
                    .to_string(),
            ));
        }

        Ok(Self {
            enabled,
            master_seed,
            workload,
            size,
            soul_count,
            familiar_count,
            render_mode,
            familiar_policy_mode,
            operation_dialog_mode,
            dashboard_mode,
            warmup_secs,
            measure_secs,
            output_dir,
            #[cfg(feature = "profiling")]
            renderdoc_capture,
            rtt_light,
            behavior_case,
            window_width,
            window_height,
            window_scale_factor,
            rtt_quality,
            clock_mode,
            fixed_step_hz,
            fixed_warmup_ticks,
            fixed_audit_ticks,
        })
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn requested_window_size(&self) -> Option<(u32, u32)> {
        if !self.enabled {
            return None;
        }
        match (self.window_width, self.window_height) {
            (Some(width), Some(height)) => Some((width, height)),
            _ => None,
        }
    }

    pub const fn rtt_light_selection(&self) -> Option<PerfRttLightSelection> {
        self.rtt_light
    }

    #[cfg(feature = "profiling")]
    pub(crate) const fn renderdoc_capture_enabled(&self) -> bool {
        self.enabled && self.renderdoc_capture
    }

    #[cfg(feature = "profiling")]
    pub(super) const fn behavior_case(&self) -> Option<PerfBehaviorCase> {
        self.behavior_case
    }

    pub const fn behavior_case_as_str(&self) -> Option<&'static str> {
        match self.behavior_case {
            Some(case) => Some(case.as_str()),
            None => None,
        }
    }

    pub const fn requested_window_scale_factor(&self) -> Option<f32> {
        if self.enabled {
            self.window_scale_factor
        } else {
            None
        }
    }

    pub const fn requested_rtt_quality(&self) -> Option<RttQualityPreset> {
        if self.enabled { self.rtt_quality } else { None }
    }

    pub const fn uses_fixed_timesteps(&self) -> bool {
        matches!(
            self.clock_mode,
            PerfClockMode::Fixed | PerfClockMode::FixedBehavior
        )
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

#[cfg(feature = "profiling")]
pub(crate) fn is_fixed_step_behavior(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| {
        config.enabled() && matches!(config.clock_mode, PerfClockMode::FixedBehavior)
    })
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_fixed_step_behavior(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_fixed_step_behavior(config)
}

#[cfg(feature = "profiling")]
/// 固定 step シナリオでは、初期 fixture を通常の Logic ゲートより先に適用する。
///
/// 監査開始時は `Time<Virtual>` を停止したままにするため、通常の `Logic`
/// system set に置かれた spawn consumer は実行できない。この条件は、固定
/// 監査・固定挙動の両方で専用経路と通常経路を相互排他的にするために使う。
pub(crate) fn is_fixed_step_scenario(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| config.enabled() && config.uses_fixed_timesteps())
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_fixed_step_scenario(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_fixed_step_scenario(config)
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_renderdoc_capture(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !config.is_some_and(|config| config.renderdoc_capture_enabled())
}

fn parse_rtt_light_selection(
    args: &[String],
    workload: PerfWorkload,
) -> Result<Option<PerfRttLightSelection>, PerfScenarioConfigError> {
    let contract = value_from_args_or_env(args, "--perf-contract", "HW_PERF_CONTRACT")?;
    let stage = value_from_args_or_env(args, "--perf-stage", "HW_PERF_STAGE")?;
    let lane = value_from_args_or_env(args, "--perf-lane", "HW_PERF_LANE")?;
    let any_selected = contract.is_some() || stage.is_some() || lane.is_some();

    if workload != PerfWorkload::IndoorLight {
        if any_selected {
            return Err(PerfScenarioConfigError(
                "--perf-contract, --perf-stage, and --perf-lane are reserved for --perf-workload indoor-light"
                    .to_string(),
            ));
        }
        return Ok(None);
    }

    let (Some(contract), Some(stage), Some(lane)) = (contract, stage, lane) else {
        return Err(PerfScenarioConfigError(
            "--perf-workload indoor-light requires --perf-contract rtt-light-v1 --perf-stage current --perf-lane static"
                .to_string(),
        ));
    };
    let selection = match (contract.as_str(), stage.as_str(), lane.as_str()) {
        ("rtt-light-v1", "current", "static") => PerfRttLightSelection::CURRENT_STATIC_V1,
        ("rtt-light-v1", "current", "behavior") => PerfRttLightSelection::CURRENT_BEHAVIOR_V1,
        _ => {
            return Err(PerfScenarioConfigError(format!(
                "this binary supports only rtt-light-v1/current/static|behavior; got {contract}/{stage}/{lane}"
            )));
        }
    };
    Ok(Some(selection))
}

fn validate_behavior_lane(
    rtt_light: Option<PerfRttLightSelection>,
    behavior_case: Option<PerfBehaviorCase>,
    size: PerfScenarioSize,
    render_mode: PerfRenderMode,
    clock_mode: PerfClockMode,
) -> Result<(), PerfScenarioConfigError> {
    match rtt_light.map(PerfRttLightSelection::lane) {
        Some("behavior") => {
            if behavior_case.is_none() {
                return Err(PerfScenarioConfigError(
                    "the rtt-light behavior lane requires --perf-behavior-case".to_string(),
                ));
            }
            if size != PerfScenarioSize::Small
                || render_mode != PerfRenderMode::Cpu
                || !matches!(clock_mode, PerfClockMode::FixedBehavior)
            {
                return Err(PerfScenarioConfigError(
                    "the rtt-light behavior lane requires small/cpu/fixed-behavior".to_string(),
                ));
            }
        }
        Some(_) => {
            if behavior_case.is_some() {
                return Err(PerfScenarioConfigError(
                    "--perf-behavior-case is only valid for --perf-lane behavior".to_string(),
                ));
            }
            if matches!(clock_mode, PerfClockMode::FixedBehavior) {
                return Err(PerfScenarioConfigError(
                    "--perf-clock fixed-behavior is only valid for --perf-lane behavior"
                        .to_string(),
                ));
            }
        }
        None if behavior_case.is_some() || matches!(clock_mode, PerfClockMode::FixedBehavior) => {
            return Err(PerfScenarioConfigError(
                "behavior selection is reserved for the rtt-light behavior lane".to_string(),
            ));
        }
        None => {}
    }
    Ok(())
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
            familiar_policy_mode: PerfFamiliarPolicyMode::Baseline,
            operation_dialog_mode: PerfOperationDialogMode::Hidden,
            dashboard_mode: PerfDashboardMode::Hidden,
            warmup_secs: DEFAULT_WARMUP_SECS,
            measure_secs: DEFAULT_MEASURE_SECS,
            output_dir: None,
            #[cfg(feature = "profiling")]
            renderdoc_capture: false,
            rtt_light: None,
            behavior_case: None,
            window_width: None,
            window_height: None,
            window_scale_factor: None,
            rtt_quality: None,
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
