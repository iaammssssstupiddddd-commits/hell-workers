from __future__ import annotations

import argparse
import csv
import json
import re
import shutil
import tempfile
from dataclasses import asdict
from pathlib import Path
from types import SimpleNamespace

from .artifacts import (
    P02_PRESENTATION_COLUMNS,
    determinism_records_checksum,
    expected_indoor_light_fixture_row,
    measurement_duration_clock,
    read_indoor_light_consumer_lifecycle,
    read_indoor_light_consumers,
    read_indoor_light_field,
    read_indoor_light_gpu,
    read_window,
    read_wall_density_sidecars,
    sha256,
    validate_run,
    write_json,
)
from .arguments import (
    DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING,
    build_parser,
    validate_arguments,
)
from .compare import compare_dashboard_modes, compare_sessions
from .execution import (
    collect_profile_artifact,
    read_native_memory,
    read_task_dashboard_cpu,
    read_tracy_zone_summary,
)
from .model import (
    DEFAULT_SEED,
    DETERMINISM_COLUMNS,
    DETERMINISM_EARLY_CHECKPOINTS,
    DETERMINISM_RECORD_COLUMNS,
    DETERMINISM_SCHEMA_VERSION,
    DREAM_UI_METRICS_COLUMNS,
    DREAM_UI_METRICS_SCHEMA_VERSION,
    EXPECTED_SUMMARY_COLUMNS,
    INDOOR_LIGHT_CONSUMER_COLUMNS,
    INDOOR_LIGHT_CPU_COLUMNS,
    INDOOR_LIGHT_FIXTURE_COLUMNS,
    INDOOR_LIGHT_LAYOUT_COLUMNS,
    INDOOR_LIGHT_PRESENTATION_COLUMNS,
    ONE_F64_BITS,
    RENDER_INVENTORY_COLUMNS,
    RENDER_INVENTORY_SCHEMA_VERSION,
    REPO_ROOT,
    SCENE_ROOT_COLUMNS,
    SESSION_MANIFEST_SCHEMA_VERSION,
    SPATIAL_QUERY_METRICS_COLUMNS,
    SPATIAL_QUERY_METRICS_CONTRACTS,
    SPATIAL_QUERY_METRICS_SCHEMA_VERSION,
    SUMMARY_SCHEMA_VERSION,
    TRANSPORT_REQUEST_CHANGE_COLUMNS,
    TRANSPORT_REQUEST_CHANGES_SCHEMA_VERSION,
    TRANSPORT_REQUEST_KIND_NAMES,
    Validation,
    WALL_DENSITY_CASES,
    WALL_DENSITY_CONTRACT_SHA256,
    WALL_DENSITY_LAYOUT_COLUMNS,
    WALL_FORMWORK_DENSITY_CONTRACT_SHA256,
    WINDOW_COLUMNS,
    WINDOW_COLUMNS_V2,
    WINDOW_HISTORICAL_SCHEMA_VERSION,
    WINDOW_SCHEMA_VERSION,
    ZERO_F64_BITS,
    Case,
)
from .policy import load_valid_runs, validate_session_artifact_set
from .rtt_light_contract import (
    EXPECTED_CONTRACT_SHA256,
    RTT_LIGHT_STAGES,
    build_fixture_audit_actor_counts,
    build_fixture_layout,
    build_fixture_ledger,
    build_fixture_presentation_rows,
    canonical_sha256,
    contract_fingerprints,
    expected_eligible_supplied_emitters,
    expected_formal_cases,
    expected_gate_result_rows,
    is_compatible_contract_predecessor,
    load_rtt_light_contract,
    projection_field_applicability,
    validate_gate_result_rows,
    validate_projection_rows,
    validate_rtt_light_contract,
    validate_stage_lane,
)
from .rtt_light_bundle import (
    _checksum_text as rtt_light_checksum_text,
    _expected_requested_environment as expected_rtt_light_requested_environment,
    _gate_observed as observe_rtt_light_gate,
    _recorded_repo_root as recorded_rtt_light_repo_root,
    _runtime_field_projection as project_rtt_light_runtime,
    _upgrade_compatible_baseline_index as upgrade_rtt_light_baseline_index,
    _validate_run_file_set as validate_rtt_light_run_file_set,
    _validate_session_matrix as validate_rtt_light_session_matrix,
    _validate_p08_cross_sidecar as validate_p08_cross_sidecar,
    _verify_case_entry as verify_rtt_light_case_entry,
    build_gate_result_rows as build_rtt_light_gate_result_rows,
    build_projection_rows as build_rtt_light_projection_rows,
    directory_digest,
    resolve_baseline_locator,
)
from .summary import summarize_session

