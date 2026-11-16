"""OS-input verification of the currently selected stockpile policy target."""
import copy
import math
import native_acceptance as native

CASE = "refactor-stockpile-target"
CHECKPOINTS = ("stockpile-initial", "stockpile-b-selected", "stockpile-b-cleared",
               "stockpile-a-selected", "stockpile-a-cleared")


def cells(value):
    rows = value["refactor_stockpile"]["cells"]
    native.require(isinstance(rows, list) and len(rows) == 2 and [r["index"] for r in rows] == [0, 1]
        and len({r["entity"] for r in rows}) == 2
        and all(type(r["entity"]) is int and r["map_owner"] == r["entity"]
            and r["priority"] == "Normal" and r["target_amount"] == 10 and r["allow_export"] is True for r in rows),
        "stockpile fixture ownership/policy differs")
    return rows


def check_observation(case, value):
    native.require(case in CHECKPOINTS and value.get("paused") is True, "stockpile checkpoint differs")
    a, b = cells(value)
    expected = {"stockpile-initial": (True, True), "stockpile-b-selected": (True, True),
                "stockpile-b-cleared": (True, False), "stockpile-a-selected": (True, False),
                "stockpile-a-cleared": (False, False)}[case]
    native.require((a["is_any"], b["is_any"]) == expected
        and (a["is_none"], b["is_none"]) == tuple(not item for item in expected), "stockpile acceptance differs")
    if case != "stockpile-initial":
        target = b if case.startswith("stockpile-b") else a
        button = value["refactor_stockpile"]["button"]
        native.require(value.get("selected") == value.get("inspection_target") == target["entity"]
            and isinstance(button, dict) and button.get("target") == target["entity"]
            and button.get("clear") is True and button.get("other_empty") is True and button.get("rect")
            and value["controls"].get(button["key"], {}).get("entity") == button["entity"]
            and value["controls"][button["key"]].get("fully_visible") is True, "stockpile visible button target differs")


def action(driver, name, *, index=None):
    first = len(driver.events) + 1
    if index is not None:
        row = cells(driver.last)[index]
        point = row.get("point") or {}
        native.require(point.get("rank") == 0 and isinstance(point.get("screen"), list), "stockpile direct target missing")
        scale = driver.last["scale_factor"]
        driver.send(point=tuple(round(n * scale) for n in point["screen"]))
        driver.last = driver.wait(lambda v: v.get("pointer_target") == row["entity"]
                                  and not v["pointer_over_ui"] and not v["world_input_captured"], after=driver.last["frame"])
        before = copy.deepcopy(driver.last)
        driver.send(button=1, pressed=True)
        driver.send(button=1, pressed=False)
        driver.last = driver.wait(lambda v: v.get("selected") == row["entity"]
                                  and v.get("inspection_target") == row["entity"], after=driver.last["frame"])
    else:
        before = copy.deepcopy(driver.last)
        button = before["refactor_stockpile"]["button"]
        native.require(button and button["clear"] and button["other_empty"]
                       and button["target"] == before["inspection_target"], "stockpile button changed before input")
        driver.click(button["key"])
        driver.last = driver.wait(lambda v: cells(v)[1 if name == "clear-b" else 0]["is_none"], after=driver.last["frame"])
    after = copy.deepcopy(driver.last)
    driver.events[-1]["stockpile_action"] = {"name": name, "first": first, "last": len(driver.events), "before": before, "after": after}
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    driver.capture(CHECKPOINTS[0])
    for name, index, checkpoint in (("select-b", 1, 1), ("clear-b", None, 2),
                                    ("select-a", 0, 3), ("clear-a", None, 4)):
        action(driver, name, index=index)
        driver.capture(CHECKPOINTS[checkpoint])


