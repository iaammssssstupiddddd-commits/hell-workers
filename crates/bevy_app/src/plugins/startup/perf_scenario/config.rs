use super::*;

mod parse;
mod validated;
use parse::*;
use validated::{PerfCommonConfig, PerfScenarioState, ValidatedWorkload};

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
    DreamUiBurst,
    IndoorLight,
    WallDensity,
    DoorDensity,
    BuildingArtStatic,
    BuildingArtActive,
    Deconstruction,
    SaveTransaction,
}

impl PerfWorkload {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "gather" => Some(Self::Gather),
            "path-door" => Some(Self::PathDoor),
            "construction" => Some(Self::Construction),
            "ui-gpu" => Some(Self::UiGpu),
            "task-dashboard" => Some(Self::TaskDashboard),
            "dream-ui-burst" => Some(Self::DreamUiBurst),
            "indoor-light" => Some(Self::IndoorLight),
            "wall-density" => Some(Self::WallDensity),
            "door-density" => Some(Self::DoorDensity),
            "building-art-static" => Some(Self::BuildingArtStatic),
            "building-art-active" => Some(Self::BuildingArtActive),
            "deconstruction" => Some(Self::Deconstruction),
            "save-transaction" => Some(Self::SaveTransaction),
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
            Self::DreamUiBurst => "dream-ui-burst",
            Self::IndoorLight => "indoor-light",
            Self::WallDensity => "wall-density",
            Self::DoorDensity => "door-density",
            Self::BuildingArtStatic => "building-art-static",
            Self::BuildingArtActive => "building-art-active",
            Self::Deconstruction => "deconstruction",
            Self::SaveTransaction => "save-transaction",
        }
    }

    #[cfg(feature = "profiling")]
    pub(super) const fn has_automated_setup(self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfWallPhase {
    Completed,
    Provisional,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfWallPresentation {
    Production,
    FallbackControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfDoorPresentation {
    Production,
    FallbackControl,
}

impl PerfDoorPresentation {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "production" => Some(Self::Production),
            "fallback-control" => Some(Self::FallbackControl),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::FallbackControl => "fallback-control",
        }
    }
}

impl PerfWallPresentation {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "production" => Some(Self::Production),
            "fallback-control" => Some(Self::FallbackControl),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::FallbackControl => "fallback-control",
        }
    }
}

