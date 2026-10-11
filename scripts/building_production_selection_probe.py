"""Bounded feedback-only equipment-card input; no placement or asset authority."""
from __future__ import annotations

import math
from pathlib import Path
import time

KINDS = ("Tank", "MudMixer", "RestArea", "SoulSpa", "Door", "WheelbarrowParking",
         "SandPile", "BonePile", "OutdoorLamp")


def require(value: bool, reason: str) -> None:
    if not value:
        raise RuntimeError(reason)


def card(sample: dict, kind: str) -> tuple[dict, tuple[int, int]]:
    require(kind in KINDS and type(sample.get("dpi")) in (int, float) and sample.get("dpi") == 1
            and sample.get("menu_state") == "Architect"
            and sample.get("architect_panel_ready") is True
            and sample.get("play_mode") == "Normal" and sample.get("task_mode") == "None"
            and sample.get("selected_building") is None,
            "equipment selection requires ordinary unselected Architect at DPI 1")
    cards = sample.get("architect_cards")
    require(isinstance(cards, list) and len(cards) <= 32, "equipment card inventory missing or capped")
    rows = [row for row in cards if isinstance(row, dict) and row.get("kind") == kind]
    require(len(rows) == 1, "one visible equipment card is required")
    row = rows[0]
    rect = row.get("physical_rect")
    require(set(row) == {"entity", "kind", "physical_rect", "physical_viewport"}
            and isinstance(row["entity"], str) and bool(row["entity"])
            and row["physical_viewport"] == [1280, 720]
            and isinstance(rect, list) and len(rect) == 4
            and all(type(v) in (int, float) and math.isfinite(v) for v in rect),
            "equipment physical rectangle differs")
    left, top, right, bottom = rect
    require(0 <= left < right <= 1280 and 0 <= top < bottom <= 720
            and right - left >= 2 and bottom - top >= 2, "equipment rectangle outside client")
    point = (int((left + right) / 2), int((top + bottom) / 2))
    require(left < point[0] < right and top < point[1] < bottom, "equipment center is not interior")
    return row, point


def selected(sample: dict, kind: str) -> bool:
    return (sample.get("menu_state") == "Architect"
            and (sample.get("selected_building") is None
                 and sample.get("play_mode") == "TaskDesignation"
                 and sample.get("task_mode") == "SoulSpaPlace(None)"
                 if kind == "SoulSpa" else
                 sample.get("selected_building") == kind
                 and sample.get("play_mode") == "BuildingPlace" and sample.get("task_mode") == "None"))


def wait(read, predicate, *, timeout: float = 8) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        raw = read()
        if predicate(raw):
            return raw
        time.sleep(0.05)
    raise RuntimeError("ordinary equipment selection witness timeout")


def verify(raw: dict, evidence: dict, kind: str) -> None:
    require(set(evidence) == {"before", "pointer", "event_boundary", "selected", "end", "card", "point"},
            "equipment selection evidence shape differs")
    before, pointer, start, end = [evidence[k] for k in ("before", "pointer", "selected", "end")]
    require(kind in KINDS and all(type(i) is int for i in (before, pointer, start, end))
            and 1 <= before < pointer <= start < end < len(raw["samples"]),
            "equipment selection sample order differs")
    require(isinstance(evidence["point"], list) and len(evidence["point"]) == 2
            and all(type(v) is int for v in evidence["point"]), "equipment point shape differs")
    samples = raw["samples"]
    epoch = samples[before].get("world_epoch")
    require(type(epoch) is int and epoch >= 0 and all(type(s.get("world_epoch")) is int and s.get("world_epoch") == epoch
            for s in samples[before - 1:end + 1]), "equipment selection world changed")
    window = samples[before].get("primary_window")
    require(isinstance(window, str) and bool(window)
            and all(s.get("primary_window") == window and type(s.get("dpi")) in (int, float)
                    and s.get("dpi") == 1 for s in samples[before - 1:end + 1]),
            "equipment primary window changed or missing")
    for sample in samples[before - 1:pointer + 1]:
        row, point = card(sample, kind)
        require(row == evidence["card"] and list(point) == evidence["point"],
                "equipment card changed before click")
    require(samples[pointer].get("cursor_position") == evidence["point"]
            and all(selected(s, kind) for s in samples[start - 1:end + 1]),
            "equipment input position or selected capture interval differs")
    boundary = evidence["event_boundary"]
    require(type(boundary) is int and 0 <= boundary <= len(raw["events"]),
            "equipment mouse event boundary differs")
    events = [e for e in raw["events"][boundary:] if e.get("producer") == "MouseButtonInput"]
    require(len(events) == 2 and [e.get("state") for e in events] == ["Pressed", "Released"]
            and all(e.get("operation") == "MouseButtonInput" and e.get("button") == "Left"
                    and e.get("window") == window
                    and type(e.get("sample_index")) is int and pointer <= e["sample_index"] <= start
                    for e in events)
            and events[0]["window"] == events[1]["window"]
            and type(events[0].get("event_index")) is int and type(events[1].get("event_index")) is int
            and events[0]["event_index"] < events[1]["event_index"],
            "one real ordered equipment mouse press/release is required")


def run(transport, read, kind: str, root: Path, capture) -> dict:
    raw = read()
    before = len(raw["samples"]) - 1
    require(before >= 1, "equipment card has no stable sample")
    row, point = card(raw["samples"][-1], kind)
    require(card(raw["samples"][-2], kind) == (row, point), "equipment card layout is unsettled")
    epoch = raw["samples"][-1].get("world_epoch")
    window = raw["samples"][-1].get("primary_window")
    require(type(epoch) is int and epoch >= 0 and isinstance(window, str) and bool(window),
            "equipment source world/window is missing")
    require(type(raw["samples"][-2].get("world_epoch")) is int
            and raw["samples"][-2].get("world_epoch") == epoch
            and raw["samples"][-2].get("primary_window") == window,
            "equipment initial world/window is unsettled")
    transport.send("select-motion", raw["nonce"], point=point)
    raw = wait(read, lambda r: len(r["samples"]) - 1 > before
               and r["samples"][-1].get("cursor_position") == list(point), timeout=2)
    pointer = len(raw["samples"]) - 1
    require(card(raw["samples"][-1], kind) == (row, point)
            and type(raw["samples"][-1].get("world_epoch")) is int
            and raw["samples"][-1].get("world_epoch") == epoch
            and raw["samples"][-1].get("primary_window") == window,
            "equipment target changed after pointer motion")
    boundary = len(raw["events"])
    transport.send("select-press", raw["nonce"], button=1, pressed=True)
    transport.send("select-release", raw["nonce"], button=1, pressed=False)
    raw = wait(read, lambda r: len(r["samples"]) - 1 > pointer
               and all(selected(s, kind) for s in r["samples"][-2:]))
    start = len(raw["samples"]) - 1
    capture(root / "selected.png")
    raw = wait(read, lambda r: len(r["samples"]) - 1 > start, timeout=2)
    evidence = {"before": before, "pointer": pointer, "selected": start,
                "end": len(raw["samples"]) - 1, "event_boundary": boundary,
                "card": row, "point": list(point)}
    verify(raw, evidence, kind)
    return evidence
