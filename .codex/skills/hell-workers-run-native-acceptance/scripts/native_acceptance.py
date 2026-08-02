#!/usr/bin/env python3
"""Plan, run, monitor, and verify no-prompt native acceptance sessions."""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any


GIB = 1024**3
SCHEMA_VERSION = 1
RUNNING_EXIT_CODE = 2
MIN_MEMORY_GIB = 8
TWO_JOB_MEMORY_GIB = 12
MIN_WORKSPACE_FREE_GIB = 15
MIN_TMP_FREE_GIB = 1
DEFAULT_SEED = 20260802
LOCK_PATH = Path("/tmp/hell-workers-native-acceptance.lock")
DASHBOARD_MODES = {"hidden", "visible", "active-filter"}
SOURCE_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "scripts/perf.py",
}
SOURCE_PREFIXES = ("crates/", "scripts/perf_tool/")
ASSET_PREFIX = "assets/"


class AcceptanceError(RuntimeError):
    """A fail-closed acceptance error."""


def utc_now() -> str:
    return datetime.now(UTC).isoformat()


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise AcceptanceError(f"missing JSON artifact: {path}") from error
    except json.JSONDecodeError as error:
        raise AcceptanceError(f"invalid JSON artifact: {path}: {error}") from error
    if not isinstance(value, dict):
        raise AcceptanceError(f"JSON artifact is not an object: {path}")
    return value


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def print_json(value: dict[str, Any]) -> None:
    print(json.dumps(value, indent=2, sort_keys=True))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def mem_available_bytes() -> int:
    for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
        if line.startswith("MemAvailable:"):
            return int(line.split()[1]) * 1024
    raise AcceptanceError("/proc/meminfo does not expose MemAvailable")


def free_bytes(path: Path) -> int:
    stats = os.statvfs(path)
    return stats.f_bavail * stats.f_frsize


def gib(value: int) -> float:
    return round(value / GIB, 2)


def resource_snapshot(repo: Path, *, require_launcher: bool) -> dict[str, Any]:
    memory = mem_available_bytes()
    workspace_free = free_bytes(repo)
    tmp_free = free_bytes(Path("/tmp"))
    launcher = shutil.which("kitty")
    failures: list[str] = []
    if memory < MIN_MEMORY_GIB * GIB:
        failures.append(
            f"MemAvailable {gib(memory)} GiB is below {MIN_MEMORY_GIB} GiB"
        )
    if workspace_free < MIN_WORKSPACE_FREE_GIB * GIB:
        failures.append(
            f"workspace free {gib(workspace_free)} GiB is below "
            f"{MIN_WORKSPACE_FREE_GIB} GiB"
        )
    if tmp_free < MIN_TMP_FREE_GIB * GIB:
        failures.append(
            f"/tmp free {gib(tmp_free)} GiB is below {MIN_TMP_FREE_GIB} GiB"
        )
    if require_launcher and launcher is None:
        failures.append("kitty launcher is unavailable")
    return {
        "status": "ready" if not failures else "blocked",
        "failures": failures,
        "mem_available_gib": gib(memory),
        "workspace_free_gib": gib(workspace_free),
        "tmp_free_gib": gib(tmp_free),
        "cargo_jobs": 2 if memory >= TWO_JOB_MEMORY_GIB * GIB else 1,
        "cargo_incremental": 0,
        "launcher": launcher,
        "thresholds_gib": {
            "mem_available": MIN_MEMORY_GIB,
            "workspace_free": MIN_WORKSPACE_FREE_GIB,
            "tmp_free": MIN_TMP_FREE_GIB,
        },
    }


def validate_repo(value: str) -> Path:
    repo = Path(value).resolve()
    required = (repo / "Cargo.toml", repo / "scripts/perf.py")
    if not repo.is_dir() or not all(path.is_file() for path in required):
        raise AcceptanceError(f"not a hell-workers repository root: {repo}")
    return repo


def tracked_paths(repo: Path) -> list[str]:
    completed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=repo,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise AcceptanceError("git ls-files failed while fingerprinting the source")
    return sorted(filter(None, completed.stdout.splitlines()))


def source_fingerprint(repo: Path) -> str:
    digest = hashlib.sha256()
    for relative in tracked_paths(repo):
        source = repo / relative
        if not source.is_file():
            continue
        if relative in SOURCE_FILES or relative.startswith(SOURCE_PREFIXES):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif relative.startswith(ASSET_PREFIX):
            stats = source.stat()
            digest.update(b"asset-stat\0")
            digest.update(relative.encode())
            digest.update(f"\0{stats.st_size}\0{stats.st_mtime_ns}\0".encode())
    return digest.hexdigest()


