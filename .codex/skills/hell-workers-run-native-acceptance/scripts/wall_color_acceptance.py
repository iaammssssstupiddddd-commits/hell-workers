#!/usr/bin/env python3
"""Capture and verify the sealed Blender/Bevy Wall color-calibration pair."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import secrets
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import native_acceptance as native
import wall_art_acceptance as wall
import wall_density_acceptance as density

SCHEMA_VERSION = 1
PROFILE = "wall-color-calibration-v1"
PHASE = "wall-color-board"
SEED = 20_260_901
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WINDOW_SCALE_FACTOR = 1.0
BOARD_WIDTH = 320
BOARD_HEIGHT = 96
BOARD_ROI = {"x": 480, "y": 312, "width": BOARD_WIDTH, "height": BOARD_HEIGHT}
WARMUP_SECONDS = 10.0
MEASURE_SECONDS = 10.0
RUN_TIMEOUT_SECONDS = 120.0
POLL_SECONDS = 0.10
REFERENCE = "blender-reference.png"
REFERENCE_METADATA = "blender-reference.json"
CANDIDATE = "bevy-candidate.png"
CANDIDATE_METADATA = "bevy-candidate.json"
VERIFICATION = "verification.json"
CONTRACT_PATH = Path(__file__).resolve().parents[4] / "tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json"
OCIO_PATH = Path(__file__).resolve().parents[4] / "tools/blender_ai_workflow/fixtures/wall-calibration-v2.ocio"

Image = tuple[int, int, bytes]


def load_contract() -> dict[str, Any]:
    payload = native.read_json(CONTRACT_PATH)
    native.require(
        payload.get("schema_version") == 1
        and payload.get("contract_id") == "wall-color-calibration-v1",
        "Wall color contract identity differs",
    )
    return payload


def load_color_verifier(repo: Path):
    path = repo / "tools/blender_ai_workflow/scripts/verify_color_calibration.py"
    spec = importlib.util.spec_from_file_location("verify_color_calibration", path)
    native.require(spec is not None and spec.loader is not None, "cannot load Wall color verifier")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def color_command(repo: Path, root: Path, adapter: str) -> list[str]:
    command = wall.calibration_command(repo, root, adapter)
    index = command.index("--wall-actual-window")
    command[index] = "--wall-color-actual-window"
    return command


def require_object(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    native.require(isinstance(value, dict), f"{label} must be an object")
    native.require(set(value) == fields, f"{label} fields differ")
    return value


def expected_patch_status(contract: dict[str, Any]) -> list[dict[str, Any]]:
    patches = [
        {
            "id": patch["id"],
            "center_px": patch["center_px"],
            "input_hex_srgb": patch["input_hex_srgb"],
            "emissive": False,
        }
        for patch in contract["base_patches"]
    ]
    emissive = contract["emissive_sanity"]
    patches.append(
        {
            "id": emissive["id"],
            "center_px": emissive["center_px"],
            "input_hex_srgb": emissive["input_hex_srgb"],
            "emissive": True,
        }
    )
    return patches


def validate_probe_status(value: Any, *, nonce: str) -> dict[str, Any]:
    contract = load_contract()
    status = require_object(
        value,
        "Wall color status",
        {"schema_version", "status", "session_nonce", "phase", "generation", "fixture", "window", "render"},
    )
    native.require(status["schema_version"] == 1, "Wall color status schema differs")
    native.require(status["status"] == "ready", "Wall color probe is not ready")
    native.require(status["session_nonce"] == nonce, "Wall color nonce differs")
    native.require(status["phase"] == PHASE and status["generation"] == 1, "Wall color phase differs")
    fixture = require_object(
        status["fixture"],
        "Wall color fixture",
        {"contract_id", "contract_sha256", "board_roi", "patch_size_px", "patches"},
    )
    native.require(fixture["contract_id"] == contract["contract_id"], "Wall color contract differs")
    native.require(fixture["contract_sha256"] == wall.sha256(CONTRACT_PATH), "Wall color contract hash differs")
    native.require(fixture["board_roi"] == BOARD_ROI, "Wall color board ROI differs")
    native.require(fixture["patch_size_px"] == contract["image"]["patch_size_px"], "Wall color patch size differs")
    native.require(fixture["patches"] == expected_patch_status(contract), "Wall color patch inventory differs")
    window = require_object(
        status["window"],
        "Wall color window",
        {"physical_width", "physical_height", "scale_factor"},
    )
    native.require(
        window
        == {
            "physical_width": WINDOW_WIDTH,
            "physical_height": WINDOW_HEIGHT,
            "scale_factor": WINDOW_SCALE_FACTOR,
        },
        "Wall color window differs",
    )
    render = require_object(
        status["render"],
        "Wall color render",
        {
            "backend",
            "camera",
            "camera_order",
            "render_layer",
            "base_path",
            "emissive_path",
            "emissive_strength",
        },
    )
    native.require(
        render
        == {
            "backend": "vulkan",
            "camera": "dedicated-final-camera2d",
            "camera_order": 100,
            "render_layer": 31,
            "base_path": "sprite-unlit-srgb",
            "emissive_path": "sprite-linear-multiplier",
            "emissive_strength": 2.0,
        },
        "Wall color render contract differs",
    )
    return status


def read_image(path: Path, expected_size: tuple[int, int]) -> Image:
    native.require(path.is_file() and not path.is_symlink(), f"PNG is absent: {path}")
    payload = path.read_bytes()
    native.validate_png_structure(payload)
    width, height, pixels = native.decode_png_rgb(payload)
    native.require((width, height) == expected_size, f"PNG size differs: {path}")
    native.require(len(pixels) == width * height * 3, f"PNG decode length differs: {path}")
    return width, height, pixels


def median_rgb(image: Image, center: list[int], roi_size: int) -> list[float]:
    width, _, pixels = image
    half = roi_size // 2
    samples: list[tuple[int, int, int]] = []
    for y in range(center[1] - half, center[1] + half):
        for x in range(center[0] - half, center[0] + half):
            offset = (y * width + x) * 3
            samples.append(tuple(pixels[offset : offset + 3]))
    return [float(statistics.median(pixel[channel] for pixel in samples)) for channel in range(3)]


def relative_luminance(rgb: list[float]) -> float:
    def linear(channel: float) -> float:
        value = channel / 255.0
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    return linear(rgb[0]) * 0.2126 + linear(rgb[1]) * 0.7152 + linear(rgb[2]) * 0.0722


def candidate_image_evidence(image: Image) -> dict[str, Any]:
    contract = load_contract()
    roi_size = contract["image"]["roi_size_px"]
    medians = {
        patch["id"]: median_rgb(image, patch["center_px"], roi_size)
        for patch in [*contract["base_patches"], contract["emissive_sanity"]]
    }
    native.require(
        all(max(medians[patch["id"]]) >= 8 for patch in contract["base_patches"]),
        "Wall color base patch is effectively black",
    )
    base = medians[contract["emissive_sanity"]["base_patch_id"]]
    emissive = medians[contract["emissive_sanity"]["id"]]
    lift = relative_luminance(emissive) - relative_luminance(base)
    native.require(
        lift >= contract["emissive_sanity"]["minimum_relative_luminance_lift"],
        "Wall color emissive patch lacks the required luminance lift",
    )
    return {"patch_medians_srgb8": medians, "emissive_luminance_lift": round(lift, 6)}


def capture_color_board(
    destination: Path, *, root_pid: int, status: dict[str, Any]
) -> dict[str, Any] | None:
    candidates = native.x11_client_windows_for_process_tree(root_pid)
    if not candidates:
        return None
    native.require(len(candidates) == 1, "Wall color profile exposed multiple X11 clients")
    window_id, window_pid = candidates[0]
    import_command = shutil.which("import")
    convert_command = shutil.which("magick") or shutil.which("convert")
    native.require(import_command is not None, "Wall color profile requires ImageMagick import")
    native.require(convert_command is not None, "Wall color profile requires ImageMagick crop")
    full = destination.with_name("client-window.png")
    completed = native.run_bounded_capture_tool(
        [import_command, "-window", window_id, "-depth", "8", "-type", "TrueColor", str(full)],
        label="Wall color X11 client screenshot",
    )
    if completed.returncode != 0 or not full.is_file():
        full.unlink(missing_ok=True)
        return None
    read_image(full, (WINDOW_WIDTH, WINDOW_HEIGHT))
    roi = status["fixture"]["board_roi"]
    completed = native.run_bounded_capture_tool(
        [
            convert_command,
            str(full),
            "-crop",
            f"{roi['width']}x{roi['height']}+{roi['x']}+{roi['y']}",
            "+repage",
            "-depth",
            "8",
            "-type",
            "TrueColor",
            str(destination),
        ],
        label="Wall color exact board crop",
    )
    full.unlink(missing_ok=True)
    if completed.returncode != 0 or not destination.is_file():
        destination.unlink(missing_ok=True)
        return None
    image = read_image(destination, (BOARD_WIDTH, BOARD_HEIGHT))
    return {
        "file": destination.name,
        "sha256": wall.sha256(destination),
        "capture_scope": "x11-client-window-exact-crop",
        "rescaled": False,
        "window_id": window_id,
        "window_pid": window_pid,
        **candidate_image_evidence(image),
    }


def acknowledgement(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "status": "captured",
        "session_nonce": status["session_nonce"],
        "phase": status["phase"],
        "generation": status["generation"],
    }


def candidate_metadata(path: Path, source_fingerprint: str) -> dict[str, Any]:
    contract = load_contract()
    pipeline = contract["color_pipeline"]
    patch_inputs = {patch["id"]: patch["input_hex_srgb"] for patch in contract["base_patches"]}
    emissive = contract["emissive_sanity"]
    patch_inputs[emissive["id"]] = emissive["input_hex_srgb"]
    return {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "renderer": "bevy-client",
        "source_fingerprint": source_fingerprint,
        "capture": {
            "encoded_format": "png",
            "height": BOARD_HEIGHT,
            "image_sha256": wall.sha256(path),
            "rescaled": False,
            "srgb": True,
            "width": BOARD_WIDTH,
        },
        "color": {
            "automatic_exposure": pipeline["automatic_exposure"],
            "display_device": pipeline["display_device"],
            "exposure": pipeline["exposure"],
            "gamma": pipeline["gamma"],
            "look": pipeline["look"],
            "tonemapping": pipeline["tonemapping"],
            "view_transform": pipeline["reference_view_transform"],
        },
        "patch_inputs": patch_inputs,
    }


def run_capture(
    *, repo: Path, root: Path, binary: Path, adapter: str, subject_commit: str, source_fingerprint: str, state: dict[str, Any]
) -> dict[str, Any]:
    status_path = root / "probe-status.json"
    ack_path = root / "probe-ack.json"
    candidate = root / CANDIDATE
    nonce = secrets.token_hex(16)
    environment = os.environ.copy()
    environment.update(
        {
            "HW_WALL_COLOR_ACTUAL_WINDOW": "1",
            "HW_WALL_COLOR_STATUS_PATH": str(status_path),
            "HW_WALL_COLOR_ACK_PATH": str(ack_path),
            "HW_WALL_COLOR_SESSION_NONCE": nonce,
        }
    )
    command = color_command(repo, root, adapter)
    state.update({"current_stage": "capture"})
    state.setdefault("commands", []).append({"stage": "capture", "argv": command})
    native.atomic_write_json(root / "job.json", state)
    deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
    captured_status: dict[str, Any] | None = None
    screenshot_evidence: dict[str, Any] | None = None
    with (root / "capture.log").open("w", encoding="utf-8") as log:
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
        native.atomic_write_json(root / "job.json", state)
        try:
            while process.poll() is None:
                if status_path.is_file() and not status_path.is_symlink() and captured_status is None:
                    value = native.read_json(status_path)
                    if isinstance(value, dict) and value.get("status") == "failed":
                        raise native.AcceptanceError(f"Wall color probe failed: {value.get('reason')}")
                    status = validate_probe_status(value, nonce=nonce)
                    evidence = capture_color_board(candidate, root_pid=process.pid, status=status)
                    if evidence is not None:
                        native.atomic_write_json(ack_path, acknowledgement(status))
                        captured_status = status
                        screenshot_evidence = evidence
                if time.monotonic() >= deadline:
                    native.stop_command_process(process)
                    raise native.AcceptanceError("Wall color capture timed out")
                state["heartbeat_at"] = native.utc_now()
                native.atomic_write_json(root / "job.json", state)
                time.sleep(POLL_SECONDS)
            returncode = process.wait()
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
            ack_path.unlink(missing_ok=True)
    native.require(returncode == 0, f"Wall color process exited with {returncode}")
    native.require(captured_status is not None and screenshot_evidence is not None, "Wall color PNG was not captured")
    status_path.unlink(missing_ok=True)
    metadata = candidate_metadata(candidate, source_fingerprint)
    native.atomic_write_json(root / CANDIDATE_METADATA, metadata)
    performance = wall.verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=adapter,
        subject_commit=subject_commit,
        source_fingerprint=source_fingerprint,
        binary_sha256=wall.sha256(binary),
    )
    observation = {
        "schema_version": 1,
        "status": "pass",
        "profile": PROFILE,
        "probe_status": captured_status,
        "candidate": screenshot_evidence,
        "candidate_metadata_sha256": wall.sha256(root / CANDIDATE_METADATA),
        "performance": performance,
    }
    native.atomic_write_json(root / "observation.json", observation)
    return observation


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(manifest.get("schema_version") == 1 and manifest.get("status") == "pass", "Wall color manifest is invalid")
    native.require(manifest.get("profile") == PROFILE, "Wall color profile differs")
    repo = native.validate_repo(manifest["repo"])
    density.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(native.source_fingerprint(repo) == manifest["source_fingerprint"], "Wall color source changed")
    native.require(native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"], "Wall color harness changed")
    native.require(density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"], "Wall color asset view changed")
    binary = repo / "target/profiling/bevy_app"
    native.require(wall.sha256(binary) == manifest["binary_sha256"], "Wall color binary changed")
    for name, field in (
        (REFERENCE, "reference_sha256"),
        (REFERENCE_METADATA, "reference_metadata_sha256"),
        (CANDIDATE, "candidate_sha256"),
        (CANDIDATE_METADATA, "candidate_metadata_sha256"),
        (VERIFICATION, "verification_sha256"),
    ):
        native.require(wall.sha256(root / name) == manifest[field], f"Wall color {name} hash differs")
    observation = native.read_json(root / "observation.json")
    status = validate_probe_status(observation["probe_status"], nonce=observation["probe_status"]["session_nonce"])
    read_image(root / CANDIDATE, (BOARD_WIDTH, BOARD_HEIGHT))
    native.require(candidate_image_evidence(read_image(root / CANDIDATE, (BOARD_WIDTH, BOARD_HEIGHT))) == {key: observation["candidate"][key] for key in ("patch_medians_srgb8", "emissive_luminance_lift")}, "Wall color candidate pixel evidence differs")
    metadata = native.read_json(root / CANDIDATE_METADATA)
    native.require(metadata == candidate_metadata(root / CANDIDATE, manifest["source_fingerprint"]), "Wall color candidate metadata differs")
    performance = wall.verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=manifest["adapter"],
        subject_commit=manifest["subject_commit"],
        source_fingerprint=manifest["source_fingerprint"],
        binary_sha256=manifest["binary_sha256"],
    )
    native.require(performance == observation["performance"], "Wall color performance evidence differs")
    verifier = load_color_verifier(repo)
    report = verifier.verify_calibration(
        contract_path=CONTRACT_PATH,
        reference_path=root / REFERENCE,
        reference_metadata_path=root / REFERENCE_METADATA,
        candidate_path=root / CANDIDATE,
        candidate_metadata_path=root / CANDIDATE_METADATA,
    )
    native.require(report == native.read_json(root / VERIFICATION), "Wall color verification report differs")
    native.require(report["status"] == "pass", "Wall color Delta E gate failed")
    return {
        "schema_version": 1,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "mean_delta_e_2000": report["base_color_gate"]["mean_delta_e_2000"],
        "emissive_sanity_passed": report["emissive_sanity_gate"]["passed"],
        "board_roi": status["fixture"]["board_roi"],
    }


def validate_reference(path: Path, metadata_path: Path, source_fingerprint: str) -> list[str]:
    failures: list[str] = []
    try:
        native.require(path.is_file() and not path.is_symlink(), f"reference is absent: {path}")
        native.require(metadata_path.is_file() and not metadata_path.is_symlink(), f"reference metadata is absent: {metadata_path}")
        metadata = native.read_json(metadata_path)
        native.require(metadata.get("renderer") == "blender", "reference renderer differs")
        native.require(metadata.get("source_fingerprint") == source_fingerprint, "reference source fingerprint differs")
        native.require(metadata.get("contract", {}).get("sha256") == wall.sha256(CONTRACT_PATH), "reference contract hash differs")
        native.require(metadata.get("capture", {}).get("image_sha256") == wall.sha256(path), "reference PNG hash differs")
        ocio = metadata.get("ocio", {})
        native.require(ocio.get("config_sha256") == wall.sha256(OCIO_PATH), "reference OCIO hash differs")
        native.require(ocio.get("validation_status") == "pass" and ocio.get("active_config_matches") is True and ocio.get("fallback") is False, "reference OCIO proof is not positive")
    except (native.AcceptanceError, OSError, KeyError, TypeError, ValueError) as error:
        failures.append(str(error))
    return failures


def regular_file_hash(path: Path) -> str | None:
    return wall.sha256(path) if path.is_file() and not path.is_symlink() else None


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(f"Wall color calibration is missing runtime asset {path}" for path in density.missing_runtime_assets(repo))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    try:
        density.assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    reference = Path(args.reference).expanduser().resolve()
    reference_metadata = Path(args.reference_metadata).expanduser().resolve()
    failures.extend(validate_reference(reference, reference_metadata, source))
    try:
        load_color_verifier(repo)
    except (ImportError, OSError, RuntimeError) as error:
        failures.append(f"Wall color verifier is unavailable: {error}")
    if shutil.which("import") is None or (shutil.which("magick") is None and shutil.which("convert") is None):
        failures.append("Wall color calibration requires ImageMagick import and crop tools")
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "wall-color")
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
        assets,
        "--adapter",
        args.adapter,
        "--reference",
        str(reference),
        "--reference-sha256",
        regular_file_hash(reference) or "absent",
        "--reference-metadata",
        str(reference_metadata),
        "--reference-metadata-sha256",
        regular_file_hash(reference_metadata) or "absent",
    ]
    native.print_json(
        {
            "schema_version": 1,
            "status": "ready" if not failures else "blocked",
            "profile": PROFILE,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "reference_sha256": regular_file_hash(reference),
            "reference_metadata_sha256": regular_file_hash(reference_metadata),
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "execution_contract": {"actual_window_required": True, "capture_scope": "x11-client-window-exact-crop", "screenshots": 1},
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "Wall color calibration must use planned kitty command")
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall color subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "Wall color source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "Wall color harness changed")
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(density.asset_view_fingerprint(repo) == args.asset_view_fingerprint, "Wall color asset view changed")
    reference = Path(args.reference).resolve()
    reference_metadata = Path(args.reference_metadata).resolve()
    native.require(wall.sha256(reference) == args.reference_sha256, "Wall color reference changed")
    native.require(wall.sha256(reference_metadata) == args.reference_metadata_sha256, "Wall color reference metadata changed")
    native.require(not validate_reference(reference, reference_metadata, args.source_fingerprint), "Wall color reference is invalid")
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": 1,
        "status": "running",
        "profile": PROFILE,
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "asset_view_fingerprint": args.asset_view_fingerprint,
        "adapter": args.adapter,
        "started_at": native.utc_now(),
        "current_stage": "build",
        "child_pid": None,
        "heartbeat_at": native.utc_now(),
        "commands": [],
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        shutil.copyfile(reference, root / REFERENCE)
        shutil.copyfile(reference_metadata, root / REFERENCE_METADATA)
        native.run_command(
            "build",
            density.build_command(),
            repo=repo,
            env=os.environ.copy(),
            log_path=root / "build.log",
            job_file=root / "job.json",
            state=state,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(binary.is_file() and not binary.is_symlink(), "Wall color profiling binary is missing")
        observation = run_capture(
            repo=repo,
            root=root,
            binary=binary,
            adapter=args.adapter,
            subject_commit=args.subject_commit,
            source_fingerprint=args.source_fingerprint,
            state=state,
        )
        verifier = load_color_verifier(repo)
        report = verifier.verify_calibration(
            contract_path=CONTRACT_PATH,
            reference_path=root / REFERENCE,
            reference_metadata_path=root / REFERENCE_METADATA,
            candidate_path=root / CANDIDATE,
            candidate_metadata_path=root / CANDIDATE_METADATA,
        )
        native.atomic_write_json(root / VERIFICATION, report)
        native.require(report["status"] == "pass", "Wall color Delta E gate failed")
        manifest = {
            "schema_version": 1,
            "status": "pass",
            "profile": PROFILE,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "adapter": args.adapter,
            "binary_sha256": wall.sha256(binary),
            "reference_sha256": wall.sha256(root / REFERENCE),
            "reference_metadata_sha256": wall.sha256(root / REFERENCE_METADATA),
            "candidate_sha256": observation["candidate"]["sha256"],
            "candidate_metadata_sha256": observation["candidate_metadata_sha256"],
            "verification_sha256": wall.sha256(root / VERIFICATION),
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
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
        native.atomic_write_json(root / "job.json", state)
        raise


def status(args: argparse.Namespace) -> int:
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def verify(args: argparse.Namespace) -> int:
    native.print_json(verify_root(Path(args.job_root).resolve()))
    return 0


def self_test() -> int:
    command = color_command(Path("/repo"), Path("/artifact"), "Intel")
    native.require("--wall-color-actual-window" in command and "--wall-actual-window" not in command, "Wall color command differs")
    contract = load_contract()
    nonce = "0123456789abcdef0123456789abcdef"
    status = {
        "schema_version": 1,
        "status": "ready",
        "session_nonce": nonce,
        "phase": PHASE,
        "generation": 1,
        "fixture": {
            "contract_id": contract["contract_id"],
            "contract_sha256": wall.sha256(CONTRACT_PATH),
            "board_roi": BOARD_ROI,
            "patch_size_px": 32,
            "patches": expected_patch_status(contract),
        },
        "window": {"physical_width": 1280, "physical_height": 720, "scale_factor": 1.0},
        "render": {
            "backend": "vulkan",
            "camera": "dedicated-final-camera2d",
            "camera_order": 100,
            "render_layer": 31,
            "base_path": "sprite-unlit-srgb",
            "emissive_path": "sprite-linear-multiplier",
            "emissive_strength": 2.0,
        },
    }
    validate_probe_status(status, nonce=nonce)
    native.require(acknowledgement(status)["phase"] == PHASE, "Wall color acknowledgement differs")
    native.print_json({"schema_version": 1, "status": "pass", "profile": PROFILE})
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    plan_parser.add_argument("--reference", required=True)
    plan_parser.add_argument("--reference-metadata", required=True)
    run_parser = commands.add_parser("run")
    for name in (
        "repo",
        "job_root",
        "subject_commit",
        "source_fingerprint",
        "harness_fingerprint",
        "asset_view_fingerprint",
        "adapter",
        "reference",
        "reference_sha256",
        "reference_metadata",
        "reference_metadata_sha256",
    ):
        run_parser.add_argument("--" + name.replace("_", "-"), required=True)
    for name in ("status", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    return {
        "plan": plan,
        "run": run,
        "status": status,
        "verify": verify,
        "self-test": lambda _args: self_test(),
    }[args.command](args)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (native.AcceptanceError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"wall color acceptance failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
