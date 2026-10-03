"""Freeze M5 candidate inputs and native recipes; never claim acceptance.

Coordinator owns validation registration, sequential Capture/Memory execution,
independent verification, art decisions and the existing release transactions.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
import building_asset_pipeline as pipeline  # noqa: E402
import build_building_m5_candidate as candidate  # noqa: E402

COMMON = {
    "gallery": "Capture near, normal and maximum zoom-out beside approved M2/M3, Wall, Door and Soul, on bright/dark terrain and at N/4N density. Foreground2d remains foreground, not depth-tested structural art. Art decision is independent of technical verification.",
    "consumers": "Open catalog and placement before exact identity arrives. Capture completed world, normal ghost, Blueprint, late-created pulse and still-open catalog across generation A, failed B retaining A, successful C and explicit invalidation fallback. World paths use identical original, canvas and anchor; catalog alone uses contain. M5 has no player move operation: synthetic move coverage must not be called gameplay acceptance.",
    "placement-lifecycle": "Use real input for valid/invalid placement and cancellation, construction completion and deconstruction. Compare occupancy, traversability, costs and source entities to legacy. Repeat removal and world rebuild ten times; no stale owner sprites or retired asset handles. Pool stays one active and one pending per kind.",
    "save-load": "Save populated world, load paused, inspect rebuilt shells using the existing live pool; resume without duplicate presentation owners or previous-world references. Repeat valid/failed generation arrival while paused.",
}
SPECIFIC = {
    "WheelbarrowParking": {"vehicle": "Capture empty parking, actual separate parked wheelbarrow, departure, return and loaded wheelbarrow; image contains no vehicle. Compare ownership/capacity and vehicle/item image hashes to baseline."},
    "SandPile": {"source": "Gather real sand and transport it: infinite source and item icon stay unchanged; compare item image hash and visible carried/stored items to baseline."},
    "BonePile": {"source": "Gather real bones and transport them: infinite source and bone item icon stay unchanged; distinguish the mound from OutdoorLamp at maximum zoom-out."},
    "OutdoorLamp": {"power": "Use real allocation, supply loss, policy and producer removal. Capture PoweredVisualState and off/on sprite in the same frame; white RGB avoids old gray tint. Save lit lamp, paused load is off, resume real recalculation relights only with supply. Compare logical emitter position/radius/effect, walkability and allocation to baseline; no new light or save field."},
}
GATES = ("focused_tests", "change_aware_CI", "Help", "native_gallery", "native_lifecycle",
         "performance", "art", "candidate", "fixed_review", "promotion", "install", "rollback", "release", "storage")


def legs(kind):
    candidate.contract(kind)
    return [{"id": name, "action": action, "accepted": False, "evidence": []}
            for name, action in {**COMMON, **SPECIFIC[kind]}.items()]


def protected(asset_root):
    value = json.loads(candidate.CONTRACT.read_bytes())
    return [pipeline.record(relative, pipeline.rooted(asset_root, relative).read_bytes())
            for relative in value["protected_images"]]


def subject(repo):
    if __package__:
        from . import building_art_acceptance as art
    else:
        import building_art_acceptance as art
    repo = art.native.validate_repo(str(repo))
    value = art.subject(repo)
    art.validate_subject(value, feedback=False)
    return {**value, "m5_driver": pipeline.digest(Path(__file__).read_bytes()),
            "m5_candidate_helper": pipeline.digest(Path(candidate.__file__).read_bytes())}


def plan(args):
    frozen = subject(args.repo)
    pipeline.require(frozen["source"] == args.subject_sha256, "source fingerprint differs")
    baseline = pipeline.read(candidate.staged(args.protected_baseline))
    pipeline.require(baseline["scope"] == "m5-protected-image-inventory"
                     and baseline["images"] == protected(args.asset_root), "protected images differ from baseline")
    runtimes = dict(zip(candidate.KINDS, (args.parking_runtime, args.sand_runtime,
                                        args.bone_runtime, args.lamp_runtime), strict=True))
    value = {"schema_version": 1, "scope": "m5-recipe-not-acceptance", "accepted": False,
             "runtime_published": False, "repo": str(args.repo.absolute()), "subject": frozen,
             "codec": str(args.codec.absolute()), "codec_sha256": pipeline.digest(args.codec.read_bytes()),
             "contract_sha256": pipeline.digest(candidate.CONTRACT.read_bytes()),
             "asset_root": str(args.asset_root.absolute()), "protected_images": protected(args.asset_root),
             "protected_baseline": str(args.protected_baseline.absolute()),
             "protected_baseline_sha256": pipeline.digest(args.protected_baseline.read_bytes()),
             "kinds": {kind: {**candidate.verify(kind, root, args.codec), "legs": legs(kind)}
                       for kind, root in runtimes.items()},
             "native": {
                 "launcher": "Primary dev.py validation coordinator and hell-workers-run-native-acceptance skill; guarded no-prompt launcher, one heavy slot, Capture then Memory, one Cargo job/test thread",
                 "static": "building_art_acceptance.py plan/run/verify for each exact kind identity; small building-art-static fixture proves only paused presentation, never power/lifecycle or art acceptance",
                 "lifecycle": "Run each listed gameplay leg via coordinator-owned actual-window recipe; record logical state and owned-window images at the same subject",
                 "failure": "Missing evidence, wrong window owner, GPU/backend drift, source/binary/asset mutation, warning/error or incomplete leg cannot pass",
             },
             "performance": {"accepted": False, "budget": None,
                             "runs": ["same-binary legacy-control N", "candidate N", "legacy-control 4N", "candidate 4N", "cumulative nine kinds"],
                             "fixed": ["binary/source", "GPU/backend", "camera/zoom", "population", "activity", "warmup/measurement"],
                             "measure": ["frame p50/p95/p99", "GPU when available", "RSS/native allocator", "image bytes", "pool strong handles", "owner sprite count"],
                             "limits": "Coordinator freezes numeric budget before run; no budget or measurements means no acceptance"},
             "independent_gates": {gate: None for gate in GATES}}
    pipeline.put(candidate.staged(args.output), pipeline.canonical(value))
    verify(args.output)
    return value


def verify(path):
    value = pipeline.read(candidate.staged(path))
    pipeline.require(value["scope"] == "m5-recipe-not-acceptance" and value["accepted"] is False
                     and value["runtime_published"] is False and value["performance"]["accepted"] is False,
                     "recipe cannot claim acceptance")
    pipeline.require(value["independent_gates"] == {gate: None for gate in GATES}, "recipe cannot contain approvals")
    repo, codec = Path(value["repo"]), Path(value["codec"])
    pipeline.require(subject(repo) == value["subject"]
                     and pipeline.digest(codec.read_bytes()) == value["codec_sha256"]
                     and pipeline.digest(candidate.CONTRACT.read_bytes()) == value["contract_sha256"], "frozen inputs drift")
    pipeline.require(protected(Path(value["asset_root"])) == value["protected_images"], "protected resource/vehicle images changed")
    baseline = candidate.staged(Path(value["protected_baseline"]))
    pipeline.require(pipeline.digest(baseline.read_bytes()) == value["protected_baseline_sha256"]
                     and pipeline.read(baseline)["images"] == value["protected_images"], "baseline inventory drift")
    pipeline.require(set(value["kinds"]) == set(candidate.KINDS), "all four M5 kinds required")
    for kind, record in value["kinds"].items():
        pipeline.require(record == {**candidate.verify(kind, Path(record["root"]), codec), "legs": legs(kind)},
                         "candidate identity or native recipe drift")
    pipeline.require(subject(repo) == value["subject"] and pipeline.read(path) == value
                     and protected(Path(value["asset_root"])) == value["protected_images"]
                     and pipeline.digest(codec.read_bytes()) == value["codec_sha256"], "inputs changed during verification")
    return {"scope": "m5-input-integrity-only", "accepted": False,
            "recipe_sha256": pipeline.digest(path.read_bytes()), "subject": value["subject"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    create = sub.add_parser("plan")
    for name in ("parking-runtime", "sand-runtime", "bone-runtime", "lamp-runtime", "codec", "asset-root", "protected-baseline", "output"):
        create.add_argument("--" + name, type=Path, required=True)
    create.add_argument("--subject-sha256", required=True)
    create.add_argument("--repo", type=Path, default=ROOT)
    check = sub.add_parser("verify")
    check.add_argument("--plan", type=Path, required=True)
    inventory = sub.add_parser("inventory")
    inventory.add_argument("--asset-root", type=Path, required=True)
    inventory.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "inventory":
        result = {"schema_version": 1, "scope": "m5-protected-image-inventory",
                  "asset_root": str(args.asset_root.absolute()), "images": protected(args.asset_root)}
        pipeline.put(candidate.staged(args.output), pipeline.canonical(result))
    else:
        result = plan(args) if args.action == "plan" else verify(args.plan)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