impl PerfWallPhase {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "completed" => Some(Self::Completed),
            "provisional" => Some(Self::Provisional),
            "mixed" => Some(Self::Mixed),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Provisional => "provisional",
            Self::Mixed => "mixed",
        }
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
    const P01_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p01",
        lane: "static",
    };
    const P01_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p01",
        lane: "behavior",
    };
    const P02_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p02",
        lane: "static",
    };
    const P02_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p02",
        lane: "behavior",
    };
    const P03_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p03",
        lane: "static",
    };
    const P03_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p03",
        lane: "behavior",
    };
    const P03_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p03",
        lane: "field-core",
    };
    const P04_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p04",
        lane: "static",
    };
    const P04_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p04",
        lane: "behavior",
    };
    const P04_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p04",
        lane: "field-core",
    };
    const P05_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p05",
        lane: "static",
    };
    const P05_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p05",
        lane: "behavior",
    };
    const P05_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p05",
        lane: "field-core",
    };
    const P06_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p06",
        lane: "static",
    };
    const P06_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p06",
        lane: "behavior",
    };
    const P06_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p06",
        lane: "field-core",
    };
    const P07_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p07",
        lane: "static",
    };
    const P07_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p07",
        lane: "behavior",
    };
    const P07_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p07",
        lane: "field-core",
    };
    const P07_CONSUMER_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p07",
        lane: "consumer-core",
    };
    const P08_STATIC_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p08",
        lane: "static",
    };
    const P08_BEHAVIOR_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p08",
        lane: "behavior",
    };
    const P08_FIELD_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p08",
        lane: "field-core",
    };
    const P08_CONSUMER_CORE_V1: Self = Self {
        contract_id: "rtt-light-v1",
        stage_id: "p08",
        lane: "consumer-core",
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

    pub fn uses_p02_presentation(self) -> bool {
        matches!(
            self.stage_id,
            "p02" | "p03" | "p04" | "p05" | "p06" | "p07" | "p08"
        )
    }

    pub fn uses_runtime_field(self) -> bool {
        matches!(self.stage_id, "p04" | "p05" | "p06" | "p07" | "p08")
    }

    pub fn uses_gpu_light_field(self) -> bool {
        matches!(self.stage_id, "p06" | "p08")
    }

    pub fn uses_cpu_consumers(self) -> bool {
        matches!(self.stage_id, "p07" | "p08")
    }

    #[cfg(any(feature = "profiling-renderdoc", test))]
    fn supports_renderdoc_capture(self) -> bool {
        self.lane == "static"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PerfBehaviorCase {
    DoorStateV1,
    LoadNormalV1,
    LoadPreflightRejectV1,
    LoadRollbackV1,
    LoadRecoveryOnlyV1,
    LoadRecoveryFailedV1,
    LoadDuplicateResetV1,
}

impl PerfBehaviorCase {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "door-state-v1" => Some(Self::DoorStateV1),
            "load-normal-v1" => Some(Self::LoadNormalV1),
            "load-preflight-reject-v1" => Some(Self::LoadPreflightRejectV1),
            "load-rollback-v1" => Some(Self::LoadRollbackV1),
            "load-recovery-only-v1" => Some(Self::LoadRecoveryOnlyV1),
            "load-recovery-failed-v1" => Some(Self::LoadRecoveryFailedV1),
            "load-duplicate-reset-v1" => Some(Self::LoadDuplicateResetV1),
            _ => None,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DoorStateV1 => "door-state-v1",
            Self::LoadNormalV1 => "load-normal-v1",
            Self::LoadPreflightRejectV1 => "load-preflight-reject-v1",
            Self::LoadRollbackV1 => "load-rollback-v1",
            Self::LoadRecoveryOnlyV1 => "load-recovery-only-v1",
            Self::LoadRecoveryFailedV1 => "load-recovery-failed-v1",
            Self::LoadDuplicateResetV1 => "load-duplicate-reset-v1",
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
    common: PerfCommonConfig,
    state: PerfScenarioState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfScenarioConfigError(String);

/// Validates the paired Wall art zoom keys. The farthest zoom-out is the only
/// selectable value; its absence keeps the standard gallery zoom.
pub(super) fn wall_art_zoom_selection(
    flag: Option<&str>,
    environment: Option<&str>,
    wall_actual_window: bool,
) -> Result<bool, PerfScenarioConfigError> {
    if flag != environment {
        return Err(PerfScenarioConfigError(
            "--perf-wall-art-zoom and HW_WALL_ART_ZOOM must be paired".to_string(),
        ));
    }
    let Some(zoom) = flag else {
        return Ok(false);
    };
    if zoom != "farthest" {
        return Err(PerfScenarioConfigError(format!(
            "--perf-wall-art-zoom must be farthest when present; got '{zoom}'"
        )));
    }
    if !wall_actual_window {
        return Err(PerfScenarioConfigError(
            "Wall art zoom requires the current-Wall actual-window profile".to_string(),
        ));
    }
    Ok(true)
}

impl fmt::Display for PerfScenarioConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PerfScenarioConfigError {}

impl PerfScenarioConfig {
    pub fn try_from_process() -> Result<Self, PerfScenarioConfigError> {
        let args = env::args().collect::<Vec<_>>();
        Self::try_from_input(
            &PerfConfigInput {
                args: &args,
                environment: &|key| env::var_os(key),
            },
            rand::random,
        )
    }

    fn try_from_input(
        input: &PerfConfigInput<'_>,
        seed: impl FnOnce() -> u64,
    ) -> Result<Self, PerfScenarioConfigError> {
        let args = input.args;
        let enabled = has_flag(args, "--perf-scenario")
            || input
                .env_text("HW_PERF_SCENARIO")
                .is_some_and(|value| value == "1");

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
            input.value("--perf-workload", "HW_PERF_WORKLOAD")?,
            "--perf-workload",
            "gather|path-door|construction|ui-gpu|task-dashboard|dream-ui-burst|indoor-light|wall-density|door-density|building-art-static|building-art-active|deconstruction|save-transaction",
            PerfWorkload::parse,
            PerfWorkload::Gather,
        )?;
        let size = parse_value_or_default(
            input.value("--perf-size", "HW_PERF_SIZE")?,
            "--perf-size",
            "small|medium|large",
            PerfScenarioSize::parse,
            PerfScenarioSize::Medium,
        )?;
        let render_mode = parse_value_or_default(
            input.value("--perf-render", "HW_PERF_RENDER")?,
            "--perf-render",
            "cpu|gpu",
            PerfRenderMode::parse,
            PerfRenderMode::Gpu,
        )?;
        let familiar_policy_mode = parse_value_or_default(
            input.value("--perf-familiar-policy", "HW_PERF_FAMILIAR_POLICY")?,
            "--perf-familiar-policy",
            "baseline|default|disabled",
            PerfFamiliarPolicyMode::parse,
            PerfFamiliarPolicyMode::Baseline,
        )?;
        let operation_dialog_mode = parse_value_or_default(
            input.value("--perf-operation-dialog", "HW_PERF_OPERATION_DIALOG")?,
            "--perf-operation-dialog",
            "hidden|open",
            PerfOperationDialogMode::parse,
            PerfOperationDialogMode::Hidden,
        )?;
        let dashboard_mode = parse_value_or_default(
            input.value("--perf-dashboard", "HW_PERF_DASHBOARD")?,
            "--perf-dashboard",
            "hidden|visible|active-filter",
            PerfDashboardMode::parse,
            PerfDashboardMode::Hidden,
        )?;
        let clock_mode = parse_value_or_default(
            input.value("--perf-clock", "HW_PERF_CLOCK")?,
            "--perf-clock",
            "realtime|fixed|fixed-behavior",
            PerfClockMode::parse,
            PerfClockMode::Realtime,
        )?;
        let fixed_step_hz = parse_u32_value_or_default(
            input.value("--perf-fixed-hz", "HW_PERF_FIXED_HZ")?,
            "--perf-fixed-hz",
            DEFAULT_FIXED_STEP_HZ,
        )?;
        let fixed_warmup_ticks = parse_u64_value_or_default(
            input.value("--perf-warmup-ticks", "HW_PERF_WARMUP_TICKS")?,
            "--perf-warmup-ticks",
            DEFAULT_FIXED_WARMUP_TICKS,
        )?;
        let fixed_audit_ticks = parse_u64_value_or_default(
            input.value("--perf-audit-ticks", "HW_PERF_AUDIT_TICKS")?,
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
            input.value("--spawn-souls", "HW_SPAWN_SOULS")?,
            "--spawn-souls",
            default_souls,
        )?;
        let familiar_count = parse_u32_value_or_default(
            input.value("--spawn-familiars", "HW_SPAWN_FAMILIARS")?,
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
        if workload == PerfWorkload::DreamUiBurst
            && (size != PerfScenarioSize::Small
                || render_mode != PerfRenderMode::Cpu
                || (soul_count, familiar_count) != (default_souls, default_familiars)
                || !matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden))
        {
            return Err(PerfScenarioConfigError(
                "the dream-ui-burst workload requires the default small/cpu population, familiar policy baseline, operation dialog hidden, and dashboard hidden"
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
        if workload == PerfWorkload::Deconstruction
            && (size != PerfScenarioSize::Medium
                || render_mode != PerfRenderMode::Cpu
                || !matches!(clock_mode, PerfClockMode::Fixed)
                || !matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden))
        {
            return Err(PerfScenarioConfigError(
                "the deconstruction workload requires medium/cpu/fixed, familiar policy baseline, operation dialog hidden, and dashboard hidden"
                    .to_string(),
            ));
        }
        if workload == PerfWorkload::SaveTransaction
            && (!matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden)
                || matches!(
                    clock_mode,
                    PerfClockMode::Fixed | PerfClockMode::FixedBehavior
                ))
        {
            return Err(PerfScenarioConfigError(
                "the save-transaction workload requires familiar policy baseline, operation dialog hidden, dashboard hidden, and realtime clock"
                    .to_string(),
            ));
        }
        if workload == PerfWorkload::SaveTransaction {
            let runtime_root = input
                .env_os("HW_PERF_SAVE_RUNTIME_ROOT")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute() && !path.as_os_str().is_empty());
            if runtime_root.is_none() {
                return Err(PerfScenarioConfigError(
                    "save-transaction requires an absolute HW_PERF_SAVE_RUNTIME_ROOT".to_string(),
                ));
            }
        }
        let master_seed = parse_u64_value_or_random(
            input
                .value("--perf-seed", "HW_PERF_SEED")?
                .or_else(|| input.env_text("HELL_WORKERS_WORLDGEN_SEED")),
            "--perf-seed",
            seed,
        )?;
        let warmup_secs = parse_duration_secs(
            input.value("--perf-warmup-secs", "HW_PERF_WARMUP_SECS")?,
            "--perf-warmup-secs",
            DEFAULT_WARMUP_SECS,
            true,
        )?;
        let measure_secs = parse_duration_secs(
            input.value("--perf-measure-secs", "HW_PERF_MEASURE_SECS")?,
            "--perf-measure-secs",
            DEFAULT_MEASURE_SECS,
            false,
        )?;
        let output_dir = input
            .value("--perf-output-dir", "HW_PERF_OUTPUT_DIR")?
            .map(PathBuf::from)
            .filter(|path| !path.as_os_str().is_empty());
        let renderdoc_requested = has_flag(args, "--perf-renderdoc-capture")
            || input
                .env_text("HW_PERF_RENDERDOC_CAPTURE")
                .is_some_and(|value| value == "1");
        #[cfg(feature = "profiling-renderdoc")]
        let renderdoc_capture = resolve_renderdoc_capture(renderdoc_requested)?;
        #[cfg(not(feature = "profiling-renderdoc"))]
        resolve_renderdoc_capture(renderdoc_requested)?;
        let rtt_light = parse_rtt_light_selection(input, workload)?;
        let behavior_case_value = input.value("--perf-behavior-case", "HW_PERF_BEHAVIOR_CASE")?;
        let behavior_case = match behavior_case_value {
            Some(value) => Some(PerfBehaviorCase::parse(&value).ok_or_else(|| {
                PerfScenarioConfigError(format!(
                    "--perf-behavior-case must be door-state-v1|load-normal-v1; got '{value}'"
                ))
            })?),
            None => None,
        };
        let wall_phase_value = input.value("--perf-wall-phase", "HW_PERF_WALL_PHASE")?;
        let wall_phase = match wall_phase_value {
            Some(value) if workload == PerfWorkload::WallDensity => {
                Some(PerfWallPhase::parse(&value).ok_or_else(|| {
                    PerfScenarioConfigError(format!(
                        "--perf-wall-phase must be completed|provisional|mixed; got '{value}'"
                    ))
                })?)
            }
            Some(_) => {
                return Err(PerfScenarioConfigError(
                    "--perf-wall-phase is reserved for --perf-workload wall-density".to_string(),
                ));
            }
            None if workload == PerfWorkload::WallDensity => {
                return Err(PerfScenarioConfigError(
                    "--perf-workload wall-density requires --perf-wall-phase completed|provisional|mixed"
                        .to_string(),
                ));
            }
            None => None,
        };
        let wall_presentation_flag = value_from_args(args, "--perf-wall-presentation")?;
        let wall_presentation_environment = input.env_text("HW_WALL_PERF_PRESENTATION");
        if wall_presentation_flag != wall_presentation_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-presentation and HW_WALL_PERF_PRESENTATION must be paired and equal"
                    .to_string(),
            ));
        }
        let wall_presentation = wall_presentation_flag
            .map(|value| {
                PerfWallPresentation::parse(&value).ok_or_else(|| {
                    PerfScenarioConfigError(format!(
                        "--perf-wall-presentation must be production|fallback-control; got '{value}'"
                    ))
                })
            })
            .transpose()?;
        let door_presentation_flag = value_from_args(args, "--perf-door-presentation")?;
        let door_presentation_environment = input.env_text("HW_DOOR_PERF_PRESENTATION");
        if door_presentation_flag != door_presentation_environment {
            return Err(PerfScenarioConfigError(
                "--perf-door-presentation and HW_DOOR_PERF_PRESENTATION must be paired and equal"
                    .to_string(),
            ));
        }
        let door_presentation = door_presentation_flag
            .map(|value| {
                PerfDoorPresentation::parse(&value).ok_or_else(|| {
                    PerfScenarioConfigError(format!(
                        "--perf-door-presentation must be production|fallback-control; got '{value}'"
                    ))
                })
            })
            .transpose()?;
        let wall_door_joint_flag = has_flag(args, "--perf-wall-door-joint-actual-window");
        let wall_door_joint_environment = input
            .env_text("HW_WALL_DOOR_JOINT_ACTUAL_WINDOW")
            .is_some_and(|value| value == "1");
        if wall_door_joint_flag != wall_door_joint_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-door-joint-actual-window and HW_WALL_DOOR_JOINT_ACTUAL_WINDOW=1 must be paired"
                    .to_string(),
            ));
        }
        let wall_door_joint_actual_window = wall_door_joint_flag && wall_door_joint_environment;
        let joint_release = has_flag(args, "--perf-wall-door-joint-release");
        if joint_release
            != input
                .env_text("HW_WALL_DOOR_JOINT_RELEASE")
                .is_some_and(|value| value == "1")
            || (joint_release && !wall_door_joint_actual_window)
        {
            return Err(PerfScenarioConfigError(
                "Wall/Door joint release requires the paired release flag/environment and joint actual-window profile"
                    .to_string(),
            ));
        }
        let wall_actual_window_flag = has_flag(args, "--perf-wall-actual-window");
        let wall_actual_window_environment = input
            .env_text("HW_WALL_ART_ACTUAL_WINDOW")
            .is_some_and(|value| value == "1");
        if wall_actual_window_flag != wall_actual_window_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-actual-window and HW_WALL_ART_ACTUAL_WINDOW=1 must be paired"
                    .to_string(),
            ));
        }
        let wall_actual_window = wall_actual_window_flag && wall_actual_window_environment;
        let wall_art_preview = input
            .env_text("HW_WALL_ART_PREVIEW")
            .is_some_and(|value| value == "1");
        let wall_art_matrix_flag = has_flag(args, "--perf-wall-art-matrix");
        let wall_art_matrix_environment = input
            .env_text("HW_WALL_ART_MATRIX")
            .is_some_and(|value| value == "1");
        if wall_art_matrix_flag != wall_art_matrix_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-art-matrix and HW_WALL_ART_MATRIX=1 must be paired".to_string(),
            ));
        }
        let wall_art_matrix = wall_art_matrix_flag && wall_art_matrix_environment;
        if wall_art_matrix && !wall_actual_window {
            return Err(PerfScenarioConfigError(
                "Wall art matrix requires the current-Wall actual-window profile".to_string(),
            ));
        }
        let wall_formwork_acceptance_flag = has_flag(args, "--perf-wall-formwork-acceptance");
        let wall_formwork_acceptance_environment = input
            .env_text("HW_WALL_FORMWORK_ACCEPTANCE")
            .is_some_and(|value| value == "1");
        if wall_formwork_acceptance_flag != wall_formwork_acceptance_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-formwork-acceptance and HW_WALL_FORMWORK_ACCEPTANCE=1 must be paired"
                    .to_string(),
            ));
        }
        let wall_formwork_acceptance =
            wall_formwork_acceptance_flag && wall_formwork_acceptance_environment;
        let wall_candidate_requested = input
            .env_text("HW_WALL_CANDIDATE")
            .is_some_and(|value| value == "1");
        if wall_formwork_acceptance
            && (!wall_actual_window
                || !wall_art_matrix
                || wall_art_preview
                || !wall_candidate_requested
                || wall_phase != Some(PerfWallPhase::Mixed))
        {
            return Err(PerfScenarioConfigError(
                "Wall formwork acceptance requires the formal isolated-candidate mixed current-Wall matrix"
                    .to_string(),
            ));
        }
        wall_art_zoom_selection(
            value_from_args(args, "--perf-wall-art-zoom")?.as_deref(),
            input.env_text("HW_WALL_ART_ZOOM").as_deref(),
            wall_actual_window,
        )?;
        let wall_color_actual_window_flag = has_flag(args, "--perf-wall-color-actual-window");
        let wall_color_actual_window_environment = input
            .env_text("HW_WALL_COLOR_ACTUAL_WINDOW")
            .is_some_and(|value| value == "1");
        if wall_color_actual_window_flag != wall_color_actual_window_environment {
            return Err(PerfScenarioConfigError(
                "--perf-wall-color-actual-window and HW_WALL_COLOR_ACTUAL_WINDOW=1 must be paired"
                    .to_string(),
            ));
        }
        let wall_color_actual_window =
            wall_color_actual_window_flag && wall_color_actual_window_environment;
        if wall_actual_window && wall_color_actual_window {
            return Err(PerfScenarioConfigError(
                "Wall current-visual and color-board actual-window profiles are mutually exclusive"
                    .to_string(),
            ));
        }
        if wall_art_preview
            && !wall_actual_window_phase_matches(wall_actual_window, false, true, false, wall_phase)
        {
            return Err(PerfScenarioConfigError(
                "Wall art preview requires the completed or provisional current-Wall actual-window profile"
                    .to_string(),
            ));
        }
        if (wall_actual_window || wall_color_actual_window)
            && (workload != PerfWorkload::WallDensity
                || size
                    != if wall_formwork_acceptance {
                        PerfScenarioSize::Medium
                    } else {
                        PerfScenarioSize::Small
                    }
                || !wall_actual_window_phase_matches(
                    wall_actual_window,
                    wall_color_actual_window,
                    wall_art_preview,
                    wall_formwork_acceptance,
                    wall_phase,
                ))
        {
            return Err(PerfScenarioConfigError(
                "Wall actual-window calibration requires wall-density/small and its authorized phase"
                    .to_string(),
            ));
        }
        if wall_presentation.is_some()
            && (workload != PerfWorkload::WallDensity
                || wall_actual_window
                || wall_color_actual_window
                || wall_art_matrix)
        {
            return Err(PerfScenarioConfigError(
                "Wall performance presentation selection is reserved for the formal wall-density profile"
                    .to_string(),
            ));
        }
        if wall_phase == Some(PerfWallPhase::Mixed)
            && !wall_formwork_acceptance
            && (wall_presentation.is_none() || size != PerfScenarioSize::Medium)
        {
            return Err(PerfScenarioConfigError(
                "Wall mixed density requires the formal performance presentation and medium size"
                    .to_string(),
            ));
        }
        if door_presentation.is_some() && workload != PerfWorkload::DoorDensity {
            return Err(PerfScenarioConfigError(
                "Door performance presentation selection is reserved for the formal door-density profile"
                    .to_string(),
            ));
        }
        if wall_door_joint_actual_window
            && (workload != PerfWorkload::DoorDensity
                || size != PerfScenarioSize::Small
                || door_presentation != Some(PerfDoorPresentation::Production)
                || !joint_asset_opt_ins_match(
                    joint_release,
                    ["HW_WALL_CANDIDATE", "HW_DOOR_CANDIDATE"]
                        .map(|key| input.env_text(key).is_some_and(|value| value == "1")),
                    ["HW_WALL_ART_PREVIEW", "HW_DOOR_ART_PREVIEW"]
                        .map(|key| input.env_text(key).is_some_and(|value| value == "1")),
                )
                || input
                    .env_text("HW_WALL_ART_ACTUAL_WINDOW")
                    .is_some_and(|value| value == "1")
                || input
                    .env_text("HW_DOOR_ART_ACTUAL_WINDOW")
                    .is_some_and(|value| value == "1"))
        {
            return Err(PerfScenarioConfigError(
                "Wall/Door joint actual-window requires door-density/small/production, both isolated candidates or explicit release, and no art-preview or single-track actual-window profile"
                    .to_string(),
            ));
        }
        match rtt_light.map(PerfRttLightSelection::lane) {
            Some("behavior") if behavior_case.is_none() => {
                return Err(PerfScenarioConfigError(
                    "the rtt-light behavior lane requires --perf-behavior-case".to_string(),
                ));
            }
            Some("behavior")
                if size != PerfScenarioSize::Small
                    || render_mode != PerfRenderMode::Cpu
                    || !matches!(clock_mode, PerfClockMode::FixedBehavior) =>
            {
                return Err(PerfScenarioConfigError(
                    "the rtt-light behavior lane requires small/cpu/fixed-behavior".to_string(),
                ));
            }
            Some("behavior") => {}
            Some("field-core")
                if size != PerfScenarioSize::Large
                    || render_mode != PerfRenderMode::Cpu
                    || !matches!(clock_mode, PerfClockMode::Fixed)
                    || output_dir.is_none() =>
            {
                return Err(PerfScenarioConfigError(
                    "the rtt-light field-core lane requires large/cpu/fixed and an output directory"
                        .to_string(),
                ));
            }
            Some("field-core") => {}
            Some(_) if behavior_case.is_some() => {
                return Err(PerfScenarioConfigError(
                    "--perf-behavior-case is only valid for --perf-lane behavior".to_string(),
                ));
            }
            Some(_) if matches!(clock_mode, PerfClockMode::FixedBehavior) => {
                return Err(PerfScenarioConfigError(
                    "--perf-clock fixed-behavior is only valid for --perf-lane behavior"
                        .to_string(),
                ));
            }
            None if behavior_case.is_some()
                || matches!(clock_mode, PerfClockMode::FixedBehavior) =>
            {
                return Err(PerfScenarioConfigError(
                    "behavior selection is reserved for the rtt-light behavior lane".to_string(),
                ));
            }
            _ => {}
        }
        let window_width = parse_optional_u32(
            input.value("--perf-window-width", "HW_PERF_WINDOW_WIDTH")?,
            "--perf-window-width",
        )?;
        let window_height = parse_optional_u32(
            input.value("--perf-window-height", "HW_PERF_WINDOW_HEIGHT")?,
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
            input.value("--perf-window-scale-factor", "HW_PERF_WINDOW_SCALE_FACTOR")?,
            "--perf-window-scale-factor",
        )?;
        let rtt_quality = match input.value("--perf-rtt-quality", "HW_PERF_RTT_QUALITY")? {
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
        let wall_window_contract = wall_density_window_contract_matches(
            wall_art_matrix,
            window_width,
            window_height,
            window_scale_factor,
            rtt_quality,
        );
        if workload == PerfWorkload::WallDensity
            && (!matches!(size, PerfScenarioSize::Small | PerfScenarioSize::Medium)
                || render_mode != PerfRenderMode::Gpu
                || soul_count != if wall_formwork_acceptance { 2 } else { 0 }
                || familiar_count != 0
                || !matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden)
                || !matches!(clock_mode, PerfClockMode::Realtime)
                || master_seed != 20_260_901
                || !wall_density_durations_match(
                    wall_actual_window || wall_color_actual_window,
                    warmup_secs,
                    measure_secs,
                )
                || output_dir.is_none()
                || !wall_window_contract)
        {
            return Err(PerfScenarioConfigError(
                "wall-density requires small|medium/gpu/realtime, zero actors, seed 20260901, its formal 30s/60s or paired actual-window 10s/10s duration, an output directory, baseline policies, and the authorized 1280x720 quality/DPI window contract"
                    .to_string(),
            ));
        }
        if workload == PerfWorkload::DoorDensity
            && (!matches!(size, PerfScenarioSize::Small | PerfScenarioSize::Medium)
                || render_mode != PerfRenderMode::Gpu
                || soul_count != 0
                || familiar_count != 0
                || !matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
                || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
                || !matches!(dashboard_mode, PerfDashboardMode::Hidden)
                || !matches!(clock_mode, PerfClockMode::Realtime)
                || master_seed != 20_260_906
                || warmup_secs != 30.0
                || measure_secs != 60.0
                || output_dir.is_none()
                || door_presentation.is_none()
                || window_width != Some(1280)
                || window_height != Some(720)
                || window_scale_factor != Some(1.0)
                || rtt_quality != Some(RttQualityPreset::High))
        {
            return Err(PerfScenarioConfigError(
                "door-density requires small|medium/gpu/realtime, zero actors, seed 20260906, formal 30s/60s duration, a production|fallback-control presentation, an output directory, baseline policies, and the exact 1280x720/high/DPI-1 window contract"
                    .to_string(),
            ));
        }
        let building_art_population = match (workload, size) {
            (PerfWorkload::BuildingArtActive, PerfScenarioSize::Small) => (29, 2),
            (PerfWorkload::BuildingArtActive, _) => (116, 8),
            (_, PerfScenarioSize::Small) => (15, 0),
            _ => (60, 0),
        };
        if matches!(
            workload,
            PerfWorkload::BuildingArtStatic | PerfWorkload::BuildingArtActive
        ) && (!matches!(size, PerfScenarioSize::Small | PerfScenarioSize::Medium)
            || render_mode != PerfRenderMode::Gpu
            || soul_count != building_art_population.0
            || familiar_count != building_art_population.1
            || !matches!(familiar_policy_mode, PerfFamiliarPolicyMode::Baseline)
            || !matches!(operation_dialog_mode, PerfOperationDialogMode::Hidden)
            || !matches!(dashboard_mode, PerfDashboardMode::Hidden)
            || !matches!(clock_mode, PerfClockMode::Realtime)
            || master_seed != 20_260_920
            || warmup_secs != 30.0
            || measure_secs != 60.0
            || output_dir.is_none()
            || window_width != Some(1280)
            || window_height != Some(720)
            || window_scale_factor != Some(1.0)
            || rtt_quality != Some(RttQualityPreset::High))
        {
            return Err(PerfScenarioConfigError("building-art requires small/medium: static 15/60 Souls and no Familiars; active 29/116 Souls and 2/8 Familiars; gpu/realtime, seed 20260920, 30s/60s, output directory, baseline policies and 1280x720/high/DPI-1".into()));
        }
        #[cfg(feature = "profiling-renderdoc")]
        if renderdoc_capture
            && workload != PerfWorkload::WallDensity
            && (workload != PerfWorkload::IndoorLight
                || !rtt_light.is_some_and(PerfRttLightSelection::supports_renderdoc_capture)
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
                "--perf-renderdoc-capture requires either the exact wall-density contract or rtt-light-v1/current|p01|p02|p03|p04|p05/static medium/gpu/fixed with an output directory and the exact 1920x1080/scale-1/high window contract"
                    .to_string(),
            ));
        }

        let workload = match workload {
            PerfWorkload::Gather => ValidatedWorkload::Gather {
                familiar_policy_mode,
                operation_dialog_mode,
            },
            PerfWorkload::TaskDashboard => ValidatedWorkload::TaskDashboard { dashboard_mode },
            PerfWorkload::IndoorLight => ValidatedWorkload::IndoorLight {
                selection: rtt_light.expect("validated indoor-light selection"),
                behavior_case,
            },
            PerfWorkload::WallDensity => ValidatedWorkload::WallDensity {
                phase: wall_phase.expect("validated wall phase"),
                presentation: wall_presentation,
            },
            PerfWorkload::DoorDensity => ValidatedWorkload::DoorDensity {
                presentation: door_presentation.expect("validated door presentation"),
                joint_actual_window: wall_door_joint_actual_window,
            },
            PerfWorkload::BuildingArtStatic => ValidatedWorkload::BuildingArtStatic,
            PerfWorkload::BuildingArtActive => ValidatedWorkload::BuildingArtActive,
            PerfWorkload::PathDoor => ValidatedWorkload::PathDoor,
            PerfWorkload::Construction => ValidatedWorkload::Construction,
            PerfWorkload::UiGpu => ValidatedWorkload::UiGpu,
            PerfWorkload::DreamUiBurst => ValidatedWorkload::DreamUiBurst,
            PerfWorkload::Deconstruction => ValidatedWorkload::Deconstruction,
            PerfWorkload::SaveTransaction => ValidatedWorkload::SaveTransaction,
        };
        Ok(Self {
            common: PerfCommonConfig {
                master_seed,
                size,
                soul_count,
                familiar_count,
                render_mode,
                warmup_secs,
                measure_secs,
                output_dir,
                #[cfg(feature = "profiling-renderdoc")]
                renderdoc_capture,
                window_width,
                window_height,
                window_scale_factor,
                rtt_quality,
                clock_mode,
                fixed_step_hz,
                fixed_warmup_ticks,
                fixed_audit_ticks,
            },
            state: PerfScenarioState::Enabled(workload),
        })
    }

    pub const fn master_seed(&self) -> u64 {
        self.common.master_seed
    }
    pub const fn size(&self) -> PerfScenarioSize {
        self.common.size
    }
    pub const fn soul_count(&self) -> u32 {
        self.common.soul_count
    }
    pub const fn familiar_count(&self) -> u32 {
        self.common.familiar_count
    }
    pub const fn render_mode(&self) -> PerfRenderMode {
        self.common.render_mode
    }
    pub const fn warmup_secs(&self) -> f32 {
        self.common.warmup_secs
    }
    pub const fn measure_secs(&self) -> f32 {
        self.common.measure_secs
    }

    pub fn output_dir(&self) -> Option<&std::path::Path> {
        self.common.output_dir.as_deref()
    }

    pub const fn workload(&self) -> PerfWorkload {
        match self.state {
            PerfScenarioState::Disabled => PerfWorkload::Gather,
            PerfScenarioState::Enabled(workload) => workload.kind(),
        }
    }
    pub const fn familiar_policy_mode(&self) -> PerfFamiliarPolicyMode {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::Gather {
                familiar_policy_mode,
                ..
            }) => familiar_policy_mode,
            _ => PerfFamiliarPolicyMode::Baseline,
        }
    }
    pub const fn operation_dialog_mode(&self) -> PerfOperationDialogMode {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::Gather {
                operation_dialog_mode,
                ..
            }) => operation_dialog_mode,
            _ => PerfOperationDialogMode::Hidden,
        }
    }
    pub const fn dashboard_mode(&self) -> PerfDashboardMode {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::TaskDashboard { dashboard_mode }) => {
                dashboard_mode
            }
            _ => PerfDashboardMode::Hidden,
        }
    }
    pub const fn enabled(&self) -> bool {
        matches!(self.state, PerfScenarioState::Enabled(_))
    }

    pub const fn requested_window_size(&self) -> Option<(u32, u32)> {
        if !self.enabled() {
            return None;
        }
        match (self.common.window_width, self.common.window_height) {
            (Some(width), Some(height)) => Some((width, height)),
            _ => None,
        }
    }

    pub const fn rtt_light_selection(&self) -> Option<PerfRttLightSelection> {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::IndoorLight { selection, .. }) => {
                Some(selection)
            }
            _ => None,
        }
    }

    #[cfg(feature = "profiling-renderdoc")]
    pub(crate) const fn renderdoc_capture_enabled(&self) -> bool {
        self.enabled() && self.common.renderdoc_capture
    }

    pub(super) const fn behavior_case(&self) -> Option<PerfBehaviorCase> {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::IndoorLight {
                behavior_case, ..
            }) => behavior_case,
            _ => None,
        }
    }

    pub const fn behavior_case_as_str(&self) -> Option<&'static str> {
        match self.behavior_case() {
            Some(case) => Some(case.as_str()),
            None => None,
        }
    }

    pub const fn wall_phase(&self) -> Option<PerfWallPhase> {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::WallDensity { phase, .. }) => Some(phase),
            _ => None,
        }
    }

    pub const fn wall_presentation(&self) -> Option<PerfWallPresentation> {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::WallDensity { presentation, .. }) => {
                presentation
            }
            _ => None,
        }
    }

    pub const fn door_presentation(&self) -> Option<PerfDoorPresentation> {
        match self.state {
            PerfScenarioState::Enabled(ValidatedWorkload::DoorDensity { presentation, .. }) => {
                Some(presentation)
            }
            _ => None,
        }
    }

    pub const fn wall_door_joint_actual_window(&self) -> bool {
        matches!(
            self.state,
            PerfScenarioState::Enabled(ValidatedWorkload::DoorDensity {
                joint_actual_window: true,
                ..
            })
        )
    }

    pub const fn requested_window_scale_factor(&self) -> Option<f32> {
        if self.enabled() {
            self.common.window_scale_factor
        } else {
            None
        }
    }

    pub const fn requested_rtt_quality(&self) -> Option<RttQualityPreset> {
        if self.enabled() {
            self.common.rtt_quality
        } else {
            None
        }
    }

    pub const fn uses_fixed_timesteps(&self) -> bool {
        matches!(
            self.common.clock_mode,
            PerfClockMode::Fixed | PerfClockMode::FixedBehavior
        )
    }

    pub const fn freezes_fixture_setup(&self) -> bool {
        // Every automated profiling fixture must capture its initial checksum
        // before production Logic / Actor systems can advance the spawned
        // actors.  Otherwise a realtime workload records its "initial" state
        // on the next frame after a variable-delta movement step, so repeated
        // runs with the same seed fail the mandatory checksum contract.
        self.enabled()
    }

    /// Indoor-light lanes other than behavior treat their seeded Door states as
    /// immutable scene topology. Fixed audits still advance simulation time,
    /// so Door automation must be excluded explicitly instead of relying on
    /// pause.
    pub fn freezes_indoor_light_door_automation(&self) -> bool {
        self.enabled()
            && self.workload() == PerfWorkload::IndoorLight
            && self
                .rtt_light_selection()
                .is_some_and(|selection| selection.lane() != "behavior")
    }

    /// 静的 light fixture / Door gallery の描画計測中はゲーム simulation を進めない。
    ///
    /// 計測窓そのものは `Time<Real>` で進める。これにより Soul/Familiar AI が
    /// showcase building の bucket 等を運ぶことなく、同一 scene topology を
    /// Capture / Memory / RenderDoc で共有できる。
    pub fn keeps_virtual_time_paused_during_capture(&self) -> bool {
        #[cfg(feature = "profiling")]
        let door_gallery_requested =
            super::door_actual_window::DoorActualWindowAcceptance::requested_from_environment();
        #[cfg(not(feature = "profiling"))]
        let door_gallery_requested = false;

        self.enabled()
            && !self.uses_fixed_timesteps()
            && (matches!(
                self.workload(),
                PerfWorkload::IndoorLight
                    | PerfWorkload::WallDensity
                    | PerfWorkload::DoorDensity
                    | PerfWorkload::BuildingArtStatic
            ) || door_gallery_requested)
    }

    /// Density fixtures own the logical contents of their measurement world.
    ///
    /// Terrain and renderer startup still run, but the normal generated trees,
    /// rocks, facilities, and regrowth targets must not reserve cells or add
    /// unrelated draw work before the fixed wall fixture is installed.
    pub const fn uses_isolated_density_world(&self) -> bool {
        self.enabled()
            && matches!(
                self.workload(),
                PerfWorkload::WallDensity
                    | PerfWorkload::DoorDensity
                    | PerfWorkload::BuildingArtStatic
                    | PerfWorkload::BuildingArtActive
            )
    }

    pub fn is_field_core(&self) -> bool {
        self.enabled()
            && self.workload() == PerfWorkload::IndoorLight
            && self
                .rtt_light_selection()
                .is_some_and(|selection| selection.lane() == "field-core")
    }

    pub fn is_pure_field_core(&self) -> bool {
        self.is_field_core()
            && self
                .rtt_light_selection()
                .is_some_and(|selection| selection.stage_id() == "p03")
    }

    pub fn is_consumer_core(&self) -> bool {
        self.enabled()
            && self.workload() == PerfWorkload::IndoorLight
            && self
                .rtt_light_selection()
                .is_some_and(|selection| selection.lane() == "consumer-core")
    }

    /// P08 static evidence needs one production slow-simulation step after the
    /// CPU field and Room summaries are current, but before capture freezes the
    /// fixture again. Other stages and lanes must retain their existing setup
    /// timing.
    pub fn requires_p08_cross_consumer_setup_step(&self) -> bool {
        self.enabled()
            && self.workload() == PerfWorkload::IndoorLight
            && self.rtt_light_selection().is_some_and(|selection| {
                selection.stage_id() == "p08" && selection.lane() == "static"
            })
    }

    /// P04 field-core advances the light runtime through `Update`, but must not
    /// advance normal simulation while its canonical fixture is settling.
    /// `PreActor` and `PostActor` remain scheduled while virtual time is paused,
    /// so manual mutations and the light snapshot/rebuild boundary still run.
    pub fn pauses_virtual_time_for_field_core(&self) -> bool {
        self.is_field_core()
            && self
                .rtt_light_selection()
                .is_some_and(|selection| selection.uses_runtime_field())
    }

    /// 自動 perf の CPU 条件では、計測対象外の 3D scene root を生成しない。
    ///
    /// これは起動時 fixture の生成だけに使う。通常プレイと、実行中の F8/F3
    /// 切替は既存どおり scene root を維持する。
    pub const fn omits_3d_scene_roots(&self) -> bool {
        self.enabled() && matches!(self.common.render_mode, PerfRenderMode::Cpu)
    }

    pub const fn clock_mode_as_str(&self) -> &'static str {
        self.common.clock_mode.as_str()
    }

    pub const fn fixed_step_hz(&self) -> u32 {
        self.common.fixed_step_hz
    }

    pub const fn fixed_warmup_ticks(&self) -> u64 {
        self.common.fixed_warmup_ticks
    }

    pub const fn fixed_audit_ticks(&self) -> u64 {
        self.common.fixed_audit_ticks
    }

    #[cfg(feature = "profiling")]
    pub(super) const fn fixed_audit_end_tick(&self) -> u64 {
        self.common.fixed_warmup_ticks + self.common.fixed_audit_ticks
    }

    pub fn initial_render_resources(&self) -> (Render3dVisible, RenderPerfToggles) {
        if !self.enabled() {
            return (Render3dVisible::default(), RenderPerfToggles::default());
        }

        match self.common.render_mode {
            PerfRenderMode::Cpu => (Render3dVisible(false), RenderPerfToggles::all_disabled()),
            PerfRenderMode::Gpu => (Render3dVisible(true), RenderPerfToggles::gpu_baseline()),
        }
    }

    fn stream_seed(&self, stream: PerfRandomStream) -> u64 {
        splitmix64(self.common.master_seed ^ stream.salt())
    }
}

