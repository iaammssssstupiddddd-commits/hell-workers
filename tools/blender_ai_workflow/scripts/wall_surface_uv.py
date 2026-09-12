"""Face-oriented Wall UVs and an opt-in check of the exported surface profile.

Coordinates here are Blender authoring coordinates (one tile, Z up). The
runtime keeps its existing atlas, single primitive and lit shared material.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

PROFILE = "wall-face-uv-v3"
# Dedicated 256px cut-core tile, with a two-pixel filtering gutter. Image-space
# packed rectangle is (702, 750, 260, 260); all exterior UV regions stay intact.
CORE_PACK_RECT = (702, 750, 260, 260)
# Square atlas: equal U/V scale gives equal texel density in both face axes.
SURFACES = {
    "stone": (0.03, 0.34, 0.60),
    "rust": (0.68, 0.34, 0.28),
    "purple": (0.03, 0.04, 0.22),
    # Pixel centers of the new core tile; no decorative border at tile ports.
    "core": (704.5 / 1024, 16.5 / 1024, 255 / 1024),
}


def face_uv(
    surface: str,
    start: tuple[float, float],
    end: tuple[float, float],
    point: tuple[float, float, float],
) -> tuple[float, float]:
    """Map length, not normalized edge fraction, so short walls do not stretch."""
    x, y, z = point
    if not all(math.isfinite(value) for value in (*start, *end, *point)):
        raise ValueError("non-finite Wall surface coordinate")
    u0, v0, scale = SURFACES[surface]
    if surface == "core":
        return u0 + scale * (x + 0.5), v0 + scale * (y + 0.5)
    dx, dy = end[0] - start[0], end[1] - start[1]
    if (abs(dx) > 1e-7) == (abs(dy) > 1e-7):
        raise ValueError("Wall side requires a nonzero axis-aligned edge")
    along = x if abs(dx) > 1e-7 else y
    return u0 + scale * (along + 0.5), v0 + scale * (z + 0.5)


def validate_surface_uv_glb(path: Path, family: str) -> dict:
    """Recheck actual GLB UV0, independently of the scene-create report.

    This profile is explicit: legacy Wall and formwork assets remain valid
    under their original geometry contracts, but cannot pass this new check.
    """
    from validate_wall_glb import (
        accessor_values,
        index_values,
        read_glb,
        require,
        validate_wall_glb,
    )

    report = validate_wall_glb(path, family)
    document, binary = read_glb(path)
    primitive = document["meshes"][0]["primitives"][0]
    attrs = primitive["attributes"]
    positions = accessor_values(document, binary, attrs["POSITION"])
    uvs = accessor_values(document, binary, attrs["TEXCOORD_0"])
    indices = index_values(document, binary, primitive["indices"])
    counts = {surface: 0 for surface in SURFACES}
    tolerance = 2e-6
    for offset in range(0, len(indices), 3):
        ids = indices[offset : offset + 3]
        # Blender glTF conversion: (x, y, z) -> 32 * (x, z, -y), V flips.
        points = [(positions[i][0] / 32, -positions[i][2] / 32,
                   positions[i][1] / 32) for i in ids]
        actual = [(uvs[i][0], 1 - uvs[i][1]) for i in ids]
        is_cap = max(p[2] for p in points) - min(p[2] for p in points) < tolerance
        if is_cap:
            require(abs(abs(points[0][2]) - 0.5) < tolerance,
                    "Wall cut surface must be at top or bottom")
            start, end = (0.0, 0.0), (1.0, 0.0)
            candidates = ("core",)
        else:
            pair = next(((a, b) for a in points for b in points
                         if math.hypot(a[0] - b[0], a[1] - b[1]) > tolerance), None)
            require(pair is not None, "Wall side has no horizontal extent")
            start, end = pair[0][:2], pair[1][:2]
            candidates = ("stone", "rust", "purple")
        matches = []
        for surface in candidates:
            expected = [face_uv(surface, start, end, p) for p in points]
            if all(abs(a - b) < tolerance for uv, target in zip(actual, expected)
                   for a, b in zip(uv, target)):
                matches.append(surface)
        require(len(matches) == 1,
                f"Wall surface UV mismatch at triangle {offset // 3} (cap={is_cap})")
        counts[matches[0]] += 1
    require(counts["core"] > 0 and counts["stone"] > 0,
            "Wall surface profile requires both cut core and stone sides")
    report["surface_uv"] = {"profile": PROFILE, "triangles_by_surface": counts}
    return report


def main() -> None:
    from workflow_common import staging_path, write_json_atomic

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("glb", type=Path)
    parser.add_argument("--family", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    report = validate_surface_uv_glb(args.glb, args.family)
    write_json_atomic(staging_path(args.report, "reports"), report)
    print(f"WALL_SURFACE_UV status=pass family={args.family} profile={PROFILE}")


if __name__ == "__main__":
    main()
