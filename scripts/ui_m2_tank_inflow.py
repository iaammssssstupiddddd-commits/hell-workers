"""Natural Tank pour and water presentation, not River-fill or release acceptance."""
from pathlib import Path
from scripts import ui_m2_tank_completion as construction
from scripts import building_production_lifecycle as lifecycle

native, pipeline, planning = construction.native, construction.pipeline, construction.planning
CASE, SCOPE = construction.INFLOW_CASE, construction.INFLOW_SCOPE
CHECKPOINTS = (*construction.CHECKPOINTS, "m2-tank-inflow")
FULL_CASE, FULL_SCOPE = construction.FULL_CASE, construction.FULL_SCOPE
FULL_CHECKPOINTS = (*construction.CHECKPOINTS, "m2-tank-full")
PROFILES = ((CASE, SCOPE), (FULL_CASE, FULL_SCOPE))
prepare_view = construction.prepare_view
check_view = construction.check_view
session_setting = construction.session_setting


def source_contract(repo, source, codec):
    return construction.source_contract(repo, source, codec, profile=(CASE, SCOPE))


def planned_contract(args, repo, *, profile=(CASE, SCOPE)):
    native.require(profile in PROFILES and args.case == profile[0] and args.smoke and args.input_backend == "portal-eis"
                   and args.m2_art_source and args.art_codec and not args.layout_menu
                   and not any(getattr(args, key, None) for key in
                               ("m5_art_source", "rest_area_art_source", "soul_spa_art_source", "spa_resume_source")),
                   "Tank inflow requires its dedicated candidate EIS case")
    contract = construction.source_contract(repo, Path(args.m2_art_source), Path(args.art_codec), profile=profile)
    if getattr(args, "art_contract_sha256", None):
        native.require(planning.art_contract_digest(contract) == args.art_contract_sha256, "Tank inflow source changed")
    return contract


def check_observation(case, value):
    if case in construction.CHECKPOINTS:
        construction.check_observation(case, value)
        return
    native.require(case in (CHECKPOINTS[-1], FULL_CHECKPOINTS[-1]), "Tank inflow checkpoint differs")
    construction.check_observation(construction.CHECKPOINTS[-1], value)
    roots = construction.context(value).get("consumers", {}).get("roots", [])
    native.require(len(roots) == 1 and type(roots[0].get("tank_water_count")) is int
                   and 0 < roots[0]["tank_water_count"] <= 50, "Tank inflow capture has no real water")
    if case == FULL_CHECKPOINTS[-1]:
        native.require(roots[0]["tank_water_count"] == 50, "Tank Full capture requires fifty actual Water items")


def exercise(driver, contract):
    native.require((contract.get("case"),contract.get("scope")) in PROFILES, "Tank supply exercise profile differs")
    full = contract["case"] == FULL_CASE
    construction.exercise(driver, contract)
    construction.capture_action(driver, "menu:SetTimeSpeed(Normal)")
    def supplied(value):
        roots = construction.context(value).get("consumers", {}).get("roots", [])
        return len(roots) == 1 and type(roots[0].get("tank_water_count")) is int and (
            roots[0]["tank_water_count"] == 50 if full else roots[0]["tank_water_count"] > 0)
    driver.last = driver.wait(supplied, after=driver.last["frame"], timeout=180)
    construction.capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture((FULL_CHECKPOINTS if full else CHECKPOINTS)[-1])


