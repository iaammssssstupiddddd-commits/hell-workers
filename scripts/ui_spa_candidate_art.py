"""Adopted Spa candidate in ordinary occupied-save feedback; no new approval."""
from __future__ import annotations

import json
import math
from pathlib import Path
import shutil
import struct
import sys

from scripts import building_art_acceptance as art
from scripts import building_body_paint_observation as paint_observation

sys.path.insert(0, str(art.HELPERS))
import ui_rest_area_art as shared  # noqa: E402
import ui_spa_planning as spa  # noqa: E402
import ui_spa_occupied_save as occupied  # noqa: E402

native, pipeline = art.native, art.pipeline
CASE = "soul-spa-candidate-occupied-save"
SCOPE = "ordinary-spa-candidate-occupied-v1"
RESUMED_CASE = "soul-spa-candidate-resumed-use"
RESUMED_SCOPE = "ordinary-spa-candidate-resumed-v1"
LOADED_CASE = "soul-spa-candidate-loaded-use"
LOADED_SCOPE = "ordinary-spa-candidate-load-v1"


def case_scope(case):
    native.require(case in (CASE, RESUMED_CASE, LOADED_CASE), "Spa candidate case differs")
    return {CASE: SCOPE, RESUMED_CASE: RESUMED_SCOPE, LOADED_CASE: LOADED_SCOPE}[case]


def source_contract(repo, source_root, codec, *, case=CASE, resume_source=None):
    source_root, codec = pipeline.no_symlinks(source_root), pipeline.no_symlinks(codec)
    pipeline.verify_set(source_root, "SoulSpa", codec)
    manifest, text, receipt = pipeline.load_set(source_root, "SoulSpa", codec)
    native.require(manifest["identity"]["authority"] == "isolated_candidate" and receipt is None,
                   "Spa ordinary art requires the exact approved isolated candidate")
    geometry = json.loads(pipeline.rooted(source_root, "provenance/geometry.json").read_bytes())
    result = {"case": case, "scope": case_scope(case), "source_root": str(source_root), "codec": str(codec),
            "codec_sha256": pipeline.digest(codec.read_bytes()), "identity": manifest["identity"],
            "manifest_sha256": pipeline.digest(text.encode()), "artifacts": manifest["artifacts"],
            "parts": manifest["parts"], "relative_tiles": geometry["ordered_relative_tiles"],
            **paint_observation.fields(manifest),
            "body_paint_verifier_sha256": paint_observation.verifier_sha256(),
            "base_assets_sha256": art.asset_view_fingerprint(repo),
            "numeric_verifiers": pipeline.numeric_verifier_inventory(),
            "verifier_sha256": pipeline.digest(Path(__file__).read_bytes())}
    if case in (RESUMED_CASE, LOADED_CASE):
        result["resumed_verifier_sha256"] = pipeline.digest(Path(__file__).with_name("ui_spa_resumed_use.py").read_bytes())
    native.require((resume_source is not None) == (case == LOADED_CASE), "Spa imported save requires its explicit load case")
    if case == LOADED_CASE:
        from scripts import ui_spa_loaded_use as loaded
        result["resume_input"] = loaded.origin_contract(repo, Path(resume_source), result)
        result["loaded_verifier_sha256"] = pipeline.digest(Path(loaded.__file__).read_bytes())
    return result


contract_digest = shared.contract_digest
wait_ready = shared.wait_ready


def planned_contract(args, repo):
    source, codec = getattr(args, "soul_spa_art_source", None), getattr(args, "art_codec", None)
    native.require(args.case in (CASE, RESUMED_CASE, LOADED_CASE) and source and codec and args.smoke and args.input_backend in ("portal", "portal-eis")
                   and not args.layout_menu and not getattr(args, "rest_area_art_source", None),
                   "Spa candidate requires its explicit bounded portal source/codec case")
    contract = source_contract(repo, Path(source), Path(codec), case=args.case,
                               resume_source=getattr(args, "spa_resume_source", None))
    if getattr(args, "art_contract_sha256", None):
        native.require(contract_digest(contract) == args.art_contract_sha256, "Spa candidate contract changed")
    return contract


