"""Ordinary Tank construction to one observed Mixer refinement, not release.

Only Tank uses the adopted candidate. Mixer deliberately retains the base
presentation; this campaign grants neither Mixer art nor production authority.
"""
import copy
from pathlib import Path

from scripts import ui_m2_tank_completion as tank
from scripts import ui_m2_completion as mixer
from scripts import ui_m2_tank_inflow as inflow
from scripts import building_water_transfers as water
from scripts import building_production_lifecycle as lifecycle

native, pipeline, planning = tank.native, tank.pipeline, tank.planning
CASE, SCOPE = tank.SUPPLY_CASE, tank.SUPPLY_SCOPE
CHECKPOINTS = (*tank.CHECKPOINTS, "m2-supply-mixer-placed", "m2-supply-mixer-completed", "m2-supply-refined")
session_setting, check_view = tank.session_setting, tank.check_view
WORLDGEN = dict(seed=42, attempt=0, fallback=False,
                layout_sha256="d8eff05ae2b6c56d8bbfa39570c6ce6e7174aae0c69f2c5ab53527add5dcff72")
EVENT_CONTRACT = dict(id="m2-supply-events-v1", excluded_producers=["DreamTransferredVisualMessage"])
SETUP = "ordinary-tank-full-empty-returned-buckets-v1"
# Nineteen declared pointer moves plus the optional developer-panel minimize.
MOTION_CAP = 20


def source_contract(repo, source, codec):
    return dict(tank.source_contract(repo, source, codec, profile=(CASE, SCOPE)),
                mixer_presentation="base-assets", material_supply="ordinary-auto-gather",
                supply_verifier_sha256=pipeline.digest(Path(__file__).read_bytes()), worldgen=copy.deepcopy(WORLDGEN),
                event_contract=copy.deepcopy(EVENT_CONTRACT), setup=SETUP, motion_cap=MOTION_CAP)


def planned_contract(args, repo):
    native.require(args.case == CASE and args.smoke and args.input_backend == "portal-eis"
                   and args.m2_art_source and args.art_codec and not args.layout_menu
                   and not any(getattr(args, key, None) for key in
                               ("m5_art_source", "rest_area_art_source", "soul_spa_art_source", "spa_resume_source")),
                   "Tank supply needs its dedicated candidate EIS case")
    contract = source_contract(repo, Path(args.m2_art_source), Path(args.art_codec))
    if getattr(args, "art_contract_sha256", None):
        native.require(planning.art_contract_digest(contract) == args.art_contract_sha256, "Tank supply source changed")
    return contract


def prepare_view(repo, directory, contract):
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"])), contract),
                   "Tank supply contract changed")
    original = tank.source_contract(repo, Path(contract["source_root"]), Path(contract["codec"]), profile=(CASE, SCOPE))
    return tank.prepare_view(repo, directory, original)


def context(value):
    """Geometry/owner observation only; never re-label an art or raw identity."""
    check_worldgen(value)
    ctx = tank.context(value).get("supply_mixer")
    native.require(isinstance(ctx, dict), "Supply Mixer observation missing")
    return mixer.context(dict(value, m2_planning=ctx))


def check_worldgen(value):
    observed = tank.context(value).get("supply_worldgen")
    native.require(isinstance(observed, dict) and set(observed) == {*WORLDGEN, "projected_routes"}
                   and pipeline.exact_json({key: observed[key] for key in WORLDGEN}, WORLDGEN),
                   "Supply natural world identity differs")
    routes = observed["projected_routes"]
    native.require(isinstance(routes, list) and len(routes) == 3 and all(
        isinstance(row, dict) and set(row) == {"phase", "start", "trees", "rocks"}
        and type(row["phase"]) is int and row["phase"] == phase
        and isinstance(row["start"], list) and len(row["start"]) == 2
        and all(type(axis) is int for axis in row["start"])
        and all(type(row[key]) is int and 0 < row[key] <= 128 for key in ("trees", "rocks"))
        for phase, row in enumerate(routes)), "Supply projected natural resource route unavailable")


