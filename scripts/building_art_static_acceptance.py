"""Native paused-building reference: six Capture runs, then three Memory runs.

Use through dev.py validation plan. This is a static reference, not evidence for
animation, live production, art approval, or the complete migration budget.
"""
from __future__ import annotations

import argparse
import os
import statistics
import sys
from dataclasses import asdict
from pathlib import Path

HELPERS = Path(__file__).resolve().parents[1] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPERS))
import native_acceptance as native  # noqa: E402
from wall_density_acceptance import asset_view_fingerprint, session_digest, sha256  # noqa: E402
from perf_tool.artifacts import frame_summary, read_frames, validate_run  # noqa: E402
from perf_tool.execution import read_native_memory, read_resource_usage  # noqa: E402
from perf_tool.model import Case  # noqa: E402

PROFILE = "building-art-static-nine-reference-v2"
MATRIX = (("capture", "small", 15), ("capture", "medium", 60), ("memory", "medium", 60))
SEED = 20260920


def command(root: Path, instrumentation: str, size: str, souls: int, adapter: str) -> list[str]:
    return ["python3", "scripts/perf.py", "run", "--workload", "building-art-static",
            "--sizes", size, "--renders", "gpu", "--seed", str(SEED), "--repeat", "3",
            "--preflight-runs", "0", "--souls", str(souls), "--familiars", "0",
            "--instrumentation", instrumentation, "--warmup-secs", "30", "--measure-secs", "60",
            "--backend", "vulkan", "--window-backend", "x11", "--present-mode", "novsync",
            "--window-width", "1280", "--window-height", "720", "--window-scale-factor", "1",
            "--rtt-quality", "high", "--adapter", adapter, "--timeout-secs", "300",
            "--output", str(root / f"{instrumentation}-{size}")]


def identity(repo: Path) -> dict:
    return {"subject_commit": native.git_subject(repo), "source_fingerprint": native.source_fingerprint(repo),
            "harness_fingerprint": native.native_harness_fingerprint(repo),
            "asset_view_fingerprint": asset_view_fingerprint(repo), "driver_sha256": sha256(Path(__file__))}


def require_identity(repo: Path, expected: dict) -> None:
    native.require(not native.git_dirty_paths(repo), "static reference requires a fully clean committed subject")
    native.require(identity(repo) == expected, "static reference source/driver/assets changed")


def check_root(repo: Path, root: Path) -> None:
    native.require(root.parent == repo / "target/native-acceptance", "job root must be a direct native-acceptance child")
    native.require_persistent_storage(root, label="building static reference")


def plan(args) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    frozen = identity(repo)
    if native.git_dirty_paths(repo):
        failures.append("commit the validated fixture before planning native reference")
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "building-art-static")
    check_root(repo, root)
    if root.exists():
        failures.append("job root already exists")
    launcher = ["kitty", "--directory", str(repo), "--detach", "env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
                "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()), "run",
                "--repo", str(repo), "--job-root", str(root), "--adapter", args.adapter]
    for key, value in frozen.items():
        launcher.extend(["--" + key.replace("_", "-"), value])
    native.print_json({"status": "blocked" if failures else "ready", "profile": PROFILE, "job_root": str(root),
                       **frozen, "resources": resources, "failures": failures, "launcher_command": launcher,
                       "status_command": ["python3", str(Path(__file__).resolve()), "status", "--job-root", str(root)],
                       "execution_contract": {"capture_runs": 6, "memory_runs": 3, "warmup_secs": 30,
                                              "measure_secs": 60, "parallel_processes": 1, "active_simulation": False}})
    return int(bool(failures))


