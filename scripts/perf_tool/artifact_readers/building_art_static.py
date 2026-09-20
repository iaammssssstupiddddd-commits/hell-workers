"""Independent paused-building fixture oracle; never approves active simulation."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from ..model import Case

KINDS = ("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking",
         "SandPile", "BonePile", "Door", "OutdoorLamp", "Bridge")


def expected_records(copies: int) -> list[dict]:
    records = []
    for row, kind in enumerate(KINDS):
        for ordinal in range(copies):
            x, y = 8 + ordinal * 5, 65 if kind == "Bridge" else 8 + row * 5
            quarter = ordinal % 4
            if kind == "Bridge":
                offsets = [[dx, dy] for dy in range(5) for dx in range(2)]
                center_offset = (0.5, 2)
            elif kind == "SoulSpa":
                offsets = [[0, 0], [1, 0], [0, -1], [1, -1]]
                center_offset = (0.5, -0.5)
            elif kind in {"Tank", "MudMixer", "RestArea", "WheelbarrowParking"}:
                offsets = [[0, 0], [1, 0], [0, 1], [1, 1]]
                center_offset = (0.5, 0.5)
            else:
                offsets = [[0, 0]]
                center_offset = (0, 0)
            state = {"state": "Static"}
            if kind == "Tank":
                state = {"stored_water": (0, 25, 50, 50)[quarter], "capacity": 50}
            elif kind == "MudMixer":
                state = {"refining": quarter >= 2}
            elif kind == "RestArea":
                state = {"occupants": (0, 1, 5, 0)[quarter]}
            elif kind == "SoulSpa":
                state = {"operational": True, "mask": (0, 1, 3, 15)[quarter]}
            elif kind == "OutdoorLamp":
                state = {"powered": quarter >= 2}
            elif kind == "Door":
                state = {"state": "Closed", "axis": "EastWest"}
            records.append({"kind": kind, "ordinal": ordinal, "anchor": [x, y],
                            "tiles": [[x + dx, y + dy] for dx, dy in offsets],
                            "center": [float((x - 49.5 + center_offset[0]) * 32),
                                       float((y - 49.5 + center_offset[1]) * 32)], "state": state})
    return records


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key}")
        result[key] = value
    return result


def reject_constant(value):
    raise ValueError(f"nonfinite JSON number {value}")


def read_building_art_static(data_dir: Path, *, expected_case: Case) -> tuple[dict | None, list[str]]:
    try:
        value = json.loads((data_dir / "building_art_static.json").read_text(),
                           object_pairs_hook=unique_object, parse_constant=reject_constant)
        copies = {"small": 4, "medium": 16}[expected_case.size]
        if expected_case != Case("building-art-static", expected_case.size, "gpu", 20260920, copies // 4 * 15, 0):
            raise ValueError("case differs from frozen static contract")
        expected = {"records": expected_records(copies), "target_count": copies * 10,
                    "target_structural_roots": copies * 6, "target_foreground_owners": copies * 4,
                    "target_active_unique_meshes": 3, "souls": copies // 4 * 15}
        keys = {"schema_version", "contract_id", "evidence_kind", "active_simulation_evidence",
                "camera_scale", "stable_frames", "layout_sha256", "initial", "final"}
        if not isinstance(value, dict) or set(value) != keys:
            raise ValueError("sidecar keys differ")
        if (type(value["schema_version"]) is not int or value["schema_version"] != 1
                or value["contract_id"] != "building-art-static-v1"
                or value["evidence_kind"] != "paused-static-only"
                or value["active_simulation_evidence"] is not False
                or value["camera_scale"] != 5.0
                or type(value["stable_frames"]) is not int or value["stable_frames"] < 2):
            raise ValueError("static identity/coverage differs")
        # Canonical bytes distinguish booleans from integers as well as rejecting
        # duplicate/missing owners and self-consistent but incorrect final data.
        def canonical(obj):
            return json.dumps(obj, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        expected_bytes = canonical(expected)
        if canonical(value["initial"]) != expected_bytes or canonical(value["final"]) != expected_bytes:
            raise ValueError("state/layout/counts differ from independent fixture oracle")
        if value["layout_sha256"] != hashlib.sha256(expected_bytes).hexdigest():
            raise ValueError("layout digest differs")
        return value, []
    except (OSError, UnicodeError, ValueError, TypeError, KeyError) as error:
        return None, [f"invalid building_art_static.json: {error}"]
