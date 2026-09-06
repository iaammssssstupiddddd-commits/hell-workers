"""Fail-closed post-export validation for a production Wall GLB."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
if str(SCRIPT_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPT_ROOT))

from workflow_common import staging_path, write_json_atomic

DEFAULT_CONTRACT = SCRIPT_ROOT.parent / "fixtures/wall-production-v1.geometry.json"
JSON_CHUNK = 0x4E4F534A
BIN_CHUNK = 0x004E4942
COMPONENT_FORMATS = {
    5120: ("b", 1),
    5121: ("B", 1),
    5122: ("h", 2),
    5123: ("H", 2),
    5125: ("I", 4),
    5126: ("f", 4),
}
TYPE_COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
PROFILE_AXIAL_SAMPLES = (8.01, 12.0, 15.99)
CORE_AXIAL_SAMPLES = (0.01, 4.0, 7.99)
SOLID_T_SAMPLES = (-4.0, 0.0, 4.0)
SOLID_Y_SAMPLES = (-15.0, 0.0, 15.0)


class ContractError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def no_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        require(key not in value, f"duplicate JSON field: {key}")
        value[key] = item
    return value


def load_contract(path: Path) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"geometry contract is absent: {path}")
    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_object)
    require(value.get("schema_version") == 1, "geometry contract schema differs")
    require(value.get("asset_set_id") == "wall-production-v1", "geometry contract id differs")
    return value


def read_glb(path: Path) -> tuple[dict[str, Any], bytes]:
    require(path.is_file() and not path.is_symlink(), f"GLB is absent: {path}")
    payload = path.read_bytes()
    require(len(payload) >= 20, "GLB is truncated")
    magic, version, declared_length = struct.unpack_from("<4sII", payload, 0)
    require(magic == b"glTF" and version == 2, "GLB header differs")
    require(declared_length == len(payload), "GLB declared length differs")
    offset = 12
    json_bytes: bytes | None = None
    binary: bytes | None = None
    while offset < len(payload):
        require(offset + 8 <= len(payload), "GLB chunk header is truncated")
        length, kind = struct.unpack_from("<II", payload, offset)
        offset += 8
        require(offset + length <= len(payload), "GLB chunk is truncated")
        chunk = payload[offset : offset + length]
        offset += length
        if kind == JSON_CHUNK:
            require(json_bytes is None, "GLB has multiple JSON chunks")
            json_bytes = chunk
        elif kind == BIN_CHUNK:
            require(binary is None, "GLB has multiple BIN chunks")
            binary = chunk
        else:
            raise ContractError(f"GLB has unsupported chunk type: {kind}")
    require(offset == len(payload), "GLB has trailing bytes")
    require(json_bytes is not None and binary is not None, "GLB requires JSON and BIN chunks")
    document = json.loads(
        json_bytes.decode("utf-8").rstrip(" \t\r\n\x00"),
        object_pairs_hook=no_duplicate_object,
    )
    require(isinstance(document, dict), "GLB JSON root must be an object")
    return document, binary


def exact_identity_node(node: dict[str, Any]) -> bool:
    return (
        "matrix" not in node
        and node.get("translation", [0.0, 0.0, 0.0]) == [0.0, 0.0, 0.0]
        and node.get("rotation", [0.0, 0.0, 0.0, 1.0]) == [0.0, 0.0, 0.0, 1.0]
        and node.get("scale", [1.0, 1.0, 1.0]) == [1.0, 1.0, 1.0]
    )


def accessor_values(document: dict[str, Any], binary: bytes, index: int) -> list[tuple[float, ...]]:
    accessors = document.get("accessors", [])
    require(type(index) is int and 0 <= index < len(accessors), "accessor index is invalid")
    accessor = accessors[index]
    require("sparse" not in accessor, "sparse accessors are unsupported")
    require(accessor.get("normalized", False) is False, "normalized accessor is unsupported")
    component_type = accessor.get("componentType")
    element_type = accessor.get("type")
    require(component_type in COMPONENT_FORMATS, "accessor component type is unsupported")
    require(element_type in TYPE_COMPONENTS, "accessor element type is unsupported")
    buffer_views = document.get("bufferViews", [])
    view_index = accessor.get("bufferView")
    require(type(view_index) is int and 0 <= view_index < len(buffer_views), "accessor bufferView is invalid")
    view = buffer_views[view_index]
    require(view.get("buffer") == 0, "accessor must use GLB buffer 0")
    fmt, component_size = COMPONENT_FORMATS[component_type]
    components = TYPE_COMPONENTS[element_type]
    packed_size = component_size * components
    stride = view.get("byteStride", packed_size)
    require(type(stride) is int and stride >= packed_size, "accessor byteStride is invalid")
    count = accessor.get("count")
    require(type(count) is int and count > 0, "accessor count is invalid")
    start = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
    view_end = view.get("byteOffset", 0) + view.get("byteLength", 0)
    require(start >= 0 and view_end <= len(binary), "accessor buffer range is invalid")
    require(start + (count - 1) * stride + packed_size <= view_end, "accessor exceeds bufferView")
    unpack = struct.Struct("<" + fmt * components)
    values = [tuple(float(value) for value in unpack.unpack_from(binary, start + row * stride)) for row in range(count)]
    require(all(math.isfinite(value) for row in values for value in row), "accessor contains non-finite values")
    return values


def index_values(document: dict[str, Any], binary: bytes, index: int) -> list[int]:
    accessor = document["accessors"][index]
    require(accessor.get("type") == "SCALAR", "index accessor must be SCALAR")
    require(accessor.get("componentType") in {5121, 5123, 5125}, "index component type differs")
    return [int(row[0]) for row in accessor_values(document, binary, index)]


def direction_coordinates(point: tuple[float, ...], direction: str) -> tuple[float, float]:
    x, _, z = point
    return {
        "N": (-z, x),
        "S": (z, -x),
        "W": (-x, -z),
        "E": (x, z),
    }[direction]


def unique_points(points: list[tuple[float, float]], tolerance: float) -> list[tuple[float, float]]:
    unique: list[tuple[float, float]] = []
    for point in points:
        if not any(abs(point[0] - other[0]) <= tolerance and abs(point[1] - other[1]) <= tolerance for other in unique):
            unique.append(point)
    return unique


def cross_section_segments(
    positions: list[tuple[float, ...]],
    triangles: list[tuple[int, int, int]],
    direction: str,
    axial: float,
    tolerance: float,
) -> list[tuple[tuple[float, float], tuple[float, float]]]:
    segments: list[tuple[tuple[float, float], tuple[float, float]]] = []
    for triangle in triangles:
        vertices = [positions[index] for index in triangle]
        coordinates = [direction_coordinates(point, direction) for point in vertices]
        distances = [coordinate[0] - axial for coordinate in coordinates]
        if all(distance > tolerance for distance in distances) or all(
            distance < -tolerance for distance in distances
        ):
            continue
        if all(abs(distance) <= tolerance for distance in distances):
            continue
        intersections: list[tuple[float, float]] = []
        for first, second in ((0, 1), (1, 2), (2, 0)):
            first_distance = distances[first]
            second_distance = distances[second]
            first_point = (coordinates[first][1], vertices[first][1])
            second_point = (coordinates[second][1], vertices[second][1])
            if abs(first_distance) <= tolerance:
                intersections.append(first_point)
            if first_distance * second_distance < -(tolerance * tolerance):
                ratio = -first_distance / (second_distance - first_distance)
                intersections.append(
                    (
                        first_point[0] + (second_point[0] - first_point[0]) * ratio,
                        first_point[1] + (second_point[1] - first_point[1]) * ratio,
                    )
                )
        intersections = unique_points(intersections, tolerance)
        if len(intersections) >= 2:
            pair = max(
                (
                    (first, second)
                    for ordinal, first in enumerate(intersections)
                    for second in intersections[ordinal + 1 :]
                ),
                key=lambda pair: (pair[0][0] - pair[1][0]) ** 2
                + (pair[0][1] - pair[1][1]) ** 2,
            )
            segments.append(pair)
    require(segments, f"{direction} cross-section at {axial} has no surface")
    return segments


def horizontal_crossings(
    segments: list[tuple[tuple[float, float], tuple[float, float]]],
    y: float,
    tolerance: float,
) -> list[float]:
    crossings: list[float] = []
    for (first_t, first_y), (second_t, second_y) in segments:
        if (first_y > y) == (second_y > y):
            continue
        ratio = (y - first_y) / (second_y - first_y)
        crossings.append(first_t + (second_t - first_t) * ratio)
    crossings.sort()
    unique: list[float] = []
    for value in crossings:
        if not unique or abs(value - unique[-1]) > tolerance:
            unique.append(value)
    return unique


def section_contains(crossings: list[float], t: float, tolerance: float) -> bool:
    return sum(1 for value in crossings if value > t + tolerance) % 2 == 1


def validate_section(
    positions: list[tuple[float, ...]],
    triangles: list[tuple[int, int, int]],
    direction: str,
    axial: float,
    *,
    expected_half_width: float | None,
    tolerance: float,
) -> dict[str, Any]:
    segments = cross_section_segments(positions, triangles, direction, axial, tolerance)
    endpoints = [point for segment in segments for point in segment]
    minimum_t = min(point[0] for point in endpoints)
    maximum_t = max(point[0] for point in endpoints)
    minimum_y = min(point[1] for point in endpoints)
    maximum_y = max(point[1] for point in endpoints)
    if expected_half_width is not None:
        require(abs(minimum_t + expected_half_width) <= tolerance, f"{direction} port minimum width differs at {axial}")
        require(abs(maximum_t - expected_half_width) <= tolerance, f"{direction} port maximum width differs at {axial}")
    require(abs(minimum_y + 16.0) <= tolerance, f"{direction} section Y minimum differs at {axial}")
    require(abs(maximum_y - 16.0) <= tolerance, f"{direction} section Y maximum differs at {axial}")
    for y in SOLID_Y_SAMPLES:
        crossings = horizontal_crossings(segments, y, tolerance)
        require(len(crossings) >= 2 and len(crossings) % 2 == 0, f"{direction} section is open at axial={axial}, y={y}")
        for t in SOLID_T_SAMPLES:
            require(section_contains(crossings, t, tolerance), f"{direction} section has a core hole at axial={axial}, t={t}, y={y}")
    return {
        "direction": direction,
        "axial_wu": axial,
        "min_t_wu": round(minimum_t, 6),
        "max_t_wu": round(maximum_t, 6),
        "min_y_wu": round(minimum_y, 6),
        "max_y_wu": round(maximum_y, 6),
    }


def validate_corridor_envelope(
    positions: list[tuple[float, ...]], arms: list[str], half_envelope: float, tolerance: float
) -> None:
    if not arms:
        require(
            all(abs(point[0]) <= half_envelope + tolerance and abs(point[2]) <= half_envelope + tolerance for point in positions),
            "isolated mesh exceeds the ornament envelope",
        )
        return
    for point in positions:
        inside = abs(point[0]) <= half_envelope + tolerance and abs(point[2]) <= half_envelope + tolerance
        for direction in arms:
            axial, perpendicular = direction_coordinates(point, direction)
            inside = inside or (axial >= -tolerance and abs(perpendicular) <= half_envelope + tolerance)
        require(inside, "mesh vertex exceeds the union of active ornament corridors")


def validate_formwork_geometry(
    positions: list[tuple[float, ...]],
    triangles: list[tuple[int, int, int]],
    family: str,
    arms: list[str],
    tolerance: float,
) -> list[dict[str, Any]]:
    expected_triangles = {
        "isolated": 60,
        "end": 60,
        "straight": 108,
        "corner": 108,
        "t_junction": 156,
        "cross": 204,
    }
    require(
        len(triangles) == expected_triangles[family],
        "formwork family triangle count differs",
    )
    if not arms:
        require(
            all(abs(point[0]) <= 6.4 + tolerance and abs(point[2]) <= 6.4 + tolerance for point in positions),
            "isolated formwork exceeds its 12.8 wu square",
        )
        return []

    observations: list[dict[str, Any]] = []
    for direction in arms:
        boundary = [
            point
            for point in positions
            if abs(direction_coordinates(point, direction)[0] - 16.0) <= tolerance
        ]
        require(boundary, f"{direction} formwork port does not reach the cell boundary")
        transverse = [direction_coordinates(point, direction)[1] for point in boundary]
        vertical = [point[1] for point in boundary]
        require(
            abs(min(transverse) + 4.8) <= tolerance
            and abs(max(transverse) - 4.8) <= tolerance,
            f"{direction} formwork port width differs",
        )
        require(
            abs(min(vertical) + 16.0) <= tolerance
            and abs(max(vertical) - 16.0) <= tolerance,
            f"{direction} formwork terminal post height differs",
        )
        gap_segments = cross_section_segments(
            positions, triangles, direction, 8.4, tolerance
        )
        gap_crossings = horizontal_crossings(gap_segments, 0.0, tolerance)
        require(
            not section_contains(gap_crossings, 4.0, tolerance),
            f"{direction} formwork has no mid-height construction gap",
        )
        observations.append(
            {
                "direction": direction,
                "gap_probe": {"axial_wu": 8.4, "t_wu": 4.0, "y_wu": 0.0},
                "port_max_t_wu": round(max(transverse), 6),
                "port_min_t_wu": round(min(transverse), 6),
            }
        )
    return observations


def validate_wall_glb(path: Path, family: str, contract_path: Path = DEFAULT_CONTRACT) -> dict[str, Any]:
    contract = load_contract(contract_path)
    families = {
        entry["family"]: entry["arms"]
        for entry in contract["canonical_orientation"]["canonical_families"]
    }
    require(family in families, f"unknown Wall family: {family}")
    document, binary = read_glb(path)
    require(document.get("asset", {}).get("version") == "2.0", "glTF asset version differs")
    require(len(document.get("buffers", [])) == 1 and "uri" not in document["buffers"][0], "GLB buffer contract differs")
    require(document["buffers"][0].get("byteLength", 0) <= len(binary), "GLB BIN length differs")
    require(not document.get("animations"), "Wall GLB must not contain animations")
    require(not document.get("skins"), "Wall GLB must not contain skins")
    require(not document.get("cameras"), "Wall GLB must not contain cameras")
    require(len(document.get("scenes", [])) == 1, "Wall GLB requires one scene")
    require(document.get("scene", 0) == 0, "Wall GLB default scene differs")
    require(len(document.get("nodes", [])) == 1, "Wall GLB requires one node")
    node = document["nodes"][0]
    require(node.get("mesh") == 0 and exact_identity_node(node), "Wall node transform or mesh differs")
    require(document["scenes"][0].get("nodes") == [0], "Wall scene root differs")
    require(len(document.get("meshes", [])) == 1, "Wall GLB requires one mesh")
    primitives = document["meshes"][0].get("primitives", [])
    require(len(primitives) == 1, "Wall GLB requires one primitive")
    primitive = primitives[0]
    require(primitive.get("mode", 4) == 4, "Wall primitive must use TRIANGLES")
    require(not primitive.get("targets"), "Wall primitive must not use morph targets")
    require(not primitive.get("extensions"), "Wall primitive extensions are unsupported")
    attributes = primitive.get("attributes", {})
    require(set(attributes) >= {"POSITION", "NORMAL", "TEXCOORD_0"}, "Wall primitive requires POSITION, NORMAL, and UV0")
    require(
        set(attributes) <= {"POSITION", "NORMAL", "TEXCOORD_0", "TANGENT"},
        "Wall primitive has unsupported vertex attributes",
    )
    require("indices" in primitive, "Wall primitive must be indexed")
    positions = accessor_values(document, binary, attributes["POSITION"])
    require(document["accessors"][attributes["POSITION"]].get("componentType") == 5126, "POSITION must use float32")
    require(document["accessors"][attributes["POSITION"]].get("type") == "VEC3", "POSITION must be VEC3")
    normals = accessor_values(document, binary, attributes["NORMAL"])
    uv0 = accessor_values(document, binary, attributes["TEXCOORD_0"])
    normal_accessor = document["accessors"][attributes["NORMAL"]]
    uv_accessor = document["accessors"][attributes["TEXCOORD_0"]]
    require(
        normal_accessor.get("componentType") == 5126
        and normal_accessor.get("type") == "VEC3",
        "NORMAL must use float32 VEC3",
    )
    require(
        uv_accessor.get("componentType") == 5126 and uv_accessor.get("type") == "VEC2",
        "UV0 must use float32 VEC2",
    )
    require(len(normals) == len(positions) and len(uv0) == len(positions), "vertex attribute counts differ")
    indices = index_values(document, binary, primitive["indices"])
    require(len(indices) % 3 == 0, "Wall index count is not triangular")
    require(all(0 <= index < len(positions) for index in indices), "Wall index is outside POSITION")
    triangles = [tuple(indices[index : index + 3]) for index in range(0, len(indices), 3)]
    for first, second, third in triangles:
        require(len({first, second, third}) == 3, "Wall triangle repeats an index")
        a = positions[first]
        b = positions[second]
        c = positions[third]
        cross = (
            (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
            (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
        )
        require(sum(value * value for value in cross) > 1.0e-12, "Wall triangle is degenerate")
    triangle_cap = contract["mesh_contract"]["hard_triangle_cap_per_mesh"]
    require(len(triangles) <= triangle_cap, "Wall triangle cap exceeded")

    minimum = [min(point[axis] for point in positions) for axis in range(3)]
    maximum = [max(point[axis] for point in positions) for axis in range(3)]
    tolerance = contract["geometry"]["absolute_tolerance_wu"]
    envelope_min = contract["bounds"]["local_min"]
    envelope_max = contract["bounds"]["local_max"]
    require(all(minimum[axis] >= envelope_min[axis] - tolerance for axis in range(3)), "Wall minimum bounds exceed the cell envelope")
    require(all(maximum[axis] <= envelope_max[axis] + tolerance for axis in range(3)), "Wall maximum bounds exceed the cell envelope")
    require(abs(minimum[1] + 16.0) <= tolerance and abs(maximum[1] - 16.0) <= tolerance, "Wall vertical bounds differ")

    images = document.get("images", [])
    embedded_images = 0
    external_images: list[str] = []
    for image in images:
        uri = image.get("uri")
        if "bufferView" in image or (isinstance(uri, str) and uri.startswith("data:")):
            embedded_images += 1
        elif isinstance(uri, str):
            image_path = (path.parent / uri).resolve()
            try:
                image_path.relative_to(path.parent.resolve())
            except ValueError as error:
                raise ContractError(f"external image escapes GLB directory: {uri}") from error
            require(image_path.is_file(), f"external image is missing: {uri}")
            external_images.append(uri)
        else:
            raise ContractError("Wall image has neither external URI nor embedded data")
    require(embedded_images == contract["mesh_contract"]["embedded_images"], "embedded image count differs")

    arms = families[family]
    half_width = contract["geometry"]["port_half_width_wu"]
    half_envelope = contract["geometry"]["ornament_envelope_width_wu"] / 2.0
    validate_corridor_envelope(positions, arms, half_envelope, tolerance)
    sections: list[dict[str, Any]] = []
    formwork_ports: list[dict[str, Any]] = []
    is_formwork = contract["mesh_contract"].get("profile") == "formwork"
    if is_formwork:
        formwork_ports = validate_formwork_geometry(
            positions, triangles, family, arms, tolerance
        )
    elif arms:
        for direction in arms:
            for axial in (*CORE_AXIAL_SAMPLES, *PROFILE_AXIAL_SAMPLES):
                sections.append(
                    validate_section(
                        positions,
                        triangles,
                        direction,
                        axial,
                        expected_half_width=half_width if axial >= 8.0 else None,
                        tolerance=tolerance,
                    )
                )
    else:
        for direction in ("E", "S"):
            sections.append(
                validate_section(
                    positions,
                    triangles,
                    direction,
                    0.0,
                    expected_half_width=half_width,
                    tolerance=tolerance,
                )
            )

    tangent_present = "TANGENT" in attributes
    if tangent_present:
        tangents = accessor_values(document, binary, attributes["TANGENT"])
        tangent_accessor = document["accessors"][attributes["TANGENT"]]
        require(
            tangent_accessor.get("componentType") == 5126
            and tangent_accessor.get("type") == "VEC4",
            "TANGENT must use float32 VEC4",
        )
        require(len(tangents) == len(positions), "TANGENT count differs")
    return {
        "schema_version": 1,
        "status": "pass",
        "asset_set_id": contract["asset_set_id"],
        "contract_sha256": hashlib.sha256(contract_path.read_bytes()).hexdigest(),
        "family": family,
        "glb": str(path.resolve()),
        "glb_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "mesh_count": 1,
        "primitive_count": 1,
        "triangle_count": len(triangles),
        "vertex_count": len(positions),
        "uv0_present": True,
        "tangent_present": tangent_present,
        "embedded_images": embedded_images,
        "external_images": external_images,
        "raw_primitive_local_bounds": {
            "min": [round(value, 6) for value in minimum],
            "max": [round(value, 6) for value in maximum],
        },
        "node_transform": contract["bounds"]["node_transform"],
        "arms": arms,
        "cross_sections": sections,
        "formwork_ports": formwork_ports,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True)
    parser.add_argument("--family", required=True)
    parser.add_argument("--contract", default=str(DEFAULT_CONTRACT))
    parser.add_argument("--report", required=True)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    input_path = staging_path(args.input, "exports")
    report_path = staging_path(args.report, "reports")
    try:
        report = validate_wall_glb(input_path, args.family, Path(args.contract).resolve())
    except (ContractError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        write_json_atomic(
            report_path,
            {
                "schema_version": 1,
                "status": "failed",
                "family": args.family,
                "glb": str(input_path),
                "reason": str(error),
            },
        )
        raise ContractError(str(error)) from error
    write_json_atomic(report_path, report)
    print(f"WALL_GLB_VALIDATION status=pass family={args.family} report={report_path}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ContractError as error:
        print(f"Wall GLB validation failed: {error}")
        raise SystemExit(1) from error
