"""Ordinary unfilled Spa save/load and confirmed cancellation, never art authority."""
import copy
import hashlib
import math
import re

import native_acceptance as native
from ui_progress_bars import catalog_control

CHECKPOINTS = tuple("soul-spa-" + phase for phase in ("empty", "placed", "saved", "loaded", "cancelled"))
COMPLETION_CHECKPOINTS = tuple("soul-spa-completion-" + phase for phase in ("empty", "placed", "completed"))


def cancel_key(owner):
    # Match UiIntent's declared Debug field order, also observed in the real UI.
    return f"menu:CancelSoulSpaConstruction {{ source_task: None, target: {owner} }}"


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def context(value):
    state = value["spa_planning"]["context"]
    native.require(isinstance(state, dict) and state.get("failure") is None
                   and state.get("outcome_channels") == {"construction_cancel": True, "slots_change": True},
                   "Spa context/outcome channels missing")
    for key, cap in (("sites", 512), ("tiles", 2048), ("souls", 128), ("working_relations", 128), ("grids", 512)):
        rows = state.get(key)
        native.require(isinstance(rows, list) and len(rows) <= cap
                       and all(isinstance(row, dict) and isinstance(row.get("entity"), str) and row["entity"] for row in rows)
                       and len({row["entity"] for row in rows}) == len(rows), "Spa inventory missing/duplicated")
    return state


def check_observation(case, value):
    native.require(case in CHECKPOINTS and value.get("paused") is True, "Spa checkpoint/capture pause differs")
    plan, state = value["spa_planning"], context(value)
    anchor = plan.get("anchor")
    native.require(isinstance(anchor, list) and len(anchor) == 2 and all(type(x) is int for x in anchor), "Spa anchor differs")
    x, y = anchor
    footprint = [[x, y], [x + 1, y], [x, y - 1], [x + 1, y - 1]]
    cells = plan.get("cells")
    native.require(plan.get("footprint") == footprint and isinstance(cells, list) and len(cells) == 4
                   and [cell.get("grid") for cell in cells] == footprint
                   and all(cell.get("walkable") is True for cell in cells), "Spa southward walkable footprint differs")
    if case in (CHECKPOINTS[0], CHECKPOINTS[-1]):
        native.require(plan.get("legal") is True and not state["sites"] and not state["tiles"]
                       and all(cell.get("owner") is None for cell in cells), "Spa empty/cancelled ownership differs")
        return
    native.require(plan.get("legal") is False and len(state["sites"]) == 1 and len(state["tiles"]) == 4,
                   "Spa site/four tiles not unique")
    site = state["sites"][0]
    generator = site.get("generator")
    native.require(type(site.get("entity_bits")) is int and site.get("building_kind") == "SoulSpa"
                   and site.get("provisional") is False and site.get("phase") == "Constructing"
                   and type(site.get("bones_required")) is int and site["bones_required"] == 12
                   and type(site.get("bones_delivered")) is int and site["bones_delivered"] == 0
                   and type(site.get("active_slots")) is int and site["active_slots"] == 4
                   and isinstance(generator, dict) and set(generator) == {"current_output", "output_per_soul"}
                   and all(finite(generator[k]) for k in generator)
                   and generator == {"current_output": 0.0, "output_per_soul": 1.0}
                   and sorted(site.get("map_building_grids", [])) == sorted(footprint)
                   and all(cell["owner"] == site["entity_bits"] for cell in cells), "Spa construction values differ")
    native.require(sorted(tile.get("grid") for tile in state["tiles"]) == sorted(footprint)
                   and all(tile.get("parent_site") == site["entity"] and tile.get("parent_exists") is True
                           and type(tile.get("entity_bits")) is int
                           and tile.get("workers") in (None, []) and tile.get("designation") is None
                           and tile.get("task_slots") is None
                           and tile.get("map") == {"owner": site["entity"], "walkable": True}
                           for tile in state["tiles"]), "Spa durable tile state differs")
    targets = {tile["entity"] for tile in state["tiles"]}
    native.require(not any(row.get("target") in targets for row in state["working_relations"]), "unfilled Spa has actual workers")


def raw_checkpoint(raw, value):
    plan, state = value["spa_planning"], context(value)
    rows = [row for row in raw["samples"] if row.get("world_epoch") == value["world_epoch"]
            and row.get("paused") is True and finite(row.get("real_seconds"))
            and abs(row["real_seconds"] - plan["real_seconds"]) <= 1
            and row.get("virtual_seconds") == plan["virtual_seconds"] and row.get("spa_context") == state]
    native.require(rows, "Spa checkpoint has no same-epoch/time/context raw sample")
    return min(rows, key=lambda row: abs(row["real_seconds"] - plan["real_seconds"]))


def selected_state(state):
    return {key: copy.deepcopy(state[key]) for key in ("sites", "tiles", "grids")}


def after_press(receipt, pressed):
    # Receipts and raw input use separate producer cursors. Same-frame equal
    # timestamps are valid even when the receipt is appended before raw input.
    native.require(type(receipt.get("sample_index")) is int and type(pressed.get("sample_index")) is int
                   and receipt["sample_index"] >= pressed["sample_index"]
                   and finite(receipt.get("real_seconds")) and finite(pressed.get("real_seconds"))
                   and (receipt["real_seconds"] >= pressed["real_seconds"]
                        if receipt["sample_index"] == pressed["sample_index"]
                        else receipt["real_seconds"] > pressed["real_seconds"]),
                   "Spa receipt precedes its actual raw press")