def session_setting(directory, nonce, contract):
    case = contract.get("case", CASE)
    native.require(contract["scope"] == case_scope(case), "Spa candidate scope differs")
    return json.dumps({"mode": {CASE: "candidate", RESUMED_CASE: "candidate-resumed", LOADED_CASE: "candidate-loaded"}[case], "identity": contract["identity"],
                       "locator": pipeline.locator("SoulSpa"), "nonce": nonce,
                       "output": str(directory / "production-cancellation.json")})


def prepare_view(repo, directory, contract):
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"]),
                                                     case=contract.get("case", CASE),
                                                     resume_source=contract.get("resume_input", {}).get("root")), contract),
                   "Spa candidate source changed")
    view = shared.checked_view(repo, directory / "asset-view", prepared=False)
    native.require(not view.exists(), "Spa candidate view must be fresh")
    shutil.copytree(repo / "assets", view / "assets")
    native.require(art.asset_view_fingerprint(view) == contract["base_assets_sha256"], "Spa base assets changed")
    source = Path(contract["source_root"])
    for entry in contract["artifacts"]:
        pipeline.put(pipeline.rooted(view / "assets", entry["path"]), pipeline.file_bytes(source, entry))
    locator = pipeline.locator("SoulSpa")
    pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
    return view


def check_view(repo, view, contract):
    view = shared.checked_view(repo, view, prepared=True)
    def inventory(root):
        result = {}
        for path in root.rglob("*"):
            native.require(not path.is_symlink(), "Spa candidate asset-view symlink")
            if path.is_file():
                result[path.relative_to(root).as_posix()] = pipeline.digest(path.read_bytes())
        return result
    expected = inventory(repo / "assets")
    expected.update({entry["path"]: entry["sha256"] for entry in contract["artifacts"]})
    expected[pipeline.locator("SoulSpa")] = contract["manifest_sha256"]
    native.require(inventory(view / "assets") == expected, "Spa candidate asset-view inventory differs")


def runtime_parts(parts):
    """Project authored vectors to the codec's Rust f32 serialization, not a tolerance."""
    projected = []
    for part in parts:
        item = dict(part)
        for key, length in (("translation_wu", 3), ("rotation_xyzw", 4), ("scale", 3)):
            values = part.get(key)
            native.require(isinstance(values, list) and len(values) == length
                           and all(type(value) in (int, float) and math.isfinite(value)
                                   for value in values), "Spa authored vector invalid")
            try:
                item[key] = [struct.unpack("<f", struct.pack("<f", value))[0] for value in values]
            except (OverflowError, struct.error) as error:
                raise native.AcceptanceError("Spa authored vector exceeds f32") from error
            native.require(all(math.isfinite(value) for value in item[key]), "Spa authored vector exceeds f32")
        projected.append(item)
    return projected


