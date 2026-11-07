"""Ordinary RestArea admission and positive Dream transfer, never art authority."""
import math

import native_acceptance as native
import ui_rest_area_planning as planning

CHECKPOINTS = (*planning.COMPLETION_CHECKPOINTS,
               "rest-area-usage-occupied", "rest-area-usage-drained")
EXIT_CHECKPOINTS = (*CHECKPOINTS, "rest-area-usage-exited")


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
    native.require(case in EXIT_CHECKPOINTS[3:], "unsupported rest usage checkpoint")
    planning.check_observation("rest-area-completion-completed", value)
    bits = value["rest_area_planning"]["buildings"][0]
    state = context(value)
    if case == EXIT_CHECKPOINTS[-1]:
        native.require(any(released_soul(row) for row in state["souls"]), "no visible released Soul with active cooldown")
        candidates(state, bits)
    else:
        native.require(candidates(state, bits), "no actual hidden occupant on a walkable entry")


def released_soul(row):
    return (row.get("resting_in") is None and row.get("reserved_for") is None
            and row.get("visibility") == "Visible" and row.get("walkable") is True
            and isinstance(row.get("idle"), dict)
            and row["idle"].get("behavior") not in (None, "Resting", "GoingToRest")
            and finite(row.get("cooldown")) and 0 < row["cooldown"] <= 15
            and finite(row.get("dream")) and 0 <= row["dream"] <= 100
            and all(finite(row.get(key)) and 0 <= row[key] <= 1 for key in ("fatigue", "stress"))
            and type(row.get("entity_bits")) is int and row["entity_bits"] >= 0
            and isinstance(row.get("position"), list) and len(row["position"]) == 3
            and all(finite(axis) for axis in row["position"]))


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


def exercise_exit(driver):
    exercise(driver)
    value = driver.checkpoints[-1]["value"]
    bits = value["rest_area_planning"]["buildings"][0]
    occupied = candidates(context(driver.checkpoints[-2]["value"]), bits)
    final = {row["entity"]: row for _, row in candidates(context(value), bits)}
    _, soul = next((owner, row) for owner, row in occupied
                   if row["entity"] in final and final[row["entity"]]["dream"] <= row["dream"] - 0.5)
    entity = soul["entity"]
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda observed: any(row.get("entity") == entity and released_soul(row)
                                                  for row in context(observed)["souls"]),
                              after=driver.last["frame"], timeout=120)
    planning.set_refund_paused(driver, True)
    driver.capture(EXIT_CHECKPOINTS[-1])
    wait_for_raw_checkpoint(driver, bits, entity, released=True)


def wait_for_raw_checkpoint(driver, bits, entity, *, released=False):
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
            raw_checkpoint(raw["samples"], value, bits, entity, released=released)
        except RawCheckpointPending:
            return False
        return True

    driver.wait(recorded, after=value["frame"], timeout=15)


def checkpoint_rows(state, bits, entity, *, released):
    occupants = candidates(state, bits)
    if not released:
        return [(owner, soul) for owner, soul in occupants if soul["entity"] == entity]
    owner = next(row for row in state["owners"] if row["entity_bits"] == bits)
    return [(owner, row) for row in state["souls"] if row["entity"] == entity and released_soul(row)]


def raw_checkpoint(samples, value, bits, entity, *, released=False):
    ui = context(value)
    selected = checkpoint_rows(ui, bits, entity, released=released)
    native.require(len(selected) == 1, "checkpoint lost the selected Soul/state")
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
        rows = checkpoint_rows(state, bits, entity, released=released)
        if rows == selected and state.get("dream_pool") == ui["dream_pool"]:
            matching.append(sample)
    if not matching:
        raise RawCheckpointPending("UI rest checkpoint is not bound to a paused same-subject raw sample")
    return min(matching, key=lambda sample: abs(sample["real_seconds"] - ui["real_seconds"]))


