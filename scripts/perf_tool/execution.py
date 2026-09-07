from __future__ import annotations

import csv
import hashlib
import json
import math
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import time
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path

from .artifacts import sha256, validate_run, write_json
from .model import REPO_ROOT, SESSION_MANIFEST_SCHEMA_VERSION, TRACY_DASHBOARD_ZONE_FILTER
from .rtt_light_contract import build_fixture_layout, contract_fingerprints, load_rtt_light_contract

try:
    from cargo_runtime import (
        cargo_environment as controlled_cargo_environment,
        persistent_storage_error,
        require_cargo_memory,
        resource_policy as cargo_resource_policy,
        workspace_cargo_target,
    )
except ModuleNotFoundError:
    from scripts.cargo_runtime import (
        cargo_environment as controlled_cargo_environment,
        persistent_storage_error,
        require_cargo_memory,
        resource_policy as cargo_resource_policy,
        workspace_cargo_target,
    )


SOURCE_FINGERPRINT_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "scripts/perf.py",
    "tools/blender_ai_workflow/fixtures/wall-calibration-v2.ocio",
    "tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json",
    "tools/blender_ai_workflow/fixtures/wall-density-v1.json",
    "tools/blender_ai_workflow/scripts/render_color_calibration.py",
    "tools/blender_ai_workflow/scripts/verify_color_calibration.py",
}
SOURCE_FINGERPRINT_PREFIXES = ("crates/", "scripts/perf_tool/")
SOURCE_FINGERPRINT_ASSET_PREFIX = "assets/"
MEASUREMENT_HARNESS_FILES = (
    ".codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/door_art_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/door_behavior_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/door_density_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/p02_presentation_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_color_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_density_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_production_performance_acceptance.py",
    ".codex/skills/hell-workers-run-native-acceptance/scripts/wall_renderdoc_acceptance.py",
    "scripts/build_coordination.py",
    "scripts/cargo_runtime.py",
    "scripts/perf_tool/execution.py",
    "scripts/perf_tool/renderdoc_capture.py",
    "scripts/perf_tool/renderdoc_foundation.py",
    "scripts/perf_tool/wall_renderdoc_extract.py",
    "scripts/perf_tool/rtt_light_bundle.py",
)
SAVE_TRANSACTION_RUNTIME_ROOT = (REPO_ROOT / "target" / ".save-transaction-runtime").resolve()

def command_output(command: list[str], *, cwd: Path = REPO_ROOT) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        return "<unavailable>"
    return completed.stdout.strip()


def tracked_source_paths() -> list[str]:
    completed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError("git ls-files failed while fingerprinting the source")
    return sorted(filter(None, completed.stdout.splitlines()))


