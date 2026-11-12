"""Owned-window group gallery, separate from performance and formal release."""
from __future__ import annotations

import argparse
import itertools
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys
import time

if __package__:
    from . import building_m6_acceptance as m6
    from . import building_group_art_observation as oracle
    from . import building_group_art_window as window_probe
    from . import building_native_executable as executable
else:
    import building_m6_acceptance as m6
    import building_group_art_observation as oracle
    import building_group_art_window as window_probe
    import building_native_executable as executable

native, pipeline, static = m6.native, m6.pipeline, m6.static
PROFILE = oracle.PROFILE
ENV = "HW_BUILDING_GROUP_ART_SESSION"


def matrix(connection_only=False):
    return [("high", 2.0, "normal")] if connection_only else list(itertools.product(
        ("high", "medium", "low"), (1.0, 1.5, 2.0), ("normal", "far")))


def build_command(value):
    pipeline.require(type(value["feedback_only"]) is bool and type(value["connection_only"]) is bool
        and (not value["feedback_only"] or value["connection_only"]), "feedback is connection-only, never a formal matrix")
    return ["python3", "scripts/dev.py", "feedback", "--build-only"] if value["feedback_only"] else [
        "python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
        "--no-default-features", "--features", "profiling"]


def label(case):
    quality, dpi, view = case
    return f"{quality}-dpi{dpi:g}-{view}"


def read_artifact(path):
    metadata = path.lstat()
    pipeline.require(path.is_file() and not path.is_symlink() and metadata.st_size <= 8 * 1024 * 1024,
                     "gallery JSON must be a bounded regular file")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            pipeline.require(key not in result, "duplicate gallery JSON key")
            result[key] = value
        return result
    def reject(value):
        raise ValueError("nonfinite gallery JSON number: " + value)
    value = json.loads(path.read_bytes(), object_pairs_hook=unique, parse_constant=reject)
    pipeline.require(isinstance(value, dict), "gallery JSON object required")
    return value


def driver_inventory():
    paths = (Path(__file__), Path(oracle.__file__), Path(m6.__file__), Path(m6.art.__file__), Path(window_probe.__file__), Path(executable.__file__))
    return {p.name: pipeline.digest(p.read_bytes()) for p in paths}


def check(value):
    repo = Path(value["repo"])
    pipeline.require(value["profile"] == PROFILE and value["status"] == "ready"
        and value["accepted"] is False and value["promotion_authority"] is False
        and value["performance_evidence"] is False and type(value["connection_only"]) is bool
        and type(value["feedback_only"]) is bool and (not value["feedback_only"] or value["connection_only"]),
        "invalid gallery scope")
    static.require_identity(repo, value["identity"])
    static.check_root(repo, Path(value["job_root"]))
    codec = Path(value["codec"])
    pipeline.require(pipeline.digest(codec.read_bytes()) == value["codec_sha256"]
        and driver_inventory() == value["drivers"] and value["numeric_verifiers"] == pipeline.numeric_verifier_inventory()
        and value["foreground_verifier_sha256"] == m6.art.foreground.verifier_sha256(), "gallery verifier/codec changed")
    assets = m6.inventory(repo, codec, value["candidate_roots"], value["group"])
    pipeline.require(pipeline.exact_json(assets, value["assets"])
        and pipeline.exact_json(m6.group_declaration(assets, value["group"]), value["group_declaration"]), "gallery candidate inputs changed")
    pipeline.require(value["cases"] == [list(c) for c in matrix(value["connection_only"])], "gallery matrix differs")


def plan(args):
    repo, codec = native.validate_repo(str(args.repo)), args.codec.resolve()
    root = args.job_root.resolve() if args.job_root else native.unique_job_root(repo, "building-group-art")
    static.check_root(repo, root)
    pipeline.require(not root.exists() and not args.output.resolve().is_relative_to(root), "fresh gallery root/external plan required")
    roots = pipeline.read(args.candidate_roots)
    assets = m6.inventory(repo, codec, roots, args.group)
    value = {"schema_version": 1, "profile": PROFILE, "status": "ready", "repo": str(repo),
        "job_root": str(root), "codec": str(codec), "codec_sha256": pipeline.digest(codec.read_bytes()),
        "identity": static.identity(repo), "group": args.group, "candidate_roots": roots,
        "assets": assets, "group_declaration": m6.group_declaration(assets, args.group),
        "drivers": driver_inventory(), "numeric_verifiers": pipeline.numeric_verifier_inventory(),
        "foreground_verifier_sha256": m6.art.foreground.verifier_sha256(), "adapter": args.adapter,
        "connection_only": args.connection_only, "feedback_only": args.feedback, "cases": [list(c) for c in matrix(args.connection_only)],
        "accepted": False, "promotion_authority": False, "performance_evidence": False}
    check(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.resolve())]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def session(value, case, root, nonce):
    quality, dpi, view = case
    return {"schema_version": 1, "contract_id": PROFILE, "bindings": value["group_declaration"],
        "nonce": nonce, "status_path": str(root / "probe.json"), "view": view, "quality": quality, "scale_factor": dpi}


