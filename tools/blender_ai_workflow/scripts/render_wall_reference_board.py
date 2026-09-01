"""Render the six-family production Wall reference board at the game camera angle."""

from __future__ import annotations

import hashlib
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
from workflow_common import asset_root, staging_path, write_json_atomic

POSITIONS = {
    "isolated": (-1.5, 0.85, 0.0),
    "end": (0.0, 0.85, 0.0),
    "straight": (1.5, 0.85, 0.0),
    "corner": (-1.5, -0.85, 0.0),
    "t_junction": (0.0, -0.85, 0.0),
    "cross": (1.5, -0.85, 0.0),
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
    bpy.ops.mesh.primitive_plane_add(size=7.0, location=(0.0, 0.0, -0.505))
    floor = bpy.context.object
    floor.name = "Wall_Reference_Floor"
    material = bpy.data.materials.new("Wall_Reference_Floor_Material")
    material.diffuse_color = (0.035, 0.028, 0.026, 1.0)
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (0.035, 0.028, 0.026, 1.0)
    shader.inputs["Roughness"].default_value = 0.95
    floor.data.materials.append(material)


def add_camera_and_lights() -> dict[str, float]:
    horizontal_angle = 59.036243
    horizontal_distance = 6.0
    target = Vector((0.0, 0.0, 0.15))
    camera_data = bpy.data.cameras.new("Wall_Reference_Camera")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = 5.2
    camera = bpy.data.objects.new("Wall_Reference_Camera", camera_data)
    bpy.context.scene.collection.objects.link(camera)
    camera.location = (
        0.0,
        -horizontal_distance,
        target.z + horizontal_distance * math.tan(math.radians(horizontal_angle)),
    )
    point_at(camera, target)
    bpy.context.scene.camera = camera

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
    rim_data.color = (0.65, 0.08, 0.7)
    rim_data.size = 3.0
    rim = bpy.data.objects.new("Wall_Reference_Rim", rim_data)
    bpy.context.scene.collection.objects.link(rim)
    rim.location = (3.5, 3.0, 5.0)
    point_at(rim, target)
    return {
        "horizontal_angle_degrees": horizontal_angle,
        "orthographic_scale": camera_data.ortho_scale,
    }


def main() -> None:
    root = asset_root()
    output = staging_path(
        root / "staging/renders/wall-production-v1-reference-board.png", "renders"
    )
    report = staging_path(
        root / "staging/reports/wall-production-v1.reference-board.json",
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
    scene.render.resolution_x = 1200
    scene.render.resolution_y = 800
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
    background.inputs["Color"].default_value = (0.012, 0.007, 0.008, 1.0)
    background.inputs["Strength"].default_value = 0.25
    scene.world = world

    objects = wall_objects()
    for family, obj in objects.items():
        obj.location = POSITIONS[family]
    add_floor()
    camera = add_camera_and_lights()
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
        "camera": camera,
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
