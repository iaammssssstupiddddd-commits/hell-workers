"""Occupied Spa durable restoration; runtime tasks are deliberately non-persistent."""
import copy
import hashlib
import json
import re

import native_acceptance as native
import ui_spa_planning as spa

CHECKPOINTS = spa.USAGE_CHECKPOINTS + ("soul-spa-occupied-saved", "soul-spa-occupied-loaded")


def check_observation(case, value):
    native.require(case in CHECKPOINTS[-2:] and value.get("paused") is True,
                   "Occupied Spa persistence checkpoint differs")
    state = spa.context(value)
    if case == CHECKPOINTS[-2]:
        spa.check_usage_observation(spa.USAGE_CHECKPOINTS[-1], value)
    else:
        native.require(len(state["sites"]) == 1 and len(state["tiles"]) == 4,
                       "Occupied Spa loaded inventory differs")
        runtime_reset(state)


def exact(left, right):
    def encode(value):
        return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)
    return encode(left) == encode(right)


def durable(state):
    """Explicit persisted building projection; workers are checked separately."""
    result = spa.selected_state(state)
    for tile in result["tiles"]:
        native.require("workers" in tile, "Occupied Spa worker observation missing")
        del tile["workers"]
    return result


def runtime_reset(state):
    native.require(not state["working_relations"]
                   and all("workers" in tile and tile["workers"] in (None, []) for tile in state["tiles"])
                   and all(all(key in soul for key in ("task", "working_on", "generate_power"))
                           and soul["task"] == "None" and soul["working_on"] is None
                           and soul.get("generate_power") is None for soul in state["souls"]),
                   "Occupied Spa load retained runtime tasks or worker relations")


def mapped_durable(before, after, mapping):
    # Reuse the structural map guards, then additionally compare canonical JSON
    # so bool/int/float substitutions cannot masquerade as preserved values.
    old, new = durable(before), durable(after)
    lookup = spa.check_mapping(old, new, mapping)
    indexed = {row["entity"]: row for key in new for row in new[key]}
    native.require(not {row["entity"] for key in old for row in old[key]}.intersection(indexed),
                   "Occupied Spa retained an old building identity")
    for category, rows in old.items():
        for row in rows:
            translated = copy.deepcopy(row)
            entity = translated.pop("entity")
            translated.pop("entity_bits")
            if category == "sites":
                translated["grid"] = lookup[row["grid"]] if row["grid"] is not None else None
            elif category == "tiles":
                translated["parent_site"] = lookup[row["parent_site"]]
                translated["map"]["owner"] = lookup[row["map"]["owner"]]
            elif row["generators"] is not None:
                translated["generators"] = sorted(lookup[g] for g in row["generators"])
            observed = copy.deepcopy(indexed[lookup[entity]])
            observed.pop("entity")
            observed.pop("entity_bits")
            if category == "grids" and observed["generators"] is not None:
                observed["generators"] = sorted(observed["generators"])
            native.require(exact(translated, observed), "Occupied Spa durable JSON values differ")
    return lookup


def mapped_souls(before, after, mapping):
    old = {soul["entity"]: soul for soul in before["souls"]}
    new = {soul["entity"]: soul for soul in after["souls"]}
    native.require(isinstance(mapping, list) and len(mapping) <= 640
                   and all(isinstance(row, dict) and row.get("kind") in ("Soul", "RestArea")
                           for row in mapping), "Occupied Spa Soul map missing/foreign")
    rows = [row for row in mapping if row["kind"] == "Soul"]
    native.require(len(rows) == len(old) == len(new), "Occupied Spa Soul map incomplete")
    lookup = {}
    for row in rows:
        a, b = row.get("saved_entity"), row.get("loaded_entity")
        native.require(a in old and b in new and a != b
                       and a not in lookup and b not in lookup.values()
                       and type(row.get("saved_bits")) is int and type(row.get("loaded_bits")) is int
                       and type(old[a].get("entity_bits")) is int and type(new[b].get("entity_bits")) is int
                       and row["saved_bits"] == old[a].get("entity_bits")
                       and row["loaded_bits"] == new[b].get("entity_bits"),
                       "Occupied Spa Soul remap duplicated/foreign/stale")
        lookup[a] = b
    native.require(not set(old).intersection(new), "Occupied Spa old Soul identity remains")
    workers = spa.generating(before)
    native.require(workers, "Occupied Spa save has no actual generating Soul")
    for entity, soul in workers.items():
        loaded = new[lookup[entity]]
        for key in ("dream", "fatigue", "stress"):
            native.require(spa.finite(soul.get(key)) and spa.finite(loaded.get(key)),
                           "Occupied Spa Soul vitals missing/nonfinite")
        native.require(isinstance(soul.get("pose"), dict)
                       and exact({k: soul[k] for k in ("dream", "fatigue", "stress", "pose")},
                                 {k: loaded.get(k) for k in ("dream", "fatigue", "stress", "pose")}),
                       "Occupied Spa generating Soul durable values differ")
    runtime_reset(after)
    return lookup


