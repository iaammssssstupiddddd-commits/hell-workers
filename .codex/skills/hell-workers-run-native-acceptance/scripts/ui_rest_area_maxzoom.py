"""Explicit ordinary ArtPreview farthest-zoom UI feedback, never art authority."""
import math

import native_acceptance as native
import ui_rest_area_art as art
import ui_rest_area_planning as planning
import ui_rest_area_usage as usage

CASES = tuple(f"rest-area-maxzoom-{state}-{kind}" for state in ("empty", "occupied") for kind in ("hover", "selected"))
CHECKPOINTS = (*planning.COMPLETION_CHECKPOINTS, *CASES)
MAX_WHEEL_CLICKS = 48  # Existing PanCamera: 1.0 -> 5.0 at 0.1 per line step.


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def checkpoint_sample(samples, value):
    # A future click may have the same paused rest context. Never use it as
    # the preceding hover's boundary just because its timestamp is nearest.
    now = usage.context(value)["real_seconds"]
    return usage.raw_context_checkpoint([sample for sample in samples
                                         if finite(sample.get("real_seconds")) and sample["real_seconds"] <= now], value)


def camera_scale(value):
    cameras = value.get("camera_numeric")
    native.require(isinstance(cameras, list) and len(cameras) == 1, "maxzoom requires one main camera")
    camera = cameras[0]
    scale = camera.get("scale")
    native.require(camera.get("active") is True and type(value.get("primary_window")) is int
                   and type(camera.get("entity")) is int and type(camera.get("target_window")) is int
                   and camera.get("target_window") == value["primary_window"]
                   and isinstance(scale, list) and len(scale) == 3
                   and all(finite(axis) and axis > 0 for axis in scale) and scale[0] == scale[1]
                   and finite(camera.get("orthographic_scale")) and camera["orthographic_scale"] > 0,
                   "maxzoom camera owner/projection/scale differs")
    return scale[0]


def owner(value):
    buildings = value["rest_area_planning"]["buildings"]
    native.require(len(buildings) == 1 and type(buildings[0]) is int, "maxzoom needs one actual RestArea")
    state = usage.context(value)
    rows = [row for row in state["owners"] if row.get("entity_bits") == buildings[0]]
    native.require(len(rows) == 1, "maxzoom owner context differs")
    usage.candidates(state, buildings[0])  # Validate real relationship inverses, not a display count.
    return buildings[0], rows[0]


def point(value):
    projected = value["rest_area_planning"].get("owner_point")
    factor = value.get("scale_factor")
    native.require(isinstance(projected, list) and len(projected) == 2 and all(finite(axis) for axis in projected)
                   and finite(factor) and factor > 0, "maxzoom owner projection missing")
    result = tuple(round(axis * factor) for axis in projected)
    native.require(all(0 < axis < limit for axis, limit in zip(result, value["viewport"], strict=True)),
                   "maxzoom owner projection is outside the physical client")
    return result


def visible_rect(rect, viewport):
    return (isinstance(rect, list) and len(rect) == 4 and all(finite(axis) for axis in rect)
            and 0 <= rect[0] < rect[2] <= viewport[0] and 0 <= rect[1] < rect[3] <= viewport[1])


def zoom_after(before, ack):
    native.require(ack.get("unit") in ("Line", "Pixel") and ack.get("phase") == "Moved"
                   and isinstance(ack.get("delta"), list) and len(ack["delta"]) == 2
                   and all(finite(axis) for axis in ack["delta"]) and ack["delta"][0] == 0 and ack["delta"][1] < 0,
                   "maxzoom actual wheel unit/delta differs")
    lines = ack["delta"][1] / (100 if ack["unit"] == "Pixel" else 1)
    return min(5., before - lines * .1)


