"""Building-art native introduction recipe, separate from Wall/Door acceptance.

Feedback is an incremental development view. Art-preview is clean-source visual
review without art approval. Candidate requires the separately art-approved set.
All three measure only the paused presentation fixture; they neither approve
art nor replace later per-kind lifecycle/state/performance acceptance. Cross-M1
cost comparison consumes two separately frozen static-reference jobs.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
HELPERS = ROOT / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPERS))
import building_asset_pipeline as pipeline  # noqa: E402
import native_acceptance as native  # noqa: E402
from wall_density_acceptance import asset_view_fingerprint  # noqa: E402
import building_art_static_acceptance as static  # noqa: E402

MODES = {"feedback": "art_preview", "art-preview": "art_preview", "candidate": "isolated_candidate"}
PROFILE = "building-art"


def subject(repo: Path) -> dict:
    return {"commit": native.git_subject(repo), "source": native.source_fingerprint(repo),
            "harness": native.native_harness_fingerprint(repo), "assets": asset_view_fingerprint(repo),
            "driver": pipeline.digest(Path(__file__).read_bytes()), "dirty": native.git_dirty_paths(repo)}


def validate_subject(value: dict, *, feedback: bool) -> None:
    pipeline.require(set(value) == {"commit", "source", "harness", "assets", "driver", "dirty"}
                     and re.fullmatch(r"[0-9a-f]{40}", value["commit"]) is not None
                     and all(re.fullmatch(r"[0-9a-f]{64}", value[k]) for k in ("source", "harness", "assets", "driver")),
                     "incomplete source identity")
    pipeline.require(feedback or value["dirty"] == [], "non-feedback requires a clean source")


def check_plan(value: dict, binary: Path) -> dict:
    pipeline.require(value["kind"] != "Bridge", "Bridge requires its independent M4 recipe; nine-kind static evidence is inapplicable")
    pipeline.require(value["profile"] == PROFILE and value["mode"] in MODES, "recipe mode differs")
    repo = native.validate_repo(value["repo"])
    validate_subject(value["subject"], feedback=value["mode"] == "feedback")
    pipeline.require(subject(repo) == value["subject"], "frozen recipe subject changed")
    root = pipeline.no_symlinks(Path(value["job_root"]))
    static.check_root(repo, root)
    manifest, text, _ = pipeline.load_set(Path(value["source_root"]), value["kind"], binary)
    pipeline.require(manifest["identity"] == value["identity"]
                     and pipeline.digest(text.encode()) == value["manifest_sha256"]
                     and manifest["identity"]["authority"] == MODES[value["mode"]], "candidate identity/authority changed")
    if value["mode"] == "candidate":
        # Reuse promotion's art approval/provenance checks without approving or
        # installing the set. This target is only a read-only planning namespace.
        pipeline.plan(Path(value["source_root"]), root / "approval-check", value["kind"], binary)
    return manifest


def plan(args) -> dict:
    repo = native.validate_repo(args.repo)
    frozen = subject(repo)
    validate_subject(frozen, feedback=args.mode == "feedback")
    root = Path(args.job_root).absolute() if args.job_root else native.unique_job_root(repo, "building-art")
    static.check_root(repo, root)
    pipeline.require(not root.exists(), "recipe job root already exists")
    manifest, text, _ = pipeline.load_set(args.source_root, args.kind, args.codec)
    value = {"schema_version": 1, "profile": PROFILE, "mode": args.mode, "repo": str(repo),
             "kind": args.kind, "source_root": str(args.source_root.absolute()), "job_root": str(root),
             "subject": frozen, "identity": manifest["identity"], "manifest_sha256": pipeline.digest(text.encode()),
             "adapter": args.adapter, "codec": str(args.codec.absolute()),
             "codec_sha256": pipeline.digest(args.codec.read_bytes()),
             "scope": "paused-static-presentation", "promotion_authority": False,
             "feedback_only": args.mode == "feedback"}
    check_plan(value, args.codec)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    pipeline.require(args.output is not None, "plan requires --output")
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3",
        str(repo / "scripts/building_art_acceptance.py"), "run", "--plan", str(args.output.absolute())]
    value["resources"] = resources
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def game_command(binary: Path, root: Path) -> list[str]:
    return [str(binary), "--perf-scenario", "--perf-seed", str(static.SEED), "--perf-size", "small",
        "--perf-workload", "building-art-static", "--perf-render", "gpu", "--perf-clock", "realtime",
        "--perf-familiar-policy", "baseline", "--perf-operation-dialog", "hidden", "--perf-dashboard", "hidden",
        "--perf-output-dir", str(root / "data"), "--perf-warmup-secs", "30", "--perf-measure-secs", "60",
        "--spawn-souls", "15", "--spawn-familiars", "0", "--perf-window-width", "1280", "--perf-window-height", "720",
        "--perf-window-scale-factor", "1", "--perf-rtt-quality", "high"]


def capture(root: Path, pid: int) -> dict | None:
    clients = native.x11_client_windows_for_process_tree(pid)
    if not clients:
        return None
    pipeline.require(len(clients) == 1, "building-art requires exactly one owned client")
    window, owner = clients[0]
    command = shutil.which("import")
    pipeline.require(command is not None, "ImageMagick import is unavailable")
    path = root / "actual-window.png"
    if not native.capture_client_png(window, path, import_cmd=command):
        return None
    pipeline.require(native.validate_png_structure(path.read_bytes()) == (1280, 720), "actual client dimensions differ")
    return {"path": path.name, "sha256": pipeline.digest(path.read_bytes()), "window_id": window,
            "window_pid": owner, "capture_scope": "owned-x11-client"}


def validate_probe(probe: dict, value: dict, nonce: str) -> None:
    pipeline.require(probe.get("profile") == PROFILE and probe.get("status") == "ready"
        and probe.get("mode") == value["mode"] and probe.get("identity") == value["identity"]
        and probe.get("nonce") == nonce and probe.get("scope") == value["scope"]
        and type(probe.get("stable_frames")) is int and probe["stable_frames"] >= 30,
        "building-art runtime acknowledgement differs")
    pipeline.require(probe["fixture"]["target_count"] == 36, "building-art fixture inventory differs")


def verify(root: Path) -> dict:
    result = pipeline.read(root / "manifest.json")
    value = result["plan"]
    binary = Path(result["binary"]["path"])
    check_plan(value, binary)
    pipeline.require(pipeline.digest(binary.read_bytes()) == result["binary"]["sha256"], "native binary changed")
    pipeline.require(result["profile"] == PROFILE and result["status"] == "pass"
                     and result["mode"] == value["mode"] and result["identity"] == value["identity"]
                     and result["feedback_only"] == (value["mode"] == "feedback")
                     and result["promotion_authority"] is False, "evidence authority differs")
    probe = json.loads((root / "probe.json").read_bytes())
    validate_probe(probe, value, result["nonce"])
    pipeline.require(probe == result["probe"], "sealed runtime probe changed")
    screenshot = pipeline.rooted(root, result["screenshot"]["path"])
    pipeline.require(pipeline.digest(screenshot.read_bytes()) == result["screenshot"]["sha256"]
                     and native.validate_png_structure(screenshot.read_bytes()) == (1280, 720), "native screenshot changed")
    sidecar = json.loads((root / "data/building_art_candidate.json").read_bytes())
    pipeline.require(sidecar["evidence_kind"] == "candidate-paused-static-only"
                     and sidecar["initial"] == sidecar["final"] == probe["fixture"]
                     and sidecar["stable_frames"] >= 30, "candidate fixture did not remain stable")
    log = (root / "actual-window.log").read_text()
    pipeline.require(not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log), "native log contains warning/error")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    pipeline.require(len(adapters) == 1 and value["adapter"].casefold() in adapters[0][0].casefold()
                     and adapters[0][1] == "Vulkan", "actual adapter/backend differs")
    return result


@native.activity_locked
def run(args) -> dict:
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use the planned no-prompt launcher")
    value = pipeline.read(args.plan)
    codec = Path(value["codec"])
    pipeline.require(pipeline.digest(codec.read_bytes()) == value["codec_sha256"], "planning codec changed")
    manifest = check_plan(value, codec)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    job = root / "job.json"
    state = {"status": "running", "profile": PROFILE, "pid": os.getpid(), "started_at": native.utc_now(),
             "heartbeat_at": native.utc_now(), "mode": value["mode"]}
    native.atomic_write_json(job, state)
    try:
        feedback = value["mode"] == "feedback"
        environment = native.cargo_environment(repo)
        build = ["python3", "scripts/dev.py", "feedback", "--build-only"] if feedback else [
            "python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
            "--no-default-features", "--features", "profiling"]
        native.run_command("build", build, repo=repo, env=environment, log_path=root / "build.log",
                           job_file=job, state=state, timeout_seconds=3600)
        binary = repo / "target" / ("debug" if feedback else "profiling") / "bevy_app"
        check_plan(value, binary)
        # An exact full base asset view is copied. No missing asset is invented
        # as a baseline and no production asset directory is modified.
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        pipeline.require(asset_view_fingerprint(view) == value["subject"]["assets"], "fallback asset view differs")
        for entry in manifest["artifacts"]:
            pipeline.put(pipeline.rooted(view / "assets", entry["path"]),
                         pipeline.file_bytes(Path(value["source_root"]), entry))
        pipeline.put(pipeline.rooted(view / "assets", pipeline.locator(value["kind"])),
                     pipeline.rooted(Path(value["source_root"]), pipeline.locator(value["kind"])).read_bytes(), replace=True)
        nonce = secrets.token_hex(16)
        for key in list(environment):
            if key.startswith(("HW_BUILDING_ART", "HW_WALL", "HW_DOOR", "HW_DISABLE_", "HW_PERF_")):
                del environment[key]
        environment.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
            "WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": value["adapter"],
            "HW_BUILDING_ART_SESSION": json.dumps({"mode": value["mode"], "identity": value["identity"],
                "locator": pipeline.locator(value["kind"]), "nonce": nonce, "status_path": str(root / "probe.json")})})
        native.admit_stage_start("actual-window", state=state, job_file=job)
        screenshot = None
        deadline = time.monotonic() + 300
        with (root / "actual-window.log").open("w") as log:
            process = subprocess.Popen(game_command(binary, root), cwd=repo, env=environment,
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                pass_fds=native.activity_pass_fds(environment))
            native.update_state(job, state, child_pid=process.pid, current_stage="actual-window")
            try:
                while process.poll() is None:
                    if screenshot is None and (root / "probe.json").exists():
                        probe = json.loads((root / "probe.json").read_bytes())
                        validate_probe(probe, value, nonce)
                        screenshot = capture(root, process.pid)
                    pipeline.require(time.monotonic() < deadline, "building-art native deadline exceeded")
                    native.update_state(job, state, heartbeat_at=native.utc_now())
                    time.sleep(0.5)
            finally:
                if process.poll() is None:
                    native.stop_command_process(process)
        pipeline.require(process.returncode == 0 and screenshot is not None, "native run/capture incomplete")
        check_plan(value, binary)
        result = {"schema_version": 1, "profile": PROFILE, "mode": value["mode"], "status": "pass",
            "identity": value["identity"], "scope": value["scope"], "feedback_only": feedback,
            "promotion_authority": False, "plan": value, "nonce": nonce,
            "probe": json.loads((root / "probe.json").read_bytes()), "screenshot": screenshot,
            "binary": {"path": str(binary), "sha256": pipeline.digest(binary.read_bytes())},
            "not_covered": ["active-simulation", "all-kind-states", "all-preview-consumers", "lifecycle", "performance-budget", "art-approval"]}
        pipeline.put(root / "manifest.json", pipeline.canonical(result))
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", failure=str(error), child_pid=None)
        raise


def verify_frozen_static(root: Path, manifest: dict, expected: dict) -> dict:
    """Run the sealed source's verifier with its own Python module inventory.

    Importing both drivers in this interpreter is insufficient: native_acceptance
    and perf_tool would still be cached from the comparison checkout. A separate
    process also keeps historical harness fingerprint definitions source-local.
    """
    repo = native.validate_repo(manifest["repo"])
    driver = pipeline.no_symlinks(repo / "scripts/building_art_static_acceptance.py")
    root = root.resolve()

    def unchanged() -> None:
        pipeline.require(native.git_subject(repo) == expected["subject_commit"]
                         and not native.git_dirty_paths(repo), "frozen verifier source is dirty or changed")
        pipeline.require(driver.is_file() and pipeline.digest(driver.read_bytes()) == expected["driver_sha256"],
                         "frozen verifier driver identity differs")
        pipeline.require(native.read_json(root / "manifest.json") == manifest
                         and manifest["identity"] == expected, "frozen job manifest/identity changed")

    unchanged()
    completed = subprocess.run(
        [sys.executable, "-B", "-E", "-s", str(driver), "verify", "--job-root", str(root)],
        cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        timeout=300, check=False,
    )
    # -B preserves frozen trees; -E/-s exclude inherited Python path overrides
    # and user-site modules. The script directory and its helper imports belong
    # to this source, not to the checkout running the comparison.
    pipeline.require(completed.returncode == 0, "frozen raw-job verification failed: " + completed.stderr)
    result = json.loads(completed.stdout)
    pipeline.require(isinstance(result, dict) and result.get("status") == "pass"
                     and result.get("profile") == manifest["profile"] == static.PROFILE
                     and result.get("subject_commit") == expected["subject_commit"]
                     and result.get("capture_runs") == 6 and result.get("memory_runs") == 3
                     and result.get("active_simulation") == "not-measured"
                     and result.get("sessions") == manifest["sessions"],
                     "frozen verifier result differs from the sealed raw job")
    unchanged()
    return result


def compare(baseline: Path, candidate: Path, baseline_identity: dict, candidate_identity: dict) -> dict:
    """Revalidate raw runs for two clean, separately built fallback subjects."""
    left = native.read_json(baseline / "manifest.json")
    right = native.read_json(candidate / "manifest.json")
    for manifest, expected in ((left, baseline_identity), (right, candidate_identity)):
        pipeline.require(manifest["identity"] == expected and set(expected) == {
            "subject_commit", "source_fingerprint", "harness_fingerprint", "asset_view_fingerprint", "driver_sha256"},
            "full frozen comparison identity differs")
        pipeline.require(re.fullmatch(r"[0-9a-f]{40}", expected["subject_commit"])
            and all(re.fullmatch(r"[0-9a-f]{64}", v) for k, v in expected.items() if k != "subject_commit"),
            "comparison identity is incomplete")
    pipeline.require(baseline_identity["subject_commit"] != candidate_identity["subject_commit"]
        and baseline_identity["source_fingerprint"] != candidate_identity["source_fingerprint"], "comparison requires distinct sources")
    pipeline.require(baseline_identity["asset_view_fingerprint"] == candidate_identity["asset_view_fingerprint"]
        and left["adapter"] == right["adapter"], "comparison assets/adapter differ")
    source_roles(Path(left["repo"]), Path(right["repo"]))
    checked = [verify_frozen_static(root, manifest, expected) for root, manifest, expected in (
        (baseline, left, baseline_identity), (candidate, right, candidate_identity))]
    rows = []
    for before, after in zip(checked[0]["sessions"], checked[1]["sessions"], strict=True):
        pipeline.require(before["instrumentation"] == after["instrumentation"] and before["size"] == after["size"]
            and before["binary_sha256"] != after["binary_sha256"], "comparison is not a separate-source binary pair")
        pipeline.require([r["layout_sha256"] for r in before["observations"]]
            == [r["layout_sha256"] for r in after["observations"]], "fallback fixture layout differs")
        pipeline.require([r["adapter"] for r in before["observations"]]
            == [r["adapter"] for r in after["observations"]], "actual comparison adapters differ")
        rows.append({"instrumentation": before["instrumentation"], "size": before["size"],
                     "before": before["aggregate"], "after": after["aggregate"]})
    return {"status": "compared", "profile": "building-art-foundation", "baseline": baseline_identity,
            "candidate": candidate_identity, "sessions": rows, "budget_decision": "not-evaluated",
            "scope": "paused-static-fallback-only", "promotion_authority": False}


def source_roles(baseline: Path, candidate: Path) -> None:
    marker = "crates/bevy_app/src/assets/building_asset_set/mod.rs"
    fixture = "crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/mod.rs"
    pipeline.require(not (baseline / marker).exists() and (baseline / fixture).is_file()
                     and (candidate / marker).is_file() and (candidate / fixture).is_file(),
                     "requires the pre-M1 fixture-only source and post-M1 foundation source")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify", "status", "compare"))
    for key in ("repo", "source-root", "job-root", "codec", "output", "plan", "baseline", "candidate", "baseline-identity", "candidate-identity"):
        parser.add_argument("--" + key, type=Path)
    parser.add_argument("--kind", choices=tuple(pipeline.KINDS))
    parser.add_argument("--mode", choices=tuple(MODES))
    parser.add_argument("--adapter", default="Intel")
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.repo, args.source_root, args.codec, args.kind, args.mode)), "incomplete recipe inputs")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    elif args.command == "verify":
        result = verify(args.job_root)
    elif args.command == "status":
        if native.read_json(args.job_root / "job.json").get("status") == "valid":
            verify(args.job_root)
        return native.status_command(argparse.Namespace(job_root=str(args.job_root), stale_after_secs=120))
    else:
        result = compare(args.baseline, args.candidate, pipeline.read(args.baseline_identity), pipeline.read(args.candidate_identity))
    native.print_json(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
