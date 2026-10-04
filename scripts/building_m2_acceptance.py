"""Plan same-subject M2 feedback legs and inspect passive observations.

Coordinator execution only. This does not launch a build/window or accept art,
numeric freeze, performance, candidate promotion, installation, or release.
Use the native acceptance skill's guarded launcher and owned-window capture.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))

# Workflow helpers require the repository-relative search path above.
import building_asset_pipeline as pipeline  # noqa: E402
from building_clay_geometry import read_json  # noqa: E402
from building_m2_art import NUMERIC_SHA256, candidate_geometry  # noqa: E402

COMMON_LEGS = {
    "world-near-far": "Observe the normal world at normal and distant zoom; retain kind/state recognition captures.",
    "placement": "Use actual placement input: valid and rejected ghost, confirm construction, observe Blueprint and pulse.",
    "catalog": "Open the real build catalog before and after candidate load; capture its unchanged interaction.",
    "move-success": "Select the real move workflow, capture ghost and destination, then complete the move and verify feet.",
    "move-reject-cancel": "Attempt an invalid destination, cancel, and verify owner, resource and placement state did not change.",
    "save-load": "Save with existing gameplay controls, reload, verify owner reconstruction, feet and no duplicate roots.",
    "deconstruct": "Use actual deconstruction; capture owner, parts and associated items before and after cleanup.",
    "generation-fallback": "Coordinator exercises existing loader regression and isolated failure/reload fixture; do not promote authority.",
}
KIND_LEGS = {
    "Tank": {
        "water": "Use actual resource transport to observe Empty, Partial and Full with the same owner and two parts.",
        "companion": "Exercise Tank/BucketStorage placement and move companion; inspect partner ghost and destination anchors.",
    },
    "MudMixer": {
        "refining": "Supply actual inputs, begin refining, stop, pause while active, resume and finish; capture rotor and item overlap.",
    },
}


def frozen_subject(repo):
    if __package__:
        from . import building_art_acceptance as art
    else:
        import building_art_acceptance as art
    repo = art.native.validate_repo(str(repo))
    value = art.subject(repo)
    art.validate_subject(value, feedback=False)
    value["m2_driver"] = pipeline.digest(Path(__file__).read_bytes())
    return value


def verify(plan_path):
    """Re-read both prepared runtime sets, provenance, codec and frozen source."""
    value = pipeline.read(plan_path)
    pipeline.require(value["scope"] == "m2-feedback-plan-not-acceptance"
                     and value["numeric_candidate_sha256"] == NUMERIC_SHA256
                     and value["accepted"] is False and value["runtime_published"] is False,
                     "wrong runtime plan or approval claim")
    pipeline.require(value["independent_gates"] and all(gate is None for gate in value["independent_gates"].values())
                     and value["performance"]["accepted"] is False,
                     "runtime integrity verification cannot adopt approval claims")
    pipeline.require(value["subject"] == frozen_subject(Path(value["repo"]))
                     and value["source_sha256"] == value["subject"]["source"], "frozen subject changed")
    codec = pipeline.no_symlinks(Path(value["codec"]))
    pipeline.require(pipeline.digest(codec.read_bytes()) == value["codec_sha256"], "codec changed")
    pipeline.require(set(value["kinds"]) == set(KIND_LEGS), "both runtime kinds are required")
    identities = {}
    for kind, spec in value["kinds"].items():
        candidate_geometry(kind)
        root = pipeline.no_symlinks(Path(spec["source_root"]))
        manifest, text, _ = pipeline.load_set(root, kind, codec)
        pipeline.require(manifest["identity"] == spec["identity"]
                         and manifest["identity"]["authority"] == "art_preview"
                         and pipeline.digest(text.encode()) == spec["manifest_sha256"]
                         and manifest["geometry_contract_sha256"] == NUMERIC_SHA256,
                         "runtime identity or numeric binding changed")
        pipeline.require(manifest["receipt"] is None and manifest["art_approval_sha256"] is None,
                         "ArtPreview runtime cannot carry release/art approval")
        for relative, field in (("provenance/source", "source_sha256"),
                                ("provenance/geometry.json", "geometry_contract_sha256")):
            pipeline.require(pipeline.digest(pipeline.rooted(root, relative).read_bytes()) == manifest[field],
                             "runtime provenance changed")
        pipeline.require(manifest["export_sha256"] == pipeline.digest(pipeline.canonical(manifest["artifacts"])),
                         "runtime artifact inventory changed")
        identities[kind] = manifest["identity"]
    # Detect changes while the external codec and artifact readers were running.
    pipeline.require(value["subject"] == frozen_subject(Path(value["repo"]))
                     and pipeline.digest(codec.read_bytes()) == value["codec_sha256"]
                     and value == pipeline.read(plan_path), "inputs changed during runtime verification")
    return {"schema_version": 1, "status": "pass", "scope": "m2-prepared-runtime-integrity-only",
            "plan_sha256": pipeline.digest(plan_path.read_bytes()), "subject": value["subject"],
            "identities": identities, "numeric_candidate_sha256": NUMERIC_SHA256,
            "accepted": False, "numeric_freeze": False, "runtime_published": False}


def plan(args):
    pipeline.require(re.fullmatch(r"[0-9a-f]{64}", args.subject_sha256) is not None,
                     "use the coordinator frozen source fingerprint")
    repo = Path(args.repo).absolute()
    subject = frozen_subject(repo)
    pipeline.require(subject["source"] == args.subject_sha256, "supplied source fingerprint differs")
    kinds = {}
    for kind, root in (("Tank", args.tank_runtime), ("MudMixer", args.mixer_runtime)):
        candidate_geometry(kind)
        manifest, text, _ = pipeline.load_set(root, kind, args.codec)
        pipeline.require(manifest["identity"]["authority"] == "art_preview",
                         "M2 feedback must remain ArtPreview")
        # The M1-c loader seals this geometry record into the descriptor identity.
        pipeline.require(manifest["geometry_contract_sha256"] == NUMERIC_SHA256,
                         "runtime descriptor does not bind the exact numeric proposal")
        kinds[kind] = {"source_root": str(root.absolute()), "identity": manifest["identity"],
                       "manifest_sha256": pipeline.digest(text.encode()),
                       "legs": [{"id": name, "action": action, "accepted": False,
                                 "evidence": []} for name, action in
                                {**COMMON_LEGS, **KIND_LEGS[kind]}.items()]}
    result = {
        "schema_version": 1, "scope": "m2-feedback-plan-not-acceptance",
        "source_sha256": args.subject_sha256, "numeric_candidate_sha256": NUMERIC_SHA256,
        "repo": str(repo), "subject": subject, "codec": str(args.codec.absolute()),
        "codec_sha256": pipeline.digest(args.codec.read_bytes()), "kinds": kinds,
        "probe": {
            "environment": {"HW_M2_ACCEPTANCE_PROBE": "1"},
            "session": "Existing HW_BUILDING_ART_SESSION: mode feedback, exact per-kind identity, fresh nonce/status_path",
            "workload": "normal interactive profiling game, not paused building-art-static",
            "output": "session status_path with suffix .m2-trace.json",
            "limits": {"seconds_per_leg_max": 170, "samples_max": 1800, "entities_per_category_max": 128},
            "capture": "Use the established native skill launcher and owned actual-window capture; record renderer, GPU and backend",
            "restriction": "Observation trace changes cost; disable it for performance comparison",
        },
        "performance": {
            "accepted": False, "budget": None,
            "runs": ["baseline N", "candidate N", "baseline 4N", "candidate 4N"],
            "must_match": ["source", "renderer/GPU/backend", "population", "camera", "workload", "clock", "warmup/measurement"],
            "metrics": ["frame p50/p95/p99", "GPU frame time where supported", "RSS/native allocator", "mesh/material/image counts", "pool/owner cleanup"],
            "gate": "Coordinator records and independently approves explicit limits before comparing measured results",
        },
        "independent_gates": {gate: None for gate in (
            "numeric_freeze", "clay", "color_planes", "per_face_uv", "lines_brush_strokes",
            "near_far_readability", "Help", "art", "candidate", "fixed_review",
            "promotion", "install", "rollback", "formal_release")},
        "accepted": False, "runtime_published": False,
    }
    pipeline.put(args.output, pipeline.canonical(result))
    verify(args.output)
    return result


def inspect(plan_path, kind, trace_path):
    value = pipeline.read(plan_path)
    # The plan is canonical pipeline metadata, but the observation producer is
    # Rust serde_json::to_vec (no trailing newline, independent float spelling).
    # Keep strict JSON parsing without imposing Python's canonical byte format.
    trace = read_json(pipeline.no_symlinks(trace_path))
    pipeline.require(isinstance(trace, dict) and type(trace.get("schema_version")) is int
                     and trace["schema_version"] == 1, "unsupported observation schema")
    pipeline.require(value["scope"] == "m2-feedback-plan-not-acceptance"
                     and value["numeric_candidate_sha256"] == NUMERIC_SHA256, "wrong plan")
    pipeline.require(trace["scope"] == "m2-feedback-observations-only"
                     and trace["identity"] == value["kinds"][kind]["identity"]
                     and trace["failure"] is None and trace["accepted"] is False
                     and trace["performance_evidence"] is False, "invalid observation trace")
    samples = trace["samples"]
    pipeline.require(2 <= len(samples) <= 1800, "insufficient or overflowing observation trace")
    states, consumers, owners = set(), set(), set()
    active_samples = paused_samples = 0
    for sample in samples:
        pipeline.require(sample["identity"] in (None, trace["identity"]), "mixed candidate identity")
        pipeline.require(all(len(sample[key]) <= 128 for key in ("owners", "roots", "items", "consumers")),
                         "entity observation cap exceeded")
        if sample["identity"] is None:
            continue
        owners.update(item["entity"] for item in sample["owners"])
        states.update(root["state"] for root in sample["roots"])
        active_samples += int(not sample["paused"])
        paused_samples += int(sample["paused"])
        expected = sample["expected_images"]
        for consumer in sample["consumers"]:
            role = consumer["role"]
            target = expected["catalog" if role == "catalog" else "world"]
            if consumer["image"] == target:
                consumers.add(role)
    # Inventory is deliberately not a lifecycle verdict: a state string alone
    # does not prove a task succeeded, a rejected move, overlap, or visual quality.
    return {"scope": "m2-observation-inventory", "source_sha256": value["source_sha256"],
            "identity": trace["identity"], "trace_sha256": pipeline.digest(trace_path.read_bytes()),
            "samples": len(samples), "owners_seen": len(owners), "states_seen": sorted(states),
            "matching_consumers_seen": sorted(consumers), "unpaused_samples": active_samples,
            "paused_samples": paused_samples, "accepted": False,
            "remaining": "same-subject captures, lifecycle decisions, independent gates and performance/release evidence"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    create = sub.add_parser("plan")
    create.add_argument("--tank-runtime", type=Path, required=True)
    create.add_argument("--mixer-runtime", type=Path, required=True)
    create.add_argument("--codec", type=Path, required=True)
    create.add_argument("--subject-sha256", required=True)
    create.add_argument("--output", type=Path, required=True)
    create.add_argument("--repo", type=Path, default=ROOT)
    check = sub.add_parser("verify")
    location = check.add_mutually_exclusive_group(required=True)
    location.add_argument("--plan", type=Path)
    location.add_argument("--job-root", type=Path, help="Directory containing plan.json")
    inventory = sub.add_parser("inspect")
    inventory.add_argument("--plan", type=Path, required=True)
    inventory.add_argument("--kind", choices=tuple(KIND_LEGS), required=True)
    inventory.add_argument("--trace", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "plan":
        result = plan(args)
    elif args.action == "verify":
        result = verify(args.plan if args.plan else args.job_root / "plan.json")
    else:
        result = inspect(args.plan, args.kind, args.trace)
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
