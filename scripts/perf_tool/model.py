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


SCRIPT_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = SCRIPT_DIR.parent
PERF_DESCRIPTION = __doc__
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
