"""Render the six-family production Wall reference board at the game camera angle."""

from __future__ import annotations

import hashlib
import argparse
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
from workflow_common import asset_root, staging_path, write_json_atomic, script_arguments
from create_wall_production_scene import PREVIEW_SURFACE_INPUTS
import wall_preview_projection as projection

POSITIONS = {
    "isolated": (-2.5, 1.9, 0.0),
    "end": (0.0, 1.9, 0.0),
    "straight": (2.5, 1.9, 0.0),
    "corner": (-2.5, 0.0, 0.0),
    "t_junction": (0.0, 0.0, 0.0),
    "cross": (2.5, 0.0, 0.0),
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def point_at(obj: bpy.types.Object, target: Vector) -> None:
    obj.rotation_euler = (target - obj.location).to_track_quat("-Z", "Y").to_euler()


def wall_objects() -> dict[str, bpy.types.Object]:
    result: dict[str, bpy.types.Object] = {}
    for obj in bpy.context.scene.objects:
        family = obj.get("hw_family")
        if obj.type == "MESH" and family in POSITIONS:
            if family in result:
                raise RuntimeError(
                    f"duplicate Wall family in reference scene: {family}"
                )
            result[family] = obj
    if set(result) != set(POSITIONS):
        raise RuntimeError(
            "reference scene does not contain the exact six Wall families"
        )
    return result


def add_floor() -> None:
    bpy.ops.mesh.primitive_plane_add(size=9.0, location=(0.0, 0.0, -0.505))
    floor = bpy.context.object
    floor.name = "Wall_Reference_Floor"
    material = bpy.data.materials.new("Wall_Reference_Floor_Material")
    material.diffuse_color = (0.035, 0.028, 0.026, 1.0)
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (0.035, 0.028, 0.026, 1.0)
    shader.inputs["Roughness"].default_value = 0.95
    floor.data.materials.append(material)


def add_camera_and_lights(*, neutral_light: bool = False, zoom_out: float = 1.0) -> dict:
    horizontal_angle = 59.036243
    horizontal_distance = 6.0
    target = Vector((0.0, 0.0, 0.0))
    camera_data = bpy.data.cameras.new("Wall_Reference_Camera")
    camera_data.type = "ORTHO"
    settings = projection.camera_settings()
    camera_data.ortho_scale = settings["ortho_scale"] * zoom_out
    camera = bpy.data.objects.new("Wall_Reference_Camera", camera_data)
    bpy.context.scene.collection.objects.link(camera)
    camera.location = (
        0.0,
        -horizontal_distance,
        target.z + horizontal_distance * math.tan(math.radians(horizontal_angle)),
    )
    point_at(camera, target)
    bpy.context.scene.camera = camera
    bpy.context.scene.render.pixel_aspect_x = settings["pixel_aspect_x"]
    bpy.context.scene.render.pixel_aspect_y = settings["pixel_aspect_y"]

    key_data = bpy.data.lights.new("Wall_Reference_Key", type="AREA")
    key_data.energy = 850.0
    key_data.shape = "DISK"
    key_data.size = 4.0
    key = bpy.data.objects.new("Wall_Reference_Key", key_data)
    bpy.context.scene.collection.objects.link(key)
    key.location = (-3.0, -4.0, 7.0)
    point_at(key, target)

    rim_data = bpy.data.lights.new("Wall_Reference_Rim", type="AREA")
    rim_data.energy = 600.0
    rim_data.color = (1.0, 1.0, 1.0) if neutral_light else (0.65, 0.08, 0.7)
    rim_data.size = 3.0
    rim = bpy.data.objects.new("Wall_Reference_Rim", rim_data)
    bpy.context.scene.collection.objects.link(rim)
    rim.location = (3.5, 3.0, 5.0)
    point_at(rim, target)
    return {
        **settings,
        "horizontal_angle_degrees": horizontal_angle,
        "orthographic_scale": camera_data.ortho_scale,
        "zoom_out": zoom_out,
        "lighting_profile": "neutral-color-review" if neutral_light else "purple-rim-reference",
    }


def add_review_guides(objects: dict) -> list:
    """Guides/copies exist only in this unsaved comparison scene, never in GLBs."""
    scene = bpy.context.scene
    guide = bpy.data.materials.new("Wall_Grid_Material")
    guide.use_nodes = True
    guide.node_tree.nodes.get("Principled BSDF").inputs["Base Color"].default_value = (.07, .065, .06, 1)
    for index in range(-4, 5):
        for axis in (0, 1):
            position = (index, 0, -.502) if axis == 0 else (0, index, -.502)
            bpy.ops.mesh.primitive_plane_add(size=1, location=position)
            line = bpy.context.object
            line.scale = (.004, 9, 1) if axis == 0 else (9, .004, 1)
            line.data.materials.append(guide)
    specimens = []
    for axis, x, angle in (("NS", -2.5, 0), ("EW", 0, math.pi / 2)):
        obj = objects["straight"].copy()
        scene.collection.objects.link(obj)
        obj.location = (x, -1.9, 0)
        obj.rotation_euler.z = angle
        specimens.append({"axis": axis, "position": list(obj.location), "rotation_z": angle})
    # Three end-to-end EW tiles expose accidental top-cap borders/seams.
    for x in (1.2, 2.2, 3.2):
        obj = objects["straight"].copy()
        scene.collection.objects.link(obj)
        obj.location = (x, -1.9, 0)
        obj.rotation_euler.z = math.pi / 2
    labels = [(family, position[0], position[1] - .9)
              for family, position in POSITIONS.items()]
    labels += [("NS", -2.5, -2.8), ("EW", 0, -2.8), ("3 connected tiles", 2.2, -2.8),
               ("Grid 32 x 32 wu | thickness 9.6 | height 32", 0, 2.85)]
    for label, x, y in labels:
        text_data = bpy.data.curves.new("Wall_Review_Label", "FONT")
        text_data.body, text_data.size, text_data.align_x = label, .09, "CENTER"
        obj = bpy.data.objects.new("Wall_Review_Label", text_data)
        scene.collection.objects.link(obj)
        obj.location = (x, y, 0)
        obj.rotation_euler = scene.camera.rotation_euler
    return specimens


def validate_projection(zoom_out: float) -> dict:
    scene = bpy.context.scene
    bpy.context.view_layer.update()
    width, height = scene.render.resolution_x, scene.render.resolution_y
    samples = {}
    for name, point in projection.reference_points().items():
        ndc = world_to_camera_view(scene, scene.camera, Vector(point))
        samples[name] = [ndc.x * width, (1 - ndc.y) * height]
    projection.validate_samples(samples, width, height, zoom_out)
    for obj in scene.objects:
        if obj.type == "MESH" and obj.get("hw_family") in POSITIONS:
            for vertex in obj.data.vertices:
                ndc = world_to_camera_view(scene, scene.camera, obj.matrix_world @ vertex.co)
                if not (0 <= ndc.x <= 1 and 0 <= ndc.y <= 1):
                    raise ValueError(f"Wall review specimen is cropped: {obj.name}")
    return {"samples_px": samples, "all_wall_vertices_in_canvas": True,
            "logical_pixels_per_wu": width / (
                projection.CANVAS_TILES * projection.AUTHORING_SCALE * zoom_out)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--neutral-light", action="store_true")
    parser.add_argument("--review-scale", choices=("detail", "standard", "farthest"), default="detail")
    args = parser.parse_args(script_arguments())
    root = asset_root()
    # A copied immutable source may reference its original staging workspace.
    # Bind only this isolated run's inputs, without modifying the saved blend.
    texture_paths = {
        name: root / "staging/exports/textures/buildings/wall" / name
        for name in ("wall_albedo.png", "wall_emissive.png")
    }
    for image in bpy.data.images:
        name = Path(image.filepath).name
        if name in texture_paths:
            if not texture_paths[name].is_file():
                raise FileNotFoundError(texture_paths[name])
            image.filepath = str(texture_paths[name])
            image.reload()
    image_name = (
        "wall-production-v1-reference-board-neutral.png" if args.neutral_light
        else "wall-production-v1-reference-board.png"
    )
    report_name = (
        "wall-production-v1.reference-board-neutral.json" if args.neutral_light
        else "wall-production-v1.reference-board.json"
    )
    if args.review_scale != "detail":
        image_name = image_name.replace(".png", f"-{args.review_scale}.png")
        report_name = report_name.replace(".json", f"-{args.review_scale}.json")
    output = staging_path(
        root / "staging/renders" / image_name, "renders"
    )
    report = staging_path(
        root / "staging/reports" / report_name,
        "reports",
    )
    contract_path = SCRIPT_DIR.parent / "fixtures/wall-color-calibration-v1.json"
    contract = load_contract(contract_path)
    ocio = resolve_ocio_evidence()
    if ocio["fallback"]:
        raise RuntimeError("reference board requires positive OCIO evidence")

    scene = bpy.context.scene
    scene.name = "Wall_Production_V1_Reference_Board"
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 1280 if args.review_scale == "detail" else 256
    scene.render.resolution_y = 960 if args.review_scale == "detail" else 192
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 15
    scene.render.filepath = str(output)
    configure_color_pipeline(scene, contract)
    world = bpy.data.worlds.new("Wall_Reference_World")
    world.use_nodes = True
    background = world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (
        (0.01, 0.01, 0.01, 1.0) if args.neutral_light else (0.012, 0.007, 0.008, 1.0)
    )
    background.inputs["Strength"].default_value = 0.25
    scene.world = world

    objects = wall_objects()
    for family, obj in objects.items():
        obj.location = POSITIONS[family]
    add_floor()
    zoom_out = 5.0 if args.review_scale == "farthest" else 1.0
    camera = add_camera_and_lights(neutral_light=args.neutral_light, zoom_out=zoom_out)
    specimens = add_review_guides(objects)
    camera.update(validate_projection(zoom_out))
    result = bpy.ops.render.render(write_still=True)
    if "FINISHED" not in result or not output.is_file():
        raise RuntimeError(f"Wall reference board render failed: {result}")

    blend_path = Path(bpy.data.filepath).resolve()
    payload = {
        "schema_version": 1,
        "status": "pass",
        "asset_set_id": "wall-production-v1",
        "blend_sha256": sha256(blend_path),
        "output": str(output),
        "output_sha256": sha256(output),
        "texture_sha256": {name: sha256(path) for name, path in texture_paths.items()},
        "surface_inputs_by_family": {
            family: {
                name: obj.data.materials[0].node_tree.nodes.get("Principled BSDF")
                .inputs[name].default_value
                for name in PREVIEW_SURFACE_INPUTS
            }
            for family, obj in objects.items()
        },
        "camera": camera,
        "review_scale": args.review_scale,
        "axis_specimens": specimens,
        "families": [
            {"family": family, "position": list(POSITIONS[family])}
            for family in POSITIONS
        ],
        "ocio": ocio,
    }
    write_json_atomic(report, payload)
    print(f"WALL_REFERENCE_BOARD status=pass output={output} report={report}")


if __name__ == "__main__":
    main()
