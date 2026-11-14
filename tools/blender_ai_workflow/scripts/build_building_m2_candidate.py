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
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image

from building_infernal_style import STYLE, ILLUSTRATED_STYLE

import building_asset_pipeline as pipeline
from building_clay_geometry import FIXTURES, PILOTS, load_pilot, read_json, require
from building_m2_art import candidate_geometry, face_layout, numeric_contract, paint
from prepare_building_clay_runtime import recipe, state_geometry
from validate_building_m2_indexed_faces import check_indexed_faces as check_uv
from workflow_common import asset_root, require_within, write_json_atomic

WORKFLOW = FIXTURES.parent


def illustrated_environment():
    from build_tank_illustration_candidate import blender_environment
    environment = blender_environment(Path('/usr/bin/flatpak'))
    node = shutil.which('node')
    require(node is not None, 'installed node executable required for the fixed glTF validator')
    environment['PATH'] = '/usr/bin' + os.pathsep + str(Path(node).resolve().parent) + os.pathsep + os.defpath
    environment.pop('NODE_OPTIONS', None)
    environment.pop('NODE_PATH', None)
    environment.update(OCIO=str(FIXTURES / 'wall-calibration-v2.ocio'),
                       GLTF_VALIDATOR_ROOT=str(Path.home() / 'tools/gltf-validator'))
    return environment


def illustrated_toolchain():
    from build_tank_illustration_candidate import blender_chain, SAFE_BLENDER
    environment = illustrated_environment()
    validator = Path(environment['GLTF_VALIDATOR_ROOT'])
    files = [*sorted((WORKFLOW / 'scripts').glob('*.py')),
             *sorted(path for path in FIXTURES.rglob('*') if path.is_file()),
             *(WORKFLOW / 'bin' / name for name in ('blender-safe', 'export-staging-glb', 'gltf-validate')),
             Path(shutil.which('node', path=environment['PATH'])).resolve(),
             validator / 'validate.mjs', validator / 'package-lock.json',
             *sorted(path for path in (validator / 'node_modules/gltf-validator').rglob('*') if path.is_file())]
    return {'blender': blender_chain(SAFE_BLENDER, Path('/usr/bin/flatpak')),
            'files': {str(pipeline.no_symlinks(path)): digest(path) for path in files}}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def illustrated_outputs(blend, output, report, reports):
    files = [blend, report, *sorted(path for root in (output, reports) for path in root.rglob('*') if path.is_file())]
    return {str(pipeline.no_symlinks(path)): digest(path) for path in files}


def paths(kind, name):
    require(kind in PILOTS and re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None,
            "invalid candidate kind/name")
    root = asset_root() / "staging"
    return tuple(require_within(root / area / suffix, root / area, "M2 candidate")
                 for area, suffix in (("blend", f"{name}.blend"),
                                      ("exports", f"building-m2/{name}"),
                                      ("reports", f"{name}.json"),
                                      ("reports", f"building-m2/{name}")))