def placement_prepress(raw, before, window):
    # The product clears TaskMode on commit. The raw observer samples less often
    # than the UI observer, so bind the latest raw state at/before the UI guard,
    # never a convenient older mode or a post-commit sample.
    now = before["spa_planning"]["real_seconds"]
    samples = [s for s in raw["samples"] if finite(s.get("real_seconds")) and s["real_seconds"] <= now]
    native.require(samples, "Spa pre-press raw context missing")
    sample = max(samples, key=lambda s: s["real_seconds"])
    native.require(0 <= now - sample["real_seconds"] <= 1
                   and sample.get("world_epoch") == before["world_epoch"]
                   and sample.get("primary_window") == window
                   and sample.get("primary_window_bits") == before["primary_window"]
                   and sample.get("menu_state") == "Architect"
                   and sample.get("task_mode", "").startswith("SoulSpaPlace("),
                   "Spa pre-press raw mode/window/epoch differs")


def pressed_for(raw, action):
    start, end = (action[side]["spa_planning"]["real_seconds"] for side in ("before", "after"))
    rows = [e for e in raw["events"] if e.get("producer") == "MouseButtonInput" and e.get("state") == "Pressed"
            and start < e.get("real_seconds", -1) <= end]
    native.require(len(rows) == 1, "Spa action raw press is ambiguous")
    return rows[0]


def check_mapping(before, after, mapping):
    def indexed(state):
        result = {}
        for key, kind in (("sites", "SoulSpa"), ("tiles", "SoulSpaTile"), ("grids", "PowerGrid")):
            for row in state[key]:
                native.require(row["entity"] not in result and type(row.get("entity_bits")) is int,
                               "Spa durable identity duplicated/missing bits")
                result[row["entity"]] = (kind, row)
        return result
    old, new = indexed(before), indexed(after)
    native.require(isinstance(mapping, list) and len(mapping) == len(old) == len(new), "Spa durable remap inventory differs")
    lookup = {}
    for row in mapping:
        a, b = row.get("saved_entity"), row.get("loaded_entity")
        native.require(a in old and b in new and a not in lookup and b not in lookup.values()
                       and row.get("kind") == old[a][0] == new[b][0]
                       and type(row.get("saved_bits")) is int and row["saved_bits"] == old[a][1]["entity_bits"]
                       and type(row.get("loaded_bits")) is int and row["loaded_bits"] == new[b][1]["entity_bits"],
                       "Spa durable remap is foreign/duplicate/mistyped")
        lookup[a] = b
    for entity, (kind, row) in old.items():
        translated = {k: copy.deepcopy(v) for k, v in row.items() if k not in ("entity", "entity_bits")}
        if kind == "SoulSpa":
            native.require(row.get("grid") is None or row["grid"] in lookup, "Spa grid remap missing")
            translated["grid"] = lookup[row["grid"]] if row["grid"] is not None else None
        elif kind == "SoulSpaTile":
            native.require(row["parent_site"] in lookup and row["map"]["owner"] in lookup, "Spa tile parent remap missing")
            translated["parent_site"] = lookup[row["parent_site"]]
            translated["map"]["owner"] = lookup[row["map"]["owner"]]
        else:
            generators = row.get("generators")
            native.require(generators is None or isinstance(generators, list)
                           and len(generators) == len(set(generators)) and all(g in lookup for g in generators),
                           "Spa grid generator remap missing/duplicated")
            translated["generators"] = sorted(lookup[g] for g in generators) if generators is not None else None
        observed = {k: copy.deepcopy(v) for k, v in new[lookup[entity]][1].items() if k not in ("entity", "entity_bits")}
        if kind == "PowerGrid" and observed.get("generators") is not None:
            observed["generators"] = sorted(observed["generators"])
        native.require(translated == observed, "Spa durable restored state differs")
    return lookup


def check_raw_session(raw, client):
    native.require(raw.get("scope") == "production-raw-observations-v1" and raw.get("schema_version") == 2
                   and raw.get("failure") is None and raw.get("nonce") == client["nonce"]
                   and raw.get("process_id") == client["pid"]
                   and all(raw.get(k) is False for k in ("accepted", "performance_evidence", "promotion_authority")),
                   "Spa raw session/authority differs")
    samples, events = raw["samples"], raw["events"]
    native.require(0 < len(samples) <= 1200 and len(events) < 4096
                   and all(row.get("sample_index") == i and type(row.get("sample_index")) is int
                           and finite(row.get("real_seconds")) for i, row in enumerate(samples))
                   and all(row.get("event_index") == i and type(row.get("event_index")) is int for i, row in enumerate(events)),
                   "Spa raw ordinal/cap differs")


