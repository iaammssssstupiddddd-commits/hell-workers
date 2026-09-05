from __future__ import annotations

import json
import math
import re
from pathlib import Path
from typing import Any

from ..artifact_io import (compare_exact_rows, read_exact_csv_rows, reject_duplicate_json_keys)
from ..model import (
    Case, INDOOR_LIGHT_CONSUMER_COLUMNS, INDOOR_LIGHT_CONSUMER_PROOF_SCHEMA_VERSION,
    INDOOR_LIGHT_CPU_COLUMNS,
    INDOOR_LIGHT_FIELD_SCHEMA_VERSION, INDOOR_LIGHT_FIXTURE_COLUMNS,
    INDOOR_LIGHT_FIXTURE_SCHEMA_VERSION,
    INDOOR_LIGHT_LAYOUT_COLUMNS, INDOOR_LIGHT_PRESENTATION_COLUMNS, ONE_F64_BITS,
    Validation, ZERO_F64_BITS,
)
from ..rtt_light_contract import (
    build_fixture_layout, build_fixture_ledger, build_fixture_presentation_rows,
    contract_fingerprints, expected_eligible_supplied_emitters, load_rtt_light_contract,
    validate_stage_lane,
)

def expected_indoor_light_fixture_row(
    contract: dict[str, Any],
    case: Case,
    *,
    contract_id: str,
    stage_id: str,
    lane: str,
) -> dict[str, str]:
    layout = build_fixture_layout(contract, case.size)
    size_contract = contract["fixture"]["sizes"][case.size]
    counts = layout["counts"]
    hashes = contract_fingerprints(contract)
    lamp_demand = contract["fixture"]["outdoor_lamp_demand"]
    return {
        "schema_version": INDOOR_LIGHT_FIXTURE_SCHEMA_VERSION,
        "contract_id": contract_id,
        "stage_id": stage_id,
        "lane": lane,
        "checkpoint": "fixture-pre-update",
        "case_id": f"indoor-light-{case.size}-{case.render}-seed-{case.seed}",
        "fixture_id": layout["fixture_id"],
        "size": case.size,
        "layout_checksum": layout["layout_checksum"],
        "measurement_contract_sha256": hashes["measurement_contract_sha256"],
        "fixture_contract_sha256": hashes["fixture_contract_sha256"],
        "completed_floors": str(counts["completed_floors"]),
        "completed_walls": str(counts["completed_walls"]),
        "doors": str(counts["doors"]),
        "supplied_lamp_candidates": str(counts["supplied_lamp_candidates"]),
        "unsupplied_lamp_candidates": str(counts["unsupplied_lamp_candidates"]),
        "rooms": str(counts["rooms"]),
        "room_tiles": str(counts["completed_floors"]),
        "room_boundary_lookup_cells": str(counts["room_boundary_lookup_cells"]),
        "souls": str(counts["souls"]),
        "familiars": str(counts["familiars"]),
        "yards": str(counts["yards"]),
        "operational_soul_spas": str(counts["operational_soul_spas"]),
        "generator_souls": str(counts["generator_souls"]),
        "main_generation": f'{layout["energy"]["generation"]:.6f}',
        "main_demand": f'{size_contract["runtime_f32_active_lamp_demand"]:.6f}',
        "main_headroom": f'{size_contract["runtime_f32_headroom"]:.6f}',
        "main_supplied_count": str(counts["supplied_lamp_candidates"]),
        "main_shed_count": "0",
        "control_generation": "0.000000",
        "control_demand": f"{lamp_demand:.6f}",
        "control_supplied_count": "0",
        "control_shed_count": "1",
    }

