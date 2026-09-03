#!/usr/bin/env python3
"""Run the final Wall production versus fallback-control Capture pair."""

from __future__ import annotations

import argparse
import csv
import os
import re
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402
import wall_art_acceptance as art  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 2
PROFILE = "wall-production-performance-v2"
PRESENTATIONS = ("fallback-control", "production")
EXPECTED_FAMILY_MULTIPLIERS = {
    "isolated": 1,
    "end": 4,
    "straight": 2,
    "corner": 4,
    "t_junction": 4,
    "cross": 1,
}
EXPECTED_ROTATION_MULTIPLIERS = {"0": 6, "1": 4, "2": 3, "3": 3}
CANDIDATE_ENV_KEYS = (
    "HW_WALL_CANDIDATE",
    "HW_WALL_CANDIDATE_GENERATION",
    "HW_WALL_CANDIDATE_MANIFEST_SHA256",
)
CAPTURE_ENV_KEYS = (*CANDIDATE_ENV_KEYS, "HW_WALL_PERF_PRESENTATION")


def presentation_command(
    repo: Path,
    root: Path,
    presentation: str,
    phase: str,
    adapter: str,
) -> list[str]:
    command = density.phase_command(repo, root, phase, adapter)
    command.extend(["--wall-presentation", presentation])
    return command


def presentation_environment(
    presentation: str, candidate: dict[str, Any]
) -> dict[str, str]:
    environment = os.environ.copy()
    for key in CANDIDATE_ENV_KEYS:
        environment.pop(key, None)
    environment["HW_WALL_PERF_PRESENTATION"] = presentation
    if presentation == "production":
        environment.update(
            {
                "HW_WALL_CANDIDATE": "1",
                "HW_WALL_CANDIDATE_GENERATION": str(
                    candidate["asset_set_generation"]
                ),
                "HW_WALL_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
            }
        )
    return environment


def capture_schedule() -> list[dict[str, Any]]:
    """Pair controls with production while counterbalancing which mode runs first."""
    schedule = []
    sequence = 0
    for run_number in range(1, density.REPEAT + 1):
        for phase_index, phase in enumerate(density.PHASES):
            for size_index, size in enumerate(density.SIZES):
                fallback_first = (run_number + phase_index + size_index) % 2 == 1
                presentations = (
                    PRESENTATIONS if fallback_first else tuple(reversed(PRESENTATIONS))
                )
                for presentation in presentations:
                    sequence += 1
                    schedule.append(
                        {
                            "sequence": sequence,
                            "presentation": presentation,
                            "phase": phase,
                            "size": size,
                            "run": run_number,
                        }
                    )
    return schedule


@contextmanager
def selected_presentation_environment(
    presentation: str, candidate: dict[str, Any]
):
    replacement = presentation_environment(presentation, candidate)
    previous = {key: os.environ.get(key) for key in CAPTURE_ENV_KEYS}
    try:
        for key in CAPTURE_ENV_KEYS:
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


def expected_distribution(
    multipliers: dict[str, int], target_count: int
) -> dict[str, int]:
    native.require(target_count % 16 == 0, "Wall target count is not mask-balanced")
    repeat = target_count // 16
    return {key: value * repeat for key, value in multipliers.items()}


