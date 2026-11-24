#!/usr/bin/env python3
"""OS-driven UI feedback. This bounded subset is not full P1/formal acceptance."""
from __future__ import annotations

import argparse
import copy
from datetime import UTC, datetime
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import time

import native_acceptance as native
import wall_door_joint_acceptance as joint
import ui_refactor_rows as refactor_rows
import ui_progress_bars as progress_bars
import ui_bridge_planning as bridge_planning
import ui_rest_area_planning as rest_area_planning
import ui_rest_area_usage as rest_area_usage
import ui_rest_area_art as rest_area_art
import ui_rest_area_maxzoom as rest_area_maxzoom
import ui_spa_planning as spa_planning
import ui_spa_occupied_save as spa_occupied
import ui_terrain_materials as terrain_materials

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from scripts.native_ui_input import X11Input
from scripts.native_ui_portal import PortalSession, PortalX11Input
from scripts.native_ui_eis import PortalEisInput
from scripts import ui_spa_candidate_art as spa_art
from scripts import ui_spa_resumed_use as spa_resumed
from scripts import ui_spa_loaded_use as spa_loaded
from scripts import ui_m5_planning as m5
from scripts import ui_m2_planning as m2
from scripts import ui_m2_completion as m2_completion
from scripts import ui_m2_tank_completion as tank_completion
from scripts import ui_m2_tank_inflow as tank_inflow
from scripts import ui_m2_tank_full as tank_full
from scripts import ui_m2_tank_supply as tank_supply
from scripts import ui_m2_tank_persistence as tank_persistence
from scripts import ui_first_waterworks as waterworks
from scripts import ui_refactor_stockpile as stockpile_target
from scripts import ui_refactor_spa_slots as refactor_spa

PROFILE = "ui-usability-feedback"
SETTINGS_CASE = "refactor-settings-sliders"
SETTINGS_CHECKPOINTS = ("settings-initial", "settings-scale", "settings-camera",
                        "settings-notification", "settings-interval", "settings-generations",
                        "settings-closed-refactor", "settings-reopened-scale", "settings-reopened-camera",
                        "settings-reopened-notification", "settings-reopened-interval", "settings-reopened-generations")
SETTINGS_DRAGS = (
    ("UiScale", "ui_scale", 1.25, "125%"),
    ("CameraPanSpeed", "camera_pan_speed", 1000.0, "1000 ワールド単位/秒"),
    ("NotificationDuration", "notification_duration_seconds", 12, "12 秒"),
    ("AutosaveInterval", "autosave_interval_minutes", 30, "30 分"),
    ("AutosaveGenerations", "autosave_generations", 5, "5 世代"),
)
CHECKPOINTS = ("tasks", "last-page", "minimized", "restored", "entities",
               "paused-tooltip", "settings", "settings-scrolled", "settings-closed",
               "history", "history-oldest", "history-closed", "guide", "guide-closed")
VIEWPORTS = ((1920, 1080, 1.0), (1280, 720, 1.25), (1280, 720, 0.85),
             (1280, 720, 1.0), (1920, 1080, 0.85), (1920, 1080, 1.25))
CASES = ("navigation", "refactor-rows", "refactor-pinned-rename", "progress-bars", "terrain-materials", "refactor-suite",
         "bridge-planning", "soul-spa-planning", "soul-spa-completion", "soul-spa-usage", "soul-spa-persistence", "soul-spa-slots", "soul-spa-occupied-save", "rest-area-planning", "rest-area-refund", "rest-area-completion", "rest-area-usage", "rest-area-exit", "rest-area-save", "rest-area-lifecycle", "rest-area-maxzoom", "rest-area-candidate-lifecycle")
CASES += (waterworks.CASE, SETTINGS_CASE, tank_completion.GHOST_CASE, stockpile_target.CASE, refactor_spa.CASE)
SPA_CASES = (refactor_spa.CASE, "soul-spa-planning", "soul-spa-completion", "soul-spa-usage", "soul-spa-persistence", "soul-spa-slots", "soul-spa-occupied-save", spa_art.CASE, spa_resumed.CASE, spa_loaded.CASE)
SPA_ART_CASES = (spa_art.CASE, spa_resumed.CASE, spa_loaded.CASE)
CASES += SPA_ART_CASES
CASES += tuple(m5.KINDS)
CASES += tuple(m2.KINDS)
COMPLETION_CASES = (m2_completion.CASE, tank_completion.CASE, tank_inflow.CASE, tank_full.CASE, tank_supply.CASE, tank_persistence.CASE)
CASES += COMPLETION_CASES
PLANNING_CASES = (*m5.KINDS, *m2.KINDS)
REST_CASES = ("rest-area-planning", "rest-area-refund", "rest-area-completion", "rest-area-usage", "rest-area-exit", "rest-area-save", "rest-area-lifecycle", "rest-area-maxzoom", "rest-area-candidate-lifecycle")


def completion_recipe(case):
    native.require(case in COMPLETION_CASES, "dedicated completion case required")
    if case == tank_persistence.CASE:
        return tank_persistence
    return tank_supply if case == tank_supply.CASE else tank_full if case == tank_full.CASE else tank_inflow if case == tank_inflow.CASE else tank_completion if case == tank_completion.CASE else m2_completion


def planning_recipe(case):
    native.require(case in PLANNING_CASES, "dedicated planning case required")
    return m2 if case in m2.KINDS else m5


def planned_art(args, repo):
    native.require(not getattr(args, "m5_art_source", None) or args.case in m5.KINDS, "M5 source requires dedicated case")
    native.require(not getattr(args, "m2_art_source", None) or args.case in (*m2.KINDS, *COMPLETION_CASES), "M2 source requires dedicated case")
    if args.case in COMPLETION_CASES:
        return completion_recipe(args.case).planned_contract(args, repo)
    if args.case in PLANNING_CASES:
        return planning_recipe(args.case).planned_contract(args, repo)
    native.require(not getattr(args, "spa_resume_source", None) or args.case == spa_loaded.CASE,
                   "Spa save input requires the dedicated loaded case")
    native.require(not getattr(args, "soul_spa_art_source", None) or args.case in SPA_ART_CASES,
                   "Spa art source requires its explicit candidate case")
    return (spa_art if args.case in SPA_ART_CASES else rest_area_art).planned_contract(args, repo)


def session_matrix(case, smoke):
    native.require(case in CASES, "unsupported UI case")
    native.require(case != SETTINGS_CASE or smoke, "settings drag case requires the default smoke viewport")
    cases = ("refactor-rows", "progress-bars") if case == "refactor-suite" else (case,)
    return [(item, *viewport) for item in cases for viewport in (VIEWPORTS[:1] if smoke else VIEWPORTS)]