def check_observation(case, value):
    if case in tank.CHECKPOINTS:
        tank.check_observation(case, value)
        ctx = context(value)
        native.require(ctx["blueprints"] == ctx["buildings"] == ctx["building_inventory"] == []
                       and ctx["legal"] is True, "Supply Mixer must not pre-exist")
        return
    native.require(case in CHECKPOINTS[3:], "Unknown supply checkpoint")
    tank.check_observation(tank.CHECKPOINTS[-1], value)
    # The ordinary Mixer owner/geometry checks are shared, not its candidate art.
    phase = mixer.CHECKPOINTS[1 if case == CHECKPOINTS[3] else 2]
    mixer.check_observation(phase, dict(value, m2_planning=context(value)))


def full_setup_ready(sample, owner, children):
    """Same-snapshot Full inventory and five returned, unheld Empty buckets."""
    ctx = sample.get("m2_context", {})
    tanks = [t for t in ctx.get("tanks", []) if t.get("entity") == owner]
    if len(tanks) != 1 or not isinstance(tanks[0].get("stored_items"), list) or len(tanks[0]["stored_items"]) != 50:
        return False
    native.require(tanks[0].get("provisional") is False
                   and pipeline.exact_json(tanks[0].get("stockpile"), dict(resource="Water", capacity=50)),
                   "Supply Full owner differs")
    water.check_inventory(ctx, tanks[0])
    buckets = [item for item in ctx.get("items", []) if item.get("belongs_to") == owner]
    native.require(len(buckets) == 5 and len({b.get("entity") for b in buckets}) == 5,
                   "Supply Full dedicated bucket identities differ")
    storages = [s for s in ctx.get("bucket_storages", []) if s.get("belongs_to") == owner]
    native.require(isinstance(children, list) and len(children) == len(set(children)) == 2
                   and all(isinstance(c, str) and c for c in children)
                   and len(storages) == 2 and {s.get("entity") for s in storages} == set(children)
                   and all("pending_blueprint" in s and s["pending_blueprint"] is None
                           and pipeline.exact_json(s.get("stockpile"), dict(resource=None, capacity=10))
                           for s in storages), "Supply Full committed companions differ")
    for storage in storages:
        forward = [i.get("entity") for i in ctx.get("items", []) if i.get("stored_in") == storage["entity"]]
        inverse = storage.get("stored_items")
        native.require("stored_items" in storage and (inverse is None or isinstance(inverse, list)
                       and all(isinstance(i, str) for i in inverse) and len(inverse) == len(set(inverse)))
                       and len(forward) == len(set(forward)) and set(forward) == set(inverse or []),
                       "Supply Full companion inventory differs")
    holders = ctx.get("inventory_holders")
    native.require(isinstance(holders, list) and all(isinstance(h, dict)
                   and isinstance(h.get("entity"), str) and isinstance(h.get("carried_item"), str) for h in holders),
                   "Supply Full inventory holders missing")
    for bucket in buckets:
        if bucket.get("resource") != "BucketEmpty" or any(bucket.get(k) is not None
                for k in ("loaded_in", "delivering_to", "stored_by_mixer")):
            return False
        if not all(k in bucket for k in ("loaded_in", "delivering_to", "stored_by_mixer", "stored_in")):
            return False
        own = [s for s in storages if s.get("entity") == bucket["stored_in"]]
        if len(own) != 1 or own[0].get("pending_blueprint") is not None:
            return False
        inverse = own[0].get("stored_items")
        if not isinstance(inverse, list) or inverse.count(bucket["entity"]) != 1:
            return False
        if any(h["carried_item"] == bucket["entity"] for h in holders):
            return False
    return True


def check_full_setup(raw, actions, owner, children):
    paused, normal = actions[1:3]
    start, end = (tank.context(paused[k])["real_seconds"] for k in ("before", "after"))
    released = [e for e in raw["events"] if e.get("producer") == "MouseButtonInput"
                and e.get("state") == "Released" and start < e["real_seconds"] <= end]
    native.require(len(released) == 1, "Supply Full pause receipt missing")
    first = released[0]["sample_index"]
    until = tank.context(normal["before"])["real_seconds"]
    samples = [s for s in raw["samples"][first:] if s["real_seconds"] <= until]
    native.require(samples and until - samples[-1]["real_seconds"] <= 1
                   and all(s.get("paused") is True and full_setup_ready(s, owner, children) for s in samples),
                   "Supply Full setup not held before Mixer placement")


