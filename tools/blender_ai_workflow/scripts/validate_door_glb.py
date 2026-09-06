"""Fail-closed post-export validation for a production Door GLB."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

SCRIPT_ROOT = Path(__file__).resolve().parent
if str(SCRIPT_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPT_ROOT))

from validate_wall_glb import accessor_values, exact_identity_node, index_values, read_glb
from workflow_common import staging_path, write_json_atomic

DEFAULT_CONTRACT = SCRIPT_ROOT.parent / "fixtures/door-production-v1.geometry.json"


class DoorContractError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise DoorContractError(message)


def axis_range(points: list[list[float]], axis: int) -> tuple[float, float]:
    values = [point[axis] for point in points]
    return min(values), max(values)


def require_range(
    actual: tuple[float, float],
    expected: list[float],
    tolerance: float,
    label: str,
) -> None:
    require(
        all(abs(left - right) <= tolerance for left, right in zip(actual, expected, strict=True)),
        f"{label} differs: {actual} != {tuple(expected)}",
    )


def validate(path: Path, state: str, contract_path: Path) -> dict[str, object]:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    require(contract.get("asset_set_id") == "door-production-v1", "contract identity differs")
    require(state in contract.get("states", {}), "Door state is unknown")
    document, binary = read_glb(path)
    require(len(document.get("scenes", [])) == 1, "Door GLB must have one scene")
    scene = document["scenes"][0]
    require(scene.get("nodes") == [0], "Door GLB scene root differs")
    require(len(document.get("nodes", [])) == 1, "Door GLB must have one node")
    node = document["nodes"][0]
    require(node.get("mesh") == 0 and exact_identity_node(node), "Door node identity differs")
    require(len(document.get("meshes", [])) == 1, "Door GLB must have one mesh")
    primitives = document["meshes"][0].get("primitives", [])
    require(len(primitives) == 1, "Door GLB must have one primitive")
    primitive = primitives[0]
    attributes = primitive.get("attributes", {})
    require(set(attributes) >= {"POSITION", "NORMAL", "TEXCOORD_0"}, "Door attributes differ")
    require("indices" in primitive, "Door indices are absent")
    positions = accessor_values(document, binary, attributes["POSITION"])
    indices = index_values(document, binary, primitive["indices"])
    require(len(indices) % 3 == 0, "Door index count is not triangular")
    triangles = len(indices) // 3
    expected = contract["states"][state]["triangles"]
    require(triangles == expected, f"Door triangle count differs: {triangles} != {expected}")
    require(triangles <= contract["triangle_cap"], "Door triangle cap exceeded")
    require(not document.get("images"), "Door GLB embeds images")
    bounds = {
        "min_x": min(point[0] for point in positions),
        "max_x": max(point[0] for point in positions),
        "min_y": min(point[1] for point in positions),
        "max_y": max(point[1] for point in positions),
        "min_z": min(point[2] for point in positions),
        "max_z": max(point[2] for point in positions),
    }
    tolerance = contract["tolerance_wu"]
    require(abs(bounds["min_x"] + 16.0) <= tolerance, "Door minimum X differs")
    require(abs(bounds["max_x"] - 16.0) <= tolerance, "Door maximum X differs")
    require(abs(bounds["min_y"] + 16.0) <= tolerance, "Door minimum Y differs")
    require(abs(bounds["max_y"] - 16.0) <= tolerance, "Door maximum Y differs")
    if state == "open":
        open_leaf = contract["open_leaf_envelope_wu"]
        minimum_depth = min(open_leaf["left_z"][0], open_leaf["right_z"][0]) - tolerance
        maximum_depth = max(4.8, open_leaf["left_z"][1], open_leaf["right_z"][1]) + tolerance
    else:
        minimum_depth = -7.5
        maximum_depth = 10.0
    require(
        bounds["min_z"] >= minimum_depth and bounds["max_z"] <= maximum_depth,
        "Door depth envelope differs",
    )
    frame = contract["frame"]
    # glTF exports each authored box as 24 positions because the six UV faces
    # intentionally do not share vertices.
    frame_parts = (positions[0:24], positions[24:48], positions[48:72])
    for index, jamb in enumerate(frame_parts[:2]):
        require_range(
            axis_range(jamb, 0),
            frame["jamb_x_ranges_wu"][index],
            tolerance,
            f"Door jamb {index} X range",
        )
        require_range(
            axis_range(jamb, 2),
            frame["jamb_z_range_wu"],
            tolerance,
            f"Door jamb {index} Z range",
        )
    top = frame_parts[2]
    require_range(axis_range(top, 0), frame["top_x_range_wu"], tolerance, "Door top frame X range")
    require_range(axis_range(top, 1), frame["top_y_range_wu"], tolerance, "Door top frame Y range")
    require_range(axis_range(top, 2), frame["top_z_range_wu"], tolerance, "Door top frame Z range")
    if state == "open":
        left_leaf, right_leaf = positions[72:96], positions[96:120]
        require_range(axis_range(left_leaf, 0), open_leaf["left_x"], tolerance, "Door open left leaf X range")
        require_range(axis_range(right_leaf, 0), open_leaf["right_x"], tolerance, "Door open right leaf X range")
        for label, leaf in (("left", left_leaf), ("right", right_leaf)):
            require_range(
                axis_range(leaf, 2),
                open_leaf[f"{label}_z"],
                tolerance,
                f"Door open {label} leaf Z range",
            )
    frame_points = sorted(tuple(round(value, 5) for value in point) for point in positions[:72])
    frame_sha256 = hashlib.sha256(
        json.dumps(frame_points, separators=(",", ":")).encode()
    ).hexdigest()
    return {
        "asset_set_id": "door-production-v1",
        "bounds_wu": {key: round(value, 6) for key, value in bounds.items()},
        "frame_sha256": frame_sha256,
        "glb_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "schema_version": 1,
        "state": state,
        "status": "pass",
        "triangles": triangles,
        "vertices": len(positions),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("state", choices=("closed", "open", "locked"))
    parser.add_argument("report")
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    args = parser.parse_args()
    result = validate(args.input.resolve(), args.state, args.contract.resolve())
    report = staging_path(Path(args.report), "reports")
    write_json_atomic(report, result)
    print(
        "DOOR_GLB_VALIDATION "
        f"status=pass state={args.state} triangles={result['triangles']} report={report}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (DoorContractError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"Door GLB validation failed: {error}")
        raise SystemExit(1) from error
