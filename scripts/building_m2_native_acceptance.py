"""Registered-launcher recipe for bounded, normal interactive M2 observation.

Plan/run/verify are coordinator-only entrypoints. A pass seals runtime integrity,
active simulation and owned X11/Vulkan observation, not a lifecycle verdict or
numeric/art/performance/release approval. Water/refining use domain fixtures;
other lifecycle legs fail closed until their own deterministic drivers exist.
"""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import time

if __package__:
    from . import building_art_acceptance as art
    from . import building_m2_acceptance as m2
else:
    import building_art_acceptance as art
    import building_m2_acceptance as m2

pipeline, native = art.pipeline, art.native
PROFILE = "building-m2-interactive-v1"
SCOPE = "interactive-feedback-observation-only"
DRIVEN_LEGS = {"Tank": "water", "MudMixer": "refining"}
LIFECYCLE_LEGS = {"save-load", "deconstruct", "placement", "catalog", "world-near-far",
                  "move-success", "move-reject-cancel", "companion"}


def check_plan(value):
    pipeline.require(value.get("status") == "ready", "native plan is not ready")
    pipeline.require(value["profile"] == PROFILE and value["scope"] == SCOPE
                     and value["kind"] in m2.KIND_LEGS and value["accepted"] is False
                     and value["promotion_authority"] is False, "wrong interactive plan")
    pipeline.require(value["leg"] in LIFECYCLE_LEGS | {DRIVEN_LEGS[value["kind"]]}
                     and (value["leg"] != "companion" or value["kind"] == "Tank")
                     and type(value["duration_seconds"]) is int
                     and 30 <= value["duration_seconds"] <= 170, "invalid native leg")
    adapter_environment(value["adapter"])
    plan_path = pipeline.no_symlinks(Path(value["runtime_plan"]))
    pipeline.require(pipeline.digest(plan_path.read_bytes()) == value["runtime_plan_sha256"], "runtime plan changed")
    result = m2.verify(plan_path)
    runtime = pipeline.read(plan_path)
    pipeline.require(result["subject"] == value["subject"]
                     and runtime["repo"] == value["repo"]
                     and runtime["kinds"][value["kind"]]["identity"] == value["identity"], "native subject differs")
    root = pipeline.no_symlinks(Path(value["job_root"]))
    art.static.check_root(Path(value["repo"]), root)
    pipeline.require(not plan_path.is_relative_to(root), "input plan must survive output creation")
    return runtime