def check_exit(raw, entries, client):
    native.require([entry["case"] for entry in entries] == list(EXIT_CHECKPOINTS), "rest exit checkpoints differ")
    check_usage(raw, entries[:5], client)
    occupied, drained, exited = [entry["value"] for entry in entries[3:]]
    check_observation(EXIT_CHECKPOINTS[-1], exited)
    native.require(exited.get("nonce") == client["nonce"] and exited.get("pid") == client["pid"]
                   and exited.get("world_epoch") == drained["world_epoch"] and exited.get("paused") is True
                   and exited["frame"] > drained["frame"] and entries[-1]["last_step"] > entries[-2]["last_step"]
                   and all(exited["rest_area_planning"][key] == drained["rest_area_planning"][key]
                           for key in ("anchor", "footprint", "draw_pos", "buildings")),
                   "rest exit checkpoint subject/target differs")
    bits = drained["rest_area_planning"]["buildings"][0]
    remaining = {row["entity"]: row for _, row in candidates(context(drained), bits)}
    owner, initial = next((owner, remaining[row["entity"]]) for owner, row in candidates(context(occupied), bits)
                          if row["entity"] in remaining and remaining[row["entity"]]["dream"] <= row["dream"] - 0.5)
    entity = initial["entity"]
    first = raw_checkpoint(raw["samples"], drained, bits, entity)
    last = raw_checkpoint(raw["samples"], exited, bits, entity, released=True)
    start, end = first["sample_index"], last["sample_index"]
    native.require(start < end and first["virtual_seconds"] < last["virtual_seconds"], "rest exit time did not advance")
    released, release_index, cooldown, previous_time = False, None, 15, first["virtual_seconds"]
    for sample in raw["samples"][start:end + 1]:
        state = sample["rest_area_context"]
        native.require(sample["world_epoch"] == drained["world_epoch"]
                       and finite(sample.get("real_seconds")) and finite(sample.get("virtual_seconds"))
                       and state.get("real_seconds") == sample["real_seconds"]
                       and state.get("virtual_seconds") == sample["virtual_seconds"]
                       and previous_time <= sample["virtual_seconds"] <= last["virtual_seconds"],
                       "rest exit crosses bad time/epoch")
        previous_time = sample["virtual_seconds"]
        candidates(state, bits)
        rows = [row for row in state["souls"] if row["entity"] == entity]
        native.require(len(rows) == 1 and rows[0]["entity_bits"] == initial["entity_bits"], "rest exit Soul generation differs")
        row = rows[0]
        if released_soul(row):
            native.require(row["cooldown"] <= cooldown, "rest exit cooldown increased/reset")
            # Existing rules: at most 0.5 Dream/s awake, 15s cooldown,
            # and at most five 0.1s slow steps in the application frame.
            # The Rust contract test pins these bounds; this is not exact zero.
            native.require(row["dream"] <= 0.5 * (15 - row["cooldown"] + 0.5) + 1e-3,
                           "rest exit retains more Dream than bounded awake regeneration")
            cooldown = row["cooldown"]
            if not released:
                release_index = sample["sample_index"]
            released = True
        else:
            native.require(not released and row.get("resting_in") == owner["entity"]
                           and row.get("position") == initial["position"], "rest exit is not a one-way lifecycle")
    native.require(released, "rest exit did not release the selected Soul")
    final = [row for row in raw["events"] if row.get("producer") == "DreamTransferredVisualMessage"
             and row.get("worker") == entity and row.get("is_final") is True
             and type(row.get("sample_index")) is int and start < row["sample_index"] <= end]
    native.require(len(final) == 1 and final[0].get("operation") == "DreamTransferred"
                   and final[0].get("source", {}).get("kind") == "RestArea"
                   and final[0]["source"].get("owner") == owner["entity"]
                   and final[0]["sample_index"] <= release_index
                   and finite(final[0].get("amount")) and final[0]["amount"] > 0,
                   "no exact same-owner natural final Dream transfer")
    transfers = [row for row in raw["events"] if row.get("producer") == "DreamTransferredVisualMessage"
                 and row.get("worker") == entity and type(row.get("sample_index")) is int
                 and start < row["sample_index"] <= end]
    native.require(transfers and all(row.get("operation") == "DreamTransferred"
                                    and row.get("source", {}).get("kind") == "RestArea"
                                    and row["source"].get("owner") == owner["entity"]
                                    and finite(row.get("real_seconds"))
                                    and first["real_seconds"] < row["real_seconds"]
                                    <= raw["samples"][row["sample_index"]]["real_seconds"]
                                    and row["real_seconds"] > raw["samples"][row["sample_index"] - 1]["real_seconds"]
                                    and row["sample_index"] <= release_index
                                    and any(item.get("entity") == owner["entity"]
                                            and item.get("position", [])[:2] == row["source"].get("origin")
                                            for item in raw["samples"][row["sample_index"]].get("owners", []))
                                    and finite(row.get("amount")) and row["amount"] > 0 for row in transfers)
                   and math.isclose(sum(row["amount"] for row in transfers), initial["dream"],
                                    rel_tol=1e-4, abs_tol=1e-3)
                   and last["rest_area_context"]["dream_pool"] - first["rest_area_context"]["dream_pool"]
                   >= sum(row["amount"] for row in transfers) - 0.01,
                   "rest final messages do not account for natural Dream exhaustion")


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
