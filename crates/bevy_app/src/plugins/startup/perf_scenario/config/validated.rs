use super::*;

#[derive(Debug, Clone)]
pub(super) struct PerfCommonConfig {
    pub(super) master_seed: u64,
    pub(super) size: PerfScenarioSize,
    pub(super) soul_count: u32,
    pub(super) familiar_count: u32,
    pub(super) render_mode: PerfRenderMode,
    pub(super) warmup_secs: f32,
    pub(super) measure_secs: f32,
    pub(super) output_dir: Option<PathBuf>,
    #[cfg(feature = "profiling-renderdoc")]
    pub(super) renderdoc_capture: bool,
    pub(super) window_width: Option<u32>,
    pub(super) window_height: Option<u32>,
    pub(super) window_scale_factor: Option<f32>,
    pub(super) rtt_quality: Option<RttQualityPreset>,
    pub(super) clock_mode: PerfClockMode,
    pub(super) fixed_step_hz: u32,
    pub(super) fixed_warmup_ticks: u64,
    pub(super) fixed_audit_ticks: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum PerfScenarioState {
    Disabled,
    Enabled(ValidatedWorkload),
}

/// The discriminant and its required payload are constructed together only
/// after CLI/environment validation. Consumers receive read-only projections.
#[derive(Debug, Clone, Copy)]
pub(super) enum ValidatedWorkload {
    Gather {
        familiar_policy_mode: PerfFamiliarPolicyMode,
        operation_dialog_mode: PerfOperationDialogMode,
    },
    TaskDashboard {
        dashboard_mode: PerfDashboardMode,
    },
    IndoorLight {
        selection: PerfRttLightSelection,
        behavior_case: Option<PerfBehaviorCase>,
    },
    WallDensity {
        phase: PerfWallPhase,
        presentation: Option<PerfWallPresentation>,
    },
    DoorDensity {
        presentation: PerfDoorPresentation,
        joint_actual_window: bool,
    },
    PathDoor,
    Construction,
    UiGpu,
    DreamUiBurst,
    Deconstruction,
    SaveTransaction,
}

impl ValidatedWorkload {
    pub(super) const fn kind(self) -> PerfWorkload {
        match self {
            Self::Gather { .. } => PerfWorkload::Gather,
            Self::TaskDashboard { .. } => PerfWorkload::TaskDashboard,
            Self::IndoorLight { .. } => PerfWorkload::IndoorLight,
            Self::WallDensity { .. } => PerfWorkload::WallDensity,
            Self::DoorDensity { .. } => PerfWorkload::DoorDensity,
            Self::PathDoor => PerfWorkload::PathDoor,
            Self::Construction => PerfWorkload::Construction,
            Self::UiGpu => PerfWorkload::UiGpu,
            Self::DreamUiBurst => PerfWorkload::DreamUiBurst,
            Self::Deconstruction => PerfWorkload::Deconstruction,
            Self::SaveTransaction => PerfWorkload::SaveTransaction,
        }
    }
}
