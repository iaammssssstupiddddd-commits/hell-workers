"""Ordinary Tank construction only; no fixture, refinement or release claim."""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
from scripts import ui_m2_planning as planning
from scripts import ui_spa_candidate_art as spa

native, pipeline = planning.native, planning.pipeline
CASE = "m2-tank-candidate-completion"
SCOPE = "ordinary-m2-tank-candidate-completion-v1"
INFLOW_CASE = "m2-tank-candidate-inflow"
INFLOW_SCOPE = "ordinary-m2-tank-candidate-inflow-v1"
FULL_CASE = "m2-tank-candidate-full"
FULL_SCOPE = "ordinary-m2-tank-candidate-full-v1"
SUPPLY_CASE = "m2-tank-candidate-supply"
SUPPLY_SCOPE = "ordinary-m2-tank-candidate-supply-v5"
PROFILES = ((CASE, SCOPE), (INFLOW_CASE, INFLOW_SCOPE), (FULL_CASE, FULL_SCOPE), (SUPPLY_CASE, SUPPLY_SCOPE))
CHECKPOINTS = tuple("m2-tank-completion-" + phase for phase in ("empty", "placed", "completed"))


def source_contract(repo, source, codec, *, profile=(CASE, SCOPE)):
    native.require(profile in PROFILES, "Unknown Tank observation profile")
    source, codec = pipeline.no_symlinks(source), pipeline.no_symlinks(codec)
    pipeline.verify_set(source, "Tank", codec)
    manifest, text, receipt = pipeline.load_set(source, "Tank", codec)
    native.require(manifest["identity"]["authority"] == "isolated_candidate" and receipt is None,
                   "Tank completion requires an adopted isolated candidate")
    pipeline.check_numeric_provenance(source, manifest, codec)
    return dict(case=profile[0], scope=profile[1], source_root=str(source), codec=str(codec),
                codec_sha256=pipeline.digest(codec.read_bytes()), identity=manifest["identity"],
                manifest_sha256=pipeline.digest(text.encode()), artifacts=manifest["artifacts"],
                parts=manifest["parts"], world=manifest["world_preview"], catalog=manifest["catalog_preview"],
                production_state=manifest["production_state"], art_approval_sha256=manifest["art_approval_sha256"],
                numeric_approval_sha256=manifest["numeric_approval_sha256"],
                numeric_verifiers=pipeline.numeric_verifier_inventory(),
                base_assets_sha256=planning.art.asset_view_fingerprint(repo),
                verifier_sha256=pipeline.digest(Path(__file__).read_bytes()),
                **(dict(supply_speed="Super") if profile in ((FULL_CASE, FULL_SCOPE), (SUPPLY_CASE, SUPPLY_SCOPE)) else {}))


def planned_contract(args, repo):
    native.require(args.case == CASE and args.smoke and args.input_backend == "portal-eis"
                   and args.m2_art_source and args.art_codec and not args.layout_menu
                   and not any(getattr(args, key, None) for key in
                               ("m5_art_source", "rest_area_art_source", "soul_spa_art_source", "spa_resume_source")),
                   "Tank completion requires its dedicated candidate EIS case")
    contract = source_contract(repo, Path(args.m2_art_source), Path(args.art_codec))
    if getattr(args, "art_contract_sha256", None):
        native.require(planning.art_contract_digest(contract) == args.art_contract_sha256, "Tank completion source changed")
    return contract


def prepare_view(repo, directory, contract):
    import shutil
    import ui_rest_area_art as shared
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"]),
                                                     profile=(contract["case"], contract["scope"])), contract),
                   "Tank completion source changed")
    view = shared.checked_view(repo, directory / "asset-view", prepared=False)
    native.require(not view.exists(), "Tank asset view must be fresh")
    shutil.copytree(repo / "assets", view / "assets")
    native.require(planning.art.asset_view_fingerprint(view) == contract["base_assets_sha256"], "Tank base assets changed")
    source = Path(contract["source_root"])
    for entry in contract["artifacts"]:
        pipeline.put(pipeline.rooted(view / "assets", entry["path"]), pipeline.file_bytes(source, entry))
    locator = pipeline.locator("Tank")
    pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
    return view


check_view = planning.check_view


def session_setting(directory, nonce, contract):
    native.require((contract["case"], contract["scope"]) in PROFILES, "Tank session profile differs")
    return json.dumps({"mode": contract["case"], "identity": contract["identity"],
                       "locator": pipeline.locator("Tank"), "nonce": nonce,
                       "output": str(directory / "production-cancellation.json")})


