from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

from ..artifact_io import read_exact_csv_rows
from ..model import Case, DOOR_DENSITY_LAYOUT_COLUMNS

DOOR_DENSITY_CONTRACT_SHA256 = (
    "30319cb86471a18a338de3941ee1103646078737c91844bebe7ae40134e95928"
)


def expected_layout_checksum(target_count: int) -> str:
    contract = (
        b"door-density-v1\nseed=20260906\norigin=12,12\nstride=5,5\ncolumns=8\n"
        b"small=32\nmedium=128\naxis=even-ew,odd-ns\nstate=closed,open,locked\n"
        b"support_walls=2\ncamera_scale=5\n"
    )
    digest = hashlib.sha256()
    digest.update(contract)
    digest.update(("N" if target_count == 32 else "4N").encode())
    for ordinal in range(target_count):
        grid_x = 12 + (ordinal % 8) * 5
        grid_y = 12 + (ordinal // 8) * 5
        axis = "east_west" if ordinal % 2 == 0 else "north_south"
        state = ("closed", "open", "locked")[ordinal % 3]
        digest.update(ordinal.to_bytes(4, "little"))
        digest.update(grid_x.to_bytes(4, "little", signed=True))
        digest.update(grid_y.to_bytes(4, "little", signed=True))
        digest.update(axis.encode())
        digest.update(state.encode())
        supports = (
            ((grid_x - 1, grid_y), (grid_x + 1, grid_y))
            if axis == "east_west"
            else ((grid_x, grid_y - 1), (grid_x, grid_y + 1))
        )
        for support_x, support_y in supports:
            digest.update(support_x.to_bytes(4, "little", signed=True))
            digest.update(support_y.to_bytes(4, "little", signed=True))
    return digest.hexdigest()


def read_door_density_sidecars(
    data_dir: Path,
    *,
    expected_case: Case,
) -> tuple[dict[str, Any] | None, list[dict[str, str]] | None, list[str]]:
    """Validate the frozen Door-density layout and finite presentation pool."""
    errors: list[str] = []
    target_count = {"small": 32, "medium": 128}.get(expected_case.size)
    if target_count is None:
        return None, None, ["door-density case size is outside the frozen matrix"]
    summary_path = data_dir / "door_density_fixture.json"
    try:
        fixture = json.loads(summary_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, None, [f"cannot parse door_density_fixture.json: {error}"]
    expected_keys = {
        "schema_version", "contract_id", "contract_sha256", "layout_checksum",
        "target_size", "perf_size", "target_door_count", "support_wall_count",
        "grid", "camera_scale", "stable", "initial", "final",
    }
    if not isinstance(fixture, dict) or set(fixture) != expected_keys:
        return None, None, ["door_density_fixture.json keys differ from schema v1"]
    if (
        fixture.get("schema_version") != 1
        or fixture.get("contract_id") != "door-density-v1"
        or fixture.get("contract_sha256") != DOOR_DENSITY_CONTRACT_SHA256
        or fixture.get("layout_checksum") != expected_layout_checksum(target_count)
        or fixture.get("target_size") != ("N" if target_count == 32 else "4N")
        or fixture.get("perf_size") != expected_case.size
        or fixture.get("target_door_count") != target_count
        or fixture.get("support_wall_count") != target_count * 2
        or fixture.get("grid") != {"origin": [12, 12], "stride": [5, 5], "columns": 8}
        or fixture.get("camera_scale") != 5.0
        or fixture.get("stable") is not True
        or fixture.get("initial") != fixture.get("final")
    ):
        errors.append("door_density_fixture.json values differ from the frozen matrix")
    presentation = fixture.get("initial")
    if not isinstance(presentation, dict):
        errors.append("door_density_fixture.json presentation evidence is absent")
    else:
        mode = presentation.get("expected_mode")
        expected_production = target_count if mode == "production" else 0
        expected_fallback = target_count if mode == "fallback-control" else 0
        expected_active = 3 if mode == "production" else 1
        if (
            mode not in {"production", "fallback-control"}
            or presentation.get("target_door_count") != target_count
            or presentation.get("support_wall_count") != target_count * 2
            or presentation.get("visual_count") != target_count
            or presentation.get("production_count") != expected_production
            or presentation.get("fallback_count") != expected_fallback
            or presentation.get("active_mesh_count") != expected_active
            or presentation.get("active_material_count") != (1 if mode == "production" else 3)
            or presentation.get("resident_production_mesh_count") != 3
            or presentation.get("resident_production_material_count") != 1
            or presentation.get("resident_fallback_mesh_count") != 1
            or presentation.get("resident_fallback_material_count") != 3
            or presentation.get("resident_production_image_count") != 3
            or presentation.get("state_counts")
            != {
                "closed": (target_count + 2) // 3,
                "open": (target_count + 1) // 3,
                "locked": target_count // 3,
            }
            or presentation.get("axis_counts")
            != {"east_west": target_count // 2, "north_south": target_count // 2}
        ):
            errors.append("door_density_fixture.json presentation pool differs")

    rows, row_errors = read_exact_csv_rows(
        data_dir / "door_density_layout.csv",
        columns=DOOR_DENSITY_LAYOUT_COLUMNS,
        artifact_name="door_density_layout.csv",
    )
    errors.extend(row_errors)
    if rows is None:
        return fixture, None, errors
    expected_rows: list[dict[str, str]] = []
    support_ordinal = 0
    for ordinal in range(target_count):
        grid_x = 12 + (ordinal % 8) * 5
        grid_y = 12 + (ordinal // 8) * 5
        axis = "east_west" if ordinal % 2 == 0 else "north_south"
        state = ("closed", "open", "locked")[ordinal % 3]
        common = {
            "schema_version": "1",
            "target_ordinal": str(ordinal),
            "axis": axis,
            "state": state,
        }
        expected_rows.append({
            **common,
            "record_kind": "target",
            "ordinal": str(ordinal),
            "grid_x": str(grid_x),
            "grid_y": str(grid_y),
        })
        supports = (
            ((grid_x - 1, grid_y), (grid_x + 1, grid_y))
            if axis == "east_west"
            else ((grid_x, grid_y - 1), (grid_x, grid_y + 1))
        )
        for support_x, support_y in supports:
            expected_rows.append({
                **common,
                "record_kind": "support",
                "ordinal": str(support_ordinal),
                "grid_x": str(support_x),
                "grid_y": str(support_y),
            })
            support_ordinal += 1
    if rows != expected_rows:
        errors.append("door_density_layout.csv rows differ from the frozen row-major layout")
    return fixture, rows, errors
