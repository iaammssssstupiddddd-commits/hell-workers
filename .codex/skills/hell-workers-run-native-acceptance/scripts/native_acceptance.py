#!/usr/bin/env python3
"""Plan, run, monitor, and verify no-prompt native acceptance sessions."""

from __future__ import annotations

import argparse
import fcntl
from functools import wraps
import hashlib
import json
import os
import re
import secrets
import signal
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
import zlib
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

try:
    from scripts.build_coordination import acquire_activity
except ModuleNotFoundError:
    repository_root = Path(__file__).resolve().parents[4]
    sys.path.insert(0, str(repository_root))
    from scripts.build_coordination import acquire_activity


GIB = 1024**3
SCHEMA_VERSION = 1
RUNNING_EXIT_CODE = 2
MIN_START_MEMORY_GIB = 10
MIN_RUNTIME_MEMORY_GIB = 8
TWO_JOB_MEMORY_GIB = 16
MIN_WORKSPACE_FREE_GIB = 15
RESOURCE_POLL_SECONDS = 1.0
PROCESS_GROUP_POLL_SECONDS = 0.1
PROCESS_GROUP_TERM_GRACE_SECONDS = 10.0
PROCESS_GROUP_KILL_GRACE_SECONDS = 10.0
MAX_NATIVE_SCREENSHOT_BYTES = 16 * 1024 * 1024
DEFAULT_SEED = 20260802
LOCK_PATH = Path("/tmp/hell-workers-native-acceptance.lock")
MEMORY_FILESYSTEM_TYPES = frozenset({"tmpfs", "ramfs", "devtmpfs"})
TMP_ROOT = Path("/tmp")
TMP_CARGO_TARGET_GLOB = "hell-workers-*-target"
NATIVE_JOB_DIRECTORY = "native-acceptance"
NATIVE_TEMP_DIRECTORY = ".native-acceptance-tmp"
DASHBOARD_MODES = {"hidden", "visible", "active-filter"}
SOURCE_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "scripts/perf.py",
}
SOURCE_PREFIXES = (
    "crates/",
    "scripts/perf_tool/",
)
ASSET_PREFIX = "assets/"
DECONSTRUCTION_CHECKS = {"V1", "V2", "V3", "V4", "V5"}
RTT_LIGHT_CONTRACT_ID = "rtt-light-v1"
RTT_LIGHT_STAGE = "current"
RTT_LIGHT_LEGS = ("audit", "behavior", "capture", "renderdoc", "memory")
RTT_LIGHT_SOURCE_CHECKPOINTS = (
    "start",
    "after-audit",
    "after-behavior",
    "after-capture",
    "after-renderdoc",
    "after-memory",
    "before-registration",
)
RTT_LIGHT_RENDERDOC_API_VERSION = "1.6.0"
RTT_LIGHT_SETTLE_SECS = 8.0


class AcceptanceError(RuntimeError):
    """A fail-closed acceptance error."""


def activity_locked(function):
    """Keep the whole native recipe exclusive of interactive Cargo activity."""
    @wraps(function)
    def wrapped(args: argparse.Namespace) -> int:
        repo = Path(args.repo).resolve()
        with acquire_activity(repo, "exclusive"):
            return function(args)

    return wrapped


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


def meminfo_bytes() -> dict[str, int]:
    """Read the Linux memory counters used by the native resource guard."""
    try:
        lines = Path("/proc/meminfo").read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise AcceptanceError(f"cannot read /proc/meminfo: {error}") from error

    values: dict[str, int] = {}
    for line in lines:
        key, separator, remainder = line.partition(":")
        fields = remainder.split()
        if not separator or not fields or not fields[0].isdigit():
            continue
        values[key] = int(fields[0]) * 1024
    return values


def memory_snapshot() -> dict[str, int | None]:
    values = meminfo_bytes()
    available = values.get("MemAvailable")
    if available is None:
        raise AcceptanceError("/proc/meminfo does not expose MemAvailable")
    return {
        "mem_available_bytes": available,
        "swap_total_bytes": values.get("SwapTotal"),
        "swap_free_bytes": values.get("SwapFree"),
    }


def mem_available_bytes() -> int:
    available = memory_snapshot()["mem_available_bytes"]
    assert available is not None
    return available


def free_bytes(path: Path) -> int:
    probe = path
    while not probe.exists() and probe.parent != probe:
        probe = probe.parent
    stats = os.statvfs(probe)
    return stats.f_bavail * stats.f_frsize


def gib(value: int) -> float:
    return round(value / GIB, 2)


def optional_gib(value: int | None) -> float | None:
    return gib(value) if value is not None else None


def memory_safety_failures(
    snapshot: dict[str, int | None],
    *,
    minimum_memory_gib: int,
    phase: str,
) -> list[str]:
    """Return deterministic host-memory guard failures for one native phase."""
    available = snapshot["mem_available_bytes"]
    assert available is not None
    failures: list[str] = []
    if available < minimum_memory_gib * GIB:
        failures.append(
            f"MemAvailable {gib(available)} GiB is below {minimum_memory_gib} GiB "
            f"native {phase} floor"
        )

    # MemAvailable is the primary admission signal: it estimates how much
    # memory the kernel can provide without swapping. Swap counters remain in
    # the snapshot for diagnosis, but a low/unknown swap balance does not block
    # a run while the RAM floor is satisfied.
    return failures


def runtime_resource_failure() -> tuple[str, dict[str, float | None]] | None:
    """Return live evidence when a launched native stage must stop safely."""
    try:
        snapshot = memory_snapshot()
    except AcceptanceError as error:
        return f"native runtime resource monitoring failed: {error}", {}
    failures = memory_safety_failures(
        snapshot,
        minimum_memory_gib=MIN_RUNTIME_MEMORY_GIB,
        phase="runtime",
    )
    if not failures:
        return None
    return (
        "; ".join(failures),
        {
            "mem_available_gib": optional_gib(snapshot["mem_available_bytes"]),
            "swap_total_gib": optional_gib(snapshot["swap_total_bytes"]),
            "swap_free_gib": optional_gib(snapshot["swap_free_bytes"]),
        },
    )


def decode_mount_path(value: str) -> str:
    return re.sub(
        r"\\([0-7]{3})",
        lambda match: chr(int(match.group(1), 8)),
        value,
    )