def context(value):
    ctx = value.get("m2_planning")
    native.require(isinstance(ctx, dict) and ctx.get("failure") is None and ctx.get("inventory_overflow") is False
                   and ctx.get("kind") == "Tank" and ctx.get("initial_building_inventory") == []
                   and type(ctx.get("world_epoch")) is int and ctx["world_epoch"] == value.get("world_epoch")
                   and type(ctx.get("primary_window")) is int and ctx["primary_window"] == value.get("primary_window")
                   and ctx.get("camera", {}).get("primary") is True
                   and ctx["camera"].get("viewport") == value.get("viewport"), "Tank completion context differs")
    anchor = ctx.get("anchor")
    native.require(isinstance(anchor, list) and len(anchor) == 2 and all(type(axis) is int for axis in anchor),
                   "Tank completion anchor differs")
    x, y = anchor
    footprint = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]]
    native.require(pipeline.exact_json(ctx.get("footprint"), footprint)
                   and [row.get("grid") for row in ctx.get("cells", [])] == footprint, "Tank completion footprint differs")
    return ctx


def check_observation(case, value):
    native.require(case in CHECKPOINTS and value.get("paused") is True, "Tank completion checkpoint/pause differs")
    ctx = context(value)
    if case == CHECKPOINTS[0]:
        native.require(ctx["blueprints"] == [] and ctx["buildings"] == [] and ctx["building_inventory"] == []
                       and ctx["legal"] is True and all(row["owner"] is None and row["walkable"] is True for row in ctx["cells"]),
                       "Tank completion must start with an actually empty legal footprint")
    elif case == CHECKPOINTS[1]:
        native.require(ctx["buildings"] == [] and len(ctx["blueprints"]) == 1 and ctx["legal"] is False
                       and all(row["owner"] == ctx["blueprints"][0]["entity"] for row in ctx["cells"]),
                       "Tank completion placed Blueprint differs")
    else:
        native.require(ctx["blueprints"] == [] and len(ctx["buildings"]) == len(ctx["building_inventory"]) == 1
                       and ctx["legal"] is False and all(row["owner"] == ctx["buildings"][0] and row["walkable"] is False for row in ctx["cells"]),
                       "Tank completion owner/blocked footprint differs")


def check_world_action(value, key, baseline):
    ctx = context(value)
    native.require(key in ("placement", "companion-placement")
                   and all(pipeline.exact_json(ctx[k], baseline[k]) for k in ("anchor", "footprint", "draw_pos", "primary_window", "world_epoch"))
                   and ctx.get("play_mode") == "BuildingPlace" and ctx.get("build_kind") == "Tank"
                   and value.get("pointer_over_ui") is False and value.get("world_input_captured") is False
                   and ctx["legal"] is True and ctx["blueprints"] == [] and ctx["buildings"] == []
                   and ctx.get("companion_legal") is True, "Tank world placement context changed; no press")
    x, y = baseline["anchor"]
    if key == "placement":
        native.require(ctx.get("cursor_grid") == [x, y] and ctx.get("companion_pending") is None,
                       "Tank first click must precede companion pending")
    else:
        pending = ctx.get("companion_pending", {})
        native.require(ctx.get("cursor_grid") == [x + 2, y] and ctx.get("companion_grid") == [x + 2, y]
                       and pipeline.exact_json(pending, dict(parent_anchor=[x, y], parent_kind="Tank", kind="BucketStorage",
                                           center=ctx["draw_pos"], radius=160., required=True)),
                       "Tank companion pending/target differs")


