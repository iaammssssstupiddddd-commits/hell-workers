#!/usr/bin/env python3
"""Run, validate, summarize, and compare Hell Workers performance captures.

The game writes one CSV pair per run. This script owns the experiment-level
contract: a clean output tree, direct binary execution, captured environment,
log validation, and repeat aggregation. It intentionally uses only Python's
standard library so it can run on every developer machine that can run Cargo.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
SUMMARY_SCHEMA_VERSION = "10"
DETERMINISM_SCHEMA_VERSION = "1"
DEFAULT_SEED = 20_260_712
SCENE_ROOT_COLUMNS = (
    "soul_proxy_3d",
    "soul_mask_proxy_3d",
    "soul_shadow_proxy_3d",
    "familiar_proxy_3d",
    "building_3d_visual",
)
EXPECTED_SUMMARY_COLUMNS = {
    "schema_version",
    "seed",
    "workload",
    "size",
    "render",
    "configured_souls",
    "configured_familiars",
    "initial_souls",
    "initial_familiars",
    "initial_designations",
    "initial_state_checksum",
    "warmup_souls",
    "warmup_familiars",
    "warmup_designations",
    "warmup_state_checksum",
    "measure_end_souls",
    "measure_end_familiars",
    "measure_end_designations",
    "measure_end_state_checksum",
    "samples",
    "p50_ms",
    "p95_ms",
    "p99_ms",
    "max_ms",
    "warmup_virtual_secs",
    "warmup_real_secs",
    "measure_virtual_secs",
    "measure_real_secs",
    "virtual_time_speed",
    "delegation_latest_ms",
    "delegation_cycles",
    "incoming_snapshot_builds",
    "delegation_familiars_processed",
    "source_selector_calls",
    "source_selector_scanned_items",
    "reachable_with_cache_calls",
    "task_execution_souls_queried",
    "task_execution_idle_skips",
    "task_execution_handler_runs",
    "reservation_sync_full_rebuilds",
    "reservation_sync_pending_tasks_scanned",
    "reservation_sync_assigned_tasks_scanned",
    "runtime_path_actor_new_core_searches",
    "runtime_path_actor_new_deferred",
    "runtime_path_actor_reuse_core_searches",
    "runtime_path_actor_reuse_deferred",
    "runtime_path_actor_rest_fallback_core_searches",
    "runtime_path_actor_rest_fallback_deferred",
    "runtime_path_escape_core_searches",
    "runtime_path_escape_deferred",
    "runtime_path_task_execution_core_searches",
    "runtime_path_task_execution_deferred",
    "runtime_path_bucket_transport_core_searches",
    "runtime_path_bucket_transport_deferred",
    "runtime_path_total_core_searches",
    "runtime_path_expanded_nodes",
    "runtime_path_max_expanded_nodes_per_search",
    "runtime_path_active_task_max_defer_frames",
    "runtime_path_idle_or_rest_max_defer_frames",
    "runtime_path_deferred_actor_retries",
    "door_open_souls_scanned",
    "door_open_waypoints_scanned",
    "door_close_souls_scanned",
    "construction_floor_sites_considered",
    "construction_wall_sites_considered",
    "construction_floor_tiles_inspected",
    "construction_wall_tiles_inspected",
    "construction_evacuation_candidates_scanned",
    "construction_floor_phase_elapsed_micros",
    "construction_floor_completion_elapsed_micros",
    "construction_wall_phase_elapsed_micros",
    "construction_wall_completion_elapsed_micros",
    "slow_simulation_steps",
    "slow_simulation_souls_updated",
    "slow_simulation_idle_decisions",
    "slow_simulation_idle_spatial_target_lookups",
    "slow_simulation_state_sanity_audits",
    "energy_power_output_runs",
    "energy_grid_recalc_runs",
    "energy_lamp_steps",
    "energy_lamp_candidates_scanned",
}
ADAPTER_RE = re.compile(
    r'AdapterInfo \{ name: "(?P<name>[^"]+)".*?driver: "(?P<driver>[^"]*)", '
    r'driver_info: "(?P<driver_info>[^"]*)", backend: (?P<backend>[A-Za-z0-9_]+)'
)
LOG_LEVEL_RE = re.compile(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler")
CHECKSUM_POLICY_REASON_PREFIXES = (
    "initial_state_checksum differs across repeated runs:",
    "warmup_state_checksum differs across repeated runs:",
    "measure_end_state_checksum differs across repeated runs:",
    "determinism checkpoints differ across repeated runs:",
)
DETERMINISM_COLUMNS = (
    "schema_version",
    "checkpoint",
    "update_tick",
    "fixed_timestep_ns",
    "virtual_delta_ns",
    "virtual_elapsed_ns",
    "fixed_delta_ns",
    "fixed_elapsed_ns",
    "fixed_overstep_ns",
    "virtual_paused",
    "virtual_relative_speed_bits",
    "virtual_effective_speed_bits",
    "souls",
    "familiars",
    "designations",
    "state_checksum",
)
DETERMINISM_EARLY_CHECKPOINTS = (
    ("post-update-1", 1),
    ("post-update-8", 8),
    ("post-update-32", 32),
    ("post-update-128", 128),
)
ONE_F64_BITS = "3ff0000000000000"
ZERO_F64_BITS = "0000000000000000"


@dataclass(frozen=True)
class Case:
    workload: str
    size: str
    render: str
    seed: int
    souls: int | None
    familiars: int | None

    @property
    def identifier(self) -> str:
        population = ""
        if self.souls is not None:
            population = f"-souls-{self.souls}-familiars-{self.familiars}"
        return f"{self.workload}-{self.size}-{self.render}-seed-{self.seed}{population}"


@dataclass
class Validation:
    valid: bool
    reasons: list[str]
    summary: dict[str, str] | None
    adapter: dict[str, str] | None
    warning_lines: list[str]
    teardown_warning_lines: list[str]
    determinism: list[dict[str, str]] | None = None
    scene_roots: dict[str, str] | None = None

    def to_json(self) -> dict[str, Any]:
        return {
            "valid": self.valid,
            "reasons": self.reasons,
            "summary": self.summary,
            "adapter": self.adapter,
            "warning_lines": self.warning_lines,
            "teardown_warning_lines": self.teardown_warning_lines,
            "determinism": self.determinism,
            "scene_roots": self.scene_roots,
        }


def parse_csv_list(value: str, allowed: set[str], label: str) -> list[str]:
    values = [item.strip() for item in value.split(",") if item.strip()]
    if not values:
        raise ValueError(f"{label} must not be empty")
    unknown = sorted(set(values) - allowed)
    if unknown:
        raise ValueError(f"unsupported {label}: {', '.join(unknown)}")
    return values


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


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


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


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_summary(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
            headers = set(rows[0].keys()) if rows else set()
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse summary.csv: {error}"]
    missing = sorted(EXPECTED_SUMMARY_COLUMNS - headers)
    if missing:
        errors.append("summary.csv missing columns: " + ", ".join(missing))
    if len(rows) != 1:
        errors.append(f"summary.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    return rows[0], errors


def read_scene_roots(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse scene_roots.csv: {error}"]

    errors: list[str] = []
    missing = sorted(set(SCENE_ROOT_COLUMNS) - headers)
    if missing:
        errors.append("scene_roots.csv missing columns: " + ", ".join(missing))
    if len(rows) != 1:
        errors.append(f"scene_roots.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    for column in SCENE_ROOT_COLUMNS:
        try:
            if int(rows[0][column]) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"scene_roots.csv {column} must be a nonnegative integer")
    return rows[0], errors


def read_frames(path: Path, expected_samples: int | None) -> list[str]:
    errors: list[str] = []
    if not path.is_file():
        return [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
    except (csv.Error, OSError, UnicodeError) as error:
        return [f"cannot parse frames.csv: {error}"]
    if not rows:
        errors.append("frames.csv has no samples")
    if expected_samples is not None and len(rows) != expected_samples:
        errors.append(f"frames.csv has {len(rows)} rows but summary declares {expected_samples}")
    for index, row in enumerate(rows):
        try:
            float(row["frame_time_ms"])
        except (KeyError, TypeError, ValueError):
            errors.append(f"frames.csv row {index} has an invalid frame_time_ms")
            break
    return errors


def read_determinism(
    path: Path, *, warmup_ticks: int, audit_ticks: int
) -> tuple[list[dict[str, str]] | None, list[str]]:
    expected_columns = set(DETERMINISM_COLUMNS)
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse determinism.csv: {error}"]

    errors: list[str] = []
    if headers != expected_columns:
        missing = sorted(expected_columns - headers)
        unexpected = sorted(headers - expected_columns)
        if missing:
            errors.append("determinism.csv missing columns: " + ", ".join(missing))
        if unexpected:
            errors.append("determinism.csv has unexpected columns: " + ", ".join(unexpected))
    expected_checkpoints = [
        ("fixture-pre-update", 0),
        *DETERMINISM_EARLY_CHECKPOINTS,
        ("post-warmup", warmup_ticks),
        ("post-audit-end", warmup_ticks + audit_ticks),
    ]
    observed_checkpoints = [
        (row.get("checkpoint", ""), row.get("update_tick", "")) for row in rows
    ]
    expected_pairs = [(name, str(tick)) for name, tick in expected_checkpoints]
    if observed_checkpoints != expected_pairs:
        errors.append(
            "determinism.csv checkpoints are "
            + ",".join(f"{name}@{tick}" for name, tick in observed_checkpoints)
            + "; expected "
            + ",".join(f"{name}@{tick}" for name, tick in expected_pairs)
        )
    for index, row in enumerate(rows):
        if row.get("schema_version") != DETERMINISM_SCHEMA_VERSION:
            errors.append(
                f"determinism.csv row {index} schema_version is {row.get('schema_version')!r}, "
                f"expected {DETERMINISM_SCHEMA_VERSION}"
            )
        for field in (
            "update_tick",
            "fixed_timestep_ns",
            "virtual_delta_ns",
            "virtual_elapsed_ns",
            "fixed_delta_ns",
            "fixed_elapsed_ns",
            "fixed_overstep_ns",
            "souls",
            "familiars",
            "designations",
        ):
            try:
                if int(row[field]) < 0:
                    raise ValueError
            except (KeyError, TypeError, ValueError):
                errors.append(f"determinism.csv row {index} has invalid {field}")
                break
        checksum = row.get("state_checksum", "")
        if not re.fullmatch(r"[0-9a-f]{16}", checksum):
            errors.append(f"determinism.csv row {index} has invalid state_checksum")
        if row.get("virtual_paused") not in {"0", "1"}:
            errors.append(f"determinism.csv row {index} has invalid virtual_paused")
        for field in ("virtual_relative_speed_bits", "virtual_effective_speed_bits"):
            if not re.fullmatch(r"[0-9a-f]{16}", row.get(field, "")):
                errors.append(f"determinism.csv row {index} has invalid {field}")

    if rows:
        try:
            timestep_ns = int(rows[0]["fixed_timestep_ns"])
        except (KeyError, TypeError, ValueError):
            timestep_ns = 0
        if timestep_ns <= 0:
            errors.append("determinism.csv fixed_timestep_ns must be greater than zero")
        for index, row in enumerate(rows):
            if timestep_ns <= 0:
                break
            try:
                tick = int(row["update_tick"])
            except (KeyError, TypeError, ValueError):
                continue
            is_initial = index == 0
            expected_elapsed = tick * timestep_ns
            if row.get("fixed_timestep_ns") != str(timestep_ns):
                errors.append(f"determinism.csv row {index} changes fixed_timestep_ns")
            if is_initial:
                expected_values = {
                    "virtual_paused": "1",
                    "virtual_delta_ns": "0",
                    "virtual_elapsed_ns": "0",
                    "fixed_delta_ns": "0",
                    "fixed_elapsed_ns": "0",
                    "fixed_overstep_ns": "0",
                    "virtual_relative_speed_bits": ONE_F64_BITS,
                    "virtual_effective_speed_bits": ZERO_F64_BITS,
                }
            else:
                expected_values = {
                    "virtual_paused": "0",
                    "virtual_delta_ns": str(timestep_ns),
                    "virtual_elapsed_ns": str(expected_elapsed),
                    "fixed_delta_ns": str(timestep_ns),
                    "fixed_elapsed_ns": str(expected_elapsed),
                    "fixed_overstep_ns": "0",
                    "virtual_relative_speed_bits": ONE_F64_BITS,
                    "virtual_effective_speed_bits": ONE_F64_BITS,
                }
            for field, expected in expected_values.items():
                if row.get(field) != expected:
                    errors.append(
                        f"determinism.csv row {index} {field} is {row.get(field)!r}, expected {expected!r}"
                    )
    return (rows if not errors else None), errors


def parse_adapter(log_text: str) -> dict[str, str] | None:
    match = ADAPTER_RE.search(log_text)
    return match.groupdict() if match else None


def classify_log_warnings(log_text: str, allow_patterns: Iterable[str]) -> tuple[list[str], list[str]]:
    compiled_allow = [re.compile(pattern) for pattern in allow_patterns]
    pre_capture_problems: list[str] = []
    post_capture_warnings: list[str] = []
    capture_completed = False
    for line in log_text.splitlines():
        if "PERF_CAPTURE: wrote" in line or "PERF_DETERMINISM_AUDIT: wrote" in line:
            capture_completed = True
            continue
        if not LOG_LEVEL_RE.search(line):
            continue
        if any(pattern.search(line) for pattern in compiled_allow):
            continue
        if capture_completed:
            post_capture_warnings.append(line)
        else:
            pre_capture_problems.append(line)
    return pre_capture_problems, post_capture_warnings


def validate_run(
    run_dir: Path,
    *,
    returncode: int,
    expected_case: Case,
    expected_adapter: str | None,
    expected_backend: str | None,
    allow_log_patterns: Iterable[str],
    capture_kind: str = "frame-time",
    expected_warmup_secs: float | None = None,
    expected_measure_secs: float | None = None,
    expected_fixed_hz: int | None = None,
    expected_warmup_ticks: int | None = None,
    expected_audit_ticks: int | None = None,
) -> Validation:
    reasons: list[str] = []
    data_dir = run_dir / "data"
    summary = None
    determinism = None
    scene_roots = None
    if capture_kind == "frame-time":
        summary, summary_errors = read_summary(data_dir / "summary.csv")
        reasons.extend(summary_errors)
        scene_roots, scene_root_errors = read_scene_roots(data_dir / "scene_roots.csv")
        reasons.extend(scene_root_errors)
    elif capture_kind == "fixed-step-determinism":
        if expected_fixed_hz is None or expected_warmup_ticks is None or expected_audit_ticks is None:
            reasons.append("fixed-step audit validation is missing tick configuration")
        else:
            determinism, determinism_errors = read_determinism(
                data_dir / "determinism.csv",
                warmup_ticks=expected_warmup_ticks,
                audit_ticks=expected_audit_ticks,
            )
            reasons.extend(determinism_errors)
    else:
        reasons.append(f"unsupported capture kind {capture_kind!r}")
    if capture_kind == "fixed-step-determinism" and (
        (data_dir / "summary.csv").exists()
        or (data_dir / "frames.csv").exists()
        or (data_dir / "scene_roots.csv").exists()
    ):
        reasons.append("fixed-step audit must not write frame-time artifacts")
    if capture_kind == "frame-time" and (data_dir / "determinism.csv").exists():
        reasons.append("frame-time capture must not write determinism.csv")
    if returncode != 0:
        reasons.append(f"process exited with status {returncode}")

    if summary is not None:
        if summary.get("schema_version") != SUMMARY_SCHEMA_VERSION:
            reasons.append(
                f"summary schema is {summary.get('schema_version')!r}, expected {SUMMARY_SCHEMA_VERSION}"
            )
        expected_values = {
            "seed": str(expected_case.seed),
            "workload": expected_case.workload,
            "size": expected_case.size,
            "render": expected_case.render,
        }
        for key, expected in expected_values.items():
            if summary.get(key) != expected:
                reasons.append(f"summary {key} is {summary.get(key)!r}, expected {expected!r}")
        try:
            samples = int(summary["samples"])
        except (KeyError, ValueError):
            samples = None
            reasons.append("summary samples is invalid")
        if samples is not None and samples <= 0:
            reasons.append("summary samples must be greater than zero")
        reasons.extend(read_frames(data_dir / "frames.csv", samples))

    if summary is not None and scene_roots is not None:
        try:
            expected_souls = int(summary["initial_souls"])
            expected_familiars = int(summary["initial_familiars"])
        except (KeyError, ValueError):
            reasons.append("summary initial population is invalid for scene root validation")
        else:
            expected_counts = {
                "soul_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "soul_mask_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "soul_shadow_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                "familiar_proxy_3d": 0 if expected_case.render == "cpu" else expected_familiars,
            }
            for column, expected in expected_counts.items():
                if scene_roots.get(column) != str(expected):
                    reasons.append(
                        f"scene_roots.csv {column} is {scene_roots.get(column)!r}, "
                        f"expected {expected!r} for {expected_case.render}"
                    )

    log_path = run_dir / "run.log"
    if not log_path.is_file():
        log_text = ""
        reasons.append("missing run.log")
    else:
        log_text = log_path.read_text(encoding="utf-8", errors="replace")
        completion_marker = (
            "PERF_CAPTURE: wrote"
            if capture_kind == "frame-time"
            else "PERF_DETERMINISM_AUDIT: wrote"
        )
        if completion_marker not in log_text:
            reasons.append(f"{completion_marker} completion marker is absent")
        expected_clock_mode = "fixed" if capture_kind == "fixed-step-determinism" else "realtime"
        if f"clock={expected_clock_mode}" not in log_text:
            reasons.append(
                f"PERF_SCENARIO clock marker is absent or does not match {expected_clock_mode!r}"
            )
        for marker in (
            f"seed={expected_case.seed}",
            f"workload={expected_case.workload}",
            f"size={expected_case.size}",
            f"render={expected_case.render}",
        ):
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        if capture_kind == "fixed-step-determinism" and expected_fixed_hz is not None:
            marker = f"fixed_hz={expected_fixed_hz}"
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
    warnings, teardown_warnings = classify_log_warnings(log_text, allow_log_patterns)
    reasons.extend(f"unexpected log warning/error: {line}" for line in warnings)

    adapter = parse_adapter(log_text)
    if expected_adapter:
        if adapter is None:
            reasons.append("actual WGPU adapter was not found in run.log")
        elif expected_adapter.casefold() not in adapter["name"].casefold():
            reasons.append(
                f"actual adapter {adapter['name']!r} does not match requested {expected_adapter!r}"
            )
    if expected_backend and expected_backend != "auto":
        if adapter is None:
            reasons.append("actual WGPU backend was not found in run.log")
        elif adapter["backend"].casefold() != expected_backend.casefold():
            reasons.append(
                f"actual backend {adapter['backend']!r} does not match requested {expected_backend!r}"
            )

    return Validation(
        valid=not reasons,
        reasons=reasons,
        summary=summary,
        adapter=adapter,
        warning_lines=warnings,
        teardown_warning_lines=teardown_warnings,
        determinism=determinism,
        scene_roots=scene_roots,
    )


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


def cargo_features(instrumentation: str) -> str:
    return {
        "capture": "profiling",
        "tracy": "profiling-tracy",
        "memory": "profiling-memory",
    }[instrumentation]


def build_binary(args: argparse.Namespace) -> Path:
    binary = Path(args.binary).resolve() if args.binary else REPO_ROOT / "target/profiling/bevy_app"
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
    completed = subprocess.run(command, cwd=REPO_ROOT, check=False)
    if completed.returncode != 0:
        raise RuntimeError("profiling binary build failed")
    if not binary.is_file():
        raise RuntimeError(f"Cargo succeeded but profiling binary is missing: {binary}")
    return binary


def prepare_session(args: argparse.Namespace, binary: Path, cases: list[Case]) -> Path:
    if args.output:
        output = Path(args.output)
        session_dir = output if output.is_absolute() else REPO_ROOT / output
    else:
        timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
        session_dir = REPO_ROOT / "target/perf-runs" / f"{timestamp}-{git_metadata()['short_commit']}"
    session_dir = session_dir.resolve()
    if session_dir.exists():
        raise RuntimeError(f"output directory already exists: {session_dir}")
    session_dir.mkdir(parents=True)
    (session_dir / "cases").mkdir()

    matrix = {
        "workload": args.workload,
        "sizes": [case.size for case in cases],
        "renders": [case.render for case in cases],
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
        "capture_kind": args.capture_kind,
        "clock_mode": args.clock_mode,
        "warmup_checksum_policy": getattr(args, "warmup_checksum_policy", None),
        "measure_end_checksum_policy": getattr(args, "measure_end_checksum_policy", None),
    }
    write_json(session_dir / "matrix.json", matrix)
    manifest = {
        "schema_version": 1,
        "created_at": datetime.now(UTC).isoformat(),
        "repo_root": str(REPO_ROOT),
        "git": git_metadata(),
        "host": host_metadata(),
        "binary": {
            "path": str(binary),
            "sha256": sha256(binary),
            "instrumentation": args.instrumentation,
        },
        "requested_environment": fixed_environment(args),
        "matrix": matrix,
        "cases": [asdict(case) | {"id": case.identifier} for case in cases],
        "actual_adapters": [],
        "status": "running",
    }
    write_json(session_dir / "manifest.json", manifest)
    return session_dir


def run_one(
    *,
    args: argparse.Namespace,
    binary: Path,
    session_dir: Path,
    case: Case,
    run_number: int,
    preflight: bool,
) -> Validation:
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
        "--perf-output-dir",
        str(data_dir),
    ]
    if args.capture_kind == "frame-time":
        command.extend(
            [
                "--perf-warmup-secs",
                str(args.warmup_secs),
                "--perf-measure-secs",
                str(args.measure_secs),
            ]
        )
    else:
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
    if case.souls is not None:
        command.extend(["--spawn-souls", str(case.souls)])
        command.extend(["--spawn-familiars", str(case.familiars)])
    env = os.environ.copy()
    env.update(fixed_environment(args))
    (temporary_dir / "command.txt").write_text(" ".join(command) + "\n", encoding="utf-8")
    write_json(
        temporary_dir / "requested-environment.json",
        {key: env[key] for key in sorted(fixed_environment(args))},
    )

    print(f"[{case.identifier} {label}]", flush=True)
    with (temporary_dir / "run.log").open("w", encoding="utf-8") as log_handle:
        try:
            completed = subprocess.run(
                command,
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
            log_handle.write(f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n")

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
    )
    write_json(temporary_dir / "validation.json", validation.to_json())
    write_json(
        temporary_dir / "run-metadata.json",
        {
            "case": asdict(case),
            "preflight": preflight,
            "returncode": returncode,
            "started_by": "scripts/perf.py",
            "actual_adapter": validation.adapter,
        },
    )
    temporary_dir.replace(final_dir)
    return validation


def load_valid_runs(session_dir: Path) -> list[tuple[Path, Validation]]:
    runs: list[tuple[Path, Validation]] = []
    for validation_path in sorted((session_dir / "cases").glob("*/run-*/validation.json")):
        payload = json.loads(validation_path.read_text(encoding="utf-8"))
        validation = Validation(
            valid=bool(payload["valid"]),
            reasons=list(payload["reasons"]),
            summary=payload.get("summary"),
            adapter=payload.get("adapter"),
            warning_lines=list(payload.get("warning_lines", [])),
            teardown_warning_lines=list(payload.get("teardown_warning_lines", [])),
            determinism=payload.get("determinism"),
            scene_roots=payload.get("scene_roots"),
        )
        runs.append((validation_path.parent, validation))
    return runs


def median_and_mad(values: list[float]) -> tuple[float, float]:
    center = statistics.median(values)
    return center, statistics.median([abs(value - center) for value in values])


def reset_checksum_policy(runs: list[tuple[Path, Validation]]) -> bool:
    """Restore per-run validation before applying the session checksum policy.

    This lets `summarize --warmup-checksum-policy ...` safely switch between
    recording and requiring a dynamic warm-up checkpoint without masking an
    unrelated capture failure.
    """
    changed = False
    for run_dir, validation in runs:
        reasons = [
            reason
            for reason in validation.reasons
            if not reason.startswith(CHECKSUM_POLICY_REASON_PREFIXES)
        ]
        valid = not reasons
        if reasons != validation.reasons or valid != validation.valid:
            validation.reasons = reasons
            validation.valid = valid
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed


def apply_checksum_policy(
    runs: list[tuple[Path, Validation]],
    warmup_policy: str,
    measure_end_policy: str,
) -> bool:
    by_case: dict[str, list[tuple[Path, Validation]]] = {}
    for run_dir, validation in runs:
        by_case.setdefault(run_dir.parent.name, []).append((run_dir, validation))
    changed = False
    for case_runs in by_case.values():
        initial_checksums = {
            validation.summary["initial_state_checksum"]
            for _, validation in case_runs
            if validation.valid and validation.summary is not None
        }
        if len(initial_checksums) > 1:
            reason = "initial_state_checksum differs across repeated runs: " + ", ".join(
                sorted(initial_checksums)
            )
            for run_dir, validation in case_runs:
                if not validation.valid:
                    continue
                validation.valid = False
                validation.reasons.append(reason)
                write_json(run_dir / "validation.json", validation.to_json())
                changed = True

        for field, policy in (
            ("warmup_state_checksum", warmup_policy),
            ("measure_end_state_checksum", measure_end_policy),
        ):
            if policy != "require":
                continue
            checksums = {
                validation.summary[field]
                for _, validation in case_runs
                if validation.valid and validation.summary is not None
            }
            if len(checksums) <= 1:
                continue
            reason = f"{field} differs across repeated runs: " + ", ".join(sorted(checksums))
            for run_dir, validation in case_runs:
                if not validation.valid:
                    continue
                validation.valid = False
                validation.reasons.append(reason)
                write_json(run_dir / "validation.json", validation.to_json())
                changed = True
    return changed


def determinism_signature(checkpoints: list[dict[str, str]]) -> str:
    fields = DETERMINISM_COLUMNS
    serialized = "\n".join(
        ",".join(checkpoint[field] for field in fields) for checkpoint in checkpoints
    )
    return hashlib.sha256(serialized.encode("utf-8")).hexdigest()[:16]


def apply_determinism_policy(runs: list[tuple[Path, Validation]]) -> bool:
    by_case: dict[str, list[tuple[Path, Validation]]] = {}
    for run_dir, validation in runs:
        by_case.setdefault(run_dir.parent.name, []).append((run_dir, validation))

    changed = False
    for case_runs in by_case.values():
        signatures = {
            determinism_signature(validation.determinism)
            for _, validation in case_runs
            if validation.valid and validation.determinism is not None
        }
        if len(signatures) <= 1:
            continue
        reason = "determinism checkpoints differ across repeated runs: " + ", ".join(
            sorted(signatures)
        )
        for run_dir, validation in case_runs:
            if not validation.valid:
                continue
            validation.valid = False
            validation.reasons.append(reason)
            write_json(run_dir / "validation.json", validation.to_json())
            changed = True
    return changed


def summarize_determinism_session(
    session_dir: Path, manifest: dict[str, Any], runs: list[tuple[Path, Validation]]
) -> bool:
    groups: dict[str, list[Validation]] = {}
    all_adapters: list[dict[str, str]] = []
    invalid_runs: list[tuple[Path, Validation]] = []
    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in all_adapters:
            all_adapters.append(validation.adapter)
        if validation.valid and validation.determinism is not None:
            groups.setdefault(run_dir.parent.name, []).append(validation)
        else:
            invalid_runs.append((run_dir, validation))

    aggregate_columns = [
        "case_id",
        "valid_runs",
        "determinism_signature",
        "post_capture_teardown_warning_counts",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id, validations in sorted(groups.items()):
        signatures = {
            determinism_signature(validation.determinism)
            for validation in validations
            if validation.determinism is not None
        }
        aggregate_rows.append(
            {
                "case_id": case_id,
                "valid_runs": str(len(validations)),
                "determinism_signature": ";".join(sorted(signatures)),
                "post_capture_teardown_warning_counts": ";".join(
                    str(len(validation.teardown_warning_lines)) for validation in validations
                ),
                "adapter": json.dumps(validations[0].adapter, sort_keys=True),
            }
        )

    with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report_lines = [
        "# Fixed-step determinism audit report",
        "",
        f"- Valid runs: {sum(len(rows) for rows in groups.values())}",
        f"- Invalid runs: {len(invalid_runs)}",
        "- Contract: every `determinism.csv` checkpoint must be byte-for-byte identical per case.",
        "- Frame-time quantiles are intentionally absent and this session cannot be used with `compare`.",
        "- Post-capture teardown warnings (recorded, not validity failures): "
        + str(sum(len(validation.teardown_warning_lines) for _, validation in runs)),
        "",
    ]
    if aggregate_rows:
        report_lines.extend(
            [
                "## Aggregate",
                "",
                "| Case | Valid runs | Determinism signature |",
                "| --- | ---: | --- |",
            ]
        )
        for row in aggregate_rows:
            report_lines.append(
                f"| {row['case_id']} | {row['valid_runs']} | {row['determinism_signature']} |"
            )
        report_lines.append("")
    if invalid_runs:
        report_lines.extend(["## Invalid runs", ""])
        for run_dir, validation in invalid_runs:
            report_lines.append(f"- `{run_dir.relative_to(session_dir)}`: {'; '.join(validation.reasons)}")
    (session_dir / "report.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")

    manifest["actual_adapters"] = all_adapters
    manifest["status"] = "valid" if not invalid_runs else "invalid"
    write_json(session_dir / "manifest.json", manifest)
    return not invalid_runs


def summarize_session(
    session_dir: Path,
    warmup_policy: str | None = None,
    measure_end_policy: str | None = None,
) -> bool:
    manifest_path = session_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    matrix = manifest["matrix"]
    if matrix.get("capture_kind") == "fixed-step-determinism":
        runs = load_valid_runs(session_dir)
        reset_checksum_policy(runs)
        runs = load_valid_runs(session_dir)
        apply_determinism_policy(runs)
        runs = load_valid_runs(session_dir)
        return summarize_determinism_session(session_dir, manifest, runs)

    warmup_policy = warmup_policy or matrix["warmup_checksum_policy"]
    measure_end_policy = measure_end_policy or matrix.get("measure_end_checksum_policy", "record")
    runs = load_valid_runs(session_dir)
    reset_checksum_policy(runs)
    runs = load_valid_runs(session_dir)
    apply_checksum_policy(runs, warmup_policy, measure_end_policy)
    runs = load_valid_runs(session_dir)

    groups: dict[str, list[Validation]] = {}
    all_adapters: list[dict[str, str]] = []
    invalid_runs: list[tuple[Path, Validation]] = []
    for run_dir, validation in runs:
        if validation.adapter and validation.adapter not in all_adapters:
            all_adapters.append(validation.adapter)
        if validation.valid and validation.summary is not None:
            groups.setdefault(run_dir.parent.name, []).append(validation)
        else:
            invalid_runs.append((run_dir, validation))

    aggregate_columns = [
        "case_id",
        "valid_runs",
        "p50_median_ms",
        "p50_mad_ms",
        "p95_median_ms",
        "p95_mad_ms",
        "p99_median_ms",
        "p99_mad_ms",
        "max_median_ms",
        "max_mad_ms",
        "initial_state_checksum",
        "warmup_checksums",
        "measure_end_checksums",
        "post_capture_teardown_warning_counts",
        "task_execution_souls_queried_median",
        "task_execution_souls_queried_mad",
        "task_execution_idle_skips_median",
        "task_execution_idle_skips_mad",
        "task_execution_handler_runs_median",
        "task_execution_handler_runs_mad",
        "task_execution_idle_skip_pct_median",
        "task_execution_idle_skip_pct_mad",
        "task_execution_handler_run_pct_median",
        "task_execution_handler_run_pct_mad",
        "reservation_sync_full_rebuilds_median",
        "reservation_sync_full_rebuilds_mad",
        "reservation_sync_pending_tasks_scanned_median",
        "reservation_sync_pending_tasks_scanned_mad",
        "reservation_sync_assigned_tasks_scanned_median",
        "reservation_sync_assigned_tasks_scanned_mad",
        "runtime_path_actor_new_core_searches_median",
        "runtime_path_actor_new_core_searches_mad",
        "runtime_path_actor_new_deferred_median",
        "runtime_path_actor_new_deferred_mad",
        "runtime_path_actor_reuse_core_searches_median",
        "runtime_path_actor_reuse_core_searches_mad",
        "runtime_path_actor_reuse_deferred_median",
        "runtime_path_actor_reuse_deferred_mad",
        "runtime_path_actor_rest_fallback_core_searches_median",
        "runtime_path_actor_rest_fallback_core_searches_mad",
        "runtime_path_actor_rest_fallback_deferred_median",
        "runtime_path_actor_rest_fallback_deferred_mad",
        "runtime_path_escape_core_searches_median",
        "runtime_path_escape_core_searches_mad",
        "runtime_path_escape_deferred_median",
        "runtime_path_escape_deferred_mad",
        "runtime_path_task_execution_core_searches_median",
        "runtime_path_task_execution_core_searches_mad",
        "runtime_path_task_execution_deferred_median",
        "runtime_path_task_execution_deferred_mad",
        "runtime_path_bucket_transport_core_searches_median",
        "runtime_path_bucket_transport_core_searches_mad",
        "runtime_path_bucket_transport_deferred_median",
        "runtime_path_bucket_transport_deferred_mad",
        "runtime_path_total_core_searches_median",
        "runtime_path_total_core_searches_mad",
        "runtime_path_expanded_nodes_median",
        "runtime_path_expanded_nodes_mad",
        "runtime_path_max_expanded_nodes_per_search_median",
        "runtime_path_max_expanded_nodes_per_search_mad",
        "runtime_path_active_task_max_defer_frames_median",
        "runtime_path_active_task_max_defer_frames_mad",
        "runtime_path_idle_or_rest_max_defer_frames_median",
        "runtime_path_idle_or_rest_max_defer_frames_mad",
        "runtime_path_deferred_actor_retries_median",
        "runtime_path_deferred_actor_retries_mad",
        "door_open_souls_scanned_median",
        "door_open_souls_scanned_mad",
        "door_open_waypoints_scanned_median",
        "door_open_waypoints_scanned_mad",
        "door_close_souls_scanned_median",
        "door_close_souls_scanned_mad",
        "construction_floor_sites_considered_median",
        "construction_floor_sites_considered_mad",
        "construction_wall_sites_considered_median",
        "construction_wall_sites_considered_mad",
        "construction_floor_tiles_inspected_median",
        "construction_floor_tiles_inspected_mad",
        "construction_wall_tiles_inspected_median",
        "construction_wall_tiles_inspected_mad",
        "construction_evacuation_candidates_scanned_median",
        "construction_evacuation_candidates_scanned_mad",
        "construction_floor_phase_elapsed_micros_median",
        "construction_floor_phase_elapsed_micros_mad",
        "construction_floor_completion_elapsed_micros_median",
        "construction_floor_completion_elapsed_micros_mad",
        "construction_wall_phase_elapsed_micros_median",
        "construction_wall_phase_elapsed_micros_mad",
        "construction_wall_completion_elapsed_micros_median",
        "construction_wall_completion_elapsed_micros_mad",
        "slow_simulation_steps_median",
        "slow_simulation_steps_mad",
        "slow_simulation_souls_updated_median",
        "slow_simulation_souls_updated_mad",
        "slow_simulation_idle_decisions_median",
        "slow_simulation_idle_decisions_mad",
        "slow_simulation_idle_spatial_target_lookups_median",
        "slow_simulation_idle_spatial_target_lookups_mad",
        "slow_simulation_state_sanity_audits_median",
        "slow_simulation_state_sanity_audits_mad",
        "energy_power_output_runs_median",
        "energy_power_output_runs_mad",
        "energy_grid_recalc_runs_median",
        "energy_grid_recalc_runs_mad",
        "energy_lamp_steps_median",
        "energy_lamp_steps_mad",
        "energy_lamp_candidates_scanned_median",
        "energy_lamp_candidates_scanned_mad",
        "adapter",
    ]
    aggregate_rows: list[dict[str, str]] = []
    for case_id, validations in sorted(groups.items()):
        metric_values = {
            metric: [float(validation.summary[metric]) for validation in validations]
            for metric in ("p50_ms", "p95_ms", "p99_ms", "max_ms")
        }
        work_counter_values = {}
        for counter in (
            "task_execution_souls_queried",
            "task_execution_idle_skips",
            "task_execution_handler_runs",
            "reservation_sync_full_rebuilds",
            "reservation_sync_pending_tasks_scanned",
            "reservation_sync_assigned_tasks_scanned",
            "runtime_path_actor_new_core_searches",
            "runtime_path_actor_new_deferred",
            "runtime_path_actor_reuse_core_searches",
            "runtime_path_actor_reuse_deferred",
            "runtime_path_actor_rest_fallback_core_searches",
            "runtime_path_actor_rest_fallback_deferred",
            "runtime_path_escape_core_searches",
            "runtime_path_escape_deferred",
            "runtime_path_task_execution_core_searches",
            "runtime_path_task_execution_deferred",
            "runtime_path_bucket_transport_core_searches",
            "runtime_path_bucket_transport_deferred",
            "runtime_path_total_core_searches",
            "runtime_path_expanded_nodes",
            "runtime_path_max_expanded_nodes_per_search",
            "runtime_path_active_task_max_defer_frames",
            "runtime_path_idle_or_rest_max_defer_frames",
            "runtime_path_deferred_actor_retries",
            "door_open_souls_scanned",
            "door_open_waypoints_scanned",
            "door_close_souls_scanned",
            "construction_floor_sites_considered",
            "construction_wall_sites_considered",
            "construction_floor_tiles_inspected",
            "construction_wall_tiles_inspected",
            "construction_evacuation_candidates_scanned",
            "construction_floor_phase_elapsed_micros",
            "construction_floor_completion_elapsed_micros",
            "construction_wall_phase_elapsed_micros",
            "construction_wall_completion_elapsed_micros",
            "slow_simulation_steps",
            "slow_simulation_souls_updated",
            "slow_simulation_idle_decisions",
            "slow_simulation_idle_spatial_target_lookups",
            "slow_simulation_state_sanity_audits",
            "energy_power_output_runs",
            "energy_grid_recalc_runs",
            "energy_lamp_steps",
            "energy_lamp_candidates_scanned",
        ):
            # schema v3 以前の既存baselineはreservation counterを持たない。
            # frame-time aggregateの再集約・比較は維持し、存在しないcounterを
            # 推測で0埋めしない。
            if all(counter in validation.summary for validation in validations):
                work_counter_values[counter] = [
                    float(validation.summary[counter]) for validation in validations
                ]
        row = {
            "case_id": case_id,
            "valid_runs": str(len(validations)),
            "initial_state_checksum": ";".join(
                sorted({validation.summary["initial_state_checksum"] for validation in validations})
            ),
            "warmup_checksums": ";".join(
                sorted({validation.summary["warmup_state_checksum"] for validation in validations})
            ),
            "measure_end_checksums": ";".join(
                sorted({validation.summary["measure_end_state_checksum"] for validation in validations})
            ),
            "post_capture_teardown_warning_counts": ";".join(
                str(len(validation.teardown_warning_lines)) for validation in validations
            ),
            "adapter": json.dumps(validations[0].adapter, sort_keys=True),
        }
        for metric, values in metric_values.items():
            median, mad = median_and_mad(values)
            prefix = metric.removesuffix("_ms")
            row[f"{prefix}_median_ms"] = f"{median:.6f}"
            row[f"{prefix}_mad_ms"] = f"{mad:.6f}"
        for counter, values in work_counter_values.items():
            median, mad = median_and_mad(values)
            row[f"{counter}_median"] = f"{median:.6f}"
            row[f"{counter}_mad"] = f"{mad:.6f}"
        if work_counter_values and all(
            float(validation.summary["task_execution_souls_queried"]) > 0
            for validation in validations
        ):
            task_execution_ratios = {
                "task_execution_idle_skip_pct": [
                    100.0
                    * float(validation.summary["task_execution_idle_skips"])
                    / float(validation.summary["task_execution_souls_queried"])
                    for validation in validations
                ],
                "task_execution_handler_run_pct": [
                    100.0
                    * float(validation.summary["task_execution_handler_runs"])
                    / float(validation.summary["task_execution_souls_queried"])
                    for validation in validations
                ],
            }
            for ratio, values in task_execution_ratios.items():
                median, mad = median_and_mad(values)
                row[f"{ratio}_median"] = f"{median:.6f}"
                row[f"{ratio}_mad"] = f"{mad:.6f}"
        aggregate_rows.append(row)

    with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=aggregate_columns)
        writer.writeheader()
        writer.writerows(aggregate_rows)

    report_lines = ["# Performance run report", "", f"- Valid runs: {sum(len(rows) for rows in groups.values())}"]
    report_lines.append(f"- Invalid runs: {len(invalid_runs)}")
    report_lines.append("- Initial fixture checksum policy: `require`")
    report_lines.append(f"- Warm-up checksum policy: `{warmup_policy}`")
    report_lines.append(f"- Measure-end checksum policy: `{measure_end_policy}`")
    capture_kind = matrix.get("capture_kind", "frame-time")
    report_lines.append(f"- Capture kind: `{capture_kind}`")
    report_lines.append(
        "- Post-capture teardown warnings (recorded, not validity failures): "
        + str(sum(len(validation.teardown_warning_lines) for _, validation in runs))
    )
    report_lines.append("")
    if aggregate_rows:
        report_lines.extend(["## Aggregate", "", "| Case | Valid runs | p50 median ms | p95 median ms | p99 median ms |", "| --- | ---: | ---: | ---: | ---: |"])
        for row in aggregate_rows:
            report_lines.append(
                f"| {row['case_id']} | {row['valid_runs']} | {row['p50_median_ms']} | {row['p95_median_ms']} | {row['p99_median_ms']} |"
            )
        report_lines.append("")
    if invalid_runs:
        report_lines.extend(["## Invalid runs", ""])
        for run_dir, validation in invalid_runs:
            report_lines.append(f"- `{run_dir.relative_to(session_dir)}`: {'; '.join(validation.reasons)}")
    (session_dir / "report.md").write_text("\n".join(report_lines) + "\n", encoding="utf-8")

    manifest["actual_adapters"] = all_adapters
    manifest["status"] = "valid" if not invalid_runs else "invalid"
    write_json(manifest_path, manifest)
    return not invalid_runs


def run_suite(args: argparse.Namespace) -> int:
    sizes = parse_csv_list(args.sizes, {"small", "medium", "large"}, "sizes")
    renders = parse_csv_list(args.renders, {"cpu", "gpu"}, "renders")
    if (args.souls is None) != (args.familiars is None):
        raise ValueError("--souls and --familiars must be provided together")
    cases = [
        Case(args.workload, size, render, args.seed, args.souls, args.familiars)
        for size in sizes
        for render in renders
    ]
    if args.dry_run:
        binary = Path(args.binary or "target/profiling/bevy_app")
        for case in cases:
            print(
                f"{case.identifier}: {binary} --perf-scenario --perf-seed {case.seed} "
                f"--perf-clock {args.clock_mode} ..."
            )
        return 0

    binary = build_binary(args)
    session_dir = prepare_session(args, binary, cases)
    print(f"Artifacts: {session_dir}", flush=True)
    for case in cases:
        for index in range(1, args.preflight_runs + 1):
            run_one(
                args=args,
                binary=binary,
                session_dir=session_dir,
                case=case,
                run_number=index,
                preflight=True,
            )
        for index in range(1, args.repeat + 1):
            run_one(
                args=args,
                binary=binary,
                session_dir=session_dir,
                case=case,
                run_number=index,
                preflight=False,
            )
    return 0 if summarize_session(session_dir) else 1


def read_aggregate(path: Path) -> dict[str, dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return {row["case_id"]: row for row in csv.DictReader(handle)}


def ensure_comparison_contract(
    baseline_manifest: dict[str, Any], candidate_manifest: dict[str, Any], *, allow_case_subset: bool
) -> None:
    for key in ("actual_adapters", "requested_environment"):
        if baseline_manifest.get(key) != candidate_manifest.get(key):
            raise RuntimeError(f"cannot compare sessions with different {key}")

    baseline_instrumentation = baseline_manifest.get("binary", {}).get("instrumentation")
    candidate_instrumentation = candidate_manifest.get("binary", {}).get("instrumentation")
    if baseline_instrumentation != candidate_instrumentation:
        raise RuntimeError("cannot compare sessions with different instrumentation")

    baseline_matrix = baseline_manifest["matrix"]
    candidate_matrix = candidate_manifest["matrix"]
    if not allow_case_subset:
        if baseline_matrix != candidate_matrix:
            raise RuntimeError("cannot compare sessions with different matrix")
        return

    case_axes = {"sizes", "renders"}
    baseline_contract = {key: value for key, value in baseline_matrix.items() if key not in case_axes}
    candidate_contract = {key: value for key, value in candidate_matrix.items() if key not in case_axes}
    if baseline_contract != candidate_contract:
        raise RuntimeError("cannot compare subset sessions with different non-case matrix settings")
    for axis in case_axes:
        baseline_values = set(baseline_matrix.get(axis, []))
        candidate_values = set(candidate_matrix.get(axis, []))
        if not candidate_values <= baseline_values:
            raise RuntimeError(f"candidate {axis} is not a subset of the baseline")


def compare_sessions(args: argparse.Namespace) -> int:
    baseline = Path(args.baseline).resolve()
    candidate = Path(args.candidate).resolve()
    baseline_manifest = json.loads((baseline / "manifest.json").read_text(encoding="utf-8"))
    candidate_manifest = json.loads((candidate / "manifest.json").read_text(encoding="utf-8"))
    if (
        baseline_manifest.get("matrix", {}).get("capture_kind", "frame-time") != "frame-time"
        or candidate_manifest.get("matrix", {}).get("capture_kind", "frame-time") != "frame-time"
    ):
        raise RuntimeError("fixed-step determinism audits cannot be compared as frame-time sessions")
    ensure_comparison_contract(
        baseline_manifest,
        candidate_manifest,
        allow_case_subset=args.allow_case_subset,
    )
    baseline_rows = read_aggregate(baseline / "aggregate.csv")
    candidate_rows = read_aggregate(candidate / "aggregate.csv")
    if args.allow_case_subset:
        missing_baseline_cases = sorted(set(candidate_rows) - set(baseline_rows))
        if missing_baseline_cases:
            raise RuntimeError(
                "candidate has no baseline aggregate for: " + ", ".join(missing_baseline_cases)
            )
        common_cases = sorted(candidate_rows)
    else:
        common_cases = sorted(set(baseline_rows) & set(candidate_rows))
    if not common_cases:
        raise RuntimeError("sessions have no common valid cases")
    output = Path(args.output).resolve() if args.output else candidate / "comparison.csv"
    rows: list[dict[str, str]] = []
    regressed = False
    metric_column = f"{args.metric}_median_ms"
    for case_id in common_cases:
        base = baseline_rows[case_id]
        current = candidate_rows[case_id]
        if int(base["valid_runs"]) < args.min_runs or int(current["valid_runs"]) < args.min_runs:
            raise RuntimeError(f"{case_id} has fewer than {args.min_runs} valid runs")
        baseline_value = float(base[metric_column])
        candidate_value = float(current[metric_column])
        percent = ((candidate_value / baseline_value) - 1.0) * 100.0 if baseline_value else 0.0
        is_regression = percent > args.max_regression_pct
        regressed |= is_regression
        rows.append(
            {
                "case_id": case_id,
                "metric": args.metric,
                "baseline_ms": f"{baseline_value:.6f}",
                "candidate_ms": f"{candidate_value:.6f}",
                "delta_pct": f"{percent:.3f}",
                "regression": str(is_regression).lower(),
            }
        )
    with output.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    return 1 if regressed else 0


def write_fixture_run(
    root: Path,
    *,
    warning: bool = False,
    teardown_warning: bool = False,
    fixed_step_audit: bool = False,
) -> None:
    run_dir = root / "data"
    run_dir.mkdir(parents=True)
    if fixed_step_audit:
        timestep_ns = 15_625_000
        checkpoints = [
            ("fixture-pre-update", 0),
            *DETERMINISM_EARLY_CHECKPOINTS,
            ("post-warmup", 1920),
            ("post-audit-end", 2048),
        ]
        with (run_dir / "determinism.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=DETERMINISM_COLUMNS)
            writer.writeheader()
            for index, (checkpoint, tick) in enumerate(checkpoints):
                elapsed = tick * timestep_ns
                writer.writerow(
                    {
                        "schema_version": DETERMINISM_SCHEMA_VERSION,
                        "checkpoint": checkpoint,
                        "update_tick": str(tick),
                        "fixed_timestep_ns": str(timestep_ns),
                        "virtual_delta_ns": "0" if index == 0 else str(timestep_ns),
                        "virtual_elapsed_ns": str(elapsed),
                        "fixed_delta_ns": "0" if index == 0 else str(timestep_ns),
                        "fixed_elapsed_ns": str(elapsed),
                        "fixed_overstep_ns": "0",
                        "virtual_paused": "1" if index == 0 else "0",
                        "virtual_relative_speed_bits": ONE_F64_BITS,
                        "virtual_effective_speed_bits": ZERO_F64_BITS if index == 0 else ONE_F64_BITS,
                        "souls": "0",
                        "familiars": "0",
                        "designations": "0",
                        "state_checksum": "0000000000000000",
                    }
                )
    else:
        summary = {column: "0" for column in EXPECTED_SUMMARY_COLUMNS}
        summary.update(
            {
                "schema_version": SUMMARY_SCHEMA_VERSION,
                "seed": str(DEFAULT_SEED),
                "workload": "gather",
                "size": "small",
                "render": "cpu",
                "samples": "1",
                "p50_ms": "1.0",
                "p95_ms": "1.0",
                "p99_ms": "1.0",
                "max_ms": "1.0",
            }
        )
        with (run_dir / "summary.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=sorted(EXPECTED_SUMMARY_COLUMNS))
            writer.writeheader()
            writer.writerow(summary)
        (run_dir / "frames.csv").write_text("frame_index,frame_time_ms\n0,1.0\n", encoding="utf-8")
        with (run_dir / "scene_roots.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=SCENE_ROOT_COLUMNS)
            writer.writeheader()
            writer.writerow({column: "0" for column in SCENE_ROOT_COLUMNS})
    extra = "2026 WARN unexpected warning\n" if warning else ""
    teardown_extra = "2026 WARN teardown warning\n" if teardown_warning else ""
    (root / "run.log").write_text(
        (
            "PERF_SCENARIO: seed=20260712 workload=gather size=small souls=50 familiars=4 "
            "render=cpu clock=fixed fixed_hz=64 fixed_warmup_ticks=1920 fixed_audit_ticks=128\n"
            if fixed_step_audit
            else "PERF_SCENARIO: seed=20260712 workload=gather size=small souls=50 familiars=4 "
            "render=cpu clock=realtime\n"
        )
        + "AdapterInfo { name: \"Test GPU\", driver: \"test\", driver_info: \"test\", backend: Vulkan }\n"
        + extra
        + (
            "PERF_DETERMINISM_AUDIT: wrote 7 checkpoints to x\n"
            if fixed_step_audit
            else "PERF_CAPTURE: wrote 1 samples to x\n"
        )
        + teardown_extra,
        encoding="utf-8",
    )


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        write_fixture_run(root, teardown_warning=True)
        case = Case("gather", "small", "cpu", DEFAULT_SEED, None, None)
        validation = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert validation.valid, validation.reasons
        assert validation.teardown_warning_lines == ["2026 WARN teardown warning"]
        shutil.rmtree(root / "data")
        invalid = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not invalid.valid and any("missing summary.csv" in reason for reason in invalid.reasons)

        session = root / "session"
        run_dirs = [
            session / "cases" / case.identifier / "run-001",
            session / "cases" / case.identifier / "run-002",
        ]
        for run_dir in run_dirs:
            write_fixture_run(run_dir)
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            session / "manifest.json",
            {"matrix": {"warmup_checksum_policy": "require"}, "actual_adapters": []},
        )
        assert summarize_session(session)
        with (session / "aggregate.csv").open(newline="", encoding="utf-8") as handle:
            aggregate = list(csv.DictReader(handle))
        assert aggregate and "max_mad_ms" in aggregate[0]

        summary_path = run_dirs[1] / "data" / "summary.csv"
        with summary_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            fieldnames = reader.fieldnames
            rows = list(reader)
        assert fieldnames is not None and len(rows) == 1
        rows[0]["initial_state_checksum"] = "0000000000000001"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)
        validation = validate_run(
            run_dirs[1],
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert validation.valid, validation.reasons
        write_json(run_dirs[1] / "validation.json", validation.to_json())
        assert not summarize_session(session)
        invalidated = json.loads((run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("initial_state_checksum differs" in reason for reason in invalidated["reasons"])

        rows[0]["initial_state_checksum"] = "0"
        rows[0]["warmup_state_checksum"] = "0000000000000002"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=fieldnames)
            writer.writeheader()
            writer.writerows(rows)
        for run_dir in run_dirs:
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        assert not summarize_session(session, "require")
        invalidated = json.loads((run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("warmup_state_checksum differs" in reason for reason in invalidated["reasons"])
        assert summarize_session(session, "record")

        fixed_session = root / "fixed-session"
        fixed_run_dirs = [
            fixed_session / "cases" / case.identifier / "run-001",
            fixed_session / "cases" / case.identifier / "run-002",
        ]
        for run_dir in fixed_run_dirs:
            write_fixture_run(run_dir, fixed_step_audit=True)
            validation = validate_run(
                run_dir,
                returncode=0,
                expected_case=case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
                capture_kind="fixed-step-determinism",
                expected_fixed_hz=64,
                expected_warmup_ticks=1920,
                expected_audit_ticks=128,
            )
            assert validation.valid, validation.reasons
            write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            fixed_session / "manifest.json",
            {
                "matrix": {
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                },
                "actual_adapters": [],
            },
        )
        assert summarize_session(fixed_session)
        checkpoints_path = fixed_run_dirs[1] / "data" / "determinism.csv"
        with checkpoints_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            checkpoint_fields = reader.fieldnames
            checkpoints = list(reader)
        assert checkpoint_fields is not None and checkpoints
        checkpoints[1]["state_checksum"] = "0000000000000001"
        with checkpoints_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=checkpoint_fields)
            writer.writeheader()
            writer.writerows(checkpoints)
        validation = validate_run(
            fixed_run_dirs[1],
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
            capture_kind="fixed-step-determinism",
            expected_fixed_hz=64,
            expected_warmup_ticks=1920,
            expected_audit_ticks=128,
        )
        assert validation.valid, validation.reasons
        write_json(fixed_run_dirs[1] / "validation.json", validation.to_json())
        assert not summarize_session(fixed_session)
        invalidated = json.loads((fixed_run_dirs[0] / "validation.json").read_text(encoding="utf-8"))
        assert any("determinism checkpoints differ" in reason for reason in invalidated["reasons"])

        audit_args = build_parser().parse_args(["audit", "--dry-run"])
        assert audit_args.clock_mode == "fixed"
        assert audit_args.capture_kind == "fixed-step-determinism"
        assert audit_args.fixed_hz == 64
        assert audit_args.warmup_ticks == 1920
        assert audit_args.audit_ticks == 128

        def write_comparison_fixture(
            session_dir: Path, *, sizes: list[str], renders: list[str], case_ids: list[str]
        ) -> None:
            session_dir.mkdir()
            write_json(
                session_dir / "manifest.json",
                {
                    "matrix": {
                        "workload": "gather",
                        "sizes": sizes,
                        "renders": renders,
                        "seed": DEFAULT_SEED,
                        "repeat": 3,
                        "warmup_secs": 30.0,
                        "measure_secs": 60.0,
                        "preflight_runs": 0,
                        "souls": None,
                        "familiars": None,
                        "warmup_checksum_policy": "record",
                    },
                    "actual_adapters": [{"name": "Test GPU", "backend": "Vulkan"}],
                    "requested_environment": {"WGPU_BACKEND": "vulkan"},
                    "binary": {"instrumentation": "capture"},
                },
            )
            with (session_dir / "aggregate.csv").open("w", newline="", encoding="utf-8") as handle:
                writer = csv.DictWriter(
                    handle,
                    fieldnames=["case_id", "valid_runs", "p50_median_ms"],
                )
                writer.writeheader()
                writer.writerows(
                    {
                        "case_id": case_id,
                        "valid_runs": "3",
                        "p50_median_ms": "1.0",
                    }
                    for case_id in case_ids
                )

        comparison_baseline = root / "comparison-baseline"
        comparison_candidate = root / "comparison-candidate"
        large_cpu_case = "gather-large-cpu-seed-20260712"
        write_comparison_fixture(
            comparison_baseline,
            sizes=["medium", "large"],
            renders=["cpu", "gpu"],
            case_ids=[large_cpu_case, "gather-medium-cpu-seed-20260712"],
        )
        write_comparison_fixture(
            comparison_candidate,
            sizes=["large"],
            renders=["cpu"],
            case_ids=[large_cpu_case],
        )
        comparison_args = argparse.Namespace(
            baseline=str(comparison_baseline),
            candidate=str(comparison_candidate),
            metric="p50",
            max_regression_pct=5.0,
            min_runs=3,
            output=None,
            allow_case_subset=False,
        )
        try:
            compare_sessions(comparison_args)
        except RuntimeError as error:
            assert "different matrix" in str(error)
        else:
            raise AssertionError("subset comparison unexpectedly bypassed the matrix contract")
        comparison_args.allow_case_subset = True
        assert compare_sessions(comparison_args) == 0
    print("perf.py self-test: pass")
    return 0


def add_run_arguments(parser: argparse.ArgumentParser, *, fixed_step_audit: bool = False) -> None:
    parser.add_argument("--workload", default="gather", choices=["gather", "path-door", "construction", "ui-gpu"])
    parser.add_argument("--sizes", default="medium", help="comma-separated: small,medium,large")
    parser.add_argument("--renders", default="cpu", help="comma-separated: cpu,gpu")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--preflight-runs", type=int, default=0)
    parser.add_argument("--souls", type=int)
    parser.add_argument("--familiars", type=int)
    parser.add_argument("--output", help="new artifact directory, relative to the repository when not absolute")
    parser.add_argument("--adapter", help="required substring of the actual WGPU adapter name")
    parser.add_argument("--backend", default="auto", choices=["auto", "vulkan", "gl", "dx12", "metal"])
    parser.add_argument("--window-backend", default="auto", choices=["auto", "wayland", "x11"])
    parser.add_argument("--present-mode", default="novsync")
    parser.add_argument("--instrumentation", default="capture", choices=["capture", "tracy", "memory"])
    parser.add_argument("--binary", help="prebuilt profiling binary path")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--timeout-secs", type=float, default=600.0)
    if fixed_step_audit:
        parser.add_argument("--fixed-hz", type=int, default=64)
        parser.add_argument("--warmup-ticks", type=int, default=1920)
        parser.add_argument("--audit-ticks", type=int, default=128)
        parser.set_defaults(
            capture_kind="fixed-step-determinism",
            clock_mode="fixed",
        )
    else:
        parser.add_argument("--warmup-secs", type=float, default=30.0)
        parser.add_argument("--measure-secs", type=float, default=60.0)
        parser.add_argument("--warmup-checksum-policy", default="record", choices=["require", "record"])
        parser.add_argument(
            "--measure-end-checksum-policy",
            default="record",
            choices=["require", "record"],
        )
        parser.set_defaults(
            capture_kind="frame-time",
            clock_mode="realtime",
        )
    parser.add_argument(
        "--allow-log-pattern",
        action="append",
        default=[],
        help="regular expression for a known, explicitly allowed pre-capture warning",
    )
    parser.add_argument("--dry-run", action="store_true")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    run_parser = subparsers.add_parser("run", help="build, run, validate, and summarize a matrix")
    add_run_arguments(run_parser)
    audit_parser = subparsers.add_parser(
        "audit",
        help="run a fixed-step determinism audit with all state checkpoints required",
    )
    add_run_arguments(audit_parser, fixed_step_audit=True)
    summarize_parser = subparsers.add_parser("summarize", help="rebuild aggregate.csv and report.md")
    summarize_parser.add_argument("session")
    summarize_parser.add_argument("--warmup-checksum-policy", choices=["require", "record"])
    summarize_parser.add_argument("--measure-end-checksum-policy", choices=["require", "record"])
    compare_parser = subparsers.add_parser("compare", help="compare compatible valid sessions")
    compare_parser.add_argument("--baseline", required=True)
    compare_parser.add_argument("--candidate", required=True)
    compare_parser.add_argument("--metric", default="p50", choices=["p50", "p95", "p99", "max"])
    compare_parser.add_argument("--max-regression-pct", type=float, default=5.0)
    compare_parser.add_argument("--min-runs", type=int, default=3)
    compare_parser.add_argument("--output")
    compare_parser.add_argument(
        "--allow-case-subset",
        action="store_true",
        help="allow a candidate that measures a size/render subset of the baseline; all other settings must match",
    )
    subparsers.add_parser("self-test", help="run stdlib-only validation fixtures")
    return parser


def validate_arguments(args: argparse.Namespace) -> None:
    if args.command not in {"run", "audit"}:
        return
    if args.repeat < 1:
        raise ValueError("--repeat must be at least 1")
    if args.preflight_runs < 0:
        raise ValueError("--preflight-runs cannot be negative")
    if args.seed < 0:
        raise ValueError("--seed cannot be negative")
    if args.command == "run" and (args.warmup_secs < 0 or args.measure_secs <= 0):
        raise ValueError("--warmup-secs must be nonnegative and --measure-secs must be positive")
    if args.timeout_secs <= 0:
        raise ValueError("--timeout-secs must be positive")
    if args.command == "audit" and args.instrumentation != "capture":
        raise ValueError("fixed-step audit only supports --instrumentation capture")
    if args.command == "audit":
        if args.fixed_hz <= 0:
            raise ValueError("--fixed-hz must be positive")
        if args.warmup_ticks <= DETERMINISM_EARLY_CHECKPOINTS[-1][1]:
            raise ValueError("--warmup-ticks must be greater than 128")
        if args.audit_ticks <= 0:
            raise ValueError("--audit-ticks must be positive")


def main() -> int:
    args = build_parser().parse_args()
    try:
        validate_arguments(args)
        if args.command in {"run", "audit"}:
            return run_suite(args)
        if args.command == "summarize":
            return 0 if summarize_session(
                Path(args.session).resolve(),
                args.warmup_checksum_policy,
                args.measure_end_checksum_policy,
            ) else 1
        if args.command == "compare":
            return compare_sessions(args)
        return self_test()
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"perf.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
