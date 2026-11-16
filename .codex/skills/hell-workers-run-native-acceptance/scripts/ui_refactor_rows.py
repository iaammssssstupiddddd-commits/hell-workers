"""Independent expectations and OS input sequence for R04/R08 acceptance."""
import native_acceptance as native

TASKS = {
    "deconstruct": ("解体", "Deconstruct (GoingToTarget)"),
    "power": ("発電", "GeneratePower (Generating)"),
    "bucket": ("給水", "BucketTransport (GoingToBucket)"),
}
CHECKPOINTS = ("refactor-rows", "detail-deconstruct", "tooltip-deconstruct",
               "detail-power", "tooltip-power", "detail-bucket", "tooltip-bucket",
               "row-renamed", "row-folded", "row-searched", "row-search-cleared", "row-restored",
               "row-values-updated")


def check_value_update(before, after):
    changed = False
    for key in TASKS:
        original = before["refactor_rows"]["souls"][key]
        current = after["refactor_rows"]["souls"][key]
        row = current["row"]
        native.require(row and row["rect"] and row["entity"] == original["row"]["entity"],
                       "simulation value update replaced or hid the row")
        text = row["fatigue"]["text"]
        native.require(row["fatigue"]["rect"] and text.endswith("%")
                       and abs(float(text[:-1]) - current["fatigue"] * 100) <= 0.51,
                       "row fatigue does not match simulated Soul value")
        changed |= text != original["row"]["fatigue"]["text"] and abs(current["fatigue"] - original["fatigue"]) > 0.005
    native.require(changed, "simulation did not change a visible row value")


def check_observation(case, value):
    native.require(value["paused"], "row fixture must stay paused")
    evidence = value["refactor_rows"]
    souls = evidence["souls"]
    native.require(set(souls) == set(TASKS), "task caller coverage differs")
    native.require(len({soul["entity"] for soul in souls.values()}) == 3, "task Souls must be distinct")
    if case.startswith(("detail-", "tooltip-")):
        key = case.split("-", 1)[1]
        native.require(value["selected"] == souls[key]["entity"], "detail target differs")
        task = evidence["detail_task"]
        native.require(task and task["rect"] and TASKS[key][1] in task["text"], "task detail label differs or is hidden")
        if case.startswith("tooltip-"):
            native.require(evidence["hovered"] == souls[key]["entity"], "world hover target differs")
            visible = [item["text"] for item in value["tooltip_text"] if item["rect"] and item["alpha"] >= 0.99]
            native.require(value["tooltip_alpha"] >= 0.99 and any(TASKS[key][1] in text for text in visible), "task Tooltip differs or is hidden")
            native.require(key != "deconstruct" or not any("BucketTransport" in text for text in visible), "deconstruction mislabeled as bucket transport")
        return
    expected = set(TASKS) if case in ("refactor-rows", "row-renamed", "row-restored", "row-values-updated") else {"deconstruct"} if case == "row-searched" else set()
    visible = {key for key, soul in souls.items() if soul["row"] and soul["row"]["rect"]}
    native.require(visible == expected, "search/fold row visibility differs")
    for key in expected:
        row = souls[key]["row"]
        if case != "row-values-updated":
            native.require(row["label"]["rect"] and row["label"]["text"] == TASKS[key][0], "list task label differs or is hidden")
        native.require(row["name"]["rect"] and row["name"]["text"] == souls[key]["name"], "row name is stale or hidden")
        native.require(row["icon"]["rect"] and row["icon"]["icon"], "task icon is missing")
    if case != "refactor-rows":
        native.require(souls["deconstruct"]["name"] == "renamed", "real rename did not reach Soul identity")


