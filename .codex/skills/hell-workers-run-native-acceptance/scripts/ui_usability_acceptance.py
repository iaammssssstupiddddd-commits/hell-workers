#!/usr/bin/env python3
"""OS-driven UI feedback. This bounded subset is not full P1/formal acceptance."""
from __future__ import annotations

import argparse
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

sys.path.insert(0, str(Path(__file__).resolve().parents[4]))
from scripts.native_ui_input import X11Input
from scripts.native_ui_portal import PortalSession, PortalX11Input

PROFILE = "ui-usability-feedback"
CHECKPOINTS = ("tasks", "last-page", "minimized", "restored", "entities",
               "paused-tooltip", "settings", "settings-scrolled", "settings-closed",
               "history", "history-oldest", "history-closed", "guide", "guide-closed")
VIEWPORTS = ((1920, 1080, 1.0), (1280, 720, 1.25), (1280, 720, 0.85),
             (1280, 720, 1.0), (1920, 1080, 0.85), (1920, 1080, 1.25))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_observation(case: str, value: dict) -> None:
    native.require(value["ready"] and value["frame"] > 90, "fixture is not ready")
    native.require(value["task_rows"] <= 20, "task resident row limit exceeded")
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
    native.require(value.get("list_text") and any(
        item["text"].strip() and item["rect"] is not None for item in value["list_text"]
    ), "Familiar header text has no visible layout")
    native.require(value["paused"] and "text-field:DevPoc" not in value["controls"],
                   "render fixture must be paused with developer body hidden")
    if value.get("menu_state") == "Zones":
        menu = value.get("zones_rect")
        hint = value["mode_presentation"]["rect"]
        native.require(menu and hint and menu[3] <= hint[1], "submenu obscures mode guidance")