def check_tail_inputs(raw, entries, client, inputs):
    """Two time controls only, exact sent/raw pairs and same client throughout."""
    lower, upper = entries[-2:]
    tail = inputs[lower["last_step"]:]
    actions = [row["tank_completion_action"] for row in tail if "tank_completion_action" in row]
    native.require([a["key"] for a in actions] == ["menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"],
                   "Tank inflow permits only Normal then Paused")
    claimed, raw_claimed, effects = set(), set(), []
    epoch = upper["value"]["world_epoch"]
    planning.check_transport(inputs, client, epoch)
    for action, paused in zip(actions, (False, True), strict=True):
        before, after = action["before"], action["after"]
        native.require(all(type(value.get(k)) is int for value in (before,after)
                           for k in ("pid","world_epoch","primary_window","frame")), "Tank inflow action integer schema differs")
        first, last = action["first_step"], action["last_step"]
        native.require(type(first) is int and type(last) is int and last == first + 1
                       and lower["last_step"] < first < last <= upper["last_step"] == len(inputs)
                       and not claimed.intersection((first, last)), "Tank inflow sent pair differs")
        claimed.update((first, last))
        start, end = (construction.context(v)["real_seconds"] for v in (before, after))
        native.require(planning.finite(start) and planning.finite(end) and start < end
                       and construction.context(lower["value"])["real_seconds"] <= start < end
                       < construction.context(upper["value"])["real_seconds"]
                       and lower["value"]["frame"] <= before["frame"] < after["frame"] < upper["value"]["frame"]
                       and all(v.get("nonce") == client["nonce"] and v.get("pid") == client["pid"]
                               and v.get("world_epoch") == epoch and v.get("primary_window") == upper["value"]["primary_window"]
                               for v in (before, after)) and after.get("paused") is paused,
                       "Tank inflow time-control context differs")
        sent = inputs[first-1:last]
        native.require([row.get("pressed") for row in sent] == [True, False]
                       and all(type(row.get("pressed")) is bool and type(row.get("button")) is int and row["button"] == 1
                               and row.get("key") is None and before["frame"] <= row["pre_frame"] < row["post_frame"] <= after["frame"]
                               for row in sent), "Tank inflow input is not an exact mouse pair")
        control = before["controls"].get(action["key"], {})
        native.require(control.get("fully_visible") is True and control.get("interaction") == "Hovered"
                       and sent[0].get("observed_hover", {}).get(action["key"]) == "Pressed", "Tank inflow time target differs")
        rows = [row for row in raw["events"] if row.get("producer") in ("KeyboardInput", "MouseButtonInput")
                and start < row.get("real_seconds", -1) <= end]
        native.require(len(rows) == 2 and [r.get("state") for r in rows] == ["Pressed", "Released"], "Tank inflow raw pair missing")
        for row in rows:
            index = row.get("sample_index")
            native.require(type(index) is int and 0 <= index < len(raw["samples"]), "Tank inflow input ordinal differs")
            sample = raw["samples"][index]
            cursor, dpi = sample.get("cursor_position"), sample.get("dpi")
            native.require(row.get("producer") == "MouseButtonInput" and row.get("button") == "Left"
                           and row.get("window") == sample.get("primary_window")
                           and row["real_seconds"] <= sample["real_seconds"]
                           and isinstance(cursor, list) and len(cursor) == 2 and planning.finite(dpi) and dpi > 0
                           and dpi == before.get("scale_factor")
                           and all(planning.finite(a) and abs(a*dpi-b) <= 2 for a,b in zip(cursor, before["cursor"], strict=True))
                           and row["event_index"] not in raw_claimed, "Tank inflow raw input ownership/cursor differs")
            raw_claimed.add(row["event_index"])
        effects.append((rows[-1]["sample_index"], paused))
    native.require(all(row["step"].isdigit() and int(row["step"]) in claimed for row in tail
                       if row.get("button") is not None or row.get("key") is not None), "Tank inflow extra input")
    start = construction.context(lower["value"])["real_seconds"]
    end = construction.context(upper["value"])["real_seconds"]
    native.require(raw_claimed == {row["event_index"] for row in raw["events"]
                                  if row.get("producer") in ("KeyboardInput", "MouseButtonInput") and row["real_seconds"] > start},
                   "Tank inflow unclaimed raw input")
    samples = [s for s in raw["samples"] if start <= s["real_seconds"] <= end]
    native.require(samples and construction.context(upper["value"])["virtual_seconds"] > construction.context(lower["value"])["virtual_seconds"],
                   "Tank inflow has no simulation advance")
    for sample in samples:
        paused = next((state for index,state in reversed(effects) if index <= sample["sample_index"]), True)
        native.require(sample.get("paused") is paused and planning.finite(sample.get("relative_speed")) and sample["relative_speed"] == 1, "Tank inflow time phase differs")
    native.require(all(a["virtual_seconds"] <= b["virtual_seconds"]
                       and (not (a["paused"] and b["paused"]) or a["virtual_seconds"] == b["virtual_seconds"])
                       for a,b in zip(samples, samples[1:])), "Tank inflow virtual clock differs")
    return effects, actions