def verify(raw, entries, client, save_path, inputs):
    native.require([row["case"] for row in entries] == list(CHECKPOINTS), "Spa checkpoint inventory differs")
    check_raw_session(raw, client)
    samples, events = raw["samples"], raw["events"]
    values = [row["value"] for row in entries]
    empty, placed, saved, loaded, cancelled = values
    for case, value in zip(CHECKPOINTS, values, strict=True):
        check_observation(case, value)
        native.require(value.get("pid") == client["pid"] and value.get("nonce") == client["nonce"]
                       and all(value["spa_planning"][k] == empty["spa_planning"][k] for k in ("anchor", "footprint", "draw_pos")),
                       "Spa checkpoint process/position differs")
    native.require(all(type(e.get("last_step")) is int and e["last_step"] >= 0
                       and type(e["value"].get("frame")) is int and e["value"]["frame"] > 90
                       and type(e["value"].get("world_epoch")) is int and e["value"]["world_epoch"] >= 0
                       for e in entries), "Spa checkpoint ordinals are mistyped")
    snapshots = [raw_checkpoint(raw, value) for value in values]
    check_inputs(raw, entries, client, inputs)
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(b["value"]["frame"] > a["value"]["frame"] and b["last_step"] > a["last_step"]
                           for a, b in zip(entries, entries[1:]))
                   and placed["world_epoch"] == empty["world_epoch"] == saved["world_epoch"]
                   and loaded["world_epoch"] == saved["world_epoch"] + 1 == cancelled["world_epoch"],
                   "Spa checkpoint time/input/epoch order differs")
    native.require(selected_state(context(placed)) == selected_state(context(saved)), "Spa paused save changed construction")
    native.require(save_path.name == "manual-1.scn.ron" and save_path.parent.name == "saves"
                   and save_path.is_file() and not save_path.is_symlink() and not save_path.parent.is_symlink()
                   and 0 < save_path.stat().st_size <= 64 * 1024 * 1024, "Spa save bytes missing/unsafe/capped")
    with save_path.open("rb") as stream:
        data = stream.read(64 * 1024 * 1024 + 1)
    native.require(0 < len(data) <= 64 * 1024 * 1024, "Spa save read capped")
    receipts = [e for e in events if e.get("operation") in ("DurableSave", "DurableLoad")]
    native.require([e.get("operation") for e in receipts] == ["DurableSave", "DurableLoad"], "Spa durable commits missing/duplicate")
    for row, value, op, source in zip(receipts, (saved, loaded), ("save", "load"),
                                    (r"Manual\(ManualCatalog \{ dialog_session: [1-9][0-9]* \}\)",
                                     r"Load\(NormalCatalog \{ dialog_session: [1-9][0-9]* \}\)")):
        native.require(row.get("producer") == f"ordinary-{op}-commit-v1" and row.get("result") == "Committed"
                       and row.get("target") == "Manual slot 1" and isinstance(row.get("request_source"), str)
                       and re.fullmatch(source, row["request_source"])
                       and row.get("file_sha256") == hashlib.sha256(data).hexdigest()
                       and type(row.get("file_bytes")) is int and row["file_bytes"] == len(data)
                       and row.get("world_epoch") == value["world_epoch"]
                       and type(row.get("sample_index")) is int and 0 <= row["sample_index"] < len(samples)
                       and row.get("spa_context") == context(value), "Spa durable commit bytes/context differs")
        sample = samples[row["sample_index"]]
        native.require(sample.get("world_epoch") == row["world_epoch"] and sample.get("paused") is True
                       and finite(row.get("real_seconds")) and row["real_seconds"] <= sample["real_seconds"]
                       and sample.get("spa_context") == row["spa_context"], "Spa durable commit sample differs")
        start, end = row["sample_index"], raw_checkpoint(raw, value)["sample_index"]
        native.require(0 <= end - start < 240 and 0 <= value["spa_planning"]["real_seconds"] - row["real_seconds"] <= 480
                       and all(s.get("paused") is True and s.get("world_epoch") == value["world_epoch"]
                               and s.get("virtual_seconds") == value["spa_planning"]["virtual_seconds"]
                               and s.get("spa_context") == context(value) for s in samples[start:end + 1]),
                       "Spa durable interval changes paused state")
        prefix = "menu:SelectSaveCatalogSlot" if op == "save" else "menu:ConfirmLoadCatalogSlot"
        actions = [e["spa_action"] for e in inputs if e.get("spa_action", {}).get("key", "").startswith(prefix)]
        native.require(len(actions) == 1, "Spa durable request input missing/duplicate")
        action = actions[0]
        after_press(row, pressed_for(raw, action))
        session = re.search(r"session: ([1-9][0-9]*) \}", action["key"])
        native.require(session is not None and f"dialog_session: {session[1]} }}" in row["request_source"]
                       and action["before"]["spa_planning"]["real_seconds"] < row["real_seconds"]
                       <= value["spa_planning"]["real_seconds"], "Spa durable request/input interval differs")
    native.require(receipts[0]["sample_index"] < receipts[1]["sample_index"]
                   and receipts[0]["real_seconds"] < receipts[1]["real_seconds"]
                   and receipts[0]["real_seconds"] > placed["spa_planning"]["real_seconds"]
                   and receipts[1]["real_seconds"] > saved["spa_planning"]["real_seconds"],
                   "Spa durable commits do not follow actual requests")
    lookup = check_mapping(context(saved), context(loaded), receipts[1].get("spa_entity_remap"))
    old, owner = context(saved)["sites"][0], context(loaded)["sites"][0]
    native.require(old["entity"] != owner["entity"] and lookup[old["entity"]] == owner["entity"]
                   and receipts[1].get("previous_epoch") == saved["world_epoch"], "Spa actual owner remap missing")
    outcomes = [e for e in events if e.get("producer") == "SoulSpaConstructionCancelOutcome"]
    native.require(len(outcomes) == 1, "Spa cancellation outcome missing/duplicate")
    from scripts import building_production_lifecycle as lifecycle
    outcome = outcomes[0]
    lifecycle.check_spa_outcome(outcome, samples)
    confirms = [e["spa_action"] for e in inputs if e.get("spa_action", {}).get("key", "").startswith("menu:ConfirmSoulSpaConstructionCancel")]
    native.require(len(confirms) == 1
                   and confirms[0]["before"]["spa_planning"]["real_seconds"] < outcome["real_seconds"]
                   <= cancelled["spa_planning"]["real_seconds"], "Spa cancel outcome is not from the confirmed input interval")
    after_press(outcome, pressed_for(raw, confirms[0]))
    native.require(outcome["owner"] == owner["entity"] and outcome["result"] == "Canceled { refunded_bones: 0 }"
                   and outcome["refunded_bones"] == 0
                   and snapshots[3]["sample_index"] < outcome["sample_index"] <= snapshots[4]["sample_index"]
                   and samples[outcome["sample_index"]].get("world_epoch") == loaded["world_epoch"],
                   "Spa cancel result/owner/epoch differs")
    removed = {owner["entity"], *(tile["entity"] for tile in context(loaded)["tiles"])}
    native.require(not any(row.get("target") in removed for row in context(cancelled)["working_relations"])
                   and not any(g in removed for row in context(cancelled)["grids"] for g in (row.get("generators") or [])),
                   "Spa cancellation leaves actual relation sources")


