from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from ..artifact_io import read_exact_csv_rows, sha256
from ..model import (
    Case, REPO_ROOT, WALL_DENSITY_CASES, WALL_DENSITY_CONTRACT_SHA256,
    WALL_DENSITY_LAYOUT_COLUMNS,
)

def read_wall_density_sidecars(
    data_dir: Path,
    *,
    expected_case: Case,
) -> tuple[dict[str, Any] | None, list[dict[str, str]] | None, list[str]]:
    """Validate the exact wall-density contract independently of the Rust fixture."""
    errors: list[str] = []
    phase = expected_case.wall_phase
    expected = WALL_DENSITY_CASES.get((expected_case.size, phase))
    if expected is None:
        return None, None, ["wall-density case size/phase is outside the frozen matrix"]
    target_size, target_count, connector_count, repetitions, layout_checksum = expected

    contract_path = REPO_ROOT / "tools/blender_ai_workflow/fixtures/wall-density-v1.json"
    if not contract_path.is_file():
        errors.append("wall-density contract file is missing")
    elif sha256(contract_path) != WALL_DENSITY_CONTRACT_SHA256:
        errors.append("wall-density contract hash differs from the frozen M0 contract")

    summary_path = data_dir / "wall_density_fixture.json"
    try:
        fixture = json.loads(summary_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return None, None, errors + [f"cannot parse wall_density_fixture.json: {error}"]
    expected_keys = {
        "schema_version", "contract_id", "contract_sha256", "layout_checksum",
        "target_size", "perf_size", "phase", "target_wall_count",
        "connector_count", "mask_counts", "grid", "camera_scale",
    }
    if not isinstance(fixture, dict) or set(fixture) != expected_keys:
        return None, None, errors + ["wall_density_fixture.json keys differ from schema v1"]
    expected_fixture = {
        "schema_version": 1,
        "contract_id": "wall-density-v1",
        "contract_sha256": WALL_DENSITY_CONTRACT_SHA256,
        "layout_checksum": layout_checksum,
        "target_size": target_size,
        "perf_size": expected_case.size,
        "phase": phase,
        "target_wall_count": target_count,
        "connector_count": connector_count,
        "mask_counts": {f"{mask:04b}": repetitions for mask in range(16)},
        "grid": {"origin": [2, 2], "stride": [5, 5], "columns": 20},
        "camera_scale": 5.0,
    }
    if fixture != expected_fixture:
        errors.append("wall_density_fixture.json values differ from the frozen matrix")

    rows, row_errors = read_exact_csv_rows(
        data_dir / "wall_density_layout.csv",
        columns=WALL_DENSITY_LAYOUT_COLUMNS,
        artifact_name="wall_density_layout.csv",
    )
    errors.extend(row_errors)
    if rows is None:
        return fixture, None, errors

    expected_rows: list[dict[str, str]] = []
    connector_ordinal = 0
    directions = (
        (0b1000, "N", 0, 1),
        (0b0100, "S", 0, -1),
        (0b0010, "W", -1, 0),
        (0b0001, "E", 1, 0),
    )
    for ordinal in range(target_count):
        grid_x = 2 + (ordinal % 20) * 5
        grid_y = 2 + (ordinal // 20) * 5
        mask = ordinal % 16
        common = {
            "schema_version": "1",
            "target_ordinal": str(ordinal),
            "mask": f"{mask:04b}",
            "phase": phase,
        }
        expected_rows.append({
            **common,
            "record_kind": "target",
            "ordinal": str(ordinal),
            "grid_x": str(grid_x),
            "grid_y": str(grid_y),
            "direction": "",
        })
        for bit, direction, offset_x, offset_y in directions:
            if mask & bit == 0:
                continue
            expected_rows.append({
                **common,
                "record_kind": "connector",
                "ordinal": str(connector_ordinal),
                "grid_x": str(grid_x + offset_x),
                "grid_y": str(grid_y + offset_y),
                "direction": direction,
            })
            connector_ordinal += 1
    if rows != expected_rows:
        errors.append("wall_density_layout.csv rows differ from the frozen row-major layout")
    return fixture, rows, errors
