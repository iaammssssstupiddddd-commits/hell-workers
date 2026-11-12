"""Exact-candidate seeded M2 domain storyboards; no OS-input or release authority.

Four bounded cases supplement existing candidate construction/water/save evidence.
The legacy ArtPreview runtime and scope remain separate. Main owns execution.
"""
from __future__ import annotations

import argparse
import math
from pathlib import Path
import sys

if __package__:
    from . import building_m2_native_acceptance as shared
    from . import building_native_executable as executable
else:
    import building_m2_native_acceptance as shared
    import building_native_executable as executable

art, pipeline, native, m2 = shared.art, shared.pipeline, shared.native, shared.m2
PROFILE = "building-m2-candidate-storyboard-v1"
SCOPE = "m2-candidate-seeded-storyboard-only"
CASES = {"Tank": {"move-success"}, "MudMixer": {"refining", "save-load", "move-success"}}


def drivers():
    return {Path(module.__file__).name: executable.digest(Path(module.__file__))
            for module in (sys.modules[__name__], shared, m2, executable)}


def check_dependencies(value):
    pipeline.require(value["drivers"] == drivers()
        and pipeline.exact_json(value.get("numeric_verifiers"), pipeline.numeric_verifier_inventory()),
        "candidate storyboard numeric/driver dependencies changed")


def check_plan(value):
    pipeline.require(value["profile"] == PROFILE and value["scope"] == SCOPE
        and value["status"] == "ready" and value["kind"] in CASES
        and value["leg"] in CASES[value["kind"]]
        and value["identity"]["authority"] == "isolated_candidate"
        and value["accepted"] is False and value["promotion_authority"] is False
        and value["input_scope"] == "seeded-domain-operations"
        and type(value["duration_seconds"]) is int and 35 <= value["duration_seconds"] <= 170,
        "candidate storyboard contract differs")
    check_dependencies(value)
    shared.adapter_environment(value["adapter"])
    repo = Path(value["repo"])
    art.validate_subject(value["subject"], feedback=False)
    pipeline.require(art.subject(repo) == value["subject"] and value["drivers"] == drivers(),
                     "candidate storyboard source/drivers changed")
    root = Path(value["job_root"])
    art.static.check_root(repo, root)
    path = pipeline.no_symlinks(Path(value["runtime_plan"]))
    pipeline.require(not path.is_relative_to(root) and executable.digest(path) == value["runtime_plan_sha256"],
                     "candidate runtime input changed")
    runtime = pipeline.read(path)
    pipeline.require(runtime["repo"] == value["repo"] and set(runtime["kinds"]) == {value["kind"]},
                     "candidate runtime kind/repo differs")
    spec = runtime["kinds"][value["kind"]]
    codec = pipeline.no_symlinks(Path(value["codec"]))
    pipeline.require(executable.digest(codec) == value["codec_sha256"], "candidate codec changed")
    source = pipeline.no_symlinks(Path(spec["source_root"]))
    pipeline.verify_set(source, value["kind"], codec)
    manifest, text, _ = pipeline.load_set(source, value["kind"], codec)
    binding = m2.numeric_binding(value["kind"], value)
    pipeline.require(manifest["identity"] == value["identity"] == spec["identity"]
        and pipeline.digest(text.encode()) == spec["manifest_sha256"]
        and manifest["geometry_contract_sha256"] == binding["numeric_candidate_sha256"]
        and m2.numeric_binding(value["kind"], spec) == binding
        and manifest["parts"] == value["parts"] and manifest.get("production_state") == value["production_state"],
        "candidate manifest/numeric/production state differs")
    check_dependencies(value)
    pipeline.require(art.subject(repo) == value["subject"]
        and executable.digest(codec) == value["codec_sha256"], "candidate inputs changed while checking")
    return runtime


def plan(args):
    repo, source, codec = native.validate_repo(str(args.repo)), args.source_root.resolve(), args.codec.resolve()
    pipeline.require(args.kind in CASES and args.leg in CASES[args.kind], "unsupported candidate case")
    root = args.job_root.resolve() if args.job_root else native.unique_job_root(repo, "building-m2-candidate")
    pipeline.require(not root.exists() and not args.output.resolve().is_relative_to(root), "fresh root/external plan required")
    manifest, text, _ = pipeline.load_set(source, args.kind, codec)
    binding = m2.numeric_binding(args.kind, {"revision": "v1" if args.kind == "Tank" else "v2",
        "numeric_candidate_sha256": manifest["geometry_contract_sha256"]})
    spec = {"source_root": str(source), "identity": manifest["identity"],
            "manifest_sha256": pipeline.digest(text.encode()), **binding}
    inputs = args.output.resolve().with_suffix(".inputs.json")
    pipeline.put(inputs, pipeline.canonical({"repo": str(repo), "kinds": {args.kind: spec}}))
    value = {"schema_version": 1, "status": "ready", "profile": PROFILE, "scope": SCOPE,
        "input_scope": "seeded-domain-operations", "repo": str(repo), "job_root": str(root),
        "subject": art.subject(repo), "runtime_plan": str(inputs), "runtime_plan_sha256": executable.digest(inputs),
        "kind": args.kind, "identity": manifest["identity"], "leg": args.leg,
        "codec": str(codec), "codec_sha256": executable.digest(codec), "parts": manifest["parts"],
        "production_state": manifest["production_state"], **binding,
        "duration_seconds": args.duration_seconds, "adapter": args.adapter, "drivers": drivers(),
        "numeric_verifiers": pipeline.numeric_verifier_inventory(),
        "driver_sha256": executable.digest(Path(__file__)), "accepted": False, "promotion_authority": False}
    check_plan(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.resolve())]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def part(root, name):
    matches = [item for item in root["parts"] if item["name"] == name]
    pipeline.require(len(matches) == 1, "candidate part is missing/duplicated")
    return matches[0]


