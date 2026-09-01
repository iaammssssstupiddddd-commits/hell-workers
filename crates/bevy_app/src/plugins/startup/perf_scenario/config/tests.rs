use super::{
    DEFAULT_FIXED_AUDIT_TICKS, DEFAULT_FIXED_STEP_HZ, DEFAULT_FIXED_WARMUP_TICKS, PerfClockMode,
    PerfRandomStream, PerfScenarioConfig, splitmix64,
};

#[test]
fn renderdoc_capture_requires_its_dedicated_feature() {
    assert!(!super::resolve_renderdoc_capture(false).unwrap());
    #[cfg(feature = "profiling-renderdoc")]
    assert!(super::resolve_renderdoc_capture(true).unwrap());
    #[cfg(not(feature = "profiling-renderdoc"))]
    assert!(super::resolve_renderdoc_capture(true).is_err());
}

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
        #[cfg(feature = "profiling-renderdoc")]
        renderdoc_capture: false,
        rtt_light: None,
        behavior_case: None,
        wall_phase: None,
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
fn every_enabled_fixture_freezes_until_the_initial_checkpoint() {
    let mut config = PerfScenarioConfig {
        enabled: true,
        ..PerfScenarioConfig::default()
    };
    assert!(config.freezes_fixture_setup());

    config.workload = super::PerfWorkload::IndoorLight;
    assert!(config.freezes_fixture_setup());
    assert!(config.keeps_virtual_time_paused_during_capture());
    config.rtt_light = Some(super::PerfRttLightSelection::P02_STATIC_V1);
    assert!(config.freezes_indoor_light_door_automation());
    config.rtt_light = Some(super::PerfRttLightSelection::P02_BEHAVIOR_V1);
    assert!(!config.freezes_indoor_light_door_automation());
    config.rtt_light = Some(super::PerfRttLightSelection::P04_FIELD_CORE_V1);
    assert!(config.freezes_indoor_light_door_automation());
    assert!(config.pauses_virtual_time_for_field_core());
    config.rtt_light = Some(super::PerfRttLightSelection::P03_FIELD_CORE_V1);
    assert!(!config.pauses_virtual_time_for_field_core());
    config.rtt_light = Some(super::PerfRttLightSelection::P08_STATIC_V1);
    assert!(config.requires_p08_cross_consumer_setup_step());
    config.clock_mode = PerfClockMode::Fixed;
    assert!(config.requires_p08_cross_consumer_setup_step());
    config.rtt_light = Some(super::PerfRttLightSelection::P08_BEHAVIOR_V1);
    assert!(!config.requires_p08_cross_consumer_setup_step());
    config.rtt_light = Some(super::PerfRttLightSelection::P07_STATIC_V1);
    assert!(!config.requires_p08_cross_consumer_setup_step());
    config.clock_mode = PerfClockMode::Realtime;
    config.rtt_light = None;

    config.workload = super::PerfWorkload::TaskDashboard;
    assert!(config.freezes_fixture_setup());
    assert!(!config.keeps_virtual_time_paused_during_capture());
    assert!(!config.pauses_virtual_time_for_field_core());
    assert!(!config.freezes_indoor_light_door_automation());
    assert!(!config.requires_p08_cross_consumer_setup_step());

    config.workload = super::PerfWorkload::WallDensity;
    assert!(config.freezes_fixture_setup());
    assert!(config.keeps_virtual_time_paused_during_capture());

    config.workload = super::PerfWorkload::Gather;
    config.clock_mode = PerfClockMode::Realtime;
    assert!(config.freezes_fixture_setup());
    config.clock_mode = PerfClockMode::Fixed;
    assert!(config.freezes_fixture_setup());
    assert!(!config.keeps_virtual_time_paused_during_capture());

    config.enabled = false;
    assert!(!config.freezes_fixture_setup());
    assert!(!config.keeps_virtual_time_paused_during_capture());
}