def path_is_within(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def filesystem_type(path: Path, *, mountinfo_path: Path = Path("/proc/self/mountinfo")) -> str:
    target = path.resolve()
    matched: tuple[int, str] | None = None
    try:
        lines = mountinfo_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise AcceptanceError(f"cannot inspect mount table for {target}: {error}") from error
    for line in lines:
        left, separator, right = line.partition(" - ")
        if not separator:
            continue
        left_fields = left.split()
        right_fields = right.split()
        if len(left_fields) < 5 or not right_fields:
            continue
        mount_point = Path(decode_mount_path(left_fields[4])).resolve()
        if not path_is_within(target, mount_point):
            continue
        candidate = (len(str(mount_point)), right_fields[0])
        if matched is None or candidate[0] > matched[0]:
            matched = candidate
    if matched is None:
        raise AcceptanceError(f"cannot resolve filesystem type for {target}")
    return matched[1]


def workspace_cargo_target(repo: Path) -> Path:
    return (repo / "target").resolve()


def workspace_process_temp_dir(repo: Path) -> Path:
    return workspace_cargo_target(repo) / NATIVE_TEMP_DIRECTORY


def native_job_directory(repo: Path) -> Path:
    return workspace_cargo_target(repo) / NATIVE_JOB_DIRECTORY


def persistent_storage_error(
    path: Path,
    *,
    label: str,
    mountinfo_path: Path = Path("/proc/self/mountinfo"),
    temporary_root: Path = TMP_ROOT,
) -> str | None:
    resolved = path.resolve()
    if path_is_within(resolved, temporary_root):
        return f"{label} must not be placed under {temporary_root}: {resolved}"
    filesystem = filesystem_type(resolved, mountinfo_path=mountinfo_path)
    if filesystem in MEMORY_FILESYSTEM_TYPES:
        return (
            f"{label} must use persistent storage, not {filesystem}: {resolved}"
        )
    return None


def require_persistent_storage(path: Path, *, label: str) -> None:
    error = persistent_storage_error(path, label=label)
    if error:
        raise AcceptanceError(error)


def account_home() -> Path:
    """Resolve the account home without trusting an inherited ``HOME`` value."""
    try:
        import pwd

        return Path(pwd.getpwuid(os.getuid()).pw_dir).resolve()
    except (ImportError, KeyError, OSError):
        return Path.home().resolve()


def environment_path(repo: Path, value: str) -> Path:
    candidate = Path(value).expanduser()
    if not candidate.is_absolute():
        candidate = repo / candidate
    return candidate.resolve()


def persistent_toolchain_home(
    repo: Path,
    environment: dict[str, str],
    *,
    variable: str,
    default_name: str,
    label: str,
) -> Path:
    """Keep Cargo/rustup caches on disk while preserving safe custom homes."""
    inherited = environment.get(variable)
    if inherited:
        candidate = environment_path(repo, inherited)
        if persistent_storage_error(candidate, label=label) is None:
            return candidate
    fallback = account_home() / default_name
    require_persistent_storage(fallback, label=label)
    return fallback


def cargo_environment(
    repo: Path, environment: dict[str, str] | None = None
) -> dict[str, str]:
    values = os.environ.copy() if environment is None else environment.copy()
    cargo_target = workspace_cargo_target(repo)
    temporary = workspace_process_temp_dir(repo)
    require_persistent_storage(cargo_target, label="workspace Cargo target")
    require_persistent_storage(temporary, label="native process temporary directory")
    temporary.mkdir(parents=True, exist_ok=True)
    cargo_home = persistent_toolchain_home(
        repo,
        values,
        variable="CARGO_HOME",
        default_name=".cargo",
        label="Cargo home",
    )
    rustup_home = persistent_toolchain_home(
        repo,
        values,
        variable="RUSTUP_HOME",
        default_name=".rustup",
        label="rustup home",
    )
    values.update(
        {
            "CARGO_TARGET_DIR": str(cargo_target),
            "CARGO_HOME": str(cargo_home),
            "RUSTUP_HOME": str(rustup_home),
            "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "1",
            "TMPDIR": str(temporary),
            "TMP": str(temporary),
            "TEMP": str(temporary),
        }
    )
    return values


def directory_allocated_bytes(directory: Path) -> int:
    total = 0
    for root, directories, files in os.walk(directory, followlinks=False):
        root_path = Path(root)
        directories[:] = [
            name for name in directories if not (root_path / name).is_symlink()
        ]
        for name in files:
            path = root_path / name
            try:
                total += path.lstat().st_blocks * 512
            except FileNotFoundError:
                continue
    return total


def legacy_tmp_cargo_targets(tmp_root: Path = TMP_ROOT) -> list[dict[str, Any]]:
    targets = []
    for candidate in sorted(tmp_root.glob(TMP_CARGO_TARGET_GLOB)):
        if candidate.is_symlink() or not candidate.is_dir():
            continue
        profiles = ("debug", "profiling", "release")
        has_profile_lock = any(
            (candidate / profile / ".cargo-lock").is_file()
            for profile in profiles
        )
        if not (candidate / ".rustc_info.json").is_file() and not has_profile_lock:
            continue
        allocated = directory_allocated_bytes(candidate)
        if allocated:
            targets.append(
                {
                    "path": str(candidate.resolve()),
                    "bytes": allocated,
                    "gib": gib(allocated),
                }
            )
    return targets


def resource_snapshot(repo: Path, *, require_launcher: bool) -> dict[str, Any]:
    memory = memory_snapshot()
    available_memory = memory["mem_available_bytes"]
    assert available_memory is not None
    workspace_free = free_bytes(repo)
    tmp_free = free_bytes(TMP_ROOT)
    cargo_target = workspace_cargo_target(repo)
    cargo_target_free = free_bytes(cargo_target)
    process_temp = workspace_process_temp_dir(repo)
    native_jobs = native_job_directory(repo)
    performance_artifacts = cargo_target / "perf-runs"
    renderdoc_temp = cargo_target / ".renderdoc-tmp"
    cargo_target_filesystem = filesystem_type(cargo_target)
    process_temp_filesystem = filesystem_type(process_temp)
    inherited_target = os.environ.get("CARGO_TARGET_DIR")
    inherited_temp = {
        key: os.environ[key]
        for key in ("TMPDIR", "TMP", "TEMP")
        if key in os.environ
    }
    inherited_toolchain_homes = {
        key: os.environ[key]
        for key in ("CARGO_HOME", "RUSTUP_HOME")
        if key in os.environ
    }
    stale_tmp_targets = legacy_tmp_cargo_targets()
    launcher = shutil.which("kitty")
    failures: list[str] = []
    try:
        cargo_home = persistent_toolchain_home(
            repo,
            os.environ,
            variable="CARGO_HOME",
            default_name=".cargo",
            label="Cargo home",
        )
        rustup_home = persistent_toolchain_home(
            repo,
            os.environ,
            variable="RUSTUP_HOME",
            default_name=".rustup",
            label="rustup home",
        )
    except AcceptanceError as error:
        failures.append(str(error))
        cargo_home = account_home() / ".cargo"
        rustup_home = account_home() / ".rustup"
    failures.extend(
        memory_safety_failures(
            memory,
            minimum_memory_gib=MIN_START_MEMORY_GIB,
            phase="start",
        )
    )
    if cargo_target_free < MIN_WORKSPACE_FREE_GIB * GIB:
        failures.append(
            f"Cargo target free {gib(cargo_target_free)} GiB is below "
            f"{MIN_WORKSPACE_FREE_GIB} GiB"
        )
    for path, label in (
        (cargo_target, "workspace Cargo target"),
        (process_temp, "native process temporary directory"),
        (cargo_home, "Cargo home"),
        (rustup_home, "rustup home"),
        (native_jobs, "native acceptance job directory"),
        (performance_artifacts, "performance artifact root"),
        (renderdoc_temp, "RenderDoc temporary directory"),
    ):
        storage_error = persistent_storage_error(path, label=label)
        if storage_error:
            failures.append(storage_error)
    for stale_target in stale_tmp_targets:
        failures.append(
            "legacy Cargo target under /tmp uses "
            f"{stale_target['gib']} GiB: {stale_target['path']} "
            "(preserved; clean up separately)"
        )
    if require_launcher and launcher is None:
        failures.append("kitty launcher is unavailable")
    return {
        "status": "ready" if not failures else "blocked",
        "failures": failures,
        "mem_available_gib": gib(available_memory),
        "swap_total_gib": optional_gib(memory["swap_total_bytes"]),
        "swap_free_gib": optional_gib(memory["swap_free_bytes"]),
        "workspace_free_gib": gib(workspace_free),
        "tmp_free_gib": gib(tmp_free),
        "cargo_jobs": 2 if available_memory >= TWO_JOB_MEMORY_GIB * GIB else 1,
        "cargo_incremental": 0,
        "cargo_target": {
            "path": str(cargo_target),
            "filesystem": cargo_target_filesystem,
            "free_gib": gib(cargo_target_free),
            "inherited_target_dir": inherited_target,
            "enforced": True,
        },
        "process_temp": {
            "path": str(process_temp),
            "filesystem": process_temp_filesystem,
            "inherited_temp_dirs": inherited_temp,
            "enforced": True,
        },
        "toolchain_homes": {
            "cargo": {
                "path": str(cargo_home),
                "filesystem": filesystem_type(cargo_home),
            },
            "rustup": {
                "path": str(rustup_home),
                "filesystem": filesystem_type(rustup_home),
            },
            "inherited": inherited_toolchain_homes,
            "enforced": True,
        },
        "legacy_tmp_cargo_targets": stale_tmp_targets,
        "launcher": launcher,
        "thresholds_gib": {
            "native_start_mem_available": MIN_START_MEMORY_GIB,
            "native_runtime_mem_available": MIN_RUNTIME_MEMORY_GIB,
            "cargo_target_free": MIN_WORKSPACE_FREE_GIB,
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


def command_output(command: list[str], *, repo: Path) -> str:
    completed = subprocess.run(
        command,
        cwd=repo,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        raise AcceptanceError(
            f"command failed ({' '.join(command)}): {detail or completed.returncode}"
        )
    return completed.stdout.strip()


def git_subject(repo: Path) -> str:
    commit = command_output(["git", "rev-parse", "HEAD"], repo=repo)
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise AcceptanceError(f"HEAD is not a full commit SHA: {commit!r}")
    return commit


def git_dirty_paths(repo: Path) -> list[str]:
    output = command_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], repo=repo
    )
    return output.splitlines() if output else []


def assert_clean_subject(repo: Path, expected_commit: str) -> None:
    actual_commit = git_subject(repo)
    if actual_commit != expected_commit:
        raise AcceptanceError(
            f"subject commit changed: expected {expected_commit}, got {actual_commit}"
        )
    dirty = git_dirty_paths(repo)
    if dirty:
        preview = ", ".join(dirty[:8])
        raise AcceptanceError(f"formal RtT-light subject is dirty: {preview}")


def assert_prerequisite_ancestors(
    repo: Path, subject_commit: str, prerequisite_commits: list[str]
) -> None:
    if not prerequisite_commits:
        raise AcceptanceError(
            "formal RtT-light registration requires at least one prerequisite correctness commit"
        )
    if len(prerequisite_commits) != len(set(prerequisite_commits)):
        raise AcceptanceError("prerequisite commits contain duplicates")
    for commit in prerequisite_commits:
        if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
            raise AcceptanceError(f"prerequisite is not a full commit SHA: {commit!r}")
        completed = subprocess.run(
            ["git", "merge-base", "--is-ancestor", commit, subject_commit],
            cwd=repo,
            check=False,
        )
        if completed.returncode != 0:
            raise AcceptanceError(
                f"prerequisite commit is not an ancestor of the subject: {commit}"
            )


def rtt_light_contract(repo: Path) -> dict[str, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    try:
        from perf_tool.rtt_light_contract import load_rtt_light_contract

        contract = load_rtt_light_contract(RTT_LIGHT_CONTRACT_ID)
    except (ImportError, OSError, ValueError, RuntimeError) as error:
        raise AcceptanceError(f"RtT-light contract validation failed: {error}") from error
    legs = [
        leg.get("leg_id")
        for leg in contract.get("formal_legs", [])
        if isinstance(leg, dict) and leg.get("first_required_stage") == "current"
    ]
    if legs != list(RTT_LIGHT_LEGS):
        raise AcceptanceError(
            f"RtT-light current leg order differs from the launcher: {legs}"
        )
    return contract


def formal_contract_ready(contract: dict[str, Any]) -> list[str]:
    lifecycle = contract.get("lifecycle")
    if lifecycle != {
        "status": "frozen",
        "formal_registration_allowed": True,
        "freeze_blockers": [],
    }:
        return ["RtT-light contract is not frozen for formal registration"]
    return []


def rtt_light_attempt_path(
    repo: Path, *, subject_commit: str, attempt_id: str
) -> Path:
    try:
        parsed = uuid.UUID(attempt_id)
    except ValueError as error:
        raise AcceptanceError("attempt id is not a UUID") from error
    if parsed.version != 4 or str(parsed) != attempt_id:
        raise AcceptanceError("attempt id must be a canonical UUIDv4")
    return (
        repo
        / "target/perf-runs/rtt-light"
        / RTT_LIGHT_CONTRACT_ID
        / f"{RTT_LIGHT_STAGE}-{subject_commit[:16]}"
        / "attempts"
        / attempt_id
    )


def source_checkpoint(
    repo: Path, *, checkpoint: str, subject_commit: str, fingerprint: str
) -> dict[str, Any]:
    if checkpoint not in RTT_LIGHT_SOURCE_CHECKPOINTS:
        raise AcceptanceError(f"unknown RtT-light source checkpoint {checkpoint}")
    assert_clean_subject(repo, subject_commit)
    actual = source_fingerprint(repo)
    if actual != fingerprint:
        raise AcceptanceError(
            f"source fingerprint changed at {checkpoint}: expected {fingerprint}, got {actual}"
        )
    return {
        "checkpoint": checkpoint,
        "commit": subject_commit,
        "clean": True,
        "fingerprint": fingerprint,
    }


def unique_job_root(repo: Path, profile: str) -> Path:
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    return native_job_directory(repo) / f"{profile}-{stamp}-{secrets.token_hex(4)}"


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


def executable(value: str | None, label: str) -> Path:
    if not value:
        raise AcceptanceError(f"{label} is unavailable")
    path = Path(value).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise AcceptanceError(f"{label} is not executable: {path}")
    return path


def renderdoc_version(path: Path) -> str:
    failures: list[str] = []
    for arguments in (("version",), ("--version",)):
        completed = subprocess.run(
            [str(path), *arguments],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        output = (completed.stdout or completed.stderr).strip()
        if completed.returncode == 0 and output:
            return output.splitlines()[0]
        failures.append(f"{' '.join(arguments)} rc={completed.returncode}")
    raise AcceptanceError(
        f"cannot query RenderDoc version from {path}: {'; '.join(failures)}"
    )


def inspect_renderdoc_tools(
    repo: Path, args: argparse.Namespace
) -> tuple[dict[str, Any] | None, list[str]]:
    try:
        renderdoccmd = executable(
            args.renderdoccmd or shutil.which("renderdoccmd"), "renderdoccmd"
        )
        qrenderdoc = executable(
            args.qrenderdoc or shutil.which("qrenderdoc"), "qrenderdoc"
        )
        library_value = args.renderdoc_library or os.environ.get(
            "RENDERDOC_LIBRARY"
        )
        if not library_value:
            raise AcceptanceError(
                "librenderdoc path is unavailable; pass --renderdoc-library"
            )
        library = Path(library_value).resolve()
        if not library.is_file():
            raise AcceptanceError(f"librenderdoc is not a file: {library}")
        capture_helper = repo / "scripts/perf_tool/renderdoc_capture.py"
        extractor = repo / "scripts/perf_tool/renderdoc_extract.py"
        for path, label in (
            (capture_helper, "RenderDoc capture helper"),
            (extractor, "RenderDoc extractor"),
        ):
            if not path.is_file():
                raise AcceptanceError(f"{label} is missing: {path}")
        version = renderdoc_version(renderdoccmd)
        qversion = renderdoc_version(qrenderdoc)
        probe = subprocess.run(
            [
                "python3",
                str(capture_helper),
                "probe",
                "--renderdoccmd",
                str(renderdoccmd),
                "--qrenderdoc",
                str(qrenderdoc),
                "--renderdoc-library",
                str(library),
            ],
            cwd=repo,
            check=False,
            capture_output=True,
            text=True,
            timeout=60,
        )
        if probe.returncode != 0:
            raise AcceptanceError(
                "RenderDoc helper probe failed: "
                + (probe.stderr or probe.stdout).strip()
            )
        helper_path = Path(__file__).resolve()
        skill_path = helper_path.parent.parent / "SKILL.md"
        metadata = {
            "paths": {
                "renderdoccmd": str(renderdoccmd),
                "qrenderdoc": str(qrenderdoc),
                "renderdoc_library": str(library),
                "capture_helper": str(capture_helper),
                "extractor": str(extractor),
            },
            "job": {
                "native_helper_sha256": sha256(helper_path),
                "native_skill_sha256": sha256(skill_path),
                "perf_runner_sha256": sha256(repo / "scripts/perf.py"),
                "renderdoccmd_sha256": sha256(renderdoccmd),
                "qrenderdoc_sha256": sha256(qrenderdoc),
                "librenderdoc_sha256": sha256(library),
                "renderdoc_version": version,
                "qrenderdoc_version": qversion,
                "renderdoc_api_version": RTT_LIGHT_RENDERDOC_API_VERSION,
                "renderdoc_capture_helper_sha256": sha256(capture_helper),
                "renderdoc_extractor_sha256": sha256(extractor),
            },
        }
        return metadata, []
    except (AcceptanceError, OSError, subprocess.SubprocessError) as error:
        return None, [str(error)]


def append_allow_patterns(command: list[str], patterns: list[str]) -> None:
    for pattern in patterns:
        command.extend(["--allow-log-pattern", pattern])


def rtt_light_session_commands(
    *,
    repo: Path,
    output_root: Path,
    environment_lock: Path,
    adapter: str,
    window_backend: str,
    contract: dict[str, Any],
    formal: bool,
) -> dict[str, list[str]]:
    perf = ["python3", str(repo / "scripts/perf.py")]
    matrix = contract["formal_matrix"]
    selector = [
        "--workload",
        "indoor-light",
        "--contract",
        RTT_LIGHT_CONTRACT_ID,
        "--stage",
        RTT_LIGHT_STAGE,
        "--seed",
        str(matrix["seed"]),
        "--backend",
        matrix["backend"],
        "--present-mode",
        matrix["present_mode"],
        "--rtt-quality",
        matrix["window"]["rtt_quality"],
    ]
    repeat = matrix["repeat"]
    if formal:
        audit_warmup = matrix["audit"]["warmup_ticks"]
        audit_ticks = matrix["audit"]["audit_ticks"]
        warmup_secs = matrix["capture"]["warmup_secs"]
        measure_secs = matrix["capture"]["measure_secs"]
        preflight_runs = matrix["capture"]["preflight_runs"]
    else:
        audit_warmup = 129
        audit_ticks = 16
        warmup_secs = 3.0
        measure_secs = 5.0
        preflight_runs = 1
    audit = perf + [
        "audit",
        *selector,
        "--lane",
        "static",
        "--sizes",
        "small,medium,large" if formal else "small",
        "--renders",
        "cpu",
        "--repeat",
        str(repeat),
        "--preflight-runs",
        "0",
        "--window-backend",
        "headless",
        "--fixed-hz",
        str(matrix["fixed_hz"]),
        "--warmup-ticks",
        str(audit_warmup),
        "--audit-ticks",
        str(audit_ticks),
        "--output",
        str(output_root / "audit"),
    ]
    append_allow_patterns(audit, contract["allow_log_patterns"]["headless_audit"])
    behavior = perf + [
        "behavior",
        *selector,
        "--lane",
        "behavior",
        "--sizes",
        "small",
        "--renders",
        "cpu",
        "--behavior-cases",
        ",".join(contract["stages"][RTT_LIGHT_STAGE]["required_behavior_cases"]),
        "--repeat",
        str(repeat),
        "--preflight-runs",
        "0",
        "--window-backend",
        "headless",
        "--fixed-hz",
        str(matrix["fixed_hz"]),
        "--warmup-ticks",
        str(matrix["audit"]["warmup_ticks"]),
        "--audit-ticks",
        str(matrix["audit"]["audit_ticks"]),
        "--output",
        str(output_root / "behavior"),
    ]
    append_allow_patterns(behavior, contract["allow_log_patterns"]["headless_audit"])

    window = matrix["window"]

    def windowed(instrumentation: str) -> list[str]:
        leg = matrix[instrumentation]
        command = perf + [
            "run",
            *selector,
            "--lane",
            "static",
            "--sizes",
            "small,medium,large",
            "--renders",
            "cpu,gpu",
            "--instrumentation",
            instrumentation,
            "--repeat",
            str(repeat),
            "--preflight-runs",
            str(preflight_runs),
            "--warmup-secs",
            str(warmup_secs if not formal else leg["warmup_secs"]),
            "--measure-secs",
            str(measure_secs if not formal else leg["measure_secs"]),
            "--warmup-checksum-policy",
            "record",
            "--measure-end-checksum-policy",
            "record",
            "--adapter",
            adapter,
            "--window-backend",
            window_backend,
            "--window-width",
            str(window["physical_width"]),
            "--window-height",
            str(window["physical_height"]),
            "--window-scale-factor",
            str(window["scale_factor"]),
            "--environment-lock",
            str(environment_lock),
            "--output",
            str(output_root / instrumentation),
        ]
        append_allow_patterns(command, contract["allow_log_patterns"]["windowed"])
        return command

    commands = {
        "audit": audit,
        "capture": windowed("capture"),
        "memory": windowed("memory"),
    }
    if formal:
        commands["behavior"] = behavior
    return commands


def plan_task_dashboard(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    job_root = (
        Path(args.job_root).resolve()
        if args.job_root
        else unique_job_root(repo, "task-dashboard")
    )
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    failures = list(resources["failures"])
    storage_error = persistent_storage_error(
        job_root,
        label="native acceptance job root",
    )
    if storage_error:
        failures.append(storage_error)
    status = "ready" if not failures else "blocked"
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
        "status": status,
        "profile": "task-dashboard",
        "measurement_kind": (
            "formal" if args.warmup_secs >= 30 and args.measure_secs >= 60 else "acceptance-smoke"
        ),
        "job_root": str(job_root),
        "resources": {**resources, "status": status, "failures": failures},
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
    return 0 if status == "ready" else 1


def plan_deconstruction(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    job_root = (
        Path(args.job_root).resolve()
        if args.job_root
        else unique_job_root(repo, "building-deconstruction")
    )
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    failures = list(resources["failures"])
    storage_error = persistent_storage_error(
        job_root,
        label="native acceptance job root",
    )
    if storage_error:
        failures.append(storage_error)
    status = "ready" if not failures else "blocked"
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
        "run-deconstruction",
        "--repo",
        str(repo),
        "--job-root",
        str(job_root),
        "--seed",
        str(args.seed),
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
        "status": status,
        "profile": "building-deconstruction",
        "measurement_kind": "native-functional-acceptance",
        "job_root": str(job_root),
        "resources": {**resources, "status": status, "failures": failures},
        "launcher_command": command,
        "status_command": [
            "python3",
            str(Path(__file__).resolve()),
            "status",
            "--job-root",
            str(job_root),
        ],
        "execution_contract": {
            "game_processes": 1,
            "parallel_game_processes": 1,
            "actual_feature_builds": 1,
            "actual_window_required": True,
            "renderer_evidence_required": True,
            "in_game_screenshot_required": True,
            "synthetic_desktop_input": False,
            "automatic_cleanup": False,
        },
    }
    print_json(payload)
    return 0 if status == "ready" else 1


def plan_rtt_light(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    contract = rtt_light_contract(repo)
    subject_commit = git_subject(repo)
    fingerprint = source_fingerprint(repo)
    attempt_id = args.attempt_id or str(uuid.uuid4())
    state_root = (
        Path(args.job_root).resolve()
        if args.job_root
        else unique_job_root(repo, f"rtt-light-{args.level}")
    )
    failures = list(resources["failures"])
    state_root_error = persistent_storage_error(
        state_root,
        label="RtT-light state root",
    )
    if state_root_error:
        failures.append(state_root_error)
    tooling: dict[str, Any] | None = None
    if args.level == "formal":
        failures.extend(formal_contract_ready(contract))
        try:
            assert_clean_subject(repo, subject_commit)
            assert_prerequisite_ancestors(
                repo, subject_commit, args.prerequisite_commit
            )
            if args.s0_job_root is None or args.s1_job_root is None:
                raise AcceptanceError(
                    "formal RtT-light planning requires --s0-job-root and --s1-job-root"
                )
            verify_rtt_light_prerequisites(
                repo=repo,
                s0_job_root=Path(args.s0_job_root).resolve(),
                s1_job_root=Path(args.s1_job_root).resolve(),
                subject_commit=subject_commit,
                fingerprint=fingerprint,
                adapter=args.adapter,
                window_backend=args.window_backend,
            )
            attempt = rtt_light_attempt_path(
                repo, subject_commit=subject_commit, attempt_id=attempt_id
            )
            if attempt.exists():
                failures.append(f"attempt path already exists: {attempt}")
        except AcceptanceError as error:
            failures.append(str(error))
        tooling, tool_failures = inspect_renderdoc_tools(repo, args)
        failures.extend(tool_failures)
        output_root = rtt_light_attempt_path(
            repo, subject_commit=subject_commit, attempt_id=attempt_id
        )
    else:
        output_root = state_root / "artifacts"
    output_root_error = persistent_storage_error(
        output_root,
        label="RtT-light artifact root",
    )
    if output_root_error:
        failures.append(output_root_error)
    if state_root.exists():
        failures.append(f"state root already exists: {state_root}")
    launcher_command = [
        "kitty",
        "--directory",
        str(repo),
        "--detach",
        "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
        "PYTHONDONTWRITEBYTECODE=1",
        "python3",
        str(Path(__file__).resolve()),
        "run-rtt-light",
        "--repo",
        str(repo),
        "--level",
        args.level,
        "--state-root",
        str(state_root),
        "--attempt-id",
        attempt_id,
        "--subject-commit",
        subject_commit,
        "--source-fingerprint",
        fingerprint,
        "--adapter",
        args.adapter,
        "--window-backend",
        args.window_backend,
    ]
    for commit in args.prerequisite_commit:
        launcher_command.extend(["--prerequisite-commit", commit])
    if args.level == "formal" and args.s0_job_root and args.s1_job_root:
        launcher_command.extend(
            [
                "--s0-job-root",
                str(Path(args.s0_job_root).resolve()),
                "--s1-job-root",
                str(Path(args.s1_job_root).resolve()),
            ]
        )
    if args.level == "formal" and tooling is not None:
        launcher_command.extend(
            [
                "--renderdoccmd",
                tooling["paths"]["renderdoccmd"],
                "--qrenderdoc",
                tooling["paths"]["qrenderdoc"],
                "--renderdoc-library",
                tooling["paths"]["renderdoc_library"],
            ]
        )
    status = "ready" if not failures else "blocked"
    payload = {
        "schema_version": SCHEMA_VERSION,
        "status": status,
        "profile": "rtt-light",
        "measurement_kind": "formal" if args.level == "formal" else "s1-smoke",
        "level": args.level,
        "subject_commit": subject_commit,
        "source_fingerprint": fingerprint,
        "attempt_id": attempt_id,
        "output_root": str(output_root),
        "state_root": str(state_root),
        "prerequisite_commits": args.prerequisite_commit,
        "s0_job_root": (
            str(Path(args.s0_job_root).resolve()) if args.s0_job_root else None
        ),
        "s1_job_root": (
            str(Path(args.s1_job_root).resolve()) if args.s1_job_root else None
        ),
        "resources": resources,
        "failures": failures,
        "tooling": tooling,
        "launcher_command": launcher_command,
        "status_command": [
            "python3",
            str(Path(__file__).resolve()),
            "status",
            "--job-root",
            str(state_root),
        ],
        "execution_contract": {
            "leg_order": (
                list(RTT_LIGHT_LEGS)
                if args.level == "formal"
                else ["audit", "capture", "memory"]
            ),
            "game_processes": 64 if args.level == "formal" else 51,
            "parallel_game_processes": 1,
            "actual_feature_builds": 2,
            "uses_skip_build": False,
            "uses_binary_copy": False,
            "automatic_cleanup": False,
            "repository_lock_covers_registration": args.level == "formal",
            "settle_secs": RTT_LIGHT_SETTLE_SECS,
            "settle_after": (
                ["behavior", "renderdoc"]
                if args.level == "formal"
                else ["audit", "capture"]
            ),
        },
    }
    print_json(payload)
    return 0 if status == "ready" else 1


def update_state(job_file: Path, state: dict[str, Any], **changes: Any) -> None:
    state.update(changes)
    state["heartbeat_at"] = utc_now()
    atomic_write_json(job_file, state)


def process_group_exists(process_group: int) -> bool:
    try:
        os.killpg(process_group, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def signal_process_group(process: subprocess.Popen[str], signal_number: int) -> None:
    try:
        os.killpg(process.pid, signal_number)
    except ProcessLookupError:
        return
    except PermissionError:
        process.send_signal(signal_number)


def wait_for_process_group_exit(
    process: subprocess.Popen[str], *, timeout_seconds: float
) -> bool:
    deadline = time.monotonic() + timeout_seconds
    while process_group_exists(process.pid):
        process.poll()
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return False
        time.sleep(min(PROCESS_GROUP_POLL_SECONDS, remaining))
    process.poll()
    return True


def stop_command_process(
    process: subprocess.Popen[str],
    *,
    term_grace_seconds: float = PROCESS_GROUP_TERM_GRACE_SECONDS,
    kill_grace_seconds: float = PROCESS_GROUP_KILL_GRACE_SECONDS,
) -> None:
    """Stop a stage and every child it launched without touching other jobs."""
    signal_process_group(process, signal.SIGTERM)
    if not wait_for_process_group_exit(
        process,
        timeout_seconds=term_grace_seconds,
    ):
        signal_process_group(process, signal.SIGKILL)
        if not wait_for_process_group_exit(
            process,
            timeout_seconds=kill_grace_seconds,
        ):
            raise AcceptanceError(
                f"stage process group {process.pid} did not exit after SIGKILL"
            )
    if process.poll() is None:
        process.wait(timeout=kill_grace_seconds)


def run_command(
    stage: str,
    command: list[str],
    *,
    repo: Path,
    env: dict[str, str],
    log_path: Path,
    job_file: Path,
    state: dict[str, Any],
    timeout_seconds: float | None = None,
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
            start_new_session=True,
        )
        try:
            update_state(job_file, state, child_pid=process.pid)
            deadline = (
                time.monotonic() + timeout_seconds
                if timeout_seconds is not None
                else None
            )
            while process.poll() is None:
                runtime_failure = runtime_resource_failure()
                if runtime_failure is not None:
                    reason, evidence = runtime_failure
                    event = {
                        "at": utc_now(),
                        "stage": stage,
                        "child_pid": process.pid,
                        "reason": reason,
                        **evidence,
                    }
                    state.setdefault("resource_guard_events", []).append(event)
                    update_state(job_file, state, child_pid=process.pid)
                    log.write(
                        f"[{event['at']}] resource guard stopped stage={stage}: {reason}\n"
                    )
                    log.flush()
                    stop_command_process(process)
                    update_state(job_file, state, child_pid=None)
                    raise AcceptanceError(
                        f"stage {stage} stopped to preserve host resources: {reason}"
                    )
                if deadline is not None and time.monotonic() >= deadline:
                    stop_command_process(process)
                    update_state(job_file, state, child_pid=None)
                    raise AcceptanceError(
                        f"stage {stage} exceeded its {timeout_seconds:g}s timeout"
                    )
                update_state(job_file, state, child_pid=process.pid)
                time.sleep(RESOURCE_POLL_SECONDS)
            if process.returncode != 0:
                raise AcceptanceError(
                    f"stage {stage} failed with exit code {process.returncode}"
                )
            completed = state.setdefault("completed_stages", [])
            completed.append(stage)
            update_state(job_file, state, child_pid=None)
        finally:
            if process.poll() is None:
                cleanup_event = {
                    "at": utc_now(),
                    "stage": stage,
                    "child_pid": process.pid,
                    "reason": "stage interrupted before normal completion",
                }
                state.setdefault("cleanup_events", []).append(cleanup_event)
                log.write(
                    f"[{cleanup_event['at']}] stopping interrupted stage={stage} "
                    f"process_group={process.pid}\n"
                )
                log.flush()
                stop_command_process(process)
            if state.get("child_pid") == process.pid:
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


def session_binary_hash(session: Path) -> str:
    manifest = read_json(session / "manifest.json")
    value = manifest.get("binary", {}).get("sha256")
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise AcceptanceError(f"invalid binary hash in {session / 'manifest.json'}")
    return value


def renderdoc_capture_command(
    *,
    repo: Path,
    attempt: Path,
    environment_lock: Path,
    subject_commit: str,
    fingerprint: str,
    adapter: str,
    window_backend: str,
    tooling: dict[str, Any],
) -> list[str]:
    paths = tooling["paths"]
    job = tooling["job"]
    return [
        "python3",
        paths["capture_helper"],
        "capture",
        "--repo",
        str(repo),
        "--binary",
        str(repo / "target/profiling/bevy_app"),
        "--output",
        str(attempt / "renderdoc"),
        "--environment-lock",
        str(environment_lock),
        "--contract",
        RTT_LIGHT_CONTRACT_ID,
        "--stage",
        RTT_LIGHT_STAGE,
        "--adapter",
        adapter,
        "--window-backend",
        window_backend,
        "--subject-commit",
        subject_commit,
        "--source-fingerprint",
        fingerprint,
        "--renderdoccmd",
        paths["renderdoccmd"],
        "--qrenderdoc",
        paths["qrenderdoc"],
        "--renderdoc-library",
        paths["renderdoc_library"],
        "--renderdoc-version",
        job["renderdoc_version"],
        "--qrenderdoc-version",
        job["qrenderdoc_version"],
    ]


def verify_rtt_light_smoke(
    *, audit: Path, capture: Path, memory: Path, adapter: str, window_backend: str
) -> dict[str, Any]:
    manifests = {
        name: read_json(path / "manifest.json")
        for name, path in (
            ("audit", audit),
            ("capture", capture),
            ("memory", memory),
        )
    }
    observed_game_processes = 0
    for name, manifest in manifests.items():
        require(manifest.get("status") == "valid", f"{name} smoke session is invalid")
        matrix = manifest.get("matrix", {})
        require(
            matrix.get("workload") == "indoor-light",
            f"{name} smoke session has the wrong workload",
        )
        require(matrix.get("repeat") == 3, f"{name} smoke session has wrong repeat")
        cases = manifest.get("cases")
        require(isinstance(cases, list), f"{name} S1 cases are invalid")
        preflight_runs = matrix.get("preflight_runs")
        require(
            isinstance(preflight_runs, int) and preflight_runs >= 0,
            f"{name} S1 preflight count is invalid",
        )
        observed_game_processes += len(cases) * (matrix["repeat"] + preflight_runs)
    require(
        {
            key: manifests["audit"].get("matrix", {}).get(key)
            for key in (
                "sizes",
                "renders",
                "repeat",
                "preflight_runs",
                "fixed_hz",
                "warmup_ticks",
                "audit_ticks",
                "capture_kind",
                "clock_mode",
            )
        }
        == {
            "sizes": ["small"],
            "renders": ["cpu"],
            "repeat": 3,
            "preflight_runs": 0,
            "fixed_hz": 64,
            "warmup_ticks": 129,
            "audit_ticks": 16,
            "capture_kind": "fixed-step-determinism",
            "clock_mode": "fixed",
        },
        "S1 audit matrix differs from the exact smoke contract",
    )
    for name in ("capture", "memory"):
        manifest = manifests[name]
        matrix = manifest.get("matrix", {})
        require(
            {
                key: matrix.get(key)
                for key in (
                    "sizes",
                    "renders",
                    "repeat",
                    "preflight_runs",
                    "warmup_secs",
                    "measure_secs",
                    "capture_kind",
                    "clock_mode",
                )
            }
            == {
                "sizes": ["small", "medium", "large"],
                "renders": ["cpu", "gpu"],
                "repeat": 3,
                "preflight_runs": 1,
                "warmup_secs": 3.0,
                "measure_secs": 5.0,
                "capture_kind": "frame-time",
                "clock_mode": "realtime",
            },
            f"{name} S1 matrix differs from the exact smoke contract",
        )
        require(
            manifest.get("requested_environment", {}).get("HW_WINDOW_BACKEND")
            == window_backend,
            f"{name} S1 window backend differs",
        )
        require(
            any(
                adapter.casefold() in str(value.get("name", "")).casefold()
                for value in manifest.get("actual_adapters", [])
            ),
            f"{name} S1 adapter differs",
        )
    require(
        observed_game_processes == 51,
        f"S1 game process count is {observed_game_processes}, expected 51",
    )
    capture_hash = session_binary_hash(capture)
    require(
        session_binary_hash(audit) == capture_hash,
        "S1 audit and Capture binary hashes differ",
    )
    require(
        session_binary_hash(memory) != capture_hash,
        "S1 Capture and Memory binary hashes match",
    )
    return {
        "status": "pass",
        "audit": str(audit),
        "capture": str(capture),
        "memory": str(memory),
        "capture_binary_sha256": capture_hash,
        "memory_binary_sha256": session_binary_hash(memory),
    }


def verify_rtt_light_prerequisites(
    *,
    repo: Path,
    s0_job_root: Path,
    s1_job_root: Path,
    subject_commit: str,
    fingerprint: str,
    adapter: str,
    window_backend: str,
) -> dict[str, Any]:
    for root, label in ((s0_job_root, "S0"), (s1_job_root, "S1")):
        if not root.is_dir() or root.is_symlink():
            raise AcceptanceError(f"{label} job root is missing or symlinked: {root}")
    s0 = read_json(s0_job_root / "job.json")
    if (
        s0.get("profile") != "task-dashboard"
        or s0.get("status") != "valid"
        or s0.get("repo") != str(repo)
        or s0.get("source_fingerprint") != fingerprint
        or s0.get("verification", {}).get("status") != "pass"
    ):
        raise AcceptanceError("S0 state is not a valid same-source prerequisite")
    s0_paths = s0.get("paths", {})
    s0_result = verify_artifact_set(
        Path(s0_paths.get("audit", "")),
        Path(s0_paths.get("capture", "")),
        Path(s0_paths.get("memory", "")),
        adapter=adapter,
        backend="vulkan",
        window_backend=window_backend,
        min_runs=3,
    )
    for name in ("audit", "capture", "memory"):
        manifest = read_json(Path(s0_paths[name]) / "manifest.json")
        if manifest.get("git", {}).get("commit") != subject_commit:
            raise AcceptanceError(f"S0 {name} was not produced from the subject commit")

    s1 = read_json(s1_job_root / "job.json")
    if (
        s1.get("profile") != "rtt-light"
        or s1.get("measurement_kind") != "s1-smoke"
        or s1.get("status") != "valid"
        or s1.get("repo") != str(repo)
        or s1.get("subject_commit") != subject_commit
        or s1.get("source_fingerprint") != fingerprint
        or s1.get("verification", {}).get("status") != "pass"
    ):
        raise AcceptanceError("S1 state is not a valid same-source prerequisite")
    s1_attempt = Path(s1.get("paths", {}).get("attempt", ""))
    s1_result = verify_rtt_light_smoke(
        audit=s1_attempt / "audit",
        capture=s1_attempt / "capture",
        memory=s1_attempt / "memory",
        adapter=adapter,
        window_backend=window_backend,
    )
    return {
        "status": "pass",
        "subject_commit": subject_commit,
        "source_fingerprint": fingerprint,
        "s0": {"job_root": str(s0_job_root), "verification": s0_result},
        "s1": {"job_root": str(s1_job_root), "verification": s1_result},
    }


@activity_locked
def run_rtt_light(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-rtt-light must be launched by the planned direct kitty command"
        )
    repo = validate_repo(args.repo)
    contract = rtt_light_contract(repo)
    subject_commit = args.subject_commit
    if git_subject(repo) != subject_commit:
        raise AcceptanceError("planned RtT-light subject commit changed before launch")
    fingerprint = source_fingerprint(repo)
    if fingerprint != args.source_fingerprint:
        raise AcceptanceError("planned RtT-light source fingerprint changed before launch")
    state_root = Path(args.state_root).resolve()
    require_persistent_storage(state_root, label="RtT-light state root")
    if state_root.exists():
        raise AcceptanceError(f"state root already exists: {state_root}")
    state_root.mkdir(parents=True)
    state_file = state_root / "job.json"
    resources = resource_snapshot(repo, require_launcher=False)
    formal = args.level == "formal"
    attempt = (
        rtt_light_attempt_path(
            repo, subject_commit=subject_commit, attempt_id=args.attempt_id
        )
        if formal
        else state_root / "artifacts"
    )
    require_persistent_storage(attempt, label="RtT-light artifact root")
    environment_lock = (
        attempt.parent.parent / "environment-lock.json"
        if formal
        else attempt / "environment-lock.json"
    )
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "profile": "rtt-light",
        "measurement_kind": "formal" if formal else "s1-smoke",
        "status": "running",
        "started_at": utc_now(),
        "heartbeat_at": utc_now(),
        "pid": os.getpid(),
        "repo": str(repo),
        "job_root": str(state_root),
        "attempt_id": args.attempt_id,
        "subject_commit": subject_commit,
        "source_fingerprint": fingerprint,
        "completed_stages": [],
        "execution_contract": {
            "settle_secs": RTT_LIGHT_SETTLE_SECS,
            "settle_after": (
                ["behavior", "renderdoc"] if formal else ["audit", "capture"]
            ),
        },
        "paths": {
            "attempt": str(attempt),
            "environment_lock": str(environment_lock),
        },
    }
    atomic_write_json(state_file, state)
    if resources["status"] != "ready":
        update_state(
            state_file,
            state,
            status="invalid",
            error="; ".join(resources["failures"]),
            finished_at=utc_now(),
        )
        return 1
    tooling: dict[str, Any] | None = None
    if formal:
        try:
            readiness = formal_contract_ready(contract)
            if readiness:
                raise AcceptanceError("; ".join(readiness))
            assert_clean_subject(repo, subject_commit)
            assert_prerequisite_ancestors(
                repo, subject_commit, args.prerequisite_commit
            )
            verify_rtt_light_prerequisites(
                repo=repo,
                s0_job_root=Path(args.s0_job_root).resolve(),
                s1_job_root=Path(args.s1_job_root).resolve(),
                subject_commit=subject_commit,
                fingerprint=fingerprint,
                adapter=args.adapter,
                window_backend=args.window_backend,
            )
            tooling, failures = inspect_renderdoc_tools(repo, args)
            if failures or tooling is None:
                raise AcceptanceError("; ".join(failures or ["RenderDoc tooling is unavailable"]))
        except Exception as error:
            update_state(
                state_file,
                state,
                status="invalid",
                error=str(error),
                finished_at=utc_now(),
            )
            return 1

    env = cargo_environment(repo)
    env.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "CARGO_BUILD_JOBS": str(resources["cargo_jobs"]),
            "CARGO_INCREMENTAL": "0",
        }
    )
    commands = rtt_light_session_commands(
        repo=repo,
        output_root=attempt,
        environment_lock=environment_lock,
        adapter=args.adapter,
        window_backend=args.window_backend,
        contract=contract,
        formal=formal,
    )
    LOCK_PATH.touch(exist_ok=True)
    with LOCK_PATH.open("r+", encoding="utf-8") as lock:
        try:
            fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            update_state(
                state_file,
                state,
                status="invalid",
                error=f"another native acceptance job holds {LOCK_PATH}",
                finished_at=utc_now(),
            )
            return 1
        try:
            if attempt.exists():
                raise AcceptanceError(f"RtT-light output already exists: {attempt}")
            attempt.mkdir(parents=True)
            log_path = attempt / "orchestrator.log"
            source_checks: list[dict[str, Any]] = []
            if formal:
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="start",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                    )
                )

            run_command(
                "audit",
                commands["audit"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=state_file,
                state=state,
            )
            assert_source_unchanged(repo, fingerprint)
            capture_hash = manifest_binary_hash(attempt / "audit", repo)
            if formal:
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-audit",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                    )
                )
                run_command(
                    "behavior",
                    commands["behavior"],
                    repo=repo,
                    env=env,
                    log_path=log_path,
                    job_file=state_file,
                    state=state,
                )
                if manifest_binary_hash(attempt / "behavior", repo) != capture_hash:
                    raise AcceptanceError("audit and behavior binary hashes differ")
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-behavior",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                    )
                )
                settle(
                    RTT_LIGHT_SETTLE_SECS,
                    job_file=state_file,
                    state=state,
                    after_stage="behavior",
                )
            else:
                settle(
                    RTT_LIGHT_SETTLE_SECS,
                    job_file=state_file,
                    state=state,
                    after_stage="audit",
                )

            run_command(
                "capture",
                commands["capture"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=state_file,
                state=state,
            )
            if manifest_binary_hash(attempt / "capture", repo) != capture_hash:
                raise AcceptanceError("audit and Capture binary hashes differ")
            assert_source_unchanged(repo, fingerprint)
            if formal:
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-capture",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                    )
                )
                if tooling is None:
                    raise AcceptanceError("formal RenderDoc tooling disappeared")
                run_command(
                    "renderdoc",
                    renderdoc_capture_command(
                        repo=repo,
                        attempt=attempt,
                        environment_lock=environment_lock,
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        adapter=args.adapter,
                        window_backend=args.window_backend,
                        tooling=tooling,
                    ),
                    repo=repo,
                    env=env,
                    log_path=log_path,
                    job_file=state_file,
                    state=state,
                )
                renderdoc_manifest = read_json(attempt / "renderdoc/manifest.json")
                if renderdoc_manifest.get("binary", {}).get("sha256") != capture_hash:
                    raise AcceptanceError("RenderDoc did not use the Capture binary")
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-renderdoc",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                    )
                )
                settle(
                    RTT_LIGHT_SETTLE_SECS,
                    job_file=state_file,
                    state=state,
                    after_stage="renderdoc",
                )
            else:
                settle(
                    RTT_LIGHT_SETTLE_SECS,
                    job_file=state_file,
                    state=state,
                    after_stage="capture",
                )

            run_command(
                "memory",
                commands["memory"],
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=state_file,
                state=state,
            )
            memory_hash = manifest_binary_hash(attempt / "memory", repo)
            if memory_hash == capture_hash:
                raise AcceptanceError("Capture and Memory binary hashes match")
            assert_source_unchanged(repo, fingerprint)

            if not formal:
                verification = verify_rtt_light_smoke(
                    audit=attempt / "audit",
                    capture=attempt / "capture",
                    memory=attempt / "memory",
                    adapter=args.adapter,
                    window_backend=args.window_backend,
                )
                update_state(
                    state_file,
                    state,
                    status="valid",
                    current_stage=None,
                    child_pid=None,
                    verification=verification,
                    finished_at=utc_now(),
                )
                return 0

            source_checks.append(
                source_checkpoint(
                    repo,
                    checkpoint="after-memory",
                    subject_commit=subject_commit,
                    fingerprint=fingerprint,
                )
            )
            source_checks.append(
                source_checkpoint(
                    repo,
                    checkpoint="before-registration",
                    subject_commit=subject_commit,
                    fingerprint=fingerprint,
                )
            )
            refreshed_tooling, failures = inspect_renderdoc_tools(repo, args)
            if failures or refreshed_tooling != tooling:
                raise AcceptanceError(
                    "formal tooling changed before registration: "
                    + "; ".join(failures or ["tool hash drift"])
                )
            formal_job = {
                "schema_version": SCHEMA_VERSION,
                "profile": "rtt-light",
                "measurement_kind": "formal",
                "contract_id": RTT_LIGHT_CONTRACT_ID,
                "stage_id": RTT_LIGHT_STAGE,
                "attempt_id": args.attempt_id,
                "subject_commit": subject_commit,
                "prerequisite_commits": args.prerequisite_commit,
                "adapter_filter": args.adapter,
                "window_backend": args.window_backend,
                "leg_order": list(RTT_LIGHT_LEGS),
                "completed_legs": list(RTT_LIGHT_LEGS),
                "source_checks": source_checks,
                "tooling": tooling["job"],
                "status": "completed",
            }
            atomic_write_json(attempt / "job.json", formal_job)
            sys.path.insert(0, str(repo / "scripts"))
            from perf_tool.rtt_light_bundle import finalize_attempt

            manifest = finalize_attempt(attempt)
            update_state(
                state_file,
                state,
                status="valid",
                current_stage=None,
                child_pid=None,
                verification={
                    "status": "pass",
                    "attempt_manifest": manifest,
                },
                finished_at=utc_now(),
            )
            return 0
        except Exception as error:
            update_state(
                state_file,
                state,
                status="invalid",
                current_stage=None,
                child_pid=None,
                error=str(error),
                finished_at=utc_now(),
            )
            return 1