def check_world_action(value, baseline):
    ctx = context(value)
    native.require(all(pipeline.exact_json(ctx[k], baseline[k]) for k in
                       ("anchor", "footprint", "draw_pos", "primary_window", "world_epoch"))
                   and ctx.get("play_mode") == "BuildingPlace" and ctx.get("build_kind") == "MudMixer"
                   and ctx.get("cursor_grid") == baseline["anchor"] and ctx.get("companion_pending") is None
                   and value.get("pointer_over_ui") is False and value.get("world_input_captured") is False
                   and ctx["legal"] is True and ctx["blueprints"] == ctx["buildings"] == [],
                   "Supply Mixer target changed; no press")


def place_mixer(driver, baseline):
    point = context(driver.last)["point"]
    native.require(isinstance(point, list) and len(point) == 2
                   and all(planning.finite(axis) and 0 < axis < limit
                           for axis, limit in zip(point, driver.last["viewport"], strict=True)),
                   "Supply Mixer outside client")
    driver.send(point=tuple(round(axis * driver.last["scale_factor"]) for axis in point))
    before, first = copy.deepcopy(driver.last), len(driver.events)
    check_world_action(before, baseline)
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)

    def flushed(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "Supply raw flush failed")
        rows = [e for e in raw.get("events", []) if e.get("producer") == "MouseButtonInput"
                and tank.context(before)["real_seconds"] < e.get("real_seconds", -1) <= tank.context(value)["real_seconds"]]
        return len(rows) == 2 and [e.get("state") for e in rows] == ["Pressed", "Released"]

    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["tank_completion_action"] = dict(key="mixer-placement", first_step=first + 1,
        last_step=len(driver.events), before=before, after=copy.deepcopy(driver.last))
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise(driver, contract):
    native.require(contract.get("case") == CASE and contract.get("scope") == SCOPE
                   and contract.get("supply_speed") == "Super" and contract.get("mixer_presentation") == "base-assets"
                   and contract.get("material_supply") == "ordinary-auto-gather"
                   and pipeline.exact_json(contract.get("worldgen"), WORLDGEN)
                   and contract.get("setup") == SETUP and type(contract.get("motion_cap")) is int
                   and contract["motion_cap"] == MOTION_CAP, "Supply exercise differs")
    baseline = copy.deepcopy(context(driver.last))
    tank.exercise(driver, contract)
    owner = tank.context(driver.last)["consumers"]["roots"][0]["owner"]
    prior_raw = native.read_json(driver.root / "production-cancellation.json")
    placements = [e for e in prior_raw.get("events", []) if e.get("producer") == "ordinary-building-placement-branch-v1"
                  and e.get("outcome", {}).get("kind") == "Tank"
                  and e["outcome"].get("status") == "CompanionCommitted"]
    native.require(len(placements) == 1, "Supply Full companion placement missing")
    children = placements[0]["outcome"]["companion_owners"]
    tank.capture_action(driver, "menu:SetTimeSpeed(Super)")

    def full(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == value["nonce"]
                       and raw.get("process_id") == value["pid"], "Supply Full setup raw failed")
        return bool(raw.get("samples")) and full_setup_ready(raw["samples"][-1], owner, children)

    driver.last = driver.wait(full, after=driver.last["frame"], timeout=180)
    tank.capture_action(driver, "menu:SetTimeSpeed(Paused)")
    for key in ("menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", "menu:SelectBuild(MudMixer)"):
        tank.capture_action(driver, key)
    place_mixer(driver, baseline)
    driver.last = driver.wait(lambda v: len(context(v)["blueprints"]) == 1, after=driver.last["frame"])
    tank.capture_action(driver, "Escape")
    tank.capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[3])
    tank.capture_action(driver, "menu:SetTimeSpeed(Super)")
    driver.last = driver.wait(lambda v: len(context(v)["buildings"]) == 1 and not context(v)["blueprints"],
                              after=driver.last["frame"], timeout=180)
    tank.capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[4])
    tank.capture_action(driver, "menu:SetTimeSpeed(Super)")

    def refined(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == value["nonce"]
                       and raw.get("process_id") == value["pid"], "Supply raw failed")
        owners = context(value)["buildings"]
        return len(owners) == 1 and any(e.get("operation") == "RefineCommit" for e in raw.get("events", []))

    driver.last = driver.wait(refined, after=driver.last["frame"], timeout=180)
    tank.capture_action(driver, "menu:SetTimeSpeed(Paused)")
    driver.capture(CHECKPOINTS[5])


