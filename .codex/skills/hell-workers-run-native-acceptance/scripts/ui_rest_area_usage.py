"""Ordinary RestArea admission and positive Dream transfer, never art authority."""
import math
import hashlib
import re

import native_acceptance as native
import ui_rest_area_planning as planning

CHECKPOINTS = (*planning.COMPLETION_CHECKPOINTS,
               "rest-area-usage-occupied", "rest-area-usage-drained")
EXIT_CHECKPOINTS = (*CHECKPOINTS, "rest-area-usage-exited")
SAVE_CHECKPOINTS = (*CHECKPOINTS, "rest-area-usage-saved", "rest-area-usage-loaded")
LIFECYCLE_CHECKPOINTS = (*SAVE_CHECKPOINTS, "rest-area-usage-exited", "rest-area-usage-expired")


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
    native.require(case in (*EXIT_CHECKPOINTS[3:], *SAVE_CHECKPOINTS[5:], LIFECYCLE_CHECKPOINTS[-1]),
                   "unsupported rest usage checkpoint")
    planning.check_observation("rest-area-completion-completed", value)
    bits = value["rest_area_planning"]["buildings"][0]
    state = context(value)
    if case == LIFECYCLE_CHECKPOINTS[-1]:
        candidates(state, bits)
    elif case == EXIT_CHECKPOINTS[-1]:
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
                              # Ordinary reservations may select distant Souls. In the
                              # recorded low-FPS art run they kept moving throughout
                              # 123 wall seconds but advanced only 54 virtual seconds.
                              # Allow arrival without changing speed/state or the
                              # actual occupant predicate; Driver still caps the
                              # complete session at 480 wall seconds.
                              after=driver.last["frame"], timeout=240)
    ready = [row for _, row in candidates(context(driver.last), bits) if row["dream"] >= 2]
    native.require(ready, "ready rest occupant disappeared before pause input")
    entity = ready[0]["entity"]
    planning.set_refund_paused(driver, True)
    driver.capture(CHECKPOINTS[-2])
    # OS pause/capture spans rendered frames. Pin the observed Soul instead of
    # applying the pre-pause readiness threshold again after natural transfer.
    paused = [row for _, row in candidates(context(driver.last), bits) if row["entity"] == entity]
    native.require(driver.last["paused"] is True and len(paused) == 1 and paused[0]["dream"] > 0.5,
                   "ready rest occupant was replaced/exited or lacks a positive drain margin")
    initial = paused[0]["dream"]
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


def exercise_save(driver, *, raw_scope="production-raw-observations-v1"):
    """Save/load an ordinarily admitted occupant through the existing catalogs."""
    exercise(driver)
    driver.key("F5")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectSaveCatalogSlot")
                                              for key in value["controls"]), after=driver.last["frame"])
    driver.click(planning.catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda value: value["catalog"] == "Closed"
                              and "Save:Succeeded" in value["save_outcomes"], after=driver.last["frame"])
    driver.capture(SAVE_CHECKPOINTS[-2])
    driver.key("F9")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectLoadCatalogSlot")
                                              for key in value["controls"]), after=driver.last["frame"])
    driver.click(planning.catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    driver.click(planning.catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda value: value["world_epoch"] > epoch and value["catalog"] == "Closed",
                              after=driver.last["frame"], timeout=60)
    planning.set_refund_paused(driver, True)
    driver.capture(SAVE_CHECKPOINTS[-1])
    selected = [None]

    def recorded(_):
        path = driver.root / "production-cancellation.json"
        if not path.is_file():
            return False
        raw = native.read_json(path)
        native.require(raw.get("nonce") == driver.last["nonce"]
                       and raw.get("process_id") == driver.last["pid"] and raw.get("failure") is None,
                       "durable raw collector failed/changed process")
        if not any(row.get("operation") == "DurableLoad" for row in raw.get("events", [])):
            return False
        try:
            selected[0] = check_save(raw, driver.checkpoints, {"nonce": driver.last["nonce"], "pid": driver.last["pid"]},
                                      driver.root / "saves" / "manual-1.scn.ron", raw_scope=raw_scope)
        except RawCheckpointPending:
            return False
        return True

    driver.wait(recorded, after=driver.last["frame"], timeout=15)
    native.require(isinstance(selected[0], str) and selected[0], "saved occupant remap missing")
    return selected[0]


def exercise_lifecycle(driver, *, raw_scope="production-raw-observations-v1"):
    entity = exercise_save(driver, raw_scope=raw_scope)
    loaded = driver.checkpoints[-1]["value"]
    bits = loaded["rest_area_planning"]["buildings"][0]
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda value: any(row.get("entity") == entity and released_soul(row)
                                              for row in context(value)["souls"]),
                              after=driver.last["frame"], timeout=120)
    planning.set_refund_paused(driver, True)
    driver.capture(LIFECYCLE_CHECKPOINTS[-2])
    wait_for_raw_checkpoint(driver, bits, entity, released=True)
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda value: any(row.get("entity") == entity and row.get("cooldown") is None
                                              for row in context(value)["souls"]),
                              after=driver.last["frame"], timeout=60)
    planning.set_refund_paused(driver, True)
    driver.capture(LIFECYCLE_CHECKPOINTS[-1])
    captured = driver.checkpoints[-1]["value"]

    def recorded(_):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("nonce") == captured["nonce"] and raw.get("process_id") == captured["pid"]
                       and raw.get("failure") is None, "post-load lifecycle raw changed identity/failed")
        try:
            raw_context_checkpoint(raw["samples"], captured)
        except RawCheckpointPending:
            return False
        return True

    driver.wait(recorded, after=captured["frame"], timeout=15)


