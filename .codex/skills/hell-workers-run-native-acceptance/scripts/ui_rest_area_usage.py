"""Ordinary RestArea admission and positive Dream transfer, never art authority."""
import math

import native_acceptance as native
import ui_rest_area_planning as planning

CHECKPOINTS = (*planning.COMPLETION_CHECKPOINTS,
               "rest-area-usage-occupied", "rest-area-usage-drained")


class RawCheckpointPending(native.AcceptanceError):
    """The bounded raw writer has not reached the captured state yet."""


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def context(value):
    result = value["rest_area_planning"]["rest_area_usage"]
    native.require(isinstance(result, dict) and result.get("failure") is None
                   and finite(result.get("dream_pool")) and result["dream_pool"] >= 0
                   and all(finite(result.get(key)) and result[key] >= 0
                           for key in ("real_seconds", "virtual_seconds")), "rest usage context missing")
    return result


def candidates(value, bits):
    owners, souls = value.get("owners"), value.get("souls")
    native.require(isinstance(owners, list) and len(owners) <= 512
                   and isinstance(souls, list) and len(souls) <= 128
                   and all(isinstance(row.get("entity"), str) and row["entity"] for row in owners)
                   and len({row["entity"] for row in owners}) == len(owners)
                   and all(isinstance(row.get("entity"), str) and row["entity"] for row in souls)
                   and len({row["entity"] for row in souls}) == len(souls), "rest rows missing/duplicated")
    matches = [row for row in owners if row.get("entity_bits") == bits]
    native.require(len(matches) == 1, "rest owner is missing/ambiguous")
    owner = matches[0]
    occupants, reservations = owner.get("occupants"), owner.get("reservations")
    native.require(owner.get("building_kind") == "RestArea" and owner.get("provisional") is False
                   and isinstance(owner.get("entity"), str) and owner["entity"]
                   and type(owner.get("capacity")) is int and owner["capacity"] > 0,
                   "rest owner is not a completed production building")
    for rows in (occupants, reservations):
        native.require(rows is None or isinstance(rows, list) and len(rows) <= 128
                       and all(isinstance(item, str) and item for item in rows)
                       and len(set(rows)) == len(rows), "rest relationship inventory differs")
    occupants, reservations = occupants or [], reservations or []
    native.require(not set(occupants).intersection(reservations)
                   and len(occupants) + len(reservations) <= owner["capacity"], "rest capacity differs")
    by_id = {row["entity"]: row for row in souls}
    native.require({entity for entity, row in by_id.items() if row.get("resting_in") == owner["entity"]}
                   == set(occupants)
                   and {entity for entity, row in by_id.items() if row.get("reserved_for") == owner["entity"]}
                   == set(reservations), "rest relationship reverse inventory differs")
    native.require(all(entity in by_id and by_id[entity].get("reserved_for") == owner["entity"]
                       and by_id[entity].get("resting_in") is None for entity in reservations),
                   "rest reservation inverse differs")
    result = []
    for entity in occupants:
        row = by_id.get(entity, {})
        native.require(row.get("resting_in") == owner["entity"] and row.get("reserved_for") is None
                       and isinstance(row.get("idle"), dict) and row["idle"].get("behavior") == "Resting"
                       and row.get("visibility") == "Hidden", "rest occupant inverse/state differs")
        native.require(type(row.get("entity_bits")) is int and row["entity_bits"] >= 0
                       and all(finite(row.get(key)) for key in ("dream", "fatigue", "stress"))
                       and 0 <= row["dream"] <= 100 and 0 <= row["fatigue"] <= 1 and 0 <= row["stress"] <= 1
                       and isinstance(row.get("position"), list) and len(row["position"]) == 3
                       and all(finite(axis) for axis in row["position"]), "rest Soul state differs")
        native.require(row.get("cooldown") is None or finite(row["cooldown"]) and row["cooldown"] <= 0,
                       "rest occupant has an active/invalid entry cooldown")
        if row["dream"] > 0 and row.get("walkable") is True:
            result.append((owner, row))
    return result