def input_context(value, key=None):
    result = {k: copy.deepcopy(value[k]) for k in (
        "frame", "nonce", "pid", "world_epoch", "primary_window", "paused", "cursor", "scale_factor",
        "mouse_left_pressed", "world_input_captured", "pointer_over_ui", "spa_planning", "viewport", "menu_state")}
    result["control"] = copy.deepcopy(value["controls"].get(key)) if key else None
    return result


def record_action(driver, key, first, before):
    driver.events[first]["spa_action"] = {"key": key, "first_step": first + 1,
        "last_step": len(driver.events), "before": before, "after": input_context(driver.last)}
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def click(driver, key):
    driver.send(point=driver.point(key))
    native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered",
                   "Spa target moved or is occluded; no press sent")
    before, first = input_context(driver.last, key), len(driver.events)
    driver.send(button=1, pressed=True)
    native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed", "Spa target press missing")
    driver.send(button=1, pressed=False)
    record_action(driver, key, first, before)


def tap(driver, key):
    before, first = input_context(driver.last), len(driver.events)
    driver.key(key)
    # Last-stage raw observation may trail the UI ACK. Wait for the real
    # press/release pair before ending this action; never resend the key.
    def recorded(value):
        path = driver.root / "production-cancellation.json"
        return path.is_file() and keyboard_recorded(native.read_json(path), before, value, key)
    driver.last = driver.wait(recorded, after=driver.last["frame"], timeout=10)
    record_action(driver, key, first, before)


def keyboard_recorded(raw, before, value, key):
    native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                   and raw.get("process_id") == before["pid"], "Spa keyboard raw owner/failure differs")
    rows = [e for e in raw.get("events", []) if e.get("producer") == "KeyboardInput"
            and finite(e.get("real_seconds"))
            and before["spa_planning"]["real_seconds"] < e["real_seconds"] <= value["spa_planning"]["real_seconds"]]
    return (len(rows) == 2 and [r.get("state") for r in rows] == ["Pressed", "Released"]
            and all(r.get("key") == key and r.get("repeat") is False for r in rows))


def pause(driver, paused):
    if driver.last["paused"] != paused:
        click(driver, "menu:SetTimeSpeed(Paused)" if paused else "menu:SetTimeSpeed(Normal)")
        driver.last = driver.wait(lambda value: value["paused"] == paused, after=driver.last["frame"])


def world_click(driver, name, projection):
    before = input_context(driver.last)
    point = before["spa_planning"][projection]
    factor = before["scale_factor"]
    native.require(isinstance(point, list) and len(point) == 2 and all(finite(x) for x in point)
                   and finite(factor) and factor > 0, "Spa world projection missing")
    physical = tuple(round(x * factor) for x in point)
    native.require(all(0 < x < limit for x, limit in zip(physical, driver.last["viewport"], strict=True)),
                   "Spa world projection outside owned client")
    driver.send(point=physical)
    current = driver.last
    native.require(current["world_epoch"] == before["world_epoch"] and current["primary_window"] == before["primary_window"]
                   and current["spa_planning"][projection] == point and not current["pointer_over_ui"]
                   and not current["world_input_captured"] and current["cursor"] is not None
                   and all(abs(a - b) <= 2 for a, b in zip(current["cursor"], physical, strict=True)),
                   "Spa world target changed before press")
    if name == "placement":
        native.require(current["spa_planning"]["legal"] is True
                       and current["spa_planning"]["footprint"] == before["spa_planning"]["footprint"]
                       and current["spa_planning"]["task_mode"].startswith("SoulSpaPlace(")
                       and current["menu_state"] == "Architect", "Spa placement context changed")
    before, first = input_context(current), len(driver.events)
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    record_action(driver, name, first, before)