def check_observation(case, value):
    bits, state = owner(value)
    native.require(value["paused"] is True and value["world_input_captured"] is False
                   and not value["pointer_over_ui"] and abs(camera_scale(value) - 5) <= 1e-5
                   and value["menu_state"] == "Hidden" and value["mode_presentation"]["rect"] is None
                   and value["hovered"] == bits and value["pointer_target"] == bits
                   and value["tooltip_target"] == bits and value["tooltip_epoch"] == value["world_epoch"]
                   and all(type(value[key]) is int for key in ("hovered", "pointer_target", "tooltip_target", "tooltip_epoch", "world_epoch"))
                   and not value["wheel_overflow"], "maxzoom actual hover/tooltip/camera differs")
    native.require(value["cursor"] is not None and all(abs(a - b) <= 2 for a, b in zip(value["cursor"], point(value), strict=True)),
                   "maxzoom pointer differs from current owner projection")
    occupants = state.get("occupants") or []
    occupied = "-occupied-" in case
    native.require(bool(occupants) == occupied, "maxzoom actual occupancy phase differs")
    visible = [row["text"] for row in value["tooltip_text"]
               if visible_rect(row.get("rect"), value["viewport"]) and finite(row.get("alpha")) and row["alpha"] >= .95]
    expected = f"Resting: {len(occupants)}/{state['capacity']} | Dream: {len(occupants) * .5:.2f}/s"
    native.require(finite(value["tooltip_alpha"]) and value["tooltip_alpha"] >= .95
                   and value["tooltip_presentation"] is not None and visible_rect(value["tooltip_presentation"]["rect"], value["viewport"])
                   and "Building: RestArea" in visible and any(expected in text.splitlines() for text in visible),
                   "maxzoom visible type/utilization tooltip differs from the actual owner")
    if case.endswith("-selected"):
        header = value["controls"].get("text:inspection-header", {})
        native.require(type(value["selected"]) is int and value["selected"] == bits and visible_rect(header.get("rect"), value["viewport"])
                       and header.get("fully_visible") is True and header.get("text") == "Building: RestArea",
                       "maxzoom selected owner/visible inspection header differs")


def wheel(driver):
    before = driver.last
    old = camera_scale(before)
    native.require(not before["world_input_captured"] and not before["pointer_over_ui"]
                   and before["paused"] and not before["wheel_overflow"], "maxzoom wheel foreground differs")
    count = len(before["wheel_events"])
    driver.input.scroll(str(len(driver.events) + 1), driver.nonce, 1)

    def acknowledged(value):
        native.require(value["world_epoch"] == before["world_epoch"]
                       and value["primary_window"] == before["primary_window"]
                       and value["paused"] and not value["wheel_overflow"]
                       and not value["world_input_captured"] and not value["pointer_over_ui"],
                       "maxzoom wheel changed world/window/foreground")
        rows = value["wheel_events"][count:]
        if not rows:
            return False
        native.require(len(rows) == 1, "maxzoom wheel generated ambiguous input")
        event = rows[0]
        native.require(event["window"] == before["primary_window"] and event["epoch"] == before["world_epoch"]
                       and before["frame"] < event["frame"] <= value["frame"] and event["unit"] in ("Line", "Pixel")
                       and event["phase"] == "Moved" and event["delta"][0] == 0
                       and finite(event["delta"][1]) and event["delta"][1] < 0,
                       "maxzoom received wheel differs")
        observed = camera_scale(value)
        if observed <= old:
            return False
        native.require(abs(observed - zoom_after(old, event)) <= 1e-5, "maxzoom camera motion differs from actual wheel")
        return True

    driver.last = driver.wait(acknowledged, after=before["frame"] + 3)
    captured = driver.last
    selected = [None]
    def recorded(_):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("scope") == art.SCOPE and raw.get("failure") is None
                       and raw.get("nonce") == captured["nonce"] and raw.get("process_id") == captured["pid"],
                       "maxzoom wheel raw context changed")
        for sample in raw["samples"]:
            if sample.get("camera_scale") != camera_scale(captured):
                continue
            try:
                usage.raw_context_checkpoint([sample], captured)
            except usage.RawCheckpointPending:
                continue
            selected[0] = sample["sample_index"]
            return True
        return False
    driver.wait(recorded, after=captured["frame"], timeout=15)
    driver.events[-1].update(camera_before=old, camera_after=camera_scale(driver.last),
                             wheel_ack=driver.last["wheel_events"][-1], wheel_raw_index=selected[0],
                             camera_ack={key: captured[key] for key in (
                                 "frame", "world_epoch", "primary_window", "camera_numeric", "nonce", "pid",
                                 "paused", "rest_area_planning", "pointer_over_ui", "world_input_captured")})
    driver.acknowledge_events(driver.events[-1:])