def unique_job_root() -> Path:
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    return Path(f"/tmp/hell-workers-native-acceptance-{stamp}-{secrets.token_hex(4)}")


def common_dashboard_args(args: argparse.Namespace) -> list[str]:
    return [
        "--workload",
        "task-dashboard",
        "--sizes",
        "small",
        "--renders",
        "cpu",
        "--dashboard-modes",
        "hidden,visible,active-filter",
        "--seed",
        str(args.seed),
        "--backend",
        args.backend,
    ]


def task_dashboard_commands(
    args: argparse.Namespace, repo: Path, job_root: Path
) -> list[tuple[str, list[str]]]:
    perf = ["python3", str(repo / "scripts/perf.py")]
    common = common_dashboard_args(args)
    audit = perf + [
        "audit",
        *common,
        "--repeat",
        "1",
        "--fixed-hz",
        "64",
        "--warmup-ticks",
        "129",
        "--audit-ticks",
        "16",
        "--window-backend",
        "headless",
        "--allow-log-pattern",
        "driver that only supports software rendering",
        "--output",
        str(job_root / "audit"),
    ]
    capture = perf + [
        "run",
        "--instrumentation",
        "capture",
        *common,
        "--repeat",
        str(args.repeat),
        "--warmup-secs",
        str(args.warmup_secs),
        "--measure-secs",
        str(args.measure_secs),
        "--warmup-checksum-policy",
        "record",
        "--measure-end-checksum-policy",
        "record",
        "--adapter",
        args.adapter,
        "--window-backend",
        args.window_backend,
        "--present-mode",
        args.present_mode,
        "--output",
        str(job_root / "capture"),
    ]
    compare_capture = perf + [
        "compare-dashboard-modes",
        "--session",
        str(job_root / "capture"),
        "--min-runs",
        str(args.repeat),
    ]
    build_memory = [
        "cargo",
        "build",
        "--profile",
        "profiling",
        "-p",
        "bevy_app@0.1.0",
        "--no-default-features",
        "--features",
        "profiling-memory",
    ]
    memory = perf + [
        "run",
        "--instrumentation",
        "memory",
        *common,
        "--repeat",
        str(args.repeat),
        "--warmup-secs",
        str(args.warmup_secs),
        "--measure-secs",
        str(args.measure_secs),
        "--warmup-checksum-policy",
        "record",
        "--measure-end-checksum-policy",
        "record",
        "--adapter",
        args.adapter,
        "--window-backend",
        args.window_backend,
        "--present-mode",
        args.present_mode,
        "--output",
        str(job_root / "memory"),
    ]
    compare_memory = perf + [
        "compare-dashboard-modes",
        "--session",
        str(job_root / "memory"),
        "--min-runs",
        str(args.repeat),
    ]
    return [
        ("audit", audit),
        ("capture", capture),
        ("compare-capture", compare_capture),
        ("build-memory", build_memory),
        ("memory", memory),
        ("compare-memory", compare_memory),
    ]


def plan_task_dashboard(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    job_root = Path(args.job_root).resolve() if args.job_root else unique_job_root()
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
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
        "run-task-dashboard",
        "--repo",
        str(repo),
        "--job-root",
        str(job_root),
        "--seed",
        str(args.seed),
        "--repeat",
        str(args.repeat),
        "--warmup-secs",
        str(args.warmup_secs),
        "--measure-secs",
        str(args.measure_secs),
        "--settle-secs",
        str(args.settle_secs),
        "--adapter",
        args.adapter,
        "--backend",
        args.backend,
        "--window-backend",
        args.window_backend,
        "--present-mode",
        args.present_mode,
    ]
    payload = {
        "schema_version": SCHEMA_VERSION,
        "status": resources["status"],
        "profile": "task-dashboard",
        "measurement_kind": (
            "formal" if args.warmup_secs >= 30 and args.measure_secs >= 60 else "acceptance-smoke"
        ),
        "job_root": str(job_root),
        "resources": resources,
        "launcher_command": command,
        "status_command": [
            "python3",
            str(Path(__file__).resolve()),
            "status",
            "--job-root",
            str(job_root),
        ],
        "execution_contract": {
            "game_processes": 3 + (args.repeat * 3 * 2),
            "parallel_game_processes": 1,
            "actual_feature_builds": 2,
            "uses_skip_build": False,
            "uses_binary_copy": False,
            "uses_tracy": False,
            "automatic_cleanup": False,
        },
    }
    print_json(payload)
    return 0 if resources["status"] == "ready" else 1


