"""Ordinary unfilled Spa save/load and confirmed cancellation, never art authority."""
import copy
import hashlib
import math
import re

import native_acceptance as native
from ui_progress_bars import catalog_control

CHECKPOINTS = tuple("soul-spa-" + phase for phase in ("empty", "placed", "saved", "loaded", "cancelled"))
COMPLETION_CHECKPOINTS = tuple("soul-spa-completion-" + phase for phase in ("empty", "placed", "completed"))
USAGE_CHECKPOINTS = COMPLETION_CHECKPOINTS + ("soul-spa-using", "soul-spa-used")
PERSISTENCE_CHECKPOINTS = COMPLETION_CHECKPOINTS + ("soul-spa-operational-saved", "soul-spa-operational-loaded")
SLOTS_CHECKPOINTS = USAGE_CHECKPOINTS + ("soul-spa-slots-closed", "soul-spa-slots-held")


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


def check_raw_session(raw, client, *, raw_scope="production-raw-observations-v1"):
    native.require(raw_scope in ("production-raw-observations-v1", "ordinary-spa-candidate-occupied-v1", "ordinary-spa-candidate-resumed-v1", "ordinary-spa-candidate-load-v1"),
                   "unsupported Spa raw scope")
    native.require(raw.get("scope") == raw_scope and raw.get("schema_version") == 2
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


def verify(raw, entries, client, save_path, inputs, *, persistence=False):
    expected = PERSISTENCE_CHECKPOINTS if persistence else CHECKPOINTS
    native.require([row["case"] for row in entries] == list(expected), "Spa checkpoint inventory differs")
    check_raw_session(raw, client)
    samples, events = raw["samples"], raw["events"]
    values = [row["value"] for row in entries]
    empty, placed = values[:2]
    saved, loaded = values[3:5] if persistence else values[2:4]
    for case, value in zip(expected, values, strict=True):
        if persistence:
            check_persistence_observation(case, value)
        else:
            check_observation(case, value)
        native.require(value.get("pid") == client["pid"] and value.get("nonce") == client["nonce"]
                       and all(value["spa_planning"][k] == empty["spa_planning"][k] for k in ("anchor", "footprint", "draw_pos")),
                       "Spa checkpoint process/position differs")
    native.require(all(type(e.get("last_step")) is int and e["last_step"] >= 0
                       and type(e["value"].get("frame")) is int and e["value"]["frame"] > 90
                       and type(e["value"].get("world_epoch")) is int and e["value"]["world_epoch"] >= 0
                       for e in entries), "Spa checkpoint ordinals are mistyped")
    snapshots = [raw_checkpoint(raw, value) for value in values]
    check_inputs(raw, entries, client, inputs, persistence=persistence)
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(b["value"]["frame"] > a["value"]["frame"] and b["last_step"] > a["last_step"]
                           for a, b in zip(entries, entries[1:]))
                   and placed["world_epoch"] == empty["world_epoch"] == saved["world_epoch"]
                   and loaded["world_epoch"] == saved["world_epoch"] + 1
                   and (persistence or loaded["world_epoch"] == values[4]["world_epoch"]),
                   "Spa checkpoint time/input/epoch order differs")
    baseline = values[2] if persistence else placed
    native.require(selected_state(context(baseline)) == selected_state(context(saved)), "Spa paused save changed construction")
    if persistence:
        end = raw_checkpoint(raw, values[2])["sample_index"]
        prefix = dict(raw, samples=samples[:end + 1], events=[e for e in events if e["sample_index"] <= end])
        verify_completion(prefix, entries[:3], client, inputs[:entries[2]["last_step"]], persistence=True)
        interval = samples[end:snapshots[3]["sample_index"] + 1]
        native.require(all(s.get("paused") is True and s.get("world_epoch") == baseline["world_epoch"]
                           and s.get("primary_window") == snapshots[2]["primary_window"]
                           and s.get("primary_window_bits") == snapshots[2]["primary_window_bits"]
                           and s.get("virtual_seconds") == baseline["spa_planning"]["virtual_seconds"]
                           and selected_state(s["spa_context"]) == selected_state(context(baseline)) for s in interval),
                       "Spa Operational pre-save interval changed")
        native.require(all(s.get("paused") is True and s.get("world_epoch") in (saved["world_epoch"], loaded["world_epoch"])
                           and s.get("primary_window") == snapshots[2]["primary_window"]
                           and s.get("primary_window_bits") == snapshots[2]["primary_window_bits"]
                           and s.get("virtual_seconds") == baseline["spa_planning"]["virtual_seconds"]
                           for s in samples[end:snapshots[4]["sample_index"] + 1]),
                       "Spa Operational persistence advanced time/window/epoch")
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
    if persistence:
        native.require(not any(e.get("operation") in ("SoulSpaConstructionCancel", "SoulSpaSlotsChange") for e in events),
                       "Spa Operational persistence contains foreign operations")
        start, stop = snapshots[2]["sample_index"], snapshots[4]["sample_index"]
        boundary = receipts[1]["sample_index"]
        tail = samples[start:stop + 1]
        native.require(all(a["real_seconds"] < b["real_seconds"] for a, b in zip(tail, tail[1:])),
                       "Spa Operational persistence clock order differs")
        for sample in tail:
            expected_value = loaded if sample["sample_index"] >= boundary else baseline
            native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == expected_value["world_epoch"]
                           and selected_state(sample["spa_context"]) == selected_state(context(expected_value)),
                           "Spa Operational persistence transient state/epoch differs")
            observed = {"paused": sample["paused"], "spa_planning": dict(expected_value["spa_planning"], context=sample["spa_context"])}
            check_persistence_observation(PERSISTENCE_CHECKPOINTS[-1], observed)
        return
    cancelled = values[4]
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
    result["selected"] = value.get("selected")
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
    if name == "select-operational":
        native.require(check_selection_target(current) == point
                       and current["spa_planning"]["selection_target"] == before["spa_planning"]["selection_target"],
                       "Spa resolver target changed before press")
    before, first = input_context(current), len(driver.events)
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    record_action(driver, name, first, before)