def verify_presentation_sidecar(
    path: Path,
    *,
    presentation: str,
    phase: str,
    size: str,
    candidate: dict[str, Any],
) -> dict[str, Any]:
    value = native.read_json(path)
    native.require(
        set(value) == {"schema_version", "stable", "initial", "final"},
        "Wall presentation sidecar fields differ",
    )
    native.require(
        value["schema_version"] == SCHEMA_VERSION and value["stable"] is True,
        "Wall presentation sidecar is not stable",
    )
    initial = value["initial"]
    native.require(
        isinstance(initial, dict) and value["final"] == initial,
        "Wall presentation changed during capture",
    )
    target_count = 96 if size == "small" else 384
    native.require(
        initial.get("schema_version") == SCHEMA_VERSION
        and initial.get("expected_mode") == presentation
        and initial.get("phase") == phase
        and initial.get("target_wall_count") == target_count
        and initial.get("visual_count") == target_count,
        "Wall presentation fixture identity differs",
    )
    expected_production = target_count if presentation == "production" else 0
    expected_fallback = target_count if presentation == "fallback-control" else 0
    expected_active_meshes = 6 if presentation == "production" else 1
    expected_active_pairs = 6 if presentation == "production" else 1
    native.require(
        initial.get("production_count") == expected_production
        and initial.get("fallback_count") == expected_fallback
        and initial.get("active_mesh_count") == expected_active_meshes
        and initial.get("active_material_count") == 1
        and initial.get("active_mesh_material_pair_count") == expected_active_pairs,
        "Wall presentation active residency differs",
    )
    native.require(
        initial.get("resident_production_mesh_count") == 6
        and initial.get("resident_production_material_count") == 2
        and initial.get("resident_fallback_mesh_count") == 1
        and initial.get("resident_fallback_material_count") == 2
        and initial.get("total_wall_mesh_pool_count") == 7
        and initial.get("total_wall_material_pool_count") == 4
        and initial.get("production_materials_lit") is True,
        "Wall presentation finite pool differs",
    )
    triangles = initial.get("production_mesh_triangles")
    native.require(
        isinstance(triangles, list)
        and len(triangles) == 6
        and all(type(value) is int and 0 < value <= 72 for value in triangles)
        and initial.get("max_production_mesh_triangles") == max(triangles),
        "Wall production triangle budget differs",
    )
    native.require(
        initial.get("family_counts")
        == expected_distribution(EXPECTED_FAMILY_MULTIPLIERS, target_count)
        and initial.get("rotation_counts")
        == expected_distribution(EXPECTED_ROTATION_MULTIPLIERS, target_count),
        "Wall topology distribution differs",
    )
    native.require(
        initial.get("asset_set_generation") == candidate["asset_set_generation"]
        and initial.get("authority") == candidate["authority"]
        and initial.get("manifest_sha256") == candidate["manifest_sha256"],
        "Wall presentation candidate identity differs",
    )
    for field in (
        "session_id",
        "readiness_revision",
        "decision_revision",
        "presentation_revision",
    ):
        native.require(
            type(initial.get(field)) is int and initial[field] >= 0,
            f"Wall presentation {field} is invalid",
        )
    return value


def verify_session_presentations(
    session: Path,
    *,
    repo: Path,
    presentation: str,
    phase: str,
    candidate: dict[str, Any],
) -> list[dict[str, Any]]:
    Case, _ = density.load_perf_modules(repo)
    sidecars = []
    for size in density.SIZES:
        case = Case(
            "wall-density", size, "gpu", density.SEED, 0, 0, wall_phase=phase
        )
        for run_number in range(1, density.REPEAT + 1):
            run = density.locate_run(session, case.identifier, run_number)
            sidecars.append(
                verify_presentation_sidecar(
                    run / "data" / "wall_density_presentation.json",
                    presentation=presentation,
                    phase=phase,
                    size=size,
                    candidate=candidate,
                )
            )
    return sidecars


def comparison_command(
    baseline: Path, candidate: Path, metric: str, output: Path
) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "compare",
        "--baseline",
        str(baseline),
        "--candidate",
        str(candidate),
        "--metric",
        metric,
        "--max-regression-pct",
        "5",
        "--min-runs",
        "3",
        "--output",
        str(output),
    ]


def interleaved_command(
    *,
    repo: Path,
    root: Path,
    subject_commit: str,
    source_fingerprint: str,
    harness_fingerprint: str,
    asset_view_fingerprint: str,
    candidate: dict[str, Any],
    adapter: str,
) -> list[str]:
    return [
        "python3",
        str(Path(__file__).resolve()),
        "capture-interleaved",
        "--repo",
        str(repo),
        "--job-root",
        str(root),
        "--subject-commit",
        subject_commit,
        "--source-fingerprint",
        source_fingerprint,
        "--harness-fingerprint",
        harness_fingerprint,
        "--asset-view-fingerprint",
        asset_view_fingerprint,
        "--candidate-generation",
        str(candidate["asset_set_generation"]),
        "--candidate-manifest-sha256",
        candidate["manifest_sha256"],
        "--adapter",
        adapter,
    ]


