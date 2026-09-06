"""Render deterministic runtime and state-review Door preview PNGs."""

from __future__ import annotations

import hashlib
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from render_color_calibration import (
    configure_color_pipeline,
    load_contract,
    resolve_ocio_evidence,
)
from workflow_common import asset_root


def look_at(camera: bpy.types.Object, target: Vector) -> None:
    camera.rotation_euler = (target - camera.location).to_track_quat("-Z", "Y").to_euler()


def select_state(state: str) -> bpy.types.Object:
    selected = f"Door_{state.title()}"
    for collection in bpy.data.collections:
        if collection.name.startswith("Door_"):
            collection.hide_render = collection.name != selected
    return next(obj for obj in bpy.data.collections[selected].objects if obj.type == "MESH")


def configure_scene() -> None:
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 256
    scene.render.resolution_y = 256
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.film_transparent = True
    scene.render.image_settings.color_depth = "8"
    contract = load_contract(
        SCRIPT_DIR.parent / "fixtures/wall-color-calibration-v1.json"
    )
    configure_color_pipeline(scene, contract)

    camera_data = bpy.data.cameras.new("DoorPreviewCamera")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = 1.75
    camera = bpy.data.objects.new("DoorPreviewCamera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = Vector((1.8, -2.5, 2.2))
    look_at(camera, Vector((0.0, 0.0, -0.06)))
    scene.camera = camera

    key_data = bpy.data.lights.new("DoorPreviewKey", type="AREA")
    key_data.energy = 700.0
    key_data.shape = "DISK"
    key_data.size = 4.0
    key = bpy.data.objects.new("DoorPreviewKey", key_data)
    scene.collection.objects.link(key)
    key.location = Vector((-2.5, -3.5, 4.5))
    look_at(key, Vector((0.0, 0.0, 0.0)))

    fill_data = bpy.data.lights.new("DoorPreviewFill", type="AREA")
    fill_data.energy = 250.0
    fill_data.size = 3.0
    fill = bpy.data.objects.new("DoorPreviewFill", fill_data)
    scene.collection.objects.link(fill)
    fill.location = Vector((3.0, 1.5, 2.5))
    look_at(fill, Vector((0.0, 0.0, 0.0)))


def render(path: Path, door: bpy.types.Object, rotation: float) -> dict[str, object]:
    door.rotation_euler[2] = rotation
    bpy.context.view_layer.update()
    bpy.context.scene.render.filepath = str(path)
    bpy.ops.render.render(write_still=True)
    return {
        "bytes": path.stat().st_size,
        "path": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def main() -> None:
    root = asset_root()
    ocio = resolve_ocio_evidence()
    if ocio["fallback"]:
        raise RuntimeError("Door previews require positive OCIO evidence")
    output = root / "staging/exports/textures/buildings/door"
    output.mkdir(parents=True, exist_ok=True)
    configure_scene()
    door = select_state("closed")
    previews = {
        "ew": render(output / "door_preview_ew.png", door, 0.0),
        "ns": render(output / "door_preview_ns.png", door, math.pi / 2.0),
    }
    review_output = root / "staging/reviews/door-production-v1"
    review_output.mkdir(parents=True, exist_ok=True)
    review_previews = {}
    for state in ("closed", "open", "locked"):
        door = select_state(state)
        review_previews[state] = {
            "ew": render(review_output / f"door_{state}_ew.png", door, 0.0),
            "ns": render(review_output / f"door_{state}_ns.png", door, math.pi / 2.0),
        }
    report = root / "staging/reports/door-production-v1.previews.json"
    report.write_text(
        json.dumps(
            {
                "anchor_px": [128, 192],
                "asset_set_id": "door-production-v1",
                "ocio": ocio,
                "previews": previews,
                "review_previews": review_previews,
                "schema_version": 1,
                "status": "rendered",
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    print(f"DOOR_PREVIEWS_RENDERED report={report}")


if __name__ == "__main__":
    main()