def update_state(job_file: Path, state: dict[str, Any], **changes: Any) -> None:
    state.update(changes)
    state["heartbeat_at"] = utc_now()
    atomic_write_json(job_file, state)


def run_command(
    stage: str,
    command: list[str],
    *,
    repo: Path,
    env: dict[str, str],
    log_path: Path,
    job_file: Path,
    state: dict[str, Any],
) -> None:
    state.setdefault("commands", []).append({"stage": stage, "argv": command})
    update_state(job_file, state, current_stage=stage)
    with log_path.open("a", encoding="utf-8") as log:
        log.write(f"\n[{utc_now()}] stage={stage}\n")
        log.write(json.dumps(command) + "\n")
        log.flush()
        process = subprocess.Popen(
            command,
            cwd=repo,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            text=True,
        )
        while process.poll() is None:
            update_state(job_file, state, child_pid=process.pid)
            time.sleep(5)
    if process.returncode != 0:
        raise AcceptanceError(f"stage {stage} failed with exit code {process.returncode}")
    completed = state.setdefault("completed_stages", [])
    completed.append(stage)
    update_state(job_file, state, child_pid=None)


def settle(
    seconds: float, *, job_file: Path, state: dict[str, Any], after_stage: str
) -> None:
    if seconds <= 0:
        return
    update_state(job_file, state, current_stage=f"settle-after-{after_stage}")
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        time.sleep(min(2, max(0, deadline - time.monotonic())))
        update_state(job_file, state)


def assert_source_unchanged(repo: Path, expected: str) -> None:
    actual = source_fingerprint(repo)
    if actual != expected:
        raise AcceptanceError(
            f"relevant source changed during acceptance: expected {expected}, got {actual}"
        )


def manifest_binary_hash(session: Path, repo: Path) -> str:
    manifest = read_json(session / "manifest.json")
    expected = manifest.get("binary", {}).get("sha256")
    binary = repo / "target/profiling/bevy_app"
    if not isinstance(expected, str) or len(expected) != 64:
        raise AcceptanceError(f"invalid binary hash in {session / 'manifest.json'}")
    if not binary.is_file():
        raise AcceptanceError(f"profiling binary is missing after session: {binary}")
    actual = sha256(binary)
    if actual != expected:
        raise AcceptanceError(
            f"profiling binary changed during {session.name}: expected {expected}, got {actual}"
        )
    return actual


