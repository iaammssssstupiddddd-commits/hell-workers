"""Check M2 proposal arithmetic; this never grants numeric or visual acceptance.

Coordinator-only execution, without Blender. Actual exported GLBs and projection
reports still require the existing clay export verifier on the same subject.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path

from building_clay_geometry import drum_sweep, FIXTURES, PILOTS, build, load_pilot, read_json, require


def same(actual, expected, path: str) -> None:
    if isinstance(expected, dict):
        require(isinstance(actual, dict) and actual.keys() == expected.keys(), path)
        for key, value in expected.items():
            same(actual[key], value, f"{path}/{key}")
    elif isinstance(expected, list):
        require(isinstance(actual, list) and len(actual) == len(expected), path)
        for index, (a, e) in enumerate(zip(actual, expected, strict=True)):
            same(a, e, f"{path}/{index}")
    elif isinstance(expected, bool):
        require(actual is expected, path)
    elif isinstance(expected, (int, float)):
        require(type(actual) in (int, float) and math.isfinite(actual)
                and abs(actual - expected) <= 1e-9, path)
    else:
        require(actual == expected, path)


def expected_kind(kind: str, mixer_revision: str = "v1") -> dict:
    revision = mixer_revision if kind == "MudMixer" else "v1"
    fixture = FIXTURES / f"building-{PILOTS[kind]}-{revision}.geometry.json"
    contract = load_pilot(kind, revision)
    meshes = build(kind, contract)
    body_min, body_max = meshes["body"].bounds()
    result = {
        "geometry_file": fixture.name,
        "geometry_sha256": hashlib.sha256(fixture.read_bytes()).hexdigest(),
        "height_wu": body_max[1] - body_min[1],
        "bounds_min_wu": body_min, "bounds_max_wu": body_max,
        "part_cap": len(meshes),
        "triangle_caps": {role: spec["triangle_cap"]
                          for role, spec in contract["roles"].items()},
        "world_preview": contract["preview"],
        "catalog": {"canvas_px": [256, 256], "anchor_px": [128, 128]},
    }
    if kind == "MudMixer" and revision in {"v3", "v4", "v5"}:
        result["rotor_sweep"] = drum_sweep(meshes["rotor"].vertices, contract)
        result["motion"] = {"axis": [0, 1, 0],
                            "pivot_wu": contract["roles"]["rotor"]["translation_wu"],
                            "rotation_xyzw": contract["roles"]["rotor"]["rotation_xyzw"],
                            "seconds_per_revolution": 4, "clock": "Virtual",
                            "active_requires": "Refining", "idle": "hold", "paused": "hold",
                            "load_angle_radians": 0}
        return result
    if kind == "Tank":
        low, high = meshes["water"].bounds()
        result["states"] = {}
        for state, spec in contract["states"].items():
            height = spec["water_y_wu"]
            result["states"][state] = {
                "visible": spec["water_visible"], "translation_wu": [0, height, 0],
                "bounds_min_wu": [low[0], low[1] + height, low[2]],
                "bounds_max_wu": [high[0], high[1] + height, high[2]],
            }
            require(low[1] + height > contract["construction"]["floor_y"]
                    and high[1] + height < contract["construction"]["rim_y"],
                    f"{state}: water intersects floor/rim")
    else:
        config = contract["construction"]
        pivot = contract["roles"]["rotor"]["translation_wu"]
        rotor = meshes["rotor"]
        radius = max(math.hypot(x, z) for x, _, z in rotor.vertices)
        low, high = rotor.bounds()
        # A vertex radius bounds every triangle point under every rotation angle.
        floor, left, _, back, *rest = contract["construction"]["body_boxes"]
        floor_top = floor["center"][1] + floor["size"][1] / 2
        inner_x = abs(left["center"][0]) - left["size"][0] / 2
        inner_z = abs(back["center"][2]) - back["size"][2] / 2
        support = rest[-1]
        support_y = support["center"][1] - support["size"][1] / 2
        minimum_y, maximum_y = low[1] + pivot[1], high[1] + pivot[1]
        require(radius < min(inner_x, inner_z) and minimum_y > floor_top,
                "rotor sweep intersects trough")
        require(abs(maximum_y - support_y) <= contract["tolerance_wu"],
                "shaft must meet support")
        if revision == "v2":
            baseline = load_pilot("MudMixer")
            require(config["body_boxes"] == baseline["construction"]["body_boxes"]
                    and config["rotor_boxes"] == baseline["construction"]["rotor_boxes"]
                    and contract["roles"]["rotor"] == baseline["roles"]["rotor"]
                    and contract["states"] == baseline["states"]
                    and contract["preview"] == baseline["preview"]
                    and contract["roles"]["body"] == {
                        **baseline["roles"]["body"], "triangle_cap": 132},
                    "mud proposal changed the existing construction or state")
            mud = config["mud_box"]
            require(set(mud) == {"center", "size"}
                    and all(type(value) in (int, float) and math.isfinite(value)
                            for row in (mud["center"], mud["size"]) for value in row)
                    and all(value > 0 for value in mud["size"]),
                    "mud proposal requires one finite positive surface slab")
            mud_bottom = mud["center"][1] - mud["size"][1] / 2
            mud_top = mud["center"][1] + mud["size"][1] / 2
            require(mud_bottom > floor_top and mud_top < minimum_y
                    and abs(mud["center"][0]) + mud["size"][0] / 2 < inner_x
                    and abs(mud["center"][2]) + mud["size"][2] / 2 < inner_z,
                    "mud surface clips floor, walls, or full rotor sweep")
            result["mud_surface"] = {
                "bounds_min_wu": [mud["center"][i] - mud["size"][i] / 2 for i in range(3)],
                "bounds_max_wu": [mud["center"][i] + mud["size"][i] / 2 for i in range(3)],
                "floor_clearance_wu": mud_bottom - floor_top,
                "rotor_clearance_wu": minimum_y - mud_top,
                "inner_x_clearance_wu": inner_x - abs(mud["center"][0]) - mud["size"][0] / 2,
                "inner_z_clearance_wu": inner_z - abs(mud["center"][2]) - mud["size"][2] / 2,
                "state": "fixed_residue_not_fill_level",
            }
        result["motion"] = {
            "axis": [0, 1, 0], "pivot_wu": pivot, "seconds_per_revolution": 4,
            "clock": "Virtual", "active_requires": "Refining", "idle": "hold",
            "paused": "hold", "load_angle_radians": 0,
        }
        result["rotor_sweep"] = {
            "radius_wu": radius,
            "bounds_min_wu": [pivot[0] - radius, minimum_y, pivot[2] - radius],
            "bounds_max_wu": [pivot[0] + radius, maximum_y, pivot[2] + radius],
            "floor_clearance_wu": minimum_y - floor_top,
            "inner_x_clearance_wu": inner_x - radius,
            "inner_z_clearance_wu": inner_z - radius,
            "shaft_support_y_wu": support_y,
        }
    return result


def validate(candidate: Path) -> dict:
    data = read_json(candidate)
    revision = data.get("mud_mixer_revision", "v1")
    require(revision in {"v1", "v2", "v3", "v4", "v5"}, "unknown MudMixer numeric revision")
    expected_status = {
        "schema_version": 1, "stage": "numeric_freeze_candidate",
        "decision": "pending_independent_review", "numeric_freeze": False,
        "runtime_published": False, "units": "world_unit",
    }
    if revision in {"v2", "v3", "v4", "v5"}:
        expected_status["mud_mixer_revision"] = revision
    same({key: data[key] for key in expected_status},
        expected_status, "proposal status")
    same(data["kinds"], {kind: expected_kind(kind, revision) for kind in PILOTS}, "geometry")
    budget = data["texture_budget_per_kind"]
    same(budget, {
        "format": "RGBA8", "albedo_px": [512, 512], "world_preview_px": [256, 256],
        "catalog_px": [256, 256], "image_cap": 3, "base_level_bytes_cap": 1572864,
        "full_mip_chain_bytes_cap": 2097152, "atlas_gutter_px": 4,
    }, "proposed texture budget")
    base_bytes = mip_bytes = 0
    for key in ("albedo_px", "world_preview_px", "catalog_px"):
        width, height = budget[key]
        base_bytes += width * height * 4
        while True:
            mip_bytes += width * height * 4
            if width == height == 1:
                break
            width, height = max(1, width // 2), max(1, height // 2)
    require(base_bytes <= budget["base_level_bytes_cap"]
            and mip_bytes <= budget["full_mip_chain_bytes_cap"], "texture byte cap exceeded")
    return {
        "schema_version": 1, "check": "proposal_arithmetic_only",
        "candidate_sha256": hashlib.sha256(candidate.read_bytes()).hexdigest(),
        "geometry_sha256": {kind: spec["geometry_sha256"]
                            for kind, spec in data["kinds"].items()},
        "numeric_freeze": False, "runtime_published": False,
        "texture_base_bytes_per_kind": base_bytes,
        "texture_full_mip_bytes_per_kind": mip_bytes,
        "excludes": ["exported GLB", "preview projection", "actual-window",
                     "independent acceptance", "performance budget", "formal release"],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("candidate", type=Path, nargs="?",
                        default=FIXTURES / "building-m2-v1.numeric-candidate.json")
    args = parser.parse_args()
    print(json.dumps(validate(args.candidate), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
