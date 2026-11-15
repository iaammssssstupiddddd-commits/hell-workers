"""One ordinary Tank companion click, observing real Blueprint/stockpile owners.

This is technical planning feedback, not construction or production acceptance.
"""
from __future__ import annotations

import math

from . import building_production_lifecycle as lifecycle
from . import building_production_placement_probe as placement
from . import building_production_selection_probe as selection
from . import building_production_yard_probe as yard


def context(sample):
    value = sample.get("companion_context")
    selection.require(isinstance(value, dict) and set(value) == {"available", "pending", "storages"}
                      and value["available"] is True and isinstance(value["storages"], list)
                      and len(value["storages"]) <= 512, "actual companion context missing")
    return value


def pending(sample):
    value = context(sample)["pending"]
    observed, anchor, _ = yard.point(sample)
    selection.require(isinstance(value, dict)
                      and set(value) == {"parent_kind", "parent_anchor", "kind", "center", "radius", "required"}
                      and value["parent_kind"] == "Tank" and value["kind"] == "BucketStorage"
                      and value["parent_anchor"] == anchor and value["required"] is True,
                      "actual Tank companion pending anchor differs")
    # Tank is 2x2; the ordinary two-tile storage trial is immediately to its right.
    width = observed["grid_max"][0] - observed["grid_min"][0]
    height = observed["grid_max"][1] - observed["grid_min"][1]
    tile_sizes = [(observed["max"][i] - observed["min"][i]) / size
                  for i, size in enumerate((width, height))]
    center = [observed["min"][i] + (anchor[i] - observed["grid_min"][i] + .5) * tile_sizes[i] for i in range(2)]
    selection.require(tile_sizes == [32, 32] and value["center"] == center
                      and type(value["radius"]) in (int, float) and math.isfinite(value["radius"])
                      and value["radius"] == 160, "Tank companion geometry differs from current product contract")
    a, b, _, d = observed["logical_viewport_corners"]
    grid = [anchor[0] + 2, anchor[1]]
    x, y = [(grid[i] - observed["grid_min"][i]) / size for i, size in enumerate((width, height))]
    target = [round(a[i] + x * (b[i] - a[i]) + y * (d[i] - a[i])) for i in range(2)]
    selection.require(100 <= target[0] <= 1180 and 90 <= target[1] <= 350,
                      "companion trial is outside unobstructed viewport")
    return anchor, [anchor[0] + 2, anchor[1]], target


def verify(raw, evidence, first):
    lifecycle.check_raw_transitions(raw)
    placement.verify(raw, first, yard=True)
    selection.require(set(evidence) == {"before", "pointer", "after", "event_boundary", "point", "outcome_index"},
                      "companion evidence shape differs")
    before, pointer, after, boundary, outcome_index = [evidence[k] for k in
        ("before", "pointer", "after", "event_boundary", "outcome_index")]
    selection.require(all(type(i) is int for i in (before, pointer, after, boundary, outcome_index))
                      and first["after"] <= before < pointer < after < len(raw["samples"])
                      and 0 <= boundary <= outcome_index < len(raw["events"]), "companion ordinals differ")
    samples = raw["samples"][before:after + 1]
    window, epoch = samples[0]["primary_window"], samples[0]["world_epoch"]
    placement.check_selected_context(samples, window, epoch)
    anchor, grid, target = pending(samples[0])
    selection.require(evidence["point"] == target and samples[0]["blueprints"] == []
                      and all(s["placement_context"] == samples[0]["placement_context"] for s in samples)
                      and all(s.get("cursor_position") == target for s in raw["samples"][pointer:after + 1]),
                      "companion click context changed")
    for sample in raw["samples"][before:pointer + 1]:
        selection.require(context(sample) == context(samples[0]) and sample["blueprints"] == [],
                          "companion pending changed before click")
        pending(sample)
    events = [event for event in raw["events"][boundary:] if event.get("sample_index", after + 1) <= after]
    mouse = [e for e in events if e.get("producer") == "MouseButtonInput"]
    outcomes = [e for e in events if e.get("producer") == placement.PRODUCER]
    selection.require(len(mouse) == 2 and [e.get("state") for e in mouse] == ["Pressed", "Released"]
                      and all(e.get("window") == window and e.get("button") == "Left"
                              and pointer < e["sample_index"] <= after for e in mouse)
                      and mouse[0]["sample_index"] <= mouse[1]["sample_index"]
                      and mouse[0]["real_seconds"] <= mouse[1]["real_seconds"]
                      and len(outcomes) == 1 and raw["events"][outcome_index] == outcomes[0],
                      "companion original click/outcome inventory differs")
    event = outcomes[0]
    lifecycle.check_placement_receipt(event, raw["samples"])
    outcome = event["outcome"]
    footprint = [[anchor[0] + x, anchor[1] + y] for y in range(2) for x in range(2)]
    selection.require(outcome["status"] == "CompanionCommitted" and outcome["kind"] == "Tank"
                      and outcome["grid"] == grid and sorted(outcome["footprint"]) == sorted(footprint)
                      and event["sample_index"] == mouse[0]["sample_index"]
                      and event["real_seconds"] == mouse[0]["real_seconds"], "actual companion commit missing")
    final = samples[-1]
    owner, children = outcome["owner"], outcome["companion_owners"]
    blueprints = final["blueprints"]
    selection.require(len(blueprints) == 1 and blueprints[0]["entity"] == owner
                      and blueprints[0]["kind"] == "Tank"
                      and sorted(blueprints[0]["footprint"]) == sorted(footprint)
                      and context(final)["pending"] is None,
                      "committed Tank Blueprint state missing")
    old = {row["entity"]: row for row in context(samples[0])["storages"]}
    new = {row["entity"]: row for row in context(final)["storages"]}
    selection.require(len(old) == len(context(samples[0])["storages"])
                      and len(new) == len(context(final)["storages"])
                      and len(children) == 2 and set(new) - set(old) == set(children)
                      and all(new.get(key) == row for key, row in old.items()), "companion inventory differs")
    expected_grids = [grid, [grid[0] + 1, grid[1]]]
    observed = final["placement_context"]["yards"][0]
    for child, child_grid in zip(children, expected_grids, strict=True):
        row = new[child]
        position = row.get("position")
        expected_world = [observed["min"][i] + (child_grid[i] - observed["grid_min"][i]) * 32 for i in range(2)]
        selection.require(set(row) == {"entity", "position", "grid", "pending_owner", "owner", "stockpile_grids"}
                          and row["pending_owner"] == owner and row["owner"] is None
                          and row["grid"] == child_grid and row["stockpile_grids"] == [child_grid]
                          and isinstance(position, list) and len(position) == 3
                          and all(type(v) in (int, float) and math.isfinite(v) for v in position)
                          and position[:2] == expected_world,
                          "companion root/map ownership differs")
    layers = [row for row in final["owner_layers"] if row["entity"] == owner]
    selection.require(len(layers) == 1 and layers[0]["exists"] is True
                      and sorted(layers[0]["building_grids"]) == sorted(footprint), "Tank map owner missing")


