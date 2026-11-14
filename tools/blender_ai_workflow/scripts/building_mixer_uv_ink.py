"""Object-attached Mixer ink, independent of camera/light/face normals.

Sample one continuous local-space drawing into the established per-polygon UV
cells, including gutters. This paints material joints, never triangle borders.
No geometry, transform, numerical decision or runtime authority is created.
"""

from __future__ import annotations

import math
from PIL import Image
import building_infernal_style as palette

STYLE = "illustrated-v1"


def color(role, surface, point):
    x, y, z = point
    label = surface.split(":", 1)[0]
    if role == "rotor" and label == "drum":
        band = int(surface.split(":")[1].removeprefix("band"))
        angle = math.atan2(z, x)
        if band >= 5:
            base = (66, 50, 50)  # the real cavity, not face-facing shading
        elif band == 4:
            base = palette.WORN_EDGE  # the physical mouth's narrow folded rim
        else:
            base = palette.DRUM
        wobble = 0.18 * math.sin(5 * angle)
        if band < 4 and (abs(y + 3.6 + wobble) < 0.55 or abs(y - 3 + wobble) < 0.45):
            return (*palette.INK, 255)
        # A single broad chalk/wear mark is deliberately not half-turn symmetric.
        # Its position is drum-local: yaw, tilt and animation are applied later.
        if band < 4 and math.cos(angle + 1.0) > 0.95 and -1.5 < y < 2.4:
            return (*palette.WORN_EDGE, 255)
        if (
            band < 4
            and math.cos(angle + 2.1 + 0.04 * math.sin(y)) > 0.999
            and -9 < y < 9
        ):
            return (*palette.INK, 255)
        variation = 3 * math.sin(4 * angle + 0.25 * math.sin(0.7 * y)) + 2 * math.sin(
            0.6 * y
        )
    elif role == "rotor" and label == "vane":
        base, variation = palette.IRON, 2 * math.sin(y * 0.7 + z * 0.4)
    elif role == "body" and label == "wheel":
        radius = math.hypot(y - 6, z - 9)
        if abs(radius - 4.5) < 0.7 or radius < 1.5:
            return (*palette.INK, 255)
        base, variation = palette.WHEEL, 2 * math.sin((y - 6) * 0.8)
    elif role == "body" and label == "frame":
        base, variation = palette.IRON, 3 * math.sin(y * 0.5 + x * 0.16 + z * 0.18)
        if 22 < y < 25.5 and abs(x) > 15:
            return (*palette.RUST, 255)
        if 2 < y < 5 and -10 < x < -5 and abs(z) < 12:
            return (*palette.WORN_EDGE, 255)
    else:
        raise ValueError("unknown Mixer local ink surface")
    return (*(min(255, max(0, round(c + variation))) for c in base), 255)


def local_point(pixel, polygon, points):
    """Inverse the face's planar UV mapping, also outside it for safe gutters."""
    a, b, c = polygon[:3]
    dx, dy = pixel[0] - a[0], pixel[1] - a[1]
    bx, by, cx, cy = b[0] - a[0], b[1] - a[1], c[0] - a[0], c[1] - a[1]
    determinant = bx * cy - by * cx
    if abs(determinant) < 1e-9:
        raise ValueError("degenerate Mixer UV face")
    u, v = (dx * cy - dy * cx) / determinant, (bx * dy - by * dx) / determinant
    p, q, r = points[:3]
    return tuple(p[i] + u * (q[i] - p[i]) + v * (r[i] - p[i]) for i in range(3))


def paint(path, stage="strokes"):
    # Lazy import avoids changing the existing infernal painter's dependency path.
    from building_m2_art import ATLAS_SIZE, CELL, candidate_geometry, face_layout

    if stage not in ("planes", "strokes"):
        raise ValueError("unknown Mixer ink stage")
    image = Image.new("RGBA", (ATLAS_SIZE, ATLAS_SIZE), (*palette.INK, 255))
    pixels = image.load()
    layout = face_layout("MudMixer", "v3")
    geometries = candidate_geometry("MudMixer", "v3")
    for role, faces in layout.items():
        mesh = geometries[role]
        for face, indices in zip(faces, mesh.faces, strict=True):
            x0, y0 = (face["cell"] % 16) * CELL, (face["cell"] // 16) * CELL
            points = [mesh.vertices[index] for index in indices]
            for py in range(y0, y0 + CELL):
                for px in range(x0, x0 + CELL):
                    point = local_point(
                        (px + 0.5, py + 0.5), face["polygon_px"], points
                    )
                    pixels[px, py] = (
                        color(role, face["surface"], point)
                        if stage == "strokes"
                        else (*(palette.DRUM if role == "rotor" else palette.IRON), 255)
                    )
    image.save(path)
    return layout
