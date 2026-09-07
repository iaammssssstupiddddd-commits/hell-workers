#!/usr/bin/env python3
"""Run Door production versus fallback-control Capture and Memory acceptance."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import statistics
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import door_art_acceptance as art  # noqa: E402
import door_behavior_acceptance as behavior  # noqa: E402
import native_acceptance as native  # noqa: E402
import wall_density_acceptance as density  # noqa: E402
import wall_production_performance_acceptance as wallperf  # noqa: E402

SCHEMA_VERSION = 1
PROFILE = "door-density-v1"
PRESENTATIONS = ("fallback-control", "production")
SIZES = {"capture": ("small", "medium"), "memory": ("medium",)}
SEED = 20_260_906
REPEAT = 3
MEASURE_SECONDS = 60.0
CAPTURE_LIMIT = 1.05
RSS_LIMIT = 1.05
PEAK_LIVE_ALLOWANCE = 4 * 1024 * 1024
ENV_KEYS = (
    "HW_DOOR_ART_PREVIEW",
    "HW_DOOR_CANDIDATE",
    "HW_DOOR_CANDIDATE_GENERATION",
    "HW_DOOR_CANDIDATE_MANIFEST_SHA256",
    "HW_DOOR_PERF_PRESENTATION",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    native.require(files, f"Door density artifact tree is empty: {root}")
    for path in files:
        native.require(not path.is_symlink(), f"artifact is a symlink: {path}")
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256(path)))
    return digest.hexdigest()


def build_command(instrumentation: str) -> list[str]:
    return [
        "python3",
        "scripts/dev.py",
        "cargo",
        "--",
        "build",
        "--profile",
        "profiling",
        "--no-default-features",
        "--features",
        "profiling" if instrumentation == "capture" else "profiling-memory",
    ]


def perf_command(
    repo: Path,
    output: Path,
    instrumentation: str,
    presentation: str,
    adapter: str,
) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "door-density",
        "--door-presentation",
        presentation,
        "--sizes",
        ",".join(SIZES[instrumentation]),
        "--renders",
        "gpu",
        "--seed",
        str(SEED),
        "--repeat",
        str(REPEAT),
        "--preflight-runs",
        "0",
        "--souls",
        "0",
        "--familiars",
        "0",
        "--warmup-secs",
        "30",
        "--measure-secs",
        "60",
        "--instrumentation",
        instrumentation,
        "--adapter",
        adapter,
        "--backend",
        "vulkan",
        "--window-backend",
        "x11",
        "--present-mode",
        "novsync",
        "--window-width",
        "1280",
        "--window-height",
        "720",
        "--window-scale-factor",
        "1.0",
        "--rtt-quality",
        "high",
        "--binary",
        str(repo / "target/profiling/bevy_app"),
        "--skip-build",
        "--output",
        str(output),
    ]


def presentation_environment(
    presentation: str, candidate: dict[str, Any]
) -> dict[str, str]:
    environment = os.environ.copy()
    for key in ENV_KEYS:
        environment.pop(key, None)
    environment["HW_DOOR_PERF_PRESENTATION"] = presentation
    if presentation == "production":
        environment.update(
            {
                "HW_DOOR_CANDIDATE": "1",
                "HW_DOOR_CANDIDATE_GENERATION": str(candidate["asset_set_generation"]),
                "HW_DOOR_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
            }
        )
    return environment


@contextmanager
def selected_environment(presentation: str, candidate: dict[str, Any]):
    replacement = presentation_environment(presentation, candidate)
    previous = {key: os.environ.get(key) for key in ENV_KEYS}
    try:
        for key in ENV_KEYS:
            value = replacement.get(key)
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        yield
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def schedule(instrumentation: str) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    sequence = 0
    for run_number in range(1, REPEAT + 1):
        for size_index, size in enumerate(SIZES[instrumentation]):
            fallback_first = (run_number + size_index) % 2 == 1
            modes = PRESENTATIONS if fallback_first else tuple(reversed(PRESENTATIONS))
            for presentation in modes:
                sequence += 1
                result.append(
                    {
                        "sequence": sequence,
                        "instrumentation": instrumentation,
                        "presentation": presentation,
                        "size": size,
                        "run": run_number,
                    }
                )
    return result


def verify_fixture(
    fixture: dict[str, Any] | None,
    *,
    presentation: str,
    size: str,
    candidate: dict[str, Any],
) -> None:
    native.require(isinstance(fixture, dict), "Door density fixture evidence is absent")
    target_count = 32 if size == "small" else 128
    evidence = fixture.get("initial")
    native.require(
        fixture.get("stable") is True
        and fixture.get("final") == evidence
        and fixture.get("target_door_count") == target_count
        and fixture.get("support_wall_count") == target_count * 2,
        "Door density fixture identity differs",
    )
    native.require(
        isinstance(evidence, dict)
        and evidence.get("expected_mode") == presentation
        and evidence.get("asset_set_generation") == candidate["asset_set_generation"]
        and evidence.get("authority") == candidate["authority"]
        and evidence.get("manifest_sha256") == candidate["manifest_sha256"],
        "Door density presentation identity differs",
    )


def frame_metrics(run_dir: Path) -> dict[str, float | int]:
    with (run_dir / "data/summary.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    native.require(len(rows) == 1, "Door density summary is not a single row")
    values: dict[str, float | int] = {
        "samples": int(rows[0]["samples"]),
        "p50_ms": float(rows[0]["p50_ms"]),
        "p95_ms": float(rows[0]["p95_ms"]),
        "p99_ms": float(rows[0]["p99_ms"]),
    }
    fps = int(values["samples"]) / MEASURE_SECONDS
    native.require(
        not (
            abs(fps - wallperf.DISPLAY_FRAME_CLOCK_HZ)
            <= wallperf.FRAME_CLOCK_FPS_TOLERANCE
            and abs(float(values["p50_ms"]) - 1000.0 / wallperf.DISPLAY_FRAME_CLOCK_HZ)
            <= wallperf.FRAME_CLOCK_P50_TOLERANCE_MS
        ),
        "Door density Capture was paced by the display frame clock",
    )
    return values


def memory_metrics(run_dir: Path) -> dict[str, int]:
    profile = native.read_json(run_dir / "profile-artifact.json")
    allocation = profile.get("allocation_memory")
    process = profile.get("process_memory")
    native.require(
        isinstance(allocation, dict)
        and isinstance(process, dict)
        and allocation.get("accounting_errors") == 0,
        "Door density Memory accounting evidence differs",
    )
    return {
        "peak_live_bytes": int(allocation["peak_live_bytes"]),
        "max_rss_kib": int(process["max_rss_kib"]),
    }


def run_directory(session: Path, case: Any, run_number: int) -> Path:
    path = session / "cases" / case.identifier / f"run-{run_number:03d}"
    native.require(path.is_dir() and not path.is_symlink(), f"run directory missing: {path}")
    return path


def run_matrix(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Door subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Door source changed")
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Door harness changed",
    )
    behavior.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Door asset view changed",
    )
    candidate = behavior.candidate_identity(repo)
    native.require(candidate == args.candidate_identity, "Door candidate changed")
    instrumentation = args.instrumentation
    binary = repo / "target/profiling/bevy_app"
    native.require(sha256(binary) == args.binary_sha256, "Door profiling binary changed")
    root = Path(args.job_root).resolve()

    build_parser, validate_arguments, prepare_session, run_one, loaded = (
        wallperf.load_interleaved_modules(repo)
    )
    Case, summarize_session = loaded
    sessions: dict[str, tuple[Any, Path, dict[str, Any]]] = {}
    for presentation in PRESENTATIONS:
        command = perf_command(
            repo,
            root / instrumentation / presentation,
            instrumentation,
            presentation,
            args.adapter,
        )
        perf_args = build_parser().parse_args(command[2:])
        cases = {
            size: Case("door-density", size, "gpu", SEED, 0, 0)
            for size in SIZES[instrumentation]
        }
        with selected_environment(presentation, candidate):
            validate_arguments(perf_args)
            session = prepare_session(
                perf_args, binary, list(cases.values()), args.source_fingerprint
            )
        sessions[presentation] = (perf_args, session, cases)

    order_path = root / f"{instrumentation}-order.json"
    order = {
        "schema_version": SCHEMA_VERSION,
        "profile": PROFILE,
        "instrumentation": instrumentation,
        "status": "running",
        "schedule": schedule(instrumentation),
        "completed": [],
    }
    native.atomic_write_json(order_path, order)
    observations: dict[str, dict[str, list[dict[str, Any]]]] = {
        presentation: {size: [] for size in SIZES[instrumentation]}
        for presentation in PRESENTATIONS
    }
    for item in order["schedule"]:
        presentation = item["presentation"]
        perf_args, session, cases = sessions[presentation]
        case = cases[item["size"]]
        with selected_environment(presentation, candidate):
            validation = run_one(
                args=perf_args,
                binary=binary,
                session_dir=session,
                case=case,
                run_number=item["run"],
                preflight=False,
            )
        native.require(
            validation.valid,
            "Door density raw validation failed: " + "; ".join(validation.reasons),
        )
        verify_fixture(
            validation.door_density_fixture,
            presentation=presentation,
            size=item["size"],
            candidate=candidate,
        )
        run_dir = run_directory(session, case, item["run"])
        metrics = (
            frame_metrics(run_dir)
            if instrumentation == "capture"
            else memory_metrics(run_dir)
        )
        observations[presentation][item["size"]].append(metrics)
        order["completed"].append({**item, "metrics": metrics, "completed_at": native.utc_now()})
        native.atomic_write_json(order_path, order)

    session_records = []
    for presentation in PRESENTATIONS:
        _, session, _ = sessions[presentation]
        native.require(summarize_session(session), "Door density session summary failed")
        session_records.append(
            {
                "presentation": presentation,
                "path": str(session.relative_to(root)),
                "digest": tree_digest(session),
            }
        )
    order["status"] = "pass"
    native.atomic_write_json(order_path, order)
    result = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "instrumentation": instrumentation,
        "binary_sha256": args.binary_sha256,
        "sessions": session_records,
        "observations": observations,
        "order_sha256": sha256(order_path),
    }
    native.atomic_write_json(root / f"{instrumentation}-result.json", result)
    native.print_json(result)
    return 0


def median_and_mad(values: list[float | int]) -> tuple[float, float]:
    median = float(statistics.median(values))
    mad = float(statistics.median(abs(float(value) - median) for value in values))
    return median, mad


def compare_results(root: Path) -> list[dict[str, Any]]:
    results = {
        name: native.read_json(root / f"{name}-result.json")
        for name in ("capture", "memory")
    }
    comparisons: list[dict[str, Any]] = []
    directory = root / "comparisons"
    directory.mkdir(exist_ok=True)
    metrics = {
        "capture": ("p95_ms", "p99_ms"),
        "memory": ("max_rss_kib", "peak_live_bytes"),
    }
    for instrumentation, names in metrics.items():
        observations = results[instrumentation]["observations"]
        for size in SIZES[instrumentation]:
            for metric in names:
                fallback = [row[metric] for row in observations["fallback-control"][size]]
                production = [row[metric] for row in observations["production"][size]]
                fallback_median, fallback_mad = median_and_mad(fallback)
                production_median, production_mad = median_and_mad(production)
                if metric == "peak_live_bytes":
                    limit = fallback_median + PEAK_LIVE_ALLOWANCE
                    rule = "fallback_median_plus_4_mib"
                else:
                    limit = fallback_median * (
                        CAPTURE_LIMIT if instrumentation == "capture" else RSS_LIMIT
                    )
                    rule = "fallback_median_plus_5_percent"
                passed = production_median <= limit
                record = {
                    "instrumentation": instrumentation,
                    "size": size,
                    "metric": metric,
                    "fallback_values": fallback,
                    "production_values": production,
                    "fallback_median": fallback_median,
                    "fallback_mad": fallback_mad,
                    "production_median": production_median,
                    "production_mad": production_mad,
                    "limit": limit,
                    "rule": rule,
                    "pass": passed,
                }
                path = directory / f"fallback-control-vs-production-{instrumentation}-{size}-{metric}.csv"
                with path.open("x", newline="", encoding="utf-8") as handle:
                    writer = csv.DictWriter(handle, fieldnames=record.keys())
                    writer.writeheader()
                    writer.writerow(record)
                comparisons.append({**record, "path": str(path.relative_to(root)), "sha256": sha256(path)})
                native.require(passed, f"Door density {instrumentation}/{size}/{metric} exceeded its limit")
    return comparisons


def matrix_command(
    args: argparse.Namespace,
    instrumentation: str,
    binary_sha256: str,
    candidate: dict[str, Any],
) -> list[str]:
    return [
        "python3",
        str(Path(__file__).resolve()),
        "matrix",
        "--repo",
        str(Path(args.repo).resolve()),
        "--job-root",
        str(Path(args.job_root).resolve()),
        "--subject-commit",
        args.subject_commit,
        "--source-fingerprint",
        args.source_fingerprint,
        "--harness-fingerprint",
        args.harness_fingerprint,
        "--asset-view-fingerprint",
        args.asset_view_fingerprint,
        "--candidate-identity",
        json.dumps(candidate, separators=(",", ":")),
        "--binary-sha256",
        binary_sha256,
        "--instrumentation",
        instrumentation,
        "--adapter",
        args.adapter,
    ]


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "Door density manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    behavior.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"]
        and native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"]
        and density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"]
        and behavior.candidate_identity(repo) == manifest["candidate_identity"],
        "Door density provenance changed",
    )
    for instrumentation in ("capture", "memory"):
        result_path = root / f"{instrumentation}-result.json"
        native.require(
            sha256(result_path) == manifest[f"{instrumentation}_result_sha256"],
            f"Door density {instrumentation} result changed",
        )
        result = native.read_json(result_path)
        for session in result["sessions"]:
            path = root / session["path"]
            native.require(tree_digest(path) == session["digest"], "Door density session changed")
        order_path = root / f"{instrumentation}-order.json"
        native.require(sha256(order_path) == result["order_sha256"], "Door density order changed")
    for comparison in manifest["comparisons"]:
        native.require(
            sha256(root / comparison["path"]) == comparison["sha256"]
            and comparison["pass"] is True,
            "Door density comparison changed",
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "capture_runs": 12,
        "memory_runs": 6,
        "comparisons": len(manifest["comparisons"]),
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    candidate = None
    try:
        candidate = behavior.candidate_identity(repo)
    except native.AcceptanceError as error:
        failures.append(str(error))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    try:
        behavior.assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "door-density")
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1",
        "python3", str(Path(__file__).resolve()), "run",
        "--repo", str(repo), "--job-root", str(root),
        "--subject-commit", subject, "--source-fingerprint", source,
        "--harness-fingerprint", harness, "--asset-view-fingerprint", assets,
        "--candidate-identity", json.dumps(candidate or {}, separators=(",", ":")),
        "--adapter", args.adapter,
    ]
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": PROFILE,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "status_command": ["python3", str(Path(__file__).resolve()), "status", "--job-root", str(root)],
            "execution_contract": {
                "actual_window_required": True,
                "parallel_game_processes": 1,
                "capture_runs": 12,
                "memory_runs": 6,
                "pairing": "adjacent-counterbalanced",
                "build_order": ["capture", "memory"],
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Door density must use the planned kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Door subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Door source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "Door harness changed")
    behavior.assert_fully_clean(repo, args.subject_commit)
    native.require(density.asset_view_fingerprint(repo) == args.asset_view_fingerprint, "Door asset view changed")
    candidate = behavior.candidate_identity(repo)
    native.require(candidate == args.candidate_identity, "Door candidate changed")
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": PROFILE,
        "subject_commit": args.subject_commit,
        "current_stage": "capture-build",
        "child_pid": None,
        "started_at": native.utc_now(),
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        binary_hashes = {}
        for instrumentation in ("capture", "memory"):
            state["current_stage"] = f"{instrumentation}-build"
            native.atomic_write_json(root / "job.json", state)
            native.run_command(
                f"{instrumentation}-build",
                build_command(instrumentation),
                repo=repo,
                env=os.environ.copy(),
                log_path=root / f"{instrumentation}-build.log",
                job_file=root / "job.json",
                state=state,
            )
            binary = repo / "target/profiling/bevy_app"
            native.require(binary.is_file() and not binary.is_symlink(), "profiling binary missing")
            binary_hashes[instrumentation] = sha256(binary)
            command = matrix_command(args, instrumentation, binary_hashes[instrumentation], candidate)
            state["current_stage"] = f"{instrumentation}-matrix"
            native.atomic_write_json(root / "job.json", state)
            native.run_command(
                f"{instrumentation}-matrix",
                command,
                repo=repo,
                env=os.environ.copy(),
                log_path=root / f"{instrumentation}-matrix.log",
                job_file=root / "job.json",
                state=state,
                timeout_seconds=(90.0 * len(schedule(instrumentation))) + 900.0,
            )
        comparisons = compare_results(root)
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": PROFILE,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "binary_sha256": binary_hashes,
            "capture_result_sha256": sha256(root / "capture-result.json"),
            "memory_result_sha256": sha256(root / "memory-result.json"),
            "comparisons": comparisons,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "current_stage": "complete", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        native.print_json(verify_root(root))
        return 0
    except Exception as error:
        state.update({"status": "invalid", "failure": f"{type(error).__name__}: {error}", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        raise


def status(args: argparse.Namespace) -> int:
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def self_test() -> int:
    candidate = {"authority": "isolated_candidate", "asset_set_generation": 6, "manifest_sha256": "a" * 64}
    production = presentation_environment("production", candidate)
    fallback = presentation_environment("fallback-control", candidate)
    native.require(
        production.get("HW_DOOR_CANDIDATE") == "1"
        and "HW_DOOR_CANDIDATE" not in fallback
        and production["HW_DOOR_PERF_PRESENTATION"] == "production"
        and fallback["HW_DOOR_PERF_PRESENTATION"] == "fallback-control",
        "Door density environments are not isolated",
    )
    for instrumentation, expected in (("capture", 12), ("memory", 6)):
        rows = schedule(instrumentation)
        native.require(len(rows) == expected, "Door density schedule length differs")
        for first, second in zip(rows[::2], rows[1::2], strict=True):
            native.require(
                (first["size"], first["run"]) == (second["size"], second["run"])
                and {first["presentation"], second["presentation"]} == set(PRESENTATIONS),
                "Door density pairing differs",
            )
    native.require(median_and_mad([10, 12, 14]) == (12.0, 2.0), "median/MAD differs")
    native.print_json({"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE})
    return 0


def json_object(value: str) -> dict[str, Any]:
    parsed = json.loads(value)
    if not isinstance(parsed, dict):
        raise argparse.ArgumentTypeError("expected a JSON object")
    return parsed


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    commands = result.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", default=".")
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel(R) Arc(TM)")
    run_parser = commands.add_parser("run")
    matrix_parser = commands.add_parser("matrix")
    for target in (run_parser, matrix_parser):
        target.add_argument("--repo", required=True)
        target.add_argument("--job-root", required=True)
        target.add_argument("--subject-commit", required=True)
        target.add_argument("--source-fingerprint", required=True)
        target.add_argument("--harness-fingerprint", required=True)
        target.add_argument("--asset-view-fingerprint", required=True)
        target.add_argument("--candidate-identity", required=True, type=json_object)
        target.add_argument("--adapter", required=True)
    matrix_parser.add_argument("--instrumentation", required=True, choices=("capture", "memory"))
    matrix_parser.add_argument("--binary-sha256", required=True)
    status_parser = commands.add_parser("status")
    status_parser.add_argument("--job-root", required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return result


def main() -> int:
    args = parser().parse_args()
    if args.command == "plan":
        return plan(args)
    if args.command == "run":
        return run(args)
    if args.command == "matrix":
        return run_matrix(args)
    if args.command == "status":
        return status(args)
    if args.command == "verify":
        native.print_json(verify_root(Path(args.job_root).resolve()))
        return 0
    return self_test()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (native.AcceptanceError, RuntimeError, OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