def check_observation(case, value):
    native.require(case in CHECKPOINTS[-2:], "unsupported rest usage checkpoint")
    planning.check_observation("rest-area-completion-completed", value)
    bits = value["rest_area_planning"]["buildings"][0]
    native.require(candidates(context(value), bits), "no actual hidden occupant on a walkable entry")


def exercise(driver):
    planning.exercise(driver, completion=True)
    bits = driver.last["rest_area_planning"]["buildings"][0]
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda value: any(row["dream"] >= 2 for _, row in candidates(context(value), bits)),
                              after=driver.last["frame"], timeout=120)
    planning.set_refund_paused(driver, True)
    driver.capture(CHECKPOINTS[-2])
    _, soul = next((owner, row) for owner, row in candidates(context(driver.last), bits) if row["dream"] >= 2)
    entity, initial = soul["entity"], soul["dream"]
    wait_for_raw_checkpoint(driver, bits, entity)
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda value: any(row["entity"] == entity and row["dream"] <= initial - 0.5
                                              for _, row in candidates(context(value), bits)),
                              after=driver.last["frame"], timeout=15)
    planning.set_refund_paused(driver, True)
    driver.capture(CHECKPOINTS[-1])
    wait_for_raw_checkpoint(driver, bits, entity)


def wait_for_raw_checkpoint(driver, bits, entity):
    # UI observation runs in PostUpdate; raw observation runs in Last. Do not
    # terminate the child while its final captured frame is still being written.
    # Keep the verifier's exact state/time predicates, and never resend input.
    value = driver.checkpoints[-1]["value"]

    def recorded(_):
        path = driver.root / "production-cancellation.json"
        if not path.is_file():
            return False
        raw = native.read_json(path)
        native.require(raw.get("nonce") == value["nonce"]
                       and raw.get("process_id") == value["pid"]
                       and raw.get("failure") is None, "raw rest collector failed or changed identity")
        try:
            raw_checkpoint(raw["samples"], value, bits, entity)
        except RawCheckpointPending:
            return False
        return True

    driver.wait(recorded, after=value["frame"], timeout=15)


def raw_checkpoint(samples, value, bits, entity):
    ui = context(value)
    selected = [(owner, soul) for owner, soul in candidates(ui, bits) if soul["entity"] == entity]
    native.require(len(selected) == 1, "checkpoint lost the selected resting Soul")
    matching = []
    for sample in samples:
        state = sample.get("rest_area_context")
        if (sample.get("world_epoch") != value["world_epoch"] or sample.get("paused") is not True
                or not isinstance(state, dict) or not finite(state.get("real_seconds"))
                or not finite(state.get("virtual_seconds"))
                or abs(state["real_seconds"] - ui["real_seconds"]) > 1
                or abs(state["virtual_seconds"] - ui["virtual_seconds"]) > 1e-6):
            continue
        native.require(abs(state["real_seconds"] - sample["real_seconds"]) < 1e-9
                       and finite(sample.get("virtual_seconds"))
                       and abs(state["virtual_seconds"] - sample["virtual_seconds"]) < 1e-9,
                       "raw rest timestamp differs from its containing sample")
        rows = [(owner, soul) for owner, soul in candidates(state, bits) if soul["entity"] == entity]
        if rows == selected and state.get("dream_pool") == ui["dream_pool"]:
            matching.append(sample)
    if not matching:
        raise RawCheckpointPending("UI rest checkpoint is not bound to a paused same-subject raw sample")
    return min(matching, key=lambda sample: abs(sample["real_seconds"] - ui["real_seconds"]))


