"""Check indexed candidate faces with separate bounded M2/M3 face contracts."""

from collections import Counter, defaultdict
import struct

from building_clay_geometry import require
from validate_wall_glb import accessor_values, index_values, read_glb


def check_indexed_faces(glb, geometry, faces):
    return _check_faces(glb, geometry, faces, (3, 4), "M2")


def check_m3_indexed_faces(glb, geometry, faces):
    # M3 adds octagonal bone ends; slot caps are already triangles. M2's
    # three/four-sided admission remains unchanged at its own entrypoint.
    return _check_faces(glb, geometry, faces, (3, 4, 8), "M3")


def _check_faces(glb, geometry, faces, allowed_sides, label):
    document, binary = read_glb(glb)
    primitive = document["meshes"][0]["primitives"][0]
    attributes = primitive["attributes"]
    positions = accessor_values(document, binary, attributes["POSITION"])
    uvs = accessor_values(document, binary, attributes["TEXCOORD_0"])
    indices = index_values(document, binary, primitive["indices"])
    require(len(positions) == len(uvs) and len(indices) % 3 == 0,
            f"{label} candidate indexed accessor shape differs")

    def key(point, uv):
        return tuple(round(value, 5) for value in (*point, *uv))

    def f32(value):
        return struct.unpack("<f", struct.pack("<f", value))[0]

    expected = {}
    for face_index, (polygon, face) in enumerate(zip(geometry.faces, faces, strict=True)):
        require(len(polygon) in allowed_sides, f"{label} candidate polygon has unsupported sides")
        for corner, (vertex, uv) in enumerate(zip(polygon, face["uv"], strict=True)):
            # Blender UV origin is bottom-left; glTF exports top-left V.
            # Match the exported Float32 representation before applying the
            # existing five-decimal key. UVs are stored as f32 in Blender, then
            # glTF flips V and stores f32 again. Rounding the original double
            # first can place one valid value on the opposite key boundary.
            # Position matching and the rounding width remain unchanged.
            entry = key(geometry.vertices[vertex],
                        (f32(uv[0]), f32(1.0 - f32(uv[1]))))
            require(entry not in expected, f"{label} candidate face UV cell is not private")
            expected[entry] = (face_index, corner)
    actual = [key(point, uv) for point, uv in zip(positions, uvs, strict=True)]
    require(set(actual) == set(expected), f"{label} candidate vertex/UV set differs")
    corners_by_face = defaultdict(list)
    for triangle in zip(*(iter(indices),) * 3, strict=True):
        require(all(type(index) is int and 0 <= index < len(actual) for index in triangle),
                f"{label} candidate index is out of bounds")
        corners = [expected[actual[index]] for index in triangle]
        require(len({face for face, _ in corners}) == 1
                and len({corner for _, corner in corners}) == 3,
                f"{label} candidate triangle crosses or degenerates a face")
        corners_by_face[corners[0][0]].append(tuple(corner for _, corner in corners))
    require(set(corners_by_face) == set(range(len(faces))),
            f"{label} candidate indexed face is missing")
    for face_index, face in enumerate(geometry.faces):
        triangles = corners_by_face[face_index]
        require(len(triangles) == len(face) - 2,
                f"{label} candidate indexed face triangle count differs")
        directed = Counter((a, b) for triangle in triangles
                           for a, b in zip(triangle, triangle[1:] + triangle[:1], strict=True))
        perimeter = {(index, (index + 1) % len(face)) for index in range(len(face))}
        require(all(directed[edge] == 1 and directed[(edge[1], edge[0])] == 0
                    for edge in perimeter),
                f"{label} candidate indexed face winding or perimeter differs")
        internal = {edge: count for edge, count in directed.items()
                    if edge not in perimeter and (edge[1], edge[0]) not in perimeter}
        require(len(internal) == 2 * (len(face) - 3)
                and all(count == 1 and internal.get((edge[1], edge[0])) == 1
                        for edge, count in internal.items()),
                f"{label} candidate indexed face triangulation differs")
