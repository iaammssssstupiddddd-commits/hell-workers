"""Guarded ordinary-world host probe, not building acceptance or asset authority.

This is the first normal-development producer boundary: it observes one owned
X11/Vulkan game process and the opt-in raw product stream. It drives no input,
admits no release identities, and cannot satisfy lifecycle or performance gates.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys
import time

HELPERS = Path(__file__).resolve().parents[1] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPERS))
import native_acceptance as native  # noqa: E402

if __package__:
    from . import building_production_validation_context as ownership
else:
    import building_production_validation_context as ownership

PROFILE = "building-production-local-observation-v1"
OWNER = "TAK-14 normal-development coordinator"
WINDOW = (1280, 720)


def require(value: bool, reason: str) -> None:
    if not value:
        raise native.AcceptanceError(reason)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(plan: Path, repo: Path, root: Path) -> list[str]:
    return ["env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1",
            "python3", str(Path(__file__).resolve()), "run", "--plan", str(plan),
            "--repo", str(repo), "--job-root", str(root)]


def verifier(root: Path) -> list[str]:
    return ["python3", str(Path(__file__).resolve()), "verify", "--job-root", str(root)]


def check_plan(value: dict, plan: Path, *, require_fresh: bool = True,
               require_plan_file: bool = True) -> tuple[Path, Path, Path]:
    require(set(value) == {"schema", "profile", "repo", "job_root", "head", "source",
                           "harness", "driver", "binary", "binary_sha256", "owner",
                           "accepted", "promotion_authority"}, "local probe plan shape differs")
    repo = native.validate_repo(value["repo"])
    root = Path(value["job_root"])
    binary = Path(value["binary"])
    require(value["schema"] == 1 and value["profile"] == PROFILE
            and value["owner"] == OWNER and value["accepted"] is False
            and value["promotion_authority"] is False, "local probe cannot grant authority")
    ownership.exact_path(plan)
    ownership.exact_path(root)
    ownership.exact_path(binary)
    require((not require_plan_file or plan.is_file())
            and plan.parent == repo / "target/native-acceptance"
            and root != plan and root != plan.parent
            and root.parent == repo / "target/native-acceptance"
            and binary == repo / "target/profiling/bevy_app"
            and binary.is_file() and os.access(binary, os.X_OK), "local probe paths differ")
    require(not native.git_dirty_paths(repo)
            and value["head"] == native.git_subject(repo)
            and value["source"] == native.source_fingerprint(repo)
            and value["harness"] == native.native_harness_fingerprint(repo)
            and value["driver"] == digest(Path(__file__))
            and value["binary_sha256"] == digest(binary), "local probe frozen source differs")
    if require_fresh:
        require(not root.exists(), "local probe output root already exists")
    return repo, root, binary


def plan(args: argparse.Namespace) -> dict:
    repo = native.validate_repo(args.repo)
    root, output = Path(args.job_root).absolute(), Path(args.output).absolute()
    ownership.exact_path(root)
    ownership.exact_path(output)
    require(not root.exists() and not output.exists(), "local probe requires fresh plan and output")
    binary = repo / "target/profiling/bevy_app"
    require(binary.is_file(), "build the profiling-feature binary through dev.py cargo first")
    value = {"schema": 1, "profile": PROFILE, "repo": str(repo), "job_root": str(root),
             "head": native.git_subject(repo), "source": native.source_fingerprint(repo),
             "harness": native.native_harness_fingerprint(repo), "driver": digest(Path(__file__)),
             "binary": str(binary), "binary_sha256": digest(binary), "owner": OWNER,
             "accepted": False, "promotion_authority": False}
    # Check paths and source before publishing the command file.
    require(root.parent == repo / "target/native-acceptance" and not native.git_dirty_paths(repo),
            "local probe requires a clean primary subject and direct output root")
    resources = native.resource_snapshot(repo, require_launcher=True)
    require(not resources["failures"], f"native resources unavailable: {resources['failures']}")
    require(native.save_catalog_window_capture_tool_error() is None,
            "ordinary-world X11 capture tools are unavailable")
    check_plan(value, output, require_plan_file=False)
    with output.open("x") as handle:
        handle.write(json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n")
    return {"status": "ready", "job_root": str(root), "profile": PROFILE,
            "launcher_command": ["kitty", "--directory", str(repo), "--detach", *command(output, repo, root)],
            "verify_command": verifier(root), "accepted": False, "promotion_authority": False}


def process_metrics(pid: int) -> dict:
    status = Path(f"/proc/{pid}/status").read_text()
    match = re.search(r"^VmRSS:\s+(\d+) kB$", status, flags=re.MULTILINE)
    require(match is not None, "game RSS is unavailable")
    handles = len(tuple(Path(f"/proc/{pid}/fd").iterdir()))
    require(handles > 0, "game file descriptor inventory is unavailable")
    return {"rss_kib": int(match[1]), "process_fd_count": handles}


def check_raw(raw: dict, nonce: str, pid: int) -> None:
    require(raw.get("schema_version") == 2
            and raw.get("scope") == "production-raw-observations-v1"
            and raw.get("nonce") == nonce and raw.get("process_id") == pid
            and raw.get("failure") is None and raw.get("accepted") is False
            and raw.get("performance_evidence") is False
            and raw.get("promotion_authority") is False
            and isinstance(raw.get("samples"), list) and 3 <= len(raw["samples"]) <= 1200
            and isinstance(raw.get("events"), list) and len(raw["events"]) <= 4096,
            "ordinary-world raw observation differs")
    previous = -1.0
    for index, sample in enumerate(raw["samples"]):
        require(isinstance(sample, dict), "ordinary-world raw sample shape differs")
        moment = sample.get("real_seconds")
        require(sample.get("sample_index") == index and type(moment) in (float, int)
                and math.isfinite(moment) and moment >= previous,
                "ordinary-world raw sample order differs")
        previous = moment
    for index, event in enumerate(raw["events"]):
        require(isinstance(event, dict) and event.get("event_index") == index
                and type(event.get("sample_index")) is int
                and 0 <= event["sample_index"] <= len(raw["samples"]),
                "ordinary-world raw event order differs")


def check_registered_context(repo: Path, root: Path, plan: Path, context: dict) -> None:
    require(ownership.storage.primary_for(repo) == repo,
            "local probe ownership requires the primary checkout")
    require(set(context) == {"schema_version", "scope", "batch", "repo", "owner", "root",
                             "head", "source", "command", "verifier", "plan_sha256",
                             "runner_pid", "runner_identity", "coordinator_pid",
                             "coordinator_identity", "accepted", "performance_evidence",
                             "art_approved", "promotion_authority"}
            and context["schema_version"] == 1
            and context["scope"] == "normal-validation-runner-ownership-only"
            and type(context["runner_pid"]) is int and context["runner_pid"] > 0
            and isinstance(context["runner_identity"], str) and context["runner_identity"]
            and all(context[key] is False for key in
                    ("accepted", "performance_evidence", "art_approved", "promotion_authority")),
            "local probe ownership context shape differs")
    # The verifier can start before validation_storage.tracked_command records
    # its child. Only the exact former runner may be observed while that live
    # sealing parent publishes this verifier; a partial/foreign child never waits.
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
                    "local probe sealing parent identity differs")
            child = (pending.get("child_pid"), pending.get("child_identity"))
            if child == (os.getpid(), own_birth):
                break
            require(child == (context["runner_pid"], context["runner_identity"]),
                    "local probe sealing child is foreign or partial")
        require(time.monotonic() < deadline, "local probe verifier child was not published")
        time.sleep(0.02)
    with ownership.storage.locked(repo) as ledger:
        batch = ledger["batches"].get(context["batch"])
        require(batch is not None and batch["phase"] in
                {"running", "sealing", "finalized"}
                and batch["repo"] == context["repo"] == str(repo)
                and batch["owner"] == context["owner"] == OWNER
                and batch["roots"] == [context["root"]] == [str(root)]
                and batch["subject"] == context["head"]
                and batch["subject_fingerprint"] == context["source"]
                and batch["command"] == context["command"] == command(plan, repo, root)
                and batch["verify_command"] == context["verifier"] == verifier(root)
                and any(row["path"] == str(plan)
                        and row["sha256"] == context["plan_sha256"] == digest(plan)
                        for row in batch["command_files"]),
                "local probe registered ownership differs")
        if batch["phase"] == "running":
            require(batch["child_pid"] == context["runner_pid"]
                    and batch["child_identity"] == context["runner_identity"],
                    "local probe live runner identity differs")
        else:
            # tracked_command replaces child_pid with the *independent verifier*
            # during seal. The original runner remains represented by the
            # captured live context and the successful registered command exit.
            require(batch.get("exit_code") == 0, "local probe runner did not exit successfully")
            if batch["phase"] == "sealing":
                verifier_birth = ownership.storage.live_identity(os.getpid())
                require(batch["child_pid"] == os.getpid()
                        and verifier_birth is not None
                        and batch["child_identity"] is not None
                        and batch["child_identity"] == verifier_birth,
                        "local probe independent verifier identity differs")
            else:
                require(batch.get("result") == "pass", "local probe batch was not sealed as pass")


@native.activity_locked
def run(args: argparse.Namespace) -> dict:
    require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use the planned launcher")
    plan_path = Path(args.plan).absolute()
    value = json.loads(plan_path.read_bytes())
    repo, root, binary = check_plan(value, plan_path)
    require(repo == Path(args.repo).resolve() and root == Path(args.job_root).resolve(),
            "local probe recipe arguments differ")
    context = ownership.running_context(repo=repo, plan=plan_path, root=root, owner=OWNER,
                                        command=command(plan_path, repo, root), verifier=verifier(root))
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
    deadline = time.monotonic() + 45
    screenshot = root / "actual-window.png"
    measurements = []
    owned = None
    with (root / "game.log").open("w") as log:
        # Normal saves/settings are relative to cwd. Keep them away from the
        # player's files without changing the ordinary-world startup contract.
        game = subprocess.Popen([str(binary)], cwd=runtime, env=environment, stdout=log,
                                stderr=subprocess.STDOUT, start_new_session=True)
        try:
            while time.monotonic() < deadline:
                require(game.poll() is None, "ordinary-world game exited before observation")
                windows = native.x11_client_windows_for_process_tree(game.pid)
                require(len(windows) <= 1, "ordinary-world game has multiple owned windows")
                if windows:
                    owned = windows[0]
                    measurements.append(process_metrics(owned[1]))
                if owned and raw_path.is_file():
                    raw = json.loads(raw_path.read_bytes())
                    if len(raw.get("samples", [])) >= 3:
                        check_raw(raw, nonce, owned[1])
                        require(native.capture_client_png(owned[0], screenshot, import_cmd="import"),
                                "owned X11 client capture failed")
                        break
                time.sleep(0.25)
            else:
                raise native.AcceptanceError("ordinary-world process/window/raw timeout")
        finally:
            if game.poll() is None:
                native.stop_command_process(game)
    require(owned is not None and native.validate_png_structure(screenshot.read_bytes()) == WINDOW,
            "ordinary-world X11 client dimensions differ")
    require(measurements and all(row["rss_kib"] > 0 and row["process_fd_count"] > 0
                                 for row in measurements), "host RSS/fd samples missing")
    result = {"schema": 1, "profile": PROFILE, "status": "technical-pass", "plan": value,
              "context": context, "nonce": nonce, "game_pid": owned[1],
              "raw_sha256": digest(raw_path), "screenshot_sha256": digest(screenshot),
              "measurements": measurements, "gpu_metrics": None, "native_allocator_metrics": None,
              "application_handle_total": None, "accepted": False,
              "performance_evidence": False, "art_approved": False,
              "promotion_authority": False}
    (root / "result.json").write_text(json.dumps(result, sort_keys=True, separators=(",", ":")) + "\n")
    verify(root)
    return result


def verify(root: Path) -> dict:
    ownership.exact_path(root)
    result = json.loads((root / "result.json").read_bytes())
    value = result["plan"]
    plan_path = Path(result["context"]["command"][-5])
    require(value == json.loads(plan_path.read_bytes()), "local probe frozen plan differs")
    repo, expected, _ = check_plan(value, plan_path, require_fresh=False)
    check_registered_context(repo, root, plan_path, result["context"])
    require(root == expected and result["status"] == "technical-pass"
            and result["accepted"] is False and result["performance_evidence"] is False
            and result["art_approved"] is False and result["promotion_authority"] is False
            and result["gpu_metrics"] is None and result["native_allocator_metrics"] is None
            and result["application_handle_total"] is None, "local probe authority differs")
    raw_path, screenshot = root / "raw.json", root / "actual-window.png"
    require(digest(raw_path) == result["raw_sha256"]
            and digest(screenshot) == result["screenshot_sha256"]
            and native.validate_png_structure(screenshot.read_bytes()) == WINDOW,
            "local probe artifacts differ")
    check_raw(json.loads(raw_path.read_bytes()), result["nonce"], result["game_pid"])
    log = (root / "game.log").read_text()
    require(not re.search(r"\b(?:WARN|ERROR)\b", log), "ordinary-world game log has warnings/errors")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    require(len(adapters) == 1 and "intel" in adapters[0][0].casefold()
            and adapters[0][1] == "Vulkan", "ordinary-world adapter differs")
    require(result["context"]["repo"] == str(repo)
            and result["context"]["head"] == value["head"]
            and result["context"]["command"] == command(plan_path, repo, root)
            and result["context"]["accepted"] is False
            and result["measurements"]
            and all(row["rss_kib"] > 0 and row["process_fd_count"] > 0
                    for row in result["measurements"]), "local probe context or host measurements differ")
    return {"status": "technical-pass", "accepted": False, "scope": "ordinary-world host observation only"}


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