def verify_session(root: Path, instrumentation: str, size: str, souls: int, frozen: dict, adapter: str) -> dict:
    session = root / f"{instrumentation}-{size}"
    manifest = native.read_json(session / "manifest.json")
    matrix = native.read_json(session / "matrix.json")
    case = Case("building-art-static", size, "gpu", SEED, souls, 0)
    expected_matrix = {"workload": case.workload, "sizes": [size], "renders": ["gpu"], "seed": SEED,
                       "repeat": 3, "preflight_runs": 0, "warmup_secs": 30.0, "measure_secs": 60.0,
                       "souls": souls, "familiars": 0, "familiar_policies": ["baseline"],
                       "operation_dialog_modes": ["hidden"], "dashboard_modes": ["hidden"],
                       "capture_kind": "frame-time", "clock_mode": "realtime", "window_width": 1280,
                       "window_height": 720, "window_scale_factor": 1.0, "rtt_quality": "high"}
    native.require(all(matrix.get(key) == value for key, value in expected_matrix.items()), "matrix differs")
    native.require(manifest.get("status") == "valid", "performance session is invalid")
    git, source, binary = (manifest.get(key, {}) for key in ("git", "source", "binary"))
    native.require(git.get("commit") == frozen["subject_commit"] and git.get("dirty_paths") == [], "subject differs")
    native.require(source.get("fingerprint_start") == source.get("fingerprint_end") == frozen["source_fingerprint"]
                   and source.get("unchanged") is True, "source provenance differs")
    native.require(binary.get("instrumentation") == instrumentation and len(binary.get("sha256", "")) == 64,
                   "binary instrumentation/hash differs")
    observations = []
    expected_runs = {f"run-{number:03d}" for number in range(1, 4)}
    case_dir = session / "cases" / case.identifier
    native.require({p.name for p in case_dir.iterdir() if p.is_dir()} == expected_runs, "run inventory differs")
    for number in range(1, 4):
        run = case_dir / f"run-{number:03d}"
        metadata = native.read_json(run / "run-metadata.json")
        native.require(metadata.get("case") == asdict(case) and metadata.get("returncode") == 0, "run metadata differs")
        validation = validate_run(run, returncode=0, expected_case=case, expected_adapter=adapter,
            expected_backend="vulkan", allow_log_patterns=[], capture_kind="frame-time",
            expected_warmup_secs=30.0, expected_measure_secs=60.0, expected_window_backend="x11",
            expected_present_mode="novsync", expected_window_width=1280, expected_window_height=720,
            expected_window_scale_factor=1.0, expected_rtt_quality="high")
        native.require(validation.valid, "raw validation failed: " + "; ".join(validation.reasons))
        samples, errors = read_frames(run / "data/frames.csv", int(validation.summary["samples"]))
        native.require(not errors and bool(samples), "raw frame samples invalid")
        fixture = native.read_json(run / "data/building_art_static.json")
        native.require(fixture["stable_frames"] >= len(samples), "fixture was not checked through every measured frame")
        metrics = frame_summary(samples)
        if instrumentation == "memory":
            allocation, allocation_errors = read_native_memory(run / "data/memory.csv", frame_samples=len(samples))
            process, process_errors = read_resource_usage(run / "resource-usage.txt")
            native.require(not allocation_errors + process_errors, "native memory evidence invalid")
            validation.profile_artifact = {
                "instrumentation": "memory", "allocation_memory": allocation, "process_memory": process}
            native.require(native.read_json(run / "profile-artifact.json") == validation.profile_artifact,
                "stored memory evidence differs from raw counters/time")
            metrics = {"peak_live_bytes": allocation["peak_live_bytes"], "max_rss_kib": process["max_rss_kib"]}
        else:
            native.require(not (run / "profile-artifact.json").exists(), "unexpected Capture profile artifact")
        native.require(native.read_json(run / "validation.json") == validation.to_json(), "stored validation differs")
        native.require(metadata.get("actual_adapter") == validation.adapter
                       and metadata.get("actual_window") == validation.window
                       and metadata.get("runtime_data_cleaned") is True, "run environment/cleanup differs")
        observations.append({"run": number, "metrics": metrics, "adapter": validation.adapter,
                             "layout_sha256": fixture["layout_sha256"]})
    native.require(len({row["layout_sha256"] for row in observations}) == 1, "repeated fixture differs")
    aggregate = {}
    for metric in observations[0]["metrics"]:
        values = [row["metrics"][metric] for row in observations]
        median = statistics.median(values)
        aggregate[metric] = {"median": median, "mad": statistics.median(abs(value - median) for value in values)}
    return {"instrumentation": instrumentation, "size": size, "binary_sha256": binary["sha256"],
            "session_sha256": session_digest(session), "observations": observations, "aggregate": aggregate}


