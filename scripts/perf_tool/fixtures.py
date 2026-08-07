from __future__ import annotations

import tempfile

from .compare import *
from .rtt_light_contract import (
    expected_formal_cases,
    expected_gate_result_rows,
    projection_field_applicability,
    validate_gate_result_rows,
    validate_projection_rows,
)
from .rtt_light_bundle import (
    _checksum_text as rtt_light_checksum_text,
    _verify_case_entry as verify_rtt_light_case_entry,
    build_gate_result_rows as build_rtt_light_gate_result_rows,
    build_projection_rows as build_rtt_light_projection_rows,
    directory_digest,
    resolve_baseline_locator,
)

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
) -> None:
    run_dir = root / "data"
    run_dir.mkdir(parents=True)
    window_row = {column: "" for column in WINDOW_COLUMNS}
    window_row.update(
        {
            "schema_version": WINDOW_SCHEMA_VERSION,
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
        writer = csv.DictWriter(handle, fieldnames=WINDOW_COLUMNS)
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
                "render": "cpu",
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
            writer.writerow({column: "0" for column in SCENE_ROOT_COLUMNS})
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
            "render=cpu clock=realtime behavior_case=none familiar_policy=baseline operation_dialog=hidden "
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
    root: Path, case: Case, *, lane: str = "static"
) -> None:
    data_dir = root / "data"
    contract = load_rtt_light_contract("rtt-light-v1")
    fixture_row = expected_indoor_light_fixture_row(
        contract,
        case,
        contract_id="rtt-light-v1",
        stage_id="current",
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
        writer.writerows(build_fixture_presentation_rows(contract, case.size))


def write_render_inventory_fixture(
    root: Path,
    *,
    scene_roots: dict[str, str] | None = None,
) -> None:
    values = {column: "0" for column in RENDER_INVENTORY_COLUMNS}
    values.update(
        {
            "schema_version": RENDER_INVENTORY_SCHEMA_VERSION,
            "scene_target_count": "1",
            "mask_target_count": "1",
            "camera_3d_rtt_count": "2",
            "camera_2d_count": "3",
            "layer_2d_pass_count": "2",
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


def write_behavior_fixture_run(root: Path, case: Case) -> None:
    if case.behavior_case is None:
        raise ValueError("behavior fixture requires a behavior case")
    data_dir = root / "data"
    data_dir.mkdir(parents=True)
    window_row = {column: "" for column in WINDOW_COLUMNS}
    window_row.update(
        {
            "schema_version": WINDOW_SCHEMA_VERSION,
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
        writer = csv.DictWriter(handle, fieldnames=WINDOW_COLUMNS)
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
                    step["current_applied"]
                    if case.behavior_case == "door-state-v1"
                    else load_applied[index]
                ),
                "semantic_state": (
                    step["current_semantic_state"]
                    if case.behavior_case == "door-state-v1"
                    else None
                ),
                "active_presentation_state": (
                    step["current_active_presentation_state"]
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
    write_indoor_light_sidecars(root, case, lane="behavior")
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


def self_test() -> int:
    temporary_root = workspace_temp_dir(REPO_ROOT, ".perf-self-test-tmp")
    temporary_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=temporary_root) as temporary:
        root = Path(temporary)
        rtt_contract = load_rtt_light_contract("rtt-light-v1")
        rtt_layouts = {
            size: build_fixture_layout(rtt_contract, size)
            for size in ("small", "medium", "large")
        }
        assert rtt_layouts["small"]["counts"]["completed_walls"] == 27
        assert rtt_layouts["medium"]["counts"]["rooms"] == 4
        assert rtt_layouts["large"]["counts"]["completed_floors"] == 576
        assert {
            size: layout["layout_checksum"] for size, layout in rtt_layouts.items()
        } == {
            "small": "e87a3b1aeb7ee1fbe334d311ad731bef24ce90ec80066af1e35c006ef4273af2",
            "medium": "e18320b3bcf8089c1ea2743003eadd79a0c938caa44682ed414e9d9d54af8f2d",
            "large": "3dec65d6c30ee9b88678af28a818a05fa70ededc66f20242ff78dcb6772c56fd",
        }
        for size in ("medium", "large"):
            showcase = rtt_layouts[size]["showcase_buildings"]
            assert [entry["kind"] for entry in showcase] == rtt_contract["fixture"][
                "building_types"
            ]
            assert {
                entry["kind"]: entry["anchor"]
                for entry in showcase
                if entry["source"] == "dedicated-showcase"
            } == {
                "Tank": [17, 28],
                "MudMixer": [20, 28],
                "RestArea": [17, 31],
                "Bridge": [90, 65],
                "SandPile": [27, 28],
                "BonePile": [28, 28],
                "WheelbarrowParking": [24, 28],
            }
            tank = showcase[3]
            assert tank["companion"] == {
                "kind": "BucketStorage",
                "anchor": [17, 30],
                "occupied_grids": [[17, 30], [18, 30]],
                "production_route": "tank-companion-placement",
            }
            assert showcase[6]["kind"] == "Bridge"
            assert showcase[6]["occupied_grids"] == [
                [x, y] for y in range(65, 70) for x in (90, 91)
            ]
            assert rtt_layouts[size]["counts"]["showcase_footprint_cells"] == 36
        assert {
            size: len(build_fixture_ledger(rtt_contract, size))
            for size in ("small", "medium", "large")
        } == {"small": 187, "medium": 722, "large": 2306}
        assert len(build_fixture_presentation_rows(rtt_contract, "small")) == 5
        for size in ("medium", "large"):
            presentation = build_fixture_presentation_rows(rtt_contract, size)
            assert [row["building_kind"] for row in presentation] == rtt_contract[
                "fixture"
            ]["building_types"]
            bridge = next(row for row in presentation if row["building_kind"] == "Bridge")
            assert bridge["entity_count"] == "1"
            assert bridge["child_sprite_count"] == "1"
            assert bridge["owner_3d_count"] == "0"
        validate_stage_lane(rtt_contract, "current", "static")
        try:
            validate_stage_lane(rtt_contract, "current", "field-core")
        except ValueError as error:
            assert "not applicable" in str(error)
        else:
            raise AssertionError("pre-P03 field-core unexpectedly became applicable")
        mutated_contract = json.loads(json.dumps(rtt_contract))
        mutated_contract["formal_matrix"]["repeat"] = 4
        try:
            validate_rtt_light_contract(mutated_contract)
        except ValueError as error:
            assert "pinned contract snapshot" in str(error)
        else:
            raise AssertionError("rtt-light-v1 changed without updating its pinned snapshot")

        mutated_showcase = json.loads(json.dumps(rtt_contract))
        mutated_showcase["fixture"]["sizes"]["medium"]["showcase_buildings"][6][
            "occupied_grids"
        ][-1] = [89, 68]
        original_pin = EXPECTED_CONTRACT_SHA256["rtt-light-v1"]
        EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = canonical_sha256(mutated_showcase)
        try:
            validate_rtt_light_contract(mutated_showcase)
        except ValueError as error:
            assert "production geometry" in str(error)
        else:
            raise AssertionError("invalid Bridge showcase geometry unexpectedly passed")
        finally:
            EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = original_pin

        assert {
            stage: len(expected_formal_cases(rtt_contract, stage))
            for stage in RTT_LIGHT_STAGES
        } == {
            "current": 18,
            "p01": 18,
            "p02": 18,
            "p03": 19,
            "p04": 19,
            "p05": 24,
            "p06": 24,
            "p07": 25,
            "p08": 25,
        }
        assert projection_field_applicability(
            rtt_contract, "p06", "renderdoc"
        )["gpu_upload"] == "available"
        assert projection_field_applicability(
            rtt_contract, "p06", "capture", "capture-medium-cpu"
        )["gpu_upload"] == "render_not_selected"
        assert projection_field_applicability(
            rtt_contract, "p06", "capture", "capture-medium-gpu"
        )["gpu_upload"] == "available"
        assert projection_field_applicability(
            rtt_contract, "p07", "renderdoc"
        )["gpu_upload"] == "stage_without_gpu_owner"
        assert projection_field_applicability(
            rtt_contract, "p06", "field-core"
        )["consumer_core"] == "stage_before_consumer_owner"

        projection_contract = rtt_contract["projection"]
        projection_columns = projection_contract["columns"]

        def valid_projection_value(column: dict[str, str]) -> str:
            kind = column["type"]
            if kind in {"sha256", "sha256_or_empty"}:
                return "0" * 64
            if kind in {"u32", "u64"} or kind.endswith("_or_empty"):
                return "0"
            if kind == "f64":
                return "0.0"
            return "fixture"

        def build_projection_rows(stage: str) -> list[dict[str, str]]:
            rows: list[dict[str, str]] = []
            for case in expected_formal_cases(rtt_contract, stage):
                row = {column["name"]: "" for column in projection_columns}
                row.update(
                    {
                        "schema_version": "1",
                        "contract_id": "rtt-light-v1",
                        "stage_id": stage,
                        "lane": case["lane"],
                        "leg_id": case["leg_id"],
                        "case_id": case["case_id"],
                    }
                )
                applicability = projection_field_applicability(
                    rtt_contract, stage, case["leg_id"], case["case_id"]
                )
                columns_by_name = {
                    column["name"]: column for column in projection_columns
                }
                for group_name, group in projection_contract["field_groups"].items():
                    availability = applicability[group_name]
                    row[group["availability_column"]] = availability
                    if availability == "available":
                        for name in group["value_columns"]:
                            row[name] = valid_projection_value(columns_by_name[name])
                layout = build_fixture_layout(rtt_contract, case["size"])
                row.update(
                    {
                        "fixture_checksum": layout["layout_checksum"],
                        "rooms": str(layout["counts"]["rooms"]),
                        "completed_floors": str(
                            layout["counts"]["completed_floors"]
                        ),
                        "completed_walls": str(
                            layout["counts"]["completed_walls"]
                        ),
                        "doors": str(layout["counts"]["doors"]),
                        "supplied_lamp_candidates": str(
                            layout["counts"]["supplied_lamp_candidates"]
                        ),
                        "unsupplied_lamp_candidates": str(
                            layout["counts"]["unsupplied_lamp_candidates"]
                        ),
                    }
                )
                if applicability["indoor_mask"] == "available":
                    row["indoor_mask_cells"] = str(
                        layout["counts"]["completed_floors"]
                    )
                    row["indoor_mask_checksum"] = rtt_contract["fixture"]["sizes"][
                        case["size"]
                    ]["indoor_mask_checksum"]
                if applicability["emitter"] == "available":
                    row["typed_emitter_components"] = str(
                        layout["counts"]["supplied_lamp_candidates"]
                        + layout["counts"]["unsupplied_lamp_candidates"]
                    )
                if applicability["eligible_emitter"] == "available":
                    row["eligible_supplied_emitters"] = str(
                        layout["counts"]["supplied_lamp_candidates"]
                    )
                rows.append(row)
            return rows

        current_projection_rows = build_projection_rows("current")
        validate_projection_rows(rtt_contract, "current", current_projection_rows)
        missing_projection_row = current_projection_rows[:-1]
        try:
            validate_projection_rows(rtt_contract, "current", missing_projection_row)
        except ValueError as error:
            assert "primary key set or order" in str(error)
        else:
            raise AssertionError("projection with a missing formal case unexpectedly passed")
        duplicate_projection_row = [
            *current_projection_rows,
            dict(current_projection_rows[-1]),
        ]
        try:
            validate_projection_rows(rtt_contract, "current", duplicate_projection_row)
        except ValueError as error:
            assert "primary key set or order" in str(error)
        else:
            raise AssertionError("projection with a duplicate case unexpectedly passed")
        invalid_projection_availability = [
            dict(row) for row in current_projection_rows
        ]
        invalid_projection_availability[0]["gpu_upload_availability"] = "available"
        try:
            validate_projection_rows(
                rtt_contract, "current", invalid_projection_availability
            )
        except ValueError as error:
            assert "gpu_upload_availability" in str(error)
        else:
            raise AssertionError("wrong projection applicability unexpectedly passed")
        invalid_projection_value = [dict(row) for row in current_projection_rows]
        invalid_projection_value[0]["gpu_upload_count"] = "1"
        try:
            validate_projection_rows(rtt_contract, "current", invalid_projection_value)
        except ValueError as error:
            assert "values while not applicable" in str(error)
        else:
            raise AssertionError("value in an unavailable projection group passed")
        invalid_projection_overflow = [dict(row) for row in current_projection_rows]
        invalid_projection_overflow[0]["rooms"] = str(1 << 32)
        try:
            validate_projection_rows(
                rtt_contract, "current", invalid_projection_overflow
            )
        except ValueError as error:
            assert "invalid rooms" in str(error)
        else:
            raise AssertionError("overflowing u32 projection value unexpectedly passed")

        p06_projection_rows = build_projection_rows("p06")
        validate_projection_rows(rtt_contract, "p06", p06_projection_rows)
        p06_gpu_row = next(
            row
            for row in p06_projection_rows
            if row["case_id"] == "capture-medium-gpu"
        )
        p06_gpu_row["gpu_upload_count"] = str(1 << 64)
        try:
            validate_projection_rows(rtt_contract, "p06", p06_projection_rows)
        except ValueError as error:
            assert "invalid gpu_upload_count" in str(error)
        else:
            raise AssertionError("overflowing u64 projection value unexpectedly passed")

        def build_gate_rows(stage: str) -> list[dict[str, str]]:
            rows: list[dict[str, str]] = []
            for expected in expected_gate_result_rows(rtt_contract, stage):
                row = {
                    column: "" for column in rtt_contract["gate_result"]["columns"]
                }
                row.update(
                    {
                        column: expected[column]
                        for column in (
                            "gate_id",
                            "stage_id",
                            "case_id",
                            "metric_id",
                            "unit",
                            "comparator",
                            "threshold",
                        )
                    }
                )
                row.update(
                    {
                        "status": "pass",
                        "observed": expected["threshold"],
                        "reference_artifact": expected["reference_artifact"],
                        "subject_artifact": expected["subject_artifact"],
                        "reason_code": "none",
                    }
                )
                rows.append(row)
            return rows

        current_gate_rows = build_gate_rows("current")
        validate_gate_result_rows(
            rtt_contract, "current", current_gate_rows, require_pass=True
        )
        for mutation, expected_reason in (
            (current_gate_rows[:-1], "primary key set or order"),
            ([*current_gate_rows, dict(current_gate_rows[-1])], "primary key set or order"),
        ):
            try:
                validate_gate_result_rows(
                    rtt_contract, "current", mutation, require_pass=True
                )
            except ValueError as error:
                assert expected_reason in str(error)
            else:
                raise AssertionError("gate result with a missing or duplicate row passed")
        invalid_gate_unit = [dict(row) for row in current_gate_rows]
        invalid_gate_unit[0]["unit"] = "wrong"
        try:
            validate_gate_result_rows(
                rtt_contract, "current", invalid_gate_unit, require_pass=True
            )
        except ValueError as error:
            assert "wrong unit" in str(error)
        else:
            raise AssertionError("gate result with the wrong unit unexpectedly passed")
        invalid_gate_status = [dict(row) for row in current_gate_rows]
        invalid_gate_status[0]["status"] = "fail"
        try:
            validate_gate_result_rows(
                rtt_contract, "current", invalid_gate_status, require_pass=False
            )
        except ValueError as error:
            assert "status and reason_code" in str(error)
        else:
            raise AssertionError("inconsistent gate status and reason unexpectedly passed")
        invalid_gate_scalar = [dict(row) for row in current_gate_rows]
        invalid_gate_scalar[0]["observed"] = "garbage"
        try:
            validate_gate_result_rows(
                rtt_contract, "current", invalid_gate_scalar, require_pass=True
            )
        except ValueError as error:
            assert "invalid scalar" in str(error)
        else:
            raise AssertionError("gate result with an untyped scalar unexpectedly passed")
        invalid_gate_subject = [dict(row) for row in current_gate_rows]
        invalid_gate_subject[0]["subject_artifact"] = "wrong-stage/index.json"
        try:
            validate_gate_result_rows(
                rtt_contract, "current", invalid_gate_subject, require_pass=True
            )
        except ValueError as error:
            assert "subject artifact lineage" in str(error)
        else:
            raise AssertionError("gate result with the wrong subject lineage passed")

        p08_gate_rows = build_gate_rows("p08")
        signed_delta = next(
            row
            for row in p08_gate_rows
            if row["metric_id"] == "large_peak_live_delta_bytes"
        )
        signed_delta["observed"] = "-1"
        validate_gate_result_rows(
            rtt_contract, "p08", p08_gate_rows, require_pass=True
        )

        p01_gate_rows = build_gate_rows("p01")
        reference_row = next(
            row for row in p01_gate_rows if row["metric_id"].endswith("relative_pct")
        )
        reference_row["reference_artifact"] = ""
        try:
            validate_gate_result_rows(
                rtt_contract, "p01", p01_gate_rows, require_pass=True
            )
        except ValueError as error:
            assert "reference artifact lineage" in str(error)
        else:
            raise AssertionError("relative gate without a reference unexpectedly passed")

        p04_emitter_thresholds = {
            (row["case_id"], row["metric_id"]): row["threshold"]
            for row in expected_gate_result_rows(rtt_contract, "p04")
            if row["gate_id"] == "RLV1-P04-EMITTER"
            and row["metric_id"]
            in {"typed_emitter_components", "eligible_supplied_emitters"}
        }
        assert p04_emitter_thresholds == {
            ("audit-small-cpu", "typed_emitter_components"): "2",
            ("audit-medium-cpu", "typed_emitter_components"): "11",
            ("audit-large-cpu", "typed_emitter_components"): "51",
            ("audit-small-cpu", "eligible_supplied_emitters"): "1",
            ("audit-medium-cpu", "eligible_supplied_emitters"): "10",
            ("audit-large-cpu", "eligible_supplied_emitters"): "50",
        }
        p04_mask_thresholds = {
            (row["case_id"], row["metric_id"]): row["threshold"]
            for row in expected_gate_result_rows(rtt_contract, "p04")
            if row["gate_id"] == "RLV1-P04-EMITTER"
            and row["metric_id"] in {"indoor_mask_cells", "indoor_mask_checksum"}
        }
        assert p04_mask_thresholds == {
            ("audit-small-cpu", "indoor_mask_cells"): "36",
            ("audit-medium-cpu", "indoor_mask_cells"): "144",
            ("audit-large-cpu", "indoor_mask_cells"): "576",
            (
                "audit-small-cpu",
                "indoor_mask_checksum",
            ): "5a8aa70044f3b944c838eab0a7043344db71abcbf5f93532f1bb1e5ad11d9960",
            (
                "audit-medium-cpu",
                "indoor_mask_checksum",
            ): "574f63940b48f33ec4f0179041a72649b235608983a64c6242dcf5664d589a16",
            (
                "audit-large-cpu",
                "indoor_mask_checksum",
            ): "11008aa69297a381263083d0dfe2c444e3076a3eb9b73a06d2185a317f4759d0",
        }

        assert [
            leg["leg_id"] for leg in rtt_contract["formal_legs"][:5]
        ] == ["audit", "behavior", "capture", "renderdoc", "memory"]
        bundle_cases: dict[str, dict[str, Any]] = {}
        determinism_row = {column: "0" for column in DETERMINISM_COLUMNS}
        render_inventory = {
            "schema_version": RENDER_INVENTORY_SCHEMA_VERSION,
            "scene_target_count": "1",
            "mask_target_count": "1",
            "camera_3d_rtt_count": "2",
            "camera_2d_count": "1",
            "layer_2d_pass_count": "1",
            "soul_proxy_3d": "1",
            "soul_mask_proxy_3d": "1",
            "soul_shadow_proxy_3d": "1",
            "familiar_proxy_3d": "0",
        }
        for formal_case in expected_formal_cases(rtt_contract, "current"):
            layout = rtt_layouts[formal_case["size"]]
            counts = layout["counts"]
            fixture = {
                "layout_checksum": layout["layout_checksum"],
                **{
                    column: str(counts[column])
                    for column in (
                        "rooms",
                        "completed_floors",
                        "completed_walls",
                        "doors",
                        "supplied_lamp_candidates",
                        "unsupplied_lamp_candidates",
                    )
                },
            }
            common = {
                "formal": formal_case,
                "unexpected_log_lines": 0,
                "environment_contract_match": True,
                "required_sidecars_valid": True,
            }
            if formal_case["leg_id"] == "renderdoc":
                bundle_cases[formal_case["case_id"]] = common | {
                    "validations": [],
                    "fixture": {
                        "fixture_checksum": layout["layout_checksum"],
                        **{
                            column: counts[column]
                            for column in (
                                "rooms",
                                "completed_floors",
                                "completed_walls",
                                "doors",
                                "supplied_lamp_candidates",
                                "unsupplied_lamp_candidates",
                            )
                        },
                    },
                    "render_inventory": {
                        key: value
                        for key, value in render_inventory.items()
                        if key != "schema_version"
                    },
                    "validated_frames": 1,
                }
                continue
            validations = []
            for run_index in range(3):
                validations.append(
                    Validation(
                        valid=True,
                        reasons=[],
                        summary=(
                            {
                                "p50_ms": "1.0",
                                "p95_ms": "2.0",
                                "p99_ms": "3.0",
                                "max_ms": "4.0",
                            }
                            if formal_case["leg_id"] in {"capture", "memory"}
                            else None
                        ),
                        adapter=None,
                        warning_lines=[],
                        teardown_warning_lines=[],
                        determinism=(
                            [determinism_row]
                            if formal_case["leg_id"] == "audit"
                            else None
                        ),
                        render_inventory=(
                            render_inventory
                            if formal_case["leg_id"] in {"capture", "memory"}
                            else None
                        ),
                        indoor_light_fixture=fixture,
                        profile_artifact=(
                            {
                                "process_memory": {
                                    "max_rss_kib": 1000 + run_index
                                },
                                "allocation_memory": {
                                    "peak_live_bytes": 2000 + run_index
                                },
                            }
                            if formal_case["leg_id"] == "memory"
                            else None
                        ),
                    )
                )
            bundle_cases[formal_case["case_id"]] = common | {
                "validations": validations
            }
        generated_projection = build_rtt_light_projection_rows(
            rtt_contract, "current", bundle_cases
        )
        assert len(generated_projection) == 18
        assert all(len(row) == 58 for row in generated_projection)
        assert generated_projection[-1]["case_id"] == "memory-large-gpu"
        generated_gates = build_rtt_light_gate_result_rows(
            rtt_contract, "current", bundle_cases
        )
        assert len(generated_gates) == 94
        assert all(row["status"] == "pass" for row in generated_gates)
        assert generated_gates[0]["subject_artifact"] == (
            "rtt-light-v1/baseline-index.json#/stages/current/cases/attempt"
        )

        ledger_root = root / "rtt-light-ledger"
        generation = ledger_root / f"current-{'0' * 40}"
        attempt = generation / "attempts" / "00000000-0000-4000-8000-000000000000"
        attempt.mkdir(parents=True)
        payload = attempt / "payload.txt"
        payload.write_text("registered\n", encoding="utf-8")
        environment_lock = generation / "environment-lock.json"
        write_json(environment_lock, {"status": "valid"})
        raw_artifacts = [
            {
                "path": "payload.txt",
                "bytes": payload.stat().st_size,
                "sha256": sha256(payload),
            }
        ]
        raw_digest = directory_digest(raw_artifacts)
        manifest = {
            "environment_lock": {
                "path": "../../environment-lock.json",
                "sha256": sha256(environment_lock),
                "value": {"status": "valid"},
            },
            "raw_artifacts": raw_artifacts,
            "raw_directory_sha256": raw_digest,
            "cases": {
                "attempt": {
                    "leg_id": "attempt",
                    "lane": "static",
                    "size": "not_applicable",
                    "render": "not_applicable",
                    "repeat": 1,
                    "status": "valid",
                    "path": ".",
                    "artifact_count": 1,
                    "directory_sha256": raw_digest,
                    "inventory_scope": "raw-artifacts",
                }
            },
        }
        manifest_path = attempt / "attempt-manifest.json"
        write_json(manifest_path, manifest)
        attempt_prefix = attempt.relative_to(ledger_root).as_posix()
        rooted_case = {
            **manifest["cases"]["attempt"],
            "path": f"{attempt_prefix}/.",
        }
        index = {
            "schema_version": 1,
            "contract_id": "rtt-light-v1",
            "measurement_contract_sha256": "0" * 64,
            "fixture_contract_sha256": "1" * 64,
            "stages": {
                "current": {
                    "attempt_manifest": {
                        "path": manifest_path.relative_to(ledger_root).as_posix(),
                        "sha256": sha256(manifest_path),
                    },
                    "cases": {"attempt": rooted_case},
                }
            },
        }
        write_json(ledger_root / "baseline-index.json", index)
        locator = (
            "rtt-light-v1/baseline-index.json#/stages/current/cases/attempt"
        )
        resolved_case = resolve_baseline_locator(ledger_root, index, locator)
        assert resolved_case == rooted_case
        verify_rtt_light_case_entry(
            baseline_root=ledger_root,
            attempt=attempt,
            manifest=manifest,
            case_id="attempt",
            entry=resolved_case,
        )
        registered_checksum = rtt_light_checksum_text(ledger_root, index)
        unregistered = generation / "attempts" / "unregistered" / "failure.log"
        unregistered.parent.mkdir()
        unregistered.write_text("failed attempt\n", encoding="utf-8")
        assert rtt_light_checksum_text(ledger_root, index) == registered_checksum
        payload.write_text("tampered\n", encoding="utf-8")
        assert rtt_light_checksum_text(ledger_root, index) != registered_checksum

        mutated_semantics = json.loads(json.dumps(rtt_contract))
        mutated_semantics["behavior_fixture"]["door_state_v1"]["steps"][1][
            "current_applied"
        ] = True
        original_pin = EXPECTED_CONTRACT_SHA256["rtt-light-v1"]
        EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = canonical_sha256(mutated_semantics)
        try:
            validate_rtt_light_contract(mutated_semantics)
        except ValueError as error:
            assert "current Door baseline" in str(error)
        else:
            raise AssertionError("invalid Door behavior semantics unexpectedly passed")
        finally:
            EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = original_pin

        for mutation_name, mutate, expected_reason in (
            (
                "behavior-first-stage",
                lambda value: value["stages"]["current"][
                    "required_behavior_cases"
                ].__setitem__(1, "load-preflight-reject-v1"),
                "behavior cases differ",
            ),
            (
                "dead-stage-filter",
                lambda value: value["gate_result"]["metric_templates"][
                    "RLV1-P05-LIFECYCLE"
                ][-1].__setitem__("required_stages", ["current"]),
                "dead or inapplicable stage filter",
            ),
            (
                "unknown-aggregation",
                lambda value: value["gate_result"]["metric_templates"][
                    "RLV1-VALID-SOURCE"
                ][0].__setitem__("aggregation", "bogus"),
                "invalid metric template",
            ),
            (
                "nonoptional-unavailable-projection",
                lambda value: next(
                    column
                    for column in value["projection"]["columns"]
                    if column["name"] == "gpu_upload_count"
                ).__setitem__("type", "u64"),
                "must be optional",
            ),
            (
                "garbage-gate-threshold",
                lambda value: next(
                    gate
                    for gate in value["gates"]
                    if gate["gate_id"] == "RLV1-P03-FIELD"
                ).__setitem__("p95_ms_max", "garbage"),
                "invalid threshold",
            ),
            (
                "boolean-order-comparator",
                lambda value: value["gate_result"]["metric_templates"][
                    "RLV1-VALID-SOURCE"
                ][0].__setitem__("comparator", "le"),
                "incompatible comparator",
            ),
            (
                "missing-reference-lineage",
                lambda value: value["gate_result"]["metric_templates"][
                    "RLV1-P01-PERF"
                ][0].pop("reference_stage"),
                "wrong reference lineage",
            ),
            (
                "wrong-reference-stage",
                lambda value: value["gate_result"]["metric_templates"][
                    "RLV1-P02-PERF"
                ][0].__setitem__("reference_stage", "current"),
                "wrong reference lineage",
            ),
            (
                "unknown-gate-kind",
                lambda value: value["gates"][1].__setitem__("kind", "bogus"),
                "gate definitions are invalid",
            ),
        ):
            mutated = json.loads(json.dumps(rtt_contract))
            mutate(mutated)
            original_pin = EXPECTED_CONTRACT_SHA256["rtt-light-v1"]
            EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = canonical_sha256(mutated)
            try:
                validate_rtt_light_contract(mutated)
            except ValueError as error:
                assert expected_reason in str(error), (mutation_name, str(error))
            else:
                raise AssertionError(
                    f"semantic mutation {mutation_name} unexpectedly passed"
                )
            finally:
                EXPECTED_CONTRACT_SHA256["rtt-light-v1"] = original_pin

        indoor_root = root / "indoor-sidecar"
        indoor_case = Case(
            "indoor-light", "small", "cpu", 20_260_803, None, None
        )
        write_fixture_run(
            indoor_root,
            fixed_step_audit=True,
            workload="indoor-light",
            seed=indoor_case.seed,
        )
        write_indoor_light_sidecars(indoor_root, indoor_case)

        def validate_indoor_fixture() -> Validation:
            return validate_run(
                indoor_root,
                returncode=0,
                expected_case=indoor_case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
                capture_kind="fixed-step-determinism",
                expected_fixed_hz=64,
                expected_warmup_ticks=1920,
                expected_audit_ticks=128,
                expected_contract="rtt-light-v1",
                expected_stage="current",
                expected_lane="static",
            )

        indoor_validation = validate_indoor_fixture()
        assert indoor_validation.valid, indoor_validation.reasons
        assert indoor_validation.indoor_light_fixture is not None
        assert len(indoor_validation.indoor_light_layout or []) == 187
        assert len(indoor_validation.indoor_light_presentation or []) == 5

        indoor_realtime_root = root / "indoor-realtime-sidecar"
        write_fixture_run(
            indoor_realtime_root,
            workload="indoor-light",
            size="small",
            seed=indoor_case.seed,
        )
        write_indoor_light_sidecars(indoor_realtime_root, indoor_case)
        write_render_inventory_fixture(indoor_realtime_root)

        def validate_indoor_realtime_fixture() -> Validation:
            return validate_run(
                indoor_realtime_root,
                returncode=0,
                expected_case=indoor_case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
                expected_contract="rtt-light-v1",
                expected_stage="current",
                expected_lane="static",
            )

        indoor_realtime_validation = validate_indoor_realtime_fixture()
        assert indoor_realtime_validation.valid, indoor_realtime_validation.reasons
        assert indoor_realtime_validation.render_inventory is not None
        inventory_path = indoor_realtime_root / "data" / "render_inventory.csv"
        with inventory_path.open(newline="", encoding="utf-8") as handle:
            inventory_reader = csv.DictReader(handle)
            inventory_fields = inventory_reader.fieldnames
            inventory_rows = list(inventory_reader)
        assert inventory_fields is not None and len(inventory_rows) == 1
        inventory_rows[0]["camera_3d_rtt_count"] = "1"
        with inventory_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=inventory_fields)
            writer.writeheader()
            writer.writerows(inventory_rows)
        invalid_inventory = validate_indoor_realtime_fixture()
        assert not invalid_inventory.valid
        assert any(
            "camera_3d_rtt_count differs" in reason
            for reason in invalid_inventory.reasons
        )

        for behavior_case, expected_rows, expects_save in (
            ("door-state-v1", 5, False),
            ("load-normal-v1", 6, True),
        ):
            behavior_root = root / f"behavior-{behavior_case}"
            behavior_case_value = Case(
                "indoor-light",
                "small",
                "cpu",
                20_260_803,
                None,
                None,
                behavior_case=behavior_case,
            )
            write_behavior_fixture_run(behavior_root, behavior_case_value)

            def validate_behavior_fixture() -> Validation:
                return validate_run(
                    behavior_root,
                    returncode=0,
                    expected_case=behavior_case_value,
                    expected_adapter="Test",
                    expected_backend="vulkan",
                    allow_log_patterns=[],
                    capture_kind="fixed-step-behavior",
                    expected_fixed_hz=64,
                    expected_warmup_ticks=1920,
                    expected_audit_ticks=128,
                    expected_window_backend="headless",
                    expected_contract="rtt-light-v1",
                    expected_stage="current",
                    expected_lane="behavior",
                )

            behavior_validation = validate_behavior_fixture()
            assert behavior_validation.valid, behavior_validation.reasons
            assert len(behavior_validation.timeline or []) == expected_rows
            assert (
                behavior_validation.behavior_save_artifact is not None
            ) == expects_save

            timeline_path = behavior_root / "data" / "timeline.json"
            timeline_payload = json.loads(timeline_path.read_text(encoding="utf-8"))
            timeline_payload["rows"][0]["fixture_checksum"] = "0" * 64
            timeline_path.write_text(
                json.dumps(timeline_payload, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            tampered_timeline = validate_behavior_fixture()
            assert not tampered_timeline.valid
            assert any(
                "wrong fixture_checksum" in reason
                for reason in tampered_timeline.reasons
            )
            timeline_payload["rows"][0]["fixture_checksum"] = rtt_layouts["small"][
                "layout_checksum"
            ]
            timeline_path.write_text(
                json.dumps(timeline_payload, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            assert validate_behavior_fixture().valid

        for size, ledger_rows, presentation_rows, building_records in (
            ("medium", 722, 12, 244),
            ("large", 2306, 12, 902),
        ):
            sized_root = root / f"indoor-sidecar-{size}"
            sized_case = Case(
                "indoor-light", size, "cpu", 20_260_803, None, None
            )
            write_fixture_run(
                sized_root,
                fixed_step_audit=True,
                workload="indoor-light",
                size=size,
                seed=sized_case.seed,
            )
            write_indoor_light_sidecars(sized_root, sized_case)
            sized_validation = validate_run(
                sized_root,
                returncode=0,
                expected_case=sized_case,
                expected_adapter="Test",
                expected_backend="vulkan",
                allow_log_patterns=[],
                capture_kind="fixed-step-determinism",
                expected_fixed_hz=64,
                expected_warmup_ticks=1920,
                expected_audit_ticks=128,
                expected_contract="rtt-light-v1",
                expected_stage="current",
                expected_lane="static",
            )
            assert sized_validation.valid, sized_validation.reasons
            assert len(sized_validation.indoor_light_layout or []) == ledger_rows
            assert (
                len(sized_validation.indoor_light_presentation or [])
                == presentation_rows
            )
            actor_counts = build_fixture_audit_actor_counts(rtt_contract, size)
            assert actor_counts["indoor-building"] == building_records

        records_path = indoor_root / "data" / "determinism_records.csv"
        with records_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            record_fields = reader.fieldnames
            record_rows = list(reader)
        assert record_fields is not None and record_rows
        removed_record = next(
            row
            for row in record_rows
            if row["checkpoint"] == "fixture-pre-update"
            and row["actor_kind"] == "indoor-yard"
            and row["actor_key"] == "1"
        )
        record_rows.remove(removed_record)
        with records_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=record_fields)
            writer.writeheader()
            writer.writerows(record_rows)
        missing_audit_actor = validate_indoor_fixture()
        assert not missing_audit_actor.valid
        assert any(
            "fixture-pre-update has 1 indoor-yard records; expected 2" in reason
            for reason in missing_audit_actor.reasons
        )
        record_rows.append(removed_record)
        record_rows.sort(
            key=lambda row: (
                [
                    "fixture-pre-update",
                    "post-update-1",
                    "post-update-8",
                    "post-update-32",
                    "post-update-128",
                    "post-warmup",
                    "post-audit-end",
                ].index(row["checkpoint"]),
                row["actor_kind"],
                int(row["actor_key"]),
            )
        )
        with records_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=record_fields)
            writer.writeheader()
            writer.writerows(record_rows)

        layout_path = indoor_root / "data" / "indoor_light_layout.csv"
        with layout_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            layout_fields = reader.fieldnames
            layout_rows = list(reader)
        assert layout_fields is not None and layout_rows
        layout_rows[0]["grid_x"] = "999"
        with layout_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=layout_fields)
            writer.writeheader()
            writer.writerows(layout_rows)
        tampered_layout = validate_indoor_fixture()
        assert not tampered_layout.valid
        assert any(
            "indoor_light_layout.csv row 0 grid_x" in reason
            for reason in tampered_layout.reasons
        )
        layout_rows[0]["grid_x"] = "17"
        with layout_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=layout_fields)
            writer.writeheader()
            writer.writerows(layout_rows)

        presentation_path = (
            indoor_root / "data" / "indoor_light_presentation.csv"
        )
        with presentation_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            presentation_fields = reader.fieldnames
            presentation_rows = list(reader)
        assert presentation_fields is not None and presentation_rows
        presentation_rows[1]["child_sprite_count"] = "27"
        with presentation_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=presentation_fields)
            writer.writeheader()
            writer.writerows(presentation_rows)
        tampered_presentation = validate_indoor_fixture()
        assert not tampered_presentation.valid
        assert any(
            "indoor_light_presentation.csv row 1 child_sprite_count" in reason
            for reason in tampered_presentation.reasons
        )
        presentation_rows[1]["child_sprite_count"] = "0"
        with presentation_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=presentation_fields)
            writer.writeheader()
            writer.writerows(presentation_rows)

        fixture_path = indoor_root / "data" / "indoor_light_fixture.csv"
        with fixture_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            fixture_fields = reader.fieldnames
            fixture_rows = list(reader)
        assert fixture_fields is not None and len(fixture_rows) == 1
        fixture_rows[0]["fixture_contract_sha256"] = "0" * 64
        with fixture_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=fixture_fields)
            writer.writeheader()
            writer.writerows(fixture_rows)
        tampered_contract = validate_indoor_fixture()
        assert not tampered_contract.valid
        assert any(
            "fixture_contract_sha256" in reason
            for reason in tampered_contract.reasons
        )

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
        wrong_duration = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
            expected_warmup_secs=30.0,
            expected_measure_secs=60.0,
        )
        assert not wrong_duration.valid
        assert any("below requested" in reason for reason in wrong_duration.reasons)
        frames_path = root / "data" / "frames.csv"
        frames_path.write_text(
            "frame_index,frame_time_ms\n1,nan\n", encoding="utf-8"
        )
        malformed_frames = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not malformed_frames.valid
        assert any("sequential frame_index" in reason for reason in malformed_frames.reasons)
        frames_path.write_text(
            "frame_index,frame_time_ms\n0,1.0\n", encoding="utf-8"
        )
        window_path = root / "data" / "window.csv"
        with window_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            window_fields = reader.fieldnames
            window_rows = list(reader)
        assert window_fields is not None and len(window_rows) == 1
        window_rows[0]["end_scene_target_width"] = "1279"
        with window_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=window_fields)
            writer.writeheader()
            writer.writerows(window_rows)
        unstable_window = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not unstable_window.valid
        assert any("changed scene_target_width" in reason for reason in unstable_window.reasons)
        window_rows[0]["end_scene_target_width"] = "1280"
        window_rows[0]["logical_width"] = "1279.000000"
        window_rows[0]["end_logical_width"] = "1279.000000"
        with window_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=window_fields)
            writer.writeheader()
            writer.writerows(window_rows)
        inconsistent_window = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not inconsistent_window.valid
        assert any(
            "logical_width does not match" in reason
            for reason in inconsistent_window.reasons
        )
        window_rows[0]["logical_width"] = "1280.000000"
        window_rows[0]["end_logical_width"] = "1280.000000"
        with window_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=window_fields)
            writer.writeheader()
            writer.writerows(window_rows)
        summary_path = root / "data" / "summary.csv"
        with summary_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            summary_fields = reader.fieldnames
            summary_rows = list(reader)
        assert summary_fields is not None and len(summary_rows) == 1
        summary_rows[0]["p95_ms"] = "2.0"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=summary_fields)
            writer.writeheader()
            writer.writerows(summary_rows)
        mismatched_quantile = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not mismatched_quantile.valid
        assert any("frames.csv computes" in reason for reason in mismatched_quantile.reasons)
        summary_rows[0]["p95_ms"] = "1.0"
        summary_rows[0]["candidate_membership_checks"] = "not-a-counter"
        with summary_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=summary_fields)
            writer.writeheader()
            writer.writerows(summary_rows)
        malformed_counter = validate_run(
            root,
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
        )
        assert not malformed_counter.valid
        assert any(
            "candidate_membership_checks must be a nonnegative integer" in reason
            for reason in malformed_counter.reasons
        )
        tracy_zones = root / "tracy-zones.csv"
        tracy_zones.write_text(
            "name,src_file,src_line,total_ns,total_perc,counts,mean_ns,min_ns,max_ns,std_ns\n"
            "task_list_update_system,update.rs,20,3000,1.0,3,1000,900,1100,10\n",
            encoding="utf-8",
        )
        zone_summary, zone_errors = read_tracy_zone_summary(tracy_zones)
        assert not zone_errors and zone_summary["mean_ns_per_invocation"] == 1000.0
        memory_csv = root / "memory.csv"
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,4096,2048,8192,7144,8,7,2,0\n",
            encoding="utf-8",
        )
        memory_summary, memory_errors = read_native_memory(
            memory_csv, frame_samples=2
        )
        assert not memory_errors
        assert memory_summary["peak_live_bytes"] == 4096
        assert memory_summary["peak_growth_bytes"] == 3096
        assert memory_summary["net_live_growth_bytes"] == 1048
        assert memory_summary["allocated_bytes_per_frame"] == 4096.0
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,900,1100,8192,8092,8,7,2,0\n",
            encoding="utf-8",
        )
        _, malformed_memory_errors = read_native_memory(memory_csv, frame_samples=1)
        assert malformed_memory_errors == [
            "native memory artifact contains invalid values"
        ]
        memory_csv.write_text(
            "schema_version,baseline_live_bytes,peak_live_bytes,final_live_bytes,"
            "allocated_bytes,deallocated_bytes,allocation_calls,deallocation_calls,"
            "reallocation_calls,accounting_errors\n"
            "1,1000,4096,2048,8192,7144,8,7,9,0\n",
            encoding="utf-8",
        )
        _, malformed_reallocation_errors = read_native_memory(
            memory_csv, frame_samples=1
        )
        assert malformed_reallocation_errors == [
            "native memory artifact contains invalid values"
        ]
        fixed_profile, fixed_profile_errors = collect_profile_artifact(
            args=argparse.Namespace(
                instrumentation="capture",
                capture_kind="fixed-step-determinism",
            ),
            case=Case("task-dashboard", "small", "cpu", DEFAULT_SEED, None, None),
            run_dir=root,
            trace_returncode=None,
            frame_samples=None,
            environment={},
        )
        assert fixed_profile is None
        assert not fixed_profile_errors
        dashboard_cpu = root / "task_dashboard_cpu.csv"
        dashboard_cpu.write_text(
            "schema_version,system_invocations,total_elapsed_ns\n1,4,1000\n",
            encoding="utf-8",
        )
        cpu_summary, cpu_errors = read_task_dashboard_cpu(dashboard_cpu)
        assert not cpu_errors
        assert cpu_summary["mean_ns_per_invocation"] == 250.0
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

        rows[0]["initial_state_checksum"] = "0000000000000000"
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
        wrong_rate = validate_run(
            fixed_run_dirs[0],
            returncode=0,
            expected_case=case,
            expected_adapter="Test",
            expected_backend="vulkan",
            allow_log_patterns=[],
            capture_kind="fixed-step-determinism",
            expected_fixed_hz=32,
            expected_warmup_ticks=1920,
            expected_audit_ticks=128,
        )
        assert not wrong_rate.valid
        assert any("for 32 Hz" in reason for reason in wrong_rate.reasons)
        record_path = fixed_run_dirs[0] / "data" / "determinism_records.csv"
        hidden_record_path = fixed_run_dirs[0] / "data" / "determinism_records.hidden"
        record_path.rename(hidden_record_path)
        missing_records = validate_run(
            fixed_run_dirs[0],
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
        assert not missing_records.valid
        assert any("missing determinism_records.csv" in reason for reason in missing_records.reasons)
        hidden_record_path.rename(record_path)
        with record_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            record_fields = reader.fieldnames
            record_rows = list(reader)
        assert record_fields is not None and record_rows
        original_record_hex = record_rows[0]["record_hex"]
        record_rows[0]["record_hex"] = "00"
        with record_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=record_fields)
            writer.writeheader()
            writer.writerows(record_rows)
        mismatched_records = validate_run(
            fixed_run_dirs[0],
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
        assert not mismatched_records.valid
        assert any(
            "computes state_checksum" in reason
            for reason in mismatched_records.reasons
        )
        record_rows[0]["record_hex"] = original_record_hex
        with record_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=record_fields)
            writer.writeheader()
            writer.writerows(record_rows)
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
        variant_record_path = (
            fixed_run_dirs[1] / "data" / "determinism_records.csv"
        )
        with variant_record_path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            variant_record_fields = reader.fieldnames
            variant_record_rows = list(reader)
        assert variant_record_fields is not None and variant_record_rows
        variant_checkpoint = checkpoints[1]["checkpoint"]
        for record in variant_record_rows:
            if record["checkpoint"] == variant_checkpoint:
                record["record_hex"] = "00"
        with variant_record_path.open("w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=variant_record_fields)
            writer.writeheader()
            writer.writerows(variant_record_rows)
        checkpoints[1]["state_checksum"] = determinism_records_checksum([b"\x00"])
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

        controlled_session = root / "controlled-session"
        controlled_cases = [
            Case(
                "gather",
                "small",
                "cpu",
                DEFAULT_SEED,
                None,
                None,
                familiar_policy,
                operation_dialog,
            )
            for familiar_policy in ("default", "disabled")
            for operation_dialog in ("hidden", "open")
        ]
        default_work = {
            "delegation_cycles": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "8",
            "policy_disabled_rejections": "0",
            "candidate_snapshot_attempts": "8",
            "candidate_score_attempts": "8",
            "worker_score_attempts": "8",
            "source_selector_calls": "1",
            "source_selector_scanned_items": "1",
            "reachable_with_cache_calls": "1",
        }
        disabled_work = {
            "delegation_cycles": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "8",
            "policy_disabled_rejections": "8",
            "candidate_snapshot_attempts": "0",
            "candidate_score_attempts": "0",
            "worker_score_attempts": "0",
            "source_selector_calls": "0",
            "source_selector_scanned_items": "0",
            "reachable_with_cache_calls": "0",
        }
        for case in controlled_cases:
            run_dir = controlled_session / "cases" / case.identifier / "run-001"
            write_fixture_run(
                run_dir,
                fixed_step_audit=True,
                familiar_policy=case.familiar_policy,
                operation_dialog=case.operation_dialog,
                controlled_work=(
                    default_work
                    if case.familiar_policy == "default"
                    else disabled_work
                ),
                determinism_record_payload=(
                    "default-policy"
                    if case.familiar_policy == "default"
                    else "disabled-policy"
                ),
            )
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
            controlled_session / "manifest.json",
            {
                "matrix": {
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                    "familiar_policies": ["default", "disabled"],
                    "operation_dialog_modes": ["hidden", "open"],
                },
                "cases": [
                    asdict(case) | {"id": case.identifier}
                    for case in controlled_cases
                ],
                "actual_adapters": [],
            },
        )
        assert summarize_session(controlled_session)
        controlled_result = json.loads(
            (controlled_session / "familiar_policy_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert controlled_result["status"] == "pass"

        dashboard_session = root / "dashboard-session"
        dashboard_cases = [
            Case(
                "task-dashboard",
                "small",
                "cpu",
                DEFAULT_SEED,
                None,
                None,
                dashboard_mode=mode,
            )
            for mode in ("hidden", "visible", "active-filter")
        ]
        dashboard_common_work = {
            "delegation_cycles": "2",
            "incoming_snapshot_builds": "2",
            "delegation_familiars_processed": "8",
            "candidate_membership_checks": "16",
            "policy_disabled_rejections": "0",
            "candidate_snapshot_attempts": "16",
            "candidate_score_attempts": "16",
            "worker_score_attempts": "16",
            "top_k_partition_runs": "4",
            "top_k_retained_candidates": "16",
            "top_k_fallback_candidates": "0",
            "source_selector_calls": "1",
            "source_selector_cache_build_scanned_items": "4",
            "source_selector_candidate_scanned_items": "2",
            "source_selector_scanned_items": "6",
            "reachable_with_cache_calls": "8",
            "wheelbarrow_arbitration_rebuilds": "2",
            "wheelbarrow_request_bucket_builds": "1",
            "wheelbarrow_bucket_items_scanned": "3",
            "wheelbarrow_candidates_after_top_k": "3",
            "runtime_path_total_core_searches": "4",
            "runtime_path_actor_new_core_searches": "4",
            "dashboard_state_rebuilds": "2",
            "dashboard_snapshot_rows_scanned": "130",
            "dashboard_summary_rows_scanned": "130",
            "dashboard_snapshot_changes": "2",
            "dashboard_summary_changes": "2",
        }
        dashboard_render_work = {
            "hidden": {},
            "visible": {
                "dashboard_render_rebuilds": "2",
                "dashboard_render_input_rows": "130",
                "dashboard_render_visible_rows": "130",
                "dashboard_render_group_headers": "6",
                "dashboard_despawn_roots_requested": "20",
            },
            "active-filter": {
                "dashboard_render_rebuilds": "2",
                "dashboard_render_input_rows": "130",
                "dashboard_render_visible_rows": "64",
                "dashboard_render_group_headers": "2",
                "dashboard_despawn_roots_requested": "20",
            },
        }
        for case in dashboard_cases:
            run_dir = dashboard_session / "cases" / case.identifier / "run-001"
            write_fixture_run(
                run_dir,
                fixed_step_audit=True,
                workload="task-dashboard",
                dashboard_mode=case.dashboard_mode,
                controlled_work=(
                    dashboard_common_work | dashboard_render_work[case.dashboard_mode]
                ),
            )
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
            dashboard_session / "manifest.json",
            {
                "matrix": {
                    "workload": "task-dashboard",
                    "capture_kind": "fixed-step-determinism",
                    "fixed_hz": 64,
                    "warmup_ticks": 1920,
                    "audit_ticks": 128,
                    "dashboard_modes": ["hidden", "visible", "active-filter"],
                },
                "cases": [
                    asdict(case) | {"id": case.identifier} for case in dashboard_cases
                ],
                "actual_adapters": [],
            },
        )
        assert summarize_session(dashboard_session)
        dashboard_result = json.loads(
            (dashboard_session / "dashboard_mode_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert dashboard_result["status"] == "pass"

        dashboard_cost_session = root / "dashboard-cost-session"
        for case in dashboard_cases:
            mode_overrides = {
                "hidden": {
                    "dashboard_render_rebuilds": "0",
                    "dashboard_render_input_rows": "0",
                    "dashboard_render_visible_rows": "0",
                },
                "visible": {
                    "dashboard_render_rebuilds": "2",
                    "dashboard_render_input_rows": "130",
                    "dashboard_render_visible_rows": "130",
                },
                "active-filter": {
                    # Realtime modes may rebuild a different number of times in
                    # the measurement window. The comparator must normalize row
                    # work per rebuild instead of comparing cumulative totals.
                    "dashboard_render_rebuilds": "3",
                    "dashboard_render_input_rows": "195",
                    "dashboard_render_visible_rows": "96",
                },
            }[case.dashboard_mode]
            for run_number in range(1, 4):
                run_dir = (
                    dashboard_cost_session
                    / "cases"
                    / case.identifier
                    / f"run-{run_number:03d}"
                )
                write_fixture_run(
                    run_dir,
                    dashboard_mode=case.dashboard_mode,
                    workload="task-dashboard",
                    summary_overrides=mode_overrides,
                )
                validation = validate_run(
                    run_dir,
                    returncode=0,
                    expected_case=case,
                    expected_adapter="Test",
                    expected_backend="vulkan",
                    allow_log_patterns=[],
                )
                assert validation.valid, validation.reasons
                validation.profile_artifact = {
                    "instrumentation": "capture",
                    "task_dashboard_cpu": {
                        "source": "fixture",
                        "invocations": 2,
                        "total_ns": 2000,
                        "mean_ns_per_invocation": 1000.0,
                    },
                }
                write_json(run_dir / "validation.json", validation.to_json())
        write_json(
            dashboard_cost_session / "manifest.json",
            {
                "schema_version": SESSION_MANIFEST_SCHEMA_VERSION,
                "matrix": {
                    "workload": "task-dashboard",
                    "capture_kind": "frame-time",
                    "dashboard_modes": ["hidden", "visible", "active-filter"],
                    "warmup_checksum_policy": "record",
                    "measure_end_checksum_policy": "record",
                    "repeat": 3,
                    "preflight_runs": 0,
                },
                "cases": [
                    asdict(case) | {"id": case.identifier} for case in dashboard_cases
                ],
                "actual_adapters": [{"name": "Test GPU", "backend": "Vulkan"}],
                "requested_environment": {"WGPU_BACKEND": "vulkan"},
                "binary": {"instrumentation": "capture"},
            },
        )
        assert summarize_session(dashboard_cost_session)
        capture_report = (dashboard_cost_session / "report.md").read_text(
            encoding="utf-8"
        )
        assert "## Frame-time aggregate" in capture_report
        assert "diagnostic only" not in capture_report
        assert (
            compare_dashboard_modes(
                argparse.Namespace(
                    session=str(dashboard_cost_session), min_runs=3, output=None
                )
            )
            == 0
        )
        capture_costs = json.loads(
            (dashboard_cost_session / "dashboard_mode_cost_comparison.json").read_text(
                encoding="utf-8"
            )
        )
        assert "p50_median_ms" in capture_costs["groups"][0]["modes"]["hidden"]

        memory_manifest = json.loads(
            (dashboard_cost_session / "manifest.json").read_text(encoding="utf-8")
        )
        memory_manifest["binary"]["instrumentation"] = "memory"
        write_json(dashboard_cost_session / "manifest.json", memory_manifest)
        for case in dashboard_cases:
            for run_number in range(1, 4):
                validation_path = (
                    dashboard_cost_session
                    / "cases"
                    / case.identifier
                    / f"run-{run_number:03d}"
                    / "validation.json"
                )
                validation_payload = json.loads(
                    validation_path.read_text(encoding="utf-8")
                )
                validation_payload["profile_artifact"] = {
                    "instrumentation": "memory",
                    "allocation_memory": {
                        "allocation_calls_per_frame": 4.0,
                        "allocated_bytes_per_frame": 4096.0,
                        "peak_live_bytes": 8192,
                        "peak_growth_bytes": 2048,
                    },
                    "process_memory": {"max_rss_kib": 1024},
                }
                write_json(validation_path, validation_payload)
        assert summarize_session(dashboard_cost_session)
        memory_report = (dashboard_cost_session / "report.md").read_text(
            encoding="utf-8"
        )
        assert "## Instrumented frame timing (diagnostic only)" in memory_report
        assert "must not be used for mode or baseline comparison" in memory_report
        memory_comparison = dashboard_cost_session / "memory-comparison.json"
        assert (
            compare_dashboard_modes(
                argparse.Namespace(
                    session=str(dashboard_cost_session),
                    min_runs=3,
                    output=str(memory_comparison),
                )
            )
            == 0
        )
        memory_costs = json.loads(memory_comparison.read_text(encoding="utf-8"))
        hidden_memory = memory_costs["groups"][0]["modes"]["hidden"]
        assert hidden_memory["allocation_peak_live_bytes_median"] == 8192
        assert "p50_median_ms" not in hidden_memory

        audit_args = build_parser().parse_args(["audit", "--dry-run"])
        assert audit_args.clock_mode == "fixed"
        assert audit_args.capture_kind == "fixed-step-determinism"
        assert audit_args.fixed_hz == 64
        assert audit_args.warmup_ticks == 1920
        assert audit_args.audit_ticks == 128
        assert audit_args.familiar_policies == "baseline"
        assert audit_args.operation_dialog_modes == "hidden"
        indoor_args = build_parser().parse_args(
            [
                "audit",
                "--dry-run",
                "--workload",
                "indoor-light",
                "--contract",
                "rtt-light-v1",
                "--stage",
                "current",
                "--lane",
                "static",
                "--sizes",
                "small",
                "--seed",
                "20260803",
            ]
        )
        validate_arguments(indoor_args)
        missing_indoor_selection = build_parser().parse_args(
            [
                "audit",
                "--dry-run",
                "--workload",
                "indoor-light",
                "--sizes",
                "small",
                "--seed",
                "20260803",
            ]
        )
        try:
            validate_arguments(missing_indoor_selection)
        except ValueError as error:
            assert "requires --contract rtt-light-v1" in str(error)
        else:
            raise AssertionError("indoor-light without a selection unexpectedly passed")
        supported_indoor_size = build_parser().parse_args(
            [
                "audit",
                "--dry-run",
                "--workload",
                "indoor-light",
                "--contract",
                "rtt-light-v1",
                "--stage",
                "current",
                "--lane",
                "static",
                "--sizes",
                "medium",
                "--seed",
                "20260803",
            ]
        )
        validate_arguments(supported_indoor_size)
        rejected_headless_gpu = build_parser().parse_args(
            ["run", "--dry-run", "--window-backend", "headless", "--renders", "gpu"]
        )
        try:
            validate_arguments(rejected_headless_gpu)
        except ValueError as error:
            assert "headless only supports --renders cpu" in str(error)
        else:
            raise AssertionError("headless GPU capture unexpectedly passed validation")
        rejected_headless_window = build_parser().parse_args(
            [
                "run",
                "--dry-run",
                "--window-backend",
                "headless",
                "--window-width",
                "1920",
                "--window-height",
                "1080",
            ]
        )
        try:
            validate_arguments(rejected_headless_window)
        except ValueError as error:
            assert "not applicable" in str(error)
        else:
            raise AssertionError("headless window dimensions unexpectedly passed validation")
        rejected_nan_threshold = build_parser().parse_args(
            [
                "compare",
                "--baseline",
                "/tmp/baseline",
                "--candidate",
                "/tmp/candidate",
                "--max-regression-pct",
                "nan",
            ]
        )
        try:
            validate_arguments(rejected_nan_threshold)
        except ValueError as error:
            assert "finite and nonnegative" in str(error)
        else:
            raise AssertionError("NaN comparison threshold unexpectedly passed validation")
        rejected_partial_trace = build_parser().parse_args(
            [
                "run",
                "--instrumentation",
                "tracy",
                "--tracy-capture-binary",
                "/tmp/capture",
                "--tracy-csvexport-binary",
                "/tmp/csvexport",
                "--tracy-capture-secs",
                "1",
            ]
        )
        try:
            validate_arguments(rejected_partial_trace)
        except ValueError as error:
            assert "measure-artifact boundary" in str(error)
        else:
            raise AssertionError("partial Tracy capture unexpectedly passed validation")

        def write_comparison_fixture(
            session_dir: Path, *, sizes: list[str], renders: list[str], case_ids: list[str]
        ) -> None:
            session_dir.mkdir()
            write_json(
                session_dir / "manifest.json",
                {
                    "schema_version": SESSION_MANIFEST_SCHEMA_VERSION,
                    "status": "valid",
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
                    "cases": [{"id": case_id} for case_id in case_ids],
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
        candidate_manifest_path = comparison_candidate / "manifest.json"
        candidate_manifest = json.loads(candidate_manifest_path.read_text(encoding="utf-8"))
        candidate_manifest["status"] = "invalid"
        write_json(candidate_manifest_path, candidate_manifest)
        try:
            compare_sessions(comparison_args)
        except RuntimeError as error:
            assert "status is not valid" in str(error)
        else:
            raise AssertionError("invalid session status unexpectedly compared successfully")
        candidate_manifest["status"] = "valid"
        write_json(candidate_manifest_path, candidate_manifest)
        candidate_aggregate_path = comparison_candidate / "aggregate.csv"
        candidate_aggregate_path.write_text(
            "case_id,valid_runs,p50_median_ms\n"
            f"{large_cpu_case},3,nan\n",
            encoding="utf-8",
        )
        try:
            compare_sessions(comparison_args)
        except RuntimeError as error:
            assert "must be finite" in str(error)
        else:
            raise AssertionError("NaN aggregate unexpectedly compared successfully")
        candidate_aggregate_path.write_text(
            "case_id,valid_runs,p50_median_ms\n"
            f"{large_cpu_case},3,1.0\n",
            encoding="utf-8",
        )

        exact_session = root / "exact-session"
        exact_case = Case("gather", "small", "cpu", DEFAULT_SEED, None, None)
        exact_case_dir = exact_session / "cases" / exact_case.identifier
        exact_manifest = {
            "schema_version": SESSION_MANIFEST_SCHEMA_VERSION,
            "matrix": {"repeat": 2, "preflight_runs": 1},
            "cases": [asdict(exact_case) | {"id": exact_case.identifier}],
        }
        for label in ("preflight-001", "run-001", "run-002"):
            (exact_case_dir / label).mkdir(parents=True)
            write_json(
                exact_case_dir / label / "validation.json",
                {
                    "valid": True,
                    "reasons": [],
                },
            )
        assert not validate_session_artifact_set(exact_session, exact_manifest)

        write_json(
            exact_case_dir / "preflight-001" / "validation.json",
            {"valid": False, "reasons": ["fixture mismatch"]},
        )
        exact_errors = validate_session_artifact_set(exact_session, exact_manifest)
        assert exact_errors == [
            f"{exact_case.identifier}/preflight-001 failed: fixture mismatch"
        ]
        shutil.rmtree(exact_case_dir / "run-002")
        exact_errors = validate_session_artifact_set(exact_session, exact_manifest)
        assert any("missing runs: run-002" in error for error in exact_errors)
    print("perf.py self-test: pass")
    return 0
