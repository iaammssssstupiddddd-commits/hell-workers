use super::{PerfClockMode, PerfScenarioConfig, wall_density_window_contract_matches};
use hw_core::quality::RttQualityPreset;

fn parse_input(
    args: &[&str],
    environment: &[(&str, &str)],
) -> Result<PerfScenarioConfig, super::PerfScenarioConfigError> {
    let args: Vec<_> = args.iter().map(|arg| (*arg).to_string()).collect();
    PerfScenarioConfig::try_from_input(
        &super::PerfConfigInput {
            args: &args,
            environment: &|key| {
                environment
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, value)| std::ffi::OsString::from(value))
            },
        },
        || 99,
    )
}

#[cfg(feature = "profiling")]
#[test]
fn building_art_static_rejects_wrong_population_clock_and_measurement_contract() {
    for (size, souls) in [("small", "15"), ("medium", "60")] {
        let args = vec![
            "--perf-scenario",
            "--perf-workload",
            "building-art-static",
            "--perf-size",
            size,
            "--spawn-souls",
            souls,
            "--spawn-familiars",
            "0",
            "--perf-seed",
            "20260920",
            "--perf-output-dir",
            "/tmp/unused-building-art-config-test",
            "--perf-window-width",
            "1280",
            "--perf-window-height",
            "720",
            "--perf-window-scale-factor",
            "1",
            "--perf-rtt-quality",
            "high",
        ];
        let config = parse_input(&args, &[]).unwrap();
        assert!(config.keeps_virtual_time_paused_during_capture());
        assert!(config.uses_isolated_density_world());
        assert_eq!(config.soul_count().to_string(), souls);
        for (flag, bad) in [
            ("--spawn-souls", "0"),
            ("--spawn-familiars", "1"),
            ("--perf-size", "large"),
            ("--perf-seed", "20260906"),
            ("--perf-window-width", "1920"),
        ] {
            let mut invalid = args.clone();
            let index = invalid.iter().position(|&arg| arg == flag).unwrap();
            invalid[index + 1] = bad;
            assert!(parse_input(&invalid, &[]).is_err(), "{flag}={bad}");
        }
        for extra in [
            ["--perf-render", "cpu"],
            ["--perf-clock", "fixed"],
            ["--perf-warmup-secs", "1"],
            ["--perf-measure-secs", "2"],
        ] {
            let mut invalid = args.clone();
            invalid.extend(extra);
            assert!(parse_input(&invalid, &[]).is_err(), "{extra:?}");
        }
    }
}

#[cfg(feature = "profiling")]
#[test]
fn dashboard_fixture_opens_the_management_shell_for_visible_modes_test() {
    use super::super::fixture::{PerfScenarioApplied, setup_perf_ui_mode_if_enabled};
    use bevy::prelude::*;
    use hw_ui::components::{LeftPanelMode, OperationDialog, OperationDialogState};
    use hw_ui::panels::task_list::{TaskDashboardViewState, TaskListDirty, TaskWorkTypeFilter};
    use hw_ui::shell::UiShellState;
    for mode in ["hidden", "visible", "active-filter"] {
        let config = parse_input(
            &[
                "--perf-scenario",
                "--perf-workload",
                "task-dashboard",
                "--perf-dashboard",
                mode,
            ],
            &[],
        )
        .unwrap();
        let mut app = App::new();
        app.insert_resource(config)
            .init_resource::<PerfScenarioApplied>()
            .init_resource::<OperationDialogState>()
            .init_resource::<LeftPanelMode>()
            .init_resource::<UiShellState>()
            .init_resource::<TaskDashboardViewState>()
            .init_resource::<TaskListDirty>()
            .add_systems(Update, setup_perf_ui_mode_if_enabled);
        app.world_mut()
            .resource_mut::<PerfScenarioApplied>()
            .workload = true;
        app.world_mut().spawn((OperationDialog, Node::default()));
        app.update();
        assert_eq!(
            app.world().resource::<UiShellState>().management_open(),
            mode != "hidden"
        );
        assert_eq!(
            *app.world().resource::<LeftPanelMode>(),
            if mode == "hidden" {
                LeftPanelMode::EntityList
            } else {
                LeftPanelMode::TaskList
            }
        );
        assert_eq!(
            app.world().resource::<TaskDashboardViewState>().work_type,
            if mode == "active-filter" {
                TaskWorkTypeFilter::Only(hw_jobs::WorkType::Chop)
            } else {
                TaskWorkTypeFilter::All
            }
        );
        assert!(app.world().resource::<PerfScenarioApplied>().complete());
    }
}