fn joint_asset_opt_ins_match(release: bool, candidates: [bool; 2], previews: [bool; 2]) -> bool {
    candidates == [!release; 2] && previews == [false; 2]
}

fn wall_density_durations_match(actual_window: bool, warmup_secs: f32, measure_secs: f32) -> bool {
    let expected = if actual_window {
        (10.0, 10.0)
    } else {
        (30.0, 60.0)
    };
    (warmup_secs, measure_secs) == expected
}

const fn wall_actual_window_phase_matches(
    wall_actual_window: bool,
    wall_color_actual_window: bool,
    art_preview: bool,
    formwork_acceptance: bool,
    phase: Option<PerfWallPhase>,
) -> bool {
    if wall_color_actual_window {
        return matches!(phase, Some(PerfWallPhase::Completed));
    }
    wall_actual_window
        && (matches!(phase, Some(PerfWallPhase::Completed))
            || (art_preview && matches!(phase, Some(PerfWallPhase::Provisional)))
            || (formwork_acceptance && matches!(phase, Some(PerfWallPhase::Mixed))))
}

fn wall_density_window_contract_matches(
    art_matrix: bool,
    width: Option<u32>,
    height: Option<u32>,
    scale_factor: Option<f32>,
    quality: Option<RttQualityPreset>,
) -> bool {
    if width != Some(1280) || height != Some(720) {
        return false;
    }
    if art_matrix {
        scale_factor.is_some_and(|scale| [1.0, 1.5, 2.0].contains(&scale))
            && matches!(
                quality,
                Some(RttQualityPreset::High | RttQualityPreset::Medium | RttQualityPreset::Low)
            )
    } else {
        scale_factor == Some(1.0) && quality == Some(RttQualityPreset::High)
    }
}