def check_action_inventory(raw, entries, client, inputs, *, slots=False):
    native.require(isinstance(inputs, list) and 0 < len(inputs) <= 256
                   and [row.get("step") for row in inputs] == [str(i + 1) for i in range(len(inputs))], "Spa input inventory differs")
    actions = [row["spa_action"] for row in inputs if "spa_action" in row]
    native.require(all(row["spa_action"].get("first_step") == i + 1 for i, row in enumerate(inputs) if "spa_action" in row)
                   and all(a["after"]["spa_planning"]["real_seconds"] <= b["before"]["spa_planning"]["real_seconds"]
                           for a, b in zip(actions, actions[1:])), "Spa action annotation/order differs")
    claimed = check_picker_scrolls(entries, client, inputs) if slots else set()
    raw_claimed = set()
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
        if not keyboard and (key.startswith(("menu:", "world-candidate:")) or key == "dev-minimize"):
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
    return actions


def check_inputs(raw, entries, client, inputs, *, completion=False, usage=False, persistence=False, slots=False, occupied=False, resumed=False):
    actions = check_action_inventory(raw, entries, client, inputs, slots=slots)
    ignored = {"Escape", "dev-minimize", "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Normal)"}
    if usage:
        ignored.add("menu:SetTimeSpeed(Fast)")
    native.require(not resumed or occupied and len(entries) == 10, "Spa resumed input requires its occupied prefix")
    if occupied:
        tail = [a for a in actions if a["first_step"] > entries[4]["last_step"]]
        resumed_tail = [a for a in tail if a["first_step"] > entries[6]["last_step"]] if resumed else []
        if resumed:
            tail = [a for a in tail if a["first_step"] <= entries[6]["last_step"]]
            native.require([a["key"] for a in resumed_tail] == ["menu:SetTimeSpeed(Super)", "menu:SetTimeSpeed(Paused)"] * 3,
                           "Spa resumed input sequence differs")
            for index, action in enumerate(resumed_tail):
                lower, upper = entries[6 + index // 2:8 + index // 2]
                a, b = action["before"], action["after"]
                native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                               and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                               and a["world_epoch"] == b["world_epoch"] == entries[6]["value"]["world_epoch"]
                               and a["paused"] is (index % 2 == 0) and b["paused"] is (index % 2 == 1)
                               and (index % 2 == 1 or context(b).get("relative_speed") == 4),
                               "Spa resumed input phase/clock differs")
            ignored.update(a["key"] for a in resumed_tail)
        patterns = ["F5", r"menu:SelectSaveCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}",
                    "F9", r"menu:SelectLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}",
                    r"menu:ConfirmLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}"]
        native.require(len(tail) == len(patterns)
                       and all(re.fullmatch(pattern, action["key"]) for pattern, action in zip(patterns, tail, strict=True))
                       and all(a[side]["paused"] is True for a in tail for side in ("before", "after")),
                       "Occupied Spa save/load input sequence differs")
        for group, lower, upper in ((tail[:2], entries[4], entries[5]),
                                    (tail[2:], entries[5], entries[6])):
            for action in group:
                a, b = action["before"], action["after"]
                native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                               and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                               and lower["value"]["spa_planning"]["real_seconds"] <= a["spa_planning"]["real_seconds"]
                               < b["spa_planning"]["real_seconds"] <= upper["value"]["spa_planning"]["real_seconds"]
                               and a["world_epoch"] == lower["value"]["world_epoch"]
                               and b["world_epoch"] == (upper["value"]["world_epoch"]
                                   if action["key"].startswith("menu:ConfirmLoadCatalogSlot ")
                                   else lower["value"]["world_epoch"]),
                               "Occupied Spa request crossed checkpoint phase")
        ignored.update(a["key"] for a in tail)
    if slots:
        ignored.update(a["key"] for a in actions if a["first_step"] > entries[4]["last_step"])
    required = [a for a in actions if a["key"] not in ignored]
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
    owner = context(entries[-1 if persistence else 3]["value"])["sites"][0]["entity"]
    patterns = [r"menu:ToggleArchitect", r"menu:SelectArchitectCategory\(Some\(Plant\)\)",
                r"menu:SelectTaskMode\(SoulSpaPlace\(None\)\)", "placement", "F5",
                r"menu:SelectSaveCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}", "F9",
                r"menu:SelectLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}",
                r"menu:ConfirmLoadCatalogSlot \{ slot: Manual1, session: [1-9][0-9]* \}"]
    if not persistence:
        patterns += ["select-loaded",
                re.escape(cancel_key(owner)),
                re.escape(f"menu:ConfirmSoulSpaConstructionCancel {{ target: {owner}, epoch: {entries[3]['value']['world_epoch']}, ticket: ") + r"[1-9][0-9]* \}"]
    else:
        native.require([a for a in actions if a["first_step"] > entries[2]["last_step"]] == required[4:],
                       "Spa Operational persistence has extra post-completion input")
    native.require(len(filtered) == len(patterns) and all(re.fullmatch(pattern, key) for pattern, key in zip(patterns, filtered, strict=True)),
                   "Spa exact control/confirmation sequence differs")
    for index, action in enumerate(required):
        phase = 0 if index < 4 else (2 if persistence else 1) if index < 6 else (3 if persistence else 2) if index < 9 else 3
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
    native.require(value.get("paused") is True, "Spa completion capture is not paused")
    check_operational(context(value), value["spa_planning"])