def command(binary, root, case):
    quality, dpi, _ = case
    return [str(binary), "--perf-scenario", "--perf-seed", "20260920", "--perf-size", "small",
        "--perf-workload", "building-art-static", "--perf-render", "gpu", "--perf-clock", "realtime",
        "--perf-familiar-policy", "baseline", "--perf-operation-dialog", "hidden", "--perf-dashboard", "hidden",
        "--perf-output-dir", str(root / "data"), "--perf-warmup-secs", "30", "--perf-measure-secs", "60",
        "--spawn-souls", "15", "--spawn-familiars", "0", "--perf-window-width", str(oracle.WIDTH),
        "--perf-window-height", str(oracle.HEIGHT), "--perf-window-scale-factor", str(dpi), "--perf-rtt-quality", quality]


def environment(parent, value, declaration):
    env = {k: v for k, v in parent.items() if not k.startswith(("HW_BUILDING_", "HW_M2_", "HW_BRIDGE_",
        "HW_WALL", "HW_DOOR", "HW_DISABLE_", "HW_PERF_", "HW_NATIVE_UI_"))}
    env.update({ENV: json.dumps(declaration), "BEVY_ASSET_ROOT": str(Path(value["job_root"]) / "asset-view"),
        "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync", "WGPU_BACKEND": "vulkan",
        "WGPU_ADAPTER_NAME": value["adapter"], "RUST_LOG": "info,wgpu=error"})
    return env


def publish_ack(root, value):
    payload = pipeline.canonical(value)
    temporary = root / "capture-ack.partial"
    with temporary.open("xb") as file:
        file.write(payload)
        file.flush()
        os.fsync(file.fileno())
    os.link(temporary, root / "capture-ack.json")
    temporary.unlink()


