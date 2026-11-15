"""Prepare dedicated M5 raster originals through the existing building codec.

Coordinator-only execution. Artist supplies world PNGs under staging; this tool
derives only the catalog image, never crops/rescales a world role and never
approves art, promotes, installs or writes product assets. Outputs stay under
the workflow's staging/exports/building-m5 directory.
"""
from __future__ import annotations

import argparse
import base64
import io
import json
import re
from pathlib import Path

import building_asset_pipeline as pipeline
from workflow_common import asset_root

CONTRACT = Path(__file__).resolve().parents[1] / "fixtures/building-m5-v2.contract.json"
KINDS = ("WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp")
ISOLATED_JOBS = Path(__file__).resolve().parents[3] / "target/native-acceptance"


def contract(kind):
    pipeline.require(kind in KINDS, "unsupported M5 kind")
    value = json.loads(CONTRACT.read_bytes())
    spec = value["kinds"][kind]
    if "anchor_px" in spec:
        value = {**value, "anchor_px": spec["anchor_px"]}
    inherited = pipeline.KINDS[kind]
    pipeline.require(inherited["presentation"] == value["presentation"] == "Foreground2d"
                     and inherited["mesh_roles"] == []
                     and inherited["image_roles"] == spec["world_roles"] + ["catalog"]
                     and inherited["representative_state"] == spec["representative_state"],
                     "M5 differs from shared role contract")
    return value, spec


def staged(path):
    path = pipeline.no_symlinks(path)
    root = pipeline.no_symlinks(asset_root() / "staging")
    pipeline.require(path.is_relative_to(root) and path != root, "M5 inputs/outputs must be in staging")
    return path


def destination(name):
    pipeline.require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None, "invalid candidate name")
    return staged(asset_root() / "staging/exports/building-m5" / name)


def decode(payload, size):
    from PIL import Image
    with Image.open(io.BytesIO(payload)) as source:
        pipeline.require(source.format == "PNG" and source.mode == "RGBA"
                         and source.size == tuple(size) and not getattr(source, "is_animated", False),
                         "world must be a single-frame RGBA PNG on the fixed canvas")
        image = source.copy()
    bounds = image.getchannel("A").getbbox()
    pipeline.require(bounds is not None and bounds[0] > 0 and bounds[1] > 0
                     and bounds[2] < image.width and bounds[3] < image.height, "empty/clipped world image")
    pipeline.require(not any(r == 255 and g == 0 and b == 255 and a > 0
                             for r, g, b, a in image.getdata()), "visible magenta remains")
    return image


