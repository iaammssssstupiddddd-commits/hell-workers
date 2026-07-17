from __future__ import annotations

from .compare import *

def write_fixture_run(
    root: Path,
    *,
    warning: bool = False,
    teardown_warning: bool = False,
    fixed_step_audit: bool = False,
) -> None:
    run_dir = root / "data"
    run_dir.mkdir(parents=True)
    if fixed_step_audit:
        timestep_ns = 15_625_000
        checkpoints = [
            ("fixture-pre-update", 0),
            *DETERMINISM_EARLY_CHECKPOINTS,
            ("post-warmup", 1920),
            ("post-audit-end", 2048),
        ]
        with (run_dir / "determinism.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=DETERMINISM_COLUMNS)
            writer.writeheader()
            for index, (checkpoint, tick) in enumerate(checkpoints):
                elapsed = tick * timestep_ns
                writer.writerow(
                    {
                        "schema_version": DETERMINISM_SCHEMA_VERSION,
                        "checkpoint": checkpoint,
                        "update_tick": str(tick),
                        "fixed_timestep_ns": str(timestep_ns),
                        "virtual_delta_ns": "0" if index == 0 else str(timestep_ns),
                        "virtual_elapsed_ns": str(elapsed),
                        "fixed_delta_ns": "0" if index == 0 else str(timestep_ns),
                        "fixed_elapsed_ns": str(elapsed),
                        "fixed_overstep_ns": "0",
                        "virtual_paused": "1" if index == 0 else "0",
                        "virtual_relative_speed_bits": ONE_F64_BITS,
                        "virtual_effective_speed_bits": ZERO_F64_BITS if index == 0 else ONE_F64_BITS,
                        "souls": "0",
                        "familiars": "0",
                        "designations": "0",
                        "state_checksum": "0000000000000000",
                    }
                )
    else:
        summary = {column: "0" for column in EXPECTED_SUMMARY_COLUMNS}
        summary.update(
            {
                "schema_version": SUMMARY_SCHEMA_VERSION,
                "seed": str(DEFAULT_SEED),
                "workload": "gather",
                "size": "small",
                "render": "cpu",
                "samples": "1",
                "p50_ms": "1.0",
                "p95_ms": "1.0",
                "p99_ms": "1.0",
                "max_ms": "1.0",
            }
        )
        with (run_dir / "summary.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=sorted(EXPECTED_SUMMARY_COLUMNS))
            writer.writeheader()
            writer.writerow(summary)
        (run_dir / "frames.csv").write_text("frame_index,frame_time_ms\n0,1.0\n", encoding="utf-8")
        with (run_dir / "scene_roots.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=SCENE_ROOT_COLUMNS)
            writer.writeheader()
            writer.writerow({column: "0" for column in SCENE_ROOT_COLUMNS})
    extra = "2026 WARN unexpected warning\n" if warning else ""
    teardown_extra = "2026 WARN teardown warning\n" if teardown_warning else ""
    (root / "run.log").write_text(
        (
            "PERF_SCENARIO: seed=20260712 workload=gather size=small souls=50 familiars=4 "
            "render=cpu clock=fixed fixed_hz=64 fixed_warmup_ticks=1920 fixed_audit_ticks=128\n"
            if fixed_step_audit
            else "PERF_SCENARIO: seed=20260712 workload=gather size=small souls=50 familiars=4 "
            "render=cpu clock=realtime\n"
        )
        + "AdapterInfo { name: \"Test GPU\", driver: \"test\", driver_info: \"test\", backend: Vulkan }\n"
        + extra
        + (
            "PERF_DETERMINISM_AUDIT: wrote 7 checkpoints to x\n"
            if fixed_step_audit
            else "PERF_CAPTURE: wrote 1 samples to x\n"
        )
        + teardown_extra,
        encoding="utf-8",
    )


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        write_fixture_run(root, teardown_warning=True)
        case = Case("gather", "small", "cpu", DEFAULT_SEED, None, None)
        validation = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert validation.valid, validation.reasons
        assert validation.teardown_warning_lines == ["2026 WARN teardown warning"]
        shutil.rmtree(root / "data")
        invalid = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not invalid.valid and any("missing summary.csv" in reason for reason in invalid.reasons)

        session = root / "session"
        run_dirs = [
            session / "cases" / case.identifier / "run-001",
            session / "cases" / case.identifier / "run-002",
        ]
        for run_dir in run_dirs:
            write_fixture_run(run_dir)
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            session / "manifest.json",
            {"matrix": {"warmup_checksum_policy": "require"}, "actual_adapters": []},
        )
        assert summarize_session(session)
        with (session / "aggregate.csv").open(newline="", encoding="utf-8") as handle:
            aggregate = list(csv.DictReader(handle))
        assert aggregate and "max_mad_ms" in aggregate[0]

        summary_path = run_dirs[1] / "data" / "summary.csv"
        with summary_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            fieldnames = reader.fieldnames
            rows = list(reader)
        assert fieldnames is not None and len(rows) == 1
        rows[0]["initial_state_checksum"] = "0000000000000001"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)
        validation = validate_run(
            run_dirs[1],
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert validation.valid, validation.reasons
        write_json(run_dirs[1] / "validation.json", validation.to_json())
        assert not summarize_session(session)
        invalidated = json.loads((run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("initial_state_checksum differs" in reason for reason in invalidated["reasons"])

        rows[0]["initial_state_checksum"] = "0"
        rows[0]["warmup_state_checksum"] = "0000000000000002"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)
        for run_dir in run_dirs:
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        assert not summarize_session(session, "require")
        invalidated = json.loads((run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("warmup_state_checksum differs" in reason for reason in invalidated["reasons"])
        assert summarize_session(session, "record")

        fixed_session = root / "fixed-session"
        fixed_run_dirs = [
            fixed_session / "cases" / case.identifier / "run-001",
            fixed_session / "cases" / case.identifier / "run-002",
        ]
        for run_dir in fixed_run_dirs:
            write_fixture_run(run_dir, fixed_step_audit=True)
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
                capture_kind="fixed-step-determinism",
                expected_fixed_hz=64,
                expected_warmup_ticks=1920,
                expected_audit_ticks=128,
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            fixed_session / "manifest.json",
            {
                "matrix": {
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                },
                "actual_adapters": [],
            },
        )
        assert summarize_session(fixed_session)
        checkpoints_path = fixed_run_dirs[1] / "data" / "determinism.csv"
        with checkpoints_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            checkpoint_fields = reader.fieldnames
            checkpoints = list(reader)
        assert checkpoint_fields is not None and checkpoints
        checkpoints[1]["state_checksum"] = "0000000000000001"
        with checkpoints_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=checkpoint_fields)
            writer.writeheader()
            writer.writerows(checkpoints)
        validation = validate_run(
            fixed_run_dirs[1],
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
            capture_kind="fixed-step-determinism",
            expected_fixed_hz=64,
            expected_warmup_ticks=1920,
            expected_audit_ticks=128,
        )
        assert validation.valid, validation.reasons
        write_json(fixed_run_dirs[1] / "validation.json", validation.to_json())
        assert not summarize_session(fixed_session)
        invalidated = json.loads((fixed_run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("determinism checkpoints differ" in reason for reason in invalidated["reasons"])

        audit_args = build_parser().parse_args(["audit", "--dry-run"])
        assert audit_args.clock_mode == "fixed"
        assert audit_args.capture_kind == "fixed-step-determinism"
        assert audit_args.fixed_hz == 64
        assert audit_args.warmup_ticks == 1920
        assert audit_args.audit_ticks == 128

        def write_comparison_fixture(
            session_dir: Path, *, sizes: list[str], renders: list[str], case_ids: list[str]
        ) -> None:
            session_dir.mkdir()
            write_json(
                session_dir / "manifest.json",
                {
                    "matrix": {
                        "workload": "gather",
                        "sizes": sizes,
                        "renders": renders,
                        "seed": DEFAULT_SEED,
                        "repeat": 3,
                        "warmup_secs": 30.0,
                        "measure_secs": 60.0,
                        "preflight_runs": 0,
                        "souls": None,
                        "familiars": None,
                        "warmup_checksum_policy": "record",
                    },
                    "actual_adapters": [{"name": "Test GPU", "backend": "Vulkan"}],
                    "requested_environment": {"WGPU_BACKEND": "vulkan"},
                    "binary": {"instrumentation": "capture"},
                },
            )
            with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(
                    handle,
                    fieldnames=["case_id", "valid_runs", "p50_median_ms"],
                )
                writer.writeheader()
                writer.writerows(
                    {
                        "case_id": case_id,
                        "valid_runs": "3",
                        "p50_median_ms": "1.0",
                    }
                    for case_id in case_ids
                )

        comparison_baseline = root / "comparison-baseline"
        comparison_candidate = root / "comparison-candidate"
        large_cpu_case = "gather-large-cpu-seed-20260712"
        write_comparison_fixture(
            comparison_baseline,
            sizes=["medium", "large"],
            renders=["cpu", "gpu"],
            case_ids=[large_cpu_case, "gather-medium-cpu-seed-20260712"],
        )
        write_comparison_fixture(
            comparison_candidate,
            sizes=["large"],
            renders=["cpu"],
            case_ids=[large_cpu_case],
        )
        comparison_args = argparse.Namespace(
            baseline=str(comparison_baseline),
            candidate=str(comparison_candidate),
            metric="p50",
            max_regression_pct=5.0,
            min_runs=3,
            output=None,
            allow_case_subset=False,
        )
        try:
            compare_sessions(comparison_args)
        except RuntimeError as error:
            assert "different matrix" in str(error)
        else:
            raise AssertionError("subset comparison unexpectedly bypassed the matrix contract")
        comparison_args.allow_case_subset = True
        assert compare_sessions(comparison_args) == 0
    print("perf.py self-test: pass")
    return 0