#[test]
fn wall_density_phase_names_are_explicit() {
    assert_eq!(
        super::PerfWallPhase::parse("completed"),
        Some(super::PerfWallPhase::Completed)
    );
    assert_eq!(
        super::PerfWallPhase::parse("provisional"),
        Some(super::PerfWallPhase::Provisional)
    );
    assert_eq!(super::PerfWallPhase::parse("mixed"), None);
    assert_eq!(super::PerfWallPhase::Completed.as_str(), "completed");
    assert_eq!(super::PerfWallPhase::Provisional.as_str(), "provisional");
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

    let mut p01 = exact.clone();
    p01[4] = "p01".to_string();
    let p01_selection = super::parse_rtt_light_selection(&p01, super::PerfWorkload::IndoorLight)
        .expect("p01/static v1 is implemented")
        .expect("P01 requires an explicit selection");
    assert_eq!(p01_selection.stage_id(), "p01");
    let mut p01_behavior = p01;
    p01_behavior[6] = "behavior".to_string();
    let p01_behavior_selection =
        super::parse_rtt_light_selection(&p01_behavior, super::PerfWorkload::IndoorLight)
            .expect("p01/behavior v1 is implemented")
            .expect("P01 behavior requires an explicit selection");
    assert_eq!(p01_behavior_selection.lane(), "behavior");

    let mut p02 = p01_behavior;
    p02[4] = "p02".to_string();
    let p02_selection = super::parse_rtt_light_selection(&p02, super::PerfWorkload::IndoorLight)
        .expect("p02/behavior v1 is implemented")
        .expect("P02 requires an explicit selection");
    assert_eq!(p02_selection.stage_id(), "p02");
    assert_eq!(p02_selection.lane(), "behavior");

    let mut p03 = exact.clone();
    p03[4] = "p03".to_string();
    p03[6] = "field-core".to_string();
    let p03_selection = super::parse_rtt_light_selection(&p03, super::PerfWorkload::IndoorLight)
        .expect("p03/field-core v1 is implemented")
        .expect("P03 field-core requires an explicit selection");
    assert_eq!(p03_selection.stage_id(), "p03");
    assert_eq!(p03_selection.lane(), "field-core");
    assert!(p03_selection.uses_p02_presentation());
    assert!(!p03_selection.uses_runtime_field());

    let mut p04 = p03.clone();
    p04[4] = "p04".to_string();
    let p04_selection = super::parse_rtt_light_selection(&p04, super::PerfWorkload::IndoorLight)
        .expect("p04/field-core v1 is implemented")
        .expect("P04 field-core requires an explicit selection");
    assert_eq!(p04_selection.stage_id(), "p04");
    assert_eq!(p04_selection.lane(), "field-core");
    assert!(p04_selection.uses_p02_presentation());
    assert!(p04_selection.uses_runtime_field());
    assert!(!p04_selection.supports_renderdoc_capture());

    p04[6] = "static".to_string();
    let p04_static_selection =
        super::parse_rtt_light_selection(&p04, super::PerfWorkload::IndoorLight)
            .expect("p04/static v1 is implemented")
            .expect("P04 static requires an explicit selection");
    assert!(p04_static_selection.supports_renderdoc_capture());

    let mut p05 = p04;
    p05[4] = "p05".to_string();
    let p05_selection = super::parse_rtt_light_selection(&p05, super::PerfWorkload::IndoorLight)
        .expect("p05/static v1 is implemented")
        .expect("P05 static requires an explicit selection");
    assert_eq!(p05_selection.stage_id(), "p05");
    assert!(p05_selection.uses_runtime_field());
    assert!(p05_selection.uses_p02_presentation());

    let mut p06 = p05;
    p06[4] = "p06".to_string();
    let p06_selection = super::parse_rtt_light_selection(&p06, super::PerfWorkload::IndoorLight)
        .expect("p06/static v1 is implemented")
        .expect("P06 static requires an explicit selection");
    assert_eq!(p06_selection.stage_id(), "p06");
    assert!(p06_selection.uses_runtime_field());
    assert!(p06_selection.uses_p02_presentation());
    assert!(p06_selection.uses_gpu_light_field());
    assert!(!p06_selection.uses_cpu_consumers());
    assert!(p06_selection.supports_renderdoc_capture());

    let mut p07 = p06;
    p07[4] = "p07".to_string();
    p07[6] = "consumer-core".to_string();
    let p07_selection = super::parse_rtt_light_selection(&p07, super::PerfWorkload::IndoorLight)
        .expect("p07/consumer-core v1 is implemented")
        .expect("P07 consumer-core requires an explicit selection");
    assert_eq!(p07_selection.stage_id(), "p07");
    assert_eq!(p07_selection.lane(), "consumer-core");
    assert!(p07_selection.uses_runtime_field());
    assert!(p07_selection.uses_p02_presentation());
    assert!(!p07_selection.uses_gpu_light_field());
    assert!(p07_selection.uses_cpu_consumers());
    assert!(!p07_selection.supports_renderdoc_capture());

    let mut p08 = p07;
    p08[4] = "p08".to_string();
    let p08_selection = super::parse_rtt_light_selection(&p08, super::PerfWorkload::IndoorLight)
        .expect("p08/consumer-core v1 is implemented")
        .expect("P08 consumer-core requires an explicit selection");
    assert_eq!(p08_selection.stage_id(), "p08");
    assert_eq!(p08_selection.lane(), "consumer-core");
    assert!(p08_selection.uses_runtime_field());
    assert!(p08_selection.uses_p02_presentation());
    assert!(p08_selection.uses_gpu_light_field());
    assert!(p08_selection.uses_cpu_consumers());
    assert!(!p08_selection.supports_renderdoc_capture());

    let mut wrong_stage = exact;
    wrong_stage[4] = "p09".to_string();
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
    assert_eq!(
        super::PerfBehaviorCase::parse("load-recovery-failed-v1"),
        Some(super::PerfBehaviorCase::LoadRecoveryFailedV1)
    );
    assert_eq!(super::PerfBehaviorCase::parse("unknown"), None);
}
