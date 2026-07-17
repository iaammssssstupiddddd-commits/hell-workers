from __future__ import annotations

from .policy import *

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
    matrix = manifest["matrix"]
    if matrix.get("capture_kind") == "fixed-step-determinism":
        runs = load_valid_runs(session_dir)
        reset_checksum_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_determinism_policy(runs)
        runs = load_valid_runs(session_dir)
        return summarize_determinism_session(session_dir, manifest, runs)

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
    report_lines.append(f"- Capture kind: `{capture_kind}`")
    report_lines.append(
        "- Post-capture teardown warnings (recorded, not validity failures): "
        + str(sum(len(validation.teardown_warning_lines) for _, validation in runs))
    )
    report_lines.append("")
    if aggregate_rows:
        report_lines.extend(["## Aggregate", "", "| Case | Valid runs | p50 median ms | p95 median ms | p99 median ms |", "| --- | ---: | ---: | ---: | ---: |"])
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