def plan(args):
    runtime = pipeline.read(args.runtime_plan)
    m2.verify(args.runtime_plan)
    repo = Path(runtime["repo"])
    root = (args.job_root.absolute() if args.job_root else native.unique_job_root(repo, "building-m2-interactive"))
    pipeline.require(not root.exists(), "use a fresh native job root")
    # Readiness permits controller registration; it does not grant acceptance.
    value = {"schema_version": 1, "status": "ready", "profile": PROFILE, "scope": SCOPE,
             "repo": str(repo), "job_root": str(root), "subject": runtime["subject"],
             "runtime_plan": str(args.runtime_plan.absolute()),
             "runtime_plan_sha256": pipeline.digest(args.runtime_plan.read_bytes()),
             "identity": runtime["kinds"][args.kind]["identity"], "kind": args.kind,
             "leg": args.leg, "duration_seconds": args.duration_seconds, "adapter": args.adapter,
             "driver_sha256": pipeline.digest(Path(__file__).read_bytes()),
             "accepted": False, "promotion_authority": False}
    check_plan(value)
    pipeline.require(not args.output.absolute().is_relative_to(root), "launcher plan must be outside fresh job root")
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3",
        str(repo / "scripts/building_m2_native_acceptance.py"), "run", "--plan", str(args.output.absolute())]
    value["resources"] = resources
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def check_trace(trace, value, nonce, pid):
    pipeline.require(value.get("leg") in LIFECYCLE_LEGS | {DRIVEN_LEGS.get(value["identity"]["kind"])},
                     "unsupported fixture leg")
    pipeline.require(type(pid) is int and pid > 0 and isinstance(nonce, str)
                     and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None, "invalid trace process/session")
    pipeline.require(isinstance(trace, dict) and type(trace.get("schema_version")) is int
                     and trace["schema_version"] == 1
                     and trace.get("scope") == "m2-feedback-observations-only"
                     and trace.get("leg") == value["leg"]
                     and trace.get("identity") == value["identity"] and trace.get("nonce") == nonce
                     and type(trace.get("process_id")) is int and trace["process_id"] == pid
                     and trace.get("failure") is None and trace.get("accepted") is False
                     and trace.get("performance_evidence") is False, "trace session/authority mismatch")
    samples = trace["samples"]
    pipeline.require(isinstance(samples, list) and 2 <= len(samples) <= 1800, "trace sample count differs")
    active = []
    observed_target = False
    owned = []
    previous = -1.0
    for sample in samples:
        pipeline.require(sample["identity"] in (None, value["identity"]), "trace generation differs")
        for key in ("real_seconds", "virtual_seconds"):
            pipeline.require(type(sample[key]) in (int, float) and math.isfinite(sample[key])
                             and sample[key] >= 0, "invalid trace clock")
        pipeline.require(sample["real_seconds"] > previous and type(sample["paused"]) is bool,
                         "trace clock/order differs")
        previous = sample["real_seconds"]
        for key in ("owners", "roots", "items", "consumers"):
            pipeline.require(isinstance(sample[key], list) and len(sample[key]) <= 128, "trace entity cap differs")
        if sample["identity"] == value["identity"] and not sample["paused"]:
            active.append(sample["virtual_seconds"])
        if sample["identity"] == value["identity"]:
            owner = sample.get("fixture_owner")
            roots = [root for root in sample["roots"] if owner is not None
                     and root.get("owner") == owner and root.get("owner_exists") is True
                     and len(root.get("parts", [])) == 2
                     and any(entity.get("entity") == owner for entity in sample["owners"])]
            pipeline.require(len(roots) <= 1, "duplicate fixture presentation root")
            if roots:
                logical = next(entity for entity in sample["owners"] if entity["entity"] == owner)
                position = logical.get("pose", {}).get("translation")
                feet = roots[0].get("pose", {}).get("translation")
                pipeline.require(isinstance(position, list) and len(position) == 3
                                 and isinstance(feet, list) and len(feet) == 3
                                 and all(type(number) in (int, float) and math.isfinite(number)
                                         for number in position + feet)
                                 and abs(feet[0] - position[0]) <= 0.0001
                                 and abs(feet[2] + position[1]) <= 0.0001, "fixture feet do not follow logical owner")
                owned.append((sample, roots[0]))
            expected = sample.get("expected_images") or {}
            observed_target |= any(isinstance(consumer.get("image"), str)
                                   and consumer["image"] != "None"
                                   and consumer["image"] == expected.get(
                                       "catalog" if consumer.get("role") == "catalog" else "world")
                                   for consumer in sample["consumers"])
    pipeline.require(len(active) >= 2 and max(active) - min(active) >= 0.1,
                     "no active simulation with the exact candidate")
    pipeline.require(observed_target, "no candidate world/preview consumer observed")
    state_rows = owned
    if value["leg"] in LIFECYCLE_LEGS:
        pipeline.require(owned, "no owned fixture presentation observed")
        state_rows = [(sample, root) for sample, root in owned if root["owner"] == owned[0][1]["owner"]]
        check_lifecycle(samples, owned, value)
    check_owned_states(state_rows, value)