def check_operational(state, plan, *, active_slots=4):
    native.require(plan.get("legal") is False
                   and len(state["sites"]) == 1 and len(state["tiles"]) == 4, "Spa completion site inventory differs")
    site = state["sites"][0]
    x, y = plan["anchor"]
    footprint = [[x, y], [x + 1, y], [x, y - 1], [x + 1, y - 1]]
    native.require(plan["footprint"] == footprint and sorted(site.get("map_building_grids", [])) == sorted(footprint)
                   and site.get("phase") == "Operational" and site.get("building_kind") == "SoulSpa"
                   and site.get("provisional") is False and type(site.get("entity_bits")) is int
                   and all(type(site.get(k)) is int for k in ("bones_required", "bones_delivered", "active_slots"))
                   and site["bones_required"] == site["bones_delivered"] == 12 and site["active_slots"] == active_slots
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


def verify_completion(raw, entries, client, inputs, *, persistence=False, raw_scope="production-raw-observations-v1"):
    native.require([e["case"] for e in entries] == list(COMPLETION_CHECKPOINTS), "Spa completion inventory differs")
    check_raw_session(raw, client, raw_scope=raw_scope)
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
                   and 0 < snapshots[2]["real_seconds"] - snapshots[1]["real_seconds"] <= (310 if persistence else 190)
                   and not any(e.get("operation") in ("DurableSave", "DurableLoad", "SoulSpaConstructionCancel", "SoulSpaSlotsChange")
                               for e in raw["events"]), "Spa completion elapsed/progress/operation differs")


def exercise_completion(driver, *, persistence=False):
    exercise_placement(driver, COMPLETION_CHECKPOINTS)
    pause(driver, False)
    driver.last = driver.wait(lambda v: len(context(v)["sites"]) == 1
                              and context(v)["sites"][0].get("phase") == "Operational", after=driver.last["frame"], timeout=300 if persistence else 180)
    pause(driver, True)
    driver.capture(COMPLETION_CHECKPOINTS[2])
    wait_final_raw(driver)


def generating(state):
    tiles = {t["entity"]: t for t in state["tiles"]}
    return {s["entity"]: s for s in state["souls"] if isinstance(s.get("generate_power"), dict)
            and s["generate_power"].get("phase") == "Generating"
            and s["generate_power"].get("tile") in tiles
            and s["entity"] in (tiles[s["generate_power"]["tile"]].get("workers") or [])}


def measurement_workers(state):
    # Select a real observation window, never change the worker's vitals or
    # assignment. Allow room for owned-input ACK/capture and the measured leg.
    return {key: soul for key, soul in generating(state).items()
            if all(finite(soul.get(k)) for k in ("dream", "fatigue", "stress"))
            and soul["dream"] >= 20 and 0 <= soul["fatigue"] <= .6 and 0 <= soul["stress"] <= .65}


def safe_slot_workers(state):
    workers = generating(state)
    native.require(workers and workers.keys() == measurement_workers(state).keys(),
                   "Spa slot baseline has an unsafe generating worker; no slot input permitted")
    return workers


def check_usage_observation(case, value):
    if case in COMPLETION_CHECKPOINTS:
        check_completion_observation(case, value)
        return
    native.require(case in USAGE_CHECKPOINTS and value.get("paused") is True, "Spa usage checkpoint differs")
    state = context(value)
    check_operational(state, value["spa_planning"])
    native.require(generating(state) and state["sites"][0]["generator"]["current_output"] > 0,
                   "Spa has no actual generating worker/positive output")


