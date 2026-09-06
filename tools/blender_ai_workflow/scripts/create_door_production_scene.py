"""Create the deterministic three-state production Door scene."""

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

STATES = {
    "closed": "Door_Closed",
    "open": "Door_Open",
    "locked": "Door_Locked",
}
UV_REGIONS = {
    "wood": (0.04, 0.04, 0.48, 0.48),
    "bone": (0.04, 0.54, 0.48, 0.96),
    "iron": (0.54, 0.04, 0.96, 0.48),
}
OPEN_LEAF_ANGLE_DEGREES = 78.0
OPEN_LEAF_POSES = (
    (-13.2, 1.0, -OPEN_LEAF_ANGLE_DEGREES),
    (13.2, -1.0, OPEN_LEAF_ANGLE_DEGREES),
)


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
    face_regions: list[str],
    center_wu: tuple[float, float, float],
    dimensions_wu: tuple[float, float, float],
    region: str,
    rotation: Matrix | None = None,
) -> None:
    """Add a box using GLB coordinates (X horizontal, Y up, Z depth)."""
    start = len(vertices)
    # Blender +Y exports as glTF -Z; negate authored GLB depth explicitly.
    center = Vector((center_wu[0], -center_wu[2], center_wu[1])) / 32.0
    half = Vector(
        (dimensions_wu[0], dimensions_wu[2], dimensions_wu[1])
    ) / 64.0
    transform = Matrix.Translation(center) @ (rotation or Matrix.Identity(4))
    for x, y, z in (
        (-1, -1, -1),
        (1, -1, -1),
        (1, 1, -1),
        (-1, 1, -1),
        (-1, -1, 1),
        (1, -1, 1),
        (1, 1, 1),
        (-1, 1, 1),
    ):
        point = transform @ Vector((half.x * x, half.y * y, half.z * z))
        vertices.append(tuple(point))
    box_faces = (
        (0, 3, 2, 1),
        (4, 5, 6, 7),
        (0, 1, 5, 4),
        (1, 2, 6, 5),
        (2, 3, 7, 6),
        (3, 0, 4, 7),
    )
    faces.extend(tuple(start + index for index in face) for face in box_faces)
    face_regions.extend([region] * len(box_faces))


def add_uv(mesh: bpy.types.Mesh, face_regions: list[str]) -> None:
    uv_layer = mesh.uv_layers.new(name="UVMap")
    corners = ((0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0))
    for polygon, region_name in zip(mesh.polygons, face_regions, strict=True):
        u0, v0, u1, v1 = UV_REGIONS[region_name]
        for loop_index, (u, v) in zip(polygon.loop_indices, corners, strict=True):
            uv_layer.data[loop_index].uv = (u0 + (u1 - u0) * u, v0 + (v1 - v0) * v)
        polygon.use_smooth = False


def create_material(root: Path) -> bpy.types.Material:
    texture_path = root / "staging/exports/textures/buildings/door/door_albedo.png"
    if not texture_path.is_file():
        raise FileNotFoundError(f"Door albedo is absent: {texture_path}")
    material = bpy.data.materials.new("Door_Production_Preview")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    shader = nodes.get("Principled BSDF")
    texture = nodes.new("ShaderNodeTexImage")
    texture.image = bpy.data.images.load(str(texture_path), check_existing=True)
    material.node_tree.links.new(texture.outputs["Color"], shader.inputs["Base Color"])
    shader.inputs["Roughness"].default_value = 0.9
    shader.inputs["Metallic"].default_value = 0.0
    return material


def add_frame(vertices, faces, regions) -> None:
    add_box(vertices, faces, regions, (-14.8, 0.0, 0.0), (2.4, 32.0, 9.6), "wood")
    add_box(vertices, faces, regions, (14.8, 0.0, 0.0), (2.4, 32.0, 9.6), "wood")
    add_box(vertices, faces, regions, (0.0, 15.2, 0.0), (27.2, 1.6, 7.2), "bone")


def add_hinged_box(
    vertices,
    faces,
    regions,
    *,
    hinge_x: float,
    local_x: float,
    local_z: float,
    center_y: float,
    dimensions_wu: tuple[float, float, float],
    angle_degrees: float,
    region: str,
) -> None:
    angle = math.radians(angle_degrees)
    center_x = hinge_x + math.cos(angle) * local_x + math.sin(angle) * local_z
    center_z = -math.sin(angle) * local_x + math.cos(angle) * local_z - 3.2
    add_box(
        vertices,
        faces,
        regions,
        (center_x, center_y, center_z),
        dimensions_wu,
        region,
        Matrix.Rotation(angle, 4, "Z"),
    )