def check_lifecycle(samples, owned, value):
    first_sample, first_root = owned[0]
    final_sample = samples[-1]
    lifecycle = final_sample.get("lifecycle") or {}
    pipeline.require(lifecycle.get("leg") == value["leg"], "lifecycle leg binding differs")
    events = lifecycle.get("events", [])
    owner = first_root["owner"]
    if value["leg"] == "deconstruct":
        pipeline.require(any(event.get("operation") == "Deconstruct" and event.get("result") == "Committed"
                             and event.get("owner") == owner for event in events), "no production deconstruction commit")
        pipeline.require(final_sample.get("fixture_owner") == owner
                         and final_sample.get("fixture_owner_exists") is False
                         and not any(root.get("owner") == owner for root in final_sample["roots"])
                         and not any(item.get("owner") == owner for item in final_sample["items"]),
                         "deconstruction owner/parts/items cleanup incomplete")
    elif value["leg"] == "save-load":
        pipeline.require([(event.get("operation"), event.get("result")) for event in events]
                         == [("Save", "Succeeded"), ("Load", "Succeeded")], "save/load outcome sequence differs")
        pipeline.require(type(first_sample.get("world_epoch")) is int
                         and type(final_sample.get("world_epoch")) is int
                         and final_sample["world_epoch"] > first_sample["world_epoch"], "world was not replaced")
        final_owner = final_sample.get("fixture_owner")
        roots = [root for root in final_sample["roots"] if root.get("owner") == final_owner]
        baseline = next(root for _, root in reversed(owned) if root["owner"] == owner)
        pipeline.require(final_owner != owner and final_sample.get("fixture_owner_exists") is True
                         and len(roots) == 1 and roots[0].get("owner_exists") is True
                         and roots[0].get("pose") is not None and roots[0].get("pose") == baseline.get("pose")
                         and len(roots[0].get("parts", [])) == 2
                         and not any(root.get("owner") == owner for root in final_sample["roots"]),
                         "loaded owner/feet/parts reconstruction differs")
    else:
        check_interaction(samples, owned, value)


def placement_pulse_observed(sample, kind):
    owner = sample.get("destination_owner")
    image = (sample.get("expected_images") or {}).get("world")
    if sample.get("paused") is not False or not owner or not isinstance(image, str) or image == "None":
        return False
    def matches(consumer, role):
        progress = consumer.get("blueprint_progress")
        return (consumer.get("role") == role and consumer.get("visible") is True
                and consumer.get("image") == image and consumer.get("blueprint_owner") == owner
                and consumer.get("blueprint_kind") == kind and consumer.get("blueprint_state") == "Building"
                and type(progress) in (int, float) and 0 < progress < 1
                and ((role == "blueprint" and consumer.get("entity") == owner)
                     or (role == "pulse" and consumer.get("entity") not in (None, owner))))
    return all(any(matches(consumer, role) for consumer in sample["consumers"])
               for role in ("blueprint", "pulse"))


def check_interaction(samples, owned, value):
    def seen(role):
        return any(consumer.get("role") == role and consumer.get("visible") is True
                   and consumer.get("image") == (sample.get("expected_images") or {}).get(
                       "catalog" if role == "catalog" else "world")
                   for sample in samples for consumer in sample["consumers"])
    leg = value["leg"]
    owner = owned[0][1]["owner"]
    final = samples[-1]
    if leg == "world-near-far":
        pipeline.require(any(sample.get("camera_scale") == 1.0 for sample, _ in owned)
                         and any(sample.get("camera_scale") == 2.5 for sample, _ in owned), "near/far views missing")
    elif leg == "catalog":
        pipeline.require(seen("catalog"), "selected catalog card was not displayed")
    else:
        pipeline.require(seen("ghost"), "real candidate ghost missing")
        if leg == "move-reject-cancel":
            pipeline.require(any((sample.get("placement_rejection") or {}).get("header") == "Cannot place"
                                 for sample in samples) and final.get("play_mode") == "Normal"
                             and not seen("destination"), "move rejection/cancel outcome missing")
            positions = [entity["pose"]["translation"] for sample, _ in owned for entity in sample["owners"]
                         if entity["entity"] == owner]
            pipeline.require(positions and all(pos == positions[0] for pos in positions), "rejected move changed owner")
        elif leg == "placement":
            held_since = None
            held_owner = None
            held = False
            for sample in samples:
                if placement_pulse_observed(sample, value["identity"]["kind"]):
                    if held_since is None or held_owner != sample["destination_owner"]:
                        held_since, held_owner = sample["real_seconds"], sample["destination_owner"]
                    held |= sample["real_seconds"] - held_since >= 3.0
                else:
                    held_since = None
            pipeline.require(held, "production Blueprint/pulse owner-bound hold missing")
            lifecycle = final.get("lifecycle") or {}
            events = lifecycle.get("events", [])
            presses = [event for event in events if event.get("operation") == "PlacementPress"]
            anchor = final.get("destination_anchor")
            pipeline.require(isinstance(anchor, list) and len(anchor) == 2
                             and len(presses) >= 2 and presses[0].get("target") == [-10, -10]
                             and presses[1].get("target") == anchor
                             and all(press.get("projected") == press.get("target") for press in presses),
                             "placement cursor/anchor evidence differs")
            rejection = next((index for index, event in enumerate(events)
                              if event.get("operation") == "PlacementRejected" and event.get("target") == [-10, -10]), None)
            pipeline.require(rejection is not None and events.index(presses[0]) < rejection < events.index(presses[1]),
                             "placement rejection/retry sequence missing")
            pipeline.require(lifecycle.get("pulse_observation_complete") is True
                             and lifecycle.get("placement_blueprint") == held_owner,
                             "placement observation ACK missing or owner differs")
            pipeline.require(any((sample.get("placement_rejection") or {}).get("header") == "Cannot place"
                                 for sample in samples), "rejected placement was not observed")
            destination = final.get("destination_owner")
            pipeline.require(destination is not None and destination != owner
                             and any(root.get("owner") == destination and root.get("owner_exists") is True
                                     and len(root.get("parts", [])) == 2 for root in final["roots"]),
                             "placed Blueprint did not produce an owned building")
        else:
            pipeline.require(seen("destination") and final.get("destination_owner") == owner,
                             "move destination did not become owned")
            initial = next(entity["pose"]["translation"] for entity in owned[0][0]["owners"] if entity["entity"] == owner)
            current = next((entity["pose"]["translation"] for entity in final["owners"] if entity["entity"] == owner), None)
            pipeline.require(current is not None and current != initial, "move did not change owner position")
        if value["identity"]["kind"] == "Tank" and leg != "move-reject-cancel":
            pipeline.require(seen("companion") and len(final.get("companions", [])) == 2,
                             "Tank companion ghost/ownership missing")