def verify_usage(raw, entries, client, inputs, *, raw_scope="production-raw-observations-v1"):
    native.require([e["case"] for e in entries] == list(USAGE_CHECKPOINTS), "Spa usage inventory differs")
    check_raw_session(raw, client, raw_scope=raw_scope)
    prefix_end = raw_checkpoint(raw, entries[2]["value"])["sample_index"]
    prefix = dict(raw, samples=raw["samples"][:prefix_end + 1],
                  events=[e for e in raw["events"] if e["sample_index"] <= prefix_end])
    verify_completion(prefix, entries[:3], client, inputs[:entries[2]["last_step"]], raw_scope=raw_scope)
    check_inputs(raw, entries, client, inputs, completion=True, usage=True)
    values = [e["value"] for e in entries]
    snapshots = [raw_checkpoint(raw, v) for v in values]
    for entry, snap in zip(entries, snapshots, strict=True):
        value = entry["value"]
        check_usage_observation(entry["case"], value)
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and type(value.get("world_epoch")) is int and value["world_epoch"] == values[0]["world_epoch"]
                       and type(entry.get("last_step")) is int
                       and all(value["spa_planning"][k] == values[0]["spa_planning"][k]
                               for k in ("anchor", "footprint", "draw_pos")), "Spa usage subject/geometry differs")
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(a["last_step"] < b["last_step"] and a["value"]["frame"] < b["value"]["frame"]
                           for a, b in zip(entries, entries[1:])), "Spa usage order differs")
    actions = [r["spa_action"] for r in inputs if "spa_action" in r and int(r["step"]) > entries[2]["last_step"]]
    native.require([a["key"] for a in actions] == ["menu:SetTimeSpeed(Fast)", "menu:SetTimeSpeed(Paused)",
                                                     "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"],
                   "Spa usage normal/pause sequence differs")
    for action, speeds in zip(actions, ((1, 2), (2, 2), (2, 1), (1, 1)), strict=True):
        actual = tuple(context(action[side]).get("relative_speed") for side in ("before", "after"))
        native.require(all(finite(speed) for speed in actual) and actual == speeds,
                       "Spa actual HUD speed transition differs")
    windows = []
    for pair, lower, upper in ((actions[:2], entries[2], entries[3]), (actions[2:], entries[3], entries[4])):
        for action in pair:
            a, b = action["before"], action["after"]
            native.require(lower["last_step"] < action["first_step"] <= action["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= a["frame"] < b["frame"] < upper["value"]["frame"]
                           and a["world_epoch"] == b["world_epoch"] == values[0]["world_epoch"], "Spa usage input phase differs")
        native.require(pair[0]["before"]["paused"] is True and pair[0]["after"]["paused"] is False
                       and pair[1]["before"]["paused"] is False and pair[1]["after"]["paused"] is True,
                       "Spa usage actual pause transition missing")
        windows.append((pressed_for(raw, pair[0])["real_seconds"], pair[1]["after"]["spa_planning"]["real_seconds"]))
    owner = context(values[2])["sites"][0]
    tile_keys = ("entity", "entity_bits", "parent_site", "parent_exists", "grid", "pose", "map")
    def shape(state):
        return sorted([{k: t.get(k) for k in tile_keys} for t in state["tiles"]], key=lambda t: t["entity"])
    expected_tiles = shape(context(values[2]))
    interval = raw["samples"][snapshots[2]["sample_index"]:snapshots[4]["sample_index"] + 1]
    for sample in interval:
        state = context({"spa_planning": {"context": sample["spa_context"]}})
        check_operational(state, values[2]["spa_planning"])
        now = sample["real_seconds"]
        native.require(finite(now), "Spa raw speed time is missing/nonfinite")
        fast, normal = actions[0], actions[2]
        fast_start, fast_end = (fast[s]["spa_planning"]["real_seconds"] for s in ("before", "after"))
        normal_start, normal_end = (normal[s]["spa_planning"]["real_seconds"] for s in ("before", "after"))
        expected_speeds = {1, 2} if fast_start <= now <= fast_end or normal_start <= now <= normal_end else (
            {2} if fast_end < now < normal_start else {1})
        native.require(type(state.get("relative_speed")) in (int, float) and state["relative_speed"] in expected_speeds,
                       "Spa raw HUD speed interval differs")
        native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == values[0]["world_epoch"]
                       and sample.get("primary_window") == snapshots[0]["primary_window"]
                       and type(sample.get("primary_window_bits")) is int and sample["primary_window_bits"] == snapshots[0]["primary_window_bits"]
                       and finite(sample.get("real_seconds")) and finite(sample.get("virtual_seconds"))
                       and sample["real_seconds"] >= 0 and sample["virtual_seconds"] >= 0 and type(sample.get("paused")) is bool
                       and (sample["paused"] or any(a <= sample["real_seconds"] <= b for a, b in windows))
                       and all(state["sites"][0][k] == owner[k] for k in ("entity", "entity_bits", "pose", "grid"))
                       and shape(state) == expected_tiles, "Spa usage continuity differs")
    native.require(all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                       and (not (a["paused"] and b["paused"]) or a["virtual_seconds"] == b["virtual_seconds"])
                       for a, b in zip(interval, interval[1:])), "Spa usage clocks differ")
    first, last = [generating(context(v)) for v in values[3:]]
    shared = sorted(set(first) & set(last))
    native.require(shared, "Spa usage has no stable generating Soul")
    duration = snapshots[4]["virtual_seconds"] - snapshots[3]["virtual_seconds"]
    native.require(3 <= duration <= 15, "Spa usage duration differs")
    # One same Soul must remain Generating throughout the measured interval.
    measured = raw["samples"][snapshots[3]["sample_index"]:snapshots[4]["sample_index"] + 1]
    passed = False
    for worker in shared:
        souls = [generating(s["spa_context"]).get(worker) for s in measured]
        if not all(s is not None and s["generate_power"]["tile"] == first[worker]["generate_power"]["tile"] for s in souls):
            continue
        dream = [s["dream"] for s in souls]
        passed |= (all(a >= b for a, b in zip(dream, dream[1:]))
                   and all(s["paused"] or s["spa_context"]["relative_speed"] == 1 for s in measured)
                   and all((a["dream"] == b["dream"] if right["virtual_seconds"] == left["virtual_seconds"]
                            else abs((a["dream"] - b["dream"]) - .5 * (right["virtual_seconds"] - left["virtual_seconds"])) <= .15)
                           for a, b, left, right in zip(souls, souls[1:], measured, measured[1:]))
                   and abs((dream[0] - dream[-1]) - .5 * duration) <= .15 and dream[0] > dream[-1]
                   and last[worker]["fatigue"] >= first[worker]["fatigue"]
                   and any(s["paused"] is False for s in measured))
    native.require(passed and not any(e.get("operation") in ("DurableSave", "DurableLoad", "SoulSpaConstructionCancel", "SoulSpaSlotsChange")
                                     for e in raw["events"]), "Spa usage Dream consumption/operation differs")


def exercise_usage(driver):
    exercise_completion(driver)
    click(driver, "menu:SetTimeSpeed(Fast)")
    driver.last = driver.wait(lambda v: not v["paused"] and context(v).get("relative_speed") == 2,
                              after=driver.last["frame"])
    driver.last = driver.wait(lambda v: bool(measurement_workers(context(v))),
                              after=driver.last["frame"], timeout=300)
    pause(driver, True)
    driver.capture(USAGE_CHECKPOINTS[3])
    workers = measurement_workers(context(driver.last))
    native.require(workers, "Spa generating worker vanished before capture")
    worker = sorted(workers)[0]
    start = driver.last["spa_planning"]["virtual_seconds"]
    pause(driver, False)
    driver.last = driver.wait(lambda v: worker in generating(context(v))
                              and v["spa_planning"]["virtual_seconds"] >= start + 3,
                              after=driver.last["frame"], timeout=15)
    pause(driver, True)
    driver.capture(USAGE_CHECKPOINTS[4])
    wait_final_raw(driver)


def slots_key(owner, count):
    return f"menu:SetSoulSpaActiveSlots {{ target: {owner}, active_slots: {count} }}"


def spa_assignment_keys(state):
    targets = {t["entity"] for t in state["tiles"]}
    # Already dispatched GoingToTile tasks may arrive after closing admission.
    # Count those identities before WorkingOn is installed, not just occupants.
    return {(s["entity"], task["tile"]) for s in state["souls"]
            if isinstance(task := s.get("generate_power"), dict) and task.get("tile") in targets}


def check_selection_target(value):
    plan, state = value["spa_planning"], context(value)
    native.require(len(state["sites"]) == 1 and len(state["tiles"]) == 4,
                   "Spa selection has no exact site/four tiles")
    target = plan.get("selection_target")
    poses = [row.get("pose") for row in state["tiles"] + state["sites"]]
    native.require(all(isinstance(pose, dict) and isinstance(pose.get("translation"), list)
                       and len(pose["translation"]) == 3 and all(finite(x) for x in pose["translation"])
                       for pose in poses), "Spa selection pose missing/nonfinite")
    points = [pose["translation"][:2] for pose in poses]
    native.require(isinstance(target, dict) and type(target.get("owner")) is int
                   and target["owner"] == state["sites"][0]["entity_bits"]
                   and target.get("hit") == "Direct" and target.get("class") == "Object"
                   and type(target.get("rank")) is int and 0 <= target["rank"] < 256
                   and target.get("world") in points and isinstance(target.get("world"), list)
                   and len(target["world"]) == 2 and all(finite(x) for x in target["world"])
                   and isinstance(target.get("point"), list) and len(target["point"]) == 2
                   and all(finite(x) for x in target["point"]) and plan.get("selection_point") == target["point"],
                   "Spa has no ordinary resolver-confirmed direct owner point")
    return target["point"]


