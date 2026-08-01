"""Create a deterministic low-poly scene used to verify the AI asset pipeline."""

from __future__ import annotations

import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import asset_root, staging_path, write_json_atomic


def clear_scene() -> None:
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for material in list(bpy.data.materials):
        bpy.data.materials.remove(material)


def cube_uv(mesh: bpy.types.Mesh) -> None:
    uv_layer = mesh.uv_layers.new(name="UVMap")
    for polygon in mesh.polygons:
        axis = max(range(3), key=lambda index: abs(polygon.normal[index]))
        uv_axes = (1, 2) if axis == 0 else (0, 2) if axis == 1 else (0, 1)
        for loop_index in polygon.loop_indices:
            vertex = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            uv_layer.data[loop_index].uv = (
                float(vertex[uv_axes[0]]) + 0.5,
                float(vertex[uv_axes[1]]) + 0.5,
            )


def create_block(
    name: str,
    location: tuple[float, float, float],
    scale: tuple[float, float, float],
    material: bpy.types.Material,
) -> bpy.types.Object:
    mesh = bpy.data.meshes.new(f"{name}_Mesh")
    graph = bmesh.new()
    try:
        bmesh.ops.create_cube(graph, size=1.0)
        graph.to_mesh(mesh)
    finally:
        graph.free()
    mesh.update()
    cube_uv(mesh)

    obj = bpy.data.objects.new(name, mesh)
    bpy.context.scene.collection.objects.link(obj)
    obj.location = location
    obj.scale = scale
    for vertex in mesh.vertices:
        vertex.co.x *= 2.0 * scale[0]
        vertex.co.y *= 2.0 * scale[1]
        vertex.co.z *= 2.0 * scale[2]
    obj.scale = (1.0, 1.0, 1.0)
    obj.data.materials.append(material)
    obj["hw_ai_fixture"] = True
    obj["hw_ai_generator"] = "tools/blender_ai_workflow/scripts/create_smoke_scene.py"

    bevel = obj.modifiers.new(name="Edge Softening", type="BEVEL")
    bevel.width = 0.04
    bevel.segments = 2
    return obj


def create_material() -> bpy.types.Material:
    material = bpy.data.materials.new(name="InfernalBasalt")
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (0.11, 0.025, 0.018, 1.0)
    shader.inputs["Roughness"].default_value = 0.78
    shader.inputs["Metallic"].default_value = 0.05
    return material


def point_at(obj: bpy.types.Object, target: Vector) -> None:
    obj.rotation_euler = (target - obj.location).to_track_quat("-Z", "Y").to_euler()


def setup_camera_and_lights() -> None:
    camera_data = bpy.data.cameras.new("WorkflowCamera")
    camera = bpy.data.objects.new("WorkflowCamera", camera_data)
    bpy.context.scene.collection.objects.link(camera)
    camera.location = (5.6, -7.2, 4.8)
    camera_data.lens = 52
    point_at(camera, Vector((0.0, 0.0, 1.0)))
    bpy.context.scene.camera = camera

    key_data = bpy.data.lights.new("KeyLight", type="AREA")
    key_data.energy = 1000
    key_data.shape = "DISK"
    key_data.size = 4.0
    key = bpy.data.objects.new("KeyLight", key_data)
    bpy.context.scene.collection.objects.link(key)
    key.location = (3.0, -4.0, 6.0)
    point_at(key, Vector((0.0, 0.0, 1.0)))

    rim_data = bpy.data.lights.new("RimLight", type="AREA")
    rim_data.energy = 700
    rim_data.color = (1.0, 0.12, 0.03)
    rim_data.size = 3.0
    rim = bpy.data.objects.new("RimLight", rim_data)
    bpy.context.scene.collection.objects.link(rim)
    rim.location = (-4.0, 2.0, 4.0)
    point_at(rim, Vector((0.0, 0.0, 1.0)))


def main() -> None:
    root = asset_root()
    blend_path = staging_path(root / "staging/blend/ai_workflow_smoke.blend", "blend")
    render_path = staging_path(root / "staging/renders/ai_workflow_smoke.png", "renders")
    report_path = staging_path(root / "staging/reports/ai_workflow_smoke_create.json", "reports")

    clear_scene()
    scene = bpy.context.scene
    scene.name = "AI_Workflow_Smoke"
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 640
    scene.render.resolution_y = 640
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.filepath = str(render_path)
    scene.world.color = (0.008, 0.004, 0.003)

    material = create_material()
    create_block("Basalt_Base", (0.0, 0.0, 0.25), (2.2, 1.4, 0.25), material)
    create_block("Basalt_Pillar_Left", (-1.55, 0.0, 1.75), (0.38, 0.55, 1.35), material)
    create_block("Basalt_Pillar_Right", (1.55, 0.0, 1.75), (0.38, 0.55, 1.35), material)
    create_block("Basalt_Lintel", (0.0, 0.0, 3.25), (1.9, 0.55, 0.3), material)
    setup_camera_and_lights()

    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)
    bpy.ops.render.render(write_still=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)

    payload = {
        "schema_version": 1,
        "status": "created",
        "blender_version": bpy.app.version_string,
        "blend": str(blend_path),
        "render": str(render_path),
        "mesh_objects": sorted(
            obj.name for obj in bpy.context.scene.objects if obj.type == "MESH"
        ),
    }
    write_json_atomic(report_path, payload)
    print(f"SMOKE_SCENE_CREATED blend={blend_path} render={render_path} report={report_path}")


if __name__ == "__main__":
    main()
