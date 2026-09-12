"""Render deterministic runtime and state-review Door preview PNGs."""

from __future__ import annotations

import hashlib
import json
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector
from bpy_extras.object_utils import world_to_camera_view

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from render_color_calibration import (
    configure_color_pipeline,
    load_contract,
    resolve_ocio_evidence,
)
from workflow_common import asset_root
import door_preview_projection as projection


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
    settings = projection.camera_settings()
    camera_data.ortho_scale = settings["ortho_scale"]
    camera = bpy.data.objects.new("DoorPreviewCamera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = Vector(settings["location"])
    look_at(camera, Vector(settings["target"]))
    scene.camera = camera
    scene.render.pixel_aspect_x = settings["pixel_aspect_x"]
    scene.render.pixel_aspect_y = settings["pixel_aspect_y"]

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


def validate_projection(door: bpy.types.Object, axis: str) -> dict[str, object]:
    scene = bpy.context.scene
    samples = {}
    for name, point in projection.reference_points().items():
        x, y, z = point
        world = door.matrix_world @ (Vector((x, -z, y)) / projection.AUTHORING_SCALE)
        ndc = world_to_camera_view(scene, scene.camera, world)
        actual = (ndc.x * 256.0, (1.0 - ndc.y) * 256.0)
        expected = projection.project_glb(point, axis)
        if any(abs(a - e) > 0.01 for a, e in zip(actual, expected, strict=True)):
            raise RuntimeError(f"Door {axis} {name} projection mismatch: {actual} != {expected}")
        samples[name] = list(actual)
    # Check every source vertex, not just the declared anchor or alpha bbox.
    for vertex in door.data.vertices:
        ndc = world_to_camera_view(scene, scene.camera, door.matrix_world @ vertex.co)
        if not (-1e-5 <= ndc.x <= 1.00001 and -1e-5 <= ndc.y <= 1.00001):
            raise RuntimeError(f"Door {axis} vertex would be cropped: {tuple(ndc)}")
    return {"profile": projection.PROFILE, "samples_px": samples, "all_vertices_in_canvas": True}


def render(path: Path, door: bpy.types.Object, rotation: float, *, axis: str) -> dict[str, object]:
    door.rotation_euler[2] = rotation
    bpy.context.view_layer.update()
    evidence = validate_projection(door, axis)
    bpy.context.scene.render.filepath = str(path)
    bpy.ops.render.render(write_still=True)
    return {
        "bytes": path.stat().st_size,
        "path": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "projection": evidence,
    }


def main() -> None:
    root = asset_root()
    ocio = resolve_ocio_evidence()
    if ocio["fallback"]:
        raise RuntimeError("Door previews require positive OCIO evidence")
    output = root / "staging/exports/textures/buildings/door"
    output.mkdir(parents=True, exist_ok=True)
    # A copied immutable blend may retain its original workspace's absolute path.
    # Resolve the one runtime albedo from this generation, without editing the blend.
    for image in bpy.data.images:
        if Path(image.filepath).name == "door_albedo.png":
            image.filepath = str(output / "door_albedo.png")
            image.reload()
    configure_scene()
    door = select_state("closed")
    previews = {
        "ew": render(output / "door_preview_ew.png", door, 0.0, axis="ew"),
        "ns": render(output / "door_preview_ns.png", door, math.pi / 2.0, axis="ns"),
    }
    review_output = root / "staging/reviews/door-production-v1"
    review_output.mkdir(parents=True, exist_ok=True)
    review_previews = {}
    for state in ("closed", "open", "locked"):
        door = select_state(state)
        review_previews[state] = {
            "ew": render(review_output / f"door_{state}_ew.png", door, 0.0, axis="ew"),
            "ns": render(review_output / f"door_{state}_ns.png", door, math.pi / 2.0, axis="ns"),
        }
    report = root / "staging/reports/door-production-v1.previews.json"
    report.write_text(
        json.dumps(
            {
                "anchor_px": [128, 192],
                "projection": {"profile": projection.PROFILE, **projection.camera_settings()},
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