#[cfg(feature = "profiling-renderdoc")]
fn resolve_renderdoc_capture(requested: bool) -> Result<bool, PerfScenarioConfigError> {
    Ok(requested)
}

#[cfg(not(feature = "profiling-renderdoc"))]
fn resolve_renderdoc_capture(requested: bool) -> Result<bool, PerfScenarioConfigError> {
    if requested {
        Err(PerfScenarioConfigError(
            "--perf-renderdoc-capture requires the profiling-renderdoc feature; rebuild with --features profiling-renderdoc"
                .to_string(),
        ))
    } else {
        Ok(false)
    }
}

/// 固定 step 監査では、初期 fixture を通常の Logic ゲートより先に適用する。
///
/// 監査開始時は `Time<Virtual>` を停止したままにするため、通常の `Logic`
/// system set に置かれた spawn consumer は実行できない。この条件は、その
/// 専用経路と通常経路を相互排他的にするために使う。
#[cfg(feature = "profiling")]
pub(crate) fn is_fixed_step_audit(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| {
        config.enabled() && matches!(config.common.clock_mode, PerfClockMode::Fixed)
    })
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_fixed_step_audit(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_fixed_step_audit(config)
}

#[cfg(feature = "profiling")]
pub(crate) fn is_fixed_step_behavior(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| {
        config.enabled() && matches!(config.common.clock_mode, PerfClockMode::FixedBehavior)
    })
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_fixed_step_behavior(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_fixed_step_behavior(config)
}

