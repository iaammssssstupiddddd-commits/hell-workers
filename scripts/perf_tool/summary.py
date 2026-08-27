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
    # Revisions are monotonic diagnostics, not semantic behavior identity.
    # A dirty source can converge through one extra intermediate input while
    # publishing the same checksummed field. Preserve the raw values in the
    # artifact and validate them there, but keep repeat identity scoped to the
    # observable lifecycle, counters, epochs, and checksums.
    semantic_rows = [
        {
            key: value
            for key, value in row.items()
            if key not in {"field_input_revision", "field_output_revision"}
        }
        for row in rows
    ]
    serialized = json.dumps(
        semantic_rows,
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
    groups: dict[str, list[tuple[Path, Validation]]] = {}
    preflight_groups: dict[str, list[tuple[Path, Validation]]] = {}
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


def _nearest_rank_p95(values: list[int]) -> int:
    if not values:
        raise ValueError("p95 requires at least one value")
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


SAVE_TRANSACTION_SIZES = ("small", "medium", "large")
SAVE_TRANSACTION_MEASURE_NS = 2_000_000_000
SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS = 100_000_000
SAVE_TRANSACTION_LARGE_TOTAL_MAX_LIMIT_NS = 250_000_000


def summarize_save_transaction_session(
    session_dir: Path,
    manifest: dict[str, Any],
    runs: list[tuple[Path, Validation]],
) -> bool:
    """Aggregate the save-only sidecar without inventing frame samples.

    A save transaction has one operation per process rather than a frame-time
    distribution. Capture reports its phase p95/max over the 20 measured
    process samples; Memory additionally reports allocator peak-live growth and
    GNU-time process maximum RSS as distinct values.
    """
    matrix = manifest["matrix"]
    invalid_runs: dict[Path, Validation] = {}
    groups: dict[str, list[tuple[Path, Validation]]] = {}
    preflight_groups: dict[str, list[tuple[Path, Validation]]] = {}
    contract_errors: list[str] = []
    adapters: list[dict[str, str]] = []
    instrumentation = manifest.get("binary", {}).get("instrumentation", "capture")

    def invalidate(run_dir: Path, validation: Validation, reason: str | None = None) -> None:
        """Persist an aggregation failure on the owning run exactly once."""
        if reason is not None and reason not in validation.reasons:
            validation.reasons.append(reason)
            validation.valid = False
            write_json(run_dir / "validation.json", validation.to_json())
        invalid_runs[run_dir] = validation

    def validate_memory_contract(
        run_dir: Path, validation: Validation, row: dict[str, str]
    ) -> bool:
        if instrumentation == "memory":
            profile = validation.profile_artifact
            allocation = profile.get("allocation_memory") if isinstance(profile, dict) else None
            process = profile.get("process_memory") if isinstance(profile, dict) else None
            try:
                expected_growth = int(row["peak_live_growth_bytes"])
                if (
                    not isinstance(allocation, dict)
                    or not isinstance(process, dict)
                    or int(allocation["peak_growth_bytes"]) != expected_growth
                    or int(process["max_rss_kib"]) <= 0
                ):
                    raise ValueError
            except (KeyError, TypeError, ValueError):
                invalidate(
                    run_dir,
                    validation,
                    "save-transaction memory artifact disagrees with its allocator/RSS sidecars",
                )
                return False
            return True
        if row.get("peak_live_growth_bytes") not in {"", None}:
            invalidate(
                run_dir,
                validation,
                "capture save-transaction artifact must not claim allocator growth",
            )
            return False
        return True

    def validate_measure_window(
        run_dir: Path, validation: Validation, row: dict[str, str]
    ) -> bool:
        try:
            virtual_ns = int(row["measure_virtual_ns"])
            real_ns = int(row["measure_real_ns"])
            if (
                virtual_ns < SAVE_TRANSACTION_MEASURE_NS
                or real_ns < SAVE_TRANSACTION_MEASURE_NS
            ):
                raise ValueError
        except (KeyError, TypeError, ValueError):
            invalidate(
                run_dir,
                validation,
                "save-transaction did not complete the fixed two-second measurement window",
            )
            return False
        return True

    matrix_errors = []
    if matrix.get("repeat") != 20 or matrix.get("preflight_runs") != 3:
        matrix_errors.append(
            "save-transaction requires exactly 20 measured runs and 3 preflight runs"
        )
    if matrix.get("sizes") != list(SAVE_TRANSACTION_SIZES) or matrix.get("renders") != ["cpu"]:
        matrix_errors.append(
            "save-transaction requires the canonical small/medium/large CPU matrix"
        )
    if matrix.get("warmup_secs") != 1.0 or matrix.get("measure_secs") != 2.0:
        matrix_errors.append("save-transaction requires a 1s warmup and 2s measure contract")
    if instrumentation not in {"capture", "memory"}:
        matrix_errors.append("save-transaction requires capture or memory instrumentation")
    if matrix_errors:
        manifest["status"] = "invalid"
        manifest["artifact_set_errors"] = matrix_errors
        write_json(session_dir / "manifest.json", manifest)
        (session_dir / "report.md").write_text(
            "# Save transaction report\n\n- Status: `INVALID`\n- Invalid matrix contract.\n",
            encoding="utf-8",
        )
        return False

    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in adapters:
            adapters.append(validation.adapter)
        row = validation.save_transaction
        if not validation.valid or row is None or row.get("sample_kind") != "measured":
            invalidate(
                run_dir,
                validation,
                "save-transaction measured run has no measured transaction sidecar"
                if validation.valid
                else None,
            )
            continue
        if not validate_memory_contract(run_dir, validation, row):
            continue
        if not validate_measure_window(run_dir, validation, row):
            continue
        groups.setdefault(run_dir.parent.name, []).append((run_dir, validation))

    for run_dir, validation in load_preflight_runs(session_dir):
        row = validation.save_transaction
        if (
            not validation.valid
            or row is None
            or row.get("sample_kind") != "preflight"
        ):
            invalidate(
                run_dir,
                validation,
                "save-transaction preflight has no preflight transaction sidecar"
                if validation.valid
                else None,
            )
            continue
        if not validate_memory_contract(run_dir, validation, row):
            continue
        if not validate_measure_window(run_dir, validation, row):
            continue
        preflight_groups.setdefault(run_dir.parent.name, []).append((run_dir, validation))

    case_records = {
        case["id"]: case
        for case in manifest.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    expected_cases = set(case_records)
    if (
        len(case_records) != len(SAVE_TRANSACTION_SIZES)
        or {record.get("size") for record in case_records.values()} != set(SAVE_TRANSACTION_SIZES)
        or any(record.get("render") != "cpu" for record in case_records.values())
    ):
        contract_errors.append(
            "save-transaction manifest cases must contain one CPU case for each canonical size"
        )
    for case_id in sorted(expected_cases):
        case_preflights = preflight_groups.get(case_id, [])
        if len(case_preflights) != 3:
            reason = "save-transaction requires exactly 3 valid preflight samples per case"
            contract_errors.append(f"{case_id}: {reason}")
            for run_dir, validation in case_preflights:
                invalidate(run_dir, validation, reason)

        case_runs = groups.get(case_id, [])
        if len(case_runs) != 20:
            reason = "save-transaction requires exactly 20 valid measured samples per case"
            contract_errors.append(f"{case_id}: {reason}")
            for run_dir, validation in case_runs:
                invalidate(run_dir, validation, reason)

    columns = [
        "case_id",
        "valid_runs",
        "fixture_checksums",
        "serialize_p95_ns",
        "serialize_max_ns",
        "write_file_sync_p95_ns",
        "write_file_sync_max_ns",
        "commit_directory_sync_p95_ns",
        "commit_directory_sync_max_ns",
        "total_p95_ns",
        "total_max_ns",
        "peak_live_growth_p95_bytes",
        "peak_live_growth_max_bytes",
        "max_rss_kib_max",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id in sorted(expected_cases):
        case_runs = groups.get(case_id, [])
        if any(run_dir in invalid_runs for run_dir, _ in case_runs):
            continue
        validations = [validation for _, validation in case_runs]
        if len(validations) != 20:
            continue
        rows = [validation.save_transaction for validation in validations]
        assert all(row is not None for row in rows)
        typed_rows = [row for row in rows if row is not None]
        fixture_checksums = {row["fixture_checksum"] for row in typed_rows}
        if len(fixture_checksums) != 1:
            for run_dir, validation in case_runs:
                invalidate(
                    run_dir,
                    validation,
                    "save-transaction fixture checksum differs across measured runs",
                )
            continue
        preflight_checksums = {
            validation.save_transaction["fixture_checksum"]
            for _, validation in preflight_groups.get(case_id, [])
            if validation.save_transaction is not None
        }
        if preflight_checksums != fixture_checksums:
            for run_dir, validation in case_runs:
                invalidate(
                    run_dir,
                    validation,
                    "save-transaction preflight fixture checksum differs from measured samples",
                )
            continue
        metric_values = {
            column: [int(row[column]) for row in typed_rows]
            for column in (
                "serialize_ns",
                "write_file_sync_ns",
                "commit_directory_sync_ns",
                "total_ns",
            )
        }
        row = {
            "case_id": case_id,
            "valid_runs": str(len(validations)),
            "fixture_checksums": ";".join(sorted(fixture_checksums)),
            "adapter": json.dumps(validations[0].adapter, sort_keys=True),
            "peak_live_growth_p95_bytes": "",
            "peak_live_growth_max_bytes": "",
            "max_rss_kib_max": "",
        }
        for source, prefix in (
            ("serialize_ns", "serialize"),
            ("write_file_sync_ns", "write_file_sync"),
            ("commit_directory_sync_ns", "commit_directory_sync"),
            ("total_ns", "total"),
        ):
            row[f"{prefix}_p95_ns"] = str(_nearest_rank_p95(metric_values[source]))
            row[f"{prefix}_max_ns"] = str(max(metric_values[source]))
        if instrumentation == "capture" and case_records[case_id].get("size") == "large":
            total_p95 = int(row["total_p95_ns"])
            total_max = int(row["total_max_ns"])
            if total_p95 > SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS:
                contract_errors.append(
                    "large Capture total p95 exceeds "
                    f"{SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS}ns: {total_p95}ns"
                )
            if total_max > SAVE_TRANSACTION_LARGE_TOTAL_MAX_LIMIT_NS:
                contract_errors.append(
                    "large Capture total max exceeds "
                    f"{SAVE_TRANSACTION_LARGE_TOTAL_MAX_LIMIT_NS}ns: {total_max}ns"
                )
        if instrumentation == "memory":
            growth = [int(item["peak_live_growth_bytes"]) for item in typed_rows]
            rss = [
                int(validation.profile_artifact["process_memory"]["max_rss_kib"])
                for validation in validations
                if isinstance(validation.profile_artifact, dict)
            ]
            if len(rss) != len(validations):
                for run_dir, validation in case_runs:
                    invalidate(
                        run_dir,
                        validation,
                        "save-transaction memory aggregate is missing a process RSS sample",
                    )
                continue
            row["peak_live_growth_p95_bytes"] = str(_nearest_rank_p95(growth))
            row["peak_live_growth_max_bytes"] = str(max(growth))
            row["max_rss_kib_max"] = str(max(rss))
        aggregate_rows.append(row)

    with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report_lines = [
        "# Save transaction report",
        "",
        f"- Instrumentation: `{instrumentation}`",
        f"- Valid measured runs: {sum(1 for rows in groups.values() for run_dir, _ in rows if run_dir not in invalid_runs)}",
        f"- Invalid measured runs: {len(invalid_runs)}",
        "- Preflight runs: 3 per case (validated but excluded from aggregate).",
        "- p95 uses nearest-rank over the 20 measured transactions.",
        "",
    ]
    if aggregate_rows:
        report_lines.extend(
            [
                "| Case | Valid runs | Total p95 ns | Total max ns |",
                "| --- | ---: | ---: | ---: |",
                *(
                    f"| {row['case_id']} | {row['valid_runs']} | {row['total_p95_ns']} | {row['total_max_ns']} |"
                    for row in aggregate_rows
                ),
                "",
            ]
        )
    if invalid_runs:
        report_lines.extend(["## Invalid runs", ""])
        for run_dir, validation in sorted(invalid_runs.items(), key=lambda item: str(item[0])):
            report_lines.append(
                f"- `{run_dir.relative_to(session_dir) if run_dir.is_relative_to(session_dir) else run_dir}`: "
                + "; ".join(validation.reasons)
            )
    if contract_errors:
        report_lines.extend(["## Session contract errors", ""])
        report_lines.extend(f"- {reason}" for reason in sorted(set(contract_errors)))
    (session_dir / "report.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")
    manifest["actual_adapters"] = adapters
    manifest["status"] = "valid" if not invalid_runs and not contract_errors else "invalid"
    write_json(session_dir / "manifest.json", manifest)
    return not invalid_runs and not contract_errors


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
    if matrix.get("workload") == "save-transaction":
        return summarize_save_transaction_session(
            session_dir, manifest, load_valid_runs(session_dir)
        )
    if matrix.get("capture_kind") == "consumer-core":
        runs = load_valid_runs(session_dir)
        invalid = [
            (run_dir, validation)
            for run_dir, validation in runs
            if not validation.valid or validation.indoor_light_consumers is None
        ]
        consumers = [
            validation.indoor_light_consumers
            for _, validation in runs
            if validation.valid and validation.indoor_light_consumers is not None
        ]
        aggregate_columns = [
            "case_id",
            "valid_runs",
            "consumer_p95_median_ms",
            "consumer_p99_median_ms",
            "samples_per_soul_slow_step",
            "effects_per_soul_slow_step",
            "revision_epoch_consistency",
            "mask_or_stale_effects",
            "scoped_allocation_events",
            "scoped_allocation_bytes",
        ]
        aggregate_rows: list[dict[str, str]] = []
        if consumers and not invalid:
            aggregate_rows.append(
                {
                    "case_id": manifest["cases"][0]["id"],
                    "valid_runs": str(len(consumers)),
                    "consumer_p95_median_ms": f"{statistics.median(row['consumer_p95_ms'] for row in consumers):.6f}",
                    "consumer_p99_median_ms": f"{statistics.median(row['consumer_p99_ms'] for row in consumers):.6f}",
                    "samples_per_soul_slow_step": str(
                        max(row["samples_per_soul_slow_step"] for row in consumers)
                    ),
                    "effects_per_soul_slow_step": str(
                        max(row["effects_per_soul_slow_step"] for row in consumers)
                    ),
                    "revision_epoch_consistency": str(
                        all(row["revision_epoch_consistency"] for row in consumers)
                    ).lower(),
                    "mask_or_stale_effects": str(
                        sum(row["mask_or_stale_effects"] for row in consumers)
                    ),
                    "scoped_allocation_events": str(
                        sum(row["scoped_allocation_events"] for row in consumers)
                    ),
                    "scoped_allocation_bytes": str(
                        sum(row["scoped_allocation_bytes"] for row in consumers)
                    ),
                }
            )
        with (session_dir / "aggregate.csv").open(
            "w", newline="", encoding="utf-8"
        ) as handle:
            writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
            writer.writeheader()
            writer.writerows(aggregate_rows)
        report = [
            "# Consumer-core performance report",
            "",
            f"- Valid runs: {len(consumers) if not invalid else 0}",
            f"- Invalid runs: {len(invalid)}",
            "- Capture kind: `consumer-core`",
            "",
        ]
        (session_dir / "report.md").write_text("\n".join(report), encoding="utf-8")
        manifest["actual_adapters"] = []
        manifest["status"] = "invalid" if invalid else "valid"
        write_json(session_dir / "manifest.json", manifest)
        return not invalid
    if matrix.get("capture_kind") == "field-core":
        runs = load_valid_runs(session_dir)
        invalid = [
            (run_dir, validation)
            for run_dir, validation in runs
            if not validation.valid or validation.indoor_light_field is None
        ]
        fields = [
            validation.indoor_light_field
            for _, validation in runs
            if validation.valid and validation.indoor_light_field is not None
        ]
        aggregate_columns = [
            "case_id",
            "valid_runs",
            "field_rebuild_p95_median_ms",
            "field_rebuild_p99_median_ms",
            "field_rebuild_allocation_events",
            "field_rebuild_allocation_bytes",
        ]
        aggregate_rows: list[dict[str, str]] = []
        if fields and not invalid:
            allocations = [field["field_rebuild_allocation"] for field in fields]
            if len({(row["events"], row["bytes"], row["scope"]) for row in allocations}) != 1:
                invalid.append((session_dir, Validation(False, ["field allocation evidence differs across runs"], None, None, [], [])))
            else:
                aggregate_rows.append(
                    {
                        "case_id": manifest["cases"][0]["id"],
                        "valid_runs": str(len(fields)),
                        "field_rebuild_p95_median_ms": f"{statistics.median(field['field_rebuild_p95_ms'] for field in fields):.6f}",
                        "field_rebuild_p99_median_ms": f"{statistics.median(field['field_rebuild_p99_ms'] for field in fields):.6f}",
                        "field_rebuild_allocation_events": str(allocations[0]["events"]),
                        "field_rebuild_allocation_bytes": str(allocations[0]["bytes"]),
                    }
                )
        with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
            writer.writeheader()
            writer.writerows(aggregate_rows)
        report = [
            "# Field-core performance report",
            "",
            f"- Valid runs: {len(fields) if not invalid else 0}",
            f"- Invalid runs: {len(invalid)}",
            "- Capture kind: `field-core`",
            "",
        ]
        if aggregate_rows:
            row = aggregate_rows[0]
            report.extend(
                [
                    f"- Rebuild p95 median: {row['field_rebuild_p95_median_ms']} ms",
                    f"- Rebuild p99 median: {row['field_rebuild_p99_median_ms']} ms",
                    "",
                ]
            )
        (session_dir / "report.md").write_text("\n".join(report), encoding="utf-8")
        manifest["actual_adapters"] = []
        manifest["status"] = "invalid" if invalid else "valid"
        write_json(session_dir / "manifest.json", manifest)
        return not invalid
    if matrix.get("capture_kind") == "fixed-step-determinism":
        runs = load_valid_runs(session_dir)
        reset_checksum_policy(runs)
        reset_dream_ui_repeat_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_determinism_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_dream_ui_determinism_policy(runs)
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
