"""Bounded seeded Mixer ArtPreview observations; no approval or OS-input claim."""
from __future__ import annotations

import argparse
import math
from pathlib import Path
import sys

if __package__:
    from . import building_m2_native_acceptance as shared
    from . import building_m2_candidate_storyboard as storyboard
    from . import building_body_paint_observation as body_paint
    from . import building_native_executable as executable
else:
    import building_m2_native_acceptance as shared
    import building_m2_candidate_storyboard as storyboard
    import building_body_paint_observation as body_paint
    import building_native_executable as executable

pipeline, native, m2 = shared.pipeline, shared.native, shared.m2
PROFILE = shared.PROFILE
SCOPE = "interactive-dirty-feedback-motion-observation-only"
TRACE_SCOPE = "m2-dirty-feedback-motion-observations-only"
MOTION = {"kind": "MudMixer", "axis": [0, 1, 0], "radians_per_second": math.pi / 2}


def drivers():
    return {Path(module.__file__).name: executable.digest(Path(module.__file__)) for module in
            (sys.modules[__name__], shared, m2, storyboard, body_paint, executable)}


def check_plan(value):
    pipeline.require(value.get("scope") == SCOPE and value.get("dirty_feedback") is True
        and value.get("kind") == "MudMixer" and value.get("leg") in ("refining", "save-load")
        and value.get("identity", {}).get("authority") == "art_preview"
        and value.get("input_scope") == "seeded-domain-operations"
        and value.get("clock_scope") == "preview-motion-quarter-speed-save-hold-v1"
        and pipeline.exact_json(value.get("production_state"), MOTION)
        and value.get("drivers") == drivers()
        and pipeline.exact_json(value.get("numeric_verifiers"), pipeline.numeric_verifier_inventory()),
        "dirty motion contract differs")
    base = dict(value, scope=shared.DIRTY_SCOPE)
    runtime = shared.check_plan(base)
    codec = pipeline.no_symlinks(Path(runtime["codec"]))
    source = pipeline.no_symlinks(Path(runtime["kinds"]["MudMixer"]["source_root"]))
    pipeline.verify_set(source, "MudMixer", codec)
    manifest, text, _ = pipeline.load_set(source, "MudMixer", codec)
    pipeline.require(manifest["identity"] == value["identity"]
        and pipeline.digest(text.encode()) == runtime["kinds"]["MudMixer"]["manifest_sha256"]
        and manifest["parts"] == value["parts"]
        and all(pipeline.exact_json(value.get(key), body_paint.fields(manifest).get(key))
                for key in ("body_paint_contract", "surface_paint_contract"))
        and value["drivers"] == drivers(), "dirty motion source or dependencies changed")
    shared.check_plan(base)
    return runtime