def checkpoint_key(sample):
    return [sample.get("fixture_owner_exists"), sample.get("camera_scale"), sample.get("play_mode"),
            sorted(root.get("state", "") for root in sample["roots"]),
            sorted(consumer["role"] for consumer in sample["consumers"] if consumer.get("visible") is True),
            sample["paused"]]


def required_owned_states(value):
    kind, leg = value["identity"]["kind"], value["leg"]
    pipeline.require(kind in DRIVEN_LEGS and leg in LIFECYCLE_LEGS | {DRIVEN_LEGS[kind]}
                     and (leg != "companion" or kind == "Tank"), "unsupported fixture leg")
    if kind == "Tank":
        return [("Some(TankEmpty)", False), ("Some(TankPartial)", False), ("Some(TankFull)", False)]
    if leg == "refining":
        return [("Some(MixerIdle)", False), ("Some(MixerActive)", False),
                ("Some(MixerActive)", True), ("Some(MixerActive)", False), ("Some(MixerIdle)", False)]
    return [("Some(MixerIdle)", False)]


def check_state_checkpoints(checkpoints, value):
    expected = required_owned_states(value)
    if value["identity"]["kind"] == "Tank":
        pipeline.require({state for state, _ in expected}
                         <= {state for shot in checkpoints for state in shot["key"][3]},
                         "Tank state client captures missing")
        return
    pipeline.require(all(any(state in shot["key"][3] and shot["key"][5] is paused for shot in checkpoints)
                         for state, paused in expected), "required leg state/pause client captures missing")


def check_owned_states(owned, value):
    """Require observations of the real owner; catalog matches cannot stand in for it."""
    pipeline.require(owned, "no owned fixture presentation observed")
    pipeline.require(len({root["owner"] for _, root in owned}) == 1, "fixture owner changed during state leg")
    expected = required_owned_states(value)
    roles = {"body", "water" if value["identity"]["kind"] == "Tank" else "rotor"}
    pipeline.require(all({part.get("name") for part in root["parts"]} == roles for _, root in owned),
                     "fixture parts are not the selected candidate roles")
    index = 0
    for sample, root in owned:
        if index < len(expected) and (root.get("state"), sample["paused"]) == expected[index]:
            index += 1
    pipeline.require(index == len(expected), "required owned state/pause/resume sequence missing")
    if value["identity"]["kind"] == "MudMixer" and value["leg"] == "refining":
        pipeline.require(any(root.get("state") == "Some(MixerActive)"
                             and any(item.get("owner") == root["owner"] for item in sample["items"])
                             for sample, root in owned), "no active item/rotor overlap observation")
        rotations = [part.get("pose", {}).get("rotation") for sample, root in owned
                     if root.get("state") == "Some(MixerActive)" and not sample["paused"]
                     for part in root["parts"] if "rotor" in (part.get("name") or "").lower()]
        pipeline.require(len({json.dumps(rotation) for rotation in rotations if rotation is not None}) >= 2,
                         "active rotor did not move")
        paused = [part.get("pose", {}).get("rotation") for sample, root in owned
                  if root.get("state") == "Some(MixerActive)" and sample["paused"]
                  for part in root["parts"] if part.get("name") == "rotor"]
        pipeline.require(len(paused) >= 2 and all(rotation is not None for rotation in paused)
                         and len({json.dumps(rotation) for rotation in paused[1:]}) == 1,
                         "paused rotor did not remain frozen")