def check_companions(raw, entries, pending, committed, complete, inputs):
    from scripts import building_production_lifecycle as lifecycle
    samples = raw["samples"]
    lifecycle.check_placement_receipt(pending, samples)
    anchor = context(entries[0]["value"])["anchor"]
    native.require(pipeline.exact_json(pending["outcome"], dict(kind="Tank", grid=anchor, status="CompanionPending"))
                   and pending["sample_index"] < committed["sample_index"], "Tank first click did not remain pending")
    actions = raw.get("events", [])
    first_action = next(a["tank_completion_action"] for a in inputs
                        if a.get("tank_completion_action", {}).get("key") == "placement")
    presses = [event for event in actions if event.get("producer") == "MouseButtonInput" and event.get("state") == "Pressed"
               and context(first_action["before"])["real_seconds"] < event.get("real_seconds", -1)
               <= context(first_action["after"])["real_seconds"]]
    native.require(len(presses) == 1 and pending["sample_index"] >= presses[0]["sample_index"]
                   and presses[0]["real_seconds"] <= pending["real_seconds"] <= context(first_action["after"])["real_seconds"],
                   "Tank pending receipt lacks first click")
    children = committed["outcome"].get("companion_owners")
    native.require(isinstance(children, list) and len(children) == len(set(children)) == 2
                   and all(isinstance(child, str) and child for child in children), "Tank requires two distinct companion owners")
    initial = samples[0].get("companion_context", {})
    native.require(initial.get("available") is True and initial.get("pending") is None
                   and isinstance(initial.get("storages"), list), "Tank initial companion inventory missing")
    old = {row["entity"]: row for row in initial["storages"]}
    native.require(not set(children).intersection(old), "Tank companions reused old entities")
    for sample in samples:
        ctx = sample.get("companion_context", {})
        rows = ctx.get("storages", [])
        by_id = {row["entity"]: row for row in rows}
        index = sample["sample_index"]
        native.require(ctx.get("available") is True and len(by_id) == len(rows)
                       and all(pipeline.exact_json(by_id.get(entity), row) for entity, row in old.items()), "Tank changed initial companions")
        if index < committed["sample_index"]:
            native.require(by_id == old, "Tank companions created before second click")
            if pending["sample_index"] <= index:
                state = ctx.get("pending", {})
                native.require(pipeline.exact_json(state, dict(parent_kind="Tank", parent_anchor=anchor, kind="BucketStorage",
                                             center=context(entries[0]["value"])["draw_pos"], radius=160., required=True)),
                               "Tank pending state changed before commit")
            continue
        native.require(set(by_id) == set(old) | set(children) and ctx.get("pending") is None,
                       "Tank committed companions missing/replaced")
        done = index >= complete["sample_index"]
        typed = sample.get("m2_context", {})
        typed_rows = typed.get("bucket_storages", [])
        typed_by_id = {row["entity"]: row for row in typed_rows}
        native.require(len(typed_by_id) == len(typed_rows), "Tank duplicate typed storage")
        for offset, child in enumerate(children):
            row = by_id[child]
            grid = [anchor[0] + 2 + offset, anchor[1]]
            # Same map constants and grid-center transform as WorldMap::grid_to_world.
            position = [(grid[0] - 49.5) * 32., (grid[1] - 49.5) * 32., .01]
            actual = row.get("position")
            native.require(isinstance(actual, list) and len(actual) == 3
                           and all(planning.finite(a) and abs(a-b) < 1e-5 for a, b in zip(actual, position, strict=True))
                           and pipeline.exact_json(row.get("grid"), grid) and pipeline.exact_json(row.get("stockpile_grids"), [grid])
                           and row.get("pending_owner") == (None if done else committed["outcome"]["owner"])
                           and row.get("owner") == (complete["owner"] if done else None), "Tank companion ownership promotion differs")
            child_row = typed_by_id.get(child, {})
            native.require(pipeline.exact_json(child_row.get("stockpile"), dict(resource=None, capacity=10))
                           and child_row.get("belongs_to") == row["owner"]
                           and child_row.get("pending_blueprint") == row["pending_owner"]
                           and pipeline.exact_json(child_row.get("pose", {}).get("translation"), actual),
                           "Tank typed companion pose/stockpile/ownership differs")
            native.require("stored_items" in child_row and (child_row["stored_items"] is None or isinstance(child_row["stored_items"], list)),
                           "Tank companion StoredItems missing")
            inverse = child_row["stored_items"] or []
            stored = [item["entity"] for item in typed.get("items", []) if item.get("stored_in") == child]
            native.require(len(inverse) == len(set(inverse)) and sorted(inverse) == sorted(stored),
                           "Tank companion StoredItems inverse differs")
        if index == complete["sample_index"]:
            typed = sample.get("m2_context", {})
            tanks = [row for row in typed.get("tanks", []) if row.get("entity") == complete["owner"]]
            buckets = [row for row in typed.get("items", []) if row.get("belongs_to") == complete["owner"]]
            native.require(len(tanks) == 1 and tanks[0].get("provisional") is False
                           and pipeline.exact_json(tanks[0].get("stockpile"), dict(resource="Water", capacity=50))
                           and len(buckets) == 5 and len({row.get("entity") for row in buckets}) == 5
                           and all(row.get("resource") == "BucketEmpty" and row.get("stored_in") in children
                                   and all(key in row and row[key] is None for key in ("loaded_in", "delivering_to", "stored_by_mixer"))
                                   for row in buckets)
                           and set().union(*(set(typed_by_id[child]["stored_items"] or []) for child in children)) == {row["entity"] for row in buckets},
                           "Tank completed inventory lacks five dedicated buckets")


