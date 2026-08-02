from __future__ import annotations

from .summary import *

DASHBOARD_INPUT_ROWS_PER_REBUILD_REL_TOLERANCE = 0.05


def read_aggregate(path: Path) -> dict[str, dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return {row["case_id"]: row for row in csv.DictReader(handle)}


def load_profile_artifacts(session: Path, case_id: str) -> list[dict[str, Any]]:
    artifacts: list[dict[str, Any]] = []
    for path in sorted((session / "cases" / case_id).glob("run-*/validation.json")):
        payload = json.loads(path.read_text(encoding="utf-8"))
        artifact = payload.get("profile_artifact")
        if payload.get("valid") and isinstance(artifact, dict):
            artifacts.append(artifact)
    return artifacts


def ensure_comparison_contract(
    baseline_manifest: dict[str, Any], candidate_manifest: dict[str, Any], *, allow_case_subset: bool
) -> None:
    for key in ("actual_adapters", "requested_environment"):
        if baseline_manifest.get(key) != candidate_manifest.get(key):
            raise RuntimeError(f"cannot compare sessions with different {key}")

    baseline_instrumentation = baseline_manifest.get("binary", {}).get("instrumentation")
    candidate_instrumentation = candidate_manifest.get("binary", {}).get("instrumentation")
    if baseline_instrumentation != candidate_instrumentation:
        raise RuntimeError("cannot compare sessions with different instrumentation")

    baseline_matrix = baseline_manifest["matrix"]
    candidate_matrix = candidate_manifest["matrix"]
    if not allow_case_subset:
        if baseline_matrix != candidate_matrix:
            raise RuntimeError("cannot compare sessions with different matrix")
        return

    case_axes = {"sizes", "renders"}
    baseline_contract = {key: value for key, value in baseline_matrix.items() if key not in case_axes}
    candidate_contract = {key: value for key, value in candidate_matrix.items() if key not in case_axes}
    if baseline_contract != candidate_contract:
        raise RuntimeError("cannot compare subset sessions with different non-case matrix settings")
    for axis in case_axes:
        baseline_values = set(baseline_matrix.get(axis, []))
        candidate_values = set(candidate_matrix.get(axis, []))
        if not candidate_values <= baseline_values:
            raise RuntimeError(f"candidate {axis} is not a subset of the baseline")


def compare_sessions(args: argparse.Namespace) -> int:
    baseline = Path(args.baseline).resolve()
    candidate = Path(args.candidate).resolve()
    baseline_manifest = json.loads((baseline / "manifest.json").read_text(encoding="utf-8"))
    candidate_manifest = json.loads((candidate / "manifest.json").read_text(encoding="utf-8"))
    if (
        baseline_manifest.get("matrix", {}).get("capture_kind", "frame-time") != "frame-time"
        or candidate_manifest.get("matrix", {}).get("capture_kind", "frame-time") != "frame-time"
    ):
        raise RuntimeError("fixed-step determinism audits cannot be compared as frame-time sessions")
    ensure_comparison_contract(
        baseline_manifest,
        candidate_manifest,
        allow_case_subset=args.allow_case_subset,
    )
    baseline_rows = read_aggregate(baseline / "aggregate.csv")
    candidate_rows = read_aggregate(candidate / "aggregate.csv")
    if args.allow_case_subset:
        missing_baseline_cases = sorted(set(candidate_rows) - set(baseline_rows))
        if missing_baseline_cases:
            raise RuntimeError(
                "candidate has no baseline aggregate for: " + ", ".join(missing_baseline_cases)
            )
        common_cases = sorted(candidate_rows)
    else:
        common_cases = sorted(set(baseline_rows) & set(candidate_rows))
    if not common_cases:
        raise RuntimeError("sessions have no common valid cases")
    output = Path(args.output).resolve() if args.output else candidate / "comparison.csv"
    rows: list[dict[str, str]] = []
    regressed = False
    metric_column = f"{args.metric}_median_ms"
    for case_id in common_cases:
        base = baseline_rows[case_id]
        current = candidate_rows[case_id]
        if int(base["valid_runs"]) < args.min_runs or int(current["valid_runs"]) < args.min_runs:
            raise RuntimeError(f"{case_id} has fewer than {args.min_runs} valid runs")
        baseline_value = float(base[metric_column])
        candidate_value = float(current[metric_column])
        percent = ((candidate_value / baseline_value) - 1.0) * 100.0 if baseline_value else 0.0
        is_regression = percent > args.max_regression_pct
        regressed |= is_regression
        rows.append(
            {
                "case_id": case_id,
                "metric": args.metric,
                "baseline_ms": f"{baseline_value:.6f}",
                "candidate_ms": f"{candidate_value:.6f}",
                "delta_pct": f"{percent:.3f}",
                "regression": str(is_regression).lower(),
            }
        )
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    return 1 if regressed else 0


def compare_dashboard_modes(args: argparse.Namespace) -> int:
    session = Path(args.session).resolve()
    manifest = json.loads((session / "manifest.json").read_text(encoding="utf-8"))
    matrix = manifest.get("matrix", {})
    expected_modes = {"hidden", "visible", "active-filter"}
    instrumentation = manifest.get("binary", {}).get("instrumentation", "capture")
    if matrix.get("capture_kind", "frame-time") != "frame-time":
        raise RuntimeError("dashboard cost comparison requires a frame-time session")
    if matrix.get("workload") != "task-dashboard":
        raise RuntimeError("dashboard cost comparison requires workload task-dashboard")
    if set(matrix.get("dashboard_modes", [])) != expected_modes:
        raise RuntimeError("dashboard cost comparison requires hidden,visible,active-filter")
    if args.min_runs < 1:
        raise ValueError("--min-runs must be at least 1")

    aggregate = read_aggregate(session / "aggregate.csv")
    groups: dict[tuple[Any, ...], dict[str, str]] = {}
    for case in manifest.get("cases", []):
        contract = (
            case["workload"],
            case["size"],
            case["render"],
            case["seed"],
            case.get("souls"),
            case.get("familiars"),
            case.get("familiar_policy", "baseline"),
            case.get("operation_dialog", "hidden"),
        )
        groups.setdefault(contract, {})[case.get("dashboard_mode", "hidden")] = case["id"]

    failures: list[str] = []
    results: list[dict[str, Any]] = []
    for contract, mode_cases in sorted(groups.items()):
        group_failures: list[str] = []
        if set(mode_cases) != expected_modes:
            group_failures.append("case matrix is missing one or more dashboard modes")
        rows = {mode: aggregate.get(case_id) for mode, case_id in mode_cases.items()}
        if any(row is None for row in rows.values()):
            group_failures.append("one or more dashboard modes has no valid aggregate")
        if not group_failures:
            for mode, row in rows.items():
                assert row is not None
                if int(row["valid_runs"]) < args.min_runs:
                    group_failures.append(
                        f"{mode} has fewer than {args.min_runs} valid runs"
                    )
            checksums = {
                row["initial_state_checksum"] for row in rows.values() if row is not None
            }
            if len(checksums) != 1:
                group_failures.append("initial fixture checksums differ across dashboard modes")

        mode_values: dict[str, dict[str, float | int | str]] = {}
        if not group_failures:
            for mode, row in rows.items():
                assert row is not None
                mode_values[mode] = {
                    "valid_runs": int(row["valid_runs"]),
                    "dashboard_render_rebuilds_median": float(
                        row["dashboard_render_rebuilds_median"]
                    ),
                    "dashboard_render_input_rows_median": float(
                        row["dashboard_render_input_rows_median"]
                    ),
                    "dashboard_render_visible_rows_median": float(
                        row["dashboard_render_visible_rows_median"]
                    ),
                }
                if instrumentation == "capture":
                    mode_values[mode].update(
                        {
                            "p50_median_ms": float(row["p50_median_ms"]),
                            "p95_median_ms": float(row["p95_median_ms"]),
                            "p99_median_ms": float(row["p99_median_ms"]),
                            "max_median_ms": float(row["max_median_ms"]),
                        }
                    )
                rebuilds = float(mode_values[mode]["dashboard_render_rebuilds_median"])
                if rebuilds > 0:
                    mode_values[mode]["dashboard_render_input_rows_per_rebuild"] = (
                        float(mode_values[mode]["dashboard_render_input_rows_median"])
                        / rebuilds
                    )
                    mode_values[mode]["dashboard_render_visible_rows_per_rebuild"] = (
                        float(mode_values[mode]["dashboard_render_visible_rows_median"])
                        / rebuilds
                    )
                if instrumentation in {"capture", "tracy", "memory"}:
                    artifacts = load_profile_artifacts(session, mode_cases[mode])
                    if len(artifacts) < args.min_runs:
                        group_failures.append(
                            f"{mode} has fewer than {args.min_runs} valid profiling artifacts"
                        )
                        continue
                    if instrumentation == "tracy":
                        mode_values[mode]["trace_bytes_median"] = statistics.median(
                            [float(artifact["trace"]["bytes"]) for artifact in artifacts]
                        )
                    if instrumentation in {"capture", "tracy"}:
                        cpu_means = [
                            float(artifact["task_dashboard_cpu"]["mean_ns_per_invocation"])
                            for artifact in artifacts
                        ]
                        mode_values[mode]["task_dashboard_cpu_mean_ns_median"] = (
                            statistics.median(cpu_means)
                        )
                    else:
                        mode_values[mode]["allocation_calls_per_frame_median"] = (
                            statistics.median(
                                [
                                    float(
                                        artifact["allocation_memory"][
                                            "allocation_calls_per_frame"
                                        ]
                                    )
                                    for artifact in artifacts
                                ]
                            )
                        )
                        mode_values[mode]["allocated_bytes_per_frame_median"] = (
                            statistics.median(
                                [
                                    float(
                                        artifact["allocation_memory"][
                                            "allocated_bytes_per_frame"
                                        ]
                                    )
                                    for artifact in artifacts
                                ]
                            )
                        )
                        mode_values[mode]["allocation_peak_live_bytes_median"] = (
                            statistics.median(
                                [
                                    float(
                                        artifact["allocation_memory"]["peak_live_bytes"]
                                    )
                                    for artifact in artifacts
                                ]
                            )
                        )
                        mode_values[mode]["allocation_peak_growth_bytes_median"] = (
                            statistics.median(
                                [
                                    float(
                                        artifact["allocation_memory"]["peak_growth_bytes"]
                                    )
                                    for artifact in artifacts
                                ]
                            )
                        )
                        mode_values[mode]["process_max_rss_kib_median"] = statistics.median(
                            [
                                float(artifact["process_memory"]["max_rss_kib"])
                                for artifact in artifacts
                            ]
                        )
            if group_failures:
                mode_values.clear()
        if not group_failures:
            hidden = mode_values["hidden"]
            visible = mode_values["visible"]
            active = mode_values["active-filter"]
            if hidden["dashboard_render_rebuilds_median"] != 0:
                group_failures.append("hidden mode performed Task Dashboard render work")
            if visible["dashboard_render_rebuilds_median"] <= 0:
                group_failures.append("visible mode never rebuilt the Task Dashboard")
            if active["dashboard_render_rebuilds_median"] <= 0:
                group_failures.append("active-filter mode never rebuilt the Task Dashboard")
            if not group_failures:
                visible_input = float(visible["dashboard_render_input_rows_per_rebuild"])
                active_input = float(active["dashboard_render_input_rows_per_rebuild"])
                input_scale = max(visible_input, active_input)
                if input_scale <= 0:
                    group_failures.append("visible dashboard modes rendered no input rows")
                elif abs(visible_input - active_input) / input_scale > (
                    DASHBOARD_INPUT_ROWS_PER_REBUILD_REL_TOLERANCE
                ):
                    group_failures.append(
                        "visible and active-filter input rows per rebuild differ by more than 5%"
                    )
                if (
                    float(visible["dashboard_render_visible_rows_per_rebuild"])
                    != visible_input
                ):
                    group_failures.append("visible mode did not render every input row")
                if (
                    float(active["dashboard_render_visible_rows_per_rebuild"])
                    >= active_input
                ):
                    group_failures.append("active-filter did not reduce visible rows")

        failures.extend(group_failures)
        costs: dict[str, Any] = mode_values
        hidden_p50 = mode_values.get("hidden", {}).get("p50_median_ms")
        if instrumentation == "capture" and isinstance(hidden_p50, float):
            costs = {}
            for mode, values in mode_values.items():
                value = float(values["p50_median_ms"])
                costs[mode] = {
                    **values,
                    "p50_delta_from_hidden_ms": value - hidden_p50,
                    "p50_delta_from_hidden_pct": (
                        ((value / hidden_p50) - 1.0) * 100.0 if hidden_p50 else 0.0
                    ),
                }
        results.append(
            {
                "contract": {
                    "workload": contract[0],
                    "size": contract[1],
                    "render": contract[2],
                    "seed": contract[3],
                    "souls": contract[4],
                    "familiars": contract[5],
                    "familiar_policy": contract[6],
                    "operation_dialog": contract[7],
                },
                "status": "fail" if group_failures else "pass",
                "failures": group_failures,
                "modes": costs,
            }
        )

    result = {
        "schema_version": 1,
        "status": "fail" if failures else "pass",
        "instrumentation": instrumentation,
        "required_dashboard_modes": ["hidden", "visible", "active-filter"],
        "minimum_valid_runs_per_mode": args.min_runs,
        "groups": results,
        "failures": failures,
    }
    output = (
        Path(args.output).resolve()
        if args.output
        else session / "dashboard_mode_cost_comparison.json"
    )
    write_json(output, result)
    return 1 if failures else 0