def subject_file_bytes(relative: str) -> bytes | None:
    """Read the committed subject copy for a mutable measurement harness file."""
    completed = subprocess.run(
        ["git", "show", f"HEAD:{relative}"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
    )
    return completed.stdout if completed.returncode == 0 else None


def source_fingerprint() -> str:
    """Match the native acceptance source boundary exactly.

    Hash source and asset contents so an identical detached worktree has the
    same identity regardless of checkout path or filesystem timestamps.
    """
    digest = hashlib.sha256()
    for relative in tracked_source_paths():
        source = REPO_ROOT / relative
        if not source.is_file():
            continue
        subject_bytes = (
            subject_file_bytes(relative)
            if relative in MEASUREMENT_HARNESS_FILES
            else None
        )
        if relative in MEASUREMENT_HARNESS_FILES and subject_bytes is None:
            continue
        if relative in SOURCE_FINGERPRINT_FILES or relative.startswith(
            SOURCE_FINGERPRINT_PREFIXES
        ):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            if subject_bytes is not None:
                digest.update(subject_bytes)
            else:
                with source.open("rb") as handle:
                    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                        digest.update(chunk)
        elif relative.startswith(SOURCE_FINGERPRINT_ASSET_PREFIX):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
    return digest.hexdigest()


def finalize_session_source(manifest: dict[str, Any]) -> list[str]:
    source = manifest.get("source")
    if source is None:
        return []
    if not isinstance(source, dict):
        return ["manifest source provenance is not an object"]
    started = source.get("fingerprint_start")
    if not isinstance(started, str) or not re.fullmatch(r"[0-9a-f]{64}", started):
        return ["manifest source fingerprint_start is invalid"]
    ended = source_fingerprint()
    unchanged = ended == started
    source.update(
        {
            "fingerprint_end": ended,
            "unchanged": unchanged,
            "finished_at": datetime.now(UTC).isoformat(),
        }
    )
    return [] if unchanged else ["source fingerprint changed during the session"]


ENVIRONMENT_LOCK_WINDOW_FIELDS = (
    "logical_width",
    "logical_height",
    "physical_width",
    "physical_height",
    "scale_factor",
    "rtt_quality",
    "scene_target_width",
    "scene_target_height",
    "mask_target_width",
    "mask_target_height",
    "target_scale_factor",
)


def environment_lock_payload(
    *,
    manifest: dict[str, Any],
    validation: Validation,
    contract_id: str,
    stage_id: str,
) -> dict[str, Any]:
    if validation.window is None or validation.adapter is None:
        raise RuntimeError("environment lock requires validated window and adapter evidence")
    window = validation.window
    return {
        "schema_version": 2,
        "contract_id": contract_id,
        "stage_id": stage_id,
        "subject_commit": manifest["git"]["commit"],
        "source_fingerprint": manifest["source"]["fingerprint_start"],
        "host": manifest["host"],
        "adapter": validation.adapter,
        "resolved_window_backend": window["resolved_window_backend"],
        "adapter_backend": window["adapter_backend"],
        "requested_present_mode": window["requested_present_mode"],
        "effective_present_mode": window["effective_present_mode"],
        "window": {field: window[field] for field in ENVIRONMENT_LOCK_WINDOW_FIELDS},
        "capture_binary_sha256": manifest["binary"]["sha256"],
        "renderdoc_binary_sha256": None,
        "memory_binary_sha256": None,
    }


def write_json_exclusive(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def enforce_environment_lock(
    *,
    args: argparse.Namespace,
    session_dir: Path,
    validation: Validation,
    preflight: bool,
) -> list[str]:
    if args.environment_lock is None:
        return []
    lock_path = Path(args.environment_lock).resolve()
    manifest = json.loads((session_dir / "manifest.json").read_text(encoding="utf-8"))
    try:
        observed = environment_lock_payload(
            manifest=manifest,
            validation=validation,
            contract_id=args.contract,
            stage_id=args.stage,
        )
    except (KeyError, RuntimeError) as error:
        return [str(error)]
    if not lock_path.exists():
        if args.instrumentation != "capture" or not preflight:
            return ["environment lock is missing before a non-Capture-preflight run"]
        if not validation.valid:
            return ["invalid Capture preflight cannot create the environment lock"]
        try:
            write_json_exclusive(lock_path, observed)
        except FileExistsError:
            pass
        except OSError as error:
            return [f"cannot create environment lock: {error}"]
    try:
        expected = json.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read environment lock: {error}"]
    try:
        comparable = comparable_environment_lock_payload(
            observed=observed,
            expected=expected,
            instrumentation=args.instrumentation,
        )
    except RuntimeError as error:
        return [str(error)]
    if expected != comparable:
        return ["run environment differs from the generation environment lock"]
    return []


def comparable_environment_lock_payload(
    *,
    observed: dict[str, Any],
    expected: dict[str, Any],
    instrumentation: str,
) -> dict[str, Any]:
    """Keep hashes sealed by other legs out of a perf-run environment diff.

    The generation lock survives failed formal attempts.  Capture runs do not
    produce RenderDoc or Memory hashes, and Memory runs do not produce the
    Capture hash.  Preserve those already-sealed values while continuing to
    compare every environment field and the binary owned by the active leg.
    """

    def sealed_hash(field: str) -> str | None:
        value = expected.get(field)
        if value is None:
            return None
        if (
            not isinstance(value, str)
            or len(value) != 64
            or any(character not in "0123456789abcdef" for character in value)
        ):
            raise RuntimeError(f"generation environment lock has invalid {field}")
        return value

    comparable = dict(observed)
    comparable["renderdoc_binary_sha256"] = sealed_hash(
        "renderdoc_binary_sha256"
    )
    comparable["memory_binary_sha256"] = sealed_hash("memory_binary_sha256")
    if instrumentation == "memory":
        comparable["capture_binary_sha256"] = sealed_hash(
            "capture_binary_sha256"
        )
    return comparable


def git_metadata() -> dict[str, Any]:
    status = command_output(["git", "status", "--short"])
    return {
        "commit": command_output(["git", "rev-parse", "HEAD"]),
        "short_commit": command_output(["git", "rev-parse", "--short", "HEAD"]),
        "dirty_paths": [] if status == "" else status.splitlines(),
    }


def cpu_model() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "<unknown>"


def host_metadata() -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "python": sys.version.split()[0],
        "cpu": cpu_model(),
        "hostname": platform.node(),
        "cargo": command_output(["cargo", "--version"]),
        "rustc": command_output(["rustc", "--version"]),
    }


def fixed_environment(args: argparse.Namespace) -> dict[str, str]:
    values = {
        "BEVY_ASSET_ROOT": str(REPO_ROOT),
        "HW_PRESENT_MODE": args.present_mode,
        "HW_WINDOW_BACKEND": args.window_backend,
    }
    if args.backend != "auto":
        values["WGPU_BACKEND"] = args.backend
    if args.adapter:
        values["WGPU_ADAPTER_NAME"] = args.adapter
    return values


def performance_environment() -> dict[str, str]:
    """Use one disk-backed, bounded environment for build and game processes."""
    return controlled_cargo_environment(
        REPO_ROOT,
        namespace=".perf-tmp",
        incremental=False,
    )


def cargo_features(instrumentation: str) -> str:
    return {
        "capture": "profiling",
        "tracy": "profiling-tracy",
        "memory": "profiling-memory",
        "renderdoc": "profiling-renderdoc",
    }[instrumentation]


def build_binary(args: argparse.Namespace) -> Path:
    if args.workload == "save-transaction" and (args.binary or args.skip_build):
        raise RuntimeError(
            "save-transaction must build and run its canonical profiling binary"
        )
    binary = (
        Path(args.binary).resolve()
        if args.binary
        else workspace_cargo_target(REPO_ROOT) / "profiling/bevy_app"
    )
    storage_error = persistent_storage_error(binary, label="profiling binary")
    if storage_error:
        raise RuntimeError(storage_error)
    if args.skip_build:
        if not binary.is_file():
            raise RuntimeError(f"profiling binary does not exist: {binary}")
        return binary

    command = [
        "cargo",
        "build",
        "--profile",
        "profiling",
        "-p",
        "bevy_app@0.1.0",
        "--no-default-features",
        "--features",
        cargo_features(args.instrumentation),
    ]
    print("+", " ".join(command), flush=True)
    completed = subprocess.run(
        command,
        cwd=REPO_ROOT,
        env=performance_environment(),
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError("profiling binary build failed")
    if not binary.is_file():
        raise RuntimeError(f"Cargo succeeded but profiling binary is missing: {binary}")
    return binary


def executable_path(value: str | None, label: str) -> Path:
    if not value:
        raise RuntimeError(f"{label} was not provided")
    path = Path(value).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise RuntimeError(f"{label} is not an executable file: {path}")
    return path


def profiling_tool_metadata(args: argparse.Namespace) -> dict[str, Any]:
    if args.instrumentation == "capture":
        return {}
    metadata: dict[str, Any] = {}
    if args.instrumentation == "tracy":
        capture = executable_path(args.tracy_capture_binary, "Tracy capture binary")
        csvexport = executable_path(args.tracy_csvexport_binary, "Tracy csvexport binary")
        metadata.update(
            {
                "tracy_version": "0.13.1",
                "capture": {"path": str(capture), "sha256": sha256(capture)},
                "csvexport": {"path": str(csvexport), "sha256": sha256(csvexport)},
            }
        )
    if args.instrumentation == "memory":
        timer = shutil.which("time")
        if not timer:
            raise RuntimeError("GNU time is required for --instrumentation memory")
        timer_path = executable_path(timer, "GNU time binary")
        metadata["time"] = {"path": str(timer_path), "sha256": sha256(timer_path)}
    return metadata


def explicit_session_dir(args: argparse.Namespace) -> Path | None:
    if not args.output:
        return None
    output = Path(args.output)
    return (output if output.is_absolute() else REPO_ROOT / output).resolve()


def default_output_root() -> Path:
    return (REPO_ROOT / "target" / "perf-runs").resolve()


def require_persistent_output(path: Path) -> None:
    storage_error = persistent_storage_error(
        path,
        label="performance artifact output",
    )
    if storage_error:
        raise RuntimeError(storage_error)


def validate_requested_output(args: argparse.Namespace) -> None:
    session_dir = explicit_session_dir(args)
    require_persistent_output(
        session_dir if session_dir is not None else default_output_root()
    )


def prepare_save_transaction_runtime_parent(
    args: argparse.Namespace, session_dir: Path
) -> Path | None:
    """Reserve an artifact-external, disk-backed root for save transaction data.

    Every invocation uses the same per-operation cleanup contract, including
    the formal native recipe. Serialized save bodies must never survive below
    the evidence session, the controlled runtime tree, or user data roots.
    """
    if args.workload != "save-transaction":
        return None
    configured = getattr(args, "save_runtime_root", None)
    parent = (
        Path(configured)
        if configured is not None
        else SAVE_TRANSACTION_RUNTIME_ROOT / session_dir.name
    )
    if not parent.is_absolute():
        parent = REPO_ROOT / parent
    parent = parent.resolve()
    if not parent.is_relative_to(SAVE_TRANSACTION_RUNTIME_ROOT):
        raise RuntimeError(
            "save-transaction runtime root must be below target/.save-transaction-runtime"
        )
    if parent == SAVE_TRANSACTION_RUNTIME_ROOT:
        raise RuntimeError("save-transaction runtime root must name one fresh run parent")
    if parent.is_relative_to(session_dir) or session_dir.is_relative_to(parent):
        raise RuntimeError("save-transaction runtime root must stay outside the artifact session")
    storage_error = persistent_storage_error(
        parent, label="save-transaction runtime root"
    )
    if storage_error:
        raise RuntimeError(storage_error)
    if parent.exists():
        raise RuntimeError(f"save-transaction runtime root already exists: {parent}")
    return parent


def prepare_session(
    args: argparse.Namespace,
    binary: Path,
    cases: list[Case],
    source_start: str,
) -> Path:
    if re.fullmatch(r"[0-9a-f]{64}", source_start) is None:
        raise RuntimeError("profiling source fingerprint before build is invalid")
    binary_hash = sha256(binary)
    # Keep a process-local pin for every child launch. The manifest is not a
    # substitute for this check: another build can replace the shared profiling
    # path while a multi-run session is still collecting samples.
    args._profiling_binary_sha256 = binary_hash
    session_dir = explicit_session_dir(args)
    if session_dir is None:
        timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
        session_dir = default_output_root() / f"{timestamp}-{git_metadata()['short_commit']}"
        session_dir = session_dir.resolve()
    validate_requested_output(args)
    require_persistent_output(session_dir)
    if session_dir.exists():
        raise RuntimeError(f"output directory already exists: {session_dir}")
    save_runtime_parent = prepare_save_transaction_runtime_parent(args, session_dir)
    # This is process-local orchestration state, intentionally never persisted
    # into a session artifact: it points at a directory that can contain a
    # serialized save body.
    args._save_transaction_runtime_parent = save_runtime_parent
    session_dir.mkdir(parents=True)
    (session_dir / "cases").mkdir()

    rtt_light_contract = None
    if args.contract is not None:
        contract = load_rtt_light_contract(args.contract)
        selected_sizes = list(dict.fromkeys(case.size for case in cases))
        fixture_layouts = {
            size: build_fixture_layout(contract, size) for size in selected_sizes
        }
        rtt_light_contract = {
            "contract_id": args.contract,
            "stage_id": args.stage,
            "lane": args.lane,
            **contract_fingerprints(contract),
            "fixture_id": contract["fixture"]["fixture_id"],
            "layout_checksums": {
                size: layout["layout_checksum"]
                for size, layout in fixture_layouts.items()
            },
            "lifecycle": contract["lifecycle"],
        }

    matrix = {
        "workload": args.workload,
        "sizes": list(dict.fromkeys(case.size for case in cases)),
        "renders": list(dict.fromkeys(case.render for case in cases)),
        "seed": args.seed,
        "repeat": args.repeat,
        "warmup_secs": getattr(args, "warmup_secs", None),
        "measure_secs": getattr(args, "measure_secs", None),
        "fixed_hz": getattr(args, "fixed_hz", None),
        "warmup_ticks": getattr(args, "warmup_ticks", None),
        "audit_ticks": getattr(args, "audit_ticks", None),
        "preflight_runs": args.preflight_runs,
        "souls": args.souls,
        "familiars": args.familiars,
        "familiar_policies": sorted({case.familiar_policy for case in cases}),
        "operation_dialog_modes": sorted({case.operation_dialog for case in cases}),
        "dashboard_modes": sorted({case.dashboard_mode for case in cases}),
        "behavior_cases": list(
            dict.fromkeys(
                case.behavior_case
                for case in cases
                if case.behavior_case is not None
            )
        ),
        "wall_phase": args.wall_phase,
        "door_presentation": args.door_presentation,
        "capture_kind": args.capture_kind,
        "clock_mode": args.clock_mode,
        "warmup_checksum_policy": getattr(args, "warmup_checksum_policy", None),
        "measure_end_checksum_policy": getattr(args, "measure_end_checksum_policy", None),
        "allow_log_patterns": list(args.allow_log_pattern),
        "tracy_capture_secs": args.tracy_capture_secs,
        "window_width": args.window_width,
        "window_height": args.window_height,
        "window_scale_factor": args.window_scale_factor,
        "rtt_quality": args.rtt_quality,
        "environment_lock": (
            str(Path(args.environment_lock).resolve())
            if args.environment_lock is not None
            else None
        ),
        "rtt_light_contract": rtt_light_contract,
        "save_transaction_runtime": (
            {
                "isolated": True,
                "cleanup": "per-run",
            }
            if save_runtime_parent is not None
            else None
        ),
    }
    write_json(session_dir / "matrix.json", matrix)
    manifest = {
        "schema_version": SESSION_MANIFEST_SCHEMA_VERSION,
        "created_at": datetime.now(UTC).isoformat(),
        "repo_root": str(REPO_ROOT),
        "git": git_metadata(),
        "source": {
            "algorithm": "hell-workers-source-v1",
            "fingerprint_start": source_start,
            "fingerprint_end": None,
            "unchanged": None,
            "started_at": datetime.now(UTC).isoformat(),
            "finished_at": None,
        },
        "host": host_metadata(),
        "binary": {
            "path": str(binary),
            "sha256": binary_hash,
            "instrumentation": args.instrumentation,
        },
        "profiling_tools": profiling_tool_metadata(args),
        "resource_policy": {
            **cargo_resource_policy(
                REPO_ROOT,
                namespace=".perf-tmp",
                incremental=False,
            ),
            "artifact_root": str(session_dir),
        },
        "requested_environment": fixed_environment(args),
        "matrix": matrix,
        "cases": [asdict(case) | {"id": case.identifier} for case in cases],
        "actual_adapters": [],
        "status": "running",
    }
    write_json(session_dir / "manifest.json", manifest)
    return session_dir


def run_csvexport(
    csvexport: Path,
    trace_path: Path,
    output_path: Path,
    log_path: Path,
    arguments: list[str],
    timeout_secs: float,
    environment: dict[str, str],
) -> tuple[int, str | None]:
    with output_path.open("w", encoding="utf-8") as output_handle, log_path.open(
        "w", encoding="utf-8"
    ) as log_handle:
        try:
            completed = subprocess.run(
                [str(csvexport), *arguments, str(trace_path)],
                cwd=REPO_ROOT,
                env=environment,
                stdout=output_handle,
                stderr=log_handle,
                check=False,
                timeout=timeout_secs,
            )
            return completed.returncode, None
        except subprocess.TimeoutExpired:
            return 124, f"Tracy csvexport timed out after {timeout_secs} seconds"


def read_tracy_zone_summary(path: Path) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse Tracy Task Dashboard zones: {error}"]
    required = {"name", "total_ns", "counts", "mean_ns", "min_ns", "max_ns"}
    if not required <= headers:
        errors.append("Tracy Task Dashboard zone CSV is missing required columns")
        return {}, errors
    zones: list[dict[str, Any]] = []
    total_ns = 0
    invocations = 0
    for index, row in enumerate(rows):
        try:
            zone_total = int(row["total_ns"])
            zone_count = int(row["counts"])
            zone_mean = int(row["mean_ns"])
            zone_min = int(row["min_ns"])
            zone_max = int(row["max_ns"])
            if min(zone_total, zone_count, zone_mean, zone_min, zone_max) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"Tracy Task Dashboard zone row {index} has invalid numeric data")
            continue
        zones.append(
            {
                "name": row["name"],
                "source": row.get("src_file", ""),
                "line": row.get("src_line", ""),
                "total_ns": zone_total,
                "count": zone_count,
                "mean_ns": zone_mean,
                "min_ns": zone_min,
                "max_ns": zone_max,
            }
        )
        total_ns += zone_total
        invocations += zone_count
    if not zones:
        errors.append(
            f"Tracy trace has no zones matching {TRACY_DASHBOARD_ZONE_FILTER!r}"
        )
    return {
        "zone_count": len(zones),
        "total_ns": total_ns,
        "invocations": invocations,
        "mean_ns_per_invocation": total_ns / invocations if invocations else 0.0,
        "zones": zones,
    }, errors


def read_native_memory(
    path: Path, *, frame_samples: int | None
) -> tuple[dict[str, Any], list[str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse native memory artifact: {error}"]
    required = {
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
    }
    if headers != required:
        return {}, ["native memory artifact has unexpected columns"]
    if len(rows) != 1:
        return {}, ["native memory artifact must contain exactly one row"]
    try:
        if rows[0]["schema_version"] != "1":
            raise ValueError
        values = {name: int(rows[0][name]) for name in required - {"schema_version"}}
        if min(values.values()) < 0:
            raise ValueError
        if values["accounting_errors"] != 0:
            raise ValueError
        if values["peak_live_bytes"] < max(
            values["baseline_live_bytes"], values["final_live_bytes"]
        ):
            raise ValueError
        if values["baseline_live_bytes"] + values["allocated_bytes"] != (
            values["final_live_bytes"] + values["deallocated_bytes"]
        ):
            raise ValueError
        if (values["allocation_calls"] == 0) != (values["allocated_bytes"] == 0):
            raise ValueError
        if (values["deallocation_calls"] == 0) != (
            values["deallocated_bytes"] == 0
        ):
            raise ValueError
        if values["reallocation_calls"] > min(
            values["allocation_calls"], values["deallocation_calls"]
        ):
            raise ValueError
        if frame_samples is None or frame_samples <= 0:
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["native memory artifact contains invalid values"]
    return {
        "source": "profiling-memory global allocator counters",
        **values,
        "peak_growth_bytes": values["peak_live_bytes"]
        - values["baseline_live_bytes"],
        "net_live_growth_bytes": values["final_live_bytes"]
        - values["baseline_live_bytes"],
        "allocated_bytes_per_frame": values["allocated_bytes"] / frame_samples,
        "deallocated_bytes_per_frame": values["deallocated_bytes"] / frame_samples,
        "allocation_calls_per_frame": values["allocation_calls"] / frame_samples,
        "deallocation_calls_per_frame": values["deallocation_calls"] / frame_samples,
    }, []


def read_resource_usage(path: Path) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return {}, ["missing GNU time resource-usage.txt"]
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        key, separator, value = line.partition("=")
        if separator:
            values[key] = value
    try:
        result = {
            "max_rss_kib": int(values["max_rss_kib"]),
            "user_cpu_secs": float(values["user_cpu_secs"]),
            "system_cpu_secs": float(values["system_cpu_secs"]),
            "exit_status": int(values["exit_status"]),
        }
        if (
            result["max_rss_kib"] <= 0
            or result["user_cpu_secs"] < 0
            or result["system_cpu_secs"] < 0
            or not math.isfinite(result["user_cpu_secs"])
            or not math.isfinite(result["system_cpu_secs"])
        ):
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["GNU time resource usage contains invalid values"]
    return result, errors


def read_task_dashboard_cpu(path: Path) -> tuple[dict[str, Any], list[str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse Task Dashboard CPU artifact: {error}"]
    required = {"schema_version", "system_invocations", "total_elapsed_ns"}
    if headers != required:
        return {}, ["Task Dashboard CPU artifact has unexpected columns"]
    if len(rows) != 1:
        return {}, ["Task Dashboard CPU artifact must contain exactly one row"]
    try:
        if rows[0]["schema_version"] != "1":
            raise ValueError
        invocations = int(rows[0]["system_invocations"])
        total_ns = int(rows[0]["total_elapsed_ns"])
        if invocations <= 0 or total_ns <= 0:
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["Task Dashboard CPU artifact contains invalid values"]
    return {
        "source": "profiling-only Instant timer",
        "invocations": invocations,
        "total_ns": total_ns,
        "mean_ns_per_invocation": total_ns / invocations,
    }, []


def collect_profile_artifact(
    *,
    args: argparse.Namespace,
    case: Case,
    run_dir: Path,
    trace_returncode: int | None,
    frame_samples: int | None,
    environment: dict[str, str],
) -> tuple[dict[str, Any] | None, list[str]]:
    if args.instrumentation == "capture":
        if args.capture_kind != "frame-time" or case.workload != "task-dashboard":
            return None, []
        cpu_summary, errors = read_task_dashboard_cpu(
            run_dir / "data" / "task_dashboard_cpu.csv"
        )
        artifact = {
            "instrumentation": "capture",
            "task_dashboard_cpu": cpu_summary,
        }
        write_json(run_dir / "profile-artifact.json", artifact)
        return artifact, errors
    if args.instrumentation == "memory":
        memory_frame_samples = 1 if case.workload == "save-transaction" else frame_samples
        memory_summary, memory_errors = read_native_memory(
            run_dir / "data" / "memory.csv", frame_samples=memory_frame_samples
        )
        resource_usage, resource_errors = read_resource_usage(
            run_dir / "resource-usage.txt"
        )
        artifact = {
            "instrumentation": "memory",
            "allocation_memory": memory_summary,
            "process_memory": resource_usage,
        }
        write_json(run_dir / "profile-artifact.json", artifact)
        return artifact, memory_errors + resource_errors
    errors: list[str] = []
    trace_path = run_dir / "trace.tracy"
    if trace_returncode != 0:
        errors.append(f"Tracy capture exited with status {trace_returncode}")
    if not trace_path.is_file() or trace_path.stat().st_size <= 0:
        errors.append("missing or empty trace.tracy")
        return None, errors
    artifact: dict[str, Any] = {
        "instrumentation": args.instrumentation,
        "trace": {
            "path": "trace.tracy",
            "bytes": trace_path.stat().st_size,
            "sha256": sha256(trace_path),
        },
    }
    csvexport = executable_path(args.tracy_csvexport_binary, "Tracy csvexport binary")
    if args.instrumentation == "tracy" and case.workload == "task-dashboard":
        zones_path = run_dir / "tracy-task-dashboard-zones.csv"
        returncode, timeout_error = run_csvexport(
            csvexport,
            trace_path,
            zones_path,
            run_dir / "tracy-task-dashboard-zones.log",
            ["-f", TRACY_DASHBOARD_ZONE_FILTER],
            min(args.timeout_secs, 120.0),
            environment,
        )
        if timeout_error:
            errors.append(timeout_error)
        if returncode != 0:
            errors.append(f"Tracy Task Dashboard csvexport exited with status {returncode}")
        else:
            cpu_summary, cpu_errors = read_tracy_zone_summary(zones_path)
            errors.extend(cpu_errors)
            artifact["task_dashboard_cpu"] = cpu_summary
    write_json(run_dir / "profile-artifact.json", artifact)
    return artifact, errors


def run_one(
    *,
    args: argparse.Namespace,
    binary: Path,
    session_dir: Path,
    case: Case,
    run_number: int,
    preflight: bool,
) -> Validation:
    require_persistent_output(session_dir)
    expected_binary_hash = getattr(args, "_profiling_binary_sha256", None)
    if not isinstance(expected_binary_hash, str) or not re.fullmatch(
        r"[0-9a-f]{64}", expected_binary_hash
    ):
        raise RuntimeError("profiling session binary fingerprint was not prepared")
    if sha256(binary) != expected_binary_hash:
        raise RuntimeError("profiling binary changed before starting a run")
    case_dir = session_dir / "cases" / case.identifier
    case_dir.mkdir(exist_ok=True)
    label = ("preflight-" if preflight else "run-") + f"{run_number:03d}"
    final_dir = case_dir / label
    temporary_dir = case_dir / f".{label}.tmp"
    if final_dir.exists() or temporary_dir.exists():
        raise RuntimeError(f"run directory collision: {final_dir}")
    temporary_dir.mkdir()
    data_dir = temporary_dir / "data"
    data_dir.mkdir()

    command = [
        str(binary),
        "--perf-scenario",
        "--perf-seed",
        str(case.seed),
        "--perf-size",
        case.size,
        "--perf-workload",
        case.workload,
        "--perf-render",
        case.render,
        "--perf-clock",
        args.clock_mode,
        "--perf-familiar-policy",
        case.familiar_policy,
        "--perf-operation-dialog",
        case.operation_dialog,
        "--perf-dashboard",
        case.dashboard_mode,
        "--perf-output-dir",
        str(data_dir),
    ]
    if args.contract is not None:
        command.extend(
            [
                "--perf-contract",
                args.contract,
                "--perf-stage",
                args.stage,
                "--perf-lane",
                args.lane,
            ]
        )
    if case.behavior_case is not None:
        command.extend(["--perf-behavior-case", case.behavior_case])
    if case.wall_phase is not None:
        command.extend(["--perf-wall-phase", case.wall_phase])
    if args.wall_presentation is not None:
        command.extend(["--perf-wall-presentation", args.wall_presentation])
    if args.door_presentation is not None:
        command.extend(["--perf-door-presentation", args.door_presentation])
    if args.wall_actual_window:
        command.append("--perf-wall-actual-window")
    if args.wall_art_matrix:
        command.append("--perf-wall-art-matrix")
    if args.wall_formwork_acceptance:
        command.append("--perf-wall-formwork-acceptance")
    if args.wall_art_zoom != "standard":
        command.extend(["--perf-wall-art-zoom", args.wall_art_zoom])
    if args.wall_color_actual_window:
        command.append("--perf-wall-color-actual-window")
    if args.capture_kind == "frame-time":
        command.extend(
            [
                "--perf-warmup-secs",
                str(args.warmup_secs),
                "--perf-measure-secs",
                str(args.measure_secs),
            ]
        )
    elif args.capture_kind in {"fixed-step-determinism", "field-core", "consumer-core"}:
        command.extend(
            [
                "--perf-fixed-hz",
                str(args.fixed_hz),
                "--perf-warmup-ticks",
                str(args.warmup_ticks),
                "--perf-audit-ticks",
                str(args.audit_ticks),
            ]
        )
    else:
        command.extend(["--perf-fixed-hz", str(args.fixed_hz)])
    if case.souls is not None:
        command.extend(["--spawn-souls", str(case.souls)])
        command.extend(["--spawn-familiars", str(case.familiars)])
    if args.window_width is not None:
        command.extend(["--perf-window-width", str(args.window_width)])
        command.extend(["--perf-window-height", str(args.window_height)])
    if args.window_scale_factor is not None:
        command.extend(
            ["--perf-window-scale-factor", str(args.window_scale_factor)]
        )
    if args.rtt_quality is not None:
        command.extend(["--perf-rtt-quality", args.rtt_quality])
    env = performance_environment()
    env.update(fixed_environment(args))
    runtime_root: Path | None = None
    runtime_cleanup_error: str | None = None
    if case.workload == "save-transaction":
        # This fixture owns its canonical size population. Do not let an
        # interactive shell's optional spawn overrides alter the sampled
        # world when the command deliberately relies on size defaults.
        env.pop("HW_SPAWN_SOULS", None)
        env.pop("HW_SPAWN_FAMILIARS", None)
        env["HW_PERF_SAVE_SAMPLE_KIND"] = "preflight" if preflight else "measured"
        runtime_parent = getattr(args, "_save_transaction_runtime_parent", None)
        if not isinstance(runtime_parent, Path):
            raise RuntimeError("save-transaction runtime parent was not prepared")
        runtime_root = runtime_parent / case.identifier / label
        runtime_root.mkdir(parents=True, exist_ok=False)
        env["HW_PERF_SAVE_RUNTIME_ROOT"] = str(runtime_root)
    launch_command = command
    if args.instrumentation == "memory":
        timer = executable_path(shutil.which("time"), "GNU time binary")
        launch_command = [
            str(timer),
            "-f",
            "max_rss_kib=%M\nuser_cpu_secs=%U\nsystem_cpu_secs=%S\nexit_status=%x",
            "-o",
            str(temporary_dir / "resource-usage.txt"),
            *command,
        ]
    command_for_artifact = launch_command
    if case.workload == "save-transaction":
        # Runtime/body paths are deliberately absent from retained evidence.
        command_for_artifact = [
            "<absolute-path>" if Path(argument).is_absolute() else argument
            for argument in launch_command
        ]
    (temporary_dir / "command.txt").write_text(
        " ".join(command_for_artifact) + "\n", encoding="utf-8"
    )
    write_json(
        temporary_dir / "requested-environment.json",
        {key: env[key] for key in sorted(fixed_environment(args))},
    )

    trace_process: subprocess.Popen[bytes] | None = None
    trace_log_handle = None
    trace_returncode: int | None = None
    if args.instrumentation == "tracy":
        capture = executable_path(args.tracy_capture_binary, "Tracy capture binary")
        trace_command = [
            str(capture),
            "-o",
            str(temporary_dir / "trace.tracy"),
            "-f",
        ]
        if args.tracy_capture_secs is not None:
            trace_command.extend(["-s", str(args.tracy_capture_secs)])
        (temporary_dir / "tracy-capture-command.txt").write_text(
            " ".join(trace_command) + "\n", encoding="utf-8"
        )
        trace_log_handle = (temporary_dir / "tracy-capture.log").open("wb")
        trace_process = subprocess.Popen(
            trace_command,
            cwd=REPO_ROOT,
            env=env,
            stdout=trace_log_handle,
            stderr=subprocess.STDOUT,
            creationflags=(
                subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
            ),
        )

    print(f"[{case.identifier} {label}]", flush=True)
    try:
        with (temporary_dir / "run.log").open("w", encoding="utf-8") as log_handle:
            if trace_process is None:
                try:
                    completed = subprocess.run(
                        launch_command,
                        cwd=REPO_ROOT,
                        env=env,
                        stdout=log_handle,
                        stderr=subprocess.STDOUT,
                        check=False,
                        timeout=args.timeout_secs,
                    )
                    returncode = completed.returncode
                except subprocess.TimeoutExpired:
                    returncode = 124
                    log_handle.write(
                        f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n"
                    )
            else:
                game_process = subprocess.Popen(
                    launch_command,
                    cwd=REPO_ROOT,
                    env=env,
                    stdout=log_handle,
                    stderr=subprocess.STDOUT,
                )
                deadline = time.monotonic() + args.timeout_secs
                trace_disconnect_requested = False
                while game_process.poll() is None:
                    if (
                        not trace_disconnect_requested
                        and (data_dir / "summary.csv").is_file()
                    ):
                        if trace_process.poll() is None:
                            trace_process.send_signal(
                                signal.CTRL_BREAK_EVENT
                                if os.name == "nt"
                                else signal.SIGINT
                            )
                        trace_disconnect_requested = True
                    if time.monotonic() >= deadline:
                        game_process.terminate()
                        try:
                            game_process.wait(timeout=5.0)
                        except subprocess.TimeoutExpired:
                            game_process.kill()
                            game_process.wait()
                        returncode = 124
                        log_handle.write(
                            f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n"
                        )
                        break
                    time.sleep(0.05)
                else:
                    returncode = game_process.returncode
                if not trace_disconnect_requested and trace_process.poll() is None:
                    trace_process.send_signal(
                        signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT
                    )
    finally:
        if trace_process is not None:
            try:
                trace_returncode = trace_process.wait(timeout=min(args.timeout_secs, 120.0))
            except subprocess.TimeoutExpired:
                trace_process.terminate()
                try:
                    trace_process.wait(timeout=5.0)
                except subprocess.TimeoutExpired:
                    trace_process.kill()
                    trace_process.wait()
                trace_returncode = 124
            if trace_log_handle is not None:
                trace_log_handle.close()
        if runtime_root is not None:
            try:
                # The root was created above for this one process and is never
                # an artifact. Remove only that exact fresh path so serialized
                # bodies/settings cannot accumulate inside the session.
                if not runtime_root.is_dir() or runtime_root.is_symlink():
                    raise OSError("isolated save runtime root is not a real directory")
                shutil.rmtree(runtime_root)
                runtime_parent = getattr(args, "_save_transaction_runtime_parent", None)
                if not isinstance(runtime_parent, Path):
                    raise OSError("save-transaction runtime parent was not prepared")
                empty_parent = runtime_root.parent
                while empty_parent.is_relative_to(runtime_parent):
                    try:
                        empty_parent.rmdir()
                    except OSError:
                        break
                    if empty_parent == runtime_parent:
                        break
                    empty_parent = empty_parent.parent
            except OSError as error:
                runtime_cleanup_error = (
                    f"failed to remove isolated save runtime root: {error}"
                )

    binary_provenance_error: str | None = None
    try:
        if sha256(binary) != expected_binary_hash:
            binary_provenance_error = "profiling binary changed while a run was executing"
    except OSError as error:
        binary_provenance_error = f"profiling binary could not be rehashed after a run: {error}"

    validation = validate_run(
        temporary_dir,
        returncode=returncode,
        expected_case=case,
        expected_adapter=args.adapter,
        expected_backend=args.backend,
        allow_log_patterns=args.allow_log_pattern,
        capture_kind=args.capture_kind,
        expected_warmup_secs=getattr(args, "warmup_secs", None),
        expected_measure_secs=getattr(args, "measure_secs", None),
        expected_fixed_hz=getattr(args, "fixed_hz", None),
        expected_warmup_ticks=getattr(args, "warmup_ticks", None),
        expected_audit_ticks=getattr(args, "audit_ticks", None),
        expected_window_backend=args.window_backend,
        expected_present_mode=args.present_mode,
        expected_window_width=args.window_width,
        expected_window_height=args.window_height,
        expected_window_scale_factor=args.window_scale_factor,
        expected_rtt_quality=args.rtt_quality,
        expected_contract=args.contract,
        expected_stage=args.stage,
        expected_lane=args.lane,
    )
    frame_samples = None
    if validation.summary is not None:
        try:
            frame_samples = int(validation.summary["samples"])
        except (KeyError, TypeError, ValueError):
            pass
    profile_artifact, profile_errors = collect_profile_artifact(
        args=args,
        case=case,
        run_dir=temporary_dir,
        trace_returncode=trace_returncode,
        frame_samples=frame_samples,
        environment=env,
    )
    validation.profile_artifact = profile_artifact
    validation.reasons.extend(profile_errors)
    if runtime_cleanup_error is not None:
        validation.reasons.append(runtime_cleanup_error)
    if binary_provenance_error is not None:
        validation.reasons.append(binary_provenance_error)
    validation.valid = not validation.reasons
    validation.reasons.extend(
        enforce_environment_lock(
            args=args,
            session_dir=session_dir,
            validation=validation,
            preflight=preflight,
        )
    )
    validation.valid = not validation.reasons
    write_json(temporary_dir / "validation.json", validation.to_json())
    write_json(
        temporary_dir / "run-metadata.json",
        {
            "case": asdict(case),
            "rtt_light_contract": (
                {
                    "contract_id": args.contract,
                    "stage_id": args.stage,
                    "lane": args.lane,
                    "layout_checksum": build_fixture_layout(
                        load_rtt_light_contract(args.contract), case.size
                    )["layout_checksum"],
                }
                if args.contract is not None
                else None
            ),
            "preflight": preflight,
            "returncode": returncode,
            "trace_returncode": trace_returncode,
            "started_by": "scripts/perf.py",
            "actual_adapter": validation.adapter,
            "actual_window": validation.window,
            "actual_render_inventory": validation.render_inventory,
            "runtime_data_cleaned": runtime_root is None
            or runtime_cleanup_error is None,
            "runtime_data_cleanup": "per-run",
        },
    )
    temporary_dir.replace(final_dir)
    return validation
