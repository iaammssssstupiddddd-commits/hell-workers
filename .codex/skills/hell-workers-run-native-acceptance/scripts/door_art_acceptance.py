#!/usr/bin/env python3
"""Capture and fail-closed-verify the approved Door quality/DPI gallery."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import secrets
import subprocess
import sys
import time
from dataclasses import asdict
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import door_behavior_acceptance as behavior  # noqa: E402
import native_acceptance as native  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 1
STATUS_SCHEMA_VERSION = 2
PROFILE = "door-art-v1-quality"
SEED = 20_260_906
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WARMUP_SECONDS = 10.0
MEASURE_SECONDS = 10.0
RUN_TIMEOUT_SECONDS = 120.0
POLL_SECONDS = 0.10
QUALITIES = ("high", "medium", "low")
SCALE_FACTORS = (1.0, 1.5, 2.0)
CHECKPOINTS = (
    {
        "zoom": "standard",
        "phase": "door-gallery-standard",
        "generation": 1,
        "camera_scale": 0.5,
        "screenshot": "door-standard.png",
    },
    {
        "zoom": "farthest",
        "phase": "door-gallery-farthest",
        "generation": 2,
        "camera_scale": 5.0,
        "screenshot": "door-farthest.png",
    },
)
ENV_KEYS = (
    "HW_DOOR_ART_PREVIEW",
    "HW_DOOR_CANDIDATE",
    "HW_DOOR_CANDIDATE_GENERATION",
    "HW_DOOR_CANDIDATE_MANIFEST_SHA256",
    "HW_DOOR_ART_ACTUAL_WINDOW",
    "HW_DOOR_ART_STATUS_PATH",
    "HW_DOOR_ART_ACK_PATH",
    "HW_DOOR_ART_SESSION_NONCE",
)

Image = tuple[int, int, bytes]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def matrix_cases() -> list[dict[str, Any]]:
    return [
        {
            "id": f"quality-{quality}-dpi-{str(scale).replace('.', 'p')}",
            "quality": quality,
            "scale_factor": scale,
        }
        for quality in QUALITIES
        for scale in SCALE_FACTORS
    ]


def build_command() -> list[str]:
    return [
        "python3",
        "scripts/dev.py",
        "cargo",
        "--",
        "build",
        "--profile",
        "profiling",
        "--no-default-features",
        "--features",
        "profiling",
    ]


def gallery_command(
    repo: Path,
    root: Path,
    adapter: str,
    *,
    quality: str,
    scale_factor: float,
) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "wall-density",
        "--wall-phase",
        "completed",
        "--sizes",
        "small",
        "--renders",
        "gpu",
        "--seed",
        str(SEED),
        "--repeat",
        "1",
        "--preflight-runs",
        "0",
        "--souls",
        "0",
        "--familiars",
        "0",
        "--output",
        str(root / "performance"),
        "--adapter",
        adapter,
        "--backend",
        "vulkan",
        "--window-backend",
        "x11",
        "--present-mode",
        "novsync",
        "--window-width",
        str(WINDOW_WIDTH),
        "--window-height",
        str(WINDOW_HEIGHT),
        "--window-scale-factor",
        str(scale_factor),
        "--rtt-quality",
        quality,
        "--instrumentation",
        "capture",
        "--binary",
        str(repo / "target/profiling/bevy_app"),
        "--skip-build",
        "--warmup-secs",
        str(WARMUP_SECONDS),
        "--measure-secs",
        str(MEASURE_SECONDS),
        "--timeout-secs",
        str(int(RUN_TIMEOUT_SECONDS)),
    ]


def require_object(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    native.require(isinstance(value, dict), f"{label} must be an object")
    native.require(set(value) == fields, f"{label} fields differ")
    return value


def require_number(value: Any, label: str) -> float:
    native.require(type(value) in {int, float}, f"{label} must be numeric")
    number = float(value)
    native.require(math.isfinite(number), f"{label} must be finite")
    return number


def validate_status(
    value: Any,
    *,
    nonce: str,
    candidate: dict[str, Any],
    quality: str,
    scale_factor: float,
    checkpoint: dict[str, Any],
) -> dict[str, Any]:
    status = require_object(
        value,
        "Door gallery status",
        {
            "schema_version",
            "status",
            "phase",
            "generation",
            "session_nonce",
            "evidence_kind",
            "checkpoint",
            "window",
            "render",
            "candidate_identity",
            "gallery",
        },
    )
    native.require(
        status["schema_version"] == STATUS_SCHEMA_VERSION
        and status["status"] == "ready"
        and status["session_nonce"] == nonce
        and status["phase"] == checkpoint["phase"]
        and status["generation"] == checkpoint["generation"]
        and status["evidence_kind"] == "isolated_candidate",
        "Door gallery status identity differs",
    )
    native.require(
        status["checkpoint"]
        == {
            "zoom": checkpoint["zoom"],
            "camera_scale": checkpoint["camera_scale"],
        },
        "Door gallery checkpoint differs",
    )
    native.require(
        status["window"]
        == {
            "width": WINDOW_WIDTH,
            "height": WINDOW_HEIGHT,
            "scale_factor": scale_factor,
        },
        "Door gallery window differs",
    )
    native.require(
        status["render"]
        == {"backend": "vulkan", "render3d": "visible", "rtt_quality": quality},
        "Door gallery render contract differs",
    )
    identity = require_object(
        status["candidate_identity"],
        "Door runtime candidate identity",
        {"asset_set_generation", "authority", "manifest_sha256"},
    )
    native.require(
        identity
        == {
            "asset_set_generation": candidate["asset_set_generation"],
            "authority": "IsolatedCandidate",
            "manifest_sha256": candidate["manifest_sha256"],
        },
        "Door runtime candidate identity differs",
    )
    gallery = require_object(
        status["gallery"],
        "Door gallery",
        {"production_count", "fallback_count", "targets"},
    )
    native.require(
        gallery["production_count"] == 6 and gallery["fallback_count"] == 0,
        "Door gallery presentation counts differ",
    )
    targets = gallery["targets"]
    native.require(isinstance(targets, list) and len(targets) == 6, "Door targets differ")
    expected = {
        (axis, state)
        for axis in ("EastWest", "NorthSouth")
        for state in ("Closed", "Open", "Locked")
    }
    observed: set[tuple[str, str]] = set()
    for target in targets:
        target = require_object(
            target,
            "Door target",
            {"axis", "grid", "state", "world", "viewport_center", "roi"},
        )
        observed.add((target["axis"], target["state"]))
        native.require(
            isinstance(target["grid"], list)
            and len(target["grid"]) == 2
            and all(type(item) is int for item in target["grid"]),
            "Door target grid differs",
        )
        native.require(
            isinstance(target["world"], list)
            and len(target["world"]) == 3
            and all(type(item) in {int, float} for item in target["world"]),
            "Door target world position differs",
        )
        center = require_object(
            target["viewport_center"], "Door target viewport", {"x", "y"}
        )
        require_number(center["x"], "Door target viewport x")
        require_number(center["y"], "Door target viewport y")
        roi = require_object(target["roi"], "Door target ROI", {"x", "y", "width", "height"})
        native.require(
            all(type(roi[field]) is int for field in roi)
            and roi["width"] >= 6
            and roi["height"] >= 6
            and roi["x"] >= 0
            and roi["y"] >= 0
            and roi["x"] + roi["width"] <= WINDOW_WIDTH
            and roi["y"] + roi["height"] <= WINDOW_HEIGHT,
            "Door target ROI differs",
        )
    native.require(observed == expected, "Door target state/axis coverage differs")
    return status


def read_image(path: Path) -> Image:
    native.require(path.is_file() and not path.is_symlink(), f"Door PNG is absent: {path}")
    payload = path.read_bytes()
    native.validate_png_structure(payload)
    width, height, pixels = native.decode_png_rgb(payload)
    native.require(len(pixels) == width * height * 3, "Door PNG decode length differs")
    native.require((width, height) == (WINDOW_WIDTH, WINDOW_HEIGHT), "Door PNG size differs")
    return width, height, pixels


def crop_pixels(image: Image, roi: dict[str, int]) -> bytes:
    width, _height, pixels = image
    output = bytearray()
    for y in range(roi["y"], roi["y"] + roi["height"]):
        start = (y * width + roi["x"]) * 3
        end = start + roi["width"] * 3
        output.extend(pixels[start:end])
    return bytes(output)


def crop_stats(payload: bytes) -> dict[str, Any]:
    native.require(payload, "Door ROI is empty")
    mean = sum(payload) / len(payload)
    deviation = math.sqrt(sum((value - mean) ** 2 for value in payload) / len(payload))
    pixels = [payload[index : index + 3] for index in range(0, len(payload), 3)]
    colored = sum(max(pixel) - min(pixel) >= 8 for pixel in pixels)
    non_black = sum(max(pixel) >= 18 for pixel in pixels)
    native.require(non_black >= max(1, len(pixels) // 20), "Door ROI is effectively black")
    native.require(deviation >= 2.0, "Door ROI contains no credible detail")
    return {
        "mean": round(mean, 6),
        "standard_deviation": round(deviation, 6),
        "colored_pixels": colored,
        "non_black_pixels": non_black,
    }


def mean_absolute_difference(left: bytes, right: bytes) -> float:
    native.require(len(left) == len(right) and left, "Door ROI comparison shape differs")
    return sum(abs(a - b) for a, b in zip(left, right, strict=True)) / len(left)


def image_evidence(image: Image, status: dict[str, Any]) -> dict[str, Any]:
    crops: dict[tuple[str, str], bytes] = {}
    target_stats: dict[str, Any] = {}
    for target in status["gallery"]["targets"]:
        key = (target["axis"], target["state"])
        payload = crop_pixels(image, target["roi"])
        crops[key] = payload
        target_stats[f"{key[0]}:{key[1]}"] = crop_stats(payload)
    threshold = 0.5 if status["checkpoint"]["zoom"] == "farthest" else 2.0
    differences: dict[str, float] = {}
    for axis in ("EastWest", "NorthSouth"):
        for left, right in (("Closed", "Open"), ("Closed", "Locked")):
            difference = mean_absolute_difference(crops[(axis, left)], crops[(axis, right)])
            native.require(
                difference >= threshold,
                f"Door {axis} {left}/{right} cues are not distinguishable at "
                f"{status['checkpoint']['zoom']} zoom: {difference:.3f} < {threshold:.3f}",
            )
            differences[f"{axis}:{left}-{right}"] = round(difference, 6)
    return {"targets": target_stats, "state_mean_absolute_difference": differences}


def capture_client_window(
    destination: Path, *, root_pid: int, status: dict[str, Any]
) -> dict[str, Any] | None:
    candidates = native.x11_client_windows_for_process_tree(root_pid)
    if not candidates:
        return None
    native.require(len(candidates) == 1, "Door gallery exposed multiple X11 clients")
    window_id, window_pid = candidates[0]
    import_command = native.shutil.which("import")
    native.require(import_command is not None, "Door gallery requires ImageMagick import")
    completed = native.run_bounded_capture_tool(
        [
            import_command,
            "-window",
            window_id,
            "-depth",
            "8",
            "-type",
            "TrueColor",
            str(destination),
        ],
        label="Door gallery X11 client screenshot",
    )
    if completed.returncode != 0 or not destination.is_file():
        destination.unlink(missing_ok=True)
        return None
    evidence = image_evidence(read_image(destination), status)
    return {
        "file": destination.name,
        "sha256": sha256(destination),
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "window_id": window_id,
        "window_pid": window_pid,
        "image_evidence": evidence,
    }


def acknowledgement(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "captured",
        "session_nonce": status["session_nonce"],
        "phase": status["phase"],
        "generation": status["generation"],
    }


def verify_performance(
    *,
    repo: Path,
    output: Path,
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
    quality: str,
    scale_factor: float,
) -> dict[str, Any]:
    Case, validate_run = density.load_perf_modules(repo)
    case = Case("wall-density", "small", "gpu", SEED, 0, 0, wall_phase="completed")
    run_dir = output / "cases" / case.identifier / "run-001"
    metadata = native.read_json(run_dir / "run-metadata.json")
    native.require(metadata.get("case") == asdict(case), "Door gallery case metadata differs")
    native.require(metadata.get("returncode") == 0, "Door gallery process failed")
    validation = validate_run(
        run_dir,
        returncode=0,
        expected_case=case,
        expected_adapter=adapter,
        expected_backend="vulkan",
        allow_log_patterns=[],
        capture_kind="frame-time",
        expected_warmup_secs=WARMUP_SECONDS,
        expected_measure_secs=MEASURE_SECONDS,
        expected_window_backend="x11",
        expected_present_mode="novsync",
        expected_window_width=WINDOW_WIDTH,
        expected_window_height=WINDOW_HEIGHT,
        expected_window_scale_factor=scale_factor,
        expected_rtt_quality=quality,
    )
    native.require(
        validation.valid,
        "Door gallery raw performance validation failed: " + "; ".join(validation.reasons),
    )
    native.require(
        native.read_json(run_dir / "validation.json") == validation.to_json(),
        "Door gallery stored validation differs",
    )
    manifest = native.read_json(output / "manifest.json")
    native.require(manifest.get("status") == "valid", "Door gallery performance is invalid")
    native.require(manifest.get("git", {}).get("commit") == subject_commit, "Door subject differs")
    native.require(manifest.get("git", {}).get("dirty_paths") == [], "Door run recorded dirty paths")
    native.require(
        manifest.get("source", {}).get("fingerprint_start") == source_fingerprint
        and manifest.get("source", {}).get("fingerprint_end") == source_fingerprint
        and manifest.get("source", {}).get("unchanged") is True,
        "Door source provenance differs",
    )
    native.require(manifest.get("binary", {}).get("sha256") == binary_sha256, "Door binary differs")
    return {"case_id": case.identifier, "validation": validation.to_json()}


def run_case(
    *,
    repo: Path,
    root: Path,
    job_file: Path,
    state: dict[str, Any],
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
    candidate: dict[str, Any],
    quality: str,
    scale_factor: float,
) -> dict[str, Any]:
    status_path = root / "probe-status.json"
    ack_path = root / "probe-ack.json"
    nonce = secrets.token_hex(16)
    environment = os.environ.copy()
    for key in ENV_KEYS:
        environment.pop(key, None)
    environment.update(
        {
            "HW_DOOR_CANDIDATE": "1",
            "HW_DOOR_CANDIDATE_GENERATION": str(candidate["asset_set_generation"]),
            "HW_DOOR_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
            "HW_DOOR_ART_ACTUAL_WINDOW": "1",
            "HW_DOOR_ART_STATUS_PATH": str(status_path),
            "HW_DOOR_ART_ACK_PATH": str(ack_path),
            "HW_DOOR_ART_SESSION_NONCE": nonce,
        }
    )
    command = gallery_command(
        repo, root, adapter, quality=quality, scale_factor=scale_factor
    )
    state.setdefault("commands", []).append({"stage": "capture", "argv": command})
    state["current_stage"] = "capture"
    native.atomic_write_json(job_file, state)
    deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
    observations: list[dict[str, Any]] = []
    log_path = root / "capture.log"
    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            command,
            cwd=repo,
            env=environment,
            stdout=log,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
            pass_fds=native.activity_pass_fds(os.environ),
        )
        state["child_pid"] = process.pid
        native.atomic_write_json(job_file, state)
        try:
            while process.poll() is None:
                if status_path.is_file() and not status_path.is_symlink():
                    value = native.read_json(status_path)
                    if isinstance(value, dict) and value.get("status") == "failed":
                        raise native.AcceptanceError(
                            f"Door gallery probe failed: {value.get('reason')}"
                        )
                    next_checkpoint = CHECKPOINTS[len(observations)] if len(observations) < 2 else None
                    if next_checkpoint is not None and value.get("phase") == next_checkpoint["phase"]:
                        status = validate_status(
                            value,
                            nonce=nonce,
                            candidate=candidate,
                            quality=quality,
                            scale_factor=scale_factor,
                            checkpoint=next_checkpoint,
                        )
                        screenshot = root / next_checkpoint["screenshot"]
                        evidence = capture_client_window(
                            screenshot, root_pid=process.pid, status=status
                        )
                        if evidence is not None:
                            observations.append(
                                {"checkpoint": next_checkpoint, "status": status, "screenshot": evidence}
                            )
                            native.atomic_write_json(ack_path, acknowledgement(status))
                if time.monotonic() >= deadline:
                    native.stop_command_process(process)
                    raise native.AcceptanceError("Door gallery capture timed out")
                state["heartbeat_at"] = native.utc_now()
                native.atomic_write_json(job_file, state)
                time.sleep(POLL_SECONDS)
            returncode = process.wait()
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
            ack_path.unlink(missing_ok=True)
    native.require(returncode == 0, f"Door gallery process exited with {returncode}")
    native.require(len(observations) == 2, "Door gallery did not capture both zoom checkpoints")
    state["child_pid"] = None
    native.atomic_write_json(job_file, state)
    performance = verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=adapter,
        subject_commit=subject_commit,
        source_fingerprint=source_fingerprint,
        binary_sha256=binary_sha256,
        quality=quality,
        scale_factor=scale_factor,
    )
    observation = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "quality": quality,
        "scale_factor": scale_factor,
        "session_nonce": nonce,
        "checkpoints": observations,
        "performance": performance,
    }
    native.atomic_write_json(root / "observation.json", observation)
    return observation


def verify_case(
    *, root: Path, repo: Path, manifest: dict[str, Any], spec: dict[str, Any]
) -> dict[str, Any]:
    observation = native.read_json(root / "observation.json")
    native.require(
        observation.get("schema_version") == SCHEMA_VERSION
        and observation.get("status") == "pass"
        and observation.get("profile") == PROFILE
        and observation.get("quality") == spec["quality"]
        and observation.get("scale_factor") == spec["scale_factor"],
        "Door observation identity differs",
    )
    checkpoints = observation.get("checkpoints")
    native.require(isinstance(checkpoints, list) and len(checkpoints) == 2, "Door checkpoints differ")
    hashes: dict[str, str] = {}
    for recorded, expected in zip(checkpoints, CHECKPOINTS, strict=True):
        native.require(recorded.get("checkpoint") == expected, "Door checkpoint contract differs")
        status = validate_status(
            recorded.get("status"),
            nonce=observation["session_nonce"],
            candidate=manifest["candidate_identity"],
            quality=spec["quality"],
            scale_factor=spec["scale_factor"],
            checkpoint=expected,
        )
        screenshot = recorded.get("screenshot")
        native.require(
            isinstance(screenshot, dict) and screenshot.get("file") == expected["screenshot"],
            "Door screenshot identity differs",
        )
        path = root / expected["screenshot"]
        native.require(sha256(path) == screenshot.get("sha256"), "Door screenshot hash differs")
        native.require(
            image_evidence(read_image(path), status) == screenshot.get("image_evidence"),
            "Door screenshot evidence differs",
        )
        hashes[expected["zoom"]] = screenshot["sha256"]
    performance = verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=manifest["adapter"],
        subject_commit=manifest["subject_commit"],
        source_fingerprint=manifest["source_fingerprint"],
        binary_sha256=manifest["binary_sha256"],
        quality=spec["quality"],
        scale_factor=spec["scale_factor"],
    )
    native.require(performance == observation.get("performance"), "Door performance evidence differs")
    return {"screenshots": hashes}


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "Door quality manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    behavior.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(native.source_fingerprint(repo) == manifest["source_fingerprint"], "Door source changed")
    native.require(
        native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"],
        "Door harness changed",
    )
    native.require(
        density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"],
        "Door asset view changed",
    )
    native.require(behavior.candidate_identity(repo) == manifest["candidate_identity"], "Door candidate changed")
    binary = repo / "target/profiling/bevy_app"
    native.require(sha256(binary) == manifest["binary_sha256"], "Door binary changed")
    specs = matrix_cases()
    native.require(manifest.get("cases") == specs, "Door quality matrix differs")
    hashes: dict[str, Any] = {}
    for spec in specs:
        result = verify_case(root=root / "cases" / spec["id"], repo=repo, manifest=manifest, spec=spec)
        hashes[spec["id"]] = result["screenshots"]
    native.require(manifest.get("screenshot_sha256") == hashes, "Door screenshot hashes differ")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "cases": len(specs),
        "screenshots": len(specs) * len(CHECKPOINTS),
        "production_doors_per_checkpoint": 6,
        "fallback_doors": 0,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    try:
        behavior.assert_fully_clean(repo, subject)
        candidate = behavior.candidate_identity(repo)
        assets = density.asset_view_fingerprint(repo)
    except native.AcceptanceError as error:
        failures.append(str(error))
        candidate = None
        assets = None
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "door-art")
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty",
        "--directory",
        str(repo),
        "--detach",
        "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
        "PYTHONDONTWRITEBYTECODE=1",
        "python3",
        str(Path(__file__).resolve()),
        "run",
        "--repo",
        str(repo),
        "--job-root",
        str(root),
        "--subject-commit",
        subject,
        "--source-fingerprint",
        source,
        "--harness-fingerprint",
        harness,
        "--asset-view-fingerprint",
        assets or "0" * 64,
        "--adapter",
        args.adapter,
    ]
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": PROFILE,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "status_command": ["python3", str(Path(__file__).resolve()), "status", "--job-root", str(root)],
            "verify_command": ["python3", str(Path(__file__).resolve()), "verify", "--job-root", str(root)],
            "execution_contract": {
                "actual_window_required": True,
                "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
                "cases": matrix_cases(),
                "checkpoints_per_process": len(CHECKPOINTS),
                "screenshots": len(matrix_cases()) * len(CHECKPOINTS),
                "parallel_game_processes": 1,
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Door quality run must use the planned direct kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Door subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Door source changed")
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Door harness changed",
    )
    behavior.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Door asset view changed",
    )
    candidate = behavior.candidate_identity(repo)
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": PROFILE,
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "asset_view_fingerprint": args.asset_view_fingerprint,
        "adapter": args.adapter,
        "started_at": native.utc_now(),
        "current_stage": "build",
        "current_case": None,
        "cases_completed": 0,
        "child_pid": None,
        "heartbeat_at": native.utc_now(),
        "commands": [],
    }
    job_file = root / "job.json"
    native.atomic_write_json(job_file, state)
    try:
        environment = os.environ.copy()
        native.run_command(
            "build",
            build_command(),
            repo=repo,
            env=environment,
            log_path=root / "build.log",
            job_file=job_file,
            state=state,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(binary.is_file() and not binary.is_symlink(), "Door profiling binary is absent")
        binary_hash = sha256(binary)
        observations: dict[str, dict[str, Any]] = {}
        specs = matrix_cases()
        for index, spec in enumerate(specs):
            case_root = root / "cases" / spec["id"]
            case_root.mkdir(parents=True)
            state.update({"current_case": spec["id"], "cases_completed": index})
            native.atomic_write_json(job_file, state)
            observations[spec["id"]] = run_case(
                repo=repo,
                root=case_root,
                job_file=job_file,
                state=state,
                adapter=args.adapter,
                subject_commit=args.subject_commit,
                source_fingerprint=args.source_fingerprint,
                binary_sha256=binary_hash,
                candidate=candidate,
                quality=spec["quality"],
                scale_factor=spec["scale_factor"],
            )
            state["cases_completed"] = index + 1
            native.atomic_write_json(job_file, state)
        screenshot_hashes = {
            spec["id"]: {
                item["checkpoint"]["zoom"]: item["screenshot"]["sha256"]
                for item in observations[spec["id"]]["checkpoints"]
            }
            for spec in specs
        }
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": PROFILE,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "adapter": args.adapter,
            "binary_sha256": binary_hash,
            "candidate_identity": candidate,
            "cases": specs,
            "screenshot_sha256": screenshot_hashes,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(job_file, state)
        native.print_json(verify_root(root))
        return 0
    except Exception as error:
        state.update(
            {
                "status": "invalid",
                "failure": f"{type(error).__name__}: {error}",
                "completed_at": native.utc_now(),
                "child_pid": None,
            }
        )
        native.atomic_write_json(job_file, state)
        raise


def status(args: argparse.Namespace) -> int:
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def verify(args: argparse.Namespace) -> int:
    native.print_json(verify_root(Path(args.job_root).resolve()))
    return 0


def self_test() -> int:
    cases = matrix_cases()
    native.require(
        len(cases) == 9
        and cases[0] == {"id": "quality-high-dpi-1p0", "quality": "high", "scale_factor": 1.0}
        and cases[-1] == {"id": "quality-low-dpi-2p0", "quality": "low", "scale_factor": 2.0},
        "Door quality matrix differs",
    )
    native.require(
        [item["generation"] for item in CHECKPOINTS] == [1, 2]
        and [item["zoom"] for item in CHECKPOINTS] == ["standard", "farthest"],
        "Door checkpoint contract differs",
    )
    command = gallery_command(
        Path("/repo"), Path("/artifact"), "Intel", quality="low", scale_factor=2.0
    )
    native.require(
        command[command.index("--rtt-quality") + 1] == "low"
        and command[command.index("--window-scale-factor") + 1] == "2.0"
        and command[command.index("--window-backend") + 1] == "x11",
        "Door quality command differs",
    )
    native.require(
        command[command.index("--workload") + 1] == "wall-density"
        and command[command.index("--wall-phase") + 1] == "completed"
        and command[command.index("--familiars") + 1] == "0",
        "Door quality command does not use the frozen static carrier",
    )
    nonce = "0123456789abcdef0123456789abcdef"
    targets = []
    for row, axis in enumerate(("EastWest", "NorthSouth")):
        for column, state in enumerate(("Closed", "Open", "Locked")):
            targets.append(
                {
                    "axis": axis,
                    "grid": [14 + column * 4, 18 - row * 4],
                    "state": state,
                    "world": [float(column), float(row), 0.0],
                    "viewport_center": {"x": 300.0 + column * 100, "y": 250.0 + row * 100},
                    "roi": {"x": 280 + column * 100, "y": 230 + row * 100, "width": 40, "height": 40},
                }
            )
    candidate = {
        "asset_set_generation": 6,
        "manifest_sha256": "a" * 64,
        "locator_sha256": "b" * 64,
    }
    status_value = {
        "schema_version": 2,
        "status": "ready",
        "phase": CHECKPOINTS[0]["phase"],
        "generation": 1,
        "session_nonce": nonce,
        "evidence_kind": "isolated_candidate",
        "checkpoint": {"zoom": "standard", "camera_scale": 0.5},
        "window": {"width": 1280, "height": 720, "scale_factor": 1.0},
        "render": {"backend": "vulkan", "render3d": "visible", "rtt_quality": "high"},
        "candidate_identity": {
            "asset_set_generation": 6,
            "authority": "IsolatedCandidate",
            "manifest_sha256": "a" * 64,
        },
        "gallery": {"production_count": 6, "fallback_count": 0, "targets": targets},
    }
    validated = validate_status(
        status_value,
        nonce=nonce,
        candidate=candidate,
        quality="high",
        scale_factor=1.0,
        checkpoint=CHECKPOINTS[0],
    )
    native.require(acknowledgement(validated)["generation"] == 1, "Door acknowledgement differs")
    try:
        validate_status(
            {**status_value, "generation": 2},
            nonce=nonce,
            candidate=candidate,
            quality="high",
            scale_factor=1.0,
            checkpoint=CHECKPOINTS[0],
        )
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Door status accepted the wrong checkpoint generation")
    native.print_json(
        {"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE, "cases": 9, "screenshots": 18}
    )
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    run_parser = commands.add_parser("run")
    for name in (
        "repo",
        "job_root",
        "subject_commit",
        "source_fingerprint",
        "harness_fingerprint",
        "asset_view_fingerprint",
        "adapter",
    ):
        run_parser.add_argument("--" + name.replace("_", "-"), required=True)
    for name in ("status", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        return {
            "plan": plan,
            "run": run,
            "status": status,
            "verify": verify,
            "self-test": lambda _: self_test(),
        }[args.command](args)
    except (
        native.AcceptanceError,
        OSError,
        KeyError,
        ValueError,
        json.JSONDecodeError,
    ) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
