"""Exact ArtPreview ordinary lifecycle feedback, never an adoption decision."""
from __future__ import annotations

import json
import math
from pathlib import Path
import shutil
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from scripts import building_art_acceptance as art
import ui_rest_area_usage as usage

native, pipeline = art.native, art.pipeline
SCOPE = "ordinary-rest-art-feedback-v1"


def source_contract(repo, source_root, codec):
    source_root, codec = pipeline.no_symlinks(source_root), pipeline.no_symlinks(codec)
    pipeline.verify_set(source_root, "RestArea", codec)
    manifest, text, receipt = pipeline.load_set(source_root, "RestArea", codec)
    native.require(manifest["identity"]["authority"] == "art_preview" and receipt is None,
                   "ordinary art feedback requires an unapproved RestArea ArtPreview")
    return {"scope": SCOPE, "source_root": str(source_root), "codec": str(codec),
            "codec_sha256": pipeline.digest(codec.read_bytes()), "identity": manifest["identity"],
            "manifest_sha256": pipeline.digest(text.encode()), "artifacts": manifest["artifacts"],
            "parts": manifest["parts"],
            "source_sha256": manifest["source_sha256"],
            "geometry_sha256": manifest["geometry_contract_sha256"],
            "base_assets_sha256": art.asset_view_fingerprint(repo)}


def contract_digest(contract):
    return pipeline.digest(pipeline.canonical(contract))


def checked_view(repo, view, *, prepared):
    repo, view = pipeline.no_symlinks(repo), pipeline.no_symlinks(view)
    base = pipeline.no_symlinks(repo / "assets")
    native.require(repo.is_dir() and base.is_dir() and repo.resolve() == repo,
                   "ordinary art base root is not a canonical directory")
    native.require(view.name == "asset-view" and view.parent.name == "viewport-0"
                   and view.parent.parent.parent == repo / "target/native-acceptance"
                   and view.parent.is_dir() and view.resolve() == view,
                   "ordinary art view is not a canonical bounded job directory")
    if prepared:
        assets = pipeline.no_symlinks(view / "assets")
        native.require(view.is_dir() and assets.is_dir(), "ordinary art view directories missing")
    return view


def prepare_view(repo, directory, contract):
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"])), contract),
                   "ordinary art source changed")
    view = checked_view(repo, directory / "asset-view", prepared=False)
    native.require(not view.exists(), "ordinary art view must be fresh")
    shutil.copytree(repo / "assets", view / "assets")
    native.require(art.asset_view_fingerprint(view) == contract["base_assets_sha256"], "base assets changed")
    source = Path(contract["source_root"])
    for entry in contract["artifacts"]:
        pipeline.put(pipeline.rooted(view / "assets", entry["path"]), pipeline.file_bytes(source, entry))
    locator = pipeline.locator("RestArea")
    pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
    return view


def check_view(repo, view, contract):
    """Require the exact base resource inventory plus the one declared overlay."""
    view = checked_view(repo, view, prepared=True)
    expected = {}
    for path in sorted((repo / "assets").rglob("*")):
        native.require(not path.is_symlink(), "base asset symlink")
        if path.is_file():
            expected[path.relative_to(repo / "assets").as_posix()] = pipeline.digest(path.read_bytes())
    expected.update({entry["path"]: entry["sha256"] for entry in contract["artifacts"]})
    expected[pipeline.locator("RestArea")] = contract["manifest_sha256"]
    actual = {}
    for path in sorted((view / "assets").rglob("*")):
        native.require(not path.is_symlink(), "ordinary art asset-view symlink")
        if path.is_file():
            actual[path.relative_to(view / "assets").as_posix()] = pipeline.digest(path.read_bytes())
    native.require(actual == expected, "ordinary art asset view inventory/bytes changed")


def planned_contract(args, repo):
    source, codec = getattr(args, "rest_area_art_source", None), getattr(args, "art_codec", None)
    native.require(bool(source) == bool(codec), "ordinary art needs both source and codec")
    if not source:
        native.require(not getattr(args, "art_contract_sha256", None), "unexpected art contract digest")
        return None
    native.require(args.case == "rest-area-lifecycle" and args.smoke and args.input_backend == "portal"
                   and not args.layout_menu, "ordinary art requires the bounded portal lifecycle recipe")
    contract = source_contract(repo, Path(source), Path(codec))
    if getattr(args, "art_contract_sha256", None):
        native.require(contract_digest(contract) == args.art_contract_sha256, "ordinary art plan changed")
    return contract