def adapter_environment(selector):
    """The plan stores the WGPU name selector, not an observed GPU identity."""
    pipeline.require(isinstance(selector, str) and selector.strip(), "empty adapter selector")
    return {"WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": selector}


def check_renderer(log, adapter):
    adapter_environment(adapter)
    pipeline.require(not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log), "native warnings/errors")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    pipeline.require(log.count("AdapterInfo {") == 1 and len(adapters) == 1 and adapters[0][1] == "Vulkan"
                     and adapter.casefold() in adapters[0][0].casefold(), "renderer/GPU differs")
    return {"selector": adapter, "name": adapters[0][0], "backend": adapters[0][1]}


def check_renderer_record(log, adapter, recorded):
    pipeline.require(recorded == check_renderer(log, adapter), "recorded renderer differs from native log")


def check_asset_view(repo, view, manifest, text):
    def inventory(root):
        result = {}
        for path in pipeline.no_symlinks(root).rglob("*"):
            pipeline.no_symlinks(path)
            if path.is_file():
                result[path.relative_to(root).as_posix()] = pipeline.digest(path.read_bytes())
        return result
    expected = inventory(repo / "assets")
    expected.update({item["path"]: item["sha256"] for item in manifest["artifacts"]})
    expected[pipeline.locator(manifest["identity"]["kind"])] = pipeline.digest(text.encode())
    pipeline.require(inventory(view / "assets") == expected, "private asset view differs from frozen inputs")