try:
    from cargo_runtime import workspace_temp_dir
except ModuleNotFoundError:
    from scripts.cargo_runtime import workspace_temp_dir

def write_fixture_run(
    root: Path,
    *,
    warning: bool = False,
    teardown_warning: bool = False,
    fixed_step_audit: bool = False,
    familiar_policy: str = "baseline",
    operation_dialog: str = "hidden",
    dashboard_mode: str = "hidden",
    controlled_work: dict[str, str] | None = None,
    summary_overrides: dict[str, str] | None = None,
    determinism_record_payload: str = "fixture",
    workload: str = "gather",
    size: str = "small",
    seed: int = DEFAULT_SEED,
    render: str = "cpu",
    scene_roots: dict[str, str] | None = None,
) -> None:
    run_dir = root / "data"
    run_dir.mkdir(parents=True)
    window_row = {column: "" for column in WINDOW_COLUMNS_V2}
    window_row.update(
        {
            "schema_version": WINDOW_HISTORICAL_SCHEMA_VERSION,
            "window_present": "true",
            "logical_width": "1280.000000",
            "logical_height": "720.000000",
            "physical_width": "1280",
            "physical_height": "720",
            "scale_factor": "1.000000",
            "rtt_quality": "high",
            "scene_target_width": "1280",
            "scene_target_height": "720",
            "mask_target_width": "1280",
            "mask_target_height": "720",
            "target_scale_factor": "1.000000",
            "resolved_window_backend": "x11",
            "adapter_name": "Test GPU",
            "adapter_backend": "vulkan",
            "requested_present_mode": "auto_no_vsync",
            "effective_present_mode": "immediate",
            "end_window_present": "true",
            "end_logical_width": "1280.000000",
            "end_logical_height": "720.000000",
            "end_physical_width": "1280",
            "end_physical_height": "720",
            "end_scale_factor": "1.000000",
            "end_rtt_quality": "high",
            "end_scene_target_width": "1280",
            "end_scene_target_height": "720",
            "end_mask_target_width": "1280",
            "end_mask_target_height": "720",
            "end_target_scale_factor": "1.000000",
            "end_resolved_window_backend": "x11",
            "end_adapter_name": "Test GPU",
            "end_adapter_backend": "vulkan",
            "end_requested_present_mode": "auto_no_vsync",
            "end_effective_present_mode": "immediate",
        }
    )
    with (run_dir / "window.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=WINDOW_COLUMNS_V2)
        writer.writeheader()
        writer.writerow(window_row)
    if fixed_step_audit:
        record_hex = determinism_record_payload.encode("utf-8").hex()
        audit_records = [("fixture", 0, record_hex)]
        if workload == "indoor-light":
            contract = load_rtt_light_contract("rtt-light-v1")
            audit_records.extend(
                (
                    actor_kind,
                    actor_key,
                    f"{actor_kind}:{actor_key}".encode("utf-8").hex(),
                )
                for actor_kind, count in build_fixture_audit_actor_counts(
                    contract, size
                ).items()
                for actor_key in range(count)
            )
        audit_records.sort(key=lambda record: (record[0], record[1]))
        determinism_state_checksum = determinism_records_checksum(
            [bytes.fromhex(record[2]) for record in audit_records]
        )
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
                row = {field: "0" for field in DETERMINISM_COLUMNS}
                row.update(
                    {
                        "schema_version": DETERMINISM_SCHEMA_VERSION,
                        "dashboard_mode": dashboard_mode,
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
                        "structural_checksum": "0000000000000000",
                        "state_checksum": determinism_state_checksum,
                        "delegation_cycles": "0",
                        "delegation_familiars_processed": "0",
                        "candidate_membership_checks": "0",
                        "policy_disabled_rejections": "0",
                        "candidate_snapshot_attempts": "0",
                        "candidate_score_attempts": "0",
                        "worker_score_attempts": "0",
                        "source_selector_calls": "0",
                        "source_selector_scanned_items": "0",
                        "reachable_with_cache_calls": "0",
                        **({} if index == 0 else (controlled_work or {})),
                    }
                )
                writer.writerow(row)
        with (run_dir / "determinism_records.csv").open(
            "w", newline="", encoding="utf-8"
        ) as handle:
            writer = csv.DictWriter(handle, fieldnames=DETERMINISM_RECORD_COLUMNS)
            writer.writeheader()
            for checkpoint, tick in checkpoints:
                for actor_kind, actor_key, actor_record_hex in audit_records:
                    writer.writerow(
                        {
                            "schema_version": DETERMINISM_SCHEMA_VERSION,
                            "checkpoint": checkpoint,
                            "update_tick": str(tick),
                            "actor_kind": actor_kind,
                            "actor_key": str(actor_key),
                            "record_hex": actor_record_hex,
                        }
                    )
    else:
        summary = {column: "0" for column in EXPECTED_SUMMARY_COLUMNS}
        summary.update(
            {
                "schema_version": SUMMARY_SCHEMA_VERSION,
                "seed": str(seed),
                "workload": workload,
                "size": size,
                "render": render,
                "dashboard_mode": dashboard_mode,
                "samples": "1",
                "p50_ms": "1.0",
                "p95_ms": "1.0",
                "p99_ms": "1.0",
                "max_ms": "1.0",
                "initial_state_checksum": "0000000000000000",
                "warmup_state_checksum": "0000000000000000",
                "measure_end_state_checksum": "0000000000000000",
            }
        )
        summary.update(summary_overrides or {})
        with (run_dir / "summary.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=sorted(EXPECTED_SUMMARY_COLUMNS))
            writer.writeheader()
            writer.writerow(summary)
        (run_dir / "frames.csv").write_text("frame_index,frame_time_ms\n0,1.0\n", encoding="utf-8")
        with (run_dir / "scene_roots.csv").open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=SCENE_ROOT_COLUMNS)
            writer.writeheader()
            writer.writerow(
                {
                    column: (scene_roots or {}).get(column, "0")
                    for column in SCENE_ROOT_COLUMNS
                }
            )
        if workload in {"construction", "task-dashboard"}:
            with (run_dir / "transport_request_changes.csv").open(
                "w", newline="", encoding="utf-8"
            ) as handle:
                writer = csv.DictWriter(
                    handle, fieldnames=TRANSPORT_REQUEST_CHANGE_COLUMNS
                )
                writer.writeheader()
                for request_kind in TRANSPORT_REQUEST_KIND_NAMES:
                    added = int(
                        workload == "construction"
                        and request_kind == "deliver-to-floor-construction"
                    )
                    changed_existing = int(
                        (
                            workload == "construction"
                            and request_kind == "deliver-to-floor-construction"
                        )
                        or (
                            workload == "task-dashboard"
                            and request_kind == "deposit-to-stockpile"
                        )
                    )
                    producer_spawns = added
                    producer_missing_repairs = int(
                        workload == "construction"
                        and request_kind == "deliver-to-floor-construction"
                    )
                    producer_semantic_updates = changed_existing
                    producer_disable_updates = int(
                        workload == "construction"
                        and request_kind == "deliver-to-provisional-wall"
                    )
                    producer_no_op_writes = int(
                        workload == "task-dashboard"
                        and request_kind == "deposit-to-stockpile"
                    )
                    producer_steady_observations = int(
                        request_kind == "return-bucket"
                    )
                    producer_observations = sum(
                        (
                            producer_spawns,
                            producer_missing_repairs,
                            producer_semantic_updates,
                            producer_disable_updates,
                            producer_no_op_writes,
                            producer_steady_observations,
                        )
                    )
                    writer.writerow(
                        {
                            "schema_version": TRANSPORT_REQUEST_CHANGES_SCHEMA_VERSION,
                            "request_kind": request_kind,
                            "observer_runs": "4",
                            "changed_components": str(added + changed_existing),
                            "added_components": str(added),
                            "changed_existing_components": str(changed_existing),
                            "producer_observations": str(producer_observations),
                            "producer_spawns": str(producer_spawns),
                            "producer_missing_repairs": str(producer_missing_repairs),
                            "producer_semantic_updates": str(producer_semantic_updates),
                            "producer_disable_updates": str(producer_disable_updates),
                            "producer_no_op_writes": str(producer_no_op_writes),
                            "producer_steady_observations": str(
                                producer_steady_observations
                            ),
                        }
                    )
        if workload in SPATIAL_QUERY_METRICS_CONTRACTS:
            with (run_dir / "spatial_query_metrics.csv").open(
                "w", newline="", encoding="utf-8"
            ) as handle:
                writer = csv.DictWriter(
                    handle, fieldnames=SPATIAL_QUERY_METRICS_COLUMNS
                )
                writer.writeheader()
                for caller, radius_band, radius_px in SPATIAL_QUERY_METRICS_CONTRACTS[
                    workload
                ]:
                    writer.writerow(
                        {
                            "schema_version": SPATIAL_QUERY_METRICS_SCHEMA_VERSION,
                            "tag": "soul",
                            "caller": caller,
                            "radius_band": radius_band,
                            "radius_px": radius_px,
                            "queries": "1",
                            "invalid_queries": "0",
                            "coordinate_probes": "1",
                            "occupied_buckets": "1",
                            "bucket_members_examined": "1",
                            "exact_hits": "1",
                            "position_fallbacks": "0",
                        }
                    )
    if workload == "dream-ui-burst":
        with (run_dir / "dream_ui_metrics.csv").open(
            "w", newline="", encoding="utf-8"
        ) as handle:
            writer = csv.DictWriter(handle, fieldnames=DREAM_UI_METRICS_COLUMNS)
            writer.writeheader()
            writer.writerow(
                {
                    "schema_version": DREAM_UI_METRICS_SCHEMA_VERSION,
                    "workload": "dream-ui-burst",
                    "target_active_particles": "128",
                    "measured_frames": "128",
                    "active_particle_updates": "16384",
                    "merge_pair_comparisons": "1040384",
                    "node_writes": "16384",
                    "ui_transform_writes": "0",
                    "particle_spawns": "256",
                    "particle_despawns": "128",
                    "trail_spawns": "512",
                    "trail_despawns": "384",
                    "scoped_allocator_available": "false",
                    "scoped_alloc_calls": "0",
                    "scoped_alloc_bytes": "0",
                    "dream_lane_elapsed_ns": "1000000",
                    "dream_lane_p95_ns": "10000",
                    "dream_lane_sample_overflow": "0",
                    "rng_sequence_checksum": "1111111111111111",
                    "trajectory_checksum": "2222222222222222",
                    "lifetime_checksum": "3333333333333333",
                    "maximum_active_particles": "128",
                }
            )
    extra = "2026 WARN unexpected warning\n" if warning else ""
    teardown_extra = "2026 WARN teardown warning\n" if teardown_warning else ""
    (root / "run.log").write_text(
        (
            f"PERF_SCENARIO: seed={seed} workload={workload} size={size} souls=50 familiars=4 "
            f"render=cpu clock=fixed behavior_case=none familiar_policy={familiar_policy} "
            f"operation_dialog={operation_dialog} fixed_hz=64 "
            f"dashboard_mode={dashboard_mode} "
            "fixed_warmup_ticks=1920 fixed_audit_ticks=128\n"
            if fixed_step_audit
            else f"PERF_SCENARIO: seed={seed} workload={workload} size={size} souls=50 familiars=4 "
            f"render={render} clock=realtime behavior_case=none familiar_policy=baseline operation_dialog=hidden "
            f"dashboard_mode={dashboard_mode}\n"
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


def write_indoor_light_sidecars(
    root: Path, case: Case, *, lane: str = "static", stage_id: str = "current"
) -> None:
    data_dir = root / "data"
    contract = load_rtt_light_contract("rtt-light-v1")
    fixture_row = expected_indoor_light_fixture_row(
        contract,
        case,
        contract_id="rtt-light-v1",
        stage_id=stage_id,
        lane=lane,
    )
    with (data_dir / "indoor_light_fixture.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=INDOOR_LIGHT_FIXTURE_COLUMNS)
        writer.writeheader()
        writer.writerow(fixture_row)
    with (data_dir / "indoor_light_layout.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=INDOOR_LIGHT_LAYOUT_COLUMNS)
        writer.writeheader()
        writer.writerows(build_fixture_ledger(contract, case.size))
    with (data_dir / "indoor_light_presentation.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=INDOOR_LIGHT_PRESENTATION_COLUMNS)
        writer.writeheader()
        writer.writerows(
            build_fixture_presentation_rows(
                contract, case.size, stage_id=stage_id
            )
        )


def write_render_inventory_fixture(
    root: Path,
    *,
    scene_roots: dict[str, str] | None = None,
    stage_id: str = "current",
) -> None:
    values = {column: "0" for column in RENDER_INVENTORY_COLUMNS}
    values.update(
        {
            "schema_version": RENDER_INVENTORY_SCHEMA_VERSION,
            "scene_target_count": "1",
            "mask_target_count": "0" if stage_id in {"p01", "p02"} else "1",
            "camera_3d_rtt_count": "1" if stage_id in {"p01", "p02"} else "2",
            "camera_2d_count": "2" if stage_id == "p02" else ("1" if stage_id == "p01" else "3"),
            "layer_2d_pass_count": "1" if stage_id in {"p01", "p02"} else "2",
        }
    )
    for column in (
        "soul_proxy_3d",
        "soul_mask_proxy_3d",
        "soul_shadow_proxy_3d",
        "familiar_proxy_3d",
    ):
        values[column] = (scene_roots or {}).get(column, "0")
    with (root / "data" / "render_inventory.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=RENDER_INVENTORY_COLUMNS)
        writer.writeheader()
        writer.writerow(values)


def write_p02_presentation_fixture(root: Path, *, souls: int = 50) -> None:
    values = {
        "schema_version": "1",
        "layer_2d_camera_count": "1",
        "layer_2d_pass_count": "1",
        "building_count": "187",
        "duplicate_presentation_count": "0",
        "building_exactly_one_presentation": "true",
        "soul_count": str(souls),
        "soul_billboard_count": str(souls),
        "familiar_3d_count": "0",
        "state_and_bounce_probes_pass": "true",
    }
    with (root / "data" / "p02_presentation.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=P02_PRESENTATION_COLUMNS)
        writer.writeheader()
        writer.writerow(values)


def write_behavior_fixture_run(
    root: Path, case: Case, *, stage_id: str = "current"
) -> None:
    if case.behavior_case is None:
        raise ValueError("behavior fixture requires a behavior case")
    data_dir = root / "data"
    data_dir.mkdir(parents=True)
    window_row = {column: "" for column in WINDOW_COLUMNS_V2}
    window_row.update(
        {
            "schema_version": WINDOW_HISTORICAL_SCHEMA_VERSION,
            "window_present": "false",
            "rtt_quality": "high",
            "scene_target_width": "1280",
            "scene_target_height": "720",
            "mask_target_width": "1280",
            "mask_target_height": "720",
            "target_scale_factor": "1.000000",
            "end_window_present": "false",
            "end_rtt_quality": "high",
            "end_scene_target_width": "1280",
            "end_scene_target_height": "720",
            "end_mask_target_width": "1280",
            "end_mask_target_height": "720",
            "end_target_scale_factor": "1.000000",
        }
    )
    with (data_dir / "window.csv").open(
        "w", newline="", encoding="utf-8"
    ) as handle:
        writer = csv.DictWriter(handle, fieldnames=WINDOW_COLUMNS_V2)
        writer.writeheader()
        writer.writerow(window_row)

    contract = load_rtt_light_contract("rtt-light-v1")
    columns = contract["behavior_fixture"]["timeline"]["columns"]
    fixture_checksum = build_fixture_layout(contract, "small")["layout_checksum"]
    case_contract = contract["behavior_fixture"][case.behavior_case.replace("-", "_")]
    rows: list[dict[str, Any]] = []
    door_ticks = [0, 1, 2, 2, 2]
    load_applied = [False, False, True, False, True, True]
    load_attempted = [False, True, False, True, False, False]
    for index, step in enumerate(case_contract["steps"]):
        row = {column: None for column in columns}
        row.update(
            {
                "case_id": case.behavior_case,
                "step_index": index,
                "script_update": step.get("script_update", index),
                "simulation_tick": (
                    door_ticks[index]
                    if case.behavior_case == "door-state-v1"
                    else index
                ),
                "pause_state": (
                    step["pause_state"]
                    if case.behavior_case == "door-state-v1"
                    else "running"
                ),
                "world_epoch": (
                    0
                    if case.behavior_case == "door-state-v1" or index < 4
                    else 1
                ),
                "intent": step["intent"],
                "attempted": (
                    step["attempted"]
                    if case.behavior_case == "door-state-v1"
                    else load_attempted[index]
                ),
                "applied": (
                    step[
                        "p02_applied"
                        if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}
                        else "current_applied"
                    ]
                    if case.behavior_case == "door-state-v1"
                    else load_applied[index]
                ),
                "semantic_state": (
                    step[
                        "p02_semantic_state"
                        if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}
                        else "current_semantic_state"
                    ]
                    if case.behavior_case == "door-state-v1"
                    else None
                ),
                "active_presentation_state": (
                    step[
                        "p02_active_presentation_state"
                        if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}
                        else "current_active_presentation_state"
                    ]
                    if case.behavior_case == "door-state-v1"
                    else None
                ),
                "registry_phase": "stage_before_registry_owner",
                "field_availability": "stage_before_field_owner",
                "gpu_availability": "stage_before_gpu_owner",
                "fixture_checksum": fixture_checksum,
                "terminal_outcome": (
                    "succeeded"
                    if case.behavior_case == "door-state-v1"
                    and index == len(case_contract["steps"]) - 1
                    else step.get("terminal_outcome", "in_progress")
                ),
            }
        )
        if stage_id in {"p04", "p05", "p06"}:
            row.update(
                {
                    "field_availability": "available",
                    "field_input_revision": index + 1,
                    "field_output_revision": index + 1,
                    "field_is_dark": False,
                    "field_checksum": "4" * 64,
                }
            )
        if stage_id in {"p05", "p06"}:
            row.update(
                {
                    "registry_phase": "candidate_preflight",
                    "wake_count": 0,
                    "field_read_count": 0,
                    "old_epoch_field_read_count": 0,
                }
            )
        if stage_id == "p06":
            row["gpu_availability"] = "unavailable"
        rows.append(row)
    (data_dir / "timeline.json").write_text(
        json.dumps(
            {"schema_version": 1, "complete": True, "rows": rows},
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    if case.behavior_case == "load-normal-v1":
        (data_dir / "behavior-save.scn.ron").write_text(
            (
                "HELL_WORKERS_SAVE\n"
                f"(format_version: 1, worldgen_seed: {case.seed})\n"
                "---\nfixture behavior save\n"
            ),
            encoding="utf-8",
        )
    write_indoor_light_sidecars(root, case, stage_id=stage_id, lane="behavior")
    if stage_id in {"p04", "p05", "p06"}:
        write_json(
            data_dir / "indoor_light_runtime.json",
            {
                "schema_version": 1,
                "availability": "available",
                "typed_emitter_components": 2,
                "eligible_supplied_emitters": expected_eligible_supplied_emitters(
                    contract,
                    stage_id,
                    case.size,
                    behavior_case=case.behavior_case,
                ),
                "unsupplied_snapshot_adoptions": 0,
                "indoor_mask_cells": 36,
                "indoor_mask_checksum": contract["fixture"]["sizes"]["small"]["indoor_mask_checksum"],
                "input_revision": 5,
                "output_revision": 5,
                "field_checksum": "4" * 64,
                "steady_updates": 0,
                "steady_full_scans": 0,
                "steady_field_rebuilds": 0,
                "steady_revision_increments": 0,
                "steady_scoped_allocation_events": 0,
                "steady_scoped_allocation_bytes": 0,
                "max_rebuilds_per_update": 1,
                "emitter_collect_allocation": None,
            },
        )
    (root / "run.log").write_text(
        (
            f"PERF_SCENARIO: seed={case.seed} workload=indoor-light size=small "
            "souls=50 familiars=4 render=cpu clock=fixed-behavior "
            f"behavior_case={case.behavior_case} familiar_policy=baseline "
            "operation_dialog=hidden dashboard_mode=hidden fixed_hz=64\n"
            "AdapterInfo { name: \"Test GPU\", driver: \"test\", "
            "driver_info: \"test\", backend: Vulkan }\n"
            f"PERF_BEHAVIOR: case={case.behavior_case} fixture={fixture_checksum} started\n"
            f"PERF_BEHAVIOR: wrote {len(rows)} timeline rows\n"
        ),
        encoding="utf-8",
    )
