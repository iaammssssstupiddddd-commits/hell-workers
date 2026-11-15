"""Prepare an unapproved Tank world-preview projection source from preserved g3.

This never writes canonical assets or invents art/numeric/release decisions.
Runtime export remains a separate building_asset_pipeline transaction.
"""

from __future__ import annotations
import argparse
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

from PIL import Image
import building_asset_pipeline as pipeline
from prepare_building_clay_runtime import recipe
from workflow_common import asset_root, require_within

BASE_SOURCE = "ee1a97a55950d480eb2613337a25aeef0c946706e9b6ff34c15dc2f5bf4914bc"
BASE_FILES = {
    "source.blend": BASE_SOURCE,
    "body.glb": "d14c32ac93d23db22d3158b0b02b202377d4e48c9326e26b18d8c6a001f2cf28",
    "water.glb": "06b8da02d155277b5bf9dc0388e6531d91ec734dcdc89f65f5526455fe7cbe85",
    "albedo.png": "c72f85396f2d511a82b66936269d512cefbf3bc221127cc4b6d8bb59ba2338a8",
    "Empty.png": "acff8e3e9db0effc658999dd23bef36808f75f11a6ae9a47f21ececa4b04694e",
    "geometry.json": "1b913ea0784bf6815a6c46ed0396dafc6ebcdcfc107370bc569aa875775d949d",
}
PROJECTION = {
    "method": "world_preview_v1",
    "matte_linear_rgb": [0.125, 0.09375, 0.078125],
}
PACKER = Path(__file__).with_name("pack_tank_illustration_source.py")
SAFE_BLENDER = PACKER.resolve().parents[1] / "bin/blender-safe"


def blender_environment(flatpak):
    environment = dict(os.environ)
    for key in list(environment):
        if key in ("BASH_ENV", "ENV", "SHELLOPTS", "BASHOPTS") or key.startswith(
            "BASH_FUNC_"
        ):
            environment.pop(key, None)
    environment.update(
        PATH=str(flatpak.parent) + os.pathsep + os.defpath,
        BLENDER_SAFE_NO_NETWORK="1",
        HELL_WORKERS_ASSET_ROOT=str(asset_root()),
    )
    return environment


