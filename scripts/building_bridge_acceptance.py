"""Freeze the independent Bridge recipe; input integrity is never acceptance.

No previous nine-kind/static/completion-seeded evidence is accepted here.
The coordinator executes ordinary UI legs through registered native launchers.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
import building_asset_pipeline as pipeline  # noqa: E402
from building_bridge_art import CONTRACT, contract, parts  # noqa: E402

LEGS = {
    "placement": "On untouched seed 20260920, use Architect Bridge and actual pointer press/release from both banks at one X; compare the same live 2x5 ordered footprint and ground center; reject occupied deck/bank, river gap and overlong span without mutations.",
    "passage": "Clay first: current-height Soul traverses bank, bridge end, center, other end, opposite bank in both lanes and directions; capture feet, carried items and bounce; reject geometry rather than change actor height or navigation.",
    "adjacent": "Place two nonoverlapping bridges via normal UI in adjacent columns; verify independent owner occupancy, continuous banks and readable edges; removing one preserves neighbor passage.",
    "construction": "Normal UI placement, real material delivery and work: Blueprint then partial construction/pulse then completed body; no fixture-created progress or completed owners; retain original cost and task behavior.",
    "instant-build": "Use ordinary placement with existing Instant Build; require completed owned Bridge, correct map layers and same-generation body; report debug mode separately from normal construction.",
    "cancel": "Cancel normal placed Blueprint before delivery and during partial construction; compare refunds/task cleanup, initial walkability, absent bridged bits, owner/root/pulse cleanup.",
    "save-load": "Save partial and completed Bridge separately; load paused then resume, confirm saved footprint/progress/materials, rebuilt shell/body generation, bridge bits and navigation; require real Save/Load success events and new world epoch.",
    "deconstruct": "Normal completed Bridge deconstruction: actual task commit, salvage Rock x3, no orphan visual, original River nonwalkability restored, dry deck stays walkable and adjacent owner retained.",
    "non-movable": "Check ordinary context/UI and existing move admission reject Bridge; no move task or mutation; do not invent synthetic Bridge movement as gameplay evidence.",
    "gallery": "Actual window near/normal/far and quality/DPI: compare body, placement ghost, Blueprint/pulse and open catalog, bright/dark regions, Wall/Door/Soul occlusion; independent clay then numeric/planes/UV/strokes/art decisions.",
    "cleanup": "Ten rounds of late generation, failed newer generation retaining active, cold missing/invalid fallback, active invalidation, new/load shells and owner cleanup; record one root/one part per owner and bounded one active/one pending Bridge pool.",
}
GATES = ("clay_passage", "numeric", "planes", "UV_strokes", "native_lifecycle", "renderer_gpu",
         "performance", "art", "Help", "fixed_review", "candidate", "release")


def frozen_subject(repo):
    if __package__:
        from . import building_art_acceptance as art
    else:
        import building_art_acceptance as art
    repo = art.native.validate_repo(str(repo))
    value = art.subject(repo)
    art.validate_subject(value, feedback=False)
    return {**value, "bridge_driver": pipeline.digest(Path(__file__).read_bytes())}


def runtime(root, codec):
    manifest, text, _ = pipeline.load_set(root, "Bridge", codec)
    _, spec = contract("Bridge")
    pipeline.require(manifest["world_preview"] == {"image_role": "world_preview", **spec["preview"]}
                     and manifest["catalog_preview"] == {"image_role": "catalog", **spec["preview"], "anchor_px": [128, 128]},
                     "Bridge preview geometry differs")
    pipeline.require(manifest["identity"]["authority"] == "art_preview"
                     and manifest["receipt"] is None and manifest["art_approval_sha256"] is None,
                     "recipe requires unapproved Bridge ArtPreview")
    pipeline.require(manifest["geometry_contract_sha256"] == pipeline.digest(CONTRACT.read_bytes())
                     and manifest["parts"] == parts("Bridge"), "Bridge geometry/parts drift")
    for path, field in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256")):
        pipeline.require(pipeline.digest(pipeline.rooted(root, path).read_bytes()) == manifest[field], "provenance drift")
    pipeline.require(manifest["export_sha256"] == pipeline.digest(pipeline.canonical(manifest["artifacts"])), "export drift")
    return {"root": str(root.absolute()), "identity": manifest["identity"], "manifest_sha256": pipeline.digest(text.encode())}


def performance(budget):
    keys = {"frame_p95_ratio_max", "frame_p99_ratio_max", "rss_delta_bytes_max", "native_bytes_delta_max"}
    pipeline.require(isinstance(budget, dict) and set(budget) == keys
                     and all(type(v) in (int, float) and math.isfinite(v) and v > 0 for v in budget.values()),
                     "coordinator must freeze explicit positive Bridge budgets before execution")
    return {"accepted": False, "budget": budget, "n": 4, "four_n": 16,
            "placement": "Discover legal crossings on unchanged generated terrain; reject insufficient capacity, never widen river or erase blockers",
            "runs": ["legacy N", "candidate N", "legacy 4N", "candidate 4N", "cumulative ten-kind"],
            "instrumentation": ["Capture", "Memory"], "repeat": 3,
            "match": ["source/binary", "live layout and logical state", "population", "camera/zoom", "activity", "GPU/backend", "warmup/measurement"],
            "resident": "control must not preload Bridge candidate dependencies; preserve other nine building generations identically",
            "measure": ["p50/p95/p99", "native allocator", "RSS", "mesh/image/material bytes", "root/part/handle counts"],
            "probe": "Disable HW_BRIDGE_ACCEPTANCE_PROBE for timing and memory runs; observer allocations are diagnostic overhead",
            "foundation": "If measuring root-child refactor overhead, separately build distinct source identities with the same supported fixture; same-binary A/B is only asset cost"}


def contents(args, subject, candidate, budget):
    return {"schema_version": 1, "scope": "bridge-coordinator-recipe-not-acceptance", "accepted": False,
            "runtime_published": False, "promotion_authority": False, "launchable": False,
            "result_contract": {"helper": "scripts/building_production_acceptance.py",
                "command": "verify-results", "scope": "bridge", "profile": "building-production-results-v1",
                "requires": "separate frozen released-identity plan, host registry admission, all 11 normal-world result legs and sequential Capture/Memory",
                "registry_registered": False, "input_integrity_is_acceptance": False},
            "repo": str(args.repo.absolute()), "subject": subject,
            "codec": str(args.codec.absolute()), "codec_sha256": pipeline.digest(args.codec.read_bytes()),
            "geometry_contract_sha256": pipeline.digest(CONTRACT.read_bytes()), "runtime": candidate,
            "legs": [{"id": key, "action": value, "accepted": False, "evidence": []} for key, value in LEGS.items()],
            "native": {"launcher": "primary dev.py validation registered native launcher only; sequential Capture/Memory, one heavy slot/job/test thread",
                       "worldgen_seed": 20260920, "fixture_seeded_completion": False,
                       "session": {"mode": "art-preview", "identity": candidate["identity"], "locator": pipeline.locator("Bridge")},
                       "observation": "HW_BUILDING_ART_SESSION additionally requires unique nonce and absolute fresh status_path; HW_BRIDGE_ACCEPTANCE_PROBE selects one leg; launch normal world, no building-art-static workload",
                       "required": ["owned actual-window captures", "actual GPU/Vulkan adapter", "source/binary/asset/codec hashes", "input and domain outcome trace", "no warnings/errors", "fresh per-leg nonce"],
                       "not_evidence": ["nine-kind M6", "M2 completed fixture", "legacy fixed-river fixture", "observer samples alone", "input-integrity verification"]},
            "performance": performance(budget), "independent_gates": {gate: None for gate in GATES}}


def plan(args):
    subject = frozen_subject(args.repo)
    pipeline.require(subject["source"] == args.subject_sha256, "source fingerprint differs")
    candidate = runtime(args.bridge_runtime, args.codec)
    value = contents(args, subject, candidate, pipeline.read(args.budget))
    pipeline.put(args.output, pipeline.canonical(value))
    verify(args.output)
    return value


def verify(path):
    value = pipeline.read(path)
    args = argparse.Namespace(repo=Path(value["repo"]), codec=Path(value["codec"]))
    expected = contents(args, frozen_subject(args.repo), runtime(Path(value["runtime"]["root"]), args.codec),
                        value["performance"]["budget"])
    pipeline.require(value == expected, "Bridge recipe drift or unsupported acceptance claim")
    pipeline.require(value["subject"] == frozen_subject(args.repo) and value == pipeline.read(path), "inputs changed")
    return {"scope": "bridge-input-integrity-only", "accepted": False, "promotion_authority": False,
            "recipe_sha256": pipeline.digest(path.read_bytes()), "subject": value["subject"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    create = sub.add_parser("plan")
    for name in ("bridge-runtime", "codec", "budget", "output"):
        create.add_argument("--" + name, type=Path, required=True)
    create.add_argument("--subject-sha256", required=True)
    create.add_argument("--repo", type=Path, default=ROOT)
    sub.add_parser("verify").add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(plan(args) if args.action == "plan" else verify(args.plan), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