def check_sequence(entries, events, client):
    native.require([e["case"] for e in entries] == list(CHECKPOINTS), "stockpile checkpoint order differs")
    initial = entries[0]["value"]
    ids = [r["entity"] for r in cells(initial)]
    for entry in entries:
        value = entry["value"]
        check_observation(entry["case"], value)
        native.require([r["entity"] for r in cells(value)] == ids and value["world_epoch"] == initial["world_epoch"],
                       "stockpile identity/epoch changed")
    actions = [e["stockpile_action"] for e in events if "stockpile_action" in e]
    native.require([a["name"] for a in actions] == ["select-b", "clear-b", "select-a", "clear-a"], "stockpile actions differ")
    claimed = set()
    previous = entries[0]["last_step"]
    for i, action_record in enumerate(actions):
        before, after = action_record["before"], action_record["after"]
        first, last = action_record["first"], action_record["last"]
        segment = events[first-1:last]
        native.require(type(first) is int and type(last) is int and entries[i]["last_step"] < first <= last <= entries[i+1]["last_step"]
            and before["frame"] < after["frame"] and entries[i]["value"]["frame"] <= before["frame"]
            and after["frame"] <= entries[i+1]["value"]["frame"]
            and all(v.get("nonce") == initial["nonce"] and v.get("pid") == initial["pid"]
                    and v.get("world_epoch") == initial["world_epoch"] for v in (before, after))
            and first > previous and segment and segment[-1].get("stockpile_action") == action_record
            and all("stockpile_action" not in e for e in segment[:-1])
            and all(int(e["step"]) == n and type(e.get("pre_frame")) is int and type(e.get("post_frame")) is int
                    and e["pre_frame"] < e["post_frame"] for n, e in enumerate(segment, start=first))
            and all(a["post_frame"] <= b["pre_frame"] for a, b in zip(segment, segment[1:])), "stockpile action binding differs")
        previous = last
        claimed.update(range(first, last + 1))
        buttons = [e for e in segment if e.get("button") == 1]
        moves = [e for e in segment if e.get("point") is not None]
        native.require(len(buttons) == 2 and [e.get("pressed") for e in buttons] == [True, False]
            and [e.get("observed_left_pressed") for e in buttons] == [True, False]
            and all(e.get("nonce") == client["nonce"] and e.get("pid") == client["pid"]
                    and e.get("window") == client["window"] and e.get("world_epoch") == initial["world_epoch"] for e in segment),
            "stockpile button receipt differs")
        native.require(len(moves) == 1 and moves[0]["pre_frame"] >= entries[i]["value"]["frame"]
            and isinstance(moves[0]["point"], (list, tuple)) and len(moves[0]["point"]) == 2
            and isinstance(moves[0].get("observed_cursor"), list) and len(moves[0]["observed_cursor"]) == 2
            and all(type(x) in (int, float) and type(y) in (int, float) and math.isfinite(x) and math.isfinite(y)
                    and abs(x - y * before["scale_factor"]) <= 2
                    for x, y in zip(moves[0]["point"], moves[0]["observed_cursor"], strict=True))
            and all([r["entity"] for r in cells(v)] == ids for v in (before, after)),
            "stockpile movement or action identities differ")
        native.require(all(e.get("key") is None and e.get("button") in (None, 1) for e in segment)
            and before["frame"] <= buttons[0]["pre_frame"] < buttons[-1]["post_frame"] <= after["frame"],
            "stockpile button order differs")
        target = ids[1 if i < 2 else 0]
        if i % 2 == 0:
            point = cells(before)[1 if i == 0 else 0]["point"]
            native.require(point and point.get("rank") == 0 and len(moves) == 1
                and moves[0]["post_frame"] <= before["frame"] <= buttons[0]["pre_frame"]
                and before.get("pointer_target") == target and before.get("pointer_over_ui") is False
                and before.get("world_input_captured") is False and before.get("selected") != target
                and before.get("selected") == entries[i]["value"].get("selected")
                and after.get("selected") == after.get("inspection_target") == target
                and all(isinstance(e.get("observed_cursor"), list) and len(e["observed_cursor"]) == 2
                    and all(type(x) in (int, float) and math.isfinite(x) and abs(x-y) <= 2
                            for x, y in zip(e["observed_cursor"], point["screen"], strict=True))
                    for e in moves + buttons), "stockpile world target differs")
        else:
            control = before["refactor_stockpile"]["button"]
            key, rect = control["key"], control["rect"]
            def inside(event):
                point = event.get("observed_cursor")
                return (isinstance(point, list) and len(point) == 2 and isinstance(rect, list) and len(rect) == 4
                        and all(type(n) in (int, float) and math.isfinite(n) for n in point + rect)
                        and rect[0] <= point[0] <= rect[2] and rect[1] <= point[1] <= rect[3])
            native.require(control["target"] == before["inspection_target"] == target
                and control["clear"] is True and control["other_empty"] is True
                and before["controls"].get(key, {}).get("entity") == control["entity"]
                and before["controls"][key].get("fully_visible") is True
                and len(moves) == 1 and moves[0].get("observed_hover", {}).get(key) == "Hovered"
                and moves[0]["post_frame"] <= buttons[0]["pre_frame"]
                and moves[0].get("observed_hover_entities", {}).get(key) == control["entity"]
                and buttons[0].get("observed_hover", {}).get(key) == "Pressed"
                and buttons[0].get("observed_hover_entities", {}).get(key) == control["entity"]
                and all(inside(e) for e in moves + buttons)
                and cells(after)[1 if i == 1 else 0]["is_none"] is True, "stockpile edited wrong target")
    native.require(all(int(e["step"]) in claimed for e in events
                       if entries[0]["last_step"] < int(e["step"]) <= entries[-1]["last_step"]
                       and (e.get("button") is not None or e.get("key") is not None)), "unclaimed stockpile input")
