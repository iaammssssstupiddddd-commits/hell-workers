"""Technical-only ordinary-world portal input/raw witness probe for TAK-14.

The primary validation coordinator owns this direct child. This probe neither
admits a production recipe nor claims lifecycle, art, performance or release
acceptance. It proves only that one owned normal-world window reports a real
bounded key press and release in the product's raw observation stream.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import time

if __package__:
    from . import building_production_local_probe as probe
    from . import building_production_validation_context as ownership
    from .native_ui_portal import PortalSession, PortalX11Input
else:
    import building_production_local_probe as probe
    import building_production_validation_context as ownership
    from scripts.native_ui_portal import PortalSession, PortalX11Input

native = probe.native
PROFILE = "building-production-local-input-probe-portal-v2"
CATALOG_PROFILE = "building-production-local-architect-portal-v1"
OWNER = "TAK-14 normal-development coordinator"
KEY = "F12"
WINDOW = (1280, 720)
CONSENT_RELATIVE = Path(".git/validation-storage/input-consent/remote-desktop.json")


def require(value: bool, reason: str) -> None:
    probe.require(value, reason)


def command(plan: Path, repo: Path, root: Path) -> list[str]:
    return ["env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1",
            "python3", str(Path(__file__).resolve()), "run", "--plan", str(plan),
            "--repo", str(repo), "--job-root", str(root)]


def verifier(root: Path) -> list[str]:
    return ["python3", str(Path(__file__).resolve()), "verify", "--job-root", str(root)]


def check_plan(value: dict, plan_path: Path, *, fresh: bool = True,
               plan_exists: bool = True) -> tuple[Path, Path, Path]:
    fields = {"schema", "profile", "repo", "job_root", "head", "source",
                           "harness", "driver", "input_transport_sha256",
                           "portal_transport_sha256", "consent_file", "binary",
                           "binary_sha256", "owner", "key", "accepted",
                           "performance_evidence", "art_approved", "promotion_authority"}
    catalog = value.get("profile") == CATALOG_PROFILE
    require(set(value) == (fields | {"scenario"} if catalog else fields),
            "input probe plan shape differs")
    repo = native.validate_repo(value["repo"])
    root, binary = Path(value["job_root"]), Path(value["binary"])
    for path in (plan_path, root, binary, Path(value["consent_file"])):
        ownership.exact_path(path)
    require(value["schema"] == 1 and value["profile"] in {PROFILE, CATALOG_PROFILE}
            and (not catalog or value.get("scenario") == "architect-cancel")
            and value["owner"] == OWNER and value["key"] == ("b" if catalog else KEY)
            and value["consent_file"] == str(repo / CONSENT_RELATIVE)
            and all(value[key] is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority")),
            "input probe cannot grant authority or change its key")
    require((not plan_exists or plan_path.is_file())
            and plan_path.parent == repo / "target/native-acceptance"
            and root.parent == repo / "target/native-acceptance"
            and root != plan_path and binary == repo / "target/profiling/bevy_app"
            and binary.is_file() and os.access(binary, os.X_OK),
            "input probe paths differ")
    require(not native.git_dirty_paths(repo)
            and value["head"] == native.git_subject(repo)
            and value["source"] == native.source_fingerprint(repo)
            and value["harness"] == native.native_harness_fingerprint(repo)
            and value["driver"] == probe.digest(Path(__file__))
            and value["input_transport_sha256"] == probe.digest(
                Path(__file__).with_name("native_ui_input.py"))
            and value["portal_transport_sha256"] == probe.digest(
                Path(__file__).with_name("native_ui_portal.py"))
            and value["binary_sha256"] == probe.digest(binary),
            "input probe frozen source, binary or transport differs")
    if fresh:
        require(not root.exists(), "input probe output root already exists")
    return repo, root, binary


def plan(args: argparse.Namespace) -> dict:
    repo = native.validate_repo(args.repo)
    root, output = Path(args.job_root).absolute(), Path(args.output).absolute()
    ownership.exact_path(root)
    ownership.exact_path(output)
    require(not root.exists() and not output.exists(), "input probe requires fresh paths")
    binary = repo / "target/profiling/bevy_app"
    require(binary.is_file(), "build the profiling-feature binary through dev.py cargo first")
    value = {"schema": 1, "profile": PROFILE, "repo": str(repo), "job_root": str(root),
             "head": native.git_subject(repo), "source": native.source_fingerprint(repo),
             "harness": native.native_harness_fingerprint(repo),
             "driver": probe.digest(Path(__file__)),
             "input_transport_sha256": probe.digest(Path(__file__).with_name("native_ui_input.py")),
             "portal_transport_sha256": probe.digest(Path(__file__).with_name("native_ui_portal.py")),
             "consent_file": str(repo / CONSENT_RELATIVE),
             "binary": str(binary), "binary_sha256": probe.digest(binary), "owner": OWNER,
             "key": KEY, "accepted": False, "performance_evidence": False,
             "art_approved": False, "promotion_authority": False}
    if getattr(args, "scenario", "debug-toggle") == "architect-cancel":
        value.update(profile=CATALOG_PROFILE, scenario="architect-cancel", key="b")
    require(root.parent == repo / "target/native-acceptance" and not native.git_dirty_paths(repo),
            "input probe requires a clean primary subject and direct output root")
    resources = native.resource_snapshot(repo, require_launcher=True)
    require(not resources["failures"], f"native resources unavailable: {resources['failures']}")
    require(native.save_catalog_window_capture_tool_error() is None,
            "ordinary-world X11 capture tools are unavailable")
    require((repo / CONSENT_RELATIVE).is_file(), "portal consent journal is unavailable")
    check_plan(value, output, plan_exists=False)
    with output.open("x") as handle:
        handle.write(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
    return {"status": "ready", "profile": value["profile"], "job_root": str(root),
            "launcher_command": ["kitty", "--directory", str(repo), "--detach",
                                 *command(output, repo, root)],
            "verify_command": verifier(root), "accepted": False,
            "performance_evidence": False, "art_approved": False,
            "promotion_authority": False}


def raw_snapshot(path: Path, nonce: str, pid: int) -> dict:
    raw = json.loads(path.read_bytes())
    probe.check_raw(raw, nonce, pid)
    require(raw["accepted"] is False and raw["performance_evidence"] is False
            and raw["promotion_authority"] is False, "input probe raw authority differs")
    return raw


def raw_ready(path: Path) -> bool:
    """Wait for the required sample count before applying final raw validation."""
    raw = json.loads(path.read_bytes())
    require(isinstance(raw, dict) and isinstance(raw.get("samples"), list)
            and len(raw["samples"]) <= 1200,
            "input probe startup raw shape differs")
    return len(raw["samples"]) >= 3


def numeric_window_id(window: str) -> int:
    require(type(window) is str and re.fullmatch(r"0x[0-9a-fA-F]+", window) is not None,
            "owned X11 window ID is not hexadecimal")
    value = int(window, 16)
    require(value > 0, "owned X11 window ID is zero")
    return value


def witnessed_input(raw: dict, *, before_event_count: int, key: str = KEY) -> tuple[dict, dict]:
    require(type(before_event_count) is int and 0 <= before_event_count <= len(raw["events"]),
            "input probe event boundary is invalid")
    rows = raw["events"][before_event_count:]
    matched = [row for row in rows if row.get("operation") in {"KeyPress", "KeyRelease"}
               and row.get("key") == key]
    require(len(matched) == 2 and [row["operation"] for row in matched]
            == ["KeyPress", "KeyRelease"]
            and matched[0]["event_index"] < matched[1]["event_index"]
            and matched[0]["sample_index"] <= matched[1]["sample_index"],
            "one ordered raw key press/release witness is missing")
    return matched[0], matched[1]


def check_portal_evidence(value: object) -> None:
    require(isinstance(value, dict)
            and set(value) == {"granted_devices", "persistence", "session"}
            and type(value["granted_devices"]) is int
            and value["granted_devices"] >= 0
            and value["granted_devices"] & 3 == 3
            and value["persistence"] in {"saved", "not-granted"}
            and isinstance(value["session"], str)
            and value["session"].startswith("/org/freedesktop/portal/desktop/session/"),
            "input probe portal evidence differs")


def wait_for_raw_event(path: Path, nonce: str, pid: int, operation: str,
                       *, after: int, timeout: float = 8.0, key: str = KEY) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        raw = raw_snapshot(path, nonce, pid)
        matched = [event for event in raw["events"][after:]
                   if event.get("operation") == operation and event.get("key") == key]
        require(len(matched) <= 1, "duplicate ordinary input event")
        if matched:
            return raw
        time.sleep(0.05)
    raise native.AcceptanceError(f"ordinary input {operation} raw witness timeout")


def wait_for_menu(path: Path, nonce: str, pid: int, expected: str, *, after: int) -> dict:
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        raw = raw_snapshot(path, nonce, pid)
        if (len(raw["samples"]) - 1 > after
                and raw["samples"][-1].get("menu_state") == expected
                and raw["samples"][-1].get("play_mode") == "Normal"
                and len(raw["samples"]) >= 2 and all(
                    row.get("menu_state") == expected
                    and row.get("architect_panel_ready") is (expected == "Architect")
                    for row in raw["samples"][-2:])):
            return raw
        time.sleep(0.05)
    raise native.AcceptanceError(f"ordinary Architect {expected} raw witness timeout")


def check_catalog_states(raw: dict, evidence: dict) -> None:
    require(set(evidence) == {"before", "opened", "opened_end", "closed", "closed_end", "close_event_count", "catalog_sha256"},
            "Architect evidence shape differs")
    indices = [evidence[key] for key in ("before", "opened", "closed")]
    require(all(type(index) is int for index in indices)
            and 0 <= indices[0] < indices[1] < indices[2] < len(raw["samples"]),
            "Architect sample order differs")
    states = [raw["samples"][index] for index in indices]
    end = evidence["opened_end"]
    require(type(end) is int and indices[1] < end < indices[2]
            and all(row.get("menu_state") == "Architect" and row.get("play_mode") == "Normal"
                    and row.get("architect_panel_ready") is True
                    and row.get("task_mode") == "None" and row.get("world_epoch") == states[0].get("world_epoch")
                    for row in raw["samples"][indices[1]:end + 1]),
            "Architect capture interval changed or stale")
    require([row.get("menu_state") for row in states] == ["Hidden", "Architect", "Hidden"]
            and all(row.get("play_mode") == "Normal" for row in states)
            and all(row.get("task_mode") == "None" for row in states)
            and len({row.get("world_epoch") for row in states}) == 1
            and states[0].get("world_epoch") is not None,
            "Architect mode/world witness differs")
    closed_end = evidence["closed_end"]
    require(type(closed_end) is int and indices[2] < closed_end < len(raw["samples"])
            and all(row.get("menu_state") == "Hidden" and row.get("play_mode") == "Normal"
                    and row.get("task_mode") == "None" and row.get("architect_panel_ready") is False
                    and row.get("world_epoch") == states[0].get("world_epoch")
                    for row in raw["samples"][indices[2] - 1:closed_end + 1]),
            "Architect closed capture interval changed or stale")


def check_registered_context(repo: Path, root: Path, plan_path: Path, context: dict) -> None:
    """Recheck live primary validation ownership, including independent sealing."""
    require(ownership.storage.primary_for(repo) == repo,
            "input probe ownership requires primary checkout")
    require(set(context) == {"schema_version", "scope", "batch", "repo", "owner", "root",
                             "head", "source", "command", "verifier", "plan_sha256",
                             "runner_pid", "runner_identity", "coordinator_pid",
                             "coordinator_identity", "accepted", "performance_evidence",
                             "art_approved", "promotion_authority"}
            and context.get("schema_version") == 1
            and context.get("scope") == "normal-validation-runner-ownership-only"
            and context.get("repo") == str(repo)
            and context.get("owner") == OWNER
            and context.get("root") == str(root)
            and context.get("command") == command(plan_path, repo, root)
            and context.get("verifier") == verifier(root)
            and context.get("plan_sha256") == probe.digest(plan_path)
            and all(context.get(key) is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority")),
            "input probe context differs")
    deadline = time.monotonic() + 2
    while True:
        with ownership.storage.locked(repo) as ledger:
            pending = ledger["batches"].get(context["batch"])
            if pending is None or pending["phase"] != "sealing":
                break
            parent_birth = ownership.storage.live_identity(os.getppid())
            own_birth = ownership.storage.live_identity(os.getpid())
            require(parent_birth is not None and own_birth is not None
                    and pending["pid"] == os.getppid()
                    and pending["process_identity"] == parent_birth,
                    "input probe sealing parent identity differs")
            child = (pending.get("child_pid"), pending.get("child_identity"))
            if child == (os.getpid(), own_birth):
                break
            require(child == (context["runner_pid"], context["runner_identity"]),
                    "input probe sealing child is foreign or partial")
        require(time.monotonic() < deadline, "input probe verifier child publication timed out")
        time.sleep(0.02)
    with ownership.storage.locked(repo) as ledger:
        batch = ledger["batches"].get(context.get("batch"))
        require(batch is not None and batch["phase"] in {"running", "sealing", "finalized"}
                and batch["repo"] == str(repo) and batch["owner"] == OWNER
                and batch["roots"] == [str(root)]
                and batch["subject"] == context.get("head")
                and batch["subject_fingerprint"] == context.get("source")
                and batch["command"] == context["command"]
                and batch["verify_command"] == context["verifier"]
                and batch["policy_sha256"] == ownership.storage.policy_hash(repo)
                and batch["coordinator_sha256"] == ownership.storage.digest(
                    Path(ownership.storage.__file__))
                and any(row["path"] == str(plan_path)
                        and row["sha256"] == context["plan_sha256"]
                        for row in batch["command_files"]),
                "input probe registered batch differs")
        if batch["phase"] == "running":
            require(batch.get("child_pid") == context.get("runner_pid")
                    and batch.get("child_identity") == context.get("runner_identity")
                    and batch.get("pid") == context.get("coordinator_pid")
                    and batch.get("process_identity") == context.get("coordinator_identity"),
                    "input probe running child differs")
        else:
            require(batch.get("exit_code") == 0,
                    "input probe runner did not exit successfully")
            if batch["phase"] == "sealing":
                own_birth = ownership.storage.live_identity(os.getpid())
                require(batch.get("child_pid") == os.getpid() and own_birth is not None
                        and batch.get("child_identity") == own_birth,
                        "input probe independent verifier identity differs")
            if batch["phase"] == "finalized":
                require(batch.get("result") == "pass", "input probe batch was not sealed pass")


@native.activity_locked
def run(args: argparse.Namespace) -> dict:
    require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use planned launcher")
    plan_path = Path(args.plan).absolute()
    value = json.loads(plan_path.read_bytes())
    repo, root, binary = check_plan(value, plan_path)
    require(repo == Path(args.repo).resolve() and root == Path(args.job_root).resolve(),
            "input probe recipe arguments differ")
    context = ownership.running_context(repo=repo, plan=plan_path, root=root,
                                        owner=OWNER, command=command(plan_path, repo, root),
                                        verifier=verifier(root))
    root.mkdir()
    runtime = root / "runtime"
    runtime.mkdir()
    nonce = secrets.token_hex(16)
    raw_path = root / "raw.json"
    environment = native.cargo_environment(repo)
    for key in tuple(environment):
        if key.startswith(("HW_", "HELL_WORKERS_WORLDGEN_")):
            environment.pop(key)
    environment.update({"BEVY_ASSET_ROOT": str(repo), "HW_WINDOW_BACKEND": "x11",
                        "HW_PRESENT_MODE": "novsync", "WGPU_BACKEND": "vulkan",
                        "WGPU_ADAPTER_NAME": "Intel", "WINIT_X11_SCALE_FACTOR": "1",
                        "RUST_LOG": "info,wgpu=error",
                        "HW_BUILDING_PRODUCTION_OBSERVATIONS": json.dumps({
                            "nonce": nonce, "output": str(raw_path), "admitted_identities": []})})
    owned = None
    records: list[dict] = []
    screenshot_before, screenshot_after = root / "before.png", root / "after.png"
    catalog = value["profile"] == CATALOG_PROFILE
    catalog_evidence = None
    metrics: list[dict] = []
    with (root / "game.log").open("w") as log:
        game = subprocess.Popen([str(binary)], cwd=runtime, env=environment, stdout=log,
                                stderr=subprocess.STDOUT, start_new_session=True)
        transport = None
        portal = None
        portal_evidence = None
        stage = "startup raw/window"
        try:
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                require(game.poll() is None, "ordinary-world game exited before input")
                windows = native.x11_client_windows_for_process_tree(game.pid)
                require(len(windows) <= 1, "ordinary-world game has multiple owned windows")
                if windows:
                    owned = windows[0]
                    metrics.append(probe.process_metrics(owned[1]))
                if owned and raw_path.is_file() and raw_ready(raw_path):
                    raw = raw_snapshot(raw_path, nonce, owned[1])
                    break
                time.sleep(0.25)
            else:
                raise native.AcceptanceError("ordinary-world process/window/raw timeout")
            window, pid = owned
            stage = "owned before capture"
            require(native.capture_client_png(window, screenshot_before, import_cmd="import"),
                    "owned X11 client before capture failed")
            before_count = len(raw["events"])
            stage = "owned portal input activation"
            window_number = numeric_window_id(window)
            portal = PortalSession(
                lambda _method: require(game.poll() is None, "ordinary-world game exited during portal consent"),
                consent_file=value["consent_file"])
            transport = PortalX11Input(window_number, pid, game.pid, nonce, records.append,
                                       portal=portal)
            transport.activate()
            require(portal.ready and portal.granted_devices >= 0
                    and portal.granted_devices & 3 == 3,
                    "portal keyboard/pointer grant is unavailable")
            portal_evidence = {"granted_devices": portal.granted_devices,
                               "persistence": portal.persistence,
                               "session": portal.session}
            raw = raw_snapshot(raw_path, nonce, pid)
            before_count = len(raw["events"])
            stage = "Architect key tap witness" if catalog else "F12 tap witness"
            before_sample = len(raw["samples"]) - 1
            if catalog:
                require(raw["samples"][-1].get("menu_state") == "Hidden"
                        and raw["samples"][-1].get("play_mode") == "Normal",
                        "Architect scenario must start in ordinary normal world")
            key, observed_key = ("b", "KeyB") if catalog else (KEY, KEY)
            transport.send("key-press", nonce, key=key, pressed=True)
            transport.send("key-release", nonce, key=key, pressed=False)
            raw = wait_for_raw_event(raw_path, nonce, pid, "KeyRelease", after=before_count, key=observed_key)
            witnessed_input(raw, before_event_count=before_count, key=observed_key)
            if catalog:
                stage = "Architect opened by ordinary input"
                raw = wait_for_menu(raw_path, nonce, pid, "Architect", after=before_sample)
                opened = len(raw["samples"]) - 1
                require(native.capture_client_png(window, root / "catalog.png", import_cmd="import"),
                        "owned Architect capture failed")
                raw = raw_snapshot(raw_path, nonce, pid)
                deadline = time.monotonic() + 2
                while len(raw["samples"]) - 1 <= opened and time.monotonic() < deadline:
                    time.sleep(0.05)
                    raw = raw_snapshot(raw_path, nonce, pid)
                opened_end = len(raw["samples"]) - 1
                close_boundary = len(raw["events"])
                transport.send("close-press", nonce, key="Escape", pressed=True)
                transport.send("close-release", nonce, key="Escape", pressed=False)
                raw = wait_for_raw_event(raw_path, nonce, pid, "KeyRelease", after=close_boundary, key="Escape")
                witnessed_input(raw, before_event_count=close_boundary, key="Escape")
                stage = "Architect closed by ordinary input"
                raw = wait_for_menu(raw_path, nonce, pid, "Hidden", after=opened_end)
                catalog_evidence = {"before": before_sample, "opened": opened,
                                    "opened_end": opened_end,
                                    "closed": len(raw["samples"]) - 1, "closed_end": None, "close_event_count": close_boundary,
                                    "catalog_sha256": probe.digest(root / "catalog.png")}
            stage = "owned after capture"
            require(native.capture_client_png(window, screenshot_after, import_cmd="import"),
                    "owned X11 client after capture failed")
            if catalog:
                stage = "Architect closed capture stability"
                raw = raw_snapshot(raw_path, nonce, pid)
                deadline = time.monotonic() + 2
                while len(raw["samples"]) - 1 <= catalog_evidence["closed"] and time.monotonic() < deadline:
                    time.sleep(0.05)
                    raw = raw_snapshot(raw_path, nonce, pid)
                catalog_evidence["closed_end"] = len(raw["samples"]) - 1
                check_catalog_states(raw, catalog_evidence)
            metrics.append(probe.process_metrics(pid))
        except Exception as exc:
            (root / "failure.json").write_text(json.dumps({
                "schema": 1, "status": "invalid", "stage": stage,
                "error_type": type(exc).__name__, "reason": str(exc),
                "input_receipts": records,
                "accepted": False, "performance_evidence": False,
                "art_approved": False, "promotion_authority": False,
            }, sort_keys=True, separators=(",", ":")) + "\n")
            raise
        finally:
            try:
                if transport is not None:
                    transport.close()
            finally:
                try:
                    if portal is not None:
                        portal.close()
                finally:
                    if game.poll() is None:
                        native.stop_command_process(game)
    require(owned is not None and len(records) == (4 if catalog else 2)
            and [row["step"] for row in records] == (["key-press", "key-release", "close-press", "close-release"]
                                                     if catalog else ["key-press", "key-release"])
            and all(row["nonce"] == nonce and row["window"] == window_number
                    and row["pid"] == owned[1] for row in records)
            and metrics and all(row["rss_kib"] > 0 and row["process_fd_count"] > 0
                                for row in metrics), "owned input receipts or host metrics missing")
    result = {"schema": 1, "profile": value["profile"], "status": "technical-pass",
              "plan": value, "context": context, "nonce": nonce,
              "game_pid": owned[1], "window_id": window_number,
              "raw_sha256": probe.digest(raw_path),
              "before_sha256": probe.digest(screenshot_before),
              "after_sha256": probe.digest(screenshot_after),
              "before_event_count": before_count, "input_receipts": records,
              "portal_evidence": portal_evidence,
              "measurements": metrics, "gpu_metrics": None,
              "native_allocator_metrics": None, "application_handle_total": None,
              "accepted": False, "performance_evidence": False,
              "art_approved": False, "promotion_authority": False}
    if catalog:
        result["catalog_evidence"] = catalog_evidence
    (root / "result.json").write_text(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    verify(root)
    return result


def verify(root: Path) -> dict:
    ownership.exact_path(root)
    result = json.loads((root / "result.json").read_bytes())
    catalog = result.get("profile") == CATALOG_PROFILE
    fields = {"schema", "profile", "status", "plan", "context", "nonce",
                            "game_pid", "window_id", "raw_sha256", "before_sha256",
                            "after_sha256", "before_event_count", "input_receipts",
                            "portal_evidence",
                            "measurements", "gpu_metrics", "native_allocator_metrics",
                            "application_handle_total", "accepted", "performance_evidence",
                            "art_approved", "promotion_authority"}
    require(set(result) == (fields | {"catalog_evidence"} if catalog else fields), "input probe result shape differs")
    context = result["context"]
    plan_path = Path(context["command"][7])
    require(result["plan"] == json.loads(plan_path.read_bytes()), "input probe plan changed")
    repo, expected, _ = check_plan(result["plan"], plan_path, fresh=False)
    check_registered_context(repo, root, plan_path, context)
    require(root == expected and result["schema"] == 1 and result["profile"] == result["plan"]["profile"]
            and result["status"] == "technical-pass"
            and type(result["window_id"]) is int and result["window_id"] > 0
            and all(result[key] is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority"))
            and all(result[key] is None for key in
                    ("gpu_metrics", "native_allocator_metrics", "application_handle_total")),
            "input probe authority differs")
    check_portal_evidence(result["portal_evidence"])
    raw_path = root / "raw.json"
    require(probe.digest(raw_path) == result["raw_sha256"], "input probe raw changed")
    raw = raw_snapshot(raw_path, result["nonce"], result["game_pid"])
    witnessed_input(raw, before_event_count=result["before_event_count"], key="KeyB" if catalog else KEY)
    if catalog:
        evidence = result["catalog_evidence"]
        check_catalog_states(raw, evidence)
        opened_events = witnessed_input(raw, before_event_count=result["before_event_count"], key="KeyB")
        closed_events = witnessed_input(raw, before_event_count=evidence["close_event_count"], key="Escape")
        require(all(evidence["before"] <= row["sample_index"] <= evidence["opened"] for row in opened_events)
                and all(evidence["opened_end"] <= row["sample_index"] <= evidence["closed"] for row in closed_events),
                "Architect key witness lies outside its observed interval")
        image = root / "catalog.png"
        require(probe.digest(image) == evidence["catalog_sha256"]
                and native.validate_png_structure(image.read_bytes()) == WINDOW,
                "owned Architect capture changed")
    receipts = result["input_receipts"]
    expected_receipts = ([("key-press", "b", True), ("key-release", "b", False),
                          ("close-press", "Escape", True), ("close-release", "Escape", False)]
                         if catalog else [("key-press", KEY, True), ("key-release", KEY, False)])
    require(isinstance(receipts, list) and len(receipts) == len(expected_receipts)
            and [(row.get("step"), row.get("key"), row.get("pressed")) for row in receipts]
            == expected_receipts
            and all(row.get("nonce") == result["nonce"]
                    and row.get("window") == result["window_id"]
                    and row.get("pid") == result["game_pid"] for row in receipts),
            "input probe portal receipts differ")
    for name, field in (("before.png", "before_sha256"), ("after.png", "after_sha256")):
        path = root / name
        require(probe.digest(path) == result[field]
                and native.validate_png_structure(path.read_bytes()) == WINDOW,
                "input probe owned screenshot changed")
    require(result["measurements"] and all(row["rss_kib"] > 0
            and row["process_fd_count"] > 0 for row in result["measurements"]),
            "input probe host metrics missing")
    log = (root / "game.log").read_text()
    require(not re.search(r"\b(?:WARN|ERROR)\b", log), "ordinary-world log has warnings/errors")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    require(len(adapters) == 1 and "intel" in adapters[0][0].casefold()
            and adapters[0][1] == "Vulkan", "ordinary-world adapter differs")
    return {"status": "technical-pass", "scope": "ordinary Architect open/close" if catalog
            else "one owned ordinary-world key receipt only",
            "accepted": False, "promotion_authority": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    planned = actions.add_parser("plan")
    planned.add_argument("--repo", type=Path, required=True)
    planned.add_argument("--job-root", type=Path, required=True)
    planned.add_argument("--output", type=Path, required=True)
    planned.add_argument("--scenario", choices=("debug-toggle", "architect-cancel"), default="debug-toggle")
    running = actions.add_parser("run")
    running.add_argument("--plan", type=Path, required=True)
    running.add_argument("--repo", type=Path, required=True)
    running.add_argument("--job-root", type=Path, required=True)
    checked = actions.add_parser("verify")
    checked.add_argument("--job-root", type=Path, required=True)
    args = parser.parse_args()
    result = plan(args) if args.action == "plan" else run(args) if args.action == "run" else verify(args.job_root)
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
