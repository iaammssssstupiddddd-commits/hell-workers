"""R12 independent bar ownership/geometry and real input transition checks."""
import math
import native_acceptance as native

CHECKPOINTS = ("bars-initial", "bars-saved", "bars-completed", "bars-loaded", "bars-cancelled")
INITIAL_RATIOS = {"soul": 0.25, "blueprint": 0.7, "floor": 0.25, "wall": 0.5}


def check_observation(case, value):
    evidence = value["progress_bars"]
    owners, bars = evidence["owners"], evidence["bars"]
    expected = (set(INITIAL_RATIOS) if case in ("bars-initial", "bars-saved") else
                {"blueprint", "wall"} if case == "bars-completed" else
                {"blueprint", "floor", "wall"} if case == "bars-loaded" else set())
    native.require({bar["kind"] for bar in bars} == expected, "progress caller coverage differs")
    native.require(len({bar["entity"] for bar in bars}) == len(bars), "duplicate bar entity")
    for kind in expected:
        pair = [bar for bar in bars if bar["kind"] == kind]
        native.require(len(pair) == 2 and sum(bar["background"] for bar in pair) == 1
                       and sum(bar["fill"] for bar in pair) == 1, "progress pair is missing or duplicated")
        native.require(kind in owners and all(bar["owner_key"] == kind and bar["owner"] == owners[kind]["entity"] for bar in pair), "orphan or wrong progress owner")
        background = next(bar for bar in pair if bar["background"])
        fill = next(bar for bar in pair if bar["fill"])
        native.require(abs(background["size"][0] - background["width"]) < 0.01, "background width differs")
        ratio = fill["size"][0] / fill["width"]
        native.require(math.isfinite(ratio) and 0 < ratio < 1, "expected an intermediate progress ratio")
        native.require(abs(ratio - INITIAL_RATIOS[kind]) < 0.015, "progress ratio differs")
        native.require(abs((fill["local"][0] - fill["size"][0] / 2) + background["width"] / 2) < 0.01, "fill left edge differs")
        matrix = owners[kind].get("matrix")
        native.require(isinstance(matrix, list) and len(matrix) == 16
                       and all(isinstance(item, (int, float)) and math.isfinite(item) for item in matrix),
                       "progress owner matrix is invalid")
        for bar in pair:
            native.require(bar["visible"] and bar["point"] and 0 < bar["point"][0] < value["viewport"][0]
                           and 0 < bar["point"][1] < value["viewport"][1], "progress bar is hidden or outside viewport")
            native.require(all(isinstance(bar.get(field), list) and len(bar[field]) == 3
                               and all(isinstance(item, (int, float)) and math.isfinite(item) for item in bar[field])
                               for field in ("local", "global")), "progress coordinates are invalid")
            expected_position = [sum(matrix[column * 4 + axis] * bar["local"][column] for column in range(3))
                                 + matrix[12 + axis] for axis in range(3)]
            native.require(all(abs(bar["global"][axis] - expected_position[axis]) < 0.02 for axis in range(3)),
                           "progress bar does not follow its owner")
            native.require(bar["color"] and bar["color"][3] > 0, "progress color is transparent")
    if case in ("bars-completed", "bars-cancelled"):
        native.require(evidence["floor_complete"], "bar disappearance is not backed by simulation completion")
        native.require("floor" not in owners, "completed floor owner remains")
    if case == "bars-completed":
        native.require(not evidence["source_present"], "Soul bar disappeared before gathering completed")
    if case == "bars-loaded":
        native.require(evidence["source_present"] and not evidence["floor_complete"], "loaded fixture was not restored")
        native.require(owners["soul"]["task"] == "None", "load retained a runtime-only assignment")
    if case == "bars-cancelled":
        native.require("blueprint" not in owners and "wall" not in owners, "cancelled owners remain")


def check_sequence(entries):
    native.require([item["case"] for item in entries] == list(CHECKPOINTS), "progress checkpoints differ")
    for before, after in zip(entries, entries[1:]):
        native.require(after["last_step"] > before["last_step"] and after["value"]["frame"] > before["value"]["frame"], "progress transition lacks fresh input evidence")
    for entry in entries:
        check_observation(entry["case"], entry["value"])
    initial, saved, completed, loaded, cancelled = [entry["value"] for entry in entries]
    native.require("Save:Succeeded" in saved["save_outcomes"], "save success is missing")
    native.require("Load:Succeeded" in loaded["save_outcomes"], "load success is missing")
    native.require(initial["world_epoch"] == saved["world_epoch"] == completed["world_epoch"]
                   and loaded["world_epoch"] > completed["world_epoch"]
                   and cancelled["world_epoch"] == loaded["world_epoch"], "load epoch transition differs")
    old = {bar["entity"] for bar in completed["progress_bars"]["bars"]}
    new = {bar["entity"] for bar in loaded["progress_bars"]["bars"]}
    native.require(old.isdisjoint(new), "load retained old progress entities")
    targets = {loaded["progress_bars"]["owners"]["blueprint"]["entity"], loaded["progress_bars"]["wall_target"]}
    accepted = {outcome["entity"] for outcome in cancelled["task_outcomes"]
                if outcome["action"] == "Cancel" and outcome["result"] == "CancellationRequested"
                and outcome["epoch"] == loaded["world_epoch"]
                and loaded["frame"] < outcome["frame"] <= cancelled["frame"]}
    native.require(targets <= accepted, "bar disappearance lacks accepted cancellation receipts")


def catalog_control(driver, action):
    return next(key for key in driver.last["controls"] if key.startswith(f"menu:{action}") and "Manual1" in key)


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    driver.capture("bars-initial")
    driver.key("F5")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectSaveCatalogSlot") for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda value: value["catalog"] == "Closed" and value["save_outcomes"], after=driver.last["frame"])
    driver.capture("bars-saved")
    driver.key("space")
    driver.last = driver.wait(lambda value: value["progress_bars"]["floor_complete"]
                              and not value["progress_bars"]["source_present"]
                              and {bar["kind"] for bar in value["progress_bars"]["bars"]} == {"blueprint", "wall"},
                              after=driver.last["frame"], timeout=30)
    driver.key("space")
    driver.capture("bars-completed")
    driver.key("F9")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectLoadCatalogSlot") for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    driver.click(catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda value: value["world_epoch"] > epoch and value["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    driver.capture("bars-loaded")
    driver.key("space")
    driver.click("menu:Workspace(OpenEntities)")
    driver.click("tab:TaskList")
    for kind in ("blueprint", "wall"):
        evidence = driver.last["progress_bars"]
        target = evidence["owners"][kind]["entity"] if kind == "blueprint" else evidence["wall_target"]
        driver.click(f"task:{target}")
        driver.last = driver.wait(lambda value: any(key.startswith(f"task-action:{target}:Cancel(") for key in value["controls"]), after=driver.last["frame"])
        action = next(key for key in driver.last["controls"] if key.startswith(f"task-action:{target}:Cancel("))
        driver.click(action)
        driver.click(action)
        driver.last = driver.wait(lambda value: kind not in value["progress_bars"]["owners"], after=driver.last["frame"])
    driver.last = driver.wait(lambda value: not value["progress_bars"]["bars"] and value["progress_bars"]["floor_complete"], after=driver.last["frame"], timeout=30)
    driver.capture("bars-cancelled")