def picker_target(value, owner):
    picker = value["spa_planning"].get("picker")
    native.require(isinstance(picker, dict) and "failure" not in picker
                   and type(picker.get("revision")) is int and picker["revision"] >= 0
                   and type(picker.get("epoch")) is int and picker["epoch"] == value["world_epoch"]
                   and isinstance(picker.get("entries"), list) and 1 < len(picker["entries"]) <= 256
                   and all(isinstance(row, dict) and type(row.get("entity")) is int for row in picker["entries"])
                   and len({row["entity"] for row in picker["entries"]}) == len(picker["entries"])
                   and sum(row == {"entity": owner, "class": "Object"} for row in picker["entries"]) == 1,
                   "Spa exact owner is not in the current picker")
    return f"world-candidate:{owner}:{picker['revision']}:{picker['epoch']}"


def picker_scroll(driver, key):
    picker = copy.deepcopy(driver.last["spa_planning"]["picker"])
    driver.send(point=driver.point(key))
    before, first = copy.deepcopy(driver.last), len(driver.events)
    native.require(key in before["controls"] and before["pointer_over_ui"] and before["paused"]
                   and not before["world_input_captured"] and not before["wheel_overflow"]
                   and before["spa_planning"]["picker"] == picker,
                   "Spa picker scroll foreground differs")
    control = before["controls"][key]
    rect, point = control.get("rect"), before.get("cursor")
    native.require(control.get("fully_visible") is True and isinstance(rect, list) and len(rect) == 4
                   and all(finite(x) for x in rect) and isinstance(point, list) and len(point) == 2
                   and all(finite(x) for x in point) and rect[0] < point[0] < rect[2] and rect[1] < point[1] < rect[3],
                   "Spa wheel target is not inside the visible picker list")
    picker_target(before, context(before)["sites"][0]["entity_bits"])
    driver.send(button=5, pressed=True)
    driver.send(button=5, pressed=False)
    count = len(before["wheel_events"])
    driver.last = driver.wait(lambda value: len(value["wheel_events"]) > count, after=before["frame"])
    action = {"key": key, "window": driver.window, "first_step": first + 1, "last_step": first + 2,
              "before": before, "after": copy.deepcopy(driver.last)}
    # Validate this physical step immediately; no-progress is not a retry request.
    check_picker_scroll(action, driver.events, driver.last["wheel_events"][count:])
    driver.events[first]["spa_scroll"] = action
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def check_picker_scroll(action, inputs, wheels):
    a, b, key = action["before"], action["after"], action["key"]
    first, last = action["first_step"], action["last_step"]
    native.require(type(first) is int and type(last) is int and 2 <= first < last <= len(inputs)
                   and last == first + 1, "Spa scroll physical pair differs")
    picker = a["spa_planning"]["picker"]
    native.require(key == f"scroll:world-candidates:{picker['revision']}:{picker['epoch']}"
                   and b["spa_planning"]["picker"] == picker, "Spa scroll picker revision/inventory changed")
    left, right = a["controls"].get(key), b["controls"].get(key)
    native.require(isinstance(left, dict) and isinstance(right, dict) and left.get("fully_visible") is True
                   and right.get("entity") == left.get("entity") and right.get("rect") == left.get("rect")
                   and isinstance(left.get("scroll"), list) and len(left["scroll"]) == 2
                   and isinstance(right.get("scroll"), list) and len(right["scroll"]) == 2
                   and all(finite(x) for x in left["scroll"] + right["scroll"])
                   and right["scroll"][0] == left["scroll"][0] and right["scroll"][1] > left["scroll"][1],
                   "Spa picker scroll made no progress or changed list")
    rect, point = left["rect"], a.get("cursor")
    native.require(isinstance(rect, list) and len(rect) == 4 and all(finite(x) for x in rect)
                   and isinstance(point, list) and len(point) == 2 and all(finite(x) for x in point)
                   and rect[0] < point[0] < rect[2] and rect[1] < point[1] < rect[3]
                   and b.get("cursor") == point and a.get("pointer_over_ui") is True and b.get("pointer_over_ui") is True,
                   "Spa wheel pointer is outside the actual list")
    native.require(all(a.get(k) == b.get(k) for k in ("nonce", "pid", "world_epoch", "primary_window", "camera", "selected"))
                   and a["paused"] is True and b["paused"] is True
                   and not a["world_input_captured"] and not b["world_input_captured"]
                   and not a["wheel_overflow"] and not b["wheel_overflow"]
                   and a["spa_planning"]["virtual_seconds"] == b["spa_planning"]["virtual_seconds"]
                   and selected_state(context(a)) == selected_state(context(b)), "Spa wheel changed world/camera/owner")
    sent = inputs[first - 1:last]
    motion = inputs[first - 2]
    native.require(type(action.get("window")) is int and action["window"] > 0
                   and motion.get("point") is not None and len(motion["point"]) == 2
                   and motion.get("button") is None and motion.get("key") is None
                   and motion.get("nonce") == a["nonce"] and motion.get("pid") == a["pid"]
                   and motion.get("window") == action["window"] and motion.get("world_epoch") == a["world_epoch"]
                   and motion.get("pre_frame", -1) < motion.get("post_frame", -1) == a["frame"]
                   and all(abs(x-y) <= 2 for x, y in zip(motion["point"], point, strict=True))
                   and [row.get("pressed") for row in sent] == [True, False]
                   and all(type(row.get("button")) is int and row["button"] == 5
                           and type(row.get("pressed")) is bool and row.get("key") is None and row.get("point") is None
                           and row.get("nonce") == a["nonce"] and row.get("pid") == a["pid"]
                           and row.get("window") == action["window"]
                           and row.get("world_epoch") == a["world_epoch"]
                           and a["frame"] <= row["pre_frame"] < row["post_frame"] <= b["frame"] for row in sent),
                   "Spa scroll transport pair/owner differs")
    native.require(len(wheels) == 1 and not a["wheel_overflow"] and not b["wheel_overflow"]
                   and b["wheel_events"] == a["wheel_events"] + wheels, "Spa wheel receipt missing/duplicated")
    wheel = wheels[0]
    native.require(wheel.get("index") == len(a["wheel_events"]) and wheel.get("window") == a["primary_window"]
                   and wheel.get("epoch") == a["world_epoch"]
                   and sent[1]["pre_frame"] < wheel.get("frame", -1) <= b["frame"]
                   and wheel.get("phase") == "Moved" and wheel.get("unit") in ("Line", "Pixel")
                   and isinstance(wheel.get("delta"), list) and len(wheel["delta"]) == 2
                   and all(finite(x) for x in wheel["delta"]) and wheel["delta"][0] == 0 and wheel["delta"][1] < 0,
                   "Spa actual wheel witness differs")