def picker_click(driver, key):
    """One explicit candidate UI action, not a repeated world click."""
    first_step = len(driver.events) + 1
    driver.send(point=driver.point(key))
    native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered", "candidate button is not hovered")
    observed = driver.last
    driver.send(button=1, pressed=True)
    native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed", "candidate button did not receive press")
    driver.send(button=1, pressed=False)
    return {"key": key, "hover": observed, "first_step": first_step, "last_step": len(driver.events)}


def select_owner(driver):
    bits, _ = owner(driver.last)
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    if driver.last["selected"] == bits:
        return None
    # Repeated same-point world clicks intentionally cycle overlapping actors.
    # Use the existing revision/epoch-bound named picker, never a blind retry.
    suffix = f":{driver.last['world_epoch']}"
    toggles = [key for key in driver.last["controls"] if key.startswith("world-candidate:toggle:") and key.endswith(suffix)]
    native.require(len(toggles) == 1, "selection differs and no unique existing candidate picker is visible")
    toggle = picker_click(driver, toggles[0])
    prefix = f"world-candidate:{bits}:"
    driver.last = driver.wait(lambda value: any(key.startswith(prefix) and key.endswith(suffix) for key in value["controls"]),
                              after=driver.last["frame"], timeout=15)
    targets = [key for key in driver.last["controls"] if key.startswith(prefix) and key.endswith(suffix)]
    native.require(len(targets) == 1, "candidate picker does not contain the exact owner")
    target = picker_click(driver, targets[0])
    driver.last = driver.wait(lambda value: value["selected"] == bits, after=driver.last["frame"], timeout=15)
    driver.send(point=point(driver.last))
    return {"toggle": toggle, "target": target}


def capture_phase(driver, state):
    driver.send(point=point(driver.last))
    for kind in ("hover", "selected"):
        case = f"rest-area-maxzoom-{state}-{kind}"
        picker = select_owner(driver) if kind == "selected" else None
        def ready(value):
            try:
                check_observation(case, value)
            except native.AcceptanceError:
                return False
            return True
        driver.last = driver.wait(ready, after=driver.last["frame"], timeout=15)
        driver.capture(case)
        if picker is not None:
            driver.checkpoints[-1]["selection_picker"] = picker
            native.atomic_write_json(driver.root / "checkpoints.json", {"checkpoints": driver.checkpoints})
        captured = driver.last
        def recorded(_):
            raw = native.read_json(driver.root / "production-cancellation.json")
            native.require(raw.get("scope") == art.SCOPE and raw.get("failure") is None
                           and raw.get("nonce") == captured["nonce"] and raw.get("process_id") == captured["pid"],
                           "maxzoom raw collector changed")
            try:
                checkpoint_sample(raw["samples"], captured)
            except usage.RawCheckpointPending:
                return False
            return True
        driver.wait(recorded, after=captured["frame"], timeout=15)


def exercise(driver):
    planning.exercise(driver, completion=True)
    native.require(abs(camera_scale(driver.last) - 1) <= 1e-5 and not driver.last["wheel_events"],
                   "maxzoom requires the unchanged ordinary camera before wheel input")
    driver.send(point=point(driver.last))
    for _ in range(MAX_WHEEL_CLICKS):
        if abs(camera_scale(driver.last) - 5) <= 1e-5:
            break
        wheel(driver)
    native.require(abs(camera_scale(driver.last) - 5) <= 1e-5, "maxzoom not reached within bounded wheel clicks")
    capture_phase(driver, "empty")
    bits, _ = owner(driver.last)
    planning.set_refund_paused(driver, False)
    driver.last = driver.wait(lambda value: bool(usage.candidates(usage.context(value), bits)),
                              after=driver.last["frame"], timeout=240)
    planning.set_refund_paused(driver, True)
    capture_phase(driver, "occupied")