def check_supply_chain(raw, tank_owner, mixer_owner, *, capture_time):
    """One complete selected leg. Other in-flight legs remain unaccepted.

    All raw branches still pass their strict post-commit validators. Selection
    cannot hide invalid/duplicate receipts and never calls a partial leg complete.
    """
    lifecycle.check_raw_transitions(raw)
    inflow.check_river_fill_chain(raw)
    events = raw["events"]
    refines = [e for e in events if e.get("operation") == "RefineCommit" and e.get("owner") == mixer_owner]
    native.require(refines, "Supply has no actual refinement")
    refine = min(refines, key=lambda e: (e["real_seconds"], e["event_index"]))
    native.require(planning.finite(capture_time) and refine["real_seconds"] <= capture_time
                   and raw["samples"][refine["sample_index"]]["real_seconds"] <= capture_time,
                   "Refinement occurred after its capture")

    def prior(operation, predicate, later):
        rows = [e for e in events if e.get("operation") == operation and predicate(e)
                and e["world_epoch"] == later["world_epoch"] and e["real_seconds"] < later["real_seconds"]
                and e["sample_index"] < later["sample_index"]]
        native.require(rows, f"Supply {operation} predecessor missing")
        return max(rows, key=lambda e: (e["real_seconds"], e["event_index"]))

    pour = prior("MixerPour", lambda e: e["mixer"] == mixer_owner and refine["water"] in e["outputs"], refine)
    take = prior("TankTake", lambda e: e["bucket"] == pour["bucket"], pour)
    native.require(take["owner"] == pour["owner"] == tank_owner and take["mixer"] == pour["mixer"] == mixer_owner
                   and all(take[k] == pour[k] for k in ("world_epoch", "worker", "assignment", "target", "bucket"))
                   and take["route_found"] is True and pour["returning"] is True
                   and len(take["inputs"]) == pour["amount"] == len(pour["outputs"]),
                   "Selected supply leg is foreign, failed or lossy")
    for item in take["inputs"]:
        origin = prior("TankPour", lambda e: e["owner"] == tank_owner and item in e["outputs"], take)
        prior("RiverFill", lambda e: all(e[k] == origin[k] for k in
              ("worker", "assignment", "target", "owner", "bucket")), origin)
    # A fresh ordinary Mixer has zero solid counters. Walk exact branch deltas
    # through the FIRST refinement only; later mud hauling legitimately changes
    # counters and is not inferred from a periodic sample.
    solids = [e for e in events if e.get("operation") == "MixerMaterial" and e.get("owner") == mixer_owner
              and e["real_seconds"] <= refine["real_seconds"]]
    native.require({e["resource"] for e in solids} >= {"Sand", "Rock"}, "Supply lacks actual Sand/Rock consumption")
    remaining, selected, state = list(solids), [], [0, 0, 0]
    previous_time, previous_sample = -1., -1
    while state != refine["before"]:
        next_rows = [e for e in remaining if e["before"] == state]
        native.require(len(next_rows) == 1, "Supply solid counter history is missing or ambiguous")
        event = next_rows[0]
        native.require(previous_time <= event["real_seconds"] and previous_sample <= event["sample_index"]
                       and (not selected or previous_sample != event["sample_index"]
                            or selected[-1]["event_index"] < event["event_index"]),
                       "Supply solid counter chronology is reversed")
        previous_time, previous_sample = event["real_seconds"], event["sample_index"]
        state = event["after"]
        selected.append(event)
        remaining.remove(event)
    native.require({e["resource"] for e in selected} >= {"Sand", "Rock"}
                   and not any(e["real_seconds"] < refine["real_seconds"] for e in remaining),
                   "Refinement lacks exact delivered solid counters")
    return dict(refine=refine["event_index"], tank_take=take["event_index"], mixer_pour=pour["event_index"],
                materials=[e["event_index"] for e in selected])