fn parse_selection(
    args: &[String],
    workload: super::PerfWorkload,
) -> Result<Option<super::PerfRttLightSelection>, super::PerfScenarioConfigError> {
    super::parse_rtt_light_selection(
        &super::PerfConfigInput {
            args,
            environment: &|_| None,
        },
        workload,
    )
}

#[cfg(feature = "profiling")]
type WorkloadInput = (
    &'static str,
    Vec<&'static str>,
    Vec<(&'static str, &'static str)>,
);

#[cfg(feature = "profiling")]
fn workload_inputs() -> Vec<WorkloadInput> {
    vec![
        ("gather", vec![], vec![]),
        ("path-door", vec![], vec![]),
        ("construction", vec![], vec![]),
        ("ui-gpu", vec![], vec![]),
        (
            "task-dashboard",
            vec!["--perf-dashboard", "active-filter"],
            vec![],
        ),
        (
            "dream-ui-burst",
            vec!["--perf-size", "small", "--perf-render", "cpu"],
            vec![],
        ),
        (
            "indoor-light",
            vec![
                "--perf-contract",
                "rtt-light-v1",
                "--perf-stage",
                "p08",
                "--perf-lane",
                "static",
            ],
            vec![],
        ),
        (
            "wall-density",
            vec![
                "--perf-wall-phase",
                "completed",
                "--spawn-souls",
                "0",
                "--spawn-familiars",
                "0",
                "--perf-seed",
                "20260901",
                "--perf-output-dir",
                "/tmp/perf-test-unused",
                "--perf-window-width",
                "1280",
                "--perf-window-height",
                "720",
                "--perf-window-scale-factor",
                "1",
                "--perf-rtt-quality",
                "high",
            ],
            vec![],
        ),
        (
            "door-density",
            vec![
                "--perf-door-presentation",
                "production",
                "--spawn-souls",
                "0",
                "--spawn-familiars",
                "0",
                "--perf-seed",
                "20260906",
                "--perf-output-dir",
                "/tmp/perf-test-unused",
                "--perf-window-width",
                "1280",
                "--perf-window-height",
                "720",
                "--perf-window-scale-factor",
                "1",
                "--perf-rtt-quality",
                "high",
            ],
            vec![("HW_DOOR_PERF_PRESENTATION", "production")],
        ),
        (
            "deconstruction",
            vec!["--perf-render", "cpu", "--perf-clock", "fixed"],
            vec![],
        ),
        (
            "save-transaction",
            vec![],
            vec![("HW_PERF_SAVE_RUNTIME_ROOT", "/tmp/perf-test-unused")],
        ),
    ]
}