def capture(root, pid):
    clients = native.x11_client_windows_for_process_tree(pid)
    pipeline.require(len(clients) == 1, "group gallery requires exactly one owned X11 client")
    window, owner = clients[0]
    executable = shutil.which("import")
    pipeline.require(executable is not None, "ImageMagick import is unavailable")
    path = root / "actual-window.png"
    pipeline.require(not path.exists() and native.capture_client_png(window, path, import_cmd=executable), "group client capture failed")
    payload = path.read_bytes()
    pipeline.require(native.validate_png_structure(payload) == (oracle.WIDTH, oracle.HEIGHT), "group capture extent differs")
    return {"path": path.name, "sha256": pipeline.digest(payload), "window_id": window,
        "window_pid": owner, "capture_scope": "owned-x11-client", "captured_at_unix_ms": time.time_ns() // 1_000_000}


def observe_window(root, pid, nonce):
    clients = native.x11_client_windows_for_process_tree(pid)
    if not clients:
        return
    pipeline.require(len(clients) == 1 and clients[0][1] == pid, "group geometry requires one exact game client")
    window, _ = clients[0]
    result = native.run_bounded_capture_tool([sys.executable, str(Path(window_probe.__file__).resolve()),
        "--window", window, "--pid", str(pid)], label="owned group client geometry", timeout_seconds=3)
    pipeline.require(result.returncode == 0, "owned group geometry failed: " + result.stderr[-1000:])
    geometry = json.loads(result.stdout)
    window_probe.validate(geometry, window, pid)
    path = root / "window-observations.json"
    history = read_artifact(path) if path.exists() else {"nonce": nonce, "pid": pid, "observations": []}
    pipeline.require(history["nonce"] == nonce and history["pid"] == pid, "foreign window geometry history")
    previous = history["observations"][-1] if history["observations"] else None
    if previous is None or previous["geometry"] != geometry:
        pipeline.require(len(history["observations"]) < 64, "group window geometry did not stabilize")
        history["observations"].append({"at_unix_ms": time.time_ns() // 1_000_000, "geometry": geometry})
        native.atomic_write_json(path, history)


def verify_case(value, case, binary_sha):
    root = Path(value["job_root"]) / "cases" / label(case)
    receipt = read_artifact(root / "case.json")
    probe, final, ack = (read_artifact(root / name) for name in ("probe.json", "final.json", "capture-ack.json"))
    pipeline.require(isinstance(receipt["nonce"], str) and re.fullmatch(r"[0-9a-f]{32}", receipt["nonce"]), "gallery nonce differs")
    expected = session(value, case, root, receipt["nonce"])
    shot = receipt["screenshot"]
    pipeline.require(all(type(obj[key]) is int for obj, keys in (
        (receipt, ("exit_code", "pid")), (ack, ("schema_version", "pid", "width", "height", "captured_at_unix_ms")),
        (shot, ("window_pid", "captured_at_unix_ms"))) for key in keys), "gallery receipt/ACK wire integer differs")
    pipeline.require(receipt["status"] == "pass" and receipt["exit_code"] == 0 and receipt["case"] == list(case)
        and receipt["binary_sha256"] == binary_sha and receipt["pid"] == probe["pid"], "gallery process receipt differs")
    oracle.validate(probe, final, expected, value, Path(value["job_root"]) / "asset-view")
    shot = receipt["screenshot"]
    png_path = root / "actual-window.png"
    pipeline.require(not png_path.is_symlink() and png_path.is_file()
        and png_path.stat().st_size <= oracle.WIDTH * oracle.HEIGHT * 5 + 65536, "gallery PNG must be bounded regular file")
    png = png_path.read_bytes()
    pipeline.require(set(ack) == {"schema_version", "nonce", "pid", "probe_sha256", "png_sha256", "window_id",
            "width", "height", "captured_at_unix_ms"}
        and ack["schema_version"] == 1 and ack["nonce"] == receipt["nonce"] and ack["pid"] == receipt["pid"]
        and ack["probe_sha256"] == final["probe_sha256"] == pipeline.digest((root / "probe.json").read_bytes())
        and final["ack_sha256"] == pipeline.digest((root / "capture-ack.json").read_bytes())
        and pipeline.exact_json(final["ack"], ack)
        and ack["png_sha256"] == shot["sha256"] == pipeline.digest(png)
        and ack["window_id"] == shot["window_id"] and re.fullmatch(r"0x[0-9a-fA-F]{1,16}", ack["window_id"])
        and int(ack["window_id"], 16) > 0 and shot["capture_scope"] == "owned-x11-client"
        and shot["path"] == "actual-window.png" and type(shot["window_pid"]) is int and shot["window_pid"] == receipt["pid"]
        and (ack["width"], ack["height"]) == native.validate_png_structure(png) == (oracle.WIDTH, oracle.HEIGHT)
        and probe["ready_at_unix_ms"] <= ack["captured_at_unix_ms"] == shot["captured_at_unix_ms"] <= final["completed_at_unix_ms"]
        and final["completed_at_unix_ms"] - ack["captured_at_unix_ms"] <= 30_000,
        "gallery capture/ACK/final binding differs")
    log = (root / "actual-window.log").read_text()
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    pipeline.require(not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log)
        and len(adapters) == 1 and value["adapter"].casefold() in adapters[0][0].casefold()
        and adapters[0][1] == "Vulkan", "gallery native log/adapter invalid")
    return {"case": list(case), "adapter": adapters[0][0], "screenshot_sha256": shot["sha256"]}


def verify(root):
    manifest = read_artifact(root / "manifest.json")
    value = manifest["plan"]
    pipeline.require(str(root) == value["job_root"] and manifest["profile"] == PROFILE
        and manifest["status"] == "pass" and manifest["accepted"] is False
        and manifest["performance_evidence"] is False and manifest["promotion_authority"] is False
        and type(manifest["feedback_only"]) is bool and manifest["feedback_only"] == value["feedback_only"],
        "gallery manifest scope differs")
    check(value)
    m6.check_view(value)
    binary = Path(manifest["binary"]["path"])
    if value["feedback_only"]:
        executable.validate_cache(binary, Path(value["repo"]), True, manifest["binary"]["sha256"])
    else:
        executable.validate(binary, root, manifest["binary"]["sha256"])
    rows = [verify_case(value, case, manifest["binary"]["sha256"]) for case in value["cases"]]
    pipeline.require(rows == manifest["observations"] and len({row["adapter"] for row in rows}) == 1,
        "gallery matrix actual environment differs")
    return manifest


@native.activity_locked
def run(args):
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use registered no-prompt launcher")
    value = pipeline.read(args.plan)
    pipeline.require(pipeline.exact_json(value, args.frozen_plan), "plan changed across activity admission")
    check(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    state = {"profile": PROFILE, "status": "running", "pid": os.getpid(), "started_at": native.utc_now(), "heartbeat_at": native.utc_now()}
    job = root / "job.json"
    native.atomic_write_json(job, state)
    try:
        env = native.cargo_environment(repo)
        build = build_command(value)
        native.run_command("build", build, repo=repo, env=env, log_path=root / "build.log",
            job_file=job, state=state, timeout_seconds=3600)
        check(value)
        binary = repo / "target" / ("debug" if value["feedback_only"] else "profiling") / "bevy_app"
        if not value["feedback_only"]:
            binary = executable.freeze(binary, root)
        binary_sha = pipeline.digest(binary.read_bytes())
        m6.prepare_view(value)
        rows = []
        for case in value["cases"]:
            pipeline.require(pipeline.exact_json(pipeline.read(args.plan), value), "gallery plan changed before child launch")
            check(value)
            m6.check_view(value)
            case_root = root / "cases" / label(case)
            case_root.mkdir(parents=True, exist_ok=False)
            nonce = secrets.token_hex(16)
            declaration = session(value, case, case_root, nonce)
            game_env = environment(env, value, declaration)
            native.admit_stage_start(label(case), state=state, job_file=job)
            screenshot = None
            deadline = time.monotonic() + 240
            with (case_root / "actual-window.log").open("x") as log:
                process = subprocess.Popen(command(binary, case_root, case), cwd=repo, env=game_env, stdout=log,
                    stderr=subprocess.STDOUT, start_new_session=True, pass_fds=native.activity_pass_fds(game_env))
                native.update_state(job, state, child_pid=process.pid, current_stage=label(case))
                try:
                    while process.poll() is None:
                        if screenshot is None:
                            observe_window(case_root, process.pid, nonce)
                        if screenshot is None and (case_root / "probe.json").exists():
                            probe = read_artifact(case_root / "probe.json")
                            pipeline.require(probe["pid"] == process.pid and probe["session"] == declaration
                                and probe["status"] == "ready" and probe["stable_frames"] >= 60, "foreign/early gallery probe")
                            # Let the prepared frame reach the actual compositor before capture.
                            time.sleep(1)
                            screenshot = capture(case_root, process.pid)
                            publish_ack(case_root, {"schema_version": 1, "nonce": nonce, "pid": process.pid,
                                "probe_sha256": pipeline.digest((case_root / "probe.json").read_bytes()),
                                "png_sha256": screenshot["sha256"], "window_id": screenshot["window_id"],
                                "width": oracle.WIDTH, "height": oracle.HEIGHT, "captured_at_unix_ms": screenshot["captured_at_unix_ms"]})
                        pipeline.require(time.monotonic() < deadline, "group gallery deadline exceeded")
                        native.update_state(job, state, heartbeat_at=native.utc_now())
                        time.sleep(.25)
                finally:
                    if process.poll() is None:
                        native.stop_command_process(process)
            pipeline.require(process.returncode == 0 and screenshot is not None, "group gallery run/capture incomplete")
            receipt = {"status": "pass", "case": list(case), "exit_code": process.returncode, "pid": process.pid,
                "nonce": nonce, "binary_sha256": binary_sha, "screenshot": screenshot}
            pipeline.put(case_root / "case.json", pipeline.canonical(receipt))
            rows.append(verify_case(value, case, binary_sha))
        check(value)
        m6.check_view(value)
        manifest = {"profile": PROFILE, "status": "pass", "plan": value,
            "binary": {"path": str(binary), "sha256": binary_sha}, "observations": rows,
            "accepted": False, "performance_evidence": False, "promotion_authority": False, "feedback_only": value["feedback_only"]}
        pipeline.put(root / "manifest.json", pipeline.canonical(manifest))
        pipeline.require(pipeline.exact_json(pipeline.read(args.plan), value), "gallery plan changed before verification")
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return manifest
    except BaseException as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    p = commands.add_parser("plan")
    p.add_argument("--repo", type=Path, required=True)
    p.add_argument("--codec", type=Path, required=True)
    p.add_argument("--candidate-roots", type=Path, required=True)
    p.add_argument("--group", choices=("m2", "m3", "m5"), required=True)
    p.add_argument("--adapter", default="Intel")
    p.add_argument("--job-root", type=Path)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--connection-only", action="store_true")
    p.add_argument("--feedback", action="store_true", help="connection-only dev cache; never formal matrix evidence")
    p = commands.add_parser("run")
    p.add_argument("--plan", type=Path, required=True)
    p = commands.add_parser("verify")
    p.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = Path(value["repo"]), Path(value["job_root"])
        args.frozen_plan = value
    result = plan(args) if args.action == "plan" else run(args) if args.action == "run" else verify(args.root)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
