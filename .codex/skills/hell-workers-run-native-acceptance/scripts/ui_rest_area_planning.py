"""Ordinary generated-world RestArea reservation input, not completed art acceptance.

The Rust observer changes only the camera and pause state before input. The
driver uses the player's existing build, save/load, and cancel controls; it
does not grant materials, finish construction, or seed occupants/Dream.
"""
import native_acceptance as native
from ui_bridge_planning import set_paused
from ui_progress_bars import catalog_control

CHECKPOINTS = ("rest-area-empty", "rest-area-placed", "rest-area-saved",
               "rest-area-loaded", "rest-area-cancelled")


def check_observation(case, value):
    evidence = value["rest_area_planning"]
    footprint = evidence["footprint"]
    anchor = evidence["anchor"]
    native.require(isinstance(anchor, list) and len(anchor) == 2
                   and all(type(axis) is int for axis in anchor), "RestArea anchor is invalid")
    x, y = anchor
    expected = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]]
    native.require(footprint == expected
                   and len(evidence["cells"]) == 4
                   and [cell["grid"] for cell in evidence["cells"]] == footprint,
                   "RestArea footprint/cell order differs")
    native.require(not evidence["buildings"], "planning evidence contains a completed RestArea")
    if case in ("rest-area-empty", "rest-area-cancelled"):
        native.require(evidence["legal"] and not evidence["blueprints"]
                       and all(cell["owner"] is None and cell["walkable"] for cell in evidence["cells"]),
                       "empty/cancelled RestArea yard position differs")
    else:
        native.require(not evidence["legal"] and len(evidence["blueprints"]) == 1,
                       "RestArea reservation is not unique")
        blueprint = evidence["blueprints"][0]
        native.require(blueprint["footprint"] == footprint and blueprint["progress"] == 0
                       and not blueprint["materials_complete"]
                       and all(cell["owner"] == blueprint["entity"] for cell in evidence["cells"]),
                       "ordinary RestArea blueprint differs")


def check_sequence(entries):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS),
                   "RestArea planning checkpoints differ")
    for entry in entries:
        check_observation(entry["case"], entry["value"])
    for before, after in zip(entries, entries[1:]):
        native.require(after["last_step"] > before["last_step"]
                       and after["value"]["frame"] > before["value"]["frame"],
                       "RestArea transition has no fresh input")
    empty, placed, saved, loaded, cancelled = [entry["value"] for entry in entries]
    target = empty["rest_area_planning"]
    native.require(all(value["rest_area_planning"]["anchor"] == target["anchor"]
                       and value["rest_area_planning"]["footprint"] == target["footprint"]
                       and value["rest_area_planning"]["draw_pos"] == target["draw_pos"]
                       for value in (placed, saved, loaded, cancelled)),
                   "RestArea yard position identity changed")
    native.require(cancelled["rest_area_planning"]["cells"] == target["cells"],
                   "cancellation failed to restore site footprint")
    native.require("Save:Succeeded" in saved["save_outcomes"]
                   and "Load:Succeeded" in loaded["save_outcomes"],
                   "RestArea save/load result is missing")
    native.require(empty["world_epoch"] == placed["world_epoch"] == saved["world_epoch"]
                   and loaded["world_epoch"] > saved["world_epoch"]
                   and cancelled["world_epoch"] == loaded["world_epoch"],
                   "RestArea epoch transition differs")
    old = placed["rest_area_planning"]["blueprints"][0]["entity"]
    native.require(saved["rest_area_planning"]["blueprints"][0]["entity"] == old,
                   "save changed the RestArea reservation identity")
    new = loaded["rest_area_planning"]["blueprints"][0]["entity"]
    native.require(old != new, "load reused the old RestArea entity")
    native.require(any(outcome["entity"] == new and outcome["action"] == "Cancel"
                       and outcome["result"] == "CancellationRequested"
                       and outcome["epoch"] == loaded["world_epoch"]
                       and loaded["frame"] < outcome["frame"] <= cancelled["frame"]
                       for outcome in cancelled["task_outcomes"]),
                   "RestArea cancellation receipt is missing")


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    driver.capture("rest-area-empty")
    set_paused(driver, False)
    native.require(not driver.last["paused"], "RestArea input requires running simulation")
    driver.click("menu:ToggleArchitect")
    driver.click("menu:SelectBuild(RestArea)")
    point = driver.last["rest_area_planning"]["point"]
    native.require(point is not None and all(0 < axis < limit for axis, limit in zip(point, driver.last["viewport"])),
                   "RestArea target is outside the owned window")
    driver.send(point=tuple(round(axis) for axis in point))
    native.require(not driver.last["pointer_over_ui"], "RestArea yard position is covered by UI")
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    driver.last = driver.wait(lambda value: len(value["rest_area_planning"]["blueprints"]) == 1,
                              after=driver.last["frame"])
    driver.key("Escape")
    set_paused(driver, True)
    driver.capture("rest-area-placed")
    driver.key("F5")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectSaveCatalogSlot")
                                              for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda value: value["catalog"] == "Closed"
                              and "Save:Succeeded" in value["save_outcomes"], after=driver.last["frame"])
    driver.capture("rest-area-saved")
    driver.key("F9")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectLoadCatalogSlot")
                                              for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    driver.click(catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda value: value["world_epoch"] > epoch and value["catalog"] == "Closed",
                              after=driver.last["frame"], timeout=60)
    set_paused(driver, True)
    driver.capture("rest-area-loaded")
    set_paused(driver, False)
    driver.click("menu:Workspace(OpenEntities)")
    driver.click("tab:TaskList")
    target = driver.last["rest_area_planning"]["blueprints"][0]["entity"]
    driver.click(f"task:{target}")
    driver.last = driver.wait(lambda value: any(key.startswith(f"task-action:{target}:Cancel(")
                                              for key in value["controls"]), after=driver.last["frame"])
    action = next(key for key in driver.last["controls"] if key.startswith(f"task-action:{target}:Cancel("))
    driver.click(action)
    driver.click(action)
    driver.last = driver.wait(lambda value: not value["rest_area_planning"]["blueprints"],
                              after=driver.last["frame"])
    driver.capture("rest-area-cancelled")