def verify(kind, name, revision="v1", style=STYLE):
    require(style in (STYLE, ILLUSTRATED_STYLE) and (style != ILLUSTRATED_STYLE
            or kind == "MudMixer" and revision == "v3"), "unknown M2 style/kind/revision")
    blend, output, report, reports = paths(kind, name)
    geometry = candidate_geometry(kind, revision)
    contract = load_pilot(kind, revision)
    numeric, numeric_sha256 = numeric_contract(kind, revision)
    data = read_json(report)
    frozen_outputs = illustrated_outputs(blend, output, report, reports) if style == ILLUSTRATED_STYLE else None
    if style == ILLUSTRATED_STYLE:
        require(pipeline.exact_json(data.get('source_toolchain'), illustrated_toolchain()),
                'illustrated source toolchain changed')
    require(data["schema_version"] == 1 and data["kind"] == kind
            and data["evidence_kind"] == "unapproved_m2_paint_candidate"
            and data.get("revision", "v1") == revision
            and data.get("art_style") == style
            and data["numeric_candidate_sha256"] == numeric_sha256,
            "candidate report identity mismatch")
    require(all(data[key] is False for key in ("numeric_freeze", "art_approved", "runtime_published")),
            "candidate report cannot grant approval")
    require(data["blend_sha256"] == digest(blend) and data["ocio"]["fallback"] is False,
            "source/OCIO mismatch")
    sources = ("create_building_m2_scene.py", "building_m2_art.py", "building_clay_geometry.py", "building_infernal_style.py",
               "create_building_clay_scene.py", "validate_building_numeric_candidate.py",
               "validate_building_m2_indexed_faces.py")
    if style == ILLUSTRATED_STYLE:
        sources += ("building_mixer_uv_ink.py",)
    require(data["sources"] == {source: digest(WORKFLOW / "scripts" / source) for source in sources},
            "candidate authoring source changed")
    layout = face_layout(kind, revision)
    require(data["face_layout"] == json.loads(json.dumps(layout)), "face layout drift")
    require(data["preview"] == contract["preview"], "preview drift")
    require(data["albedo_sha256"] == digest(output / "albedo.png"), "atlas digest drift")
    # Reproduce paint bytes independently from Blender's report. No authority is
    # inferred from the fact that deterministic pixels match their authoring code.
    with tempfile.TemporaryDirectory() as directory:
        expected = Path(directory) / "albedo.png"
        paint(kind, expected, data["stage"], revision, style=style)
        with Image.open(expected) as a, Image.open(output / "albedo.png") as b:
            require(a.size == b.size == (512, 512) and b.mode == "RGBA"
                    and a.tobytes() == b.tobytes(), "atlas pixels differ")
    states = (("Empty", "Partial", "Full") if kind == "Tank"
              else ("IdleAngleZero", "QuarterTurn", "HalfTurn", "DiagonalPose"))
    if style == ILLUSTRATED_STYLE:
        states += ("ThreeQuarterTurn",)
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
    if style == ILLUSTRATED_STYLE:
        require(len({data["renders"][state]["sha256"] for state in
                ("IdleAngleZero", "QuarterTurn", "HalfTurn", "ThreeQuarterTurn")}) == 4,
                "object-attached wear mark is not visible across four rotor poses")
    state_bounds = state_geometry(kind, output, revision)
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
        subprocess.run([str(WORKFLOW / "bin/gltf-validate"), str(glb)], check=True, timeout=60,
                       **({'env': illustrated_environment()} if style == ILLUSTRATED_STYLE else {}))
    if style == ILLUSTRATED_STYLE:
        require(pipeline.exact_json(data['source_toolchain'], illustrated_toolchain())
                and read_json(report) == data
                and illustrated_outputs(blend, output, report, reports) == frozen_outputs,
                'illustrated source changed during verification')
    return {"schema_version": 1, "kind": kind, "stage": data["stage"],
            "evidence_kind": "candidate_technical_only", "numeric_candidate_sha256": numeric_sha256,
            "revision": revision, "numeric_source": str(numeric),
            "numeric_freeze": False, "art_approved": False, "runtime_published": False,
            "state_geometry": state_bounds,
            **({'source_inventory': frozen_outputs, 'source_toolchain': data['source_toolchain']}
               if style == ILLUSTRATED_STYLE else {})}


def build_bundle(kind, name, stage, revision="v1", style=STYLE):
    require(style in (STYLE, ILLUSTRATED_STYLE) and (style != ILLUSTRATED_STYLE
            or kind == "MudMixer" and revision == "v3"), "unknown M2 style/kind/revision")
    blend, output, report, reports = paths(kind, name)
    require(not any(path.exists() for path in (blend, output, report, reports)), "use fresh output name")
    candidate_geometry(kind, revision)
    frozen = illustrated_toolchain() if style == ILLUSTRATED_STYLE else None
    output.mkdir(parents=True)
    paint(kind, output / "albedo.png", stage, revision, style=style)
    env = illustrated_environment() if style == ILLUSTRATED_STYLE else dict(os.environ, BLENDER_SAFE_NO_NETWORK="1", OCIO=str(FIXTURES / "wall-calibration-v2.ocio"))
    subprocess.run([str(WORKFLOW / "bin/blender-safe"), "--background", "--factory-startup",
                    "--python-exit-code", "2", "--python",
                    str(WORKFLOW / "scripts/create_building_m2_scene.py"), "--", kind,
                    "--name", name, "--stage", stage, "--revision", revision, "--style", style],
                   check=True, timeout=180, env=env)
    for role, spec in load_pilot(kind, revision)["roles"].items():
        subprocess.run([str(WORKFLOW / "bin/export-staging-glb"), str(blend),
                        f"building-m2/{name}/{role}.glb", str(spec["triangle_cap"]),
                        "--collection", spec["collection"], "--geometry-scale", "32",
                        "--materials-mode", "placeholder"], check=True, timeout=180, env=env)
    if frozen is not None:
        require(pipeline.exact_json(frozen, illustrated_toolchain()), 'illustrated toolchain changed during build')
        data = read_json(report)
        data['source_toolchain'] = frozen
        write_json_atomic(report, data)
    result = verify(kind, name, revision, style)
    write_json_atomic(reports / "bundle.json", result)
    return result