def check_sequence(entries):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS), "row checkpoints differ")
    original = entries[0]["value"]
    for before, after in zip(entries, entries[1:]):
        native.require(after["last_step"] > before["last_step"] and after["value"]["frame"] > before["value"]["frame"], "row transition lacks fresh input evidence")
    for entry in entries:
        value = entry["value"]
        native.require(value["world_epoch"] == original["world_epoch"], "unexpected world replacement in row case")
        for key in TASKS:
            native.require(value["refactor_rows"]["souls"][key]["entity"] == original["refactor_rows"]["souls"][key]["entity"], "Soul identity changed")
        check_observation(entry["case"], value)
    check_value_update(entries[-2]["value"], entries[-1]["value"])


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
        driver.last = driver.wait(lambda value: "text-field:DevPoc" not in value["controls"], after=driver.last["frame"])
    driver.click("menu:Workspace(OpenEntities)")
    driver.capture("refactor-rows")
    for key in TASKS:
        soul = driver.last["refactor_rows"]["souls"][key]
        driver.click(f"soul:{soul['entity']}")
        driver.capture(f"detail-{key}")
        point = soul["world_point"]
        native.require(point is not None, "Soul is outside the camera")
        driver.send(point=tuple(round(coordinate) for coordinate in point))
        driver.last = driver.wait(lambda value: value["refactor_rows"]["hovered"] == soul["entity"]
                                  and value["tooltip_alpha"] >= 0.99
                                  and any(TASKS[key][1] in item["text"] and item["rect"] and item["alpha"] >= 0.99 for item in value["tooltip_text"]),
                                  after=driver.last["frame"])
        driver.capture(f"tooltip-{key}")
        driver.key("Escape")
        driver.last = driver.wait(lambda value: value["management_rect"] is not None, after=driver.last["frame"])
    soul = driver.last["refactor_rows"]["souls"]["deconstruct"]
    driver.click(f"soul:{soul['entity']}")
    driver.click("soul-rename")
    for letter in "renamed":
        driver.key(letter)
    driver.key("Return")
    driver.last = driver.wait(lambda value: value["refactor_rows"]["souls"]["deconstruct"]["name"] == "renamed", after=driver.last["frame"])
    driver.key("Escape")
    driver.capture("row-renamed")
    driver.click(f"fold:{soul['familiar']}")
    driver.capture("row-folded")
    field = next(key for key in driver.last["controls"] if key.startswith("text-field:EntityList"))
    driver.focus_text(field)
    for letter in "renamed":
        driver.key(letter)
    driver.capture("row-searched")
    driver.send(key="Control_L", pressed=True)
    driver.key("a")
    driver.send(key="Control_L", pressed=False)
    driver.key("BackSpace")
    driver.key("Escape")
    driver.capture("row-search-cleared")
    driver.click(f"fold:{soul['familiar']}")
    driver.capture("row-restored")
    before = driver.last
    driver.key("space")
    def updated(value):
        try:
            check_value_update(before, value)
        except native.AcceptanceError:
            return False
        return True
    driver.last = driver.wait(updated, after=driver.last["frame"], timeout=30)
    driver.key("space")
    driver.last = driver.wait(lambda value: value["paused"] and updated(value), after=driver.last["frame"])
    driver.capture("row-values-updated")


"""Independent OS-input checks for a pinned Soul and another selection."""
import math
import native_acceptance as native

PIN_CASE = "refactor-pinned-rename"
PIN_CHECKPOINTS = ("pin-initial", "pin-context", "pin-a", "pin-selected-b",
               "pin-rename-active", "pin-edit-abandoned", "pin-rename-applied", "pin-cleared")


def pin_souls(value):
    result = value["refactor_rows"]["souls"]
    native.require(set(result) == {"deconstruct", "power", "bucket"}, "pin fixture differs")
    a, b = result["deconstruct"], result["power"]
    native.require(a["entity"] != b["entity"], "pin targets must be distinct")
    return a, b