def near(a, b, tolerance=0.0001):
    return (isinstance(a, list) and isinstance(b, list) and len(a) == len(b)
            and all(type(x) in (int, float) and math.isfinite(x) and type(y) in (int, float)
                    and math.isfinite(y) and abs(x-y) <= tolerance for x, y in zip(a, b, strict=True)))


def quaternion_step(rotation, axis, radians):
    x, y, z, w = rotation
    sine, cosine = math.sin(radians / 2), math.cos(radians / 2)
    a, b, c = (component * sine for component in axis)
    return [w*a + x*cosine + y*c - z*b, w*b - x*c + y*cosine + z*a,
            w*c + x*b - y*a + z*cosine, w*cosine - x*a - y*b - z*c]


def check_motion(rows, value):
    authored = {entry["name"]: entry for entry in value["parts"]}
    for sample, root in rows:
        pipeline.require(type(sample.get("storyboard_stage")) is int, "candidate stage observation missing")
        for name, spec in authored.items():
            pose = part(root, name)["pose"]
            translation = list(spec["translation_wu"])
            if value["kind"] == "Tank" and name == "water":
                state = root["state"]
                translation[1] = value["production_state"]["full_y_wu" if state == "Some(TankFull)" else "partial_y_wu"]
            pipeline.require(near(pose["translation"], translation)
                and near(pose["scale"], spec["scale"])
                and (name == "rotor" or near(pose["rotation"], spec["rotation_xyzw"])), "candidate body/authored pose drifted")
    if value["kind"] != "MudMixer":
        return
    for (before, first), (after, second) in zip(rows, rows[1:]):
        if first["entity"] != second["entity"]:
            continue
        if (not before["paused"] and not after["paused"]
            and first["state"] == second["state"] == "Some(MixerActive)"):
            delta = after["virtual_seconds"] - before["virtual_seconds"]
            state = value["production_state"]
            expected = quaternion_step(part(first, "rotor")["pose"]["rotation"], state["axis"],
                                       delta * state["radians_per_second"])
            actual = part(second, "rotor")["pose"]["rotation"]
            pipeline.require(near(actual, expected, 0.001) or near(actual, [-n for n in expected], 0.001),
                             "candidate rotor disagrees with Virtual clock/approved axis and speed")
        if ((before["paused"] and after["paused"])
            or first["state"] == second["state"] == "Some(MixerIdle)"):
            pipeline.require(near(part(first, "rotor")["pose"]["rotation"],
                                  part(second, "rotor")["pose"]["rotation"]), "paused/Idle rotor moved")
    if value["leg"] == "refining":
        paused = [i for i, (sample, root) in enumerate(rows)
                  if sample["paused"] and root["state"] == "Some(MixerActive)"]
        pipeline.require(paused, "candidate active pause missing")
        resumed = [root for sample, root in rows[max(paused)+1:]
                   if not sample["paused"] and root["state"] == "Some(MixerActive)"]
        pipeline.require(len(resumed) >= 2 and not near(part(resumed[0], "rotor")["pose"]["rotation"],
            part(resumed[-1], "rotor")["pose"]["rotation"]), "candidate rotor did not resume")
        pipeline.require(len({root["entity"] for _, root in rows}) == 1
            and len({tuple(part(root, name)["entity"] for name in ("body", "rotor")) for _, root in rows}) == 1,
            "candidate refining replaced its owned parts")
    if value["leg"] == "save-load":
        first = rows[0][1]
        final = rows[-1][1]
        pipeline.require(final["state"] == "Some(MixerIdle)" and rows[-1][0]["paused"]
            and near(part(final, "rotor")["pose"]["rotation"], authored["rotor"]["rotation_xyzw"])
            and final["entity"] != first["entity"]
            and {p["entity"] for p in final["parts"]}.isdisjoint(p["entity"] for p in first["parts"]),
            "loaded candidate Idle/reset/part cleanup differs")


