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
import math
import os
import platform
import re
import signal
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = SCRIPT_DIR.parent
PERF_DESCRIPTION = __doc__
SUMMARY_SCHEMA_VERSION = "11"
DETERMINISM_SCHEMA_VERSION = "4"
SESSION_MANIFEST_SCHEMA_VERSION = 2
WINDOW_SCHEMA_VERSION = "2"
RENDER_INVENTORY_SCHEMA_VERSION = "1"
INDOOR_LIGHT_FIXTURE_SCHEMA_VERSION = "1"
INDOOR_LIGHT_LAYOUT_SCHEMA_VERSION = "1"
INDOOR_LIGHT_PRESENTATION_SCHEMA_VERSION = "1"
DECONSTRUCTION_FIXTURE_SCHEMA_VERSION = "2"
DECONSTRUCTION_FIXTURE_COLUMNS = (
    "schema_version",
    "initial_completed_buildings",
    "final_completed_buildings",
    "building_type_count",
    "commit_requests",
    "committed",
    "recovery_items",
    "commit_validation_passes",
    "successful_cleanup_transactions",
    "recovery_items_spawned",
    "post_commit_updates",
    "steady_state_validation_delta",
    "successful_transaction_elapsed_ns",
)
BEHAVIOR_TIMELINE_SCHEMA_VERSION = 1
DEFAULT_SEED = 20_260_712
SCENE_ROOT_COLUMNS = (
    "soul_proxy_3d",
    "soul_mask_proxy_3d",
    "soul_shadow_proxy_3d",
    "familiar_proxy_3d",
    "building_3d_visual",
)
RENDER_INVENTORY_COLUMNS = (
    "schema_version",
    "scene_target_count",
    "mask_target_count",
    "camera_3d_rtt_count",
    "camera_2d_count",
    "layer_2d_pass_count",
    "soul_proxy_3d",
    "soul_mask_proxy_3d",
    "soul_shadow_proxy_3d",
    "familiar_proxy_3d",
)
WINDOW_COLUMNS = (
    "schema_version",
    "window_present",
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
    "resolved_window_backend",
    "adapter_name",
    "adapter_backend",
    "requested_present_mode",
    "effective_present_mode",
    "end_window_present",
    "end_logical_width",
    "end_logical_height",
    "end_physical_width",
    "end_physical_height",
    "end_scale_factor",
    "end_rtt_quality",
    "end_scene_target_width",
    "end_scene_target_height",
    "end_mask_target_width",
    "end_mask_target_height",
    "end_target_scale_factor",
    "end_resolved_window_backend",
    "end_adapter_name",
    "end_adapter_backend",
    "end_requested_present_mode",
    "end_effective_present_mode",
)
INDOOR_LIGHT_FIXTURE_COLUMNS = (
    "schema_version",
    "contract_id",
    "stage_id",
    "lane",
    "checkpoint",
    "case_id",
    "fixture_id",
    "size",
    "layout_checksum",
    "measurement_contract_sha256",
    "fixture_contract_sha256",
    "completed_floors",
    "completed_walls",
    "doors",
    "supplied_lamp_candidates",
    "unsupplied_lamp_candidates",
    "rooms",
    "room_tiles",
    "room_boundary_lookup_cells",
    "souls",
    "familiars",
    "yards",
    "operational_soul_spas",
    "generator_souls",
    "main_generation",
    "main_demand",
    "main_headroom",
    "main_supplied_count",
    "main_shed_count",
    "control_generation",
    "control_demand",
    "control_supplied_count",
    "control_shed_count",
)
INDOOR_LIGHT_LAYOUT_COLUMNS = (
    "schema_version",
    "record_kind",
    "ordinal",
    "grid_x",
    "grid_y",
    "grid_x2",
    "grid_y2",
    "state",
    "relation",
)
INDOOR_LIGHT_PRESENTATION_COLUMNS = (
    "schema_version",
    "building_kind",
    "entity_count",
    "root_sprite_count",
    "child_sprite_count",
    "owner_3d_count",
)
EXPECTED_SUMMARY_COLUMNS = {
    "schema_version",
    "seed",
    "workload",
    "size",
    "render",
    "dashboard_mode",
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
    "candidate_membership_checks",
    "policy_disabled_rejections",
    "candidate_snapshot_attempts",
    "candidate_score_attempts",
    "worker_score_attempts",
    "top_k_partition_runs",
    "top_k_retained_candidates",
    "top_k_fallback_candidates",
    "source_selector_calls",
    "source_selector_cache_build_scanned_items",
    "source_selector_candidate_scanned_items",
    "source_selector_scanned_items",
    "reachable_with_cache_calls",
    "wheelbarrow_arbitration_rebuilds",
    "wheelbarrow_request_bucket_builds",
    "wheelbarrow_bucket_items_scanned",
    "wheelbarrow_candidates_after_top_k",
    "dashboard_state_rebuilds",
    "dashboard_snapshot_rows_scanned",
    "dashboard_summary_rows_scanned",
    "dashboard_snapshot_changes",
    "dashboard_summary_changes",
    "dashboard_render_rebuilds",
    "dashboard_render_input_rows",
    "dashboard_render_visible_rows",
    "dashboard_render_group_headers",
    "dashboard_despawn_roots_requested",
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
SUMMARY_TEXT_COLUMNS = {
    "schema_version",
    "workload",
    "size",
    "render",
    "dashboard_mode",
}
SUMMARY_CHECKSUM_COLUMNS = {
    "initial_state_checksum",
    "warmup_state_checksum",
    "measure_end_state_checksum",
}
SUMMARY_FLOAT_COLUMNS = {
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
}
SUMMARY_INTEGER_COLUMNS = EXPECTED_SUMMARY_COLUMNS - (
    SUMMARY_TEXT_COLUMNS | SUMMARY_CHECKSUM_COLUMNS | SUMMARY_FLOAT_COLUMNS
)
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
    "familiar policy controlled comparison failed:",
    "dashboard mode controlled comparison failed:",
)
DETERMINISM_COLUMNS = (
    "schema_version",
    "dashboard_mode",
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
    "structural_checksum",
    "state_checksum",
    "delegation_cycles",
    "incoming_snapshot_builds",
    "delegation_familiars_processed",
    "candidate_membership_checks",
    "policy_disabled_rejections",
    "candidate_snapshot_attempts",
    "candidate_score_attempts",
    "worker_score_attempts",
    "top_k_partition_runs",
    "top_k_retained_candidates",
    "top_k_fallback_candidates",
    "source_selector_calls",
    "source_selector_cache_build_scanned_items",
    "source_selector_candidate_scanned_items",
    "source_selector_scanned_items",
    "reachable_with_cache_calls",
    "wheelbarrow_arbitration_rebuilds",
    "wheelbarrow_request_bucket_builds",
    "wheelbarrow_bucket_items_scanned",
    "wheelbarrow_candidates_after_top_k",
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
    "dashboard_state_rebuilds",
    "dashboard_snapshot_rows_scanned",
    "dashboard_summary_rows_scanned",
    "dashboard_snapshot_changes",
    "dashboard_summary_changes",
    "dashboard_render_rebuilds",
    "dashboard_render_input_rows",
    "dashboard_render_visible_rows",
    "dashboard_render_group_headers",
    "dashboard_despawn_roots_requested",
)
DETERMINISM_RECORD_COLUMNS = (
    "schema_version",
    "checkpoint",
    "update_tick",
    "actor_kind",
    "actor_key",
    "record_hex",
)
DETERMINISM_EARLY_CHECKPOINTS = (
    ("post-update-1", 1),
    ("post-update-8", 8),
    ("post-update-32", 32),
    ("post-update-128", 128),
)
ONE_F64_BITS = "3ff0000000000000"
ZERO_F64_BITS = "0000000000000000"
TRACY_DASHBOARD_ZONE_FILTER = "task_list"


