"""Blender-only M3 source and state renders, invoked by the coordinator bundle tool."""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

import bpy

sys.path.insert(0, str(Path(__file__).resolve().parent))
from building_clay_geometry import require
from building_m3_art import CONTRACT, KINDS, contract, face_layout, geometry, parts, states
from create_building_clay_scene import check_projection, configure_camera, digest
from render_color_calibration import resolve_ocio_evidence
from workflow_common import asset_root, script_arguments, staging_path, write_json_atomic


def material(name, albedo, emission=None):
    result = bpy.data.materials.new(name)
    result.use_nodes = True
    shader = result.node_tree.nodes.get("Principled BSDF")
    for key, value in (("Roughness", 1), ("Metallic", 0), ("Specular IOR Level", 0)):
        shader.inputs[key].default_value = value
    node = result.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = albedo
    result.node_tree.links.new(node.outputs["Color"], shader.inputs["Base Color"])
    if emission:
        glow = result.node_tree.nodes.new("ShaderNodeTexImage")
        glow.image = emission
        result.node_tree.links.new(glow.outputs["Color"], shader.inputs["Emission Color"])
        shader.inputs["Emission Strength"].default_value = 1
    return result


def main():
    import re
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=tuple(KINDS))
    parser.add_argument("--name", required=True)
    parser.add_argument("--stage", choices=("clay", "planes", "strokes"), required=True)
    args = parser.parse_args(script_arguments())
    require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", args.name) is not None, "unsafe name")
    root = asset_root() / "staging"
    blend = staging_path(root / "blend" / f"{args.name}.blend", "blend")
    report = staging_path(root / "reports" / f"{args.name}.json", "reports")
    output = staging_path(root / "exports" / "building-m3" / args.name, "exports")
    require(not blend.exists() and not report.exists(), "use fresh outputs")
    expected = {"albedo.png", "slot_emissive.png"} if args.kind == "SoulSpa" else {"albedo.png"}
    require(output.is_dir() and {p.name for p in output.iterdir()} == expected, "prepare only M3 atlases")
    _, spec = contract(args.kind)
    ocio = resolve_ocio_evidence()
    require(not ocio["fallback"], "explicit OCIO required")
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for collection in list(bpy.data.collections):
        bpy.data.collections.remove(collection)
    scene = bpy.context.scene
    scene.unit_settings.system = "METRIC"
    scene.unit_settings.scale_length = 1
    albedo = bpy.data.images.load(str(output / "albedo.png"))
    albedo.pack()
    opaque = material("M3Opaque", albedo)
    on = None
    if args.kind == "SoulSpa":
        emission = bpy.data.images.load(str(output / "slot_emissive.png"))
        emission.pack()
        on = material("M3SlotOn", albedo, emission)
    layout = face_layout(args.kind)
    objects = {}
    for role, mesh_data in geometry(args.kind).items():
        name = spec["roles"][role]["collection"]
        collection = bpy.data.collections.new(name)
        scene.collection.children.link(collection)
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([(x/32, -z/32, y/32) for x, y, z in mesh_data.vertices], [], mesh_data.faces)
        mesh.update()
        mesh.materials.append(opaque)
        uv = mesh.uv_layers.new(name="UVMap")
        for polygon, face in zip(mesh.polygons, layout[role], strict=True):
            for loop, point in zip(polygon.loop_indices, face["uv"], strict=True):
                uv.data[loop].uv = point
        obj = bpy.data.objects.new(name, mesh)
        collection.objects.link(obj)
        objects[role] = obj
    # Identity role collections are saved before presentation-only instances/camera.
    bpy.ops.wm.save_as_mainfile(filepath=str(blend), check_existing=False)
    configure_camera(spec["preview"])
    displayed = {"body": objects["body"]}
    if args.kind == "SoulSpa":
        objects["slot"].hide_render = True
        for part in parts(args.kind)[1:]:
            obj = bpy.data.objects.new(part["name"], objects["slot"].data)
            scene.collection.objects.link(obj)
            x, y, z = part["translation_wu"]
            obj.location = (x/32, -z/32, y/32)
            obj.material_slots[0].link = "OBJECT"
            displayed[part["name"]] = obj
    construction = bpy.data.materials.new("SharedEquipmentConstructionApproximation")
    construction.use_nodes = True
    shader = construction.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Base Color"].default_value = (0.3, 0.5, 0.6, 1)
    shader.inputs["Roughness"].default_value = 1
    shader.inputs["Metallic"].default_value = 0
    shader.inputs["Specular IOR Level"].default_value = 0
    displayed["body"].material_slots[0].link = "OBJECT"
    renders = {}
    for state in states(args.kind):
        mask = int(state[4:]) if state.startswith("Mask") else 0
        displayed["body"].material_slots[0].material = construction if state == "Constructing" else opaque
        for index in range(4) if args.kind == "SoulSpa" else ():
            displayed[f"slot{index}"].material_slots[0].material = (
                construction if state == "Constructing" else on if mask & (1 << index) else opaque)
        bpy.context.view_layer.update()
        projection = check_projection(displayed, spec["preview"])
        path = output / f"{state}.png"
        scene.render.filepath = str(path)
        bpy.ops.render.render(write_still=True)
        renders[state] = {"sha256": digest(path), "projection": projection}
    # Catalog is a separate centered composition, not the ground-anchor preview.
    for obj in list(scene.objects):
        if obj.type in ("CAMERA", "LIGHT"):
            bpy.data.objects.remove(obj, do_unlink=True)
    catalog_preview = {**spec["preview"], "anchor_px": [128, 128]}
    configure_camera(catalog_preview)
    displayed["body"].material_slots[0].material = opaque
    for index in range(4) if args.kind == "SoulSpa" else ():
        displayed[f"slot{index}"].material_slots[0].material = opaque
    bpy.context.view_layer.update()
    catalog_projection = check_projection(displayed, catalog_preview)
    scene.render.filepath = str(output / "catalog.png")
    bpy.ops.render.render(write_still=True)
    scripts = Path(__file__).resolve().parent
    write_json_atomic(report, {
        "schema_version": 1, "kind": args.kind, "stage": args.stage,
        "evidence_kind": "unapproved_m3_production_proposal",
        "numeric_freeze": False, "art_approved": False, "runtime_published": False,
        "geometry_contract_sha256": digest(CONTRACT), "blend_sha256": digest(blend), "ocio": ocio,
        "sources": {name: digest(scripts / name) for name in (
            "building_m3_art.py", "create_building_m3_scene.py", "building_clay_geometry.py",
            "create_building_clay_scene.py", "build_building_m3_candidate.py", "build_building_m2_candidate.py")},
        "face_layout": layout, "parts": parts(args.kind), "preview": spec["preview"],
        "renders": renders, "catalog_sha256": digest(output / "catalog.png"), "catalog_projection": catalog_projection,
        "albedo_sha256": digest(output / "albedo.png"),
        "slot_emissive_sha256": digest(output / "slot_emissive.png") if on else None,
        "remaining": ["numeric and art decision", "native material/construction parity", "Dream lifecycle",
                      "gameplay lifecycle", "performance", "Help/review", "release"],
    })


if __name__ == "__main__":
    main()
