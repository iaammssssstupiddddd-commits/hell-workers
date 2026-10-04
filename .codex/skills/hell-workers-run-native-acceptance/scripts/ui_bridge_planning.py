"""OS-driven Bridge planning only, with passive generated-world observations.

Does not prove completed construction, passage, salvage, art or performance.
"""
import native_acceptance as native
from ui_progress_bars import catalog_control

CHECKPOINTS = ("bridge-empty", "bridge-placed", "bridge-saved", "bridge-loaded", "bridge-cancelled")


def check_observation(case, value):
    evidence = value["bridge_planning"]
    footprint = evidence["footprint"]
    native.require(len(footprint) == 10 and len({tuple(grid) for grid in footprint}) == 10,
                   "Bridge footprint is not ten distinct cells")
    native.require(len(evidence["banks"]) == 4 and len(evidence["cells"]) == 10,
                   "Bridge banks/cells are missing")
    native.require([cell["grid"] for cell in evidence["cells"]] == footprint,
                   "Bridge cell order differs from footprint")
    native.require(not evidence["buildings"] and all(not cell["bridged"] for cell in evidence["cells"]),
                   "planning-only evidence contains completed Bridge")
    if case in ("bridge-empty", "bridge-cancelled"):
        native.require(evidence["legal"] and not evidence["blueprints"]
                       and all(cell["owner"] is None for cell in evidence["cells"]),
                       "empty/cancelled crossing remains reserved")
    else:
        native.require(not evidence["legal"] and len(evidence["blueprints"]) == 1,
                       "placed crossing is not reserved exactly once")
        bp = evidence["blueprints"][0]
        native.require(bp["footprint"] == footprint and bp["progress"] == 0
                       and not bp["materials_complete"], "ordinary unbuilt Bridge state differs")
        native.require(all(cell["owner"] == bp["entity"] for cell in evidence["cells"]),
                       "Bridge reservation ownership differs")


def check_sequence(entries):
    native.require([entry["case"] for entry in entries] == list(CHECKPOINTS), "Bridge checkpoints differ")
    for entry in entries:
        check_observation(entry["case"], entry["value"])
    for before, after in zip(entries, entries[1:]):
        native.require(after["last_step"] > before["last_step"]
                       and after["value"]["frame"] > before["value"]["frame"],
                       "Bridge transition has no fresh input")
    empty, placed, saved, loaded, cancelled = [entry["value"] for entry in entries]
    target = empty["bridge_planning"]
    native.require(all(value["bridge_planning"]["footprint"] == target["footprint"]
                       and value["bridge_planning"]["banks"] == target["banks"] for value in (placed, saved, loaded, cancelled)),
                   "Bridge subject footprint changed")
    native.require(cancelled["bridge_planning"]["cells"] == target["cells"],
                   "cancellation failed to restore original terrain/walkability")
    native.require("Save:Succeeded" in saved["save_outcomes"] and "Load:Succeeded" in loaded["save_outcomes"],
                   "Bridge save/load result is missing")
    native.require(empty["world_epoch"] == placed["world_epoch"] == saved["world_epoch"]
                   and loaded["world_epoch"] > saved["world_epoch"]
                   and cancelled["world_epoch"] == loaded["world_epoch"], "Bridge epoch transition differs")
    old = placed["bridge_planning"]["blueprints"][0]["entity"]
    new = loaded["bridge_planning"]["blueprints"][0]["entity"]
    native.require(old != new, "load reused old Bridge entity")
    native.require(any(outcome["entity"] == new and outcome["action"] == "Cancel"
                       and outcome["result"] == "CancellationRequested"
                       and outcome["epoch"] == loaded["world_epoch"]
                       and loaded["frame"] < outcome["frame"] <= cancelled["frame"]
                       for outcome in cancelled["task_outcomes"]), "Bridge lacks cancellation receipt")


def set_paused(driver, paused):
    """Send one ordinary toggle, then await its authoritative state transition."""
    if driver.last["paused"] == paused:
        return
    before = driver.last["frame"]
    driver.key("space")
    driver.last = driver.wait(lambda value: value["paused"] == paused, after=before)


def exercise(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
    driver.capture("bridge-empty")
    set_paused(driver, False)  # Ordinary building commands are disabled while paused.
    native.require(not driver.last["paused"], "Bridge input requires running simulation")
    driver.click("menu:ToggleArchitect")
    driver.click("menu:SelectBuild(Bridge)")
    point = driver.last["bridge_planning"]["point"]
    native.require(point is not None and all(0 < axis < limit for axis, limit in zip(point, driver.last["viewport"])),
                   "Bridge target is outside owned window")
    driver.send(point=tuple(round(axis) for axis in point))
    native.require(not driver.last["pointer_over_ui"], "Bridge point is covered by UI")
    driver.send(button=1, pressed=True)
    driver.send(button=1, pressed=False)
    driver.last = driver.wait(lambda value: len(value["bridge_planning"]["blueprints"]) == 1,
                              after=driver.last["frame"])
    driver.key("Escape")
    set_paused(driver, True)
    driver.capture("bridge-placed")
    driver.key("F5")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectSaveCatalogSlot") for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda value: value["catalog"] == "Closed" and "Save:Succeeded" in value["save_outcomes"], after=driver.last["frame"])
    driver.capture("bridge-saved")
    driver.key("F9")
    driver.last = driver.wait(lambda value: any(key.startswith("menu:SelectLoadCatalogSlot") for key in value["controls"]), after=driver.last["frame"])
    driver.click(catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    driver.click(catalog_control(driver, "ConfirmLoadCatalogSlot"))
    driver.last = driver.wait(lambda value: value["world_epoch"] > epoch and value["catalog"] == "Closed", after=driver.last["frame"], timeout=60)
    set_paused(driver, True)  # Pause before material delivery, never seed progress.
    driver.capture("bridge-loaded")
    set_paused(driver, False)  # Task cancellation follows the normal running guard.
    driver.click("menu:Workspace(OpenEntities)")
    driver.click("tab:TaskList")
    target = driver.last["bridge_planning"]["blueprints"][0]["entity"]
    driver.click(f"task:{target}")
    driver.last = driver.wait(lambda value: any(key.startswith(f"task-action:{target}:Cancel(") for key in value["controls"]), after=driver.last["frame"])
    action = next(key for key in driver.last["controls"] if key.startswith(f"task-action:{target}:Cancel("))
    driver.click(action)
    driver.click(action)
    driver.last = driver.wait(lambda value: not value["bridge_planning"]["blueprints"], after=driver.last["frame"])
    driver.capture("bridge-cancelled")