class Driver:
    def __init__(self, process, root, nonce, state, job_file, portal=None, *, layout_only=False):
        self.process, self.root, self.nonce = process, root, nonce
        self.state, self.job_file = state, job_file
        self.events, self.checkpoints = [], []
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
        self.input = PortalX11Input(*arguments, portal=portal) if portal else X11Input(*arguments)
        try:
            self.input.activate()
            geometry = self.input.client_size()
            native.atomic_write_json(self.root / "input-client.json", {"window": self.input.window,
                "pid": pid, "geometry": geometry, "observer_viewport": self.last["viewport"], "nonce": nonce,
                "input_backend": "portal" if portal else "xtest",
                "portal_devices": portal.granted_devices if portal else None,
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
            return True
        self.last = self.wait(acknowledged, after=self.last["frame"] + 3)
        self.events[-1]["post_frame"] = self.last["frame"]
        self.events[-1]["observed_cursor"] = self.last["cursor"]
        self.events[-1]["observed_left_pressed"] = self.last["mouse_left_pressed"]
        self.events[-1]["observed_hover"] = {key: value["interaction"] for key, value in self.last["controls"].items()
                                           if value["interaction"] in ("Hovered", "Pressed")}
        native.atomic_write_json(self.root / "events.json", {"events": self.events})

    def point(self, key):
        native.require(key in self.last["controls"], f"missing visible input target: {key}")
        rect = self.last["controls"][key]["rect"]
        return (round((rect[0] + rect[2]) / 2), round((rect[1] + rect[3]) / 2))

    def click(self, key):
        point = self.point(key)
        self.send(point=point)
        native.require(self.last["controls"].get(key, {}).get("interaction") == "Hovered",
                       f"target is occluded or not hovered: {key}")
        self.send(button=1, pressed=True)
        native.require(self.last["controls"].get(key, {}).get("interaction") == "Pressed",
                       f"target did not receive press: {key}")
        self.send(button=1, pressed=False)
        native.require(self.last["cursor"] is not None and all(abs(a-b) <= 2 for a,b in zip(self.last["cursor"], point)),
                       f"pointer moved outside click target during gesture: {key}")

    def key(self, key):
        self.send(key=key, pressed=True)
        self.send(key=key, pressed=False)

    def scroll(self, key, amount):
        self.send(point=self.point(key))
        for _ in range(amount):
            self.send(button=5, pressed=True)
            self.send(button=5, pressed=False)

    def capture(self, case):
        self.last = self.wait(lambda value: True, after=self.last["frame"] + 6)
        path = self.root / f"{case}.png"
        owned = native.x11_client_windows_for_process_tree(self.process.pid)
        native.require(len(owned) == 1 and int(owned[0][0], 0) == self.window
                       and owned[0][1] == self.owner_pid, "capture owner changed")
        native.require(native.capture_client_png(hex(self.window), path, import_cmd="import"), "client capture failed")
        dimensions = native.validate_png_structure(path.read_bytes())
        native.require(list(dimensions) == self.last["viewport"], "capture dimensions differ")
        try:
            check_observation(case, self.last)
            if self.input is None:
                check_entities_render(self.last)
        except native.AcceptanceError:
            native.atomic_write_json(self.root / "failed-checkpoint.json", {"case": case, "value": self.last})
            raise
        observation = {"case": case, "value": self.last, "last_step": len(self.events),
                       "screenshot": path.name, "sha256": sha256(path),
                       "capture_scope": "x11-client-window", "window": self.window,
                       "pid": self.owner_pid}
        self.checkpoints.append(observation)
        native.atomic_write_json(self.root / "checkpoints.json", {"checkpoints": self.checkpoints})

    def exercise(self):
        self.click("dev-minimize")
        self.last = self.wait(lambda value: "text-field:DevPoc" not in value["controls"], after=self.last["frame"])
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


def verify_root(root):
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest["profile"] == PROFILE and manifest["evidence_kind"] == "feedback", "unsupported evidence kind")
    native.require(manifest.get("input_backend") in ("portal", "xtest", "none"), "input backend missing")
    layout_only = manifest["input_backend"] == "none"
    repo = native.validate_repo(manifest["repo"])
    native.require(native.source_fingerprint(repo) == manifest["source_fingerprint"], "source changed")
    native.require(native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"], "harness changed")
    native.require(native.git_subject(repo) == manifest["subject_commit"], "subject changed")
    native.require(sha256(repo / "target/debug/bevy_app") == manifest["binary_sha256"], "binary changed")
    expected = VIEWPORTS[:1] if manifest["smoke"] else VIEWPORTS
    native.require(len(manifest["sessions"]) == len(expected), "viewport coverage missing")
    for session, (width, height, scale) in zip(manifest["sessions"], expected):
        directory = root / session["directory"]
        native.require(directory.parent == root, "session path escapes job")
        entries = native.read_json(directory / "checkpoints.json")["checkpoints"]
        events = native.read_json(directory / "events.json")["events"]
        client = native.read_json(directory / "input-client.json")
        native.require(client["input_backend"] == manifest["input_backend"] and client["nonce"] == session["nonce"],
                       "input client/backend differs")
        if manifest["input_backend"] == "portal":
            native.require(client["portal_devices"] & 3 == 3 and client["monitor_mapping"][1] > 0,
                           "portal consent or monitor mapping missing")
        native.require([item["case"] for item in entries] == (["entities"] if layout_only else list(CHECKPOINTS)), "checkpoints differ")
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
                check_entities_render(value)
                native.require(value.get("menu_state") == ("Zones" if manifest.get("layout_menu") else "Hidden"),
                               "layout menu fixture differs")
            native.require(value["nonce"] == session["nonce"] and value["viewport"] == [width, height]
                           and abs(value["ui_scale"] - scale) < 0.001, "viewport/nonce differs")
            native.require(item["pid"] == value["pid"] and item["capture_scope"] == "x11-client-window", "capture scope differs")
            image = directory / item["screenshot"]
            native.require(image.parent == directory and sha256(image) == item["sha256"], "screenshot changed")
            native.require(native.validate_png_structure(image.read_bytes()) == (width, height), "image dimensions differ")
        native.require(sha256(directory / "game.log") == session["log_sha256"], "game log changed")
        joint.verify_game_log(directory / "game.log", "Intel")
        if layout_only:
            continue
        history = next(item["value"] for item in entries if item["case"] == "history")
        oldest = next(item["value"] for item in entries if item["case"] == "history-oldest")
        native.require(history["camera"] == oldest["camera"] and history["selected"] == oldest["selected"], "history wheel leaked into world")
        native.require(min(oldest["visible_history"]) < min(history["visible_history"]), "older history entries were not reached")
        joint.verify_game_log(directory / "game.log", "Intel")
    return {"status": "valid", "profile": PROFILE, "sessions": len(expected),
            "coverage": "Entities rendering only; no input evidence" if layout_only else
                        "navigation/layout feedback only; C05/C07/C08 and full C01-C06 acceptance remain open"}


def plan(args):
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, PROFILE)
    native.require(root.is_relative_to(repo / "target/native-acceptance") and not root.exists(), "requires fresh native job root")
    native.require_persistent_storage(root, label="UI feedback job")
    subject, source, harness = native.git_subject(repo), native.source_fingerprint(repo), native.native_harness_fingerprint(repo)
    command = ["kitty", "--directory", str(repo), "--detach", "env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
               "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()), "run", "--repo", str(repo),
               "--job-root", str(root), "--subject-commit", subject, "--source-fingerprint", source,
               "--harness-fingerprint", harness, "--input-backend", args.input_backend,
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
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use planned kitty launcher")
    native.require(not args.layout_menu or args.input_backend == "none", "layout menu requires no-input rendering")
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
        if args.input_backend == "portal":
            def portal_heartbeat(method):
                state.update(current_stage=f"portal-{method}", heartbeat_at=native.utc_now())
                native.atomic_write_json(job_file, state)
            portal = PortalSession(portal_heartbeat)
        for index, (width, height, scale) in enumerate(VIEWPORTS[:1] if args.smoke else VIEWPORTS):
            directory = root / f"viewport-{index}"
            directory.mkdir()
            nonce = secrets.token_hex(16)
            environment = native.cargo_environment(repo)
            for key in tuple(environment):
                if key.startswith(("HW_NATIVE_", "HW_PERF_", "HW_WALL_", "HW_DOOR_")) or key in joint.ENV_KEYS:
                    environment.pop(key, None)
            environment.update({"BEVY_ASSET_ROOT": str(repo), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
                                "WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": "Intel", "HELL_WORKERS_WORLDGEN_SEED": "20260914",
                                "HW_NATIVE_UI_ROOT": str(directory), "HW_NATIVE_UI_NONCE": nonce,
                                "HW_NATIVE_UI_WIDTH": str(width), "HW_NATIVE_UI_HEIGHT": str(height), "HW_NATIVE_UI_SCALE": str(scale)})
            if args.input_backend == "none":
                environment["HW_NATIVE_UI_LAYOUT_ONLY"] = "1"
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
                                    layout_only=args.input_backend == "none")
                    state["current_stage"] = "ui-input"
                    if args.input_backend == "none":
                        driver.capture("entities")
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
            sessions.append({"directory": directory.name, "nonce": nonce, "log_sha256": sha256(directory / "game.log")})
        native.atomic_write_json(root / "manifest.json", {
            "profile": PROFILE, "evidence_kind": "feedback", "repo": str(repo), "smoke": args.smoke,
            "input_backend": args.input_backend,
            "layout_menu": args.layout_menu,
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
            child.add_argument("--layout-menu", action="store_true")
            child.add_argument("--input-backend", choices=("xtest", "portal", "none"), default="xtest")
        if command == "run":
            for option in ("subject-commit", "source-fingerprint", "harness-fingerprint"):
                child.add_argument(f"--{option}", required=True)
    args = parser.parse_args()
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
