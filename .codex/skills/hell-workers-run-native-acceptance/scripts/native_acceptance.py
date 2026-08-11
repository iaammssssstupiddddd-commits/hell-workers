#!/usr/bin/env python3
"""Plan, run, monitor, and verify no-prompt native acceptance sessions."""

from __future__ import annotations

import argparse
import csv
import fcntl
from functools import wraps
import hashlib
import json
import math
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
    from scripts.build_coordination import (
        ACTIVITY_LOCK_FD_ENV,
        ACTIVITY_LOCK_MODE_ENV,
        acquire_activity,
        activity_lease_environment,
        activity_pass_fds,
    )
except ModuleNotFoundError:
    repository_root = Path(__file__).resolve().parents[4]
    sys.path.insert(0, str(repository_root))
    from scripts.build_coordination import (
        ACTIVITY_LOCK_FD_ENV,
        ACTIVITY_LOCK_MODE_ENV,
        acquire_activity,
        activity_lease_environment,
        activity_pass_fds,
    )


GIB = 1024**3
SCHEMA_VERSION = 1
RUNNING_EXIT_CODE = 2
MIN_START_MEMORY_GIB = 10
MIN_STAGE_START_MEMORY_GIB = 8
TWO_JOB_MEMORY_GIB = 16
MIN_WORKSPACE_FREE_GIB = 15
RESOURCE_POLL_SECONDS = 1.0
CAPTURE_TOOL_TIMEOUT_SECONDS = 5.0
PROCESS_GROUP_POLL_SECONDS = 0.1
PROCESS_GROUP_TERM_GRACE_SECONDS = 10.0
PROCESS_GROUP_KILL_GRACE_SECONDS = 10.0
MAX_NATIVE_SCREENSHOT_BYTES = 16 * 1024 * 1024
MAX_NATIVE_SCREENSHOT_DECODED_BYTES = 64 * 1024 * 1024
MAX_SAVE_TRANSACTION_ARTIFACT_FILE_BYTES = 16 * 1024 * 1024
SAVE_CATALOG_MARKER_RGB = (255, 0, 255)
SAVE_CATALOG_MARKER_MIN_PIXELS = 1_024
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
NATIVE_HARNESS_FILES = (
    ".codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py",
    "scripts/build_coordination.py",
    "scripts/cargo_runtime.py",
    "scripts/perf_tool/execution.py",
    "scripts/perf_tool/renderdoc_capture.py",
    "scripts/perf_tool/renderdoc_foundation.py",
    "scripts/perf_tool/rtt_light_bundle.py",
)
DECONSTRUCTION_CHECKS = {"V1", "V2", "V3", "V4", "V5"}
NOTIFICATION_CHECKS = {"A1", "A2", "A3", "A4", "A5"}
SAVE_CATALOG_SCREENSHOT = "paused-after-acceptance.png"
SAVE_CATALOG_CHECKS = {"V1", "V2", "V3", "V4", "V5"}
SAVE_CATALOG_FINAL_RECOVERY_FILE = "manual-2.scn.ron"
SAVE_CATALOG_CAPTURE_SCOPE = "x11-client-window"
SAVE_TRANSACTION_LEGS = ("capture", "memory")
SAVE_TRANSACTION_SIZES = ("small", "medium", "large")
SAVE_TRANSACTION_POPULATIONS = {
    "small": (50, 4),
    "medium": (200, 12),
    "large": (500, 30),
}
SAVE_TRANSACTION_REPEAT = 20
SAVE_TRANSACTION_PREFLIGHT_RUNS = 3
SAVE_TRANSACTION_WARMUP_SECS = 1
SAVE_TRANSACTION_MEASURE_SECS = 2
SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS = 100_000_000
SAVE_TRANSACTION_LARGE_TOTAL_MAX_LIMIT_NS = 250_000_000
SAVE_TRANSACTION_SCHEMA_VERSION = "4"
SAVE_TRANSACTION_MEASURE_NS = 2_000_000_000
SAVE_TRANSACTION_COLUMNS = (
    "schema_version",
    "workload",
    "size",
    "render",
    "seed",
    "soul_count",
    "familiar_count",
    "fixture_checksum",
    "sample_kind",
    "measure_virtual_ns",
    "measure_real_ns",
    "body_bytes",
    "serialize_ns",
    "write_file_sync_ns",
    "commit_directory_sync_ns",
    "total_ns",
    "peak_live_growth_bytes",
)
SAVE_TRANSACTION_AGGREGATE_COLUMNS = (
    "case_id",
    "valid_runs",
    "fixture_checksums",
    "serialize_p95_ns",
    "serialize_max_ns",
    "write_file_sync_p95_ns",
    "write_file_sync_max_ns",
    "commit_directory_sync_p95_ns",
    "commit_directory_sync_max_ns",
    "total_p95_ns",
    "total_max_ns",
    "peak_live_growth_p95_bytes",
    "peak_live_growth_max_bytes",
    "max_rss_kib_max",
    "adapter",
)
RTT_LIGHT_CONTRACT_ID = "rtt-light-v1"
RTT_LIGHT_DEFAULT_STAGE = "current"
RTT_LIGHT_LEGS = ("audit", "behavior", "capture", "renderdoc", "memory")
RTT_LIGHT_SOURCE_CHECKPOINTS = (
    "start",
    "after-renderdoc-build",
    "after-rd0",
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
        with acquire_activity(repo, "exclusive") as lease:
            inherited = activity_lease_environment(lease)
            previous_fd = os.environ.get(ACTIVITY_LOCK_FD_ENV)
            previous_mode = os.environ.get(ACTIVITY_LOCK_MODE_ENV)
            os.environ[ACTIVITY_LOCK_FD_ENV] = inherited[ACTIVITY_LOCK_FD_ENV]
            os.environ[ACTIVITY_LOCK_MODE_ENV] = inherited[ACTIVITY_LOCK_MODE_ENV]
            try:
                return function(args)
            finally:
                for name, previous in (
                    (ACTIVITY_LOCK_FD_ENV, previous_fd),
                    (ACTIVITY_LOCK_MODE_ENV, previous_mode),
                ):
                    if previous is None:
                        os.environ.pop(name, None)
                    else:
                        os.environ[name] = previous

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


def write_new_atomic_text(path: Path, body: str) -> None:
    """Publish a new text artifact atomically without overwriting an ACK.

    The native driver polls acknowledgement files every frame, so a direct
    write would expose an empty or partial contract.  Linking a fully-synced
    temporary file gives create-only publication: an existing acknowledgement
    remains authoritative even if a second publisher races us.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        suffix=".tmp",
        dir=path.parent,
        text=True,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(body)
            handle.flush()
            os.fsync(handle.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError as error:
            raise AcceptanceError(
                f"capture acknowledgement already exists: {path}"
            ) from error
    finally:
        temporary.unlink(missing_ok=True)


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


def stage_start_resource_gate(stage: str) -> dict[str, Any]:
    """Return structured admission evidence before a stage launches."""
    try:
        snapshot = memory_snapshot()
    except AcceptanceError as error:
        raise AcceptanceError(
            f"stage {stage} start resource probe failed: {error}"
        ) from error
    failures = memory_safety_failures(
        snapshot,
        minimum_memory_gib=MIN_STAGE_START_MEMORY_GIB,
        phase="stage start",
    )
    evidence = {
        "at": utc_now(),
        "stage": stage,
        "status": "ready" if not failures else "blocked",
        "mem_available_gib": optional_gib(snapshot["mem_available_bytes"]),
        "swap_total_gib": optional_gib(snapshot["swap_total_bytes"]),
        "swap_free_gib": optional_gib(snapshot["swap_free_bytes"]),
        "minimum_mem_available_gib": MIN_STAGE_START_MEMORY_GIB,
        "failures": failures,
    }
    return evidence


def admit_stage_start(
    stage: str,
    *,
    state: dict[str, Any],
    job_file: Path,
) -> None:
    """Persist the stage-start snapshot and fail before spawning when blocked."""
    try:
        evidence = stage_start_resource_gate(stage)
    except AcceptanceError as error:
        evidence = {
            "at": utc_now(),
            "stage": stage,
            "status": "blocked",
            "reason": str(error),
        }
    state.setdefault("stage_start_resource_events", []).append(evidence)
    update_state(job_file, state, child_pid=None)
    if evidence["status"] != "ready":
        failures = evidence.get("failures")
        reason = "; ".join(failures) if isinstance(failures, list) else evidence["reason"]
        raise AcceptanceError(f"stage {stage} not started: {reason}")


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
        profiles = ("debug", "profiling", "profiling-renderdoc", "release")
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
            "native_stage_start_mem_available": MIN_STAGE_START_MEMORY_GIB,
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
        subject_bytes: bytes | None = None
        if relative in NATIVE_HARNESS_FILES:
            completed = subprocess.run(
                ["git", "show", f"HEAD:{relative}"],
                cwd=repo,
                check=False,
                capture_output=True,
            )
            if completed.returncode != 0:
                continue
            subject_bytes = completed.stdout
        if relative in SOURCE_FILES or relative.startswith(SOURCE_PREFIXES):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            if subject_bytes is not None:
                digest.update(subject_bytes)
            else:
                with source.open("rb") as handle:
                    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                        digest.update(chunk)
        elif relative.startswith(ASSET_PREFIX):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
    return digest.hexdigest()


def native_harness_fingerprint(repo: Path) -> str:
    """Hash C2 orchestration separately from the product build boundary."""
    digest = hashlib.sha256()
    for relative in NATIVE_HARNESS_FILES:
        source = (
            Path(__file__).resolve()
            if relative
            == ".codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py"
            else repo / relative
        )
        require(
            source.is_file() and not source.is_symlink(),
            f"native acceptance harness file is unavailable: {relative}",
        )
        digest.update(b"content\0")
        digest.update(relative.encode())
        digest.update(b"\0")
        with source.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
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


def git_dirty_paths(
    repo: Path, *, allowed_paths: tuple[str, ...] = ()
) -> list[str]:
    output = command_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], repo=repo
    )
    dirty: list[str] = []
    for entry in output.splitlines() if output else []:
        fields = entry.split(maxsplit=1)
        path = fields[1] if len(fields) == 2 else entry
        paths = path.split(" -> ", 1) if " -> " in path else [path]
        if paths and all(candidate in allowed_paths for candidate in paths):
            continue
        dirty.append(entry)
    return dirty


def assert_clean_subject(repo: Path, expected_commit: str) -> None:
    actual_commit = git_subject(repo)
    if actual_commit != expected_commit:
        raise AcceptanceError(
            f"subject commit changed: expected {expected_commit}, got {actual_commit}"
        )
    dirty = git_dirty_paths(repo, allowed_paths=NATIVE_HARNESS_FILES)
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


def rtt_light_contract(repo: Path, stage: str) -> dict[str, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    try:
        from perf_tool.rtt_light_contract import load_rtt_light_contract

        contract = load_rtt_light_contract(RTT_LIGHT_CONTRACT_ID)
    except (ImportError, OSError, ValueError, RuntimeError) as error:
        raise AcceptanceError(f"RtT-light contract validation failed: {error}") from error
    if stage not in contract.get("stages", {}):
        raise AcceptanceError(f"unknown RtT-light stage: {stage}")
    stage_order = list(contract["stages"])
    selected_index = stage_order.index(stage)
    legs = [
        leg.get("leg_id")
        for leg in contract.get("formal_legs", [])
        if isinstance(leg, dict)
        and leg.get("first_required_stage") in stage_order
        and stage_order.index(leg["first_required_stage"]) <= selected_index
    ]
    if legs != list(RTT_LIGHT_LEGS):
        raise AcceptanceError(
            f"RtT-light {stage} leg order differs from the launcher: {legs}"
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
    repo: Path, *, stage: str, subject_commit: str, attempt_id: str
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
        / f"{stage}-{subject_commit[:16]}"
        / "attempts"
        / attempt_id
    )


def source_checkpoint(
    repo: Path,
    *,
    checkpoint: str,
    subject_commit: str,
    fingerprint: str,
    harness_fingerprint: str,
) -> dict[str, Any]:
    if checkpoint not in RTT_LIGHT_SOURCE_CHECKPOINTS:
        raise AcceptanceError(f"unknown RtT-light source checkpoint {checkpoint}")
    assert_clean_subject(repo, subject_commit)
    actual = source_fingerprint(repo)
    if actual != fingerprint:
        raise AcceptanceError(
            f"source fingerprint changed at {checkpoint}: expected {fingerprint}, got {actual}"
        )
    assert_native_harness_unchanged(repo, harness_fingerprint)
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


def renderdoc_probe_environment(*, headless_qt: bool) -> dict[str, str]:
    environment = os.environ.copy()
    if headless_qt:
        environment["QT_QPA_PLATFORM"] = "offscreen"
    return environment


def renderdoc_version(path: Path, *, headless_qt: bool = False) -> str:
    failures: list[str] = []
    argument_options = (
        (("--version",),) if headless_qt else (("version",), ("--version",))
    )
    for arguments in argument_options:
        completed = subprocess.run(
            [str(path), *arguments],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
            env=renderdoc_probe_environment(headless_qt=headless_qt),
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
        qversion = renderdoc_version(qrenderdoc, headless_qt=True)
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
            env=renderdoc_probe_environment(headless_qt=True),
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
    stage: str,
) -> dict[str, list[str]]:
    perf = ["python3", str(repo / "scripts/perf.py")]
    matrix = contract["formal_matrix"]
    selector = [
        "--workload",
        "indoor-light",
        "--contract",
        RTT_LIGHT_CONTRACT_ID,
        "--stage",
        stage,
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
        ",".join(contract["stages"][stage]["required_behavior_cases"]),
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
        commands["build-renderdoc"] = [
            "cargo",
            "build",
            "--profile",
            "profiling-renderdoc",
            "-p",
            "bevy_app@0.1.0",
            "--no-default-features",
            "--features",
            "profiling-renderdoc",
        ]
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


def plan_notifications(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    job_root = (
        Path(args.job_root).resolve()
        if args.job_root
        else unique_job_root(repo, "player-facing-result-notifications")
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
        "run-notifications",
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
        "profile": "player-facing-result-notifications",
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
            "isolated_save_and_settings_roots": True,
            "automatic_cleanup": False,
        },
    }
    print_json(payload)
    return 0 if status == "ready" else 1


def plan_save_catalog(args: argparse.Namespace) -> int:
    require_save_catalog_x11_window_capture(args.window_backend)
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    harness = native_harness_fingerprint(repo)
    job_root = (
        Path(args.job_root).resolve()
        if args.job_root
        else unique_job_root(repo, "save-catalog")
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
    capture_error = save_catalog_window_capture_tool_error()
    if capture_error:
        failures.append(capture_error)
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
        "run-save-catalog",
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
        "--harness-fingerprint",
        harness,
    ]
    payload = {
        "schema_version": SCHEMA_VERSION,
        "status": status,
        "profile": "save-catalog",
        "measurement_kind": "native-functional-acceptance",
        "harness_fingerprint": harness,
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
            "game_processes": 1
            + len(SAVE_TRANSACTION_LEGS)
            * len(SAVE_TRANSACTION_SIZES)
            * (SAVE_TRANSACTION_PREFLIGHT_RUNS + SAVE_TRANSACTION_REPEAT),
            "parallel_game_processes": 1,
            "actual_feature_builds": 3,
            "actual_window_required": True,
            "renderer_evidence_required": True,
            "in_game_screenshot_required": True,
            "screenshot_capture_scope": SAVE_CATALOG_CAPTURE_SCOPE,
            "save_timing_capture_memory_matrix": {
                "instrumentations": list(SAVE_TRANSACTION_LEGS),
                "sizes": list(SAVE_TRANSACTION_SIZES),
                "preflight_runs_per_case": SAVE_TRANSACTION_PREFLIGHT_RUNS,
                "measured_runs_per_case": SAVE_TRANSACTION_REPEAT,
                "runtime_root": "target/.save-transaction-runtime/<run-id>/",
                "artifact_root": "target/perf-runs/save-transaction-<run-id>/",
            },
            "synthetic_desktop_input": False,
            "automatic_cleanup": False,
        },
    }
    print_json(payload)
    return 0 if status == "ready" else 1


def plan_rtt_light(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    resources = resource_snapshot(repo, require_launcher=True)
    contract = rtt_light_contract(repo, args.stage)
    subject_commit = git_subject(repo)
    fingerprint = source_fingerprint(repo)
    harness_fingerprint = native_harness_fingerprint(repo)
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
                stage=args.stage,
            )
            attempt = rtt_light_attempt_path(
                repo, stage=args.stage, subject_commit=subject_commit, attempt_id=attempt_id
            )
            if attempt.exists():
                failures.append(f"attempt path already exists: {attempt}")
        except AcceptanceError as error:
            failures.append(str(error))
        tooling, tool_failures = inspect_renderdoc_tools(repo, args)
        failures.extend(tool_failures)
        output_root = rtt_light_attempt_path(
            repo, stage=args.stage, subject_commit=subject_commit, attempt_id=attempt_id
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
        "--stage",
        args.stage,
        "--state-root",
        str(state_root),
        "--attempt-id",
        attempt_id,
        "--subject-commit",
        subject_commit,
        "--source-fingerprint",
        fingerprint,
        "--harness-fingerprint",
        harness_fingerprint,
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
        "stage_id": args.stage,
        "subject_commit": subject_commit,
        "source_fingerprint": fingerprint,
        "harness_fingerprint": harness_fingerprint,
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
            "game_processes": 65 if args.level == "formal" else 51,
            "parallel_game_processes": 1,
            "actual_feature_builds": 3 if args.level == "formal" else 2,
            "uses_skip_build": False,
            "uses_binary_copy": args.level == "formal",
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
    admit_stage_start(stage, state=state, job_file=job_file)
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
            pass_fds=activity_pass_fds(env),
        )
        try:
            update_state(job_file, state, child_pid=process.pid)
            deadline = (
                time.monotonic() + timeout_seconds
                if timeout_seconds is not None
                else None
            )
            while process.poll() is None:
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


def finalize_native_screenshot(destination: Path) -> dict[str, int]:
    png = destination.read_bytes()
    width, height = validate_png_structure(png)
    marker_pixels = count_save_catalog_marker_pixels(png)
    require(
        marker_pixels >= SAVE_CATALOG_MARKER_MIN_PIXELS,
        "native screenshot does not contain the final Save catalog marker",
    )
    return {"width": width, "height": height, "marker_pixels": marker_pixels}


def require_save_catalog_x11_window_capture(window_backend: str) -> None:
    require(
        window_backend == "x11",
        "save-catalog in-game screenshot evidence requires --window-backend x11",
    )


def save_catalog_window_capture_tool_error() -> str | None:
    missing = [
        command
        for command in ("xprop", "import")
        if shutil.which(command) is None
    ]
    if missing:
        return (
            "save-catalog in-game screenshot capture requires "
            f"{', '.join(missing)}"
        )
    return None


def run_bounded_capture_tool(
    command: list[str], *, label: str, timeout_seconds: float = CAPTURE_TOOL_TIMEOUT_SECONDS
) -> subprocess.CompletedProcess[str]:
    """Run one X11 capture helper within the native-stage deadline budget."""
    try:
        return subprocess.run(
            command,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        raise AcceptanceError(
            f"{label} timed out after {timeout_seconds:g}s"
        ) from error


def parse_linux_proc_stat_ppid(body: str) -> int | None:
    """Read field four from a Linux `/proc/<pid>/stat` record safely."""
    closing = body.rfind(")")
    if closing < 0:
        return None
    fields = body[closing + 1 :].split()
    if len(fields) < 2 or not fields[1].isdigit():
        return None
    parent = int(fields[1])
    return parent if parent > 0 else None


def descendant_process_ids(root_pid: int, *, proc_root: Path = Path("/proc")) -> set[int]:
    """Return the live process subtree rooted at a launched Cargo process."""
    parents: dict[int, list[int]] = {}
    try:
        entries = list(proc_root.iterdir())
    except OSError as error:
        raise AcceptanceError(f"cannot enumerate {proc_root} for X11 window ownership: {error}") from error
    for entry in entries:
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        try:
            parent = parse_linux_proc_stat_ppid((entry / "stat").read_text(encoding="utf-8"))
        except (OSError, UnicodeError):
            continue
        if parent is not None:
            parents.setdefault(parent, []).append(pid)
    descendants = {root_pid}
    pending = [root_pid]
    while pending:
        parent = pending.pop()
        for child in parents.get(parent, []):
            if child not in descendants:
                descendants.add(child)
                pending.append(child)
    return descendants


def parse_x11_client_window_ids(output: str) -> list[str]:
    """Extract canonical X11 client window IDs from `_NET_CLIENT_LIST`."""
    windows: list[str] = []
    for raw_window in re.findall(r"(?<![0-9A-Za-z_])0x[0-9A-Fa-f]+", output):
        window = raw_window.lower()
        if window not in windows:
            windows.append(window)
    return windows


def parse_x11_window_pid(output: str) -> int | None:
    match = re.search(r"=\s*([1-9][0-9]*)\s*$", output.strip())
    return int(match.group(1)) if match is not None else None


def x11_client_windows_for_process_tree(root_pid: int) -> list[tuple[str, int]]:
    tool_error = save_catalog_window_capture_tool_error()
    if tool_error:
        raise AcceptanceError(tool_error)
    xprop = shutil.which("xprop")
    require(xprop is not None, "xprop was unavailable after the capture tool check")
    window_ids: list[str] = []
    failures: list[str] = []
    for property_name in ("_NET_CLIENT_LIST", "_NET_CLIENT_LIST_STACKING"):
        listed = run_bounded_capture_tool(
            [xprop, "-root", property_name],
            label=f"X11 root property {property_name}",
        )
        if listed.returncode != 0:
            detail = (listed.stderr or listed.stdout).strip()
            failures.append(f"{property_name}: {detail or listed.returncode}")
            continue
        for window_id in parse_x11_client_window_ids(listed.stdout):
            if window_id not in window_ids:
                window_ids.append(window_id)
    if not window_ids and failures:
        raise AcceptanceError(
            "could not enumerate X11 client windows: " + "; ".join(failures)
        )
    owner_pids = descendant_process_ids(root_pid)
    owned: list[tuple[str, int]] = []
    for window_id in window_ids:
        owner = run_bounded_capture_tool(
            [xprop, "-id", window_id, "_NET_WM_PID"],
            label=f"X11 window owner lookup for {window_id}",
        )
        if owner.returncode != 0:
            continue
        pid = parse_x11_window_pid(owner.stdout)
        if pid is not None and pid in owner_pids:
            owned.append((window_id, pid))
    return owned


def take_native_screenshot(destination: Path, *, root_pid: int) -> dict[str, int | str] | None:
    """Capture exactly one X11 client window owned by the launched game tree.

    Root-display capture is deliberately forbidden here: a desktop overlay can
    otherwise satisfy the magenta marker check without showing the game UI.
    Returning ``None`` allows the bounded capture handshake to retry while
    Winit/XWayland publishes the actual client window.
    """
    destination.parent.mkdir(parents=True, exist_ok=True)
    import_cmd = shutil.which("import")
    if import_cmd is None:
        raise AcceptanceError("save-catalog in-game screenshot capture requires import")
    candidates: list[tuple[str, int, dict[str, int], Path]] = []
    for window_id, window_pid in x11_client_windows_for_process_tree(root_pid):
        candidate = destination.with_name(
            f".{destination.name}.{window_id.removeprefix('0x')}.candidate.png"
        )
        try:
            candidate.unlink(missing_ok=True)
            completed = run_bounded_capture_tool(
                [
                    import_cmd,
                    "-window",
                    window_id,
                    "-depth",
                    "8",
                    "-type",
                    "TrueColor",
                    str(candidate),
                ],
                label=f"X11 client screenshot capture for {window_id}",
            )
            if completed.returncode != 0 or not candidate.is_file():
                candidate.unlink(missing_ok=True)
                continue
            try:
                screenshot = finalize_native_screenshot(candidate)
            except AcceptanceError:
                candidate.unlink(missing_ok=True)
                continue
            candidates.append((window_id, window_pid, screenshot, candidate))
        except OSError as error:
            raise AcceptanceError(
                f"could not capture X11 client window {window_id}: {error}"
            ) from error
    if not candidates:
        return None
    try:
        require(
            len(candidates) == 1,
            "multiple launched-game X11 windows contained the save-catalog marker",
        )
        window_id, window_pid, screenshot, selected = candidates[0]
        os.replace(selected, destination)
        return {
            **screenshot,
            "capture_scope": SAVE_CATALOG_CAPTURE_SCOPE,
            "window_id": window_id,
            "window_pid": window_pid,
        }
    finally:
        for _, _, _, candidate in candidates:
            candidate.unlink(missing_ok=True)


def write_save_catalog_capture_ack(
    artifact: Path,
    run_id: str,
    screenshot_path: Path,
    screenshot: dict[str, int | str],
) -> None:
    png = screenshot_path.read_bytes()
    byte_count = len(png)
    digest = hashlib.sha256(png).hexdigest()
    body = (
        f"run_id={run_id}\n"
        f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
        f"bytes={byte_count}\n"
        f"width={screenshot['width']}\n"
        f"height={screenshot['height']}\n"
        f"sha256={digest}\n"
        f"marker_pixels={screenshot['marker_pixels']}\n"
        f"capture_scope={screenshot['capture_scope']}\n"
        f"capture_window_id={screenshot['window_id']}\n"
        f"capture_window_pid={screenshot['window_pid']}\n"
    )
    write_new_atomic_text(artifact / "capture.done.txt", body)


def maybe_complete_save_catalog_capture(
    artifact: Path, run_id: str, screenshot_path: Path, *, root_pid: int
) -> bool:
    ready_path = artifact / "capture-ready.txt"
    ack_path = artifact / "capture.done.txt"
    if ack_path.is_file():
        return True
    if not ready_path.is_file():
        return False
    body = ready_path.read_text(encoding="utf-8").strip()
    expected_ready = (
        f"run_id={run_id}\n"
        "marker_rgb=255,0,255\n"
        f"marker_min_pixels={SAVE_CATALOG_MARKER_MIN_PIXELS}"
    )
    require(body == expected_ready, "save-catalog capture-ready marker has an unexpected contract")
    screenshot = take_native_screenshot(screenshot_path, root_pid=root_pid)
    if screenshot is None:
        return False
    write_save_catalog_capture_ack(artifact, run_id, screenshot_path, screenshot)
    return True


def run_command_with_save_capture(
    stage: str,
    command: list[str],
    *,
    repo: Path,
    env: dict[str, str],
    log_path: Path,
    job_file: Path,
    state: dict[str, Any],
    artifact: Path,
    run_id: str,
    screenshot_path: Path,
    timeout_seconds: float | None = None,
) -> None:
    state.setdefault("commands", []).append({"stage": stage, "argv": command})
    update_state(job_file, state, current_stage=stage)
    admit_stage_start(stage, state=state, job_file=job_file)
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
            pass_fds=activity_pass_fds(env),
        )
        try:
            update_state(job_file, state, child_pid=process.pid)
            deadline = (
                time.monotonic() + timeout_seconds
                if timeout_seconds is not None
                else None
            )
            while process.poll() is None:
                try:
                    maybe_complete_save_catalog_capture(
                        artifact, run_id, screenshot_path, root_pid=process.pid
                    )
                except AcceptanceError as error:
                    stop_command_process(process)
                    update_state(job_file, state, child_pid=None)
                    raise error
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


def assert_native_harness_unchanged(repo: Path, expected: str) -> None:
    actual = native_harness_fingerprint(repo)
    if actual != expected:
        raise AcceptanceError(
            "native acceptance harness changed during acceptance: "
            f"expected {expected}, got {actual}"
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


def update_environment_lock_hashes(
    path: Path, *, renderdoc_binary_sha256: str | None = None, memory_binary_sha256: str | None = None
) -> None:
    lock = read_json(path)
    if lock.get("schema_version", 1) < 2:
        lock["schema_version"] = 2
        lock.setdefault("renderdoc_binary_sha256", None)
        lock.setdefault("memory_binary_sha256", None)
    if renderdoc_binary_sha256 is not None:
        lock["renderdoc_binary_sha256"] = renderdoc_binary_sha256
    if memory_binary_sha256 is not None:
        lock["memory_binary_sha256"] = memory_binary_sha256
    atomic_write_json(path, lock)


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
    binary: Path,
    capsule_manifest: Path,
    stage: str,
    output: Path | None = None,
    mode: str = "formal",
    capture_session: Path | None = None,
) -> list[str]:
    paths = tooling["paths"]
    job = tooling["job"]
    command = [
        "python3",
        paths["capture_helper"],
        "capture",
        "--repo",
        str(repo),
        "--binary",
        str(binary),
        "--capsule-manifest",
        str(capsule_manifest),
        "--output",
        str(output or attempt / "renderdoc"),
        "--environment-lock",
        str(environment_lock),
        "--contract",
        RTT_LIGHT_CONTRACT_ID,
        "--stage",
        stage,
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
        "--mode",
        mode,
    ]
    if capture_session is not None:
        command.extend(["--capture-session", str(capture_session)])
    return command


def verify_rtt_light_smoke(
    *, audit: Path, capture: Path, memory: Path, adapter: str, window_backend: str, stage: str
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
        rtt_light_contract = matrix.get("rtt_light_contract")
        require(
            isinstance(rtt_light_contract, dict)
            and rtt_light_contract.get("contract_id") == RTT_LIGHT_CONTRACT_ID
            and rtt_light_contract.get("stage_id") == stage
            and rtt_light_contract.get("lane") == "static",
            f"{name} smoke session has the wrong RtT-light stage",
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
    stage: str,
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
        stage=stage,
    )
    return {
        "status": "pass",
        "subject_commit": subject_commit,
        "source_fingerprint": fingerprint,
        "s0": {"job_root": str(s0_job_root), "verification": s0_result},
        "s1": {"job_root": str(s1_job_root), "verification": s1_result},
    }


def verify_formal_renderdoc_continuity(
    *,
    rd0_manifest: dict[str, Any],
    formal_manifest: dict[str, Any],
    binary_sha256: str,
) -> None:
    """Verify the properties that must remain identical from RD0 to formal.

    ``replay_digest`` proves that two replays of the *same* RDC produce the
    same normalized evidence.  RenderDoc event numbering and descriptor
    enumeration may legitimately differ between separately captured RDCs, so
    their replay digests are not a cross-capture identity.  Both capture modes
    independently validate the exact RtT topology; continuity between them is
    therefore the sealed profiling binary capsule.
    """
    if formal_manifest.get("binary", {}).get("sha256") != binary_sha256:
        raise AcceptanceError("RenderDoc did not use the profiling-renderdoc binary")
    if not isinstance(formal_manifest.get("replay_digest"), str):
        raise AcceptanceError("formal RenderDoc did not complete double replay")
    if formal_manifest.get("capsule") != rd0_manifest.get("capsule"):
        raise AcceptanceError("formal RenderDoc capsule differs from RD0")


@activity_locked
def run_rtt_light(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-rtt-light must be launched by the planned direct kitty command"
        )
    repo = validate_repo(args.repo)
    contract = rtt_light_contract(repo, args.stage)
    subject_commit = args.subject_commit
    if git_subject(repo) != subject_commit:
        raise AcceptanceError("planned RtT-light subject commit changed before launch")
    fingerprint = source_fingerprint(repo)
    if fingerprint != args.source_fingerprint:
        raise AcceptanceError("planned RtT-light source fingerprint changed before launch")
    harness_fingerprint = native_harness_fingerprint(repo)
    if harness_fingerprint != args.harness_fingerprint:
        raise AcceptanceError("planned RtT-light harness fingerprint changed before launch")
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
            repo,
            stage=args.stage,
            subject_commit=subject_commit,
            attempt_id=args.attempt_id,
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
        "harness_fingerprint": harness_fingerprint,
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
                stage=args.stage,
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
        stage=args.stage,
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
            capsule_root: Path | None = None
            renderdoc_binary: Path | None = None
            renderdoc_hash: str | None = None
            rd0_manifest: dict[str, Any] | None = None
            if formal:
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="start",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        harness_fingerprint=harness_fingerprint,
                    )
                )
                if tooling is None:
                    raise AcceptanceError("formal RenderDoc tooling disappeared")
                run_command(
                    "build-renderdoc",
                    commands["build-renderdoc"],
                    repo=repo,
                    env=env,
                    log_path=log_path,
                    job_file=state_file,
                    state=state,
                )
                sys.path.insert(0, str(repo / "scripts"))
                from perf_tool.renderdoc_foundation import (
                    FORMAL_RENDERDOC_OUTER_DEADLINE_SECONDS,
                    RD0_OUTER_DEADLINE_SECONDS,
                    copy_binary_capsule,
                    foundation_diagnostic_root,
                    verify_capsule_hash,
                )

                foundation_root = foundation_diagnostic_root(repo)
                capsule_root = foundation_root / "capsule/renderdoc"
                rustc = subprocess.run(
                    ["rustc", "--version"],
                    cwd=repo,
                    env=env,
                    check=True,
                    capture_output=True,
                    text=True,
                ).stdout.strip()
                capsule = copy_binary_capsule(
                    source_binary=repo / "target/profiling-renderdoc/bevy_app",
                    capsule_root=capsule_root,
                    leg="renderdoc",
                    profile="profiling-renderdoc",
                    features="profiling-renderdoc",
                    cargo_lock_path=repo / "Cargo.lock",
                    rustc_version=rustc,
                    linker=env.get("RUSTFLAGS", "workspace-default"),
                    env_allowlist={
                        key: env[key]
                        for key in ("CARGO_BUILD_JOBS", "CARGO_INCREMENTAL")
                        if key in env
                    },
                )
                sealed = verify_capsule_hash(capsule_root)
                if sealed != capsule:
                    raise AcceptanceError(
                        "RenderDoc capsule changed immediately after sealing"
                    )
                renderdoc_binary = capsule_root / "bevy_app"
                renderdoc_hash = capsule.binary_sha256
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-renderdoc-build",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        harness_fingerprint=harness_fingerprint,
                    )
                )
                s1_state = read_json(Path(args.s1_job_root).resolve() / "job.json")
                s1_paths = s1_state.get("paths", {})
                s1_attempt = Path(s1_paths.get("attempt", "")).resolve()
                s1_environment_lock = Path(
                    s1_paths.get("environment_lock", "")
                ).resolve()
                rd0_root = foundation_root / "rd0"
                rd0_environment_lock = rd0_root / "environment-lock.json"
                atomic_write_json(
                    rd0_environment_lock, read_json(s1_environment_lock)
                )
                update_environment_lock_hashes(
                    rd0_environment_lock,
                    renderdoc_binary_sha256=renderdoc_hash,
                )
                rd0_output = rd0_root / "renderdoc"
                run_command(
                    "renderdoc-rd0",
                    renderdoc_capture_command(
                        repo=repo,
                        attempt=attempt,
                        environment_lock=rd0_environment_lock,
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        adapter=args.adapter,
                        window_backend=args.window_backend,
                        tooling=tooling,
                        binary=renderdoc_binary,
                        capsule_manifest=capsule_root / "capsule-manifest.json",
                        stage=args.stage,
                        output=rd0_output,
                        mode="rd0",
                        capture_session=s1_attempt / "capture/manifest.json",
                    ),
                    repo=repo,
                    env=env,
                    log_path=log_path,
                    job_file=state_file,
                    state=state,
                    timeout_seconds=RD0_OUTER_DEADLINE_SECONDS,
                )
                rd0_manifest = read_json(rd0_output / "manifest.json")
                if (
                    rd0_manifest.get("binary", {}).get("sha256") != renderdoc_hash
                    or not isinstance(rd0_manifest.get("replay_digest"), str)
                ):
                    raise AcceptanceError(
                        "RD0 did not seal the RenderDoc capsule and double replay"
                    )
                verify_capsule_hash(capsule_root)
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-rd0",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        harness_fingerprint=harness_fingerprint,
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
                        harness_fingerprint=harness_fingerprint,
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
                        harness_fingerprint=harness_fingerprint,
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
                        harness_fingerprint=harness_fingerprint,
                    )
                )
                if (
                    tooling is None
                    or capsule_root is None
                    or renderdoc_binary is None
                    or renderdoc_hash is None
                    or rd0_manifest is None
                ):
                    raise AcceptanceError(
                        "RenderDoc capsule and RD0 were not sealed before Capture"
                    )
                update_environment_lock_hashes(
                    environment_lock, renderdoc_binary_sha256=renderdoc_hash
                )
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
                        binary=renderdoc_binary,
                        capsule_manifest=capsule_root / "capsule-manifest.json",
                        stage=args.stage,
                        capture_session=attempt / "capture/manifest.json",
                    ),
                    repo=repo,
                    env=env,
                    log_path=log_path,
                    job_file=state_file,
                    state=state,
                    timeout_seconds=FORMAL_RENDERDOC_OUTER_DEADLINE_SECONDS,
                )
                renderdoc_manifest = read_json(attempt / "renderdoc/manifest.json")
                verify_formal_renderdoc_continuity(
                    rd0_manifest=rd0_manifest,
                    formal_manifest=renderdoc_manifest,
                    binary_sha256=renderdoc_hash,
                )
                verify_capsule_hash(capsule_root)
                source_checks.append(
                    source_checkpoint(
                        repo,
                        checkpoint="after-renderdoc",
                        subject_commit=subject_commit,
                        fingerprint=fingerprint,
                        harness_fingerprint=harness_fingerprint,
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
            if formal:
                update_environment_lock_hashes(
                    environment_lock, memory_binary_sha256=memory_hash
                )
            assert_source_unchanged(repo, fingerprint)

            if not formal:
                verification = verify_rtt_light_smoke(
                    audit=attempt / "audit",
                    capture=attempt / "capture",
                    memory=attempt / "memory",
                    adapter=args.adapter,
                    window_backend=args.window_backend,
                    stage=args.stage,
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
                    harness_fingerprint=harness_fingerprint,
                )
            )
            source_checks.append(
                source_checkpoint(
                    repo,
                    checkpoint="before-registration",
                    subject_commit=subject_commit,
                    fingerprint=fingerprint,
                    harness_fingerprint=harness_fingerprint,
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
                "stage_id": args.stage,
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
    runtime_root = job_root / "runtime"
    (runtime_root / "saves").mkdir(parents=True)
    (runtime_root / "settings").mkdir(parents=True)
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


@activity_locked
def run_notifications(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-notifications must be launched by the planned direct kitty command"
        )
    repo = validate_repo(args.repo)
    job_root = Path(args.job_root).resolve()
    require_persistent_storage(job_root, label="native acceptance job root")
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    job_root.mkdir(parents=True)
    artifact = job_root / "artifact"
    artifact.mkdir()
    runtime_root = job_root / "runtime"
    (runtime_root / "saves").mkdir(parents=True)
    (runtime_root / "settings").mkdir(parents=True)
    job_file = job_root / "job.json"
    log_path = job_root / "orchestrator.log"
    resources = resource_snapshot(repo, require_launcher=False)
    source = source_fingerprint(repo)
    harness = native_harness_fingerprint(repo)
    run_id = f"a2-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}-{secrets.token_hex(4)}"
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "profile": "player-facing-result-notifications",
        "status": "running",
        "started_at": utc_now(),
        "heartbeat_at": utc_now(),
        "pid": os.getpid(),
        "repo": str(repo),
        "job_root": str(job_root),
        "run_id": run_id,
        "source_fingerprint": source,
        "harness_fingerprint": harness,
        "resources": resources,
        "completed_stages": [],
        "paths": {
            "artifact": str(artifact),
            "runtime": str(runtime_root),
            "driver_result": str(artifact / "driver-result.json"),
            "screenshot": str(artifact / "a2-notifications.png"),
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
        if key.startswith("HW_PERF_") or key.startswith("HW_NATIVE_"):
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
            "HW_NATIVE_NOTIFICATION_ACCEPTANCE_ARTIFACT": str(artifact),
            "HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUNTIME_ROOT": str(runtime_root),
            "HW_NATIVE_NOTIFICATION_ACCEPTANCE_RUN_ID": run_id,
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
                "native-notifications",
                command,
                repo=repo,
                env=env,
                log_path=log_path,
                job_file=job_file,
                state=state,
                timeout_seconds=360,
            )
            assert_source_unchanged(repo, source)
            require(
                native_harness_fingerprint(repo) == harness,
                "native acceptance harness changed during the A2 run",
            )
            verification = verify_notifications_artifact(
                artifact,
                runtime_root=runtime_root,
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
def run_save_catalog(args: argparse.Namespace) -> int:
    if os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") != "1":
        raise AcceptanceError(
            "run-save-catalog must be launched by the planned direct kitty command"
        )
    require_save_catalog_x11_window_capture(args.window_backend)
    capture_error = save_catalog_window_capture_tool_error()
    if capture_error:
        raise AcceptanceError(capture_error)
    repo = validate_repo(args.repo)
    job_root = Path(args.job_root).resolve()
    require_persistent_storage(job_root, label="native acceptance job root")
    if job_root.exists():
        raise AcceptanceError(f"job root already exists: {job_root}")
    job_root.mkdir(parents=True)
    artifact = job_root / "artifact"
    artifact.mkdir()
    # Keep all save and settings I/O outside the immutable evidence directory.
    # The native driver rejects an artifact-contained runtime root, and the
    # explicit children make the freshness/ownership boundary auditable.
    runtime_root = job_root / "runtime"
    (runtime_root / "saves").mkdir(parents=True)
    (runtime_root / "settings").mkdir(parents=True)
    job_file = job_root / "job.json"
    log_path = job_root / "orchestrator.log"
    resources = resource_snapshot(repo, require_launcher=False)
    source = source_fingerprint(repo)
    harness = native_harness_fingerprint(repo)
    require(
        harness == args.harness_fingerprint,
        "planned native acceptance harness changed before launch",
    )
    run_id = f"c2-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}-{secrets.token_hex(4)}"
    performance_root = repo / "target" / "perf-runs" / f"save-transaction-{run_id}"
    save_transaction_runtime_root = (
        repo / "target" / ".save-transaction-runtime" / run_id
    )
    for path, label in (
        (performance_root, "save-transaction performance root"),
        (save_transaction_runtime_root, "save-transaction runtime root"),
    ):
        require_persistent_storage(path, label=label)
        if path.exists():
            raise AcceptanceError(f"{label} already exists: {path}")
    screenshot_path = artifact / SAVE_CATALOG_SCREENSHOT
    # V5 deliberately corrupts Manual 1 to prove recovery remains fail-closed.
    # The terminal durable target is the separately saved Manual 2 recovery slot.
    save_path = runtime_root / "saves" / SAVE_CATALOG_FINAL_RECOVERY_FILE
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "profile": "save-catalog",
        "status": "running",
        "started_at": utc_now(),
        "heartbeat_at": utc_now(),
        "pid": os.getpid(),
        "repo": str(repo),
        "job_root": str(job_root),
        "run_id": run_id,
        "source_fingerprint": source,
        "harness_fingerprint": harness,
        "resources": resources,
        "completed_stages": [],
        "paths": {
            "artifact": str(artifact),
            "driver_result": str(artifact / "driver-result.json"),
            "screenshot": str(screenshot_path),
            "save": str(save_path),
            "runtime": str(runtime_root),
            "performance": {
                "capture": str(performance_root / "capture"),
                "memory": str(performance_root / "memory"),
            },
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

    native_env = cargo_environment(repo)
    for key in list(native_env):
        if key.startswith("HW_PERF_") or key in {
            "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_ARTIFACT",
            "HW_NATIVE_DECONSTRUCTION_ACCEPTANCE_RUN_ID",
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT",
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID",
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUNTIME_ROOT",
        }:
            native_env.pop(key, None)
    native_env = cargo_environment(repo, native_env)
    native_env.update(
        {
            "PYTHONDONTWRITEBYTECODE": "1",
            "CARGO_BUILD_JOBS": str(resources["cargo_jobs"]),
            "CARGO_INCREMENTAL": "0",
            "HELL_WORKERS_WORLDGEN_SEED": str(args.seed),
            "HW_WINDOW_BACKEND": args.window_backend,
            "WGPU_BACKEND": args.backend,
            "HW_PRESENT_MODE": args.present_mode,
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_ARTIFACT": str(artifact),
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUNTIME_ROOT": str(runtime_root),
            "HW_NATIVE_SAVE_LOAD_ACCEPTANCE_RUN_ID": run_id,
        }
    )
    native_command = ["cargo", "run", "--locked", "-p", "bevy_app@0.1.0"]
    perf_env = cargo_environment(repo)
    for key in list(perf_env):
        if key.startswith("HW_PERF_") or key.startswith("HW_NATIVE_"):
            perf_env.pop(key, None)
    perf_env["PYTHONDONTWRITEBYTECODE"] = "1"
    performance_root_relative = Path("target") / "perf-runs" / performance_root.name
    runtime_root_relative = Path("target") / ".save-transaction-runtime" / run_id
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
            # `perf.py` owns its own non-blocking exclusive Cargo lease. Hold
            # that lease only for the raw actual-window stage, while this tiny
            # native lock continues to serialize the complete recipe.
            with acquire_activity(repo, "exclusive"):
                run_command_with_save_capture(
                    "native-save-catalog",
                    native_command,
                    repo=repo,
                    env=native_env,
                    log_path=log_path,
                    job_file=job_file,
                    state=state,
                    artifact=artifact,
                    run_id=run_id,
                    screenshot_path=screenshot_path,
                    timeout_seconds=240,
                )
            assert_source_unchanged(repo, source)
            assert_native_harness_unchanged(repo, harness)
            native_verification = verify_save_catalog_artifact(
                artifact,
                run_id=run_id,
                adapter=args.adapter,
                backend=args.backend,
                window_backend=args.window_backend,
                runtime_root=runtime_root,
            )
            leg_binary_hashes: dict[str, str] = {}
            for instrumentation in SAVE_TRANSACTION_LEGS:
                run_command(
                    f"save-transaction-{instrumentation}",
                    save_transaction_perf_command(
                        run_id=run_id,
                        seed=args.seed,
                        backend=args.backend,
                        instrumentation=instrumentation,
                        output=performance_root_relative / instrumentation,
                        runtime_root=runtime_root_relative / instrumentation,
                    ),
                    repo=repo,
                    env=perf_env,
                    log_path=log_path,
                    job_file=job_file,
                    state=state,
                    timeout_seconds=3600,
                )
                leg_binary_hashes[instrumentation] = manifest_binary_hash(
                    performance_root / instrumentation,
                    repo,
                )
                if instrumentation == "memory":
                    require(
                        leg_binary_hashes["capture"] != leg_binary_hashes["memory"],
                        "Capture and Memory used the same profiling binary",
                    )
                assert_source_unchanged(repo, source)
                assert_native_harness_unchanged(repo, harness)
            performance_verification = verify_save_transaction_bundle(
                repo=repo,
                performance_root=performance_root,
                expected_source=source,
                expected_seed=args.seed,
                expected_backend=args.backend,
            )
            remove_empty_save_transaction_runtime_root(repo, run_id)
            require_save_transaction_runtime_absent(repo, run_id)
            assert_native_harness_unchanged(repo, harness)
            verification = {
                "status": "pass",
                "native": native_verification,
                "save_transaction": performance_verification,
                "runtime_cleanup": "verified-absent",
                "harness": {
                    "fingerprint_start": harness,
                    "fingerprint_end": native_harness_fingerprint(repo),
                    "unchanged": True,
                },
            }
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
            cleanup_error = None
            try:
                cleanup_save_transaction_runtime_root(repo, run_id)
            except Exception as cleanup_failure:
                cleanup_error = str(cleanup_failure)
            update_state(
                job_file,
                state,
                status="invalid",
                current_stage=None,
                child_pid=None,
                error=(
                    str(error)
                    if cleanup_error is None
                    else f"{error}; save-transaction runtime cleanup failed: {cleanup_error}"
                ),
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


def verify_notifications_artifact(
    artifact: Path,
    *,
    runtime_root: Path,
    run_id: str,
    adapter: str,
    backend: str,
    window_backend: str,
) -> dict[str, Any]:
    artifact = artifact.resolve()
    runtime_root = runtime_root.resolve()
    result_path = artifact / "driver-result.json"
    screenshot_path = artifact / "a2-notifications.png"
    require(artifact.is_dir(), f"A2 artifact directory is missing: {artifact}")
    artifact_entries = {entry.name for entry in artifact.iterdir()}
    require(
        artifact_entries == {"driver-result.json", "a2-notifications.png"},
        f"A2 artifact set is not exact: {sorted(artifact_entries)}",
    )
    require(
        all(entry.is_file() and not entry.is_symlink() for entry in artifact.iterdir()),
        "A2 artifact contains a non-regular file or symlink",
    )
    result = read_json(result_path)
    require(result.get("status") == "PASS", f"A2 driver did not pass: {result_path}")
    require(
        result.get("profile") == "player-facing-result-notifications",
        f"wrong A2 native profile in {result_path}",
    )
    actual_run_id = result.get("run_id")
    require(isinstance(actual_run_id, str) and actual_run_id, "A2 driver run_id is missing")
    require(run_id and actual_run_id == run_id, "A2 driver run_id does not match the job")
    checks = result.get("checks")
    require(isinstance(checks, dict), "A2 driver checks are missing")
    require(set(checks) == NOTIFICATION_CHECKS, "A2 driver check set is incomplete")
    require(
        all(checks.get(check) == "PASS" for check in NOTIFICATION_CHECKS),
        "one or more A2 driver checks did not pass",
    )

    runtime = result.get("runtime")
    require(isinstance(runtime, dict), "A2 runtime evidence is missing")
    require(
        Path(str(runtime.get("save_root"))).resolve() == runtime_root / "saves",
        "A2 save root escaped the isolated runtime",
    )
    require(
        Path(str(runtime.get("settings_root"))).resolve() == runtime_root / "settings",
        "A2 settings root escaped the isolated runtime",
    )

    screenshot = result.get("screenshot")
    require(isinstance(screenshot, dict), "A2 screenshot evidence is missing")
    require(screenshot_path.is_file(), f"A2 screenshot is missing: {screenshot_path}")
    require(
        Path(str(screenshot.get("path"))).resolve() == screenshot_path,
        "A2 driver screenshot path does not match the artifact contract",
    )
    screenshot_bytes = screenshot_path.stat().st_size
    require(
        0 < screenshot_bytes <= MAX_NATIVE_SCREENSHOT_BYTES,
        "A2 screenshot size is outside the fail-closed limit",
    )
    png = screenshot_path.read_bytes()
    width, height = validate_png_structure(png)
    require(width >= 640 and height >= 360, f"A2 screenshot is too small: {width}x{height}")
    require(screenshot.get("width") == width, "A2 driver screenshot width is stale")
    require(screenshot.get("height") == height, "A2 driver screenshot height is stale")
    require(screenshot.get("bytes") == screenshot_bytes, "A2 screenshot byte count is stale")

    renderer = result.get("renderer")
    require(isinstance(renderer, dict), "A2 renderer evidence is missing")
    adapter_name = str(renderer.get("adapter_name", ""))
    actual_backend = str(renderer.get("backend", ""))
    display_handle = str(renderer.get("display_handle", ""))
    require(adapter_name, "A2 renderer adapter name is empty")
    if adapter:
        require(
            adapter.lower() in adapter_name.lower(),
            f"actual A2 adapter does not contain {adapter!r}: {adapter_name}",
        )
    require(
        actual_backend.lower() == backend.lower(),
        f"actual A2 renderer backend is {actual_backend!r}, expected {backend!r}",
    )
    display_lower = display_handle.lower()
    if window_backend == "x11":
        require(
            "xlib" in display_lower or "xcb" in display_lower,
            f"actual A2 display handle is not X11: {display_handle}",
        )
    else:
        require(
            "wayland" in display_lower,
            f"actual A2 display handle is not Wayland: {display_handle}",
        )

    return {
        "status": "pass",
        "profile": "player-facing-result-notifications",
        "run_id": actual_run_id,
        "checks": checks,
        "runtime": {
            "save_root": str(runtime_root / "saves"),
            "settings_root": str(runtime_root / "settings"),
        },
        "screenshot": {
            "path": str(screenshot_path),
            "width": width,
            "height": height,
            "bytes": screenshot_bytes,
            "sha256": sha256(screenshot_path),
        },
        "renderer": {
            "adapter_name": adapter_name,
            "backend": actual_backend,
            "display_handle": display_handle,
        },
    }


def verify_save_catalog_artifact(
    artifact: Path,
    *,
    run_id: str,
    adapter: str,
    backend: str,
    window_backend: str,
    runtime_root: Path | None = None,
) -> dict[str, Any]:
    require_save_catalog_x11_window_capture(window_backend)
    artifact = artifact.resolve()
    require_exact_artifact_entries(
        artifact,
        directories=set(),
        files={
            "driver-result.json",
            "capture-ready.txt",
            "capture.done.txt",
            SAVE_CATALOG_SCREENSHOT,
        },
        label="save-catalog native artifact",
    )
    runtime_root = (
        runtime_root.resolve()
        if runtime_root is not None
        else (artifact.parent / "runtime").resolve()
    )
    require(runtime_root.is_dir(), f"native runtime root is missing: {runtime_root}")
    require(
        not runtime_root.is_relative_to(artifact),
        "native runtime root must be outside the screenshot artifact",
    )
    result_path = artifact / "driver-result.json"
    ready_path = artifact / "capture-ready.txt"
    acknowledgement_path = artifact / "capture.done.txt"
    screenshot_path = artifact / SAVE_CATALOG_SCREENSHOT
    save_path = runtime_root / "saves" / SAVE_CATALOG_FINAL_RECOVERY_FILE
    settings_path = runtime_root / "settings/settings.ron"
    result = read_json(result_path)
    require(result.get("status") == "PASS", f"driver did not pass: {result_path}")
    require(
        result.get("profile") == "save-catalog",
        f"wrong native profile in {result_path}",
    )
    actual_run_id = result.get("run_id")
    require(isinstance(actual_run_id, str) and actual_run_id, "driver run_id is missing")
    require(run_id, "verification requires the launched run_id")
    require(actual_run_id == run_id, "driver run_id does not match the launched job")
    checks = result.get("checks")
    require(isinstance(checks, dict), "driver checks are missing")
    require(
        set(checks) == SAVE_CATALOG_CHECKS,
        "driver save-catalog V1-V5 check set is incomplete",
    )
    require(
        all(checks.get(check) == "PASS" for check in SAVE_CATALOG_CHECKS),
        "one or more driver save-catalog V1-V5 checks did not pass",
    )
    before_epoch = result.get("world_epoch_before_valid_load")
    after_epoch = result.get("world_epoch_after_valid_load")
    require(
        isinstance(before_epoch, int)
        and isinstance(after_epoch, int)
        and after_epoch == before_epoch + 1,
        "WorldEpoch did not advance exactly once",
    )
    require(save_path.is_file(), f"native save artifact is missing: {save_path}")
    save_bytes = save_path.stat().st_size
    require(save_bytes > 0, "native save artifact is empty")
    driver_save_bytes = result.get("save_bytes")
    require(
        isinstance(driver_save_bytes, int) and driver_save_bytes == save_bytes,
        "driver save byte count is stale",
    )
    runtime = result.get("runtime")
    require(isinstance(runtime, dict), "driver runtime isolation evidence is missing")
    require(
        Path(str(runtime.get("save_root"))).resolve() == runtime_root / "saves",
        "driver save root does not match the isolated runtime contract",
    )
    require(
        Path(str(runtime.get("settings_root"))).resolve() == runtime_root / "settings",
        "driver settings root does not match the isolated runtime contract",
    )
    require(
        not settings_path.is_relative_to(artifact),
        "settings persistence path leaked into the artifact",
    )
    require(screenshot_path.is_file(), f"native screenshot is missing: {screenshot_path}")
    screenshot_bytes = screenshot_path.stat().st_size
    require(
        0 < screenshot_bytes <= MAX_NATIVE_SCREENSHOT_BYTES,
        "native screenshot size is outside the fail-closed limit",
    )
    png = screenshot_path.read_bytes()
    width, height = validate_png_structure(png)
    require(width >= 640 and height >= 360, f"screenshot is too small: {width}x{height}")
    require(result.get("screenshot") == SAVE_CATALOG_SCREENSHOT, "driver screenshot name mismatch")
    require(
        result.get("screenshot_bytes") == screenshot_bytes,
        "driver screenshot byte count is stale",
    )
    require(result.get("screenshot_width") == width, "driver screenshot width is stale")
    require(result.get("screenshot_height") == height, "driver screenshot height is stale")
    screenshot_sha = result.get("screenshot_sha256")
    require(
        isinstance(screenshot_sha, str)
        and len(screenshot_sha) == 64
        and screenshot_sha == sha256(screenshot_path),
        "driver screenshot hash is stale",
    )
    marker_pixels = result.get("screenshot_marker_pixels")
    require(
        isinstance(marker_pixels, int)
        and marker_pixels >= SAVE_CATALOG_MARKER_MIN_PIXELS
        and marker_pixels == count_save_catalog_marker_pixels(png),
        "driver screenshot does not retain the final Save catalog marker evidence",
    )
    screenshot_capture = result.get("screenshot_capture")
    require(
        isinstance(screenshot_capture, dict),
        "driver screenshot capture ownership evidence is missing",
    )
    capture_scope = screenshot_capture.get("scope")
    capture_window_id = screenshot_capture.get("window_id")
    capture_window_pid = screenshot_capture.get("window_pid")
    require(
        capture_scope == SAVE_CATALOG_CAPTURE_SCOPE,
        "driver screenshot was not captured from an X11 client window",
    )
    require(
        isinstance(capture_window_id, str)
        and re.fullmatch(r"0x[0-9a-f]+", capture_window_id) is not None,
        "driver screenshot X11 window ID is invalid",
    )
    require(
        isinstance(capture_window_pid, int) and capture_window_pid > 0,
        "driver screenshot X11 window PID is invalid",
    )
    expected_ready = (
        f"run_id={run_id}\n"
        "marker_rgb=255,0,255\n"
        f"marker_min_pixels={SAVE_CATALOG_MARKER_MIN_PIXELS}\n"
    )
    require(
        ready_path.read_text(encoding="utf-8") == expected_ready,
        "native capture-ready marker has an unexpected contract",
    )
    expected_acknowledgement = (
        f"run_id={run_id}\n"
        f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
        f"bytes={screenshot_bytes}\n"
        f"width={width}\n"
        f"height={height}\n"
        f"sha256={screenshot_sha}\n"
        f"marker_pixels={marker_pixels}\n"
        f"capture_scope={capture_scope}\n"
        f"capture_window_id={capture_window_id}\n"
        f"capture_window_pid={capture_window_pid}\n"
    )
    require(
        acknowledgement_path.read_text(encoding="utf-8") == expected_acknowledgement,
        "native capture acknowledgement has an unexpected contract",
    )
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
        "profile": "save-catalog",
        "run_id": actual_run_id,
        "checks": checks,
        "world_epoch_before_valid_load": before_epoch,
        "world_epoch_after_valid_load": after_epoch,
        "save": {
            "path": str(save_path),
            "bytes": save_bytes,
            "sha256": sha256(save_path),
        },
        "screenshot": {
            "path": str(screenshot_path),
            "width": width,
            "height": height,
            "bytes": len(png),
            "sha256": sha256(screenshot_path),
            "marker_pixels": marker_pixels,
            "capture_scope": capture_scope,
            "window_id": capture_window_id,
            "window_pid": capture_window_pid,
        },
        "requested_adapter": adapter,
        "requested_backend": backend,
        "requested_window_backend": window_backend,
        "runtime": {
            "root": str(runtime_root),
            "save_root": str(save_path.parent),
            "settings_root": str(settings_path.parent),
        },
        "renderer": {
            "adapter_name": adapter_name,
            "backend": actual_backend,
            "display_handle": display_handle,
        },
    }


def read_exact_csv(path: Path, columns: tuple[str, ...], label: str) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
    except (OSError, UnicodeError, csv.Error) as error:
        raise AcceptanceError(f"cannot parse {label}: {error}") from error
    require(reader.fieldnames == list(columns), f"{label} has an unexpected schema")
    require(
        all(None not in row for row in rows),
        f"{label} has a row with more values than its schema",
    )
    return rows


def canonical_nonnegative(row: dict[str, str], column: str, label: str) -> int:
    value = row.get(column)
    require(
        isinstance(value, str) and re.fullmatch(r"0|[1-9][0-9]*", value) is not None,
        f"{label} {column} is not a canonical nonnegative integer",
    )
    return int(value)


def save_transaction_labels() -> tuple[tuple[str, bool], ...]:
    return (
        *(
            (f"preflight-{index:03d}", True)
            for index in range(1, SAVE_TRANSACTION_PREFLIGHT_RUNS + 1)
        ),
        *(
            (f"run-{index:03d}", False)
            for index in range(1, SAVE_TRANSACTION_REPEAT + 1)
        ),
    )


def nearest_rank_p95(values: list[int]) -> int:
    require(values, "p95 requires a nonempty sample set")
    ordered = sorted(values)
    return ordered[(len(ordered) * 95 + 99) // 100 - 1]


def validate_save_transaction_memory_sidecars(
    run_dir: Path, row: dict[str, str]
) -> tuple[int, int]:
    memory_columns = (
        "schema_version",
        "baseline_live_bytes",
        "peak_live_bytes",
        "final_live_bytes",
        "allocated_bytes",
        "deallocated_bytes",
        "allocation_calls",
        "deallocation_calls",
        "reallocation_calls",
        "accounting_errors",
    )
    memory_rows = read_exact_csv(run_dir / "data" / "memory.csv", memory_columns, "memory.csv")
    require(len(memory_rows) == 1, "memory.csv must contain exactly one row")
    memory = memory_rows[0]
    require(memory.get("schema_version") == "1", "memory.csv schema version is unsupported")
    values = {
        column: canonical_nonnegative(memory, column, "memory.csv")
        for column in memory_columns[1:]
    }
    baseline = values["baseline_live_bytes"]
    peak = values["peak_live_bytes"]
    require(values["accounting_errors"] == 0, "memory.csv allocator accounting reported errors")
    require(
        peak >= max(baseline, values["final_live_bytes"]),
        "memory.csv peak live bytes is below baseline or final live bytes",
    )
    require(
        baseline + values["allocated_bytes"]
        == values["final_live_bytes"] + values["deallocated_bytes"],
        "memory.csv allocator byte accounting is unbalanced",
    )
    require(
        (values["allocation_calls"] == 0) == (values["allocated_bytes"] == 0),
        "memory.csv allocation calls and bytes disagree",
    )
    require(
        (values["deallocation_calls"] == 0) == (values["deallocated_bytes"] == 0),
        "memory.csv deallocation calls and bytes disagree",
    )
    require(
        values["reallocation_calls"]
        <= min(values["allocation_calls"], values["deallocation_calls"]),
        "memory.csv reallocation calls exceed allocation/deallocation calls",
    )
    growth = canonical_nonnegative(row, "peak_live_growth_bytes", "save_transaction.csv")
    require(peak >= baseline and peak - baseline == growth, "allocator growth disagrees with save_transaction.csv")

    usage: dict[str, str] = {}
    try:
        usage_lines = (run_dir / "resource-usage.txt").read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise AcceptanceError(f"missing GNU time resource usage: {error}") from error
    for line in usage_lines:
        key, separator, value = line.partition("=")
        require(separator == "=" and key and key not in usage, "resource-usage.txt has an invalid row")
        usage[key] = value
    require(
        set(usage) == {"max_rss_kib", "user_cpu_secs", "system_cpu_secs", "exit_status"},
        "resource-usage.txt has an unexpected schema",
    )
    rss = canonical_nonnegative(usage, "max_rss_kib", "resource-usage.txt")
    require(rss > 0, "GNU time max RSS must be positive")
    for column in ("user_cpu_secs", "system_cpu_secs"):
        try:
            value = float(usage[column])
        except (KeyError, TypeError, ValueError) as error:
            raise AcceptanceError(f"resource-usage.txt {column} is invalid") from error
        require(
            math.isfinite(value) and value >= 0.0,
            f"resource-usage.txt {column} is not a finite nonnegative number",
        )
    require(usage.get("exit_status") == "0", "GNU time reports a nonzero exit status")
    expected_profile = {
        "instrumentation": "memory",
        "allocation_memory": {
            "source": "profiling-memory global allocator counters",
            **values,
            "peak_growth_bytes": peak - baseline,
            "net_live_growth_bytes": values["final_live_bytes"] - baseline,
            "allocated_bytes_per_frame": float(values["allocated_bytes"]),
            "deallocated_bytes_per_frame": float(values["deallocated_bytes"]),
            "allocation_calls_per_frame": float(values["allocation_calls"]),
            "deallocation_calls_per_frame": float(values["deallocation_calls"]),
        },
        "process_memory": {
            "max_rss_kib": rss,
            "user_cpu_secs": float(usage["user_cpu_secs"]),
            "system_cpu_secs": float(usage["system_cpu_secs"]),
            "exit_status": 0,
        },
    }
    require(
        read_json(run_dir / "profile-artifact.json") == expected_profile,
        "profile-artifact.json differs from raw memory/RSS sidecars",
    )
    return growth, rss


def require_exact_artifact_entries(
    directory: Path,
    *,
    directories: set[str],
    files: set[str],
    label: str,
) -> None:
    """Require a closed, regular artifact directory without hidden payloads."""
    require(directory.is_dir() and not directory.is_symlink(), f"{label} is not a real directory")
    expected = directories | files
    actual = {path.name for path in directory.iterdir()}
    require(
        actual == expected,
        f"{label} artifact set differs: {sorted(actual ^ expected)}",
    )
    for name in directories:
        path = directory / name
        require(path.is_dir() and not path.is_symlink(), f"{label}/{name} is not a real directory")
    for name in files:
        path = directory / name
        require(path.is_file() and not path.is_symlink(), f"{label}/{name} is not a regular file")


def require_no_save_body_in_save_transaction_artifacts(session: Path) -> None:
    """Reject retained save payloads even when disguised under an allowed name."""
    forbidden_markers = (
        b"HELL_WORKERS_SAVE",
        b"HW_PERF_SAVE_RUNTIME_ROOT=",
        b".save-transaction-runtime/",
    )
    for path in sorted(session.rglob("*")):
        require(not path.is_symlink(), f"save-transaction artifact contains a symlink: {path}")
        if path.is_dir():
            continue
        require(path.is_file(), f"save-transaction artifact has a non-regular path: {path}")
        size = path.stat().st_size
        require(
            size <= MAX_SAVE_TRANSACTION_ARTIFACT_FILE_BYTES,
            f"save-transaction artifact file exceeds the bounded limit: {path.name}",
        )
        contents = path.read_bytes()
        require(
            not any(marker in contents for marker in forbidden_markers),
            f"save-transaction artifact retains a serialized body or runtime path: {path}",
        )


def validate_save_transaction_session_file_set(
    session: Path,
    *,
    instrumentation: str,
    case_ids: set[str],
) -> None:
    require(instrumentation in SAVE_TRANSACTION_LEGS, "invalid save-transaction instrumentation")
    require_exact_artifact_entries(
        session,
        directories={"cases"},
        files={"manifest.json", "matrix.json", "aggregate.csv", "report.md"},
        label="save-transaction session",
    )
    cases_dir = session / "cases"
    require_exact_artifact_entries(
        cases_dir,
        directories=case_ids,
        files=set(),
        label="save-transaction cases",
    )
    labels = {label for label, _ in save_transaction_labels()}
    run_files = {
        "command.txt",
        "requested-environment.json",
        "run.log",
        "validation.json",
        "run-metadata.json",
    }
    if instrumentation == "memory":
        run_files |= {"resource-usage.txt", "profile-artifact.json"}
    data_files = {"window.csv", "save_transaction.csv"}
    if instrumentation == "memory":
        data_files.add("memory.csv")
    for case_id in sorted(case_ids):
        case_dir = cases_dir / case_id
        require_exact_artifact_entries(
            case_dir,
            directories=labels,
            files=set(),
            label=f"save-transaction case {case_id}",
        )
        for label in sorted(labels):
            run_dir = case_dir / label
            require_exact_artifact_entries(
                run_dir,
                directories={"data"},
                files=run_files,
                label=f"save-transaction run {case_id}/{label}",
            )
            require_exact_artifact_entries(
                run_dir / "data",
                directories=set(),
                files=data_files,
                label=f"save-transaction data {case_id}/{label}",
            )
    require_no_save_body_in_save_transaction_artifacts(session)


def revalidate_save_transaction_raw_run(
    *,
    repo: Path,
    run_dir: Path,
    case: dict[str, Any],
    expected_backend: str,
) -> None:
    """Use the raw perf parser instead of trusting a stored validation result."""
    scripts_directory = str((repo / "scripts").resolve())
    if scripts_directory not in sys.path:
        sys.path.insert(0, scripts_directory)
    from perf_tool.artifacts import validate_run
    from perf_tool.model import Case

    expected_case = Case(**{key: value for key, value in case.items() if key != "id"})
    validation = validate_run(
        run_dir,
        returncode=0,
        expected_case=expected_case,
        expected_adapter=None,
        expected_backend=expected_backend,
        allow_log_patterns=[],
        capture_kind="frame-time",
        expected_warmup_secs=SAVE_TRANSACTION_WARMUP_SECS,
        expected_measure_secs=SAVE_TRANSACTION_MEASURE_SECS,
        expected_fixed_hz=None,
        expected_warmup_ticks=None,
        expected_audit_ticks=None,
        expected_window_backend="headless",
        expected_present_mode="novsync",
        expected_window_width=None,
        expected_window_height=None,
        expected_window_scale_factor=None,
        expected_rtt_quality=None,
        expected_contract=None,
        expected_stage=None,
        expected_lane=None,
    )
    require(
        validation.valid,
        "raw save-transaction validation failed: " + "; ".join(validation.reasons),
    )


def save_transaction_matrix(seed: int) -> dict[str, Any]:
    """The complete formal C2 matrix retained by `scripts/perf.py`."""
    return {
        "workload": "save-transaction",
        "sizes": list(SAVE_TRANSACTION_SIZES),
        "renders": ["cpu"],
        "seed": seed,
        "repeat": SAVE_TRANSACTION_REPEAT,
        "warmup_secs": SAVE_TRANSACTION_WARMUP_SECS,
        "measure_secs": SAVE_TRANSACTION_MEASURE_SECS,
        "fixed_hz": None,
        "warmup_ticks": None,
        "audit_ticks": None,
        "preflight_runs": SAVE_TRANSACTION_PREFLIGHT_RUNS,
        "souls": None,
        "familiars": None,
        "familiar_policies": ["baseline"],
        "operation_dialog_modes": ["hidden"],
        "dashboard_modes": ["hidden"],
        "behavior_cases": [],
        "capture_kind": "frame-time",
        "clock_mode": "realtime",
        "warmup_checksum_policy": "record",
        "measure_end_checksum_policy": "record",
        "allow_log_patterns": [],
        "tracy_capture_secs": None,
        "window_width": None,
        "window_height": None,
        "window_scale_factor": None,
        "rtt_quality": None,
        "environment_lock": None,
        "rtt_light_contract": None,
        "save_transaction_runtime": {"isolated": True, "cleanup": "per-run"},
    }


def save_transaction_requested_environment(repo: Path, backend: str) -> dict[str, str]:
    require(backend in {"vulkan", "gl"}, "save-transaction backend is unsupported")
    return {
        "BEVY_ASSET_ROOT": str(repo.resolve()),
        "HW_PRESENT_MODE": "novsync",
        "HW_WINDOW_BACKEND": "headless",
        "WGPU_BACKEND": backend,
    }


def verify_save_transaction_session(
    session: Path,
    *,
    repo: Path,
    instrumentation: str,
    expected_source: str,
    expected_seed: int,
    expected_backend: str,
) -> dict[str, Any]:
    """Re-read a save timing leg without trusting its prior validation JSON.

    The retained session has scalar metrics only. Each operation has already
    removed its own isolated body-containing runtime before its artifact is
    finalized, so this verifier proves the per-run cleanup contract rather
    than retaining a serialized save as evidence.
    """
    session = session.resolve()
    manifest = read_json(session / "manifest.json")
    require(manifest.get("status") == "valid", f"save-transaction session is not valid: {session}")
    binary = manifest.get("binary")
    require(isinstance(binary, dict), "save-transaction binary metadata is missing")
    binary_hash = binary.get("sha256")
    require(
        isinstance(binary_hash, str) and re.fullmatch(r"[0-9a-f]{64}", binary_hash) is not None,
        "save-transaction binary fingerprint is invalid",
    )
    require(binary.get("instrumentation") == instrumentation, "save-transaction instrumentation mismatch")
    source = manifest.get("source")
    require(isinstance(source, dict), "save-transaction source provenance is missing")
    require(
        source.get("fingerprint_start") == expected_source
        and source.get("fingerprint_end") == expected_source
        and source.get("unchanged") is True,
        "save-transaction source fingerprint differs from the native job",
    )
    matrix = manifest.get("matrix")
    require(isinstance(matrix, dict), "save-transaction matrix is missing")
    require(
        matrix == save_transaction_matrix(expected_seed),
        "save-transaction matrix differs from the fixed C2 contract",
    )
    require(
        read_json(session / "matrix.json") == matrix,
        "save-transaction matrix.json differs from its manifest",
    )
    requested = manifest.get("requested_environment")
    require(
        requested == save_transaction_requested_environment(repo, expected_backend),
        "save-transaction requested environment differs from the fixed C2 contract",
    )

    cases = manifest.get("cases")
    require(isinstance(cases, list) and len(cases) == len(SAVE_TRANSACTION_SIZES), "save-transaction cases are incomplete")
    case_by_id = {
        case.get("id"): case
        for case in cases
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    require(len(case_by_id) == len(SAVE_TRANSACTION_SIZES), "save-transaction case IDs are invalid")
    require(
        {case.get("size") for case in case_by_id.values()} == set(SAVE_TRANSACTION_SIZES)
        and all(case.get("render") == "cpu" for case in case_by_id.values()),
        "save-transaction cases must be one CPU case per canonical size",
    )
    for case_id, case in case_by_id.items():
        size = case.get("size")
        expected_case_id = f"save-transaction-{size}-cpu-seed-{expected_seed}"
        require(
            case
            == {
                "id": expected_case_id,
                "workload": "save-transaction",
                "size": size,
                "render": "cpu",
                "seed": expected_seed,
                "souls": None,
                "familiars": None,
                "familiar_policy": "baseline",
                "operation_dialog": "hidden",
                "dashboard_mode": "hidden",
                "behavior_case": None,
            },
            "save-transaction case does not match its fixed fixture",
        )
    validate_save_transaction_session_file_set(
        session,
        instrumentation=instrumentation,
        case_ids=set(case_by_id),
    )
    cases_dir = session / "cases"

    per_case: dict[str, dict[str, Any]] = {}
    fixture_checksums: dict[str, str] = {}
    for case_id, case in sorted(case_by_id.items()):
        case_dir = cases_dir / case_id
        expected_labels = {label for label, _ in save_transaction_labels()}
        require(
            {path.name for path in case_dir.iterdir() if path.is_dir()} == expected_labels,
            f"{case_id} does not have the exact preflight/measured run set",
        )
        measured_values = {
            "serialize_ns": [],
            "write_file_sync_ns": [],
            "commit_directory_sync_ns": [],
            "total_ns": [],
        }
        growth_values: list[int] = []
        rss_values: list[int] = []
        observed_checksums: set[str] = set()
        observed_populations: set[tuple[int, int]] = set()
        expected_population = SAVE_TRANSACTION_POPULATIONS.get(case.get("size"))
        require(
            expected_population is not None,
            f"{case_id} uses an unknown save-transaction fixture size",
        )
        for label, preflight in save_transaction_labels():
            run_dir = case_dir / label
            validation = read_json(run_dir / "validation.json")
            require(
                validation.get("valid") is True and validation.get("reasons") == [],
                f"{case_id}/{label} stored validation is not valid",
            )
            metadata = read_json(run_dir / "run-metadata.json")
            require(metadata.get("preflight") is preflight, f"{case_id}/{label} preflight flag is stale")
            require(metadata.get("returncode") == 0, f"{case_id}/{label} did not exit successfully")
            require(
                metadata.get("case")
                == {key: value for key, value in case.items() if key != "id"},
                f"{case_id}/{label} run metadata case differs from its manifest",
            )
            require(
                metadata.get("runtime_data_cleanup") == "per-run"
                and metadata.get("runtime_data_cleaned") is True,
                f"{case_id}/{label} runtime cleanup contract is not satisfied",
            )
            require(
                read_json(run_dir / "requested-environment.json") == requested,
                f"{case_id}/{label} requested environment differs from its manifest",
            )
            revalidate_save_transaction_raw_run(
                repo=repo,
                run_dir=run_dir,
                case=case,
                expected_backend=expected_backend,
            )
            command = (run_dir / "command.txt").read_text(encoding="utf-8")
            require("HW_PERF_SAVE_RUNTIME_ROOT" not in command, f"{case_id}/{label} leaked runtime environment")
            require("<absolute-path>" in command, f"{case_id}/{label} command record was not path-redacted")
            rows = read_exact_csv(
                run_dir / "data" / "save_transaction.csv",
                SAVE_TRANSACTION_COLUMNS,
                "save_transaction.csv",
            )
            require(len(rows) == 1, f"{case_id}/{label} must have one save-transaction row")
            row = rows[0]
            require(row.get("schema_version") == SAVE_TRANSACTION_SCHEMA_VERSION, "save-transaction schema mismatch")
            require(
                {
                    "workload": row.get("workload"),
                    "size": row.get("size"),
                    "render": row.get("render"),
                    "seed": row.get("seed"),
                    "sample_kind": row.get("sample_kind"),
                }
                == {
                    "workload": "save-transaction",
                    "size": case.get("size"),
                    "render": "cpu",
                    "seed": str(expected_seed),
                    "sample_kind": "preflight" if preflight else "measured",
                },
                f"{case_id}/{label} sidecar does not match its fixture metadata",
            )
            checksum = row.get("fixture_checksum")
            require(
                isinstance(checksum, str) and re.fullmatch(r"[0-9a-f]{16}", checksum) is not None,
                f"{case_id}/{label} fixture checksum is invalid",
            )
            observed_checksums.add(checksum)
            for column in (
                "measure_virtual_ns",
                "measure_real_ns",
                "body_bytes",
                "soul_count",
                "familiar_count",
                "serialize_ns",
                "write_file_sync_ns",
                "commit_directory_sync_ns",
                "total_ns",
            ):
                canonical_nonnegative(row, column, "save_transaction.csv")
            serialize_ns = canonical_nonnegative(row, "serialize_ns", "save_transaction.csv")
            write_file_sync_ns = canonical_nonnegative(
                row, "write_file_sync_ns", "save_transaction.csv"
            )
            commit_directory_sync_ns = canonical_nonnegative(
                row, "commit_directory_sync_ns", "save_transaction.csv"
            )
            total_ns = canonical_nonnegative(row, "total_ns", "save_transaction.csv")
            require(
                total_ns >= serialize_ns + write_file_sync_ns + commit_directory_sync_ns,
                f"{case_id}/{label} total duration is shorter than its measured phases",
            )
            completion_marker = (
                "PERF_CAPTURE: wrote save-transaction sample "
                f"(total_ns={total_ns})"
            )
            require(
                completion_marker in (run_dir / "run.log").read_text(encoding="utf-8"),
                f"{case_id}/{label} completion log does not match the transaction total",
            )
            population = (
                canonical_nonnegative(row, "soul_count", "save_transaction.csv"),
                canonical_nonnegative(row, "familiar_count", "save_transaction.csv"),
            )
            require(
                population == expected_population,
                f"{case_id}/{label} population differs from its canonical fixture",
            )
            observed_populations.add(population)
            require(canonical_nonnegative(row, "body_bytes", "save_transaction.csv") > 0, "save body must be nonempty")
            require(
                canonical_nonnegative(row, "measure_virtual_ns", "save_transaction.csv")
                >= SAVE_TRANSACTION_MEASURE_NS
                and canonical_nonnegative(row, "measure_real_ns", "save_transaction.csv")
                >= SAVE_TRANSACTION_MEASURE_NS,
                f"{case_id}/{label} did not complete the fixed two-second measurement window",
            )
            if instrumentation == "capture":
                require(row.get("peak_live_growth_bytes") == "", "Capture sidecar must not report allocator growth")
                require(not (run_dir / "data" / "memory.csv").exists(), "Capture must not retain memory.csv")
            else:
                growth, rss = validate_save_transaction_memory_sidecars(run_dir, row)
            if not preflight:
                if instrumentation == "memory":
                    growth_values.append(growth)
                    rss_values.append(rss)
                for column in measured_values:
                    measured_values[column].append(canonical_nonnegative(row, column, "save_transaction.csv"))
        require(len(observed_checksums) == 1, f"{case_id} fixture checksum drifted across samples")
        require(len(observed_populations) == 1, f"{case_id} fixture population drifted across samples")
        fixture_checksums[case_id] = next(iter(observed_checksums))
        per_case[case_id] = {
            "metrics": measured_values,
            "growth": growth_values,
            "rss": rss_values,
            "population": next(iter(observed_populations)),
        }

    aggregate_rows = read_exact_csv(
        session / "aggregate.csv", SAVE_TRANSACTION_AGGREGATE_COLUMNS, "aggregate.csv"
    )
    aggregate_ids = [row.get("case_id") for row in aggregate_rows]
    require(
        len(aggregate_rows) == len(case_by_id),
        "save-transaction aggregate row count differs from the fixed case matrix",
    )
    require(
        all(isinstance(case_id, str) and case_id for case_id in aggregate_ids),
        "save-transaction aggregate has an empty case ID",
    )
    require(
        len(set(aggregate_ids)) == len(aggregate_ids),
        "save-transaction aggregate contains duplicate case IDs",
    )
    aggregate_by_id = {row["case_id"]: row for row in aggregate_rows}
    require(set(aggregate_by_id) == set(case_by_id), "save-transaction aggregate case set is incomplete")
    for case_id, case_data in per_case.items():
        row = aggregate_by_id[case_id]
        require(row.get("valid_runs") == str(SAVE_TRANSACTION_REPEAT), f"{case_id} aggregate valid run count is stale")
        require(row.get("fixture_checksums") == fixture_checksums[case_id], f"{case_id} aggregate fixture checksum is stale")
        for source, prefix in (
            ("serialize_ns", "serialize"),
            ("write_file_sync_ns", "write_file_sync"),
            ("commit_directory_sync_ns", "commit_directory_sync"),
            ("total_ns", "total"),
        ):
            values = case_data["metrics"][source]
            require(len(values) == SAVE_TRANSACTION_REPEAT, f"{case_id} has incomplete measured values")
            require(row.get(f"{prefix}_p95_ns") == str(nearest_rank_p95(values)), f"{case_id} {prefix} p95 is stale")
            require(row.get(f"{prefix}_max_ns") == str(max(values)), f"{case_id} {prefix} max is stale")
        if instrumentation == "capture":
            require(
                row.get("peak_live_growth_p95_bytes") == ""
                and row.get("peak_live_growth_max_bytes") == ""
                and row.get("max_rss_kib_max") == "",
                "Capture aggregate must not contain Memory/RSS metrics",
            )
            if case_by_id[case_id].get("size") == "large":
                require(
                    int(row["total_p95_ns"]) <= SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS,
                    "large Capture total p95 exceeds the C2 100ms limit",
                )
                require(
                    int(row["total_max_ns"]) <= SAVE_TRANSACTION_LARGE_TOTAL_MAX_LIMIT_NS,
                    "large Capture total max exceeds the C2 250ms limit",
                )
        else:
            growth_values = case_data["growth"]
            rss_values = case_data["rss"]
            require(len(growth_values) == SAVE_TRANSACTION_REPEAT, f"{case_id} Memory growth set is incomplete")
            require(len(rss_values) == SAVE_TRANSACTION_REPEAT, f"{case_id} Memory RSS set is incomplete")
            require(
                row.get("peak_live_growth_p95_bytes") == str(nearest_rank_p95(growth_values))
                and row.get("peak_live_growth_max_bytes") == str(max(growth_values))
                and row.get("max_rss_kib_max") == str(max(rss_values)),
                f"{case_id} Memory aggregate is stale",
            )
    return {
        "status": "pass",
        "instrumentation": instrumentation,
        "binary_sha256": binary_hash,
        "fixture_checksums": fixture_checksums,
        "populations": {case_id: data["population"] for case_id, data in per_case.items()},
        "runtime_cleanup": "per-run",
    }


def verify_save_transaction_bundle(
    *,
    repo: Path,
    performance_root: Path,
    expected_source: str,
    expected_seed: int,
    expected_backend: str,
) -> dict[str, Any]:
    performance_root = performance_root.resolve()
    require_exact_artifact_entries(
        performance_root,
        directories=set(SAVE_TRANSACTION_LEGS),
        files=set(),
        label="save-transaction performance root",
    )
    capture = verify_save_transaction_session(
        performance_root / "capture",
        repo=repo,
        instrumentation="capture",
        expected_source=expected_source,
        expected_seed=expected_seed,
        expected_backend=expected_backend,
    )
    memory = verify_save_transaction_session(
        performance_root / "memory",
        repo=repo,
        instrumentation="memory",
        expected_source=expected_source,
        expected_seed=expected_seed,
        expected_backend=expected_backend,
    )
    require(
        capture["binary_sha256"] != memory["binary_sha256"],
        "Capture and Memory must use distinct instrumentation binaries",
    )
    require(
        capture["fixture_checksums"] == memory["fixture_checksums"],
        "Capture and Memory fixture checksums differ",
    )
    require(
        capture["populations"] == memory["populations"],
        "Capture and Memory fixture populations differ",
    )
    return {
        "status": "pass",
        "capture": capture,
        "memory": memory,
        "runtime_cleanup": "per-run",
    }


def save_transaction_runtime_root(repo: Path, run_id: str) -> Path:
    require(
        re.fullmatch(r"c2-[0-9]{8}T[0-9]{6}Z-[0-9a-f]{8}", run_id) is not None,
        "save-transaction runtime cleanup run ID is invalid",
    )
    allowed_root = (repo.resolve() / "target" / ".save-transaction-runtime").resolve()
    root = (allowed_root / run_id).resolve()
    require(root.parent == allowed_root, "save-transaction cleanup escaped its exact runtime root")
    return root


def cleanup_save_transaction_runtime_root(repo: Path, run_id: str) -> bool:
    root = save_transaction_runtime_root(repo, run_id)
    if not root.exists():
        return False
    require(root.is_dir() and not root.is_symlink(), "save-transaction runtime cleanup target is unsafe")
    shutil.rmtree(root)
    return True


def remove_empty_save_transaction_runtime_root(repo: Path, run_id: str) -> bool:
    """Remove only the native recipe's empty run-ID directory.

    Individual perf operations own and remove their body-containing roots.
    The outer recipe may then remove the fresh run-ID parent only if it is
    already empty; a nonempty directory is evidence of a cleanup failure and
    must remain visible to the caller rather than being recursively erased.
    """
    root = save_transaction_runtime_root(repo, run_id)
    if not root.exists():
        return False
    require(root.is_dir() and not root.is_symlink(), "save-transaction runtime parent is unsafe")
    try:
        root.rmdir()
    except OSError as error:
        raise AcceptanceError(
            "save-transaction runtime parent is not empty after per-run cleanup"
        ) from error
    return True


def require_save_transaction_runtime_absent(repo: Path, run_id: str) -> None:
    root = save_transaction_runtime_root(repo, run_id)
    require(
        not root.exists(),
        "save-transaction runtime data remains after per-run/helper cleanup",
    )


def save_transaction_perf_command(
    *,
    run_id: str,
    seed: int,
    backend: str,
    instrumentation: str,
    output: Path,
    runtime_root: Path,
) -> list[str]:
    require(instrumentation in SAVE_TRANSACTION_LEGS, "invalid save-transaction instrumentation")
    # Keep argv retained by the native orchestrator relative. The perf runner
    # resolves it against the repository, while artifacts never learn the
    # body-containing runtime path.
    return [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "save-transaction",
        "--sizes",
        ",".join(SAVE_TRANSACTION_SIZES),
        "--renders",
        "cpu",
        "--seed",
        str(seed),
        "--backend",
        backend,
        "--window-backend",
        "headless",
        "--present-mode",
        "novsync",
        "--instrumentation",
        instrumentation,
        "--repeat",
        str(SAVE_TRANSACTION_REPEAT),
        "--preflight-runs",
        str(SAVE_TRANSACTION_PREFLIGHT_RUNS),
        "--warmup-secs",
        str(SAVE_TRANSACTION_WARMUP_SECS),
        "--measure-secs",
        str(SAVE_TRANSACTION_MEASURE_SECS),
        "--timeout-secs",
        "120",
        "--output",
        str(output),
        "--save-runtime-root",
        str(runtime_root),
    ]


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


def count_save_catalog_marker_pixels(png: bytes) -> int:
    """Decode the bounded screenshot and require the native-only UI marker.

    Native capture tools emit non-interlaced 8-bit RGB/RGBA PNGs. Rejecting
    other representations is intentional: the capture command can normalize
    ImageMagick output, and a verifier must not guess at an unproven image.
    """
    width, height = validate_png_structure(png)
    offset = 8
    bit_depth: int | None = None
    color_type: int | None = None
    compressed = bytearray()
    while offset < len(png):
        data_len = int.from_bytes(png[offset : offset + 4], "big")
        chunk_type = png[offset + 4 : offset + 8]
        data_start = offset + 8
        data_end = data_start + data_len
        if chunk_type == b"IHDR":
            bit_depth = png[data_start + 8]
            color_type = png[data_start + 9]
            require(
                png[data_start + 10 : data_start + 13] == b"\x00\x00\x00",
                "native screenshot uses an unsupported PNG compression/filter/interlace mode",
            )
        elif chunk_type == b"IDAT":
            compressed.extend(png[data_start:data_end])
        elif chunk_type == b"IEND":
            break
        offset = data_end + 4
    require(bit_depth == 8 and color_type in {2, 6}, "native screenshot must be 8-bit RGB/RGBA PNG")
    channels = 3 if color_type == 2 else 4
    stride = width * channels
    expected_length = height * (stride + 1)
    require(
        expected_length <= MAX_NATIVE_SCREENSHOT_DECODED_BYTES,
        "native screenshot decoded pixel buffer exceeds the fail-closed limit",
    )
    try:
        decoder = zlib.decompressobj()
        raw = decoder.decompress(bytes(compressed), expected_length + 1)
        require(not decoder.unconsumed_tail, "native screenshot decompressed beyond its declared dimensions")
        raw += decoder.flush()
    except zlib.error as error:
        raise AcceptanceError(f"native screenshot PNG data cannot be decompressed: {error}") from error
    require(len(raw) == expected_length, "native screenshot decoded length is invalid")

    previous = bytearray(stride)
    marker_pixels = 0
    cursor = 0
    for _ in range(height):
        filter_type = raw[cursor]
        cursor += 1
        encoded = raw[cursor : cursor + stride]
        cursor += stride
        scanline = bytearray(stride)
        for index, value in enumerate(encoded):
            left = scanline[index - channels] if index >= channels else 0
            up = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 0:
                reconstructed = value
            elif filter_type == 1:
                reconstructed = (value + left) & 0xFF
            elif filter_type == 2:
                reconstructed = (value + up) & 0xFF
            elif filter_type == 3:
                reconstructed = (value + ((left + up) // 2)) & 0xFF
            elif filter_type == 4:
                predictor = left + up - upper_left
                distance_left = abs(predictor - left)
                distance_up = abs(predictor - up)
                distance_upper_left = abs(predictor - upper_left)
                nearest = (
                    left
                    if distance_left <= distance_up and distance_left <= distance_upper_left
                    else up if distance_up <= distance_upper_left else upper_left
                )
                reconstructed = (value + nearest) & 0xFF
            else:
                raise AcceptanceError("native screenshot uses an unknown PNG scanline filter")
            scanline[index] = reconstructed
        for index in range(0, stride, channels):
            red, green, blue = scanline[index : index + 3]
            if red >= 250 and green <= 5 and blue >= 250:
                marker_pixels += 1
        previous = scanline
    return marker_pixels


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


def save_catalog_marker_png(
    width: int, height: int, *, include_marker: bool = True
) -> bytes:
    require(width >= 48 and height >= 48, "marker PNG dimensions are too small")
    row = bytearray(width * 3)
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        pixels = bytearray(row)
        if include_marker and 12 <= y < 60:
            for x in range(12, 60):
                offset = x * 3
                pixels[offset : offset + 3] = bytes(SAVE_CATALOG_MARKER_RGB)
        raw.extend(pixels)
    ihdr = width.to_bytes(4, "big") + height.to_bytes(4, "big") + bytes((8, 2, 0, 0, 0))
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", zlib.compress(bytes(raw)))
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


def verify_notifications_command(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    require(
        native_harness_fingerprint(repo) == args.harness_fingerprint,
        "native acceptance harness fingerprint differs from the A2 verification",
    )
    result = verify_notifications_artifact(
        Path(args.artifact).resolve(),
        runtime_root=Path(args.runtime_root).resolve(),
        run_id=args.run_id,
        adapter=args.adapter,
        backend=args.backend,
        window_backend=args.window_backend,
    )
    print_json(result)
    return 0


def verify_save_catalog_command(args: argparse.Namespace) -> int:
    repo = validate_repo(args.repo)
    require(
        native_harness_fingerprint(repo) == args.harness_fingerprint,
        "native acceptance harness fingerprint differs from the requested verification",
    )
    native = verify_save_catalog_artifact(
        Path(args.artifact).resolve(),
        run_id=args.run_id,
        adapter=args.adapter,
        backend=args.backend,
        window_backend=args.window_backend,
        runtime_root=Path(args.runtime_root).resolve(),
    )
    save_transaction = verify_save_transaction_bundle(
        repo=repo,
        performance_root=Path(args.performance_root).resolve(),
        expected_source=args.source_fingerprint,
        expected_seed=args.seed,
        expected_backend=args.backend,
    )
    require_save_transaction_runtime_absent(repo, args.run_id)
    print_json(
        {
            "status": "pass",
            "native": native,
            "save_transaction": save_transaction,
            "runtime_cleanup": "verified-absent",
        }
    )
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
    if status == "valid" and state.get("profile") == "player-facing-result-notifications":
        parameters = state.get("parameters", {})
        try:
            expected_harness = str(state["harness_fingerprint"])
            require(
                native_harness_fingerprint(Path(state["repo"])) == expected_harness,
                "native acceptance harness differs from the completed A2 job",
            )
            summary["verification"] = verify_notifications_artifact(
                Path(state["paths"]["artifact"]),
                runtime_root=Path(state["paths"]["runtime"]),
                run_id=str(state["run_id"]),
                adapter=str(parameters.get("adapter", "Intel")),
                backend=str(parameters.get("backend", "vulkan")),
                window_backend=str(parameters.get("window_backend", "x11")),
            )
        except (AcceptanceError, KeyError, TypeError, ValueError) as error:
            summary["status"] = "invalid"
            summary["error"] = f"A2 artifact revalidation failed: {error}"
            print_json(summary)
            return 1
    if status == "valid" and state.get("profile") == "save-catalog":
        parameters = state.get("parameters", {})
        try:
            native = verify_save_catalog_artifact(
                Path(state["paths"]["artifact"]),
                run_id=str(state["run_id"]),
                adapter=str(parameters.get("adapter", "Intel")),
                backend=str(parameters.get("backend", "vulkan")),
                window_backend=str(parameters.get("window_backend", "x11")),
                runtime_root=Path(state["paths"]["runtime"]),
            )
            performance = state["paths"]["performance"]
            stored_verification = state.get("verification")
            expected_harness = state.get("harness_fingerprint")
            require(
                isinstance(stored_verification, dict)
                and stored_verification.get("runtime_cleanup") == "verified-absent",
                "save-transaction runtime cleanup was not recorded",
            )
            require(
                isinstance(expected_harness, str)
                and re.fullmatch(r"[0-9a-f]{64}", expected_harness) is not None
                and native_harness_fingerprint(Path(state["repo"])) == expected_harness,
                "native acceptance harness fingerprint differs from the completed job",
            )
            harness_record = stored_verification.get("harness")
            require(
                harness_record
                == {
                    "fingerprint_start": expected_harness,
                    "fingerprint_end": expected_harness,
                    "unchanged": True,
                },
                "native acceptance harness provenance is incomplete",
            )
            save_transaction = verify_save_transaction_bundle(
                repo=Path(state["repo"]),
                performance_root=Path(performance["capture"]).parent,
                expected_source=str(state["source_fingerprint"]),
                expected_seed=int(parameters["seed"]),
                expected_backend=str(parameters["backend"]),
            )
            require_save_transaction_runtime_absent(
                Path(state["repo"]), str(state["run_id"])
            )
            summary["verification"] = {
                "status": "pass",
                "native": native,
                "save_transaction": save_transaction,
                "runtime_cleanup": "verified-absent",
            }
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
    stage: str = "current",
) -> None:
    path.mkdir(parents=True)
    fixed = leg == "audit"
    case_count = 1 if fixed else 6
    matrix = {
        "workload": "indoor-light",
        "rtt_light_contract": {
            "contract_id": RTT_LIGHT_CONTRACT_ID,
            "stage_id": stage,
            "lane": "static",
            "measurement_contract_sha256": "a" * 64,
            "fixture_contract_sha256": "b" * 64,
            "lifecycle": {
                "status": "frozen",
                "formal_registration_allowed": True,
                "freeze_blockers": [],
            },
        },
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


def write_fake_save_transaction_session(
    path: Path,
    *,
    repo: Path,
    instrumentation: str,
    binary_hash: str,
    source_fingerprint_value: str,
    seed: int,
) -> None:
    """Build a compact but exact C2 perf leg fixture for helper self-tests."""
    from perf_tool.model import WINDOW_COLUMNS

    path.mkdir(parents=True)
    cases: list[dict[str, Any]] = []
    aggregate_rows: list[dict[str, str]] = []
    for size_index, size in enumerate(SAVE_TRANSACTION_SIZES, start=1):
        case_id = f"save-transaction-{size}-cpu-seed-{seed}"
        case = {
            "id": case_id,
            "workload": "save-transaction",
            "size": size,
            "render": "cpu",
            "seed": seed,
            "souls": None,
            "familiars": None,
            "familiar_policy": "baseline",
            "operation_dialog": "hidden",
            "dashboard_mode": "hidden",
            "behavior_case": None,
        }
        cases.append(case)
        checksum = f"{size_index:016x}"
        souls, familiars = SAVE_TRANSACTION_POPULATIONS[size]
        measured: dict[str, list[int]] = {
            "serialize_ns": [],
            "write_file_sync_ns": [],
            "commit_directory_sync_ns": [],
            "total_ns": [],
        }
        growth: list[int] = []
        rss: list[int] = []
        for run_index, (label, preflight) in enumerate(save_transaction_labels(), start=1):
            run_dir = path / "cases" / case_id / label
            data = run_dir / "data"
            data.mkdir(parents=True)
            total = 10_000 + size_index * 100 + run_index
            row = {
                "schema_version": SAVE_TRANSACTION_SCHEMA_VERSION,
                "workload": "save-transaction",
                "size": size,
                "render": "cpu",
                "seed": str(seed),
                "soul_count": str(souls),
                "familiar_count": str(familiars),
                "fixture_checksum": checksum,
                "sample_kind": "preflight" if preflight else "measured",
                "measure_virtual_ns": str(SAVE_TRANSACTION_MEASURE_NS),
                "measure_real_ns": str(SAVE_TRANSACTION_MEASURE_NS),
                "body_bytes": str(1024 * size_index),
                "serialize_ns": str(100 + run_index),
                "write_file_sync_ns": str(200 + run_index),
                "commit_directory_sync_ns": str(300 + run_index),
                "total_ns": str(total),
                "peak_live_growth_bytes": "" if instrumentation == "capture" else str(50 + run_index),
            }
            with (data / "save_transaction.csv").open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(handle, fieldnames=SAVE_TRANSACTION_COLUMNS)
                writer.writeheader()
                writer.writerow(row)
            window = {
                "schema_version": "3",
                "window_present": "false",
                "logical_width": "",
                "logical_height": "",
                "physical_width": "",
                "physical_height": "",
                "scale_factor": "",
                "rtt_quality": "high",
                "scene_target_width": "1280",
                "scene_target_height": "720",
                "mask_target_present": "false",
                "mask_target_width": "0",
                "mask_target_height": "0",
                "target_scale_factor": "1.000000",
                "resolved_window_backend": "",
                "adapter_name": "",
                "adapter_backend": "",
                "requested_present_mode": "",
                "effective_present_mode": "",
                "end_window_present": "false",
                "end_logical_width": "",
                "end_logical_height": "",
                "end_physical_width": "",
                "end_physical_height": "",
                "end_scale_factor": "",
                "end_rtt_quality": "high",
                "end_scene_target_width": "1280",
                "end_scene_target_height": "720",
                "end_mask_target_present": "false",
                "end_mask_target_width": "0",
                "end_mask_target_height": "0",
                "end_target_scale_factor": "1.000000",
                "end_resolved_window_backend": "",
                "end_adapter_name": "",
                "end_adapter_backend": "",
                "end_requested_present_mode": "",
                "end_effective_present_mode": "",
            }
            with (data / "window.csv").open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(handle, fieldnames=WINDOW_COLUMNS)
                writer.writeheader()
                writer.writerow(window)
            (run_dir / "command.txt").write_text(
                "<absolute-path> --perf-output <absolute-path>\n", encoding="utf-8"
            )
            atomic_write_json(
                run_dir / "requested-environment.json",
                save_transaction_requested_environment(repo, "vulkan"),
            )
            (run_dir / "run.log").write_text(
                (
                    f"PERF_SCENARIO: seed={seed} workload=save-transaction size={size} "
                    f"souls={souls} familiars={familiars} render=cpu "
                    "clock=realtime behavior_case=none familiar_policy=baseline "
                    "operation_dialog=hidden dashboard_mode=hidden warmup=1s measure=2s\n"
                    "AdapterInfo { name: \"Test GPU\", driver: \"test\", "
                    "driver_info: \"test\", backend: Vulkan }\n"
                    f"PERF_CAPTURE: wrote save-transaction sample (total_ns={total})\n"
                ),
                encoding="utf-8",
            )
            atomic_write_json(run_dir / "validation.json", {"valid": True, "reasons": []})
            atomic_write_json(
                run_dir / "run-metadata.json",
                {
                    "case": {key: value for key, value in case.items() if key != "id"},
                    "preflight": preflight,
                    "returncode": 0,
                    "runtime_data_cleanup": "per-run",
                    "runtime_data_cleaned": True,
                },
            )
            if instrumentation == "memory":
                value = 50 + run_index
                (data / "memory.csv").write_text(
                    "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
                    "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
                    "reallocation_calls,accounting_errors\n"
                    f"1,100,{100 + value},100,1000,1000,10,9,0,0\n",
                    encoding="utf-8",
                )
                (run_dir / "resource-usage.txt").write_text(
                    "max_rss_kib=2048\nuser_cpu_secs=0.1\nsystem_cpu_secs=0.1\nexit_status=0\n",
                    encoding="utf-8",
                )
                atomic_write_json(
                    run_dir / "profile-artifact.json",
                    {
                        "instrumentation": "memory",
                        "allocation_memory": {
                            "source": "profiling-memory global allocator counters",
                            "baseline_live_bytes": 100,
                            "peak_live_bytes": 100 + value,
                            "final_live_bytes": 100,
                            "allocated_bytes": 1000,
                            "deallocated_bytes": 1000,
                            "allocation_calls": 10,
                            "deallocation_calls": 9,
                            "reallocation_calls": 0,
                            "accounting_errors": 0,
                            "peak_growth_bytes": value,
                            "net_live_growth_bytes": 0,
                            "allocated_bytes_per_frame": 1000.0,
                            "deallocated_bytes_per_frame": 1000.0,
                            "allocation_calls_per_frame": 10.0,
                            "deallocation_calls_per_frame": 9.0,
                        },
                        "process_memory": {
                            "max_rss_kib": 2048,
                            "user_cpu_secs": 0.1,
                            "system_cpu_secs": 0.1,
                            "exit_status": 0,
                        },
                    },
                )
                growth.append(value)
                rss.append(2048)
            if not preflight:
                for column in measured:
                    measured[column].append(int(row[column]))
        aggregate = {
            "case_id": case_id,
            "valid_runs": str(SAVE_TRANSACTION_REPEAT),
            "fixture_checksums": checksum,
            "peak_live_growth_p95_bytes": "",
            "peak_live_growth_max_bytes": "",
            "max_rss_kib_max": "",
            "adapter": "null",
        }
        for source, prefix in (
            ("serialize_ns", "serialize"),
            ("write_file_sync_ns", "write_file_sync"),
            ("commit_directory_sync_ns", "commit_directory_sync"),
            ("total_ns", "total"),
        ):
            aggregate[f"{prefix}_p95_ns"] = str(nearest_rank_p95(measured[source]))
            aggregate[f"{prefix}_max_ns"] = str(max(measured[source]))
        if instrumentation == "memory":
            aggregate["peak_live_growth_p95_bytes"] = str(nearest_rank_p95(growth))
            aggregate["peak_live_growth_max_bytes"] = str(max(growth))
            aggregate["max_rss_kib_max"] = str(max(rss))
        aggregate_rows.append(aggregate)
    (path / "cases").mkdir(exist_ok=True)
    with (path / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=SAVE_TRANSACTION_AGGREGATE_COLUMNS)
        writer.writeheader()
        writer.writerows(aggregate_rows)
    (path / "report.md").write_text("# valid save transaction fixture\n", encoding="utf-8")
    atomic_write_json(path / "matrix.json", save_transaction_matrix(seed))
    atomic_write_json(
        path / "manifest.json",
        {
            "status": "valid",
            "binary": {"instrumentation": instrumentation, "sha256": binary_hash},
            "source": {
                "fingerprint_start": source_fingerprint_value,
                "fingerprint_end": source_fingerprint_value,
                "unchanged": True,
            },
            "requested_environment": save_transaction_requested_environment(repo, "vulkan"),
            "matrix": save_transaction_matrix(seed),
            "cases": cases,
        },
    )


def self_test() -> int:
    repo = Path(__file__).resolve().parents[4]
    sys.path.insert(0, str(repo / "scripts"))
    from perf_tool import execution as perf_execution
    from perf_tool import rtt_light_bundle as perf_bundle

    require(
        parse_linux_proc_stat_ppid("42 (game worker) S 7 1 1 0") == 7,
        "Linux process parent parser did not read field four",
    )
    require(
        parse_linux_proc_stat_ppid("malformed") is None,
        "malformed Linux process stat unexpectedly parsed",
    )
    require(
        parse_x11_client_window_ids(
            "_NET_CLIENT_LIST(WINDOW): window id # 0x00Aa, 0x4, 0x00Aa"
        ) == ["0x00aa", "0x4"],
        "X11 client window list parser did not retain canonical unique IDs",
    )
    require(
        parse_x11_window_pid("_NET_WM_PID(CARDINAL) = 1234") == 1234,
        "X11 window PID parser did not read a valid owner",
    )
    require(
        parse_x11_window_pid("_NET_WM_PID: not found") is None,
        "missing X11 window PID unexpectedly parsed",
    )
    require(
        SOURCE_FILES == perf_execution.SOURCE_FINGERPRINT_FILES
        and SOURCE_PREFIXES == perf_execution.SOURCE_FINGERPRINT_PREFIXES
        and ASSET_PREFIX == perf_execution.SOURCE_FINGERPRINT_ASSET_PREFIX,
        "native and perf source fingerprint boundaries differ",
    )
    require(
        NATIVE_HARNESS_FILES == perf_execution.MEASUREMENT_HARNESS_FILES,
        "native and perf measurement harness boundaries differ",
    )
    require(
        source_fingerprint(repo) == perf_execution.source_fingerprint(),
        "native and perf source fingerprints differ",
    )
    require(
        perf_bundle.measurement_harness_dirty_paths_only(
            [
                "M .codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py",
                " M scripts/perf_tool/execution.py",
                "?? scripts/perf_tool/renderdoc_foundation.py",
            ]
        ),
        "formal bundle rejected exact measurement harness dirty paths",
    )
    for rejected_dirty_paths in (
        [" M crates/bevy_app/src/main.rs"],
        ["R  old.py -> scripts/perf_tool/execution.py"],
        ["INVALID scripts/perf_tool/execution.py"],
    ):
        require(
            not perf_bundle.measurement_harness_dirty_paths_only(
                rejected_dirty_paths
            ),
            "formal bundle accepted a dirty path outside the harness boundary",
        )
    capture_hash = "1" * 64
    renderdoc_hash = "2" * 64
    memory_hash = "3" * 64
    observed_lock = {
        "schema_version": 2,
        "capture_binary_sha256": capture_hash,
        "renderdoc_binary_sha256": None,
        "memory_binary_sha256": None,
    }
    sealed_lock = {
        **observed_lock,
        "renderdoc_binary_sha256": renderdoc_hash,
        "memory_binary_sha256": memory_hash,
    }
    require(
        perf_execution.comparable_environment_lock_payload(
            observed=observed_lock,
            expected=sealed_lock,
            instrumentation="capture",
        )
        == sealed_lock,
        "Capture retry did not preserve hashes sealed by later formal legs",
    )
    changed_capture = {
        **observed_lock,
        "capture_binary_sha256": "4" * 64,
    }
    require(
        perf_execution.comparable_environment_lock_payload(
            observed=changed_capture,
            expected=sealed_lock,
            instrumentation="capture",
        )
        != sealed_lock,
        "Capture retry ignored a changed Capture binary",
    )
    memory_observation = {
        **observed_lock,
        "capture_binary_sha256": memory_hash,
    }
    require(
        perf_execution.comparable_environment_lock_payload(
            observed=memory_observation,
            expected=sealed_lock,
            instrumentation="memory",
        )
        == sealed_lock,
        "Memory retry did not preserve generation binary ownership",
    )
    try:
        perf_execution.comparable_environment_lock_payload(
            observed=observed_lock,
            expected={**sealed_lock, "renderdoc_binary_sha256": "invalid"},
            instrumentation="capture",
        )
    except RuntimeError:
        pass
    else:
        raise AcceptanceError("invalid sealed environment hash was accepted")
    renderdoc_binary_hash = "a" * 64
    renderdoc_capsule = {
        "capsule_id": "b" * 64,
        "manifest_sha256": "c" * 64,
    }
    rd0_manifest = {
        "binary": {"sha256": renderdoc_binary_hash},
        "capsule": renderdoc_capsule,
        "replay_digest": "d" * 64,
    }
    formal_manifest = {
        "binary": {"sha256": renderdoc_binary_hash},
        "capsule": renderdoc_capsule.copy(),
        "replay_digest": "e" * 64,
    }
    verify_formal_renderdoc_continuity(
        rd0_manifest=rd0_manifest,
        formal_manifest=formal_manifest,
        binary_sha256=renderdoc_binary_hash,
    )
    for invalid_manifest, expected_error in (
        (
            {**formal_manifest, "capsule": {"capsule_id": "f" * 64}},
            "formal RenderDoc capsule differs from RD0",
        ),
        (
            {**formal_manifest, "replay_digest": None},
            "formal RenderDoc did not complete double replay",
        ),
        (
            {**formal_manifest, "binary": {"sha256": "0" * 64}},
            "RenderDoc did not use the profiling-renderdoc binary",
        ),
    ):
        try:
            verify_formal_renderdoc_continuity(
                rd0_manifest=rd0_manifest,
                formal_manifest=invalid_manifest,
                binary_sha256=renderdoc_binary_hash,
            )
        except AcceptanceError as error:
            require(
                str(error) == expected_error,
                "formal RenderDoc continuity rejected with the wrong reason",
            )
        else:
            raise AcceptanceError(
                "formal RenderDoc continuity accepted invalid evidence"
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
        qrenderdoc_probe = root / "qrenderdoc-probe"
        qrenderdoc_probe.write_text(
            "#!/bin/sh\n"
            "if [ \"$QT_QPA_PLATFORM\" != offscreen ]; then exit 9; fi\n"
            "echo 'QRenderDoc v1.99'\n",
            encoding="utf-8",
        )
        qrenderdoc_probe.chmod(0o755)
        require(
            renderdoc_version(qrenderdoc_probe, headless_qt=True)
            == "QRenderDoc v1.99",
            "static qrenderdoc probe did not force the offscreen Qt platform",
        )
        capture_ack_artifact = root / "capture-ack-artifact"
        capture_ack_artifact.mkdir()
        capture_ack_screenshot = capture_ack_artifact / SAVE_CATALOG_SCREENSHOT
        capture_ack_screenshot.write_bytes(save_catalog_marker_png(1280, 720))
        capture_ack = {
            **finalize_native_screenshot(capture_ack_screenshot),
            "capture_scope": SAVE_CATALOG_CAPTURE_SCOPE,
            "window_id": "0x1234",
            "window_pid": 1234,
        }
        write_save_catalog_capture_ack(
            capture_ack_artifact,
            "self-test",
            capture_ack_screenshot,
            capture_ack,
        )
        capture_ack_path = capture_ack_artifact / "capture.done.txt"
        published_ack = capture_ack_path.read_text(encoding="utf-8")
        require(
            len(published_ack.splitlines()) == 10
            and published_ack.endswith("capture_window_pid=1234\n"),
            "capture acknowledgement was not atomically published as one complete contract",
        )
        require(
            not list(capture_ack_artifact.glob(".capture.done.txt.*.tmp")),
            "atomic capture acknowledgement left a temporary file behind",
        )
        try:
            write_save_catalog_capture_ack(
                capture_ack_artifact,
                "self-test",
                capture_ack_screenshot,
                capture_ack,
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("existing capture acknowledgement was overwritten")
        require(
            capture_ack_path.read_text(encoding="utf-8") == published_ack,
            "existing capture acknowledgement changed after a rejected second publish",
        )
        try:
            run_bounded_capture_tool(
                [sys.executable, "-c", "import time; time.sleep(60)"],
                label="capture timeout self-test",
                timeout_seconds=0.05,
            )
        except AcceptanceError as error:
            require(
                "timed out" in str(error),
                "bounded capture tool reported the wrong timeout failure",
            )
        else:
            raise AcceptanceError("bounded capture tool did not time out")
        proc = root / "proc"
        for pid, parent in ((100, 1), (101, 100), (102, 101), (200, 1)):
            process = proc / str(pid)
            process.mkdir(parents=True)
            (process / "stat").write_text(
                f"{pid} (test worker) S {parent} 0 0 0\n", encoding="utf-8"
            )
        require(
            descendant_process_ids(100, proc_root=proc) == {100, 101, 102},
            "X11 window ownership process subtree was not bounded to Cargo descendants",
        )
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
        low_stage_start_memory = {
            "mem_available_bytes": (MIN_STAGE_START_MEMORY_GIB - 1) * GIB,
            "swap_total_bytes": 0,
            "swap_free_bytes": 0,
        }
        require(
            any(
                "MemAvailable" in failure
                for failure in memory_safety_failures(
                    low_stage_start_memory,
                    minimum_memory_gib=MIN_STAGE_START_MEMORY_GIB,
                    phase="stage start",
                )
            ),
            "low memory did not block native stage admission",
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
            try:
                stage_start_resource_gate("unavailable-probe-self-test")
            except AcceptanceError as error:
                unavailable_probe_error = str(error)
            else:
                raise AcceptanceError("unavailable stage-start probe did not fail closed")
        finally:
            globals()["meminfo_bytes"] = original_meminfo_bytes
        require(
            "start resource probe failed" in unavailable_probe_error,
            "unavailable stage-start resource probe returned the wrong failure",
        )

        stage_gate_job = root / "stage-gate-job.json"
        stage_gate_log = root / "stage-gate.log"
        stage_gate_marker = root / "stage-gate-command-started"
        stage_gate_state: dict[str, Any] = {
            "status": "running",
            "completed_stages": [],
        }
        original_memory_snapshot = memory_snapshot
        try:
            globals()["memory_snapshot"] = lambda: low_stage_start_memory
            try:
                run_command(
                    "stage-start-gate-self-test",
                    [
                        sys.executable,
                        "-c",
                        f"from pathlib import Path; Path({str(stage_gate_marker)!r}).touch()",
                    ],
                    repo=workspace,
                    env=guarded_env,
                    log_path=stage_gate_log,
                    job_file=stage_gate_job,
                    state=stage_gate_state,
                )
            except AcceptanceError as error:
                require(
                    "not started" in str(error),
                    "stage-start gate returned the wrong failure",
                )
            else:
                raise AcceptanceError("stage-start gate launched a rejected stage")
        finally:
            globals()["memory_snapshot"] = original_memory_snapshot
        stage_gate_events = stage_gate_state.get("stage_start_resource_events", [])
        require(
            len(stage_gate_events) == 1
            and stage_gate_events[0]["status"] == "blocked"
            and stage_gate_state.get("child_pid") is None
            and not stage_gate_marker.exists(),
            "stage-start gate launched a child or omitted blocked evidence",
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

        original_update_state = update_state
        interrupt_raised = False

        def interrupting_update_state(
            job_file: Path,
            state: dict[str, Any],
            **updates: Any,
        ) -> None:
            nonlocal interrupt_raised
            original_update_state(job_file, state, **updates)
            child_pid = updates.get("child_pid")
            if isinstance(child_pid, int) and not interrupt_raised:
                interrupt_raised = True
                raise KeyboardInterrupt

        try:
            globals()["update_state"] = interrupting_update_state
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
            globals()["update_state"] = original_update_state
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
            stage="current",
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
                stage="current",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("invalid S1 clock mode fixture unexpectedly passed")

        rtt_p01 = root / "rtt-light-p01-smoke"
        for leg, binary_hash, window_backend in (
            ("audit", "e" * 64, "headless"),
            ("capture", "e" * 64, "x11"),
            ("memory", "f" * 64, "x11"),
        ):
            write_fake_rtt_light_smoke(
                rtt_p01 / leg,
                leg=leg,
                binary_hash=binary_hash,
                window_backend=window_backend,
                stage="p01",
            )
        require(
            verify_rtt_light_smoke(
                audit=rtt_p01 / "audit",
                capture=rtt_p01 / "capture",
                memory=rtt_p01 / "memory",
                adapter="Intel",
                window_backend="x11",
                stage="p01",
            )["status"]
            == "pass",
            "valid P01 S1 fixture did not pass",
        )
        try:
            verify_rtt_light_smoke(
                audit=rtt_p01 / "audit",
                capture=rtt_p01 / "capture",
                memory=rtt_p01 / "memory",
                adapter="Intel",
                window_backend="x11",
                stage="current",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("P01 S1 fixture was accepted as current")

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

        notifications_job = root / "notifications-job"
        notifications_artifact = notifications_job / "artifact"
        notifications_runtime = notifications_job / "runtime"
        notifications_artifact.mkdir(parents=True)
        (notifications_runtime / "saves").mkdir(parents=True)
        (notifications_runtime / "settings").mkdir(parents=True)
        notifications_screenshot = notifications_artifact / "a2-notifications.png"
        notifications_screenshot.write_bytes(structural_png(1280, 720))
        atomic_write_json(
            notifications_artifact / "driver-result.json",
            {
                "status": "PASS",
                "profile": "player-facing-result-notifications",
                "run_id": "self-test-a2",
                "checks": {check: "PASS" for check in NOTIFICATION_CHECKS},
                "runtime": {
                    "save_root": str(notifications_runtime / "saves"),
                    "settings_root": str(notifications_runtime / "settings"),
                },
                "screenshot": {
                    "path": str(notifications_screenshot),
                    "width": 1280,
                    "height": 720,
                    "bytes": notifications_screenshot.stat().st_size,
                },
                "renderer": {
                    "adapter_name": "Intel(R) Arc Graphics",
                    "backend": "Vulkan",
                    "display_handle": "Xlib(XlibDisplayHandle)",
                },
            },
        )
        notifications_result = verify_notifications_artifact(
            notifications_artifact,
            runtime_root=notifications_runtime,
            run_id="self-test-a2",
            adapter="Intel",
            backend="vulkan",
            window_backend="x11",
        )
        require(notifications_result["status"] == "pass", "valid A2 fixture did not pass")
        (notifications_artifact / "unexpected.txt").write_text("leak\n", encoding="utf-8")
        try:
            verify_notifications_artifact(
                notifications_artifact,
                runtime_root=notifications_runtime,
                run_id="self-test-a2",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("A2 fixture with an extra artifact unexpectedly passed")
        (notifications_artifact / "unexpected.txt").unlink()

        save_catalog_job = root / "save-catalog-job"
        save_catalog_artifact = save_catalog_job / "artifact"
        save_catalog_runtime = save_catalog_job / "runtime"
        (save_catalog_runtime / "saves").mkdir(parents=True)
        (save_catalog_runtime / "settings").mkdir(parents=True)
        save_catalog_artifact.mkdir()
        save_catalog_save = (
            save_catalog_runtime / "saves" / SAVE_CATALOG_FINAL_RECOVERY_FILE
        )
        save_catalog_screenshot = save_catalog_artifact / SAVE_CATALOG_SCREENSHOT
        save_catalog_save.write_text("native-save\n", encoding="utf-8")
        (save_catalog_runtime / "settings/settings.ron").write_text("()\n", encoding="utf-8")
        save_catalog_screenshot.write_bytes(save_catalog_marker_png(1280, 720))
        save_catalog_marker_pixels = count_save_catalog_marker_pixels(
            save_catalog_screenshot.read_bytes()
        )
        (save_catalog_artifact / "capture-ready.txt").write_text(
            "run_id=self-test\nmarker_rgb=255,0,255\n"
            f"marker_min_pixels={SAVE_CATALOG_MARKER_MIN_PIXELS}\n",
            encoding="utf-8",
        )
        (save_catalog_artifact / "capture.done.txt").write_text(
            f"run_id=self-test\n"
            f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
            f"bytes={save_catalog_screenshot.stat().st_size}\n"
            "width=1280\nheight=720\n"
            f"sha256={sha256(save_catalog_screenshot)}\n"
            f"marker_pixels={save_catalog_marker_pixels}\n"
            f"capture_scope={SAVE_CATALOG_CAPTURE_SCOPE}\n"
            "capture_window_id=0x1234\n"
            "capture_window_pid=1234\n",
            encoding="utf-8",
        )
        atomic_write_json(
            save_catalog_artifact / "driver-result.json",
            {
                "status": "PASS",
                "profile": "save-catalog",
                "run_id": "self-test",
                "checks": {check: "PASS" for check in SAVE_CATALOG_CHECKS},
                "world_epoch_before_valid_load": 7,
                "world_epoch_after_valid_load": 8,
                "save_bytes": save_catalog_save.stat().st_size,
                "runtime": {
                    "save_root": str(save_catalog_runtime / "saves"),
                    "settings_root": str(save_catalog_runtime / "settings"),
                },
                "screenshot": SAVE_CATALOG_SCREENSHOT,
                "screenshot_bytes": save_catalog_screenshot.stat().st_size,
                "screenshot_width": 1280,
                "screenshot_height": 720,
                "screenshot_sha256": sha256(save_catalog_screenshot),
                "screenshot_marker_pixels": save_catalog_marker_pixels,
                "screenshot_capture": {
                    "scope": SAVE_CATALOG_CAPTURE_SCOPE,
                    "window_id": "0x1234",
                    "window_pid": 1234,
                },
                "renderer": {
                    "adapter_name": "Intel(R) Arc Graphics",
                    "backend": "Vulkan",
                    "display_handle": "Xlib(XlibDisplayHandle)",
                },
            },
        )
        save_catalog_result = verify_save_catalog_artifact(
            save_catalog_artifact,
            runtime_root=save_catalog_runtime,
            run_id="self-test",
            adapter="Intel",
            backend="vulkan",
            window_backend="x11",
        )
        require(
            save_catalog_result["status"] == "pass",
            "valid save-catalog fixture did not pass",
        )
        wrong_scope_catalog = read_json(save_catalog_artifact / "driver-result.json")
        wrong_scope_catalog["screenshot_capture"]["scope"] = "root-display"
        atomic_write_json(save_catalog_artifact / "driver-result.json", wrong_scope_catalog)
        (save_catalog_artifact / "capture.done.txt").write_text(
            f"run_id=self-test\n"
            f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
            f"bytes={save_catalog_screenshot.stat().st_size}\n"
            "width=1280\nheight=720\n"
            f"sha256={sha256(save_catalog_screenshot)}\n"
            f"marker_pixels={save_catalog_marker_pixels}\n"
            "capture_scope=root-display\n"
            "capture_window_id=0x1234\n"
            "capture_window_pid=1234\n",
            encoding="utf-8",
        )
        try:
            verify_save_catalog_artifact(
                save_catalog_artifact,
                runtime_root=save_catalog_runtime,
                run_id="self-test",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("root-display save-catalog screenshot unexpectedly passed")
        wrong_scope_catalog["screenshot_capture"]["scope"] = SAVE_CATALOG_CAPTURE_SCOPE
        atomic_write_json(save_catalog_artifact / "driver-result.json", wrong_scope_catalog)
        (save_catalog_artifact / "capture.done.txt").write_text(
            f"run_id=self-test\n"
            f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
            f"bytes={save_catalog_screenshot.stat().st_size}\n"
            "width=1280\nheight=720\n"
            f"sha256={sha256(save_catalog_screenshot)}\n"
            f"marker_pixels={save_catalog_marker_pixels}\n"
            f"capture_scope={SAVE_CATALOG_CAPTURE_SCOPE}\n"
            "capture_window_id=0x1234\n"
            "capture_window_pid=1234\n",
            encoding="utf-8",
        )
        save_catalog_screenshot.write_bytes(
            save_catalog_marker_png(1280, 720, include_marker=False)
        )
        blank_catalog = read_json(save_catalog_artifact / "driver-result.json")
        blank_catalog["screenshot_bytes"] = save_catalog_screenshot.stat().st_size
        blank_catalog["screenshot_sha256"] = sha256(save_catalog_screenshot)
        blank_catalog["screenshot_marker_pixels"] = 0
        atomic_write_json(save_catalog_artifact / "driver-result.json", blank_catalog)
        (save_catalog_artifact / "capture.done.txt").write_text(
            f"run_id=self-test\n"
            f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
            f"bytes={save_catalog_screenshot.stat().st_size}\n"
            "width=1280\nheight=720\n"
            f"sha256={sha256(save_catalog_screenshot)}\n"
            "marker_pixels=0\n"
            f"capture_scope={SAVE_CATALOG_CAPTURE_SCOPE}\n"
            "capture_window_id=0x1234\n"
            "capture_window_pid=1234\n",
            encoding="utf-8",
        )
        try:
            verify_save_catalog_artifact(
                save_catalog_artifact,
                runtime_root=save_catalog_runtime,
                run_id="self-test",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("blank native save-catalog screenshot unexpectedly passed")
        save_catalog_screenshot.write_bytes(save_catalog_marker_png(1280, 720))
        restored_catalog = read_json(save_catalog_artifact / "driver-result.json")
        restored_catalog["screenshot_bytes"] = save_catalog_screenshot.stat().st_size
        restored_catalog["screenshot_sha256"] = sha256(save_catalog_screenshot)
        restored_catalog["screenshot_marker_pixels"] = count_save_catalog_marker_pixels(
            save_catalog_screenshot.read_bytes()
        )
        atomic_write_json(save_catalog_artifact / "driver-result.json", restored_catalog)
        (save_catalog_artifact / "capture.done.txt").write_text(
            f"run_id=self-test\n"
            f"screenshot={SAVE_CATALOG_SCREENSHOT}\n"
            f"bytes={save_catalog_screenshot.stat().st_size}\n"
            "width=1280\nheight=720\n"
            f"sha256={sha256(save_catalog_screenshot)}\n"
            f"marker_pixels={restored_catalog['screenshot_marker_pixels']}\n"
            f"capture_scope={SAVE_CATALOG_CAPTURE_SCOPE}\n"
            "capture_window_id=0x1234\n"
            "capture_window_pid=1234\n",
            encoding="utf-8",
        )
        broken_catalog = read_json(save_catalog_artifact / "driver-result.json")
        broken_catalog["checks"]["V5"] = "FAIL"
        atomic_write_json(save_catalog_artifact / "driver-result.json", broken_catalog)
        try:
            verify_save_catalog_artifact(
                save_catalog_artifact,
                runtime_root=save_catalog_runtime,
                run_id="self-test",
                adapter="Intel",
                backend="vulkan",
                window_backend="x11",
            )
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("invalid save-catalog V5 fixture unexpectedly passed")

        save_transaction_run_id = (
            f"c2-20260809T000000Z-{secrets.token_hex(4)}"
        )
        save_transaction_runtime = (
            repo / "target" / ".save-transaction-runtime" / save_transaction_run_id
        )
        save_transaction_perf = root / "save-transaction-perf"
        require(
            not save_transaction_runtime.exists(),
            "self-test save-transaction runtime root unexpectedly already exists",
        )
        save_transaction_runtime.mkdir(parents=True)
        require(
            remove_empty_save_transaction_runtime_root(repo, save_transaction_run_id)
            and not save_transaction_runtime.exists(),
            "empty native save-transaction runtime parent was not removed",
        )
        save_transaction_runtime.mkdir(parents=True)
        (save_transaction_runtime / "unexpected").mkdir()
        try:
            remove_empty_save_transaction_runtime_root(repo, save_transaction_run_id)
        except AcceptanceError:
            pass
        else:
            raise AcceptanceError("nonempty native runtime parent unexpectedly passed cleanup")
        cleanup_save_transaction_runtime_root(repo, save_transaction_run_id)
        try:
            fake_source = source_fingerprint(repo)
            write_fake_save_transaction_session(
                save_transaction_perf / "capture",
                repo=repo,
                instrumentation="capture",
                binary_hash="a" * 64,
                source_fingerprint_value=fake_source,
                seed=DEFAULT_SEED,
            )
            write_fake_save_transaction_session(
                save_transaction_perf / "memory",
                repo=repo,
                instrumentation="memory",
                binary_hash="b" * 64,
                source_fingerprint_value=fake_source,
                seed=DEFAULT_SEED,
            )
            save_transaction_result = verify_save_transaction_bundle(
                repo=repo,
                performance_root=save_transaction_perf,
                expected_source=fake_source,
                expected_seed=DEFAULT_SEED,
                expected_backend="vulkan",
            )
            require(
                save_transaction_result["status"] == "pass",
                "valid save-transaction bundle did not pass",
            )
            duplicate_aggregate_perf = root / "save-transaction-duplicate-aggregate"
            shutil.copytree(save_transaction_perf, duplicate_aggregate_perf)
            duplicate_aggregate = duplicate_aggregate_perf / "capture" / "aggregate.csv"
            aggregate_lines = duplicate_aggregate.read_text(encoding="utf-8").splitlines()
            duplicate_aggregate.write_text(
                "\n".join([*aggregate_lines, aggregate_lines[1]]) + "\n",
                encoding="utf-8",
            )
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=duplicate_aggregate_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("duplicate save-transaction aggregate unexpectedly passed")
            stale_total_perf = root / "save-transaction-stale-total"
            shutil.copytree(save_transaction_perf, stale_total_perf)
            stale_total_case = (
                stale_total_perf
                / "capture"
                / "cases"
                / f"save-transaction-small-cpu-seed-{DEFAULT_SEED}"
            )
            for label, _ in save_transaction_labels():
                stale_total_csv = stale_total_case / label / "data" / "save_transaction.csv"
                with stale_total_csv.open(newline="", encoding="utf-8") as handle:
                    stale_total_rows = list(csv.DictReader(handle))
                stale_total_rows[0]["total_ns"] = "1"
                with stale_total_csv.open("w", newline="", encoding="utf-8") as handle:
                    writer = csv.DictWriter(handle, fieldnames=SAVE_TRANSACTION_COLUMNS)
                    writer.writeheader()
                    writer.writerows(stale_total_rows)
                stale_total_log = stale_total_case / label / "run.log"
                stale_total_log.write_text(
                    re.sub(r"total_ns=\\d+", "total_ns=1", stale_total_log.read_text(encoding="utf-8")),
                    encoding="utf-8",
                )
            stale_total_aggregate = stale_total_perf / "capture" / "aggregate.csv"
            with stale_total_aggregate.open(newline="", encoding="utf-8") as handle:
                stale_total_rows = list(csv.DictReader(handle))
            for row in stale_total_rows:
                if row["case_id"] == f"save-transaction-small-cpu-seed-{DEFAULT_SEED}":
                    row["total_p95_ns"] = "1"
                    row["total_max_ns"] = "1"
            with stale_total_aggregate.open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(handle, fieldnames=SAVE_TRANSACTION_AGGREGATE_COLUMNS)
                writer.writeheader()
                writer.writerows(stale_total_rows)
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=stale_total_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("save-transaction phase-inconsistent total unexpectedly passed")
            leaked_csv_perf = root / "save-transaction-csv-leak"
            shutil.copytree(save_transaction_perf, leaked_csv_perf)
            leaked_csv = (
                leaked_csv_perf
                / "capture"
                / "cases"
                / f"save-transaction-small-cpu-seed-{DEFAULT_SEED}"
                / "run-001"
                / "data"
                / "save_transaction.csv"
            )
            leaked_csv.write_text(
                leaked_csv.read_text(encoding="utf-8").rstrip("\n")
                + ",HELL_WORKERS_SAVE\n(format_version: 1)\n",
                encoding="utf-8",
            )
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=leaked_csv_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("save-transaction CSV body leak unexpectedly passed")
            crlf_body_log_perf = root / "save-transaction-crlf-body-log"
            shutil.copytree(save_transaction_perf, crlf_body_log_perf)
            crlf_body_log = (
                crlf_body_log_perf
                / "capture"
                / "cases"
                / f"save-transaction-small-cpu-seed-{DEFAULT_SEED}"
                / "run-001"
                / "run.log"
            )
            crlf_body_log.write_bytes(
                crlf_body_log.read_bytes()
                + b"HELL_WORKERS_SAVE\r\n(format_version: 1, worldgen_seed: 1)\r\n---\r\n"
            )
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=crlf_body_log_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("CRLF save body retained in a log unexpectedly passed")
            unknown_file_perf = root / "save-transaction-unknown-file"
            shutil.copytree(save_transaction_perf, unknown_file_perf)
            (
                unknown_file_perf
                / "memory"
                / "cases"
                / f"save-transaction-small-cpu-seed-{DEFAULT_SEED}"
                / "run-001"
                / "leaked.scn.ron"
            ).write_text("HELL_WORKERS_SAVE\n(format_version: 1)\n", encoding="utf-8")
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=unknown_file_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("unknown save-transaction artifact unexpectedly passed")
            unbalanced_memory_perf = root / "save-transaction-unbalanced-memory"
            shutil.copytree(save_transaction_perf, unbalanced_memory_perf)
            unbalanced_memory = (
                unbalanced_memory_perf
                / "memory"
                / "cases"
                / f"save-transaction-small-cpu-seed-{DEFAULT_SEED}"
                / "run-001"
                / "data"
                / "memory.csv"
            )
            unbalanced_memory.write_text(
                unbalanced_memory.read_text(encoding="utf-8").replace(
                    ",100,1000,1000,10,9,0,0\n",
                    ",101,1000,1000,10,9,0,0\n",
                ),
                encoding="utf-8",
            )
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=unbalanced_memory_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("unbalanced save-transaction memory unexpectedly passed")
            same_binary = read_json(save_transaction_perf / "memory" / "manifest.json")
            same_binary["binary"]["sha256"] = "a" * 64
            atomic_write_json(save_transaction_perf / "memory" / "manifest.json", same_binary)
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=save_transaction_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("same-binary save-transaction fixture unexpectedly passed")
            same_binary["binary"]["sha256"] = "b" * 64
            atomic_write_json(save_transaction_perf / "memory" / "manifest.json", same_binary)
            aggregate_path = save_transaction_perf / "capture" / "aggregate.csv"
            aggregate_rows = read_exact_csv(
                aggregate_path,
                SAVE_TRANSACTION_AGGREGATE_COLUMNS,
                "aggregate.csv",
            )
            next(
                row for row in aggregate_rows if row["case_id"].split("-")[2] == "large"
            )["total_p95_ns"] = str(SAVE_TRANSACTION_LARGE_TOTAL_P95_LIMIT_NS + 1)
            with aggregate_path.open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(handle, fieldnames=SAVE_TRANSACTION_AGGREGATE_COLUMNS)
                writer.writeheader()
                writer.writerows(aggregate_rows)
            try:
                verify_save_transaction_bundle(
                    repo=repo,
                    performance_root=save_transaction_perf,
                    expected_source=fake_source,
                    expected_seed=DEFAULT_SEED,
                    expected_backend="vulkan",
                )
            except AcceptanceError:
                pass
            else:
                raise AcceptanceError("over-budget save-transaction fixture unexpectedly passed")
        finally:
            cleanup_save_transaction_runtime_root(repo, save_transaction_run_id)
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
    parser.add_argument("--stage", default=RTT_LIGHT_DEFAULT_STAGE, choices=["current", "p01"])
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
        parser.add_argument("--harness-fingerprint", required=True)
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
    notifications_plan = commands.add_parser(
        "plan-notifications", help="emit the no-prompt Track A2 launcher plan"
    )
    add_deconstruction_arguments(notifications_plan, require_job_root=False)
    notifications_run = commands.add_parser(
        "run-notifications", help="run Track A2 inside the planned actual window"
    )
    add_deconstruction_arguments(notifications_run, require_job_root=True)
    save_catalog_plan = commands.add_parser(
        "plan-save-catalog", help="emit the no-prompt save-catalog launcher plan"
    )
    add_deconstruction_arguments(save_catalog_plan, require_job_root=False)
    save_catalog_run = commands.add_parser(
        "run-save-catalog", help="run save-catalog acceptance inside the actual window"
    )
    add_deconstruction_arguments(save_catalog_run, require_job_root=True)
    save_catalog_run.add_argument("--harness-fingerprint", required=True)
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
    verify_notifications = commands.add_parser(
        "verify-notifications", help="fail-closed validation of one Track A2 artifact"
    )
    verify_notifications.add_argument("--repo", required=True)
    verify_notifications.add_argument("--artifact", required=True)
    verify_notifications.add_argument("--runtime-root", required=True)
    verify_notifications.add_argument("--run-id", required=True)
    verify_notifications.add_argument("--harness-fingerprint", required=True)
    verify_notifications.add_argument("--adapter", default="Intel")
    verify_notifications.add_argument("--backend", default="vulkan")
    verify_notifications.add_argument(
        "--window-backend", default="x11", choices=["x11", "wayland"]
    )
    verify_save_catalog = commands.add_parser(
        "verify-save-catalog", help="fail-closed validation of one save-catalog V1-V5 artifact"
    )
    verify_save_catalog.add_argument("--artifact", required=True)
    verify_save_catalog.add_argument("--runtime-root", required=True)
    verify_save_catalog.add_argument("--repo", required=True)
    verify_save_catalog.add_argument("--performance-root", required=True)
    verify_save_catalog.add_argument("--source-fingerprint", required=True)
    verify_save_catalog.add_argument("--harness-fingerprint", required=True)
    verify_save_catalog.add_argument("--seed", type=int, required=True)
    verify_save_catalog.add_argument("--run-id", required=True)
    verify_save_catalog.add_argument("--adapter", default="Intel")
    verify_save_catalog.add_argument("--backend", default="vulkan")
    verify_save_catalog.add_argument(
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
    if args.command == "verify-save-catalog":
        if re.fullmatch(r"[0-9a-f]{64}", args.source_fingerprint) is None:
            raise AcceptanceError("save-catalog source fingerprint is invalid")
        if re.fullmatch(r"[0-9a-f]{64}", args.harness_fingerprint) is None:
            raise AcceptanceError("save-catalog harness fingerprint is invalid")
        if args.seed < 0:
            raise AcceptanceError("save-catalog seed cannot be negative")
    if args.command == "verify-notifications" and re.fullmatch(
        r"[0-9a-f]{64}", args.harness_fingerprint
    ) is None:
        raise AcceptanceError("A2 harness fingerprint is invalid")
    if args.command == "run-save-catalog" and re.fullmatch(
        r"[0-9a-f]{64}", args.harness_fingerprint
    ) is None:
        raise AcceptanceError("planned save-catalog harness fingerprint is invalid")
    if args.command in {
        "plan-save-catalog",
        "run-save-catalog",
        "verify-save-catalog",
    }:
        require_save_catalog_x11_window_capture(args.window_backend)
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
            if re.fullmatch(r"[0-9a-f]{64}", args.harness_fingerprint) is None:
                raise AcceptanceError("planned harness fingerprint is invalid")
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
    if args.command == "plan-notifications":
        return plan_notifications(args)
    if args.command == "run-notifications":
        return run_notifications(args)
    if args.command == "plan-save-catalog":
        return plan_save_catalog(args)
    if args.command == "run-save-catalog":
        return run_save_catalog(args)
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
    if args.command == "verify-notifications":
        return verify_notifications_command(args)
    if args.command == "verify-save-catalog":
        return verify_save_catalog_command(args)
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
