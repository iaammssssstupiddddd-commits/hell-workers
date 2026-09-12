"""Wall comparison canvas after the game's TopDown RtT vertical compensation."""

from __future__ import annotations

import math

PROFILE = "wall-preview-topdown-v1"
VIEW_HEIGHT = 150.0
Z_OFFSET = 90.0
AUTHORING_SCALE = 32.0
CANVAS_TILES = 8.0


def project(point: tuple[float, float, float], width: int, height: int,
            zoom_out: float = 1.0) -> tuple[float, float]:
    x, y, z = point
    pixels_per_tile = width / (CANVAS_TILES * zoom_out)
    return (width / 2 + x * pixels_per_tile,
            height / 2 - (y + z * Z_OFFSET / VIEW_HEIGHT) * pixels_per_tile)


def camera_settings() -> dict:
    return {"profile": PROFILE, "ortho_scale": CANVAS_TILES,
            "pixel_aspect_x": math.hypot(VIEW_HEIGHT, Z_OFFSET) / VIEW_HEIGHT,
            "pixel_aspect_y": 1.0}


def reference_points() -> dict[str, tuple[float, float, float]]:
    return {"origin": (0, 0, 0), "tile_x": (1, 0, 0), "tile_y": (0, 1, 0),
            "wall_height": (0, 0, 1), "thickness_ns": (.3, 0, 0),
            "thickness_ew": (0, .3, 0)}


def validate_samples(samples: dict, width: int, height: int, zoom_out: float = 1.0) -> None:
    if set(samples) != set(reference_points()):
        raise ValueError("Wall projection reference inventory differs")
    for name, point in reference_points().items():
        if len(samples[name]) != 2 or any(
            not math.isfinite(a) or abs(a - b) > .01
            for a, b in zip(samples[name], project(point, width, height, zoom_out))
        ):
            raise ValueError(f"Wall projection differs at {name}")