def read_indoor_light_sidecars(
    data_dir: Path,
    *,
    expected_case: Case,
    contract_id: str,
    stage_id: str,
    lane: str,
) -> tuple[
    dict[str, str] | None,
    list[dict[str, str]] | None,
    list[dict[str, str]] | None,
    list[str],
]:
    errors: list[str] = []
    try:
        contract = load_rtt_light_contract(contract_id)
        validate_stage_lane(contract, stage_id, lane)
    except ValueError as error:
        return None, None, None, [f"invalid indoor-light selection: {error}"]

    fixture_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_fixture.csv",
        columns=INDOOR_LIGHT_FIXTURE_COLUMNS,
        artifact_name="indoor_light_fixture.csv",
    )
    errors.extend(parse_errors)
    expected_fixture = expected_indoor_light_fixture_row(
        contract,
        expected_case,
        contract_id=contract_id,
        stage_id=stage_id,
        lane=lane,
    )
    errors.extend(
        compare_exact_rows(
            "indoor_light_fixture.csv", fixture_rows, [expected_fixture]
        )
    )

    layout_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_layout.csv",
        columns=INDOOR_LIGHT_LAYOUT_COLUMNS,
        artifact_name="indoor_light_layout.csv",
    )
    errors.extend(parse_errors)
    errors.extend(
        compare_exact_rows(
            "indoor_light_layout.csv",
            layout_rows,
            build_fixture_ledger(contract, expected_case.size),
        )
    )

    presentation_rows, parse_errors = read_exact_csv_rows(
        data_dir / "indoor_light_presentation.csv",
        columns=INDOOR_LIGHT_PRESENTATION_COLUMNS,
        artifact_name="indoor_light_presentation.csv",
    )
    errors.extend(parse_errors)
    errors.extend(
        compare_exact_rows(
            "indoor_light_presentation.csv",
            presentation_rows,
            build_fixture_presentation_rows(
                contract, expected_case.size, stage_id=stage_id
            ),
        )
    )
    fixture = fixture_rows[0] if fixture_rows is not None and len(fixture_rows) == 1 else None
    return fixture, layout_rows, presentation_rows, errors