def verify(raw, entries, client, inputs, contract, *, profile=(CASE, SCOPE)):
    checkpoints = FULL_CHECKPOINTS if profile == (FULL_CASE,FULL_SCOPE) else CHECKPOINTS
    native.require(profile in PROFILES and contract.get("case") == profile[0] and contract.get("scope") == profile[1]
                   and raw.get("scope") == profile[1] and [e["case"] for e in entries] == list(checkpoints), "Tank inflow profile differs")
    samples, events = raw["samples"], raw["events"]
    epoch, window = entries[0]["value"]["world_epoch"], entries[0]["value"]["primary_window"]
    native.require(0 < len(samples) <= 1200 and len(events) < 4096
                   and type(epoch) is int and type(window) is int
                   and all(type(s.get("sample_index")) is int and s["sample_index"] == index
                           and type(s.get("world_epoch")) is int and s["world_epoch"] == epoch
                           and type(s.get("primary_window_bits")) is int and s["primary_window_bits"] == window
                           and planning.finite(s.get("real_seconds")) and s["real_seconds"] >= 0
                           and planning.finite(s.get("virtual_seconds")) and s["virtual_seconds"] >= 0
                           for index,s in enumerate(samples))
                   and all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                           for a,b in zip(samples, samples[1:]))
                   and all(type(e.get("event_index")) is int and e["event_index"] == index
                           and type(e.get("sample_index")) is int and 0 <= e["sample_index"] < len(samples)
                           for index,e in enumerate(events)), "Tank inflow full campaign envelope differs")
    for entry in entries:
        native.require(type(entry.get("last_step")) is int and all(type(entry["value"].get(k)) is int
                       for k in ("pid","world_epoch","primary_window","frame")), "Tank inflow checkpoint integer schema differs")
        check_observation(entry["case"], entry["value"])
    effects, actions = check_tail_inputs(raw, entries, client, inputs)
    # Keep the original scope and ordinal values: this is a contiguous same-process
    # prefix, not a historical pass or a relabeled completion batch.
    before = construction.context(actions[0]["before"])["real_seconds"]
    cutoff = max(s["sample_index"] for s in raw["samples"] if s["real_seconds"] <= before)
    prefix = dict(raw, samples=raw["samples"][:cutoff+1], events=[e for e in raw["events"] if e["sample_index"] <= cutoff])
    construction.verify(prefix, entries[:3], client, inputs[:entries[2]["last_step"]], contract, profile=profile)
    lifecycle.check_raw_transitions(raw)
    final = entries[-1]["value"]
    ctx = construction.context(final)
    old = construction.context(entries[2]["value"])
    native.require(all(pipeline.exact_json(ctx[k], old[k]) for k in ("anchor", "footprint", "draw_pos", "buildings"))
                   and final.get("nonce") == client["nonce"] and final.get("pid") == client["pid"]
                   and final.get("world_epoch") == entries[0]["value"]["world_epoch"]
                   and final.get("primary_window") == entries[0]["value"]["primary_window"], "Tank inflow target replaced")
    owner = old["consumers"]["roots"][0]["owner"]
    root = old["consumers"]["roots"][0]["entity"]
    pours = [e for e in raw["events"] if e.get("operation") == "TankPour"]
    native.require(pours and all(e["owner"] == owner and e["sample_index"] > cutoff for e in pours), "Tank inflow lacks actual post-construction pour")
    placement = [e for e in raw["events"] if e.get("producer") == "ordinary-building-placement-branch-v1"]
    complete = next(e for e in raw["events"] if e.get("operation") == "BuildingCompleted")
    construction.check_companions(raw, entries[:3], *placement, complete, inputs)
    initial = raw["samples"][complete["sample_index"]]["m2_context"]["items"]
    buckets = {i["entity"] for i in initial if i.get("belongs_to") == owner}
    produced = set()
    for event in pours:
        lifecycle.check_tank_pour_receipt(event, raw["samples"], raw["events"])
        native.require(event["bucket"] in buckets and not produced.intersection(event["outputs"]), "Tank inflow foreign bucket or repeated water")
        produced.update(event["outputs"])
        native.require(event["sample_index"] > 0, "Tank pour has no preceding transport observation")
        prior = raw["samples"][event["sample_index"]-1].get("m2_context", {})
        actors = [actor for actor in prior.get("bucket_actors", []) if actor.get("entity") == event["worker"]]
        prior_buckets = [item for item in prior.get("items", []) if item.get("entity") == event["bucket"]]
        native.require(len(actors) == len(prior_buckets) == 1
                       and prior_buckets[0].get("resource") == "BucketWater"
                       and prior_buckets[0].get("belongs_to") == owner
                       and prior_buckets[0].get("delivering_to") == owner
                       and "loaded_in" in prior_buckets[0] and prior_buckets[0]["loaded_in"] is None,
                       "Tank pour preceding carried bucket differs")
        # The branch receipt proves Pouring. Periodic observations may skip that
        # one-frame phase; require the immediately preceding same live transport,
        # never a stale actor found anywhere in the campaign.
        native.require(all(actor.get("entity") == event["worker"] and actor.get("bucket") == event["bucket"]
                           and actor.get("is_soul") is True and actor.get("phase") in ("GoingToDestination", "Pouring")
                           and actor.get("carried_item") == event["bucket"] and actor.get("working_on") == event["target"]
                           and pipeline.exact_json(actor.get("source"), {"kind":"River"})
                           and pipeline.exact_json(actor.get("destination"), {"kind":"Tank", "entity":owner, "exists":True})
                           and pipeline.exact_json(actor.get("identity"), {"assignment":event["assignment"], "target":event["target"],
                                                                      "work_type":"GatherWater", "matches_working_on":True})
                           for actor in actors),
                       "Tank pour lacks exact prior live assignment")
    for sample in raw["samples"][cutoff+1:]:
        native.require(sample.get("world_epoch") == final["world_epoch"] and sample.get("primary_window_bits") == final["primary_window"]
                       and not sample["blueprints"] and len(sample["owners"]) >= 1, "Tank inflow intermediate world changed")
        owners = [r for r in sample["owners"] if r.get("kind") == "Tank"]
        layers = [r for r in sample["owner_layers"] if r.get("entity") == owner]
        typed = sample.get("m2_context", {})
        tanks = [r for r in typed.get("tanks", []) if r.get("entity") == owner]
        native.require(len(owners) == len(layers) == len(tanks) == 1
                       and owners[0].get("entity") == owner and owners[0].get("entity_bits") == ctx["buildings"][0]
                       and owners[0].get("provisional") is False and layers[0].get("exists") is True
                       and pipeline.exact_json(layers[0].get("building_grids"), ctx["footprint"])
                       and all(layers[0].get(k) == [] for k in ("floor_grids", "door_grids", "bridge_grids", "stockpile_grids"))
                       and tanks[0].get("provisional") is False
                       and pipeline.exact_json(tanks[0].get("stockpile"), dict(resource="Water", capacity=50)),
                       "Tank inflow intermediate owner/map/stockpile differs")
        reference = raw["samples"][cutoff]["m2_context"]["tanks"][0]
        pose, initial_pose = tanks[0].get("pose"), reference.get("pose")
        native.require(isinstance(pose, dict) and isinstance(initial_pose, dict)
                       and all(pipeline.exact_json(pose.get(k), initial_pose.get(k)) for k in ("translation", "rotation"))
                       and isinstance(pose.get("scale"), list) and len(pose["scale"]) == 3
                       and all(planning.finite(v) and 1. <= v <= 1.20001 for v in pose["scale"])
                       and max(pose["scale"])-min(pose["scale"]) < 1e-5,
                       "Tank inflow intermediate pose differs")
        actual_buckets = [i for i in typed.get("items", []) if i.get("belongs_to") == owner and i.get("resource") in ("BucketEmpty", "BucketWater")]
        native.require(len(actual_buckets) == 5 and {b["entity"] for b in actual_buckets} == buckets,
                       "Tank inflow dedicated bucket identity changed")
        roots = sample.get("art_feedback", {}).get("roots", [])
        native.require(len(roots) == 1 and roots[0].get("entity") == root and roots[0].get("owner") == owner
                       and pipeline.exact_json(roots[0].get("identity"), contract["identity"]), "Tank inflow root replaced")
        actual = {i["entity"] for i in sample.get("m2_context", {}).get("items", []) if i.get("stored_in") == owner}
        expected = {i for e in pours if e["sample_index"] <= sample["sample_index"] for i in e["outputs"]}
        native.require(actual == expected, "Tank water appeared/disappeared without a pour")
        rows = [i for i in typed.get("items", []) if i.get("stored_in") == owner]
        native.require(len(rows) == len(actual) and all(i.get("resource") == "Water" and "loaded_in" in i and i["loaded_in"] is None for i in rows)
                       and "stored_items" in tanks[0] and (tanks[0]["stored_items"] is None or isinstance(tanks[0]["stored_items"], list))
                       and sorted(tanks[0]["stored_items"] or []) == sorted(actual),
                       "Tank inflow intermediate water inverse differs")
    count = ctx["consumers"]["roots"][0]["tank_water_count"]
    native.require(count == len(produced) and count <= 50, "Tank inflow capture water count differs")
    construction.check_completed_consumers(raw, final, contract, water_count=count)