@activity_locked
def run_task_dashboard(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-task-dashboard must be launched by the planned direct kitty command"
    )
    repo = validate_repo(args.repo)
    job_root = Path(args.job_root).resolve()
    require_persistent_storage(job_root, label="native acceptance job root")
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

    env = cargo_environment(repo)
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


@activity_locked
def run_deconstruction(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-deconstruction must be launched by the planned direct kitty command"
    )
    repo = validate_repo(args.repo)
    job_root = Path(args.job_root).resolve()
    require_persistent_storage(job_root, label="native acceptance job root")
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    job_root.mkdir(parents=True)
    artifact = job_root / "artifact"
    artifact.mkdir()
    job_file = job_root / "job.json"
    log_path = job_root / "orchestrator.log"
    resources = resource_snapshot(repo, require_launcher=False)
    source = source_fingerprint(repo)
    run_id = f"c1-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}-{secrets.token_hex(4)}"
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "profile": "building-deconstruction",
        "status": "running",
        "started_at": utc_now(),
        "heartbeat_at": utc_now(),
        "pid": os.getpid(),
        "repo": str(repo),
        "job_root": str(job_root),
        "run_id": run_id,
        "source_fingerprint": source,
        "resources": resources,
        "completed_stages": [],
        "paths": {
            "artifact": str(artifact),
            "driver_result": str(artifact / "driver-result.json"),
            "screenshot": str(artifact / "deconstruction-v1-v5.png"),
            "save": str(artifact / "runtime/saves/world.scn.ron"),
            "log": str(log_path),
        },
        "parameters": {
            "seed": args.seed,
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

    env = cargo_environment(repo)
    for key in list(env):
        if key.startswith("HW_PERF_") or key in {
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT",
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID",
        }:
            env.pop(key, None)
    env = cargo_environment(repo, env)
    env.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "CARGO_BUILD_JOBS": str(resources["cargo_jobs"]),
            "CARGO_INCREMENTAL": "0",
            "HELL_WORKERS_WORLDGEN_SEED": str(args.seed),
            "HW_WINDOW_BACKEND": args.window_backend,
            "WGPU_BACKEND": args.backend,
            "HW_PRESENT_MODE": args.present_mode,
            "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_ARTIFACT": str(artifact),
            "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_RUN_ID": run_id,
        }
    )
    command = ["cargo", "run", "--locked", "-p", "bevy_app@0.1.0"]
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
                "native-deconstruction",
                command,
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
                timeout_seconds=360,
            )
            assert_source_unchanged(repo, source)
            verification = verify_deconstruction_artifact(
                artifact,
                run_id=run_id,
                adapter=args.adapter,
                backend=args.backend,
                window_backend=args.window_backend,
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


def verify_deconstruction_artifact(
    artifact: Path,
    *,
    run_id: str,
    adapter: str,
    backend: str,
    window_backend: str,
) -> dict[str, Any]:
    artifact = artifact.resolve()
    result_path = artifact / "driver-result.json"
    screenshot_path = artifact / "deconstruction-v1-v5.png"
    save_path = artifact / "runtime/saves/world.scn.ron"
    result = read_json(result_path)
    require(result.get("status") == "PASS", f"driver did not pass: {result_path}")
    require(
        result.get("profile") == "building-deconstruction",
        f"wrong native profile in {result_path}",
    )
    actual_run_id = result.get("run_id")
    require(isinstance(actual_run_id, str) and actual_run_id, "driver run_id is missing")
    require(run_id, "verification requires the launched run_id")
    require(actual_run_id == run_id, "driver run_id does not match the launched job")
    checks = result.get("checks")
    require(isinstance(checks, dict), "driver checks are missing")
    require(set(checks) == DECONSTRUCTION_CHECKS, "driver V1-V5 check set is incomplete")
    require(
        all(checks.get(check) == "PASS" for check in DECONSTRUCTION_CHECKS),
        "one or more driver V1-V5 checks did not pass",
    )
    before_epoch = result.get("world_epoch_before_load")
    after_epoch = result.get("world_epoch_after_load")
    require(
        isinstance(before_epoch, int)
        and isinstance(after_epoch, int)
        and after_epoch == before_epoch + 1,
        "WorldEpoch did not advance exactly once",
    )

    save = result.get("save")
    require(isinstance(save, dict), "save evidence is missing")
    require(save_path.is_file(), f"native save artifact is missing: {save_path}")
    require(
        Path(str(save.get("path"))).resolve() == save_path,
        "driver save path does not match the artifact contract",
    )
    save_bytes = save_path.stat().st_size
    require(save_bytes > 0, "native save artifact is empty")
    require(save.get("bytes") == save_bytes, "driver save byte count is stale")

    screenshot = result.get("screenshot")
    require(isinstance(screenshot, dict), "screenshot evidence is missing")
    require(screenshot_path.is_file(), f"native screenshot is missing: {screenshot_path}")
    require(
        Path(str(screenshot.get("path"))).resolve() == screenshot_path,
        "driver screenshot path does not match the artifact contract",
    )
    screenshot_bytes = screenshot_path.stat().st_size
    require(
        0 < screenshot_bytes <= MAX_NATIVE_SCREENSHOT_BYTES,
        "native screenshot size is outside the fail-closed limit",
    )
    png = screenshot_path.read_bytes()
    width, height = validate_png_structure(png)
    require(width >= 640 and height >= 360, f"screenshot is too small: {width}x{height}")
    require(screenshot.get("width") == width, "driver screenshot width is stale")
    require(screenshot.get("height") == height, "driver screenshot height is stale")
    require(screenshot.get("bytes") == screenshot_bytes, "driver screenshot byte count is stale")

    renderer = result.get("renderer")
    require(isinstance(renderer, dict), "renderer evidence is missing")
    adapter_name = str(renderer.get("adapter_name", ""))
    actual_backend = str(renderer.get("backend", ""))
    display_handle = str(renderer.get("display_handle", ""))
    require(adapter_name, "renderer adapter name is empty")
    if adapter:
        require(
            adapter.lower() in adapter_name.lower(),
            f"actual adapter does not contain {adapter!r}: {adapter_name}",
        )
    require(
        actual_backend.lower() == backend.lower(),
        f"actual renderer backend is {actual_backend!r}, expected {backend!r}",
    )
    display_lower = display_handle.lower()
    if window_backend == "x11":
        require(
            "xlib" in display_lower or "xcb" in display_lower,
            f"actual display handle is not X11: {display_handle}",
        )
    else:
        require("wayland" in display_lower, f"actual display handle is not Wayland: {display_handle}")

    return {
        "status": "pass",
        "profile": "building-deconstruction",
        "run_id": actual_run_id,
        "checks": checks,
        "world_epoch_before_load": before_epoch,
        "world_epoch_after_load": after_epoch,
        "save": {"path": str(save_path), "bytes": save_bytes, "sha256": sha256(save_path)},
        "screenshot": {
            "path": str(screenshot_path),
            "width": width,
            "height": height,
            "bytes": len(png),
            "sha256": sha256(screenshot_path),
        },
        "renderer": {
            "adapter_name": adapter_name,
            "backend": actual_backend,
            "display_handle": display_handle,
        },
    }


def validate_png_structure(png: bytes) -> tuple[int, int]:
    signature = b"\x89PNG\r\n\x1a\n"
    require(
        0 < len(png) <= MAX_NATIVE_SCREENSHOT_BYTES,
        "PNG size is outside the fail-closed limit",
    )
    require(png.startswith(signature), "invalid PNG signature")
    offset = len(signature)
    dimensions: tuple[int, int] | None = None
    saw_idat = False
    while offset < len(png):
        require(offset + 8 <= len(png), "truncated PNG chunk header")
        data_len = int.from_bytes(png[offset : offset + 4], "big")
        chunk_type = png[offset + 4 : offset + 8]
        data_end = offset + 8 + data_len
        chunk_end = data_end + 4
        require(chunk_end <= len(png), "truncated PNG chunk")
        expected_crc = int.from_bytes(png[data_end:chunk_end], "big")
        actual_crc = zlib.crc32(png[offset + 4 : data_end]) & 0xFFFFFFFF
        require(actual_crc == expected_crc, "corrupt PNG chunk CRC")
        if chunk_type == b"IHDR":
            require(
                offset == len(signature) and data_len == 13 and dimensions is None,
                "invalid PNG IHDR",
            )
            width = int.from_bytes(png[offset + 8 : offset + 12], "big")
            height = int.from_bytes(png[offset + 12 : offset + 16], "big")
            require(width > 0 and height > 0, "PNG dimensions must be non-zero")
            dimensions = (width, height)
        elif chunk_type == b"IDAT":
            saw_idat = True
        elif chunk_type == b"IEND":
            require(
                data_len == 0
                and dimensions is not None
                and saw_idat
                and chunk_end == len(png),
                "invalid terminal PNG IEND",
            )
            return dimensions
        else:
            require(dimensions is not None, "PNG does not start with IHDR")
        offset = chunk_end
    raise AcceptanceError("PNG is missing terminal IEND")


def png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    require(len(chunk_type) == 4, "PNG chunk type must have four bytes")
    payload = chunk_type + data
    return (
        len(data).to_bytes(4, "big")
        + payload
        + (zlib.crc32(payload) & 0xFFFFFFFF).to_bytes(4, "big")
    )


def structural_png(width: int, height: int) -> bytes:
    ihdr = (
        width.to_bytes(4, "big")
        + height.to_bytes(4, "big")
        + bytes((8, 6, 0, 0, 0))
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", b"\x78\x9c\x03\x00\x00\x00\x00\x01")
        + png_chunk(b"IEND", b"")
    )


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


def verify_rtt_light_command(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    sys.path.insert(0, str(repo / "scripts"))
    from perf_tool.rtt_light_bundle import verify_attempt

    manifest = verify_attempt(Path(args.attempt).resolve())
    print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": "rtt-light",
            "attempt": str(Path(args.attempt).resolve()),
            "attempt_manifest": manifest,
        }
    )
    return 0


