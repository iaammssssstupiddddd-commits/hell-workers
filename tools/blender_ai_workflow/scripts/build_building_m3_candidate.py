"""Coordinator-only M3 build/verify/prepare; writes staging or a fresh isolated view.

No promotion, installation, release, numeric freeze or art approval is inferred.
All Blender/codec/validation execution belongs to the coordinator.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

from building_infernal_style import STYLE, ILLUSTRATED_STYLES
from build_building_m2_candidate import illustrated_environment, illustrated_toolchain, illustrated_outputs

import building_asset_pipeline as pipeline
from building_clay_geometry import FIXTURES, read_json, require
from building_m3_art import KINDS, contract, contract_path, face_layout, geometry, paint, parts, states
from validate_building_m2_indexed_faces import check_m3_indexed_faces as check_uv
from validate_wall_glb import accessor_values, exact_identity_node, index_values, read_glb
from workflow_common import asset_root, require_within, write_json_atomic

WORKFLOW = FIXTURES.parent
SOURCES = ("building_m3_art.py", "create_building_m3_scene.py", "building_clay_geometry.py", "building_infernal_style.py",
           "create_building_clay_scene.py", "build_building_m3_candidate.py", "build_building_m2_candidate.py",
           "validate_building_m2_indexed_faces.py")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(kind, name):
    require(kind in KINDS and re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None, "invalid kind/name")
    root = asset_root() / "staging"
    return tuple(require_within(root / area / suffix, root / area, "M3 candidate")
                 for area, suffix in (("blend", f"{name}.blend"), ("exports", f"building-m3/{name}"),
                                      ("reports", f"{name}.json"), ("reports", f"building-m3/{name}")))


def verify_mesh(path, mesh, spec, layout):
    document, binary = read_glb(path)
    require(len(document.get("nodes", [])) == 1 and exact_identity_node(document["nodes"][0])
            and document["nodes"][0].get("mesh") == 0 and not document["nodes"][0].get("children")
            and len(document.get("meshes", [])) == 1, "expected identity single role mesh")
    require(len(document["meshes"][0]["primitives"]) == 1, "single primitive required")
    for key in ("animations", "skins", "images", "textures", "extensionsUsed", "extensionsRequired"):
        require(not document.get(key), "unexpected GLB feature: " + key)
    primitive = document["meshes"][0]["primitives"][0]
    require(primitive.get("mode", 4) == 4 and not primitive.get("targets"), "static triangles only")
    require(set(primitive["attributes"]) == {"POSITION", "NORMAL", "TEXCOORD_0"}, "attributes differ")
    indices = index_values(document, binary, primitive["indices"])
    points = accessor_values(document, binary, primitive["attributes"]["POSITION"])
    require(len(indices) % 3 == 0 and len(indices)//3 == mesh.triangles() <= spec["triangle_cap"],
            "triangle inventory differs")
    require(all(0 <= i < len(points) for i in indices), "bad vertex index")
    actual = [[min(p[i] for p in points) for i in range(3)], [max(p[i] for p in points) for i in range(3)]]
    require(all(abs(a-b) < 0.001 for bound, expected in zip(actual, mesh.bounds(), strict=True)
                for a, b in zip(bound, expected, strict=True)), "bounds/scale differ")
    check_uv(path, mesh, layout)
    return {"bounds_wu": actual, "triangles": len(indices)//3, "sha256": digest(path)}


def selected_style(kind, style):
    value = style or (STYLE if kind == "RestArea" else "legacy")
    require(value in ("legacy", STYLE, *ILLUSTRATED_STYLES)
            and (value not in (STYLE, "illustrated-v2", "handpaint-v1") or kind == "RestArea"), "unknown M3 style/kind")
    return value


def verify(kind, name, style=None, revision="v2"):
    from PIL import Image
    blend, output, report, reports = paths(kind, name)
    value, spec = contract(kind, revision)
    style = selected_style(kind, style)
    require((revision in ("v4", "v5", "v6", "v7", "v8")) == (style == "handpaint-v1"), "handpaint requires explicit Rest v4/v5/v6/v7")
    require(revision != "v3" or style == "illustrated-v2", "M3 v3 requires revised ink")
    data = read_json(report)
    frozen_outputs = illustrated_outputs(blend, output, report, reports) if style in ILLUSTRATED_STYLES else None
    if style in ILLUSTRATED_STYLES:
        require(pipeline.exact_json(data.get("source_toolchain"), illustrated_toolchain()),
                "illustrated source toolchain changed")
    require(data["schema_version"] == 1 and data["kind"] == kind
            and data["evidence_kind"] == "unapproved_m3_production_proposal"
            and data.get("art_style") == style
            and data["geometry_contract_sha256"] == digest(contract_path(kind, revision))
            and all(data[k] is False for k in ("numeric_freeze", "art_approved", "runtime_published")),
            "wrong report or approval claim")
    require(data["blend_sha256"] == digest(blend) and data["ocio"]["fallback"] is False, "source/OCIO drift")
    require(data["sources"] == {s: digest(WORKFLOW / "scripts" / s) for s in
            SOURCES + (("building_m3_uv_ink.py", "building_mixer_uv_ink.py") if style in ILLUSTRATED_STYLES else ())
            + (("building_handpaint_materials.py", "verify_building_handpaint_blend.py") if style == "handpaint-v1" else ())}, "generator drift")
    if style == "handpaint-v1":
        from building_handpaint_materials import source_snapshot, verify_saved_sources
        require(pipeline.exact_json(data.get("handpaint_sources"), source_snapshot()), "handpaint input snapshot differs")
        verify_saved_sources(blend, illustrated_environment())
    layout = face_layout(kind, revision)
    require(data["face_layout"] == json.loads(json.dumps(layout)) and data["parts"] == parts(kind, revision)
            and data["preview"] == spec["preview"], "layout/parts/projection drift")
    atlas_names = ["albedo"] + (["slot_emissive"] if kind == "SoulSpa" else [])
    with tempfile.TemporaryDirectory() as directory:
        paint(kind, Path(directory), data["stage"], style=style, revision=revision)
        for role in atlas_names:
            path = output / f"{role}.png"
            require(data[f"{role}_sha256"] == digest(path), "atlas hash drift")
            with Image.open(path) as actual, Image.open(Path(directory) / path.name) as expected:
                require(actual.mode == "RGBA" and actual.size == (value["atlas"]["size_px"],)*2
                        and actual.tobytes() == expected.tobytes(), "atlas pixel drift")
    require(set(data["renders"]) == set(states(kind)), "missing state renders")
    preview = spec["preview"]
    ax, ay = preview["anchor_px"]
    scale = preview["canvas_px"][0] / preview["canvas_wu"][0]
    samples = [(ax, ay), (ax+32*scale, ay), (ax, ay-19.2*scale), (ax, ay+32*scale)]
    for state in states(kind) + ["catalog"]:
        path = output / f"{state}.png"
        if state == "catalog":
            require(data["catalog_sha256"] == digest(path), "catalog drift")
            projection = data["catalog_projection"]
            expected_samples = [(x + 128 - ax, y + 128 - ay) for x, y in samples]
        else:
            record = data["renders"][state]
            require(record["sha256"] == digest(path), "render drift")
            projection = record["projection"]
            expected_samples = samples
        require(projection["all_visible_vertices_in_canvas"] is True
                and len(projection["samples_px"]) == 4
                and all(len(a) == 2 and all(abs(x-y) < 0.01 for x, y in zip(a, b, strict=True))
                        for a, b in zip(projection["samples_px"], expected_samples, strict=True)), "projection drift")
        with Image.open(path) as image:
            bounds = image.getchannel("A").getbbox() if image.mode == "RGBA" else None
            require(image.size == tuple(preview["canvas_px"]) and bounds is not None
                    and bounds[0] > 0 and bounds[1] > 0 and bounds[2] < image.width
                    and bounds[3] < image.height, "empty/clipped preview")
    inventory = {}
    for role, mesh in geometry(kind, revision).items():
        path = output / f"{role}.glb"
        export = read_json(reports / f"{role}.glb.export.json")
        require(export["status"] == "exported" and export["sha256"] == digest(path)
                and export["bytes"] == path.stat().st_size and export["source_blend"] == str(blend)
                and export["output"] == str(path) and export["collection"] == spec["roles"][role]["collection"]
                and export["geometry_scale"] == 32 and export["materials_mode"] == "placeholder"
                and export["validation"]["summary"]["errors"] == 0, "export provenance drift")
        inventory[role] = verify_mesh(path, mesh, spec["roles"][role], layout[role])
        subprocess.run([str(WORKFLOW / "bin/gltf-validate"), str(path)], check=True, timeout=60,
                       **({"env": illustrated_environment()} if style in ILLUSTRATED_STYLES else {}))
    if style in ILLUSTRATED_STYLES:
        require(pipeline.exact_json(data["source_toolchain"], illustrated_toolchain())
                and read_json(report) == data
                and illustrated_outputs(blend, output, report, reports) == frozen_outputs,
                "illustrated source changed during verification")
    return {"schema_version": 1, "kind": kind, "stage": data["stage"], "scope": "m3-technical-only",
            "geometry_contract_sha256": digest(contract_path(kind, revision)), "inventory": inventory,
            "art_approved": False, "runtime_published": False,
            **({"source_inventory": frozen_outputs, "source_toolchain": data["source_toolchain"]}
               if style in ILLUSTRATED_STYLES else {})}


def build_bundle(kind, name, stage, style=None, revision="v2"):
    style = selected_style(kind, style)
    contract(kind, revision)
    require((revision in ("v4", "v5", "v6", "v7", "v8")) == (style == "handpaint-v1"), "handpaint requires explicit Rest v4/v5/v6/v7")
    require(revision != "v3" or style == "illustrated-v2", "M3 v3 requires revised ink")
    blend, output, report, reports = paths(kind, name)
    require(not any(path.exists() for path in (blend, output, report, reports)), "use fresh output name")
    require(style != "handpaint-v1" or stage == "strokes"
            or (kind == "RestArea" and revision in ("v7", "v8") and stage == "planes"),
            "handpaint planes require the explicit Rest v7 shape-only proposal")
    frozen = illustrated_toolchain() if style in ILLUSTRATED_STYLES else None
    output.mkdir(parents=True)
    paint(kind, output, stage, style=style, revision=revision)
    env = illustrated_environment() if style in ILLUSTRATED_STYLES else dict(os.environ, BLENDER_SAFE_NO_NETWORK="1", OCIO=str(FIXTURES / "wall-calibration-v2.ocio"))
    subprocess.run([str(WORKFLOW / "bin/blender-safe"), "--background", "--factory-startup",
                    "--python-exit-code", "2", "--python", str(WORKFLOW / "scripts/create_building_m3_scene.py"),
                    "--", kind, "--name", name, "--stage", stage, "--style", style, "--revision", revision], check=True, timeout=600, env=env)
    _, spec = contract(kind, revision)
    for role, role_spec in spec["roles"].items():
        subprocess.run([str(WORKFLOW / "bin/export-staging-glb"), str(blend), f"building-m3/{name}/{role}.glb",
                        str(role_spec["triangle_cap"]), "--collection", role_spec["collection"],
                        "--geometry-scale", "32", "--materials-mode", "placeholder"], check=True, timeout=180, env=env)
    if frozen is not None:
        require(pipeline.exact_json(frozen, illustrated_toolchain()), "illustrated toolchain changed during build")
        data = read_json(report)
        data["source_toolchain"] = frozen
        write_json_atomic(report, data)
    result = verify(kind, name, style, revision)
    write_json_atomic(reports / "bundle.json", result)
    return result


def recipe(kind, generation, source, revision="v2"):
    _, spec = contract(kind, revision)
    pipeline.identity(kind, generation, "art_preview")
    def record(name):
        return pipeline.record(name, (source / name).read_bytes())
    artifacts = [{"role": "mesh:" + role, **record(role + ".glb")} for role in spec["roles"]]
    images = [("albedo", "albedo.png")]
    if kind == "SoulSpa":
        images.append(("slot_emissive", "slot_emissive.png"))
    images.extend([("world_preview", "Empty.png" if kind == "RestArea" else "Mask00.png"),
                   ("catalog", "catalog.png")])
    artifacts.extend({"role": "image:" + role, **record(name)} for role, name in images)
    return {"schema_version": 1, "kind": kind, "generation": generation,
            "source": record("source.blend"), "geometry_contract": record("geometry.json"),
            "artifacts": artifacts, "parts": parts(kind, revision),
            "world_preview": {"image_role": "world_preview", **spec["preview"]},
            "catalog_preview": {"image_role": "catalog", **spec["preview"], "anchor_px": [128, 128]}}


def prepare(kind, name, generation, destination, codec, style=None, revision="v2"):
    style = selected_style(kind, style)
    contract(kind, revision)
    require((revision in ("v4", "v5", "v6", "v7", "v8")) == (style == "handpaint-v1"), "handpaint requires explicit Rest v4/v5/v6/v7")
    require(revision != "v3" or style == "illustrated-v2", "M3 v3 requires revised ink")
    result = verify(kind, name, style, revision)
    require(result["stage"] == "strokes", "runtime packaging requires final stroke proposal")
    destination = pipeline.no_symlinks(destination)
    require(not destination.exists(), "use fresh runtime destination")
    blend, output, report, reports = paths(kind, name)
    def unchanged():
        require(illustrated_outputs(blend, output, report, reports) == result["source_inventory"]
                and pipeline.exact_json(illustrated_toolchain(), result["source_toolchain"]),
                "verified illustrated inputs changed during preparation")
    if style in ILLUSTRATED_STYLES:
        unchanged()
    _, spec = contract(kind, revision)
    files = {"source.blend": blend.read_bytes(), "geometry.json": contract_path(kind, revision).read_bytes(), "paint-report.json": report.read_bytes()}
    names = [f"{role}.glb" for role in spec["roles"]] + [f"{state}.png" for state in states(kind)] + ["albedo.png", "catalog.png"]
    if kind == "SoulSpa":
        names.append("slot_emissive.png")
    files.update({name: (output / name).read_bytes() for name in names})
    origins = {"source.blend": blend, "geometry.json": contract_path(kind, revision), "paint-report.json": report,
               **{name: output / name for name in names}}
    if style == "handpaint-v1":
        from building_handpaint_materials import source_paths
        origins.update(source_paths())
        files.update({name: path.read_bytes() for name,path in source_paths().items()})
    if style in ILLUSTRATED_STYLES:
        require(all(pipeline.digest(payload) == (result["geometry_contract_sha256"] if name == "geometry.json"
                    else result["source_inventory"].get(str(origins[name]), result["source_toolchain"].get("files", {}).get(str(origins[name])))) for name, payload in files.items()),
                "copied illustrated source was not verified")
        unchanged()
    source = destination / "authoring"
    for filename, data in files.items():
        pipeline.put(pipeline.rooted(source, filename), data)
    recipe_path = destination / "recipe.json"
    value = recipe(kind, generation, source, revision)
    if style in ILLUSTRATED_STYLES:
        value["surface_paint"] = {"method": "uv_ink_v1"}
    pipeline.put(recipe_path, pipeline.canonical(value))
    exported = pipeline.export(recipe_path, source, destination / "runtime", codec, authority="art_preview")
    if style in ILLUSTRATED_STYLES:
        pipeline.verify_set(destination / "runtime", kind, codec)
        unchanged()
        require({path.name: pipeline.digest(path.read_bytes()) for path in source.iterdir()}
                == {name: pipeline.digest(data) for name, data in files.items()},
                "prepared source copy changed around export")
    result.update(identity=exported["identity"], runtime_root=str(destination / "runtime"),
                  remaining=["numeric/art decision", "all-consumer native lifecycle", "performance", "Help/fixed review",
                             "independent candidate acceptance", "promotion/install/rollback/release"])
    pipeline.put(destination / "m3-candidate-runtime.json", pipeline.canonical(result))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "verify", "prepare"))
    parser.add_argument("kind", choices=tuple(KINDS))
    parser.add_argument("--name", required=True)
    parser.add_argument("--stage", choices=("clay", "planes", "strokes"), default="clay")
    parser.add_argument("--revision", choices=("v2", "v3", "v4", "v5", "v6", "v7", "v8"), default="v2")
    parser.add_argument("--style", choices=("legacy", STYLE, *ILLUSTRATED_STYLES))
    parser.add_argument("--generation", type=int)
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--codec", type=Path)
    args = parser.parse_args()
    if args.action == "build":
        result = build_bundle(args.kind, args.name, args.stage, args.style, args.revision)
    elif args.action == "verify":
        result = verify(args.kind, args.name, args.style, args.revision)
    else:
        require(all(v is not None for v in (args.generation, args.destination, args.codec)), "missing prepare arguments")
        result = prepare(args.kind, args.name, args.generation, args.destination, args.codec, args.style, args.revision)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
