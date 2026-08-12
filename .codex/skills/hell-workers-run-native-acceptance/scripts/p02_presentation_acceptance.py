#!/usr/bin/env python3
"""Run the P02 production actual-window presentation matrix."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402


SCHEMA_VERSION = 1
PROFILE = "p02-presentation-actual-window-v1"
QUALITIES = ("high", "medium", "low")
SCALE_FACTORS = (1.0, 1.5, 2.0)
VISIBILITY = (("visible", "gpu"), ("hidden", "cpu"))
EXPECTED_CASES = tuple(
    (quality, scale, visibility, render)
    for quality in QUALITIES
    for scale in SCALE_FACTORS
    for visibility, render in VISIBILITY
)
SCREENSHOTS = ("frame-a.png", "frame-b.png")
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
SETTLE_SECONDS = 5.0
FRAME_INTERVAL_SECONDS = 0.75
RUN_TIMEOUT_SECONDS = 90.0


def case_id(quality: str, scale: float, visibility: str) -> str:
    scale_label = str(scale).replace(".", "p")
    return f"quality-{quality}-dpi-{scale_label}-render3d-{visibility}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_single_csv(path: Path) -> dict[str, str]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
    except OSError as error:
        raise native.AcceptanceError(f"cannot read CSV artifact {path}: {error}") from error
    native.require(len(rows) == 1, f"{path.name} must contain exactly one row")
    return rows[0]


def image_statistics(path: Path) -> dict[str, float | int]:
    identify = native.shutil.which("identify")
    native.require(identify is not None, "P02 screenshot verification requires identify")
    completed = native.run_bounded_capture_tool(
        [
            identify,
            "-format",
            "%w %h %[mean] %[standard-deviation] %[entropy]",
            str(path),
        ],
        label=f"P02 screenshot statistics for {path.name}",
    )
    native.require(completed.returncode == 0, f"identify rejected {path.name}")
    fields = completed.stdout.strip().split()
    native.require(len(fields) == 5, f"identify returned malformed statistics for {path.name}")
    try:
        width, height = int(fields[0]), int(fields[1])
        mean, deviation, entropy = map(float, fields[2:])
    except ValueError as error:
        raise native.AcceptanceError(f"identify returned nonnumeric statistics for {path.name}") from error
    native.require(width >= 640 and height >= 360, "P02 client screenshot is unexpectedly small")
    native.require(mean > 500.0, "P02 client screenshot is effectively black")
    native.require(deviation > 500.0 and entropy > 0.02, "P02 client screenshot has no credible scene detail")
    return {
        "width": width,
        "height": height,
        "mean": mean,
        "standard_deviation": deviation,
        "entropy": entropy,
    }


def changed_pixels(first: Path, second: Path) -> int:
    compare = native.shutil.which("compare")
    native.require(compare is not None, "P02 animation verification requires compare")
    completed = native.run_bounded_capture_tool(
        [compare, "-metric", "AE", str(first), str(second), "null:"],
        label="P02 animation frame comparison",
    )
    output = (completed.stderr or completed.stdout).strip()
    match = re.search(r"[0-9]+", output)
    native.require(match is not None, "compare did not return an absolute pixel difference")
    return int(match.group(0))


def capture_client_window(destination: Path, *, root_pid: int) -> dict[str, Any] | None:
    candidates = native.x11_client_windows_for_process_tree(root_pid)
    if not candidates:
        return None
    native.require(len(candidates) == 1, "P02 run exposed more than one owned X11 client window")
    window_id, window_pid = candidates[0]
    import_cmd = native.shutil.which("import")
    native.require(import_cmd is not None, "P02 screenshot capture requires import")
    destination.parent.mkdir(parents=True, exist_ok=True)
    completed = native.run_bounded_capture_tool(
        [
            import_cmd,
            "-window",
            window_id,
            "-depth",
            "8",
            "-type",
            "TrueColor",
            str(destination),
        ],
        label=f"P02 X11 client screenshot {destination.name}",
    )
    if completed.returncode != 0 or not destination.is_file():
        destination.unlink(missing_ok=True)
        return None
    native.validate_png_structure(destination.read_bytes())
    statistics = image_statistics(destination)
    return {
        **statistics,
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "window_id": window_id,
        "window_pid": window_pid,
        "sha256": sha256(destination),
    }


def validate_presentation(run_dir: Path, *, render: str) -> dict[str, Any]:
    data = run_dir / "data"
    presentation = read_single_csv(data / "p02_presentation.csv")
    window = read_single_csv(data / "window.csv")
    indoor = read_single_csv(data / "indoor_light_fixture.csv")
    metadata = native.read_json(run_dir / "run-metadata.json")
    contract = metadata.get("rtt_light_contract")
    case = metadata.get("case")
    native.require(metadata.get("started_by") == "scripts/perf.py", "P02 case did not use the production performance runner")
    native.require(isinstance(case, dict) and case.get("workload") == "indoor-light" and case.get("render") == render, "P02 case did not use the production indoor-light fixture")
    native.require(isinstance(contract, dict) and contract.get("contract_id") == "rtt-light-v1" and contract.get("stage_id") == "p02" and contract.get("lane") == "static", "P02 case selected the wrong production contract")

    integers = {
        name: int(presentation[name])
        for name in (
            "layer_2d_camera_count",
            "layer_2d_pass_count",
            "building_count",
            "duplicate_presentation_count",
            "soul_count",
            "soul_billboard_count",
            "familiar_3d_count",
        )
    }
    native.require(integers["layer_2d_camera_count"] == 1, "P02 requires one 2D camera")
    native.require(integers["layer_2d_pass_count"] == 1, "P02 requires one 2D presentation pass")
    native.require(integers["building_count"] > 0, "P02 fixture has no buildings")
    native.require(integers["duplicate_presentation_count"] == 0, "P02 has duplicate presentations")
    native.require(presentation["building_exactly_one_presentation"] == "true", "P02 building presentation is missing")
    native.require(integers["familiar_3d_count"] == 0, "P02 retained a Familiar 3D proxy")
    native.require(presentation["state_and_bounce_probes_pass"] == "true", "P02 Door/state/bounce probes failed")
    expected_billboards = integers["soul_count"] if render == "gpu" else 0
    native.require(integers["soul_billboard_count"] == expected_billboards, "P02 Render3d visibility did not control Soul billboards")
    native.require(int(indoor["doors"]) >= 3, "P02 fixture does not cover Closed/Open/Locked Doors")
    native.require(window["window_present"] == "true", "P02 actual-window run was headless")
    native.require(window["resolved_window_backend"] == "x11", "P02 actual-window run did not resolve X11")
    native.require(window["adapter_backend"] == "vulkan", "P02 actual-window run did not resolve Vulkan")
    return {"presentation": presentation, "window": window, "fixture": indoor, "run_metadata": metadata}


def case_provenance_is_valid(observation: dict[str, Any]) -> bool:
    return (
        observation.get("production_subject") == "bevy_app:indoor-light/rtt-light-v1/p02/static"
        and observation.get("capture_scope") == native.SAVE_CATALOG_CAPTURE_SCOPE
    )


def locate_run(output: Path) -> Path:
    runs = list(output.glob("cases/*/run-001"))
    native.require(len(runs) == 1, f"P02 case expected one measured run, found {len(runs)}")
    validation = native.read_json(runs[0] / "validation.json")
    native.require(validation.get("valid") is True, "P02 performance runner rejected the production run")
    return runs[0]


def run_case(
    *,
    repo: Path,
    binary: Path,
    artifact: Path,
    quality: str,
    scale: float,
    visibility: str,
    render: str,
    adapter: str,
) -> dict[str, Any]:
    output = artifact / "performance"
    command = [
        "python3", "scripts/perf.py", "run",
        "--workload", "indoor-light", "--contract", "rtt-light-v1",
        "--stage", "p02", "--lane", "static", "--sizes", "medium",
        "--renders", render, "--seed", "20260803", "--repeat", "1",
        "--preflight-runs", "0", "--output", str(output),
        "--adapter", adapter, "--backend", "vulkan", "--window-backend", "x11",
        "--present-mode", "novsync", "--window-width", str(WINDOW_WIDTH),
        "--window-height", str(WINDOW_HEIGHT), "--window-scale-factor", str(scale),
        "--rtt-quality", quality, "--instrumentation", "capture",
        "--binary", str(binary), "--skip-build", "--warmup-secs", "8",
        "--measure-secs", "4", "--timeout-secs", str(int(RUN_TIMEOUT_SECONDS)),
    ]
    artifact.mkdir(parents=True, exist_ok=False)
    log_path = artifact / "run.log"
    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            command, cwd=repo, env=os.environ.copy(), stdout=log,
            stderr=subprocess.STDOUT, text=True, start_new_session=True,
            pass_fds=native.activity_pass_fds(os.environ),
        )
        first_seen: float | None = None
        captures: list[dict[str, Any]] = []
        deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
        try:
            while process.poll() is None and len(captures) < 2:
                windows = native.x11_client_windows_for_process_tree(process.pid)
                if windows and first_seen is None:
                    first_seen = time.monotonic()
                due = first_seen is not None and time.monotonic() - first_seen >= SETTLE_SECONDS
                if due and (not captures or time.monotonic() - captures[-1]["captured_at"] >= FRAME_INTERVAL_SECONDS):
                    screenshot = artifact / SCREENSHOTS[len(captures)]
                    evidence = capture_client_window(screenshot, root_pid=process.pid)
                    if evidence is not None:
                        captures.append({**evidence, "captured_at": time.monotonic()})
                if time.monotonic() >= deadline:
                    native.stop_command_process(process)
                    raise native.AcceptanceError(f"P02 case {artifact.name} timed out")
                time.sleep(0.2)
            returncode = process.wait()
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
    native.require(returncode == 0, f"P02 case {artifact.name} exited with {returncode}")
    native.require(len(captures) == 2, f"P02 case {artifact.name} did not yield two client screenshots")
    run_dir = locate_run(output)
    semantic = validate_presentation(run_dir, render=render)
    animation_pixels = changed_pixels(artifact / SCREENSHOTS[0], artifact / SCREENSHOTS[1])
    if visibility == "visible":
        native.require(animation_pixels >= 64, "P02 visible Render3d frames contain no animation evidence")
    result = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "case_id": artifact.name,
        "quality": quality,
        "scale_factor": scale,
        "render3d": visibility,
        "render_mode": render,
        "production_subject": "bevy_app:indoor-light/rtt-light-v1/p02/static",
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "screenshots": [{k: v for k, v in item.items() if k != "captured_at"} for item in captures],
        "changed_pixels": animation_pixels,
        "semantic": semantic,
    }
    native.atomic_write_json(artifact / "observation.json", result)
    return result


def verify_root(root: Path, *, subject_commit: str | None = None, source_fingerprint: str | None = None) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest.get("profile") == PROFILE, "P02 matrix profile differs")
    native.require(manifest.get("status") == "pass", "P02 matrix is not marked pass")
    if subject_commit is not None:
        native.require(manifest.get("subject_commit") == subject_commit, "P02 matrix subject differs")
    if source_fingerprint is not None:
        native.require(manifest.get("source_fingerprint") == source_fingerprint, "P02 matrix source differs")
    cases = manifest.get("cases")
    native.require(isinstance(cases, list) and len(cases) == len(EXPECTED_CASES), "P02 matrix is incomplete")
    expected_ids = {case_id(quality, scale, visibility) for quality, scale, visibility, _ in EXPECTED_CASES}
    actual_ids = {item.get("case_id") for item in cases if isinstance(item, dict)}
    native.require(actual_ids == expected_ids, "P02 matrix case set differs")
    for identifier in sorted(expected_ids):
        observation = native.read_json(root / "cases" / identifier / "observation.json")
        native.require(observation.get("status") == "pass", f"P02 case {identifier} is invalid")
        native.require(case_provenance_is_valid(observation), f"P02 case {identifier} is not production client-window evidence")
        for screenshot in SCREENSHOTS:
            path = root / "cases" / identifier / screenshot
            native.require(path.is_file() and not path.is_symlink(), f"P02 case {identifier} screenshot is missing")
            native.validate_png_structure(path.read_bytes())
    return {"status": "pass", "cases": len(expected_ids), "root": str(root)}


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    for command in ("xprop", "import", "identify", "compare"):
        if native.shutil.which(command) is None:
            failures.append(f"P02 actual-window acceptance requires {command}")
    subject = native.git_subject(repo)
    fingerprint = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    try:
        native.assert_clean_subject(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "p02-presentation")
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1",
        "python3", str(Path(__file__).resolve()), "run", "--repo", str(repo),
        "--job-root", str(root), "--subject-commit", subject,
        "--source-fingerprint", fingerprint, "--harness-fingerprint", harness,
        "--adapter", args.adapter,
    ]
    payload = {
        "schema_version": SCHEMA_VERSION,
        "status": "ready" if not failures else "blocked",
        "profile": PROFILE,
        "job_root": str(root),
        "subject_commit": subject,
        "source_fingerprint": fingerprint,
        "harness_fingerprint": harness,
        "failures": failures,
        "resources": resources,
        "launcher_command": command,
        "execution_contract": {
            "cases": len(EXPECTED_CASES),
            "qualities": list(QUALITIES),
            "scale_factors": list(SCALE_FACTORS),
            "render3d": [item[0] for item in VISIBILITY],
            "actual_window_required": True,
            "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
            "parallel_game_processes": 1,
            "actual_feature_builds": 1,
        },
    }
    native.print_json(payload)
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "P02 run must be launched by the planned direct kitty command")
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "P02 subject changed before launch")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "P02 source changed before launch")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "P02 harness changed before launch")
    native.assert_clean_subject(repo, args.subject_commit)
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state = {
        "schema_version": SCHEMA_VERSION, "status": "running", "profile": PROFILE,
        "repo": str(repo), "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "started_at": native.utc_now(), "cases_completed": 0,
    }
    native.atomic_write_json(root / "job.json", state)
    build_log = root / "build.log"
    build = ["python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling", "--no-default-features", "--features", "profiling"]
    native.run_command("build", build, repo=repo, env=os.environ.copy(), log_path=build_log, job_file=root / "job.json", state=state)
    binary = repo / "target/profiling/bevy_app"
    native.require(binary.is_file() and not binary.is_symlink(), "P02 profiling binary is missing")
    binary_hash = sha256(binary)
    results = []
    for quality, scale, visibility, render in EXPECTED_CASES:
        identifier = case_id(quality, scale, visibility)
        state["current_case"] = identifier
        native.atomic_write_json(root / "job.json", {**state, "heartbeat_at": native.utc_now()})
        results.append(run_case(repo=repo, binary=binary, artifact=root / "cases" / identifier, quality=quality, scale=scale, visibility=visibility, render=render, adapter=args.adapter))
        state["cases_completed"] = len(results)
    manifest = {
        "schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE,
        "repo": str(repo), "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "profiling_binary_sha256": binary_hash,
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "cases": results, "completed_at": native.utc_now(),
    }
    native.atomic_write_json(root / "manifest.json", manifest)
    verification = verify_root(root, subject_commit=args.subject_commit, source_fingerprint=args.source_fingerprint)
    native.atomic_write_json(root / "job.json", {**state, "status": "valid", "current_case": None, "verification": verification, "completed_at": native.utc_now()})
    native.print_json(verification)
    return 0


def self_test() -> int:
    native.require(len(EXPECTED_CASES) == 18, "P02 matrix must contain exactly 18 cases")
    native.require(len({case_id(q, s, v) for q, s, v, _ in EXPECTED_CASES}) == 18, "P02 case IDs must be unique")
    native.require({render for _, _, visibility, render in EXPECTED_CASES if visibility == "visible"} == {"gpu"}, "visible P02 cases must use GPU")
    native.require({render for _, _, visibility, render in EXPECTED_CASES if visibility == "hidden"} == {"cpu"}, "hidden P02 cases must omit Render3d")
    valid = {
        "production_subject": "bevy_app:indoor-light/rtt-light-v1/p02/static",
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
    }
    native.require(case_provenance_is_valid(valid), "P02 production provenance was rejected")
    for replacement in (
        {**valid, "production_subject": "visual_test"},
        {**valid, "capture_scope": "root-desktop"},
        {**valid, "capture_scope": "headless"},
    ):
        native.require(not case_provenance_is_valid(replacement), "P02 negative provenance unexpectedly passed")
    native.print_json({"status": "pass", "cases": len(EXPECTED_CASES)})
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--repo", required=True)
    run_parser.add_argument("--job-root", required=True)
    run_parser.add_argument("--subject-commit", required=True)
    run_parser.add_argument("--source-fingerprint", required=True)
    run_parser.add_argument("--harness-fingerprint", required=True)
    run_parser.add_argument("--adapter", default="Intel")
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    if args.command == "plan":
        return plan(args)
    if args.command == "run":
        for value, label, length in ((args.subject_commit, "subject", 40), (args.source_fingerprint, "source fingerprint", 64), (args.harness_fingerprint, "harness fingerprint", 64)):
            native.require(re.fullmatch(f"[0-9a-f]{{{length}}}", value) is not None, f"invalid P02 {label}")
        return run(args)
    if args.command == "verify":
        native.print_json(verify_root(Path(args.job_root).resolve()))
        return 0
    if args.command == "self-test":
        return self_test()
    raise native.AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        native.print_json({"status": "invalid", "error": str(error)})
        raise SystemExit(1) from error
