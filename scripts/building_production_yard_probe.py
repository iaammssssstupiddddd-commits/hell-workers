"""Bounded ordinary camera input to an observed existing Yard, never a seed."""
from __future__ import annotations

import math
import time

from . import building_production_selection_probe as selection


def point(sample):
    """An interior Tank trial point, not a placement-can-succeed decision."""
    context = sample.get("placement_context")
    selection.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] >= 0
                      and isinstance(sample.get("primary_window"), str) and bool(sample["primary_window"])
                      and type(sample.get("dpi")) in (int, float) and sample["dpi"] == 1
                      and isinstance(context, dict) and isinstance(context.get("camera"), str)
                      and bool(context["camera"]) and context.get("target_window") == sample["primary_window"]
                      and context.get("logical_viewport_size") == [1280, 720],
                      "Yard camera is not the owned full-size primary viewport")
    def vector(value, size, *, integers=False):
        return (isinstance(value, list) and len(value) == size
                and all(type(v) is int if integers else type(v) in (int, float) and math.isfinite(v) for v in value))
    selection.require(vector(context.get("camera_position"), 3)
                      and vector(context.get("logical_viewport_size"), 2)
                      and isinstance(context.get("yards"), list) and len(context["yards"]) == 1,
                      "one observed existing Yard is required")
    yard = context["yards"][0]
    selection.require(isinstance(yard, dict) and isinstance(yard.get("entity"), str) and bool(yard["entity"])
                      and all(vector(yard.get(k), 2) for k in ("min", "max", "grid_world_min", "grid_world_max"))
                      and all(vector(yard.get(k), 2, integers=True) for k in ("grid_min", "grid_max"))
                      and yard["min"] == yard["grid_world_min"] and yard["max"] == yard["grid_world_max"],
                      "Yard boundaries are not actual aligned grid centers")
    low, high = yard["grid_min"], yard["grid_max"]
    corners = yard.get("logical_viewport_corners")
    selection.require(all(0 <= a < b < 100 and b - a >= 8 for a, b in zip(low, high, strict=True))
                      and isinstance(corners, list) and len(corners) == 4
                      and all(vector(corner, 2) for corner in corners), "Yard grid/projection is unavailable")
    a, b, c, d = corners
    selection.require(abs(a[1] - b[1]) < .001 and abs(c[1] - d[1]) < .001
                      and abs(a[0] - d[0]) < .001 and abs(b[0] - c[0]) < .001
                      and b[0] > a[0] and a[1] > d[1], "Yard projection is not unrotated top-down")
    grid = [low[0] + 3, high[1] - 3]
    x, y = [(grid[i] - low[i]) / (high[i] - low[i]) for i in range(2)]
    pixel = [round(a[i] + x * (b[i] - a[i]) + y * (d[i] - a[i])) for i in range(2)]
    return yard, grid, pixel


def normal(sample, window, epoch, yard, camera=None):
    current, _, pixel = point(sample)
    selection.require(sample["primary_window"] == window and sample["world_epoch"] == epoch
                      and sample.get("menu_state") == "Hidden" and sample.get("play_mode") == "Normal"
                      and sample.get("task_mode") == "None" and "selected_building" in sample
                      and sample["selected_building"] is None
                      and (camera is None or sample["placement_context"]["camera"] == camera)
                      and all(current[k] == yard[k] for k in ("entity", "min", "max", "grid_min", "grid_max",
                                                             "grid_world_min", "grid_world_max")),
                      "Yard normal-input context changed")
    return pixel