def check_picker_scrolls(entries, client, inputs):
    rows = [(i, row["spa_scroll"]) for i, row in enumerate(inputs) if "spa_scroll" in row]
    native.require(len(rows) <= 8, "Spa picker wheel bound exceeded")
    wheels, claimed = [], set()
    for i, action in rows:
        a, b = action["before"], action["after"]
        picker_target(a, context(entries[4]["value"])["sites"][0]["entity_bits"])
        native.require(action.get("window") == client["window"] and action["first_step"] == i + 1
                       and all(v["nonce"] == client["nonce"] and v["pid"] == client["pid"]
                       and v["primary_window"] == entries[4]["value"]["primary_window"]
                       and v["world_epoch"] == entries[4]["value"]["world_epoch"] for v in (a, b))
                       and entries[4]["last_step"] < action["first_step"] < action["last_step"] <= entries[5]["last_step"]
                       and entries[4]["value"]["frame"] <= a["frame"] < b["frame"] < entries[5]["value"]["frame"],
                       "Spa wheel crosses its actual phase/subject")
        delta = b["wheel_events"][len(a["wheel_events"]):]
        check_picker_scroll(action, inputs, delta)
        native.require(a["wheel_events"] == wheels, "Spa wheel history gap/order differs")
        wheels += delta
        claimed.update((action["first_step"], action["last_step"]))
    native.require(entries[-1]["value"].get("wheel_events", []) == wheels, "Spa has unclaimed actual wheels")
    return claimed


def check_slots_observation(case, value):
    if case in USAGE_CHECKPOINTS:
        check_usage_observation(case, value)
        return
    native.require(case in SLOTS_CHECKPOINTS[-2:] and value.get("paused") is True,
                   "Spa slots capture differs")
    check_operational(context(value), value["spa_planning"], active_slots=0)
    native.require(generating(context(value)), "Spa closed slots have no generating occupant")


def check_slot_receipts(raw, actions, owner, epoch):
    receipts = [e for e in raw["events"] if e.get("operation") == "SoulSpaSlotsChange"]
    native.require(len(actions) == len(receipts) == 4, "Spa slot outcomes missing/duplicate")
    for count, action, receipt in zip((3, 2, 1, 0), actions, receipts, strict=True):
        native.require(action["key"] == slots_key(owner, count)
                       and action["before"]["paused"] is False and action["after"]["paused"] is False
                       and action["before"]["world_epoch"] == action["after"]["world_epoch"] == epoch
                       and receipt.get("producer") == "SoulSpaSlotsChangeOutcome" and receipt.get("owner") == owner
                       and receipt.get("result") == f"Applied {{ requested: {count}, applied: {count}, clamped: false }}"
                       and receipt.get("applied") == {"requested": count, "applied": count, "clamped": False}
                       and all(type(receipt["applied"].get(k)) is int for k in ("requested", "applied"))
                       and receipt["applied"].get("clamped") is False,
                       "Spa exact slot request/outcome differs")
        pressed = pressed_for(raw, action)
        after_press(receipt, pressed)
        index = receipt["sample_index"]
        native.require(0 <= index < len(raw["samples"])
                       and raw["samples"][index].get("world_epoch") == epoch
                       and raw["samples"][index].get("paused") is False
                       and raw["samples"][index]["real_seconds"] >= receipt["real_seconds"]
                       and action["before"]["spa_planning"]["real_seconds"] < receipt["real_seconds"]
                       <= action["after"]["spa_planning"]["real_seconds"]
                       and raw["samples"][index]["spa_context"]["sites"][0]["active_slots"] == count,
                       "Spa slot receipt is outside actual input/sample")
    native.require(all(a["sample_index"] < b["sample_index"] and a["real_seconds"] < b["real_seconds"]
                       for a, b in zip(receipts, receipts[1:])), "Spa slot receipt order differs")
    return receipts


