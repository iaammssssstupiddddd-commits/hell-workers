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
    require(bounds["min_z"] >= -7.5 and bounds["max_z"] <= 10.0, "Door depth envelope differs")
    frame_points = sorted(tuple(round(value, 5) for value in point) for point in positions[:24])
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