#[cfg(feature = "profiling")]
pub(crate) fn is_field_core(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| config.is_field_core())
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_field_core(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_field_core(config)
}

#[cfg(feature = "profiling")]
pub(crate) fn is_consumer_core(config: Option<Res<PerfScenarioConfig>>) -> bool {
    config.is_some_and(|config| config.is_consumer_core())
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_consumer_core(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !is_consumer_core(config)
}

#[cfg(feature = "profiling")]
pub(crate) fn is_not_pure_field_core(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !config.is_some_and(|config| config.is_pure_field_core())
}

#[cfg(feature = "profiling")]
pub(crate) fn requires_precheckpoint_fixture_spawn(
    config: Option<Res<PerfScenarioConfig>>,
) -> bool {
    config.is_some_and(|config| config.freezes_fixture_setup() && !config.is_pure_field_core())
}

#[cfg(feature = "profiling")]
pub(crate) fn does_not_require_precheckpoint_fixture_spawn(
    config: Option<Res<PerfScenarioConfig>>,
) -> bool {
    !requires_precheckpoint_fixture_spawn(config)
}

#[cfg(feature = "profiling-renderdoc")]
pub(crate) fn is_not_renderdoc_capture(config: Option<Res<PerfScenarioConfig>>) -> bool {
    !config.is_some_and(|config| config.renderdoc_capture_enabled())
}

#[cfg(all(feature = "profiling", not(feature = "profiling-renderdoc")))]
pub(crate) fn is_not_renderdoc_capture(_config: Option<Res<PerfScenarioConfig>>) -> bool {
    true
}

fn parse_rtt_light_selection(
    input: &PerfConfigInput<'_>,
    workload: PerfWorkload,
) -> Result<Option<PerfRttLightSelection>, PerfScenarioConfigError> {
    let contract = input.value("--perf-contract", "HW_PERF_CONTRACT")?;
    let stage = input.value("--perf-stage", "HW_PERF_STAGE")?;
    let lane = input.value("--perf-lane", "HW_PERF_LANE")?;
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
            "--perf-workload indoor-light requires --perf-contract rtt-light-v1 --perf-stage current|p01|p02|p03|p04|p05|p06|p07|p08 --perf-lane static|behavior|field-core|consumer-core"
                .to_string(),
        ));
    };
    let selection = match (contract.as_str(), stage.as_str(), lane.as_str()) {
        ("rtt-light-v1", "current", "static") => PerfRttLightSelection::CURRENT_STATIC_V1,
        ("rtt-light-v1", "current", "behavior") => PerfRttLightSelection::CURRENT_BEHAVIOR_V1,
        ("rtt-light-v1", "p01", "static") => PerfRttLightSelection::P01_STATIC_V1,
        ("rtt-light-v1", "p01", "behavior") => PerfRttLightSelection::P01_BEHAVIOR_V1,
        ("rtt-light-v1", "p02", "static") => PerfRttLightSelection::P02_STATIC_V1,
        ("rtt-light-v1", "p02", "behavior") => PerfRttLightSelection::P02_BEHAVIOR_V1,
        ("rtt-light-v1", "p03", "static") => PerfRttLightSelection::P03_STATIC_V1,
        ("rtt-light-v1", "p03", "behavior") => PerfRttLightSelection::P03_BEHAVIOR_V1,
        ("rtt-light-v1", "p03", "field-core") => PerfRttLightSelection::P03_FIELD_CORE_V1,
        ("rtt-light-v1", "p04", "static") => PerfRttLightSelection::P04_STATIC_V1,
        ("rtt-light-v1", "p04", "behavior") => PerfRttLightSelection::P04_BEHAVIOR_V1,
        ("rtt-light-v1", "p04", "field-core") => PerfRttLightSelection::P04_FIELD_CORE_V1,
        ("rtt-light-v1", "p05", "static") => PerfRttLightSelection::P05_STATIC_V1,
        ("rtt-light-v1", "p05", "behavior") => PerfRttLightSelection::P05_BEHAVIOR_V1,
        ("rtt-light-v1", "p05", "field-core") => PerfRttLightSelection::P05_FIELD_CORE_V1,
        ("rtt-light-v1", "p06", "static") => PerfRttLightSelection::P06_STATIC_V1,
        ("rtt-light-v1", "p06", "behavior") => PerfRttLightSelection::P06_BEHAVIOR_V1,
        ("rtt-light-v1", "p06", "field-core") => PerfRttLightSelection::P06_FIELD_CORE_V1,
        ("rtt-light-v1", "p07", "static") => PerfRttLightSelection::P07_STATIC_V1,
        ("rtt-light-v1", "p07", "behavior") => PerfRttLightSelection::P07_BEHAVIOR_V1,
        ("rtt-light-v1", "p07", "field-core") => PerfRttLightSelection::P07_FIELD_CORE_V1,
        ("rtt-light-v1", "p07", "consumer-core") => PerfRttLightSelection::P07_CONSUMER_CORE_V1,
        ("rtt-light-v1", "p08", "static") => PerfRttLightSelection::P08_STATIC_V1,
        ("rtt-light-v1", "p08", "behavior") => PerfRttLightSelection::P08_BEHAVIOR_V1,
        ("rtt-light-v1", "p08", "field-core") => PerfRttLightSelection::P08_FIELD_CORE_V1,
        ("rtt-light-v1", "p08", "consumer-core") => PerfRttLightSelection::P08_CONSUMER_CORE_V1,
        _ => {
            return Err(PerfScenarioConfigError(format!(
                "this binary supports rtt-light-v1 current through p08; field-core starts at p03 and consumer-core at p07; got {contract}/{stage}/{lane}"
            )));
        }
    };
    Ok(Some(selection))
}

impl Default for PerfScenarioConfig {
    fn default() -> Self {
        let (soul_count, familiar_count) = PerfScenarioSize::Medium.population();
        Self {
            common: PerfCommonConfig {
                master_seed: 0,
                size: PerfScenarioSize::Medium,
                soul_count,
                familiar_count,
                render_mode: PerfRenderMode::Gpu,
                warmup_secs: DEFAULT_WARMUP_SECS,
                measure_secs: DEFAULT_MEASURE_SECS,
                output_dir: None,
                #[cfg(feature = "profiling-renderdoc")]
                renderdoc_capture: false,
                window_width: None,
                window_height: None,
                window_scale_factor: None,
                rtt_quality: None,
                clock_mode: PerfClockMode::Realtime,
                fixed_step_hz: DEFAULT_FIXED_STEP_HZ,
                fixed_warmup_ticks: DEFAULT_FIXED_WARMUP_TICKS,
                fixed_audit_ticks: DEFAULT_FIXED_AUDIT_TICKS,
            },
            state: PerfScenarioState::Disabled,
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
