"""Create the deterministic six-family production Wall authoring scene."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

import bpy

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import asset_root, staging_path, write_json_atomic

FAMILIES = {
    "isolated": {
        "collection": "Wall_Isolated",
        "outline": ((-0.15, -0.15), (0.15, -0.15), (0.15, 0.15), (-0.15, 0.15)),
    },
    "end": {
        "collection": "Wall_End",
        "outline": ((-0.15, -0.15), (0.15, -0.15), (0.15, 0.5), (-0.15, 0.5)),
    },
    "straight": {
        "collection": "Wall_Straight",
        "outline": ((-0.15, -0.5), (0.15, -0.5), (0.15, 0.5), (-0.15, 0.5)),
    },
    "corner": {
        "collection": "Wall_Corner",
        "outline": (
            (-0.5, -0.15),
            (0.15, -0.15),
            (0.15, 0.5),
            (-0.15, 0.5),
            (-0.15, 0.15),
            (-0.5, 0.15),
        ),
    },
    "t_junction": {
        "collection": "Wall_TJunction",
        "outline": (
            (-0.5, -0.15),
            (-0.15, -0.15),
            (-0.15, -0.5),
            (0.15, -0.5),
            (0.15, 0.5),
            (-0.15, 0.5),
            (-0.15, 0.15),
            (-0.5, 0.15),
        ),
    },
    "cross": {
        "collection": "Wall_Cross",
        "outline": (
            (-0.5, -0.15),
            (-0.15, -0.15),
            (-0.15, -0.5),
            (0.15, -0.5),
            (0.15, -0.15),
            (0.5, -0.15),
            (0.5, 0.15),
            (0.15, 0.15),
            (0.15, 0.5),
            (-0.15, 0.5),
            (-0.15, 0.15),
            (-0.5, 0.15),
        ),
    },
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


def atlas_uv(kind: str, x: float, y: float, z: float) -> tuple[float, float]:
    if kind == "rust":
        return (0.67 + 0.29 * x, 0.32 + 0.64 * z)
    if kind == "purple":
        return (0.03 + 0.60 * x, 0.04 + 0.22 * z)
    return (0.03 + 0.60 * x, 0.32 + 0.64 * y)


def create_prism(
    family: str,
    collection: bpy.types.Collection,
    material: bpy.types.Material,
) -> bpy.types.Object:
    definition = FAMILIES[family]
    # Every authored segment is straight and uses one atlas surface. Extra
    # collinear vertices do not alter either the silhouette or interpolated UVs.
    outline = list(definition["outline"])
    count = len(outline)
    vertices = [(x, y, -0.5) for x, y in outline]
    vertices.extend((x, y, 0.0) for x, y in outline)
    vertices.extend((x, y, 0.5) for x, y in outline)
    vertices.extend(((0.0, 0.0, 0.5), (0.0, 0.0, -0.5)))
    top_center = 3 * count
    bottom_center = top_center + 1
    faces: list[tuple[int, ...]] = []
    face_kinds: list[tuple[str, int]] = []
    for index in range(count):
        following = (index + 1) % count
        faces.append((2 * count + index, 2 * count + following, top_center))
        face_kinds.append(("top", index))
        faces.append((following, index, bottom_center))
        face_kinds.append(("bottom", index))
        faces.append((index, following, count + following, count + index))
        face_kinds.append(("side_lower", index))
        faces.append(
            (
                count + index,
                count + following,
                2 * count + following,
                2 * count + index,
            )
        )
        face_kinds.append(("side_upper", index))

    mesh = bpy.data.meshes.new(f"Wall_{family}_Mesh")
    mesh.from_pydata(vertices, [], faces)
    mesh.update(calc_edges=True)
    uv_layer = mesh.uv_layers.new(name="UVMap")
    for polygon, (kind, ordinal) in zip(mesh.polygons, face_kinds, strict=True):
        edge_group = ordinal
        surface = "stone"
        if kind == "side_lower" and edge_group % 4 == 2:
            surface = "purple"
        elif kind == "side_lower" and edge_group % 2 == 1:
            surface = "rust"
        for loop_index in polygon.loop_indices:
            vertex = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            x = min(max(float(vertex.x) + 0.5, 0.0), 1.0)
            y = min(max(float(vertex.y) + 0.5, 0.0), 1.0)
            z = (
                min(max(float(vertex.z) * 2.0, 0.0), 1.0)
                if kind == "side_upper"
                else min(max((float(vertex.z) + 0.5) * 2.0, 0.0), 1.0)
                if kind == "side_lower"
                else min(max(float(vertex.z) + 0.5, 0.0), 1.0)
            )
            uv_layer.data[loop_index].uv = atlas_uv(surface, x, y, z)
        polygon.use_smooth = False

    obj = bpy.data.objects.new(f"Wall_{family}", mesh)
    collection.objects.link(obj)
    obj.data.materials.append(material)
    obj["hw_asset_set_id"] = "wall-production-v1"
    obj["hw_family"] = family
    obj["hw_authoring_tile_size"] = 1.0
    obj["hw_export_scale"] = 32.0
    obj["hw_nominal_thickness_wu"] = 9.6
    obj["hw_ornament_envelope_wu"] = 12.8
    return obj


def create_material(root: Path) -> bpy.types.Material:
    albedo_path = root / "staging/exports/textures/buildings/wall/wall_albedo.png"
    emissive_path = root / "staging/exports/textures/buildings/wall/wall_emissive.png"
    if not albedo_path.is_file() or not emissive_path.is_file():
        raise FileNotFoundError(
            "Wall albedo and emissive candidates must exist before scene creation"
        )
    material = bpy.data.materials.new("Wall_Production_Preview")
    material.use_nodes = True
    nodes = material.node_tree.nodes
    links = material.node_tree.links
    shader = nodes.get("Principled BSDF")
    albedo = nodes.new("ShaderNodeTexImage")
    albedo.name = "Wall_Albedo"
    albedo.image = bpy.data.images.load(str(albedo_path), check_existing=True)
    emissive = nodes.new("ShaderNodeTexImage")
    emissive.name = "Wall_Emissive"
    emissive.image = bpy.data.images.load(str(emissive_path), check_existing=True)
    links.new(albedo.outputs["Color"], shader.inputs["Base Color"])
    emission_input = shader.inputs.get("Emission Color") or shader.inputs.get(
        "Emission"
    )
    if emission_input is None:
        raise RuntimeError("Blender Principled BSDF has no emission color input")
    links.new(emissive.outputs["Color"], emission_input)
    shader.inputs["Emission Strength"].default_value = 1.8
    shader.inputs["Roughness"].default_value = 0.82
    shader.inputs["Metallic"].default_value = 0.12
    return material


def main() -> None:
    root = asset_root()
    blend_path = staging_path(root / "staging/blend/wall-production-v1.blend", "blend")
    report_path = staging_path(
        root / "staging/reports/wall-production-v1.scene-create.json", "reports"
    )
    clear_scene()
    scene = bpy.context.scene
    scene.name = "Wall_Production_V1"
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1.0
    material = create_material(root)
    created = []
    for family, definition in FAMILIES.items():
        collection = bpy.data.collections.new(definition["collection"])
        scene.collection.children.link(collection)
        obj = create_prism(family, collection, material)
        mesh = obj.data
        mesh.calc_loop_triangles()
        created.append(
            {
                "family": family,
                "collection": definition["collection"],
                "object": obj.name,
                "vertices": len(mesh.vertices),
                "triangles": len(mesh.loop_triangles),
            }
        )
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path), check_existing=False)
    payload = {
        "schema_version": 1,
        "status": "created",
        "asset_set_id": "wall-production-v1",
        "blender_version": bpy.app.version_string,
        "blend": str(blend_path),
        "blend_sha256": hashlib.sha256(blend_path.read_bytes()).hexdigest(),
        "authoring_tile_size": 1.0,
        "export_scale": 32.0,
        "families": created,
        "textures": {
            "albedo": str(
                root / "staging/exports/textures/buildings/wall/wall_albedo.png"
            ),
            "emissive": str(
                root / "staging/exports/textures/buildings/wall/wall_emissive.png"
            ),
        },
    }
    write_json_atomic(report_path, payload)
    print(f"WALL_SCENE_CREATED blend={blend_path} report={report_path}")


if __name__ == "__main__":
    main()
