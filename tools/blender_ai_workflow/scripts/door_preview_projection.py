"""Game-owned TopDown projection for fixed-canvas Door sprites.

Coordinates passed to project_glb are glTF world units before the Door's
orientation rotation. Blender authoring uses (x, -z, y) / 32 and a centered
height origin; the completed runtime Door places that origin 16 wu up.
"""

from __future__ import annotations

import math

PROFILE = "door-preview-topdown-v1"
VIEW_HEIGHT = 150.0
Z_OFFSET = 90.0
CANVAS_WU = 64.0
RESOLUTION = 256
ANCHOR_PX = (128.0, 192.0)
AUTHORING_SCALE = 32.0


def camera_settings() -> dict[str, object]:
    length = math.hypot(VIEW_HEIGHT, Z_OFFSET)
    sine, cosine = VIEW_HEIGHT / length, Z_OFFSET / length
    # Place the ground anchor 1/4 canvas below the camera's optical center.
    shift = (CANVAS_WU / AUTHORING_SCALE) * sine * 0.25
    target = (0.0, shift * sine, -0.5 + shift * cosine)
    return {
        "location": [target[0], target[1] - Z_OFFSET / AUTHORING_SCALE,
                     target[2] + VIEW_HEIGHT / AUTHORING_SCALE],
        "target": list(target),
        "ortho_scale": CANVAS_WU / AUTHORING_SCALE,
        "pixel_aspect_x": 1.0 / sine,
        "pixel_aspect_y": 1.0,
    }


def project_glb(point: tuple[float, float, float], axis: str) -> tuple[float, float]:
    x, y, z = point
    if axis == "ns":
        x, z = z, -x
    elif axis != "ew":
        raise ValueError(f"unsupported Door axis: {axis}")
    pixels_per_wu = RESOLUTION / CANVAS_WU
    return (ANCHOR_PX[0] + x * pixels_per_wu,
            ANCHOR_PX[1] + (z - (y + 16.0) * Z_OFFSET / VIEW_HEIGHT) * pixels_per_wu)


def reference_points() -> dict[str, tuple[float, float, float]]:
    return {
        "ground_center": (0.0, -16.0, 0.0),
        "left_ground": (-16.0, -16.0, 0.0),
        "right_ground": (16.0, -16.0, 0.0),
        "left_top": (-16.0, 16.0, 0.0),
        "right_top": (16.0, 16.0, 0.0),
    }


def validate_report(report: dict, hashes: dict[str, str]) -> None:
    """Reject stale, skewed or cropped render evidence, including per-axis binding."""
    expected_settings = {"profile": PROFILE, **camera_settings()}
    if report.get("projection") != expected_settings or report.get("anchor_px") != list(ANCHOR_PX):
        raise ValueError("Door preview camera contract differs")
    for axis in ("ew", "ns"):
        record = report.get("previews", {}).get(axis, {})
        evidence = record.get("projection", {})
        if (record.get("sha256") != hashes[axis]
                or evidence.get("profile") != PROFILE
                or evidence.get("all_vertices_in_canvas") is not True):
            raise ValueError(f"Door {axis} projection/hash evidence differs")
        samples = evidence.get("samples_px", {})
        if set(samples) != set(reference_points()):
            raise ValueError(f"Door {axis} reference point inventory differs")
        for name, point in reference_points().items():
            actual = samples[name]
            if (not isinstance(actual, list) or len(actual) != 2
                    or any(type(value) not in (int, float) or not math.isfinite(value)
                           for value in actual)
                    or any(abs(a - e) > 0.01 for a, e in zip(actual, project_glb(point, axis)))):
                raise ValueError(f"Door {axis} {name} projection differs")
