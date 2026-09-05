from __future__ import annotations

import csv
import hashlib
import json
import math
import re
from pathlib import Path
from typing import Any, Iterable

from .model import (
    ADAPTER_RE,
    DECONSTRUCTION_FIXTURE_COLUMNS,
    DECONSTRUCTION_FIXTURE_SCHEMA_VERSION,
    DETERMINISM_COLUMNS,
    DETERMINISM_EARLY_CHECKPOINTS,
    DETERMINISM_RECORD_COLUMNS,
    DETERMINISM_SCHEMA_VERSION,
    DREAM_UI_METRICS_COLUMNS,
    DREAM_UI_METRICS_SCHEMA_VERSION,
    EXPECTED_SUMMARY_COLUMNS,
    INDOOR_LIGHT_CONSUMER_COLUMNS,
    INDOOR_LIGHT_CONSUMER_PROOF_SCHEMA_VERSION,
    INDOOR_LIGHT_CPU_COLUMNS,
    INDOOR_LIGHT_FIELD_SCHEMA_VERSION,
    INDOOR_LIGHT_FIXTURE_COLUMNS,
    INDOOR_LIGHT_FIXTURE_SCHEMA_VERSION,
    INDOOR_LIGHT_LAYOUT_COLUMNS,
    INDOOR_LIGHT_PRESENTATION_COLUMNS,
    LOG_LEVEL_RE,
    ONE_F64_BITS,
    RENDER_INVENTORY_COLUMNS,
    RENDER_INVENTORY_SCHEMA_VERSION,
    REPO_ROOT,
    SCENE_ROOT_COLUMNS,
    SPATIAL_QUERY_METRICS_COLUMNS,
    SPATIAL_QUERY_METRICS_CONTRACTS,
    SPATIAL_QUERY_METRICS_SCHEMA_VERSION,
    SUMMARY_CHECKSUM_COLUMNS,
    SUMMARY_FLOAT_COLUMNS,
    SUMMARY_INTEGER_COLUMNS,
    SUMMARY_SCHEMA_VERSION,
    TRANSPORT_REQUEST_CHANGE_COLUMNS,
    TRANSPORT_REQUEST_CHANGES_SCHEMA_VERSION,
    TRANSPORT_REQUEST_KIND_NAMES,
    Validation,
    WALL_DENSITY_CASES,
    WALL_DENSITY_CONTRACT_SHA256,
    WALL_DENSITY_LAYOUT_COLUMNS,
    WINDOW_COLUMNS,
    WINDOW_COLUMNS_V2,
    WINDOW_HISTORICAL_SCHEMA_VERSION,
    WINDOW_SCHEMA_VERSION,
    ZERO_F64_BITS,
)
from .artifact_io import (
    compare_exact_rows,
    determinism_records_checksum,
    read_exact_csv_rows,
    reject_duplicate_json_keys,
    sha256,
    write_json,
)
from .artifact_readers import (
    expected_indoor_light_fixture_row,
    read_deconstruction_fixture,
    read_indoor_light_consumer_lifecycle,
    read_indoor_light_consumers,
    read_indoor_light_field,
    read_indoor_light_gpu,
    read_indoor_light_runtime,
    read_indoor_light_sidecars,
    read_save_transaction,
    read_wall_density_sidecars,
)

from .rtt_light_contract import (
    build_fixture_audit_actor_counts,
    build_fixture_layout,
    build_fixture_ledger,
    build_fixture_presentation_rows,
    contract_fingerprints,
    expected_eligible_supplied_emitters,
    load_rtt_light_contract,
    validate_stage_lane,
)

P02_PRESENTATION_COLUMNS = (
    "schema_version",
    "layer_2d_camera_count",
    "layer_2d_pass_count",
    "building_count",
    "duplicate_presentation_count",
    "building_exactly_one_presentation",
    "soul_count",
    "soul_billboard_count",
    "familiar_3d_count",
    "state_and_bounce_probes_pass",
)









def read_summary(path: Path) -> tuple[dict[str, str] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
            headers = set(fieldnames)
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse summary.csv: {error}"]
    missing = sorted(EXPECTED_SUMMARY_COLUMNS - headers)
    if missing:
        errors.append("summary.csv missing columns: " + ", ".join(missing))
    unexpected = sorted(headers - EXPECTED_SUMMARY_COLUMNS)
    if unexpected:
        errors.append("summary.csv has unexpected columns: " + ", ".join(unexpected))
    if len(fieldnames) != len(headers):
        errors.append("summary.csv has duplicate columns")
    if len(rows) != 1:
        errors.append(f"summary.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    row = rows[0]
    if None in row:
        errors.append("summary.csv row has more values than columns")
    for column in sorted(SUMMARY_INTEGER_COLUMNS):
        try:
            if int(row[column]) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"summary.csv {column} must be a nonnegative integer")
    for column in sorted(SUMMARY_FLOAT_COLUMNS):
        try:
            value = float(row[column])
            if not math.isfinite(value) or value < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"summary.csv {column} must be a finite nonnegative number")
    for column in sorted(SUMMARY_CHECKSUM_COLUMNS):
        if not re.fullmatch(r"[0-9a-f]{16}", row.get(column, "")):
            errors.append(f"summary.csv {column} must be a 16-digit lowercase hex checksum")
    return row, errors


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


def read_render_inventory(
    path: Path,
) -> tuple[dict[str, str] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=RENDER_INVENTORY_COLUMNS,
        artifact_name="render_inventory.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != 1:
        return None, [
            *errors,
            f"render_inventory.csv must contain exactly one data row; got {len(rows)}",
        ]
    row = rows[0]
    if row.get("schema_version") != RENDER_INVENTORY_SCHEMA_VERSION:
        errors.append(
            "render_inventory.csv schema_version is "
            f"{row.get('schema_version')!r}, expected {RENDER_INVENTORY_SCHEMA_VERSION}"
        )
    parsed: dict[str, int] = {}
    for column in RENDER_INVENTORY_COLUMNS[1:]:
        value = row.get(column, "")
        try:
            parsed_value = int(value)
            if parsed_value < 0 or str(parsed_value) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"render_inventory.csv {column} must be a canonical nonnegative integer"
            )
            continue
        parsed[column] = parsed_value
    if len(parsed) == len(RENDER_INVENTORY_COLUMNS) - 1:
        if parsed["scene_target_count"] != 1:
            errors.append("render_inventory.csv must observe exactly one Scene target")
        if parsed["camera_3d_rtt_count"] != (
            parsed["scene_target_count"] + parsed["mask_target_count"]
        ):
            errors.append(
                "render_inventory.csv camera_3d_rtt_count differs from Scene + mask targets"
            )
        if parsed["layer_2d_pass_count"] > parsed["camera_2d_count"]:
            errors.append(
                "render_inventory.csv layer_2d_pass_count exceeds camera_2d_count"
            )
    return (row if not errors else None), errors




