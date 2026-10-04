"""Build or independently revalidate a staging-only clay bundle through existing gates."""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import subprocess

from PIL import Image

from building_clay_geometry import FIXTURES, PILOTS, load_pilot, read_json, require
from validate_building_clay_glb import validate
from workflow_common import asset_root, require_within, write_json_atomic

WORKFLOW = FIXTURES.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(kind, name):
    require(kind in PILOTS and re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None,
            "invalid clay kind/name")
    root = asset_root() / "staging"
    values = (root / "blend" / f"{name}.blend", root / "exports" / "building-clay" / name,
              root / "reports" / f"{name}.json", root / "reports" / "building-clay" / name)
    # Reject symlink escapes before invoking wrappers or reading any artifact.
    return tuple(require_within(value, root / area, "clay bundle") for value, area in
                 zip(values, ("blend", "exports", "reports", "reports"), strict=True))


def verify(kind, name):
    blend, output, report, reports = paths(kind, name)
    contract = load_pilot(kind)
    payload = read_json(report)
    require(payload["kind"] == kind and payload["schema_version"] == 1, "clay report identity differs")
    require(payload["evidence_kind"] == "blender_clay_only" and payload["runtime_authority"] is None
            and payload["art_approval"] is None, "clay bundle cannot carry runtime/art authority")
    require(payload["blend"] == str(blend) and payload["blend_sha256"] == digest(blend), "blend identity differs")
    for key, source in (("generator_sha256", WORKFLOW / "scripts/create_building_clay_scene.py"),
                        ("geometry_generator_sha256", WORKFLOW / "scripts/building_clay_geometry.py"),
                        ("geometry_contract_sha256", FIXTURES / f"building-{PILOTS[kind]}-v1.geometry.json")):
        require(payload[key] == digest(source), f"stale source: {key}")
    require(payload["ocio"]["fallback"] is False, "OCIO fallback is not evidence")
    albedo = output / "albedo.png"
    require(payload["albedo"] == {"path": str(albedo), "sha256": digest(albedo)}, "albedo differs")
    with Image.open(albedo) as image:
        require(image.size == (16, 16), "neutral clay albedo size differs")
        require(image.convert("RGBA").getchannel("A").getextrema() == (255, 255), "albedo not opaque")
    states = ("Empty", "Partial", "Full") if kind == "Tank" else ("IdleAngleZero", "DiagonalPose")
    require(set(payload["renders"]) == set(states), "clay state inventory differs")
    require(payload["preview"] == contract["preview"], "preview contract differs")
    require(payload["catalog"] == payload["renders"][states[0]], "catalog must alias representative square image")
    width, height = contract["preview"]["canvas_px"]
    ax, ay = contract["preview"]["anchor_px"]
    sx, sy = width / contract["preview"]["canvas_wu"][0], height / contract["preview"]["canvas_wu"][1]
    expected_points = ((ax, ay), (ax + 32 * sx, ay), (ax, ay - 19.2 * sy), (ax, ay + 32 * sy))
    for state, evidence in payload["renders"].items():
        path = output / f"{state}.png"
        require(evidence["path"] == str(path) and evidence["sha256"] == digest(path), "state image differs")
        actual = evidence["projection"]["samples_px"]
        require(len(actual) == 4 and all(len(point) == 2 for point in actual), "projection samples missing")
        require(all(abs(a - e) < 0.01 for pair, expected in zip(actual, expected_points, strict=True)
                    for a, e in zip(pair, expected, strict=True)), "projection sample drift")
        require(evidence["projection"]["all_visible_vertices_in_canvas"] is True, "clipped vertex report")
        with Image.open(path) as image:
            require(image.size == (width, height) and image.mode == "RGBA", "preview image format differs")
            bbox = image.getchannel("A").getbbox()
            require(bbox is not None and bbox[0] > 0 and bbox[1] > 0 and bbox[2] < width and bbox[3] < height,
                    "empty or clipped preview")
    require(set(payload["roles"]) == set(contract["roles"]), "role inventory differs")
    results = {}
    for role, spec in contract["roles"].items():
        glb = output / f"{role}.glb"
        result = validate(glb, kind, role)
        export = read_json(reports / f"{role}.glb.export.json")
        require(export["status"] == "exported" and export["sha256"] == digest(glb)
                and export["bytes"] == glb.stat().st_size, "export bytes differ")
        require(export["source_blend"] == str(blend) and export["output"] == str(glb)
                and export["collection"] == spec["collection"] and export["geometry_scale"] == 32
                and export["materials_mode"] == "placeholder", "export route differs")
        require(export["validation"]["summary"]["errors"] == 0 and export["validation"]["mesh_count"] == 1,
                "scene gate failed")
        # Re-run Khronos on current bytes; never trust an unbound historical JSON alone.
        validator = WORKFLOW / "bin/gltf-validate"
        subprocess.run([str(validator), str(glb)], check=True, timeout=60)
        results[role] = result
    return {"status": "pass", "evidence_kind": "technical_clay_only", "kind": kind,
            "name": name, "art_approved": False, "runtime_published": False, "roles": results}


def build_bundle(kind, name):
    blend, output, report, reports = paths(kind, name)
    require(not any(path.exists() for path in (blend, output, report, reports)), "output name already used")
    env = dict(os.environ, BLENDER_SAFE_NO_NETWORK="1", OCIO=str(FIXTURES / "wall-calibration-v2.ocio"))
    subprocess.run([str(WORKFLOW / "bin/create-building-clay-scene"), kind, "--name", name],
                   check=True, timeout=180, env=env)
    for role, spec in load_pilot(kind)["roles"].items():
        subprocess.run([str(WORKFLOW / "bin/export-staging-glb"), str(blend),
                        f"building-clay/{name}/{role}.glb", str(spec["triangle_cap"]),
                        "--collection", spec["collection"], "--geometry-scale", "32",
                        "--materials-mode", "placeholder"], check=True, timeout=180, env=env)
    result = verify(kind, name)
    write_json_atomic(reports / "bundle.json", result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "verify"))
    parser.add_argument("kind", choices=tuple(PILOTS))
    parser.add_argument("--name", required=True)
    args = parser.parse_args()
    print((build_bundle if args.action == "build" else verify)(args.kind, args.name))


if __name__ == "__main__":
    main()