def capture_action(driver, key, *, baseline=None):
    world = key in ("placement", "companion-placement")
    if not world and key != "Escape":
        driver.send(point=driver.point(key))
        native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered",
                       "Tank completion target not hovered; no press")
    before, first = copy.deepcopy(driver.last), len(driver.events)
    if world:
        check_world_action(before, key, baseline)
    if key == "Escape":
        driver.key(key)
    else:
        driver.send(button=1, pressed=True)
        if not world:
            native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed",
                           "Tank completion control press missing")
        driver.send(button=1, pressed=False)
    def flushed(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "Tank completion raw flush failed")
        rows = [row for row in raw.get("events", []) if row.get("producer") == ("KeyboardInput" if key == "Escape" else "MouseButtonInput")
                and context(before)["real_seconds"] < row.get("real_seconds", -1) <= context(value)["real_seconds"]]
        return len(rows) == 2 and [row.get("state") for row in rows] == ["Pressed", "Released"]
    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["tank_completion_action"] = dict(key=key, first_step=first + 1, last_step=len(driver.events),
                                                      before=before, after=copy.deepcopy(driver.last))
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise(driver, contract):
    baseline = copy.deepcopy(context(driver.last))
    if "text-field:DevPoc" in driver.last["controls"]:
        capture_action(driver, "dev-minimize")
    driver.capture(CHECKPOINTS[0])
    capture_action(driver, "menu:SetTimeSpeed(Normal)")
    capture_action(driver, "menu:ToggleArchitect")
    capture_action(driver, "menu:SelectBuild(Tank)")
    point = context(driver.last)["point"]
    native.require(isinstance(point, list) and len(point) == 2 and all(planning.finite(axis) and 0 < axis < limit
                   for axis, limit in zip(point, driver.last["viewport"], strict=True)), "Tank target outside window")
    driver.send(point=tuple(round(axis * driver.last["scale_factor"]) for axis in point))
    capture_action(driver, "placement", baseline=baseline)
    driver.last = driver.wait(lambda value: context(value).get("companion_pending") is not None, after=driver.last["frame"])
    point = context(driver.last)["companion_point"]
    native.require(isinstance(point, list) and len(point) == 2 and all(planning.finite(axis) and 0 < axis < limit
                   for axis, limit in zip(point, driver.last["viewport"], strict=True)), "Tank companion target outside client")
    driver.send(point=tuple(round(axis * driver.last["scale_factor"]) for axis in point))
    capture_action(driver, "companion-placement", baseline=baseline)
    driver.last = driver.wait(lambda value: len(context(value)["blueprints"]) == 1, after=driver.last["frame"])
    capture_action(driver, "Escape")
    capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[1])
    capture_action(driver, "menu:SetTimeSpeed(Normal)")
    def completed(value):
        ctx = context(value)
        return len(ctx["buildings"]) == 1 and not ctx["blueprints"] and len(ctx.get("consumers", {}).get("roots", [])) == 1
    driver.last = driver.wait(completed, after=driver.last["frame"], timeout=120)
    capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[2])


