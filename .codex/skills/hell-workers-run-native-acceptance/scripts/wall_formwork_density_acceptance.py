#!/usr/bin/env python3
"""Run Wall formwork production versus fallback Capture and Memory acceptance."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import statistics
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402
import wall_art_acceptance as art  # noqa: E402
import wall_density_acceptance as density  # noqa: E402
import wall_production_performance_acceptance as wallperf  # noqa: E402


SCHEMA_VERSION = 1
PROFILE = "wall-formwork-density-v1"
PRESENTATIONS = ("fallback-control", "production")
CASES = {
    "capture": (("provisional", "small"), ("provisional", "medium"), ("mixed", "medium")),
    "memory": (("mixed", "medium"),),
}
REPEAT = 3
CAPTURE_LIMIT = 1.05
RSS_LIMIT = 1.05
PEAK_LIVE_ALLOWANCE = 4 * 1024 * 1024


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_digest(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    native.require(files, f"Wall formwork artifact tree is empty: {root}")
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


def phase_sizes(instrumentation: str, phase: str) -> tuple[str, ...]:
    return tuple(size for case_phase, size in CASES[instrumentation] if case_phase == phase)


def phases(instrumentation: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(phase for phase, _ in CASES[instrumentation]))


def perf_command(
    repo: Path,
    output: Path,
    instrumentation: str,
    presentation: str,
    phase: str,
    adapter: str,
) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "wall-density",
        "--wall-phase",
        phase,
        "--wall-presentation",
        presentation,
        "--sizes",
        ",".join(phase_sizes(instrumentation, phase)),
        "--renders",
        "gpu",
        "--seed",
        str(density.SEED),
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


def schedule(instrumentation: str) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    sequence = 0
    for run_number in range(1, REPEAT + 1):
        for case_index, (phase, size) in enumerate(CASES[instrumentation]):
            fallback_first = (run_number + case_index) % 2 == 1
            modes = PRESENTATIONS if fallback_first else tuple(reversed(PRESENTATIONS))
            for presentation in modes:
                sequence += 1
                result.append(
                    {
                        "sequence": sequence,
                        "instrumentation": instrumentation,
                        "presentation": presentation,
                        "phase": phase,
                        "size": size,
                        "run": run_number,
                    }
                )
    return result


def run_directory(session: Path, case: Any, run_number: int) -> Path:
    path = session / "cases" / case.identifier / f"run-{run_number:03d}"
    native.require(path.is_dir() and not path.is_symlink(), f"run directory missing: {path}")
    return path


def capture_metrics(run_dir: Path) -> dict[str, float | int]:
    with (run_dir / "data/summary.csv").open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    native.require(len(rows) == 1, "Wall formwork summary is not a single row")
    values: dict[str, float | int] = {
        "samples": int(rows[0]["samples"]),
        "p50_ms": float(rows[0]["p50_ms"]),
        "p95_ms": float(rows[0]["p95_ms"]),
        "p99_ms": float(rows[0]["p99_ms"]),
    }
    return values


def memory_metrics(run_dir: Path) -> dict[str, int]:
    profile = native.read_json(run_dir / "profile-artifact.json")
    allocation = profile.get("allocation_memory")
    process = profile.get("process_memory")
    native.require(
        isinstance(allocation, dict)
        and isinstance(process, dict)
        and allocation.get("accounting_errors") == 0,
        "Wall formwork Memory accounting evidence differs",
    )
    return {
        "peak_live_bytes": int(allocation["peak_live_bytes"]),
        "max_rss_kib": int(process["max_rss_kib"]),
    }


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


def run_matrix(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Wall source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "Wall harness changed")
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed",
    )
    candidate = art.candidate_identity(repo, require_formwork=True)
    native.require(candidate == args.candidate_identity, "Wall candidate changed")
    inventory = wallperf.runtime_inventory_contract(repo, candidate)
    native.require(inventory["runtime_schema_version"] == 2, "Wall formwork runtime schema differs")
    binary = repo / "target/profiling/bevy_app"
    native.require(sha256(binary) == args.binary_sha256, "Wall profiling binary changed")
    root = Path(args.job_root).resolve()
    instrumentation = args.instrumentation

    build_parser, validate_arguments, prepare_session, run_one, loaded = wallperf.load_interleaved_modules(repo)
    Case, summarize_session = loaded
    sessions: dict[tuple[str, str], tuple[Any, Path, dict[str, Any]]] = {}
    for presentation in PRESENTATIONS:
        for phase in phases(instrumentation):
            command = perf_command(
                repo,
                root / instrumentation / presentation / "sessions" / phase,
                instrumentation,
                presentation,
                phase,
                args.adapter,
            )
            perf_args = build_parser().parse_args(command[2:])
            cases = {
                size: Case("wall-density", size, "gpu", density.SEED, 0, 0, wall_phase=phase)
                for size in phase_sizes(instrumentation, phase)
            }
            with wallperf.selected_presentation_environment(presentation, candidate):
                validate_arguments(perf_args)
                session = prepare_session(perf_args, binary, list(cases.values()), args.source_fingerprint)
            sessions[(presentation, phase)] = (perf_args, session, cases)

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
        presentation: {
            f"{phase}-{size}": [] for phase, size in CASES[instrumentation]
        }
        for presentation in PRESENTATIONS
    }
    cell_regimes: dict[tuple[str, str, str], list[dict[str, Any]]] = {}
    for item in order["schedule"]:
        presentation = item["presentation"]
        perf_args, session, cases = sessions[(presentation, item["phase"])]
        case = cases[item["size"]]
        with wallperf.selected_presentation_environment(presentation, candidate):
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
            "Wall formwork raw validation failed: " + "; ".join(validation.reasons),
        )
        run_dir = run_directory(session, case, item["run"])
        wallperf.verify_presentation_sidecar(
            run_dir / "data/wall_density_presentation.json",
            presentation=presentation,
            phase=item["phase"],
            size=item["size"],
            candidate=candidate,
            inventory=inventory,
        )
        metrics = capture_metrics(run_dir) if instrumentation == "capture" else memory_metrics(run_dir)
        observations[presentation][f"{item['phase']}-{item['size']}"].append(metrics)
        if instrumentation == "capture":
            regime = wallperf.frame_regime(int(metrics["samples"]), float(metrics["p50_ms"]))
            wallperf.require_unpaced(regime, item)
            cell = (presentation, item["phase"], item["size"])
            cell_regimes.setdefault(cell, []).append(regime)
        order["completed"].append({**item, "metrics": metrics, "completed_at": native.utc_now()})
        native.atomic_write_json(order_path, order)

    if instrumentation == "capture":
        for cell, regimes in cell_regimes.items():
            native.require(len(regimes) == REPEAT, "Wall formwork Capture cell is incomplete")
            wallperf.require_stable_cell(cell, regimes)
    session_records = []
    for (presentation, phase), (_, session, _) in sessions.items():
        native.require(summarize_session(session), "Wall formwork session summary failed")
        session_records.append(
            {
                "presentation": presentation,
                "phase": phase,
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


def comparison_records(results: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    comparisons: list[dict[str, Any]] = []
    metrics = {"capture": ("p95_ms", "p99_ms"), "memory": ("max_rss_kib", "peak_live_bytes")}
    for instrumentation, names in metrics.items():
        observations = results[instrumentation]["observations"]
        for phase, size in CASES[instrumentation]:
            case_id = f"{phase}-{size}"
            for metric in names:
                fallback = [row[metric] for row in observations["fallback-control"][case_id]]
                production = [row[metric] for row in observations["production"][case_id]]
                native.require(
                    len(fallback) == len(production) == REPEAT,
                    "Wall formwork comparison run count differs",
                )
                fallback_median, fallback_mad = median_and_mad(fallback)
                production_median, production_mad = median_and_mad(production)
                if metric == "peak_live_bytes":
                    limit = fallback_median + PEAK_LIVE_ALLOWANCE
                    rule = "fallback_median_plus_4_mib"
                else:
                    limit = fallback_median * (CAPTURE_LIMIT if instrumentation == "capture" else RSS_LIMIT)
                    rule = "fallback_median_plus_5_percent"
                comparisons.append(
                    {
                        "instrumentation": instrumentation,
                        "phase": phase,
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
                        "pass": production_median <= limit,
                    }
                )
    return comparisons


def write_comparisons(root: Path, records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    directory = root / "comparisons"
    directory.mkdir(exist_ok=True)
    sealed = []
    for record in records:
        path = directory / (
            f"fallback-control-vs-production-{record['instrumentation']}-"
            f"{record['phase']}-{record['size']}-{record['metric']}.csv"
        )
        with path.open("x", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=record.keys())
            writer.writeheader()
            writer.writerow(record)
        sealed.append({**record, "path": str(path.relative_to(root)), "sha256": sha256(path)})
        native.require(record["pass"], f"Wall formwork {record['instrumentation']}/{record['phase']}/{record['size']}/{record['metric']} exceeded its limit")
    return sealed


def rebuild_observations(
    *,
    repo: Path,
    root: Path,
    instrumentation: str,
    result: dict[str, Any],
    order: dict[str, Any],
    candidate: dict[str, Any],
    inventory: dict[str, Any],
) -> dict[str, dict[str, list[dict[str, Any]]]]:
    session_paths = {
        (entry["presentation"], entry["phase"]): root / entry["path"]
        for entry in result["sessions"]
    }
    native.require(
        set(session_paths)
        == {
            (presentation, phase)
            for presentation in PRESENTATIONS
            for phase in phases(instrumentation)
        },
        f"Wall formwork {instrumentation} session set differs",
    )
    for (presentation, phase), path in session_paths.items():
        expected = root / instrumentation / presentation / "sessions" / phase
        native.require(
            path == expected and path.is_dir() and not path.is_symlink(),
            f"Wall formwork {instrumentation} session path differs",
        )
    observations: dict[str, dict[str, list[dict[str, Any]]]] = {
        presentation: {
            f"{phase}-{size}": [] for phase, size in CASES[instrumentation]
        }
        for presentation in PRESENTATIONS
    }
    cell_regimes: dict[tuple[str, str, str], list[dict[str, Any]]] = {}
    Case, _ = density.load_perf_modules(repo)
    for expected, observed in zip(
        schedule(instrumentation), order["completed"], strict=True
    ):
        native.require(
            isinstance(observed, dict)
            and set(observed) == {*expected, "metrics", "completed_at"}
            and all(observed[key] == value for key, value in expected.items())
            and isinstance(observed["completed_at"], str),
            f"Wall formwork {instrumentation} completed order differs",
        )
        case = Case(
            "wall-density",
            observed["size"],
            "gpu",
            density.SEED,
            0,
            0,
            wall_phase=observed["phase"],
        )
        session = session_paths[(observed["presentation"], observed["phase"])]
        run_dir = run_directory(session, case, observed["run"])
        wallperf.verify_presentation_sidecar(
            run_dir / "data/wall_density_presentation.json",
            presentation=observed["presentation"],
            phase=observed["phase"],
            size=observed["size"],
            candidate=candidate,
            inventory=inventory,
        )
        metrics = (
            capture_metrics(run_dir)
            if instrumentation == "capture"
            else memory_metrics(run_dir)
        )
        native.require(
            observed["metrics"] == metrics,
            f"Wall formwork {instrumentation} metrics differ from raw artifacts",
        )
        observations[observed["presentation"]][
            f"{observed['phase']}-{observed['size']}"
        ].append(metrics)
        if instrumentation == "capture":
            regime = wallperf.frame_regime(
                int(metrics["samples"]), float(metrics["p50_ms"])
            )
            wallperf.require_unpaced(regime, observed)
            cell_regimes.setdefault(
                (observed["presentation"], observed["phase"], observed["size"]),
                [],
            ).append(regime)
    if instrumentation == "capture":
        for cell, regimes in cell_regimes.items():
            native.require(
                len(regimes) == REPEAT, "Wall formwork Capture cell is incomplete"
            )
            wallperf.require_stable_cell(cell, regimes)
    return observations


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "Wall formwork density manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    density.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"]
        and native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"]
        and density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"]
        and art.candidate_identity(repo, require_formwork=True) == manifest["candidate_identity"],
        "Wall formwork density provenance changed",
    )
    candidate = manifest["candidate_identity"]
    inventory = wallperf.runtime_inventory_contract(repo, candidate)
    native.require(
        manifest["binary_sha256"]["capture"] != manifest["binary_sha256"]["memory"]
        and sha256(repo / "target/profiling/bevy_app") == manifest["binary_sha256"]["memory"],
        "Wall formwork instrumentation binary ownership differs",
    )
    results = {}
    for instrumentation in ("capture", "memory"):
        result_path = root / f"{instrumentation}-result.json"
        native.require(
            sha256(result_path) == manifest[f"{instrumentation}_result_sha256"],
            f"Wall formwork {instrumentation} result changed",
        )
        result = native.read_json(result_path)
        native.require(
            set(result)
            == {
                "schema_version",
                "status",
                "instrumentation",
                "binary_sha256",
                "sessions",
                "observations",
                "order_sha256",
            }
            and result.get("schema_version") == SCHEMA_VERSION
            and result.get("status") == "pass"
            and result.get("instrumentation") == instrumentation
            and result.get("binary_sha256") == manifest["binary_sha256"][instrumentation],
            f"Wall formwork {instrumentation} result identity differs",
        )
        for session in result["sessions"]:
            native.require(
                tree_digest(root / session["path"]) == session["digest"],
                "Wall formwork session changed",
            )
        order_path = root / f"{instrumentation}-order.json"
        order = native.read_json(order_path)
        native.require(
            set(order)
            == {
                "schema_version",
                "profile",
                "instrumentation",
                "status",
                "schedule",
                "completed",
            }
            and order.get("schema_version") == SCHEMA_VERSION
            and order.get("profile") == PROFILE
            and order.get("instrumentation") == instrumentation
            and sha256(order_path) == result["order_sha256"]
            and order.get("status") == "pass"
            and order.get("schedule") == schedule(instrumentation)
            and len(order.get("completed", [])) == len(schedule(instrumentation)),
            f"Wall formwork {instrumentation} order differs",
        )
        observations = rebuild_observations(
            repo=repo,
            root=root,
            instrumentation=instrumentation,
            result=result,
            order=order,
            candidate=candidate,
            inventory=inventory,
        )
        native.require(
            result.get("observations") == observations,
            f"Wall formwork {instrumentation} observations differ",
        )
        results[instrumentation] = result
    recalculated = comparison_records(results)
    native.require(len(recalculated) == len(manifest["comparisons"]), "Wall formwork comparisons differ")
    for expected, sealed in zip(recalculated, manifest["comparisons"], strict=True):
        expected_path = (
            Path("comparisons")
            / (
                f"fallback-control-vs-production-{expected['instrumentation']}-"
                f"{expected['phase']}-{expected['size']}-{expected['metric']}.csv"
            )
        )
        native.require(
            all(sealed.get(key) == value for key, value in expected.items())
            and sealed.get("pass") is True
            and sealed.get("path") == str(expected_path)
            and sha256(root / sealed["path"]) == sealed["sha256"],
            "Wall formwork comparison changed",
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "capture_runs": len(schedule("capture")),
        "memory_runs": len(schedule("memory")),
        "comparisons": len(recalculated),
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    candidate = None
    try:
        candidate = art.candidate_identity(repo, require_formwork=True)
        wallperf.runtime_inventory_contract(repo, candidate)
    except native.AcceptanceError as error:
        failures.append(str(error))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    try:
        density.assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "wall-formwork-density")
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
                "capture_runs": len(schedule("capture")),
                "memory_runs": len(schedule("memory")),
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
        "Wall formwork density must use the planned kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Wall source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "Wall harness changed")
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(density.asset_view_fingerprint(repo) == args.asset_view_fingerprint, "Wall asset view changed")
    candidate = art.candidate_identity(repo, require_formwork=True)
    native.require(candidate == args.candidate_identity, "Wall candidate changed")
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
                timeout_seconds=(density.RUN_TIMEOUT_SECONDS * len(schedule(instrumentation))) + 900.0,
            )
        native.require(binary_hashes["capture"] != binary_hashes["memory"], "Capture and Memory binaries match")
        results = {
            name: native.read_json(root / f"{name}-result.json")
            for name in ("capture", "memory")
        }
        comparisons = write_comparisons(root, comparison_records(results))
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
    candidate = {
        "authority": "isolated_candidate",
        "asset_set_generation": 8,
        "manifest_sha256": "a" * 64,
    }
    build_parser, validate_arguments, _, _, _ = wallperf.load_interleaved_modules(Path.cwd())
    for instrumentation, phase in (
        ("capture", "provisional"),
        ("capture", "mixed"),
        ("memory", "mixed"),
    ):
        command = perf_command(
            Path.cwd(),
            Path("/job") / instrumentation / phase,
            instrumentation,
            "production",
            phase,
            "Intel",
        )
        perf_args = build_parser().parse_args(command[2:])
        with wallperf.selected_presentation_environment("production", candidate):
            validate_arguments(perf_args)
    for instrumentation, expected in (("capture", 18), ("memory", 6)):
        rows = schedule(instrumentation)
        native.require(len(rows) == expected, "Wall formwork schedule length differs")
        for first, second in zip(rows[::2], rows[1::2], strict=True):
            native.require(
                (first["phase"], first["size"], first["run"])
                == (second["phase"], second["size"], second["run"])
                and {first["presentation"], second["presentation"]} == set(PRESENTATIONS),
                "Wall formwork pairing differs",
            )
    native.require(phase_sizes("capture", "provisional") == ("small", "medium"), "Wall provisional cases differ")
    native.require(phase_sizes("capture", "mixed") == ("medium",), "Wall mixed Capture differs")
    native.require(phase_sizes("memory", "mixed") == ("medium",), "Wall mixed Memory differs")
    native.require(median_and_mad([10, 12, 14]) == (12.0, 2.0), "median/MAD differs")
    native.print_json({"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE})
    return 0


def json_object(value: str) -> dict[str, Any]:
    parsed = json.loads(value)
    if not isinstance(parsed, dict):
        raise argparse.ArgumentTypeError("expected a JSON object")
    return parsed


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", default=".")
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
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
        raise SystemExit(1) from error