def check_inputs(raw, entries, client, inputs, *, completion=False):
    native.require(isinstance(inputs, list) and 0 < len(inputs) <= 256
                   and [row.get("step") for row in inputs] == [str(i + 1) for i in range(len(inputs))], "Spa input inventory differs")
    actions = [row["spa_action"] for row in inputs if "spa_action" in row]
    native.require(all(row["spa_action"].get("first_step") == i + 1 for i, row in enumerate(inputs) if "spa_action" in row)
                   and all(a["after"]["spa_planning"]["real_seconds"] <= b["before"]["spa_planning"]["real_seconds"]
                           for a, b in zip(actions, actions[1:])), "Spa action annotation/order differs")
    claimed, raw_claimed = set(), set()
    first_sample = raw_checkpoint(raw, entries[0]["value"])
    expected_window, window_bits = first_sample["primary_window"], first_sample.get("primary_window_bits")
    native.require(isinstance(expected_window, str) and expected_window
                   and type(window_bits) is int and window_bits >= 0
                   and all(type(row["value"].get("primary_window")) is int
                           and row["value"]["primary_window"] == window_bits for row in entries),
                   "Spa UI/raw primary window identity differs")
    for action in actions:
        key, a, b = action["key"], action["before"], action["after"]
        first, last = action["first_step"], action["last_step"]
        native.require(type(first) is int and type(last) is int and last == first + 1
                       and 1 <= first < last <= len(inputs) and not claimed.intersection({first, last}), "Spa input action is duplicate/unbounded")
        claimed.update((first, last))
        sent = inputs[first - 1:last]
        native.require(all(v.get("nonce") == client["nonce"] and v.get("pid") == client["pid"] for v in (a, b))
                       and type(a.get("world_epoch")) is int and type(b.get("world_epoch")) is int
                       and (a["world_epoch"] <= b["world_epoch"] <= a["world_epoch"] + 1
                            if key.startswith("menu:ConfirmLoadCatalogSlot ") else a["world_epoch"] == b["world_epoch"])
                       and type(a.get("primary_window")) is int and type(b.get("primary_window")) is int
                       and a["primary_window"] == b["primary_window"] == window_bits
                       and type(a.get("frame")) is int and b["frame"] > a["frame"]
                       and all(row.get("nonce") == client["nonce"] and row.get("pid") == client["pid"]
                               and row.get("window") == client["window"]
                               and row.get("world_epoch") in (a["world_epoch"], b["world_epoch"])
                               and a["frame"] <= row.get("pre_frame", -1)
                               < row.get("post_frame", -1) <= b["frame"] for row in sent), "Spa action owner/frame differs")
        start, end = a["spa_planning"]["real_seconds"], b["spa_planning"]["real_seconds"]
        native.require(finite(start) and finite(end) and 0 <= start < end, "Spa action time differs")
        keyboard = key in {"F5", "F9", "Escape"}
        native.require([row.get("pressed") for row in sent] == [True, False]
                       and all(type(row.get("pressed")) is bool for row in sent)
                       and all(row.get("key") == key if keyboard else type(row.get("button")) is int and row["button"] == 1 for row in sent),
                       "Spa sent press/release differs")
        rows = [row for row in raw["events"] if row.get("producer") == ("KeyboardInput" if keyboard else "MouseButtonInput")
                and finite(row.get("real_seconds")) and start < row["real_seconds"] <= end]
        native.require(len(rows) == 2 and [row.get("state") for row in rows] == ["Pressed", "Released"], "Spa action has no unique raw press/release")
        for row in rows:
            index = row.get("sample_index")
            native.require(type(index) is int and 0 <= index < len(raw["samples"])
                           and row.get("event_index") not in raw_claimed, "Spa raw input reused/outside samples")
            raw_claimed.add(row["event_index"])
            sample = raw["samples"][index]
            native.require(row.get("window") == sample.get("primary_window") == expected_window
                           and type(sample.get("primary_window_bits")) is int and sample["primary_window_bits"] == window_bits
                           and type(sample.get("world_epoch")) is int
                           and sample.get("world_epoch") in (a["world_epoch"], b["world_epoch"])
                           and sample.get("real_seconds", -1) >= row["real_seconds"]
                           and (row.get("key") == key and row.get("repeat") is False if keyboard else row.get("button") == "Left"),
                           "Spa raw input window/epoch/key differs")
            if not keyboard:
                cursor, dpi = sample.get("cursor_position"), sample.get("dpi")
                native.require(isinstance(cursor, list) and len(cursor) == 2 and all(finite(x) for x in cursor)
                               and finite(dpi) and dpi > 0 and dpi == a.get("scale_factor") and isinstance(a.get("cursor"), (tuple, list))
                               and all(abs(x * dpi - y) <= 2 for x, y in zip(cursor, a["cursor"], strict=True)),
                               "Spa raw pointer differs")
        if not keyboard and (key.startswith("menu:") or key == "dev-minimize"):
            control = a.get("control")
            rect = control.get("rect") if isinstance(control, dict) else None
            native.require(isinstance(control, dict) and control.get("fully_visible") is True
                           and isinstance(rect, list) and len(rect) == 4 and all(finite(x) for x in rect)
                           and 0 <= rect[0] < rect[2] <= a["viewport"][0] and 0 <= rect[1] < rect[3] <= a["viewport"][1]
                           and rect[0] <= a["cursor"][0] <= rect[2] and rect[1] <= a["cursor"][1] <= rect[3]
                           and control.get("interaction") == "Hovered" and sent[0].get("observed_hover", {}).get(key) == "Pressed",
                           "Spa actual control target missing")
        if key == "placement":
            native.require(a["spa_planning"]["legal"] is True and not a["pointer_over_ui"] and not a["world_input_captured"]
                           and a.get("menu_state") == "Architect"
                           and a["spa_planning"].get("task_mode", "").startswith("SoulSpaPlace("),
                           "Spa placement foreground differs")
            placement_prepress(raw, a, expected_window)
            receipts = [r for r in raw["events"] if r.get("producer") == "ordinary-building-placement-branch-v1"
                        and start < r.get("real_seconds", -1) <= end]
            native.require(len(receipts) == 1, "Spa actual placement receipt missing/duplicate")
            after_press(receipts[0], rows[0])
            from scripts import building_production_lifecycle as lifecycle
            lifecycle.check_placement_receipt(receipts[0], raw["samples"])
            result = receipts[0]["outcome"]
            native.require(result.get("kind") == "SoulSpa" and result.get("status") == "BuildingCommitted"
                           and result.get("grid") == a["spa_planning"]["anchor"]
                           and result.get("footprint") == a["spa_planning"]["footprint"]
                           and set(result["tile_owners"]) == {t["entity"] for t in context(entries[1]["value"])["tiles"]}
                           and result.get("owner") == context(entries[1]["value"])["sites"][0]["entity"],
                           "Spa placement target/mode differs")
    native.require(all(i + 1 in claimed for i, row in enumerate(inputs) if row.get("button") is not None or row.get("key") is not None)
                   and raw_claimed == {r["event_index"] for r in raw["events"] if r.get("producer") in ("KeyboardInput", "MouseButtonInput")},
                   "Spa unclaimed or extraneous input")
    required = [a for a in actions if a["key"] not in {"Escape", "dev-minimize", "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Normal)"}]
    filtered = [a["key"] for a in required]
    if completion:
        patterns = [r"menu:ToggleArchitect", r"menu:SelectArchitectCategory\(Some\(Plant\)\)",
                    r"menu:SelectTaskMode\(SoulSpaPlace\(None\)\)", "placement"]
        native.require(len(filtered) == len(patterns)
                       and all(re.fullmatch(p, k) for p, k in zip(patterns, filtered, strict=True)),
                       "Spa completion input sequence differs")
        lower, upper = entries[:2]
        for action in required:
            a, b = action["before"], action["after"]
            native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                           and lower["value"]["spa_planning"]["real_seconds"] <= a["spa_planning"]["real_seconds"]
                           < b["spa_planning"]["real_seconds"] <= upper["value"]["spa_planning"]["real_seconds"]
                           and a["world_epoch"] == b["world_epoch"] == lower["value"]["world_epoch"],
                           "Spa completion placement crosses checkpoint phase")
        native.require(all(entries[0]["value"]["frame"] <= a["before"]["frame"]
                           < a["after"]["frame"] < entries[-1]["value"]["frame"] for a in actions),
                       "Spa completion input outside checkpoints")
        return
    owner = context(entries[3]["value"])["sites"][0]["entity"]
    patterns = [r"menu:ToggleArchitect", r"menu:SelectArchitectCategory\(Some\(Plant\)\)",
                r"menu:SelectTaskMode\(SoulSpaPlace\(None\)\)", "placement", "F5",
                r"menu:SelectSaveCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}", "F9",
                r"menu:SelectLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}",
                r"menu:ConfirmLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}", "select-loaded",
                re.escape(cancel_key(owner)),
                re.escape(f"menu:ConfirmSoulSpaConstructionCancel {{ target: {owner}, epoch: {entries[3]['value']['world_epoch']}, ticket: ") + r"[1-9][0-9]* \}"]
    native.require(len(filtered) == len(patterns) and all(re.fullmatch(pattern, key) for pattern, key in zip(patterns, filtered, strict=True)),
                   "Spa exact control/confirmation sequence differs")
    for index, action in enumerate(required):
        phase = 0 if index < 4 else 1 if index < 6 else 2 if index < 9 else 3
        lower, upper = entries[phase:phase + 2]
        a, b = action["before"], action["after"]
        native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                       and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                       and lower["value"]["spa_planning"]["real_seconds"] <= a["spa_planning"]["real_seconds"]
                       < b["spa_planning"]["real_seconds"] <= upper["value"]["spa_planning"]["real_seconds"]
                       and a["world_epoch"] == lower["value"]["world_epoch"]
                       and b["world_epoch"] in ({a["world_epoch"], upper["value"]["world_epoch"]} if index == 8 else {a["world_epoch"]}),
                       "Spa required action crosses its checkpoint phase")