def check_usage(raw, entries, client):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS), "rest usage checkpoints differ")
    planning.check_completion(raw, entries[:3], client)
    completed, occupied, drained = [entry["value"] for entry in entries[2:]]
    for entry in entries[-2:]:
        value = entry["value"]
        check_observation(entry["case"], value)
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and value.get("paused") is True and value.get("world_epoch") == completed["world_epoch"]
                       and all(value["rest_area_planning"][key] == completed["rest_area_planning"][key]
                               for key in ("anchor", "footprint", "draw_pos", "buildings")),
                       "rest usage checkpoint identity/epoch differs")
    native.require(all(after["value"]["frame"] > before["value"]["frame"]
                       and after["last_step"] > before["last_step"] for before, after in zip(entries[2:], entries[3:])),
                   "rest usage has no fresh input/observation")
    bits = completed["rest_area_planning"]["buildings"][0]
    initial = candidates(context(occupied), bits)
    final = {soul["entity"]: soul for _, soul in candidates(context(drained), bits)}
    witnesses = [(owner, soul, final[soul["entity"]]) for owner, soul in initial
                 if soul["entity"] in final and final[soul["entity"]]["dream"] <= soul["dream"] - 0.5]
    native.require(witnesses, "no same-Soul positive rest drain")
    owner, before, after = witnesses[0]
    entity = before["entity"]
    native.require(before["entity_bits"] == after["entity_bits"] and before["position"] == after["position"],
                   "rest transfer changed Soul generation or entry position")
    first = raw_checkpoint(raw["samples"], occupied, bits, entity)
    last = raw_checkpoint(raw["samples"], drained, bits, entity)
    start, end = first["sample_index"], last["sample_index"]
    native.require(start < end and first["virtual_seconds"] < last["virtual_seconds"], "rest drain time did not advance")
    for sample in raw["samples"][start:end + 1]:
        state = sample["rest_area_context"]
        native.require(sample.get("world_epoch") == occupied["world_epoch"]
                       and finite(sample.get("virtual_seconds"))
                       and first["virtual_seconds"] <= sample["virtual_seconds"] <= last["virtual_seconds"]
                       and finite(state.get("dream_pool"))
                       and finite(state.get("real_seconds"))
                       and state["real_seconds"] == sample["real_seconds"]
                       and finite(state.get("virtual_seconds"))
                       and state["virtual_seconds"] == sample["virtual_seconds"],
                       "rest drain crosses world replacement/bad time")
        rows = [soul for _, soul in candidates(sample["rest_area_context"], bits) if soul["entity"] == entity]
        native.require(len(rows) == 1 and rows[0]["entity_bits"] == before["entity_bits"]
                       and rows[0]["position"] == before["position"], "rest lifecycle/entry changed during transfer")
    messages = [row for row in raw["events"] if row.get("producer") == "DreamTransferredVisualMessage"
                and row.get("worker") == entity]
    native.require(all(type(row.get("sample_index")) is int and 0 <= row["sample_index"] < len(raw["samples"])
                       and finite(row.get("real_seconds"))
                       and row["real_seconds"] <= raw["samples"][row["sample_index"]]["real_seconds"]
                       and (row["sample_index"] == 0
                            or row["real_seconds"] > raw["samples"][row["sample_index"] - 1]["real_seconds"])
                       for row in messages), "Dream message sample/time is invalid")
    events = [row for row in messages if start < row["sample_index"] <= end]
    native.require(events and all(row.get("operation") == "DreamTransferred"
                                  and row.get("source", {}).get("kind") == "RestArea"
                                  and row["source"].get("owner") == owner["entity"]
                                  and isinstance(row["source"].get("origin"), list)
                                  and len(row["source"]["origin"]) == 2
                                  and all(finite(axis) for axis in row["source"]["origin"])
                                  and any(item.get("entity") == owner["entity"]
                                          and isinstance(item.get("position"), list)
                                          and item["position"][:2] == row["source"]["origin"]
                                          for item in raw["samples"][row["sample_index"]].get("owners", []))
                                  and row.get("is_final") is False and finite(row.get("amount")) and row["amount"] > 0
                                  and finite(row.get("real_seconds"))
                                  and first["real_seconds"] < row["real_seconds"] <= last["real_seconds"]
                                  for row in events), "actual same-owner RestArea Dream message missing/different")
    amount = sum(row["amount"] for row in events)
    native.require(math.isclose(before["dream"] - after["dream"], amount, rel_tol=1e-4, abs_tol=1e-3)
                   and context(drained)["dream_pool"] - context(occupied)["dream_pool"] >= amount - 0.01,
                   "Dream messages do not reconcile with actual Soul drain and pool growth")