def plan(args):
    runtime = pipeline.read(args.runtime_plan)
    m2.verify(args.runtime_plan)
    pipeline.require(runtime.get("dirty_feedback") is True and args.kind == "MudMixer"
                     and args.leg in ("refining", "save-load"), "explicit dirty Mixer motion plan required")
    repo = Path(runtime["repo"])
    root = args.job_root.absolute() if args.job_root else native.unique_job_root(repo, "building-m2-dirty-motion")
    pipeline.require(not root.exists() and not args.output.absolute().is_relative_to(root), "fresh root/external plan required")
    source = Path(runtime["kinds"]["MudMixer"]["source_root"])
    manifest, _, _ = pipeline.load_set(source, "MudMixer", Path(runtime["codec"]))
    value = {"schema_version": 1, "status": "ready", "profile": PROFILE, "scope": SCOPE,
        "dirty_feedback": True, "input_scope": "seeded-domain-operations",
        "clock_scope": "preview-motion-quarter-speed-save-hold-v1", "repo": str(repo), "job_root": str(root),
        "subject": runtime["subject"], "runtime_plan": str(args.runtime_plan.absolute()),
        "runtime_plan_sha256": pipeline.digest(args.runtime_plan.read_bytes()),
        "identity": runtime["kinds"]["MudMixer"]["identity"], "kind": "MudMixer",
        **m2.numeric_binding("MudMixer", runtime["kinds"]["MudMixer"]),
        "leg": args.leg, "duration_seconds": args.duration_seconds, "adapter": args.adapter,
        "parts": manifest["parts"], "production_state": MOTION, **body_paint.fields(manifest),
        "drivers": drivers(), "numeric_verifiers": pipeline.numeric_verifier_inventory(),
        "driver_sha256": executable.digest(Path(__file__)), "accepted": False, "promotion_authority": False}
    check_plan(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.absolute())]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def check_trace(trace, value, nonce, pid):
    shared.check_trace(trace, value, nonce, pid, scope=TRACE_SCOPE)
    rows = [(sample, root) for sample in trace["samples"] for root in sample["roots"]
            if sample["identity"] == value["identity"] and root.get("owner") == sample.get("fixture_owner")
            and root.get("owner_exists") is True]
    for sample, root in rows:
        handles = sample.get("descriptor_handles") or {}
        pipeline.require(root.get("identity") == value["identity"], "preview presentation identity differs")
        for leaf in root["parts"]:
            material = leaf.get("material") or {}
            expected = body_paint.expected_texture(value, leaf["name"], handles, material.get("body_paint"))
            rotation = leaf.get("pose", {}).get("rotation")
            pipeline.require(leaf.get("parent") == root["entity"] and leaf.get("mesh_resident") is True
                and leaf.get("layer_3d") is True and leaf.get("mesh_handle") == handles.get("meshes", {}).get(leaf["name"])
                and material.get("albedo_resident") is True and material.get("albedo") == expected
                and material.get("opaque") is True and material.get("lit") is True
                and isinstance(rotation, list) and len(rotation) == 4
                and all(type(n) in (int, float) and math.isfinite(n) for n in rotation)
                and abs(sum(n*n for n in rotation)-1) < 0.001, "preview part/dependency differs")
    storyboard.check_motion(rows, value)


check_capture_interval = storyboard.check_capture_interval
capture_observed_interval = storyboard.capture_observed_interval
check_state_checkpoints = storyboard.check_state_checkpoints
check_binary = shared.check_binary
prepare_binary = shared.prepare_binary
prepare_overlay = shared.prepare_overlay
verify_overlay = shared.verify_overlay
runtime_mode = shared.runtime_mode


def probe_environment(value):
    return {"HW_M2_DIRTY_FEEDBACK_MOTION": "1"}


def inspect(path, kind, trace_path):
    m2.verify(path)
    runtime = pipeline.read(path)
    trace = m2.read_json(trace_path)
    pipeline.require(runtime.get("dirty_feedback") is True and kind == "MudMixer"
        and trace.get("scope") == TRACE_SCOPE and trace.get("identity") == runtime["kinds"][kind]["identity"]
        and trace.get("accepted") is False and trace.get("performance_evidence") is False,
        "dirty preview inventory identity/scope differs")
    return {"kind": kind, "identity": trace["identity"], "scope": TRACE_SCOPE, "samples": len(trace["samples"]),
            "input_scope": "seeded-domain-operations", "accepted": False, "promotion_authority": False}


def verify(root):
    return shared.verify_contract(root, sys.modules[__name__])


@native.activity_locked
def run(args):
    return shared.run_contract(args, sys.modules[__name__])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify"))
    for name in ("runtime-plan", "job-root", "output", "plan"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--kind", choices=("MudMixer",))
    parser.add_argument("--leg", choices=("refining", "save-load"))
    parser.add_argument("--adapter")
    parser.add_argument("--duration-seconds", type=int, default=60)
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.runtime_plan, args.output, args.kind, args.leg, args.adapter)), "incomplete plan")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    else:
        result = verify(args.job_root)
    native.print_json(result)


if __name__ == "__main__":
    main()
