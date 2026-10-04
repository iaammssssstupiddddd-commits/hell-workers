"""Validate actual pilot GLB bytes; a pass is technical clay evidence, never approval."""

from __future__ import annotations

import argparse
import hashlib
import math
from pathlib import Path

from building_clay_geometry import load_pilot, require
from validate_wall_glb import accessor_values, exact_identity_node, index_values, read_glb
from workflow_common import staging_path, write_json_atomic


def validate(path: Path, kind: str, role: str) -> dict:
    contract = load_pilot(kind)
    require(role in contract["roles"], "unknown pilot mesh role")
    spec = contract["roles"][role]
    document, binary = read_glb(path)
    require(document.get("asset", {}).get("version") == "2.0", "glTF version differs")
    require(len(document.get("scenes", [])) == 1 and document["scenes"][0].get("nodes") == [0],
            "expected one scene with root node zero")
    require(document.get("scene", 0) == 0, "default scene differs")
    require(len(document.get("nodes", [])) == 1, "expected one node")
    node = document["nodes"][0]
    require(node.get("mesh") == 0 and exact_identity_node(node), "node must be identity")
    require(not node.get("children") and "skin" not in node and "weights" not in node,
            "node children, skin or weights are forbidden")
    for field in ("animations", "skins", "images", "textures", "extensionsUsed", "extensionsRequired"):
        require(not document.get(field), f"{field} forbidden in a clay role GLB")
    require(len(document.get("buffers", [])) == 1 and "uri" not in document["buffers"][0],
            "only the embedded geometry buffer is allowed")
    require(len(document.get("meshes", [])) == 1, "expected one mesh")
    mesh = document["meshes"][0]
    require(not mesh.get("weights"), "morph weights forbidden")
    require(len(mesh.get("primitives", [])) == 1, "expected one primitive")
    primitive = mesh["primitives"][0]
    require(primitive.get("mode", 4) == 4 and not primitive.get("targets"), "only static triangles allowed")
    require(not primitive.get("extensions"), "primitive extensions forbidden")
    attrs = primitive.get("attributes", {})
    require(set(attrs) == {"POSITION", "NORMAL", "TEXCOORD_0"}, "attribute set differs")
    values = {}
    for name, shape in (("POSITION", "VEC3"), ("NORMAL", "VEC3"), ("TEXCOORD_0", "VEC2")):
        index = attrs[name]
        require(type(index) is int and 0 <= index < len(document.get("accessors", [])), "bad accessor")
        accessor = document["accessors"][index]
        require(accessor.get("type") == shape and accessor.get("componentType") == 5126,
                f"{name} must be float {shape}")
        values[name] = accessor_values(document, binary, index)
    points, normals, uv = (values[name] for name in ("POSITION", "NORMAL", "TEXCOORD_0"))
    require(len(points) == len(normals) == len(uv), "attribute counts differ")
    index = primitive.get("indices")
    require(type(index) is int and 0 <= index < len(document["accessors"]), "bad index accessor")
    indices = index_values(document, binary, index)
    require(len(indices) % 3 == 0, "incomplete triangle")
    require(0 < len(indices) // 3 <= spec["triangle_cap"], "triangle cap exceeded")
    require(all(0 <= index < len(points) for index in indices), "index out of range")
    require(all(abs(math.sqrt(sum(v * v for v in normal)) - 1) < 0.002 for normal in normals),
            "normal is not unit length")
    require(all(0 <= value <= 1 for point in uv for value in point), "UV outside clay atlas")
    for start in range(0, len(indices), 3):
        ids = indices[start:start + 3]
        a, b, c = [points[i] for i in ids]
        ab = [b[i] - a[i] for i in range(3)]
        ac = [c[i] - a[i] for i in range(3)]
        cross = (ab[1] * ac[2] - ab[2] * ac[1], ab[2] * ac[0] - ab[0] * ac[2],
                 ab[0] * ac[1] - ab[1] * ac[0])
        require(sum(v * v for v in cross) > 1e-12, "degenerate geometry")
        a, b, c = [uv[i] for i in ids]
        require(abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) > 1e-10,
                "degenerate UV triangle")
    bounds = [[min(p[i] for p in points) for i in range(3)],
              [max(p[i] for p in points) for i in range(3)]]
    for actual, expected in zip(bounds, (spec["bounds_min_wu"], spec["bounds_max_wu"]), strict=True):
        require(all(abs(a - b) <= contract["tolerance_wu"] for a, b in zip(actual, expected, strict=True)),
                "GLB bounds differ from world-unit role contract (scale/pivot?)")
    return {"status": "pass", "evidence_kind": "technical_clay_only", "kind": kind,
            "role": role, "bounds_wu": bounds, "triangles": len(indices) // 3,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("kind", choices=("Tank", "MudMixer"))
    parser.add_argument("role")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    result = validate(args.input, args.kind, args.role)
    if args.report:
        report = staging_path(args.report, "reports")
        require(not report.exists(), "report already exists")
        write_json_atomic(report, result)
    print(result)


if __name__ == "__main__":
    main()