def wait_ready(driver, contract):
    def ready(_):
        path = driver.root / "production-cancellation.json"
        if not path.exists():
            return False
        raw = native.read_json(path)
        native.require(raw.get("scope") == SCOPE and raw.get("failure") is None
                       and raw.get("nonce") == driver.last["nonce"] and raw.get("process_id") == driver.last["pid"],
                       "ordinary art raw session failed/changed")
        sample = raw.get("samples", [])[-1:] or [{}]
        feedback = sample[0].get("art_feedback", {})
        return pipeline.exact_json(feedback.get("active"), contract["identity"]) and feedback.get("descriptor") is not None
    driver.wait(ready, after=driver.last["frame"], timeout=30)


def session_setting(directory, nonce, contract):
    return json.dumps({"mode": "feedback", "identity": contract["identity"],
                       "locator": pipeline.locator("RestArea"), "nonce": nonce,
                       "output": str(directory / "production-cancellation.json")})


def check_consumers(raw, entries, contract):
    native.require(raw.get("scope") == SCOPE, "ordinary art feedback scope differs")
    identity = contract["identity"]
    native.require(identity.get("kind") == "RestArea" and identity.get("authority") == "art_preview"
                   and type(identity.get("generation")) is int and identity["generation"] > 0,
                   "ordinary art identity authority differs")
    for entry in entries[2:]:
        value = entry["value"]
        sample = usage.raw_context_checkpoint(raw["samples"], value)
        # raw_context_checkpoint returns the containing sample; load must bind
        # the new world owner, never a still-referenced pre-load root.
        feedback = sample.get("art_feedback")
        native.require(isinstance(feedback, dict)
                       and pipeline.exact_json(feedback.get("requested"), identity)
                       and pipeline.exact_json(feedback.get("active"), identity)
                       and feedback.get("pending") is None, "ordinary art generation is not ready")
        descriptor = feedback.get("descriptor")
        native.require(isinstance(descriptor, dict) and pipeline.exact_json(descriptor.get("identity"), identity)
                       and pipeline.exact_json(descriptor.get("parts"), contract["parts"])
                       and all(isinstance(descriptor.get(key), str) and descriptor[key]
                               for key in ("body_mesh", "albedo", "world_preview", "catalog")),
                       "ordinary art descriptor incomplete")
        bits = value["rest_area_planning"]["buildings"][0]
        owners = [owner for owner in sample["owners"] if owner.get("entity_bits") == bits
                  and owner.get("kind") == "RestArea" and owner.get("provisional") is False]
        native.require(len(owners) == 1, "ordinary art completed owner differs")
        roots = [root for root in feedback.get("roots", []) if root.get("owner") == owners[0]["entity"]]
        native.require(len(roots) == 1 and pipeline.exact_json(roots[0].get("identity"), identity)
                       and roots[0].get("owner_pose_matches") is True,
                       "ordinary art root uses fallback or wrong generation")
        parts = roots[0].get("parts", [])
        native.require(len(parts) == len(descriptor.get("part_names", [])) == 1
                       and [part.get("name") for part in parts] == descriptor["part_names"]
                       and all(part.get("mesh") == descriptor["body_mesh"]
                               and part.get("albedo") == descriptor["albedo"]
                               and isinstance(part.get("material"), str) and part["material"]
                               and part.get("visible") is True for part in parts),
                       "ordinary art part mesh/material/texture differs or is hidden")
        for part, declared in zip(parts, contract["parts"], strict=True):
            native.require(part.get("parent") == roots[0].get("entity"), "ordinary art part parent differs")
            for field, declared_field in (("translation", "translation_wu"), ("rotation", "rotation_xyzw"), ("scale", "scale")):
                pose = part.get("pose", {}).get(field)
                native.require(isinstance(pose, list) and len(pose) == len(declared[declared_field])
                               and all(type(a) in (int, float) and math.isfinite(a)
                                       and math.isclose(a, b, rel_tol=1e-6, abs_tol=1e-6)
                                       for a, b in zip(pose, declared[declared_field], strict=True)),
                               "ordinary art part transform differs")
            values = part.get("material_values", {})
            native.require(all(values.get(key) is True for key in ("white", "opaque", "lit", "emissive_black", "emissive_texture_absent"))
                           and type(values.get("roughness")) in (int, float) and values["roughness"] == 1
                           and type(values.get("reflectance")) in (int, float) and values["reflectance"] == 0,
                           "ordinary art material values differ")


def check_lifecycle(raw, entries, client, save_path, contract):
    # Scope is selected by this explicit validator, never copied from raw input.
    usage.check_lifecycle(raw, entries, client, save_path, raw_scope=SCOPE)
    check_consumers(raw, entries, contract)
