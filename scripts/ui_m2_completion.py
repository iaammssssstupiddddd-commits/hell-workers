"""Ordinary Mixer construction only; no fixture, refinement or release claim."""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
from scripts import ui_m2_planning as planning
from scripts import ui_spa_candidate_art as spa

native, pipeline = planning.native, planning.pipeline
CASE = "m2-mixer-candidate-completion"
SCOPE = "ordinary-m2-candidate-completion-v1"
CHECKPOINTS = tuple("m2-completion-" + phase for phase in ("empty", "placed", "completed"))


def source_contract(repo, source, codec):
    result = planning.source_contract(repo, source, codec, "m2-mixer-candidate-planning")
    manifest, _, _ = pipeline.load_set(source, "MudMixer", codec)
    result.update(case=CASE, scope=SCOPE, parts=manifest["parts"],
                  planning_verifier_sha256=result["verifier_sha256"],
                  verifier_sha256=pipeline.digest(Path(__file__).read_bytes()))
    return result


def planned_contract(args, repo):
    native.require(args.case == CASE and args.smoke and args.input_backend == "portal-eis"
                   and args.m2_art_source and args.art_codec and not args.layout_menu
                   and not any(getattr(args, key, None) for key in
                               ("m5_art_source", "rest_area_art_source", "soul_spa_art_source", "spa_resume_source")),
                   "Mixer completion requires its dedicated candidate EIS case")
    contract = source_contract(repo, Path(args.m2_art_source), Path(args.art_codec))
    if getattr(args, "art_contract_sha256", None):
        native.require(planning.art_contract_digest(contract) == args.art_contract_sha256, "Mixer completion source changed")
    return contract


def prepare_view(repo, directory, contract):
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"])), contract),
                   "Mixer completion source changed")
    # The overlay byte operation is identical; its source contract is independently
    # reconstructed above, never inferred from an authority flag in the job.
    original = planning.source_contract(repo, Path(contract["source_root"]), Path(contract["codec"]), "m2-mixer-candidate-planning")
    return planning.prepare_view(repo, directory, original)


check_view = planning.check_view


def session_setting(directory, nonce, contract):
    return json.dumps({"mode": "m2-candidate-completion", "identity": contract["identity"],
                       "locator": pipeline.locator("MudMixer"), "nonce": nonce,
                       "output": str(directory / "production-cancellation.json")})


def context(value):
    ctx = value.get("m2_planning")
    native.require(isinstance(ctx, dict) and ctx.get("failure") is None and ctx.get("inventory_overflow") is False
                   and ctx.get("kind") == "MudMixer" and ctx.get("initial_building_inventory") == []
                   and type(ctx.get("world_epoch")) is int and ctx["world_epoch"] == value.get("world_epoch")
                   and type(ctx.get("primary_window")) is int and ctx["primary_window"] == value.get("primary_window")
                   and ctx.get("camera", {}).get("primary") is True
                   and ctx["camera"].get("viewport") == value.get("viewport"), "Mixer completion context differs")
    anchor = ctx.get("anchor")
    native.require(isinstance(anchor, list) and len(anchor) == 2 and all(type(axis) is int for axis in anchor),
                   "Mixer completion anchor differs")
    x, y = anchor
    footprint = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]]
    native.require(pipeline.exact_json(ctx.get("footprint"), footprint)
                   and [row.get("grid") for row in ctx.get("cells", [])] == footprint, "Mixer completion footprint differs")
    return ctx


def check_observation(case, value):
    native.require(case in CHECKPOINTS and value.get("paused") is True, "Mixer completion checkpoint/pause differs")
    ctx = context(value)
    if case == CHECKPOINTS[0]:
        native.require(ctx["blueprints"] == [] and ctx["buildings"] == [] and ctx["building_inventory"] == []
                       and ctx["legal"] is True and all(row["owner"] is None and row["walkable"] is True for row in ctx["cells"]),
                       "Mixer completion must start with an actually empty legal footprint")
    elif case == CHECKPOINTS[1]:
        native.require(ctx["buildings"] == [] and len(ctx["blueprints"]) == 1 and ctx["legal"] is False
                       and all(row["owner"] == ctx["blueprints"][0]["entity"] for row in ctx["cells"]),
                       "Mixer completion placed Blueprint differs")
    else:
        native.require(ctx["blueprints"] == [] and len(ctx["buildings"]) == len(ctx["building_inventory"]) == 1
                       and ctx["legal"] is False and all(row["owner"] == ctx["buildings"][0] and row["walkable"] is False for row in ctx["cells"]),
                       "Mixer completion owner/blocked footprint differs")