def add_hardware(vertices, faces, regions, *, opened: bool) -> None:
    if opened:
        for hinge_x, direction, angle in OPEN_LEAF_POSES:
            for y in (-7.0, 5.0):
                add_hinged_box(
                    vertices,
                    faces,
                    regions,
                    hinge_x=hinge_x,
                    local_x=direction * 0.5,
                    local_z=-1.8,
                    center_y=y,
                    dimensions_wu=(1.0, 4.0, 0.8),
                    angle_degrees=angle,
                    region="iron",
                )
            add_hinged_box(
                vertices,
                faces,
                regions,
                hinge_x=hinge_x,
                local_x=direction * 6.2,
                local_z=-2.8,
                center_y=-2.0,
                dimensions_wu=(2.2, 20.0, 0.8),
                angle_degrees=angle,
                region="bone",
            )
            for y in (-9.0, 5.0):
                add_hinged_box(
                    vertices,
                    faces,
                    regions,
                    hinge_x=hinge_x,
                    local_x=direction * 6.5,
                    local_z=-2.8,
                    center_y=y,
                    dimensions_wu=(10.0, 2.4, 0.8),
                    angle_degrees=angle,
                    region="bone",
                )
            add_hinged_box(
                vertices,
                faces,
                regions,
                hinge_x=hinge_x,
                local_x=direction * 11.0,
                local_z=-2.9,
                center_y=0.0,
                dimensions_wu=(1.8, 1.8, 1.6),
                angle_degrees=angle,
                region="iron",
            )
    else:
        for x in (-13.25, 13.25):
            for y in (-7.0, 5.0):
                add_box(vertices, faces, regions, (x, y, -6.0), (1.0, 4.0, 0.8), "iron")
        for x in (-7.0, 7.0):
            add_box(vertices, faces, regions, (x, -2.0, -6.0), (3.0, 20.0, 0.8), "bone")
        for x in (-6.7, 6.7):
            for y in (-9.0, 5.0):
                add_box(vertices, faces, regions, (x, y, -6.0), (10.0, 2.4, 0.8), "bone")
        for x in (-1.5, 1.5):
            add_box(vertices, faces, regions, (x, 0.0, -6.4), (1.6, 1.6, 1.6), "iron")


def create_state(state: str, collection: bpy.types.Collection, material: bpy.types.Material):
    vertices: list[tuple[float, float, float]] = []
    faces: list[tuple[int, int, int, int]] = []
    regions: list[str] = []
    add_frame(vertices, faces, regions)
    opened = state == "open"
    if opened:
        for hinge_x, direction, angle in OPEN_LEAF_POSES:
            add_hinged_box(
                vertices,
                faces,
                regions,
                hinge_x=hinge_x,
                local_x=direction * 6.5,
                local_z=-1.2,
                center_y=-2.0,
                dimensions_wu=(13.0, 26.4, 2.4),
                angle_degrees=angle,
                region="wood",
            )
    else:
        add_box(vertices, faces, regions, (-6.7, -2.0, -4.4), (13.0, 26.4, 2.4), "wood")
        add_box(vertices, faces, regions, (6.7, -2.0, -4.4), (13.0, 26.4, 2.4), "wood")
    add_hardware(vertices, faces, regions, opened=opened)
    if state == "locked":
        add_box(vertices, faces, regions, (0.0, 0.0, -6.8), (25.0, 2.4, 1.2), "bone")
    mesh = bpy.data.meshes.new(f"Door_{state}_Mesh")
    mesh.from_pydata(vertices, [], faces)
    mesh.update(calc_edges=True)
    add_uv(mesh, regions)
    obj = bpy.data.objects.new(f"Door_{state}", mesh)
    collection.objects.link(obj)
    obj.data.materials.append(material)
    obj["hw_asset_set_id"] = "door-production-v1"
    obj["hw_state"] = state
    obj["hw_export_scale"] = 32.0
    mesh.calc_loop_triangles()
    return obj


def main() -> None:
    root = asset_root()
    blend_path = staging_path(root / "staging/blend/door-production-v1.blend", "blend")
    report_path = staging_path(root / "staging/reports/door-production-v1.scene-create.json", "reports")
    clear_scene()
    scene = bpy.context.scene
    scene.name = "Door_Production_V1"
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    material = create_material(root)
    created = []
    for state, collection_name in STATES.items():
        collection = bpy.data.collections.new(collection_name)
        scene.collection.children.link(collection)
        obj = create_state(state, collection, material)
        created.append(
            {
                "collection": collection_name,
                "object": obj.name,
                "state": state,
                "triangles": len(obj.data.loop_triangles),
            }
        )
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)
    write_json_atomic(
        report_path,
        {
            "asset_set_id": "door-production-v1",
            "blend": str(blend_path),
            "blend_sha256": hashlib.sha256(blend_path.read_bytes()).hexdigest(),
            "blender_version": bpy.app.version_string,
            "geometry_revision": {
                "open_leaf_angles_degrees": [pose[2] for pose in OPEN_LEAF_POSES],
                "top_frame_dimensions_wu": [27.2, 1.6, 7.2],
            },
            "schema_version": 1,
            "states": created,
            "status": "created",
            "texture": str(root / "staging/exports/textures/buildings/door/door_albedo.png"),
        },
    )
    print(f"DOOR_PRODUCTION_SCENE_CREATED blend={blend_path} report={report_path}")


if __name__ == "__main__":
    main()