def check_pin_observation(case, value):
    native.require(case in PIN_CHECKPOINTS and value.get("paused") is True, "pin checkpoint differs")
    a, b = pin_souls(value)
    selected, pinned = value.get("selected"), value.get("pinned")
    target, rename = value.get("inspection_target"), value.get("rename_target")
    native.require(b["name"] == "RefactorPower", "other Soul was renamed")
    if case in ("pin-initial", "pin-context"):
        native.require(selected == a["entity"] and pinned is None and target == a["entity"] and rename is None,
                       "initial inspected Soul differs")
    elif case == "pin-a":
        native.require(selected == pinned == target == a["entity"] and rename is None, "pin target differs")
    elif case == "pin-cleared":
        native.require(selected == target == b["entity"] and pinned is None and rename is None, "pin clear target differs")
    elif case == "pin-edit-abandoned":
        native.require(selected == target == b["entity"] and pinned == a["entity"] and rename is None,
                       "selection change did not abandon the edit")
    else:
        native.require(selected == b["entity"] and pinned == target == a["entity"], "pinned and selected targets differ")
        native.require(rename == a["entity"] if case == "pin-rename-active" else rename is None,
                       "rename target differs")
    native.require(a["name"] == ("renamed" if case in ("pin-rename-applied", "pin-cleared") else "RefactorDeconstruct"),
                   "pin rename committed at the wrong time or to the wrong entity")
    header = value["controls"].get("text:inspection-header", {})
    shown = b["name"] if case in ("pin-edit-abandoned", "pin-cleared") else a["name"]
    native.require(value.get("inspector_rect"), "inspection panel is hidden")
    fields = [v for k, v in value["controls"].items() if k.startswith("text-field:SoulRename")]
    if case == "pin-rename-active":
        native.require(not header, "inspection header remains visible during rename")
        native.require(len(fields) == 1 and fields[0].get("fully_visible") is True
                       and fields[0].get("focused") is True and fields[0].get("editable") == "draft",
                       "draft did not reach the focused rename field")
    else:
        native.require(header.get("fully_visible") is True and shown in (header.get("text") or ""),
                       "visible inspection header differs")
        native.require(not fields, "rename field survived outside the active edit")
    if case == "pin-context":
        native.require(value.get("context_menus", 0) > 0 and
                       sum(k.startswith("menu:InspectEntity(") for k in value["controls"]) == 1,
                       "context inspect action is absent or ambiguous")


