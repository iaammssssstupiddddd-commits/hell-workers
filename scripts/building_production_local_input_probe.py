"""Technical-only ordinary-world X11 input/raw witness probe for TAK-14.

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
    from .native_ui_input import X11Input
else:
    import building_production_local_probe as probe
    import building_production_validation_context as ownership
    from native_ui_input import X11Input

native = probe.native
PROFILE = "building-production-local-input-probe-v1"
OWNER = "TAK-14 normal-development coordinator"
KEY = "F12"
WINDOW = (1280, 720)


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
    require(set(value) == {"schema", "profile", "repo", "job_root", "head", "source",
                           "harness", "driver", "input_transport_sha256", "binary",
                           "binary_sha256", "owner", "key", "accepted",
                           "performance_evidence", "art_approved", "promotion_authority"},
            "input probe plan shape differs")
    repo = native.validate_repo(value["repo"])
    root, binary = Path(value["job_root"]), Path(value["binary"])
    for path in (plan_path, root, binary):
        ownership.exact_path(path)
    require(value["schema"] == 1 and value["profile"] == PROFILE
            and value["owner"] == OWNER and value["key"] == KEY
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
             "binary": str(binary), "binary_sha256": probe.digest(binary), "owner": OWNER,
             "key": KEY, "accepted": False, "performance_evidence": False,
             "art_approved": False, "promotion_authority": False}
    require(root.parent == repo / "target/native-acceptance" and not native.git_dirty_paths(repo),
            "input probe requires a clean primary subject and direct output root")
    resources = native.resource_snapshot(repo, require_launcher=True)
    require(not resources["failures"], f"native resources unavailable: {resources['failures']}")
    require(native.save_catalog_window_capture_tool_error() is None,
            "ordinary-world X11 capture tools are unavailable")
    check_plan(value, output, plan_exists=False)
    with output.open("x") as handle:
        handle.write(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
    return {"status": "ready", "profile": PROFILE, "job_root": str(root),
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


def witnessed_input(raw: dict, *, before_event_count: int) -> tuple[dict, dict]:
    require(type(before_event_count) is int and 0 <= before_event_count <= len(raw["events"]),
            "input probe event boundary is invalid")
    rows = raw["events"][before_event_count:]
    matched = [row for row in rows if row.get("operation") in {"KeyPress", "KeyRelease"}
               and row.get("key") == KEY]
    require(len(matched) == 2 and [row["operation"] for row in matched]
            == ["KeyPress", "KeyRelease"]
            and matched[0]["event_index"] < matched[1]["event_index"]
            and matched[0]["sample_index"] <= matched[1]["sample_index"],
            "one ordered raw key press/release witness is missing")
    return matched[0], matched[1]


def wait_for_raw_event(path: Path, nonce: str, pid: int, operation: str,
                       *, after: int, timeout: float = 8.0) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        raw = raw_snapshot(path, nonce, pid)
        matched = [event for event in raw["events"][after:]
                   if event.get("operation") == operation and event.get("key") == KEY]
        require(len(matched) <= 1, "duplicate ordinary input event")
        if matched:
            return raw
        time.sleep(0.05)
    raise native.AcceptanceError(f"ordinary input {operation} raw witness timeout")


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
    metrics: list[dict] = []
    with (root / "game.log").open("w") as log:
        game = subprocess.Popen([str(binary)], cwd=runtime, env=environment, stdout=log,
                                stderr=subprocess.STDOUT, start_new_session=True)
        transport = None
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
            stage = "owned X11 input activation"
            transport = X11Input(window, pid, game.pid, nonce, records.append)
            transport.activate()
            stage = "F12 press witness"
            transport.send("key-press", nonce, key=KEY, pressed=True)
            raw = wait_for_raw_event(raw_path, nonce, pid, "KeyPress", after=before_count)
            stage = "F12 release witness"
            transport.send("key-release", nonce, key=KEY, pressed=False)
            raw = wait_for_raw_event(raw_path, nonce, pid, "KeyRelease", after=before_count)
            witnessed_input(raw, before_event_count=before_count)
            stage = "owned after capture"
            require(native.capture_client_png(window, screenshot_after, import_cmd="import"),
                    "owned X11 client after capture failed")
            metrics.append(probe.process_metrics(pid))
        except Exception as exc:
            (root / "failure.json").write_text(json.dumps({
                "schema": 1, "status": "invalid", "stage": stage,
                "error_type": type(exc).__name__, "reason": str(exc),
                "accepted": False, "performance_evidence": False,
                "art_approved": False, "promotion_authority": False,
            }, sort_keys=True, separators=(",", ":")) + "\n")
            raise
        finally:
            try:
                if transport is not None:
                    transport.close()
            finally:
                if game.poll() is None:
                    native.stop_command_process(game)
    require(owned is not None and len(records) == 2
            and [row["step"] for row in records] == ["key-press", "key-release"]
            and all(row["nonce"] == nonce and row["window"] == owned[0]
                    and row["pid"] == owned[1] for row in records)
            and metrics and all(row["rss_kib"] > 0 and row["process_fd_count"] > 0
                                for row in metrics), "owned input receipts or host metrics missing")
    result = {"schema": 1, "profile": PROFILE, "status": "technical-pass",
              "plan": value, "context": context, "nonce": nonce,
              "game_pid": owned[1], "window_id": owned[0],
              "raw_sha256": probe.digest(raw_path),
              "before_sha256": probe.digest(screenshot_before),
              "after_sha256": probe.digest(screenshot_after),
              "before_event_count": before_count, "input_receipts": records,
              "measurements": metrics, "gpu_metrics": None,
              "native_allocator_metrics": None, "application_handle_total": None,
              "accepted": False, "performance_evidence": False,
              "art_approved": False, "promotion_authority": False}
    (root / "result.json").write_text(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    verify(root)
    return result


def verify(root: Path) -> dict:
    ownership.exact_path(root)
    result = json.loads((root / "result.json").read_bytes())
    require(set(result) == {"schema", "profile", "status", "plan", "context", "nonce",
                            "game_pid", "window_id", "raw_sha256", "before_sha256",
                            "after_sha256", "before_event_count", "input_receipts",
                            "measurements", "gpu_metrics", "native_allocator_metrics",
                            "application_handle_total", "accepted", "performance_evidence",
                            "art_approved", "promotion_authority"}, "input probe result shape differs")
    context = result["context"]
    plan_path = Path(context["command"][7])
    require(result["plan"] == json.loads(plan_path.read_bytes()), "input probe plan changed")
    repo, expected, _ = check_plan(result["plan"], plan_path, fresh=False)
    check_registered_context(repo, root, plan_path, context)
    require(root == expected and result["schema"] == 1 and result["profile"] == PROFILE
            and result["status"] == "technical-pass"
            and all(result[key] is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority"))
            and all(result[key] is None for key in
                    ("gpu_metrics", "native_allocator_metrics", "application_handle_total")),
            "input probe authority differs")
    raw_path = root / "raw.json"
    require(probe.digest(raw_path) == result["raw_sha256"], "input probe raw changed")
    raw = raw_snapshot(raw_path, result["nonce"], result["game_pid"])
    witnessed_input(raw, before_event_count=result["before_event_count"])
    receipts = result["input_receipts"]
    require(isinstance(receipts, list) and len(receipts) == 2
            and [(row.get("step"), row.get("key"), row.get("pressed")) for row in receipts]
            == [("key-press", KEY, True), ("key-release", KEY, False)]
            and all(row.get("nonce") == result["nonce"]
                    and row.get("window") == result["window_id"]
                    and row.get("pid") == result["game_pid"] for row in receipts),
            "input probe X11 receipts differ")
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
    return {"status": "technical-pass", "scope": "one owned ordinary-world key receipt only",
            "accepted": False, "promotion_authority": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    planned = actions.add_parser("plan")
    planned.add_argument("--repo", type=Path, required=True)
    planned.add_argument("--job-root", type=Path, required=True)
    planned.add_argument("--output", type=Path, required=True)
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
