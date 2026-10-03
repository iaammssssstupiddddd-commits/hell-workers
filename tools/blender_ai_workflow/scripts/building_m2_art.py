"""Deterministic, unapproved paint candidate over the exact M2 numeric proposal.

No runtime authority is created here. Geometry remains the historical clay mesh;
color planes, unique per-face UV cells and strokes occupy one opaque atlas.
"""

from __future__ import annotations

import hashlib
import math

from building_clay_geometry import FIXTURES, build, load_pilot, require
from validate_building_numeric_candidate import validate

NUMERIC = FIXTURES / "building-m2-v1.numeric-candidate.json"
NUMERIC_SHA256 = "1b913ea0784bf6815a6c46ed0396dafc6ebcdcfc107370bc569aa875775d949d"
ATLAS_SIZE = 512
CELL = 32
GUTTER = 4


def candidate_geometry(kind):
    require(hashlib.sha256(NUMERIC.read_bytes()).hexdigest() == NUMERIC_SHA256,
            "numeric proposal changed: independent revision required")
    validate(NUMERIC)
    return build(kind, load_pilot(kind))


def face_layout(kind):
    """A cell per polygon; UV points retain planar proportions inside its gutter."""
    layout = {}
    index = 0
    for role, geometry in candidate_geometry(kind).items():
        layout[role] = []
        for face, surface in zip(geometry.faces, geometry.surfaces, strict=True):
            points = [geometry.vertices[i] for i in face]
            origin, b, c = points[:3]
            u = [b[i] - origin[i] for i in range(3)]
            v = [c[i] - origin[i] for i in range(3)]
            normal = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2],
                      u[0] * v[1] - u[1] * v[0]]
            length = math.sqrt(sum(n * n for n in normal))
            require(length > 0, "degenerate candidate face")
            normal = [n / length for n in normal]
            length_u = math.sqrt(sum(n * n for n in u))
            u = [n / length_u for n in u]
            v = [normal[1] * u[2] - normal[2] * u[1],
                 normal[2] * u[0] - normal[0] * u[2],
                 normal[0] * u[1] - normal[1] * u[0]]
            planar = [(sum((p[i] - origin[i]) * u[i] for i in range(3)),
                       sum((p[i] - origin[i]) * v[i] for i in range(3))) for p in points]
            low = [min(p[i] for p in planar) for i in range(2)]
            size = [max(p[i] for p in planar) - low[i] for i in range(2)]
            scale = (CELL - 2 * GUTTER - 1) / max(size)
            x, y = (index % 16) * CELL, (index // 16) * CELL
            require(y + CELL <= ATLAS_SIZE, "atlas face budget exceeded")
            pixels = [(x + GUTTER + (p[0] - low[0]) * scale,
                       y + GUTTER + (p[1] - low[1]) * scale) for p in planar]
            layout[role].append({"cell": index, "surface": surface, "normal": normal,
                                 "polygon_px": pixels,
                                 "uv": [(px / ATLAS_SIZE, 1 - py / ATLAS_SIZE)
                                        for px, py in pixels]})
            index += 1
    return layout


def paint(kind, path, stage="strokes"):
    # Pillow is already part of the workflow; Blender invokes this via the host
    # preparation entrypoint, so its bundled Python does not need Pillow.
    from PIL import Image, ImageDraw

    require(stage in ("planes", "strokes"), "unknown art stage")
    image = Image.new("RGBA", (ATLAS_SIZE, ATLAS_SIZE), (40, 35, 30, 255))
    draw = ImageDraw.Draw(image)
    palettes = {"Tank": {"body": (173, 105, 69), "water": (64, 157, 169)},
                "MudMixer": {"body": (105, 125, 83), "rotor": (184, 148, 86)}}
    layout = face_layout(kind)
    for role, faces in layout.items():
        for face in faces:
            cell = face["cell"]
            x, y = (cell % 16) * CELL, (cell // 16) * CELL
            # Plane lighting is authored into color; runtime directional light
            # remains responsible for actual scene illumination.
            tone = 0.86 + 0.12 * face["normal"][1] + 0.035 * ((cell % 3) - 1)
            base = tuple(round(channel * tone) for channel in palettes[kind][role])
            draw.rectangle((x, y, x + CELL - 1, y + CELL - 1), fill=(*base, 255))
            if stage == "planes":
                continue
            dark = tuple(round(c * 0.67) for c in base)
            light = tuple(min(255, round(c * 1.12)) for c in base)
            polygon = face["polygon_px"]
            draw.line(polygon + polygon[:1], fill=(*dark, 255), width=1)
            # Mask strokes to the authored face; no stroke leaks into neighbours.
            mask = Image.new("L", image.size)
            ImageDraw.Draw(mask).polygon(polygon, fill=255)
            layer = image.copy()
            brush = ImageDraw.Draw(layer)
            for stroke in range(3):
                yy = y + 8 + stroke * 6 + (cell % 2)
                brush.line([(x + 7, yy), (x + 15, yy - 1), (x + 25, yy)],
                           fill=(*(light if stroke % 2 == 0 else dark), 255), width=1)
            image.paste(layer, (0, 0), mask)
    image.save(path)
    return layout