def run_task_dashboard(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-task-dashboard must be launched by the planned direct kitty command"
        )
    repo = validate_repo(args.repo)
    job_root = Path(args.job_root).resolve()
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    job_root.mkdir(parents=True)
    job_file = job_root / "job.json"
    log_path = job_root / "orchestrator.log"
    resources = resource_snapshot(repo, require_launcher=False)
    source = source_fingerprint(repo)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "profile": "task-dashboard",
        "status": "running",
        "started_at": utc_now(),
        "heartbeat_at": utc_now(),
        "pid": os.getpid(),
        "repo": str(repo),
        "job_root": str(job_root),
        "source_fingerprint": source,
        "resources": resources,
        "completed_stages": [],
        "paths": {
            "audit": str(job_root / "audit"),
            "capture": str(job_root / "capture"),
            "memory": str(job_root / "memory"),
            "log": str(log_path),
        },
        "parameters": {
            "seed": args.seed,
            "repeat": args.repeat,
            "warmup_secs": args.warmup_secs,
            "measure_secs": args.measure_secs,
            "settle_secs": args.settle_secs,
            "adapter": args.adapter,
            "backend": args.backend,
            "window_backend": args.window_backend,
            "present_mode": args.present_mode,
        },
    }
    atomic_write_json(job_file, state)
    if resources["status"] != "ready":
        update_state(
            job_file,
            state,
            status="invalid",
            current_stage=None,
            error="; ".join(resources["failures"]),
            finished_at=utc_now(),
        )
        return 1

    env = os.environ.copy()
    env.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "CARGO_BUILD_JOBS": str(resources["cargo_jobs"]),
            "CARGO_INCREMENTAL": "0",
        }
    )
    commands = dict(task_dashboard_commands(args, repo, job_root))
    LOCK_PATH.touch(exist_ok=True)
    with LOCK_PATH.open("r+", encoding="utf-8") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            update_state(
                job_file,
                state,
                status="invalid",
                current_stage=None,
                error=f"another native acceptance job holds {LOCK_PATH}",
                finished_at=utc_now(),
            )
            return 1
        try:
            run_command(
                "audit",
                commands["audit"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )
            assert_source_unchanged(repo, source)
            capture_hash = manifest_binary_hash(job_root / "audit", repo)
            state["capture_binary_sha256"] = capture_hash
            settle(args.settle_secs, job_file=job_file, state=state, after_stage="audit")

            run_command(
                "capture",
                commands["capture"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )
            assert_source_unchanged(repo, source)
            if manifest_binary_hash(job_root / "capture", repo) != capture_hash:
                raise AcceptanceError("audit and Capture did not use the same binary hash")
            run_command(
                "compare-capture",
                commands["compare-capture"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )

            run_command(
                "build-memory",
                commands["build-memory"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )
            assert_source_unchanged(repo, source)
            memory_hash = sha256(repo / "target/profiling/bevy_app")
            if memory_hash == capture_hash:
                raise AcceptanceError("Memory feature build did not change the binary hash")
            state["memory_binary_sha256"] = memory_hash
            settle(
                args.settle_secs,
                job_file=job_file,
                state=state,
                after_stage="build-memory",
            )

            run_command(
                "memory",
                commands["memory"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )
            assert_source_unchanged(repo, source)
            if manifest_binary_hash(job_root / "memory", repo) != memory_hash:
                raise AcceptanceError("Memory session did not retain the built binary hash")
            run_command(
                "compare-memory",
                commands["compare-memory"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
            )
            verification = verify_artifact_set(
                job_root / "audit",
                job_root / "capture",
                job_root / "memory",
                adapter=args.adapter,
                backend=args.backend,
                window_backend=args.window_backend,
                min_runs=args.repeat,
            )
            update_state(
                job_file,
                state,
                status="valid",
                current_stage=None,
                child_pid=None,
                verification=verification,
                finished_at=utc_now(),
            )
            return 0
        except Exception as error:
            update_state(
                job_file,
                state,
                status="invalid",
                current_stage=None,
                child_pid=None,
                error=str(error),
                finished_at=utc_now(),
            )
            return 1


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AcceptanceError(message)


def verify_session(
    session: Path,
    *,
    instrumentation: str,
    comparison_name: str,
    repeat: int,
    adapter: str | None,
    backend: str,
    window_backend: str,
) -> dict[str, Any]:
    manifest = read_json(session / "manifest.json")
    comparison = read_json(session / comparison_name)
    matrix = manifest.get("matrix", {})
    binary = manifest.get("binary", {})
    require(manifest.get("status") == "valid", f"session is not valid: {session}")
    require(
        binary.get("instrumentation") == instrumentation,
        f"wrong instrumentation in {session}: {binary.get('instrumentation')}",
    )
    require(matrix.get("workload") == "task-dashboard", f"wrong workload in {session}")
    require(set(matrix.get("dashboard_modes", [])) == DASHBOARD_MODES, f"wrong modes in {session}")
    require(matrix.get("repeat") == repeat, f"wrong repeat count in {session}")
    require(comparison.get("status") == "pass", f"comparison did not pass: {session}")
    require(not comparison.get("failures"), f"comparison has failures: {session}")
    require((session / "aggregate.csv").is_file(), f"missing aggregate.csv: {session}")
    require((session / "report.md").is_file(), f"missing report.md: {session}")
    actual_adapters = manifest.get("actual_adapters", [])
    if window_backend == "headless":
        require(
            manifest.get("requested_environment", {}).get("HW_WINDOW_BACKEND") == "headless",
            f"audit is not headless: {session}",
        )
    else:
        require(
            manifest.get("requested_environment", {}).get("HW_WINDOW_BACKEND")
            == window_backend,
            f"wrong window backend in {session}",
        )
        require(bool(actual_adapters), f"missing actual adapter evidence: {session}")
        if adapter:
            require(
                any(adapter.lower() in str(item.get("name", "")).lower() for item in actual_adapters),
                f"actual adapter does not contain {adapter!r}: {session}",
            )
        require(
            any(str(item.get("backend", "")).lower() == backend.lower() for item in actual_adapters),
            f"actual backend is not {backend}: {session}",
        )
    return {
        "path": str(session),
        "status": "pass",
        "instrumentation": instrumentation,
        "binary_sha256": binary.get("sha256"),
        "repeat": repeat,
        "actual_adapters": actual_adapters,
    }


def verify_artifact_set(
    audit: Path,
    capture: Path,
    memory: Path,
    *,
    adapter: str,
    backend: str,
    window_backend: str,
    min_runs: int,
) -> dict[str, Any]:
    audit_result = verify_session(
        audit,
        instrumentation="capture",
        comparison_name="dashboard_mode_comparison.json",
        repeat=1,
        adapter=None,
        backend=backend,
        window_backend="headless",
    )
    capture_result = verify_session(
        capture,
        instrumentation="capture",
        comparison_name="dashboard_mode_cost_comparison.json",
        repeat=min_runs,
        adapter=adapter,
        backend=backend,
        window_backend=window_backend,
    )
    memory_result = verify_session(
        memory,
        instrumentation="memory",
        comparison_name="dashboard_mode_cost_comparison.json",
        repeat=min_runs,
        adapter=adapter,
        backend=backend,
        window_backend=window_backend,
    )
    require(
        audit_result["binary_sha256"] == capture_result["binary_sha256"],
        "fixed audit and Capture binary hashes differ",
    )
    require(
        capture_result["binary_sha256"] != memory_result["binary_sha256"],
        "Capture and Memory binary hashes unexpectedly match",
    )
    return {
        "status": "pass",
        "audit": audit_result,
        "capture": capture_result,
        "memory": memory_result,
    }


def verify_artifacts_command(args: argparse.Namespace) -> int:
    result = verify_artifact_set(
        Path(args.audit).resolve(),
        Path(args.capture).resolve(),
        Path(args.memory).resolve(),
        adapter=args.adapter,
        backend=args.backend,
        window_backend=args.window_backend,
        min_runs=args.min_runs,
    )
    print_json(result)
    return 0


def parse_utc(value: str) -> datetime:
    return datetime.fromisoformat(value)


def status_command(args: argparse.Namespace) -> int:
    job_root = Path(args.job_root).resolve()
    job_file = job_root / "job.json"
    if not job_file.is_file():
        print_json({"status": "not-started", "job_root": str(job_root)})
        return RUNNING_EXIT_CODE
    state = read_json(job_file)
    status = state.get("status")
    summary = {
        key: state.get(key)
        for key in (
            "schema_version",
            "profile",
            "status",
            "current_stage",
            "completed_stages",
            "started_at",
            "heartbeat_at",
            "finished_at",
            "job_root",
            "paths",
            "error",
            "verification",
        )
        if state.get(key) is not None
    }
    if status == "running":
        heartbeat = state.get("heartbeat_at")
        if isinstance(heartbeat, str):
            age = (datetime.now(UTC) - parse_utc(heartbeat)).total_seconds()
            summary["heartbeat_age_secs"] = round(age, 1)
            if age > args.stale_after_secs:
                summary["status"] = "stale"
                summary["error"] = f"heartbeat is {round(age, 1)} seconds old"
                print_json(summary)
                return 1
        print_json(summary)
        return RUNNING_EXIT_CODE
    print_json(summary)
    return 0 if status == "valid" else 1


def write_fake_session(
    path: Path,
    *,
    instrumentation: str,
    binary_hash: str,
    repeat: int,
    window_backend: str,
    comparison_name: str,
) -> None:
    path.mkdir(parents=True)
    manifest = {
        "status": "valid",
        "binary": {"instrumentation": instrumentation, "sha256": binary_hash},
        "matrix": {
            "workload": "task-dashboard",
            "dashboard_modes": sorted(DASHBOARD_MODES),
            "repeat": repeat,
        },
        "requested_environment": {"HW_WINDOW_BACKEND": window_backend},
        "actual_adapters": (
            []
            if window_backend == "headless"
            else [{"name": "Intel(R) Arc Graphics", "backend": "Vulkan"}]
        ),
    }
    atomic_write_json(path / "manifest.json", manifest)
    atomic_write_json(path / comparison_name, {"status": "pass", "failures": []})
    (path / "aggregate.csv").write_text("status\nvalid\n", encoding="utf-8")
    (path / "report.md").write_text("# valid\n", encoding="utf-8")


def self_test() -> int:
    with tempfile.TemporaryDirectory(prefix="native-acceptance-self-test-") as temporary:
        root = Path(temporary)
        write_fake_session(
            root / "audit",
            instrumentation="capture",
            binary_hash="a" * 64,
            repeat=1,
            window_backend="headless",
            comparison_name="dashboard_mode_comparison.json",
        )
        write_fake_session(
            root / "capture",
            instrumentation="capture",
            binary_hash="a" * 64,
            repeat=3,
            window_backend="x11",
            comparison_name="dashboard_mode_cost_comparison.json",
        )
        write_fake_session(
            root / "memory",
            instrumentation="memory",
            binary_hash="b" * 64,
            repeat=3,
            window_backend="x11",
            comparison_name="dashboard_mode_cost_comparison.json",
        )
        result = verify_artifact_set(
            root / "audit",
            root / "capture",
            root / "memory",
            adapter="Intel",
            backend="vulkan",
            window_backend="x11",
            min_runs=3,
        )
        require(result["status"] == "pass", "valid fixture did not pass")
        broken = read_json(root / "memory" / "manifest.json")
        broken["binary"]["instrumentation"] = "capture"
        atomic_write_json(root / "memory" / "manifest.json", broken)
        try:
            verify_artifact_set(
                root / "audit",
                root / "capture",
                root / "memory",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
                min_runs=3,
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("invalid instrumentation fixture unexpectedly passed")
    print("native_acceptance self-test: PASS")
    return 0


def add_recipe_arguments(parser: argparse.ArgumentParser, *, require_job_root: bool) -> None:
    parser.add_argument("--repo", required=True)
    parser.add_argument("--job-root", required=require_job_root)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--warmup-secs", type=float, default=1.0)
    parser.add_argument("--measure-secs", type=float, default=2.0)
    parser.add_argument("--settle-secs", type=float, default=8.0)
    parser.add_argument("--adapter", default="Intel")
    parser.add_argument("--backend", default="vulkan", choices=["vulkan", "gl"])
    parser.add_argument("--window-backend", default="x11", choices=["x11", "wayland"])
    parser.add_argument("--present-mode", default="novsync")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan = commands.add_parser("plan-task-dashboard", help="emit a no-prompt launcher plan")
    add_recipe_arguments(plan, require_job_root=False)
    run = commands.add_parser("run-task-dashboard", help="run the planned recipe inside kitty")
    add_recipe_arguments(run, require_job_root=True)
    status = commands.add_parser("status", help="print a compact atomic job status")
    status.add_argument("--job-root", required=True)
    status.add_argument("--stale-after-secs", type=float, default=90.0)
    verify = commands.add_parser("verify-artifacts", help="fail-closed validation of three sessions")
    verify.add_argument("--audit", required=True)
    verify.add_argument("--capture", required=True)
    verify.add_argument("--memory", required=True)
    verify.add_argument("--adapter", default="Intel")
    verify.add_argument("--backend", default="vulkan")
    verify.add_argument("--window-backend", default="x11", choices=["x11", "wayland"])
    verify.add_argument("--min-runs", type=int, default=3)
    commands.add_parser("self-test", help="run stdlib-only helper tests")
    return root


def validate_args(args: argparse.Namespace) -> None:
    if args.command in {"plan-task-dashboard", "run-task-dashboard"}:
        if args.repeat < 3:
            raise AcceptanceError("Task Dashboard Capture and Memory require at least 3 runs")
        if args.warmup_secs < 0 or args.measure_secs <= 0:
            raise AcceptanceError("warmup must be nonnegative and measure must be positive")
        if args.settle_secs < 0:
            raise AcceptanceError("settle seconds cannot be negative")
    if args.command == "verify-artifacts" and args.min_runs < 3:
        raise AcceptanceError("artifact verification requires at least 3 runs")
    if args.command == "status" and args.stale_after_secs < 15:
        raise AcceptanceError("stale threshold must be at least 15 seconds")


def main() -> int:
    args = parser().parse_args()
    validate_args(args)
    if args.command == "plan-task-dashboard":
        return plan_task_dashboard(args)
    if args.command == "run-task-dashboard":
        return run_task_dashboard(args)
    if args.command == "status":
        return status_command(args)
    if args.command == "verify-artifacts":
        return verify_artifacts_command(args)
    if args.command == "self-test":
        return self_test()
    raise AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AcceptanceError as error:
        print_json({"status": "invalid", "error": str(error)})
        raise SystemExit(1) from error