def verify(root: Path) -> dict:
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest.get("profile") == PROFILE and manifest.get("status") == "pass", "reference manifest invalid")
    repo = native.validate_repo(manifest["repo"])
    require_identity(repo, manifest["identity"])
    sessions = [verify_session(root, *case, manifest["identity"], manifest["adapter"]) for case in MATRIX]
    native.require(sessions == manifest["sessions"], "reference aggregates/hash inventory differs")
    native.require(sessions[0]["binary_sha256"] == sessions[1]["binary_sha256"] != sessions[2]["binary_sha256"],
                   "Capture reuse or Memory instrumentation separation differs")
    native.require(sessions[1]["observations"][0]["layout_sha256"] == sessions[2]["observations"][0]["layout_sha256"],
                   "Capture/Memory fixture differs")
    return {"status": "pass", "profile": PROFILE, "subject_commit": manifest["identity"]["subject_commit"],
            "capture_runs": 6, "memory_runs": 3, "active_simulation": "not-measured", "sessions": sessions}


@native.activity_locked
def run(args) -> int:
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use the planned direct kitty launcher")
    repo, root = native.validate_repo(args.repo), Path(args.job_root).resolve()
    check_root(repo, root)
    frozen = {key: getattr(args, key) for key in ("subject_commit", "source_fingerprint", "harness_fingerprint",
                                               "asset_view_fingerprint", "driver_sha256")}
    require_identity(repo, frozen)
    resources = native.resource_snapshot(repo, require_launcher=False)
    native.require(not resources["failures"], str(resources["failures"]))
    root.mkdir(parents=True, exist_ok=False)
    state = {"profile": PROFILE, "status": "running", "repo": str(repo), "identity": frozen,
             "pid": os.getpid(), "started_at": native.utc_now(), "heartbeat_at": native.utc_now(), "resources": resources}
    native.atomic_write_json(root / "job.json", state)
    try:
        environment = native.cargo_environment(repo)
        sessions = []
        for instrumentation, size, souls in MATRIX:
            require_identity(repo, frozen)
            native.run_command(f"{instrumentation}-{size}", command(root, instrumentation, size, souls, args.adapter),
                repo=repo, env=environment, log_path=root / f"{instrumentation}-{size}.log",
                job_file=root / "job.json", state=state, timeout_seconds=3600)
            sessions.append(verify_session(root, instrumentation, size, souls, frozen, args.adapter))
        require_identity(repo, frozen)
        native.atomic_write_json(root / "manifest.json", {"profile": PROFILE, "status": "pass", "repo": str(repo),
            "identity": frozen, "adapter": args.adapter, "sessions": sessions})
        result = verify(root)
        native.update_state(root / "job.json", state, status="valid", child_pid=None, completed_at=native.utc_now())
        native.print_json(result)
        return 0
    except Exception as error:
        native.update_state(root / "job.json", state, status="invalid", failure=str(error), child_pid=None)
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("plan", "run", "status", "verify"):
        command_parser = commands.add_parser(name)
        command_parser.add_argument("--job-root", required=name != "plan")
        if name in {"plan", "run"}:
            command_parser.add_argument("--repo", required=True)
            command_parser.add_argument("--adapter", default="Intel")
        if name == "run":
            for key in ("subject-commit", "source-fingerprint", "harness-fingerprint", "asset-view-fingerprint", "driver-sha256"):
                command_parser.add_argument("--" + key, required=True)
    args = parser.parse_args()
    if args.command == "plan":
        return plan(args)
    if args.command == "run":
        return run(args)
    root = Path(args.job_root).resolve()
    if args.command == "verify":
        native.print_json(verify(root))
        return 0
    state = native.read_json(root / "job.json")
    if state.get("status") == "running":
        return native.status_command(argparse.Namespace(job_root=str(root), stale_after_secs=120))
    native.print_json({key: state.get(key) for key in ("status", "current_stage", "heartbeat_at", "child_pid", "failure")})
    if state.get("status") == "valid":
        verify(root)
        return 0
    return 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (native.AcceptanceError, OSError, ValueError, KeyError, TypeError) as error:
        print(f"building-art-static: {error}", file=sys.stderr)
        raise SystemExit(1) from error
