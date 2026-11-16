"""Seeded occupied Spa: real UI slot edits and worker non-eviction."""
from __future__ import annotations

import native_acceptance as native
import ui_spa_planning as spa
import math

CASE = "refactor-spa-occupied-slots"
CHECKPOINTS = ("refactor-spa-baseline", "refactor-spa-closed", "refactor-spa-held")


def settled(state):
    sites = state.get("sites") or []
    scale = sites[0].get("pose", {}).get("scale") if len(sites) == 1 else None
    return (isinstance(scale, list) and len(scale) == 3
            and all(type(n) in (int, float) and math.isfinite(n) and abs(n - 1.0) < .0001 for n in scale))


def check_observation(case, value):
    native.require(case in CHECKPOINTS and value.get("paused") is True, "seeded Spa checkpoint differs")
    state = spa.context(value)
    spa.check_operational(state, value["spa_planning"], active_slots=4 if case == CHECKPOINTS[0] else 0)
    native.require(settled(state), "seeded Spa initial construction animation has not settled")
    workers = spa.safe_slot_workers(state)
    seed = value["spa_planning"].get("seeded") or {}
    native.require(len(workers) == 1 and type(seed.get("owner")) is int
        and type(seed.get("worker")) is int and type(seed.get("tile")) is int
        and state["sites"][0]["entity_bits"] == seed["owner"]
        and any(s.get("entity_bits") == seed["worker"] and s["entity"] in workers for s in state["souls"])
        and any(t.get("entity_bits") == seed["tile"] and t["entity"] == next(iter(workers.values()))["generate_power"]["tile"]
                for t in state["tiles"]), "seeded Spa worker identity differs")
    if case == CHECKPOINTS[0]:
        spa.check_selection_target(value)
        native.require(value["spa_planning"]["selection_target"]["rank"] == 0, "Spa direct target is ambiguous")


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        spa.click(driver, "dev-minimize")
    spa.pause(driver, False)
    def operating(value):
        try:
            spa.check_operational(spa.context(value), value["spa_planning"])
            return settled(spa.context(value)) and bool(spa.safe_slot_workers(spa.context(value)))
        except native.AcceptanceError:
            return False
    driver.last = driver.wait(operating, after=driver.last["frame"], timeout=30)
    spa.pause(driver, True)
    def ready(value):
        try:
            check_observation(CHECKPOINTS[0], value)
            return True
        except native.AcceptanceError:
            return False
    driver.last = driver.wait(ready, after=driver.last["frame"], timeout=30)
    driver.capture(CHECKPOINTS[0])
    owner = spa.context(driver.last)["sites"][0]
    spa.world_click(driver, "select-operational", "selection_point")
    driver.last = driver.wait(lambda v: v.get("selected") == owner["entity_bits"]
                              and v.get("inspection_target") == owner["entity_bits"], after=driver.last["frame"])
    spa.pause(driver, False)
    for count in (3, 2, 1, 0):
        key = spa.slots_key(owner["entity"], count)
        driver.last = driver.wait(lambda v: key in v["controls"], after=driver.last["frame"], timeout=10)
        spa.click(driver, key)
        driver.last = driver.wait(lambda v: spa.context(v)["sites"][0]["active_slots"] == count,
                                  after=driver.last["frame"], timeout=10)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[1])
    start = driver.last["spa_planning"]["virtual_seconds"]
    spa.pause(driver, False)
    driver.last = driver.wait(lambda v: v["spa_planning"]["virtual_seconds"] >= start + 3,
                              after=driver.last["frame"], timeout=15)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[2])
    spa.wait_final_raw(driver)