def verify(root):
    root = pipeline.no_symlinks(root)
    result = pipeline.read(root / "manifest.json")
    value = result["plan"]
    runtime = check_plan(value)
    pipeline.require(Path(value["job_root"]) == root and result["profile"] == PROFILE
                     and result["scope"] == SCOPE and result["status"] == "pass"
                     and result["accepted"] is False and result["promotion_authority"] is False
                     and value["driver_sha256"] == pipeline.digest(Path(__file__).read_bytes()), "native result differs")
    pipeline.require(result["termination"] == "bounded-stop" and result["exit_code"] in (0, -15),
                     "unexpected process termination")
    binary = pipeline.no_symlinks(Path(result["binary"]["path"]))
    pipeline.require(binary == Path(value["repo"]) / "target/debug/bevy_app"
                     and pipeline.digest(binary.read_bytes()) == result["binary"]["sha256"], "native binary changed")
    trace_path = root / "probe.m2-trace.json"
    captured_path = root / "captured-trace.json"
    for path, key in ((trace_path, "trace_sha256"), (captured_path, "captured_trace_sha256"),
                      (root / "actual-window.log", "log_sha256"), (root / "build.log", "build_log_sha256")):
        pipeline.require(pipeline.digest(pipeline.no_symlinks(path).read_bytes()) == result[key], "native artifact changed")
    trace, captured = m2.read_json(trace_path), m2.read_json(captured_path)
    check_trace(trace, value, result["nonce"], result["pid"])
    check_trace(captured, value, result["nonce"], result["pid"])
    pipeline.require(trace["samples"][:len(captured["samples"])] == captured["samples"], "capture trace is not a run prefix")
    screenshot = result["screenshot"]
    image = pipeline.rooted(root, screenshot["path"])
    pipeline.require(screenshot["capture_scope"] == "owned-x11-client"
                     and screenshot["window_pid"] == result["pid"]
                     and pipeline.digest(image.read_bytes()) == screenshot["sha256"]
                     and native.validate_png_structure(image.read_bytes()) == (1280, 720), "owned window capture differs")
    checkpoints = result.get("checkpoints", [])
    pipeline.require(1 <= len(checkpoints) <= 24, "bounded checkpoint captures missing")
    pipeline.require(len({shot["path"] for shot in checkpoints}) == len(checkpoints), "checkpoint images aliased")
    previous_index = -1
    for shot in checkpoints:
        image = pipeline.rooted(root, shot["path"])
        index = shot["sample_index"]
        pipeline.require(type(index) is int and previous_index < index < len(trace["samples"])
                         and shot["key"] == checkpoint_key(trace["samples"][index])
                         and shot["window_pid"] == result["pid"] and shot["capture_scope"] == "owned-x11-client"
                         and pipeline.digest(image.read_bytes()) == shot["sha256"]
                         and native.validate_png_structure(image.read_bytes()) == (1280, 720), "checkpoint binding differs")
        previous_index = index
    if value["leg"] == "world-near-far":
        pipeline.require({1.0, 2.5} <= {shot["key"][1] for shot in checkpoints}, "near/far client captures missing")
    check_state_checkpoints(checkpoints, value)
    log = (root / "actual-window.log").read_text()
    check_renderer_record(log, value["adapter"], result.get("renderer"))
    # Re-read the private runtime asset overlay as well as its prepared source.
    manifest, text, _ = pipeline.load_set(root / "asset-view/assets", value["kind"], binary)
    spec = runtime["kinds"][value["kind"]]
    pipeline.require(manifest["identity"] == value["identity"]
                     and pipeline.digest(text.encode()) == spec["manifest_sha256"], "loaded asset overlay changed")
    check_asset_view(Path(value["repo"]), root / "asset-view", manifest, text)
    pipeline.require(result["inventory"] == m2.inspect(Path(value["runtime_plan"]), value["kind"], trace_path),
                     "observation inventory changed")
    check_plan(value)
    pipeline.require(pipeline.digest(binary.read_bytes()) == result["binary"]["sha256"]
                     and result == pipeline.read(root / "manifest.json"), "native inputs changed during verification")
    return result