def stable_context(state):
    """Only collection order and observation clocks are non-persistent here."""
    result = {key: value for key, value in state.items() if key not in ("real_seconds", "virtual_seconds")}
    for key in ("owners", "souls"):
        result[key] = sorted(state[key], key=lambda row: row["entity_bits"])
    return result


def check_durable_mapping(before, after, mapping):
    native.require(isinstance(mapping, list) and 0 < len(mapping) <= 640, "durable map missing/capped")
    expected, actual = {}, {}
    for key, kind, limit in (("owners", "RestArea", 512), ("souls", "Soul", 128)):
        for state, target in ((before, expected), (after, actual)):
            rows = state.get(key)
            native.require(isinstance(rows, list) and len(rows) <= limit, "durable context inventory missing/capped")
            for row in rows:
                entity = row.get("entity")
                native.require(isinstance(entity, str) and entity and entity not in target
                               and type(row.get("entity_bits")) is int and row["entity_bits"] >= 0,
                               "durable context identity missing/duplicated")
                target[entity] = (kind, row)
    native.require(len(mapping) == len(expected) == len(actual), "durable map inventory incomplete")
    lookup, targets = {}, set()
    for row in mapping:
        old, new = row.get("saved_entity"), row.get("loaded_entity")
        native.require(old in expected and new in actual and old not in lookup and new not in targets
                       and old != new and row.get("kind") == expected[old][0] == actual[new][0]
                       and type(row.get("saved_bits")) is int and type(row.get("loaded_bits")) is int
                       and row["saved_bits"] == expected[old][1]["entity_bits"]
                       and row["loaded_bits"] == actual[new][1]["entity_bits"],
                       "durable map is foreign/duplicated or changes kind/bits")
        lookup[old] = new
        targets.add(new)
    for old, (kind, row) in expected.items():
        restored = actual[lookup[old]][1]
        translated = {key: value for key, value in row.items() if key not in ("entity", "entity_bits")}
        if kind == "RestArea":
            for key in ("occupants", "reservations"):
                values = row.get(key)
                native.require(values is None or isinstance(values, list)
                               and len(values) == len(set(values)) and all(value in lookup for value in values),
                               "durable owner relationship target missing/duplicated")
                translated[key] = sorted(lookup[value] for value in values) if values is not None else None
        else:
            for key in ("resting_in", "reserved_for"):
                native.require(row.get(key) is None or row[key] in lookup, "durable Soul relationship target missing")
                translated[key] = lookup[row[key]] if row.get(key) is not None else None
            # Visibility is not persisted. attach_soul_shell installs Inherited;
            # rehydrate_soul_shells overrides it to Hidden only for RestingIn.
            # Pin that existing projection, not a general equivalence of modes.
            if row.get("resting_in") is None:
                native.require(row.get("visibility") in ("Visible", "Inherited"),
                               "durable non-resting Soul has an unsupported visibility")
                translated["visibility"] = "Inherited"
            else:
                native.require(row.get("visibility") == "Hidden", "durable resting Soul was not hidden")
        observed = {key: value for key, value in restored.items() if key not in ("entity", "entity_bits")}
        if kind == "RestArea":
            for key in ("occupants", "reservations"):
                if observed.get(key) is not None:
                    observed[key] = sorted(observed[key])
        native.require(translated == observed, "durable restored state/relationship differs")
    native.require(before.get("dream_pool") == after.get("dream_pool"), "durable DreamPool differs")
    return lookup


