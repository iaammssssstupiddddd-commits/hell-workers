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


def project_xz(points, polygon, wx, wz):
    """Project a world-space mark into one face's private UV cell."""
    a, b, c = points[:3]
    pa, pb, pc = polygon[:3]
    dx1, dz1, dx2, dz2 = b[0]-a[0], b[2]-a[2], c[0]-a[0], c[2]-a[2]
    determinant = dx1*dz2-dx2*dz1
    require(abs(determinant) > 0, "horizontal mark requires X/Z projection")
    dx, dz = wx-a[0], wz-a[2]
    u, v = (dx*dz2-dz*dx2)/determinant, (dx1*dz-dz1*dx)/determinant
    return tuple(pa[i]+u*(pb[i]-pa[i])+v*(pc[i]-pa[i]) for i in range(2))


def mud_mixer_plane(role, surface):
    """Assign material colors to authored parts, never to a face's light normal."""
    label, separator, side = surface.partition(":")
    require(separator == ":" and side in {"bottom", "top", "front", "right", "back", "left"},
            "unknown MudMixer face")
    if role == "rotor":
        require(label in {"rotor0", "rotor1", "rotor2"}, "unknown MudMixer rotor")
        return (112, 111, 99) if label == "rotor0" else (194, 144, 83)
    require(role == "body" and label in {f"body{index}" for index in range(10)},
            "unknown MudMixer body")
    if label == "body0":
        return (83, 58, 43) if side == "top" else (119, 79, 50)
    inside = {"body1": "right", "body2": "left", "body3": "back", "body4": "front"}
    if label in inside:
        if side == "top":
            return (190, 143, 86)  # wood lip around the real cavity
        return (101, 67, 48) if side == inside[label] else (151, 103, 64)
    if label in {"body5", "body6", "body7", "body8"}:
        return (83, 85, 78)  # runners and upright iron supports
    return (163, 112, 68)  # overhead wooden crossbeam


