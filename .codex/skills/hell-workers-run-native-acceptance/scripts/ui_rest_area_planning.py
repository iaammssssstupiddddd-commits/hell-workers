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


def check_cancellation(raw, entries, client):
    """Bind the opt-in owner/refund observation, not a full lifecycle verdict."""
    native.require(raw.get("scope") == "production-raw-observations-v1"
                   and raw.get("schema_version") == 2 and raw.get("failure") is None
                   and raw.get("nonce") == client["nonce"]
                   and raw.get("process_id") == client["pid"]
                   and raw.get("accepted") is False and raw.get("performance_evidence") is False
                   and raw.get("promotion_authority") is False, "cancellation raw context/authority differs")
    loaded = entries[3]["value"]
    owner = loaded["rest_area_planning"]["blueprints"][0]["entity"]
    rows = [event for event in raw.get("events", []) if event.get("operation") == "BlueprintCancellationApplied"]
    native.require(len(rows) == 1, "cancellation applied observation missing or duplicated")
    row = rows[0]
    native.require(row.get("producer") == "blueprint_cancellation_system"
                   and row.get("result") == "OwnerCleanupApplied"
                   and row.get("kind") == "RestArea" and type(row.get("owner_bits")) is int
                   and row.get("owner_bits") == owner and type(row.get("world_epoch")) is int
                   and row.get("world_epoch") == loaded["world_epoch"]
                   and row.get("footprint") == loaded["rest_area_planning"]["footprint"]
                   and row.get("footprint_walkability") == [True] * 4
                   and all(type(value) is bool for value in row["footprint_walkability"]),
                   "cancellation owner/footprint differs")
    native.require(row.get("refund_items") == [] and row.get("delivered_materials") == [],
                   "zero-material planning case cannot prove a partial-material refund")
    samples = raw.get("samples", [])
    index = row.get("sample_index")
    native.require(type(index) is int and 0 <= index < len(samples), "cancellation sample index missing")
    sample = samples[index]
    native.require(sample.get("world_epoch") == row["world_epoch"]
                   and isinstance(sample.get("blueprints"), list)
                   and not any(blueprint.get("entity") == row.get("owner") for blueprint in sample["blueprints"]),
                   "cancellation sample still contains the owner or has a stale epoch")
    before = [blueprint for value in samples[:index] if value.get("world_epoch") == row["world_epoch"]
              for blueprint in value.get("blueprints", []) if blueprint.get("entity") == row.get("owner")
              and blueprint.get("entity_bits") == owner and blueprint.get("kind") == "RestArea"
              and blueprint.get("footprint") == row["footprint"] and blueprint.get("progress") == 0
              and blueprint.get("delivered_materials") == {}]
    native.require(before, "cancellation lacks the same real pre-cancel blueprint")
    layers = [layer for layer in sample.get("owner_layers", []) if layer.get("entity") == row.get("owner")]
    native.require(len(layers) == 1 and layers[0].get("exists") is False
                   and all(layers[0].get(key) == [] for key in (
                       "building_grids", "floor_grids", "door_grids", "bridge_grids", "stockpile_grids")),
                   "cancellation owner/map tombstone differs")
    for field in ("released_actors", "closed_requests", "removed_storages", "released_items"):
        values = row.get(field)
        native.require(isinstance(values, list) and all(isinstance(value, str) and value for value in values)
                       and len(values) == len(set(values)), "cancellation identities malformed")