def check_session_inventory(manifest):
    case = manifest.get("case", "navigation")
    expected = session_matrix(case, manifest["smoke"])
    sessions = manifest["sessions"]
    native.require(len(sessions) == len(expected), "case/viewport coverage missing")
    native.require([item.get("case", case) for item in sessions] == [item[0] for item in expected],
                   "session case order differs")
    for field in ("directory", "nonce"):
        native.require(len({item[field] for item in sessions}) == len(sessions), "duplicate session identity")
    return expected


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_observation(case: str, value: dict) -> None:
    if case in tank_completion.GHOST_CHECKPOINTS:
        tank_completion.check_ghost_observation(case, value)
        return
    if case in SETTINGS_CHECKPOINTS:
        check_settings_observation(case, value)
        return
    native.require(value["ready"] and value["frame"] > 90, "fixture is not ready")
    native.require(value["task_rows"] <= 20, "task resident row limit exceeded")
    if case in tank_persistence.CHECKPOINTS[-2:]:
        tank_persistence.check_observation(case, value)
        return
    if case in tank_supply.CHECKPOINTS[3:]:
        tank_supply.check_observation(case, value)
        return
    if case in (tank_inflow.CHECKPOINTS[-1], tank_full.CHECKPOINTS[-1]):
        tank_inflow.check_observation(case, value)
        return
    if case in tank_completion.CHECKPOINTS:
        tank_completion.check_observation(case, value)
        return
    if case in m2_completion.CHECKPOINTS:
        m2_completion.check_observation(case, value)
        return
    if case in m5.CHECKPOINTS:
        m5.check_observation(case, value)
        return
    if case in m2.CHECKPOINTS:
        m2.check_observation(case, value)
        return
    if case == spa_loaded.CHECKPOINTS[1]:
        spa_loaded.check_observation(case, value)
        return
    elif case in spa_resumed.CHECKPOINTS[-3:]:
        spa_resumed.check_observation(case, value)
        return
    if case in spa_occupied.CHECKPOINTS[-2:]:
        spa_occupied.check_observation(case, value)
        return
    if case in refactor_spa.CHECKPOINTS:
        refactor_spa.check_observation(case, value)
        return
    if case in spa_planning.SLOTS_CHECKPOINTS[-2:]:
        spa_planning.check_slots_observation(case, value)
        return
    if case in spa_planning.PERSISTENCE_CHECKPOINTS[3:]:
        spa_planning.check_persistence_observation(case, value)
        return
    if case in spa_planning.USAGE_CHECKPOINTS[3:]:
        spa_planning.check_usage_observation(case, value)
        return
    if case in spa_planning.COMPLETION_CHECKPOINTS:
        spa_planning.check_completion_observation(case, value)
        return
    if case in spa_planning.CHECKPOINTS:
        spa_planning.check_observation(case, value)
        return
    if case in rest_area_maxzoom.CASES:
        rest_area_maxzoom.check_observation(case, value)
        return
    if case in bridge_planning.CHECKPOINTS:
        bridge_planning.check_observation(case, value)
        return
    if case in (*rest_area_planning.CHECKPOINTS, *rest_area_planning.REFUND_CHECKPOINTS,
                *rest_area_planning.COMPLETION_CHECKPOINTS):
        rest_area_planning.check_observation(case, value)
        return
    if case in terrain_materials.CHECKPOINTS:
        terrain_materials.check_observation(case, value)
        return
    if case in (*rest_area_usage.EXIT_CHECKPOINTS[3:], *rest_area_usage.SAVE_CHECKPOINTS[5:],
                rest_area_usage.LIFECYCLE_CHECKPOINTS[-1]):
        rest_area_usage.check_observation(case, value)
        return
    if case in waterworks.CHECKPOINTS:
        waterworks.check_observation(case, value)
        return
    if case in stockpile_target.CHECKPOINTS:
        stockpile_target.check_observation(case, value)
        return
    if case in refactor_rows.PIN_CHECKPOINTS:
        refactor_rows.check_pin_observation(case, value)
        return
    if case in refactor_rows.CHECKPOINTS:
        refactor_rows.check_observation(case, value)
        return
    if case in progress_bars.CHECKPOINTS:
        progress_bars.check_observation(case, value)
        return
    controls = value["controls"]
    if case in ("tasks", "last-page", "restored"):
        native.require(value["left_panel"] == "TaskList" and not value["minimized"], "Tasks body not open")
        native.require("scroll:tasks" in controls and value["task_rows"] > 0, "task scroll missing")
        native.require(not any(key.startswith("text-field:EntityList") for key in controls), "hidden search remains visible")
        mode = value.get("mode_presentation")
        if mode and mode["rect"]:
            for action in ("FirstPage", "PreviousPage", "NextPage", "LastPage"):
                footer = controls[f"task-control:{action}"]["rect"]
                hint = mode["rect"]
                native.require(footer[3] <= hint[1] or footer[0] >= hint[2] or footer[2] <= hint[0],
                               "mode guidance overlaps task page controls")
    if case == "tasks":
        native.require(value["page"] == 0 and value["task_total"] >= 200, "task fixture/page differs")
    if case == "last-page":
        native.require(value["page"] == (value["task_total"] - 1) // 20, "last page is unreachable")
    if case == "minimized":
        native.require(value["minimized"] and "scroll:tasks" not in controls, "minimized body remains interactive")
    if case == "entities":
        native.require(value["left_panel"] == "EntityList" and "scroll:tasks" not in controls, "tab bodies overlap")
    if case == "paused-tooltip":
        native.require(value["paused"] and value["tooltip_alpha"] >= 0.95, "new paused tooltip did not appear")
        tooltip = value["tooltip_presentation"]
        native.require(tooltip and tooltip["rect"] and tooltip["stack_index"] is not None,
                       "paused tooltip has no visible layout")
        native.require(any(text["text"].strip() and text["rect"] and text["alpha"] >= 0.95
                           for text in value["tooltip_text"]), "paused tooltip has no visible text")
        mode = value["mode_presentation"]
        if mode and mode["rect"]:
            a, b = tooltip["rect"], mode["rect"]
            overlap = max(a[0], b[0]) < min(a[2], b[2]) and max(a[1], b[1]) < min(a[3], b[3])
            native.require(not overlap or (mode["stack_index"] is not None
                           and tooltip["stack_index"] > mode["stack_index"]),
                           "paused tooltip is behind the mode guidance")
    if case in ("settings", "settings-scrolled"):
        native.require("menu:CloseSettings" in controls and "scroll:Settings Scroll Area" in controls, "settings controls clipped")
    if case == "settings-scrolled":
        native.require(controls["scroll:Settings Scroll Area"]["scroll"][1] >= 0, "invalid settings scroll")
    if case == "settings-closed":
        native.require("menu:CloseSettings" not in controls, "Settings failed to close")
    if case in ("history", "history-oldest"):
        native.require(value["history_open"] and value["history_count"] == 64, "history fixture differs")
        native.require("notifications-close" in controls and "scroll:history" in controls, "history controls missing")
    if case == "history-oldest":
        native.require(controls["scroll:history"]["scroll"][1] > 0 and value["visible_history"], "history did not scroll")
    if case == "history-closed":
        native.require(not value["history_open"], "history failed to close")
    if case == "guide":
        native.require("scroll:Work Guide Scroll Area" in controls and "menu:EndWorkGuide" in controls,
                       "guide body or exit is not visible")
    if case == "guide-closed":
        native.require("scroll:Work Guide Scroll Area" not in controls and "menu:EndWorkGuide" not in controls,
                       "guide failed to close")


def paused_tooltip_ready(value: dict) -> bool:
    try:
        check_observation("paused-tooltip", value)
    except native.AcceptanceError:
        return False
    return True


def check_entities_render(value):
    if value.get("menu_state") != "Zones":
        native.require(value.get("list_text") and any(
            item["text"].strip() and item["rect"] is not None for item in value["list_text"]
        ), "Familiar header text has no visible layout")
    native.require(value["paused"] and "text-field:DevPoc" not in value["controls"],
                   "render fixture must be paused with developer body hidden")
    if value.get("menu_state") == "Zones":
        menu = value.get("zones_rect")
        hint = value["mode_presentation"]["rect"]
        native.require(menu and (hint is None or menu[3] <= hint[1]), "submenu obscures mode guidance")


def rectangle_union_area(rectangles):
    edges = sorted({x for rect in rectangles for x in (rect[0], rect[2])})
    area = 0.0
    for left, right in zip(edges, edges[1:]):
        intervals = sorted((r[1], r[3]) for r in rectangles if r[0] < right and r[2] > left)
        covered, end = 0.0, float("-inf")
        for low, high in intervals:
            covered += max(0.0, high - max(low, end))
            end = max(end, high)
        area += (right - left) * covered
    return area


def check_layout_scene(value, scene):
    native.require(value.get("layout_scene") == scene, "layout scene differs")
    native.require(value["paused"] and "text-field:DevPoc" not in value["controls"], "render fixture is not prepared")
    native.require(not (value["management_rect"] and value["inspector_rect"]), "workspace panels overlap")
    if scene == "management":
        check_entities_render(value)
        if value.get("menu_state") == "Zones":
            native.require(not value["management_rect"] and not value["inspector_rect"], "tool menu must close the workspace")
        else:
            native.require(value["management_rect"], "management page is hidden")
    elif scene in ("selection", "pinned"):
        native.require(value["inspector_rect"] and not value["management_rect"], "selection page is missing")
        header = value["controls"].get("text:inspection-header", {})
        native.require(header.get("fully_visible") and header.get("text", "").strip(), "inspection target heading is missing or clipped")
        area_entries = [key for key in value["controls"] if key.startswith("menu:SelectAreaTaskFor(")]
        native.require(len(area_entries) == 1 and value["controls"][area_entries[0]].get("fully_visible"), "familiar area entry is missing or clipped")
        label = value["controls"].get("label:" + area_entries[0], {})
        native.require(label.get("fully_visible") and label.get("text", "").strip(), "familiar area entry label is missing or clipped")
        if scene == "pinned":
            native.require(value.get("pinned") and value.get("selected") and value["pinned"] != value["selected"], "pin and selection must be distinct")
            native.require(value["controls"].get("menu:Workspace(InspectSelection)", {}).get("fully_visible"), "current selection chip is not fully visible")
    elif scene in ("area", "area-details"):
        native.require(value.get("area_edit_rect") and not value["inspector_rect"] and not value["management_rect"], "area editor is missing or overlaps workspace")
        for key in ("menu:FinishAreaEdit", "area-details-toggle"):
            native.require(value["controls"].get(key, {}).get("fully_visible"), "area editor primary action is clipped")
            label = value["controls"].get("label:" + key, {})
            native.require(label.get("fully_visible") and label.get("text", "").strip(), "area editor primary label is missing or clipped")
        controls = [item for key, item in value["controls"].items() if key.startswith("area-control:")]
        native.require(len(controls) == (10 if scene == "area-details" else 0), "area details disclosure differs")
        native.require(all(item.get("fully_visible") for item in controls), "area detail action is clipped")
        labels = [item for key, item in value["controls"].items() if key.startswith("label:area-control:")]
        native.require(len(labels) == len(controls) and all(item.get("fully_visible") and item.get("text", "").strip() for item in labels), "area detail label is missing or clipped")
    elif scene == "build":
        native.require(value["catalog_rect"] and not value["inspector_rect"], "build catalog is missing or overlaps details")
        cards = [key for key in value["controls"] if key.startswith("menu:SelectBuild(") or key in ("menu:SelectFloorPlace", "menu:SelectTaskMode(SoulSpaPlace(None))")]
        native.require(len(cards) == 12, "not every build kind is visible")
        native.require(all(value["controls"][key].get("fully_visible") for key in cards), "build card is clipped")
    elif scene == "display":
        native.require(value["display_rect"], "display page is hidden")
    else:
        native.require(value["workspace_page"] == "World" and not value["management_rect"] and not value["inspector_rect"], "normal map has a workspace open")
        native.require(value["mode_presentation"]["rect"] is None, "normal map has persistent tool guidance")
    native.require(value.get("ui_rects"), "UI occlusion measurements are missing")
    ratio = rectangle_union_area(value["ui_rects"]) / (value["viewport"][0] * value["viewport"][1])
    limit = {"normal": 0.20, "selection": 0.30, "pinned": 0.30, "area": 0.30, "area-details": 0.40, "build": 0.40}.get(scene, 0.60)
    native.require(ratio <= limit, f"UI occlusion {ratio:.1%} exceeds {limit:.0%}")
    return ratio


class Driver:
    def __init__(self, process, root, nonce, state, job_file, portal=None, *, layout_only=False, eis=False, scroll=False, fresh_ack=False):
        self.process, self.root, self.nonce = process, root, nonce
        self.state, self.job_file = state, job_file
        self.events, self.checkpoints = [], []
        self.fresh_ack = fresh_ack
        self.started = time.monotonic()
        self.input = None
        self.last = self.wait(lambda value: value["ready"], timeout=120)
        windows = native.x11_client_windows_for_process_tree(process.pid)
        native.require(len(windows) == 1, "expected one PID-owned X11 client")
        window, pid = windows[0]
        native.require(pid == self.last["pid"], "observer and X11 PID differ")
        self.window, self.owner_pid = int(window, 0), pid
        if layout_only:
            native.atomic_write_json(self.root / "events.json", {"events": []})
            native.atomic_write_json(self.root / "input-client.json", {
                "window": self.window, "pid": pid, "nonce": nonce, "input_backend": "none"})
            return
        arguments = (int(window, 0), pid, process.pid, nonce, self.record)
        native.require(not eis or portal is not None, "EIS requires the existing consented portal")
        native.require(not scroll or eis, "maxzoom requires the explicit EIS input backend")
        self.input = (PortalEisInput(*arguments, portal=portal, allow_scroll=scroll) if eis else
                      PortalX11Input(*arguments, portal=portal) if portal else X11Input(*arguments))
        try:
            self.input.activate()
            geometry = self.input.client_size()
            native.atomic_write_json(self.root / "input-client.json", {"window": self.input.window,
                "pid": pid, "geometry": geometry, "observer_viewport": self.last["viewport"], "nonce": nonce,
                "input_backend": "portal-eis" if eis else "portal" if portal else "xtest",
                "input_transport": "portal-eis-absolute-scroll-v1" if eis and scroll else "portal-eis-absolute-v1" if eis else "portal-notify-relative-v1" if portal else "xtest",
                "input_ack_policy": "next-exact-observation-v1" if fresh_ack else "settled-exact-observation-v1",
                "eis_scroll": scroll,
                "eis_region": self.input.sender.region_contract if eis else None,
                "xmodifiers": next((entry.split(b"=", 1)[1].decode() for entry in
                                    Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
                                    if entry.startswith(b"XMODIFIERS=")), None),
                "portal_devices": portal.granted_devices if portal else None,
                "portal_session": portal.session if portal else None,
                "monitor_mapping": self.input.mapping if portal else None})
            native.require(list(geometry) == self.last["viewport"], "X11 client and renderer coordinate spaces differ")
            stable = [None, time.monotonic()]
            def pointer_settled(value):
                signature = (value["cursor"], value["mouse_left_pressed"])
                if signature != stable[0]:
                    stable[:] = [signature, time.monotonic()]
                return not value["mouse_left_pressed"] and time.monotonic() - stable[1] >= 1.0
            self.last = self.wait(pointer_settled, after=self.last["frame"])
        except Exception:
            self.input.close()
            raise

    def wait(self, predicate, *, after=0, timeout=15):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            native.require(self.process.poll() is None, "game exited during UI input")
            native.require(time.monotonic() - self.started < 480, "UI session deadline exceeded")
            self.state["heartbeat_at"] = native.utc_now()
            native.atomic_write_json(self.job_file, self.state)
            path = self.root / "observed.json"
            if path.is_file():
                value = native.read_json(path)
                native.require(value["nonce"] == self.nonce and value["pid"] == self.process.pid, "stale observer")
                self.latest_observation = value
                if value["frame"] > after and predicate(value):
                    return value
            time.sleep(0.05)
        raise native.AcceptanceError("UI checkpoint timed out; input will not be resent")

    def record(self, event):
        event.update({"sent_at": native.utc_now(), "pre_frame": self.last["frame"],
                      "world_epoch": self.last["world_epoch"]})
        self.events.append(event)
        native.atomic_write_json(self.root / "events.json", {"events": self.events})

    def send(self, **event):
        self.input.send(str(len(self.events) + 1), self.nonce, **event)
        def acknowledged(value):
            if event.get("point") is not None:
                return value["cursor"] is not None and all(abs(a - b) <= 2 for a, b in zip(value["cursor"], event["point"]))
            if event.get("button") == 1:
                return value["mouse_left_pressed"] == event["pressed"] or (event["pressed"] and value["world_input_captured"])
            if event.get("button") == 3:
                return value["mouse_right_pressed"] == event["pressed"]
            return True
        try:
            self.last = self.wait(acknowledged, after=self.last["frame"] + (0 if getattr(self, "fresh_ack", False) else 3))
        except native.AcceptanceError:
            if event.get("point") is not None:
                try:
                    failure = {
                        "event": self.events[-1], "pre_frame": self.last["frame"],
                        "x11_pointer_after_wait": self.input.client_pointer(),
                        "last_observation": {key: getattr(self, "latest_observation", {}).get(key)
                                             for key in ("nonce", "pid", "frame", "cursor", "world_epoch")},
                    }
                    if isinstance(self.input, (PortalX11Input, PortalEisInput)):
                        try:
                            failure["monitor_mapping_after_wait"] = self.input.portal.motion_mapping(self.input.root_size())
                        except Exception:
                            failure["monitor_mapping_after_wait"] = None
                    native.atomic_write_json(self.root / "failed-input.json", failure)
                except Exception:
                    pass  # Preserve the original failure, including unknown input outcomes.
            raise
        self.acknowledge_events(self.events[-1:])

    def acknowledge_events(self, events):
        for event in events:
            event["post_frame"] = self.last["frame"]
            event["observed_cursor"] = self.last["cursor"]
            event["observed_left_pressed"] = self.last["mouse_left_pressed"]
            event["observed_right_pressed"] = self.last["mouse_right_pressed"]
            event["observed_hover"] = {key: value["interaction"] for key, value in self.last["controls"].items()
                                       if value["interaction"] in ("Hovered", "Pressed")}
            event["observed_hover_entities"] = {key: value.get("entity") for key, value in self.last["controls"].items()
                                                if value["interaction"] in ("Hovered", "Pressed")}
            event["observed_hover_rects"] = {key: value.get("rect") for key, value in self.last["controls"].items()
                                             if value["interaction"] in ("Hovered", "Pressed")}
        native.atomic_write_json(self.root / "events.json", {"events": self.events})

    def point(self, key):
        native.require(key in self.last["controls"], f"missing visible input target: {key}")
        rect = self.last["controls"][key]["rect"]
        return (round((rect[0] + rect[2]) / 2), round((rect[1] + rect[3]) / 2))

    def click(self, key):
        # Dynamic menu text can move an adjacent target between the observed
        # rect and the acknowledged pointer move. Reacquire before pressing;
        # an unmoved but unhovered target is still a hard occlusion failure.
        for _ in range(3):
            point = self.point(key)
            self.send(point=point)
            target = self.last["controls"].get(key)
            if target is not None and target["interaction"] == "Hovered":
                break
            native.require(target is not None and target["fully_visible"],
                           f"target disappeared or is clipped: {key}")
            rect = target["rect"]
            native.require(not (rect[0] <= point[0] <= rect[2]
                                and rect[1] <= point[1] <= rect[3]),
                           f"target is occluded or not hovered: {key}")
        else:
            raise native.AcceptanceError(f"target moved without stable hover: {key}")
        self.send(button=1, pressed=True)
        native.require(self.last["controls"].get(key, {}).get("interaction") == "Pressed",
                       f"target did not receive press: {key}")
        self.send(button=1, pressed=False)
        native.require(self.last["cursor"] is not None and all(abs(a-b) <= 2 for a,b in zip(self.last["cursor"], point)),
                       f"pointer moved outside click target during gesture: {key}")

    def key(self, key):
        first = len(self.events)
        # Release before waiting for frames: a slow renderer must not turn a tap
        # into compositor autorepeat. Both events retain the input ownership checks.
        self.input.send(str(len(self.events) + 1), self.nonce, key=key, pressed=True)
        self.input.send(str(len(self.events) + 1), self.nonce, key=key, pressed=False)
        self.last = self.wait(lambda value: True, after=self.last["frame"] + 3)
        self.acknowledge_events(self.events[first:])

    def focus_text(self, key):
        native.require(key.startswith("text-field:"), "expected a text field")
        point = self.point(key)
        self.send(point=point)
        self.send(button=1, pressed=True)
        self.send(button=1, pressed=False)
        self.last = self.wait(lambda value: value["controls"].get(key, {}).get("focused"), after=self.last["frame"])
        native.require(self.last["cursor"] is not None and all(abs(a-b) <= 2 for a,b in zip(self.last["cursor"], point)), "pointer moved during text focus gesture")

    def scroll(self, key, amount):
        self.send(point=self.point(key))
        for _ in range(amount):
            self.send(button=5, pressed=True)
            self.send(button=5, pressed=False)

    def capture(self, case, *, validate=None):
        self.last = self.wait(lambda value: True, after=self.last["frame"] + 6)
        path = self.root / f"{case}.png"
        owned = native.x11_client_windows_for_process_tree(self.process.pid)
        native.require(len(owned) == 1 and int(owned[0][0], 0) == self.window
                       and owned[0][1] == self.owner_pid, "capture owner changed")
        native.require(native.capture_client_png(hex(self.window), path, import_cmd="import"), "client capture failed")
        dimensions = native.validate_png_structure(path.read_bytes())
        native.require(list(dimensions) == self.last["viewport"], "capture dimensions differ")
        try:
            (validate or check_observation)(case, self.last)
            if self.input is None:
                check_layout_scene(self.last, self.last["layout_scene"])
        except native.AcceptanceError:
            native.atomic_write_json(self.root / "failed-checkpoint.json", {"case": case, "value": self.last})
            raise
        observation = {"case": case, "value": self.last, "last_step": len(self.events),
                       "screenshot": path.name, "sha256": sha256(path),
                       "capture_scope": "x11-client-window", "window": self.window,
                       "pid": self.owner_pid}
        if case in terrain_materials.CHECKPOINTS:
            terrain_materials.add_capture(self.root, observation)
        self.checkpoints.append(observation)
        native.atomic_write_json(self.root / "checkpoints.json", {"checkpoints": self.checkpoints})

    def exercise(self):
        if "text-field:DevPoc" in self.last["controls"]:
            self.click("dev-minimize")
            self.last = self.wait(lambda value: "text-field:DevPoc" not in value["controls"], after=self.last["frame"])
        self.click("menu:Workspace(OpenEntities)")
        self.click("tab:TaskList")
        self.capture("tasks")
        self.click("task-control:LastPage")
        self.capture("last-page")
        self.click("minimize")
        self.capture("minimized")
        self.click("minimize")
        self.capture("restored")
        self.click("tab:EntityList")
        self.capture("entities")
        self.key("space")
        self.last = self.wait(lambda value: value["paused"], after=self.last["frame"])
        self.send(point=(round(self.last["viewport"][0] - 10), 10))
        self.last = self.wait(lambda value: value["tooltip_alpha"] == 0, after=self.last["frame"])
        self.send(point=self.point("menu:ToggleSystemMenu"))
        self.last = self.wait(paused_tooltip_ready, after=self.last["frame"])
        self.capture("paused-tooltip")
        self.click("menu:ToggleSystemMenu")
        self.click("pause-menu:ToggleSettings")
        self.capture("settings")
        self.scroll("scroll:Settings Scroll Area", 12)
        self.capture("settings-scrolled")
        self.click("menu:CloseSettings")
        self.capture("settings-closed")
        self.click("pause-menu:ToggleSystemMenu")
        self.key("space")
        self.click("notifications")
        self.capture("history")
        self.scroll("scroll:history", 65)
        self.capture("history-oldest")
        self.click("notifications-close")
        self.capture("history-closed")
        self.key("F1")
        self.last = self.wait(lambda value: "menu:StartWorkGuide" in value["controls"], after=self.last["frame"])
        self.click("menu:StartWorkGuide")
        self.capture("guide")
        self.click("menu:EndWorkGuide")
        self.capture("guide-closed")


def check_input_client(manifest, session, client):
    native.require(client["input_backend"] == manifest["input_backend"] and client["nonce"] == session["nonce"],
                   "input client/backend differs")
    if manifest["input_backend"] in ("portal", "portal-eis"):
        native.require(client["portal_devices"] & 3 == 3 and client["monitor_mapping"][1] > 0,
                       "portal consent or monitor mapping missing")
        if manifest.get("case") == "refactor-suite":
            native.require(manifest.get("portal_session") and client.get("portal_session") == manifest["portal_session"],
                           "suite did not share one portal session")
    if manifest["input_backend"] == "portal-eis":
        rest_area_art.check_eis_client(client, scroll=manifest.get("case") == "rest-area-maxzoom")
    if session.get("case", manifest.get("case")) in ("refactor-rows", "refactor-pinned-rename"):
        native.require(client.get("xmodifiers") == "@im=local", "row text input requires child-local XIM")


def check_settings_observation(case, value):
    native.require(value.get("ready") is True and value.get("frame", 0) > 90
                   and case in SETTINGS_CHECKPOINTS and value.get("paused") is True,
                   "settings checkpoint or pause differs")
    settings = value.get("settings_values")
    native.require(isinstance(settings, dict), "settings values missing")
    stage = SETTINGS_CHECKPOINTS.index(case)
    expected = {"ui_scale": 1.0, "camera_pan_speed": 500.0,
                "notification_duration_seconds": 4, "autosave_interval_minutes": 10,
                "autosave_generations": 3}
    for index, (_, key, target, _) in enumerate(SETTINGS_DRAGS, start=1):
        if stage >= index:
            expected[key] = target
    native.require(all(type(settings.get(key)) in (int, float)
                       and abs(settings[key] - expected_value) < 0.001
                       for key, expected_value in expected.items()), "settings applied values differ")
    controls = value.get("controls", {})
    if case == "settings-closed-refactor":
        native.require("menu:CloseSettings" not in controls, "settings remained open")
        return
    native.require("menu:CloseSettings" in controls, "settings is not open")
    if 1 <= stage <= len(SETTINGS_DRAGS) or stage >= 7:
        field, _, target, label = SETTINGS_DRAGS[stage - 1 if stage <= 5 else stage - 7]
        slider = controls.get("slider:" + field, {})
        label_node = controls.get("settings-value:" + field, {})
        native.require(slider.get("fully_visible") is True
                       and type(slider.get("slider_value")) in (int, float)
                       and slider.get("pressed") is False
                       and label_node.get("text") == label
                       and label_node.get("fully_visible") is True,
                       "settings slider, label or release differs")
        expected_slider = {"UiScale": 1.25, "CameraPanSpeed": 1000.0,
                           "NotificationDuration": 2.0, "AutosaveInterval": 3.0,
                           "AutosaveGenerations": 5.0}[field]
        native.require(abs(slider["slider_value"] - expected_slider) < 0.001,
                       "settings slider value differs")


def settings_drag(driver, field, setting, target):
    key = "slider:" + field
    control = driver.last["controls"].get(key)
    native.require(control and control.get("fully_visible") is True, "slider missing or clipped")
    rect = control["rect"]
    start = (round(rect[0] + (rect[2] - rect[0]) * .15), round((rect[1] + rect[3]) / 2))
    end = (min(round(driver.last["viewport"][0]) - 2, round(rect[2] + (rect[2] - rect[0]))), start[1])
    driver.send(point=start)
    before = copy.deepcopy(driver.last)
    first = len(driver.events)
    entity = before["controls"][key]["entity"]
    driver.send(button=1, pressed=True)
    held = copy.deepcopy(driver.last)
    native.require(held["controls"].get(key, {}).get("entity") == entity
                   and held["controls"][key].get("pressed") is True,
                   "slider did not receive press")
    driver.send(point=end)
    driver.send(button=1, pressed=False)
    driver.last = driver.wait(lambda value: abs(value["settings_values"][setting] - target) < .001
                              and value["controls"].get(key, {}).get("pressed") is False,
                              after=driver.last["frame"], timeout=15)
    driver.events[first]["settings_drag"] = {"field": field, "entity": entity,
        "first_step": first + 1, "last_step": len(driver.events), "before": before,
        "held": held, "after": copy.deepcopy(driver.last)}
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise_settings(driver):
    if "text-field:DevPoc" in driver.last["controls"]:
        driver.click("dev-minimize")
        driver.last = driver.wait(lambda value: "text-field:DevPoc" not in value["controls"], after=driver.last["frame"])
    # Opening the actual system menu pauses the game; avoid relying on a
    # separate shortcut before the pointer-input scenario under test.
    driver.click("menu:ToggleSystemMenu")
    driver.last = driver.wait(lambda value: value["paused"], after=driver.last["frame"])
    driver.click("pause-menu:ToggleSettings")
    driver.capture(SETTINGS_CHECKPOINTS[0])
    for index, (field, setting, target, _) in enumerate(SETTINGS_DRAGS):
        if index == 3:
            driver.scroll("scroll:Settings Scroll Area", 12)
        settings_drag(driver, field, setting, target)
        driver.capture(SETTINGS_CHECKPOINTS[index + 1])
    driver.click("menu:CloseSettings")
    driver.capture("settings-closed-refactor")
    driver.click("pause-menu:ToggleSettings")
    driver.send(point=driver.point("scroll:Settings Scroll Area"))
    for _ in range(12):
        driver.send(button=4, pressed=True)
        driver.send(button=4, pressed=False)
    for index, (field, _, _, _) in enumerate(SETTINGS_DRAGS):
        for _ in range(20):
            control = driver.last["controls"].get("slider:" + field, {})
            label = driver.last["controls"].get("settings-value:" + field, {})
            if control.get("fully_visible") and label.get("fully_visible"):
                break
            driver.scroll("scroll:Settings Scroll Area", 1)
        else:
            raise native.AcceptanceError("reopened settings slider or label is not visible")
        driver.capture(SETTINGS_CHECKPOINTS[index + 7])


def verify_settings_sequence(entries, events):
    native.require([entry["case"] for entry in entries] == list(SETTINGS_CHECKPOINTS),
                   "settings checkpoint sequence differs")
    actions = [(index, event["settings_drag"]) for index, event in enumerate(events) if "settings_drag" in event]
    native.require(len(actions) == len(SETTINGS_DRAGS), "settings drag count differs")
    for (index, action), (field, setting, target, _) in zip(actions, SETTINGS_DRAGS, strict=True):
        native.require(action.get("field") == field and action.get("first_step") == index + 1
                       and action.get("last_step") == index + 3 and index + 2 < len(events),
                       "settings drag binding differs")
        press, move, release = events[index:index + 3]
        native.require(press.get("button") == 1 and press.get("pressed") is True
                       and move.get("point") is not None and release.get("button") == 1
                       and release.get("pressed") is False, "settings drag input differs")
        before, held, after = (action[name] for name in ("before", "held", "after"))
        key = "slider:" + field
        rect = before["controls"][key]["rect"]
        start = [round(rect[0] + (rect[2] - rect[0]) * .15), round((rect[1] + rect[3]) / 2)]
        end = [min(round(before["viewport"][0]) - 2, round(rect[2] + (rect[2] - rect[0]))), start[1]]
        previous = entries[SETTINGS_DRAGS.index((field, setting, target, _))]
        following = entries[SETTINGS_DRAGS.index((field, setting, target, _)) + 1]
        native.require(index > 0 and events[index - 1].get("point") == start
                       and isinstance(before.get("cursor"), list) and len(before["cursor"]) == 2
                       and isinstance(move.get("observed_cursor"), list) and len(move["observed_cursor"]) == 2
                       and isinstance(release.get("observed_cursor"), list) and len(release["observed_cursor"]) == 2
                       and all(abs(a - b) <= 2 for a, b in zip(before["cursor"], start))
                       and move.get("point") == end
                       and move.get("observed_left_pressed") is True
                       and release.get("observed_left_pressed") is False
                       and all(abs(a - b) <= 2 for a, b in zip(move["observed_cursor"], end))
                       and all(abs(a - b) <= 2 for a, b in zip(release["observed_cursor"], end))
                       and before["frame"] == press["pre_frame"]
                       and held["frame"] == press["post_frame"]
                       and held["frame"] == move["pre_frame"]
                       and move["post_frame"] == release["pre_frame"]
                       and release["post_frame"] <= after["frame"]
                       and previous["last_step"] < index + 1 <= index + 3 <= following["last_step"]
                       and previous["value"]["frame"] < before["frame"] < after["frame"] < following["value"]["frame"]
                       and all(snapshot["nonce"] == press["nonce"] and snapshot["pid"] == press["pid"]
                               for snapshot in (before, held, after)),
                       "settings drag input and observations are not bound")
        native.require(before["controls"][key]["entity"] == action["entity"]
                       and held["controls"][key]["entity"] == action["entity"]
                       and held["controls"][key]["pressed"] is True
                       and after["controls"][key]["entity"] == action["entity"]
                       and after["controls"][key]["pressed"] is False
                       and before["frame"] < held["frame"] < after["frame"]
                       and before["world_epoch"] == held["world_epoch"] == after["world_epoch"]
                       and abs(after["settings_values"][setting] - target) < .001,
                       "settings drag observation differs")


def verify_root(root):
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest["profile"] == PROFILE and manifest["evidence_kind"] == "feedback", "unsupported evidence kind")
    native.require(manifest.get("input_backend") in ("portal", "portal-eis", "xtest", "none"), "input backend missing")
    native.require(manifest.get("input_backend") != "portal-eis" or manifest.get("ordinary_art") is not None,
                   "EIS UI feedback requires explicit ordinary ArtPreview context")
    layout_only = manifest["input_backend"] == "none"
    repo = native.validate_repo(manifest["repo"])
    native.require(native.source_fingerprint(repo) == manifest["source_fingerprint"], "source changed")
    native.require(native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"], "harness changed")
    native.require(native.git_subject(repo) == manifest["subject_commit"], "subject changed")
    native.require(sha256(repo / "target/debug/bevy_app") == manifest["binary_sha256"], "binary changed")
    art_contract = manifest.get("ordinary_art")
    native.require(manifest["case"] not in COMPLETION_CASES or art_contract is not None, "Mixer completion needs exact candidate")
    native.require(manifest["case"] not in PLANNING_CASES or art_contract is not None, "planning needs exact source")
    native.require(manifest["case"] not in SPA_ART_CASES or art_contract is not None,
                   "Spa candidate case requires its exact source")
    native.require(manifest["case"] != "rest-area-candidate-lifecycle" or art_contract is not None,
                   "candidate lifecycle requires explicit candidate source")
    native.require(manifest["case"] != "rest-area-maxzoom" or art_contract is not None and manifest["input_backend"] == "portal-eis",
                   "maxzoom requires the explicit ArtPreview EIS profile")
    if art_contract is not None and manifest["case"] in COMPLETION_CASES:
        native.require(manifest["smoke"] is True and manifest["input_backend"] == "portal-eis", "Mixer completion profile differs")
        native.require(rest_area_art.pipeline.exact_json(completion_recipe(manifest["case"]).source_contract(repo, Path(art_contract["source_root"]),
            Path(art_contract["codec"])), art_contract), "Mixer completion source changed")
    elif art_contract is not None and manifest["case"] in PLANNING_CASES:
        native.require(manifest["smoke"] is True and manifest["input_backend"] == "portal-eis", "M5 profile differs")
        native.require(rest_area_art.pipeline.exact_json(planning_recipe(manifest["case"]).source_contract(repo, Path(art_contract["source_root"]),
            Path(art_contract["codec"]), manifest["case"]), art_contract), "M5 art inputs changed")
    elif art_contract is not None and manifest["case"] in SPA_ART_CASES:
        native.require(manifest["smoke"] and manifest["input_backend"] in ("portal", "portal-eis"), "Spa art profile differs")
        native.require(rest_area_art.pipeline.exact_json(spa_art.source_contract(repo,
            Path(art_contract["source_root"]), Path(art_contract["codec"]), case=manifest["case"],
            resume_source=art_contract.get("resume_input", {}).get("root")), art_contract), "Spa art inputs changed")
    elif art_contract is not None:
        native.require(manifest["case"] in ("rest-area-lifecycle", "rest-area-maxzoom", "rest-area-candidate-lifecycle") and manifest["smoke"]
                       and manifest["input_backend"] in ("portal", "portal-eis"), "ordinary art profile differs")
        native.require(manifest["case"] != "rest-area-maxzoom" or manifest["input_backend"] == "portal-eis", "maxzoom backend differs")
        native.require(rest_area_art.candidate_contract(art_contract) == (manifest["case"] == "rest-area-candidate-lifecycle"),
                       "ordinary art authority/case differs")
        expected_art = rest_area_art.source_contract(repo, Path(art_contract["source_root"]), Path(art_contract["codec"]),
                                                     candidate=manifest["case"] == "rest-area-candidate-lifecycle")
        native.require(rest_area_art.pipeline.exact_json(expected_art, art_contract), "ordinary art inputs changed")
    expected = check_session_inventory(manifest)
    for session, (case, width, height, scale) in zip(manifest["sessions"], expected):
        directory = root / session["directory"]
        native.require(directory.parent == root, "session path escapes job")
        entries = native.read_json(directory / "checkpoints.json")["checkpoints"]
        events = native.read_json(directory / "events.json")["events"]
        client = native.read_json(directory / "input-client.json")
        check_input_client(manifest, session, client)
        if case == tank_completion.GHOST_CASE:
            raw_path = directory / "production-cancellation.json"
            release = manifest.get("ghost_release")
            native.require(isinstance(release, dict) and sha256(Path(release["codec"])) == release["codec_sha256"]
                           and sha256(raw_path) == session.get("cancellation_sha256")
                           and release["bindings"] == tank_completion.ghost_bindings(repo, Path(release["codec"]))
                           and release["door"] == joint.release_identities(repo)["door"]
                           and release["verification_inputs"] == rest_area_art.pipeline.numeric_verifier_inventory()
                           and release["lifecycle_sha256"] == sha256(repo / "scripts/building_production_lifecycle.py"),
                           "ghost release/raw changed")
            tank_completion.verify_ghosts(native.read_json(raw_path), entries, client, events)
        if case in COMPLETION_CASES:
            raw_path = directory / "production-cancellation.json"
            native.require(sha256(raw_path) == session.get("cancellation_sha256"), "Mixer completion raw changed")
            completion_recipe(case).check_view(repo, directory / "asset-view", art_contract)
            extra = {"save_path": directory / "saves/manual-1.scn.ron"} if case == tank_persistence.CASE else {}
            completion_recipe(case).verify(native.read_json(raw_path), entries, client, events, art_contract, **extra)
        if case in PLANNING_CASES:
            raw_path = directory / "production-cancellation.json"
            native.require(sha256(raw_path) == session.get("cancellation_sha256"), "M5 raw changed")
            planning_recipe(case).check_view(repo, directory / "asset-view", art_contract)
            planning_recipe(case).verify(native.read_json(raw_path), entries, client, events, art_contract)
        if case in SPA_CASES:
            raw_path = directory / "production-cancellation.json"
            native.require((art_contract is None or case in SPA_ART_CASES) and sha256(raw_path) == session.get("cancellation_sha256"),
                           "Spa planning raw changed or unexpected art admission")
            if case in SPA_ART_CASES:
                spa_art.check_view(repo, directory / "asset-view", art_contract)
                (spa_loaded if case == spa_loaded.CASE else spa_resumed if case == spa_resumed.CASE else spa_art).verify(native.read_json(raw_path), entries, client,
                               directory / "saves" / "manual-1.scn.ron", events, art_contract)
            elif case == "soul-spa-occupied-save":
                spa_occupied.verify(native.read_json(raw_path), entries, client,
                                    directory / "saves" / "manual-1.scn.ron", events)
            elif case == refactor_spa.CASE:
                refactor_spa.verify(native.read_json(raw_path), entries, client, events)
            elif case == "soul-spa-slots":
                spa_planning.verify_slots(native.read_json(raw_path), entries, client, events)
            elif case == "soul-spa-persistence":
                spa_planning.verify(native.read_json(raw_path), entries, client,
                                    directory / "saves" / "manual-1.scn.ron", events, persistence=True)
            elif case == "soul-spa-usage":
                spa_planning.verify_usage(native.read_json(raw_path), entries, client, events)
            elif case == "soul-spa-completion":
                spa_planning.verify_completion(native.read_json(raw_path), entries, client, events)
            else:
                spa_planning.verify(native.read_json(raw_path), entries, client,
                                    directory / "saves" / "manual-1.scn.ron", events)
        if case in REST_CASES:
            raw_path = directory / "production-cancellation.json"
            native.require(sha256(raw_path) == session.get("cancellation_sha256"), "cancellation raw changed")
            if case == "rest-area-maxzoom":
                rest_area_art.check_view(repo, directory / "asset-view", art_contract)
                rest_area_maxzoom.verify(native.read_json(raw_path), entries, client, art_contract, events)
            elif case in ("rest-area-lifecycle", "rest-area-candidate-lifecycle"):
                if art_contract is not None:
                    rest_area_art.check_view(repo, directory / "asset-view", art_contract)
                    rest_area_art.check_lifecycle(native.read_json(raw_path), entries, client,
                                                directory / "saves" / "manual-1.scn.ron", art_contract)
                else:
                    rest_area_usage.check_lifecycle(native.read_json(raw_path), entries, client,
                                                   directory / "saves" / "manual-1.scn.ron")
            elif case == "rest-area-save":
                rest_area_usage.check_save(native.read_json(raw_path), entries, client,
                                           directory / "saves" / "manual-1.scn.ron")
            elif case == "rest-area-exit":
                rest_area_usage.check_exit(native.read_json(raw_path), entries, client)
            elif case == "rest-area-usage":
                rest_area_usage.check_usage(native.read_json(raw_path), entries, client)
            elif case == "rest-area-completion":
                rest_area_planning.check_completion(native.read_json(raw_path), entries, client)
            else:
                rest_area_planning.check_cancellation(native.read_json(raw_path), entries, client,
                                                      refund=case == "rest-area-refund")
        native.require(case == "navigation" or not layout_only, "refactor case requires actual input")
        expected_checkpoints = (SETTINGS_CHECKPOINTS if case == SETTINGS_CASE else
                                completion_recipe(case).CHECKPOINTS if case in COMPLETION_CASES else
                                planning_recipe(case).CHECKPOINTS if case in PLANNING_CASES else
                                waterworks.CHECKPOINTS if case == waterworks.CASE else
                                stockpile_target.CHECKPOINTS if case == stockpile_target.CASE else
                                refactor_rows.CHECKPOINTS if case == "refactor-rows" else
                                refactor_rows.PIN_CHECKPOINTS if case == "refactor-pinned-rename" else
                                tank_completion.GHOST_CHECKPOINTS if case == tank_completion.GHOST_CASE else
                                spa_loaded.CHECKPOINTS if case == spa_loaded.CASE else
                                spa_resumed.CHECKPOINTS if case == spa_resumed.CASE else
                                spa_occupied.CHECKPOINTS if case in ("soul-spa-occupied-save", spa_art.CASE) else
                                refactor_spa.CHECKPOINTS if case == refactor_spa.CASE else
                                spa_planning.SLOTS_CHECKPOINTS if case == "soul-spa-slots" else
                                spa_planning.PERSISTENCE_CHECKPOINTS if case == "soul-spa-persistence" else
                                spa_planning.USAGE_CHECKPOINTS if case == "soul-spa-usage" else
                                spa_planning.COMPLETION_CHECKPOINTS if case == "soul-spa-completion" else
                                spa_planning.CHECKPOINTS if case == "soul-spa-planning" else
                                bridge_planning.CHECKPOINTS if case == "bridge-planning" else
                                rest_area_planning.CHECKPOINTS if case == "rest-area-planning" else
                                rest_area_planning.REFUND_CHECKPOINTS if case == "rest-area-refund" else
                                rest_area_planning.COMPLETION_CHECKPOINTS if case == "rest-area-completion" else
                                rest_area_usage.CHECKPOINTS if case == "rest-area-usage" else
                                rest_area_usage.EXIT_CHECKPOINTS if case == "rest-area-exit" else
                                rest_area_usage.SAVE_CHECKPOINTS if case == "rest-area-save" else
                                rest_area_usage.LIFECYCLE_CHECKPOINTS if case in ("rest-area-lifecycle", "rest-area-candidate-lifecycle") else
                                rest_area_maxzoom.CHECKPOINTS if case == "rest-area-maxzoom" else
                                terrain_materials.CHECKPOINTS if case == "terrain-materials" else
                                progress_bars.CHECKPOINTS if case == "progress-bars" else CHECKPOINTS)
        native.require([item["case"] for item in entries] == (["entities"] if layout_only else list(expected_checkpoints)), "checkpoints differ")
        if layout_only:
            native.require(not events, "layout capture must not send input")
        else:
            native.require(events and [event["step"] for event in events] == [str(n) for n in range(1, len(events) + 1)], "input sequence differs")
        for event in events:
            native.require(event["nonce"] == session["nonce"] and event["post_frame"] > event["pre_frame"], "input ACK missing")
        for item in entries:
            value = item["value"]
            check_observation(item["case"], value)
            if layout_only:
                scene = manifest.get("layout_scene", "management")
                check_layout_scene(value, scene)
                native.require(value.get("menu_state") == ("Zones" if manifest.get("layout_menu") else "Architect" if scene == "build" else "Hidden"),
                               "layout menu fixture differs")
            expected_size = terrain_materials.SIZES[item["case"]] if case == "terrain-materials" else [width, height]
            expected_scale = (1.0 if item["case"] == "settings-initial" else 1.25) if case == SETTINGS_CASE else scale
            native.require(value["nonce"] == session["nonce"] and value["viewport"] == expected_size
                           and abs(value["ui_scale"] - expected_scale) < 0.001, "viewport/nonce differs")
            native.require(item["pid"] == value["pid"] and item["capture_scope"] == "x11-client-window", "capture scope differs")
            image = directory / item["screenshot"]
            native.require(image.parent == directory and sha256(image) == item["sha256"], "screenshot changed")
            native.require(list(native.validate_png_structure(image.read_bytes())) == expected_size, "image dimensions differ")
        native.require(sha256(directory / "game.log") == session["log_sha256"], "game log changed")
        joint.verify_game_log(directory / "game.log", "Intel")
        if case == SETTINGS_CASE:
            verify_settings_sequence(entries, events)
            continue
        if case == "terrain-materials":
            terrain_materials.verify_sequence(directory, entries, events)
            continue
        if case == waterworks.CASE:
            waterworks.check_sequence(entries, events, client)
            continue
        if case == stockpile_target.CASE:
            stockpile_target.check_sequence(entries, events, client)
            continue
        if case == "refactor-rows":
            refactor_rows.check_sequence(entries)
            continue
        if case == "refactor-pinned-rename":
            refactor_rows.check_pin_sequence(entries, events, client)
            continue
        if case == tank_completion.GHOST_CASE:
            continue  # Dedicated raw/domain verification ran above.
        if case == "progress-bars":
            progress_bars.check_sequence(entries)
            continue
        if case == "bridge-planning":
            bridge_planning.check_sequence(entries)
            continue
        if case in SPA_CASES:
            continue  # Dedicated raw/durable commit checks ran above.
        if case in COMPLETION_CASES:
            continue
        if case in PLANNING_CASES:
            continue  # Exact ordinary 2D consumers and input checks ran above.
        if case in ("rest-area-planning", "rest-area-refund"):
            rest_area_planning.check_sequence(entries, refund=case == "rest-area-refund")
            continue
        if case in REST_CASES and case not in ("rest-area-planning", "rest-area-refund"):
            continue  # Exact sequence/domain/raw checks ran together above.
        if layout_only:
            continue
        history = next(item["value"] for item in entries if item["case"] == "history")
        oldest = next(item["value"] for item in entries if item["case"] == "history-oldest")
        native.require(history["camera"] == oldest["camera"] and history["selected"] == oldest["selected"], "history wheel leaked into world")
        native.require(min(oldest["visible_history"]) < min(history["visible_history"]), "older history entries were not reached")
        joint.verify_game_log(directory / "game.log", "Intel")
    return {"status": "valid", "profile": PROFILE, "sessions": len(expected),
            "coverage": f"{manifest.get('layout_scene', 'management')} rendering only; no input evidence" if layout_only else
                        "Waterworks contract at two initial Yard sites, ordinary construction, water delivery and claim" if manifest.get("case") == waterworks.CASE else
                        "five settings sliders changed through actual pointer drags and retained when reopening the settings panel" if manifest.get("case") == SETTINGS_CASE else
                        "ordinary adopted Full Tank, five returned buckets and exact Manual1 save/load remap plus rebuilt candidate rendering only; no Memory, performance, art approval or release authority" if manifest.get("case") == tank_persistence.CASE else
                        "ordinary Tank candidate and base-presentation Mixer construction, one exact River-to-Tank-to-Mixer refinement leg only; no whole lifecycle, performance, Mixer art or release authority" if manifest.get("case") == tank_supply.CASE else
                        "ordinary adopted Tank construction and actual pours to capacity fifty with Full water presentation only; no River-fill, Mixer supply, performance or release" if manifest.get("case") == tank_full.CASE else
                        "ordinary adopted Tank construction and actual Tank pour with water presentation only; no River-fill, full supply lifecycle, performance or release" if manifest.get("case") == tank_inflow.CASE else
                        "ordinary adopted Tank companion placement, Wood delivery and natural construction only; no water supply, full lifecycle, performance or release" if manifest.get("case") == tank_completion.CASE else
                        "ordinary adopted Mixer candidate Wood delivery and natural construction with body/rotor consumption only; no refinement, full lifecycle, performance or release" if manifest.get("case") == m2_completion.CASE else
                        "ordinary adopted Mixer candidate planning consumers and one Blueprint input only; no completed lifecycle, art decision, performance or release" if manifest.get("case") in m2.KINDS else
                        "ordinary M5 exact-case source catalog, legal/occupied ghost, Blueprint/pulse and same-owner inspection only; no completion, new art approval, power, performance or release" if manifest.get("case") in m5.KINDS else
                        "ordinary isolated-candidate SoulSpa load of historical occupied-save input, current remap/reset/reassignment, Normal consumption and observed departure only; no current construction/domain end receipt/performance/release authority" if manifest.get("case") == spa_loaded.CASE else
                        "ordinary isolated-candidate SoulSpa occupied restoration, remapped former-worker reassignment, measured consumption and observed Dream-floor departure; no domain end receipt/performance/release authority" if manifest.get("case") == spa_resumed.CASE else
                        "ordinary isolated-candidate SoulSpa body/four-slot consumers and occupied durable restoration/runtime reset; not resumed assignment/performance/release acceptance" if manifest.get("case") == spa_art.CASE else
                        "ordinary occupied SoulSpa paused save/load durable identity and runtime reset only; no resumed assignment/art/performance/release acceptance" if manifest.get("case") == "soul-spa-occupied-save" else
                        "seeded occupied SoulSpa slot changes and bounded non-eviction; no natural assignment, persistence, art or performance acceptance" if manifest.get("case") == refactor_spa.CASE else
                        "ordinary SoulSpa occupied 4-to-zero slot changes and bounded non-eviction only; no occupied persistence/art/performance/release acceptance" if manifest.get("case") == "soul-spa-slots" else
                        "ordinary empty Operational SoulSpa exact-byte save/load and durable remapping only; no occupied persistence/art/performance/release acceptance" if manifest.get("case") == "soul-spa-persistence" else
                        "ordinary SoulSpa natural construction, generating worker and Dream consumption only; no art/performance/release acceptance" if manifest.get("case") == "soul-spa-usage" else
                        "ordinary SoulSpa natural Bone delivery and Operational transition only; no art/usage/performance/release acceptance" if manifest.get("case") == "soul-spa-completion" else
                        "ordinary unfilled SoulSpa placement, exact-byte save/load and confirmed cancellation; no art/usage/performance/release acceptance" if manifest.get("case") == "soul-spa-planning" else
                        "ordinary RestArea farthest-zoom type/utilization hover and selection feedback; not art/numeric/performance/release acceptance" if manifest.get("case") == "rest-area-maxzoom" else
                        "task labels, rename, search/fold and four progress callers with save/load/cancellation" if manifest.get("case") == "refactor-suite" else
                        "three visible terrain LODs, diagnostic normal prepass, Scene resize and restore" if manifest.get("case") == "terrain-materials" else
                        "task labels, rename, search and fold feedback" if manifest.get("case") == "refactor-rows" else
                        "pinned target, selection change and rename feedback" if manifest.get("case") == "refactor-pinned-rename" else
                        "installed release Door/Tank ghost, companion cancellation and Wall selection" if manifest.get("case") == tank_completion.GHOST_CASE else
                        "four progress callers, simulation completion, save/load and cancellation feedback" if manifest.get("case") == "progress-bars" else
                        "ordinary generated-world Bridge placement, reservation save/load and cancellation only" if manifest.get("case") == "bridge-planning" else
                        "ordinary RestArea completion, admission and positive Dream transfer; not art/numeric/performance/release acceptance" if manifest.get("case") == "rest-area-usage" else
                        "ordinary RestArea natural exit and active cooldown; not cooldown expiry/art/numeric/performance/release acceptance" if manifest.get("case") == "rest-area-exit" else
                        "ordinary occupied RestArea save/load with actual bytes and Entity remap; not whole-world inventory/art/numeric/performance/release acceptance" if manifest.get("case") == "rest-area-save" else
                        "ordinary RestArea occupied save/load, post-load natural exit and observed cooldown expiry; not whole-world inventory/art/numeric/performance/release acceptance" if manifest.get("case") in ("rest-area-lifecycle", "rest-area-candidate-lifecycle") else
                        "ordinary RestArea real delivery and construction completion feedback; not full inventory/art/release acceptance" if manifest.get("case") == "rest-area-completion" else
                        "ordinary RestArea delivered-material save/load and refund cancellation feedback; not full inventory/art/release acceptance" if manifest.get("case") == "rest-area-refund" else
                        "ordinary generated-world RestArea reservation, save/load and cancellation only; no completed art or occupancy" if manifest.get("case") == "rest-area-planning" else
                        "navigation/layout feedback only; C05/C07/C08 and full C01-C06 acceptance remain open"}


def game_environment(repo):
    environment = native.cargo_environment(repo)
    # AdapterInfo is INFO-level renderer evidence. An inherited RUST_LOG=warn
    # must not silently remove it; warnings/errors remain visible and rejected.
    environment["RUST_LOG"] = "info,wgpu=error"
    return environment


def cancellation_observation_setting(directory, nonce, admitted_identities=None):
    return json.dumps({"nonce": nonce, "output": str(directory / "production-cancellation.json"),
                       "admitted_identities": admitted_identities or []})


def plan(args):
    repo = native.validate_repo(args.repo)
    art_contract = planned_art(args, repo)
    ghost_release = None
    if args.case == tank_completion.GHOST_CASE:
        native.require(args.smoke and args.input_backend == "portal" and art_contract is None and args.release_codec,
                       "ghost case requires portal smoke and an installed-release codec")
        codec = rest_area_art.pipeline.no_symlinks(Path(args.release_codec))
        ghost_release = {"codec": str(codec), "codec_sha256": sha256(codec),
                         "bindings": tank_completion.ghost_bindings(repo, codec),
                         "door": joint.release_identities(repo)["door"],
                         "verification_inputs": rest_area_art.pipeline.numeric_verifier_inventory(),
                         "lifecycle_sha256": sha256(repo / "scripts/building_production_lifecycle.py")}
    else:
        native.require(not args.release_codec, "release codec is only for the installed ghost case")
    native.require(args.input_backend != "portal-eis" or art_contract is not None, "EIS is limited to ordinary art feedback")
    native.require(args.case == "navigation" or args.input_backend != "none", "refactor case requires actual input")
    native.require(not args.portal_consent_file or args.input_backend in ("portal", "portal-eis"),
                   "consent file requires portal input backend")
    resources = native.resource_snapshot(repo, require_launcher=True)
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, PROFILE)
    native.require(root.is_relative_to(repo / "target/native-acceptance") and not root.exists(), "requires fresh native job root")
    native.require_persistent_storage(root, label="UI feedback job")
    subject, source, harness = native.git_subject(repo), native.source_fingerprint(repo), native.native_harness_fingerprint(repo)
    command = ["kitty", "--directory", str(repo), "--detach", "env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
               "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()), "run", "--repo", str(repo),
               "--job-root", str(root), "--subject-commit", subject, "--source-fingerprint", source,
               "--harness-fingerprint", harness, "--input-backend", args.input_backend,
               "--case", args.case,
               "--layout-scene", args.layout_scene,
               *(["--portal-consent-file", args.portal_consent_file] if args.portal_consent_file else []),
               *(["--release-codec", ghost_release["codec"], "--release-codec-sha256", ghost_release["codec_sha256"],
                   "--release-contract-sha256", hashlib.sha256(json.dumps(ghost_release, sort_keys=True).encode()).hexdigest()] if ghost_release else []),
               *(["--spa-resume-source", art_contract["resume_input"]["root"]] if args.case == spa_loaded.CASE else []),
               *(["--m2-art-source" if args.case in (*m2.KINDS, *COMPLETION_CASES) else "--m5-art-source" if args.case in m5.KINDS else "--soul-spa-art-source" if args.case in SPA_ART_CASES else "--rest-area-art-source", art_contract["source_root"], "--art-codec", art_contract["codec"],
                  "--art-contract-sha256", rest_area_art.contract_digest(art_contract)] if art_contract else []),
               *(["--layout-menu"] if args.layout_menu else []),
               *(["--smoke"] if args.smoke else [])]
    native.print_json({"status": "blocked" if resources["failures"] else "ready", "profile": PROFILE,
                       "job_root": str(root), "subject_commit": subject, "source_fingerprint": source,
                       "harness_fingerprint": harness, "resources": resources, "failures": resources["failures"],
                       "launcher_command": command,
                       "verify_command": ["python3", str(Path(__file__).resolve()), "verify", "--job-root", str(root)]})
    return int(bool(resources["failures"]))


