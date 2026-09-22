"""Create role-isolated Tank/Mixer clay sources and projection-checked draft previews.

Only staging is writable. No asset projection, promotion, or runtime publication.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import re
import sys
from pathlib import Path

import bpy
from bpy_extras.object_utils import world_to_camera_view
from mathutils import Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))

from building_clay_geometry import FIXTURES, PILOTS, build, face_uv, load_pilot, require
from render_color_calibration import configure_color_pipeline, load_contract, resolve_ocio_evidence
from workflow_common import asset_root, script_arguments, staging_path, write_json_atomic

SCRIPT_DIR = Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def create_role(geometry, name, material):
    collection = bpy.data.collections.new(name)
    bpy.context.scene.collection.children.link(collection)
    mesh = bpy.data.meshes.new(name)
    # Bake glTF X/Y/Z into Blender X/-Z/Y; object transform stays exactly identity.
    mesh.from_pydata([(x / 32, -z / 32, y / 32) for x, y, z in geometry.vertices], [], geometry.faces)
    mesh.update()
    mesh.materials.append(material)
    uv = mesh.uv_layers.new(name="UVMap")
    for polygon, face in zip(mesh.polygons, geometry.faces, strict=True):
        for loop, point in zip(polygon.loop_indices, face_uv([geometry.vertices[i] for i in face]), strict=True):
            uv.data[loop].uv = point
        # Clay faceting is diagnostic, not the final per-surface smoothing decision.
        polygon.use_smooth = False
    obj = bpy.data.objects.new(name, mesh)
    collection.objects.link(obj)
    return obj


def configure_camera(preview):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x, scene.render.resolution_y = preview["canvas_px"]
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.film_transparent = True
    configure_color_pipeline(scene, load_contract(FIXTURES / "wall-color-calibration-v1.json"))
    height, offset = 150.0, 90.0
    sine, cosine = height / math.hypot(height, offset), offset / math.hypot(height, offset)
    require(preview["canvas_px"][0] == preview["canvas_px"][1] and
            preview["canvas_wu"][0] == preview["canvas_wu"][1], "draft camera requires square canvas")
    width = preview["canvas_wu"][0] / 32
    ax, ay = preview["anchor_px"]
    resolution = preview["canvas_px"][0]
    shift = width * sine * (ay / resolution - 0.5)
    target = Vector((width * (0.5 - ax / resolution), shift * sine, shift * cosine))
    camera_data = bpy.data.cameras.new("ClayReviewCamera")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = width
    camera = bpy.data.objects.new("ClayReviewCamera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = target + Vector((0, -offset / 32, height / 32))
    camera.rotation_euler = (target - camera.location).to_track_quat("-Z", "Y").to_euler()
    scene.camera = camera
    scene.render.pixel_aspect_x = 1 / sine
    scene.render.pixel_aspect_y = 1
    for name, location, energy in (("Key", (-3, -4, 6), 700), ("Fill", (4, 2, 4), 200)):
        data = bpy.data.lights.new(name, "AREA")
        data.energy, data.size = energy, 4
        obj = bpy.data.objects.new(name, data)
        scene.collection.objects.link(obj)
        obj.location = location
        obj.rotation_euler = (-obj.location).to_track_quat("-Z", "Y").to_euler()


def check_projection(objects, preview):
    scene = bpy.context.scene
    width, height = preview["canvas_px"]
    sx, sy = width / preview["canvas_wu"][0], height / preview["canvas_wu"][1]
    samples = []
    # Ground origin and three axes detect accidental Door center-offset inheritance.
    for x, y, z in ((0, 0, 0), (32, 0, 0), (0, 32, 0), (0, 0, 32)):
        ndc = world_to_camera_view(scene, scene.camera, Vector((x, -z, y)) / 32)
        actual = (ndc.x * width, (1 - ndc.y) * height)
        expected = (preview["anchor_px"][0] + x * sx,
                    preview["anchor_px"][1] + (z - 0.6 * y) * sy)
        require(all(abs(a - b) < 0.01 for a, b in zip(actual, expected, strict=True)),
                f"draft projection mismatch: {actual} != {expected}")
        samples.append(list(actual))
    for obj in objects.values():
        if obj.hide_render:
            continue
        for vertex in obj.data.vertices:
            ndc = world_to_camera_view(scene, scene.camera, obj.matrix_world @ vertex.co)
            require(0 < ndc.x < 1 and 0 < ndc.y < 1, "draft preview would crop a vertex")
    return {"samples_px": samples, "all_visible_vertices_in_canvas": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=tuple(PILOTS))
    parser.add_argument("--name", required=True, help="Fresh, unique staging output name")
    args = parser.parse_args(script_arguments())
    require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", args.name) is not None, "unsafe output name")
    contract = load_pilot(args.kind)
    geometries = build(args.kind, contract)
    root = asset_root() / "staging"
    blend = staging_path(root / "blend" / f"{args.name}.blend", "blend")
    report = staging_path(root / "reports" / f"{args.name}.json", "reports")
    output = staging_path(root / "exports" / "building-clay" / args.name, "exports")
    require(not blend.exists() and not report.exists() and not output.exists(), "clay outputs already exist")
    ocio = resolve_ocio_evidence()
    require(not ocio["fallback"], "clay render requires explicit positive OCIO evidence")
    output.mkdir()
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for collection in list(bpy.data.collections):
        bpy.data.collections.remove(collection)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1
    neutral = bpy.data.images.new("NeutralClay", width=16, height=16)
    neutral.generated_color = (0.42, 0.42, 0.42, 1)
    neutral.filepath_raw = str(output / "albedo.png")
    neutral.file_format = "PNG"
    neutral.save()
    material = bpy.data.materials.new("NeutralClayOnly")
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Roughness"].default_value = 1
    shader.inputs["Metallic"].default_value = 0
    shader.inputs["Specular IOR Level"].default_value = 0
    texture = material.node_tree.nodes.new("ShaderNodeTexImage")
    texture.image = neutral
    material.node_tree.links.new(texture.outputs["Color"], shader.inputs["Base Color"])
    objects = {role: create_role(geometry, contract["roles"][role]["collection"], material)
               for role, geometry in geometries.items()}
    # Save identity role sources, before review-only transforms, camera or state visibility.
    bpy.ops.wm.save_as_mainfile(filepath=str(blend), check_existing=False)
    configure_camera(contract["preview"])
    for role, obj in objects.items():
        x, y, z = contract["roles"][role]["translation_wu"]
        obj.location = (x / 32, -z / 32, y / 32)
    states = ("Empty", "Partial", "Full") if args.kind == "Tank" else ("IdleAngleZero", "DiagonalPose")
    renders = {}
    for state in states:
        if args.kind == "Tank":
            value = contract["states"][state]
            objects["water"].hide_render = not value["water_visible"]
            objects["water"].location.z = value["water_y_wu"] / 32
        else:
            objects["rotor"].rotation_euler.z = 0 if state == "IdleAngleZero" else math.pi / 4
        bpy.context.view_layer.update()
        projection = check_projection(objects, contract["preview"])
        path = output / f"{state}.png"
        scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        renders[state] = {"path": str(path), "sha256": digest(path), "projection": projection}
    write_json_atomic(report, {
        "schema_version": 1, "kind": args.kind, "evidence_kind": "blender_clay_only",
        "runtime_authority": None, "art_approval": None, "ocio": ocio,
        "blender_version": bpy.app.version_string, "blend": str(blend), "blend_sha256": digest(blend),
        "geometry_contract_sha256": digest(FIXTURES / f"building-{PILOTS[args.kind]}-v1.geometry.json"),
        "generator_sha256": digest(Path(__file__)), "geometry_generator_sha256": digest(SCRIPT_DIR / "building_clay_geometry.py"),
        "albedo": {"path": str(output / "albedo.png"), "sha256": digest(output / "albedo.png")},
        "preview": contract["preview"], "renders": renders,
        "catalog": renders[states[0]],
        "roles": {role: {"triangles": geo.triangles(), "bounds_wu": geo.bounds(),
                         "face_surface_map": geo.surfaces, "uv_status": "clay planar; not painted atlas"}
                  for role, geo in geometries.items()},
    })
    print(f"BUILDING_CLAY_CREATED kind={args.kind} report={report}")


if __name__ == "__main__":
    main()