def check_receipt_interval(raw, receipt, value, bits, entity):
    """Bind the exact commit to the capture through unchanged paused samples.

    UI clicks/capture each wait for several rendered frames. Their real duration
    is not simulation freshness. Bound by the existing 480-second UI session and
    240 samples, and reject any intervening time/state/epoch change instead.
    """
    last = raw_checkpoint(raw["samples"], value, bits, entity)
    start, end = receipt["sample_index"], last["sample_index"]
    native.require(0 <= end - start < 240
                   and 0 <= context(value)["real_seconds"] - receipt["real_seconds"] <= 480,
                   "durable receipt interval is absent/capped")
    expected = stable_context(context(value))
    previous = receipt["real_seconds"]
    for index in range(start, end + 1):
        sample = raw["samples"][index]
        state = sample.get("rest_area_context")
        native.require(sample.get("sample_index") == index and sample.get("world_epoch") == value["world_epoch"]
                       and sample.get("paused") is True and finite(sample.get("real_seconds"))
                       and sample["real_seconds"] >= previous and isinstance(state, dict)
                       and state.get("real_seconds") == sample["real_seconds"]
                       and state.get("virtual_seconds") == sample.get("virtual_seconds")
                       == context(value)["virtual_seconds"] and stable_context(state) == expected,
                       "durable receipt interval crosses changed/unpaused/foreign state")
        previous = sample["real_seconds"]


def check_save(raw, entries, client, save_path, *, raw_scope="production-raw-observations-v1"):
    native.require([entry["case"] for entry in entries] == list(SAVE_CHECKPOINTS), "rest save checkpoints differ")
    check_usage(raw, entries[:5], client, raw_scope=raw_scope)
    drained, saved, loaded = [entry["value"] for entry in entries[4:]]
    for entry in entries[5:]:
        value = entry["value"]
        check_observation(entry["case"], value)
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and value.get("paused") is True and value.get("catalog") == "Closed"
                       and all(value["rest_area_planning"][key] == drained["rest_area_planning"][key]
                               for key in ("anchor", "footprint", "draw_pos")), "rest save subject/target differs")
    native.require(saved["world_epoch"] == drained["world_epoch"]
                   and loaded["world_epoch"] == saved["world_epoch"] + 1
                   and "Save:Succeeded" in saved["save_outcomes"] and "Load:Succeeded" in loaded["save_outcomes"]
                   and all(after["value"]["frame"] > before["value"]["frame"]
                           and after["last_step"] > before["last_step"]
                           for before, after in zip(entries[4:], entries[5:])), "rest save result/epoch/order differs")
    native.require(stable_context(context(drained)) == stable_context(context(saved)),
                   "paused save changed the ordinary rest state")
    native.require(save_path.name == "manual-1.scn.ron" and save_path.parent.name == "saves"
                   and not save_path.is_symlink() and not save_path.parent.is_symlink()
                   and save_path.is_file() and 0 < save_path.stat().st_size <= 64 * 1024 * 1024,
                   "durable save file missing/unsafe/capped")
    with save_path.open("rb") as stream:
        payload = stream.read(64 * 1024 * 1024 + 1)
    native.require(0 < len(payload) <= 64 * 1024 * 1024, "durable save bytes capped")
    digest = hashlib.sha256(payload).hexdigest()
    receipts = [row for row in raw["events"] if row.get("operation") in ("DurableSave", "DurableLoad")]
    native.require([row.get("operation") for row in receipts] == ["DurableSave", "DurableLoad"],
                   "durable receipt inventory differs")
    for row, value, operation, source in zip(receipts, (saved, loaded), ("save", "load"),
                                            (r"Manual\(ManualCatalog \{ dialog_session: [1-9][0-9]* \}\)",
                                             r"Load\(NormalCatalog \{ dialog_session: [1-9][0-9]* \}\)")):
        native.require(row.get("producer") == f"ordinary-{operation}-commit-v1"
                       and row.get("result") == "Committed" and row.get("target") == "Manual slot 1"
                       and isinstance(row.get("request_source"), str) and re.fullmatch(source, row["request_source"])
                       and row.get("file_sha256") == digest and type(row.get("file_bytes")) is int
                       and row["file_bytes"] == len(payload)
                       and row.get("world_epoch") == value["world_epoch"]
                       and type(row.get("sample_index")) is int and 0 <= row["sample_index"] < len(raw["samples"])
                       and finite(row.get("real_seconds"))
                       and 0 <= context(value)["real_seconds"] - row["real_seconds"] <= 480
                       and row["rest_area_context"].get("real_seconds") == row["real_seconds"]
                       and row["rest_area_context"].get("virtual_seconds") == context(value)["virtual_seconds"]
                       and stable_context(row["rest_area_context"]) == stable_context(context(value)),
                       "durable receipt bytes/request/context differs")
        sample = raw["samples"][row["sample_index"]]
        native.require(sample["world_epoch"] == row["world_epoch"] and sample["paused"] is True
                       and row["real_seconds"] <= sample["real_seconds"]
                       and (row["sample_index"] == 0
                            or raw["samples"][row["sample_index"] - 1]["real_seconds"] < row["real_seconds"])
                       and stable_context(sample["rest_area_context"]) == stable_context(row["rest_area_context"]),
                       "durable receipt sample/time differs")
    save, load = receipts
    native.require(save["sample_index"] < load["sample_index"]
                   and save["real_seconds"] < load["real_seconds"]
                   and save["real_seconds"] > context(drained)["real_seconds"]
                   and load["real_seconds"] > context(saved)["real_seconds"]
                   and load.get("previous_epoch") == save["world_epoch"], "durable load origin differs")
    lookup = check_durable_mapping(context(saved), context(loaded), load.get("entity_remap"))
    bits = saved["rest_area_planning"]["buildings"][0]
    owner, soul = candidates(context(saved), bits)[0]
    restored_bits = loaded["rest_area_planning"]["buildings"][0]
    restored = candidates(context(loaded), restored_bits)
    native.require(any(new_owner["entity"] == lookup[owner["entity"]] and new_soul["entity"] == lookup[soul["entity"]]
                       for new_owner, new_soul in restored), "occupied RestArea/Soul pair not restored")
    check_receipt_interval(raw, save, saved, bits, soul["entity"])
    check_receipt_interval(raw, load, loaded, restored_bits, lookup[soul["entity"]])
    return lookup[soul["entity"]]


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