@native.activity_locked
def run(args):
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use registered no-prompt launcher")
    value = pipeline.read(args.plan)
    runtime = check_plan(value)
    pipeline.require(value["driver_sha256"] == pipeline.digest(Path(__file__).read_bytes()), "native driver changed")
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    job = root / "job.json"
    state = {"status": "running", "profile": PROFILE, "pid": os.getpid(),
             "started_at": native.utc_now(), "heartbeat_at": native.utc_now()}
    native.atomic_write_json(job, state)
    try:
        env = native.cargo_environment(repo)
        native.run_command("build", ["python3", "scripts/dev.py", "feedback", "--build-only"],
            repo=repo, env=env, log_path=root / "build.log", job_file=job, state=state, timeout_seconds=3600)
        binary = repo / "target/debug/bevy_app"
        check_plan(value)
        binary_hash = pipeline.digest(binary.read_bytes())
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        pipeline.require(art.asset_view_fingerprint(view) == value["subject"]["assets"], "fallback assets changed")
        source = Path(runtime["kinds"][value["kind"]]["source_root"])
        manifest, _, _ = pipeline.load_set(source, value["kind"], binary)
        for artifact in manifest["artifacts"]:
            pipeline.put(pipeline.rooted(view / "assets", artifact["path"]), pipeline.file_bytes(source, artifact))
        locator = pipeline.locator(value["kind"])
        pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
        for key in list(env):
            if key.startswith(("HW_BUILDING_ART", "HW_M2", "HW_WALL", "HW_DOOR", "HW_DISABLE_", "HW_PERF_")):
                del env[key]
        nonce = secrets.token_hex(16)
        env.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
            **adapter_environment(value["adapter"]), "WINIT_X11_SCALE_FACTOR": "1",
            "HW_M2_ACCEPTANCE_PROBE": "1", "HW_M2_ACCEPTANCE_LEG": value["leg"],
            "HW_BUILDING_ART_SESSION": json.dumps({
                "mode": "feedback", "identity": value["identity"], "locator": locator,
                "nonce": nonce, "status_path": str(root / "probe.json")})})
        native.admit_stage_start("actual-window", state=state, job_file=job)
        screenshot = None
        checkpoints = []
        captured_keys = set()
        started = time.monotonic()
        # Private cwd keeps interactive saves out of the repository/user save directory.
        with (root / "actual-window.log").open("w") as log:
            process = subprocess.Popen([str(binary)], cwd=view, env=env, stdout=log,
                stderr=subprocess.STDOUT, start_new_session=True, pass_fds=native.activity_pass_fds(env))
            native.update_state(job, state, child_pid=process.pid, current_stage="actual-window")
            try:
                while time.monotonic() - started < value["duration_seconds"]:
                    pipeline.require(process.poll() is None, "interactive process exited before bounded capture")
                    trace_path = root / "probe.m2-trace.json"
                    if trace_path.exists():
                        current = m2.read_json(trace_path)
                        if current.get("samples") and current["samples"][-1].get("fixture_owner") is not None:
                            key = checkpoint_key(current["samples"][-1])
                            key_text = json.dumps(key)
                            if key_text not in captured_keys:
                                pipeline.require(len(checkpoints) < 24, "checkpoint capture cap exceeded")
                                capture_root = root / "checkpoints" / str(len(checkpoints))
                                capture_root.mkdir(parents=True, exist_ok=True)
                                shot = art.capture(capture_root, process.pid)
                                if shot is not None:
                                    shot.update(path=str((capture_root / shot["path"]).relative_to(root)),
                                                key=key, sample_index=len(current["samples"]) - 1)
                                    checkpoints.append(shot)
                                    captured_keys.add(key_text)
                    if screenshot is None and time.monotonic() - started >= value["duration_seconds"] - 2:
                        trace = m2.read_json(root / "probe.m2-trace.json")
                        check_trace(trace, value, nonce, process.pid)
                        screenshot = art.capture(root, process.pid)
                        if screenshot is not None:
                            pipeline.put(root / "captured-trace.json", pipeline.canonical(trace))
                    native.update_state(job, state, heartbeat_at=native.utc_now())
                    time.sleep(0.5)
            finally:
                native.stop_command_process(process)
        pipeline.require(screenshot is not None and process.returncode in (0, -15), "native bounded stop/capture failed")
        check_plan(value)
        pipeline.require(pipeline.digest(binary.read_bytes()) == binary_hash, "binary changed during run")
        result = {"schema_version": 1, "profile": PROFILE, "scope": SCOPE, "status": "pass",
            "plan": value, "nonce": nonce, "pid": process.pid, "termination": "bounded-stop",
            "exit_code": process.returncode, "screenshot": screenshot,
            "checkpoints": checkpoints,
            "renderer": check_renderer((root / "actual-window.log").read_text(), value["adapter"]),
            "trace_sha256": pipeline.digest((root / "probe.m2-trace.json").read_bytes()),
            "captured_trace_sha256": pipeline.digest((root / "captured-trace.json").read_bytes()),
            "log_sha256": pipeline.digest((root / "actual-window.log").read_bytes()),
            "build_log_sha256": pipeline.digest((root / "build.log").read_bytes()),
            "binary": {"path": str(binary), "sha256": binary_hash},
            "inventory": m2.inspect(Path(value["runtime_plan"]), value["kind"], root / "probe.m2-trace.json"),
            "accepted": False, "promotion_authority": False,
            "not_covered": ["input outcome verdict", "all lifecycle legs", "numeric freeze", "art approval",
                            "performance budget", "candidate approval", "formal release"]}
        pipeline.put(root / "manifest.json", pipeline.canonical(result))
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify", "status"))
    for name in ("runtime-plan", "job-root", "output", "plan"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--kind", choices=tuple(m2.KIND_LEGS))
    parser.add_argument("--leg")
    parser.add_argument("--adapter", help="Nonempty WGPU adapter name selector (case-insensitive containment); required when planning")
    parser.add_argument("--duration-seconds", type=int, default=160)
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.runtime_plan, args.output, args.kind, args.leg)), "incomplete plan inputs")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    elif args.command == "verify":
        result = verify(args.job_root)
    else:
        if native.read_json(args.job_root / "job.json").get("status") == "valid":
            verify(args.job_root)
        return native.status_command(argparse.Namespace(job_root=str(args.job_root), stale_after_secs=120))
    native.print_json(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
