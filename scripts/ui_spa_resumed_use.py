"""Ordinary post-load candidate reassignment and observed Dream-floor departure."""
import math

from scripts import ui_spa_candidate_art as art

spa, occupied, native = art.spa, art.occupied, art.native
CASE, SCOPE = art.RESUMED_CASE, art.RESUMED_SCOPE
CHECKPOINTS = occupied.CHECKPOINTS + ("soul-spa-resumed", "soul-spa-resumed-used", "soul-spa-departed")


def check_observation(case, value):
    native.require(case in CHECKPOINTS[-3:] and value.get("paused") is True,
                   "Spa resumed capture differs")
    spa.check_operational(spa.context(value), value["spa_planning"])


def structure(state):
    result = occupied.durable(state)
    for site in result["sites"]:
        # Derived production is rebuilt by normal Logic, not persisted authority.
        site["generator"].pop("current_output")
    for grid in result["grids"]:
        for key in ("generation", "powered"):
            grid.pop(key)
    return result


def resumed_workers(saved, loaded, receipts):
    mapping = next(row for row in receipts if row.get("operation") == "DurableLoad")["entity_remap"]
    return occupied.mapped_souls(saved, loaded, mapping)


def dream_step(before, after, virtual_before, virtual_after):
    if not all(spa.finite(value) for value in (before, after, virtual_before, virtual_after)):
        return False
    if virtual_after < virtual_before or after > before:
        return False
    if virtual_after == virtual_before:
        return before == after
    return abs((before - after) - .5 * (virtual_after - virtual_before)) <= .15


def verify(raw, entries, client, save_path, inputs, contract):
    native.require(contract.get("case") == CASE and contract.get("scope") == SCOPE
                   and [entry["case"] for entry in entries] == list(CHECKPOINTS), "Spa resumed case/sequence differs")
    spa.check_raw_session(raw, client, raw_scope=SCOPE)
    native.require([row.get("operation") for row in raw["events"]
                    if row.get("operation") in ("DurableSave", "DurableLoad")] == ["DurableSave", "DurableLoad"]
                   and not any(row.get("operation") in ("SoulSpaConstructionCancel", "SoulSpaSlotsChange")
                               for row in raw["events"]), "Spa resumed foreign domain operation")
    loaded_sample = spa.raw_checkpoint(raw, entries[6]["value"])
    cutoff = loaded_sample["sample_index"]
    prefix = dict(raw, samples=raw["samples"][:cutoff + 1],
                  events=[row for row in raw["events"] if row["sample_index"] <= cutoff])
    occupied.verify(prefix, entries[:7], client, save_path, inputs[:entries[6]["last_step"]], raw_scope=SCOPE)
    spa.check_inputs(raw, entries, client, inputs, completion=True, usage=True, occupied=True, resumed=True)
    art.check_consumers(raw, entries, contract)
    verify_tail(raw, entries, client, inputs, spa.context(entries[5]["value"]), loaded_index=6)


