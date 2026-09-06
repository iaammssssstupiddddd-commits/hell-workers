#!/usr/bin/env python3
"""Run the door-art-v1 isolated-candidate behavior and load acceptance leg."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 1
PROFILE = "door-art-v1-behavior"
SEED = 20_260_803
BEHAVIOR_CASES = (
    "door-state-v1",
    "load-normal-v1",
    "load-preflight-reject-v1",
    "load-rollback-v1",
    "load-recovery-only-v1",
    "load-recovery-failed-v1",
    "load-duplicate-reset-v1",
)
FIXED_HZ = 64
WARMUP_TICKS = 1_920
AUDIT_TICKS = 128
RUN_TIMEOUT_SECONDS = 780.0
ALLOW_LOG_PATTERNS = (
    "driver that only supports software rendering",
    "P05 behavior injected",
    "incoming candidate was rejected before live replacement: candidate validator 'lighting\\.fixture':",
)
DOORSET_RELATIVE = "assets/manifests/door-production-v1.doorset"
CANDIDATE_ENV_KEYS = (
    "HW_DOOR_ART_PREVIEW",
    "HW_DOOR_CANDIDATE",
    "HW_DOOR_CANDIDATE_GENERATION",
    "HW_DOOR_CANDIDATE_MANIFEST_SHA256",
    "HW_DOOR_BEHAVIOR_PRESENTATION",
    "HW_DOOR_BEHAVIOR_STATUS_ROOT",
    "HW_DOOR_BEHAVIOR_NONCE",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def session_digest(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    native.require(files, f"Door behavior artifact tree is empty: {root}")
    for path in files:
        native.require(
            not path.is_symlink(), f"Door behavior artifact is a symlink: {path}"
        )
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256(path)))
    return digest.hexdigest()


def assert_fully_clean(repo: Path, subject_commit: str) -> None:
    native.assert_clean_subject(repo, subject_commit)
    dirty = native.git_dirty_paths(repo)
    native.require(
        not dirty,
        "Door behavior subject includes uncommitted paths: " + ", ".join(dirty[:8]),
    )


def candidate_identity(repo: Path) -> dict[str, Any]:
    path = repo / DOORSET_RELATIVE
    native.require(path.is_file() and not path.is_symlink(), "Door locator is absent")
    payload = native.read_json(path)
    native.require(payload.get("schema_version") == 1, "Door locator schema differs")
    native.require(
        payload.get("asset_set_id") == "door-production-v1",
        "Door locator identity differs",
    )
    native.require(
        payload.get("authority") == "isolated_candidate",
        "Door locator is not an isolated candidate",
    )
    native.require(
        payload.get("review_status") == "art_approved",
        "Door candidate is not art-approved",
    )
    generation = payload.get("asset_set_generation")
    manifest_sha256 = payload.get("manifest_sha256")
    native.require(
        isinstance(generation, int) and generation > 0,
        "Door candidate generation is invalid",
    )
    native.require(
        isinstance(manifest_sha256, str)
        and re.fullmatch(r"[0-9a-f]{64}", manifest_sha256) is not None,
        "Door candidate manifest hash is invalid",
    )
    core = payload.get("core")
    native.require(isinstance(core, list) and len(core) == 6, "Door core differs")
    expected_roles = [
        "mesh:closed",
        "mesh:open",
        "mesh:locked",
        "texture:albedo",
        "preview:ew",
        "preview:ns",
    ]
    native.require(
        [record.get("role") for record in core if isinstance(record, dict)]
        == expected_roles,
        "Door core role order differs",
    )
    for record in core:
        native.require(isinstance(record, dict), "Door core record is invalid")
        relative = record.get("path")
        native.require(
            isinstance(relative, str)
            and not Path(relative).is_absolute()
            and ".." not in Path(relative).parts,
            "Door core path is invalid",
        )
        asset = repo / "assets" / relative
        native.require(asset.is_file() and not asset.is_symlink(), f"Door asset is absent: {relative}")
        native.require(asset.stat().st_size == record.get("bytes"), f"Door asset size differs: {relative}")
        native.require(sha256(asset) == record.get("sha256"), f"Door asset hash differs: {relative}")
    return {
        "asset_set_generation": generation,
        "manifest_sha256": manifest_sha256,
        "locator_sha256": sha256(path),
    }


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


def behavior_command(repo: Path, root: Path, adapter: str) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "behavior",
        "--workload",
        "indoor-light",
        "--contract",
        "rtt-light-v1",
        "--stage",
        "p08",
        "--lane",
        "behavior",
        "--sizes",
        "small",
        "--renders",
        "cpu",
        "--seed",
        str(SEED),
        "--repeat",
        "3",
        "--preflight-runs",
        "0",
        "--behavior-cases",
        ",".join(BEHAVIOR_CASES),
        "--output",
        str(root / "session"),
        "--adapter",
        adapter,
        "--backend",
        "vulkan",
        "--window-backend",
        "headless",
        "--present-mode",
        "novsync",
        "--rtt-quality",
        "high",
        "--instrumentation",
        "capture",
        "--binary",
        str(repo / "target/profiling/bevy_app"),
        "--skip-build",
        "--timeout-secs",
        str(int(RUN_TIMEOUT_SECONDS)),
    ]


def load_perf_modules(repo: Path) -> tuple[Any, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from perf_tool.artifacts import validate_run
    from perf_tool.model import Case

    return Case, validate_run


def verify_session(
    *,
    repo: Path,
    root: Path,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
    adapter: str,
    nonce: str,
    candidate: dict[str, Any],
) -> dict[str, Any]:
    Case, validate_run = load_perf_modules(repo)
    session = root / "session"
    manifest = native.read_json(session / "manifest.json")
    matrix = native.read_json(session / "matrix.json")
    expected_matrix = {
        "workload": "indoor-light",
        "sizes": ["small"],
        "renders": ["cpu"],
        "seed": SEED,
        "repeat": 3,
        "preflight_runs": 0,
        "behavior_cases": list(BEHAVIOR_CASES),
        "allow_log_patterns": list(ALLOW_LOG_PATTERNS),
        "capture_kind": "fixed-step-behavior",
        "clock_mode": "fixed-behavior",
        "fixed_hz": FIXED_HZ,
        "warmup_ticks": WARMUP_TICKS,
        "audit_ticks": AUDIT_TICKS,
        "window_width": None,
        "window_height": None,
        "window_scale_factor": None,
        "rtt_quality": "high",
    }
    for field, expected in expected_matrix.items():
        native.require(matrix.get(field) == expected, f"Door behavior matrix {field} differs")
    native.require(manifest.get("status") == "valid", "Door behavior session is invalid")
    git = manifest.get("git")
    source = manifest.get("source")
    binary = manifest.get("binary")
    native.require(
        isinstance(git, dict)
        and git.get("commit") == subject_commit
        and git.get("dirty_paths") == [],
        "Door behavior subject provenance differs",
    )
    native.require(
        isinstance(source, dict)
        and source.get("fingerprint_start") == source_fingerprint
        and source.get("fingerprint_end") == source_fingerprint
        and source.get("unchanged") is True,
        "Door behavior source provenance differs",
    )
    native.require(
        isinstance(binary, dict)
        and binary.get("sha256") == binary_sha256
        and binary.get("instrumentation") == "capture",
        "Door behavior binary provenance differs",
    )
    statuses: list[dict[str, Any]] = []
    status_root = root / "door-status"
    status_paths = sorted(path for path in status_root.iterdir() if path.is_file())
    native.require(
        len(status_paths) == len(BEHAVIOR_CASES) * 3,
        "Door behavior status inventory differs",
    )
    for behavior_case in BEHAVIOR_CASES:
        case = Case(
            "indoor-light",
            "small",
            "cpu",
            SEED,
            None,
            None,
            behavior_case=behavior_case,
        )
        case_status_paths = [
            path for path in status_paths if path.name.startswith(f"{behavior_case}-")
        ]
        native.require(len(case_status_paths) == 3, f"{behavior_case} status count differs")
        for run_number, status_path in zip(range(1, 4), case_status_paths, strict=True):
            run_dir = session / "cases" / case.identifier / f"run-{run_number:03d}"
            metadata = native.read_json(run_dir / "run-metadata.json")
            native.require(metadata.get("case") == asdict(case), f"{behavior_case} metadata differs")
            native.require(metadata.get("returncode") == 0, f"{behavior_case} process failed")
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter=adapter,
                expected_backend="vulkan",
                allow_log_patterns=ALLOW_LOG_PATTERNS,
                capture_kind="fixed-step-behavior",
                expected_fixed_hz=FIXED_HZ,
                expected_warmup_ticks=WARMUP_TICKS,
                expected_audit_ticks=AUDIT_TICKS,
                expected_window_backend="headless",
                expected_present_mode="novsync",
                expected_rtt_quality="high",
                expected_contract="rtt-light-v1",
                expected_stage="p08",
                expected_lane="behavior",
            )
            native.require(
                native.read_json(run_dir / "validation.json") == validation.to_json(),
                f"{behavior_case} stored validation differs",
            )
            native.require(
                validation.valid,
                f"{behavior_case} raw validation failed: {'; '.join(validation.reasons)}",
            )
            status = native.read_json(status_path)
            native.require(
                status.get("schema") == "door-art-v1-behavior"
                and status.get("schema_version") == 1
                and status.get("status") == "valid"
                and status.get("session_nonce") == nonce
                and status.get("case_id") == behavior_case,
                f"{behavior_case} Door status identity differs",
            )
            expected_identity = {
                "asset_set_generation": candidate["asset_set_generation"],
                "authority": "isolated_candidate",
                "manifest_sha256": candidate["manifest_sha256"],
            }
            native.require(
                status.get("candidate_identity") == expected_identity,
                f"{behavior_case} candidate identity differs",
            )
            final = status.get("final")
            recovery_failed = behavior_case == "load-recovery-failed-v1"
            if recovery_failed:
                expected_final_presentation = ("absent_fail_dark", 0, 0, None)
            elif behavior_case == "door-state-v1":
                expected_final_presentation = ("production", 1, 0, "mesh:locked")
            else:
                expected_final_presentation = ("production", 1, 0, None)
            native.require(
                isinstance(final, dict)
                and final.get("presentation_mode") == expected_final_presentation[0]
                and final.get("production_visual_count")
                == expected_final_presentation[1]
                and final.get("fallback_visual_count")
                == expected_final_presentation[2]
                and (
                    final.get("mesh_role") == expected_final_presentation[3]
                    if expected_final_presentation[3] is not None or recovery_failed
                    else final.get("mesh_role") in {"mesh:closed", "mesh:open"}
                )
                and final.get("resident_production_meshes") == 3
                and final.get("resident_production_materials") == 1,
                f"{behavior_case} final candidate presentation differs",
            )
            if behavior_case == "door-state-v1":
                native.require(
                    status.get("production_validation_count") == 5
                    and status.get("validated_mesh_roles")
                    == ["mesh:closed", "mesh:locked", "mesh:open"]
                    and status.get("semantic_sequence")
                    == ["closed", "open", "open", "locked", "locked"],
                    "Door state producer did not cover all production states",
                )
            else:
                expected_load_roles = (
                    ["mesh:closed"]
                    if recovery_failed
                    else [final.get("mesh_role")]
                )
                native.require(
                    status.get("production_validation_count") == 1
                    and status.get("validated_mesh_roles") == expected_load_roles
                    and status.get("semantic_sequence") == [],
                    (
                        f"{behavior_case} candidate baseline evidence differs"
                        if recovery_failed
                        else f"{behavior_case} candidate rebind evidence differs"
                    ),
                )
            statuses.append(
                {
                    "case_id": behavior_case,
                    "run": run_number,
                    "status_sha256": sha256(status_path),
                    "timeline_rows": len(validation.timeline or []),
                    "world_epoch": final.get("world_epoch"),
                }
            )
    return {
        "session": str(session),
        "session_sha256": session_digest(session),
        "status_root_sha256": session_digest(status_root),
        "cases": statuses,
    }


def verify_root(
    root: Path,
    *,
    subject_commit: str | None = None,
    source_fingerprint: str | None = None,
    harness_fingerprint: str | None = None,
) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest.get("schema_version") == SCHEMA_VERSION, "Door manifest schema differs")
    native.require(manifest.get("status") == "pass", "Door manifest is not passing")
    native.require(manifest.get("profile") == PROFILE, "Door profile differs")
    repo = native.validate_repo(manifest.get("repo", ""))
    for observed, expected, label in (
        (manifest.get("subject_commit"), subject_commit, "subject commit"),
        (manifest.get("source_fingerprint"), source_fingerprint, "source fingerprint"),
        (manifest.get("harness_fingerprint"), harness_fingerprint, "harness fingerprint"),
    ):
        if expected is not None:
            native.require(observed == expected, f"Door {label} differs")
    recorded_subject = manifest.get("subject_commit")
    recorded_source = manifest.get("source_fingerprint")
    recorded_harness = manifest.get("harness_fingerprint")
    recorded_assets = manifest.get("asset_view_fingerprint")
    recorded_binary = manifest.get("binary_sha256")
    recorded_adapter = manifest.get("adapter")
    recorded_nonce = manifest.get("session_nonce")
    candidate = manifest.get("candidate_identity")
    native.require(
        all(
            isinstance(value, str)
            for value in (
                recorded_subject,
                recorded_source,
                recorded_harness,
                recorded_assets,
                recorded_binary,
                recorded_adapter,
                recorded_nonce,
            )
        )
        and isinstance(candidate, dict),
        "Door manifest provenance is invalid",
    )
    assert_fully_clean(repo, recorded_subject)
    native.require(native.source_fingerprint(repo) == recorded_source, "Door source changed")
    native.require(native.native_harness_fingerprint(repo) == recorded_harness, "Door harness changed")
    native.require(density.asset_view_fingerprint(repo) == recorded_assets, "Door asset view changed")
    native.require(candidate_identity(repo) == candidate, "Door candidate locator changed")
    recalculated = verify_session(
        repo=repo,
        root=root,
        subject_commit=recorded_subject,
        source_fingerprint=recorded_source,
        binary_sha256=recorded_binary,
        adapter=recorded_adapter,
        nonce=recorded_nonce,
        candidate=candidate,
    )
    native.require(manifest.get("behavior") == recalculated, "Door behavior evidence differs")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "behavior_cases": len(BEHAVIOR_CASES),
        "production_states": 3,
        "fallback_visuals": 0,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    try:
        assert_fully_clean(repo, subject)
        candidate = candidate_identity(repo)
        asset_fingerprint = density.asset_view_fingerprint(repo)
    except native.AcceptanceError as error:
        failures.append(str(error))
        candidate = None
        asset_fingerprint = None
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "door-behavior")
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
        asset_fingerprint or "0" * 64,
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
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "execution_contract": {
                "actual_window_required": False,
                "parallel_game_processes": 1,
                "behavior_cases": len(BEHAVIOR_CASES),
                "presentation": "isolated-candidate",
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Door behavior run must use the planned direct kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Door subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Door source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "Door harness changed")
    assert_fully_clean(repo, args.subject_commit)
    native.require(density.asset_view_fingerprint(repo) == args.asset_view_fingerprint, "Door asset view changed")
    candidate = candidate_identity(repo)
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    status_root = root / "door-status"
    status_root.mkdir()
    nonce = secrets.token_hex(16)
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
        for key in CANDIDATE_ENV_KEYS:
            environment.pop(key, None)
        environment.update(
            {
                "HW_DOOR_CANDIDATE": "1",
                "HW_DOOR_CANDIDATE_GENERATION": str(candidate["asset_set_generation"]),
                "HW_DOOR_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
                "HW_DOOR_BEHAVIOR_PRESENTATION": "isolated-candidate",
                "HW_DOOR_BEHAVIOR_STATUS_ROOT": str(status_root),
                "HW_DOOR_BEHAVIOR_NONCE": nonce,
            }
        )
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
        native.require(binary.is_file() and not binary.is_symlink(), "Door profiling binary is absent")
        binary_hash = sha256(binary)
        native.run_command(
            "behavior",
            behavior_command(repo, root, args.adapter),
            repo=repo,
            env=environment,
            log_path=root / "behavior.log",
            job_file=root / "job.json",
            state=state,
            timeout_seconds=RUN_TIMEOUT_SECONDS,
        )
        behavior = verify_session(
            repo=repo,
            root=root,
            subject_commit=args.subject_commit,
            source_fingerprint=args.source_fingerprint,
            binary_sha256=binary_hash,
            adapter=args.adapter,
            nonce=nonce,
            candidate=candidate,
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
            "session_nonce": nonce,
            "candidate_identity": candidate,
            "behavior": behavior,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
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
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def self_test() -> int:
    command = behavior_command(Path("/repo"), Path("/job"), "Intel")
    native.require(len(BEHAVIOR_CASES) == 7, "Door behavior case inventory differs")
    native.require(
        command[command.index("--behavior-cases") + 1] == ",".join(BEHAVIOR_CASES),
        "Door behavior command case order differs",
    )
    native.require(
        command[command.index("--window-backend") + 1] == "headless",
        "Door behavior command is not deterministic headless",
    )
    native.require(
        command[command.index("--repeat") + 1] == "3",
        "Door behavior command does not preserve the frozen P08 repeat count",
    )
    native.print_json({"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE})
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
        native.print_json({"schema_version": SCHEMA_VERSION, "status": "invalid", "error": str(error)})
        raise SystemExit(1) from error