def capture_action(driver, key, *, baseline=None):
    world = key == "placement"
    if not world and key != "Escape":
        driver.send(point=driver.point(key))
        native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered",
                       "Mixer completion target not hovered; no press")
    before, first = copy.deepcopy(driver.last), len(driver.events)
    if world:
        planning.check_world_action(before, key, {"identity": {"kind": "MudMixer"}}, baseline)
    if key == "Escape":
        driver.key(key)
    else:
        driver.send(button=1, pressed=True)
        if not world:
            native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed",
                           "Mixer completion control press missing")
        driver.send(button=1, pressed=False)
    def flushed(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "Mixer completion raw flush failed")
        rows = [row for row in raw.get("events", []) if row.get("producer") == ("KeyboardInput" if key == "Escape" else "MouseButtonInput")
                and context(before)["real_seconds"] < row.get("real_seconds", -1) <= context(value)["real_seconds"]]
        return len(rows) == 2 and [row.get("state") for row in rows] == ["Pressed", "Released"]
    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["m2_completion_action"] = dict(key=key, first_step=first + 1, last_step=len(driver.events),
                                                      before=before, after=copy.deepcopy(driver.last))
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise(driver, contract):
    baseline = copy.deepcopy(context(driver.last))
    if "text-field:DevPoc" in driver.last["controls"]:
        capture_action(driver, "dev-minimize")
    driver.capture(CHECKPOINTS[0])
    capture_action(driver, "menu:SetTimeSpeed(Normal)")
    capture_action(driver, "menu:ToggleArchitect")
    capture_action(driver, "menu:SelectBuild(MudMixer)")
    point = context(driver.last)["point"]
    native.require(isinstance(point, list) and len(point) == 2 and all(planning.finite(axis) and 0 < axis < limit
                   for axis, limit in zip(point, driver.last["viewport"], strict=True)), "Mixer target outside window")
    driver.send(point=tuple(round(axis * driver.last["scale_factor"]) for axis in point))
    capture_action(driver, "placement", baseline=baseline)
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