def verify_slots(raw, entries, client, inputs):
    native.require(client.get("input_ack_policy") == "next-exact-observation-v1", "Spa slot input ACK policy differs")
    native.require([e["case"] for e in entries] == list(SLOTS_CHECKPOINTS), "Spa slots inventory differs")
    check_raw_session(raw, client)
    prefix_end = raw_checkpoint(raw, entries[4]["value"])["sample_index"]
    prefix = dict(raw, samples=raw["samples"][:prefix_end + 1],
                  events=[e for e in raw["events"] if e["sample_index"] <= prefix_end])
    verify_usage(prefix, entries[:5], client, inputs[:entries[4]["last_step"]])
    check_inputs(raw, entries, client, inputs, completion=True, usage=True, slots=True)
    owner = context(entries[4]["value"])["sites"][0]
    epoch = entries[4]["value"]["world_epoch"]
    actions = [r["spa_action"] for r in inputs if "spa_action" in r and int(r["step"]) > entries[4]["last_step"]]
    keys = [a["key"] for a in actions]
    native.require(keys and keys[0] == "select-operational", "Spa slots need one world selection")
    selection_end = 1
    if len(keys) > 1 and keys[1].startswith("world-candidate:toggle:"):
        native.require(re.fullmatch(r"world-candidate:toggle:[0-9]+:" + str(epoch), keys[1])
                       and len(keys) > 2 and re.fullmatch(
                           rf"world-candidate:{owner['entity_bits']}:[0-9]+:{epoch}", keys[2]),
                       "Spa slots candidate picker target/revision differs")
        selection_end = 3
        picker_key = picker_target(actions[1]["after"], owner["entity_bits"])
        native.require(keys[2] == picker_key and keys[1] == picker_key.replace(str(owner["entity_bits"]), "toggle", 1),
                       "Spa picker buttons do not match its actual inventory")
        for row in inputs:
            if "spa_scroll" in row:
                scroll = row["spa_scroll"]
                native.require(actions[1]["last_step"] < scroll["first_step"] < scroll["last_step"] < actions[2]["first_step"]
                               and actions[1]["after"]["frame"] <= scroll["before"]["frame"]
                               < scroll["after"]["frame"] <= actions[2]["before"]["frame"]
                               and picker_target(scroll["before"], owner["entity_bits"]) == picker_key,
                               "Spa wheel is outside the exact candidate selection")
    else:
        native.require(not any("spa_scroll" in row for row in inputs), "Spa direct selection contains wheel input")
    native.require(type(actions[selection_end - 1]["after"].get("selected")) is int
                   and actions[selection_end - 1]["after"]["selected"] == owner["entity_bits"]
                   and keys[selection_end:] == ["menu:SetTimeSpeed(Normal)",
                       *(slots_key(owner["entity"], n) for n in (3, 2, 1, 0)),
                       "menu:SetTimeSpeed(Paused)", "menu:SetTimeSpeed(Normal)", "menu:SetTimeSpeed(Paused)"],
                   "Spa slot selection/action sequence differs")
    for phase, group in ((4, actions[:selection_end + 6]), (5, actions[selection_end + 6:])):
        lower, upper = entries[phase:phase + 2]
        native.require(all(lower["last_step"] < a["first_step"] <= a["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= a["before"]["frame"] < a["after"]["frame"] < upper["value"]["frame"]
                           and lower["value"]["spa_planning"]["real_seconds"] <= a["before"]["spa_planning"]["real_seconds"]
                           < a["after"]["spa_planning"]["real_seconds"] <= upper["value"]["spa_planning"]["real_seconds"]
                           and a["before"]["world_epoch"] == a["after"]["world_epoch"] == epoch for a in group),
                       "Spa slot action crosses its checkpoint phase")
    selection = actions[0]["before"]
    point = check_selection_target(selection)
    native.require(selected_state(context(selection)) == selected_state(context(entries[4]["value"]))
                   and selection.get("menu_state") == "Hidden" and not selection["world_input_captured"]
                   and not selection["pointer_over_ui"] and selection["paused"] is True
                   and isinstance(point, list) and len(point) == 2 and all(finite(x) for x in point)
                   and all(abs(a - b * selection["scale_factor"]) <= 2
                           for a, b in zip(selection["cursor"], point, strict=True)),
                   "Spa slots selection is not the actual projected site")
    changes = actions[selection_end + 1:selection_end + 5]
    receipts = check_slot_receipts(raw, changes, owner["entity"], epoch)
    snapshots = [raw_checkpoint(raw, e["value"]) for e in entries]
    native.require(all(a["sample_index"] < b["sample_index"] for a, b in zip(snapshots, snapshots[1:]))
                   and all(a["last_step"] < b["last_step"] and a["value"]["frame"] < b["value"]["frame"]
                           for a, b in zip(entries, entries[1:])), "Spa slots checkpoint order differs")
    baseline = context(entries[4]["value"])
    tile_keys = ("entity", "entity_bits", "parent_site", "parent_exists", "grid", "pose", "map")
    def shape(state):
        return sorted([{k: t.get(k) for k in tile_keys} for t in state["tiles"]], key=lambda t: t["entity"])
    required_workers = safe_slot_workers(baseline)
    native.require(required_workers, "Spa slots have no initial generating worker")
    for entry in entries[5:]:
        check_slots_observation(entry["case"], entry["value"])
        native.require(entry["value"].get("nonce") == client["nonce"] and entry["value"].get("pid") == client["pid"]
                       and type(entry["value"].get("world_epoch")) is int and entry["value"]["world_epoch"] == epoch
                       and type(entry["value"].get("primary_window")) is int
                       and entry["value"]["primary_window"] == snapshots[4]["primary_window_bits"]
                       and all(entry["value"]["spa_planning"][k] == entries[4]["value"]["spa_planning"][k]
                               for k in ("anchor", "footprint", "draw_pos")), "Spa slot subject/geometry differs")
    interval = raw["samples"][prefix_end:snapshots[-1]["sample_index"] + 1]
    pause_actions = [a for a in actions[selection_end:] if a["key"].startswith("menu:SetTimeSpeed(")]
    for pair, lower, upper in ((pause_actions[:2], entries[4], entries[5]),
                               (pause_actions[2:], entries[5], entries[6])):
        native.require(pair[0]["before"]["paused"] is True and pair[0]["after"]["paused"] is False
                       and pair[1]["before"]["paused"] is False and pair[1]["after"]["paused"] is True
                       and all(lower["last_step"] < a["first_step"] <= a["last_step"] <= upper["last_step"]
                               and lower["value"]["frame"] <= a["before"]["frame"] < a["after"]["frame"] < upper["value"]["frame"]
                               and a["before"]["world_epoch"] == a["after"]["world_epoch"] == epoch
                               and context(a["before"]).get("relative_speed") == context(a["after"]).get("relative_speed") == 1
                               for a in pair), "Spa slots actual normal/pause interval differs")
    windows = [(pressed_for(raw, a)["real_seconds"], b["after"]["spa_planning"]["real_seconds"])
               for a, b in (pause_actions[:2], pause_actions[2:])]
    closed_assignments = None
    for sample in interval:
        state = context({"spa_planning": {"context": sample["spa_context"]}})
        count = 4 - sum(r["sample_index"] <= sample["sample_index"] for r in receipts)
        check_operational(state, entries[4]["value"]["spa_planning"], active_slots=count)
        current = generating(state)
        native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == epoch
                       and sample.get("primary_window") == snapshots[4]["primary_window"]
                       and type(sample.get("primary_window_bits")) is int
                       and sample["primary_window_bits"] == snapshots[4]["primary_window_bits"]
                       and all(state["sites"][0][k] == owner[k] for k in ("entity", "entity_bits", "pose", "grid"))
                       and shape(state) == shape(baseline) and finite(state.get("relative_speed")) and state["relative_speed"] == 1
                       and type(sample.get("paused")) is bool and finite(sample.get("real_seconds"))
                       and finite(sample.get("virtual_seconds"))
                       and (sample["paused"] or any(a <= sample["real_seconds"] <= b for a, b in windows))
                       and all(worker in current and current[worker]["generate_power"]["tile"] == soul["generate_power"]["tile"]
                               for worker, soul in required_workers.items()), "Spa slots evict worker/change owner/clock")
        if count == 0:
            assignments = spa_assignment_keys(state)
            if closed_assignments is None:
                closed_assignments = assignments
            native.require(assignments == closed_assignments, "Spa zero slots admitted/replaced a worker")
    native.require(all(a["real_seconds"] < b["real_seconds"] and a["virtual_seconds"] <= b["virtual_seconds"]
                       and (not (a["paused"] and b["paused"]) or a["virtual_seconds"] == b["virtual_seconds"])
                       for a, b in zip(interval, interval[1:]))
                   and 3 <= snapshots[-1]["virtual_seconds"] - snapshots[-2]["virtual_seconds"] <= 15
                   and receipts[-1]["sample_index"] < snapshots[-2]["sample_index"]
                   and not any(e.get("operation") in ("DurableSave", "DurableLoad", "SoulSpaConstructionCancel")
                               for e in raw["events"]), "Spa slots measured duration/foreign operation differs")


