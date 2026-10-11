"""One ordinary Tank world click: branch observation, not completed placement."""
from __future__ import annotations

from . import building_production_lifecycle as lifecycle
from . import building_production_selection_probe as selection

POINT = [640, 300]
PRODUCER = "ordinary-building-placement-branch-v1"


def check_selected_context(samples, window, epoch):
    selection.require(isinstance(window, str) and bool(window) and type(epoch) is int and epoch >= 0
                      and samples and all(sample.get("primary_window") == window
                              and type(sample.get("world_epoch")) is int and sample.get("world_epoch") == epoch
                              and type(sample.get("dpi")) in (int, float) and sample.get("dpi") == 1
                              and isinstance(sample.get("blueprints"), list)
                              and selection.selected(sample, "Tank") for sample in samples),
                      "placement probe selected context changed")


def verify(raw, evidence):
    lifecycle.check_raw_transitions(raw)
    selection.require(set(evidence) == {"before", "pointer", "after", "event_boundary", "point", "outcome_index"},
                      "placement probe evidence shape differs")
    before, pointer, after, boundary, outcome_index = [evidence[key] for key in
        ("before", "pointer", "after", "event_boundary", "outcome_index")]
    selection.require(all(type(index) is int for index in (before, pointer, after, boundary, outcome_index))
                      and 0 <= before < pointer < after < len(raw["samples"])
                      and 0 <= boundary <= outcome_index < len(raw["events"]) and evidence["point"] == POINT,
                      "placement probe ordinals differ")
    samples = raw["samples"][before:after + 1]
    window, epoch = samples[0].get("primary_window"), samples[0].get("world_epoch")
    check_selected_context(samples, window, epoch)
    selection.require(all(sample.get("cursor_position") == POINT for sample in raw["samples"][pointer:after + 1]),
                      "placement probe window/mode/cursor changed")
    events = [event for event in raw["events"][boundary:] if event.get("sample_index", after + 1) <= after]
    mouse = [event for event in events if event.get("producer") == "MouseButtonInput"]
    outcomes = [event for event in events if event.get("producer") == PRODUCER]
    selection.require(len(mouse) == 2 and [event.get("state") for event in mouse] == ["Pressed", "Released"]
                      and all(event.get("button") == "Left" and event.get("window") == window
                              and pointer < event.get("sample_index", -1) <= after for event in mouse)
                      and mouse[0]["sample_index"] <= mouse[1]["sample_index"]
                      and mouse[0]["real_seconds"] <= mouse[1]["real_seconds"]
                      and len(outcomes) == 1 and raw["events"][outcome_index] == outcomes[0],
                      "placement probe input/branch inventory differs")
    event = outcomes[0]
    lifecycle.check_placement_receipt(event, raw["samples"])
    selection.require(event["outcome"]["kind"] == "Tank"
                      and event["outcome"]["status"] in {"Rejected", "CompanionPending"}
                      and event["sample_index"] == mouse[0]["sample_index"]
                      and event["real_seconds"] == mouse[0]["real_seconds"]
                      and all(sample.get("blueprints") == samples[0].get("blueprints") for sample in samples),
                      "first Tank click cannot claim a committed owner")


def run(transport, read):
    raw = read()
    before = len(raw["samples"]) - 1
    window, epoch, nonce = raw["samples"][-1].get("primary_window"), raw["samples"][-1].get("world_epoch"), raw["nonce"]
    check_selected_context(raw["samples"][-1:], window, epoch)
    transport.send("world-motion", nonce, point=POINT)
    raw = selection.wait(read, lambda value: len(value["samples"]) - 1 > before
                         and value["samples"][-1].get("cursor_position") == POINT, timeout=2)
    pointer, boundary = len(raw["samples"]) - 1, len(raw["events"])
    selection.require(raw["nonce"] == nonce, "placement probe nonce changed before click")
    check_selected_context(raw["samples"][before:pointer + 1], window, epoch)
    transport.send("world-press", nonce, button=1, pressed=True)
    transport.send("world-release", nonce, button=1, pressed=False)
    raw = selection.wait(read, lambda value: len(value["samples"]) - 1 > pointer + 1
                         and any(event.get("producer") == PRODUCER for event in value["events"][boundary:])
                         and any(event.get("producer") == "MouseButtonInput" and event.get("state") == "Released"
                                 for event in value["events"][boundary:]))
    indices = [index for index in range(boundary, len(raw["events"])) if raw["events"][index].get("producer") == PRODUCER]
    selection.require(len(indices) == 1, "one actual placement branch result is required")
    evidence = {"before": before, "pointer": pointer, "event_boundary": boundary,
                "after": len(raw["samples"]) - 1, "point": POINT, "outcome_index": indices[0]}
    verify(raw, evidence)
    return evidence