def paint(kind, path, stage="strokes"):
    # Pillow is already part of the workflow; Blender invokes this via the host
    # preparation entrypoint, so its bundled Python does not need Pillow.
    from PIL import Image, ImageDraw, ImageFilter

    require(stage in ("planes", "strokes"), "unknown art stage")
    image = Image.new("RGBA", (ATLAS_SIZE, ATLAS_SIZE), (40, 35, 30, 255))
    draw = ImageDraw.Draw(image)
    palettes = {"Tank": {"body": (173, 105, 69), "water": (64, 157, 169)}}
    layout = face_layout(kind)
    meshes = candidate_geometry(kind)
    for role, faces in layout.items():
        for face, indices in zip(faces, meshes[role].faces, strict=True):
            surface = face["surface"]
            if kind == "Tank" and role == "body":
                require(surface in {f"vessel:band{index}" for index in range(5)},
                        "unknown Tank vessel surface")
            cell = face["cell"]
            x, y = (cell % 16) * CELL, (cell // 16) * CELL
            # Material variation only. Do not bake a face-normal lighting term
            # into albedo; runtime illumination owns directional shading.
            # The twelve triangles of the Tank floor are one continuous wood
            # surface: cell-dependent colors would form a spurious star.
            if kind == "MudMixer":
                base = mud_mixer_plane(role, surface)
            else:
                tone = (1.0 if role == "water" else
                        0.96 if surface == "vessel:band4" else 0.96 + 0.035 * ((cell % 3) - 1))
                base = tuple(round(channel * tone) for channel in palettes[kind][role])
            draw.rectangle((x, y, x + CELL - 1, y + CELL - 1), fill=(*base, 255))
            if stage == "planes":
                continue
            dark = tuple(round(c * 0.67) for c in base)
            light = tuple(min(255, round(c * 1.12)) for c in base)
            polygon = face["polygon_px"]
            # Mask strokes to the authored face; no stroke leaks into neighbours.
            mask = Image.new("L", image.size)
            ImageDraw.Draw(mask).polygon(polygon, fill=255)
            layer = image.copy()
            brush = ImageDraw.Draw(layer)
            if role == "water":
                points = [meshes[role].vertices[i] for i in indices]
                if all(p[1] > 0 for p in points):
                    # One quiet world-space ripple design across the entire
                    # surface, not a repeated star in each triangle's UV cell.
                    for stroke in [[(-15, -8), (-6, -7), (4, -9), (12, -8)],
                                   [(-10, 8), (-2, 9), (8, 7), (15, 8)]]:
                        brush.line([project_xz(points, polygon, *point) for point in stroke],
                                   fill=(*light, 255), width=1)
            elif kind == "Tank" and surface == "vessel:band4":
                # World-aligned grain crosses the floor's triangular UV cells
                # without a per-face fan or radial brush marks.
                points = [meshes[role].vertices[i] for i in indices]
                for stroke in [[(-17, -8), (-5, -9), (7, -8), (17, -9)],
                               [(-18, 7), (-6, 8), (5, 7), (17, 8)]]:
                    brush.line([project_xz(points, polygon, *point) for point in stroke],
                               fill=(*dark, 255), width=1)
            elif kind == "Tank" and surface == "vessel:band1":
                # Painted iron straps are fixed to the outside, not to the
                # rim, water, or inside of the vessel. Construct each stripe
                # from the same world-height fractions on every side face.
                # The extra atlas pixels cover filtering at each UV seam.
                def along(a, b, fraction):
                    return tuple(a[i] * (1-fraction) + b[i] * fraction for i in range(2))

                def extend(a, b, pixels):
                    dx, dy = b[0]-a[0], b[1]-a[1]
                    length = math.hypot(dx, dy)
                    require(length > 0, "Tank iron band has a degenerate face edge")
                    return (dx / length * pixels, dy / length * pixels)

                for fraction in (0.28, 0.72):
                    top_left = along(polygon[0], polygon[3], fraction - 0.045)
                    top_right = along(polygon[1], polygon[2], fraction - 0.045)
                    bottom_left = along(polygon[0], polygon[3], fraction + 0.045)
                    bottom_right = along(polygon[1], polygon[2], fraction + 0.045)
                    dx, dy = extend(top_left, top_right, 2)
                    brush.polygon(((top_left[0]-dx, top_left[1]-dy),
                                   (top_right[0]+dx, top_right[1]+dy),
                                   (bottom_right[0]+dx, bottom_right[1]+dy),
                                   (bottom_left[0]-dx, bottom_left[1]-dy)),
                                  fill=(78, 76, 70, 255))
                    left = along(polygon[0], polygon[3], fraction)
                    right = along(polygon[1], polygon[2], fraction)
                    for spot in (0.24, 0.76):
                        center = tuple(left[i] * (1-spot) + right[i] * spot for i in range(2))
                        brush.ellipse((center[0]-1, center[1]-1, center[0]+1, center[1]+1),
                                      fill=(132, 78, 53, 255))
                mask = mask.filter(ImageFilter.MaxFilter(5))
            elif kind == "Tank" and surface == "vessel:band3":
                # The inside wall has upright grain, unlike the horizontal
                # marks on the outside and rim.
                left, right = min(p[0] for p in polygon), max(p[0] for p in polygon)
                top, bottom = min(p[1] for p in polygon), max(p[1] for p in polygon)
                for fraction in (0.28, 0.72):
                    xx = left * (1-fraction) + right * fraction
                    brush.line(((xx, top+2), (xx+1, bottom-2)), fill=(*dark, 255), width=1)
            else:
                for stroke in range(2):
                    yy = y + 8 + stroke * 6 + (cell % 2)
                    brush.line([(x + 7, yy), (x + 15, yy - 1), (x + 25, yy)],
                               fill=(*(light if stroke % 2 == 0 else dark), 255), width=1)
            image.paste(layer, (0, 0), mask)
    image.save(path)
    return layout