def check_completion(raw, entries, client, *, raw_scope=SCOPE):
    """Actual domain completion plus delivery/task/owner witnesses, not release."""
    from scripts import building_production_lifecycle as lifecycle
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS),
                   "completion checkpoints differ")
    for entry in entries:
        check_observation(entry["case"], entry["value"])
    empty, placed, completed = [entry["value"] for entry in entries]
    native.require(all(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and value.get("paused") is True for value in (empty, placed, completed)),
                   "completion checkpoint process/pause differs")
    footprint = placed["m2_planning"]["footprint"]
    native.require(all(value["world_epoch"] == placed["world_epoch"]
                       and value["m2_planning"]["anchor"] == empty["m2_planning"]["anchor"]
                       and value["m2_planning"]["footprint"] == footprint
                       and value["m2_planning"]["draw_pos"] == empty["m2_planning"]["draw_pos"]
                       for value in (empty, completed))
                   and all(after["value"]["frame"] > before["value"]["frame"]
                           and after["last_step"] > before["last_step"]
                           for before, after in zip(entries, entries[1:])), "completion target/epoch/order differs")
    native.require(raw_scope in (SCOPE, INFLOW_SCOPE, FULL_SCOPE, SUPPLY_SCOPE) and raw.get("scope") == raw_scope
                   and raw.get("schema_version") == 2 and raw.get("failure") is None
                   and raw.get("nonce") == client["nonce"] and raw.get("process_id") == client["pid"]
                   and all(raw.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority")),
                   "completion raw context/authority differs")
    samples, events = raw["samples"], raw["events"]
    native.require(0 < len(samples) <= 1200 and len(events) < 4096
                   and all(type(sample.get("sample_index")) is int and sample["sample_index"] == index
                           and type(sample.get("real_seconds")) in (int, float)
                           and math.isfinite(sample["real_seconds"]) and sample["real_seconds"] >= 0
                           for index, sample in enumerate(samples))
                   and all(before["real_seconds"] < after["real_seconds"] for before, after in zip(samples, samples[1:]))
                   and all(type(event.get("event_index")) is int and event["event_index"] == index
                           for index, event in enumerate(events)),
                   "completion raw ordinals differ")
    rows = [event for event in events if event.get("operation") == "BuildingCompleted"]
    native.require(len(rows) == 1, "actual completion domain event missing or duplicated")
    row = rows[0]
    index = row.get("sample_index")
    native.require(row.get("producer") == "hw_jobs::publish_building_completed"
                   and row.get("kind") == "Tank" and row.get("footprint") == footprint
                   and isinstance(row.get("owner"), str) and isinstance(row.get("blueprint_owner"), str)
                   and row["owner"] != row["blueprint_owner"]
                   and type(index) is int and 0 < index < len(samples), "completion event binding differs")
    sample = samples[index]
    epoch = placed["world_epoch"]
    native.require(type(epoch) is int and epoch >= 0 and type(sample.get("world_epoch")) is int
                   and sample["world_epoch"] == epoch
                   and type(row.get("real_seconds")) in (int, float) and math.isfinite(row["real_seconds"])
                   and 0 <= row["real_seconds"] <= sample["real_seconds"], "completion time/epoch differs")
    prior = [bp for value in samples[:index] if value.get("world_epoch") == epoch
             for bp in value.get("blueprints", []) if bp.get("entity") == row["blueprint_owner"]]
    old_bits = placed["m2_planning"]["blueprints"][0]["entity"]
    native.require(prior and all(bp.get("entity_bits") == old_bits and bp.get("kind") == "Tank"
                                and bp.get("footprint") == footprint for bp in prior)
                   and any(bp.get("materials_complete") is True
                           and bp.get("delivered_materials") == {"Wood": 2} for bp in prior)
                   and not any(bp.get("entity") == row["blueprint_owner"] for bp in sample.get("blueprints", [])),
                   "completion lacks real complete-material Blueprint history")
    owners = [owner for owner in sample.get("owners", []) if owner.get("entity") == row["owner"]]
    native.require(len(owners) == 1 and owners[0].get("kind") == "Tank"
                   and owners[0].get("provisional") is False
                   and owners[0].get("entity_bits") == completed["m2_planning"]["buildings"][0],
                   "completion new owner differs")
    for entity, exists in ((row["blueprint_owner"], False), (row["owner"], True)):
        layers = [layer for layer in sample.get("owner_layers", []) if layer.get("entity") == entity]
        native.require(len(layers) == 1 and layers[0].get("exists") is exists
                       and sorted(layers[0].get("building_grids", [])) == (sorted(footprint) if exists else [])
                       and all(layers[0].get(key) == [] for key in (
                           "floor_grids", "door_grids", "bridge_grids", "stockpile_grids")),
                       "completion ownership transfer differs")
    tasks = [event for event in events if event.get("producer") == "TaskCompletedVisualMessage"
             and event.get("operation") == "TaskCompleted" and event.get("work_type") == "Build"
             and event.get("target") == row["blueprint_owner"]]
    native.require(len(tasks) == 1 and type(tasks[0].get("sample_index")) is int
                   and 0 <= tasks[0]["sample_index"] <= index
                   and isinstance(tasks[0].get("assignment"), str) and tasks[0]["assignment"]
                   and type(tasks[0].get("real_seconds")) in (int, float)
                   and math.isfinite(tasks[0]["real_seconds"])
                   and 0 <= tasks[0]["real_seconds"]
                   and tasks[0]["real_seconds"] <= samples[tasks[0]["sample_index"]]["real_seconds"]
                   and samples[tasks[0]["sample_index"]].get("world_epoch") == epoch
                   and any(actor.get("entity") == tasks[0].get("worker")
                           for actor in samples[tasks[0]["sample_index"]].get("actors", [])),
                   "real Build task completion/actor missing")
    deliveries = [event for event in events if event.get("producer") == "ordinary-blueprint-delivery-branch-v1"
                  and event.get("owner") == row["blueprint_owner"]]
    native.require(len(deliveries) == 2 and len({event.get("item") for event in deliveries}) == 2,
                   "completion needs two distinct real Wood deliveries")
    for count, event in enumerate(deliveries):
        lifecycle.check_delivery_receipt(event, samples)
        native.require(event["world_epoch"] == epoch and event["kind"] == "Tank"
                       and event["resource"] == "Wood" and event["before_count"] == count
                       and event["after_count"] == count + 1 and event["sample_index"] <= index,
                       "completion material sequence differs")
    task, last = tasks[0], deliveries[-1]
    native.require(last["sample_index"] <= task["sample_index"]
                   and last["real_seconds"] <= task["real_seconds"]
                   and last["event_index"] < task["event_index"], "Build completion precedes material delivery")
    if task["sample_index"] == index:
        # The domain callback runs after Execute; its raw row is appended now.
        # Task visual messages are drained later in Last, not at publication.
        native.require(row["event_index"] < task["event_index"]
                       and row["real_seconds"] <= task["real_seconds"], "same-sample completion producer order differs")
    else:
        native.require(task["event_index"] < row["event_index"]
                       and task["real_seconds"] <= row["real_seconds"], "Build/domain completion time reversed")
    native.require(any(actor.get("entity") == task["worker"]
                       and actor.get("task_identity") == {"assignment": task["assignment"],
                           "target": row["blueprint_owner"], "work_type": "Build"}
                       for value in samples[:task["sample_index"]] if value.get("world_epoch") == epoch
                       for actor in value.get("actors", [])), "Build completion has no exact prior active assignment")

def verify(raw, entries, client, inputs, contract, *, profile=(CASE, SCOPE)):
    from scripts import building_production_lifecycle as lifecycle
    native.require(profile in PROFILES and contract.get("case") == profile[0] and contract.get("scope") == profile[1]
                   and contract["identity"].get("kind") == "Tank" and contract["identity"].get("authority") == "isolated_candidate",
                   "Tank completion candidate contract differs")
    check_completion(raw, entries, client, raw_scope=profile[1])
    lifecycle.check_raw_transitions(raw)
    epoch, window = entries[0]["value"]["world_epoch"], entries[0]["value"]["primary_window"]
    samples = raw["samples"]
    native.require(all(type(sample.get("world_epoch")) is int and sample["world_epoch"] == epoch
                       and type(sample.get("primary_window_bits")) is int and sample["primary_window_bits"] == window
                       and planning.finite(sample.get("virtual_seconds")) and sample["virtual_seconds"] >= 0 for sample in samples)
                   and all(a["virtual_seconds"] <= b["virtual_seconds"] for a, b in zip(samples, samples[1:])),
                   "Tank completion intermediate identity/virtual clock differs")
    for entry in entries:
        value, ctx = entry["value"], context(entry["value"])
        native.require(all(type(value.get(key)) is int for key in ("pid", "world_epoch", "primary_window", "frame"))
                       and value["pid"] == client["pid"] and value["world_epoch"] == epoch and value["primary_window"] == window
                       and planning.finite(ctx.get("real_seconds")) and planning.finite(ctx.get("virtual_seconds")),
                       "Tank completion checkpoint owner/clock differs")
        near = [sample for sample in samples if abs(sample["real_seconds"] - ctx["real_seconds"]) <= 1
                and sample["virtual_seconds"] == ctx["virtual_seconds"] and sample.get("paused") is True]
        native.require(near, "Tank completion checkpoint lacks same paused raw clock")
    planning.check_transport(inputs, client, epoch)
    actions = [row["tank_completion_action"] for row in inputs if "tank_completion_action" in row]
    expected = ["menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", "menu:SelectBuild(Tank)", "placement", "companion-placement",
                "Escape", "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"]
    native.require([action.get("key") for action in actions] in (expected, ["dev-minimize", *expected]),
                   "Tank completion input sequence differs")
    claimed, raw_claimed = set(), set()
    baseline = context(entries[0]["value"])
    placement, time_effects = None, []
    for action in actions:
        before, after, key = action["before"], action["after"], action["key"]
        first, last = action["first_step"], action["last_step"]
        native.require(type(first) is int and type(last) is int and last == first + 1 and 1 <= first < last <= len(inputs)
                       and not claimed.intersection({first, last}) and inputs[first - 1].get("tank_completion_action") == action,
                       "Tank completion input pair missing/reused")
        claimed.update((first, last))
        start, end = context(before)["real_seconds"], context(after)["real_seconds"]
        native.require(planning.finite(start) and planning.finite(end) and start < end and before["frame"] < after["frame"]
                       and all(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                               and value.get("world_epoch") == epoch and value.get("primary_window") == window
                               for value in (before, after)), "Tank completion action context differs")
        sent = inputs[first - 1:last]
        keyboard = key == "Escape"
        native.require([row.get("pressed") for row in sent] == [True, False]
                       and all(type(row.get("pressed")) is bool and before["frame"] <= row["pre_frame"] < row["post_frame"] <= after["frame"]
                               and (row.get("key") == "Escape" if keyboard else type(row.get("button")) is int and row["button"] == 1)
                               for row in sent), "Tank completion sent input differs")
        rows = [row for row in raw["events"] if row.get("producer") == ("KeyboardInput" if keyboard else "MouseButtonInput")
                and start < row.get("real_seconds", -1) <= end]
        native.require(len(rows) == 2 and [row.get("state") for row in rows] == ["Pressed", "Released"], "Tank completion raw input pair differs")
        for row in rows:
            index = row.get("sample_index")
            native.require(type(index) is int and 0 <= index < len(samples) and row["event_index"] not in raw_claimed
                           and row.get("window") == samples[index].get("primary_window")
                           and samples[index]["real_seconds"] >= row["real_seconds"]
                           and (row.get("key") == "Escape" and row.get("repeat") is False if keyboard else row.get("button") == "Left"),
                           "Tank completion raw input owner differs")
            raw_claimed.add(row["event_index"])
            if not keyboard:
                cursor, dpi = samples[index].get("cursor_position"), samples[index].get("dpi")
                native.require(isinstance(cursor, list) and len(cursor) == 2 and all(planning.finite(axis) for axis in cursor)
                               and planning.finite(dpi) and dpi == before.get("scale_factor") and dpi > 0
                               and all(abs(a * dpi - b) <= 2 for a, b in zip(cursor, before["cursor"], strict=True)),
                               "Tank completion raw cursor differs")
        if key in ("placement", "companion-placement"):
            check_world_action(before, key, baseline)
            placement = action if key == "companion-placement" else placement
        elif not keyboard:
            target = before["controls"].get(key, {})
            native.require(target.get("fully_visible") is True and target.get("interaction") == "Hovered"
                           and sent[0].get("observed_hover", {}).get(key) == "Pressed", "Tank completion menu target differs")
        if key in ("menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"):
            paused = key.endswith("(Paused)")
            native.require(after.get("paused") is paused, "Tank time control did not take effect")
            time_effects.append((rows[-1]["sample_index"], paused))
    native.require(all(index + 1 in claimed for index, row in enumerate(inputs) if row.get("key") is not None or row.get("button") is not None)
                   and raw_claimed == {row["event_index"] for row in raw["events"] if row.get("producer") in ("KeyboardInput", "MouseButtonInput")},
                   "Tank completion input unclaimed")
    for lower, upper, expected_group in ((entries[0], entries[1], expected[:7]), (entries[1], entries[2], expected[7:])):
        group = [action for action in actions if lower["last_step"] < action["first_step"] <= upper["last_step"]]
        native.require([action["key"] for action in group] == expected_group
                       and all(action["last_step"] <= upper["last_step"] and lower["value"]["frame"] <= action["before"]["frame"]
                               < action["after"]["frame"] < upper["value"]["frame"]
                               and context(lower["value"])["real_seconds"] <= context(action["before"])["real_seconds"]
                               < context(action["after"])["real_seconds"] < context(upper["value"])["real_seconds"] for action in group),
                       "Tank completion action crossed checkpoint interval")
    start, end = context(entries[0]["value"])["real_seconds"], context(entries[-1]["value"])["real_seconds"]
    tail = [sample for sample in samples if start <= sample["real_seconds"] <= end]
    native.require(tail and context(entries[-1]["value"])["virtual_seconds"] > context(entries[1]["value"])["virtual_seconds"],
                   "Tank construction has no real simulation advance")
    for sample in tail:
        expected_paused = next((paused for index, paused in reversed(time_effects) if index <= sample["sample_index"]), True)
        native.require(type(sample.get("paused")) is bool and sample["paused"] is expected_paused
                       and planning.finite(sample.get("relative_speed")) and sample["relative_speed"] == 1,
                       "Tank actual time control phase/speed differs")
    native.require(all(a["virtual_seconds"] == b["virtual_seconds"] for a, b in zip(tail, tail[1:])
                       if a["paused"] is True and b["paused"] is True), "Tank paused simulation advanced")
    receipts = [row for row in raw["events"] if row.get("producer") == "ordinary-building-placement-branch-v1"]
    native.require(len(receipts) == 2 and placement is not None, "Tank completion placement receipt missing/duplicate")
    pending, receipt = receipts
    lifecycle.check_placement_receipt(receipt, samples)
    blueprint = context(entries[1]["value"])["blueprints"][0]
    presses = [row for row in raw["events"] if row.get("producer") == "MouseButtonInput" and row.get("state") == "Pressed"
               and context(placement["before"])["real_seconds"] < row.get("real_seconds", -1) <= context(placement["after"])["real_seconds"]]
    native.require(receipt["outcome"] == dict(kind="Tank", grid=[baseline["anchor"][0] + 2, baseline["anchor"][1]], status="CompanionCommitted",
                                             owner=blueprint["debug_entity"], footprint=baseline["footprint"], companion_owners=receipt["outcome"].get("companion_owners"))
                   and len(presses) == 1 and receipt["real_seconds"] >= presses[0]["real_seconds"]
                   and receipt["sample_index"] >= presses[0]["sample_index"]
                   and context(placement["before"])["real_seconds"] < receipt["real_seconds"] <= context(placement["after"])["real_seconds"],
                   "Tank completion actual placement differs")
    complete = next(row for row in raw["events"] if row.get("operation") == "BuildingCompleted")
    for sample in samples:
        bps = [row for row in sample["blueprints"] if row.get("kind") == "Tank"]
        owners = [row for row in sample["owners"] if row.get("kind") == "Tank"]
        index = sample["sample_index"]
        if index < receipt["sample_index"]:
            native.require(not bps and not owners, "Tank existed before actual placement")
        elif index < complete["sample_index"]:
            native.require(not owners and len(bps) == 1 and bps[0].get("entity") == blueprint["debug_entity"]
                           and bps[0].get("entity_bits") == blueprint["entity"]
                           and bps[0].get("footprint") == baseline["footprint"]
                           and planning.finite(bps[0].get("progress")) and 0 <= bps[0]["progress"] <= 1,
                           "Tank Blueprint changed before completion")
        else:
            native.require(not bps and len(owners) == 1 and owners[0].get("entity") == complete["owner"],
                           "Tank completed owner replaced/disappeared")
        if index >= receipt["sample_index"]:
            owner = blueprint["debug_entity"] if index < complete["sample_index"] else complete["owner"]
            layers = [row for row in sample["owner_layers"] if row.get("entity") == owner]
            native.require(len(layers) == 1 and layers[0].get("exists") is True
                           and pipeline.exact_json(sorted(layers[0].get("building_grids", [])), sorted(baseline["footprint"]))
                           and all(pipeline.exact_json(layers[0].get(key), [])
                                   for key in ("floor_grids", "door_grids", "bridge_grids", "stockpile_grids")),
                           "Tank intermediate reservation map changed")
    check_companions(raw, entries, pending, receipt, complete, inputs)
    check_completed_consumers(raw, entries[-1]["value"], contract)


def check_completed_consumers(raw, value, contract, *, water_count=0):
    ctx = context(value)
    near = [sample for sample in raw["samples"] if sample["world_epoch"] == value["world_epoch"]
            and sample["primary_window_bits"] == value["primary_window"] and sample.get("paused") is True
            and abs(sample["real_seconds"] - ctx["real_seconds"]) <= 1 and sample["virtual_seconds"] == ctx["virtual_seconds"]]
    native.require(near, "Tank completed capture lacks same paused clock")
    for sample in (min(near, key=lambda sample: abs(sample["real_seconds"] - ctx["real_seconds"])),):
        feedback = sample.get("art_feedback", {})
        native.require(pipeline.exact_json(feedback.get("requested"), contract["identity"])
                       and pipeline.exact_json(feedback.get("active"), contract["identity"]) and feedback.get("pending") is None,
                       "Tank completed generation differs")
        descriptor = feedback.get("descriptor", {})
        declared = spa.runtime_parts(contract["parts"])
        native.require(pipeline.exact_json(descriptor.get("identity"), contract["identity"])
                       and pipeline.exact_json(descriptor.get("parts"), declared), "Tank completed descriptor differs")
        roots = feedback.get("roots", [])
        native.require(len(roots) == 1 and pipeline.exact_json(roots[0].get("identity"), contract["identity"])
                       and roots[0].get("owner_pose_matches") is True
                       and any(owner["entity"] == roots[0].get("owner") and owner.get("entity_bits") == ctx["buildings"][0]
                               for owner in sample["owners"]), "Tank completed root owner differs")
        tank = [row for row in sample.get("m2_context", {}).get("tanks", []) if row.get("entity") == roots[0]["owner"]]
        water = [row for row in sample.get("m2_context", {}).get("items", []) if row.get("stored_in") == roots[0]["owner"]]
        native.require(type(water_count) is int and 0 <= water_count <= 50 and len(tank) == 1 and "stored_items" in tank[0]
                       and (tank[0]["stored_items"] is None and water_count == 0 or isinstance(tank[0]["stored_items"], list))
                       and len(water) == water_count
                       and len({row["entity"] for row in water}) == water_count
                       and all(row.get("resource") == "Water" and "loaded_in" in row and row["loaded_in"] is None for row in water)
                       and sorted(tank[0]["stored_items"] or []) == sorted(row["entity"] for row in water),
                       "Tank capture inventory differs; never clear supply")
        parts = roots[0].get("parts", [])
        native.require(len(parts) == len(declared) == 2 and [part.get("name") for part in parts] == [part["name"] for part in declared],
                       "Tank completed body/water inventory differs")
        for part, authored in zip(parts, declared, strict=True):
            native.require(part.get("parent") == roots[0]["entity"] and part.get("visible") is (part.get("name") != "water" or water_count > 0)
                           and isinstance(part.get("material"), str) and part["material"]
                           and isinstance(descriptor.get("albedo"), str) and descriptor["albedo"]
                           and part.get("albedo") == descriptor["albedo"]
                           and isinstance(descriptor.get("meshes", {}).get(authored["mesh_role"]), str)
                           and part.get("mesh") == descriptor["meshes"][authored["mesh_role"]], "Tank completed mesh/material differs")
            for key, authored_key in (("translation", "translation_wu"), ("rotation", "rotation_xyzw"), ("scale", "scale")):
                actual, expected = part.get("pose", {}).get(key), authored[authored_key]
                if part["name"] == "water" and key == "translation" and water_count > 0:
                    expected = [*expected]
                    expected[1] = contract["production_state"]["full_y_wu" if water_count == 50 else "partial_y_wu"]
                native.require(isinstance(actual, list) and len(actual) == len(expected)
                               and all(planning.finite(a) and abs(a - b) <= 1e-5 for a, b in zip(actual, expected, strict=True)),
                               "Tank initial completed part pose differs")
            material = part.get("material_values", {})
            partial = part["name"] == "water" and 0 < water_count < 50
            native.require(material.get("white") is (not partial)
                           and all(material.get(key) is True for key in ("opaque", "lit", "emissive_black", "emissive_texture_absent"))
                           and planning.finite(material.get("roughness")) and material["roughness"] == 1
                           and planning.finite(material.get("reflectance")) and material["reflectance"] == 0,
                           "Tank completed material values differ")
            if water_count > 0:
                color = material.get("base_color_srgba")
                expected_color = [.58, .70, .73, 1.] if partial else [1., 1., 1., 1.]
                native.require(isinstance(color, list) and len(color) == 4 and all(planning.finite(a) and abs(a-b) < 1e-5
                               for a, b in zip(color, expected_color, strict=True)), "Tank actual water tint differs")
