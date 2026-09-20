"""Fail-closed active reference oracle. Idle flags or one-shot production cannot pass."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

from .building_art_static import expected_records, unique_object, reject_constant
from ..model import Case


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def natural(value):
    return type(value) is int and value >= 0


def read_building_art_active(data_dir: Path, *, expected_case: Case) -> tuple[dict | None, list[str]]:
    try:
        value = json.loads((data_dir / "building_art_active.json").read_text(),
                           object_pairs_hook=unique_object, parse_constant=reject_constant)
        copies = {"small": 4, "medium": 16}[expected_case.size]
        if expected_case != Case("building-art-active", expected_case.size, "gpu", 20260920, copies // 4 * 29, copies // 2):
            raise ValueError("case differs from frozen active contract")
        keys = {"schema_version", "contract_id", "initialized", "failed", "frames", "seconds",
                "particles_max", "particles_sum", "ui_particles_max", "ui_particles_sum",
                "initial", "layout_sha256", "lanes"}
        if not isinstance(value, dict) or set(value) != keys:
            raise ValueError("sidecar keys differ")
        if (type(value["schema_version"]) is not int or value["schema_version"] != 1
                or value["contract_id"] != "building-art-active-nine-v1"
                or value["initialized"] is not True or value["failed"] is not False
                or not natural(value["frames"]) or value["frames"] < 240
                or type(value["seconds"]) not in (int, float) or not math.isfinite(value["seconds"])
                or not 60 <= value["seconds"] <= 60.25
                or not natural(value["particles_max"]) or not 0 < value["particles_max"] <= copies // 4 * 96
                or not natural(value["particles_sum"]) or value["particles_sum"] < value["particles_max"]
                or value["particles_sum"] > value["frames"] * value["particles_max"]
                or not natural(value["ui_particles_max"]) or not 0 < value["ui_particles_max"] <= 128
                or not natural(value["ui_particles_sum"]) or value["ui_particles_sum"] < value["ui_particles_max"]
                or value["ui_particles_sum"] > value["frames"] * value["ui_particles_max"]):
            raise ValueError("active identity/time/particle evidence differs")
        initial = {"records": expected_records(copies), "target_count": copies * 9,
                   "target_structural_roots": copies * 5, "target_foreground_owners": copies * 4,
                   "target_active_unique_meshes": 2, "souls": copies // 4 * 29, "completion_effects": 0}
        if canonical(value["initial"]) != canonical(initial) or value["layout_sha256"] != hashlib.sha256(canonical(initial)).hexdigest():
            raise ValueError("initial layout differs from independent oracle")
        ordinals = [i for i in range(copies) if i % 4 >= 2]
        if not isinstance(value["lanes"], list) or len(value["lanes"]) != len(ordinals):
            raise ValueError("active lane inventory differs")
        for ordinal, lane in zip(ordinals, value["lanes"]):
            scalar_keys = {"ordinal", "produced", "delivered", "sand_in", "rock_in", "water_in", "frames", "active_frames"}
            if (not isinstance(lane, dict) or set(lane) != scalar_keys | {"produced_per_20s"}
                    or any(not natural(lane[k]) for k in scalar_keys)
                    or lane["ordinal"] != ordinal or lane["frames"] != value["frames"]
                    or not 0 < lane["active_frames"] < lane["frames"]
                    or lane["produced"] < 15 or lane["produced"] % 5
                    or lane["delivered"] < 5
                    or any(lane[k] < 1 for k in ("sand_in", "rock_in", "water_in"))):
                raise ValueError(f"lane {ordinal} lacks repeated production/input/output")
            bins = lane["produced_per_20s"]
            if (not isinstance(bins, list) or len(bins) != 3
                    or any(not natural(n) or n < 5 or n % 5 for n in bins)
                    or sum(bins) != lane["produced"]):
                raise ValueError(f"lane {ordinal} did not produce in every measurement third")
        return value, []
    except (OSError, UnicodeError, ValueError, TypeError, KeyError) as error:
        return None, [f"invalid building_art_active.json: {error}"]