def check_tail_inputs(raw, entries, client, inputs):
    """Exact ordinary controls and one Mixer click, with no unclaimed inputs."""
    lower, upper = entries[2], entries[-1]
    tail = inputs[lower["last_step"]:]
    actions = [row["tank_completion_action"] for row in tail if "tank_completion_action" in row]
    expected = ["menu:SetTimeSpeed(Super)", "menu:SetTimeSpeed(Paused)",
                "menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", "menu:SelectBuild(MudMixer)",
                "mixer-placement", "Escape", "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Super)",
                "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Super)", "menu:SetTimeSpeed(Paused)"]
    native.require([a.get("key") for a in actions] == expected, "Supply input sequence differs")
    planning.check_transport(inputs, client, upper["value"]["world_epoch"], motion_cap=MOTION_CAP)
    claimed, raw_claimed, effects = set(), set(), []
    for action in actions:
        before, after, key = action["before"], action["after"], action["key"]
        for value in (before, after):
            check_worldgen(value)
        first, last = action["first_step"], action["last_step"]
        native.require(type(first) is int and type(last) is int and last == first + 1
                       and lower["last_step"] < first < last <= upper["last_step"] == len(inputs)
                       and not claimed.intersection((first, last)) and inputs[first-1].get("tank_completion_action") == action,
                       "Supply sent pair missing or reused")
        claimed.update((first, last))
        start, end = (tank.context(v)["real_seconds"] for v in (before, after))
        native.require(planning.finite(start) and planning.finite(end) and start < end
                       and before["frame"] < after["frame"] and all(
                           type(v.get(k)) is int and v[k] == upper["value"][k]
                           for v in (before, after) for k in ("pid", "world_epoch", "primary_window"))
                       and all(v.get("nonce") == client["nonce"] for v in (before, after)), "Supply action owner/clock differs")
        keyboard = key == "Escape"
        sent = inputs[first-1:last]
        native.require([r.get("pressed") for r in sent] == [True, False]
                       and all(type(r.get("pressed")) is bool and before["frame"] <= r["pre_frame"] < r["post_frame"] <= after["frame"]
                               and (r.get("key") == "Escape" and r.get("button") is None if keyboard else
                                    type(r.get("button")) is int and r["button"] == 1 and r.get("key") is None) for r in sent),
                       "Supply input pair differs")
        rows = [e for e in raw["events"] if e.get("producer") in ("KeyboardInput", "MouseButtonInput")
                and start < e.get("real_seconds", -1) <= end]
        native.require(len(rows) == 2 and [e.get("state") for e in rows] == ["Pressed", "Released"], "Supply raw input missing")
        for event in rows:
            sample = raw["samples"][event["sample_index"]]
            native.require(event["event_index"] not in raw_claimed and event["window"] == sample.get("primary_window")
                           and event["real_seconds"] <= sample["real_seconds"]
                           and (event.get("producer") == "KeyboardInput" and event.get("key") == "Escape" and event.get("repeat") is False
                                if keyboard else event.get("producer") == "MouseButtonInput" and event.get("button") == "Left"),
                           "Supply raw input ownership differs")
            raw_claimed.add(event["event_index"])
            if not keyboard:
                cursor, dpi = sample.get("cursor_position"), sample.get("dpi")
                native.require(isinstance(cursor, list) and len(cursor) == 2 and planning.finite(dpi) and dpi > 0
                               and dpi == before.get("scale_factor")
                               and all(planning.finite(a) and abs(a*dpi-b) <= 2 for a,b in zip(cursor, before["cursor"], strict=True)),
                               "Supply raw cursor differs")
        if key == "mixer-placement":
            check_world_action(before, context(lower["value"]))
        elif not keyboard:
            control = before["controls"].get(key, {})
            native.require(control.get("fully_visible") is True and control.get("interaction") == "Hovered"
                           and sent[0].get("observed_hover", {}).get(key) == "Pressed", "Supply control target differs")
        if key.startswith("menu:SetTimeSpeed("):
            paused, speed = key.endswith("(Paused)"), 4 if key.endswith("(Super)") else 1
            native.require(after.get("paused") is paused, "Supply time control not applied")
            if paused:
                speed = effects[-1][2] if effects else 1
            effects.append((rows[-1]["sample_index"], paused, speed))
    native.require(all(int(row["step"]) in claimed for row in tail if row.get("key") is not None or row.get("button") is not None)
                   and raw_claimed == {e["event_index"] for e in raw["events"]
                       if e.get("producer") in ("KeyboardInput", "MouseButtonInput")
                       and e["real_seconds"] > tank.context(lower["value"])["real_seconds"]}, "Supply unclaimed input")
    for lo, hi, group in ((entries[2], entries[3], expected[:8]), (entries[3], entries[4], expected[8:10]),
                          (entries[4], entries[5], expected[10:])):
        selected = [a for a in actions if lo["last_step"] < a["first_step"] <= hi["last_step"]]
        native.require([a["key"] for a in selected] == group and all(a["last_step"] <= hi["last_step"]
                       and lo["value"]["frame"] <= a["before"]["frame"] < a["after"]["frame"] < hi["value"]["frame"]
                       and tank.context(lo["value"])["real_seconds"] <= tank.context(a["before"])["real_seconds"]
                       < tank.context(a["after"])["real_seconds"] < tank.context(hi["value"])["real_seconds"] for a in selected),
                       "Supply action crossed checkpoint")
    for sample in raw["samples"]:
        if sample["real_seconds"] < tank.context(lower["value"])["real_seconds"]:
            continue
        _, paused, speed = next((e for e in reversed(effects) if e[0] <= sample["sample_index"]), (-1, True, 1))
        native.require(sample.get("paused") is paused and planning.finite(sample.get("relative_speed"))
                       and sample["relative_speed"] == speed, "Supply simulation phase differs")
    return actions


