"""Create the deterministic six-family wooden Wall formwork scene."""

from __future__ import annotations

import hashlib
import math
import sys
from pathlib import Path

import bpy
from mathutils import Matrix, Vector

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import asset_root, staging_path, write_json_atomic

FAMILIES = {
    "isolated": ("Wall_Formwork_Isolated", ()),
    "end": ("Wall_Formwork_End", ("N",)),
    "straight": ("Wall_Formwork_Straight", ("N", "S")),
    "corner": ("Wall_Formwork_Corner", ("N", "W")),
    "t_junction": ("Wall_Formwork_TJunction", ("N", "S", "W")),
    "cross": ("Wall_Formwork_Cross", ("N", "S", "W", "E")),
}


def clear_scene() -> None:
    for collection in list(bpy.data.collections):
        bpy.data.collections.remove(collection)
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    for mesh in list(bpy.data.meshes):
        bpy.data.meshes.remove(mesh)
    for material in list(bpy.data.materials):
        bpy.data.materials.remove(material)


def add_box(
    vertices: list[tuple[float, float, float]],
    faces: list[tuple[int, int, int, int]],
    center: tuple[float, float, float],
    dimensions: tuple[float, float, float],
    rotation: Matrix | None = None,
) -> None:
    start = len(vertices)
    half = Vector(
        (dimensions[0] / 64.0, dimensions[1] / 64.0, dimensions[2] / 64.0)
    )
    transform = Matrix.Translation(Vector(center) / 32.0) @ (
        rotation or Matrix.Identity(4)
    )
    for x, y, z in (
        (-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1),
        (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1),
    ):
        point = transform @ Vector((half.x * x, half.y * y, half.z * z))
        vertices.append(tuple(point))
    faces.extend(
        tuple(start + index for index in face)
        for face in (
            (0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
            (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7),
        )
    )


def rotate_xy(direction: str) -> Matrix:
    angle = {"N": 0.0, "W": math.pi / 2.0, "S": math.pi, "E": -math.pi / 2.0}[direction]
    return Matrix.Rotation(angle, 4, "Z")


def rotated_point(direction: str, point: tuple[float, float, float]) -> tuple[float, float, float]:
    return tuple(rotate_xy(direction) @ Vector(point))


def add_arm(
    vertices: list[tuple[float, float, float]],
    faces: list[tuple[int, int, int, int]],
    direction: str,
) -> None:
    rotation = rotate_xy(direction)
    add_box(vertices, faces, rotated_point(direction, (0.0, 15.2, 0.0)), (9.6, 1.6, 32.0), rotation)
    for height in (-11.2, 11.2):
        add_box(vertices, faces, rotated_point(direction, (0.0, 8.4, height)), (9.6, 12.0, 3.2), rotation)
    brace_center = rotated_point(direction, (0.0, 8.4, 0.0))
    brace_angle = math.atan2(20.0, 12.0)
    brace_rotation = rotation @ Matrix.Rotation(-brace_angle, 4, "X")
    add_box(vertices, faces, brace_center, (2.4, math.hypot(12.0, 20.0), 1.6), brace_rotation)


def add_uv(mesh: bpy.types.Mesh) -> None:
    uv_layer = mesh.uv_layers.new(name="UVMap")
    for polygon in mesh.polygons:
        normal = polygon.normal
        for loop_index in polygon.loop_indices:
            point = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            point_wu = point * 32.0
            if abs(normal.z) > 0.5:
                uv = (point_wu.x / 12.8 + 0.5, point_wu.y / 32.0 + 0.5)
            elif abs(normal.x) > 0.5:
                uv = (point_wu.y / 32.0 + 0.5, point_wu.z / 32.0 + 0.5)
            else:
                uv = (point_wu.x / 32.0 + 0.5, point_wu.z / 32.0 + 0.5)
            uv_layer.data[loop_index].uv = uv
        polygon.use_smooth = False


def create_material(root: Path) -> bpy.types.Material:
    texture_path = root / "staging/exports/textures/buildings/wall/wall_formwork_albedo.png"
    if not texture_path.is_file():
        raise FileNotFoundError(f"formwork albedo is absent: {texture_path}")
    material = bpy.data.materials.new("Wall_Formwork_Preview")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    shader = nodes.get("Principled BSDF")
    albedo = nodes.new("ShaderNodeTexImage")
    albedo.image = bpy.data.images.load(str(texture_path), check_existing=True)
    links.new(albedo.outputs["Color"], shader.inputs["Base Color"])
    shader.inputs["Roughness"].default_value = 0.9
    shader.inputs["Metallic"].default_value = 0.0
    return material


def create_family(
    family: str,
    collection: bpy.types.Collection,
    material: bpy.types.Material,
) -> bpy.types.Object:
    vertices: list[tuple[float, float, float]] = []
    faces: list[tuple[int, int, int, int]] = []
    arms = FAMILIES[family][1]
    if family == "isolated":
        add_box(vertices, faces, (-4.0, 0.0, 0.0), (2.4, 3.2, 32.0))
        add_box(vertices, faces, (4.0, 0.0, 0.0), (2.4, 3.2, 32.0))
        add_box(vertices, faces, (0.0, 0.0, -11.2), (10.4, 3.2, 3.2))
        add_box(vertices, faces, (0.0, 0.0, 11.2), (10.4, 3.2, 3.2))
        brace_rotation = Matrix.Rotation(-math.atan2(8.0, 20.0), 4, "Y")
        add_box(vertices, faces, (0.0, 0.0, 0.0), (2.4, 1.6, math.hypot(8.0, 20.0)), brace_rotation)
    else:
        add_box(vertices, faces, (0.0, 0.0, 0.0), (4.8, 4.8, 32.0))
        for direction in arms:
            add_arm(vertices, faces, direction)
    mesh = bpy.data.meshes.new(f"Wall_Formwork_{family}_Mesh")
    mesh.from_pydata(vertices, [], faces)
    mesh.update(calc_edges=True)
    add_uv(mesh)
    obj = bpy.data.objects.new(f"Wall_Formwork_{family}", mesh)
    collection.objects.link(obj)
    obj.data.materials.append(material)
    obj["hw_asset_set_id"] = "wall-production-v1"
    obj["hw_family"] = family
    obj["hw_stage"] = "formwork"
    obj["hw_export_scale"] = 32.0
    return obj


def main() -> None:
    root = asset_root()
    blend_path = staging_path(root / "staging/blend/wall-formwork-v1.blend", "blend")
    report_path = staging_path(root / "staging/reports/wall-formwork-v1.scene-create.json", "reports")
    clear_scene()
    scene = bpy.context.scene
    scene.name = "Wall_Formwork_V1"
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    material = create_material(root)
    created = []
    for family, (collection_name, arms) in FAMILIES.items():
        collection = bpy.data.collections.new(collection_name)
        scene.collection.children.link(collection)
        obj = create_family(family, collection, material)
        obj.data.calc_loop_triangles()
        created.append({
            "arms": list(arms),
            "collection": collection_name,
            "family": family,
            "object": obj.name,
            "triangles": len(obj.data.loop_triangles),
        })
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)
    write_json_atomic(report_path, {
        "asset_set_id": "wall-production-v1",
        "blend": str(blend_path),
        "blend_sha256": hashlib.sha256(blend_path.read_bytes()).hexdigest(),
        "blender_version": bpy.app.version_string,
        "families": created,
        "schema_version": 1,
        "status": "created",
        "texture": str(root / "staging/exports/textures/buildings/wall/wall_formwork_albedo.png"),
    })
    print(f"WALL_FORMWORK_SCENE_CREATED blend={blend_path} report={report_path}")


if __name__ == "__main__":
    main()