def verify(raw, entries, client, contract, events):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS), "maxzoom checkpoint sequence differs")
    planning.check_completion(raw, entries[:3], client, raw_scope=art.SCOPE)
    art.check_consumers(raw, entries, contract)
    first = entries[2]["value"]
    native.require(abs(camera_scale(first) - 1) <= 1e-5 and not first["wheel_events"], "maxzoom initial camera/wheel state differs")
    bits, _ = owner(first)
    for entry in entries[3:]:
        value = entry["value"]
        check_observation(entry["case"], value)
        sample = checkpoint_sample(raw["samples"], value)
        native.require(sample["play_mode"] == "Normal" and sample["task_mode"] == "None"
                       and sample["menu_state"] == "Hidden" and abs(sample["camera_scale"] - 5) <= 1e-5,
                       "maxzoom raw camera/input mode differs")
        native.require(value["world_epoch"] == first["world_epoch"] and owner(value)[0] == bits,
                       "maxzoom changed actual owner/epoch")
    scrolls = [event for event in events if event.get("scroll_discrete") is not None]
    native.require(1 <= len(scrolls) <= MAX_WHEEL_CLICKS and client.get("eis_scroll") is True, "maxzoom bounded scroll evidence missing")
    scale = camera_scale(first)
    final_wheels = entries[-1]["value"]["wheel_events"]
    native.require(len(final_wheels) == len(scrolls)
                   and [row.get("index") for row in final_wheels] == list(range(len(scrolls))),
                   "maxzoom has unmatched or out-of-order received wheel input")
    for event in scrolls:
        ack = event.get("wheel_ack")
        index, captured = event.get("wheel_raw_index"), event.get("camera_ack")
        native.require(type(index) is int and 0 <= index < len(raw["samples"]) and isinstance(captured, dict),
                       "maxzoom immutable wheel camera sample missing")
        sample = raw["samples"][index]
        usage.raw_context_checkpoint([sample], captured)
        native.require(event["scroll_discrete"] == [0, 120] and all(type(axis) is int for axis in event["scroll_discrete"])
                       and finite(event.get("camera_before")) and event["camera_before"] == scale
                       and finite(event.get("camera_after")) and scale < event["camera_after"] <= 5.00001
                       and isinstance(ack, dict) and any(art.pipeline.exact_json(ack, row) for row in final_wheels)
                       and all(type(ack[key]) is int for key in ("index", "window", "epoch", "frame"))
                       and ack["unit"] in ("Line", "Pixel") and ack["phase"] == "Moved"
                       and all(finite(axis) for axis in ack["delta"]) and ack["delta"][0] == 0 and ack["delta"][1] < 0
                       and ack["window"] == first["primary_window"] and ack["epoch"] == first["world_epoch"]
                       and event["pre_frame"] < ack["frame"] <= event["post_frame"] == captured["frame"]
                       and captured["nonce"] == raw["nonce"] and captured["pid"] == raw["process_id"]
                       and type(captured["world_epoch"]) is int and captured["world_epoch"] == first["world_epoch"]
                       and captured["primary_window"] == first["primary_window"] and captured["paused"] is True
                       and captured["world_input_captured"] is False and captured["pointer_over_ui"] is False
                       and type(sample["sample_index"]) is int and sample["sample_index"] == index
                       and sample["play_mode"] == "Normal" and sample["task_mode"] == "None" and sample["menu_state"] == "Hidden"
                       and camera_scale(captured) == sample["camera_scale"] == event["camera_after"],
                       "maxzoom wheel ACK chain differs")
        native.require(abs(event["camera_after"] - zoom_after(scale, ack)) <= 1e-5,
                       "maxzoom recorded camera step differs from actual wheel delta")
        scale = event["camera_after"]
    native.require(abs(scale - 5) <= 1e-5 and len({event["wheel_ack"]["index"] for event in scrolls}) == len(scrolls),
                   "maxzoom final scale/unique wheel evidence differs")
    check_sequence(raw, entries, client, events, scrolls)