def read_indoor_light_field(
    data_dir: Path,
) -> tuple[dict[str, Any] | None, list[str]]:
    def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        payload: dict[str, Any] = {}
        for key, value in pairs:
            if key in payload:
                raise ValueError(f"duplicate JSON key: {key}")
            payload[key] = value
        return payload

    errors: list[str] = []
    rows, csv_errors = read_exact_csv_rows(
        data_dir / "indoor_light_cpu.csv",
        columns=INDOOR_LIGHT_CPU_COLUMNS,
        artifact_name="indoor_light_cpu.csv",
    )
    errors.extend(csv_errors)
    metadata_path = data_dir / "indoor_light_field.json"
    try:
        metadata = json.loads(
            metadata_path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        return None, [*errors, f"cannot parse indoor_light_field.json: {error}"]
    metadata_keys = {
        "schema_version",
        "grid_cells",
        "logical_payload_bytes",
        "packed_payload_bytes",
        "supplied_emitters",
        "radius_tiles",
        "warmup_calls",
        "measure_calls",
        "steady_updates",
        "steady_full_scans",
        "steady_field_rebuilds",
        "steady_revision_increments",
        "max_rebuilds_per_update",
        "input_checksum",
        "radiance_checksum",
        "mask_checksum",
        "field_checksum",
        "field_rebuild_allocation",
    }
    if not isinstance(metadata, dict) or set(metadata) != metadata_keys:
        return None, [*errors, "indoor_light_field.json keys differ from schema v1"]
    expected_scalars = {
        "schema_version": INDOOR_LIGHT_FIELD_SCHEMA_VERSION,
        "grid_cells": 10_000,
        "logical_payload_bytes": 80_000,
        "packed_payload_bytes": 40_000,
        "supplied_emitters": 50,
        "radius_tiles": 5,
        "warmup_calls": 32,
        "measure_calls": 256,
        "steady_updates": 600,
        "steady_full_scans": 0,
        "steady_field_rebuilds": 0,
        "steady_revision_increments": 0,
        "max_rebuilds_per_update": 0,
    }
    for field, expected in expected_scalars.items():
        if metadata.get(field) != expected:
            errors.append(f"indoor_light_field.json {field} differs from rtt-light-v1")
    for field in ("input_checksum", "radiance_checksum", "mask_checksum", "field_checksum"):
        if not isinstance(metadata.get(field), str) or re.fullmatch(
            r"[0-9a-f]{64}", metadata[field]
        ) is None:
            errors.append(f"indoor_light_field.json {field} is not lowercase SHA-256")
    allocation = metadata.get("field_rebuild_allocation")
    if not isinstance(allocation, dict) or set(allocation) != {"scope", "events", "bytes"}:
        errors.append("field_rebuild_allocation differs from schema v1")
    else:
        if allocation.get("scope") != "hw_infra::lighting::rebuild_field explicit owned buffers":
            errors.append("field_rebuild_allocation has the wrong scope")
        events = allocation.get("events")
        allocated_bytes = allocation.get("bytes")
        if not isinstance(events, int) or isinstance(events, bool) or not 1 <= events <= 64:
            errors.append("field_rebuild_allocation events are out of range")
        if (
            not isinstance(allocated_bytes, int)
            or isinstance(allocated_bytes, bool)
            or not 80_000 <= allocated_bytes <= 1_000_000
        ):
            errors.append("field_rebuild_allocation bytes are out of range")

    elapsed: list[int] = []
    if rows is not None:
        if len(rows) != 256:
            errors.append(f"indoor_light_cpu.csv must contain exactly 256 rows; got {len(rows)}")
        for index, row in enumerate(rows):
            exact = {
                "sample_index": str(index),
                "grid_cells": "10000",
                "supplied_emitters": "50",
                "radius_tiles": "5",
                "input_checksum": metadata.get("input_checksum"),
                "output_checksum": metadata.get("field_checksum"),
            }
            for field, expected in exact.items():
                if row.get(field) != expected:
                    errors.append(f"indoor_light_cpu.csv row {index} {field} differs")
            try:
                duration = int(row["elapsed_ns"])
                if duration <= 0 or duration > (1 << 64) - 1:
                    raise ValueError
            except (KeyError, TypeError, ValueError):
                errors.append(f"indoor_light_cpu.csv row {index} elapsed_ns is invalid")
            else:
                elapsed.append(duration)
    if errors or len(elapsed) != 256:
        return None, errors
    ordered = sorted(elapsed)
    quantile = lambda ratio: ordered[math.floor((len(ordered) - 1) * ratio + 0.5)] / 1_000_000
    return {
        **expected_scalars,
        "input_checksum": metadata["input_checksum"],
        "radiance_checksum": metadata["radiance_checksum"],
        "mask_checksum": metadata["mask_checksum"],
        "field_checksum": metadata["field_checksum"],
        "field_rebuild_p95_ms": quantile(0.95),
        "field_rebuild_p99_ms": quantile(0.99),
        "field_rebuild_allocation": allocation,
    }, []

def read_indoor_light_runtime(
    data_dir: Path,
    *,
    expected_case: Case,
    contract_id: str,
    stage_id: str,
    field_core: bool,
) -> tuple[dict[str, Any] | None, list[str]]:
    path = data_dir / "indoor_light_runtime.json"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, [f"cannot parse indoor_light_runtime.json: {error}"]
    keys = {
        "schema_version", "availability", "typed_emitter_components",
        "eligible_supplied_emitters", "unsupplied_snapshot_adoptions",
        "indoor_mask_cells", "indoor_mask_checksum", "input_revision",
        "output_revision", "field_checksum", "steady_updates",
        "steady_full_scans", "steady_field_rebuilds", "steady_revision_increments",
        "steady_scoped_allocation_events", "steady_scoped_allocation_bytes",
        "max_rebuilds_per_update", "emitter_collect_allocation",
    }
    if not isinstance(payload, dict) or set(payload) != keys:
        return None, ["indoor_light_runtime.json keys differ from schema v1"]
    errors: list[str] = []
    if payload.get("schema_version") != 1 or payload.get("availability") != "available":
        errors.append("indoor_light_runtime.json is not an available schema-v1 snapshot")
    integer_fields = keys - {
        "schema_version", "availability", "indoor_mask_checksum", "field_checksum",
        "emitter_collect_allocation",
    }
    for field in integer_fields:
        value = payload.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"indoor_light_runtime.json {field} is not a nonnegative integer")
    for field in ("indoor_mask_checksum", "field_checksum"):
        value = payload.get(field)
        if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
            errors.append(f"indoor_light_runtime.json {field} is not lowercase SHA-256")
    contract = load_rtt_light_contract(contract_id)
    layout = build_fixture_layout(contract, expected_case.size)
    size_contract = contract["fixture"]["sizes"][expected_case.size]
    if payload.get("typed_emitter_components") != layout["counts"]["supplied_lamp_candidates"] + 1:
        errors.append("indoor_light_runtime.json typed emitter count differs from fixture")
    expected_eligible_emitters = expected_eligible_supplied_emitters(
        contract,
        stage_id,
        expected_case.size,
        behavior_case=expected_case.behavior_case,
    )
    if payload.get("eligible_supplied_emitters") != expected_eligible_emitters:
        errors.append("indoor_light_runtime.json eligible emitter count differs from fixture")
    if payload.get("unsupplied_snapshot_adoptions") != 0:
        errors.append("indoor_light_runtime.json adopted the unsupplied control emitter")
    if payload.get("indoor_mask_cells") != layout["counts"]["completed_floors"]:
        errors.append("indoor_light_runtime.json mask cell count differs from fixture")
    if not field_core and payload.get("indoor_mask_checksum") != size_contract["indoor_mask_checksum"]:
        errors.append("indoor_light_runtime.json mask checksum differs from fixture")
    if payload.get("input_revision", 0) < 1 or payload.get("output_revision", 0) < 1:
        errors.append("indoor_light_runtime.json revisions did not publish")
    if payload.get("max_rebuilds_per_update", 2) > 1:
        errors.append("indoor_light_runtime.json rebuilt more than once in an update")
    if field_core:
        for field, expected in {
            "steady_updates": 600,
            "steady_full_scans": 0,
            "steady_field_rebuilds": 0,
            "steady_revision_increments": 0,
            "steady_scoped_allocation_events": 0,
            "steady_scoped_allocation_bytes": 0,
        }.items():
            if payload.get(field) != expected:
                errors.append(f"indoor_light_runtime.json {field} differs from P04 steady contract")
        allocation = payload.get("emitter_collect_allocation")
        if not isinstance(allocation, dict) or set(allocation) != {"scope", "events", "bytes"}:
            errors.append("indoor_light_runtime.json emitter allocation scope is missing")
        elif (
            allocation.get("scope") != "bevy_app::systems::lighting::collect_indoor_lighting_snapshot_system"
            or not isinstance(allocation.get("events"), int)
            or isinstance(allocation.get("events"), bool)
            or allocation["events"] < 1
            or not isinstance(allocation.get("bytes"), int)
            or isinstance(allocation.get("bytes"), bool)
            or allocation["bytes"] < 1
        ):
            errors.append("indoor_light_runtime.json emitter allocation scope is invalid")
    elif payload.get("emitter_collect_allocation") is not None:
        errors.append("non-field-core runtime sidecar unexpectedly contains allocation evidence")
    if errors:
        return None, errors
    return payload, []

