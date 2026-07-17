from __future__ import annotations

from .summary import *

def read_aggregate(path: Path) -> dict[str, dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return {row["case_id"]: row for row in csv.DictReader(handle)}


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