def check_consumers(raw, entries, contract, *, start=2):
    native.require(start in (1, 2), "Spa consumer checkpoint boundary differs")
    identity = contract["identity"]
    native.require(raw.get("scope") == contract.get("scope") == case_scope(contract.get("case", CASE))
                   and identity.get("kind") == "SoulSpa" and identity.get("authority") == "isolated_candidate",
                   "Spa candidate scope/authority differs")
    declared = {part["name"]: part for part in contract["parts"]}
    native.require(set(declared) == {"body", "slot0", "slot1", "slot2", "slot3"}
                   and contract["relative_tiles"] == [[0, 0], [1, 0], [0, -1], [1, -1]],
                   "Spa candidate part/tile contract differs")
    for entry in entries[start:]:
        sample = spa.raw_checkpoint(raw, entry["value"])
        state = sample["spa_context"]
        native.require(len(state["sites"]) == 1 and len(state["tiles"]) == 4, "Spa art site/tile inventory differs")
        owner = state["sites"][0]["entity"]
        feedback = sample.get("art_feedback", {})
        native.require(pipeline.exact_json(feedback.get("requested"), identity)
                       and pipeline.exact_json(feedback.get("active"), identity) and feedback.get("pending") is None,
                       "Spa candidate is not the active generation")
        descriptor = feedback.get("descriptor", {})
        native.require(pipeline.exact_json(descriptor.get("identity"), identity)
                       and pipeline.exact_json(descriptor.get("parts"), runtime_parts(contract["parts"]))
                       and all(isinstance(descriptor.get(key), str) and descriptor[key]
                               for key in ("body_mesh", "slot_mesh", "albedo", "slot_emissive", "world_preview", "catalog")),
                       "Spa candidate descriptor incomplete")
        roots = [root for root in feedback.get("roots", []) if root.get("owner") == owner]
        native.require(len(roots) == 1 and pipeline.exact_json(roots[0].get("identity"), identity)
                       and roots[0].get("owner_pose_matches") is True, "Spa root generation/pose differs")
        parts = roots[0].get("parts", [])
        native.require(len(parts) == 5 and {part.get("name") for part in parts} == set(declared)
                       and len({part.get("entity") for part in parts}) == 5, "Spa consumer parts differ")
        anchor = entry["value"]["spa_planning"]["anchor"]
        mask = 0
        for index, offset in enumerate(contract["relative_tiles"]):
            grid = [a + b for a, b in zip(anchor, offset, strict=True)]
            tiles = [tile for tile in state["tiles"] if tile.get("parent_site") == owner and tile.get("grid") == grid]
            native.require(len(tiles) == 1 and "workers" in tiles[0]
                           and (tiles[0]["workers"] is None or isinstance(tiles[0]["workers"], list)),
                           "Spa art tile ownership/workers differ")
            if tiles[0]["workers"]:
                mask |= 1 << index
        for part in parts:
            spec = declared[part["name"]]
            active = part["name"] != "body" and bool(mask & (1 << int(part["name"][-1])))
            native.require(part.get("parent") == roots[0].get("entity") and part.get("visible") is True
                           and part.get("mesh") == descriptor["body_mesh" if part["name"] == "body" else "slot_mesh"]
                           and part.get("albedo") == paint_observation.expected_texture(
                               contract, part.get("name"), descriptor, part.get("body_paint"))
                           and isinstance(part.get("material"), str) and part["material"], "Spa part mesh/material/parent differs")
            for field, source in (("translation", "translation_wu"), ("rotation", "rotation_xyzw"), ("scale", "scale")):
                pose = part.get("pose", {}).get(field)
                native.require(isinstance(pose, list) and len(pose) == len(spec[source])
                               and all(type(a) in (int, float) and math.isfinite(a)
                                       and math.isclose(a, b, rel_tol=1e-6, abs_tol=1e-6)
                                       for a, b in zip(pose, spec[source], strict=True)), "Spa part pose differs")
            values = part.get("material_values", {})
            native.require(all(values.get(key) is True for key in ("white", "opaque", "lit"))
                           and type(values.get("roughness")) in (int, float) and values["roughness"] == 1
                           and type(values.get("reflectance")) in (int, float) and values["reflectance"] == 0
                           and values.get("emissive_white") is active and values.get("emissive_black") is (not active)
                           and values.get("emissive_texture") == (descriptor["slot_emissive"] if active else None)
                           and values.get("emissive_texture_absent") is (not active), "Spa actual worker/emission differs")


def verify(raw, entries, client, save_path, inputs, contract):
    native.require(contract.get("case", CASE) == CASE and contract.get("scope") == SCOPE,
                   "Occupied Spa candidate verifier cannot admit a resumed case")
    occupied.verify(raw, entries, client, save_path, inputs, raw_scope=SCOPE)
    check_consumers(raw, entries, contract)


exercise = occupied.exercise
