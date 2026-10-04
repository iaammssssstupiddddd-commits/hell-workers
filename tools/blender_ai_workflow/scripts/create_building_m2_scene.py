"""Blender-only candidate authoring; launched by build_building_m2_candidate.py.

Keeps historic clay sources intact. The coordinator owns all execution/review.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))

from building_clay_geometry import PILOTS, load_pilot, require
from building_m2_art import NUMERIC_SHA256, candidate_geometry, face_layout
from create_building_clay_scene import check_projection, configure_camera, digest
from render_color_calibration import resolve_ocio_evidence
from workflow_common import asset_root, script_arguments, staging_path, write_json_atomic


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=tuple(PILOTS))
    parser.add_argument("--name", required=True)
    parser.add_argument("--stage", choices=("planes", "strokes"), required=True)
    args = parser.parse_args(script_arguments())
    import re
    require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", args.name) is not None, "unsafe name")
    root = asset_root() / "staging"
    blend = staging_path(root / "blend" / f"{args.name}.blend", "blend")
    report = staging_path(root / "reports" / f"{args.name}.json", "reports")
    output = staging_path(root / "exports" / "building-m2" / args.name, "exports")
    require(not blend.exists() and not report.exists(), "candidate output already exists")
    require(output.is_dir() and {p.name for p in output.iterdir()} == {"albedo.png"},
            "prepare only the atlas before candidate scene creation")
    geometries, layout = candidate_geometry(args.kind), face_layout(args.kind)
    contract = load_pilot(args.kind)
    ocio = resolve_ocio_evidence()
    require(not ocio["fallback"], "candidate requires explicit OCIO evidence")
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for collection in list(bpy.data.collections):
        bpy.data.collections.remove(collection)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1
    atlas = bpy.data.images.load(str(output / "albedo.png"))
    atlas.pack()
    material = bpy.data.materials.new("M2UnapprovedPaintCandidate")
    material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Roughness"].default_value = 1
    shader.inputs["Metallic"].default_value = 0
    shader.inputs["Specular IOR Level"].default_value = 0
    texture = material.node_tree.nodes.new("ShaderNodeTexImage")
    texture.image = atlas
    material.node_tree.links.new(texture.outputs["Color"], shader.inputs["Base Color"])
    objects = {}
    for role, geometry in geometries.items():
        name = contract["roles"][role]["collection"]
        collection = bpy.data.collections.new(name)
        scene.collection.children.link(collection)
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([(x / 32, -z / 32, y / 32) for x, y, z in geometry.vertices],
                        [], geometry.faces)
        mesh.update()
        mesh.materials.append(material)
        uv = mesh.uv_layers.new(name="UVMap")
        for polygon, face in zip(mesh.polygons, layout[role], strict=True):
            for loop, point in zip(polygon.loop_indices, face["uv"], strict=True):
                uv.data[loop].uv = point
            polygon.use_smooth = False
        obj = bpy.data.objects.new(name, mesh)
        collection.objects.link(obj)
        objects[role] = obj
    # Role exports see identity nodes; review transforms are never baked twice.
    bpy.ops.wm.save_as_mainfile(filepath=str(blend), check_existing=False)
    configure_camera(contract["preview"])
    for role, obj in objects.items():
        x, y, z = contract["roles"][role]["translation_wu"]
        obj.location = (x / 32, -z / 32, y / 32)
    states = (("Empty", "Partial", "Full") if args.kind == "Tank"
              else ("IdleAngleZero", "QuarterTurn", "HalfTurn", "DiagonalPose"))
    renders = {}
    for index, state in enumerate(states):
        if args.kind == "Tank":
            spec = contract["states"][state]
            objects["water"].hide_render = not spec["water_visible"]
            objects["water"].location.z = spec["water_y_wu"] / 32
        else:
            objects["rotor"].rotation_euler.z = (0, math.pi / 2, math.pi, math.pi / 4)[index]
        bpy.context.view_layer.update()
        projection = check_projection(objects, contract["preview"])
        path = output / f"{state}.png"
        scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        renders[state] = {"path": str(path), "sha256": digest(path), "projection": projection}
    scripts = Path(__file__).resolve().parent
    write_json_atomic(report, {
        "schema_version": 1, "kind": args.kind, "stage": args.stage,
        "evidence_kind": "unapproved_m2_paint_candidate",
        "numeric_candidate_sha256": NUMERIC_SHA256,
        "numeric_freeze": False, "art_approved": False, "runtime_published": False,
        "ocio": ocio, "blend_sha256": digest(blend),
        "sources": {name: digest(scripts / name) for name in (
            "create_building_m2_scene.py", "building_m2_art.py", "building_clay_geometry.py",
            "create_building_clay_scene.py", "validate_building_numeric_candidate.py")},
        "albedo_sha256": digest(output / "albedo.png"),
        "renders": renders, "preview": contract["preview"], "face_layout": layout,
        "excludes": ["active simulation", "native", "art acceptance", "formal release"],
    })


if __name__ == "__main__":
    main()