def exercise_placement(driver, checkpoints):
    if "text-field:DevPoc" in driver.last["controls"]:
        click(driver, "dev-minimize")
    driver.capture(checkpoints[0])
    pause(driver, False)
    click(driver, "menu:ToggleArchitect")
    click(driver, "menu:SelectArchitectCategory(Some(Plant))")
    click(driver, "menu:SelectTaskMode(SoulSpaPlace(None))")
    world_click(driver, "placement", "point")
    driver.last = driver.wait(lambda value: len(context(value)["sites"]) == 1, after=driver.last["frame"])
    tap(driver, "Escape")
    pause(driver, True)
    driver.capture(checkpoints[1])


def exercise(driver):
    exercise_placement(driver, CHECKPOINTS)
    tap(driver, "F5")
    driver.last = driver.wait(lambda v: any(k.startswith("menu:SelectSaveCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    click(driver, catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda v: v["catalog"] == "Closed" and "Save:Succeeded" in v["save_outcomes"], after=driver.last["frame"])
    driver.capture(CHECKPOINTS[2])
    tap(driver, "F9")
    driver.last = driver.wait(lambda v: any(k.startswith("menu:SelectLoadCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    click(driver, catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    click(driver, catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda v: v["world_epoch"] == epoch + 1 and v["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    pause(driver, True)
    driver.capture(CHECKPOINTS[3])
    # Select the loaded site through its normal world target, while paused.
    world_click(driver, "select-loaded", "owner_point")
    owner = context(driver.last)["sites"][0]["entity"]
    action = cancel_key(owner)
    driver.last = driver.wait(lambda v: action in v["controls"], after=driver.last["frame"])
    pause(driver, False)
    click(driver, action)
    prefix = f"menu:ConfirmSoulSpaConstructionCancel {{ target: {owner}, epoch: {epoch + 1}, ticket: "
    driver.last = driver.wait(lambda v: any(k.startswith(prefix) for k in v["controls"]), after=driver.last["frame"])
    matches = [k for k in driver.last["controls"] if k.startswith(prefix)]
    native.require(len(matches) == 1, "Spa confirmation ambiguous")
    click(driver, matches[0])
    driver.last = driver.wait(lambda v: not context(v)["sites"] and not context(v)["tiles"], after=driver.last["frame"])
    pause(driver, True)
    driver.capture(CHECKPOINTS[4])
    wait_final_raw(driver)


def wait_final_raw(driver):
    # UI snapshots precede Last; wait for the exact final raw state, never resend input.
    value = driver.checkpoints[-1]["value"]
    def recorded(_):
        path = driver.root / "production-cancellation.json"
        if not path.is_file():
            return False
        try:
            raw_checkpoint(native.read_json(path), value)
        except native.AcceptanceError:
            return False
        return True
    driver.wait(recorded, after=driver.last["frame"], timeout=10)


def check_completion_observation(case, value):
    native.require(case in COMPLETION_CHECKPOINTS, "Spa completion checkpoint differs")
    if case != COMPLETION_CHECKPOINTS[-1]:
        check_observation("soul-spa-" + case.rsplit("-", 1)[-1], value)
        return
    state, plan = context(value), value["spa_planning"]
    native.require(value.get("paused") is True and plan.get("legal") is False
                   and len(state["sites"]) == 1 and len(state["tiles"]) == 4, "Spa completion site inventory differs")
    site = state["sites"][0]
    x, y = plan["anchor"]
    footprint = [[x, y], [x + 1, y], [x, y - 1], [x + 1, y - 1]]
    native.require(plan["footprint"] == footprint and sorted(site.get("map_building_grids", [])) == sorted(footprint)
                   and site.get("phase") == "Operational" and site.get("building_kind") == "SoulSpa"
                   and site.get("provisional") is False and type(site.get("entity_bits")) is int
                   and all(type(site.get(k)) is int for k in ("bones_required", "bones_delivered", "active_slots"))
                   and site["bones_required"] == site["bones_delivered"] == 12 and site["active_slots"] == 4
                   and [c.get("grid") for c in plan["cells"]] == footprint
                   and all(c.get("owner") == site["entity_bits"] and c.get("walkable") is True for c in plan["cells"]),
                   "Spa completion values/footprint differ")
    native.require(sorted(t.get("grid") for t in state["tiles"]) == sorted(footprint), "Spa completion tile grid differs")
    workers, active = set(), 0
    souls = {s["entity"]: s for s in state["souls"]}
    targets = {t["entity"] for t in state["tiles"]}
    inverse = {(r["entity"], r["target"]) for r in state["working_relations"] if r.get("target") in targets}
    expected = set()
    for tile in state["tiles"]:
        current = tile.get("workers")
        native.require(type(tile.get("entity_bits")) is int and tile.get("parent_exists") is True
                       and tile.get("parent_site") == site["entity"] and tile.get("designation") == "GeneratePower"
                       and type(tile.get("task_slots")) is int and tile["task_slots"] == 1
                       and tile.get("map") == {"owner": site["entity"], "walkable": True}
                       and (current is None or isinstance(current, list) and len(current) <= 1),
                       "Spa completion tile contract differs")
        for worker in current or []:
            soul = souls.get(worker, {})
            task = soul.get("generate_power")
            native.require(worker not in workers and soul.get("working_on") == tile["entity"]
                           and isinstance(task, dict) and task.get("tile") == tile["entity"]
                           and task.get("phase") in ("GoingToTile", "Generating")
                           and all(finite(soul.get(k)) for k in ("dream", "fatigue", "stress")),
                           "Spa completion worker/task differs")
            workers.add(worker)
            expected.add((worker, tile["entity"]))
        active += bool(current)
    native.require(inverse == expected and all(r.get("is_soul") is True for r in state["working_relations"]
                                             if r.get("target") in targets), "Spa completion worker inverse differs")
    generator = site.get("generator")
    grids = [g for g in state["grids"] if g["entity"] == site.get("grid")]
    native.require(isinstance(generator, dict) and all(finite(generator.get(k)) for k in ("current_output", "output_per_soul"))
                   and generator["output_per_soul"] == 1 and generator["current_output"] == active
                   and len(grids) == 1 and type(grids[0].get("entity_bits")) is int
                   and grids[0].get("generators") == [site["entity"]]
                   and finite(grids[0].get("generation")) and grids[0]["generation"] == active,
                   "Spa completion grid/output differs")


def verify_completion(raw, entries, client, inputs):
    native.require([e["case"] for e in entries] == list(COMPLETION_CHECKPOINTS), "Spa completion inventory differs")
    check_raw_session(raw, client)
    snapshots = []
    for entry in entries:
        value = entry["value"]
        check_completion_observation(entry["case"], value)
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and type(value.get("frame")) is int and value["frame"] > 90
                       and type(value.get("world_epoch")) is int and value["world_epoch"] == entries[0]["value"]["world_epoch"]
                       and type(entry.get("last_step")) is int and entry["last_step"] >= 0
                       and all(value["spa_planning"][k] == entries[0]["value"]["spa_planning"][k]
                               for k in ("anchor", "footprint", "draw_pos")), "Spa completion subject/position differs")
        snapshots.append(raw_checkpoint(raw, value))
    check_inputs(raw, entries, client, inputs, completion=True)
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(a["value"]["frame"] < b["value"]["frame"] and a["last_step"] < b["last_step"]
                           for a, b in zip(entries, entries[1:])), "Spa completion ordering differs")
    placed, completed = [context(e["value"]) for e in entries[1:]]
    owner = placed["sites"][0]
    native.require(all(completed["sites"][0][k] == owner[k] for k in ("entity", "entity_bits", "pose"))
                   and {(t["entity"], t["entity_bits"]) for t in placed["tiles"]}
                   == {(t["entity"], t["entity_bits"]) for t in completed["tiles"]}, "Spa completion owner replaced")
    actions = [r["spa_action"] for r in inputs if "spa_action" in r
               and r["spa_action"]["before"]["frame"] >= entries[1]["value"]["frame"]]
    native.require([a["key"] for a in actions] == ["menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"]
                   and actions[0]["before"]["paused"] is True and actions[0]["after"]["paused"] is False
                   and actions[1]["before"]["paused"] is False and actions[1]["after"]["paused"] is True,
                   "Spa natural progress has no exact normal/pause input interval")
    start = pressed_for(raw, actions[0])["real_seconds"]
    end = actions[1]["after"]["spa_planning"]["real_seconds"]
    amounts = []
    interval = raw["samples"][snapshots[1]["sample_index"]:snapshots[2]["sample_index"] + 1]
    def shape(tiles):
        keys = ("entity", "entity_bits", "parent_site", "parent_exists", "grid", "pose", "map")
        return sorted([{k: t.get(k) for k in keys} for t in tiles], key=lambda t: t["entity"])
    expected_shape = shape(placed["tiles"])
    for sample in interval:
        sites = sample.get("spa_context", {}).get("sites", [])
        tiles = sample.get("spa_context", {}).get("tiles", [])
        native.require(type(sample.get("world_epoch")) is int
                       and sample["world_epoch"] == entries[0]["value"]["world_epoch"]
                       and sample.get("primary_window") == snapshots[0]["primary_window"]
                       and type(sample.get("primary_window_bits")) is int
                       and sample.get("primary_window_bits") == snapshots[0]["primary_window_bits"]
                       and finite(sample.get("real_seconds")) and finite(sample.get("virtual_seconds"))
                       and sample["real_seconds"] >= 0 and sample["virtual_seconds"] >= 0
                       and type(sample.get("paused")) is bool
                       and (sample["paused"] or start <= sample["real_seconds"] <= end)
                       and len(sites) == 1 and all(sites[0].get(k) == owner[k] for k in (
                           "entity", "entity_bits", "pose", "map_building_grids", "grid", "bones_required", "active_slots"))
                       and len(tiles) == 4 and shape(tiles) == expected_shape
                       and type(sites[0].get("bones_delivered")) is int
                       and 0 <= sites[0]["bones_delivered"] <= 12
                       and sites[0].get("phase") == ("Operational" if sites[0]["bones_delivered"] == 12 else "Constructing"),
                       "Spa natural construction continuity differs")
        amounts.append(sites[0]["bones_delivered"])
    native.require(amounts[0] == 0 and amounts[-1] == 12 and amounts == sorted(amounts)
                   and all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                           and (not (a["paused"] and b["paused"])
                                or a["virtual_seconds"] == b["virtual_seconds"]
                                and a["spa_context"]["sites"][0]["bones_delivered"] == b["spa_context"]["sites"][0]["bones_delivered"])
                           for a, b in zip(interval, interval[1:]))
                   and any(s["paused"] is False and start <= s["real_seconds"] <= end
                           and s["virtual_seconds"] > snapshots[1]["virtual_seconds"] for s in interval)
                   and snapshots[2]["virtual_seconds"] > snapshots[1]["virtual_seconds"]
                   and 0 < snapshots[2]["real_seconds"] - snapshots[1]["real_seconds"] <= 190
                   and not any(e.get("operation") in ("DurableSave", "DurableLoad", "SoulSpaConstructionCancel", "SoulSpaSlotsChange")
                               for e in raw["events"]), "Spa completion elapsed/progress/operation differs")


def exercise_completion(driver):
    exercise_placement(driver, COMPLETION_CHECKPOINTS)
    pause(driver, False)
    driver.last = driver.wait(lambda v: len(context(v)["sites"]) == 1
                              and context(v)["sites"][0].get("phase") == "Operational", after=driver.last["frame"], timeout=180)
    pause(driver, True)
    driver.capture(COMPLETION_CHECKPOINTS[2])
    wait_final_raw(driver)