def read_transport_request_changes(
    path: Path,
) -> tuple[list[dict[str, str]] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=TRANSPORT_REQUEST_CHANGE_COLUMNS,
        artifact_name="transport_request_changes.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != len(TRANSPORT_REQUEST_KIND_NAMES):
        errors.append(
            "transport_request_changes.csv must contain exactly "
            f"{len(TRANSPORT_REQUEST_KIND_NAMES)} data rows; got {len(rows)}"
        )
    observer_runs: int | None = None
    for index, (row, expected_kind) in enumerate(
        zip(rows, TRANSPORT_REQUEST_KIND_NAMES)
    ):
        if row.get("schema_version") != TRANSPORT_REQUEST_CHANGES_SCHEMA_VERSION:
            errors.append(
                "transport_request_changes.csv row "
                f"{index} schema_version is {row.get('schema_version')!r}, "
                f"expected {TRANSPORT_REQUEST_CHANGES_SCHEMA_VERSION!r}"
            )
        if row.get("request_kind") != expected_kind:
            errors.append(
                "transport_request_changes.csv row "
                f"{index} request_kind is {row.get('request_kind')!r}, "
                f"expected {expected_kind!r}"
            )
        parsed: dict[str, int] = {}
        for column in TRANSPORT_REQUEST_CHANGE_COLUMNS[2:]:
            value = row.get(column, "")
            try:
                parsed_value = int(value)
                if parsed_value < 0 or str(parsed_value) != value:
                    raise ValueError
            except (TypeError, ValueError):
                errors.append(
                    f"transport_request_changes.csv row {index} {column} "
                    "must be a canonical nonnegative integer"
                )
                continue
            parsed[column] = parsed_value
        current_runs = parsed.get("observer_runs")
        if current_runs is not None:
            if observer_runs is None:
                observer_runs = current_runs
            elif current_runs != observer_runs:
                errors.append(
                    "transport_request_changes.csv observer_runs differs between rows"
                )
        if all(
            column in parsed
            for column in (
                "changed_components",
                "added_components",
                "changed_existing_components",
            )
        ) and parsed["changed_components"] != (
            parsed["added_components"] + parsed["changed_existing_components"]
        ):
            errors.append(
                "transport_request_changes.csv row "
                f"{index} changed_components differs from added + changed-existing"
            )
        producer_parts = (
            "producer_spawns",
            "producer_missing_repairs",
            "producer_semantic_updates",
            "producer_disable_updates",
            "producer_no_op_writes",
            "producer_steady_observations",
        )
        if all(
            column in parsed for column in ("producer_observations", *producer_parts)
        ) and parsed["producer_observations"] != sum(
            parsed[column] for column in producer_parts
        ):
            errors.append(
                "transport_request_changes.csv row "
                f"{index} producer_observations differs from producer outcome sum"
            )
    if observer_runs is not None and observer_runs == 0:
        errors.append("transport_request_changes.csv observer_runs must be greater than zero")
    return (rows if not errors else None), errors


def read_spatial_query_metrics(
    path: Path,
    *,
    workload: str,
) -> tuple[list[dict[str, str]] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=SPATIAL_QUERY_METRICS_COLUMNS,
        artifact_name="spatial_query_metrics.csv",
    )
    if rows is None:
        return None, errors
    expected_rows = SPATIAL_QUERY_METRICS_CONTRACTS.get(workload, ())
    if len(rows) != len(expected_rows):
        errors.append(
            "spatial_query_metrics.csv must contain exactly "
            f"{len(expected_rows)} rows for {workload}; got {len(rows)}"
        )
    integer_columns = SPATIAL_QUERY_METRICS_COLUMNS[4:]
    for index, (row, expected) in enumerate(
        zip(rows, expected_rows)
    ):
        expected_caller, expected_band, expected_radius = expected
        if row.get("schema_version") != SPATIAL_QUERY_METRICS_SCHEMA_VERSION:
            errors.append(f"spatial_query_metrics.csv row {index} schema_version differs")
        if row.get("tag") != "soul":
            errors.append(f"spatial_query_metrics.csv row {index} tag differs")
        if row.get("caller") != expected_caller:
            errors.append(f"spatial_query_metrics.csv row {index} caller differs")
        if row.get("radius_band") != expected_band:
            errors.append(f"spatial_query_metrics.csv row {index} radius_band differs")
        parsed: dict[str, int] = {}
        for column in integer_columns:
            value = row.get(column, "")
            try:
                parsed_value = int(value)
                if parsed_value < 0 or str(parsed_value) != value:
                    raise ValueError
            except (TypeError, ValueError):
                errors.append(
                    f"spatial_query_metrics.csv row {index} {column} "
                    "must be a canonical nonnegative integer"
                )
                continue
            parsed[column] = parsed_value
        if row.get("radius_px") != expected_radius:
            errors.append(f"spatial_query_metrics.csv row {index} radius_px differs")
        if parsed.get("queries") == 0:
            errors.append(f"spatial_query_metrics.csv row {index} queries must be positive")
        if parsed.get("invalid_queries") != 0:
            errors.append(f"spatial_query_metrics.csv row {index} has invalid queries")
        if parsed.get("occupied_buckets", 0) > parsed.get("coordinate_probes", 0):
            errors.append(
                f"spatial_query_metrics.csv row {index} occupied buckets exceed probes"
            )
        if parsed.get("exact_hits", 0) > parsed.get("bucket_members_examined", 0):
            errors.append(f"spatial_query_metrics.csv row {index} hits exceed members")
        if parsed.get("position_fallbacks", 0) > parsed.get(
            "bucket_members_examined", 0
        ):
            errors.append(
                f"spatial_query_metrics.csv row {index} fallbacks exceed members"
            )
    return (rows if not errors else None), errors


def read_dream_ui_metrics(
    path: Path,
) -> tuple[dict[str, str] | None, list[str]]:
    rows, errors = read_exact_csv_rows(
        path,
        columns=DREAM_UI_METRICS_COLUMNS,
        artifact_name="dream_ui_metrics.csv",
    )
    if rows is None:
        return None, errors
    if len(rows) != 1:
        return None, [
            *errors,
            f"dream_ui_metrics.csv must contain exactly one data row; got {len(rows)}",
        ]
    row = rows[0]
    if row.get("schema_version") != DREAM_UI_METRICS_SCHEMA_VERSION:
        errors.append("dream_ui_metrics.csv schema_version differs")
    if row.get("workload") != "dream-ui-burst":
        errors.append("dream_ui_metrics.csv workload differs")
    if row.get("scoped_allocator_available") not in {"true", "false"}:
        errors.append("dream_ui_metrics.csv scoped_allocator_available must be boolean")
    integer_columns = DREAM_UI_METRICS_COLUMNS[2:12] + DREAM_UI_METRICS_COLUMNS[13:18] + (
        "maximum_active_particles",
    )
    parsed: dict[str, int] = {}
    for column in integer_columns:
        value = row.get(column, "")
        try:
            parsed_value = int(value)
            if parsed_value < 0 or str(parsed_value) != value:
                raise ValueError
        except (TypeError, ValueError):
            errors.append(
                f"dream_ui_metrics.csv {column} must be a canonical nonnegative integer"
            )
            continue
        parsed[column] = parsed_value
    for column in DREAM_UI_METRICS_COLUMNS[18:21]:
        if not re.fullmatch(r"[0-9a-f]{16}", row.get(column, "")):
            errors.append(
                f"dream_ui_metrics.csv {column} must be a 16-digit lowercase hex checksum"
            )
    if parsed.get("target_active_particles") != 128:
        errors.append("dream_ui_metrics.csv target_active_particles must be 128")
    if parsed.get("maximum_active_particles") != 128:
        errors.append("dream_ui_metrics.csv maximum_active_particles must be 128")
    for column in (
        "measured_frames",
        "active_particle_updates",
        "merge_pair_comparisons",
        "dream_lane_elapsed_ns",
        "dream_lane_p95_ns",
    ):
        if parsed.get(column, 0) <= 0:
            errors.append(f"dream_ui_metrics.csv {column} must be positive")
    if parsed.get("node_writes") != parsed.get("active_particle_updates"):
        errors.append("dream_ui_metrics.csv node_writes must equal active_particle_updates")
    if row.get("scoped_allocator_available") == "false" and (
        parsed.get("scoped_alloc_calls") != 0 or parsed.get("scoped_alloc_bytes") != 0
    ):
        errors.append("dream_ui_metrics.csv unavailable scoped allocator counters must be zero")
    if row.get("scoped_allocator_available") == "true" and (
        parsed.get("scoped_alloc_calls", 0) <= 0
        or parsed.get("scoped_alloc_bytes", 0) <= 0
    ):
        errors.append("dream_ui_metrics.csv available scoped allocator counters must be positive")
    if parsed.get("dream_lane_sample_overflow") != 0:
        errors.append("dream_ui_metrics.csv dream_lane_sample_overflow must be zero")
    return (row if not errors else None), errors








SAVE_TRANSACTION_SCHEMA_VERSION = "4"
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






def read_window(
    path: Path,
    *,
    expect_headless: bool,
    expected_width: int | None,
    expected_height: int | None,
    expected_scale_factor: float | None,
    expected_rtt_quality: str | None,
    expected_window_backend: str,
    expected_backend: str | None,
    expected_present_mode: str,
) -> tuple[dict[str, str] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse window.csv: {error}"]

    errors: list[str] = []
    schema_version = rows[0].get("schema_version") if len(rows) == 1 else None
    expected_columns = (
        WINDOW_COLUMNS_V2
        if schema_version == WINDOW_HISTORICAL_SCHEMA_VERSION
        else WINDOW_COLUMNS
    )
    if fieldnames != list(expected_columns):
        errors.append("window.csv columns differ from schema: " + ", ".join(fieldnames))
    if len(rows) != 1:
        errors.append(f"window.csv must contain exactly one data row; got {len(rows)}")
        return None, errors
    row = rows[0]
    if None in row:
        errors.append("window.csv row has more values than columns")
    if schema_version not in {WINDOW_HISTORICAL_SCHEMA_VERSION, WINDOW_SCHEMA_VERSION}:
        errors.append(
            f"window.csv schema_version is {row.get('schema_version')!r}, "
            f"expected {WINDOW_HISTORICAL_SCHEMA_VERSION!r} or {WINDOW_SCHEMA_VERSION!r}"
        )

    paired_fields = (
        ("window_present", "end_window_present"),
        ("logical_width", "end_logical_width"),
        ("logical_height", "end_logical_height"),
        ("physical_width", "end_physical_width"),
        ("physical_height", "end_physical_height"),
        ("scale_factor", "end_scale_factor"),
        ("rtt_quality", "end_rtt_quality"),
        ("scene_target_width", "end_scene_target_width"),
        ("scene_target_height", "end_scene_target_height"),
    )
    if schema_version == WINDOW_SCHEMA_VERSION:
        paired_fields += (("mask_target_present", "end_mask_target_present"),)
    paired_fields += (
        ("mask_target_width", "end_mask_target_width"),
        ("mask_target_height", "end_mask_target_height"),
        ("target_scale_factor", "end_target_scale_factor"),
        ("resolved_window_backend", "end_resolved_window_backend"),
        ("adapter_name", "end_adapter_name"),
        ("adapter_backend", "end_adapter_backend"),
        ("requested_present_mode", "end_requested_present_mode"),
        ("effective_present_mode", "end_effective_present_mode"),
    )
    for initial_field, final_field in paired_fields:
        if row.get(initial_field) != row.get(final_field):
            errors.append(
                f"window.csv changed {initial_field} during capture: "
                f"{row.get(initial_field)!r} -> {row.get(final_field)!r}"
            )

    present_text = row.get("window_present", "")
    if present_text not in {"true", "false"}:
        errors.append("window.csv window_present must be true or false")
    present = present_text == "true"
    expected_present = not expect_headless
    if present != expected_present:
        errors.append(
            f"window.csv window_present is {present_text!r}, expected {str(expected_present).lower()!r}"
        )

    render_environment_fields = (
        "resolved_window_backend",
        "adapter_name",
        "adapter_backend",
        "requested_present_mode",
        "effective_present_mode",
    )
    if present:
        for field in render_environment_fields:
            if not row.get(field):
                errors.append(f"window.csv {field} is required with a primary window")
        resolved_window_backend = row.get("resolved_window_backend", "")
        if resolved_window_backend not in {"x11", "wayland"}:
            errors.append("window.csv resolved_window_backend must be x11 or wayland")
        elif expected_window_backend not in {"auto", resolved_window_backend}:
            errors.append(
                "window.csv resolved_window_backend is "
                f"{resolved_window_backend!r}, expected {expected_window_backend!r}"
            )
        adapter_backend = row.get("adapter_backend", "")
        if expected_backend not in {None, "auto"} and adapter_backend != expected_backend:
            errors.append(
                f"window.csv adapter_backend is {adapter_backend!r}, "
                f"expected {expected_backend!r}"
            )
        requested_present = row.get("requested_present_mode", "")
        expected_requested_present = {
            "novsync": "auto_no_vsync",
            "auto_vsync": "auto_vsync",
            "fifo": "fifo",
            "mailbox": "mailbox",
            "immediate": "immediate",
        }.get(expected_present_mode)
        if requested_present != expected_requested_present:
            errors.append(
                f"window.csv requested_present_mode is {requested_present!r}, "
                f"expected {expected_requested_present!r}"
            )
        effective_present = row.get("effective_present_mode", "")
        if effective_present not in {"fifo", "fifo_relaxed", "mailbox", "immediate"}:
            errors.append(
                "window.csv effective_present_mode must be a concrete present mode"
            )
        allowed_effective = {
            "auto_no_vsync": {"immediate", "mailbox", "fifo"},
            "auto_vsync": {"fifo_relaxed", "fifo"},
            "fifo": {"fifo"},
            "mailbox": {"mailbox", "immediate", "fifo"},
            "immediate": {"immediate", "fifo"},
        }.get(requested_present, set())
        if effective_present not in allowed_effective:
            errors.append(
                "window.csv effective_present_mode is not a valid Bevy 0.19 fallback"
            )
    else:
        for field in render_environment_fields:
            if row.get(field, "") != "":
                errors.append(f"window.csv {field} must be empty without a primary window")

    quality = row.get("rtt_quality", "")
    if quality not in {"high", "medium", "low"}:
        errors.append("window.csv rtt_quality must be high, medium, or low")
    effective_quality = expected_rtt_quality or "high"
    if quality != effective_quality:
        errors.append(
            f"window.csv rtt_quality is {quality!r}, expected {effective_quality!r}"
        )
    quality_scale = {"high": 1.0, "medium": 0.75, "low": 0.5}.get(quality)

    parsed_window: dict[str, float | int] = {}
    window_float_fields = ("logical_width", "logical_height", "scale_factor")
    window_integer_fields = ("physical_width", "physical_height")
    if present:
        for field in window_float_fields:
            try:
                value = float(row[field])
                if not math.isfinite(value) or value <= 0:
                    raise ValueError
                parsed_window[field] = value
            except (KeyError, TypeError, ValueError):
                errors.append(f"window.csv {field} must be a finite positive number")
        for field in window_integer_fields:
            try:
                value = int(row[field])
                if value <= 0:
                    raise ValueError
                parsed_window[field] = value
            except (KeyError, TypeError, ValueError):
                errors.append(f"window.csv {field} must be a positive integer")
    else:
        for field in (*window_float_fields, *window_integer_fields):
            if row.get(field, "") != "":
                errors.append(f"window.csv {field} must be empty without a primary window")

    mask_present = schema_version == WINDOW_HISTORICAL_SCHEMA_VERSION
    if schema_version == WINDOW_SCHEMA_VERSION:
        mask_present_text = row.get("mask_target_present", "")
        if mask_present_text not in {"true", "false"}:
            errors.append("window.csv mask_target_present must be true or false")
        mask_present = mask_present_text == "true"

    target_values: dict[str, float | int] = {}
    for field in ("scene_target_width", "scene_target_height"):
        try:
            value = int(row[field])
            if value <= 0:
                raise ValueError
            target_values[field] = value
        except (KeyError, TypeError, ValueError):
            errors.append(f"window.csv {field} must be a positive integer")
    for field in ("mask_target_width", "mask_target_height"):
        try:
            value = int(row[field])
            if (mask_present and value <= 0) or (not mask_present and value != 0):
                raise ValueError
            target_values[field] = value
        except (KeyError, TypeError, ValueError):
            expectation = "positive" if mask_present else "zero"
            errors.append(f"window.csv {field} must be {expectation}")
    try:
        target_factor = float(row["target_scale_factor"])
        if not math.isfinite(target_factor) or target_factor <= 0:
            raise ValueError
        target_values["target_scale_factor"] = target_factor
    except (KeyError, TypeError, ValueError):
        errors.append("window.csv target_scale_factor must be a finite positive number")

    if mask_present and target_values.get("scene_target_width") != target_values.get("mask_target_width"):
        errors.append("window.csv scene and mask target widths differ")
    if mask_present and target_values.get("scene_target_height") != target_values.get("mask_target_height"):
        errors.append("window.csv scene and mask target heights differ")

    if present:
        if {
            "logical_width",
            "logical_height",
            "physical_width",
            "physical_height",
            "scale_factor",
        } <= parsed_window.keys():
            scale_factor = float(parsed_window["scale_factor"])
            expected_logical_width = float(parsed_window["physical_width"]) / scale_factor
            expected_logical_height = float(parsed_window["physical_height"]) / scale_factor
            if not math.isclose(
                float(parsed_window["logical_width"]),
                expected_logical_width,
                rel_tol=0.0,
                abs_tol=1e-3,
            ):
                errors.append(
                    "window.csv logical_width does not match physical_width / scale_factor"
                )
            if not math.isclose(
                float(parsed_window["logical_height"]),
                expected_logical_height,
                rel_tol=0.0,
                abs_tol=1e-3,
            ):
                errors.append(
                    "window.csv logical_height does not match physical_height / scale_factor"
                )
        if expected_width is not None and parsed_window.get("physical_width") != expected_width:
            errors.append(
                f"window.csv physical_width is {parsed_window.get('physical_width')!r}, "
                f"expected {expected_width!r}"
            )
        if expected_height is not None and parsed_window.get("physical_height") != expected_height:
            errors.append(
                f"window.csv physical_height is {parsed_window.get('physical_height')!r}, "
                f"expected {expected_height!r}"
            )
        if expected_scale_factor is not None and not math.isclose(
            float(parsed_window.get("scale_factor", math.nan)),
            expected_scale_factor,
            rel_tol=0.0,
            abs_tol=1e-5,
        ):
            errors.append(
                f"window.csv scale_factor is {parsed_window.get('scale_factor')!r}, "
                f"expected {expected_scale_factor!r}"
            )
        if quality_scale is not None and {
            "physical_width",
            "physical_height",
            "scale_factor",
        } <= parsed_window.keys():
            expected_target_width = max(
                1,
                math.floor(float(parsed_window["physical_width"]) * quality_scale + 0.5),
            )
            expected_target_height = max(
                1,
                math.floor(float(parsed_window["physical_height"]) * quality_scale + 0.5),
            )
            expected_target_factor = float(parsed_window["scale_factor"]) * quality_scale
            if target_values.get("scene_target_width") != expected_target_width:
                errors.append(
                    f"window.csv scene_target_width is {target_values.get('scene_target_width')!r}, "
                    f"expected {expected_target_width!r}"
                )
            if target_values.get("scene_target_height") != expected_target_height:
                errors.append(
                    f"window.csv scene_target_height is {target_values.get('scene_target_height')!r}, "
                    f"expected {expected_target_height!r}"
                )
            if not math.isclose(
                float(target_values.get("target_scale_factor", math.nan)),
                expected_target_factor,
                rel_tol=0.0,
                abs_tol=1e-5,
            ):
                errors.append(
                    "window.csv target_scale_factor does not match window scale and RtT quality"
                )
    elif quality_scale is not None:
        expected_target_width = max(1, math.floor(1280 * quality_scale + 0.5))
        expected_target_height = max(1, math.floor(720 * quality_scale + 0.5))
        if target_values.get("scene_target_width") != expected_target_width:
            errors.append(
                f"window.csv headless scene_target_width is {target_values.get('scene_target_width')!r}, "
                f"expected {expected_target_width!r}"
            )
        if target_values.get("scene_target_height") != expected_target_height:
            errors.append(
                f"window.csv headless scene_target_height is {target_values.get('scene_target_height')!r}, "
                f"expected {expected_target_height!r}"
            )
        if not math.isclose(
            float(target_values.get("target_scale_factor", math.nan)),
            quality_scale,
            rel_tol=0.0,
            abs_tol=1e-5,
        ):
            errors.append("window.csv headless target_scale_factor does not match RtT quality")

    return (row if not errors else None), errors


def read_frames(
    path: Path, expected_samples: int | None
) -> tuple[list[float] | None, list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse frames.csv: {error}"]
    if fieldnames != ["frame_index", "frame_time_ms"]:
        errors.append(
            "frames.csv columns differ from schema: " + ", ".join(fieldnames)
        )
    if not rows:
        errors.append("frames.csv has no samples")
    if expected_samples is not None and len(rows) != expected_samples:
        errors.append(f"frames.csv has {len(rows)} rows but summary declares {expected_samples}")
    samples: list[float] = []
    for index, row in enumerate(rows):
        try:
            frame_index = int(row["frame_index"])
            frame_time_ms = float(row["frame_time_ms"])
            if frame_index != index or not math.isfinite(frame_time_ms) or frame_time_ms < 0:
                raise ValueError
            samples.append(frame_time_ms)
        except (KeyError, TypeError, ValueError):
            errors.append(
                f"frames.csv row {index} must have sequential frame_index and finite nonnegative frame_time_ms"
            )
            break
    return (samples if not errors else None), errors


def frame_summary(samples: list[float]) -> dict[str, float]:
    ordered = sorted(samples)

    def percentile(ratio: float) -> float:
        index = math.floor((len(ordered) - 1) * ratio + 0.5)
        return ordered[index]

    return {
        "p50_ms": percentile(0.50),
        "p95_ms": percentile(0.95),
        "p99_ms": percentile(0.99),
        "max_ms": ordered[-1],
    }


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
        structural_checksum = row.get("structural_checksum", "")
        if not re.fullmatch(r"[0-9a-f]{16}", structural_checksum):
            errors.append(f"determinism.csv row {index} has invalid structural_checksum")
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


def read_determinism_records(
    path: Path,
    determinism: list[dict[str, str]],
    *,
    expected_workload: str,
    expected_indoor_actor_counts: dict[str, int] | None = None,
) -> tuple[list[dict[str, str]] | None, list[str]]:
    if not path.is_file():
        return None, [f"missing {path.name}"]
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (csv.Error, OSError, UnicodeError) as error:
        return None, [f"cannot parse determinism_records.csv: {error}"]

    errors: list[str] = []
    if fieldnames != list(DETERMINISM_RECORD_COLUMNS):
        errors.append(
            "determinism_records.csv columns differ from schema: "
            + ", ".join(fieldnames)
        )
    expected_checkpoints = [
        (row["checkpoint"], row["update_tick"]) for row in determinism
    ]
    checkpoint_order = {
        checkpoint: index for index, checkpoint in enumerate(expected_checkpoints)
    }
    seen: set[tuple[str, str, str]] = set()
    observed_sort_keys: list[tuple[int, str, int]] = []
    expected_actor_counts = {
        "soul": None,
        "familiar": None,
        "designation": None,
        **(expected_indoor_actor_counts or {}),
    }
    if expected_workload == "indoor-light" and expected_indoor_actor_counts is None:
        errors.append("indoor-light determinism validation is missing size-specific actor counts")
    if expected_workload != "indoor-light" and expected_indoor_actor_counts is not None:
        errors.append("non-indoor determinism validation received indoor actor counts")
    population_counts: dict[str, dict[str, int]] = {
        checkpoint: {actor_kind: 0 for actor_kind in expected_actor_counts}
        for checkpoint, _tick in expected_checkpoints
    }
    checkpoint_payloads: dict[str, list[bytes]] = {
        checkpoint: [] for checkpoint, _tick in expected_checkpoints
    }
    invalid_payload_checkpoints: set[str] = set()
    allowed_kinds = {*expected_actor_counts, "fixture"}
    for index, row in enumerate(rows):
        checkpoint_pair = (row.get("checkpoint", ""), row.get("update_tick", ""))
        if row.get("schema_version") != DETERMINISM_SCHEMA_VERSION:
            errors.append(
                f"determinism_records.csv row {index} has invalid schema_version"
            )
        if checkpoint_pair not in checkpoint_order:
            errors.append(
                f"determinism_records.csv row {index} has unknown checkpoint/tick"
            )
            continue
        actor_kind = row.get("actor_kind", "")
        if actor_kind not in allowed_kinds:
            errors.append(
                f"determinism_records.csv row {index} has invalid actor_kind "
                f"{actor_kind!r} for workload {expected_workload!r}"
            )
        try:
            actor_key = int(row["actor_key"])
            if actor_key < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"determinism_records.csv row {index} has invalid actor_key")
            continue
        record_hex = row.get("record_hex", "")
        if not record_hex or len(record_hex) % 2 != 0 or not re.fullmatch(
            r"[0-9a-f]+", record_hex
        ):
            errors.append(f"determinism_records.csv row {index} has invalid record_hex")
            invalid_payload_checkpoints.add(checkpoint_pair[0])
        else:
            checkpoint_payloads[checkpoint_pair[0]].append(bytes.fromhex(record_hex))
        identity = (checkpoint_pair[0], actor_kind, str(actor_key))
        if identity in seen:
            errors.append(f"determinism_records.csv row {index} duplicates an actor record")
        seen.add(identity)
        observed_sort_keys.append(
            (checkpoint_order[checkpoint_pair], actor_kind, actor_key)
        )
        if actor_kind in population_counts[checkpoint_pair[0]]:
            population_counts[checkpoint_pair[0]][actor_kind] += 1

    if observed_sort_keys != sorted(observed_sort_keys):
        errors.append("determinism_records.csv rows are not in stable checkpoint/actor order")
    for checkpoint_row in determinism:
        checkpoint = checkpoint_row["checkpoint"]
        for actor_kind, field in (
            ("soul", "souls"),
            ("familiar", "familiars"),
            ("designation", "designations"),
        ):
            observed = population_counts[checkpoint][actor_kind]
            expected = int(checkpoint_row[field])
            if observed != expected:
                errors.append(
                    f"determinism_records.csv {checkpoint} has {observed} {actor_kind} "
                    f"records; expected {expected}"
                )
        for actor_kind, expected in (expected_indoor_actor_counts or {}).items():
            observed = population_counts[checkpoint][actor_kind]
            if observed != expected:
                errors.append(
                    f"determinism_records.csv {checkpoint} has {observed} {actor_kind} "
                    f"records; expected {expected}"
                )
        if checkpoint not in invalid_payload_checkpoints:
            calculated_checksum = determinism_records_checksum(
                checkpoint_payloads[checkpoint]
            )
            if checkpoint_row.get("state_checksum") != calculated_checksum:
                errors.append(
                    f"determinism_records.csv {checkpoint} computes state_checksum "
                    f"{calculated_checksum}, but determinism.csv declares "
                    f"{checkpoint_row.get('state_checksum')!r}"
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
        if (
            "PERF_CAPTURE: wrote" in line
            or "PERF_DETERMINISM_AUDIT: wrote" in line
            or "PERF_BEHAVIOR: wrote" in line
            or "PERF_FIELD_CORE: wrote" in line
            or "PERF_CONSUMER_CORE: wrote" in line
        ):
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


def read_behavior_timeline(
    data_dir: Path,
    *,
    expected_case: Case,
    contract_id: str,
    stage_id: str,
) -> tuple[list[dict[str, Any]] | None, dict[str, Any] | None, list[str]]:
    errors: list[str] = []
    behavior_case = expected_case.behavior_case
    if behavior_case is None:
        return None, None, ["behavior validation requires a selected behavior case"]
    contract = load_rtt_light_contract(contract_id)
    if stage_id not in contract["stages"]:
        return None, None, [f"behavior timeline validator does not know stage {stage_id}"]
    timeline_contract = contract["behavior_fixture"]["timeline"]
    path = data_dir / "timeline.json"
    def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result
    try:
        payload = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        return None, None, [f"cannot parse timeline.json: {error}"]
    if not isinstance(payload, dict) or list(payload) != [
        "schema_version",
        "complete",
        "rows",
    ]:
        return None, None, ["timeline.json top-level schema or key order differs"]
    if payload.get("schema_version") != timeline_contract["schema_version"]:
        errors.append("timeline.json schema_version differs from the behavior contract")
    if payload.get("complete") is not True:
        errors.append("timeline.json is not marked complete")
    rows = payload.get("rows")
    if not isinstance(rows, list):
        return None, None, [*errors, "timeline.json rows must be a list"]
    case_contract = contract["behavior_fixture"].get(
        behavior_case.replace("-", "_")
    )
    if not isinstance(case_contract, dict):
        return None, None, [*errors, f"unknown behavior timeline case {behavior_case}"]
    expected_steps = case_contract["steps"]
    if len(rows) != len(expected_steps):
        errors.append(
            f"timeline.json must contain exactly {len(expected_steps)} rows; got {len(rows)}"
        )
    columns = timeline_contract["columns"]
    fixture_checksum = build_fixture_layout(contract, expected_case.size)["layout_checksum"]
    integer_fields = {
        "step_index",
        "script_update",
        "simulation_tick",
        "world_epoch",
    }
    bool_fields = {"attempted", "applied"}
    nullable_fields = {
        "registry_step_id",
        "wake_count",
        "field_input_revision",
        "field_output_revision",
        "field_read_count",
        "old_epoch_field_read_count",
        "field_is_dark",
        "field_checksum",
        "gpu_upload_epoch",
        "gpu_checksum",
    }
    for index, row in enumerate(rows):
        if not isinstance(row, dict) or list(row) != columns:
            errors.append(f"timeline.json row {index} columns or key order differ")
            continue
        for field in integer_fields:
            value = row.get(field)
            if (
                not isinstance(value, int)
                or isinstance(value, bool)
                or value < 0
                or value > (1 << 64) - 1
            ):
                errors.append(f"timeline.json row {index} {field} is not a nonnegative integer")
        for field in bool_fields:
            if not isinstance(row.get(field), bool):
                errors.append(f"timeline.json row {index} {field} is not boolean")
        if row.get("case_id") != behavior_case:
            errors.append(f"timeline.json row {index} has the wrong case_id")
        if row.get("step_index") != index:
            errors.append(f"timeline.json row {index} has the wrong step_index")
        if row.get("fixture_checksum") != fixture_checksum:
            errors.append(f"timeline.json row {index} has the wrong fixture_checksum")
        if stage_id in {"p05", "p06", "p07", "p08"}:
            if row.get("registry_phase") not in {
                "candidate_preflight",
                "load_reset",
                "wake_domains",
                "stage_before_registry_owner",
            }:
                errors.append(f"timeline.json row {index} has an invalid P05 registry phase")
        elif row.get("registry_phase") != "stage_before_registry_owner":
            errors.append(f"timeline.json row {index} has the wrong registry availability")
        if stage_id in {"p04", "p05", "p06", "p07", "p08"}:
            if row.get("field_availability") not in {"available", "unavailable"}:
                errors.append(f"timeline.json row {index} has the wrong field availability")
            if row.get("field_availability") == "available":
                for field in ("field_input_revision", "field_output_revision"):
                    value = row.get(field)
                    if not isinstance(value, int) or isinstance(value, bool) or value < 1:
                        errors.append(f"timeline.json row {index} {field} is not a published revision")
            if not isinstance(row.get("field_is_dark"), bool):
                errors.append(f"timeline.json row {index} field_is_dark is not boolean")
            if row.get("field_availability") == "available" and (
                not isinstance(row.get("field_checksum"), str) or re.fullmatch(
                    r"[0-9a-f]{64}", row["field_checksum"]
                ) is None
            ):
                errors.append(f"timeline.json row {index} field_checksum is invalid")
        elif row.get("field_availability") != "stage_before_field_owner":
            errors.append(f"timeline.json row {index} has the wrong field availability")
        if stage_id in {"p06", "p08"}:
            if row.get("gpu_availability") not in {"available", "unavailable"}:
                errors.append(f"timeline.json row {index} has the wrong GPU availability")
            if row.get("gpu_availability") == "available":
                upload_epoch = row.get("gpu_upload_epoch")
                checksum = row.get("gpu_checksum")
                if (
                    not isinstance(upload_epoch, int)
                    or isinstance(upload_epoch, bool)
                    or upload_epoch != row.get("world_epoch")
                ):
                    errors.append(f"timeline.json row {index} GPU epoch is stale")
                if not isinstance(checksum, str) or re.fullmatch(
                    r"[0-9a-f]{64}", checksum
                ) is None:
                    errors.append(f"timeline.json row {index} GPU checksum is invalid")
            elif row.get("gpu_upload_epoch") is not None or row.get("gpu_checksum") is not None:
                errors.append(f"timeline.json row {index} unavailable GPU fields must be null")
        elif row.get("gpu_availability") != "stage_before_gpu_owner":
            errors.append(f"timeline.json row {index} has the wrong GPU availability")
        required_runtime_fields = {
            "field_input_revision",
            "field_output_revision",
            "field_is_dark",
            "field_checksum",
        } if stage_id in {"p04", "p05", "p06", "p07", "p08"} else set()
        if stage_id in {"p05", "p06", "p07", "p08"}:
            required_runtime_fields |= {
                "registry_step_id",
                "wake_count",
                "field_read_count",
                "old_epoch_field_read_count",
            }
        if stage_id in {"p06", "p08"} and row.get("gpu_availability") == "available":
            required_runtime_fields |= {"gpu_upload_epoch", "gpu_checksum"}
        for field in nullable_fields - required_runtime_fields:
            if row.get(field) is not None:
                errors.append(f"timeline.json row {index} {field} must be null at this stage")
        if row.get("pause_state") not in {"running", "paused"}:
            errors.append(f"timeline.json row {index} has an invalid pause_state")
        if row.get("terminal_outcome") not in timeline_contract["terminal_outcomes"]:
            errors.append(f"timeline.json row {index} has an invalid terminal_outcome")

    comparable_rows = rows[: len(expected_steps)]
    if behavior_case == "door-state-v1":
        stage_prefix = (
            "p02" if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"} else "current"
        )
        for index, (row, expected) in enumerate(zip(comparable_rows, expected_steps)):
            exact = {
                "step_index": expected["step_index"],
                "script_update": expected["script_update"],
                "intent": expected["intent"],
                "pause_state": expected["pause_state"],
                "attempted": expected["attempted"],
                "applied": expected[f"{stage_prefix}_applied"],
                "semantic_state": expected[f"{stage_prefix}_semantic_state"],
                "active_presentation_state": expected[
                    f"{stage_prefix}_active_presentation_state"
                ],
                "terminal_outcome": (
                    "succeeded" if index == len(expected_steps) - 1 else "in_progress"
                ),
            }
            for field, expected_value in exact.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"timeline.json Door row {index} {field} differs from the contract"
                    )
        epochs = {row.get("world_epoch") for row in comparable_rows}
        if len(epochs) != 1:
            errors.append("timeline.json Door case changed WorldEpoch")
        ticks = [row.get("simulation_tick") for row in comparable_rows]
        if any(not isinstance(tick, int) for tick in ticks) or any(
            right < left for left, right in zip(ticks, ticks[1:])
        ):
            errors.append("timeline.json Door simulation ticks are not monotonic")
        if len(ticks) == 5 and not (ticks[3] == ticks[2] and ticks[4] == ticks[3]):
            errors.append("timeline.json Door pause did not freeze simulation ticks")
    elif behavior_case == "load-normal-v1":
        applied_by_step = [False, False, True, False, True, True]
        attempted_by_step = [False, True, False, True, False, False]
        for index, (row, expected) in enumerate(zip(comparable_rows, expected_steps)):
            exact = {
                "script_update": index,
                "intent": expected["intent"],
                "attempted": attempted_by_step[index],
                "applied": applied_by_step[index],
                "semantic_state": None,
                "active_presentation_state": None,
                "terminal_outcome": expected["terminal_outcome"],
            }
            for field, expected_value in exact.items():
                if row.get(field) != expected_value:
                    errors.append(
                        f"timeline.json load row {index} {field} differs from the contract"
                    )
        if len(comparable_rows) == 6:
            initial_epoch = comparable_rows[0].get("world_epoch")
            expected_epochs = [initial_epoch] * 4 + [initial_epoch + 1] * 2 if isinstance(
                initial_epoch, int
            ) else []
            if [row.get("world_epoch") for row in comparable_rows] != expected_epochs:
                errors.append("timeline.json normal load did not advance WorldEpoch exactly once")
            if len({row.get("pause_state") for row in comparable_rows}) != 1:
                errors.append("timeline.json normal load changed pause state")
            if any(row.get("pause_state") != "running" for row in comparable_rows):
                errors.append("timeline.json normal load did not remain running")
            ticks = [row.get("simulation_tick") for row in comparable_rows]
            if any(not isinstance(tick, int) for tick in ticks) or any(
                right < left for left, right in zip(ticks, ticks[1:])
            ):
                errors.append("timeline.json normal-load simulation ticks are not monotonic")

    if behavior_case not in {"door-state-v1", "load-normal-v1"}:
        for index, (row, expected) in enumerate(zip(comparable_rows, expected_steps)):
            if (
                row.get("intent") != expected["intent"]
                or row.get("terminal_outcome") != expected["terminal_outcome"]
            ):
                errors.append(
                    f"timeline.json P05 lifecycle row {index} differs from the contract"
                )

    save_path = data_dir / "behavior-save.scn.ron"
    save_artifact: dict[str, Any] | None = None
    if behavior_case.startswith("load-"):
        if not save_path.is_file():
            errors.append("normal-load behavior is missing behavior-save.scn.ron")
        else:
            size = save_path.stat().st_size
            if size <= 0:
                errors.append("behavior-save.scn.ron is empty")
            else:
                try:
                    contents = save_path.read_text(encoding="utf-8")
                except (OSError, UnicodeError) as error:
                    errors.append(f"cannot decode behavior-save.scn.ron: {error}")
                else:
                    header_match = re.fullmatch(
                        r"HELL_WORKERS_SAVE\n\(format_version: ([0-9]+), "
                        r"worldgen_seed: ([0-9]+)\)\n---\n([\s\S]+)",
                        contents,
                    )
                    if header_match is None:
                        errors.append("behavior-save.scn.ron has an invalid v1 header or empty body")
                    else:
                        format_version = int(header_match.group(1))
                        worldgen_seed = int(header_match.group(2))
                        body = header_match.group(3)
                        if format_version != 1:
                            errors.append("behavior-save.scn.ron format_version is not 1")
                        if worldgen_seed != expected_case.seed:
                            errors.append(
                                "behavior-save.scn.ron worldgen_seed differs from the case"
                            )
                        save_artifact = {
                            "path": "data/behavior-save.scn.ron",
                            "size_bytes": size,
                            "sha256": sha256(save_path),
                            "format_version": format_version,
                            "worldgen_seed": worldgen_seed,
                            "payload_size_bytes": len(body.encode("utf-8")),
                            "payload_sha256": hashlib.sha256(
                                body.encode("utf-8")
                            ).hexdigest(),
                            "terminal_fixture_checksum": fixture_checksum,
                        }
    elif save_path.exists():
        errors.append("Door behavior unexpectedly wrote behavior-save.scn.ron")

    if errors:
        return None, save_artifact, errors
    return rows, save_artifact, []














def measurement_duration_clock(workload: str) -> tuple[str, bool]:
    """Return the advancing capture clock and whether virtual time must stay frozen."""
    pauses_virtual_time = workload in {"indoor-light", "wall-density"}
    return ("real" if pauses_virtual_time else "virtual"), pauses_virtual_time


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
    expected_window_backend: str = "auto",
    expected_present_mode: str = "novsync",
    expected_window_width: int | None = None,
    expected_window_height: int | None = None,
    expected_window_scale_factor: float | None = None,
    expected_rtt_quality: str | None = None,
    expected_contract: str | None = None,
    expected_stage: str | None = None,
    expected_lane: str | None = None,
) -> Validation:
    reasons: list[str] = []
    data_dir = run_dir / "data"
    summary = None
    determinism = None
    determinism_records = None
    scene_roots = None
    render_inventory = None
    indoor_light_fixture = None
    indoor_light_layout = None
    indoor_light_presentation = None
    indoor_light_field = None
    indoor_light_runtime = None
    indoor_light_gpu = None
    indoor_light_consumers = None
    indoor_light_consumer_lifecycle = None
    p02_presentation = None
    deconstruction_fixture = None
    save_transaction = None
    dream_ui_metrics = None
    wall_density_fixture = None
    wall_density_layout = None
    timeline = None
    behavior_save_artifact = None
    if capture_kind in {"field-core", "consumer-core"}:
        window = None
        if (data_dir / "window.csv").exists():
            reasons.append(f"{capture_kind} must not write window.csv")
    else:
        window, window_errors = read_window(
            data_dir / "window.csv",
            expect_headless=expected_window_backend == "headless",
            expected_width=expected_window_width,
            expected_height=expected_window_height,
            expected_scale_factor=expected_window_scale_factor,
            expected_rtt_quality=expected_rtt_quality,
            expected_window_backend=expected_window_backend,
            expected_backend=expected_backend,
            expected_present_mode=expected_present_mode,
        )
        reasons.extend(window_errors)
    indoor_sidecar_paths = (
        data_dir / "indoor_light_fixture.csv",
        data_dir / "indoor_light_layout.csv",
        data_dir / "indoor_light_presentation.csv",
    )
    wall_density_paths = (
        data_dir / "wall_density_fixture.json",
        data_dir / "wall_density_layout.csv",
    )
    if expected_case.workload == "wall-density":
        if capture_kind != "frame-time":
            reasons.append("wall-density validation requires a frame-time capture")
        (
            wall_density_fixture,
            wall_density_layout,
            wall_density_errors,
        ) = read_wall_density_sidecars(data_dir, expected_case=expected_case)
        reasons.extend(wall_density_errors)
    elif expected_case.workload == "indoor-light" and capture_kind == "consumer-core":
        if (
            expected_contract != "rtt-light-v1"
            or expected_stage not in {"p07", "p08"}
            or expected_lane != "consumer-core"
        ):
            reasons.append("consumer-core requires rtt-light-v1/p07|p08/consumer-core")
        indoor_light_consumers, consumer_errors = read_indoor_light_consumers(data_dir)
        reasons.extend(consumer_errors)
    elif expected_case.workload == "indoor-light" and capture_kind == "field-core":
        if (
            expected_contract != "rtt-light-v1"
            or expected_stage not in {"p03", "p04", "p05", "p06", "p07", "p08"}
            or expected_lane != "field-core"
        ):
            reasons.append("field-core requires rtt-light-v1/p03|p04|p05|p06|p07|p08/field-core")
        indoor_light_field, field_errors = read_indoor_light_field(data_dir)
        reasons.extend(field_errors)
        if expected_stage in {"p04", "p05", "p06", "p07", "p08"} and expected_contract is not None:
            indoor_light_runtime, runtime_errors = read_indoor_light_runtime(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
                field_core=True,
            )
            reasons.extend(runtime_errors)
        unexpected_sidecars = [path.name for path in indoor_sidecar_paths if path.exists()]
        if unexpected_sidecars:
            reasons.append("field-core must not write ECS fixture sidecars")
    elif expected_case.workload == "indoor-light":
        if expected_contract is None or expected_stage is None or expected_lane is None:
            reasons.append(
                "indoor-light validation requires expected contract, stage, and lane"
            )
        else:
            (
                indoor_light_fixture,
                indoor_light_layout,
                indoor_light_presentation,
                indoor_errors,
            ) = read_indoor_light_sidecars(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
                lane=expected_lane,
            )
            reasons.extend(indoor_errors)
            if expected_stage in {"p04", "p05", "p06", "p07", "p08"}:
                indoor_light_runtime, runtime_errors = read_indoor_light_runtime(
                    data_dir,
                    expected_case=expected_case,
                    contract_id=expected_contract,
                    stage_id=expected_stage,
                    field_core=False,
                )
                reasons.extend(runtime_errors)
                if (
                    expected_stage in {"p06", "p08"}
                    and expected_lane == "static"
                    and expected_case.render == "gpu"
                    and capture_kind == "frame-time"
                ):
                    indoor_light_gpu, gpu_errors = read_indoor_light_gpu(
                        data_dir,
                        runtime=indoor_light_runtime,
                    )
                    reasons.extend(gpu_errors)
    elif expected_case.workload == "deconstruction":
        if capture_kind != "fixed-step-determinism":
            reasons.append("deconstruction validation requires fixed-step determinism capture")
        deconstruction_fixture, deconstruction_errors = read_deconstruction_fixture(
            data_dir / "deconstruction_fixture.csv"
        )
        reasons.extend(deconstruction_errors)
    elif expected_case.workload == "save-transaction":
        if capture_kind != "frame-time":
            reasons.append("save-transaction validation requires frame-time capture")
        save_transaction, save_transaction_errors = read_save_transaction(
            data_dir / "save_transaction.csv"
        )
        reasons.extend(save_transaction_errors)
    else:
        unexpected_sidecars = [path.name for path in indoor_sidecar_paths if path.exists()]
        if unexpected_sidecars:
            reasons.append(
                "non-indoor workload must not write indoor-light sidecars: "
                + ", ".join(unexpected_sidecars)
            )
    if expected_case.workload != "wall-density":
        unexpected_wall_sidecars = [path.name for path in wall_density_paths if path.exists()]
        if unexpected_wall_sidecars:
            reasons.append(
                "non-wall-density workload must not write wall-density sidecars: "
                + ", ".join(unexpected_wall_sidecars)
            )

    p02_sidecar = data_dir / "p02_presentation.csv"
    expects_p02_sidecar = (
        expected_case.workload == "indoor-light"
        and expected_stage in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}
        and expected_lane == "static"
        and capture_kind == "frame-time"
    )
    if expects_p02_sidecar:
        rows, parse_errors = read_exact_csv_rows(
            p02_sidecar,
            columns=P02_PRESENTATION_COLUMNS,
            artifact_name="p02_presentation.csv",
        )
        reasons.extend(parse_errors)
        if rows is not None and len(rows) == 1:
            p02_presentation = rows[0]
            if p02_presentation["schema_version"] != "1":
                reasons.append("p02_presentation.csv schema_version differs")
            for column in P02_PRESENTATION_COLUMNS[1:5] + P02_PRESENTATION_COLUMNS[6:9]:
                try:
                    if int(p02_presentation[column]) < 0:
                        raise ValueError
                except (TypeError, ValueError):
                    reasons.append(f"p02_presentation.csv {column} is invalid")
            for column in (
                "building_exactly_one_presentation",
                "state_and_bounce_probes_pass",
            ):
                if p02_presentation[column] not in {"true", "false"}:
                    reasons.append(f"p02_presentation.csv {column} is invalid")
        elif rows is not None:
            reasons.append(
                f"p02_presentation.csv must contain exactly one data row; got {len(rows)}"
            )
    elif p02_sidecar.exists():
        reasons.append("p02_presentation.csv is not allowed for this stage/lane/capture kind")
    if expected_case.workload != "deconstruction" and (
        data_dir / "deconstruction_fixture.csv"
    ).exists():
        reasons.append("non-deconstruction workload must not write deconstruction_fixture.csv")
    if expected_case.workload != "save-transaction" and (
        data_dir / "save_transaction.csv"
    ).exists():
        reasons.append("non-save-transaction workload must not write save_transaction.csv")
    dream_ui_metrics_path = data_dir / "dream_ui_metrics.csv"
    if expected_case.workload == "dream-ui-burst" and capture_kind in {
        "frame-time",
        "fixed-step-determinism",
    }:
        dream_ui_metrics, dream_ui_errors = read_dream_ui_metrics(dream_ui_metrics_path)
        reasons.extend(dream_ui_errors)
    elif dream_ui_metrics_path.exists():
        reasons.append("dream_ui_metrics.csv is only allowed for dream-ui-burst captures")
    transport_request_changes_path = data_dir / "transport_request_changes.csv"
    expects_transport_request_changes = (
        capture_kind == "frame-time"
        and expected_case.workload in {"construction", "task-dashboard"}
    )
    if expects_transport_request_changes:
        _, transport_change_errors = read_transport_request_changes(
            transport_request_changes_path
        )
        reasons.extend(transport_change_errors)
    elif transport_request_changes_path.exists():
        reasons.append(
            "transport_request_changes.csv is only allowed for frame-time "
            "construction/task-dashboard workloads"
        )
    spatial_query_metrics_path = data_dir / "spatial_query_metrics.csv"
    expects_spatial_query_metrics = (
        capture_kind == "frame-time"
        and expected_case.workload in SPATIAL_QUERY_METRICS_CONTRACTS
    )
    if expects_spatial_query_metrics:
        _, spatial_query_errors = read_spatial_query_metrics(
            spatial_query_metrics_path,
            workload=expected_case.workload,
        )
        reasons.extend(spatial_query_errors)
    elif spatial_query_metrics_path.exists():
        reasons.append(
            "spatial_query_metrics.csv is only allowed for frame-time path-door/gather workloads"
        )
    if capture_kind == "frame-time":
        if expected_case.workload == "save-transaction":
            if (data_dir / "summary.csv").exists():
                reasons.append("save-transaction workload must not write summary.csv")
        else:
            summary, summary_errors = read_summary(data_dir / "summary.csv")
            reasons.extend(summary_errors)
            scene_roots, scene_root_errors = read_scene_roots(data_dir / "scene_roots.csv")
            reasons.extend(scene_root_errors)
            if expected_case.workload == "indoor-light":
                render_inventory, render_inventory_errors = read_render_inventory(
                    data_dir / "render_inventory.csv"
                )
                reasons.extend(render_inventory_errors)
            elif (data_dir / "render_inventory.csv").exists():
                reasons.append(
                    "non-indoor workload must not write render_inventory.csv"
                )
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
            if determinism is not None:
                determinism_records, record_errors = read_determinism_records(
                    data_dir / "determinism_records.csv",
                    determinism,
                    expected_workload=expected_case.workload,
                    expected_indoor_actor_counts=(
                        build_fixture_audit_actor_counts(
                            load_rtt_light_contract(expected_contract),
                            expected_case.size,
                        )
                        if expected_case.workload == "indoor-light"
                        and expected_contract is not None
                        else None
                    ),
                )
                reasons.extend(record_errors)
    elif capture_kind == "fixed-step-behavior":
        if expected_contract is None or expected_stage is None:
            reasons.append("behavior validation requires expected contract and stage")
        else:
            timeline, behavior_save_artifact, timeline_errors = read_behavior_timeline(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
            )
            reasons.extend(timeline_errors)
            if expected_stage in {"p07", "p08"}:
                (
                    indoor_light_consumer_lifecycle,
                    lifecycle_errors,
                ) = read_indoor_light_consumer_lifecycle(
                    data_dir, expected_case=expected_case
                )
                reasons.extend(lifecycle_errors)
    elif capture_kind in {"field-core", "consumer-core"}:
        if expected_fixed_hz is None:
            reasons.append(f"{capture_kind} validation is missing fixed_hz")
    else:
        reasons.append(f"unsupported capture kind {capture_kind!r}")
    if capture_kind != "frame-time" and (data_dir / "render_inventory.csv").exists():
        reasons.append("fixed-step capture must not write render_inventory.csv")
    if capture_kind == "fixed-step-determinism" and (
        (data_dir / "summary.csv").exists()
        or (data_dir / "frames.csv").exists()
        or (data_dir / "scene_roots.csv").exists()
    ):
        reasons.append("fixed-step audit must not write frame-time artifacts")
    if capture_kind == "frame-time" and (
        (data_dir / "determinism.csv").exists()
        or (data_dir / "determinism_records.csv").exists()
    ):
        reasons.append("frame-time capture must not write determinism artifacts")
    if capture_kind == "fixed-step-behavior":
        unexpected_behavior_artifacts = [
            path.name
            for path in (
                data_dir / "summary.csv",
                data_dir / "frames.csv",
                data_dir / "scene_roots.csv",
                data_dir / "determinism.csv",
                data_dir / "determinism_records.csv",
            )
            if path.exists()
        ]
        if unexpected_behavior_artifacts:
            reasons.append(
                "behavior capture must not write frame-time or determinism artifacts: "
                + ", ".join(unexpected_behavior_artifacts)
            )
        expected_behavior_files = {
            "window.csv",
            "indoor_light_fixture.csv",
            "indoor_light_layout.csv",
            "indoor_light_presentation.csv",
            "timeline.json",
        }
        if expected_stage in {"p04", "p05", "p06", "p07", "p08"}:
            expected_behavior_files.add("indoor_light_runtime.json")
        if expected_stage in {"p07", "p08"}:
            expected_behavior_files.add("indoor_light_consumer_lifecycle.json")
        if expected_case.behavior_case is not None and expected_case.behavior_case.startswith(
            "load-"
        ):
            expected_behavior_files.add("behavior-save.scn.ron")
        actual_behavior_files = {
            path.name for path in data_dir.iterdir()
        } if data_dir.is_dir() else set()
        unknown_behavior_files = sorted(actual_behavior_files - expected_behavior_files)
        missing_behavior_files = sorted(expected_behavior_files - actual_behavior_files)
        if unknown_behavior_files:
            reasons.append(
                "behavior capture wrote unknown data artifacts: "
                + ", ".join(unknown_behavior_files)
            )
        if missing_behavior_files:
            reasons.append(
                "behavior capture is missing data artifacts: "
                + ", ".join(missing_behavior_files)
            )
    elif capture_kind == "field-core":
        expected_field_files = {"indoor_light_cpu.csv", "indoor_light_field.json"}
        if expected_stage in {"p04", "p05", "p06", "p07", "p08"}:
            expected_field_files.add("indoor_light_runtime.json")
        actual_field_files = (
            {path.name for path in data_dir.iterdir()} if data_dir.is_dir() else set()
        )
        if actual_field_files != expected_field_files:
            reasons.append(
                "field-core data artifact set differs: "
                + ", ".join(sorted(actual_field_files ^ expected_field_files))
            )
    elif capture_kind == "consumer-core":
        expected_consumer_files = {
            "indoor_light_consumers.csv",
            "indoor_light_consumer_proof.json",
        }
        actual_consumer_files = (
            {path.name for path in data_dir.iterdir()} if data_dir.is_dir() else set()
        )
        if actual_consumer_files != expected_consumer_files:
            reasons.append(
                "consumer-core data artifact set differs: "
                + ", ".join(sorted(actual_consumer_files ^ expected_consumer_files))
            )
    elif (data_dir / "timeline.json").exists() or (
        data_dir / "behavior-save.scn.ron"
    ).exists():
        reasons.append("non-behavior capture must not write behavior artifacts")
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
            "dashboard_mode": expected_case.dashboard_mode,
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
        frame_samples, frame_errors = read_frames(data_dir / "frames.csv", samples)
        reasons.extend(frame_errors)
        if frame_samples:
            computed_summary = frame_summary(frame_samples)
            for field, computed in computed_summary.items():
                try:
                    declared = float(summary[field])
                except (KeyError, TypeError, ValueError):
                    continue
                if not math.isclose(declared, computed, rel_tol=0.0, abs_tol=1e-6):
                    reasons.append(
                        f"summary {field} is {declared:.6f}, but frames.csv computes "
                        f"{computed:.6f}"
                    )
        duration_clock, pauses_virtual_time = measurement_duration_clock(
            expected_case.workload
        )
        for field, expected in (
            (f"warmup_{duration_clock}_secs", expected_warmup_secs),
            (f"measure_{duration_clock}_secs", expected_measure_secs),
        ):
            if expected is None:
                continue
            try:
                observed = float(summary[field])
            except (KeyError, TypeError, ValueError):
                continue
            if observed + 1e-6 < expected:
                reasons.append(
                    f"summary {field} is {observed:.6f}, below requested {expected:.6f}"
                )
        if pauses_virtual_time:
            for field in ("warmup_virtual_secs", "measure_virtual_secs"):
                try:
                    observed = float(summary[field])
                except (KeyError, TypeError, ValueError):
                    continue
                if not math.isclose(observed, 0.0, rel_tol=0.0, abs_tol=1e-6):
                    reasons.append(
                        f"summary {field} is {observed:.6f}, expected 0 while "
                        f"{expected_case.workload} simulation is paused"
                    )

    if summary is not None and scene_roots is not None:
        try:
            expected_souls = int(summary["initial_souls"])
            expected_familiars = int(summary["initial_familiars"])
        except (KeyError, ValueError):
            reasons.append("summary initial population is invalid for scene root validation")
        else:
            if expected_stage in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}:
                # P02 replaces the legacy Soul proxy family with
                # ActorBillboard3d and keeps Familiar presentation in the 2D
                # foreground pass. Their counts are validated by the P02
                # presentation sidecar, so every legacy proxy count is zero.
                expected_counts = {
                    "soul_proxy_3d": 0,
                    "soul_mask_proxy_3d": 0,
                    "soul_shadow_proxy_3d": 0,
                    "familiar_proxy_3d": 0,
                }
            else:
                expected_counts = {
                    "soul_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                    "soul_mask_proxy_3d": (
                        0
                        if expected_case.render == "cpu" or expected_stage == "p01"
                        else expected_souls
                    ),
                    "soul_shadow_proxy_3d": 0 if expected_case.render == "cpu" else expected_souls,
                    "familiar_proxy_3d": 0 if expected_case.render == "cpu" else expected_familiars,
                }
            for column, expected in expected_counts.items():
                if scene_roots.get(column) != str(expected):
                    reasons.append(
                        f"scene_roots.csv {column} is {scene_roots.get(column)!r}, "
                        f"expected {expected!r} for {expected_case.render}"
                    )

    if scene_roots is not None and render_inventory is not None:
        for column in (
            "soul_proxy_3d",
            "soul_mask_proxy_3d",
            "soul_shadow_proxy_3d",
            "familiar_proxy_3d",
        ):
            if render_inventory.get(column) != scene_roots.get(column):
                reasons.append(
                    f"render_inventory.csv {column} differs from scene_roots.csv"
                )

    if determinism is not None:
        if expected_fixed_hz is not None:
            expected_timestep_ns = round(1_000_000_000 / expected_fixed_hz)
            observed_timestep_ns = int(determinism[0]["fixed_timestep_ns"])
            if observed_timestep_ns != expected_timestep_ns:
                reasons.append(
                    "determinism.csv fixed_timestep_ns is "
                    f"{observed_timestep_ns}, expected {expected_timestep_ns} "
                    f"for {expected_fixed_hz} Hz"
                )
        for index, row in enumerate(determinism):
            if row.get("dashboard_mode") != expected_case.dashboard_mode:
                reasons.append(
                    "determinism.csv row "
                    f"{index} dashboard_mode is {row.get('dashboard_mode')!r}, "
                    f"expected {expected_case.dashboard_mode!r}"
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
            if capture_kind == "fixed-step-determinism"
            else "PERF_BEHAVIOR: wrote"
            if capture_kind == "fixed-step-behavior"
            else "PERF_FIELD_CORE: wrote"
            if capture_kind == "field-core"
            else "PERF_CONSUMER_CORE: wrote"
        )
        if completion_marker not in log_text:
            reasons.append(f"{completion_marker} completion marker is absent")
        expected_clock_mode = {
            "frame-time": "realtime",
            "fixed-step-determinism": "fixed",
            "fixed-step-behavior": "fixed-behavior",
            "field-core": "fixed",
            "consumer-core": "fixed",
        }.get(capture_kind)
        if f"clock={expected_clock_mode}" not in log_text:
            reasons.append(
                f"PERF_SCENARIO clock marker is absent or does not match {expected_clock_mode!r}"
            )
        for marker in (
            f"seed={expected_case.seed}",
            f"workload={expected_case.workload}",
            f"size={expected_case.size}",
            f"render={expected_case.render}",
            f"familiar_policy={expected_case.familiar_policy}",
            f"operation_dialog={expected_case.operation_dialog}",
            f"dashboard_mode={expected_case.dashboard_mode}",
        ):
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        behavior_marker = f"behavior_case={expected_case.behavior_case or 'none'}"
        if behavior_marker not in log_text:
            reasons.append(f"PERF_SCENARIO marker is absent: {behavior_marker}")
        if expected_case.wall_phase is not None:
            wall_marker = f"wall_phase={expected_case.wall_phase}"
            if wall_marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {wall_marker}")
        if capture_kind in {
            "fixed-step-determinism",
            "fixed-step-behavior",
            "field-core",
            "consumer-core",
        } and expected_fixed_hz is not None:
            marker = f"fixed_hz={expected_fixed_hz}"
            if marker not in log_text:
                reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
        if capture_kind == "frame-time":
            for name, expected in (
                ("warmup", expected_warmup_secs),
                ("measure", expected_measure_secs),
            ):
                if expected is None:
                    continue
                marker = f"{name}={expected:g}s"
                if marker not in log_text:
                    reasons.append(f"PERF_SCENARIO marker is absent: {marker}")
    warnings, teardown_warnings = classify_log_warnings(log_text, allow_log_patterns)
    reasons.extend(f"unexpected log warning/error: {line}" for line in warnings)

    adapter = parse_adapter(log_text)
    if adapter is not None and window is not None and window.get("window_present") == "true":
        if window.get("adapter_name") != adapter.get("name"):
            reasons.append("window.csv adapter_name differs from run.log")
        if window.get("adapter_backend") != adapter.get("backend", "").casefold():
            reasons.append("window.csv adapter_backend differs from run.log")
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
        determinism_records=determinism_records,
        scene_roots=scene_roots,
        render_inventory=render_inventory,
        window=window,
        indoor_light_fixture=indoor_light_fixture,
        indoor_light_layout=indoor_light_layout,
        indoor_light_presentation=indoor_light_presentation,
        indoor_light_field=indoor_light_field,
        indoor_light_runtime=indoor_light_runtime,
        indoor_light_gpu=indoor_light_gpu,
        indoor_light_consumers=indoor_light_consumers,
        indoor_light_consumer_lifecycle=indoor_light_consumer_lifecycle,
        p02_presentation=p02_presentation,
        deconstruction_fixture=deconstruction_fixture,
        save_transaction=save_transaction,
        dream_ui_metrics=dream_ui_metrics,
        wall_density_fixture=wall_density_fixture,
        wall_density_layout=wall_density_layout,
        timeline=timeline,
        behavior_save_artifact=behavior_save_artifact,
        profile_artifact=None,
    )