def check_completion(raw, entries, client):
    """Actual domain completion plus delivery/task/owner witnesses, not release."""
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
    native.require(raw.get("scope") == SCOPE
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
    native.require(sum(event.get("operation") == "BuildingCompleted" for event in events) == 1,
                   "actual completion domain event missing or duplicated")
    check_mixer_domain_completion(raw, placed, completed, context(placed), context(completed))


def check_mixer_domain_completion(raw, placed, completed, placed_context, completed_context):
    """Exact Mixer domain history, also usable inside a declared multi-owner campaign.

    This does not validate a campaign envelope, inputs, art or admission. The
    caller must validate those separately; unrelated completion events are kept.
    """
    from scripts import building_production_lifecycle as lifecycle
    samples, events = raw["samples"], raw["events"]
    footprint = placed_context["footprint"]
    rows = [event for event in events if event.get("operation") == "BuildingCompleted"
            and event.get("kind") == "MudMixer"]
    native.require(len(rows) == 1, "actual completion domain event missing or duplicated")
    row = rows[0]
    index = row.get("sample_index")
    native.require(row.get("producer") == "hw_jobs::publish_building_completed"
                   and row.get("kind") == "MudMixer" and row.get("footprint") == footprint
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
    old_bits = placed_context["blueprints"][0]["entity"]
    native.require(prior and all(bp.get("entity_bits") == old_bits and bp.get("kind") == "MudMixer"
                                and bp.get("footprint") == footprint for bp in prior)
                   and any(bp.get("materials_complete") is True
                           and bp.get("delivered_materials") == {"Wood": 4} for bp in prior)
                   and not any(bp.get("entity") == row["blueprint_owner"] for bp in sample.get("blueprints", [])),
                   "completion lacks real complete-material Blueprint history")
    owners = [owner for owner in sample.get("owners", []) if owner.get("entity") == row["owner"]]
    native.require(len(owners) == 1 and owners[0].get("kind") == "MudMixer"
                   and owners[0].get("provisional") is False
                   and owners[0].get("entity_bits") == completed_context["buildings"][0],
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
    native.require(len(deliveries) == 4 and len({event.get("item") for event in deliveries}) == 4,
                   "completion needs four distinct real Wood deliveries")
    for count, event in enumerate(deliveries):
        lifecycle.check_delivery_receipt(event, samples)
        native.require(event["world_epoch"] == epoch and event["kind"] == "MudMixer"
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

def verify(raw, entries, client, inputs, contract):
    from scripts import building_production_lifecycle as lifecycle
    native.require(contract.get("case") == CASE and contract.get("scope") == SCOPE
                   and contract["identity"].get("kind") == "MudMixer" and contract["identity"].get("authority") == "isolated_candidate",
                   "Mixer completion candidate contract differs")
    check_completion(raw, entries, client)
    lifecycle.check_raw_transitions(raw)
    epoch, window = entries[0]["value"]["world_epoch"], entries[0]["value"]["primary_window"]
    samples = raw["samples"]
    native.require(all(type(sample.get("world_epoch")) is int and sample["world_epoch"] == epoch
                       and type(sample.get("primary_window_bits")) is int and sample["primary_window_bits"] == window
                       and planning.finite(sample.get("virtual_seconds")) and sample["virtual_seconds"] >= 0 for sample in samples)
                   and all(a["virtual_seconds"] <= b["virtual_seconds"] for a, b in zip(samples, samples[1:])),
                   "Mixer completion intermediate identity/virtual clock differs")
    for entry in entries:
        value, ctx = entry["value"], context(entry["value"])
        native.require(all(type(value.get(key)) is int for key in ("pid", "world_epoch", "primary_window", "frame"))
                       and value["pid"] == client["pid"] and value["world_epoch"] == epoch and value["primary_window"] == window
                       and planning.finite(ctx.get("real_seconds")) and planning.finite(ctx.get("virtual_seconds")),
                       "Mixer completion checkpoint owner/clock differs")
        near = [sample for sample in samples if abs(sample["real_seconds"] - ctx["real_seconds"]) <= 1
                and sample["virtual_seconds"] == ctx["virtual_seconds"] and sample.get("paused") is True]
        native.require(near, "Mixer completion checkpoint lacks same paused raw clock")
    planning.check_transport(inputs, client, epoch)
    actions = [row["m2_completion_action"] for row in inputs if "m2_completion_action" in row]
    expected = ["menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", "menu:SelectBuild(MudMixer)", "placement",
                "Escape", "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"]
    native.require([action.get("key") for action in actions] in (expected, ["dev-minimize", *expected]),
                   "Mixer completion input sequence differs")
    claimed, raw_claimed = set(), set()
    baseline = context(entries[0]["value"])
    placement, time_effects = None, []
    for action in actions:
        before, after, key = action["before"], action["after"], action["key"]
        first, last = action["first_step"], action["last_step"]
        native.require(type(first) is int and type(last) is int and last == first + 1 and 1 <= first < last <= len(inputs)
                       and not claimed.intersection({first, last}) and inputs[first - 1].get("m2_completion_action") == action,
                       "Mixer completion input pair missing/reused")
        claimed.update((first, last))
        start, end = context(before)["real_seconds"], context(after)["real_seconds"]
        native.require(planning.finite(start) and planning.finite(end) and start < end and before["frame"] < after["frame"]
                       and all(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                               and value.get("world_epoch") == epoch and value.get("primary_window") == window
                               for value in (before, after)), "Mixer completion action context differs")
        sent = inputs[first - 1:last]
        keyboard = key == "Escape"
        native.require([row.get("pressed") for row in sent] == [True, False]
                       and all(type(row.get("pressed")) is bool and before["frame"] <= row["pre_frame"] < row["post_frame"] <= after["frame"]
                               and (row.get("key") == "Escape" if keyboard else type(row.get("button")) is int and row["button"] == 1)
                               for row in sent), "Mixer completion sent input differs")
        rows = [row for row in raw["events"] if row.get("producer") == ("KeyboardInput" if keyboard else "MouseButtonInput")
                and start < row.get("real_seconds", -1) <= end]
        native.require(len(rows) == 2 and [row.get("state") for row in rows] == ["Pressed", "Released"], "Mixer completion raw input pair differs")
        for row in rows:
            index = row.get("sample_index")
            native.require(type(index) is int and 0 <= index < len(samples) and row["event_index"] not in raw_claimed
                           and row.get("window") == samples[index].get("primary_window")
                           and samples[index]["real_seconds"] >= row["real_seconds"]
                           and (row.get("key") == "Escape" and row.get("repeat") is False if keyboard else row.get("button") == "Left"),
                           "Mixer completion raw input owner differs")
            raw_claimed.add(row["event_index"])
            if not keyboard:
                cursor, dpi = samples[index].get("cursor_position"), samples[index].get("dpi")
                native.require(isinstance(cursor, list) and len(cursor) == 2 and all(planning.finite(axis) for axis in cursor)
                               and planning.finite(dpi) and dpi == before.get("scale_factor") and dpi > 0
                               and all(abs(a * dpi - b) <= 2 for a, b in zip(cursor, before["cursor"], strict=True)),
                               "Mixer completion raw cursor differs")
        if key == "placement":
            planning.check_world_action(before, key, contract, baseline)
            placement = action
        elif not keyboard:
            target = before["controls"].get(key, {})
            native.require(target.get("fully_visible") is True and target.get("interaction") == "Hovered"
                           and sent[0].get("observed_hover", {}).get(key) == "Pressed", "Mixer completion menu target differs")
        if key in ("menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"):
            paused = key.endswith("(Paused)")
            native.require(after.get("paused") is paused, "Mixer time control did not take effect")
            time_effects.append((rows[-1]["sample_index"], paused))
    native.require(all(index + 1 in claimed for index, row in enumerate(inputs) if row.get("key") is not None or row.get("button") is not None)
                   and raw_claimed == {row["event_index"] for row in raw["events"] if row.get("producer") in ("KeyboardInput", "MouseButtonInput")},
                   "Mixer completion input unclaimed")
    for lower, upper, expected_group in ((entries[0], entries[1], expected[:6]), (entries[1], entries[2], expected[6:])):
        group = [action for action in actions if lower["last_step"] < action["first_step"] <= upper["last_step"]]
        native.require([action["key"] for action in group] == expected_group
                       and all(action["last_step"] <= upper["last_step"] and lower["value"]["frame"] <= action["before"]["frame"]
                               < action["after"]["frame"] < upper["value"]["frame"]
                               and context(lower["value"])["real_seconds"] <= context(action["before"])["real_seconds"]
                               < context(action["after"])["real_seconds"] < context(upper["value"])["real_seconds"] for action in group),
                       "Mixer completion action crossed checkpoint interval")
    start, end = context(entries[0]["value"])["real_seconds"], context(entries[-1]["value"])["real_seconds"]
    tail = [sample for sample in samples if start <= sample["real_seconds"] <= end]
    native.require(tail and context(entries[-1]["value"])["virtual_seconds"] > context(entries[1]["value"])["virtual_seconds"],
                   "Mixer construction has no real simulation advance")
    for sample in tail:
        expected_paused = next((paused for index, paused in reversed(time_effects) if index <= sample["sample_index"]), True)
        native.require(type(sample.get("paused")) is bool and sample["paused"] is expected_paused
                       and planning.finite(sample.get("relative_speed")) and sample["relative_speed"] == 1,
                       "Mixer actual time control phase/speed differs")
    native.require(all(a["virtual_seconds"] == b["virtual_seconds"] for a, b in zip(tail, tail[1:])
                       if a["paused"] is True and b["paused"] is True), "Mixer paused simulation advanced")
    receipts = [row for row in raw["events"] if row.get("producer") == "ordinary-building-placement-branch-v1"]
    native.require(len(receipts) == 1 and placement is not None, "Mixer completion placement receipt missing/duplicate")
    receipt = receipts[0]
    lifecycle.check_placement_receipt(receipt, samples)
    blueprint = context(entries[1]["value"])["blueprints"][0]
    presses = [row for row in raw["events"] if row.get("producer") == "MouseButtonInput" and row.get("state") == "Pressed"
               and context(placement["before"])["real_seconds"] < row.get("real_seconds", -1) <= context(placement["after"])["real_seconds"]]
    native.require(receipt["outcome"] == dict(kind="MudMixer", grid=baseline["anchor"], status="BlueprintCommitted",
                                             owner=blueprint["debug_entity"], footprint=baseline["footprint"])
                   and len(presses) == 1 and receipt["real_seconds"] >= presses[0]["real_seconds"]
                   and receipt["sample_index"] >= presses[0]["sample_index"]
                   and context(placement["before"])["real_seconds"] < receipt["real_seconds"] <= context(placement["after"])["real_seconds"],
                   "Mixer completion actual placement differs")
    complete = next(row for row in raw["events"] if row.get("operation") == "BuildingCompleted")
    for sample in samples:
        bps = [row for row in sample["blueprints"] if row.get("kind") == "MudMixer"]
        owners = [row for row in sample["owners"] if row.get("kind") == "MudMixer"]
        index = sample["sample_index"]
        if index < receipt["sample_index"]:
            native.require(not bps and not owners, "Mixer existed before actual placement")
        elif index < complete["sample_index"]:
            native.require(not owners and len(bps) == 1 and bps[0].get("entity") == blueprint["debug_entity"]
                           and bps[0].get("entity_bits") == blueprint["entity"]
                           and bps[0].get("footprint") == baseline["footprint"]
                           and planning.finite(bps[0].get("progress")) and 0 <= bps[0]["progress"] <= 1,
                           "Mixer Blueprint changed before completion")
        else:
            native.require(not bps and len(owners) == 1 and owners[0].get("entity") == complete["owner"],
                           "Mixer completed owner replaced/disappeared")
        if index >= receipt["sample_index"]:
            owner = blueprint["debug_entity"] if index < complete["sample_index"] else complete["owner"]
            layers = [row for row in sample["owner_layers"] if row.get("entity") == owner]
            native.require(len(layers) == 1 and layers[0].get("exists") is True
                           and pipeline.exact_json(sorted(layers[0].get("building_grids", [])), sorted(baseline["footprint"]))
                           and all(pipeline.exact_json(layers[0].get(key), [])
                                   for key in ("floor_grids", "door_grids", "bridge_grids", "stockpile_grids")),
                           "Mixer intermediate reservation map changed")
    check_completed_consumers(raw, entries[-1]["value"], contract)


def check_completed_consumers(raw, value, contract):
    ctx = context(value)
    near = [sample for sample in raw["samples"] if sample["world_epoch"] == value["world_epoch"]
            and sample["primary_window_bits"] == value["primary_window"] and sample.get("paused") is True
            and abs(sample["real_seconds"] - ctx["real_seconds"]) <= 1 and sample["virtual_seconds"] == ctx["virtual_seconds"]]
    native.require(near, "Mixer completed capture lacks same paused clock")
    for sample in (min(near, key=lambda sample: abs(sample["real_seconds"] - ctx["real_seconds"])),):
        feedback = sample.get("art_feedback", {})
        native.require(pipeline.exact_json(feedback.get("requested"), contract["identity"])
                       and pipeline.exact_json(feedback.get("active"), contract["identity"]) and feedback.get("pending") is None,
                       "Mixer completed generation differs")
        descriptor = feedback.get("descriptor", {})
        declared = spa.runtime_parts(contract["parts"])
        native.require(pipeline.exact_json(descriptor.get("identity"), contract["identity"])
                       and pipeline.exact_json(descriptor.get("parts"), declared), "Mixer completed descriptor differs")
        roots = feedback.get("roots", [])
        native.require(len(roots) == 1 and pipeline.exact_json(roots[0].get("identity"), contract["identity"])
                       and roots[0].get("owner_pose_matches") is True
                       and any(owner["entity"] == roots[0].get("owner") and owner.get("entity_bits") == ctx["buildings"][0]
                               for owner in sample["owners"]), "Mixer completed root owner differs")
        parts = roots[0].get("parts", [])
        native.require(len(parts) == len(declared) == 2 and [part.get("name") for part in parts] == [part["name"] for part in declared],
                       "Mixer completed body/rotor inventory differs")
        for part, authored in zip(parts, declared, strict=True):
            native.require(part.get("parent") == roots[0]["entity"] and part.get("visible") is True
                           and isinstance(part.get("material"), str) and part["material"]
                           and isinstance(descriptor.get("albedo"), str) and descriptor["albedo"]
                           and part.get("albedo") == descriptor["albedo"]
                           and isinstance(descriptor.get("meshes", {}).get(authored["mesh_role"]), str)
                           and part.get("mesh") == descriptor["meshes"][authored["mesh_role"]], "Mixer completed mesh/material differs")
            for key, authored_key in (("translation", "translation_wu"), ("rotation", "rotation_xyzw"), ("scale", "scale")):
                actual, expected = part.get("pose", {}).get(key), authored[authored_key]
                native.require(isinstance(actual, list) and len(actual) == len(expected)
                               and all(planning.finite(a) and abs(a - b) <= 1e-5 for a, b in zip(actual, expected, strict=True)),
                               "Mixer initial completed part pose differs")
            material = part.get("material_values", {})
            native.require(all(material.get(key) is True for key in ("white", "opaque", "lit", "emissive_black", "emissive_texture_absent"))
                           and planning.finite(material.get("roughness")) and material["roughness"] == 1
                           and planning.finite(material.get("reflectance")) and material["reflectance"] == 0,
                           "Mixer completed material values differ")
