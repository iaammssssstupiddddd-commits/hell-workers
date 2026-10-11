"""Ordinary Manual1 save/load of an actual Tank planning commit, not acceptance."""
from __future__ import annotations

import hashlib
import math
import os
from pathlib import Path
import stat

from . import building_production_companion_probe as companion
from . import building_production_lifecycle as lifecycle
from . import building_production_selection_probe as selection

require = selection.require
STEPS = [("save-open-press", "F5", True), ("save-open-release", "F5", False),
         ("save-motion", None, None), ("save-press", None, True), ("save-release", None, False),
         ("load-open-press", "F9", True), ("load-open-release", "F9", False),
         ("load-motion", None, None), ("load-press", None, True), ("load-release", None, False),
         ("confirmload-motion", None, None), ("confirmload-press", None, True), ("confirmload-release", None, False)]


def context(sample):
    value = sample.get("save_catalog_context")
    require(isinstance(value, dict) and set(value) == {"available", "mode", "session", "storage_root", "manual1", "controls"}
            and value["available"] is True and value["storage_root"] == "saves"
            and type(value["session"]) is int and value["session"] > 0
            and isinstance(value["controls"], list) and len(value["controls"]) <= 3,
            "actual isolated save catalog context missing")
    require(sample["dpi"] == 1 and sample["play_mode"] == "Normal"
            and sample["task_mode"] == "None" and sample["selected_building"] is None,
            "save trial requires ordinary closed placement")
    return value


def control(sample, action):
    value = context(sample)
    require(value["mode"] == {"Save": "SaveCatalog", "Load": "LoadCatalog", "ConfirmLoad": "LoadConfirmManual1"}[action],
            "save action/mode mismatch")
    entry = value["manual1"]
    require(isinstance(entry, dict) and set(entry) == {"exists", "length", "can_save", "can_load"}
            and (entry["exists"] is False and entry["can_save"] is True and entry["length"] == 0
                 if action == "Save" else entry["exists"] is True and entry["can_load"] is True and entry["length"] > 0),
            "Manual1 is not a fresh save or loadable slot")
    require(len(value["controls"]) == 1, "exactly one visible Manual1 action required")
    row = value["controls"][0]
    require(set(row) == {"action", "session", "slot", "entity", "camera", "window", "physical_rect", "physical_viewport"}
            and row["action"] == action and row["slot"] == "Manual1" and row["session"] == value["session"]
            and row["window"] == sample["primary_window"] and row["physical_viewport"] == [1280, 720]
            and all(isinstance(row[k], str) and row[k] for k in ("entity", "camera", "window")),
            "save action ownership differs")
    rect = row["physical_rect"]
    require(isinstance(rect, list) and len(rect) == 4 and all(type(v) in (int, float) and math.isfinite(v) for v in rect),
            "save control rectangle invalid")
    x, y, right, bottom = rect
    point = [int((x + right) / 2), int((y + bottom) / 2)]
    require(0 <= x < point[0] < right <= 1280 and 0 <= y < point[1] < bottom <= 720,
            "save control center outside actual viewport")
    return row, point


