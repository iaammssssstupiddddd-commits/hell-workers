#!/usr/bin/env python3
"""Run and fail-closed-verify the wall-density-v1 native Capture matrix.

This profile owns the frame-time leg only.  It records the exact current-wall
N/4N baseline for completed and provisional walls from a clean committed
subject.  Wall-specific RenderDoc draw-group evidence remains a separate M0
closure item and is deliberately reported as not collected by this profile.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import math
import os
import re
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402


SCHEMA_VERSION = 1
PROFILE = "wall-density-capture-v1"
PHASES = ("completed", "provisional")
SIZES = ("small", "medium")
SEED = 20_260_901
REPEAT = 3
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WINDOW_SCALE_FACTOR = 1.0
WARMUP_SECONDS = 30.0
MEASURE_SECONDS = 60.0
RUN_TIMEOUT_SECONDS = 780.0
CONTRACT_RELATIVE = "tools/blender_ai_workflow/fixtures/wall-density-v1.json"
REQUIRED_RUNTIME_ASSETS = (
    "assets/fonts/SourceSerif4-VF.ttf",
    "assets/textures/terrain/mud_floor.png",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def asset_view_fingerprint(repo: Path) -> str:
    root = repo / "assets"
    native.require(
        root.is_dir() and not root.is_symlink(), "runtime asset root is unavailable"
    )
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    native.require(files, "runtime asset root is empty")
    for path in files:
        native.require(
            not path.is_symlink(), f"runtime asset view contains a symlink: {path}"
        )
        relative = path.relative_to(repo).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def missing_runtime_assets(repo: Path) -> list[str]:
    return [
        relative
        for relative in REQUIRED_RUNTIME_ASSETS
        if not (repo / relative).is_file()
    ]


def assert_fully_clean(repo: Path, subject_commit: str) -> None:
    native.assert_clean_subject(repo, subject_commit)
    dirty = native.git_dirty_paths(repo)
    native.require(
        not dirty,
        "wall-density formal subject includes uncommitted harness or product paths: "
        + ", ".join(dirty[:8]),
    )


def build_command() -> list[str]:
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
        "profiling",
    ]


def phase_command(repo: Path, root: Path, phase: str, adapter: str) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "wall-density",
        "--wall-phase",
        phase,
        "--sizes",
        ",".join(SIZES),
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
        "--output",
        str(root / "sessions" / phase),
        "--adapter",
        adapter,
        "--backend",
        "vulkan",
        "--window-backend",
        "x11",
        "--present-mode",
        "novsync",
        "--window-width",
        str(WINDOW_WIDTH),
        "--window-height",
        str(WINDOW_HEIGHT),
        "--window-scale-factor",
        str(WINDOW_SCALE_FACTOR),
        "--rtt-quality",
        "high",
        "--instrumentation",
        "capture",
        "--binary",
        str(repo / "target/profiling/bevy_app"),
        "--skip-build",
        "--warmup-secs",
        str(WARMUP_SECONDS),
        "--measure-secs",
        str(MEASURE_SECONDS),
        "--timeout-secs",
        str(int(RUN_TIMEOUT_SECONDS)),
    ]


def session_digest(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    native.require(files, f"wall-density session has no files: {root}")
    for path in files:
        native.require(
            not path.is_symlink(), f"wall-density session contains a symlink: {path}"
        )
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256(path)))
    return digest.hexdigest()


def load_perf_modules(repo: Path) -> tuple[Any, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from perf_tool.artifacts import validate_run
    from perf_tool.model import Case

    return Case, validate_run


def locate_run(session: Path, case_id: str, run_number: int) -> Path:
    return session / "cases" / case_id / f"run-{run_number:03d}"


def verify_session(
    *,
    repo: Path,
    session: Path,
    phase: str,
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
) -> dict[str, Any]:
    Case, validate_run = load_perf_modules(repo)
    manifest = native.read_json(session / "manifest.json")
    matrix = native.read_json(session / "matrix.json")
    expected_matrix = {
        "workload": "wall-density",
        "sizes": list(SIZES),
        "renders": ["gpu"],
        "seed": SEED,
        "repeat": REPEAT,
        "warmup_secs": WARMUP_SECONDS,
        "measure_secs": MEASURE_SECONDS,
        "preflight_runs": 0,
        "souls": 0,
        "familiars": 0,
        "familiar_policies": ["baseline"],
        "operation_dialog_modes": ["hidden"],
        "dashboard_modes": ["hidden"],
        "behavior_cases": [],
        "wall_phase": phase,
        "capture_kind": "frame-time",
        "clock_mode": "realtime",
        "window_width": WINDOW_WIDTH,
        "window_height": WINDOW_HEIGHT,
        "window_scale_factor": WINDOW_SCALE_FACTOR,
        "rtt_quality": "high",
    }
    for field, expected in expected_matrix.items():
        native.require(
            matrix.get(field) == expected, f"{phase} matrix field {field} differs"
        )
    native.require(
        manifest.get("status") == "valid", f"{phase} performance session is not valid"
    )
    git = manifest.get("git")
    source = manifest.get("source")
    binary = manifest.get("binary")
    native.require(
        isinstance(git, dict) and git.get("commit") == subject_commit,
        f"{phase} subject differs",
    )
    native.require(
        git.get("dirty_paths") == [],
        f"{phase} performance session recorded dirty paths",
    )
    native.require(
        isinstance(source, dict)
        and source.get("fingerprint_start") == source_fingerprint
        and source.get("fingerprint_end") == source_fingerprint
        and source.get("unchanged") is True,
        f"{phase} source provenance differs",
    )
    native.require(
        isinstance(binary, dict)
        and binary.get("sha256") == binary_sha256
        and binary.get("instrumentation") == "capture",
        f"{phase} binary provenance differs",
    )

    observations: list[dict[str, Any]] = []
    for size in SIZES:
        case = Case("wall-density", size, "gpu", SEED, 0, 0, wall_phase=phase)
        for run_number in range(1, REPEAT + 1):
            run_dir = locate_run(session, case.identifier, run_number)
            metadata = native.read_json(run_dir / "run-metadata.json")
            native.require(
                metadata.get("case") == asdict(case),
                f"{case.identifier} metadata differs",
            )
            native.require(
                metadata.get("returncode") == 0, f"{case.identifier} process failed"
            )
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter=adapter,
                expected_backend="vulkan",
                allow_log_patterns=[],
                capture_kind="frame-time",
                expected_warmup_secs=WARMUP_SECONDS,
                expected_measure_secs=MEASURE_SECONDS,
                expected_window_backend="x11",
                expected_present_mode="novsync",
                expected_window_width=WINDOW_WIDTH,
                expected_window_height=WINDOW_HEIGHT,
                expected_window_scale_factor=WINDOW_SCALE_FACTOR,
                expected_rtt_quality="high",
            )
            calculated = validation.to_json()
            native.require(
                native.read_json(run_dir / "validation.json") == calculated,
                f"{case.identifier} stored validation differs from raw artifacts",
            )
            native.require(
                validation.valid,
                f"{case.identifier} raw validation failed: {'; '.join(validation.reasons)}",
            )
            native.require(
                validation.wall_density_fixture is not None
                and validation.wall_density_layout is not None,
                f"{case.identifier} lacks wall-density sidecars",
            )
            observations.append(
                {
                    "case_id": case.identifier,
                    "run": run_number,
                    "fixture": validation.wall_density_fixture,
                }
            )

    aggregate_path = session / "aggregate.csv"
    with aggregate_path.open(newline="", encoding="utf-8") as handle:
        aggregate_rows = list(csv.DictReader(handle))
    native.require(
        len(aggregate_rows) == len(SIZES), f"{phase} aggregate case count differs"
    )
    for row in aggregate_rows:
        native.require(
            row.get("valid_runs") == str(REPEAT),
            f"{phase} aggregate lacks three valid runs",
        )
        for field in ("p95_median_ms", "p95_mad_ms", "p99_median_ms", "p99_mad_ms"):
            try:
                value = float(row[field])
            except (KeyError, TypeError, ValueError) as error:
                raise native.AcceptanceError(
                    f"{phase} aggregate {field} is invalid"
                ) from error
            native.require(
                math.isfinite(value) and value >= 0.0,
                f"{phase} aggregate {field} is invalid",
            )
    return {
        "phase": phase,
        "session": str(session),
        "sha256": session_digest(session),
        "runs": len(observations),
        "aggregate": aggregate_rows,
    }


def verify_root(
    root: Path,
    *,
    subject_commit: str | None = None,
    source_fingerprint: str | None = None,
    harness_fingerprint: str | None = None,
) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION,
        "wall-density manifest schema differs",
    )
    native.require(
        manifest.get("status") == "pass", "wall-density manifest is not passing"
    )
    native.require(manifest.get("profile") == PROFILE, "wall-density profile differs")
    repo_value = manifest.get("repo")
    native.require(isinstance(repo_value, str), "wall-density manifest repo is invalid")
    repo = native.validate_repo(repo_value)
    for observed, expected, label in (
        (manifest.get("subject_commit"), subject_commit, "subject commit"),
        (manifest.get("source_fingerprint"), source_fingerprint, "source fingerprint"),
        (
            manifest.get("harness_fingerprint"),
            harness_fingerprint,
            "harness fingerprint",
        ),
    ):
        if expected is not None:
            native.require(observed == expected, f"wall-density {label} differs")
    coverage = manifest.get("coverage")
    native.require(
        coverage == {"frame_time": "pass", "draw_groups": "not-collected"},
        "wall-density coverage declaration differs",
    )
    sessions = manifest.get("sessions")
    native.require(
        isinstance(sessions, list) and len(sessions) == len(PHASES),
        "wall-density sessions differ",
    )
    recorded_subject = manifest.get("subject_commit")
    recorded_source = manifest.get("source_fingerprint")
    recorded_harness = manifest.get("harness_fingerprint")
    recorded_assets = manifest.get("asset_view_fingerprint")
    recorded_adapter = manifest.get("adapter")
    recorded_binary = manifest.get("binary_sha256")
    native.require(
        all(
            isinstance(value, str)
            for value in (
                recorded_subject,
                recorded_source,
                recorded_harness,
                recorded_assets,
                recorded_adapter,
                recorded_binary,
            )
        ),
        "wall-density manifest provenance is invalid",
    )
    assert_fully_clean(repo, recorded_subject)
    native.require(
        native.source_fingerprint(repo) == recorded_source,
        "wall-density source no longer matches",
    )
    native.require(
        native.native_harness_fingerprint(repo) == recorded_harness,
        "wall-density harness no longer matches",
    )
    native.require(
        asset_view_fingerprint(repo) == recorded_assets,
        "wall-density asset view no longer matches",
    )
    for entry, phase in zip(sessions, PHASES, strict=True):
        native.require(entry.get("phase") == phase, "wall-density phase order differs")
        session = root / "sessions" / phase
        recalculated = verify_session(
            repo=repo,
            session=session,
            phase=phase,
            adapter=recorded_adapter,
            subject_commit=recorded_subject,
            source_fingerprint=recorded_source,
            binary_sha256=recorded_binary,
        )
        native.require(entry == recalculated, f"{phase} session evidence differs")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "capture_runs": len(PHASES) * len(SIZES) * REPEAT,
        "draw_groups": "not-collected",
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(
        f"wall-density actual-window subject is missing runtime asset {relative}"
        for relative in missing_runtime_assets(repo)
    )
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    try:
        assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    asset_fingerprint = asset_view_fingerprint(repo)
    root = (
        Path(args.job_root).resolve()
        if args.job_root
        else native.unique_job_root(repo, "wall-density")
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
        asset_fingerprint,
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
            "asset_view_fingerprint": asset_fingerprint,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "execution_contract": {
                "actual_window_required": True,
                "parallel_game_processes": 1,
                "capture_runs": len(PHASES) * len(SIZES) * REPEAT,
                "draw_groups": "not-collected",
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "wall-density run must be launched by the planned direct kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(
        native.git_subject(repo) == args.subject_commit, "wall-density subject changed"
    )
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "wall-density source changed",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "wall-density harness changed",
    )
    assert_fully_clean(repo, args.subject_commit)
    native.require(
        not missing_runtime_assets(repo), "wall-density runtime assets are missing"
    )
    native.require(
        asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "wall-density asset view changed",
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
        "adapter": args.adapter,
        "started_at": native.utc_now(),
        "current_stage": None,
        "child_pid": None,
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        environment = os.environ.copy()
        native.run_command(
            "build",
            build_command(),
            repo=repo,
            env=environment,
            log_path=root / "build.log",
            job_file=root / "job.json",
            state=state,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(
            binary.is_file() and not binary.is_symlink(), "profiling binary is missing"
        )
        binary_hash = sha256(binary)
        sessions = []
        for phase in PHASES:
            native.require(
                native.source_fingerprint(repo) == args.source_fingerprint,
                "source changed before phase",
            )
            native.require(
                asset_view_fingerprint(repo) == args.asset_view_fingerprint,
                "asset view changed before phase",
            )
            native.run_command(
                f"capture-{phase}",
                phase_command(repo, root, phase, args.adapter),
                repo=repo,
                env=environment,
                log_path=root / f"capture-{phase}.log",
                job_file=root / "job.json",
                state=state,
                timeout_seconds=RUN_TIMEOUT_SECONDS,
            )
            sessions.append(
                verify_session(
                    repo=repo,
                    session=root / "sessions" / phase,
                    phase=phase,
                    adapter=args.adapter,
                    subject_commit=args.subject_commit,
                    source_fingerprint=args.source_fingerprint,
                    binary_sha256=binary_hash,
                )
            )
        native.require(
            native.source_fingerprint(repo) == args.source_fingerprint,
            "source changed after captures",
        )
        native.require(
            asset_view_fingerprint(repo) == args.asset_view_fingerprint,
            "asset view changed after captures",
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
            "adapter": args.adapter,
            "binary_sha256": binary_hash,
            "coverage": {"frame_time": "pass", "draw_groups": "not-collected"},
            "sessions": sessions,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update(
            {"status": "valid", "completed_at": native.utc_now(), "child_pid": None}
        )
        native.atomic_write_json(root / "job.json", state)
        native.print_json(
            verify_root(
                root,
                subject_commit=args.subject_commit,
                source_fingerprint=args.source_fingerprint,
                harness_fingerprint=args.harness_fingerprint,
            )
        )
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
    root = Path(args.job_root).resolve()
    job = native.read_json(root / "job.json")
    native.print_json(job)
    return (
        2
        if job.get("status") == "running"
        else 0
        if job.get("status") == "valid"
        else 1
    )


def self_test() -> int:
    native.require(
        len(PHASES) * len(SIZES) * REPEAT == 12, "wall-density matrix must have 12 runs"
    )
    native.require(
        "--perf-wall-phase"
        not in phase_command(Path("/repo"), Path("/job"), "completed", "Intel"),
        "wall-density launcher unexpectedly uses the Rust-only flag name",
    )
    command = phase_command(Path("/repo"), Path("/job"), "completed", "Intel")
    native.require(
        command[command.index("--wall-phase") + 1] == "completed",
        "wall phase is not explicit",
    )
    native.require(
        command[command.index("--souls") + 1] == "0", "wall-density actor count differs"
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
    run_parser.add_argument("--repo", required=True)
    run_parser.add_argument("--job-root", required=True)
    run_parser.add_argument("--subject-commit", required=True)
    run_parser.add_argument("--source-fingerprint", required=True)
    run_parser.add_argument("--harness-fingerprint", required=True)
    run_parser.add_argument("--asset-view-fingerprint", required=True)
    run_parser.add_argument("--adapter", default="Intel")
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
    if args.command == "run":
        for value, label, length in (
            (args.subject_commit, "subject commit", 40),
            (args.source_fingerprint, "source fingerprint", 64),
            (args.harness_fingerprint, "harness fingerprint", 64),
            (args.asset_view_fingerprint, "asset-view fingerprint", 64),
        ):
            native.require(
                re.fullmatch(f"[0-9a-f]{{{length}}}", value) is not None,
                f"invalid {label}",
            )
        return run(args)
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