def comparison_evidence(path: Path, *, phase: str, metric: str) -> dict[str, Any]:
    native.require(path.is_file() and not path.is_symlink(), "Wall comparison is absent")
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    native.require(len(rows) == len(density.SIZES), "Wall comparison case count differs")
    expected_cases = {
        f"wall-density-{size}-gpu-seed-{density.SEED}-souls-0-familiars-0-wall-{phase}"
        for size in density.SIZES
    }
    native.require(
        {row.get("case_id") for row in rows} == expected_cases,
        "Wall comparison cases differ",
    )
    for row in rows:
        native.require(
            row.get("metric") == metric and row.get("regression") == "false",
            "Wall comparison exceeds the five-percent gate",
        )
        try:
            baseline_ms = float(row["baseline_ms"])
            production_ms = float(row["candidate_ms"])
            delta_pct = float(row["delta_pct"])
        except (KeyError, TypeError, ValueError) as error:
            raise native.AcceptanceError("Wall comparison values are invalid") from error
        native.require(
            baseline_ms > 0.0 and production_ms >= 0.0 and delta_pct <= 5.0,
            "Wall comparison values exceed the formal gate",
        )
    return {
        "phase": phase,
        "metric": metric,
        "file": path.name,
        "sha256": density.sha256(path),
        "rows": rows,
    }


def capture_order_evidence(path: Path) -> dict[str, Any]:
    value = native.read_json(path)
    native.require(
        set(value) == {"schema_version", "profile", "status", "schedule", "completed"}
        and value["schema_version"] == SCHEMA_VERSION
        and value["profile"] == PROFILE
        and value["status"] == "pass"
        and value["schedule"] == capture_schedule(),
        "Wall production capture order differs",
    )
    completed = value["completed"]
    native.require(
        isinstance(completed, list) and len(completed) == len(value["schedule"]),
        "Wall production capture completion count differs",
    )
    for expected, observed in zip(value["schedule"], completed, strict=True):
        native.require(
            isinstance(observed, dict)
            and set(observed) == {*expected, "started_at", "completed_at"}
            and all(observed[key] == expected[key] for key in expected)
            and isinstance(observed["started_at"], str)
            and isinstance(observed["completed_at"], str),
            "Wall production completed capture order differs",
        )
    return {
        "file": path.name,
        "sha256": density.sha256(path),
        "runs": len(completed),
        "pairing": "adjacent-counterbalanced",
    }