def verify_tail(raw, entries, client, inputs, saved, *, loaded_index, speeds=(4, 4, 4), natural_service=False):
    """Only the current Load and its continuation, never historical construction."""
    values = [entry["value"] for entry in entries]
    loaded = spa.context(values[loaded_index])
    loaded_sample = spa.raw_checkpoint(raw, values[loaded_index])
    cutoff = loaded_sample["sample_index"]
    lookup = resumed_workers(saved, loaded, raw["events"])
    former = {lookup[key] for key in spa.generating(saved)}
    snapshots = [spa.raw_checkpoint(raw, value) for value in values]
    later = [spa.context(value) for value in values[loaded_index + 1:]]
    tracked = sorted(former.intersection(spa.measurement_workers(later[0])))
    native.require(tracked, "No formerly occupied remapped Soul resumed the measured work")
    # Pick by the shared resumed identity, not by a later successful outcome.
    worker = tracked[0]
    native.require(worker in spa.generating(later[1]), "Selected resumed worker did not remain generating")
    start, measured = (spa.generating(state)[worker] for state in later[:2])
    elapsed = values[-2]["spa_planning"]["virtual_seconds"] - values[-3]["spa_planning"]["virtual_seconds"]
    native.require(3 <= elapsed <= 15 and start["generate_power"]["tile"] == measured["generate_power"]["tile"]
                   and start["working_on"] == measured["working_on"]
                   and math.isclose(start["dream"] - measured["dream"], .5 * elapsed, abs_tol=.15),
                   "Spa resumed Dream/time/task consumption differs")
    for entry in entries[loaded_index + 1:]:
        value = entry["value"]
        check_observation(entry["case"], value)
        native.require(value["nonce"] == client["nonce"] and value["pid"] == client["pid"]
                       and type(entry.get("last_step")) is int
                       and entry["last_step"] == sum(row["post_frame"] < value["frame"] for row in inputs)
                       and value["world_epoch"] == values[loaded_index]["world_epoch"]
                       and all(value["spa_planning"][key] == values[loaded_index]["spa_planning"][key]
                               for key in ("anchor", "footprint", "draw_pos")), "Spa resumed subject/position differs")
    baseline = structure(loaded)
    tail = raw["samples"][cutoff:snapshots[-1]["sample_index"] + 1]
    native.require(all(a["sample_index"] < b["sample_index"] and a["real_seconds"] < b["real_seconds"]
                       and 0 <= b["virtual_seconds"] - a["virtual_seconds"]
                       <= 4 * (b["real_seconds"] - a["real_seconds"]) + .001
                       and (a["spa_context"].get("relative_speed") != 1
                            or b["spa_context"].get("relative_speed") != 1
                            or b["virtual_seconds"] - a["virtual_seconds"] <= b["real_seconds"] - a["real_seconds"] + .001)
                       and (not (a["paused"] and b["paused"]) or a["virtual_seconds"] == b["virtual_seconds"])
                       for a, b in zip(tail, tail[1:])),
                   "Spa resumed raw order differs")
    actions = [row["spa_action"] for row in inputs if row.get("spa_action", {}).get("first_step", 0) > entries[loaded_index]["last_step"]]
    native.require(len(actions) == 6, "Spa continuation requires three bounded input intervals")
    intervals = [(spa.pressed_for(raw, a)["real_seconds"], b["after"]["spa_planning"]["real_seconds"], speed)
                 for a, b, speed in zip(actions[::2], actions[1::2], speeds, strict=True)]
    for sample in tail:
        state = sample["spa_context"]
        native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == values[loaded_index]["world_epoch"]
                       and sample["primary_window"] == loaded_sample["primary_window"]
                       and type(sample.get("primary_window_bits")) is int
                       and sample["primary_window_bits"] == loaded_sample["primary_window_bits"]
                       and spa.finite(sample.get("virtual_seconds"))
                       and type(sample.get("paused")) is bool
                       and (sample["paused"] or any(a <= sample["real_seconds"] <= b and state.get("relative_speed") == speed
                                                      for a, b, speed in intervals))
                       and occupied.exact(structure(state), baseline), "Spa resumed structure/epoch/window changed")
        expected = {(worker, tile["entity"]) for tile in state["tiles"] for worker in tile.get("workers") or []}
        targets = {tile["entity"] for tile in state["tiles"]}
        observed = {(row["entity"], row["target"]) for row in state["working_relations"] if row["target"] in targets}
        souls = {soul["entity"]: soul for soul in state["souls"]}
        native.require(expected == observed and all(worker in souls
                       and souls[worker].get("working_on") == tile
                       and isinstance(souls[worker].get("generate_power"), dict)
                       and souls[worker]["generate_power"].get("tile") == tile
                       and souls[worker]["generate_power"].get("phase") in ("GoingToTile", "Generating")
                       for worker, tile in expected), "Spa resumed inverse worker relations differ")
    measured_interval = raw["samples"][snapshots[-3]["sample_index"]:snapshots[-2]["sample_index"] + 1]
    native.require(all(worker in spa.generating(row["spa_context"])
                       and spa.generating(row["spa_context"])[worker]["generate_power"]["tile"] == start["generate_power"]["tile"]
                       for row in measured_interval), "Spa resumed measured Soul/task changed")
    measured_souls = [spa.generating(row["spa_context"])[worker] for row in measured_interval]
    native.require(any(not row["paused"] for row in measured_interval)
                   and all(spa.finite(soul.get("dream")) and soul.get("working_on") == start["working_on"]
                           for soul in measured_souls)
                   and all(dream_step(a["dream"], b["dream"], left["virtual_seconds"], right["virtual_seconds"])
                           for a, b, left, right in zip(measured_souls, measured_souls[1:], measured_interval, measured_interval[1:])),
                   "Spa resumed per-sample Dream consumption differs")
    ending = {s["entity"]: s for s in later[-1]["souls"]}[worker]
    native.require(ending.get("generate_power") is None and ending.get("working_on") is None
                   and all(worker not in (tile.get("workers") or []) for tile in later[-1]["tiles"])
                   and not any(row["entity"] == worker and row.get("target_is_spa_tile")
                               for row in later[-1]["working_relations"]), "Spa departure did not release worker relations")
    departures = []
    for before, after in zip(tail, tail[1:]):
        previous = spa.generating(before["spa_context"]).get(worker)
        following = next((s for s in after["spa_context"]["souls"] if s["entity"] == worker), None)
        if previous and following and following.get("generate_power") is None:
            departures.append((previous, before, after))
    native.require(len(departures) == 1
                   and snapshots[-2]["sample_index"] < departures[0][2]["sample_index"] <= snapshots[-1]["sample_index"]
                   and not departures[0][1]["paused"] and not departures[0][2]["paused"],
                   "Spa departure has no unique observed unpaused transition")
    previous, before, after = departures[0]
    if natural_service and previous["dream"] > 10.5:
        from scripts import ui_spa_service_departure
        ui_spa_service_departure.verify(raw, worker, previous, before, after)
    else:
        native.require(0 <= previous["dream"] <= 10.5,
                       "Spa departure has no observed unpaused Dream-floor transition")