def verify(raw, entries, client, inputs, contract):
    check_event_contract(raw, contract)
    native.require(contract.get("case") == CASE and contract.get("scope") == raw.get("scope") == SCOPE
                   and contract.get("mixer_presentation") == "base-assets" and contract.get("material_supply") == "ordinary-auto-gather"
                   and contract.get("supply_speed") == "Super" and [e["case"] for e in entries] == list(CHECKPOINTS),
                   "Supply campaign scope differs")
    native.require(pipeline.exact_json(contract.get("worldgen"), WORLDGEN), "Supply world contract differs")
    native.require(contract.get("setup") == SETUP and type(contract.get("motion_cap")) is int
                   and contract["motion_cap"] == MOTION_CAP, "Supply setup/input budget differs")
    samples, events = raw["samples"], raw["events"]
    epoch, window = entries[0]["value"]["world_epoch"], entries[0]["value"]["primary_window"]
    native.require(0 < len(samples) <= 1200 and len(events) < 4096
                   and all(type(s.get("sample_index")) is int and s["sample_index"] == i
                       and type(s.get("world_epoch")) is int and s["world_epoch"] == epoch
                       and type(s.get("primary_window_bits")) is int and s["primary_window_bits"] == window
                       and planning.finite(s.get("real_seconds")) and planning.finite(s.get("virtual_seconds"))
                       and s["real_seconds"] >= 0 and s["virtual_seconds"] >= 0 for i,s in enumerate(samples))
                   and all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                       and (not (a.get("paused") is True and b.get("paused") is True) or a["virtual_seconds"] == b["virtual_seconds"])
                       for a,b in zip(samples,samples[1:]))
                   and all(type(e.get("event_index")) is int and e["event_index"] == i
                       and type(e.get("sample_index")) is int and 0 <= e["sample_index"] < len(samples) for i,e in enumerate(events)),
                   "Supply raw envelope differs")
    for entry in entries:
        value = entry["value"]
        check_observation(entry["case"], value)
        native.require(value.get("nonce") == client["nonce"] and all(type(value.get(k)) is int and value[k] == expected
                       for k,expected in (("pid",client["pid"]),("world_epoch",epoch),("primary_window",window)))
                       and any(abs(s["real_seconds"]-tank.context(value)["real_seconds"]) <= 1
                           and s["virtual_seconds"] == tank.context(value)["virtual_seconds"] and s.get("paused") is True for s in samples),
                       "Supply checkpoint lacks exact paused subject")
    native.require(all(a["last_step"] < b["last_step"] and a["value"]["frame"] < b["value"]["frame"]
                       for a,b in zip(entries,entries[1:])), "Supply checkpoint order differs")
    actions = check_tail_inputs(raw, entries, client, inputs)
    cutoff = max(s["sample_index"] for s in samples if s["real_seconds"] <= tank.context(actions[0]["before"])["real_seconds"])
    prefix = dict(raw, samples=samples[:cutoff+1], events=[e for e in events if e["sample_index"] <= cutoff])
    tank.verify(prefix, entries[:3], client, inputs[:entries[2]["last_step"]], contract, profile=(CASE,SCOPE))
    mixer.check_mixer_domain_completion(raw, entries[3]["value"], entries[4]["value"],
                                      context(entries[3]["value"]), context(entries[4]["value"]))
    placement = [e for e in events if e.get("producer") == "ordinary-building-placement-branch-v1"
                 and e.get("outcome",{}).get("kind") == "MudMixer"]
    native.require(len(placement) == 1 and sum(e.get("operation") == "BuildingCompleted" for e in events) == 2,
                   "Supply extra placement/completion")
    placed = placement[0]
    lifecycle.check_placement_receipt(placed, samples)
    bp = context(entries[3]["value"])["blueprints"][0]
    baseline = context(entries[0]["value"])
    action = next(a for a in actions if a["key"] == "mixer-placement")
    presses = [e for e in events if e.get("producer") == "MouseButtonInput" and e.get("state") == "Pressed"
               and tank.context(action["before"])["real_seconds"] < e["real_seconds"] <= tank.context(action["after"])["real_seconds"]]
    native.require(placed["outcome"] == dict(kind="MudMixer", grid=baseline["anchor"], status="BlueprintCommitted",
                       owner=bp["debug_entity"], footprint=baseline["footprint"])
                   and len(presses) == 1 and presses[0]["sample_index"] <= placed["sample_index"]
                   and presses[0]["real_seconds"] <= placed["real_seconds"]
                   and tank.context(action["before"])["real_seconds"] < placed["real_seconds"] <= tank.context(action["after"])["real_seconds"],
                   "Supply Mixer placement not bound to exact click")
    complete = next(e for e in events if e.get("operation") == "BuildingCompleted" and e.get("kind") == "MudMixer")
    tank_complete = next(e for e in events if e.get("operation") == "BuildingCompleted" and e.get("kind") == "Tank")
    initial_typed = samples[tank_complete["sample_index"]]["m2_context"]
    bucket_ids = {e["entity"] for e in initial_typed["items"] if e.get("belongs_to") == tank_complete["owner"]}
    tank_placements = [e for e in events if e.get("producer") == "ordinary-building-placement-branch-v1"
                       and e.get("outcome",{}).get("kind") == "Tank"]
    native.require(len(tank_placements) == 2 and len(bucket_ids) == 5, "Supply Tank companions/buckets missing")
    children = tank_placements[1]["outcome"]["companion_owners"]
    check_full_setup(raw, actions, tank_complete["owner"], children)
    initial_children = {e["entity"]:e for e in samples[tank_complete["sample_index"]]["companion_context"]["storages"]
                        if e["entity"] in children}
    for sample in samples:
        index = sample["sample_index"]
        bps = [e for e in sample["blueprints"] if e.get("kind") == "MudMixer"]
        owners = [e for e in sample["owners"] if e.get("kind") == "MudMixer"]
        if index < placed["sample_index"]:
            native.require(not bps and not owners, "Mixer existed before normal click")
        else:
            owner = bp["debug_entity"] if index < complete["sample_index"] else complete["owner"]
            native.require((len(bps) == 1 and bps[0].get("entity") == owner and not owners) if index < complete["sample_index"] else
                           (not bps and len(owners) == 1 and owners[0].get("entity") == owner), "Supply Mixer replaced")
            layers = [e for e in sample["owner_layers"] if e.get("entity") == owner]
            native.require(len(layers) == 1 and layers[0].get("exists") is True
                           and sorted(layers[0].get("building_grids",[])) == sorted(baseline["footprint"])
                           and all(layers[0].get(k) == [] for k in ("floor_grids","door_grids","bridge_grids","stockpile_grids")),
                           "Supply Mixer map changed")
        if index >= tank_complete["sample_index"]:
            tanks = [e for e in sample["owners"] if e.get("kind") == "Tank"]
            native.require(len(tanks) == 1 and tanks[0].get("entity") == tank_complete["owner"], "Supply Tank replaced")
            layers = [e for e in sample["owner_layers"] if e.get("entity") == tank_complete["owner"]]
            native.require(len(layers) == 1 and layers[0].get("exists") is True
                           and sorted(layers[0].get("building_grids",[])) == sorted(tank.context(entries[0]["value"])["footprint"])
                           and all(layers[0].get(k) == [] for k in ("floor_grids","door_grids","bridge_grids","stockpile_grids")),
                           "Supply Tank footprint changed")
            typed = sample.get("m2_context",{})
            inventory = [e for e in typed.get("tanks",[]) if e.get("entity") == tank_complete["owner"]]
            native.require(len(inventory) == 1 and inventory[0].get("provisional") is False
                           and inventory[0].get("stockpile") == dict(resource="Water",capacity=50), "Supply Tank inventory lost")
            water.check_inventory(typed,inventory[0])
            buckets = [e for e in typed.get("items",[]) if e.get("belongs_to") == tank_complete["owner"]]
            native.require(len(buckets) == 5 and {e.get("entity") for e in buckets} == bucket_ids
                           and all(e.get("resource") in ("BucketEmpty","BucketWater") for e in buckets), "Supply dedicated buckets changed")
            actual_children = {e["entity"]:e for e in sample.get("companion_context",{}).get("storages",[]) if e["entity"] in children}
            native.require(pipeline.exact_json(actual_children,initial_children), "Supply companion ownership/map changed")
            feedback = sample.get("art_feedback",{})
            roots = feedback.get("roots",[])
            native.require(pipeline.exact_json(feedback.get("requested"),contract["identity"])
                           and pipeline.exact_json(feedback.get("active"),contract["identity"]) and feedback.get("pending") is None
                           and len(roots) == 1 and roots[0].get("owner") == tank_complete["owner"]
                           and pipeline.exact_json(roots[0].get("identity"),contract["identity"])
                           and roots[0].get("owner_pose_matches") is True, "Supply Tank consumer replaced")
    check_supply_chain(raw, tank_complete["owner"], complete["owner"],
                       capture_time=tank.context(entries[-1]["value"])["real_seconds"])
    final = entries[-1]["value"]
    roots = tank.context(final).get("consumers",{}).get("roots",[])
    native.require(len(roots) == 1 and type(roots[0].get("tank_water_count")) is int
                   and 0 <= roots[0]["tank_water_count"] <= 50, "Supply final Tank inventory missing")
    tank.check_completed_consumers(raw, final, contract, water_count=roots[0]["tank_water_count"])


def check_event_contract(raw, contract):
    """Supply is not a Dream campaign; the exclusion must be declared, never inferred."""
    native.require(raw.get("scope") == contract.get("scope") == SCOPE
                   and pipeline.exact_json(raw.get("event_contract"), EVENT_CONTRACT)
                   and pipeline.exact_json(contract.get("event_contract"), EVENT_CONTRACT)
                   and all(e.get("producer") != "DreamTransferredVisualMessage"
                           and e.get("operation") != "DreamTransferred" for e in raw.get("events", [])),
                   "Supply scope/event subscription differs")
