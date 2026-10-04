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