def selection_points(entry, events, previous_step):
    value = entry["value"]
    picker = entry.get("selection_picker")
    native.require(len(events) == (9 if picker is not None else 2), "maxzoom selection input count differs")
    pairs = [events[:2]]
    points = [point(value)]
    times = []
    if picker is not None:
        native.require(isinstance(picker, dict) and set(picker) == {"toggle", "target"}, "maxzoom picker proof differs")
        bits, _ = owner(value)
        keys = [picker[name]["key"] for name in ("toggle", "target")]
        tokens = [key.split(":") for key in keys]
        native.require(all(len(row) == 4 and row[0] == "world-candidate" and row[2].isdigit()
                           and row[3] == str(value["world_epoch"]) for row in tokens)
                       and tokens[0][1] == "toggle" and tokens[1][1] == str(bits) and tokens[0][2:] == tokens[1][2:],
                       "maxzoom candidate target/revision/epoch differs")
        for offset, name in ((2, "toggle"), (5, "target")):
            proof = picker[name]
            key, observed = proof["key"], proof["hover"]
            motion, press, release = events[offset:offset + 3]
            control = observed["controls"].get(key, {})
            rect = control.get("rect")
            native.require(visible_rect(rect, value["viewport"]) and control.get("fully_visible") is True
                           and control.get("interaction") == "Hovered", "maxzoom candidate control is not visibly hovered")
            center = [round((rect[0] + rect[2]) / 2), round((rect[1] + rect[3]) / 2)]
            native.require(type(proof["first_step"]) is int and type(proof["last_step"]) is int
                           and proof["first_step"] == previous_step + offset + 1 and proof["last_step"] == previous_step + offset + 3
                           and observed["nonce"] == value["nonce"] and type(observed["pid"]) is int and observed["pid"] == value["pid"]
                           and observed["world_epoch"] == value["world_epoch"] and observed["primary_window"] == value["primary_window"]
                           and observed["frame"] == motion["post_frame"] <= press["pre_frame"] < release["post_frame"]
                           and observed["paused"] is True and abs(camera_scale(observed) - 5) <= 1e-5
                           and owner(observed)[0] == bits and observed["selected"] != bits
                           and motion.get("point") == center and motion.get("button") is None and motion.get("key") is None
                           and motion.get("scroll_discrete") is None and motion.get("observed_hover", {}).get(key) == "Hovered"
                           and press.get("observed_hover", {}).get(key) == "Pressed",
                           "maxzoom candidate action identity/input binding differs")
            points.append(center)
            pairs.append([press, release])
            times.append(usage.context(observed)["real_seconds"])
        motion = events[-1]
        native.require(motion.get("point") == list(point(value)) and motion.get("button") is None
                       and motion.get("key") is None and motion.get("scroll_discrete") is None,
                       "maxzoom candidate selection did not return to owner hover")
    for pair in pairs:
        native.require([event.get("button") for event in pair] == [1, 1]
                       and [event.get("pressed") for event in pair] == [True, False]
                       and [event.get("observed_left_pressed") for event in pair] == [True, False]
                       and all(event.get("point") is None and event.get("key") is None and event.get("scroll_discrete") is None
                               for event in pair), "maxzoom selection has no single acknowledged Left pair per action")
    return points, times


