#!/usr/bin/env python3
"""Seal and verify M0-versus-production Wall Capture comparisons."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import math
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
PROFILE = "wall-cross-subject-performance-v1"
PHASES = ("completed", "provisional")
METRICS = ("p95", "p99")
MAX_REGRESSION_PCT = 5.0
MIN_RUNS = 3
FROZEN_DENSITY_FILES = (
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_density_acceptance.py",
    "crates/bevy_app/src/plugins/startup/perf_scenario/wall_density_fixture.rs",
    "tools/blender_ai_workflow/fixtures/wall-density-v1.json",
)
BASELINE_VERIFIER = FROZEN_DENSITY_FILES[0]
PRODUCTION_VERIFIER = (
    ".codex/skills/hell-workers-run-native-acceptance/scripts/"
    "wall_production_performance_acceptance.py"
)


class AcceptanceError(RuntimeError):
    """Raised when cross-subject evidence is incomplete or inconsistent."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AcceptanceError(message)


def sha256(path: Path) -> str:
    require(path.is_file() and not path.is_symlink(), f"file is unavailable: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AcceptanceError(f"cannot read JSON: {path}: {error}") from error
    require(isinstance(value, dict), f"JSON root is not an object: {path}")
    return value


def atomic_write(path: Path, payload: bytes) -> None:
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    require(not temporary.exists(), f"temporary output already exists: {temporary}")
    with temporary.open("xb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory_fd)
    finally:
        os.close(directory_fd)


def json_bytes(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def run_frozen_verifier(job_root: Path, script_relative: str) -> dict[str, Any]:
    manifest = read_json(job_root / "manifest.json")
    repo_value = manifest.get("repo")
    require(isinstance(repo_value, str), "job manifest repo is invalid")
    repo = Path(repo_value).resolve()
    script = repo / script_relative
    require(script.is_file() and not script.is_symlink(), "frozen verifier is unavailable")
    environment = os.environ.copy()
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    result = subprocess.run(
        [sys.executable, str(script), "verify", "--job-root", str(job_root)],
        cwd=repo,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    require(
        result.returncode == 0,
        f"frozen verifier failed for {job_root}: {result.stderr or result.stdout}",
    )
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AcceptanceError("frozen verifier output is not JSON") from error
    require(
        isinstance(value, dict) and value.get("status") == "pass",
        "frozen verifier did not report pass",
    )
    return value


def normalize_environment(manifest: dict[str, Any], repo: Path) -> dict[str, Any]:
    value = manifest.get("requested_environment")
    require(isinstance(value, dict), "session requested_environment is invalid")
    expected_asset_root = value.get("BEVY_ASSET_ROOT")
    require(
        isinstance(expected_asset_root, str)
        and Path(expected_asset_root).resolve() == repo,
        "session BEVY_ASSET_ROOT does not identify its recorded repo",
    )
    return {key: item for key, item in value.items() if key != "BEVY_ASSET_ROOT"}


def read_rows(path: Path) -> dict[str, dict[str, str]]:
    require(path.is_file() and not path.is_symlink(), f"aggregate is unavailable: {path}")
    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        require(reader.fieldnames is not None and "case_id" in reader.fieldnames, "aggregate lacks case_id")
        rows: dict[str, dict[str, str]] = {}
        for row in reader:
            case_id = row["case_id"]
            require(case_id not in rows, f"aggregate duplicates case: {case_id}")
            rows[case_id] = row
    require(rows, "aggregate has no rows")
    return rows


def manifest_case_ids(manifest: dict[str, Any]) -> set[str]:
    cases = manifest.get("cases")
    require(isinstance(cases, list) and cases, "session manifest has no cases")
    values = [case.get("id") for case in cases if isinstance(case, dict)]
    require(
        len(values) == len(cases)
        and all(isinstance(value, str) for value in values)
        and len(set(values)) == len(values),
        "session manifest case identities are invalid",
    )
    return set(values)


def require_session_contract(
    baseline: dict[str, Any],
    production: dict[str, Any],
    *,
    baseline_repo: Path,
    production_repo: Path,
) -> None:
    for label, manifest in (("baseline", baseline), ("production", production)):
        require(manifest.get("schema_version") == 2, f"{label} session schema differs")
        require(manifest.get("status") == "valid", f"{label} session is not valid")
        require(not manifest.get("artifact_set_errors"), f"{label} session has artifact errors")
        require(
            manifest.get("binary", {}).get("instrumentation") == "capture",
            f"{label} session is not Capture instrumentation",
        )
    require(
        baseline.get("actual_adapters") == production.get("actual_adapters"),
        "sessions used different actual adapters",
    )
    require(
        normalize_environment(baseline, baseline_repo)
        == normalize_environment(production, production_repo),
        "sessions used different requested environments beyond BEVY_ASSET_ROOT",
    )
    require(baseline.get("matrix") == production.get("matrix"), "session matrices differ")
    require(
        manifest_case_ids(baseline) == manifest_case_ids(production),
        "session case identities differ",
    )


def finite_value(row: dict[str, str], field: str, case_id: str) -> float:
    try:
        value = float(row[field])
    except (KeyError, TypeError, ValueError) as error:
        raise AcceptanceError(f"{case_id} has invalid {field}") from error
    require(math.isfinite(value), f"{case_id} {field} is not finite")
    return value


def comparison_rows(
    baseline_rows: dict[str, dict[str, str]],
    production_rows: dict[str, dict[str, str]],
    metric: str,
) -> list[dict[str, str]]:
    require(metric in METRICS, "unsupported comparison metric")
    require(set(baseline_rows) == set(production_rows), "aggregate case sets differ")
    output = []
    field = f"{metric}_median_ms"
    for case_id in sorted(baseline_rows):
        baseline = baseline_rows[case_id]
        production = production_rows[case_id]
        require(
            int(baseline["valid_runs"]) >= MIN_RUNS
            and int(production["valid_runs"]) >= MIN_RUNS,
            f"{case_id} has fewer than three valid runs",
        )
        baseline_ms = finite_value(baseline, field, case_id)
        production_ms = finite_value(production, field, case_id)
        require(baseline_ms > 0.0 and production_ms >= 0.0, f"{case_id} timing is invalid")
        delta_pct = ((production_ms / baseline_ms) - 1.0) * 100.0
        regressed = delta_pct > MAX_REGRESSION_PCT
        output.append(
            {
                "case_id": case_id,
                "metric": metric,
                "baseline_ms": f"{baseline_ms:.6f}",
                "candidate_ms": f"{production_ms:.6f}",
                "delta_pct": f"{delta_pct:.3f}",
                "regression": str(regressed).lower(),
            }
        )
    return output


def csv_bytes(rows: list[dict[str, str]]) -> bytes:
    require(rows, "comparison has no rows")
    output = io.StringIO(newline="")
    writer = csv.DictWriter(output, fieldnames=list(rows[0]), lineterminator="\n")
    writer.writeheader()
    writer.writerows(rows)
    return output.getvalue().encode()


def session_paths(job_root: Path, production: bool, phase: str) -> Path:
    prefix = ("production", "sessions", phase) if production else ("sessions", phase)
    return job_root.joinpath(*prefix)


def collect_inputs(
    baseline_job: Path, production_job: Path
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[tuple[str, str], bytes]]:
    baseline_manifest = read_json(baseline_job / "manifest.json")
    production_manifest = read_json(production_job / "manifest.json")
    require(
        baseline_manifest.get("profile") == "wall-density-capture-v1"
        and baseline_manifest.get("status") == "pass",
        "baseline job is not a passing wall-density capture",
    )
    require(
        production_manifest.get("profile") == "wall-production-performance-v1"
        and production_manifest.get("status") == "pass",
        "production job is not a passing Wall production pair",
    )
    baseline_repo = Path(str(baseline_manifest.get("repo"))).resolve()
    production_repo = Path(str(production_manifest.get("repo"))).resolve()
    require(
        baseline_manifest.get("asset_view_fingerprint")
        == production_manifest.get("asset_view_fingerprint"),
        "baseline and production asset views differ",
    )
    require(
        baseline_manifest.get("adapter") == production_manifest.get("adapter"),
        "baseline and production adapter selectors differ",
    )

    frozen_files = []
    for relative in FROZEN_DENSITY_FILES:
        baseline_hash = sha256(baseline_repo / relative)
        production_hash = sha256(production_repo / relative)
        require(baseline_hash == production_hash, f"frozen density file differs: {relative}")
        frozen_files.append({"path": relative, "sha256": baseline_hash})

    comparisons: dict[tuple[str, str], bytes] = {}
    session_inputs = []
    for phase in PHASES:
        baseline_session = session_paths(baseline_job, False, phase)
        production_session = session_paths(production_job, True, phase)
        baseline_session_manifest = read_json(baseline_session / "manifest.json")
        production_session_manifest = read_json(production_session / "manifest.json")
        require_session_contract(
            baseline_session_manifest,
            production_session_manifest,
            baseline_repo=baseline_repo,
            production_repo=production_repo,
        )
        baseline_rows = read_rows(baseline_session / "aggregate.csv")
        production_rows = read_rows(production_session / "aggregate.csv")
        expected_cases = manifest_case_ids(baseline_session_manifest)
        require(
            set(baseline_rows) == expected_cases and set(production_rows) == expected_cases,
            "aggregate rows do not match the session case contract",
        )
        session_inputs.append(
            {
                "phase": phase,
                "baseline_manifest_sha256": sha256(baseline_session / "manifest.json"),
                "baseline_aggregate_sha256": sha256(baseline_session / "aggregate.csv"),
                "production_manifest_sha256": sha256(production_session / "manifest.json"),
                "production_aggregate_sha256": sha256(production_session / "aggregate.csv"),
                "actual_adapters": baseline_session_manifest["actual_adapters"],
                "requested_environment": normalize_environment(
                    baseline_session_manifest, baseline_repo
                ),
                "matrix": baseline_session_manifest["matrix"],
            }
        )
        for metric in METRICS:
            comparisons[(phase, metric)] = csv_bytes(
                comparison_rows(baseline_rows, production_rows, metric)
            )

    provenance = {
        "baseline": {
            "job_root": str(baseline_job),
            "manifest_sha256": sha256(baseline_job / "manifest.json"),
            "repo": str(baseline_repo),
            "subject_commit": baseline_manifest["subject_commit"],
            "source_fingerprint": baseline_manifest["source_fingerprint"],
            "harness_fingerprint": baseline_manifest["harness_fingerprint"],
            "asset_view_fingerprint": baseline_manifest["asset_view_fingerprint"],
            "binary_sha256": baseline_manifest["binary_sha256"],
        },
        "production": {
            "job_root": str(production_job),
            "manifest_sha256": sha256(production_job / "manifest.json"),
            "repo": str(production_repo),
            "subject_commit": production_manifest["subject_commit"],
            "source_fingerprint": production_manifest["source_fingerprint"],
            "harness_fingerprint": production_manifest["harness_fingerprint"],
            "asset_view_fingerprint": production_manifest["asset_view_fingerprint"],
            "binary_sha256": production_manifest["binary_sha256"],
            "candidate_identity": production_manifest["candidate_identity"],
        },
    }
    inputs = {
        "provenance": provenance,
        "frozen_density_files": frozen_files,
        "sessions": session_inputs,
    }
    return inputs, frozen_files, comparisons


def comparison_manifest(comparisons: dict[tuple[str, str], bytes]) -> list[dict[str, Any]]:
    values = []
    for phase in PHASES:
        for metric in METRICS:
            payload = comparisons[(phase, metric)]
            rows = list(csv.DictReader(io.StringIO(payload.decode())))
            require(
                all(row["regression"] == "false" for row in rows),
                f"{phase} {metric} exceeds the five-percent gate",
            )
            values.append(
                {
                    "phase": phase,
                    "metric": metric,
                    "file": f"m0-vs-production-{phase}-{metric}.csv",
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "rows": rows,
                }
            )
    return values


def verify_root(root: Path) -> dict[str, Any]:
    manifest = read_json(root / "manifest.json")
    require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "cross-subject manifest identity differs",
    )
    require(
        manifest.get("verifier_sha256") == sha256(Path(__file__).resolve()),
        "cross-subject verifier changed",
    )
    provenance = manifest.get("provenance")
    require(isinstance(provenance, dict), "cross-subject provenance is invalid")
    baseline_job = Path(provenance["baseline"]["job_root"]).resolve()
    production_job = Path(provenance["production"]["job_root"]).resolve()
    baseline_result = run_frozen_verifier(baseline_job, BASELINE_VERIFIER)
    production_result = run_frozen_verifier(production_job, PRODUCTION_VERIFIER)
    inputs, frozen_files, comparisons = collect_inputs(baseline_job, production_job)
    require(manifest.get("provenance") == inputs["provenance"], "job provenance changed")
    require(manifest.get("frozen_density_files") == frozen_files, "density contract changed")
    require(manifest.get("sessions") == inputs["sessions"], "session inputs changed")
    expected_comparisons = comparison_manifest(comparisons)
    require(
        manifest.get("comparisons") == expected_comparisons,
        "comparison manifest changed",
    )
    sealed = root / "sealed-artifacts"
    require(sealed.is_dir() and not sealed.is_symlink(), "sealed artifact directory is invalid")
    expected_names = {entry["file"] for entry in expected_comparisons}
    actual_names = {path.name for path in sealed.iterdir() if path.is_file()}
    require(actual_names == expected_names, "sealed comparison file set differs")
    for entry in expected_comparisons:
        path = sealed / entry["file"]
        require(path.read_bytes() == comparisons[(entry["phase"], entry["metric"])], "comparison CSV changed")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "baseline_capture_runs": baseline_result["capture_runs"],
        "production_capture_runs": production_result["capture_runs"],
        "comparisons": len(expected_comparisons),
    }


def seal(args: argparse.Namespace) -> int:
    root = Path(args.output).resolve()
    require(root.is_dir() and not root.is_symlink(), "output root must already exist")
    sealed = root / "sealed-artifacts"
    require(sealed.is_dir() and not sealed.is_symlink(), "sealed-artifacts must already exist")
    require(not any(sealed.iterdir()), "sealed-artifacts must be empty before comparison")
    require(set(root.iterdir()) == {sealed}, "output root contains unexpected entries")
    baseline_job = Path(args.baseline_job).resolve()
    production_job = Path(args.production_job).resolve()
    baseline_result = run_frozen_verifier(baseline_job, BASELINE_VERIFIER)
    production_result = run_frozen_verifier(production_job, PRODUCTION_VERIFIER)
    inputs, frozen_files, comparisons = collect_inputs(baseline_job, production_job)
    comparison_entries = comparison_manifest(comparisons)
    for entry in comparison_entries:
        atomic_write(
            sealed / entry["file"],
            comparisons[(entry["phase"], entry["metric"])],
        )
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "verifier_sha256": sha256(Path(__file__).resolve()),
        "provenance": inputs["provenance"],
        "frozen_density_files": frozen_files,
        "sessions": inputs["sessions"],
        "frozen_verifier_results": {
            "baseline": baseline_result,
            "production": production_result,
        },
        "gate": {
            "metrics": list(METRICS),
            "max_regression_pct": MAX_REGRESSION_PCT,
            "min_runs": MIN_RUNS,
        },
        "comparisons": comparison_entries,
    }
    atomic_write(root / "manifest.json", json_bytes(manifest))
    print(json.dumps(verify_root(root), indent=2, sort_keys=True))
    return 0


def self_test() -> int:
    baseline = {
        "case": {"valid_runs": "3", "p95_median_ms": "10", "p99_median_ms": "12"}
    }
    production = {
        "case": {"valid_runs": "3", "p95_median_ms": "10.4", "p99_median_ms": "11"}
    }
    rows = comparison_rows(baseline, production, "p95")
    require(rows[0]["delta_pct"] == "4.000" and rows[0]["regression"] == "false", "passing comparison differs")
    production["case"]["p95_median_ms"] = "10.6"
    rows = comparison_rows(baseline, production, "p95")
    require(rows[0]["regression"] == "true", "regression boundary is not fail-closed")
    print(json.dumps({"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE}, indent=2, sort_keys=True))
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    seal_parser = commands.add_parser("seal")
    seal_parser.add_argument("--baseline-job", required=True)
    seal_parser.add_argument("--production-job", required=True)
    seal_parser.add_argument("--output", required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    if args.command == "seal":
        return seal(args)
    if args.command == "verify":
        print(json.dumps(verify_root(Path(args.root).resolve()), indent=2, sort_keys=True))
        return 0
    if args.command == "self-test":
        return self_test()
    raise AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(
            json.dumps(
                {"schema_version": SCHEMA_VERSION, "status": "invalid", "error": str(error)},
                indent=2,
                sort_keys=True,
            )
        )
        raise SystemExit(1) from error
