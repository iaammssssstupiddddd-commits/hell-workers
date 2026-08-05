from __future__ import annotations

from .policy import *

DASHBOARD_REALTIME_COUNTERS = (
    "candidate_membership_checks",
    "policy_disabled_rejections",
    "candidate_snapshot_attempts",
    "candidate_score_attempts",
    "worker_score_attempts",
    "top_k_partition_runs",
    "top_k_retained_candidates",
    "top_k_fallback_candidates",
    "source_selector_calls",
    "source_selector_cache_build_scanned_items",
    "source_selector_candidate_scanned_items",
    "source_selector_scanned_items",
    "reachable_with_cache_calls",
    "wheelbarrow_arbitration_rebuilds",
    "wheelbarrow_request_bucket_builds",
    "wheelbarrow_bucket_items_scanned",
    "wheelbarrow_candidates_after_top_k",
    "dashboard_state_rebuilds",
    "dashboard_snapshot_rows_scanned",
    "dashboard_summary_rows_scanned",
    "dashboard_snapshot_changes",
    "dashboard_summary_changes",
    "dashboard_render_rebuilds",
    "dashboard_render_input_rows",
    "dashboard_render_visible_rows",
    "dashboard_render_group_headers",
    "dashboard_despawn_roots_requested",
)


def behavior_timeline_signature(rows: list[dict[str, Any]]) -> str:
    serialized = json.dumps(
        rows,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(serialized).hexdigest()


def apply_behavior_timeline_policy(
    runs: list[tuple[Path, Validation]],
) -> bool:
    by_case: dict[str, list[tuple[Path, Validation]]] = {}
    for run_dir, validation in runs:
        by_case.setdefault(run_dir.parent.name, []).append((run_dir, validation))
    changed = False
    for case_runs in by_case.values():
        signatures = {
            behavior_timeline_signature(validation.timeline)
            for _, validation in case_runs
            if validation.valid and validation.timeline is not None
        }
        if len(signatures) <= 1:
            continue
        reason = "behavior timelines differ across repeated runs: " + ", ".join(
            sorted(signatures)
        )
        for run_dir, validation in case_runs:
            if not validation.valid:
                continue
            validation.valid = False
            validation.reasons.append(reason)
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed


def reset_behavior_timeline_policy(
    runs: list[tuple[Path, Validation]],
) -> bool:
    prefix = "behavior timelines differ across repeated runs:"
    changed = False
    for run_dir, validation in runs:
        reasons = [
            reason for reason in validation.reasons if not reason.startswith(prefix)
        ]
        valid = not reasons
        if reasons != validation.reasons or valid != validation.valid:
            validation.reasons = reasons
            validation.valid = valid
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed


def summarize_behavior_session(
    session_dir: Path,
    manifest: dict[str, Any],
    runs: list[tuple[Path, Validation]],
) -> bool:
    groups: dict[str, list[Validation]] = {}
    invalid_runs: list[tuple[Path, Validation]] = []
    adapters: list[dict[str, str]] = []
    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in adapters:
            adapters.append(validation.adapter)
        if validation.valid and validation.timeline is not None:
            groups.setdefault(run_dir.parent.name, []).append(validation)
        else:
            invalid_runs.append((run_dir, validation))

    columns = [
        "case_id",
        "valid_runs",
        "timeline_signature",
        "terminal_fixture_checksum",
        "behavior_save_sha256",
        "post_capture_teardown_warning_counts",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id, validations in sorted(groups.items()):
        signatures = {
            behavior_timeline_signature(validation.timeline)
            for validation in validations
            if validation.timeline is not None
        }
        fixture_checksums = {
            validation.timeline[-1]["fixture_checksum"]
            for validation in validations
            if validation.timeline
        }
        save_hashes = {
            validation.behavior_save_artifact["sha256"]
            for validation in validations
            if validation.behavior_save_artifact is not None
        }
        aggregate_rows.append(
            {
                "case_id": case_id,
                "valid_runs": str(len(validations)),
                "timeline_signature": ";".join(sorted(signatures)),
                "terminal_fixture_checksum": ";".join(sorted(fixture_checksums)),
                "behavior_save_sha256": ";".join(sorted(save_hashes)),
                "post_capture_teardown_warning_counts": ";".join(
                    str(len(validation.teardown_warning_lines))
                    for validation in validations
                ),
                "adapter": json.dumps(validations[0].adapter, sort_keys=True),
            }
        )

    with (session_dir / "aggregate.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report = [
        "# RtT-light fixed-step behavior report",
        "",
        f"- Valid runs: {sum(len(values) for values in groups.values())}",
        f"- Invalid runs: {len(invalid_runs)}",
        "- Contract: each case timeline must be exact and repeat-stable.",
        "- Frame-time and determinism checkpoint artifacts are intentionally absent.",
        "",
        "## Aggregate",
        "",
        "| Case | Valid runs | Timeline signature | Save artifact |",
        "| --- | ---: | --- | --- |",
    ]
    for row in aggregate_rows:
        report.append(
            f"| {row['case_id']} | {row['valid_runs']} | {row['timeline_signature']} "
            f"| {row['behavior_save_sha256'] or 'N/A'} |"
        )
    if invalid_runs:
        report.extend(["", "## Invalid runs", ""])
        report.extend(
            f"- `{run_dir.relative_to(session_dir)}`: {'; '.join(validation.reasons)}"
            for run_dir, validation in invalid_runs
        )
    (session_dir / "report.md").write_text(
        "\n".join(report) + "\n", encoding="utf-8"
    )
    manifest["actual_adapters"] = adapters
    manifest["status"] = "valid" if not invalid_runs else "invalid"
    write_json(session_dir / "manifest.json", manifest)
    return not invalid_runs

def summarize_determinism_session(
    session_dir: Path, manifest: dict[str, Any], runs: list[tuple[Path, Validation]]
) -> bool:
    groups: dict[str, list[Validation]] = {}
    all_adapters: list[dict[str, str]] = []
    invalid_runs: list[tuple[Path, Validation]] = []
    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in all_adapters:
            all_adapters.append(validation.adapter)
        if validation.valid and validation.determinism is not None:
            groups.setdefault(run_dir.parent.name, []).append(validation)
        else:
            invalid_runs.append((run_dir, validation))

    aggregate_columns = [
        "case_id",
        "valid_runs",
        "determinism_signature",
        "post_capture_teardown_warning_counts",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id, validations in sorted(groups.items()):
        signatures = {
            determinism_signature(validation.determinism)
            for validation in validations
            if validation.determinism is not None
        }
        aggregate_rows.append(
            {
                "case_id": case_id,
                "valid_runs": str(len(validations)),
                "determinism_signature": ";".join(sorted(signatures)),
                "post_capture_teardown_warning_counts": ";".join(
                    str(len(validation.teardown_warning_lines)) for validation in validations
                ),
                "adapter": json.dumps(validations[0].adapter, sort_keys=True),
            }
        )

    with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report_lines = [
        "# Fixed-step determinism audit report",
        "",
        f"- Valid runs: {sum(len(rows) for rows in groups.values())}",
        f"- Invalid runs: {len(invalid_runs)}",
        "- Contract: every `determinism.csv` checkpoint must be byte-for-byte identical per case.",
        "- Frame-time quantiles are intentionally absent and this session cannot be used with `compare`.",
        "- Post-capture teardown warnings (recorded, not validity failures): "
        + str(sum(len(validation.teardown_warning_lines) for _, validation in runs)),
        "",
    ]
    comparison_path = session_dir / "familiar_policy_comparison.json"
    if comparison_path.is_file():
        comparison = json.loads(comparison_path.read_text(encoding="utf-8"))
        report_lines.extend(
            [
                "## Familiar policy controlled comparison",
                "",
                f"- Status: `{comparison['status'].upper()}`",
                f"- Counter checkpoint: `{comparison['checkpoint']}`",
                "- Dialog hidden/open requires exact simulation checksum and AI work equality.",
                "- Disabled policy requires every candidate to stop at the policy gate and all downstream counters to be zero.",
                "",
                "| Case | Status | Default snapshot | Disabled snapshot | Default source calls | Disabled source calls |",
                "| --- | --- | ---: | ---: | ---: | ---: |",
            ]
        )
        for group in comparison["groups"]:
            counters = group.get("post_warmup_counters", {})
            default = counters.get("default", {})
            disabled = counters.get("disabled", {})
            contract = group["contract"]
            report_lines.append(
                f"| {contract['workload']}-{contract['size']}-{contract['render']} "
                f"| {group['status']} "
                f"| {default.get('candidate_snapshot_attempts', 'N/A')} "
                f"| {disabled.get('candidate_snapshot_attempts', 'N/A')} "
                f"| {default.get('source_selector_calls', 'N/A')} "
                f"| {disabled.get('source_selector_calls', 'N/A')} |"
            )
        report_lines.append("")
    dashboard_comparison_path = session_dir / "dashboard_mode_comparison.json"
    if dashboard_comparison_path.is_file():
        comparison = json.loads(dashboard_comparison_path.read_text(encoding="utf-8"))
        report_lines.extend(
            [
                "## Task Dashboard controlled comparison",
                "",
                f"- Status: `{comparison['status'].upper()}`",
                f"- Counter checkpoint: `{comparison['checkpoint']}`",
                "- Simulation, producer, candidate, arbitration, and runtime A* work must match exactly.",
                "- Hidden render work must be zero; active-filter must render fewer rows than visible.",
                "",
            ]
        )
    if aggregate_rows:
        report_lines.extend(
            [
                "## Aggregate",
                "",
                "| Case | Valid runs | Determinism signature |",
                "| --- | ---: | --- |",
            ]
        )
        for row in aggregate_rows:
            report_lines.append(
                f"| {row['case_id']} | {row['valid_runs']} | {row['determinism_signature']} |"
            )
        report_lines.append("")
    if invalid_runs:
        report_lines.extend(["## Invalid runs", ""])
        for run_dir, validation in invalid_runs:
            report_lines.append(f"- `{run_dir.relative_to(session_dir)}`: {'; '.join(validation.reasons)}")
    (session_dir / "report.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")

    manifest["actual_adapters"] = all_adapters
    manifest["status"] = "valid" if not invalid_runs else "invalid"
    write_json(session_dir / "manifest.json", manifest)
    return not invalid_runs


def summarize_session(
    session_dir: Path,
    warmup_policy: str | None = None,
    measure_end_policy: str | None = None,
) -> bool:
    manifest_path = session_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    source_errors = finalize_session_source(manifest)
    artifact_set_errors = [
        *source_errors,
        *validate_session_artifact_set(session_dir, manifest),
    ]
    if artifact_set_errors:
        manifest["status"] = "invalid"
        manifest["artifact_set_errors"] = artifact_set_errors
        write_json(manifest_path, manifest)
        report_lines = [
            "# Performance capture report",
            "",
            "- Status: `INVALID`",
            "- Artifact set validation failed before aggregation.",
            "",
            "## Artifact set errors",
            "",
            *(f"- {error}" for error in artifact_set_errors),
            "",
        ]
        (session_dir / "report.md").write_text(
            "\n".join(report_lines), encoding="utf-8"
        )
        return False
    matrix = manifest["matrix"]
    if matrix.get("capture_kind") == "fixed-step-determinism":
        runs = load_valid_runs(session_dir)
        reset_checksum_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_determinism_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_familiar_policy_controlled_audit(session_dir, manifest, runs)
        runs = load_valid_runs(session_dir)
        apply_dashboard_mode_controlled_audit(session_dir, manifest, runs)
        runs = load_valid_runs(session_dir)
        return summarize_determinism_session(session_dir, manifest, runs)
    if matrix.get("capture_kind") == "fixed-step-behavior":
        runs = load_valid_runs(session_dir)
        reset_behavior_timeline_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_behavior_timeline_policy(runs)
        runs = load_valid_runs(session_dir)
        return summarize_behavior_session(session_dir, manifest, runs)

    warmup_policy = warmup_policy or matrix["warmup_checksum_policy"]
    measure_end_policy = measure_end_policy or matrix.get("measure_end_checksum_policy", "record")
    runs = load_valid_runs(session_dir)
    reset_checksum_policy(runs)
    runs = load_valid_runs(session_dir)
    apply_checksum_policy(runs, warmup_policy, measure_end_policy)
    runs = load_valid_runs(session_dir)

    groups: dict[str, list[Validation]] = {}
    all_adapters: list[dict[str, str]] = []
    invalid_runs: list[tuple[Path, Validation]] = []
    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in all_adapters:
            all_adapters.append(validation.adapter)
        if validation.valid and validation.summary is not None:
            groups.setdefault(run_dir.parent.name, []).append(validation)
        else:
            invalid_runs.append((run_dir, validation))

    aggregate_columns = [
        "case_id",
        "valid_runs",
        "p50_median_ms",
        "p50_mad_ms",
        "p95_median_ms",
        "p95_mad_ms",
        "p99_median_ms",
        "p99_mad_ms",
        "max_median_ms",
        "max_mad_ms",
        "initial_state_checksum",
        "warmup_checksums",
        "measure_end_checksums",
        "post_capture_teardown_warning_counts",
        *[
            column
            for counter in DASHBOARD_REALTIME_COUNTERS
            for column in (f"{counter}_median", f"{counter}_mad")
        ],
        "task_execution_souls_queried_median",
        "task_execution_souls_queried_mad",
        "task_execution_idle_skips_median",
        "task_execution_idle_skips_mad",
        "task_execution_handler_runs_median",
        "task_execution_handler_runs_mad",
        "task_execution_idle_skip_pct_median",
        "task_execution_idle_skip_pct_mad",
        "task_execution_handler_run_pct_median",
        "task_execution_handler_run_pct_mad",
        "reservation_sync_full_rebuilds_median",
        "reservation_sync_full_rebuilds_mad",
        "reservation_sync_pending_tasks_scanned_median",
        "reservation_sync_pending_tasks_scanned_mad",
        "reservation_sync_assigned_tasks_scanned_median",
        "reservation_sync_assigned_tasks_scanned_mad",
        "runtime_path_actor_new_core_searches_median",
        "runtime_path_actor_new_core_searches_mad",
        "runtime_path_actor_new_deferred_median",
        "runtime_path_actor_new_deferred_mad",
        "runtime_path_actor_reuse_core_searches_median",
        "runtime_path_actor_reuse_core_searches_mad",
        "runtime_path_actor_reuse_deferred_median",
        "runtime_path_actor_reuse_deferred_mad",
        "runtime_path_actor_rest_fallback_core_searches_median",
        "runtime_path_actor_rest_fallback_core_searches_mad",
        "runtime_path_actor_rest_fallback_deferred_median",
        "runtime_path_actor_rest_fallback_deferred_mad",
        "runtime_path_escape_core_searches_median",
        "runtime_path_escape_core_searches_mad",
        "runtime_path_escape_deferred_median",
        "runtime_path_escape_deferred_mad",
        "runtime_path_task_execution_core_searches_median",
        "runtime_path_task_execution_core_searches_mad",
        "runtime_path_task_execution_deferred_median",
        "runtime_path_task_execution_deferred_mad",
        "runtime_path_bucket_transport_core_searches_median",
        "runtime_path_bucket_transport_core_searches_mad",
        "runtime_path_bucket_transport_deferred_median",
        "runtime_path_bucket_transport_deferred_mad",
        "runtime_path_total_core_searches_median",
        "runtime_path_total_core_searches_mad",
        "runtime_path_expanded_nodes_median",
        "runtime_path_expanded_nodes_mad",
        "runtime_path_max_expanded_nodes_per_search_median",
        "runtime_path_max_expanded_nodes_per_search_mad",
        "runtime_path_active_task_max_defer_frames_median",
        "runtime_path_active_task_max_defer_frames_mad",
        "runtime_path_idle_or_rest_max_defer_frames_median",
        "runtime_path_idle_or_rest_max_defer_frames_mad",
        "runtime_path_deferred_actor_retries_median",
        "runtime_path_deferred_actor_retries_mad",
        "door_open_souls_scanned_median",
        "door_open_souls_scanned_mad",
        "door_open_waypoints_scanned_median",
        "door_open_waypoints_scanned_mad",
        "door_close_souls_scanned_median",
        "door_close_souls_scanned_mad",
        "construction_floor_sites_considered_median",
        "construction_floor_sites_considered_mad",
        "construction_wall_sites_considered_median",
        "construction_wall_sites_considered_mad",
        "construction_floor_tiles_inspected_median",
        "construction_floor_tiles_inspected_mad",
        "construction_wall_tiles_inspected_median",
        "construction_wall_tiles_inspected_mad",
        "construction_evacuation_candidates_scanned_median",
        "construction_evacuation_candidates_scanned_mad",
        "construction_floor_phase_elapsed_micros_median",
        "construction_floor_phase_elapsed_micros_mad",
        "construction_floor_completion_elapsed_micros_median",
        "construction_floor_completion_elapsed_micros_mad",
        "construction_wall_phase_elapsed_micros_median",
        "construction_wall_phase_elapsed_micros_mad",
        "construction_wall_completion_elapsed_micros_median",
        "construction_wall_completion_elapsed_micros_mad",
        "slow_simulation_steps_median",
        "slow_simulation_steps_mad",
        "slow_simulation_souls_updated_median",
        "slow_simulation_souls_updated_mad",
        "slow_simulation_idle_decisions_median",
        "slow_simulation_idle_decisions_mad",
        "slow_simulation_idle_spatial_target_lookups_median",
        "slow_simulation_idle_spatial_target_lookups_mad",
        "slow_simulation_state_sanity_audits_median",
        "slow_simulation_state_sanity_audits_mad",
        "energy_power_output_runs_median",
        "energy_power_output_runs_mad",
        "energy_grid_recalc_runs_median",
        "energy_grid_recalc_runs_mad",
        "energy_lamp_steps_median",
        "energy_lamp_steps_mad",
        "energy_lamp_candidates_scanned_median",
        "energy_lamp_candidates_scanned_mad",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id, validations in sorted(groups.items()):
        metric_values = {
            metric: [float(validation.summary[metric]) for validation in validations]
            for metric in ("p50_ms", "p95_ms", "p99_ms", "max_ms")
        }
        work_counter_values = {}
        for counter in (
            *DASHBOARD_REALTIME_COUNTERS,
            "task_execution_souls_queried",
            "task_execution_idle_skips",
            "task_execution_handler_runs",
            "reservation_sync_full_rebuilds",
            "reservation_sync_pending_tasks_scanned",
            "reservation_sync_assigned_tasks_scanned",
            "runtime_path_actor_new_core_searches",
            "runtime_path_actor_new_deferred",
            "runtime_path_actor_reuse_core_searches",
            "runtime_path_actor_reuse_deferred",
            "runtime_path_actor_rest_fallback_core_searches",
            "runtime_path_actor_rest_fallback_deferred",
            "runtime_path_escape_core_searches",
            "runtime_path_escape_deferred",
            "runtime_path_task_execution_core_searches",
            "runtime_path_task_execution_deferred",
            "runtime_path_bucket_transport_core_searches",
            "runtime_path_bucket_transport_deferred",
            "runtime_path_total_core_searches",
            "runtime_path_expanded_nodes",
            "runtime_path_max_expanded_nodes_per_search",
            "runtime_path_active_task_max_defer_frames",
            "runtime_path_idle_or_rest_max_defer_frames",
            "runtime_path_deferred_actor_retries",
            "door_open_souls_scanned",
            "door_open_waypoints_scanned",
            "door_close_souls_scanned",
            "construction_floor_sites_considered",
            "construction_wall_sites_considered",
            "construction_floor_tiles_inspected",
            "construction_wall_tiles_inspected",
            "construction_evacuation_candidates_scanned",
            "construction_floor_phase_elapsed_micros",
            "construction_floor_completion_elapsed_micros",
            "construction_wall_phase_elapsed_micros",
            "construction_wall_completion_elapsed_micros",
            "slow_simulation_steps",
            "slow_simulation_souls_updated",
            "slow_simulation_idle_decisions",
            "slow_simulation_idle_spatial_target_lookups",
            "slow_simulation_state_sanity_audits",
            "energy_power_output_runs",
            "energy_grid_recalc_runs",
            "energy_lamp_steps",
            "energy_lamp_candidates_scanned",
        ):
            # schema v3 以前の既存baselineはreservation counterを持たない。
            # frame-time aggregateの再集約・比較は維持し、存在しないcounterを
            # 推測で0埋めしない。
            if all(counter in validation.summary for validation in validations):
                work_counter_values[counter] = [
                    float(validation.summary[counter]) for validation in validations
                ]
        row = {
            "case_id": case_id,
            "valid_runs": str(len(validations)),
            "initial_state_checksum": ";".join(
                sorted({validation.summary["initial_state_checksum"] for validation in validations})
            ),
            "warmup_checksums": ";".join(
                sorted({validation.summary["warmup_state_checksum"] for validation in validations})
            ),
            "measure_end_checksums": ";".join(
                sorted({validation.summary["measure_end_state_checksum"] for validation in validations})
            ),
            "post_capture_teardown_warning_counts": ";".join(
                str(len(validation.teardown_warning_lines)) for validation in validations
            ),
            "adapter": json.dumps(validations[0].adapter, sort_keys=True),
        }
        for metric, values in metric_values.items():
            median, mad = median_and_mad(values)
            prefix = metric.removesuffix("_ms")
            row[f"{prefix}_median_ms"] = f"{median:.6f}"
            row[f"{prefix}_mad_ms"] = f"{mad:.6f}"
        for counter, values in work_counter_values.items():
            median, mad = median_and_mad(values)
            row[f"{counter}_median"] = f"{median:.6f}"
            row[f"{counter}_mad"] = f"{mad:.6f}"
        if work_counter_values and all(
            float(validation.summary["task_execution_souls_queried"]) > 0
            for validation in validations
        ):
            task_execution_ratios = {
                "task_execution_idle_skip_pct": [
                    100.0
                    * float(validation.summary["task_execution_idle_skips"])
                    / float(validation.summary["task_execution_souls_queried"])
                    for validation in validations
                ],
                "task_execution_handler_run_pct": [
                    100.0
                    * float(validation.summary["task_execution_handler_runs"])
                    / float(validation.summary["task_execution_souls_queried"])
                    for validation in validations
                ],
            }
            for ratio, values in task_execution_ratios.items():
                median, mad = median_and_mad(values)
                row[f"{ratio}_median"] = f"{median:.6f}"
                row[f"{ratio}_mad"] = f"{mad:.6f}"
        aggregate_rows.append(row)

    with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report_lines = ["# Performance run report", "", f"- Valid runs: {sum(len(rows) for rows in groups.values())}"]
    report_lines.append(f"- Invalid runs: {len(invalid_runs)}")
    report_lines.append("- Initial fixture checksum policy: `require`")
    report_lines.append(f"- Warm-up checksum policy: `{warmup_policy}`")
    report_lines.append(f"- Measure-end checksum policy: `{measure_end_policy}`")
    capture_kind = matrix.get("capture_kind", "frame-time")
    instrumentation = manifest.get("binary", {}).get("instrumentation", "capture")
    report_lines.append(f"- Capture kind: `{capture_kind}`")
    report_lines.append(f"- Instrumentation: `{instrumentation}`")
    report_lines.append(
        "- Post-capture teardown warnings (recorded, not validity failures): "
        + str(sum(len(validation.teardown_warning_lines) for _, validation in runs))
    )
    report_lines.append("")
    if aggregate_rows:
        aggregate_heading = (
            "## Frame-time aggregate"
            if instrumentation == "capture"
            else "## Instrumented frame timing (diagnostic only)"
        )
        report_lines.extend(
            [
                aggregate_heading,
                "",
                *(
                    []
                    if instrumentation == "capture"
                    else [
                        "These values include instrumentation overhead and must not be used for mode or baseline comparison.",
                        "",
                    ]
                ),
                "| Case | Valid runs | p50 median ms | p95 median ms | p99 median ms |",
                "| --- | ---: | ---: | ---: | ---: |",
            ]
        )
        for row in aggregate_rows:
            report_lines.append(
                f"| {row['case_id']} | {row['valid_runs']} | {row['p50_median_ms']} | {row['p95_median_ms']} | {row['p99_median_ms']} |"
            )
        report_lines.append("")
    if invalid_runs:
        report_lines.extend(["## Invalid runs", ""])
        for run_dir, validation in invalid_runs:
            report_lines.append(f"- `{run_dir.relative_to(session_dir)}`: {'; '.join(validation.reasons)}")
    (session_dir / "report.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")

    manifest["actual_adapters"] = all_adapters
    manifest["status"] = "valid" if not invalid_runs else "invalid"
    write_json(manifest_path, manifest)
    return not invalid_runs
