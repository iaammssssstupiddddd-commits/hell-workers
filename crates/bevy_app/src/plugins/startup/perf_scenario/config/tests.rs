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