def prepare(kind, name, generation, destination, codec, revision="v1", style=STYLE):
    result = verify(kind, name, revision, style)
    require(result["stage"] == "strokes", "runtime candidate requires the final paint stage")
    destination = pipeline.no_symlinks(destination)
    require(not destination.exists(), "use fresh candidate destination")
    blend, output, report, reports = paths(kind, name)
    contract = load_pilot(kind, revision)
    numeric, _ = numeric_contract(kind, revision)
    def unchanged():
        require(illustrated_outputs(blend, output, report, reports) == result['source_inventory']
                and pipeline.exact_json(illustrated_toolchain(), result['source_toolchain']),
                'verified illustrated inputs changed during preparation')
    if style == ILLUSTRATED_STYLE:
        unchanged()
    origins = {"source.blend": blend, "geometry.json": numeric,
               "paint-report.json": report, "albedo.png": output / "albedo.png"}
    for role in contract["roles"]:
        origins[f"{role}.glb"] = output / f"{role}.glb"
    for state in read_json(report)["renders"]:
        origins[f"{state}.png"] = output / f"{state}.png"
    files = {name: path.read_bytes() for name, path in origins.items()}
    if style == ILLUSTRATED_STYLE:
        require(all(pipeline.digest(payload) == (result['numeric_candidate_sha256'] if name == 'geometry.json'
                    else result['source_inventory'][str(origins[name])]) for name, payload in files.items()),
                'copied illustrated source was not verified')
        unchanged()
    source = destination / "authoring"
    for filename, data in files.items():
        pipeline.put(pipeline.rooted(source, filename), data)
    recipe_path = destination / "recipe.json"
    value = recipe(kind, generation, source, revision)
    if style == ILLUSTRATED_STYLE:
        value["surface_paint"] = {"method": "uv_ink_v1"}
    pipeline.put(recipe_path, pipeline.canonical(value))
    exported = pipeline.export(recipe_path, source, destination / "runtime", codec, authority="art_preview")
    if style == ILLUSTRATED_STYLE:
        pipeline.verify_set(destination / 'runtime', kind, codec)
        unchanged()
        require({path.name: pipeline.digest(path.read_bytes()) for path in source.iterdir()}
                == {name: pipeline.digest(data) for name, data in files.items()},
                'prepared source copy changed around export')
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
    parser.add_argument("--revision", choices=("v1", "v2", "v3"), default="v1")
    parser.add_argument("--style", choices=(STYLE, ILLUSTRATED_STYLE), default=STYLE)
    parser.add_argument("--generation", type=int)
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--codec", type=Path)
    args = parser.parse_args()
    require(args.revision == "v1" or args.kind == "MudMixer",
            "v2 is only an unapproved MudMixer proposal")
    if args.action == "build":
        result = build_bundle(args.kind, args.name, args.stage, args.revision, args.style)
    elif args.action == "verify":
        result = verify(args.kind, args.name, args.revision, args.style)
    else:
        require(all(value is not None for value in (args.generation, args.destination, args.codec)),
                "prepare requires generation, destination and already-built codec")
        result = prepare(args.kind, args.name, args.generation, args.destination, args.codec,
                         args.revision, args.style)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