def raw_context_checkpoint(samples, value):
    ui = context(value)
    matches = [sample for sample in samples if sample.get("world_epoch") == value["world_epoch"]
               and sample.get("paused") is True and finite(sample.get("real_seconds"))
               and abs(sample["real_seconds"] - ui["real_seconds"]) <= 1
               and sample.get("virtual_seconds") == ui["virtual_seconds"]
               and isinstance(sample.get("rest_area_context"), dict)
               and sample["rest_area_context"].get("real_seconds") == sample["real_seconds"]
               and sample["rest_area_context"].get("virtual_seconds") == sample["virtual_seconds"]
               and stable_context(sample["rest_area_context"]) == stable_context(ui)]
    if not matches:
        raise RawCheckpointPending("full rest context has no same-subject paused raw sample")
    return min(matches, key=lambda sample: abs(sample["real_seconds"] - ui["real_seconds"]))


def check_exit(raw, entries, client):
    native.require([entry["case"] for entry in entries] == list(EXIT_CHECKPOINTS), "rest exit checkpoints differ")
    check_usage(raw, entries[:5], client)
    occupied, drained, exited = [entry["value"] for entry in entries[3:]]
    bits = drained["rest_area_planning"]["buildings"][0]
    remaining = {row["entity"]: row for _, row in candidates(context(drained), bits)}
    owner, initial = next((owner, remaining[row["entity"]]) for owner, row in candidates(context(occupied), bits)
                          if row["entity"] in remaining and remaining[row["entity"]]["dream"] <= row["dream"] - 0.5)
    entity = initial["entity"]
    check_exit_leg(raw, entries[-2], entries[-1], client, entity)


def check_exit_leg(raw, before_entry, exited_entry, client, entity):
    drained, exited = before_entry["value"], exited_entry["value"]
    check_observation(EXIT_CHECKPOINTS[-1], exited)
    native.require(exited.get("nonce") == client["nonce"] and exited.get("pid") == client["pid"]
                   and exited.get("world_epoch") == drained["world_epoch"] and exited.get("paused") is True
                   and exited["frame"] > drained["frame"] and exited_entry["last_step"] > before_entry["last_step"]
                   and all(exited["rest_area_planning"][key] == drained["rest_area_planning"][key]
                           for key in ("anchor", "footprint", "draw_pos", "buildings")),
                   "rest exit checkpoint subject/target differs")
    bits = drained["rest_area_planning"]["buildings"][0]
    owner, initial = next((owner, row) for owner, row in candidates(context(drained), bits) if row["entity"] == entity)
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