@native.activity_locked
def run(args):
    repo, root = native.validate_repo(args.repo), Path(args.job_root).resolve()
    art_contract = planned_art(args, repo)
    ghost_release = None
    if args.case == tank_completion.GHOST_CASE:
        native.require(args.smoke and args.input_backend == "portal" and art_contract is None and args.release_codec,
                       "ghost case requires portal smoke and an installed-release codec")
        codec = rest_area_art.pipeline.no_symlinks(Path(args.release_codec))
        ghost_release = {"codec": str(codec), "codec_sha256": sha256(codec),
                         "bindings": tank_completion.ghost_bindings(repo, codec),
                         "door": joint.release_identities(repo)["door"],
                         "verification_inputs": rest_area_art.pipeline.numeric_verifier_inventory(),
                         "lifecycle_sha256": sha256(repo / "scripts/building_production_lifecycle.py")}
    else:
        native.require(not args.release_codec, "release codec is only for the installed ghost case")
    native.require(args.input_backend != "portal-eis" or art_contract is not None, "EIS is limited to ordinary art feedback")
    native.require(ghost_release is None or (args.release_codec_sha256 == ghost_release["codec_sha256"]
                   and args.release_contract_sha256 == hashlib.sha256(json.dumps(ghost_release, sort_keys=True).encode()).hexdigest()),
                   "ghost release contract changed")
    native.require(art_contract is None or getattr(args, "art_contract_sha256", None),
                   "ordinary art run requires the frozen plan digest")
    native.require(not args.portal_consent_file or args.input_backend in ("portal", "portal-eis"),
                   "consent file requires portal input backend")
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use planned kitty launcher")
    native.require(args.case == "navigation" or args.input_backend != "none", "refactor case requires actual input")
    native.require(not args.layout_menu or args.input_backend == "none", "layout menu requires no-input rendering")
    native.require(not args.layout_menu or args.layout_scene == "management", "layout menu requires management scene")
    native.require(native.git_subject(repo) == args.subject_commit and native.source_fingerprint(repo) == args.source_fingerprint
                   and native.native_harness_fingerprint(repo) == args.harness_fingerprint, "frozen inputs changed")
    native.require(root.is_relative_to(repo / "target/native-acceptance") and not root.exists(), "invalid job root")
    native.require_persistent_storage(root, label="UI feedback job")
    resources = native.resource_snapshot(repo, require_launcher=True)
    native.require(not resources["failures"], f"resource preflight: {resources['failures']}")
    root.mkdir(parents=True)
    state = {"status": "running", "profile": PROFILE, "current_stage": "build", "child_pid": None,
             "heartbeat_at": native.utc_now(), "commands": []}
    job_file = root / "job.json"
    native.atomic_write_json(job_file, state)
    portal = None
    try:
        native.run_command("build", ["python3", "scripts/dev.py", "feedback", "--build-only"], repo=repo,
                           env=native.cargo_environment(repo), log_path=root / "build.log", job_file=job_file,
                           state=state, timeout_seconds=3600)
        sessions = []
        if args.input_backend in ("portal", "portal-eis"):
            def portal_heartbeat(method):
                state.update(current_stage=f"portal-{method}", heartbeat_at=native.utc_now())
                native.atomic_write_json(job_file, state)
            portal = PortalSession(portal_heartbeat, consent_file=args.portal_consent_file)
        for index, (case, width, height, scale) in enumerate(session_matrix(args.case, args.smoke)):
            directory = root / f"viewport-{index}"
            directory.mkdir()
            nonce = secrets.token_hex(16)
            environment = game_environment(repo)
            for key in tuple(environment):
                if key.startswith(("HW_NATIVE_", "HW_PERF_", "HW_WALL_", "HW_DOOR_")) or key in joint.ENV_KEYS:
                    environment.pop(key, None)
                if key.startswith(("HW_BUILDING_", "HW_M2_", "HW_BRIDGE_ACCEPTANCE_")):
                    environment.pop(key, None)
            environment.update({"BEVY_ASSET_ROOT": str(repo), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
                                "WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": "Intel", "HELL_WORKERS_WORLDGEN_SEED": "20260914",
                                "HW_NATIVE_UI_ROOT": str(directory), "HW_NATIVE_UI_NONCE": nonce,
                                "HW_NATIVE_UI_WIDTH": str(width), "HW_NATIVE_UI_HEIGHT": str(height), "HW_NATIVE_UI_SCALE": str(scale)})
            environment["HW_NATIVE_UI_CASE"] = case
            if case == tank_supply.CASE:
                environment["HELL_WORKERS_WORLDGEN_SEED"] = str(art_contract["worldgen"]["seed"])
            if case == tank_completion.GHOST_CASE:
                bindings = ghost_release["bindings"]
                environment["HW_BUILDING_ASSET_RELEASES"] = json.dumps(bindings)
                environment["HW_BUILDING_PRODUCTION_OBSERVATIONS"] = cancellation_observation_setting(
                    directory, nonce, [row["identity"] for row in bindings])
            if case in COMPLETION_CASES:
                view = completion_recipe(case).prepare_view(repo, directory, art_contract)
                environment["BEVY_ASSET_ROOT"] = str(view)
                environment["HW_BUILDING_ORDINARY_ART_SESSION"] = completion_recipe(case).session_setting(directory, nonce, art_contract)
            if case in PLANNING_CASES:
                view = planning_recipe(case).prepare_view(repo, directory, art_contract)
                environment["BEVY_ASSET_ROOT"] = str(view)
                environment["HW_BUILDING_ORDINARY_ART_SESSION"] = planning_recipe(case).session_setting(directory, nonce, art_contract)
            if case in SPA_CASES:
                if case in SPA_ART_CASES:
                    view = spa_art.prepare_view(repo, directory, art_contract)
                    if case == spa_loaded.CASE:
                        spa_loaded.prepare_input(repo, directory, art_contract)
                    environment["BEVY_ASSET_ROOT"] = str(view)
                    environment["HW_BUILDING_ORDINARY_ART_SESSION"] = spa_art.session_setting(directory, nonce, art_contract)
                else:
                    environment["HW_BUILDING_PRODUCTION_OBSERVATIONS"] = cancellation_observation_setting(directory, nonce)
            if case in REST_CASES:
                if art_contract is not None:
                    view = rest_area_art.prepare_view(repo, directory, art_contract)
                    environment["BEVY_ASSET_ROOT"] = str(view)
                    environment["HW_BUILDING_ORDINARY_ART_SESSION"] = rest_area_art.session_setting(directory, nonce, art_contract)
                else:
                    environment["HW_BUILDING_PRODUCTION_OBSERVATIONS"] = cancellation_observation_setting(directory, nonce)
            if case in ("refactor-rows", "refactor-pinned-rename"):
                # Use winit's local XIM backend for this ASCII rename/search case.
                # Keep the desktop's IME and every other process unchanged.
                environment["XMODIFIERS"] = "@im=local"
            if args.input_backend == "none":
                environment["HW_NATIVE_UI_LAYOUT_ONLY"] = "1"
                environment["HW_NATIVE_UI_LAYOUT_SCENE"] = args.layout_scene
                if args.layout_menu:
                    environment["HW_NATIVE_UI_LAYOUT_MENU"] = "1"
            state["current_stage"] = f"viewport-{index}"
            native.admit_stage_start(state["current_stage"], state=state, job_file=job_file)
            with (directory / "game.log").open("w") as log:
                process = subprocess.Popen([str(repo / "target/debug/bevy_app")], cwd=repo, env=environment,
                                           stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                                           pass_fds=native.activity_pass_fds(environment))
                state["child_pid"] = process.pid
                native.atomic_write_json(job_file, state)
                driver = None
                try:
                    driver = Driver(process, directory, nonce, state, job_file, portal,
                                    layout_only=args.input_backend == "none", eis=args.input_backend == "portal-eis",
                                    scroll=case == "rest-area-maxzoom", fresh_ack=case in ("soul-spa-slots", refactor_spa.CASE))
                    if art_contract is not None:
                        rest_area_art.wait_ready(driver, art_contract)
                    state["current_stage"] = "ui-input"
                    if args.input_backend == "none":
                        driver.capture("entities")
                    elif case == SETTINGS_CASE:
                        exercise_settings(driver)
                    elif case == tank_completion.GHOST_CASE:
                        tank_completion.exercise_ghosts(driver)
                    elif case in COMPLETION_CASES:
                        completion_recipe(case).exercise(driver, art_contract)
                    elif case in PLANNING_CASES:
                        planning_recipe(case).exercise(driver, art_contract)
                    elif case == waterworks.CASE:
                        waterworks.exercise(driver)
                    elif case == stockpile_target.CASE:
                        stockpile_target.exercise(driver)
                    elif case == "refactor-rows":
                        refactor_rows.exercise(driver)
                    elif case == "refactor-pinned-rename":
                        refactor_rows.exercise_pin(driver)
                    elif case == "progress-bars":
                        progress_bars.exercise(driver)
                    elif case == "bridge-planning":
                        bridge_planning.exercise(driver)
                    elif case == "soul-spa-planning":
                        spa_planning.exercise(driver)
                    elif case == "soul-spa-completion":
                        spa_planning.exercise_completion(driver)
                    elif case == "soul-spa-usage":
                        spa_planning.exercise_usage(driver)
                    elif case == "soul-spa-persistence":
                        spa_planning.exercise_persistence(driver)
                    elif case == spa_loaded.CASE:
                        spa_loaded.exercise(driver, art_contract)
                    elif case == spa_resumed.CASE:
                        spa_resumed.exercise(driver)
                    elif case in ("soul-spa-occupied-save", spa_art.CASE):
                        spa_occupied.exercise(driver)
                    elif case == refactor_spa.CASE:
                        refactor_spa.exercise(driver)
                    elif case == "soul-spa-slots":
                        spa_planning.exercise_slots(driver)
                    elif case == "rest-area-maxzoom":
                        rest_area_maxzoom.exercise(driver)
                    elif case in ("rest-area-lifecycle", "rest-area-candidate-lifecycle"):
                        if art_contract is not None:
                            rest_area_art.exercise_lifecycle(driver, candidate=case == "rest-area-candidate-lifecycle")
                        else:
                            rest_area_usage.exercise_lifecycle(driver)
                    elif case == "rest-area-save":
                        rest_area_usage.exercise_save(driver)
                    elif case == "rest-area-exit":
                        rest_area_usage.exercise_exit(driver)
                    elif case == "rest-area-usage":
                        rest_area_usage.exercise(driver)
                    elif case in ("rest-area-planning", "rest-area-refund", "rest-area-completion"):
                        rest_area_planning.exercise(driver, refund=case == "rest-area-refund",
                                                    completion=case == "rest-area-completion")
                    elif case == "terrain-materials":
                        terrain_materials.exercise(driver)
                    else:
                        driver.exercise()
                except Exception:
                    owned = native.x11_client_windows_for_process_tree(process.pid)
                    if len(owned) == 1:
                        try:
                            native.capture_client_png(owned[0][0], directory / "failure-client.png", import_cmd="import")
                        except (native.AcceptanceError, OSError):
                            pass
                    raise
                finally:
                    try:
                        if driver and driver.input:
                            driver.input.close()
                    finally:
                        if process.poll() is None:
                            native.stop_command_process(process)
                        state["child_pid"] = None
            joint.verify_game_log(directory / "game.log", "Intel")
            session = {"case": case, "directory": directory.name, "nonce": nonce, "log_sha256": sha256(directory / "game.log")}
            if case in PLANNING_CASES or case in COMPLETION_CASES or case == tank_completion.GHOST_CASE:
                session["cancellation_sha256"] = sha256(directory / "production-cancellation.json")
            if case in SPA_CASES:
                session["cancellation_sha256"] = sha256(directory / "production-cancellation.json")
            if case in REST_CASES:
                session["cancellation_sha256"] = sha256(directory / "production-cancellation.json")
            sessions.append(session)
        native.atomic_write_json(root / "manifest.json", {
            "profile": PROFILE, "evidence_kind": "feedback", "repo": str(repo), "smoke": args.smoke,
            "input_backend": args.input_backend,
            "portal_session": portal.session if portal else None,
            "portal_persistence": portal.persistence if portal else "disabled",
            "case": args.case,
            "ordinary_art": art_contract,
            "ghost_release": ghost_release,
            "layout_menu": args.layout_menu,
            "layout_scene": args.layout_scene,
            "subject_commit": args.subject_commit, "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint, "binary_sha256": sha256(repo / "target/debug/bevy_app"), "sessions": sessions})
        native.print_json(verify_root(root))
        state.update(status="valid", completed_at=native.utc_now())
    except Exception as error:
        state.update(status="invalid", failure=f"{type(error).__name__}: {error}", completed_at=native.utc_now())
        raise
    finally:
        if portal:
            portal.close()
        native.atomic_write_json(job_file, state)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    for command in ("plan", "run", "status", "verify"):
        child = subparsers.add_parser(command)
        child.add_argument("--job-root", required=command != "plan")
        if command in ("plan", "run"):
            child.add_argument("--repo", required=True)
            child.add_argument("--smoke", action="store_true")
            child.add_argument("--case", choices=CASES, default="navigation")
            child.add_argument("--layout-menu", action="store_true")
            child.add_argument("--layout-scene", choices=("management", "normal", "selection", "pinned", "build", "display", "area", "area-details"), default="management")
            child.add_argument("--input-backend", choices=("xtest", "portal", "portal-eis", "none"), default="xtest")
            child.add_argument("--portal-consent-file", help="explicit owner-only restore-token journal; never a consent bypass")
            child.add_argument("--rest-area-art-source", help="explicit RestArea ArtPreview, or approved isolated candidate only in candidate-lifecycle; no new approval")
            child.add_argument("--soul-spa-art-source", help="approved isolated SoulSpa candidate only in its occupied-save case; no new approval")
            child.add_argument("--spa-resume-source", help="verified occupied-save job as immutable historical input in loaded-use only")
            child.add_argument("--m5-art-source", help="one exact M5 source in its dedicated preview or adopted-candidate planning case; no approval")
            child.add_argument("--m2-art-source", help="one adopted Mixer isolated candidate in its dedicated ordinary planning case; no release")
            child.add_argument("--art-codec", help="frozen already-built codec for the ArtPreview input")
            child.add_argument("--release-codec", help="frozen codec for installed release ghost verification")
        if command == "run":
            child.add_argument("--art-contract-sha256")
            child.add_argument("--release-codec-sha256")
            child.add_argument("--release-contract-sha256")
        if command == "run":
            for option in ("subject-commit", "source-fingerprint", "harness-fingerprint"):
                child.add_argument(f"--{option}", required=True)
    args = parser.parse_args()
    if args.command in ("plan", "run") and args.case == "terrain-materials":
        native.require(args.smoke and args.input_backend != "none", "terrain-materials requires --smoke and owned-window resize input")
    if args.command == "plan":
        return plan(args)
    if args.command == "run":
        return run(args)
    if args.command == "verify":
        native.print_json(verify_root(Path(args.job_root).resolve()))
        return 0
    state = native.read_json(Path(args.job_root).resolve() / "job.json")
    if state["status"] == "running":
        age = (datetime.now(UTC) - native.parse_utc(state["heartbeat_at"])).total_seconds()
        native.require(0 <= age <= 120, "UI feedback heartbeat is stale")
    native.print_json(state)
    return 2 if state["status"] == "running" else int(state["status"] != "valid")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (native.AcceptanceError, OSError, ValueError) as error:
        print(f"UI feedback failed: {error}", file=sys.stderr)
        raise SystemExit(1)