def check_trace(trace, value, nonce, pid):
    shared.check_trace(trace, value, nonce, pid, scope=SCOPE)
    rows = [(sample, root) for sample in trace["samples"] for root in sample["roots"]
            if sample["identity"] == value["identity"]
            and root.get("owner") == sample.get("fixture_owner") and root.get("owner_exists") is True]
    for sample, root in rows:
        handles = sample.get("descriptor_handles") or {}
        pipeline.require(root.get("identity") == value["identity"], "candidate presentation identity differs")
        for leaf in root["parts"]:
            material = leaf.get("material") or {}
            pipeline.require(leaf.get("parent") == root["entity"] and leaf.get("mesh_resident") is True and leaf.get("layer_3d") is True
                and leaf.get("mesh_handle") == handles.get("meshes", {}).get(leaf["name"])
                and material.get("albedo_resident") is True and material.get("albedo") == handles.get("albedo")
                and material.get("opaque") is True and material.get("lit") is True,
                "candidate part/dependency residency differs")
            rotation = leaf.get("pose", {}).get("rotation")
            pipeline.require(isinstance(rotation, list) and len(rotation) == 4
                and all(type(n) in (int, float) and math.isfinite(n) for n in rotation)
                and abs(sum(n*n for n in rotation)-1) < 0.001, "candidate quaternion invalid")
    check_motion(rows, value)
    if value["kind"] == "Tank" and value["leg"] == "move-success":
        first, final = rows[0][0], rows[-1][0]
        owners = [next(owner["pose"]["translation"] for owner in sample["owners"]
                       if owner["entity"] == sample["fixture_owner"]) for sample in (first, final)]
        delta = [b-a for a, b in zip(*owners, strict=True)]
        initial = {item["entity"]: item["pose"]["translation"] for item in first["companions"]}
        moved = {item["entity"]: item["pose"]["translation"] for item in final["companions"]}
        pipeline.require(len(initial) == 2 and initial.keys() == moved.keys()
            and all(near(moved[key], [x+d for x, d in zip(position, delta, strict=True)])
                    for key, position in initial.items()), "Tank linked companions did not follow the same move")


def check_capture_interval(samples, shot):
    shared.check_capture_interval(samples, shot)
    for sample in samples[shot["sample_index"]:shot["sample_end_index"]+1]:
        roots = [root for root in sample["roots"] if root.get("owner") == sample.get("fixture_owner")
                 and root.get("owner_exists") is True]
        pipeline.require(len(roots) == 1, "candidate capture has no unique owned presentation")
        for leaf in roots[0]["parts"]:
            visible = not (leaf["name"] == "water" and roots[0]["state"] == "Some(TankEmpty)")
            pipeline.require(leaf.get("layer_3d") is True
                and (not visible or leaf.get("visible") is True
                     and leaf.get("visibility") in ("Some(Inherited)", "Some(Visible)")),
                "candidate capture part is hidden or outside the render layer")


def capture_observed_interval(root, pid, path, before):
    shot, after = shared.capture_observed_interval(root, pid, path, before)
    if shot is not None:
        try:
            check_capture_interval(after["samples"], shot)
        except ValueError as error:
            pipeline.put(root / "candidate-rejected.json", pipeline.canonical({"reason": str(error), "capture": shot}))
            return None, after
    return shot, after


def check_state_checkpoints(shots, value):
    shared.check_state_checkpoints(shots, value)
    if value["leg"] == "refining":
        sequence = [("Some(MixerIdle)", False), ("Some(MixerActive)", False),
                    ("Some(MixerActive)", True), ("Some(MixerActive)", False), ("Some(MixerIdle)", False)]
        index = 0
        for shot in shots:
            if index < len(sequence) and sequence[index][0] in shot["key"][3] and sequence[index][1] is shot["key"][5]:
                index += 1
        pipeline.require(index == len(sequence), "candidate pause/resume/Idle capture sequence missing")


def prepare_binary(binary, root):
    return executable.freeze(binary, root)


def check_binary(result, root, value):
    executable.validate(Path(result["binary"]["path"]), root, result["binary"]["sha256"])


def probe_environment(value):
    return {"HW_M2_CANDIDATE_STORYBOARD": "1"}


def runtime_mode(value):
    return "candidate"


def inspect(path, kind, trace_path):
    runtime = pipeline.read(path)
    trace = m2.read_json(trace_path)
    pipeline.require(trace["scope"] == SCOPE and trace["identity"] == runtime["kinds"][kind]["identity"],
                     "candidate inventory identity/scope differs")
    return {"kind": kind, "identity": trace["identity"], "scope": SCOPE, "samples": len(trace["samples"]),
            "input_scope": "seeded-domain-operations", "accepted": False, "promotion_authority": False}


def verify(root):
    return shared.verify_contract(root, sys.modules[__name__])


@native.activity_locked
def run(args):
    return shared.run_contract(args, sys.modules[__name__])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify"))
    for name in ("repo", "source-root", "codec", "job-root", "output", "plan"):
        parser.add_argument("--"+name, type=Path)
    parser.add_argument("--kind", choices=tuple(CASES))
    parser.add_argument("--leg")
    parser.add_argument("--adapter")
    parser.add_argument("--duration-seconds", type=int, default=60)
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.repo, args.source_root, args.codec, args.output, args.kind, args.leg, args.adapter)),
                         "incomplete candidate plan")
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