def file_identity(root: Path):
    """Hash only the real, bounded job-local canonical file. Never write it."""
    runtime = root / "runtime"
    directory = runtime / "saves"
    path = directory / "manual-1.scn.ron"
    require(all(p.is_dir() and not p.is_symlink() for p in (root, runtime, directory))
            and path.resolve().parent == directory.resolve(), "save file escapes isolated runtime")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and 0 < before.st_size <= 64 * 1024 * 1024,
                "save file is not a bounded unique regular file")
        with os.fdopen(os.dup(fd), "rb") as stream:
            data = stream.read(64 * 1024 * 1024 + 1)
        after = os.fstat(fd)
        require(before == after and len(data) == before.st_size and path.stat() == after,
                "save file changed during hashing")
        return {"path": "runtime/saves/manual-1.scn.ron", "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    finally:
        os.close(fd)


def planning(sample):
    blueprints = sample["blueprints"]
    require(len(blueprints) == 1 and blueprints[0]["kind"] == "Tank", "one real Tank Blueprint required")
    bp = blueprints[0]
    owner = bp["entity"]
    layers = [v for v in sample["owner_layers"] if v["entity"] == owner]
    children = [v for v in companion.context(sample)["storages"] if v["pending_owner"] == owner]
    require(companion.context(sample)["pending"] is None and len(layers) == 1 and layers[0]["exists"] is True
            and sorted(layers[0]["building_grids"]) == sorted(bp["footprint"])
            and len(children) == len(companion.context(sample)["storages"]) == 2
            and len({v["entity"] for v in children}) == 2
            and all(v["owner"] is None and v["stockpile_grids"] == [v["grid"]] for v in children),
            "actual saved parent/child ownership missing")
    semantic = {"blueprint": {k: v for k, v in bp.items() if k != "entity"},
                "children": sorted([{k: v for k, v in c.items() if k not in {"entity", "pending_owner"}}
                                    for c in children], key=lambda v: v["grid"])}
    return owner, {c["entity"] for c in children}, semantic


def click(transport, read, action, nonce):
    raw = read()
    before = len(raw["samples"]) - 1
    initial = raw["samples"][-1]
    row, point = control(initial, action)
    prefix = action.lower()
    transport.send(prefix + "-motion", nonce, point=point)
    raw = selection.wait(read, lambda v: len(v["samples"]) - 1 > before and v["samples"][-1].get("cursor_position") == point, timeout=2)
    pointer, boundary = len(raw["samples"]) - 1, len(raw["events"])
    require(raw["nonce"] == nonce, "save click nonce changed")
    for sample in raw["samples"][before:pointer + 1]:
        require(sample["world_epoch"] == initial["world_epoch"] and control(sample, action) == (row, point)
                and context(sample) == context(initial), "save control changed before original press")
    transport.send(prefix + "-press", nonce, button=1, pressed=True)
    transport.send(prefix + "-release", nonce, button=1, pressed=False)
    raw = selection.wait(read, lambda v: any(e.get("producer") == "MouseButtonInput" and e.get("state") == "Released"
                                           for e in v["events"][boundary:]))
    return {"before": before, "pointer": pointer, "boundary": boundary, "point": point}


def run(transport, read, root, capture):
    raw = read()
    nonce, before = raw["nonce"], len(raw["samples"]) - 1
    initial = raw["samples"][-1]
    require(initial["save_catalog_context"]["mode"] == "Closed"
            and not (root / "runtime/saves/manual-1.scn.ron").exists(), "fresh save trial must not overwrite")
    planning(initial)
    def open_catalog(prefix, key, mode):
        boundary = len(read()["events"])
        transport.send(prefix + "-open-press", nonce, key=key, pressed=True)
        transport.send(prefix + "-open-release", nonce, key=key, pressed=False)
        selection.wait(read, lambda v: v["samples"][-1].get("save_catalog_context", {}).get("mode") == mode
                       and any(e.get("producer") == "KeyboardInput" and e.get("key") == key and e.get("state") == "Released"
                               for e in v["events"][boundary:]))
    open_catalog("save", "F5", "SaveCatalog")
    saved = click(transport, read, "Save", nonce)
    raw = selection.wait(read, lambda v: any(e.get("producer") == "SaveLoadOutcome" for e in v["events"][saved["boundary"]:])
                         and v["samples"][-1]["save_catalog_context"]["mode"] == "Closed")
    saved["after"] = len(raw["samples"]) - 1
    identity = file_identity(root)
    capture(root / "saved.png")
    open_catalog("load", "F9", "LoadCatalog")
    loaded = click(transport, read, "Load", nonce)
    selection.wait(read, lambda v: v["samples"][-1]["save_catalog_context"]["mode"] == "LoadConfirmManual1")
    confirmed = click(transport, read, "ConfirmLoad", nonce)
    raw = selection.wait(read, lambda v: any(e.get("producer") == "SaveLoadOutcome" and e.get("operation") == "Load"
                         for e in v["events"][confirmed["boundary"]:]) and v["samples"][-1]["world_epoch"] != initial["world_epoch"])
    confirmed["after"] = len(raw["samples"]) - 1
    capture(root / "loaded.png")
    evidence = {"before": before, "save": saved, "load": loaded, "confirm": confirmed, "file": identity}
    verify(raw, evidence, root)
    return evidence


def verify(raw, evidence, root):
    lifecycle.check_raw_transitions(raw)
    require(set(evidence) == {"before", "save", "load", "confirm", "file"}, "save evidence shape differs")
    require(type(evidence["before"]) is int and 0 <= evidence["before"] < len(raw["samples"]), "save start ordinal differs")
    save, load, confirm = (evidence[k] for k in ("save", "load", "confirm"))
    require(set(save) == set(confirm) == {"before", "pointer", "boundary", "point", "after"}
            and set(load) == {"before", "pointer", "boundary", "point"}
            and all(type(leg[k]) is int for leg in (save, load, confirm) for k in ("before", "pointer", "boundary"))
            and all(type(leg["after"]) is int for leg in (save, confirm))
            and evidence["before"] <= save["before"] < save["pointer"] < save["after"] < load["before"]
            < load["pointer"] < confirm["before"] < confirm["pointer"] < confirm["after"] < len(raw["samples"]),
            "save/load ordered intervals differ")
    start = raw["samples"][evidence["before"]]
    saved = raw["samples"][evidence["save"]["after"]]
    final = raw["samples"][evidence["confirm"]["after"]]
    old_owner, old_children, old_semantic = planning(start)
    new_owner, new_children, new_semantic = planning(final)
    retired = {old_owner, *old_children}
    require(not any(row["entity"] in retired for category in ("owners", "blueprints", "roots") for row in final[category])
            and not any(row.get("owner") in retired for row in final["roots"]), "old planning owner/visual survived load")
    for row in final["owner_layers"]:
        if row["entity"] in retired:
            require(row["exists"] is False and all(row.get(key) == [] for key in
                    ("building_grids", "floor_grids", "door_grids", "bridge_grids", "stockpile_grids")),
                    "old planning entity/map reservation survived load")
    require(planning(saved) == planning(start) and old_semantic == new_semantic
            and new_owner != old_owner and old_children.isdisjoint(new_children)
            and final["world_epoch"] == start["world_epoch"] + 1
            and final["primary_window"] == start["primary_window"]
            and final["save_catalog_context"]["mode"] == "Closed", "loaded actual planning identities/state differ")
    for action, leg in (("Save", evidence["save"]), ("Load", evidence["load"]), ("ConfirmLoad", evidence["confirm"])):
        before, pointer, boundary = (leg[k] for k in ("before", "pointer", "boundary"))
        require(all(type(i) is int for i in (before, pointer, boundary)) and before < pointer < len(raw["samples"])
                and 0 <= boundary < len(raw["events"]), "save click ordinals differ")
        row, point = control(raw["samples"][before], action)
        require(leg["point"] == point and all(control(s, action) == (row, point)
                and s["primary_window"] == start["primary_window"] and s["world_epoch"] == start["world_epoch"]
                for s in raw["samples"][before:pointer + 1]), "save click control changed")
        end = evidence["confirm"]["after"] if action == "ConfirmLoad" else evidence["load"]["before"] if action == "Save" else evidence["confirm"]["before"]
        events = [e for e in raw["events"][boundary:] if e["sample_index"] <= end and e.get("producer") == "MouseButtonInput"]
        require(len(events) == 2 and [e["state"] for e in events] == ["Pressed", "Released"]
                and all(e["window"] == start["primary_window"] and e["button"] == "Left" and pointer < e["sample_index"] for e in events),
                "original save click receipts differ")
        require(all(s.get("cursor_position") == point for s in raw["samples"][pointer:events[-1]["sample_index"] + 1]),
                "save pointer moved before original release")
    for key, begin, end in (("F5", evidence["before"], save["before"]), ("F9", save["after"], load["before"])):
        events = [e for e in raw["events"] if begin < e["sample_index"] <= end and e.get("producer") == "KeyboardInput"]
        require(len(events) == 2 and [e["state"] for e in events] == ["Pressed", "Released"]
                and all(e["key"] == key and e["window"] == start["primary_window"] and e["repeat"] is False for e in events),
                "original save catalog keyboard receipts differ")
    outcomes = [e for e in raw["events"] if e.get("producer") == "SaveLoadOutcome"
                and evidence["before"] < e["sample_index"] <= evidence["confirm"]["after"]]
    save_session = context(raw["samples"][evidence["save"]["before"]])["session"]
    load_session = context(raw["samples"][evidence["confirm"]["before"]])["session"]
    require(len(outcomes) == 2 and [e["operation"] for e in outcomes] == ["Save", "Load"]
            and all(e["result"] == "Succeeded" and e["target"] == "Manual slot 1" for e in outcomes)
            and outcomes[0]["source"] == f"Manual(ManualCatalog {{ dialog_session: {save_session} }})"
            and outcomes[1]["source"] == f"Load(NormalCatalog {{ dialog_session: {load_session} }})"
            and save["pointer"] < outcomes[0]["sample_index"] <= save["after"]
            and confirm["pointer"] < outcomes[1]["sample_index"] <= confirm["after"]
            and load_session > save_session and evidence["file"] == file_identity(root), "exact save/load outcome or bytes differ")