def run(transport, read, first):
    raw = read()
    before = len(raw["samples"]) - 1
    original = raw["samples"][-1]
    window, epoch, nonce = original["primary_window"], original["world_epoch"], raw["nonce"]
    placement.check_selected_context([original], window, epoch)
    _, _, target = pending(original)
    selection.require(original["blueprints"] == [], "fresh Tank trial already has a Blueprint")
    transport.send("companion-motion", nonce, point=target)
    raw = selection.wait(read, lambda value: len(value["samples"]) - 1 > before
                         and value["samples"][-1].get("cursor_position") == target, timeout=2)
    pointer, boundary = len(raw["samples"]) - 1, len(raw["events"])
    selection.require(raw["nonce"] == nonce, "companion nonce changed before click")
    placement.check_selected_context(raw["samples"][before:pointer + 1], window, epoch)
    for sample in raw["samples"][before:pointer + 1]:
        selection.require(sample["placement_context"] == original["placement_context"]
                          and context(sample) == context(original) and sample["blueprints"] == [],
                          "companion context changed before click")
    transport.send("companion-press", nonce, button=1, pressed=True)
    transport.send("companion-release", nonce, button=1, pressed=False)
    raw = selection.wait(read, lambda value: len(value["samples"]) - 1 > pointer + 1
                         and any(e.get("producer") == placement.PRODUCER for e in value["events"][boundary:])
                         and any(e.get("producer") == "MouseButtonInput" and e.get("state") == "Released"
                                 for e in value["events"][boundary:]))
    selection.require(raw["nonce"] == nonce, "companion nonce changed after click")
    indices = [i for i in range(boundary, len(raw["events"])) if raw["events"][i].get("producer") == placement.PRODUCER]
    selection.require(len(indices) == 1, "one companion branch result required")
    evidence = {"before": before, "pointer": pointer, "after": len(raw["samples"]) - 1,
                "event_boundary": boundary, "point": target, "outcome_index": indices[0]}
    verify(raw, evidence, first)
    return evidence


def check_closed(sample, committed):
    """Escape ends selection, not the actual committed plan/map ownership."""
    def inventory(value):
        return [(bp["entity"], bp["kind"], bp["footprint"]) for bp in value["blueprints"]]
    selection.require(context(sample) == context(committed) and inventory(sample) == inventory(committed),
                      "Escape changed committed parent/companion planning inventory")
    selection.require(len(committed["blueprints"]) == 1, "committed Tank inventory differs")
    parent = committed["blueprints"][0]
    layers = [row for row in sample["owner_layers"] if row["entity"] == parent["entity"]]
    selection.require(len(layers) == 1 and layers[0]["exists"] is True
                      and sorted(layers[0]["building_grids"]) == sorted(parent["footprint"]),
                      "Escape changed committed Tank map ownership")