def worker_values(state, identities):
    rows = {row["entity"]: row for row in state["souls"]}
    native.require(set(identities) <= rows.keys(), "Occupied Spa tracked Soul missing")
    return {key: {field: rows[key].get(field) for field in ("dream", "fatigue", "stress", "pose")}
            for key in identities}


def verify(raw, entries, client, save_path, inputs):
    native.require([row["case"] for row in entries] == list(CHECKPOINTS), "Occupied Spa sequence differs")
    spa.check_raw_session(raw, client)
    end = spa.raw_checkpoint(raw, entries[4]["value"])["sample_index"]
    prefix = dict(raw, samples=raw["samples"][:end + 1],
                  events=[row for row in raw["events"] if row["sample_index"] <= end])
    spa.verify_usage(prefix, entries[:5], client, inputs[:entries[4]["last_step"]])
    spa.check_inputs(raw, entries, client, inputs, completion=True, usage=True, occupied=True)
    for entry in entries[5:]:
        check_observation(entry["case"], entry["value"])
        value = entry["value"]
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and type(value.get("frame")) is int and type(entry.get("last_step")) is int
                       and entry["last_step"] == sum(row["post_frame"] < value["frame"] for row in inputs)
                       and all(value["spa_planning"][key] == entries[0]["value"]["spa_planning"][key]
                               for key in ("anchor", "footprint", "draw_pos")),
                       "Occupied Spa checkpoint subject/geometry differs")
    values = [entry["value"] for entry in entries]
    baseline, saved, loaded = values[4:]
    before, after = spa.context(saved), spa.context(loaded)
    native.require(exact(spa.context(baseline), before), "Occupied Spa paused save changed baseline")
    snapshots = [spa.raw_checkpoint(raw, value) for value in values]
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(a["last_step"] < b["last_step"] and a["value"]["frame"] < b["value"]["frame"]
                           for a, b in zip(entries, entries[1:]))
                   and type(saved.get("world_epoch")) is int and type(loaded.get("world_epoch")) is int
                   and saved["world_epoch"] == baseline["world_epoch"]
                   and loaded["world_epoch"] == saved["world_epoch"] + 1,
                   "Occupied Spa frame/input/epoch order differs")
    native.require(save_path.name == "manual-1.scn.ron" and save_path.parent.name == "saves"
                   and not save_path.is_symlink() and not save_path.parent.is_symlink()
                   and save_path.is_file() and 0 < save_path.stat().st_size <= 64 * 1024 * 1024,
                   "Occupied Spa save path/size differs")
    with save_path.open("rb") as stream:
        data = stream.read(64 * 1024 * 1024 + 1)
    native.require(0 < len(data) <= 64 * 1024 * 1024, "Occupied Spa save read capped")
    receipts = [row for row in raw["events"] if row.get("operation") in ("DurableSave", "DurableLoad")]
    native.require([row.get("operation") for row in receipts] == ["DurableSave", "DurableLoad"]
                   and not any(row.get("operation") in ("SoulSpaConstructionCancel", "SoulSpaSlotsChange")
                               for row in raw["events"]), "Occupied Spa foreign/missing durable operation")
    mapped_durable(before, after, receipts[1].get("spa_entity_remap"))
    lookup = mapped_souls(before, after, receipts[1].get("entity_remap"))
    tracked = list(spa.generating(before))
    loaded_tracked = [lookup[key] for key in tracked]
    native.require(receipts[1].get("previous_epoch") == saved["world_epoch"]
                   and type(receipts[1].get("previous_epoch")) is int,
                   "Occupied Spa previous epoch differs")
    for row, value, op, source in zip(receipts, (saved, loaded), ("save", "load"),
            (r"Manual\(ManualCatalog \{ dialog_session: [1-9][0-9]* \}\)",
             r"Load\(NormalCatalog \{ dialog_session: [1-9][0-9]* \}\)"), strict=True):
        native.require(row.get("producer") == f"ordinary-{op}-commit-v1" and row.get("result") == "Committed"
                       and row.get("target") == "Manual slot 1" and isinstance(row.get("request_source"), str)
                       and re.fullmatch(source, row["request_source"])
                       and row.get("file_sha256") == hashlib.sha256(data).hexdigest()
                       and type(row.get("file_bytes")) is int and row["file_bytes"] == len(data)
                       and type(row.get("world_epoch")) is int and row["world_epoch"] == value["world_epoch"]
                       and type(row.get("sample_index")) is int
                       and end < row["sample_index"] < len(raw["samples"])
                       and spa.finite(row.get("real_seconds")), "Occupied Spa commit binding differs")
        state = spa.context({"spa_planning": {"context": row.get("spa_context")}})
        wanted = spa.context(value)
        identities = tracked if op == "save" else loaded_tracked
        sample = raw["samples"][row["sample_index"]]
        capture_index = spa.raw_checkpoint(raw, value)["sample_index"]
        native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == row["world_epoch"]
                       and sample.get("paused") is True and sample["real_seconds"] >= row["real_seconds"]
                       and 0 <= capture_index - row["sample_index"] < 240
                       and 0 <= value["spa_planning"]["real_seconds"] - row["real_seconds"] <= 480,
                       "Occupied Spa commit sample precedes commit or follows capture")
        sample_state = spa.context({"spa_planning": {"context": sample.get("spa_context")}})
        native.require(exact(durable(state), durable(wanted))
                       and exact(durable(sample_state), durable(state))
                       and exact(worker_values(state, identities), worker_values(wanted, identities))
                       and exact(worker_values(sample_state, identities), worker_values(state, identities)),
                       "Occupied Spa commit durable values differ")
        if op == "load":
            runtime_reset(state)
        else:
            native.require(exact(state, before), "Occupied Spa save commit changed usage state")
        key = "menu:SelectSaveCatalogSlot" if op == "save" else "menu:ConfirmLoadCatalogSlot"
        actions = [event["spa_action"] for event in inputs if event.get("spa_action", {}).get("key", "").startswith(key)]
        native.require(len(actions) == 1, "Occupied Spa commit input missing/duplicate")
        action = actions[0]
        spa.after_press(row, spa.pressed_for(raw, action))
        session = re.search(r"session: ([1-9][0-9]*) \}", action["key"])
        native.require(session is not None and f"dialog_session: {session[1]} }}" in row["request_source"]
                       and action["before"]["spa_planning"]["real_seconds"] < row["real_seconds"]
                       <= value["spa_planning"]["real_seconds"], "Occupied Spa commit request timing differs")
    boundary = receipts[1]["sample_index"]
    native.require(end < receipts[0]["sample_index"] < boundary <= snapshots[-1]["sample_index"]
                   and saved["spa_planning"]["real_seconds"] < receipts[1]["real_seconds"],
                   "Occupied Spa commit order differs")
    tail = raw["samples"][end:snapshots[-1]["sample_index"] + 1]
    native.require(all(a["real_seconds"] < b["real_seconds"] for a, b in zip(tail, tail[1:])),
                   "Occupied Spa raw clock order differs")
    for sample in tail:
        post = sample["sample_index"] >= boundary
        wanted, identities = (after, loaded_tracked) if post else (before, tracked)
        state = spa.context({"spa_planning": {"context": sample.get("spa_context")}})
        native.require(sample.get("paused") is True and type(sample.get("world_epoch")) is int
                       and sample["world_epoch"] == (loaded if post else saved)["world_epoch"]
                       and sample.get("primary_window") == snapshots[4]["primary_window"]
                       and type(sample.get("primary_window_bits")) is int
                       and sample["primary_window_bits"] == snapshots[4]["primary_window_bits"]
                       and spa.finite(sample.get("virtual_seconds"))
                       and sample["virtual_seconds"] == baseline["spa_planning"]["virtual_seconds"]
                       and exact(durable(state), durable(wanted))
                       and exact(worker_values(state, identities), worker_values(wanted, identities)),
                       "Occupied Spa transient durable/time/window change")
        if post:
            runtime_reset(state)
        else:
            native.require(exact(state, before), "Occupied Spa pre-load state changed while paused")


def exercise(driver):
    spa.exercise_usage(driver)
    spa.tap(driver, "F5")
    driver.last = driver.wait(lambda value: any(k.startswith("menu:SelectSaveCatalogSlot") for k in value["controls"]), after=driver.last["frame"])
    spa.click(driver, spa.catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda value: value["catalog"] == "Closed" and "Save:Succeeded" in value["save_outcomes"], after=driver.last["frame"])
    driver.capture(CHECKPOINTS[-2])
    spa.tap(driver, "F9")
    driver.last = driver.wait(lambda value: any(k.startswith("menu:SelectLoadCatalogSlot") for k in value["controls"]), after=driver.last["frame"])
    spa.click(driver, spa.catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    spa.click(driver, spa.catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda value: value["world_epoch"] == epoch + 1 and value["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    spa.pause(driver, True)
    driver.capture(CHECKPOINTS[-1])
    spa.wait_final_raw(driver)