@dataclass(frozen=True)
class Case:
    workload: str
    size: str
    render: str
    seed: int
    souls: int | None
    familiars: int | None
    familiar_policy: str = "baseline"
    operation_dialog: str = "hidden"
    dashboard_mode: str = "hidden"
    behavior_case: str | None = None

    @property
    def identifier(self) -> str:
        population = ""
        if self.souls is not None:
            population = f"-souls-{self.souls}-familiars-{self.familiars}"
        familiar_policy = (
            "" if self.familiar_policy == "baseline" else f"-policy-{self.familiar_policy}"
        )
        operation_dialog = (
            "" if self.operation_dialog == "hidden" else f"-dialog-{self.operation_dialog}"
        )
        dashboard_mode = (
            f"-dashboard-{self.dashboard_mode}"
            if self.workload == "task-dashboard" or self.dashboard_mode != "hidden"
            else ""
        )
        behavior_case = (
            "" if self.behavior_case is None else f"-behavior-{self.behavior_case}"
        )
        return (
            f"{self.workload}-{self.size}-{self.render}-seed-{self.seed}"
            f"{population}{familiar_policy}{operation_dialog}{dashboard_mode}{behavior_case}"
        )


@dataclass
class Validation:
    valid: bool
    reasons: list[str]
    summary: dict[str, str] | None
    adapter: dict[str, str] | None
    warning_lines: list[str]
    teardown_warning_lines: list[str]
    determinism: list[dict[str, str]] | None = None
    determinism_records: list[dict[str, str]] | None = None
    scene_roots: dict[str, str] | None = None
    render_inventory: dict[str, str] | None = None
    window: dict[str, str] | None = None
    indoor_light_fixture: dict[str, str] | None = None
    indoor_light_layout: list[dict[str, str]] | None = None
    indoor_light_presentation: list[dict[str, str]] | None = None
    deconstruction_fixture: dict[str, str] | None = None
    timeline: list[dict[str, Any]] | None = None
    behavior_save_artifact: dict[str, Any] | None = None
    profile_artifact: dict[str, Any] | None = None

    def to_json(self) -> dict[str, Any]:
        return {
            "valid": self.valid,
            "reasons": self.reasons,
            "summary": self.summary,
            "adapter": self.adapter,
            "warning_lines": self.warning_lines,
            "teardown_warning_lines": self.teardown_warning_lines,
            "determinism": self.determinism,
            "determinism_records": self.determinism_records,
            "scene_roots": self.scene_roots,
            "render_inventory": self.render_inventory,
            "window": self.window,
            "indoor_light_fixture": self.indoor_light_fixture,
            "indoor_light_layout": self.indoor_light_layout,
            "indoor_light_presentation": self.indoor_light_presentation,
            "deconstruction_fixture": self.deconstruction_fixture,
            "timeline": self.timeline,
            "behavior_save_artifact": self.behavior_save_artifact,
            "profile_artifact": self.profile_artifact,
        }


def parse_csv_list(value: str, allowed: set[str], label: str) -> list[str]:
    values = [item.strip() for item in value.split(",") if item.strip()]
    if not values:
        raise ValueError(f"{label} must not be empty")
    unknown = sorted(set(values) - allowed)
    if unknown:
        raise ValueError(f"unsupported {label}: {', '.join(unknown)}")
    return values