def verify(raw, evidence):
    selection.require(set(evidence) == {"before", "after", "legs"}
                      and all(type(evidence[k]) is int for k in ("before", "after"))
                      and 0 <= evidence["before"] <= evidence["after"] < len(raw["samples"])
                      and isinstance(evidence["legs"], list) and len(evidence["legs"]) <= 2,
                      "Yard input evidence ordinals differ")
    samples = raw["samples"][evidence["before"]:evidence["after"] + 1]
    yard, _, _ = point(samples[0])
    window, epoch, camera = samples[0]["primary_window"], samples[0]["world_epoch"], samples[0]["placement_context"]["camera"]
    for sample in samples:
        normal(sample, window, epoch, yard, camera)
    previous = evidence["before"]
    last_axis = -1
    for leg in evidence["legs"]:
        selection.require(set(leg) == {"before", "after", "event_boundary", "key"}
                          and all(type(leg[k]) is int for k in ("before", "after", "event_boundary"))
                          and previous <= leg["before"] < leg["after"] <= evidence["after"]
                          and 0 <= leg["event_boundary"] <= len(raw["events"])
                          and leg["key"] in {"a", "d", "w", "s"}, "Yard pan leg differs")
        events = [event for event in raw["events"][leg["event_boundary"]:]
                  if event.get("producer") == "KeyboardInput" and event.get("key") == "Key" + leg["key"].upper()
                  and event.get("repeat") is False and event.get("sample_index", len(raw["samples"])) <= leg["after"]]
        selection.require(len(events) == 2 and [event.get("state") for event in events] == ["Pressed", "Released"]
                          and all(event.get("window") == window and type(event.get("sample_index")) is int
                                  and leg["before"] < event["sample_index"] <= leg["after"] for event in events),
                          "Yard pan original key messages differ")
        coordinate = 0 if leg["key"] in {"a", "d"} else 1
        selection.require(coordinate > last_axis, "Yard pan axes are duplicated or reordered")
        delta = raw["samples"][leg["after"]]["placement_context"]["camera_position"][coordinate] - raw["samples"][leg["before"]]["placement_context"]["camera_position"][coordinate]
        selection.require(delta > 0 if leg["key"] in {"d", "w"} else delta < 0, "Yard camera did not move in original key direction")
        previous = leg["after"]
        last_axis = coordinate
    _, _, pixel = point(samples[-1])
    selection.require(100 <= pixel[0] <= 1180 and 90 <= pixel[1] <= 350,
                      "Yard final point is outside unobstructed world viewport")


def run(transport, read):
    raw = read()
    before = len(raw["samples"]) - 1
    initial = raw["samples"][-1]
    yard, _, _ = point(initial)
    window, epoch, nonce = initial["primary_window"], initial["world_epoch"], raw["nonce"]
    camera = initial["placement_context"]["camera"]
    normal(initial, window, epoch, yard, camera)
    legs = []
    for axis, low, high, positive, negative in ((0, 512, 768, "d", "a"), (1, 140, 300, "s", "w")):
        raw = read()
        selection.require(raw["nonce"] == nonce, "Yard input nonce changed")
        pixel = normal(raw["samples"][-1], window, epoch, yard, camera)
        if low <= pixel[axis] <= high:
            continue
        key = positive if pixel[axis] > high else negative
        start, boundary = len(raw["samples"]) - 1, len(raw["events"])
        transport.send(f"pan-{axis}-press", nonce, key=key, pressed=True)
        try:
            deadline = time.monotonic() + 4
            while time.monotonic() < deadline:
                raw = read()
                selection.require(raw["nonce"] == nonce, "Yard input nonce changed")
                pixel = normal(raw["samples"][-1], window, epoch, yard, camera)
                if low <= pixel[axis] <= high:
                    break
                increasing = key in {"a", "w"}
                selection.require(not (pixel[axis] > high if increasing else pixel[axis] < low),
                                  "ordinary Yard camera crossed endpoint without an observation")
                time.sleep(.05)
            else:
                raise RuntimeError("ordinary Yard camera pan timeout")
        finally:
            transport.send(f"pan-{axis}-release", nonce, key=key, pressed=False)
        raw = selection.wait(read, lambda value: len(value["samples"]) - 1 > start + 1
                             and any(e.get("producer") == "KeyboardInput" and e.get("key") == "Key" + key.upper()
                                     and e.get("state") == "Released" for e in value["events"][boundary:]), timeout=2)
        normal(raw["samples"][-1], window, epoch, yard, camera)
        legs.append({"before": start, "after": len(raw["samples"]) - 1, "event_boundary": boundary, "key": key})
    raw = selection.wait(read, lambda value: len(value["samples"]) >= 2
                         and value["samples"][-1].get("placement_context") == value["samples"][-2].get("placement_context"), timeout=2)
    selection.require(raw["nonce"] == nonce, "Yard input nonce changed")
    pixel = normal(raw["samples"][-1], window, epoch, yard, camera)
    selection.require(100 <= pixel[0] <= 1180 and 90 <= pixel[1] <= 350,
                      "Yard trial point is outside unobstructed world viewport")
    evidence = {"before": before, "after": len(raw["samples"]) - 1, "legs": legs}
    verify(raw, evidence)
    return evidence