def pin_click(driver, key):
    import copy
    before = copy.deepcopy(driver.last)
    first = len(driver.events) + 1
    driver.click(key)
    action = {"key": key, "first": first, "last": len(driver.events),
              "before": before, "after": copy.deepcopy(driver.last)}
    driver.events[-1]["pin_action"] = action
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def pin_keys(driver, keys):
    import copy
    before = copy.deepcopy(driver.last)
    first = len(driver.events) + 1
    for key in keys:
        driver.key(key)
    driver.events[-1]["pin_action"] = {"key": "keys:" + ",".join(keys), "first": first,
                                       "last": len(driver.events), "before": before,
                                       "after": copy.deepcopy(driver.last)}
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def check_pin_sequence(entries, events, client):
    native.require([e["case"] for e in entries] == list(PIN_CHECKPOINTS), "pin checkpoint order differs")
    first = entries[0]["value"]
    first_value = first
    a, b = pin_souls(first)
    for entry in entries:
        value = entry["value"]
        check_pin_observation(entry["case"], value)
        x, y = pin_souls(value)
        native.require(value["world_epoch"] == first["world_epoch"] and x["entity"] == a["entity"]
                       and y["entity"] == b["entity"], "pin subject changed")
    for before, after in zip(entries, entries[1:]):
        native.require(before["last_step"] < after["last_step"] and before["value"]["frame"] < after["value"]["frame"],
                       "pin transition lacks fresh input")
    actions = [event["pin_action"] for event in events if "pin_action" in event]
    expected = ["inspect", "menu:Workspace(OpenEntities)", f"soul:{b['entity']}", "soul-rename",
                "keys:d,r,a,f,t", "menu:Workspace(InspectSelection)", "menu:Workspace(Back)",
                "soul-rename", "keys:r,e,n,a,m,e,d", "keys:Return", "menu:ClearInspectPin"]
    actual = ["inspect" if a["key"].startswith("menu:InspectEntity(") else a["key"] for a in actions]
    native.require(actual == expected, "pin action inventory differs")
    phases = [(1, 2), (2, 3), (2, 3), (3, 4), (3, 4), (4, 5),
              (5, 6), (5, 6), (5, 6), (5, 6), (6, 7)]
    previous_last = 0
    previous_frame = first_value["frame"]
    claimed = set()
    for action, (lower, upper) in zip(actions, phases, strict=True):
        first, last = action["first"], action["last"]
        native.require(type(first) is int and type(last) is int and 1 <= first <= last <= len(events)
                       and entries[lower]["last_step"] < first <= last <= entries[upper]["last_step"],
                       "pin action is outside its checkpoint interval")
        before, after = action["before"], action["after"]
        native.require(all(v.get("nonce") == first_value.get("nonce") and v.get("pid") == first_value.get("pid")
                           and v.get("world_epoch") == first_value["world_epoch"] for v in (before, after))
                       and before["frame"] < after["frame"], "pin action subject or frame differs")
        segment = events[first - 1:last]
        native.require(first > previous_last and segment[-1].get("pin_action") == action
                       and all("pin_action" not in e for e in segment[:-1]), "pin action overlaps or is misplaced")
        previous_last = last
        claimed.update(range(first, last + 1))
        native.require(previous_frame <= before["frame"], "pin action frame order differs")
        previous_frame = after["frame"]
        native.require(entries[lower]["value"]["frame"] <= before["frame"] < after["frame"]
                       <= entries[upper]["value"]["frame"] and all(
                           int(e["step"]) == i and e.get("nonce") == first_value.get("nonce")
                           and e.get("pid") == client.get("pid") and e.get("window") == client.get("window")
                           and e.get("world_epoch") == first_value["world_epoch"]
                           and before["frame"] <= e.get("pre_frame", -1) < e.get("post_frame", -1) <= after["frame"]
                           for i, e in enumerate(segment, first)), "pin action input subject, timing or sequence differs")
        for left, right_event in zip(segment, segment[1:]):
            same_tap = (left.get("key") is not None and left.get("key") == right_event.get("key")
                        and left.get("pressed") is True and right_event.get("pressed") is False
                        and left["pre_frame"] == right_event["pre_frame"] and left["post_frame"] == right_event["post_frame"])
            native.require(same_tap or left["post_frame"] <= right_event["pre_frame"], "pin event frame order differs")
        if action["key"].startswith("keys:"):
            keys = action["key"][5:].split(",")
            native.require([(e.get("key"), e.get("pressed")) for e in segment] ==
                           [(key, pressed) for key in keys for pressed in (True, False)],
                           "pin text gesture differs")
        else:
            control = before.get("controls", {}).get(action["key"], {})
            buttons = [e for e in segment if e.get("button") == 1]
            moves = [e for e in segment if e.get("observed_hover", {}).get(action["key"]) == "Hovered"]
            rect = moves[-1].get("observed_hover_rects", {}).get(action["key"]) if moves else None
            def inside(point):
                return (isinstance(rect, list) and len(rect) == 4 and isinstance(point, list) and len(point) == 2
                        and rect[0] <= point[0] <= rect[2] and rect[1] <= point[1] <= rect[3])
            native.require(control.get("fully_visible") is True and type(control.get("entity")) is int
                           and len(buttons) == 2 and [e.get("pressed") for e in buttons] == [True, False]
                           and all(e.get("key") is None and (e.get("button") in (None, 1)) for e in segment)
                           and buttons[0].get("observed_hover", {}).get(action["key"]) == "Pressed"
                           and buttons[0].get("observed_hover_entities", {}).get(action["key"]) == control["entity"]
                           and moves and any(e.get("observed_hover_entities", {}).get(action["key"]) == control["entity"]
                                             and inside(e.get("observed_cursor")) for e in moves)
                           and all(inside(e.get("observed_cursor")) for e in buttons),
                           "pin click does not match its visible control")
    right = [event for event in events if event.get("button") == 3]
    native.require(len(right) == 2 and [event.get("pressed") for event in right] == [True, False]
                   and [event.get("observed_right_pressed") for event in right] == [True, False]
                   and int(entries[0]["last_step"]) < int(right[0]["step"]) < int(right[1]["step"]) <= int(entries[1]["last_step"])
                   and all(event.get("nonce") == first_value.get("nonce") and event.get("pid") == client.get("pid")
                           and event.get("window") == client.get("window") and event.get("world_epoch") == first_value["world_epoch"]
                           and entries[0]["value"]["frame"] <= event.get("pre_frame", -1) < event.get("post_frame", -1)
                           <= entries[1]["value"]["frame"] and isinstance(event.get("observed_cursor"), list)
                           and len(event["observed_cursor"]) == 2 and isinstance(a.get("world_point"), list)
                           and len(a["world_point"]) == 2 and all(type(n) in (int, float) and math.isfinite(n)
                               for n in event["observed_cursor"] + a["world_point"])
                           and all(abs(x - y) <= 2 for x, y in zip(event["observed_cursor"], a["world_point"]))
                           for event in right) and right[0]["post_frame"] <= right[1]["pre_frame"],
                   "pin context lacks exact acknowledged right press/release")
    claimed.update(int(event["step"]) for event in right)
    native.require(all(int(event["step"]) in claimed for event in events
                       if int(event["step"]) > entries[0]["last_step"]
                       and (event.get("button") is not None or event.get("key") is not None)),
                   "unclaimed pin button or key input")


