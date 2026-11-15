"""Continuous local-space fabric/bone drawing in the existing M3 UV cells.

Geometry, indexed UVs, logical slots and the original emission atlas are kept.
No lighting/face-normal shading or per-polygon outlines are baked into albedo.
"""

from __future__ import annotations

import math
from PIL import Image
import building_infernal_style as palette
from building_mixer_uv_ink import local_point


def color(kind, role, surface, point, *, stage="strokes", spec=None, style=palette.ILLUSTRATED_STYLE):
    if style == palette.READABLE_STYLE:
        if kind != "RestArea":
            raise ValueError("revised ink is RestArea-only")
        return readable_rest_color(surface, point, stage=stage)
    if style != palette.ILLUSTRATED_STYLE:
        raise ValueError("unknown M3 ink style")
    if spec is None:
        from building_m3_art import contract

        _, spec = contract(kind)
    if stage == "clay":
        return (125, 125, 125, 255)
    x, y, z = point
    material = surface.split(":", 1)[0]
    variation = 0
    if kind == "RestArea":
        base = {"cloth": palette.CLOTH, "wood": palette.WOOD, "iron": palette.IRON}[
            material
        ]
        if stage == "strokes" and material == "cloth":
            if surface == "cloth:canopy":
                if -25 < x < -12 and -18 + 0.06 * x < z < -3 + 0.06 * x:
                    base = palette.CLOTH_PATCH
                if (
                    abs(z + 12 + 0.8 * math.sin(x * 0.16)) < 0.55
                    or abs(z - 10 + math.sin(x * 0.13)) < 0.55
                ):
                    return (*palette.SEAM, 255)
                if 8 < z < 12 and abs((x + 28) % 6 - 1) < 0.65:
                    return (*palette.SEAM, 255)
            else:
                across = x if abs(z) > 20 else z
                if abs(across - 3 * math.sin(y * 0.12)) < 0.55 or abs(y - 3) < 0.5:
                    return (*palette.SEAM, 255)
                if -12 < across < -3 and 8 < y < 18:
                    base = palette.CLOTH_PATCH
            variation = 2 * math.sin(x * 0.21 + z * 0.32)
        elif stage == "strokes" and material == "wood":
            variation = 3 * math.sin(y * 0.4 + x + z)
            if abs(math.sin(x * 1.2 + z * 0.8 + 0.08 * math.sin(y * 0.2))) < 0.09:
                return (*palette.INK, 255)
        elif stage == "strokes" and material == "iron":
            base = palette.RUST
    elif kind == "SoulSpa" and role == "slot" and material == "slot":
        # Keep the quiet inactive sigil; actual workers control emission later.
        base = tuple(spec["palette"]["slot"])
    elif kind == "SoulSpa" and role == "body" and material == "bone":
        base = tuple(spec["palette"]["bone"])
        if stage == "strokes":

            def segment_coordinates(segment):
                a, b = segment
                dx, dz = b[0] - a[0], b[2] - a[2]
                length = math.hypot(dx, dz)
                t = ((x - a[0]) * dx + (z - a[2]) * dz) / (length * length)
                distance = (x - (a[0] + max(0, min(1, t)) * dx)) ** 2 + (
                    z - (a[2] + max(0, min(1, t)) * dz)
                ) ** 2
                across = (-(x - a[0]) * dz + (z - a[2]) * dx) / length
                return distance, t, across

            _, t, across = min(
                segment_coordinates(segment) for segment in spec["bone_segments"]
            )
            # One coarse crack follows each physical bone, crossing UV cells.
            # It does not depend on the polygon normal or a triangle boundary.
            if 0.12 < t < 0.88 and abs(across - 0.35 * math.sin(t * 17)) < 0.35:
                return (*palette.SEAM, 255)
            variation = 2 * math.sin(t * 18 + across * 0.35)
    else:
        raise ValueError("unknown M3 ink material")
    return (*(min(255, max(0, round(c + variation))) for c in base), 255)


def paint(kind, output, stage, *, style=palette.ILLUSTRATED_STYLE):
    from building_m3_art import contract, geometry, face_layout, paint as legacy_paint

    if (stage not in ("clay", "planes", "strokes")
            or style not in palette.ILLUSTRATED_STYLES
            or style == palette.READABLE_STYLE and kind != "RestArea"):
        raise ValueError("unknown M3 ink stage")
    # The original producer remains the only author of the emission mask.
    # Its bytes are unchanged; the new pass replaces only the diffuse atlas.
    legacy_paint(kind, output, stage, style="legacy")
    value, spec = contract(kind)
    size, cell = value["atlas"]["size_px"], value["atlas"]["cell_px"]
    image = Image.new("RGBA", (size, size), (*palette.INK, 255))
    pixels = image.load()
    meshes = geometry(kind)
    for role, faces in face_layout(kind).items():
        for face, indices in zip(faces, meshes[role].faces, strict=True):
            x0, y0 = (
                (face["cell"] % (size // cell)) * cell,
                (face["cell"] // (size // cell)) * cell,
            )
            points = [meshes[role].vertices[index] for index in indices]
            for py in range(y0, y0 + cell):
                for px in range(x0, x0 + cell):
                    point = local_point(
                        (px + 0.5, py + 0.5), face["polygon_px"], points
                    )
                    pixels[px, py] = color(
                        kind, role, face["surface"], point, stage=stage, spec=spec, style=style
                    )
    image.save(output / "albedo.png")


def readable_rest_color(surface, point, *, stage="strokes"):
    """Roof, sheltered walls and a closed entry curtain read as separate areas."""
    if stage == "clay":
        return (125, 125, 125, 255)
    x, y, z = point
    material = surface.split(":", 1)[0]
    ink, hem, roof = (39, 31, 36), (67, 51, 48), (132, 113, 93)
    if material == "cloth" and surface == "cloth:canopy":
        base = roof
        if stage == "strokes":
            # One broad cloth repair, a continuous ridge and an irregular hem.
            if -26 < x < -11 and -20 + .08*x < z < -1 + .08*x:
                base = (156, 127, 97)
            if abs(x + .18*math.sin(z*.3)) < .7:
                base = ink
            elif 1.5 < x < 3.2:
                base = (159, 138, 113)
            if abs(x) > 29.7 or abs(z) > 27.8:
                base = hem
            if abs(z + 14 + .55*math.sin(x*.2)) < .45:
                base = hem
    elif material == "cloth":
        # The physical centre curtain sits at z=23, in front of the enclosed
        # interior. Its dark, opaque folds never expose a resting Soul.
        entry = z > 21 and abs(x) < 7.2
        base = (53, 43, 51) if entry else (90, 73, 79)
        if stage == "strokes":
            if entry:
                if abs(x) > 5.4 or y > 25.5:
                    base = (145, 121, 94)
                elif abs(x - .3*math.sin(y*.4)) < .6:
                    base = ink
                elif 2.4 < abs(x) < 3.3:
                    base = (109, 88, 89)
                if 12.7 < y < 14 and abs(x) > 3.3:
                    base = (164, 141, 107)  # curtain tie, not an occupancy lamp
            elif y < 2 or abs(abs(x) - 8) < .55 and z > 24:
                base = ink
    elif material == "wood":
        base = (107, 87, 73)
        if stage == "strokes" and abs(math.sin(x*1.1 + z*.8 + .07*math.sin(y*.3))) < .12:
            base = ink
    elif material == "iron":
        base = (74, 58, 57)
    else:
        raise ValueError("unknown RestArea local ink material")
    return (*base, 255)