def verify_deconstruction_command(args: argparse.Namespace) -> int:
    result = verify_deconstruction_artifact(
        Path(args.artifact).resolve(),
        run_id=args.run_id,
        adapter=args.adapter,
        backend=args.backend,
        window_backend=args.window_backend,
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
    if status == "valid" and state.get("profile") == "building-deconstruction":
        parameters = state.get("parameters", {})
        try:
            summary["verification"] = verify_deconstruction_artifact(
                Path(state["paths"]["artifact"]),
                run_id=str(state["run_id"]),
                adapter=str(parameters.get("adapter", "Intel")),
                backend=str(parameters.get("backend", "vulkan")),
                window_backend=str(parameters.get("window_backend", "x11")),
            )
        except (AcceptanceError, KeyError, TypeError, ValueError) as error:
            summary["status"] = "invalid"
            summary["error"] = f"artifact revalidation failed: {error}"
            print_json(summary)
            return 1
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


def write_fake_rtt_light_smoke(
    path: Path,
    *,
    leg: str,
    binary_hash: str,
    window_backend: str,
) -> None:
    path.mkdir(parents=True)
    fixed = leg == "audit"
    case_count = 1 if fixed else 6
    matrix = {
        "workload": "indoor-light",
        "sizes": ["small"] if fixed else ["small", "medium", "large"],
        "renders": ["cpu"] if fixed else ["cpu", "gpu"],
        "repeat": 3,
        "preflight_runs": 0 if fixed else 1,
        "capture_kind": "fixed-step-determinism" if fixed else "frame-time",
        "clock_mode": "fixed" if fixed else "realtime",
    }
    if fixed:
        matrix.update({"fixed_hz": 64, "warmup_ticks": 129, "audit_ticks": 16})
    else:
        matrix.update({"warmup_secs": 3.0, "measure_secs": 5.0})
    atomic_write_json(
        path / "manifest.json",
        {
            "status": "valid",
            "binary": {"sha256": binary_hash},
            "matrix": matrix,
            "requested_environment": {"HW_WINDOW_BACKEND": window_backend},
            "actual_adapters": (
                []
                if fixed
                else [{"name": "Intel(R) Arc Graphics", "backend": "Vulkan"}]
            ),
            "cases": [{"id": f"{leg}-{index}"} for index in range(case_count)],
        },
    )


def self_test() -> int:
    repo = Path(__file__).resolve().parents[4]
    sys.path.insert(0, str(repo / "scripts"))
    from perf_tool import execution as perf_execution

    require(
        SOURCE_FILES == perf_execution.SOURCE_FINGERPRINT_FILES
        and SOURCE_PREFIXES == perf_execution.SOURCE_FINGERPRINT_PREFIXES
        and ASSET_PREFIX == perf_execution.SOURCE_FINGERPRINT_ASSET_PREFIX,
        "native and perf source fingerprint boundaries differ",
    )
    require(
        source_fingerprint(repo) == perf_execution.source_fingerprint(),
        "native and perf source fingerprints differ",
    )
    temporary_root = workspace_process_temp_dir(repo)
    temporary_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="native-acceptance-self-test-",
        dir=temporary_root,
    ) as temporary:
        root = Path(temporary)
        workspace = root / "workspace"
        tmp_root = root / "tmp"
        workspace.mkdir()
        tmp_root.mkdir()
        mountinfo = root / "mountinfo"
        mountinfo.write_text(
            f"36 25 0:32 / {workspace} rw - ext4 /dev/fake rw\n"
            f"37 36 0:33 / {tmp_root} rw - tmpfs tmpfs rw\n",
            encoding="utf-8",
        )
        require(
            filesystem_type(workspace_cargo_target(workspace), mountinfo_path=mountinfo)
            == "ext4",
            "workspace target filesystem detection failed",
        )
        require(
            filesystem_type(tmp_root / "target", mountinfo_path=mountinfo) == "tmpfs",
            "tmpfs target filesystem detection failed",
        )
        require(
            decode_mount_path("/tmp/with\\040space") == "/tmp/with space",
            "mount path decoding failed",
        )
        require(
            persistent_storage_error(
                workspace_cargo_target(workspace),
                label="workspace Cargo target",
                mountinfo_path=mountinfo,
                temporary_root=tmp_root,
            )
            is None,
            "persistent workspace target was rejected",
        )
        require(
            persistent_storage_error(
                tmp_root / "unsafe-target",
                label="workspace Cargo target",
                mountinfo_path=mountinfo,
                temporary_root=tmp_root,
            )
            is not None,
            "tmpfs target was not rejected",
        )
        guarded_env = cargo_environment(
            workspace,
            {
                "CARGO_TARGET_DIR": "/tmp/hell-workers-self-test-target",
                "CARGO_HOME": "/tmp/hell-workers-self-test-cargo-home",
                "RUSTUP_HOME": "/tmp/hell-workers-self-test-rustup-home",
            },
        )
        require(
            guarded_env["CARGO_TARGET_DIR"] == str(workspace_cargo_target(workspace))
            and guarded_env["CARGO_INCREMENTAL"] == "0"
            and guarded_env["CARGO_BUILD_JOBS"] == "1"
            and guarded_env["TMPDIR"] == str(workspace_process_temp_dir(workspace))
            and guarded_env["TMP"] == guarded_env["TMPDIR"]
            and guarded_env["TEMP"] == guarded_env["TMPDIR"]
            and not path_is_within(Path(guarded_env["CARGO_HOME"]), TMP_ROOT)
            and not path_is_within(Path(guarded_env["RUSTUP_HOME"]), TMP_ROOT),
            "Cargo target environment was not normalized to the workspace",
        )
        require(
            not path_is_within(unique_job_root(workspace, "self-test"), tmp_root),
            "native job root is still placed under tmpfs",
        )
        stale_target = tmp_root / "hell-workers-self-test-target"
        stale_target.mkdir()
        (stale_target / ".rustc_info.json").write_text("{}\n", encoding="utf-8")
        stale_targets = legacy_tmp_cargo_targets(tmp_root)
        require(
            len(stale_targets) == 1
            and stale_targets[0]["path"] == str(stale_target.resolve())
            and stale_targets[0]["bytes"] > 0,
            "temporary Cargo target inventory failed",
        )
        healthy_memory = {
            "mem_available_bytes": 10 * GIB,
            "swap_total_bytes": 8 * GIB,
            "swap_free_bytes": 2 * GIB,
        }
        require(
            not memory_safety_failures(
                healthy_memory,
                minimum_memory_gib=MIN_START_MEMORY_GIB,
                phase="start",
            ),
            "healthy native start resources were rejected",
        )
        depleted_swap = {
            "mem_available_bytes": 12 * GIB,
            "swap_total_bytes": 8 * GIB,
            "swap_free_bytes": 0,
        }
        require(
            not memory_safety_failures(
                depleted_swap,
                minimum_memory_gib=MIN_START_MEMORY_GIB,
                phase="start",
            ),
            "depleted swap blocked a native start despite healthy RAM",
        )
        low_runtime_memory = {
            "mem_available_bytes": (MIN_RUNTIME_MEMORY_GIB - 1) * GIB,
            "swap_total_bytes": 0,
            "swap_free_bytes": 0,
        }
        require(
            any(
                "MemAvailable" in failure
                for failure in memory_safety_failures(
                    low_runtime_memory,
                    minimum_memory_gib=MIN_RUNTIME_MEMORY_GIB,
                    phase="runtime",
                )
            ),
            "low runtime memory did not block a native stage",
        )
        unknown_swap = {
            "mem_available_bytes": 12 * GIB,
            "swap_total_bytes": None,
            "swap_free_bytes": None,
        }
        require(
            not memory_safety_failures(
                unknown_swap,
                minimum_memory_gib=MIN_START_MEMORY_GIB,
                phase="start",
            ),
            "missing swap counters blocked a native start despite healthy RAM",
        )
        original_meminfo_bytes = meminfo_bytes
        try:
            globals()["meminfo_bytes"] = lambda: {}
            unavailable_monitor = runtime_resource_failure()
        finally:
            globals()["meminfo_bytes"] = original_meminfo_bytes
        require(
            unavailable_monitor is not None
            and "resource monitoring failed" in unavailable_monitor[0],
            "unavailable runtime resource monitor did not fail closed",
        )

        resource_guard_job = root / "resource-guard-job.json"
        resource_guard_log = root / "resource-guard.log"
        resource_guard_state: dict[str, Any] = {
            "status": "running",
            "completed_stages": [],
        }
        original_runtime_resource_failure = runtime_resource_failure
        try:
            globals()["runtime_resource_failure"] = lambda: (
                "forced resource guard failure",
                {
                    "mem_available_gib": 7.0,
                    "swap_total_gib": 8.0,
                    "swap_free_gib": 0.0,
                },
            )
            try:
                run_command(
                    "resource-guard-self-test",
                    [sys.executable, "-c", "import time; time.sleep(60)"],
                    repo=workspace,
                    env=guarded_env,
                    log_path=resource_guard_log,
                    job_file=resource_guard_job,
                    state=resource_guard_state,
                )
            except AcceptanceError as error:
                require(
                    "stopped to preserve host resources" in str(error),
                    "resource guard returned the wrong failure",
                )
            else:
                raise AcceptanceError("resource guard did not stop its stage")
        finally:
            globals()["runtime_resource_failure"] = original_runtime_resource_failure
        resource_guard_events = resource_guard_state.get("resource_guard_events", [])
        require(
            len(resource_guard_events) == 1
            and resource_guard_state.get("child_pid") is None
            and not process_group_exists(resource_guard_events[0]["child_pid"]),
            "resource guard left a stage process behind",
        )

        ignore_term_child = (
            "import signal, time; "
            "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
            "time.sleep(60)"
        )
        ignore_term_parent = (
            "import signal, subprocess, sys, time; "
            "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
            f"child = subprocess.Popen([sys.executable, '-c', {ignore_term_child!r}]); "
            "print(child.pid, flush=True); time.sleep(60)"
        )
        ignored_term_process = subprocess.Popen(
            [sys.executable, "-c", ignore_term_parent],
            cwd=workspace,
            env=guarded_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        try:
            require(
                bool(ignored_term_process.stdout and ignored_term_process.stdout.readline()),
                "SIGTERM-ignoring child did not start",
            )
            stop_command_process(
                ignored_term_process,
                term_grace_seconds=0.1,
                kill_grace_seconds=2,
            )
            require(
                not process_group_exists(ignored_term_process.pid),
                "SIGKILL did not clear the stage process group",
            )
        finally:
            if ignored_term_process.poll() is None:
                stop_command_process(
                    ignored_term_process,
                    term_grace_seconds=0.1,
                    kill_grace_seconds=2,
                )

        interrupted_job = root / "interrupted-command-job.json"
        interrupted_log = root / "interrupted-command.log"
        interrupted_state: dict[str, Any] = {
            "status": "running",
            "completed_stages": [],
        }

        def interrupting_monitor() -> tuple[str, dict[str, float | None]] | None:
            raise KeyboardInterrupt

        original_runtime_resource_failure = runtime_resource_failure
        try:
            globals()["runtime_resource_failure"] = interrupting_monitor
            try:
                run_command(
                    "interrupted-command-self-test",
                    [sys.executable, "-c", "import time; time.sleep(60)"],
                    repo=workspace,
                    env=guarded_env,
                    log_path=interrupted_log,
                    job_file=interrupted_job,
                    state=interrupted_state,
                )
            except KeyboardInterrupt:
                pass
            else:
                raise AcceptanceError("interrupted stage did not propagate the interrupt")
        finally:
            globals()["runtime_resource_failure"] = original_runtime_resource_failure
        interrupted_events = interrupted_state.get("cleanup_events", [])
        require(
            len(interrupted_events) == 1
            and interrupted_state.get("child_pid") is None
            and not process_group_exists(interrupted_events[0]["child_pid"]),
            "interrupted stage left a process group behind",
        )
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

        rtt_smoke = root / "rtt-light-smoke"
        write_fake_rtt_light_smoke(
            rtt_smoke / "audit",
            leg="audit",
            binary_hash="c" * 64,
            window_backend="headless",
        )
        write_fake_rtt_light_smoke(
            rtt_smoke / "capture",
            leg="capture",
            binary_hash="c" * 64,
            window_backend="x11",
        )
        write_fake_rtt_light_smoke(
            rtt_smoke / "memory",
            leg="memory",
            binary_hash="d" * 64,
            window_backend="x11",
        )
        rtt_result = verify_rtt_light_smoke(
            audit=rtt_smoke / "audit",
            capture=rtt_smoke / "capture",
            memory=rtt_smoke / "memory",
            adapter="Intel",
            window_backend="x11",
        )
        require(rtt_result["status"] == "pass", "valid S1 fixture did not pass")
        broken_rtt = read_json(rtt_smoke / "capture" / "manifest.json")
        broken_rtt["matrix"]["clock_mode"] = "wall"
        atomic_write_json(rtt_smoke / "capture" / "manifest.json", broken_rtt)
        try:
            verify_rtt_light_smoke(
                audit=rtt_smoke / "audit",
                capture=rtt_smoke / "capture",
                memory=rtt_smoke / "memory",
                adapter="Intel",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("invalid S1 clock mode fixture unexpectedly passed")

        native = root / "deconstruction"
        (native / "runtime/saves").mkdir(parents=True)
        save = native / "runtime/saves/world.scn.ron"
        screenshot = native / "deconstruction-v1-v5.png"
        save.write_text("native-save\n", encoding="utf-8")
        screenshot.write_bytes(structural_png(1280, 720))
        atomic_write_json(
            native / "driver-result.json",
            {
                "status": "PASS",
                "profile": "building-deconstruction",
                "run_id": "self-test",
                "checks": {check: "PASS" for check in DECONSTRUCTION_CHECKS},
                "world_epoch_before_load": 7,
                "world_epoch_after_load": 8,
                "save": {"path": str(save), "bytes": save.stat().st_size},
                "screenshot": {
                    "path": str(screenshot),
                    "width": 1280,
                    "height": 720,
                    "bytes": screenshot.stat().st_size,
                },
                "renderer": {
                    "adapter_name": "Intel(R) Arc Graphics",
                    "backend": "Vulkan",
                    "display_handle": "Xlib(XlibDisplayHandle)",
                },
            },
        )
        native_result = verify_deconstruction_artifact(
            native,
            run_id="self-test",
            adapter="Intel",
            backend="vulkan",
            window_backend="x11",
        )
        require(native_result["status"] == "pass", "valid V1-V5 fixture did not pass")
        corrupted = bytearray(screenshot.read_bytes())
        corrupted[41] ^= 0x01
        screenshot.write_bytes(corrupted)
        try:
            verify_deconstruction_artifact(
                native,
                run_id="self-test",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("corrupt native PNG fixture unexpectedly passed")
        screenshot.write_bytes(structural_png(1280, 720))
        truncated = screenshot.read_bytes()[:24]
        screenshot.write_bytes(truncated)
        try:
            verify_deconstruction_artifact(
                native,
                run_id="self-test",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("truncated native PNG fixture unexpectedly passed")
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


def add_deconstruction_arguments(
    parser: argparse.ArgumentParser, *, require_job_root: bool
) -> None:
    parser.add_argument("--repo", required=True)
    parser.add_argument("--job-root", required=require_job_root)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--adapter", default="Intel")
    parser.add_argument("--backend", default="vulkan", choices=["vulkan", "gl"])
    parser.add_argument("--window-backend", default="x11", choices=["x11", "wayland"])
    parser.add_argument("--present-mode", default="novsync")


def add_rtt_light_arguments(
    parser: argparse.ArgumentParser, *, planned_run: bool
) -> None:
    parser.add_argument("--repo", required=True)
    parser.add_argument("--level", required=True, choices=["s1", "formal"])
    parser.add_argument("--attempt-id")
    parser.add_argument("--adapter", default="Intel")
    parser.add_argument("--window-backend", default="x11", choices=["x11", "wayland"])
    parser.add_argument("--prerequisite-commit", action="append", default=[])
    parser.add_argument("--s0-job-root")
    parser.add_argument("--s1-job-root")
    parser.add_argument("--renderdoccmd")
    parser.add_argument("--qrenderdoc")
    parser.add_argument("--renderdoc-library")
    if planned_run:
        parser.add_argument("--state-root", required=True)
        parser.add_argument("--subject-commit", required=True)
        parser.add_argument("--source-fingerprint", required=True)
    else:
        parser.add_argument("--job-root")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan = commands.add_parser("plan-task-dashboard", help="emit a no-prompt launcher plan")
    add_recipe_arguments(plan, require_job_root=False)
    run = commands.add_parser("run-task-dashboard", help="run the planned recipe inside kitty")
    add_recipe_arguments(run, require_job_root=True)
    deconstruction_plan = commands.add_parser(
        "plan-deconstruction", help="emit the no-prompt V1-V5 launcher plan"
    )
    add_deconstruction_arguments(deconstruction_plan, require_job_root=False)
    deconstruction_run = commands.add_parser(
        "run-deconstruction", help="run V1-V5 inside the planned actual window"
    )
    add_deconstruction_arguments(deconstruction_run, require_job_root=True)
    rtt_plan = commands.add_parser(
        "plan-rtt-light", help="emit the S1 or formal RtT-light no-prompt launcher plan"
    )
    add_rtt_light_arguments(rtt_plan, planned_run=False)
    rtt_run = commands.add_parser(
        "run-rtt-light", help="run the planned RtT-light recipe inside kitty"
    )
    add_rtt_light_arguments(rtt_run, planned_run=True)
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
    verify_deconstruction = commands.add_parser(
        "verify-deconstruction", help="fail-closed validation of one V1-V5 artifact"
    )
    verify_deconstruction.add_argument("--artifact", required=True)
    verify_deconstruction.add_argument("--run-id", required=True)
    verify_deconstruction.add_argument("--adapter", default="Intel")
    verify_deconstruction.add_argument("--backend", default="vulkan")
    verify_deconstruction.add_argument(
        "--window-backend", default="x11", choices=["x11", "wayland"]
    )
    verify_rtt = commands.add_parser(
        "verify-rtt-light", help="revalidate a registered formal RtT-light attempt"
    )
    verify_rtt.add_argument("--repo", required=True)
    verify_rtt.add_argument("--attempt", required=True)
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
    if args.command in {"plan-rtt-light", "run-rtt-light"}:
        if not args.adapter:
            raise AcceptanceError("RtT-light adapter filter must be nonempty")
        if args.attempt_id is not None:
            try:
                parsed = uuid.UUID(args.attempt_id)
            except ValueError as error:
                raise AcceptanceError("RtT-light attempt id is not a UUID") from error
            if parsed.version != 4 or str(parsed) != args.attempt_id:
                raise AcceptanceError("RtT-light attempt id must be a canonical UUIDv4")
        if args.command == "run-rtt-light":
            if args.attempt_id is None:
                raise AcceptanceError("planned RtT-light run requires --attempt-id")
            if re.fullmatch(r"[0-9a-f]{40}", args.subject_commit) is None:
                raise AcceptanceError("planned subject commit is invalid")
            if re.fullmatch(r"[0-9a-f]{64}", args.source_fingerprint) is None:
                raise AcceptanceError("planned source fingerprint is invalid")
            if args.level == "formal" and (
                args.s0_job_root is None or args.s1_job_root is None
            ):
                raise AcceptanceError(
                    "formal RtT-light run requires S0 and S1 job roots"
                )


def main() -> int:
    args = parser().parse_args()
    validate_args(args)
    if args.command == "plan-task-dashboard":
        return plan_task_dashboard(args)
    if args.command == "run-task-dashboard":
        return run_task_dashboard(args)
    if args.command == "plan-deconstruction":
        return plan_deconstruction(args)
    if args.command == "run-deconstruction":
        return run_deconstruction(args)
    if args.command == "plan-rtt-light":
        return plan_rtt_light(args)
    if args.command == "run-rtt-light":
        return run_rtt_light(args)
    if args.command == "status":
        return status_command(args)
    if args.command == "verify-artifacts":
        return verify_artifacts_command(args)
    if args.command == "verify-deconstruction":
        return verify_deconstruction_command(args)
    if args.command == "verify-rtt-light":
        return verify_rtt_light_command(args)
    if args.command == "self-test":
        return self_test()
    raise AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print_json({"status": "invalid", "error": str(error)})
        raise SystemExit(1) from error
