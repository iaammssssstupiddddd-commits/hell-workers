use super::{
    DEFAULT_FIXED_AUDIT_TICKS, DEFAULT_FIXED_STEP_HZ, DEFAULT_FIXED_WARMUP_TICKS, PerfClockMode,
    PerfRandomStream, PerfScenarioConfig, splitmix64,
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
        familiar_policy_mode: super::PerfFamiliarPolicyMode::Baseline,
        operation_dialog_mode: super::PerfOperationDialogMode::Hidden,
        dashboard_mode: super::PerfDashboardMode::Hidden,
        warmup_secs: 30.0,
        measure_secs: 60.0,
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
fn perf_window_axis_parsers_are_strict() {
    assert_eq!(
        super::parse_optional_u32(Some("1920".to_string()), "--perf-window-width").unwrap(),
        Some(1920)
    );
    assert!(super::parse_optional_u32(Some("1.5".to_string()), "--perf-window-width").is_err());
    assert_eq!(
        super::parse_optional_positive_f32(Some("1.5".to_string()), "--perf-window-scale-factor")
            .unwrap(),
        Some(1.5)
    );
    for invalid in ["0", "-1", "NaN", "inf"] {
        assert!(
            super::parse_optional_positive_f32(
                Some(invalid.to_string()),
                "--perf-window-scale-factor"
            )
            .is_err()
        );
    }
}

#[test]
fn disabled_perf_config_cannot_override_production_window_or_rtt_quality() {
    let mut config = PerfScenarioConfig {
        window_width: Some(1920),
        window_height: Some(1080),
        window_scale_factor: Some(2.0),
        rtt_quality: Some(hw_core::quality::RttQualityPreset::Low),
        ..PerfScenarioConfig::default()
    };

    assert_eq!(config.requested_window_size(), None);
    assert_eq!(config.requested_window_scale_factor(), None);
    assert_eq!(config.requested_rtt_quality(), None);

    config.enabled = true;
    assert_eq!(config.requested_window_size(), Some((1920, 1080)));
    assert_eq!(config.requested_window_scale_factor(), Some(2.0));
    assert_eq!(
        config.requested_rtt_quality(),
        Some(hw_core::quality::RttQualityPreset::Low)
    );
}

#[test]
fn fixed_clock_mode_is_explicit() {
    assert_eq!(PerfClockMode::parse("fixed"), Some(PerfClockMode::Fixed));
    assert_eq!(
        PerfClockMode::parse("fixed-behavior"),
        Some(PerfClockMode::FixedBehavior)
    );
    assert_eq!(
        PerfClockMode::parse("realtime"),
        Some(PerfClockMode::Realtime)
    );
    assert_eq!(PerfClockMode::parse("auto"), None);
    assert_eq!(PerfClockMode::Fixed.as_str(), "fixed");
    assert_eq!(PerfClockMode::FixedBehavior.as_str(), "fixed-behavior");
}

#[test]
fn familiar_policy_and_dialog_modes_are_explicit() {
    assert_eq!(
        super::PerfFamiliarPolicyMode::parse("baseline"),
        Some(super::PerfFamiliarPolicyMode::Baseline)
    );
    assert_eq!(
        super::PerfFamiliarPolicyMode::parse("default"),
        Some(super::PerfFamiliarPolicyMode::Default)
    );
    assert_eq!(
        super::PerfFamiliarPolicyMode::parse("disabled"),
        Some(super::PerfFamiliarPolicyMode::Disabled)
    );
    assert_eq!(super::PerfFamiliarPolicyMode::parse("all"), None);
    assert_eq!(
        super::PerfOperationDialogMode::parse("hidden"),
        Some(super::PerfOperationDialogMode::Hidden)
    );
    assert_eq!(
        super::PerfOperationDialogMode::parse("open"),
        Some(super::PerfOperationDialogMode::Open)
    );
    assert_eq!(
        super::PerfDashboardMode::parse("hidden"),
        Some(super::PerfDashboardMode::Hidden)
    );
    assert_eq!(
        super::PerfDashboardMode::parse("visible"),
        Some(super::PerfDashboardMode::Visible)
    );
    assert_eq!(
        super::PerfDashboardMode::parse("active-filter"),
        Some(super::PerfDashboardMode::ActiveFilter)
    );
    assert_eq!(super::PerfDashboardMode::parse("all"), None);
}

#[test]
fn indoor_light_selection_is_exact_and_not_implicit() {
    let exact = [
        "perf-test",
        "--perf-contract",
        "rtt-light-v1",
        "--perf-stage",
        "current",
        "--perf-lane",
        "static",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let selection = super::parse_rtt_light_selection(&exact, super::PerfWorkload::IndoorLight)
        .expect("current/static v1 is the implemented vertical slice")
        .expect("indoor-light requires an explicit selection");
    assert_eq!(selection.contract_id(), "rtt-light-v1");
    assert_eq!(selection.stage_id(), "current");
    assert_eq!(selection.lane(), "static");

    let missing_lane = exact[..exact.len() - 2].to_vec();
    assert!(
        super::parse_rtt_light_selection(&missing_lane, super::PerfWorkload::IndoorLight).is_err()
    );
    assert!(super::parse_rtt_light_selection(&exact, super::PerfWorkload::Gather).is_err());

    let mut wrong_stage = exact;
    wrong_stage[4] = "p01".to_string();
    assert!(
        super::parse_rtt_light_selection(&wrong_stage, super::PerfWorkload::IndoorLight).is_err()
    );

    let mut behavior = wrong_stage;
    behavior[4] = "current".to_string();
    behavior[6] = "behavior".to_string();
    let behavior_selection =
        super::parse_rtt_light_selection(&behavior, super::PerfWorkload::IndoorLight)
            .expect("current/behavior v1 is implemented")
            .expect("behavior selection must be present");
    assert_eq!(behavior_selection.lane(), "behavior");
    assert_eq!(
        super::PerfBehaviorCase::parse("door-state-v1"),
        Some(super::PerfBehaviorCase::DoorStateV1)
    );
    assert_eq!(
        super::PerfBehaviorCase::parse("load-normal-v1"),
        Some(super::PerfBehaviorCase::LoadNormalV1)
    );
    assert_eq!(super::PerfBehaviorCase::parse("unknown"), None);
}

#[test]
fn behavior_lane_accepts_the_canonical_behavior_fixture() {
    let args = [
        "perf-test",
        "--perf-contract",
        "rtt-light-v1",
        "--perf-stage",
        "current",
        "--perf-lane",
        "behavior",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let selection = super::parse_rtt_light_selection(&args, super::PerfWorkload::IndoorLight)
        .expect("current/behavior v1 is implemented");

    assert!(
        super::validate_behavior_lane(
            selection,
            Some(super::PerfBehaviorCase::DoorStateV1),
            super::PerfScenarioSize::Small,
            super::PerfRenderMode::Cpu,
            PerfClockMode::FixedBehavior,
        )
        .is_ok()
    );
}