def verify(raw, entries, client, inputs):
    native.require(client.get("input_ack_policy") == "next-exact-observation-v1"
        and [e["case"] for e in entries] == list(CHECKPOINTS), "seeded Spa contract differs")
    spa.check_raw_session(raw, client)
    snapshots = [spa.raw_checkpoint(raw, e["value"]) for e in entries]
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
        and all(a["last_step"] < b["last_step"] and a["value"]["frame"] < b["value"]["frame"]
                for a, b in zip(entries, entries[1:])), "seeded Spa checkpoint order differs")
    for entry in entries:
        check_observation(entry["case"], entry["value"])
        native.require(entry["value"]["nonce"] == client["nonce"] and entry["value"]["pid"] == client["pid"],
                       "seeded Spa client differs")
    initial = entries[0]["value"]
    baseline = spa.context(initial)
    native.require(all(e["value"]["spa_planning"].get("seeded") == initial["spa_planning"]["seeded"] for e in entries),
                   "seeded Spa identities changed")
    owner = baseline["sites"][0]
    workers = spa.safe_slot_workers(baseline)
    native.require(len(workers) == 1, "seeded Spa requires exactly one initial worker")
    actions = spa.check_action_inventory(raw, entries, client, inputs)
    prefix = [a["key"] for a in actions if a["last_step"] <= entries[0]["last_step"]]
    native.require(prefix in (["menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"],
        ["dev-minimize", "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"]),
        "seeded Spa initial simulation inputs differ")
    relevant = [a for a in actions if a["first_step"] > entries[0]["last_step"]]
    keys = [a["key"] for a in relevant]
    native.require(keys == ["select-operational", "menu:SetTimeSpeed(Normal)",
        *(spa.slots_key(owner["entity"], n) for n in (3, 2, 1, 0)), "menu:SetTimeSpeed(Paused)",
        "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"], "seeded Spa action sequence differs")
    selection = relevant[0]
    point = spa.check_selection_target(selection["before"])
    native.require(selection["before"]["spa_planning"]["selection_target"]["rank"] == 0
        and selection["before"]["selected"] != owner["entity_bits"]
        and selection["after"]["selected"] == owner["entity_bits"]
        and selection["after"]["inspection_target"] == owner["entity_bits"]
        and not selection["before"]["pointer_over_ui"] and not selection["before"]["world_input_captured"]
        and all(abs(a-b*selection["before"]["scale_factor"]) <= 2 for a, b in zip(selection["before"]["cursor"], point, strict=True)),
        "seeded Spa actual selection differs")
    epoch = initial["world_epoch"]
    receipts = spa.check_slot_receipts(raw, relevant[2:6], owner["entity"], epoch)
    for action in relevant[2:6]:
        control = action["before"].get("control") or {}
        pressed = inputs[action["first_step"] - 1]
        native.require(action["before"].get("inspection_target") == owner["entity_bits"]
            and action["before"].get("selected") == owner["entity_bits"]
            and type(control.get("entity")) is int
            and pressed.get("observed_hover_entities", {}).get(action["key"]) == control["entity"],
            "seeded Spa pressed button identity/target differs")
    for index, action in enumerate(relevant):
        lower, upper = entries[(0 if index < 7 else 1):(2 if index < 7 else 3)]
        native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
            and lower["value"]["frame"] <= action["before"]["frame"] < action["after"]["frame"] < upper["value"]["frame"]
            and action["before"]["world_epoch"] == action["after"]["world_epoch"] == epoch,
            "seeded Spa input outside checkpoint interval")
    tile_keys = ("entity", "entity_bits", "parent_site", "parent_exists", "grid", "pose", "map")
    def shape(state):
        return sorted([{k: tile.get(k) for k in tile_keys} for tile in state["tiles"]], key=lambda tile: tile["entity"])
    interval = raw["samples"][snapshots[0]["sample_index"]:snapshots[-1]["sample_index"] + 1]
    windows = [(spa.pressed_for(raw, start)["real_seconds"], stop["after"]["spa_planning"]["real_seconds"])
               for start, stop in ((relevant[1], relevant[6]), (relevant[7], relevant[8]))]
    closed_assignments = None
    for sample in interval:
        state = spa.context({"spa_planning": {"context": sample["spa_context"]}})
        count = 4 - sum(r["sample_index"] <= sample["sample_index"] for r in receipts)
        spa.check_operational(state, initial["spa_planning"], active_slots=count)
        current = spa.generating(state)
        native.require(sample["world_epoch"] == epoch and sample["primary_window"] == snapshots[0]["primary_window"]
            and sample.get("primary_window_bits") == snapshots[0]["primary_window_bits"]
            and state.get("relative_speed") == 1 and type(sample.get("paused")) is bool
            and (sample["paused"] or any(start <= sample["real_seconds"] <= end for start, end in windows))
            and all(state["sites"][0][k] == owner[k] for k in ("entity", "entity_bits", "pose", "grid"))
            and shape(state) == shape(baseline) and all(worker in current
                and current[worker]["generate_power"]["tile"] == soul["generate_power"]["tile"] for worker, soul in workers.items()),
            "seeded Spa worker or owner changed")
        if count == 0:
            assignments = spa.spa_assignment_keys(state)
            if closed_assignments is None:
                closed_assignments = assignments
            native.require(assignments == closed_assignments, "seeded Spa admitted a worker after closure")
    native.require(receipts[-1]["sample_index"] < snapshots[1]["sample_index"]
        and 3 <= snapshots[2]["virtual_seconds"] - snapshots[1]["virtual_seconds"] <= 15
        and all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                and (not (a["paused"] and b["paused"]) or a["virtual_seconds"] == b["virtual_seconds"])
                for a, b in zip(interval, interval[1:])), "seeded Spa hold/clock differs")