def exercise_pin(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
        driver.last = driver.wait(lambda v: "text-field:DevPoc" not in v["controls"], after=driver.last["frame"])
    driver.click("menu:Workspace(OpenEntities)")
    a, b = pin_souls(driver.last)
    driver.click(f"soul:{a['entity']}")
    driver.capture("pin-initial")
    point = a["world_point"]
    native.require(point is not None, "pin Soul is outside camera")
    driver.send(point=tuple(round(n) for n in point))
    native.require(not driver.last["pointer_over_ui"] and driver.last["refactor_rows"]["hovered"] == a["entity"],
                   "pin context does not target the Soul")
    driver.send(button=3, pressed=True)
    driver.send(button=3, pressed=False)
    driver.last = driver.wait(lambda v: any(k.startswith("menu:InspectEntity(") for k in v["controls"]), after=driver.last["frame"])
    driver.capture("pin-context")
    key = next(k for k in driver.last["controls"] if k.startswith("menu:InspectEntity("))
    pin_click(driver, key)
    driver.last = driver.wait(lambda v: v.get("pinned") == a["entity"] and v.get("inspection_target") == a["entity"], after=driver.last["frame"])
    driver.capture("pin-a")
    pin_click(driver, "menu:Workspace(OpenEntities)")
    pin_click(driver, f"soul:{b['entity']}")
    driver.last = driver.wait(lambda v: v.get("selected") == b["entity"] and v.get("pinned") == a["entity"] and v.get("inspection_target") == a["entity"], after=driver.last["frame"])
    driver.capture("pin-selected-b")
    pin_click(driver, "soul-rename")
    driver.last = driver.wait(lambda v: v.get("rename_target") == a["entity"], after=driver.last["frame"])
    pin_keys(driver, list("draft"))
    driver.capture("pin-rename-active")
    pin_click(driver, "menu:Workspace(InspectSelection)")
    driver.last = driver.wait(lambda v: v.get("rename_target") is None and v.get("inspection_target") == b["entity"], after=driver.last["frame"])
    driver.capture("pin-edit-abandoned")
    pin_click(driver, "menu:Workspace(Back)")
    driver.last = driver.wait(lambda v: v.get("inspection_target") == a["entity"], after=driver.last["frame"])
    pin_click(driver, "soul-rename")
    driver.last = driver.wait(lambda v: v.get("rename_target") == a["entity"], after=driver.last["frame"])
    pin_keys(driver, list("renamed"))
    pin_keys(driver, ["Return"])
    driver.last = driver.wait(lambda v: v.get("rename_target") is None and pin_souls(v)[0]["name"] == "renamed", after=driver.last["frame"])
    driver.capture("pin-rename-applied")
    pin_click(driver, "menu:ClearInspectPin")
    driver.last = driver.wait(lambda v: v.get("pinned") is None and v.get("inspection_target") == b["entity"], after=driver.last["frame"])
    driver.capture("pin-cleared")