def exercise(driver):
    occupied.exercise(driver)
    saved = spa.context(driver.checkpoints[5]["value"])
    loaded = spa.context(driver.last)
    # Actual save/load receipt maps bind which former occupant we wait for.
    raw = native.read_json(driver.root / "production-cancellation.json")
    lookup = resumed_workers(saved, loaded, raw["events"])
    former = {lookup[key] for key in spa.generating(saved)}
    exercise_tail(driver, former)


def exercise_tail(driver, former, *, normal_measurement=False):
    spa.click(driver, "menu:SetTimeSpeed(Super)")
    driver.last = driver.wait(lambda value: bool(former.intersection(spa.measurement_workers(spa.context(value)))),
                              after=driver.last["frame"], timeout=120)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[7])
    workers = former.intersection(spa.measurement_workers(spa.context(driver.last)))
    native.require(workers, "Former Spa occupant disappeared before resumed capture")
    worker = sorted(workers)[0]
    start = driver.last["spa_planning"]["virtual_seconds"]
    spa.click(driver, "menu:SetTimeSpeed(Normal)" if normal_measurement else "menu:SetTimeSpeed(Super)")
    driver.last = driver.wait(lambda value: worker in spa.generating(spa.context(value))
                              and value["spa_planning"]["virtual_seconds"] >= start + 3,
                              after=driver.last["frame"], timeout=20)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[8])
    spa.click(driver, "menu:SetTimeSpeed(Super)")
    driver.last = driver.wait(lambda value: any(s["entity"] == worker and s.get("generate_power") is None
                                               and s.get("working_on") is None for s in spa.context(value)["souls"]),
                              after=driver.last["frame"], timeout=120)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[9])
    spa.wait_final_raw(driver)