#[test]
fn input_disabled_and_feature_rejection_never_consume_a_seed_test() {
    let disabled = ["--perf-workload".to_string()];
    let config = PerfScenarioConfig::try_from_input(
        &super::PerfConfigInput {
            args: &disabled,
            environment: &|_| None,
        },
        || panic!("disabled must not request seed"),
    )
    .unwrap();
    assert!(!config.enabled());
    assert_eq!(config.master_seed(), 0);
    #[cfg(not(feature = "profiling"))]
    {
        let args = ["--perf-scenario".to_string(), "--perf-workload".to_string()];
        let error = PerfScenarioConfig::try_from_input(
            &super::PerfConfigInput {
                args: &args,
                environment: &|_| None,
            },
            || panic!("feature rejection precedes seed"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("requires the profiling feature"));
    }
}

#[cfg(feature = "profiling")]
#[test]
fn input_precedence_duplicates_seed_and_error_order_are_preserved_test() {
    let config = parse_input(
        &[
            "--perf-scenario",
            "--perf-seed",
            "7",
            "--perf-seed",
            "8",
            "--unknown",
            "ignored",
            "--perf-fixed-hz",
            "0",
        ],
        &[("HW_PERF_SEED", "9")],
    )
    .unwrap();
    assert_eq!(config.master_seed(), 7);
    assert_eq!(
        config.fixed_step_hz(),
        0,
        "inactive realtime values remain observable"
    );
    assert_eq!(
        parse_input(
            &["--perf-scenario"],
            &[("HW_PERF_SEED", "9"), ("HELL_WORKERS_WORLDGEN_SEED", "10")]
        )
        .unwrap()
        .master_seed(),
        9
    );
    assert_eq!(
        parse_input(
            &["--perf-scenario"],
            &[("HELL_WORKERS_WORLDGEN_SEED", "10")]
        )
        .unwrap()
        .master_seed(),
        10
    );
    assert_eq!(
        parse_input(&["--perf-scenario"], &[])
            .unwrap()
            .master_seed(),
        99
    );
    assert!(
        parse_input(&["--perf-scenario", "--perf-seed"], &[])
            .unwrap_err()
            .to_string()
            .contains("requires a value")
    );
    assert!(
        parse_input(
            &[
                "--perf-scenario",
                "--perf-clock",
                "fixed",
                "--perf-fixed-hz",
                "0",
                "--perf-seed",
                "bad"
            ],
            &[]
        )
        .unwrap_err()
        .to_string()
        .contains("greater than 0")
    );
    assert!(
        parse_input(
            &["--perf-scenario", "--perf-door-presentation", "production"],
            &[]
        )
        .unwrap_err()
        .to_string()
        .contains("paired and equal")
    );
    let calls = std::cell::Cell::new(0);
    let args = [
        "--perf-scenario".to_string(),
        "--perf-renderdoc-capture".to_string(),
        "--perf-measure-secs".to_string(),
        "0".to_string(),
    ];
    let result = PerfScenarioConfig::try_from_input(
        &super::PerfConfigInput {
            args: &args,
            environment: &|_| None,
        },
        || {
            calls.set(calls.get() + 1);
            99
        },
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("--perf-measure-secs")
    );
    assert_eq!(calls.get(), 1);
    #[cfg(not(feature = "profiling-renderdoc"))]
    {
        calls.set(0);
        let args = [
            "--perf-scenario".to_string(),
            "--perf-renderdoc-capture".to_string(),
        ];
        let error = PerfScenarioConfig::try_from_input(
            &super::PerfConfigInput {
                args: &args,
                environment: &|_| None,
            },
            || {
                calls.set(calls.get() + 1);
                99
            },
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("requires the profiling-renderdoc feature")
        );
        assert_eq!(calls.get(), 1);
    }
}

#[cfg(feature = "profiling")]
#[test]
fn field_core_preserves_the_preexisting_behavior_case_projection_test() {
    let config = parse_input(
        &[
            "--perf-scenario",
            "--perf-workload",
            "indoor-light",
            "--perf-contract",
            "rtt-light-v1",
            "--perf-stage",
            "p04",
            "--perf-lane",
            "field-core",
            "--perf-size",
            "large",
            "--perf-render",
            "cpu",
            "--perf-clock",
            "fixed",
            "--perf-output-dir",
            "/tmp/perf-test-unused",
            "--perf-behavior-case",
            "door-state-v1",
        ],
        &[],
    )
    .unwrap();
    assert_eq!(config.behavior_case_as_str(), Some("door-state-v1"));
    assert_eq!(config.rtt_light_selection().unwrap().lane(), "field-core");
}

#[cfg(all(feature = "profiling", unix))]
#[test]
fn save_runtime_root_accepts_non_unicode_absolute_os_paths_test() {
    use std::os::unix::ffi::OsStringExt;
    let args = ["--perf-scenario", "--perf-workload", "save-transaction"].map(str::to_string);
    let config = PerfScenarioConfig::try_from_input(
        &super::PerfConfigInput {
            args: &args,
            environment: &|key| {
                (key == "HW_PERF_SAVE_RUNTIME_ROOT")
                    .then(|| std::ffi::OsString::from_vec(b"/tmp/perf-\xff".to_vec()))
            },
        },
        || 99,
    )
    .unwrap();
    assert_eq!(config.workload(), super::PerfWorkload::SaveTransaction);
}

#[test]
fn joint_release_rejects_candidate_and_preview_opt_ins() {
    for release in [false, true] {
        for wall_candidate in [false, true] {
            for door_candidate in [false, true] {
                for wall_preview in [false, true] {
                    for door_preview in [false, true] {
                        let expected = !wall_preview
                            && !door_preview
                            && if release {
                                !wall_candidate && !door_candidate
                            } else {
                                wall_candidate && door_candidate
                            };
                        assert_eq!(
                            super::joint_asset_opt_ins_match(
                                release,
                                [wall_candidate, door_candidate],
                                [wall_preview, door_preview],
                            ),
                            expected,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn renderdoc_capture_requires_its_dedicated_feature() {
    assert!(!super::resolve_renderdoc_capture(false).unwrap());
    #[cfg(feature = "profiling-renderdoc")]
    assert!(super::resolve_renderdoc_capture(true).unwrap());
    #[cfg(not(feature = "profiling-renderdoc"))]
    assert!(super::resolve_renderdoc_capture(true).is_err());
}

#[cfg(feature = "profiling")]
#[test]
fn random_streams_are_stable_and_independent() {
    use super::{PerfRandomStream, splitmix64};
    let config = parse_input(
        &[
            "--perf-scenario",
            "--perf-seed",
            "42",
            "--perf-size",
            "small",
            "--perf-render",
            "cpu",
        ],
        &[],
    )
    .unwrap();
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
    let gpu_config = parse_input(&["--perf-scenario", "--perf-seed", "42"], &[]).unwrap();
    assert!(!gpu_config.omits_3d_scene_roots());
    assert_eq!(splitmix64(42), splitmix64(42));
    assert_ne!(splitmix64(42), splitmix64(43));
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
    let args = [
        "--perf-window-width",
        "1920",
        "--perf-window-height",
        "1080",
        "--perf-window-scale-factor",
        "2",
        "--perf-rtt-quality",
        "low",
    ];
    let config = parse_input(&args, &[]).unwrap();
    assert_eq!(config.requested_window_size(), None);
    assert_eq!(config.requested_window_scale_factor(), None);
    assert_eq!(config.requested_rtt_quality(), None);
    #[cfg(feature = "profiling")]
    {
        let args: Vec<_> = ["--perf-scenario"].into_iter().chain(args).collect();
        let config = parse_input(&args, &[]).unwrap();
        assert_eq!(config.requested_window_size(), Some((1920, 1080)));
        assert_eq!(config.requested_window_scale_factor(), Some(2.0));
        assert_eq!(config.requested_rtt_quality(), Some(RttQualityPreset::Low));
    }
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
fn invalid_workload_precedes_renderdoc_feature_check_without_consuming_seed_test() {
    let args = [
        "--perf-scenario",
        "--perf-workload",
        "invalid-workload",
        "--perf-renderdoc-capture",
    ]
    .map(str::to_string);
    let calls = std::cell::Cell::new(0);
    let error = PerfScenarioConfig::try_from_input(
        &super::PerfConfigInput {
            args: &args,
            environment: &|_| None,
        },
        || {
            calls.set(calls.get() + 1);
            99
        },
    )
    .unwrap_err()
    .to_string();
    if cfg!(feature = "profiling") {
        assert!(error.contains("--perf-workload must be one of"), "{error}");
        assert!(error.contains("got 'invalid-workload'"), "{error}");
    } else {
        assert!(error.contains("requires the profiling feature"), "{error}");
    }
    assert_eq!(calls.get(), 0);
}

#[cfg(feature = "profiling")]
#[test]
fn workload_config_projection_is_compatible_test() {
    let inputs = workload_inputs();
    assert_eq!(inputs.len(), 11);
    for (workload, extra, env) in inputs {
        let args: Vec<_> = ["--perf-scenario", "--perf-workload", workload]
            .into_iter()
            .chain(extra)
            .collect();
        let config = parse_input(&args, &env).unwrap();
        let expected = match workload {
            "gather" | "path-door" | "construction" | "ui-gpu" | "task-dashboard"
            | "indoor-light" | "save-transaction" => (99, 200, 12, "medium", "gpu", "realtime"),
            "dream-ui-burst" => (99, 50, 4, "small", "cpu", "realtime"),
            "deconstruction" => (99, 200, 12, "medium", "cpu", "fixed"),
            "wall-density" => (20260901, 0, 0, "medium", "gpu", "realtime"),
            "door-density" => (20260906, 0, 0, "medium", "gpu", "realtime"),
            _ => panic!("missing projection for {workload}"),
        };
        assert_eq!(
            (
                config.master_seed(),
                config.soul_count(),
                config.familiar_count(),
                config.size().as_str(),
                config.render_mode().as_str(),
                config.clock_mode_as_str()
            ),
            expected,
            "{workload}"
        );
        let density = matches!(workload, "wall-density" | "door-density");
        assert_eq!(
            config.requested_window_size(),
            density.then_some((1280, 720))
        );
        assert_eq!(
            config.requested_window_scale_factor(),
            density.then_some(1.0)
        );
        assert_eq!(
            config.requested_rtt_quality(),
            density.then_some(RttQualityPreset::High)
        );
        assert_eq!(
            config.output_dir(),
            density.then_some(std::path::Path::new("/tmp/perf-test-unused"))
        );
        assert_eq!(
            config.wall_phase().map(|phase| phase.as_str()),
            (workload == "wall-density").then_some("completed")
        );
        assert_eq!((config.warmup_secs(), config.measure_secs()), (30.0, 60.0));
        assert_eq!(
            (
                config.fixed_step_hz(),
                config.fixed_warmup_ticks(),
                config.fixed_audit_ticks()
            ),
            (64, 1920, 128)
        );
        assert_eq!(
            config.dashboard_mode().as_str(),
            if workload == "task-dashboard" {
                "active-filter"
            } else {
                "hidden"
            }
        );
        assert_eq!(
            config.door_presentation().map(|value| value.as_str()),
            (workload == "door-density").then_some("production")
        );
        assert_eq!(config.wall_presentation(), None);
        assert_eq!(config.behavior_case_as_str(), None);
        assert_eq!(
            config.rtt_light_selection().map(|value| (
                value.contract_id(),
                value.stage_id(),
                value.lane()
            )),
            (workload == "indoor-light").then_some(("rtt-light-v1", "p08", "static"))
        );
    }
}

#[cfg(feature = "profiling")]
#[test]
fn every_enabled_fixture_freezes_until_the_initial_checkpoint() {
    for (workload, extra, env) in workload_inputs() {
        let args: Vec<_> = ["--perf-scenario", "--perf-workload", workload]
            .into_iter()
            .chain(extra)
            .collect();
        let config = parse_input(&args, &env).unwrap_or_else(|error| panic!("{workload}: {error}"));
        assert!(config.enabled());
        assert_eq!(config.workload().as_str(), workload);
        assert!(config.freezes_fixture_setup());
        assert_eq!(
            config.uses_isolated_density_world(),
            matches!(workload, "wall-density" | "door-density")
        );
        assert_eq!(
            config.keeps_virtual_time_paused_during_capture(),
            matches!(workload, "indoor-light" | "wall-density" | "door-density")
        );
    }
    for (stage, lane, size, clock) in [
        ("p02", "static", "medium", "realtime"),
        ("p02", "behavior", "small", "fixed-behavior"),
        ("p04", "field-core", "large", "fixed"),
        ("p03", "field-core", "large", "fixed"),
        ("p08", "static", "medium", "fixed"),
        ("p08", "behavior", "small", "fixed-behavior"),
        ("p07", "static", "medium", "realtime"),
    ] {
        let mut args = vec![
            "--perf-scenario",
            "--perf-workload",
            "indoor-light",
            "--perf-contract",
            "rtt-light-v1",
            "--perf-stage",
            stage,
            "--perf-lane",
            lane,
            "--perf-size",
            size,
            "--perf-render",
            "cpu",
            "--perf-clock",
            clock,
            "--perf-output-dir",
            "/tmp/perf-test-unused",
        ];
        if lane == "behavior" {
            args.extend(["--perf-behavior-case", "door-state-v1"]);
        }
        let config = parse_input(&args, &[]).unwrap();
        assert_eq!(
            config.freezes_indoor_light_door_automation(),
            lane != "behavior"
        );
        assert_eq!(
            config.pauses_virtual_time_for_field_core(),
            lane == "field-core" && stage != "p03"
        );
        assert_eq!(
            config.requires_p08_cross_consumer_setup_step(),
            stage == "p08" && lane == "static"
        );
    }
    let config = PerfScenarioConfig::default();
    assert!(!config.freezes_fixture_setup());
    assert!(!config.keeps_virtual_time_paused_during_capture());
    assert!(!config.uses_isolated_density_world());
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
    assert_eq!(
        super::PerfWallPhase::parse("mixed"),
        Some(super::PerfWallPhase::Mixed)
    );
    assert_eq!(super::PerfWallPhase::parse("unknown"), None);
    assert_eq!(super::PerfWallPhase::Completed.as_str(), "completed");
    assert_eq!(super::PerfWallPhase::Provisional.as_str(), "provisional");
    assert_eq!(super::PerfWallPhase::Mixed.as_str(), "mixed");
}

#[test]
fn wall_density_presentation_names_are_explicit() {
    assert_eq!(
        super::PerfWallPresentation::parse("production"),
        Some(super::PerfWallPresentation::Production)
    );
    assert_eq!(
        super::PerfWallPresentation::parse("fallback-control"),
        Some(super::PerfWallPresentation::FallbackControl)
    );
    assert_eq!(super::PerfWallPresentation::parse("fallback"), None);
    assert_eq!(
        super::PerfWallPresentation::Production.as_str(),
        "production"
    );
    assert_eq!(
        super::PerfWallPresentation::FallbackControl.as_str(),
        "fallback-control"
    );
}

#[test]
fn door_density_presentation_names_are_explicit() {
    assert_eq!(
        super::PerfDoorPresentation::parse("production"),
        Some(super::PerfDoorPresentation::Production)
    );
    assert_eq!(
        super::PerfDoorPresentation::parse("fallback-control"),
        Some(super::PerfDoorPresentation::FallbackControl)
    );
    assert_eq!(super::PerfDoorPresentation::parse("fallback"), None);
    assert_eq!(
        super::PerfDoorPresentation::Production.as_str(),
        "production"
    );
    assert_eq!(
        super::PerfDoorPresentation::FallbackControl.as_str(),
        "fallback-control"
    );
}

#[test]
fn wall_density_actual_window_has_a_distinct_duration_contract() {
    assert!(super::wall_density_durations_match(false, 30.0, 60.0));
    assert!(!super::wall_density_durations_match(false, 10.0, 10.0));
    assert!(super::wall_density_durations_match(true, 10.0, 10.0));
    assert!(!super::wall_density_durations_match(true, 30.0, 60.0));
}

#[test]
fn wall_formwork_gallery_is_the_only_mixed_actual_window_profile() {
    use super::PerfWallPhase;

    assert!(super::wall_actual_window_phase_matches(
        true,
        false,
        false,
        false,
        Some(PerfWallPhase::Completed),
    ));
    assert!(!super::wall_actual_window_phase_matches(
        true,
        false,
        false,
        false,
        Some(PerfWallPhase::Provisional),
    ));
    assert!(super::wall_actual_window_phase_matches(
        true,
        false,
        true,
        false,
        Some(PerfWallPhase::Provisional),
    ));
    assert!(super::wall_actual_window_phase_matches(
        true,
        false,
        false,
        true,
        Some(PerfWallPhase::Mixed),
    ));
    assert!(!super::wall_actual_window_phase_matches(
        true,
        false,
        false,
        true,
        Some(PerfWallPhase::Provisional),
    ));
    assert!(!super::wall_actual_window_phase_matches(
        false,
        true,
        true,
        false,
        Some(PerfWallPhase::Provisional),
    ));
}

#[test]
fn wall_art_matrix_has_a_scoped_quality_and_dpi_contract() {
    assert!(wall_density_window_contract_matches(
        false,
        Some(1280),
        Some(720),
        Some(1.0),
        Some(RttQualityPreset::High),
    ));
    assert!(!wall_density_window_contract_matches(
        false,
        Some(1280),
        Some(720),
        Some(1.5),
        Some(RttQualityPreset::High),
    ));
    for quality in [
        RttQualityPreset::High,
        RttQualityPreset::Medium,
        RttQualityPreset::Low,
    ] {
        for scale_factor in [1.0, 1.5, 2.0] {
            assert!(wall_density_window_contract_matches(
                true,
                Some(1280),
                Some(720),
                Some(scale_factor),
                Some(quality),
            ));
        }
    }
    assert!(!wall_density_window_contract_matches(
        true,
        Some(1280),
        Some(720),
        Some(1.25),
        Some(RttQualityPreset::Medium),
    ));
}

#[test]
fn wall_art_preview_accepts_completed_and_provisional_but_not_mixed_or_non_window() {
    for phase in [
        super::PerfWallPhase::Completed,
        super::PerfWallPhase::Provisional,
    ] {
        assert!(super::wall_actual_window_phase_matches(
            true,
            false,
            true,
            false,
            Some(phase)
        ));
        assert!(!super::wall_actual_window_phase_matches(
            false,
            false,
            true,
            false,
            Some(phase)
        ));
    }
    assert!(!super::wall_actual_window_phase_matches(
        true,
        false,
        true,
        false,
        Some(super::PerfWallPhase::Mixed)
    ));
    assert!(!super::wall_actual_window_phase_matches(
        true, false, true, false, None
    ));
}

#[test]
fn wall_art_zoom_requires_paired_keys_and_the_actual_window_profile() {
    assert_eq!(super::wall_art_zoom_selection(None, None, false), Ok(false));
    assert_eq!(
        super::wall_art_zoom_selection(Some("farthest"), Some("farthest"), true),
        Ok(true)
    );
    assert!(super::wall_art_zoom_selection(Some("farthest"), None, true).is_err());
    assert!(super::wall_art_zoom_selection(None, Some("farthest"), true).is_err());
    assert!(super::wall_art_zoom_selection(Some("standard"), Some("standard"), true).is_err());
    assert!(super::wall_art_zoom_selection(Some("farthest"), Some("farthest"), false).is_err());
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
    let selection = parse_selection(&exact, super::PerfWorkload::IndoorLight)
        .expect("current/static v1 is the implemented vertical slice")
        .expect("indoor-light requires an explicit selection");
    assert_eq!(selection.contract_id(), "rtt-light-v1");
    assert_eq!(selection.stage_id(), "current");
    assert_eq!(selection.lane(), "static");

    let missing_lane = exact[..exact.len() - 2].to_vec();
    assert!(parse_selection(&missing_lane, super::PerfWorkload::IndoorLight).is_err());
    assert!(parse_selection(&exact, super::PerfWorkload::Gather).is_err());

    let mut p01 = exact.clone();
    p01[4] = "p01".to_string();
    let p01_selection = parse_selection(&p01, super::PerfWorkload::IndoorLight)
        .expect("p01/static v1 is implemented")
        .expect("P01 requires an explicit selection");
    assert_eq!(p01_selection.stage_id(), "p01");
    let mut p01_behavior = p01;
    p01_behavior[6] = "behavior".to_string();
    let p01_behavior_selection = parse_selection(&p01_behavior, super::PerfWorkload::IndoorLight)
        .expect("p01/behavior v1 is implemented")
        .expect("P01 behavior requires an explicit selection");
    assert_eq!(p01_behavior_selection.lane(), "behavior");

    let mut p02 = p01_behavior;
    p02[4] = "p02".to_string();
    let p02_selection = parse_selection(&p02, super::PerfWorkload::IndoorLight)
        .expect("p02/behavior v1 is implemented")
        .expect("P02 requires an explicit selection");
    assert_eq!(p02_selection.stage_id(), "p02");
    assert_eq!(p02_selection.lane(), "behavior");

    let mut p03 = exact.clone();
    p03[4] = "p03".to_string();
    p03[6] = "field-core".to_string();
    let p03_selection = parse_selection(&p03, super::PerfWorkload::IndoorLight)
        .expect("p03/field-core v1 is implemented")
        .expect("P03 field-core requires an explicit selection");
    assert_eq!(p03_selection.stage_id(), "p03");
    assert_eq!(p03_selection.lane(), "field-core");
    assert!(p03_selection.uses_p02_presentation());
    assert!(!p03_selection.uses_runtime_field());

    let mut p04 = p03.clone();
    p04[4] = "p04".to_string();
    let p04_selection = parse_selection(&p04, super::PerfWorkload::IndoorLight)
        .expect("p04/field-core v1 is implemented")
        .expect("P04 field-core requires an explicit selection");
    assert_eq!(p04_selection.stage_id(), "p04");
    assert_eq!(p04_selection.lane(), "field-core");
    assert!(p04_selection.uses_p02_presentation());
    assert!(p04_selection.uses_runtime_field());
    assert!(!p04_selection.supports_renderdoc_capture());

    p04[6] = "static".to_string();
    let p04_static_selection = parse_selection(&p04, super::PerfWorkload::IndoorLight)
        .expect("p04/static v1 is implemented")
        .expect("P04 static requires an explicit selection");
    assert!(p04_static_selection.supports_renderdoc_capture());

    let mut p05 = p04;
    p05[4] = "p05".to_string();
    let p05_selection = parse_selection(&p05, super::PerfWorkload::IndoorLight)
        .expect("p05/static v1 is implemented")
        .expect("P05 static requires an explicit selection");
    assert_eq!(p05_selection.stage_id(), "p05");
    assert!(p05_selection.uses_runtime_field());
    assert!(p05_selection.uses_p02_presentation());

    let mut p06 = p05;
    p06[4] = "p06".to_string();
    let p06_selection = parse_selection(&p06, super::PerfWorkload::IndoorLight)
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
    let p07_selection = parse_selection(&p07, super::PerfWorkload::IndoorLight)
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
    let p08_selection = parse_selection(&p08, super::PerfWorkload::IndoorLight)
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
    assert!(parse_selection(&wrong_stage, super::PerfWorkload::IndoorLight).is_err());

    let mut behavior = wrong_stage;
    behavior[4] = "current".to_string();
    behavior[6] = "behavior".to_string();
    let behavior_selection = parse_selection(&behavior, super::PerfWorkload::IndoorLight)
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