def check_lifecycle(raw, entries, client, save_path, *, raw_scope="production-raw-observations-v1"):
    native.require([entry["case"] for entry in entries] == list(LIFECYCLE_CHECKPOINTS),
                   "post-load lifecycle checkpoints differ")
    entity = check_save(raw, entries[:7], client, save_path, raw_scope=raw_scope)
    loaded, exited, expired = [entry["value"] for entry in entries[6:]]
    bits = loaded["rest_area_planning"]["buildings"][0]
    selected = [row for _, row in candidates(context(loaded), bits) if row["entity"] == entity]
    native.require(len(selected) == 1, "post-load selected saved occupant disappeared")
    initial = selected[0]
    check_exit_leg(raw, entries[6], entries[7], client, entity)
    check_observation(LIFECYCLE_CHECKPOINTS[-1], expired)
    native.require(expired.get("nonce") == client["nonce"] and expired.get("pid") == client["pid"]
                   and expired.get("world_epoch") == loaded["world_epoch"] and expired.get("paused") is True
                   and expired["frame"] > exited["frame"] and entries[8]["last_step"] > entries[7]["last_step"]
                   and all(expired["rest_area_planning"][key] == loaded["rest_area_planning"][key]
                           for key in ("anchor", "footprint", "draw_pos", "buildings")),
                   "cooldown expiry checkpoint subject/order differs")
    first = raw_checkpoint(raw["samples"], exited, bits, entity, released=True)
    last = raw_context_checkpoint(raw["samples"], expired)
    start, end = first["sample_index"], last["sample_index"]
    native.require(0 < end - start < 240 and last["virtual_seconds"] > first["virtual_seconds"],
                   "cooldown expiry interval missing/capped")
    soul = next(row for row in first["rest_area_context"]["souls"] if row["entity"] == entity)
    duration, previous, previous_time, expired_at = soul["cooldown"], soul["cooldown"], first["virtual_seconds"], None
    previous_real = first["real_seconds"]
    for index in range(start, end + 1):
        sample = raw["samples"][index]
        state = sample["rest_area_context"]
        native.require(sample.get("sample_index") == index and sample.get("world_epoch") == loaded["world_epoch"]
                       and finite(sample.get("real_seconds")) and finite(sample.get("virtual_seconds"))
                       and state.get("real_seconds") == sample["real_seconds"]
                       and state.get("virtual_seconds") == sample["virtual_seconds"]
                       and sample["real_seconds"] >= previous_real
                       and previous_time <= sample["virtual_seconds"] <= last["virtual_seconds"],
                       "cooldown expiry crosses foreign/non-monotonic time")
        previous_time = sample["virtual_seconds"]
        previous_real = sample["real_seconds"]
        candidates(state, bits)
        rows = [row for row in state["souls"] if row["entity"] == entity]
        native.require(len(rows) == 1 and rows[0]["entity_bits"] == initial["entity_bits"],
                       "cooldown expiry changed Soul generation")
        row = rows[0]
        native.require(all(finite(row.get(key)) for key in ("dream", "fatigue", "stress"))
                       and 0 <= row["dream"] <= 100 and 0 <= row["fatigue"] <= 1 and 0 <= row["stress"] <= 1
                       and (row.get("cooldown") is None or finite(row["cooldown"]) and row["cooldown"] >= 0)
                       and row.get("visibility") in ("Hidden", "Visible", "Inherited")
                       and isinstance(row.get("position"), list) and len(row["position"]) == 3
                       and all(finite(axis) for axis in row["position"]), "cooldown Soul state malformed")
        if expired_at is not None:
            continue  # Later ordinary re-entry is not an early-entry violation.
        elapsed = sample["virtual_seconds"] - first["virtual_seconds"]
        if row.get("cooldown") is None:
            native.require(elapsed + 0.5 >= duration, "cooldown removed before its actual duration")
            expired_at = index
        else:
            native.require(released_soul(row) and row["cooldown"] <= previous
                           and abs(row["cooldown"] - max(0, duration - elapsed)) <= 0.5,
                           "cooldown reset, stopped counting, or allowed early entry")
            previous = row["cooldown"]
    native.require(expired_at is not None, "no actual post-load cooldown removal observed")


def check_usage(raw, entries, client, *, raw_scope="production-raw-observations-v1"):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS), "rest usage checkpoints differ")
    planning.check_completion(raw, entries[:3], client, raw_scope=raw_scope)
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
