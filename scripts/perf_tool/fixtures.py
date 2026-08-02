from __future__ import annotations

from .compare import *

def write_fixture_run(
    root: Path,
    *,
    warning: bool = False,
    teardown_warning: bool = False,
    fixed_step_audit: bool = False,
    familiar_policy: str = "baseline",
    operation_dialog: str = "hidden",
    dashboard_mode: str = "hidden",
    controlled_work: dict[str, str] | None = None,
    summary_overrides: dict[str, str] | None = None,
    determinism_state_checksum: str = "0000000000000000",
    workload: str = "gather",
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
                row = {field: "0" for field in DETERMINISM_COLUMNS}
                row.update(
                    {
                        "schema_version": DETERMINISM_SCHEMA_VERSION,
                        "dashboard_mode": dashboard_mode,
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
                        "structural_checksum": "0000000000000000",
                        "state_checksum": determinism_state_checksum,
                        "delegation_cycles": "0",
                        "delegation_familiars_processed": "0",
                        "candidate_membership_checks": "0",
                        "policy_disabled_rejections": "0",
                        "candidate_snapshot_attempts": "0",
                        "candidate_score_attempts": "0",
                        "worker_score_attempts": "0",
                        "source_selector_calls": "0",
                        "source_selector_scanned_items": "0",
                        "reachable_with_cache_calls": "0",
                        **({} if index == 0 else (controlled_work or {})),
                    }
                )
                writer.writerow(row)
    else:
        summary = {column: "0" for column in EXPECTED_SUMMARY_COLUMNS}
        summary.update(
            {
                "schema_version": SUMMARY_SCHEMA_VERSION,
                "seed": str(DEFAULT_SEED),
                "workload": workload,
                "size": "small",
                "render": "cpu",
                "dashboard_mode": dashboard_mode,
                "samples": "1",
                "p50_ms": "1.0",
                "p95_ms": "1.0",
                "p99_ms": "1.0",
                "max_ms": "1.0",
                "initial_state_checksum": "0000000000000000",
                "warmup_state_checksum": "0000000000000000",
                "measure_end_state_checksum": "0000000000000000",
            }
        )
        summary.update(summary_overrides or {})
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
            f"PERF_SCENARIO: seed=20260712 workload={workload} size=small souls=50 familiars=4 "
            f"render=cpu clock=fixed familiar_policy={familiar_policy} "
            f"operation_dialog={operation_dialog} fixed_hz=64 "
            f"dashboard_mode={dashboard_mode} "
            "fixed_warmup_ticks=1920 fixed_audit_ticks=128\n"
            if fixed_step_audit
            else f"PERF_SCENARIO: seed=20260712 workload={workload} size=small souls=50 familiars=4 "
            "render=cpu clock=realtime familiar_policy=baseline operation_dialog=hidden "
            f"dashboard_mode={dashboard_mode}\n"
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
        summary_path = root / "data" / "summary.csv"
        with summary_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            summary_fields = reader.fieldnames
            summary_rows = list(reader)
        assert summary_fields is not None and len(summary_rows) == 1
        summary_rows[0]["candidate_membership_checks"] = "not-a-counter"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=summary_fields)
            writer.writeheader()
            writer.writerows(summary_rows)
        malformed_counter = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not malformed_counter.valid
        assert any(
            "candidate_membership_checks must be a nonnegative integer" in reason
            for reason in malformed_counter.reasons
        )
        tracy_zones = root / "tracy-zones.csv"
        tracy_zones.write_text(
            "name,src_file,src_line,total_ns,total_perc,counts,mean_ns,min_ns,max_ns,std_ns\n"
            "task_list_update_system,update.rs,20,3000,1.0,3,1000,900,1100,10\n",
            encoding="utf-8",
        )
        zone_summary, zone_errors = read_tracy_zone_summary(tracy_zones)
        assert not zone_errors and zone_summary["mean_ns_per_invocation"] == 1000.0
        memory_csv = root / "memory.csv"
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,4096,2048,8192,7144,8,7,2,0\n",
            encoding="utf-8",
        )
        memory_summary, memory_errors = read_native_memory(
            memory_csv, frame_samples=2
        )
        assert not memory_errors
        assert memory_summary["peak_live_bytes"] == 4096
        assert memory_summary["peak_growth_bytes"] == 3096
        assert memory_summary["net_live_growth_bytes"] == 1048
        assert memory_summary["allocated_bytes_per_frame"] == 4096.0
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,900,1100,8192,8092,8,7,2,0\n",
            encoding="utf-8",
        )
        _, malformed_memory_errors = read_native_memory(memory_csv, frame_samples=1)
        assert malformed_memory_errors == [
            "native memory artifact contains invalid values"
        ]
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,4096,2048,8192,7144,8,7,9,0\n",
            encoding="utf-8",
        )
        _, malformed_reallocation_errors = read_native_memory(
            memory_csv, frame_samples=1
        )
        assert malformed_reallocation_errors == [
            "native memory artifact contains invalid values"
        ]
        fixed_profile, fixed_profile_errors = collect_profile_artifact(
            args=argparse.Namespace(
                instrumentation="capture",
                capture_kind="fixed-step-determinism",
            ),
            case=Case("task-dashboard", "small", "cpu", DEFAULT_SEED, None, None),
            run_dir=root,
            trace_returncode=None,
            frame_samples=None,
        )
        assert fixed_profile is None
        assert not fixed_profile_errors
        dashboard_cpu = root / "task_dashboard_cpu.csv"
        dashboard_cpu.write_text(
            "schema_version,system_invocations,total_elapsed_ns\n1,4,1000\n",
            encoding="utf-8",
        )
        cpu_summary, cpu_errors = read_task_dashboard_cpu(dashboard_cpu)
        assert not cpu_errors
        assert cpu_summary["mean_ns_per_invocation"] == 250.0
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

        rows[0]["initial_state_checksum"] = "0000000000000000"
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

        controlled_session = root / "controlled-session"
        controlled_cases = [
            Case(
                "gather",
                "small",
                "cpu",
                DEFAULT_SEED,
                None,
                None,
                familiar_policy,
                operation_dialog,
            )
            for familiar_policy in ("default", "disabled")
            for operation_dialog in ("hidden", "open")
        ]
        default_work = {
            "delegation_cycles": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "8",
            "policy_disabled_rejections": "0",
            "candidate_snapshot_attempts": "8",
            "candidate_score_attempts": "8",
            "worker_score_attempts": "8",
            "source_selector_calls": "1",
            "source_selector_scanned_items": "1",
            "reachable_with_cache_calls": "1",
        }
        disabled_work = {
            "delegation_cycles": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "8",
            "policy_disabled_rejections": "8",
            "candidate_snapshot_attempts": "0",
            "candidate_score_attempts": "0",
            "worker_score_attempts": "0",
            "source_selector_calls": "0",
            "source_selector_scanned_items": "0",
            "reachable_with_cache_calls": "0",
        }
        for case in controlled_cases:
            run_dir = controlled_session / "cases" / case.identifier / "run-001"
            write_fixture_run(
                run_dir,
                fixed_step_audit=True,
                familiar_policy=case.familiar_policy,
                operation_dialog=case.operation_dialog,
                controlled_work=(
                    default_work
                    if case.familiar_policy == "default"
                    else disabled_work
                ),
                determinism_state_checksum=(
                    "0000000000000001"
                    if case.familiar_policy == "default"
                    else "0000000000000002"
                ),
            )
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
            controlled_session / "manifest.json",
            {
                "matrix": {
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                    "familiar_policies": ["default", "disabled"],
                    "operation_dialog_modes": ["hidden", "open"],
                },
                "cases": [
                    asdict(case) | {"id": case.identifier}
                    for case in controlled_cases
                ],
                "actual_adapters": [],
            },
        )
        assert summarize_session(controlled_session)
        controlled_result = json.loads(
            (controlled_session / "familiar_policy_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert controlled_result["status"] == "pass"

        dashboard_session = root / "dashboard-session"
        dashboard_cases = [
            Case(
                "task-dashboard",
                "small",
                "cpu",
                DEFAULT_SEED,
                None,
                None,
                dashboard_mode=mode,
            )
            for mode in ("hidden", "visible", "active-filter")
        ]
        dashboard_common_work = {
            "delegation_cycles": "2",
            "incoming_snapshot_builds": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "16",
            "policy_disabled_rejections": "0",
            "candidate_snapshot_attempts": "16",
            "candidate_score_attempts": "16",
            "worker_score_attempts": "16",
            "top_k_partition_runs": "4",
            "top_k_retained_candidates": "16",
            "top_k_fallback_candidates": "0",
            "source_selector_calls": "1",
            "source_selector_cache_build_scanned_items": "4",
            "source_selector_candidate_scanned_items": "2",
            "source_selector_scanned_items": "6",
            "reachable_with_cache_calls": "8",
            "wheelbarrow_arbitration_rebuilds": "2",
            "wheelbarrow_request_bucket_builds": "1",
            "wheelbarrow_bucket_items_scanned": "3",
            "wheelbarrow_candidates_after_top_k": "3",
            "runtime_path_total_core_searches": "4",
            "runtime_path_actor_new_core_searches": "4",
            "dashboard_state_rebuilds": "2",
            "dashboard_snapshot_rows_scanned": "130",
            "dashboard_summary_rows_scanned": "130",
            "dashboard_snapshot_changes": "2",
            "dashboard_summary_changes": "2",
        }
        dashboard_render_work = {
            "hidden": {},
            "visible": {
                "dashboard_render_rebuilds": "2",
                "dashboard_render_input_rows": "130",
                "dashboard_render_visible_rows": "130",
                "dashboard_render_group_headers": "6",
                "dashboard_despawn_roots_requested": "20",
            },
            "active-filter": {
                "dashboard_render_rebuilds": "2",
                "dashboard_render_input_rows": "130",
                "dashboard_render_visible_rows": "64",
                "dashboard_render_group_headers": "2",
                "dashboard_despawn_roots_requested": "20",
            },
        }
        for case in dashboard_cases:
            run_dir = dashboard_session / "cases" / case.identifier / "run-001"
            write_fixture_run(
                run_dir,
                fixed_step_audit=True,
                workload="task-dashboard",
                dashboard_mode=case.dashboard_mode,
                controlled_work=(
                    dashboard_common_work | dashboard_render_work[case.dashboard_mode]
                ),
            )
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
            dashboard_session / "manifest.json",
            {
                "matrix": {
                    "workload": "task-dashboard",
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                    "dashboard_modes": ["hidden", "visible", "active-filter"],
                },
                "cases": [
                    asdict(case) | {"id": case.identifier} for case in dashboard_cases
                ],
                "actual_adapters": [],
            },
        )
        assert summarize_session(dashboard_session)
        dashboard_result = json.loads(
            (dashboard_session / "dashboard_mode_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert dashboard_result["status"] == "pass"

        dashboard_cost_session = root / "dashboard-cost-session"
        for case in dashboard_cases:
            mode_overrides = {
                "hidden": {
                    "dashboard_render_rebuilds": "0",
                    "dashboard_render_input_rows": "0",
                    "dashboard_render_visible_rows": "0",
                },
                "visible": {
                    "dashboard_render_rebuilds": "2",
                    "dashboard_render_input_rows": "130",
                    "dashboard_render_visible_rows": "130",
                },
                "active-filter": {
                    # Realtime modes may rebuild a different number of times in
                    # the measurement window. The comparator must normalize row
                    # work per rebuild instead of comparing cumulative totals.
                    "dashboard_render_rebuilds": "3",
                    "dashboard_render_input_rows": "195",
                    "dashboard_render_visible_rows": "96",
                },
            }[case.dashboard_mode]
            for run_number in range(1, 4):
                run_dir = (
                    dashboard_cost_session
                    / "cases"
                    / case.identifier
                    / f"run-{run_number:03d}"
                )
                write_fixture_run(
                    run_dir,
                    dashboard_mode=case.dashboard_mode,
                    workload="task-dashboard",
                    summary_overrides=mode_overrides,
                )
                validation = validate_run(
                    run_dir,
                    returncode=0,
                    expected_case=case,
                    expected_adapter="Test",
                    expected_backend="vulkan",
                    allow_log_patterns=[],
                )
                assert validation.valid, validation.reasons
                validation.profile_artifact = {
                    "instrumentation": "capture",
                    "task_dashboard_cpu": {
                        "source": "fixture",
                        "invocations": 2,
                        "total_ns": 2000,
                        "mean_ns_per_invocation": 1000.0,
                    },
                }
                write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            dashboard_cost_session / "manifest.json",
            {
                "matrix": {
                    "workload": "task-dashboard",
                    "capture_kind": "frame-time",
                    "dashboard_modes": ["hidden", "visible", "active-filter"],
                    "warmup_checksum_policy": "record",
                    "measure_end_checksum_policy": "record",
                },
                "cases": [
                    asdict(case) | {"id": case.identifier} for case in dashboard_cases
                ],
                "actual_adapters": [{"name": "Test GPU", "backend": "Vulkan"}],
                "requested_environment": {"WGPU_BACKEND": "vulkan"},
                "binary": {"instrumentation": "capture"},
            },
        )
        assert summarize_session(dashboard_cost_session)
        capture_report = (dashboard_cost_session / "report.md").read_text(
            encoding="utf-8"
        )
        assert "## Frame-time aggregate" in capture_report
        assert "diagnostic only" not in capture_report
        assert (
            compare_dashboard_modes(
                argparse.Namespace(
                    session=str(dashboard_cost_session), min_runs=3, output=None
                )
            )
            == 0
        )
        capture_costs = json.loads(
            (dashboard_cost_session / "dashboard_mode_cost_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert "p50_median_ms" in capture_costs["groups"][0]["modes"]["hidden"]

        memory_manifest = json.loads(
            (dashboard_cost_session / "manifest.json").read_text(encoding="utf-8")
        )
        memory_manifest["binary"]["instrumentation"] = "memory"
        write_json(dashboard_cost_session / "manifest.json", memory_manifest)
        for case in dashboard_cases:
            for run_number in range(1, 4):
                validation_path = (
                    dashboard_cost_session
                    / "cases"
                    / case.identifier
                    / f"run-{run_number:03d}"
                    / "validation.json"
                )
                validation_payload = json.loads(
                    validation_path.read_text(encoding="utf-8")
                )
                validation_payload["profile_artifact"] = {
                    "instrumentation": "memory",
                    "allocation_memory": {
                        "allocation_calls_per_frame": 4.0,
                        "allocated_bytes_per_frame": 4096.0,
                        "peak_live_bytes": 8192,
                        "peak_growth_bytes": 2048,
                    },
                    "process_memory": {"max_rss_kib": 1024},
                }
                write_json(validation_path, validation_payload)
        assert summarize_session(dashboard_cost_session)
        memory_report = (dashboard_cost_session / "report.md").read_text(
            encoding="utf-8"
        )
        assert "## Instrumented frame timing (diagnostic only)" in memory_report
        assert "must not be used for mode or baseline comparison" in memory_report
        memory_comparison = dashboard_cost_session / "memory-comparison.json"
        assert (
            compare_dashboard_modes(
                argparse.Namespace(
                    session=str(dashboard_cost_session),
                    min_runs=3,
                    output=str(memory_comparison),
                )
            )
            == 0
        )
        memory_costs = json.loads(memory_comparison.read_text(encoding="utf-8"))
        hidden_memory = memory_costs["groups"][0]["modes"]["hidden"]
        assert hidden_memory["allocation_peak_live_bytes_median"] == 8192
        assert "p50_median_ms" not in hidden_memory

        audit_args = build_parser().parse_args(["audit", "--dry-run"])
        assert audit_args.clock_mode == "fixed"
        assert audit_args.capture_kind == "fixed-step-determinism"
        assert audit_args.fixed_hz == 64
        assert audit_args.warmup_ticks == 1920
        assert audit_args.audit_ticks == 128
        assert audit_args.familiar_policies == "baseline"
        assert audit_args.operation_dialog_modes == "hidden"
        rejected_headless_gpu = build_parser().parse_args(
            ["run", "--dry-run", "--window-backend", "headless", "--renders", "gpu"]
        )
        try:
            validate_arguments(rejected_headless_gpu)
        except ValueError as error:
            assert "headless only supports --renders cpu" in str(error)
        else:
            raise AssertionError("headless GPU capture unexpectedly passed validation")
        rejected_partial_trace = build_parser().parse_args(
            [
                "run",
                "--instrumentation",
                "tracy",
                "--tracy-capture-binary",
                "/tmp/capture",
                "--tracy-csvexport-binary",
                "/tmp/csvexport",
                "--tracy-capture-secs",
                "1",
            ]
        )
        try:
            validate_arguments(rejected_partial_trace)
        except ValueError as error:
            assert "measure-artifact boundary" in str(error)
        else:
            raise AssertionError("partial Tracy capture unexpectedly passed validation")

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
