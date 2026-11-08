"""Exact M2 seeded maximum-zoom tooltip feedback with owned EIS input.

One-time domain preparation is not natural construction, water delivery or task
assignment. This recipe never approves art/numbers, performance or a release.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys

if not __package__:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import building_art_acceptance as art
from scripts import building_m2_native_acceptance as interactive
import ui_usability_acceptance as ui

native, pipeline = art.native, art.pipeline
PROFILE = "building-m2-ui-art-feedback-v1"
SCOPE = "seeded-m2-ui-art-feedback-v1"
CASES = {"m2-ui-tank-empty": ("Tank", 0), "m2-ui-tank-partial": ("Tank", 25),
         "m2-ui-tank-full": ("Tank", 50), "m2-ui-mixer-refining": ("MudMixer", None)}


def context(value, case, identity):
    pipeline.require(case in CASES, "unknown M2 UI case")
    ctx = value.get("m2_ui")
    kind, amount = CASES[case]
    pipeline.require(isinstance(ctx, dict) and ctx.get("scope") == SCOPE and ctx.get("case") == case
                     and ctx.get("ready") is True and pipeline.exact_json(ctx.get("identity"), identity)
                     and pipeline.exact_json(ctx.get("active"), identity)
                     and identity.get("kind") == ctx.get("kind") == kind and identity.get("authority") == "art_preview"
                     and ctx.get("inventory_overflow") is False
                     and all(ctx.get(key) is False for key in ("natural_lifecycle_evidence", "performance_evidence", "promotion_authority")),
                     "M2 UI identity/scope/authority differs")
    owner, target = ctx.get("owner"), ctx.get("target")
    pipeline.require(type(owner) is int and type(target) is int
                     and ui.m5.finite(ctx.get("virtual_seconds")) and ctx["virtual_seconds"] >= 0, "M2 UI target/clock missing")
    if kind == "Tank":
        items, inverse = ctx.get("items"), ctx.get("stored_items")
        pipeline.require(target == owner and pipeline.exact_json(ctx.get("stockpile"), {"capacity": 50, "resource": "Some(Water)"})
                         and isinstance(items, list) and len(items) == amount and isinstance(inverse, list)
                         and all(type(row.get("entity")) is int and row.get("owner") == owner and row.get("resource") == "Water" for row in items)
                         and len({row["entity"] for row in items}) == amount
                         and sorted(row["entity"] for row in items) == inverse
                         and ctx.get("worker") is None and all(ctx.get(key) == [] for key in ("workers", "working_on", "assigned")),
                         "M2 UI actual Water/inverse differs")
    else:
        worker = ctx.get("worker")
        pipeline.require(isinstance(worker, dict) and worker.get("entity") == target != owner
                         and worker.get("soul") is True and type(worker.get("progress")) in (int, float) and worker.get("progress") == 0.
                         and worker.get("phase") == "Refining { progress: 0.0 }"
                         and worker.get("work_type") == "Refine"
                         and all(worker.get(key) == owner for key in ("mixer", "working_on", "assignment", "target"))
                         and all(ctx.get(key) == [target] for key in ("workers", "working_on", "assigned")),
                         "M2 UI actual Refining/worker identities differ")
    return ctx


def check_observation(case, value, identity):
    ctx = context(value, case, identity)
    pipeline.require(value.get("ready") is True and value.get("paused") is True
                     and value.get("viewport") == [1920., 1080.] and value.get("ui_scale") == 1.
                     and value.get("window_focused") is True and value.get("world_input_captured") is False
                     and value.get("pointer_over_ui") is False and value.get("menu_state") == "Hidden"
                     and all(type(value.get(key)) is int for key in ("pid", "frame", "world_epoch", "primary_window"))
                     and value.get("hovered") == value.get("pointer_target") == value.get("tooltip_target") == ctx["target"]
                     and value.get("tooltip_epoch") == value["world_epoch"] and value.get("wheel_events") == []
                     and value.get("wheel_overflow") is False, "M2 UI actual hover/foreground differs")
    pipeline.require(abs(ui.rest_area_maxzoom.camera_scale(value) - 5.) <= 1e-5, "M2 UI actual maximum camera differs")
    pipeline.require(value["camera_numeric"][0]["orthographic_scale"] == 1.
                     and type(value.get("tooltip_presentation", {}).get("entity")) is int
                     and value.get("tooltip_roots") == [value["tooltip_presentation"]["entity"]], "M2 UI projection/tooltip root differs")
    point = ctx.get("target_point")
    pipeline.require(isinstance(point, list) and len(point) == 2 and all(ui.m5.finite(axis) for axis in point)
                     and value.get("scale_factor") == 1. and value.get("cursor") is not None
                     and all(abs(a-b) <= 2 for a, b in zip(value["cursor"], point, strict=True)), "M2 UI cursor/projection differs")
    # Production tooltip fading follows Real Time even while the world is paused.
    # Wait for its real endpoint; never normalize alpha or relax exact interval checks.
    texts = value.get("tooltip_text", [])
    pipeline.require(texts and ui.m5.finite(value.get("tooltip_alpha")) and value["tooltip_alpha"] == 1.
                     and all(ui.m5.finite(row.get("alpha")) and row["alpha"] == 1. for row in texts),
                     "M2 UI tooltip fade has not settled")
    visible = [row["text"] for row in texts
               if ui.rest_area_maxzoom.visible_rect(row.get("rect"), value["viewport"])
               and row.get("root") == value["tooltip_presentation"]["entity"]
               and row.get("fully_visible") is True and row.get("primary_camera") is True
               and row["alpha"] == 1.]
    expected = f"Building: Tank: Water ({CASES[case][1]}/50)" if CASES[case][0] == "Tank" else "Task: Refine (Refining { progress: 0.0 })"
    pipeline.require(ui.m5.finite(value.get("tooltip_alpha")) and value["tooltip_alpha"] >= .95
                     and value.get("tooltip_presentation") is not None
                     and ui.rest_area_maxzoom.visible_rect(value["tooltip_presentation"].get("rect"), value["viewport"])
                     and any(expected in text.splitlines() for text in visible), "M2 UI actual visible state text differs")
    return ctx


def source_contract(repo, source, codec, case):
    pipeline.require(case in CASES, "unknown M2 UI case")
    source, codec = pipeline.no_symlinks(source), pipeline.no_symlinks(codec)
    kind = CASES[case][0]
    pipeline.verify_set(source, kind, codec)
    manifest, text, receipt = pipeline.load_set(source, kind, codec)
    pipeline.require(manifest["identity"]["authority"] == "art_preview" and receipt is None
                     and manifest["geometry_contract_sha256"] in art.M2_NUMERIC[kind], "M2 UI requires the exact preview/numeric source")
    return {"scope": SCOPE, "identity": manifest["identity"], "manifest_sha256": pipeline.digest(text.encode()),
            "source_root": str(source), "codec": str(codec), "codec_sha256": ui.sha256(codec),
            "geometry_sha256": manifest["geometry_contract_sha256"], "artifacts": manifest["artifacts"],
            "parts": manifest["parts"],
            "base_assets": art.asset_view_fingerprint(repo)}


def session_setting(value, root, nonce):
    return json.dumps({"mode": "feedback", "identity": value["input"]["identity"],
        "view": "normal", "variant": "gallery", "lighting": "standard",
        "locator": pipeline.locator(CASES[value["case"]][0]), "nonce": nonce, "status_path": str(root / "probe.json")})


def check_plan(value):
    repo = Path(value["repo"])
    pipeline.require(value["profile"] == PROFILE and value["scope"] == SCOPE and value["case"] in CASES
                     and value["status"] == "ready" and value["promotion_authority"] is False
                     and value["driver_sha256"] == ui.sha256(Path(__file__))
                     and pipeline.exact_json(art.subject(repo), value["subject"])
                     and pipeline.exact_json(source_contract(repo, Path(value["input"]["source_root"]),
                         Path(value["input"]["codec"]), value["case"]), value["input"]), "M2 UI frozen plan changed")
    art.static.check_root(repo, Path(value["job_root"]))
    return repo


def plan(args):
    repo = native.validate_repo(args.repo)
    root = args.job_root.absolute()
    art.static.check_root(repo, root)
    pipeline.require(not root.exists() and not args.output.absolute().is_relative_to(root), "M2 UI needs a fresh root and external plan")
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value = {"profile": PROFILE, "scope": SCOPE, "status": "ready", "repo": str(repo), "job_root": str(root),
             "case": args.case, "input": source_contract(repo, args.source_root, args.codec, args.case),
             "subject": art.subject(repo), "driver_sha256": ui.sha256(Path(__file__)),
             "portal_consent_file": str(args.portal_consent_file.absolute()), "promotion_authority": False,
             "resources": resources}
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
        "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()), "run", "--plan", str(args.output.absolute())]
    check_plan(value)
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def verify(root):
    result = native.read_json(root / "manifest.json")
    value = result["plan"]
    repo = check_plan(value)
    pipeline.require(Path(value["job_root"]) == root and result["profile"] == PROFILE
                     and result["binary_sha256"] == ui.sha256(repo / "target/debug/bevy_app"), "M2 UI result binding differs")
    directory = root / "viewport-0"
    pipeline.require(result["log_sha256"] == ui.sha256(directory / "game.log"), "M2 UI game log changed")
    ui.joint.verify_game_log(directory / "game.log", "Intel")
    manifest, text, _ = pipeline.load_set(Path(value["input"]["source_root"]), CASES[value["case"]][0], Path(value["input"]["codec"]))
    interactive.check_asset_view(repo, root / "asset-view", manifest, text)
    entries = native.read_json(directory / "checkpoints.json")["checkpoints"]
    events = native.read_json(directory / "events.json")["events"]
    client = native.read_json(directory / "input-client.json")
    pipeline.require(client.get("input_backend") == "portal-eis" and type(client.get("portal_devices")) is int
                     and client["portal_devices"] & 3 == 3 and client.get("portal_session"), "M2 UI authenticated input backend differs")
    ui.rest_area_art.check_eis_client(client, scroll=False)
    pipeline.require(len(entries) == 1 and result["checkpoints_sha256"] == ui.sha256(directory / "checkpoints.json")
                     and result["events_sha256"] == ui.sha256(directory / "events.json")
                     and result["input_sha256"] == ui.sha256(directory / "input-client.json"), "M2 UI input inventory changed")
    entry = entries[0]
    observed = entry["value"]
    check_observation(value["case"], observed, value["input"]["identity"])
    ui.m5.check_transport(events, client, observed["world_epoch"])
    pipeline.require(all(a["post_frame"] <= b["pre_frame"] for a, b in zip(events, events[1:])), "M2 UI input intervals overlap")
    pipeline.require(entry["case"] == value["case"] and entry["capture_scope"] == "x11-client-window"
                     and entry["pid"] == client["pid"] == observed["pid"] and entry["window"] == client["window"]
                     and client["nonce"] == observed["nonce"] == result["nonce"]
                     and entry["last_step"] == len(events) and len(events) in (1, 4)
                     and [event["step"] for event in events] == [str(index+1) for index in range(len(events))], "M2 UI owned input sequence differs")
    if len(events) == 4:
        pipeline.require(events[0].get("point") is not None
                         and [event.get("button") for event in events[1:3]] == [1, 1]
                         and [event.get("pressed") for event in events[1:3]] == [True, False]
                         and events[0].get("observed_hover", {}).get("dev-minimize") == "Hovered"
                         and events[1].get("observed_hover", {}).get("dev-minimize") == "Pressed", "M2 UI unexpected preparation input")
    motion = events[-1]
    pipeline.require(motion.get("point") is not None and motion.get("button") is None and motion.get("key") is None
                     and motion.get("scroll_discrete") is None
                     and all(abs(a-b) <= 2 for a, b in zip(motion["point"], observed["m2_ui"]["target_point"], strict=True)), "M2 UI fresh target motion missing")
    for event in events:
        pipeline.require(event["pid"] == client["pid"] and event["window"] == client["window"]
                         and event["nonce"] == client["nonce"] and event["world_epoch"] == observed["world_epoch"]
                         and event["pre_frame"] < event["post_frame"] < observed["frame"]
                         and event.get("key") is None and event.get("scroll_discrete") is None, "M2 UI foreign/unclaimed input")
    image = directory / entry["screenshot"]
    pipeline.require(image.parent == directory and ui.sha256(image) == entry["sha256"]
                     and list(native.validate_png_structure(image.read_bytes())) == observed["viewport"], "M2 UI owned image changed")
    before, after = result["capture_before"], result["capture_after"]
    for sample in (before, after):
        check_observation(value["case"], sample, value["input"]["identity"])
        pipeline.require(sample["m2_ui"] == observed["m2_ui"] and sample["world_epoch"] == observed["world_epoch"]
                         and sample["pid"] == observed["pid"] and sample["nonce"] == observed["nonce"], "M2 UI capture state changed")
        pipeline.require(all(pipeline.exact_json(sample.get(key), observed.get(key)) for key in
                             ("primary_window", "camera_numeric", "camera", "tooltip_roots", "tooltip_text", "tooltip_presentation")),
                         "M2 UI capture camera/tooltip identity changed")
    pipeline.require(before["frame"] < observed["frame"] < after["frame"], "M2 UI capture not bracketed")
    raw_path = root / "probe.m2-trace.json"
    pipeline.require(result["trace_sha256"] == ui.sha256(raw_path), "M2 UI Last trace changed")
    check_trace(native.read_json(raw_path), value, before, after)
    return {"status": "valid", "profile": PROFILE, "scope": SCOPE, "promotion_authority": False}


def check_trace(raw, plan_value, before, after):
    identity, case = plan_value["input"]["identity"], plan_value["case"]
    pipeline.require(raw.get("scope") == "m2-feedback-observations-only" and raw.get("leg") == "ui-art"
                     and raw.get("failure") is None and raw.get("accepted") is False
                     and raw.get("performance_evidence") is False and pipeline.exact_json(raw.get("identity"), identity)
                     and raw.get("numeric_candidate_sha256") == plan_value["input"]["geometry_sha256"]
                     and raw.get("nonce") == before["nonce"] and type(raw.get("process_id")) is int
                     and raw.get("process_id") == before["pid"], "M2 UI Last trace binding differs")
    rows = raw.get("samples")
    pipeline.require(isinstance(rows, list) and 2 <= len(rows) <= 1800, "M2 UI bounded Last trace missing")
    pipeline.require(all(ui.m5.finite(row.get("real_seconds")) for row in rows)
                     and all(a["real_seconds"] < b["real_seconds"] for a, b in zip(rows, rows[1:])), "M2 UI Last clock reversed")
    usable = [row for row in rows if isinstance(row.get("ui_art_observed"), dict)
              and type(row["ui_art_observed"].get("frame")) is int]
    older = [row for row in usable if row["ui_art_observed"]["frame"] <= before["frame"]]
    newer = [row for row in usable if row["ui_art_observed"]["frame"] >= after["frame"]]
    pipeline.require(older and newer, "M2 UI capture lacks Last flush brackets")
    first, last = older[-1], newer[0]
    selected = rows[rows.index(first):rows.index(last) + 1]
    pipeline.require(len(selected) >= 2, "M2 UI capture needs distinct Last samples")
    kind = CASES[case][0]
    expected_state = "MixerActive" if kind == "MudMixer" else "Tank" + {0: "Empty", 25: "Partial", 50: "Full"}[CASES[case][1]]
    stable_root = None
    stable_parts = None
    for row in selected:
        published = row["ui_art_observed"]
        check_observation(case, published, identity)
        pipeline.require(published["m2_ui"] == before["m2_ui"] and published["nonce"] == before["nonce"]
                         and published["pid"] == before["pid"] and published["world_epoch"] == before["world_epoch"]
                         and pipeline.exact_json(row["identity"], identity) and row["paused"] is True
                         and type(row.get("world_epoch")) is int and type(row.get("fixture_owner_bits")) is int
                         and ui.m5.finite(row.get("virtual_seconds")) and row["virtual_seconds"] >= 0
                         and row.get("fixture_owner_bits") == before["m2_ui"]["owner"]
                         and row["world_epoch"] == before["world_epoch"]
                         and row["virtual_seconds"] == before["m2_ui"]["virtual_seconds"], "M2 UI Last state drift")
        pipeline.require(all(pipeline.exact_json(published.get(key), before.get(key)) for key in
                             ("primary_window", "camera_numeric", "camera", "tooltip_roots", "tooltip_text", "tooltip_presentation")),
                         "M2 UI intermediate camera/tooltip changed")
        roots = [root for root in row["roots"] if root["owner"] == row["fixture_owner"]]
        pipeline.require(len(roots) == 1 and roots[0]["owner_exists"] is True
                         and roots[0]["state"] == f"Some({expected_state})", "M2 UI actual production root/state missing")
        root = roots[0]
        owners = [owner for owner in row.get("owners", []) if owner["entity"] == row["fixture_owner"]]
        pipeline.require(len(owners) == 1 and ui.m5.valid_pose(owners[0].get("pose")), "M2 UI actual owner pose missing")
        owner_pose = owners[0]["pose"]
        pipeline.require(owner_pose["rotation"] == [0., 0., 0., 1.] and owner_pose["scale"] == [1.] * 3,
                         "M2 UI unexpected owner rotation/scale")
        expected_root = {"translation": [owner_pose["translation"][0], 12.8, -owner_pose["translation"][1]],
                         "rotation": [0., 0., 0., 1.], "scale": [1.] * 3}
        pipeline.require(ui.m5.valid_pose(root.get("pose")) and root.get("parent") is None
                         and pipeline.exact_json(root.get("identity"), identity)
                         and all(abs(a-b) <= 1e-5 for key in expected_root for a, b in zip(root["pose"][key], expected_root[key], strict=True)),
                         "M2 UI root detached from actual owner")
        root_matrix = art.foreground.matrix(expected_root)
        pipeline.require(art.m5_context.close(root.get("global_matrix"), root_matrix), "M2 UI root global differs")
        if stable_root is None:
            stable_root = copy.deepcopy(root)
        pipeline.require(root["entity"] == stable_root["entity"] and root["pose"] == stable_root["pose"], "M2 UI root replaced/moved")
        expected_names = ("body", "water") if kind == "Tank" else ("body", "rotor")
        parts = roots[0]["parts"]
        if stable_parts is None:
            stable_parts = [(part["name"], part["entity"]) for part in parts]
        pipeline.require([(part["name"], part["entity"]) for part in parts] == stable_parts, "M2 UI parts replaced")
        pipeline.require(sorted(part["name"] for part in parts) == sorted(expected_names), "M2 UI actual part inventory differs")
        for part in parts:
            material = part.get("material")
            pipeline.require(part.get("mesh_resident") is True and part.get("layer_3d") is True
                             and part.get("mesh_handle") == row["descriptor_handles"]["meshes"][part["name"]]
                             and part.get("visible") is (not (case == "m2-ui-tank-empty" and part["name"] == "water"))
                             and isinstance(material, dict) and material.get("albedo") == row["descriptor_handles"]["albedo"]
                             and material.get("albedo_resident") is True
                             and isinstance(material.get("albedo"), str) and material.get("opaque") is True
                             and material.get("lit") is True, "M2 UI missing/foreign production mesh/material")
            source_part = next(item for item in plan_value["input"]["parts"] if item["name"] == part["name"])
            expected_pose = {"translation": list(source_part["translation_wu"]), "rotation": source_part["rotation_xyzw"], "scale": source_part["scale"]}
            if kind == "Tank" and part["name"] == "water":
                expected_pose["translation"][1] = 22. if CASES[case][1] == 50 else 12.
            pipeline.require(ui.m5.valid_pose(part.get("pose")) and all(
                abs(a-b) <= 1e-5 for key in expected_pose for a, b in zip(part["pose"][key], expected_pose[key], strict=True)),
                "M2 UI part pose differs from admitted source/state")
            pipeline.require(part.get("parent") == root["entity"] and art.m5_context.close(part.get("global_matrix"),
                art.foreground.product(root_matrix, art.foreground.matrix(part["pose"]))), "M2 UI part parent/global differs")
            expected_color = [.58, .70, .73, 1.] if kind == "Tank" and part["name"] == "water" and CASES[case][1] == 25 else [1.] * 4
            color = material.get("color")
            pipeline.require(isinstance(color, list) and len(color) == 4 and all(ui.m5.finite(axis) for axis in color)
                             and all(abs(a-b) <= 1e-5 for a, b in zip(color, expected_color, strict=True)), "M2 UI part color differs")


@native.activity_locked
def run(args):
    value = native.read_json(args.plan)
    repo = check_plan(value)
    root = Path(value["job_root"])
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1" and not root.exists(), "use the fresh registered launcher")
    root.mkdir()
    job = root / "job.json"
    state = {"status": "running", "profile": PROFILE, "heartbeat_at": native.utc_now(), "commands": []}
    native.atomic_write_json(job, state)
    process = driver = portal = None
    try:
        native.run_command("build", ["python3", "scripts/dev.py", "feedback", "--build-only"], repo=repo,
            env=native.cargo_environment(repo), log_path=root / "build.log", job_file=job, state=state, timeout_seconds=3600)
        check_plan(value)
        source = Path(value["input"]["source_root"])
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        for artifact in value["input"]["artifacts"]:
            pipeline.put(pipeline.rooted(view / "assets", artifact["path"]), pipeline.file_bytes(source, artifact))
        locator = pipeline.locator(CASES[value["case"]][0])
        pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
        directory = root / "viewport-0"
        directory.mkdir()
        nonce = secrets.token_hex(16)
        environment = ui.game_environment(repo)
        for key in tuple(environment):
            if key.startswith(("HW_NATIVE_", "HW_PERF_", "HW_WALL_", "HW_DOOR_", "HW_BUILDING_", "HW_M2_", "HW_BRIDGE_ACCEPTANCE_")):
                environment.pop(key, None)
        environment.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
            "WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": "Intel", "HELL_WORKERS_WORLDGEN_SEED": "20260914",
            "HW_NATIVE_UI_ROOT": str(directory), "HW_NATIVE_UI_NONCE": nonce, "HW_NATIVE_UI_CASE": value["case"],
            "HW_NATIVE_UI_WIDTH": "1920", "HW_NATIVE_UI_HEIGHT": "1080", "HW_NATIVE_UI_SCALE": "1",
            "HW_M2_ACCEPTANCE_PROBE": "1", "HW_M2_ACCEPTANCE_LEG": "ui-art",
            "HW_M2_ACCEPTANCE_REVISION": "v2" if value["input"]["geometry_sha256"] == art.M2_NUMERIC["MudMixer"][-1] else "v1",
            "HW_BUILDING_ART_SESSION": session_setting(value, root, nonce)})
        def heartbeat(method):
            state.update(current_stage=f"portal-{method}", heartbeat_at=native.utc_now())
            native.atomic_write_json(job, state)
        portal = ui.PortalSession(heartbeat, consent_file=value["portal_consent_file"])
        native.admit_stage_start("actual-window", state=state, job_file=job)
        with (directory / "game.log").open("w") as log:
            process = subprocess.Popen([str(repo / "target/debug/bevy_app")], cwd=repo, env=environment,
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True, pass_fds=native.activity_pass_fds(environment))
            state["child_pid"] = process.pid
            driver = ui.Driver(process, directory, nonce, state, job, portal, eis=True)
            driver.last = driver.wait(lambda observed: observed.get("m2_ui", {}).get("ready") is True, after=driver.last["frame"])
            if "text-field:DevPoc" in driver.last["controls"]:
                driver.click("dev-minimize")
            point = driver.last["m2_ui"]["target_point"]
            driver.send(point=tuple(round(axis) for axis in point))
            def ready(observed):
                try:
                    check_observation(value["case"], observed, value["input"]["identity"])
                except (ValueError, KeyError):
                    return False
                return True
            driver.last = driver.wait(ready, after=driver.last["frame"])
            driver.last = driver.wait(ready, after=driver.last["frame"] + 30)
            def flush(captured):
                def flushed(_):
                    trace = native.read_json(root / "probe.m2-trace.json")
                    pipeline.require(trace.get("failure") is None, "M2 UI passive trace failed")
                    return any(isinstance(sample.get("ui_art_observed"), dict)
                               and sample["ui_art_observed"].get("frame", -1) >= captured["frame"]
                               for sample in trace["samples"])
                driver.wait(flushed, after=driver.last["frame"], timeout=15)
            before = copy.deepcopy(driver.last)
            flush(before)
            driver.capture(value["case"], validate=lambda case, observed: check_observation(case, observed, value["input"]["identity"]))
            after = driver.wait(ready, after=driver.last["frame"] + 3)
            flush(after)
            native.atomic_write_json(root / "manifest.json", {"profile": PROFILE, "plan": value, "nonce": nonce,
                "binary_sha256": ui.sha256(repo / "target/debug/bevy_app"), "capture_before": before, "capture_after": after,
                "checkpoints_sha256": ui.sha256(directory / "checkpoints.json"), "events_sha256": ui.sha256(directory / "events.json"),
                "input_sha256": ui.sha256(directory / "input-client.json")})
        driver.input.close()
        driver.input = None
        native.stop_command_process(process)
        process = None
        result = native.read_json(root / "manifest.json")
        result["log_sha256"] = ui.sha256(directory / "game.log")
        result["trace_sha256"] = ui.sha256(root / "probe.m2-trace.json")
        native.atomic_write_json(root / "manifest.json", result)
        verify(root)
        state.update(status="valid", completed_at=native.utc_now())
    except Exception as error:
        state.update(status="invalid", failure=f"{type(error).__name__}: {error}", completed_at=native.utc_now())
        raise
    finally:
        if driver and driver.input:
            driver.input.close()
        if process:
            native.stop_command_process(process)
        if portal:
            portal.close()
        native.atomic_write_json(job, state)
    return state


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify", "status"))
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--codec", type=Path)
    parser.add_argument("--case", choices=CASES)
    parser.add_argument("--job-root", type=Path)
    parser.add_argument("--portal-consent-file", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--plan", type=Path)
    args = parser.parse_args()
    if args.command == "plan":
        result = plan(args)
    elif args.command == "run":
        value = native.read_json(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    elif args.command == "verify":
        result = verify(args.job_root)
    else:
        result = native.read_json(args.job_root / "job.json")
    native.print_json(result)
    return 2 if result.get("status") == "running" else int(result.get("status") not in ("ready", "valid"))


if __name__ == "__main__":
    raise SystemExit(main())
