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


def load_pilot(kind: str, revision: str = "v1") -> dict:
    require(kind in PILOTS, "only Tank and MudMixer have clay drafts")
    require(revision == "v1" or (kind == "MudMixer" and revision in {"v2", "v3", "v4", "v5", "v6"}),
            "unknown clay geometry revision")
    contract = read_json(FIXTURES / f"building-{PILOTS[kind]}-{revision}.geometry.json")
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


def construction_mixer(config):
    """A tilted open drum on a wheeled site frame; axis stays drum-local Y."""
    body, drum = Geometry(), Geometry()
    for index, box in enumerate(config["body_boxes"]):
        label = f"frame{index}" if config.get("outlined_members") else "frame"
        body.box(box["center"], box["size"], label)
    wheels = config["wheels"]
    for cx, cy, cz in wheels["centers"]:
        wheel = Geometry()
        r, half = wheels["radius"], wheels["half_thickness"]
        wheel.lathe([(0, -half), (r, -half), (r, half), (0, half)], wheels["segments"], "wheel")
        start = len(body.vertices)
        body.vertices.extend((cx+y, cy-vx, cz+z) for vx, y, z in wheel.vertices)
        for face, surface in zip(wheel.faces, wheel.surfaces, strict=True):
            body.face((start+i for i in face), surface)
    drum.lathe(config["drum_profile"], 12, "drum")
    # Two broad internal mixing vanes, visible through the true drum mouth.
    for x in (-5, 5):
        drum.box((x*0.6, 5, 0), (1.5, 6, 7), "vane")
    return {"body": body, "rotor": drum}


def drum_sweep(points, contract):
    """Conservative continuous full turn, not sampled display angles."""
    angle = contract["construction"]["tilt_radians"]
    q = contract["roles"]["rotor"]["rotation_xyzw"]
    yaw = contract["construction"]["yaw_radians"]
    sy, cy = math.sin(yaw/2), math.cos(yaw/2)
    sx, cx = math.sin(angle/2), math.cos(angle/2)
    expected = [cy*sx, sy*cx, -sy*sx, cy*cx]
    body_rotation = contract["roles"]["body"]["rotation_xyzw"]
    require(len(body_rotation) == 4 and all(abs(a-b) < 1e-9 for a, b in
            zip(body_rotation, [0, sy, 0, cy], strict=True)), "frame yaw differs from sweep")
    require(len(q) == 4 and all(math.isfinite(a) and abs(a-b) < 1e-9
                               for a, b in zip(q, expected, strict=True)),
            "drum authored quaternion differs from sweep tilt")
    c, s = math.cos(angle), math.sin(angle)
    pivot = contract["roles"]["rotor"]["translation_wu"]
    radial = [(math.hypot(x, z), y) for x, y, z in points]
    radius = max(r for r, _ in radial)
    low_y = min(y*c-r*s for r, y in radial)+pivot[1]
    high_y = max(y*c+r*s for r, y in radial)+pivot[1]
    low_z = min(y*s-r*c for r, y in radial)+pivot[2]
    high_z = max(y*s+r*c for r, y in radial)+pivot[2]
    boxes = contract["construction"]["body_boxes"]
    chassis_top = boxes[0]["center"][1]+boxes[0]["size"][1]/2
    inner_x = abs(boxes[1]["center"][0])-boxes[1]["size"][0]/2
    require(low_y > chassis_top and radius < inner_x, "drum sweep intersects frame")
    lower = [pivot[0]-radius, low_y, low_z]
    upper = [pivot[0]+radius, high_y, high_z]
    components = [(box["center"], [size/2 for size in box["size"]]) for box in boxes]
    wheels = contract["construction"]["wheels"]
    components += [(center, [wheels["half_thickness"], wheels["radius"], wheels["radius"]])
                   for center in wheels["centers"]]
    clearances = []
    for center, half in components:
        gap = max(max(center[i]-half[i]-upper[i], lower[i]-center[i]-half[i])
                  for i in range(3))
        require(gap > contract["tolerance_wu"], "drum sweep intersects static component")
        clearances.append(gap)
    # The same reviewed yaw applies to every static part and the drum.
    # Nonintersection is invariant under this rigid transform; compute world
    # bounds from all corners of the conservative frame-space swept box.
    world = [(math.cos(yaw)*x+math.sin(yaw)*z, y,
              -math.sin(yaw)*x+math.cos(yaw)*z)
             for x in (lower[0], upper[0]) for y in (lower[1], upper[1])
             for z in (lower[2], upper[2])]
    return {"axis": [0, 1, 0], "pivot_wu": pivot,
            "rotation_xyzw": contract["roles"]["rotor"]["rotation_xyzw"],
            "radius_wu": radius,
            "bounds_min_wu": [min(p[i] for p in world) for i in range(3)],
            "bounds_max_wu": [max(p[i] for p in world) for i in range(3)],
            "frame_clearance_y_wu": low_y-chassis_top,
            "support_clearance_x_wu": inner_x-radius,
            "static_component_clearances_wu": clearances, "continuous_full_turn": True}


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
    elif kind == "MudMixer" and "drum_profile" in config:
        result = construction_mixer(config)
    elif kind == "MudMixer":
        for role, geometry in (("body", body), ("rotor", moving)):
            for index, box in enumerate(config[f"{role}_boxes"]):
                geometry.box(box["center"], box["size"], f"{role}{index}")
        if "mud_box" in config:
            body.box(config["mud_box"]["center"], config["mud_box"]["size"], "mud0")
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