def read_indoor_light_consumers(
    data_dir: Path,
) -> tuple[dict[str, Any] | None, list[str]]:
    errors: list[str] = []
    rows, csv_errors = read_exact_csv_rows(
        data_dir / "indoor_light_consumers.csv",
        columns=INDOOR_LIGHT_CONSUMER_COLUMNS,
        artifact_name="indoor_light_consumers.csv",
    )
    errors.extend(csv_errors)
    proof_path = data_dir / "indoor_light_consumer_proof.json"
    try:
        proof = json.loads(
            proof_path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_json_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        return None, [*errors, f"cannot parse indoor_light_consumer_proof.json: {error}"]
    proof_keys = {
        "schema",
        "schema_version",
        "warmup_calls",
        "measure_calls",
        "souls",
        "rooms",
        "room_cells",
        "max_samples_per_soul_slow_step",
        "max_effects_per_soul_slow_step",
        "revision_epoch_consistency",
        "mask_or_stale_effects",
        "scoped_allocation_events",
        "scoped_allocation_bytes",
    }
    if not isinstance(proof, dict) or set(proof) != proof_keys:
        return None, [*errors, "indoor_light_consumer_proof.json keys differ from schema v1"]
    expected = {
        "schema": "consumer-proof-v1",
        "schema_version": INDOOR_LIGHT_CONSUMER_PROOF_SCHEMA_VERSION,
        "warmup_calls": 32,
        "measure_calls": 256,
        "souls": 500,
        "rooms": 16,
        "room_cells": 576,
        "max_samples_per_soul_slow_step": 1,
        "revision_epoch_consistency": True,
        "mask_or_stale_effects": 0,
        "scoped_allocation_events": 0,
        "scoped_allocation_bytes": 0,
    }
    for field, value in expected.items():
        if proof.get(field) != value:
            errors.append(f"indoor_light_consumer_proof.json {field} differs")
    effects = proof.get("max_effects_per_soul_slow_step")
    if not isinstance(effects, int) or isinstance(effects, bool) or not 0 <= effects <= 1:
        errors.append("indoor_light_consumer_proof.json max effects is invalid")

    elapsed: list[int] = []
    if rows is not None:
        if len(rows) != 256:
            errors.append(
                f"indoor_light_consumers.csv must contain exactly 256 rows; got {len(rows)}"
            )
        epoch_revision: tuple[str, str] | None = None
        for index, row in enumerate(rows):
            exact = {
                "sample_index": str(index),
                "souls": "500",
                "rooms": "16",
                "room_cells": "576",
                "samples_per_soul_slow_step": "1",
                "revision_epoch_consistency": "true",
                "mask_or_stale_effects": "0",
            }
            for field, value in exact.items():
                if row.get(field) != value:
                    errors.append(
                        f"indoor_light_consumers.csv row {index} {field} differs"
                    )
            if row.get("effects_per_soul_slow_step") not in {"0", "1"}:
                errors.append(
                    f"indoor_light_consumers.csv row {index} effects value is invalid"
                )
            identity = (row.get("world_epoch", ""), row.get("field_revision", ""))
            epoch, revision = identity
            if not (
                epoch.isdigit()
                and int(epoch) >= 0
                and revision.isdigit()
                and int(revision) > 0
            ):
                errors.append(
                    f"indoor_light_consumers.csv row {index} epoch/revision is invalid"
                )
            elif epoch_revision is None:
                epoch_revision = identity
            elif identity != epoch_revision:
                errors.append(
                    f"indoor_light_consumers.csv row {index} epoch/revision changed"
                )
            try:
                duration = int(row["elapsed_ns"])
                if duration <= 0 or duration > (1 << 64) - 1:
                    raise ValueError
            except (KeyError, TypeError, ValueError):
                errors.append(
                    f"indoor_light_consumers.csv row {index} elapsed_ns is invalid"
                )
            else:
                elapsed.append(duration)
    if errors or len(elapsed) != 256:
        return None, errors
    ordered = sorted(elapsed)
    quantile = lambda ratio: ordered[
        math.floor((len(ordered) - 1) * ratio + 0.5)
    ] / 1_000_000
    return {
        "samples_per_soul_slow_step": 1,
        "effects_per_soul_slow_step": effects,
        "revision_epoch_consistency": True,
        "mask_or_stale_effects": 0,
        "consumer_p95_ms": quantile(0.95),
        "consumer_p99_ms": quantile(0.99),
        "scoped_allocation_events": 0,
        "scoped_allocation_bytes": 0,
    }, []

def read_indoor_light_consumer_lifecycle(
    data_dir: Path, *, expected_case: Case
) -> tuple[dict[str, Any] | None, list[str]]:
    path = data_dir / "indoor_light_consumer_lifecycle.json"
    try:
        payload = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_json_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        return None, [f"cannot parse indoor_light_consumer_lifecycle.json: {error}"]
    keys = {
        "schema",
        "schema_version",
        "case_id",
        "world_epoch",
        "field_revision",
        "old_epoch_recovery_effects",
        "old_epoch_room_summary_reads",
        "room_state_available",
    }
    errors: list[str] = []
    if not isinstance(payload, dict) or set(payload) != keys:
        return None, ["indoor_light_consumer_lifecycle.json keys differ from schema v1"]
    if payload.get("schema") != "consumer-lifecycle-v1" or payload.get("schema_version") != 1:
        errors.append("indoor_light_consumer_lifecycle.json schema differs")
    if payload.get("case_id") != expected_case.behavior_case:
        errors.append("indoor_light_consumer_lifecycle.json case_id differs")
    if (
        not isinstance(payload.get("world_epoch"), int)
        or isinstance(payload["world_epoch"], bool)
        or payload["world_epoch"] < 0
    ):
        errors.append("indoor_light_consumer_lifecycle.json world_epoch is invalid")
    if payload.get("field_revision") is not None and (
        not isinstance(payload["field_revision"], int)
        or isinstance(payload["field_revision"], bool)
        or payload["field_revision"] < 1
    ):
        errors.append("indoor_light_consumer_lifecycle.json field_revision is invalid")
    for field in ("old_epoch_recovery_effects", "old_epoch_room_summary_reads"):
        if payload.get(field) != 0:
            errors.append(f"indoor_light_consumer_lifecycle.json {field} must be zero")
    if not isinstance(payload.get("room_state_available"), bool):
        errors.append("indoor_light_consumer_lifecycle.json availability is invalid")
    else:
        expected_available = expected_case.behavior_case != "load-recovery-failed-v1"
        if payload["room_state_available"] != expected_available:
            errors.append(
                "indoor_light_consumer_lifecycle.json Room availability differs"
            )
    expected_field_revision = expected_case.behavior_case != "load-recovery-failed-v1"
    if (payload.get("field_revision") is not None) != expected_field_revision:
        errors.append(
            "indoor_light_consumer_lifecycle.json field availability differs"
        )
    return (None, errors) if errors else (payload, [])

def read_indoor_light_gpu(
    data_dir: Path,
    *,
    runtime: dict[str, Any] | None,
) -> tuple[dict[str, Any] | None, list[str]]:
    path = data_dir / "indoor_light_gpu.json"
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, [f"cannot parse indoor_light_gpu.json: {error}"]
    keys = {
        "schema_version", "availability", "field_image_count", "field_handle_count",
        "logical_payload_bytes", "staging_bytes", "upload_count",
        "uploads_per_changed_revision", "changed_revision_samples", "steady_updates",
        "steady_uploads", "steady_scoped_allocation_events",
        "steady_scoped_allocation_bytes", "upload_allocation_events",
        "upload_allocation_bytes", "old_epoch_uploads", "uploaded_epoch", "gpu_checksum",
    }
    if not isinstance(payload, dict) or set(payload) != keys:
        return None, ["indoor_light_gpu.json keys differ from schema v1"]
    errors: list[str] = []
    if payload.get("schema_version") != 1 or payload.get("availability") != "available":
        errors.append("indoor_light_gpu.json is not an available schema-v1 snapshot")
    for field in keys - {"schema_version", "availability", "gpu_checksum"}:
        value = payload.get(field)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            errors.append(f"indoor_light_gpu.json {field} is not a nonnegative integer")
    checksum = payload.get("gpu_checksum")
    if not isinstance(checksum, str) or re.fullmatch(r"[0-9a-f]{64}", checksum) is None:
        errors.append("indoor_light_gpu.json gpu_checksum is not lowercase SHA-256")
    if payload.get("field_image_count") != 1 or payload.get("field_handle_count") != 1:
        errors.append("indoor_light_gpu.json does not own exactly one shared image handle")
    if runtime is None:
        errors.append("indoor_light_gpu.json has no validated CPU runtime snapshot")
    elif checksum != runtime.get("field_checksum"):
        errors.append("indoor_light_gpu.json checksum differs from the CPU runtime")
    if payload.get("upload_count", 0) < 1 or payload.get("changed_revision_samples", 0) < 1:
        errors.append("indoor_light_gpu.json has no changed-revision upload")
    if errors:
        return None, errors
    return payload, []