def blender_chain(blender, flatpak):
    """Pin the real sandbox launcher/deployments, never call a report command."""
    pipeline.require(
        blender == SAFE_BLENDER
        and blender.is_file()
        and flatpak.is_absolute()
        and flatpak.resolve() == flatpak
        and flatpak.is_file(),
        "known safe Blender launcher and explicit Flatpak executable required",
    )
    environment = blender_environment(flatpak)
    selected = shutil.which("flatpak", path=environment["PATH"])
    pipeline.require(
        selected is not None and Path(selected).resolve() == flatpak,
        "safe launcher must resolve the declared Flatpak executable",
    )

    def info(reference, option):
        result = subprocess.run(
            [str(flatpak), "info", option, reference],
            env=environment,
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
        pipeline.require(result and "\n" not in result, "ambiguous Flatpak identity")
        return result

    def deployment(reference):
        ref = info(reference, "--show-ref")
        commit = info(reference, "--show-commit")
        location = Path(info(reference, "--show-location"))
        pipeline.require(
            ref == reference
            and re.fullmatch(r"[0-9a-f]{64}", commit) is not None
            and location.is_absolute()
            and location.name == commit,
            "Flatpak deployment identity differs",
        )
        return {
            "ref": ref,
            "commit": commit,
            "location": str(location),
            "metadata_sha256": pipeline.digest((location / "metadata").read_bytes()),
        }

    app_ref = info("org.blender.Blender", "--show-ref")
    pipeline.require(
        app_ref.startswith("app/org.blender.Blender/"), "unexpected Blender deployment"
    )
    runtime_ref = "runtime/" + info(app_ref, "--show-runtime")
    files = (
        blender,
        flatpak,
        Path(shutil.which("bash", path=environment["PATH"])).resolve(),
    )
    return {
        "files": {str(path): pipeline.digest(path.read_bytes()) for path in files},
        "application": deployment(app_ref),
        "runtime": deployment(runtime_ref),
        "asset_root": str(asset_root()),
        "network": False,
    }


def frozen_inputs(base, source, extra, blender, flatpak):
    paths = [
        *(base / name for name in BASE_FILES),
        *source.iterdir(),
        Path(__file__),
        PACKER,
        *extra,
    ]
    for path in paths:
        pipeline.no_symlinks(path)
    return {
        "files": {str(path): pipeline.digest(path.read_bytes()) for path in paths},
        "blender_chain": json.loads(
            pipeline.canonical(blender_chain(blender, flatpak))
        ),
    }


def preview(paint: bytes, mask: bytes) -> bytes:
    """Reuse the immutable geometry alpha, not its old paint or lighting RGB."""
    with Image.open(io.BytesIO(paint)) as art, Image.open(io.BytesIO(mask)) as guide:
        pipeline.require(
            art.format == guide.format == "PNG"
            and art.mode == guide.mode == "RGBA"
            and 0 < min(art.size) <= max(art.size) <= 2048
            and guide.size == (256, 256),
            "bounded RGBA paint and exact canonical mask required",
        )
        composite = Image.alpha_composite(
            Image.new("RGBA", art.size, (99, 86, 79, 255)), art
        )
        result = composite.resize(guide.size, Image.Resampling.LANCZOS)
        result.putalpha(guide.getchannel("A"))
        output = io.BytesIO()
        result.save(output, format="PNG", compress_level=9)
        return output.getvalue()


def base_inputs(base):
    pipeline.no_symlinks(base)
    pipeline.require(base.is_dir(), "preserved g3 authoring bundle missing")
    for name, sha in BASE_FILES.items():
        path = base / name
        pipeline.require(
            path.is_file() and pipeline.digest(path.read_bytes()) == sha,
            "immutable base authoring file differs: " + name,
        )
    pipeline.require(
        (base / "geometry.json").is_file(), "base geometry contract missing"
    )


def packer(blender, flatpak, action, base, source, config):
    command = [
        str(blender),
        "--background",
        "--factory-startup",
        "--disable-autoexec",
        "--python",
        str(PACKER.resolve()),
        "--",
        action,
        "--base",
        str(base / "source.blend"),
        "--source",
        str(source),
        "--config",
        str(config),
    ]
    result = subprocess.run(
        command,
        env=blender_environment(flatpak),
        capture_output=True,
        text=True,
        timeout=120,
        check=True,
    )
    prefix = "HW_TANK_SOURCE_AUDIT_JSON:"
    rows = [
        line[len(prefix) :]
        for line in result.stdout.splitlines()
        if line.startswith(prefix)
    ]
    pipeline.require(len(rows) == 1, "missing independent Blender source audit")
    return json.loads(rows[0])


def verify(root, blender, flatpak, paint_sha256):
    pipeline.no_symlinks(root)
    pipeline.require(
        blender.is_absolute() and blender.resolve() == blender and blender.is_file(),
        "explicit trusted Blender executable required",
    )
    report = pipeline.read(root / "illustration-source.json")
    pipeline.require(
        set(report)
        == {
            "schema_version",
            "scope",
            "generation",
            "base_bundle",
            "blender",
            "blender_chain",
            "source_audit",
            "producer_sha256",
            "packer_sha256",
            "files",
            "art_approved",
            "numeric_freeze",
            "runtime_published",
        }
        and type(report["schema_version"]) is int
        and report["schema_version"] == 1
        and type(report["generation"]) is int
        and report["generation"] > 3
        and report["scope"] == "unapproved-tank-world-preview-source"
        and all(
            report[k] is False
            for k in ("art_approved", "numeric_freeze", "runtime_published")
        ),
        "source cannot grant approval",
    )
    # Report data never selects an executable. The registered verify argv owns it.
    pipeline.require(
        report["blender"] == str(blender)
        and pipeline.exact_json(
            report["blender_chain"], blender_chain(blender, flatpak)
        ),
        "registered Blender identity changed",
    )
    base = Path(report["base_bundle"])
    base_inputs(base)
    pipeline.require(
        report["producer_sha256"] == pipeline.digest(Path(__file__).read_bytes())
        and report["packer_sha256"] == pipeline.digest(PACKER.read_bytes()),
        "source producer changed",
    )
    source = root / "authoring"
    pipeline.require(
        set(report["files"])
        == {
            "source.blend",
            "body.glb",
            "water.glb",
            "albedo.png",
            "geometry.json",
            "paint-original.png",
            "geometry-mask.png",
            "Empty.png",
            "projection.json",
        }
        and report["files"]["paint-original.png"] == paint_sha256,
        "declared source/paint inventory differs",
    )
    for name, sha in report["files"].items():
        pipeline.require(
            pipeline.digest(pipeline.rooted(source, name).read_bytes()) == sha,
            "prepared source file changed: " + name,
        )
    pipeline.require(
        set(p.name for p in source.iterdir()) == set(report["files"]),
        "source file inventory differs",
    )
    pipeline.require(
        (source / "Empty.png").read_bytes()
        == preview(
            (source / "paint-original.png").read_bytes(),
            (base / "Empty.png").read_bytes(),
        ),
        "illustrated preview pixels differ",
    )
    value = recipe("Tank", report["generation"], source)
    value["body_paint_projection"] = PROJECTION
    pipeline.require(
        pipeline.read(root / "recipe.json") == value, "illustration recipe differs"
    )
    config = pipeline.read(source / "projection.json")
    expected = {
        "schema_version": 1,
        "base_source_sha256": BASE_SOURCE,
        "body_paint_projection": PROJECTION,
        "packed_images": {
            name: pipeline.record(path, (source / path).read_bytes())
            for name, path in (
                ("HW_Tank_Illustration_Original", "paint-original.png"),
                ("HW_Tank_Illustration_Mask", "geometry-mask.png"),
                ("HW_Tank_Illustration_Preview", "Empty.png"),
            )
        },
    }
    pipeline.require(pipeline.exact_json(config, expected), "packed projection differs")
    for name in ("body.glb", "water.glb", "albedo.png", "geometry.json"):
        pipeline.require(
            (source / name).read_bytes() == (base / name).read_bytes(),
            "base geometry/atlas changed",
        )
    pipeline.require(
        (source / "geometry-mask.png").read_bytes()
        == (base / "Empty.png").read_bytes(),
        "base mask changed",
    )
    extra = [root / "recipe.json", root / "illustration-source.json"]
    frozen = frozen_inputs(base, source, extra, blender, flatpak)
    audit = packer(
        blender,
        flatpak,
        "audit",
        base,
        source / "source.blend",
        source / "projection.json",
    )
    pipeline.require(
        audit == report["source_audit"], "source geometry/packed-image audit differs"
    )
    pipeline.require(
        pipeline.exact_json(
            frozen_inputs(base, source, extra, blender, flatpak), frozen
        ),
        "source or Blender inputs changed during audit",
    )
    return {
        "status": "verified-unapproved-source",
        "generation": report["generation"],
        "art_approved": False,
        "numeric_freeze": False,
        "runtime_published": False,
    }


def prepare(args):
    root = require_within(
        args.output, asset_root() / "staging/exports", "illustration output"
    )
    pipeline.require(
        not root.exists() and type(args.generation) is int and args.generation > 3,
        "fresh post-g3 source generation required",
    )
    base = args.base_bundle.resolve()
    base_inputs(base)
    paint = pipeline.no_symlinks(args.paint.resolve())
    pipeline.require(
        paint.is_file() and paint.stat().st_size <= 32 * 1024 * 1024,
        "bounded paint input required",
    )
    rendered = preview(paint.read_bytes(), (base / "Empty.png").read_bytes())
    source = root / "authoring"
    source.mkdir(parents=True)
    for name in ("body.glb", "water.glb", "albedo.png", "geometry.json"):
        shutil.copyfile(base / name, source / name)
    pipeline.put(source / "paint-original.png", paint.read_bytes())
    pipeline.put(source / "geometry-mask.png", (base / "Empty.png").read_bytes())
    pipeline.put(source / "Empty.png", rendered)
    config = {
        "schema_version": 1,
        "base_source_sha256": BASE_SOURCE,
        "body_paint_projection": PROJECTION,
        "packed_images": {
            name: pipeline.record(path, (source / path).read_bytes())
            for name, path in (
                ("HW_Tank_Illustration_Original", "paint-original.png"),
                ("HW_Tank_Illustration_Mask", "geometry-mask.png"),
                ("HW_Tank_Illustration_Preview", "Empty.png"),
            )
        },
    }
    pipeline.put(source / "projection.json", pipeline.canonical(config))
    blender = args.blender.resolve()
    flatpak = args.flatpak.resolve()
    frozen = frozen_inputs(base, source, [], blender, flatpak)
    audit = packer(
        blender,
        flatpak,
        "pack",
        base,
        source / "source.blend",
        source / "projection.json",
    )
    # The pack operation may create only the new blend; all its inputs stay fixed.
    after = frozen_inputs(base, source, [], blender, flatpak)
    created = after["files"].pop(str(source / "source.blend"), None)
    pipeline.require(
        created is not None and pipeline.exact_json(after, frozen),
        "source inputs changed during packing",
    )
    value = recipe("Tank", args.generation, source)
    value["body_paint_projection"] = PROJECTION
    pipeline.put(root / "recipe.json", pipeline.canonical(value))
    report = {
        "schema_version": 1,
        "scope": "unapproved-tank-world-preview-source",
        "generation": args.generation,
        "base_bundle": str(base),
        "blender": str(blender),
        "blender_chain": frozen["blender_chain"],
        "source_audit": audit,
        "producer_sha256": pipeline.digest(Path(__file__).read_bytes()),
        "packer_sha256": pipeline.digest(PACKER.read_bytes()),
        "files": {
            p.name: pipeline.digest(p.read_bytes()) for p in sorted(source.iterdir())
        },
        "art_approved": False,
        "numeric_freeze": False,
        "runtime_published": False,
    }
    pipeline.put(root / "illustration-source.json", pipeline.canonical(report))
    return verify(root, blender, flatpak, pipeline.digest(paint.read_bytes()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    p = commands.add_parser("prepare")
    p.add_argument("--base-bundle", type=Path, required=True)
    p.add_argument("--paint", type=Path, required=True)
    p.add_argument("--blender", type=Path, required=True)
    p.add_argument("--flatpak", type=Path, required=True)
    p.add_argument("--generation", type=int, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = commands.add_parser("verify")
    p.add_argument("--root", type=Path, required=True)
    p.add_argument("--blender", type=Path, required=True)
    p.add_argument("--flatpak", type=Path, required=True)
    p.add_argument("--paint-sha256", required=True)
    args = parser.parse_args()
    print(
        json.dumps(
            prepare(args)
            if args.action == "prepare"
            else verify(args.root, args.blender, args.flatpak, args.paint_sha256),
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
