"""Pure, deterministic pilot geometry in glTF world units (not final artwork)."""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path

FIXTURES = Path(__file__).resolve().parent.parent / "fixtures"
PILOTS = {"Tank": "tank", "MudMixer": "mud-mixer"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate field: {key}")
        result[key] = value
    return result


def read_json(path: Path) -> dict:
    def invalid_number(value):
        raise ValueError(f"non-finite JSON number: {value}")

    return json.loads(path.read_text(), object_pairs_hook=unique_fields,
                      parse_constant=invalid_number)


def load_pilot(kind: str) -> dict:
    require(kind in PILOTS, "only Tank and MudMixer have clay drafts")
    contract = read_json(FIXTURES / f"building-{PILOTS[kind]}-v1.geometry.json")
    require(contract["schema_version"] == 1 and contract["kind"] == kind, "pilot identity differs")
    require(contract["stage"] == "clay_draft" and contract["final_art"] is None,
            "this generator only produces unapproved clay drafts")
    require(contract["units"] == "world_unit" and contract["geometry_scale"] == 32,
            "pilot units differ")
    return contract


@dataclass
class Geometry:
    vertices: list[tuple[float, float, float]] = field(default_factory=list)
    faces: list[tuple[int, ...]] = field(default_factory=list)
    surfaces: list[str] = field(default_factory=list)

    def face(self, indices, surface: str) -> None:
        self.faces.append(tuple(indices))
        self.surfaces.append(surface)

    def box(self, center, size, label: str) -> None:
        start = len(self.vertices)
        for x, y, z in ((-1, -1, -1), (1, -1, -1), (1, -1, 1), (-1, -1, 1),
                        (-1, 1, -1), (1, 1, -1), (1, 1, 1), (-1, 1, 1)):
            self.vertices.append(tuple(c + s * sign / 2 for c, s, sign in
                                       zip(center, size, (x, y, z), strict=True)))
        for name, corners in (("bottom", (0, 1, 2, 3)), ("top", (4, 7, 6, 5)),
                              ("front", (0, 4, 5, 1)), ("right", (1, 5, 6, 2)),
                              ("back", (2, 6, 7, 3)), ("left", (3, 7, 4, 0))):
            self.face((start + i for i in corners), f"{label}:{name}")

    def lathe(self, profile, segments: int, label: str) -> None:
        """Revolve an outward-wound radius/height profile; radius zero is one vertex."""
        rings = []
        for radius, height in profile:
            ring = []
            for index in range(1 if radius == 0 else segments):
                angle = math.tau * index / segments
                ring.append(len(self.vertices))
                self.vertices.append((radius * math.cos(angle), height, radius * math.sin(angle)))
            rings.append(ring)
        for row in range(len(rings) - 1):
            a, b = rings[row], rings[row + 1]
            for index in range(segments):
                next_index = (index + 1) % segments
                if len(a) == 1:
                    face = (a[0], b[next_index], b[index])
                elif len(b) == 1:
                    face = (a[index], a[next_index], b[0])
                else:
                    face = (a[index], a[next_index], b[next_index], b[index])
                self.face(reversed(face), f"{label}:band{row}")

    def bounds(self):
        return ([min(p[i] for p in self.vertices) for i in range(3)],
                [max(p[i] for p in self.vertices) for i in range(3)])

    def triangles(self) -> int:
        return sum(len(face) - 2 for face in self.faces)


def build(kind: str, contract: dict) -> dict[str, Geometry]:
    body, moving = Geometry(), Geometry()
    config = contract["construction"]
    if kind == "Tank":
        # One closed shell, including the actual cavity and floor. No painted opening.
        body.lathe([(0, 0), (config["outer_radius_bottom"], 0),
                    (config["outer_radius_top"], config["rim_y"]),
                    (config["inner_radius_top"], config["rim_y"]),
                    (config["inner_radius_bottom"], config["floor_y"]),
                    (0, config["floor_y"])], config["segments"], "vessel")
        radius, half = config["water_radius"], config["water_half_thickness"]
        moving.lathe([(0, -half), (radius, -half), (radius, half), (0, half)],
                     config["segments"], "water")
        result = {"body": body, "water": moving}
    elif kind == "MudMixer":
        for role, geometry in (("body", body), ("rotor", moving)):
            for index, box in enumerate(config[f"{role}_boxes"]):
                geometry.box(box["center"], box["size"], f"{role}{index}")
        result = {"body": body, "rotor": moving}
    else:
        raise ValueError(f"unknown clay kind: {kind}")
    require(set(result) == set(contract["roles"]), "mesh role mismatch")
    for role, geometry in result.items():
        spec = contract["roles"][role]
        minimum, maximum = geometry.bounds()
        for actual, expected in ((minimum, spec["bounds_min_wu"]), (maximum, spec["bounds_max_wu"])):
            require(all(abs(a - b) <= contract["tolerance_wu"] for a, b in zip(actual, expected, strict=True)),
                    f"{kind}/{role} draft bounds differ")
        require(geometry.triangles() <= spec["triangle_cap"], f"{kind}/{role} draft cap exceeded")
    return result


def face_uv(points):
    """Planar, metric-density clay UVs; final atlas packing needs the art-freeze gate."""
    a, b, c = points[:3]
    u = [b[i] - a[i] for i in range(3)]
    v = [c[i] - a[i] for i in range(3)]
    normal = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2],
              u[0] * v[1] - u[1] * v[0])
    omit = max(range(3), key=lambda i: abs(normal[i]))
    axes = [i for i in range(3) if i != omit]
    return [(0.5 + p[axes[0]] / 128, 0.5 + p[axes[1]] / 128) for p in points]