def select_operational(driver, owner):
    check_selection_target(driver.last)
    world_click(driver, "select-operational", "selection_point")
    if driver.last.get("selected") != owner["entity_bits"]:
        target = picker_target(driver.last, owner["entity_bits"])
        toggle = target.replace(str(owner["entity_bits"]), "toggle", 1)
        click(driver, toggle)
        scroll_key = target.replace(f"world-candidate:{owner['entity_bits']}", "scroll:world-candidates", 1)
        driver.last = driver.wait(lambda value: scroll_key in value["controls"], after=driver.last["frame"])
        for _ in range(8):
            native.require(picker_target(driver.last, owner["entity_bits"]) == target, "Spa picker changed before scroll")
            if driver.last["controls"].get(target, {}).get("fully_visible") is True:
                break
            picker_scroll(driver, scroll_key)
        native.require(driver.last["controls"].get(target, {}).get("fully_visible") is True,
                       "Spa exact candidate is not visible within wheel bound")
        click(driver, target)
    native.require(driver.last.get("selected") == owner["entity_bits"], "Spa actual owner selection failed")


def exercise_slots(driver):
    exercise_usage(driver)
    safe_slot_workers(context(driver.last))
    owner = context(driver.last)["sites"][0]
    select_operational(driver, owner)
    pause(driver, False)
    for count in (3, 2, 1, 0):
        key = slots_key(owner["entity"], count)
        driver.last = driver.wait(lambda v: key in v["controls"], after=driver.last["frame"], timeout=10)
        click(driver, key)
        driver.last = driver.wait(lambda v: context(v)["sites"][0]["active_slots"] == count,
                                  after=driver.last["frame"], timeout=10)
    pause(driver, True)
    driver.capture(SLOTS_CHECKPOINTS[-2])
    start = driver.last["spa_planning"]["virtual_seconds"]
    pause(driver, False)
    driver.last = driver.wait(lambda v: v["spa_planning"]["virtual_seconds"] >= start + 3,
                              after=driver.last["frame"], timeout=15)
    pause(driver, True)
    driver.capture(SLOTS_CHECKPOINTS[-1])
    wait_final_raw(driver)


def check_persistence_observation(case, value):
    if case in COMPLETION_CHECKPOINTS:
        check_completion_observation(case, value)
        if case != COMPLETION_CHECKPOINTS[-1]:
            return
    else:
        native.require(case in PERSISTENCE_CHECKPOINTS and value.get("paused") is True,
                       "Spa persistence capture differs")
        check_operational(context(value), value["spa_planning"])
    state = context(value)
    # This first persistence leg deliberately covers an empty Operational site.
    # Occupied tasks/Soul remapping need their own explicit save contract.
    targets = {tile["entity"] for tile in state["tiles"]}
    native.require(all(not tile.get("workers") for tile in state["tiles"])
                   and not any(s.get("working_on") in targets
                               or isinstance(s.get("generate_power"), dict) and s["generate_power"].get("tile") in targets
                               for s in state["souls"])
                   and not any(r.get("target") in targets for r in state["working_relations"]),
                   "Spa empty Operational persistence has occupants")


def exercise_persistence(driver):
    exercise_completion(driver, persistence=True)
    check_persistence_observation(COMPLETION_CHECKPOINTS[-1], driver.last)
    tap(driver, "F5")
    driver.last = driver.wait(lambda v: any(k.startswith("menu:SelectSaveCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    click(driver, catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda v: v["catalog"] == "Closed" and "Save:Succeeded" in v["save_outcomes"], after=driver.last["frame"])
    driver.capture(PERSISTENCE_CHECKPOINTS[3])
    tap(driver, "F9")
    driver.last = driver.wait(lambda v: any(k.startswith("menu:SelectLoadCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    click(driver, catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    click(driver, catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda v: v["world_epoch"] == epoch + 1 and v["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    pause(driver, True)
    driver.capture(PERSISTENCE_CHECKPOINTS[4])
    wait_final_raw(driver)