def catalog(image, margin):
    from PIL import Image, ImageOps
    crop = image.crop(image.getchannel("A").getbbox())
    content = ImageOps.contain(crop, (image.width - 2 * margin, image.height - 2 * margin),
                              method=Image.Resampling.LANCZOS)
    result = Image.new("RGBA", image.size)
    result.alpha_composite(content, ((image.width-content.width)//2, (image.height-content.height)//2))
    output = io.BytesIO()
    result.save(output, format="PNG")
    return output.getvalue()


def originals(kind, payloads):
    value, spec = contract(kind)
    pipeline.require(set(payloads) == set(spec["world_roles"]), "world roles differ")
    decoded = {role: decode(payload, value["canvas_px"]) for role, payload in payloads.items()}
    if kind == "OutdoorLamp":
        off, on = decoded["world_off"], decoded["world_on"]
        pipeline.require(off.getchannel("A").tobytes() == on.getchannel("A").tobytes(),
                         "Lamp off/on silhouette differs")
        pipeline.require(any(a > 0 and left != right
                             for left, right, a in zip(off.getdata(), on.getdata(),
                                                       off.getchannel("A").getdata(), strict=True)),
                         "Lamp off/on have no visible state difference")
    return catalog(decoded[spec["world_roles"][0]], value["catalog_margin_px"])


def previews(kind):
    value, spec = contract(kind)
    common = {"canvas_px": value["canvas_px"], "canvas_wu": spec["canvas_wu"],
              "anchor_px": value["anchor_px"], "representative_state": spec["representative_state"]}
    return ({"image_role": spec["world_roles"][0], **common}, {"image_role": "catalog", **common, "anchor_px": [x/2 for x in value["canvas_px"]]})


def recipe(kind, generation, source):
    _, spec = contract(kind)
    pipeline.identity(kind, generation, "art_preview")
    def record(name):
        return pipeline.record(name, pipeline.rooted(source, name).read_bytes())
    world, card = previews(kind)
    return {"schema_version": 1, "kind": kind, "generation": generation,
            "source": record("source.json"), "geometry_contract": record("geometry.json"),
            "artifacts": [{"role": "image:" + role, **record(role + ".png")}
                          for role in spec["world_roles"] + ["catalog"]],
            "parts": [], "world_preview": world, "catalog_preview": card}


def prepare(kind, source, name, generation, codec, isolated_destination=None):
    source = staged(source)
    staged_output = destination(name)  # Always validate the candidate name.
    output = staged_output if isolated_destination is None else pipeline.no_symlinks(isolated_destination)
    pipeline.require(not output.exists() and not source.is_relative_to(output)
                     and not output.is_relative_to(source), "use a fresh separate output")
    # Explicit isolated output is for a coordinator-registered candidate job,
    # never an installed product inventory. Preserve the staging-only default.
    product = Path(__file__).resolve().parents[3] / "assets"
    pipeline.require(not output.is_relative_to(product), "cannot write product assets")
    if isolated_destination is not None:
        pipeline.require(output.parent == pipeline.no_symlinks(ISOLATED_JOBS)
                         and re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", output.name) is not None,
                         "isolated output requires a primary native-acceptance job root")
    _, spec = contract(kind)
    payloads = {role: pipeline.rooted(source, role + ".png").read_bytes() for role in spec["world_roles"]}
    card = originals(kind, payloads)
    # Embed exact originals in the source identity, not just external filenames.
    original = {"schema_version": 1, "kind": kind,
                "images_base64": {role: base64.b64encode(data).decode("ascii") for role, data in payloads.items()}}
    authoring = output / "authoring"
    files = {"source.json": pipeline.canonical(original), "geometry.json": CONTRACT.read_bytes(), "catalog.png": card}
    files.update({role + ".png": data for role, data in payloads.items()})
    for filename, data in files.items():
        pipeline.put(pipeline.rooted(authoring, filename), data)
    path = output / "recipe.json"
    pipeline.put(path, pipeline.canonical(recipe(kind, generation, authoring)))
    exported = pipeline.export(path, authoring, output / "runtime", codec, authority="art_preview")
    result = {"schema_version": 1, "scope": "m5-unapproved-raster-candidate",
              "identity": exported["identity"], "runtime_root": str(output / "runtime"),
              "art_approved": False, "runtime_published": False}
    pipeline.put(output / "m5-candidate.json", pipeline.canonical(result))
    verify(kind, output / "runtime", codec, isolated=isolated_destination is not None)
    return result


def verify(kind, root, codec, *, isolated=False):
    root = pipeline.no_symlinks(root) if isolated else staged(root)
    if isolated:
        pipeline.require(root.name == "runtime" and root.parent.parent == pipeline.no_symlinks(ISOLATED_JOBS),
                         "isolated verify requires a primary native-acceptance runtime")
    manifest, text, _ = pipeline.load_set(root, kind, codec)
    pipeline.require(manifest["identity"]["authority"] == "art_preview"
                     and manifest["receipt"] is None and manifest["art_approval_sha256"] is None,
                     "requires unapproved ArtPreview")
    pipeline.require(manifest["geometry_contract_sha256"] == pipeline.digest(CONTRACT.read_bytes())
                     and pipeline.rooted(root, "provenance/geometry.json").read_bytes() == CONTRACT.read_bytes(),
                     "M5 contract drift")
    world, card = previews(kind)
    pipeline.require(manifest["parts"] == [] and manifest["world_preview"] == world
                     and manifest["catalog_preview"] == card, "world/catalog geometry differs")
    original_bytes = pipeline.rooted(root, "provenance/source").read_bytes()
    pipeline.require(pipeline.digest(original_bytes) == manifest["source_sha256"], "source identity drift")
    original = json.loads(original_bytes)
    pipeline.require(original["schema_version"] == 1 and original["kind"] == kind
                     and pipeline.canonical(original) == original_bytes, "source kind/encoding drift")
    payloads = {role: base64.b64decode(data, validate=True) for role, data in original["images_base64"].items()}
    expected = {**payloads, "catalog": originals(kind, payloads)}
    pipeline.require([entry["role"] for entry in manifest["artifacts"]]
                     == ["image:" + role for role in contract(kind)[1]["world_roles"] + ["catalog"]],
                     "artifact role order differs")
    for entry in manifest["artifacts"]:
        pipeline.require(pipeline.file_bytes(root, entry) == expected[entry["role"].removeprefix("image:")],
                         "world original or derived contain catalog differs")
    pipeline.require(manifest["export_sha256"] == pipeline.digest(pipeline.canonical(manifest["artifacts"])),
                     "export inventory differs")
    return {"root": str(root), "identity": manifest["identity"],
            "manifest_sha256": pipeline.digest(text.encode()), "accepted": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "verify"))
    parser.add_argument("kind", choices=KINDS)
    parser.add_argument("--codec", type=Path, required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--name")
    parser.add_argument("--generation", type=int)
    parser.add_argument("--runtime", type=Path)
    parser.add_argument("--destination", type=Path,
                        help="fresh isolated candidate job output, never product assets")
    parser.add_argument("--isolated", action="store_true", help="read-only verification of an isolated job")
    args = parser.parse_args()
    if args.action == "prepare":
        pipeline.require(all(value is not None for value in (args.source, args.name, args.generation)),
                         "prepare needs source, name and generation")
        pipeline.require(not args.isolated, "prepare does not accept --isolated")
        result = prepare(args.kind, args.source, args.name, args.generation, args.codec, args.destination)
    else:
        pipeline.require(args.runtime is not None, "verify needs runtime")
        pipeline.require(args.destination is None, "verify does not accept --destination")
        result = verify(args.kind, args.runtime, args.codec, isolated=args.isolated)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
