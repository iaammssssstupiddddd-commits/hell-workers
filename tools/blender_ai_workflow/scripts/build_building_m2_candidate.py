"""Coordinator entrypoint for unapproved M2 paint candidates, never release.

Build planes and strokes under different fresh names. Prepare only exposes an
explicit ArtPreview runtime view through the existing M1-c codec/export path.
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

from PIL import Image

import building_asset_pipeline as pipeline
from building_clay_geometry import FIXTURES, PILOTS, load_pilot, read_json, require
from building_m2_art import NUMERIC, NUMERIC_SHA256, candidate_geometry, face_layout, paint
from prepare_building_clay_runtime import recipe, state_geometry
from validate_wall_glb import accessor_values, read_glb
from workflow_common import asset_root, require_within, write_json_atomic

WORKFLOW = FIXTURES.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(kind, name):
    require(kind in PILOTS and re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None,
            "invalid candidate kind/name")
    root = asset_root() / "staging"
    return tuple(require_within(root / area / suffix, root / area, "M2 candidate")
                 for area, suffix in (("blend", f"{name}.blend"),
                                      ("exports", f"building-m2/{name}"),
                                      ("reports", f"{name}.json"),
                                      ("reports", f"building-m2/{name}")))


def check_uv(glb, geometry, faces):
    document, binary = read_glb(glb)
    attributes = document["meshes"][0]["primitives"][0]["attributes"]
    positions = accessor_values(document, binary, attributes["POSITION"])
    uvs = accessor_values(document, binary, attributes["TEXCOORD_0"])

    def key(point, uv):
        return tuple(round(value, 5) for value in (*point, *uv))

    # Blender UV origin is bottom-left; the glTF exporter flips V to top-left.
    expected = {key(geometry.vertices[index], (uv[0], 1 - uv[1]))
                for polygon, face in zip(geometry.faces, faces, strict=True)
                for index, uv in zip(polygon, face["uv"], strict=True)}
    require({key(point, uv) for point, uv in zip(positions, uvs, strict=True)} == expected,
            "exported per-face UV differs from exact numeric/atlas candidate")


def verify(kind, name):
    blend, output, report, reports = paths(kind, name)
    geometry = candidate_geometry(kind)
    contract = load_pilot(kind)
    data = read_json(report)
    require(data["schema_version"] == 1 and data["kind"] == kind
            and data["evidence_kind"] == "unapproved_m2_paint_candidate"
            and data["numeric_candidate_sha256"] == NUMERIC_SHA256,
            "candidate report identity mismatch")
    require(all(data[key] is False for key in ("numeric_freeze", "art_approved", "runtime_published")),
            "candidate report cannot grant approval")
    require(data["blend_sha256"] == digest(blend) and data["ocio"]["fallback"] is False,
            "source/OCIO mismatch")
    sources = ("create_building_m2_scene.py", "building_m2_art.py", "building_clay_geometry.py",
               "create_building_clay_scene.py", "validate_building_numeric_candidate.py")
    require(data["sources"] == {source: digest(WORKFLOW / "scripts" / source) for source in sources},
            "candidate authoring source changed")
    layout = face_layout(kind)
    require(data["face_layout"] == json.loads(json.dumps(layout)), "face layout drift")
    require(data["preview"] == contract["preview"], "preview drift")
    require(data["albedo_sha256"] == digest(output / "albedo.png"), "atlas digest drift")
    # Reproduce paint bytes independently from Blender's report. No authority is
    # inferred from the fact that deterministic pixels match their authoring code.
    with tempfile.TemporaryDirectory() as directory:
        expected = Path(directory) / "albedo.png"
        paint(kind, expected, data["stage"])
        with Image.open(expected) as a, Image.open(output / "albedo.png") as b:
            require(a.size == b.size == (512, 512) and b.mode == "RGBA"
                    and a.tobytes() == b.tobytes(), "atlas pixels differ")
    states = (("Empty", "Partial", "Full") if kind == "Tank"
              else ("IdleAngleZero", "QuarterTurn", "HalfTurn", "DiagonalPose"))
    require(set(data["renders"]) == set(states), "missing state render")
    preview = contract["preview"]
    width, height = preview["canvas_px"]
    ax, ay = preview["anchor_px"]
    sx, sy = width / preview["canvas_wu"][0], height / preview["canvas_wu"][1]
    samples = ((ax, ay), (ax + 32 * sx, ay), (ax, ay - 19.2 * sy), (ax, ay + 32 * sy))
    for state in states:
        image = output / f"{state}.png"
        item = data["renders"][state]
        require(item["path"] == str(image) and item["sha256"] == digest(image), "render hash drift")
        points = item["projection"]["samples_px"]
        require(len(points) == 4 and all(len(point) == 2 for point in points)
                and all(abs(a - b) < 0.01 for point, expected in zip(points, samples, strict=True)
                        for a, b in zip(point, expected, strict=True))
                and item["projection"]["all_visible_vertices_in_canvas"] is True,
                "projection drift")
        with Image.open(image) as png:
            require(png.size == (width, height) and png.mode == "RGBA", "preview budget drift")
            bounds = png.getchannel("A").getbbox()
            require(bounds is not None and bounds[0] > 0 and bounds[1] > 0
                    and bounds[2] < width and bounds[3] < height, "empty/clipped preview")
    state_bounds = state_geometry(kind, output)
    for role, spec in contract["roles"].items():
        glb = output / f"{role}.glb"
        export = read_json(reports / f"{role}.glb.export.json")
        require(export["status"] == "exported" and export["sha256"] == digest(glb)
                and export["bytes"] == glb.stat().st_size
                and export["source_blend"] == str(blend) and export["output"] == str(glb)
                and export["collection"] == spec["collection"]
                and export["geometry_scale"] == 32 and export["materials_mode"] == "placeholder"
                and export["validation"]["summary"]["errors"] == 0
                and export["validation"]["mesh_count"] == 1, "export provenance mismatch")
        check_uv(glb, geometry[role], layout[role])
        subprocess.run([str(WORKFLOW / "bin/gltf-validate"), str(glb)], check=True, timeout=60)
    return {"schema_version": 1, "kind": kind, "stage": data["stage"],
            "evidence_kind": "candidate_technical_only", "numeric_candidate_sha256": NUMERIC_SHA256,
            "numeric_freeze": False, "art_approved": False, "runtime_published": False,
            "state_geometry": state_bounds}


def build_bundle(kind, name, stage):
    blend, output, report, reports = paths(kind, name)
    require(not any(path.exists() for path in (blend, output, report, reports)), "use fresh output name")
    candidate_geometry(kind)
    output.mkdir(parents=True)
    paint(kind, output / "albedo.png", stage)
    env = dict(os.environ, BLENDER_SAFE_NO_NETWORK="1", OCIO=str(FIXTURES / "wall-calibration-v2.ocio"))
    subprocess.run([str(WORKFLOW / "bin/blender-safe"), "--background", "--factory-startup",
                    "--python-exit-code", "2", "--python",
                    str(WORKFLOW / "scripts/create_building_m2_scene.py"), "--", kind,
                    "--name", name, "--stage", stage], check=True, timeout=180, env=env)
    for role, spec in load_pilot(kind)["roles"].items():
        subprocess.run([str(WORKFLOW / "bin/export-staging-glb"), str(blend),
                        f"building-m2/{name}/{role}.glb", str(spec["triangle_cap"]),
                        "--collection", spec["collection"], "--geometry-scale", "32",
                        "--materials-mode", "placeholder"], check=True, timeout=180, env=env)
    result = verify(kind, name)
    write_json_atomic(reports / "bundle.json", result)
    return result


def prepare(kind, name, generation, destination, codec):
    result = verify(kind, name)
    require(result["stage"] == "strokes", "runtime candidate requires the final paint stage")
    destination = pipeline.no_symlinks(destination)
    require(not destination.exists(), "use fresh candidate destination")
    blend, output, report, _ = paths(kind, name)
    contract = load_pilot(kind)
    files = {"source.blend": blend.read_bytes(), "geometry.json": NUMERIC.read_bytes(),
             "paint-report.json": report.read_bytes(), "albedo.png": (output / "albedo.png").read_bytes()}
    for role in contract["roles"]:
        files[f"{role}.glb"] = (output / f"{role}.glb").read_bytes()
    for state in read_json(report)["renders"]:
        files[f"{state}.png"] = (output / f"{state}.png").read_bytes()
    source = destination / "authoring"
    for filename, data in files.items():
        pipeline.put(pipeline.rooted(source, filename), data)
    recipe_path = destination / "recipe.json"
    pipeline.put(recipe_path, pipeline.canonical(recipe(kind, generation, source)))
    exported = pipeline.export(recipe_path, source, destination / "runtime", codec, authority="art_preview")
    result.update(identity=exported["identity"], runtime_root=str(destination / "runtime"),
                  production_candidate=False,
                  remaining=["independent numeric freeze", "all-consumer active lifecycle native",
                             "performance budget", "independent art/candidate/Help and fixed review",
                             "approved promotion/install/rollback release"])
    pipeline.put(destination / "m2-candidate-runtime.json", pipeline.canonical(result))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "verify", "prepare"))
    parser.add_argument("kind", choices=tuple(PILOTS))
    parser.add_argument("--name", required=True)
    parser.add_argument("--stage", choices=("planes", "strokes"), default="strokes")
    parser.add_argument("--generation", type=int)
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--codec", type=Path)
    args = parser.parse_args()
    if args.action == "build":
        result = build_bundle(args.kind, args.name, args.stage)
    elif args.action == "verify":
        result = verify(args.kind, args.name)
    else:
        require(all(value is not None for value in (args.generation, args.destination, args.codec)),
                "prepare requires generation, destination and already-built codec")
        result = prepare(args.kind, args.name, args.generation, args.destination, args.codec)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