def check_sequence(raw, entries, client, events, scrolls):
    native.require([event.get("step") for event in events] == [str(index + 1) for index in range(len(events))],
                   "maxzoom input sequence differs")
    first = entries[2]
    previous_frame, previous_step = first["value"]["frame"], first["last_step"]
    previous_sample = checkpoint_sample(raw["samples"], first["value"])
    for event in events:
        native.require(event["nonce"] == client["nonce"] and event["pid"] == client["pid"] and event["window"] == client["window"]
                       and event["world_epoch"] == first["value"]["world_epoch"]
                       and all(type(event[key]) is int for key in ("pre_frame", "post_frame"))
                       and event["pre_frame"] < event["post_frame"], "maxzoom input identity/ACK differs")
    maxzoom_events = events[previous_step:]
    native.require(all(a["post_frame"] <= b["pre_frame"] for a, b in zip(maxzoom_events, maxzoom_events[1:])),
                   "maxzoom input intervals overlap or reverse")
    for index, entry in enumerate(entries[3:]):
        value, step = entry["value"], entry["last_step"]
        native.require(value.get("nonce") == first["value"]["nonce"] == client["nonce"] == raw["nonce"]
                       and type(value.get("pid")) is int and type(client.get("pid")) is int
                       and value["pid"] == first["value"]["pid"] == client["pid"] == raw["process_id"]
                       and type(value.get("primary_window")) is int
                       and value["primary_window"] == first["value"]["primary_window"],
                       "maxzoom checkpoint nonce/PID/window differs")
        sample = checkpoint_sample(raw["samples"], value)
        native.require(type(step) is int and previous_step < step <= len(events)
                       and type(value["frame"]) is int and value["frame"] > previous_frame
                       and sample["sample_index"] > previous_sample["sample_index"]
                       and sample["real_seconds"] > previous_sample["real_seconds"],
                       "maxzoom checkpoint input/frame/raw order differs")
        phase_events = events[previous_step:step]
        native.require(all(previous_frame <= event["pre_frame"] < event["post_frame"] <= value["frame"]
                           for event in phase_events), "maxzoom input lies outside its checkpoint interval")
        if index % 2 == 0:
            motion = phase_events[-1]
            native.require(motion.get("point") is not None and list(motion["point"]) == list(point(value))
                           and motion.get("button") is None and motion.get("key") is None
                           and motion.get("scroll_discrete") is None, "maxzoom hover has no fresh owner motion")
        else:
            points, action_times = selection_points(entry, phase_events, previous_step)
            times = [previous_sample["real_seconds"], *action_times, sample["real_seconds"]]
            native.require(all(finite(at) for at in times) and all(a < b for a, b in zip(times, times[1:])),
                           "maxzoom selection action times differ")
            buttons = [event for event in raw["events"] if event.get("producer") == "MouseButtonInput"
                       and previous_sample["sample_index"] < event.get("sample_index", -1) <= sample["sample_index"]]
            native.require(len(buttons) == 2 * len(points) and [event.get("state") for event in buttons] == ["Pressed", "Released"] * len(points)
                           and all(a["event_index"] < b["event_index"] for a, b in zip(buttons, buttons[1:])),
                           "maxzoom selection lacks a unique ordered raw Left pair")
            for button_index, event in enumerate(buttons):
                row = raw["samples"][event["sample_index"]]
                cursor, dpi = row.get("cursor_position"), row.get("dpi")
                bits, _ = owner(value)
                native.require(event.get("operation") == "MouseButtonInput" and event.get("button") == "Left"
                               and event.get("window") == row.get("primary_window") == previous_sample.get("primary_window")
                               and isinstance(event.get("window"), str) and bool(event["window"])
                               and row.get("world_epoch") == value["world_epoch"] and row.get("paused") is True
                               and row.get("virtual_seconds") == previous_sample["virtual_seconds"] == sample["virtual_seconds"]
                               and finite(event.get("real_seconds"))
                               and times[button_index // 2] < event["real_seconds"] <= row["real_seconds"] <= times[button_index // 2 + 1]
                               and finite(dpi) and dpi > 0 and isinstance(cursor, list) and len(cursor) == 2
                               and all(finite(axis) for axis in cursor)
                               and all(abs(axis * dpi - expected) <= 2 for axis, expected in zip(cursor, points[button_index // 2], strict=True))
                               and any(item.get("entity_bits") == bits
                                       for item in row.get("rest_area_context", {}).get("owners", [])),
                               "maxzoom raw Left window/epoch/owner/point/time differs")
        if index in (0, 1, 3):
            native.require(sample["virtual_seconds"] == previous_sample["virtual_seconds"], "maxzoom paused phase advanced simulation")
        else:
            native.require(sample["virtual_seconds"] > previous_sample["virtual_seconds"]
                           and any(row.get("paused") is False and previous_sample["sample_index"] < row["sample_index"] < sample["sample_index"]
                                   for row in raw["samples"]), "maxzoom occupancy lacks ordinary simulation between phases")
        previous_frame, previous_step, previous_sample = value["frame"], step, sample
    empty_hover = entries[3]
    native.require(all(first["last_step"] < int(event["step"]) <= empty_hover["last_step"]
                       and first["value"]["frame"] <= event["pre_frame"] < event["post_frame"] < empty_hover["value"]["frame"]
                       for event in scrolls), "maxzoom wheel was not between completion and empty hover")
