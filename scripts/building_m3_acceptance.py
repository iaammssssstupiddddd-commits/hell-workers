"""Freeze M3 coordinator acceptance recipes, never execute or approve native work.

Each prepared runtime is ArtPreview; independent candidate and release gates
remain separate. Verification proves input integrity only, not a lifecycle pass.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
import building_asset_pipeline as pipeline  # noqa: E402
from building_m3_art import CONTRACT, KINDS, contract, parts  # noqa: E402

COMMON = {
    "gallery": "Capture clay, planes and strokes at near/normal/max zoom-out beside M2, Wall, Door and Soul; compare bright/dark areas and occlusion. Inspect projected feet against world preview and centered catalog.",
    "late-consumers": "Open catalog and placement before load; then admit exact identity. Capture world, ghost, catalog, construction and any existing Blueprint/pulse against the same generation. Do not invent Spa Blueprint or movement gameplay.",
    "failure-generation": "Run pool tests for missing/corrupt/late/wrong identity and typed dependencies. Native capture valid A, failed B retaining A, successful newer C, invalidation fallback and late new owner; do not change runtime authority.",
    "cleanup": "Repeat state changes, owner removal and world reload 10 times. Record roots/parts, logical children, strong handles and finite shared mesh/image/material counts after stabilization.",
    "placement": "Exercise actual valid/invalid input and cancellation; require unchanged footprint, resources and ground anchor on rejection. Recheck Wall/Door and M2 placement beside M3.",
}
REST = {
    "occupancy-dream": "Use real rest assignment: empty, reservation-only, occupied, last occupant exit. Record RestAreaOccupants independently of reservations and preserve Dream origin, cooldown and lingering particles; body material stays fixed. Pause/resume and compare old path timing, not an invented instant-off rule.",
    "construction": "Build RestArea via existing Blueprint and material delivery, capture pulse/completion and preserved costs. Deconstruct through existing workflow, confirm particles/owner/root cleanup.",
    "save-load": "Save occupied RestArea, paused load, confirm durable RestingIn rebuilds occupants with the existing Dream cooldown/lifetime contract; no duplicate shells or carried entity references.",
}
SPA = {
    "all-masks": "Exercise all 16 coordinate masks with reverse tile creation order and shuffled Entity/Children order. Only nonempty TaskWorkers for durable parent_site light their ordered tile. Empty workers, reservation-only and unrelated site workers stay dark. Record 5 fixed parts; representative actual-window captures mask0, mask1 and mask15.",
    "construction-cancel": "Use SoulSpa-specific placement. Observe Constructing from first frame, actual bone delivery/progress, cancel before and after partial delivery. All slots stay off regardless of active_slots/workers. Verify material refund/task cancellation and no orphan root/parts using existing gameplay expectations.",
    "operational-energy-walk": "Finish actual construction; walk across all four tiles, assign 0/1/4 workers, vary active_slots independently, compare established generation output/distribution to nearby consumers. Emissive adds no logical light or power. Deconstruct Operational separately from Constructing cancel.",
    "save-load": "Save active Spa, load paused: phase persists, runtime workers do not, mask0 and four slot leaves. Resume actual reassignment and require coordinate-specific relighting, identical footprint/energy contracts and zero old-world visual references.",
}
GATES = ("numeric", "clay", "planes", "UV_strokes", "native_gallery", "native_lifecycle", "performance",
         "Help", "fixed_review", "art", "candidate", "promotion", "install", "rollback", "formal_release")


def frozen_subject(repo):
    if __package__:
        from . import building_art_acceptance as art
    else:
        import building_art_acceptance as art
    repo = art.native.validate_repo(str(repo))
    subject = art.subject(repo)
    art.validate_subject(subject, feedback=False)
    return {**subject, "m3_driver": pipeline.digest(Path(__file__).read_bytes())}


def runtime(kind, root, codec):
    contract(kind)
    manifest, text, _ = pipeline.load_set(root, kind, codec)
    pipeline.require(manifest["identity"]["authority"] == "art_preview"
                     and manifest["receipt"] is None and manifest["art_approval_sha256"] is None,
                     "recipe requires unapproved ArtPreview identity")
    pipeline.require(manifest["geometry_contract_sha256"] == pipeline.digest(CONTRACT.read_bytes())
                     and manifest["parts"] == parts(kind), "numeric fixture/part transforms differ")
    for relative, field in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256")):
        pipeline.require(pipeline.digest(pipeline.rooted(root, relative).read_bytes()) == manifest[field], "provenance drift")
    pipeline.require(manifest["export_sha256"] == pipeline.digest(pipeline.canonical(manifest["artifacts"])), "export inventory drift")
    return {"root": str(root.absolute()), "identity": manifest["identity"], "manifest_sha256": pipeline.digest(text.encode())}


def legs(kind):
    return [{"id": name, "action": action, "accepted": False, "evidence": []}
            for name, action in {**COMMON, **(REST if kind == "RestArea" else SPA)}.items()]


def plan(args):
    subject = frozen_subject(args.repo)
    pipeline.require(subject["source"] == args.subject_sha256, "source fingerprint differs")
    kinds = {kind: {**runtime(kind, root, args.codec), "legs": legs(kind)}
             for kind, root in (("RestArea", args.rest_runtime), ("SoulSpa", args.spa_runtime))}
    result = {
        "schema_version": 1, "scope": "m3-coordinator-recipe-not-acceptance", "accepted": False,
        "runtime_published": False, "repo": str(args.repo.absolute()), "subject": subject,
        "codec": str(args.codec.absolute()), "codec_sha256": pipeline.digest(args.codec.read_bytes()),
        "geometry_contract_sha256": pipeline.digest(CONTRACT.read_bytes()), "kinds": kinds,
        "native": {
            "launcher": "hell-workers-run-native-acceptance skill: guarded no-prompt launcher, sequential Capture/Memory",
            "registration": "primary dev.py validation coordinator; read validation-storage-workflow.md, register batch/hold, wrap plans, use returned direct kitty command, seal/finalize/storage check",
            "resource_limits": "one host heavy slot, one Cargo job, one Rust test thread; preserve review-active workspace and cache",
            "gallery": "building_art_acceptance.py plan/run/verify per exact kind identity; small static fixture is presentation evidence only",
            "lifecycle": "coordinator-owned actual gameplay through native launcher; each leg needs before/after logical observations and owned-window captures",
            "freeze": ["base/head/source", "binary", "harness", "codec", "asset view and exact identities", "GPU/backend/adapter", "nonce"],
            "fail_closed": ["missing artifacts", "window ownership mismatch", "backend drift", "warning/error", "subject mutation", "incomplete legs"],
            "all_masks_are_not": "proof of gameplay assignment, power, save/load or construction; synthetic and real gameplay evidence must be distinguished",
        },
        "performance": {
            "accepted": False, "budget": None,
            "runs": ["baseline N", "candidate N", "baseline 4N", "candidate 4N", "cumulative M2+M3"],
            "match": ["same source/binary", "renderer/GPU/backend", "population", "camera/zoom", "simulation activity", "warmup", "measurement"],
            "measure": ["frame p50/p95/p99", "GPU where available", "RSS/native allocator", "mesh/image/material bytes", "roots/parts/pool handles"],
            "counts": {"RestArea_parts_per_owner": 1, "SoulSpa_parts_per_owner": 5, "SoulSpa_shared_mesh_roles": 2},
            "repeat": "10 state/lifecycle/generation rounds; masks never allocate new parts/materials; retired generation/owner references converge",
            "decision": "coordinator fixes numeric limits before execution; absent limits/results cannot pass",
        },
        "independent_gates": {gate: None for gate in GATES},
    }
    pipeline.put(args.output, pipeline.canonical(result))
    verify(args.output)
    return result


def verify(path):
    value = pipeline.read(path)
    pipeline.require(value["schema_version"] == 1 and value["scope"] == "m3-coordinator-recipe-not-acceptance"
                     and value["accepted"] is False and value["runtime_published"] is False
                     and value["performance"]["accepted"] is False, "recipe cannot claim acceptance")
    pipeline.require(value["independent_gates"] == {gate: None for gate in GATES}, "independent approvals belong outside this recipe")
    repo, codec = Path(value["repo"]), pipeline.no_symlinks(Path(value["codec"]))
    pipeline.require(value["subject"] == frozen_subject(repo)
                     and value["codec_sha256"] == pipeline.digest(codec.read_bytes())
                     and value["geometry_contract_sha256"] == pipeline.digest(CONTRACT.read_bytes()), "frozen subject drift")
    pipeline.require(set(value["kinds"]) == set(KINDS), "both M3 kinds required")
    for kind, spec in value["kinds"].items():
        expected = {**runtime(kind, Path(spec["root"]), codec), "legs": legs(kind)}
        pipeline.require(spec == expected, "recipe/identity drift or missing leg")
    pipeline.require(value["subject"] == frozen_subject(repo) and value == pipeline.read(path)
                     and value["codec_sha256"] == pipeline.digest(codec.read_bytes()), "inputs changed during verification")
    return {"scope": "m3-input-integrity-only", "accepted": False, "runtime_published": False,
            "recipe_sha256": pipeline.digest(path.read_bytes()), "subject": value["subject"]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    create = sub.add_parser("plan")
    create.add_argument("--rest-runtime", type=Path, required=True)
    create.add_argument("--spa-runtime", type=Path, required=True)
    create.add_argument("--codec", type=Path, required=True)
    create.add_argument("--subject-sha256", required=True)
    create.add_argument("--output", type=Path, required=True)
    create.add_argument("--repo", type=Path, default=ROOT)
    check = sub.add_parser("verify")
    check.add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(plan(args) if args.action == "plan" else verify(args.plan), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