def load_interleaved_modules(repo: Path) -> tuple[Any, Any, Any, Any, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from perf_tool.arguments import build_parser, validate_arguments
    from perf_tool.execution import prepare_session, run_one
    from perf_tool.model import Case
    from perf_tool.summary import summarize_session

    return build_parser, validate_arguments, prepare_session, run_one, (Case, summarize_session)


def capture_interleaved(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "Wall source changed before interleaved capture",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Wall harness changed before interleaved capture",
    )
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed before interleaved capture",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        int(args.candidate_generation) == candidate["asset_set_generation"]
        and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
        "Wall candidate changed before interleaved capture",
    )
    binary = repo / "target/profiling/bevy_app"
    native.require(binary.is_file() and not binary.is_symlink(), "profiling binary missing")
    root = Path(args.job_root).resolve()
    native.require(root.is_dir() and not root.is_symlink(), "Wall job root is unavailable")

    build_parser, validate_arguments, prepare_session, run_one, loaded = (
        load_interleaved_modules(repo)
    )
    Case, summarize_session = loaded
    sessions: dict[tuple[str, str], tuple[Any, Path, dict[str, Any]]] = {}
    for presentation in PRESENTATIONS:
        for phase in density.PHASES:
            command = presentation_command(
                repo, root / presentation, presentation, phase, args.adapter
            )
            perf_args = build_parser().parse_args(command[2:])
            cases = {
                size: Case(
                    "wall-density",
                    size,
                    "gpu",
                    density.SEED,
                    0,
                    0,
                    wall_phase=phase,
                )
                for size in density.SIZES
            }
            with selected_presentation_environment(presentation, candidate):
                validate_arguments(perf_args)
                session = prepare_session(
                    perf_args,
                    binary,
                    list(cases.values()),
                    args.source_fingerprint,
                )
            sessions[(presentation, phase)] = (perf_args, session, cases)

    order_path = root / "capture-order.json"
    order = {
        "schema_version": SCHEMA_VERSION,
        "profile": PROFILE,
        "status": "running",
        "schedule": capture_schedule(),
        "completed": [],
    }
    native.atomic_write_json(order_path, order)
    for scheduled in order["schedule"]:
        presentation = scheduled["presentation"]
        phase = scheduled["phase"]
        perf_args, session, cases = sessions[(presentation, phase)]
        native.require(
            native.source_fingerprint(repo) == args.source_fingerprint,
            "source changed before Wall capture",
        )
        native.require(
            density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
            "asset view changed before Wall capture",
        )
        completed = {**scheduled, "started_at": native.utc_now()}
        with selected_presentation_environment(presentation, candidate):
            validation = run_one(
                args=perf_args,
                binary=binary,
                session_dir=session,
                case=cases[scheduled["size"]],
                run_number=scheduled["run"],
                preflight=False,
            )
        native.require(
            validation.valid,
            "Wall interleaved capture failed raw validation: "
            + "; ".join(validation.reasons),
        )
        completed["completed_at"] = native.utc_now()
        order["completed"].append(completed)
        native.atomic_write_json(order_path, order)

    for _, session, _ in sessions.values():
        native.require(summarize_session(session), "Wall performance session is invalid")
    order["status"] = "pass"
    native.atomic_write_json(order_path, order)
    native.print_json(capture_order_evidence(order_path))
    return 0


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "Wall production performance manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    density.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"]
        and native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"]
        and density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"],
        "Wall production performance provenance changed",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        candidate == manifest.get("candidate_identity"),
        "Wall production performance candidate changed",
    )
    binary = repo / "target/profiling/bevy_app"
    native.require(
        density.sha256(binary) == manifest["binary_sha256"],
        "Wall production performance binary changed",
    )
    sessions = manifest.get("sessions")
    native.require(
        isinstance(sessions, dict) and set(sessions) == set(PRESENTATIONS),
        "Wall production performance session modes differ",
    )
    for presentation in PRESENTATIONS:
        entries = sessions[presentation]
        native.require(
            isinstance(entries, list) and len(entries) == len(density.PHASES),
            "Wall production performance phase set differs",
        )
        for entry, phase in zip(entries, density.PHASES, strict=True):
            session = root / presentation / "sessions" / phase
            recalculated = density.verify_session(
                repo=repo,
                session=session,
                phase=phase,
                adapter=manifest["adapter"],
                subject_commit=manifest["subject_commit"],
                source_fingerprint=manifest["source_fingerprint"],
                binary_sha256=manifest["binary_sha256"],
            )
            recalculated["presentation"] = presentation
            recalculated["presentation_sidecars"] = len(
                verify_session_presentations(
                    session,
                    repo=repo,
                    presentation=presentation,
                    phase=phase,
                    candidate=candidate,
                )
            )
            native.require(entry == recalculated, "Wall presentation session differs")
    native.require(
        manifest.get("capture_order")
        == capture_order_evidence(root / "capture-order.json"),
        "Wall production capture-order evidence changed",
    )
    recorded_comparisons = manifest.get("comparisons")
    native.require(
        isinstance(recorded_comparisons, list)
        and len(recorded_comparisons) == len(density.PHASES) * 2,
        "Wall comparison manifest differs",
    )
    recalculated_comparisons = []
    for phase in density.PHASES:
        for metric in ("p95", "p99"):
            path = root / "comparisons" / f"fallback-control-vs-production-{phase}-{metric}.csv"
            recalculated_comparisons.append(
                comparison_evidence(path, phase=phase, metric=metric)
            )
    native.require(
        recorded_comparisons == recalculated_comparisons,
        "Wall comparison evidence changed",
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "capture_runs": len(PRESENTATIONS)
        * len(density.PHASES)
        * len(density.SIZES)
        * density.REPEAT,
        "comparisons": len(density.PHASES) * 2,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(
        f"Wall performance is missing runtime asset {relative}"
        for relative in density.missing_runtime_assets(repo)
    )
    candidate: dict[str, Any] | None = None
    try:
        candidate = art.candidate_identity(repo)
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
    root = (
        Path(args.job_root).resolve()
        if args.job_root
        else native.unique_job_root(repo, "wall-production-performance")
    )
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty",
        "--directory",
        str(repo),
        "--detach",
        "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
        "PYTHONDONTWRITEBYTECODE=1",
        "python3",
        str(Path(__file__).resolve()),
        "run",
        "--repo",
        str(repo),
        "--job-root",
        str(root),
        "--subject-commit",
        subject,
        "--source-fingerprint",
        source,
        "--harness-fingerprint",
        harness,
        "--asset-view-fingerprint",
        assets,
        "--candidate-generation",
        str(candidate["asset_set_generation"] if candidate else 0),
        "--candidate-manifest-sha256",
        candidate["manifest_sha256"] if candidate else "invalid",
        "--adapter",
        args.adapter,
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
            "status_command": [
                "python3",
                str(Path(__file__).resolve()),
                "status",
                "--job-root",
                str(root),
            ],
            "execution_contract": {
                "actual_window_required": True,
                "parallel_game_processes": 1,
                "capture_runs": 24,
                "comparisons": 4,
                "pairing": "adjacent-counterbalanced",
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Wall production performance must use the planned kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "Wall source changed",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Wall harness changed",
    )
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        int(args.candidate_generation) == candidate["asset_set_generation"]
        and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
        "Planned Wall candidate identity changed",
    )
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": PROFILE,
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "asset_view_fingerprint": args.asset_view_fingerprint,
        "candidate_identity": candidate,
        "adapter": args.adapter,
        "started_at": native.utc_now(),
        "current_stage": "build",
        "child_pid": None,
        "commands": [],
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        native.run_command(
            "build",
            density.build_command(),
            repo=repo,
            env=os.environ.copy(),
            log_path=root / "build.log",
            job_file=root / "job.json",
            state=state,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(binary.is_file() and not binary.is_symlink(), "profiling binary missing")
        binary_hash = density.sha256(binary)
        command = interleaved_command(
            repo=repo,
            root=root,
            subject_commit=args.subject_commit,
            source_fingerprint=args.source_fingerprint,
            harness_fingerprint=args.harness_fingerprint,
            asset_view_fingerprint=args.asset_view_fingerprint,
            candidate=candidate,
            adapter=args.adapter,
        )
        state["commands"].append({"stage": "capture-interleaved", "argv": command})
        native.run_command(
            "capture-interleaved",
            command,
            repo=repo,
            env=os.environ.copy(),
            log_path=root / "capture-interleaved.log",
            job_file=root / "job.json",
            state=state,
            timeout_seconds=density.RUN_TIMEOUT_SECONDS * len(capture_schedule()) + 600.0,
        )
        sessions: dict[str, list[dict[str, Any]]] = {}
        for presentation in PRESENTATIONS:
            entries = []
            for phase in density.PHASES:
                session = root / presentation / "sessions" / phase
                entry = density.verify_session(
                    repo=repo,
                    session=session,
                    phase=phase,
                    adapter=args.adapter,
                    subject_commit=args.subject_commit,
                    source_fingerprint=args.source_fingerprint,
                    binary_sha256=binary_hash,
                )
                entry["presentation"] = presentation
                entry["presentation_sidecars"] = len(
                    verify_session_presentations(
                        session,
                        repo=repo,
                        presentation=presentation,
                        phase=phase,
                        candidate=candidate,
                    )
                )
                entries.append(entry)
            sessions[presentation] = entries
        capture_order = capture_order_evidence(root / "capture-order.json")
        comparisons = root / "comparisons"
        comparisons.mkdir()
        comparison_results = []
        for phase in density.PHASES:
            for metric in ("p95", "p99"):
                output = comparisons / f"fallback-control-vs-production-{phase}-{metric}.csv"
                command = comparison_command(
                    root / "fallback-control" / "sessions" / phase,
                    root / "production" / "sessions" / phase,
                    metric,
                    output,
                )
                native.run_command(
                    f"compare-{phase}-{metric}",
                    command,
                    repo=repo,
                    env=os.environ.copy(),
                    log_path=root / f"compare-{phase}-{metric}.log",
                    job_file=root / "job.json",
                    state=state,
                )
                comparison_results.append(
                    comparison_evidence(output, phase=phase, metric=metric)
                )
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
            "binary_sha256": binary_hash,
            "sessions": sessions,
            "capture_order": capture_order,
            "comparisons": comparison_results,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        native.print_json(verify_root(root))
        return 0
    except Exception as error:
        state.update(
            {
                "status": "invalid",
                "failure": f"{type(error).__name__}: {error}",
                "completed_at": native.utc_now(),
                "child_pid": None,
            }
        )
        native.atomic_write_json(root / "job.json", state)
        raise


def status(args: argparse.Namespace) -> int:
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def self_test() -> int:
    candidate = {
        "authority": "isolated_candidate",
        "asset_set_generation": 2,
        "manifest_sha256": "a" * 64,
    }
    production = presentation_environment("production", candidate)
    fallback = presentation_environment("fallback-control", candidate)
    native.require(
        production["HW_WALL_CANDIDATE"] == "1"
        and "HW_WALL_CANDIDATE" not in fallback
        and production["HW_WALL_PERF_PRESENTATION"] == "production"
        and fallback["HW_WALL_PERF_PRESENTATION"] == "fallback-control",
        "Wall performance environments are not isolated",
    )
    command = presentation_command(
        Path("/repo"), Path("/job"), "production", "completed", "Intel"
    )
    native.require(
        command[command.index("--wall-presentation") + 1] == "production",
        "Wall presentation command is not explicit",
    )
    build_parser, validate_arguments, _, _, _ = load_interleaved_modules(Path.cwd())
    perf_args = build_parser().parse_args(command[2:])
    with selected_presentation_environment("production", candidate):
        validate_arguments(perf_args)
    schedule = capture_schedule()
    native.require(len(schedule) == 24, "Wall capture schedule length differs")
    for first, second in zip(schedule[::2], schedule[1::2], strict=True):
        native.require(
            (first["phase"], first["size"], first["run"])
            == (second["phase"], second["size"], second["run"])
            and {first["presentation"], second["presentation"]} == set(PRESENTATIONS),
            "Wall control and production captures are not adjacent pairs",
        )
    first_modes = [entry["presentation"] for entry in schedule[::2]]
    native.require(
        first_modes.count("fallback-control") == first_modes.count("production") == 6,
        "Wall capture pair order is not counterbalanced",
    )
    native.print_json(
        {"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE}
    )
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    run_parser = commands.add_parser("run")
    capture_parser = commands.add_parser("capture-interleaved")
    for command_parser in (run_parser, capture_parser):
        command_parser.add_argument("--repo", required=True)
        command_parser.add_argument("--job-root", required=True)
        command_parser.add_argument("--subject-commit", required=True)
        command_parser.add_argument("--source-fingerprint", required=True)
        command_parser.add_argument("--harness-fingerprint", required=True)
        command_parser.add_argument("--asset-view-fingerprint", required=True)
        command_parser.add_argument("--candidate-generation", required=True)
        command_parser.add_argument("--candidate-manifest-sha256", required=True)
        command_parser.add_argument("--adapter", default="Intel")
    status_parser = commands.add_parser("status")
    status_parser.add_argument("--job-root", required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    if args.command == "plan":
        return plan(args)
    if args.command in {"run", "capture-interleaved"}:
        for value, label, length in (
            (args.subject_commit, "subject commit", 40),
            (args.source_fingerprint, "source fingerprint", 64),
            (args.harness_fingerprint, "harness fingerprint", 64),
            (args.asset_view_fingerprint, "asset-view fingerprint", 64),
            (args.candidate_manifest_sha256, "candidate manifest", 64),
        ):
            native.require(
                re.fullmatch(f"[0-9a-f]{{{length}}}", value) is not None,
                f"invalid {label}",
            )
        return run(args) if args.command == "run" else capture_interleaved(args)
    if args.command == "status":
        return status(args)
    if args.command == "verify":
        native.print_json(verify_root(Path(args.job_root).resolve()))
        return 0
    if args.command == "self-test":
        return self_test()
    raise native.AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        native.print_json(
            {"schema_version": SCHEMA_VERSION, "status": "invalid", "error": str(error)}
        )
        raise SystemExit(1) from error
