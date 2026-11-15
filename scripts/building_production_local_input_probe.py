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

from PIL import Image

if __package__:
    from . import building_production_local_probe as probe
    from . import building_production_validation_context as ownership
    from . import building_production_selection_probe as selection_probe
    from . import building_production_placement_probe as placement_probe
    from . import building_production_yard_probe as yard_probe
    from . import building_production_companion_probe as companion_probe
    from . import building_production_save_probe as save_probe
    from .building_m2_native_acceptance import check_capture_pixels
    from .native_ui_portal import PortalSession, PortalX11Input
else:
    import building_production_local_probe as probe
    import building_production_validation_context as ownership
    import building_production_selection_probe as selection_probe
    from scripts import building_production_placement_probe as placement_probe
    from scripts import building_production_yard_probe as yard_probe
    from scripts import building_production_companion_probe as companion_probe
    from scripts import building_production_save_probe as save_probe
    from building_m2_native_acceptance import check_capture_pixels
    from scripts.native_ui_portal import PortalSession, PortalX11Input

native = probe.native
PROFILE = "building-production-local-input-probe-portal-v2"
CATALOG_PROFILE = "building-production-local-architect-portal-v1"
SELECTION_PROFILE = "building-production-local-equipment-feedback-eis-v2"
PLACEMENT_PROFILE = "building-production-local-tank-branch-feedback-eis-v1"
YARD_PROFILE = "building-production-local-tank-yard-feedback-eis-v1"
COMPANION_PROFILE = "building-production-local-tank-companion-feedback-eis-v1"
SAVE_PROFILE = "building-production-local-tank-save-feedback-eis-v1"
COMPANION_PROFILES = {COMPANION_PROFILE, SAVE_PROFILE}
YARD_PROFILES = {YARD_PROFILE, *COMPANION_PROFILES}
PLACEMENT_PROFILES = {PLACEMENT_PROFILE, *YARD_PROFILES}
SELECTION_PROFILES = {SELECTION_PROFILE, *PLACEMENT_PROFILES}
CATALOG_PROFILES = {CATALOG_PROFILE, *SELECTION_PROFILES}
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
    save = value.get("profile") == SAVE_PROFILE
    companion = value.get("profile") in COMPANION_PROFILES
    yard = value.get("profile") in YARD_PROFILES
    placement = value.get("profile") in PLACEMENT_PROFILES
    selection = value.get("profile") in SELECTION_PROFILES
    catalog = value.get("profile") in CATALOG_PROFILES
    expected = fields | {"scenario"} if catalog else fields
    if selection:
        expected |= {"selection_kind", "selection_driver_sha256", "eis_transport_sha256"}
    if placement:
        expected.add("placement_driver_sha256")
    if yard:
        expected.add("yard_driver_sha256")
    if companion:
        expected.add("companion_driver_sha256")
    if save:
        expected.add("save_driver_sha256")
    require(set(value) == expected,
            "input probe plan shape differs")
    repo = native.validate_repo(value["repo"])
    root, binary = Path(value["job_root"]), Path(value["binary"])
    for path in (plan_path, root, binary, Path(value["consent_file"])):
        ownership.exact_path(path)
    require(value["schema"] == 1 and value["profile"] in {PROFILE, *CATALOG_PROFILES}
            and (not catalog or value.get("scenario") == ("architect-select-place-companion-save-load" if save else "architect-select-place-companion-cancel" if companion else "architect-select-place-yard-cancel" if yard else "architect-select-place-cancel" if placement
                 else "architect-select-cancel" if selection else "architect-cancel"))
            and (not selection or value.get("selection_kind") in selection_probe.KINDS)
            and (not placement or value.get("selection_kind") == "Tank")
            and value["owner"] == OWNER and value["key"] == ("b" if catalog else KEY)
            and value["consent_file"] == str(repo / CONSENT_RELATIVE)
            and all(value[key] is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority")),
            "input probe cannot grant authority or change its key")
    require((not plan_exists or plan_path.is_file())
            and plan_path.parent == repo / "target/native-acceptance"
            and root.parent == repo / "target/native-acceptance"
            and root != plan_path and binary == repo / ("target/debug/bevy_app" if selection else "target/profiling/bevy_app")
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
    require(not selection or value["selection_driver_sha256"] == probe.digest(Path(selection_probe.__file__)),
            "equipment selection driver differs")
    require(not selection or value["eis_transport_sha256"] == probe.digest(
        Path(__file__).with_name("native_ui_eis.py")), "equipment EIS transport differs")
    require(not placement or value["placement_driver_sha256"] == probe.digest(Path(placement_probe.__file__)),
            "placement branch driver differs")
    require(not yard or value["yard_driver_sha256"] == probe.digest(Path(yard_probe.__file__)), "Yard input driver differs")
    require(not companion or value["companion_driver_sha256"] == probe.digest(Path(companion_probe.__file__)),
            "companion input driver differs")
    require(not save or value["save_driver_sha256"] == probe.digest(Path(save_probe.__file__)), "save input driver differs")
    if fresh:
        require(not root.exists(), "input probe output root already exists")
    return repo, root, binary


def plan(args: argparse.Namespace) -> dict:
    repo = native.validate_repo(args.repo)
    root, output = Path(args.job_root).absolute(), Path(args.output).absolute()
    ownership.exact_path(root)
    ownership.exact_path(output)
    require(not root.exists() and not output.exists(), "input probe requires fresh paths")
    save = getattr(args, "scenario", None) == "architect-select-place-companion-save-load"
    companion = save or getattr(args, "scenario", None) == "architect-select-place-companion-cancel"
    yard = companion or getattr(args, "scenario", None) == "architect-select-place-yard-cancel"
    placement = yard or getattr(args, "scenario", None) == "architect-select-place-cancel"
    selection = placement or getattr(args, "scenario", None) == "architect-select-cancel"
    require((getattr(args, "kind", None) in selection_probe.KINDS) if selection
            else getattr(args, "kind", None) is None, "equipment kind must belong to the fixed selection scenario")
    require(not placement or args.kind == "Tank", "first branch feedback is Tank-only")
    binary = repo / ("target/debug/bevy_app" if selection else "target/profiling/bevy_app")
    require(binary.is_file(), "build the profiling-feature binary through dev.py feedback or cargo first")
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
    if selection:
        value.update(profile=SELECTION_PROFILE, scenario="architect-select-cancel", key="b",
                     selection_kind=args.kind, selection_driver_sha256=probe.digest(Path(selection_probe.__file__)),
                     eis_transport_sha256=probe.digest(Path(__file__).with_name("native_ui_eis.py")))
    if placement:
        value.update(profile=PLACEMENT_PROFILE, scenario="architect-select-place-cancel",
                     placement_driver_sha256=probe.digest(Path(placement_probe.__file__)))
    if yard:
        value.update(profile=YARD_PROFILE, scenario="architect-select-place-yard-cancel",
                     yard_driver_sha256=probe.digest(Path(yard_probe.__file__)))
    if companion:
        value.update(profile=COMPANION_PROFILE, scenario="architect-select-place-companion-cancel",
                     companion_driver_sha256=probe.digest(Path(companion_probe.__file__)))
    if save:
        value.update(profile=SAVE_PROFILE, scenario="architect-select-place-companion-save-load",
                     save_driver_sha256=probe.digest(Path(save_probe.__file__)))
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


def check_startup_capture(raw: dict, evidence: dict) -> None:
    require(set(evidence) == {"before", "after", "epoch"}, "startup capture shape differs")
    before, after = evidence["before"], evidence["after"]
    require(type(before) is int and type(after) is int and 0 <= before < after < len(raw["samples"])
            and type(evidence["epoch"]) is int and evidence["epoch"] >= 0
            and all(row.get("menu_state") == "Hidden" and row.get("play_mode") == "Normal"
                    and row.get("task_mode") == "None" and row.get("architect_panel_ready") is False
                    and type(row.get("world_epoch")) is int and row.get("world_epoch") == evidence["epoch"]
                    for row in raw["samples"][before:after + 1]),
            "startup capture interval missing, stale or changed")


def check_startup_background(path: Path) -> None:
    """Reject actor/UI-only startup frames; not a terrain/art acceptance test."""
    with Image.open(path) as image:
        require(image.size == WINDOW, "startup client dimensions differ")
        # GamePlugin's ClearColor is srgb(0.1, 0.1, 0.1). This fixed normal
        # startup viewport is inside the map; majority clear background is not
        # a rendered world, even when actor lights make the nonblank gate pass.
        pixels = list(image.convert("RGB").crop((160, 120, 1120, 400)).get_flattened_data())
    clear = sum(all(abs(channel - 26) <= 2 for channel in pixel) for pixel in pixels)
    if clear * 2 >= len(pixels):
        raise ValueError("startup viewport still majority clear background")


def capture_startup(window: str, root: Path, raw_path: Path, nonce: str, pid: int) -> dict:
    """Keep rejected blank attempts; never treat raw readiness as rendered readiness."""
    deadline = time.monotonic() + 8
    for attempt in range(16):
        require(time.monotonic() < deadline, "startup rendered capture timeout")
        before = raw_snapshot(raw_path, nonce, pid)
        index = len(before["samples"]) - 1
        evidence = {"before": index, "after": index, "epoch": before["samples"][-1].get("world_epoch")}
        path = root / f"startup-capture-{attempt}.png"
        require(not path.exists() and not (root / "before.png").exists(), "startup capture path already exists")
        require(native.capture_client_png(window, path, import_cmd="import"), "owned startup capture failed")
        after = raw_snapshot(raw_path, nonce, pid)
        fresh_deadline = min(deadline, time.monotonic() + 2)
        while len(after["samples"]) - 1 <= index and time.monotonic() < fresh_deadline:
            time.sleep(0.05)
            after = raw_snapshot(raw_path, nonce, pid)
        evidence["after"] = len(after["samples"]) - 1
        check_startup_capture(after, evidence)
        try:
            check_capture_pixels(path)
            check_startup_background(path)
        except ValueError as exc:
            if str(exc) not in {"blank central viewport", "startup viewport still majority clear background"}:
                raise
            (root / f"startup-rejected-{attempt}.json").write_text(json.dumps({
                "reason": str(exc), "capture": path.name, "sha256": probe.digest(path),
                "interval": evidence}, sort_keys=True) + "\n")
            continue
        path.rename(root / "before.png")
        return evidence
    raise native.AcceptanceError("startup blank capture attempt cap exceeded")


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
                and len(raw["samples"]) >= 2 and all(
                    row.get("menu_state") == expected
                    and row.get("play_mode") == "Normal"
                    and row.get("task_mode") == "None"
                    and "selected_building" in row and row["selected_building"] is None
                    and row.get("architect_panel_ready") is (expected == "Architect")
                    for row in raw["samples"][-2:])):
            return raw
        time.sleep(0.05)
    raise native.AcceptanceError(f"ordinary Architect {expected} raw witness timeout")


def check_rendered_catalog(path: Path, sample: dict) -> None:
    """Limited panel-presence check, not label, asset or art acceptance.

    Source theme.rs fixes default card sRGB to (0.25, 0.25, 0.3).
    Insets avoid the radius/icon/text. One hover and its tooltip may cover two
    cards; the remaining seven must actually be rendered at their raw bounds.
    """
    with Image.open(path) as image:
        require(image.size == WINDOW, "catalog image extent differs")
        rgb = image.convert("RGB")
        visible = 0
        for kind in selection_probe.KINDS:
            row, _ = selection_probe.card(sample, kind)
            left, top, right, bottom = row["physical_rect"]
            require(right - left >= 16 and bottom - top >= 16,
                    "catalog card too small for interior background patches")
            points = [(int(x), int(y)) for x in (left + 6, right - 7)
                      for y in (top + 6, bottom - 7)]
            matches = sum(all(abs(actual - expected) <= 2
                              for actual, expected in zip(rgb.getpixel(point), (64, 64, 77), strict=True))
                          for point in points)
            visible += matches >= 3
        require(visible >= 7, "Architect card backgrounds not rendered in owned capture")


def capture_catalog(window: str, root: Path, read) -> None:
    # Retry only observation/capture, never input; preserve every rejected frame.
    for attempt in range(8):
        raw = read()
        path = root / f"catalog-capture-{attempt}.png"
        require(native.capture_client_png(window, path, import_cmd="import"), "owned Architect capture failed")
        check_capture_pixels(path)
        try:
            check_rendered_catalog(path, raw["samples"][-1])
        except native.AcceptanceError:
            if attempt == 7:
                raise
            time.sleep(0.1)
        else:
            path.replace(root / "catalog.png")
            return


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
    save = value["profile"] == SAVE_PROFILE
    companion = value["profile"] in COMPANION_PROFILES
    yard = value["profile"] in YARD_PROFILES
    placement = value["profile"] in PLACEMENT_PROFILES
    selection = value["profile"] in SELECTION_PROFILES
    catalog = value["profile"] in CATALOG_PROFILES
    catalog_evidence = None
    selection_evidence = None
    placement_evidence = None
    yard_evidence = None
    companion_evidence = None
    save_evidence = None
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
            startup_capture = capture_startup(window, root, raw_path, nonce, pid)
            raw = raw_snapshot(raw_path, nonce, pid)
            before_count = len(raw["events"])
            stage = "owned portal input activation"
            window_number = numeric_window_id(window)
            portal = PortalSession(
                lambda _method: require(game.poll() is None, "ordinary-world game exited during portal consent"),
                consent_file=value["consent_file"])
            if selection:
                from scripts.native_ui_eis import PortalEisInput
                transport = PortalEisInput(window_number, pid, game.pid, nonce, records.append, portal=portal)
            else:
                transport = PortalX11Input(window_number, pid, game.pid, nonce, records.append, portal=portal)
            transport.activate()
            require(portal.ready and portal.granted_devices >= 0
                    and portal.granted_devices & 3 == 3,
                    "portal keyboard/pointer grant is unavailable")
            portal_evidence = {"granted_devices": portal.granted_devices,
                               "persistence": portal.persistence,
                               "session": portal.session}
            if yard:
                stage = "ordinary camera navigation to existing Yard"
                yard_evidence = yard_probe.run(transport, lambda: raw_snapshot(raw_path, nonce, pid))
                require(native.capture_client_png(window, root / "yard.png", import_cmd="import"), "owned Yard capture failed")
                check_capture_pixels(root / "yard.png")
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
                if selection:
                    capture_catalog(window, root, lambda: raw_snapshot(raw_path, nonce, pid))
                else:
                    require(native.capture_client_png(window, root / "catalog.png", import_cmd="import"),
                            "owned Architect capture failed")
                    check_capture_pixels(root / "catalog.png")
                raw = raw_snapshot(raw_path, nonce, pid)
                deadline = time.monotonic() + 2
                while len(raw["samples"]) - 1 <= opened and time.monotonic() < deadline:
                    time.sleep(0.05)
                    raw = raw_snapshot(raw_path, nonce, pid)
                opened_end = len(raw["samples"]) - 1
                if selection:
                    stage = "ordinary equipment selection feedback"

                    def capture_selection(path):
                        require(native.capture_client_png(window, path, import_cmd="import"),
                                "owned equipment selection capture failed")
                        check_capture_pixels(path)

                    selection_evidence = selection_probe.run(
                        transport, lambda: raw_snapshot(raw_path, nonce, pid), value["selection_kind"],
                        root, capture_selection)
                    if placement:
                        stage = "ordinary first Tank placement branch"
                        placement_evidence = placement_probe.run(transport, lambda: raw_snapshot(raw_path, nonce, pid), yard=yard)
                        capture_selection(root / "placement.png")
                        if companion:
                            stage = "ordinary Tank companion commit"
                            companion_evidence = companion_probe.run(transport, lambda: raw_snapshot(raw_path, nonce, pid), placement_evidence)
                            capture_selection(root / "companion.png")
                    raw = raw_snapshot(raw_path, nonce, pid)
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
            check_capture_pixels(screenshot_after)
            if catalog:
                stage = "Architect closed capture stability"
                raw = raw_snapshot(raw_path, nonce, pid)
                deadline = time.monotonic() + 2
                while len(raw["samples"]) - 1 <= catalog_evidence["closed"] and time.monotonic() < deadline:
                    time.sleep(0.05)
                    raw = raw_snapshot(raw_path, nonce, pid)
                catalog_evidence["closed_end"] = len(raw["samples"]) - 1
                check_catalog_states(raw, catalog_evidence)
            if save:
                stage = "ordinary Tank planning save/load"
                save_evidence = save_probe.run(transport, lambda: raw_snapshot(raw_path, nonce, pid), root, capture_selection)
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
    expected_steps = []
    if yard:
        for leg in yard_evidence["legs"]:
            axis = 0 if leg["key"] in {"a", "d"} else 1
            expected_steps.extend([f"pan-{axis}-press", f"pan-{axis}-release"])
    expected_steps.extend(["key-press", "key-release"])
    if selection:
        expected_steps.extend(["select-motion", "select-press", "select-release"])
    if placement:
        expected_steps.extend(["world-motion", "world-press", "world-release"])
    if companion:
        expected_steps.extend(["companion-motion", "companion-press", "companion-release"])
    if catalog:
        expected_steps.extend(["close-press", "close-release"])
    if save:
        expected_steps.extend(row[0] for row in save_probe.STEPS)
    require(owned is not None and len(records) == len(expected_steps)
            and [row["step"] for row in records] == expected_steps
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
    result["startup_capture"] = startup_capture
    if catalog:
        result["catalog_evidence"] = catalog_evidence
    if selection:
        result["selection_evidence"] = selection_evidence
        result["selected_sha256"] = probe.digest(root / "selected.png")
    if placement:
        result.update(placement_evidence=placement_evidence, placement_sha256=probe.digest(root / "placement.png"))
    if yard:
        result.update(yard_evidence=yard_evidence, yard_sha256=probe.digest(root / "yard.png"))
    if companion:
        result.update(companion_evidence=companion_evidence, companion_sha256=probe.digest(root / "companion.png"))
    if save:
        result.update(save_evidence=save_evidence, saved_sha256=probe.digest(root / "saved.png"), loaded_sha256=probe.digest(root / "loaded.png"))
    (root / "result.json").write_text(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    verify(root)
    return result


def verify(root: Path) -> dict:
    ownership.exact_path(root)
    result = json.loads((root / "result.json").read_bytes())
    save = result.get("profile") == SAVE_PROFILE
    companion = result.get("profile") in COMPANION_PROFILES
    yard = result.get("profile") in YARD_PROFILES
    placement = result.get("profile") in PLACEMENT_PROFILES
    selection = result.get("profile") in SELECTION_PROFILES
    catalog = result.get("profile") in CATALOG_PROFILES
    fields = {"schema", "profile", "status", "plan", "context", "nonce",
                            "game_pid", "window_id", "raw_sha256", "before_sha256",
                            "after_sha256", "before_event_count", "input_receipts",
                            "portal_evidence", "startup_capture",
                            "measurements", "gpu_metrics", "native_allocator_metrics",
                            "application_handle_total", "accepted", "performance_evidence",
                            "art_approved", "promotion_authority"}
    expected = fields | {"catalog_evidence"} if catalog else fields
    if selection:
        expected |= {"selection_evidence", "selected_sha256"}
    if placement:
        expected |= {"placement_evidence", "placement_sha256"}
    if yard:
        expected |= {"yard_evidence", "yard_sha256"}
    if companion:
        expected |= {"companion_evidence", "companion_sha256"}
    if save:
        expected |= {"save_evidence", "saved_sha256", "loaded_sha256"}
    require(set(result) == expected, "input probe result shape differs")
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
    check_startup_capture(raw, result["startup_capture"])
    check_startup_background(root / "before.png")
    input_events = witnessed_input(raw, before_event_count=result["before_event_count"], key="KeyB" if catalog else KEY)
    require(all(row["sample_index"] >= result["startup_capture"]["after"] for row in input_events),
            "input precedes startup capture")
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
        check_capture_pixels(image)
    if selection:
        check_rendered_catalog(root / "catalog.png", raw["samples"][result["catalog_evidence"]["opened"]])
        selection_raw = raw
        if placement:
            selection_raw = {**raw, "events": [event for event in raw["events"]
                             if event["sample_index"] <= result["selection_evidence"]["end"]]}
        selection_probe.verify(selection_raw, result["selection_evidence"], result["plan"]["selection_kind"])
        require(result["catalog_evidence"]["opened_end"] <= result["selection_evidence"]["before"]
                and result["selection_evidence"]["end"] < result["catalog_evidence"]["closed"],
                "equipment selection is outside the ordinary catalog cycle")
        require(all("selected_building" in s and s["selected_building"] is None
                    for s in raw["samples"][evidence["closed"] - 1:evidence["closed_end"] + 1]),
                "equipment cancellation retained a build selection")
        image = root / "selected.png"
        require(probe.digest(image) == result["selected_sha256"], "equipment selection capture changed")
        check_capture_pixels(image)
    if placement:
        placement_probe.verify(raw, result["placement_evidence"], yard=yard)
        require(result["selection_evidence"]["end"] <= result["placement_evidence"]["before"]
                and result["placement_evidence"]["after"] < result["catalog_evidence"]["closed"],
                "world click outside selected ordinary interval")
        image = root / "placement.png"
        require(probe.digest(image) == result["placement_sha256"], "placement branch capture changed")
        check_capture_pixels(image)
    if yard:
        yard_probe.verify(raw, result["yard_evidence"])
        require(result["yard_evidence"]["before"] >= result["startup_capture"]["after"]
                and result["yard_evidence"]["after"] <= result["catalog_evidence"]["before"], "Yard pan is outside normal startup interval")
        require(probe.digest(root / "yard.png") == result["yard_sha256"], "Yard capture changed")
        check_capture_pixels(root / "yard.png")
    if companion:
        companion_probe.verify(raw, result["companion_evidence"], result["placement_evidence"])
        require(result["companion_evidence"]["after"] < result["catalog_evidence"]["closed"],
                "companion click outside ordinary selection interval")
        require(probe.digest(root / "companion.png") == result["companion_sha256"], "companion capture changed")
        check_capture_pixels(root / "companion.png")
        committed = raw["samples"][result["companion_evidence"]["after"]]
        for sample in raw["samples"][evidence["closed"] - 1:evidence["closed_end"] + 1]:
            companion_probe.check_closed(sample, committed)
    receipts = result["input_receipts"]
    expected_receipts = ([("key-press", "b", True), ("key-release", "b", False),
                          ("close-press", "Escape", True), ("close-release", "Escape", False)]
                         if catalog else [("key-press", KEY, True), ("key-release", KEY, False)])
    if selection:
        expected_receipts[2:2] = [("select-motion", None, None), ("select-press", None, True),
                                  ("select-release", None, False)]
    if placement:
        expected_receipts[5:5] = [("world-motion", None, None), ("world-press", None, True), ("world-release", None, False)]
    if companion:
        expected_receipts[8:8] = [("companion-motion", None, None), ("companion-press", None, True), ("companion-release", None, False)]
    pan_receipts = []
    if yard:
        for leg in result["yard_evidence"]["legs"]:
            axis = 0 if leg["key"] in {"a", "d"} else 1
            pan_receipts.extend([(f"pan-{axis}-press", leg["key"], True), (f"pan-{axis}-release", leg["key"], False)])
    expected_receipts[0:0] = pan_receipts
    if save:
        expected_receipts.extend(save_probe.STEPS)
    require(isinstance(receipts, list) and len(receipts) == len(expected_receipts)
            and [(row.get("step"), row.get("key"), row.get("pressed")) for row in receipts]
            == expected_receipts
            and all(row.get("nonce") == result["nonce"]
                    and row.get("window") == result["window_id"]
                    and row.get("pid") == result["game_pid"] for row in receipts),
            "input probe portal receipts differ")
    if selection:
        motion, press, release = receipts[2 + len(pan_receipts):5 + len(pan_receipts)]
        require(motion.get("point") == result["selection_evidence"]["point"]
                and motion.get("button") is None
                and all(row.get("point") is None and row.get("button") == 1 for row in (press, release)),
                "equipment pointer receipts differ")
    if placement:
        motion, press, release = receipts[5 + len(pan_receipts):8 + len(pan_receipts)]
        require(motion.get("point") == result["placement_evidence"]["point"] and motion.get("button") is None
                and all(row.get("point") is None and row.get("button") == 1 for row in (press, release)),
                "world pointer receipts differ")
    if companion:
        motion, press, release = receipts[8 + len(pan_receipts):11 + len(pan_receipts)]
        require(motion.get("point") == result["companion_evidence"]["point"] and motion.get("button") is None
                and all(row.get("point") is None and row.get("button") == 1 for row in (press, release)),
                "companion pointer receipts differ")
    if save:
        save_probe.verify(raw, result["save_evidence"], root)
        require(result["save_evidence"]["before"] >= result["catalog_evidence"]["closed_end"], "save precedes ordinary placement closure")
        for action, leg in (("save", "save"), ("load", "load"), ("confirmload", "confirm")):
            rows = [row for row in receipts if row["step"].startswith(action + "-") and "open" not in row["step"]]
            require(len(rows) == 3 and rows[0].get("point") == result["save_evidence"][leg]["point"]
                    and rows[0].get("button") is None and all(row.get("point") is None and row.get("button") == 1 for row in rows[1:]),
                    "save pointer receipts differ")
        for name in ("saved", "loaded"):
            require(probe.digest(root / (name + ".png")) == result[name + "_sha256"], "save/load capture changed")
            check_capture_pixels(root / (name + ".png"))
    for name, field in (("before.png", "before_sha256"), ("after.png", "after_sha256")):
        path = root / name
        require(probe.digest(path) == result[field]
                and native.validate_png_structure(path.read_bytes()) == WINDOW,
                "input probe owned screenshot changed")
        check_capture_pixels(path)
    require(result["measurements"] and all(row["rss_kib"] > 0
            and row["process_fd_count"] > 0 for row in result["measurements"]),
            "input probe host metrics missing")
    log = (root / "game.log").read_text()
    require(not re.search(r"\b(?:WARN|ERROR)\b", log), "ordinary-world log has warnings/errors")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    require(len(adapters) == 1 and "intel" in adapters[0][0].casefold()
            and adapters[0][1] == "Vulkan", "ordinary-world adapter differs")
    return {"status": "technical-pass", "scope": "ordinary Tank and companion planning save/load only (no completed construction)" if save else "ordinary Tank Blueprint and companion planning commit only (no completed construction)" if companion else "first Tank Yard click pending only (no committed owner)" if yard else "first Tank click branch only (pending/rejected, no committed owner)" if placement
            else "feedback-only ordinary equipment selection (no placement)" if selection
            else "ordinary Architect open/close" if catalog
            else "one owned ordinary-world key receipt only",
            "accepted": False, "promotion_authority": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    planned = actions.add_parser("plan")
    planned.add_argument("--repo", type=Path, required=True)
    planned.add_argument("--job-root", type=Path, required=True)
    planned.add_argument("--output", type=Path, required=True)
    planned.add_argument("--scenario", choices=("debug-toggle", "architect-cancel", "architect-select-cancel",
                        "architect-select-place-cancel", "architect-select-place-yard-cancel", "architect-select-place-companion-cancel", "architect-select-place-companion-save-load"), default="debug-toggle")
    planned.add_argument("--kind", choices=selection_probe.KINDS)
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
