#!/usr/bin/env python3
"""Capture and fail-closed-verify the Wall fallback or candidate gallery."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import secrets
import subprocess
import sys
import time
from dataclasses import asdict
from pathlib import Path
from typing import Any

import native_acceptance as native
import wall_density_acceptance as density

SCHEMA_VERSION = 1
PROFILE = "wall-art-current-calibration-v1"
CANDIDATE_PROFILE = "wall-art-approved-candidate-v1"
MATRIX_PROFILE = "wall-art-approved-candidate-matrix-v1"
FARTHEST_PROFILE = "wall-art-approved-candidate-farthest-zoom-v1"
ZOOM_MODES = ("standard", "farthest")
# The straight-run band is judged against the terrain it sits on instead of a
# fixed colour: a wall column counts as visible when its darkest pixel is at
# least this many standard deviations below the terrain rows of the same band.
STRAIGHT_CONTRAST_SIGMA = 3.0
PHASE = "current-wall"
SEED = 20_260_901
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WINDOW_SCALE_FACTOR = 1.0
CAPTURE_REGION = (320, 40, 516, 674)
WARMUP_SECONDS = 10.0
MEASURE_SECONDS = 10.0
RUN_TIMEOUT_SECONDS = 120.0
POLL_SECONDS = 0.10
SCREENSHOT = "current-wall.png"
MATRIX_QUALITIES = ("high", "medium", "low")
MATRIX_SCALE_FACTORS = (1.0, 1.5, 2.0)
CONTRACT_PATH = Path(__file__).resolve().parents[4] / density.CONTRACT_RELATIVE


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
            "id": f"quality-{quality}-dpi-{str(scale_factor).replace('.', 'p')}",
            "quality": quality,
            "scale_factor": scale_factor,
        }
        for quality in MATRIX_QUALITIES
        for scale_factor in MATRIX_SCALE_FACTORS
    ]


def calibration_command(
    repo: Path,
    root: Path,
    adapter: str,
    *,
    matrix_mode: bool = False,
    quality: str = "high",
    scale_factor: float = WINDOW_SCALE_FACTOR,
    zoom: str = "standard",
) -> list[str]:
    command = [
        "python3",
        "scripts/perf.py",
        "run",
        "--workload",
        "wall-density",
        "--wall-phase",
        "completed",
        "--wall-actual-window",
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
    if matrix_mode:
        command.append("--wall-art-matrix")
    if zoom != "standard":
        command.extend(["--wall-art-zoom", zoom])
    return command


def require_object(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    native.require(isinstance(value, dict), f"{label} must be an object")
    native.require(set(value) == fields, f"{label} fields differ")
    return value


def require_number(value: Any, label: str) -> float:
    native.require(type(value) in {int, float}, f"{label} must be numeric")
    number = float(value)
    native.require(math.isfinite(number), f"{label} must be finite")
    return number


def profile_name(candidate: bool, matrix: bool = False, zoom: str = "standard") -> str:
    if matrix:
        native.require(candidate, "Wall matrix requires candidate mode")
        return FARTHEST_PROFILE if zoom == "farthest" else MATRIX_PROFILE
    native.require(zoom == "standard", "Wall zoom selection requires the matrix profile")
    return CANDIDATE_PROFILE if candidate else PROFILE


def candidate_identity(repo: Path) -> dict[str, Any]:
    path = repo / "assets/manifests/wall-production-v1.wallset"
    native.require(
        path.is_file() and not path.is_symlink(),
        "Wall candidate projection is absent",
    )
    value = native.read_json(path)
    native.require(
        value.get("authority") == "isolated_candidate",
        "Wall candidate authority differs",
    )
    generation = value.get("asset_set_generation")
    native.require(
        type(generation) is int and generation > 0,
        "Wall candidate generation differs",
    )
    native.require(
        value.get("normal_decision") == "rejected"
        and value.get("review_status") == "art_approved"
        and value.get("candidate_normal") is None,
        "Wall candidate is not the approved normal-free payload",
    )
    digest = value.get("manifest_sha256")
    native.require(
        isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest),
        "Wall candidate manifest hash is invalid",
    )
    return {
        "authority": "isolated_candidate",
        "asset_set_generation": generation,
        "manifest_sha256": digest,
    }


def validate_probe_status(
    value: Any,
    *,
    nonce: str,
    candidate: bool = False,
    quality: str = "high",
    scale_factor: float = WINDOW_SCALE_FACTOR,
    zoom: str = "standard",
) -> dict[str, Any]:
    fields = {
        "schema_version",
        "status",
        "session_nonce",
        "phase",
        "generation",
        "fixture",
        "window",
        "render",
        "probe",
    }
    if candidate:
        fields.update({"gallery", "capture_view"})
    status = require_object(
        value,
        "Wall calibration status",
        fields,
    )
    native.require(
        status["schema_version"] == SCHEMA_VERSION, "Wall status schema differs"
    )
    native.require(status["status"] == "ready", "Wall calibration probe is not ready")
    native.require(status["session_nonce"] == nonce, "Wall calibration nonce differs")
    native.require(status["phase"] == PHASE, "Wall calibration phase differs")
    native.require(status["generation"] == 1, "Wall calibration generation differs")
    fixture = require_object(
        status["fixture"],
        "Wall calibration fixture",
        {
            "contract_id",
            "contract_sha256",
            "layout_checksum",
            "target_size",
            "wall_phase",
            "subject_ordinal",
            "subject_grid",
            "subject_mask",
        },
    )
    native.require(fixture["contract_id"] == "wall-density-v1", "Wall contract differs")
    native.require(
        fixture["contract_sha256"] == density.sha256(CONTRACT_PATH),
        "Wall contract hash differs",
    )
    native.require(
        isinstance(fixture["layout_checksum"], str)
        and re.fullmatch(r"[0-9a-f]{64}", fixture["layout_checksum"]),
        "Wall layout checksum is invalid",
    )
    native.require(
        fixture["target_size"] == "N"
        and fixture["wall_phase"] == "completed"
        and fixture["subject_ordinal"] == 64
        and fixture["subject_grid"] == [22, 17]
        and fixture["subject_mask"] == "0000",
        "Wall calibration subject differs",
    )
    window = require_object(
        status["window"],
        "Wall calibration window",
        {"physical_width", "physical_height", "scale_factor"},
    )
    native.require(
        window
        == {
            "physical_width": WINDOW_WIDTH,
            "physical_height": WINDOW_HEIGHT,
            "scale_factor": scale_factor,
        },
        "Wall calibration window differs",
    )
    render = require_object(
        status["render"],
        "Wall calibration render",
        {
            "backend",
            "render3d",
            "rtt_quality",
            "camera_scale",
            "fallback_mesh_resident",
        },
    )
    gallery_camera_scale = 5.0 if zoom == "farthest" else 1.0
    expected_render = {
        "backend": "vulkan",
        "render3d": "visible",
        "rtt_quality": quality,
        "camera_scale": gallery_camera_scale if candidate else 5.0,
        "fallback_mesh_resident": not candidate,
    }
    native.require(render == expected_render, "Wall calibration render contract differs")
    if candidate:
        gallery = require_object(
            status["gallery"],
            "Wall approved candidate gallery",
            {
                "lighting",
                "asset_set_generation",
                "authority",
                "manifest_sha256",
                "layout_checksum",
                "wall_phase",
                "target_wall_count",
                "connector_count",
                "production_count",
                "fallback_count",
                "distinct_meshes",
                "distinct_materials",
                "mask_counts",
            },
        )
        native.require(gallery["lighting"] == "lit", "Wall lighting mode differs")
        native.require(
            type(gallery["asset_set_generation"]) is int
            and gallery["asset_set_generation"] > 0,
            "Wall candidate generation differs",
        )
        native.require(
            gallery["authority"] == "isolated_candidate",
            "Wall candidate authority differs",
        )
        native.require(
            isinstance(gallery["manifest_sha256"], str)
            and re.fullmatch(r"[0-9a-f]{64}", gallery["manifest_sha256"]),
            "Wall candidate manifest hash is invalid",
        )
        native.require(
            gallery["layout_checksum"] == fixture["layout_checksum"]
            and gallery["wall_phase"] == "completed"
            and gallery["target_wall_count"] == 96
            and gallery["connector_count"] == 192
            and gallery["production_count"] == 96
            and gallery["fallback_count"] == 0
            and gallery["distinct_meshes"] == 6
            and gallery["distinct_materials"] == 1,
            "Wall candidate gallery residency differs",
        )
        native.require(
            gallery["mask_counts"] == {f"{mask:04b}": 6 for mask in range(16)},
            "Wall candidate mask coverage differs",
        )
        capture_view = require_object(
            status["capture_view"],
            "Wall candidate capture view",
            {
                "focus",
                "hidden_ui_roots",
                "visible_ui_roots",
                "hidden_connector_visuals",
                "visible_connector_visuals",
            },
        )
        native.require(
            capture_view["focus"] == "subject"
            and type(capture_view["hidden_ui_roots"]) is int
            and capture_view["hidden_ui_roots"] > 0
            and capture_view["visible_ui_roots"] == 0
            and capture_view["hidden_connector_visuals"] == 192
            and capture_view["visible_connector_visuals"] == 0,
            "Wall candidate capture view differs",
        )
    probe_fields = {"viewport_center", "roi", "world_position"}
    if zoom == "farthest":
        probe_fields.add("straight")
    probe = require_object(
        status["probe"],
        "Wall calibration probe",
        probe_fields,
    )
    if zoom == "farthest":
        validate_straight_probe(probe["straight"])
    center = require_object(
        probe["viewport_center"], "Wall viewport center", {"x", "y"}
    )
    require_number(center["x"], "Wall viewport center x")
    require_number(center["y"], "Wall viewport center y")
    roi = require_object(probe["roi"], "Wall ROI", {"x", "y", "width", "height"})
    native.require(
        all(type(roi[key]) is int for key in roi)
        and roi["width"] > 0
        and roi["height"] > 0
        and roi["x"] >= 0
        and roi["y"] >= 0
        and roi["x"] + roi["width"] <= WINDOW_WIDTH
        and roi["y"] + roi["height"] <= WINDOW_HEIGHT,
        "Wall ROI is invalid",
    )
    min_x, min_y, max_x, max_y = (
        (0, 0, WINDOW_WIDTH, WINDOW_HEIGHT)
        if candidate
        else CAPTURE_REGION
    )
    native.require(
        roi["x"] >= min_x
        and roi["y"] >= min_y
        and roi["x"] + roi["width"] <= max_x
        and roi["y"] + roi["height"] <= max_y,
        "Wall ROI overlaps fixed UI chrome",
    )
    world = require_object(
        probe["world_position"], "Wall world position", {"x", "y", "z"}
    )
    for axis in ("x", "y", "z"):
        require_number(world[axis], f"Wall world position {axis}")
    return status


def validate_straight_probe(value: Any) -> None:
    straight = require_object(
        value,
        "Wall straight probe",
        {"ordinal", "grid", "mask", "viewport_center", "roi", "terrain_margin"},
    )
    margin = straight["terrain_margin"]
    native.require(
        type(margin) is int and margin >= 1, "Wall straight terrain margin is invalid"
    )
    native.require(
        straight["mask"] == "0011",
        "Wall straight probe is not the east-west specimen",
    )
    native.require(
        type(straight["ordinal"]) is int
        and isinstance(straight["grid"], list)
        and len(straight["grid"]) == 2
        and all(type(value) is int for value in straight["grid"]),
        "Wall straight probe identity is invalid",
    )
    center = require_object(
        straight["viewport_center"], "Wall straight viewport center", {"x", "y"}
    )
    require_number(center["x"], "Wall straight viewport center x")
    require_number(center["y"], "Wall straight viewport center y")
    roi = require_object(
        straight["roi"], "Wall straight ROI", {"x", "y", "width", "height"}
    )
    native.require(
        all(type(roi[key]) is int for key in roi)
        and roi["width"] > 0
        and roi["height"] >= 2 * margin + 2
        and roi["x"] >= 0
        and roi["y"] >= 0
        and roi["x"] + roi["width"] <= WINDOW_WIDTH
        and roi["y"] + roi["height"] <= WINDOW_HEIGHT,
        "Wall straight ROI is invalid",
    )


def straight_run_evidence(image: Image, status: dict[str, Any]) -> dict[str, Any]:
    """Measure whether the thinnest wall run stays visible at the farthest zoom.

    The wall is under two pixels wide there, so the band is judged against the
    terrain rows of the same ROI rather than against a fixed colour.
    """
    width, _height, pixels = image
    straight = status["probe"]["straight"]
    roi = straight["roi"]
    margin = straight["terrain_margin"]

    def luminance(x: int, y: int) -> float:
        offset = (y * width + x) * 3
        red, green, blue = pixels[offset : offset + 3]
        return 0.2126 * red + 0.7152 * green + 0.0722 * blue

    columns = range(roi["x"], roi["x"] + roi["width"])
    rows = range(roi["y"], roi["y"] + roi["height"])
    terrain = [
        luminance(x, y)
        for x in columns
        for y in list(rows)[:margin] + list(rows)[-margin:]
    ]
    mean = sum(terrain) / len(terrain)
    deviation = math.sqrt(sum((value - mean) ** 2 for value in terrain) / len(terrain))
    floor = max(deviation, 1.0)
    interior = list(rows)[margin:-margin]
    scores = []
    for x in columns:
        darkest = min(luminance(x, y) for y in interior)
        scores.append((mean - darkest) / floor)
    weakest = min(scores)
    native.require(
        weakest >= STRAIGHT_CONTRAST_SIGMA,
        "Wall straight run is not continuously visible at the farthest zoom: "
        f"weakest column is {weakest:.2f} sigma below terrain "
        f"(mean {mean:.2f}, deviation {deviation:.2f})",
    )
    return {
        "columns": len(scores),
        "terrain_mean": round(mean, 6),
        "terrain_standard_deviation": round(deviation, 6),
        "weakest_column_sigma": round(weakest, 6),
        "median_column_sigma": round(sorted(scores)[len(scores) // 2], 6),
    }


def read_image(path: Path) -> Image:
    native.require(path.is_file() and not path.is_symlink(), "Wall PNG is absent")
    payload = path.read_bytes()
    native.validate_png_structure(payload)
    width, height, pixels = native.decode_png_rgb(payload)
    native.require(len(pixels) == width * height * 3, "Wall PNG decode length differs")
    return width, height, pixels


def image_evidence(image: Image, status: dict[str, Any]) -> dict[str, Any]:
    width, height, pixels = image
    native.require(
        (width, height) == (WINDOW_WIDTH, WINDOW_HEIGHT), "Wall PNG size differs"
    )
    roi = status["probe"]["roi"]
    samples: list[tuple[int, int, int]] = []
    for y in range(roi["y"], roi["y"] + roi["height"]):
        for x in range(roi["x"], roi["x"] + roi["width"]):
            offset = (y * width + x) * 3
            samples.append(tuple(pixels[offset : offset + 3]))
    flat = [channel for pixel in samples for channel in pixel]
    mean = sum(flat) / len(flat)
    variance = sum((value - mean) ** 2 for value in flat) / len(flat)
    colored = sum(max(pixel) - min(pixel) >= 8 for pixel in samples)
    non_black = sum(max(pixel) >= 18 for pixel in samples)
    native.require(non_black >= len(samples) // 20, "Wall ROI is effectively black")
    native.require(colored >= 4, "Wall ROI contains no credible colored geometry")
    native.require(math.sqrt(variance) >= 2.0, "Wall ROI contains no credible detail")
    return {
        "width": width,
        "height": height,
        "roi_mean": round(mean, 6),
        "roi_standard_deviation": round(math.sqrt(variance), 6),
        "roi_colored_pixels": colored,
        "roi_non_black_pixels": non_black,
    }


def capture_client_window(
    destination: Path, *, root_pid: int, status: dict[str, Any], zoom: str = "standard"
) -> dict[str, Any] | None:
    candidates = native.x11_client_windows_for_process_tree(root_pid)
    if not candidates:
        return None
    native.require(
        len(candidates) == 1, "Wall calibration exposed multiple X11 clients"
    )
    window_id, window_pid = candidates[0]
    import_command = native.shutil.which("import")
    native.require(
        import_command is not None, "Wall calibration requires ImageMagick import"
    )
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
        label="Wall calibration X11 client screenshot",
    )
    if completed.returncode != 0 or not destination.is_file():
        destination.unlink(missing_ok=True)
        return None
    image = read_image(destination)
    evidence = image_evidence(image, status)
    if zoom == "farthest":
        evidence["straight_run"] = straight_run_evidence(image, status)
    return {
        "file": destination.name,
        "sha256": sha256(destination),
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "window_id": window_id,
        "window_pid": window_pid,
        **evidence,
    }


def acknowledgement(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
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
    quality: str = "high",
    scale_factor: float = WINDOW_SCALE_FACTOR,
) -> dict[str, Any]:
    Case, validate_run = density.load_perf_modules(repo)
    case = Case("wall-density", "small", "gpu", SEED, 0, 0, wall_phase="completed")
    run_dir = output / "cases" / case.identifier / "run-001"
    metadata = native.read_json(run_dir / "run-metadata.json")
    native.require(
        metadata.get("case") == asdict(case), "Wall calibration case metadata differs"
    )
    native.require(metadata.get("returncode") == 0, "Wall calibration process failed")
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
        "Wall calibration raw performance validation failed: "
        + "; ".join(validation.reasons),
    )
    native.require(
        native.read_json(run_dir / "validation.json") == validation.to_json(),
        "Wall calibration stored validation differs",
    )
    manifest = native.read_json(output / "manifest.json")
    native.require(
        manifest.get("status") == "valid",
        "Wall calibration performance manifest is invalid",
    )
    native.require(
        manifest.get("git", {}).get("commit") == subject_commit,
        "Wall calibration subject differs",
    )
    native.require(
        manifest.get("git", {}).get("dirty_paths") == [],
        "Wall calibration recorded dirty paths",
    )
    native.require(
        manifest.get("source", {}).get("fingerprint_start") == source_fingerprint
        and manifest.get("source", {}).get("fingerprint_end") == source_fingerprint
        and manifest.get("source", {}).get("unchanged") is True,
        "Wall calibration source provenance differs",
    )
    native.require(
        manifest.get("binary", {}).get("sha256") == binary_sha256,
        "Wall calibration binary differs",
    )
    fixture = validation.wall_density_fixture
    native.require(
        isinstance(fixture, dict), "Wall calibration fixture sidecar is absent"
    )
    return {
        "case_id": case.identifier,
        "fixture": fixture,
        "validation": validation.to_json(),
    }


def run_calibration(
    *,
    repo: Path,
    root: Path,
    binary: Path,
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
    state: dict[str, Any],
    candidate_mode: bool,
    candidate: dict[str, Any] | None,
    matrix_mode: bool = False,
    quality: str = "high",
    scale_factor: float = WINDOW_SCALE_FACTOR,
    zoom: str = "standard",
    job_file: Path | None = None,
) -> dict[str, Any]:
    job_file = job_file or root / "job.json"
    status_path = root / "probe-status.json"
    ack_path = root / "probe-ack.json"
    screenshot = root / SCREENSHOT
    nonce = secrets.token_hex(16)
    environment = os.environ.copy()
    environment.update(
        {
            "HW_WALL_ART_ACTUAL_WINDOW": "1",
            "HW_WALL_ART_STATUS_PATH": str(status_path),
            "HW_WALL_ART_ACK_PATH": str(ack_path),
            "HW_WALL_ART_SESSION_NONCE": nonce,
        }
    )
    if candidate_mode:
        native.require(candidate is not None, "Wall candidate identity is absent")
        environment.update(
            {
                "HW_WALL_CANDIDATE": "1",
                "HW_WALL_CANDIDATE_GENERATION": str(
                    candidate["asset_set_generation"]
                ),
                "HW_WALL_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
            }
        )
    if matrix_mode:
        environment["HW_WALL_ART_MATRIX"] = "1"
    if zoom != "standard":
        environment["HW_WALL_ART_ZOOM"] = zoom
    command = calibration_command(
        repo,
        root,
        adapter,
        matrix_mode=matrix_mode,
        quality=quality,
        scale_factor=scale_factor,
        zoom=zoom,
    )
    state.update({"current_stage": "capture"})
    state.setdefault("commands", []).append({"stage": "capture", "argv": command})
    native.atomic_write_json(job_file, state)
    deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
    captured_status: dict[str, Any] | None = None
    screenshot_evidence: dict[str, Any] | None = None
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
                if (
                    status_path.is_file()
                    and not status_path.is_symlink()
                    and captured_status is None
                ):
                    value = native.read_json(status_path)
                    if isinstance(value, dict) and value.get("status") == "failed":
                        raise native.AcceptanceError(
                            f"Wall production probe failed: {value.get('reason')}"
                        )
                    status = validate_probe_status(
                        value,
                        nonce=nonce,
                        candidate=candidate_mode,
                        quality=quality,
                        scale_factor=scale_factor,
                        zoom=zoom,
                    )
                    evidence = capture_client_window(
                        screenshot,
                        root_pid=process.pid,
                        status=status,
                        zoom=zoom,
                    )
                    if evidence is not None:
                        native.atomic_write_json(ack_path, acknowledgement(status))
                        captured_status = status
                        screenshot_evidence = evidence
                if time.monotonic() >= deadline:
                    native.stop_command_process(process)
                    raise native.AcceptanceError("Wall calibration capture timed out")
                state["heartbeat_at"] = native.utc_now()
                native.atomic_write_json(job_file, state)
                time.sleep(POLL_SECONDS)
            returncode = process.wait()
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
            ack_path.unlink(missing_ok=True)
    native.require(
        returncode == 0, f"Wall calibration process exited with {returncode}"
    )
    native.require(
        captured_status is not None and screenshot_evidence is not None,
        "Wall calibration PNG was not captured",
    )
    state["child_pid"] = None
    native.atomic_write_json(job_file, state)
    status_path.unlink(missing_ok=True)
    performance = verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=adapter,
        subject_commit=subject_commit,
        source_fingerprint=source_fingerprint,
        binary_sha256=sha256(binary),
        quality=quality,
        scale_factor=scale_factor,
    )
    native.require(
        performance["fixture"]["layout_checksum"]
        == captured_status["fixture"]["layout_checksum"],
        "Wall PNG fixture checksum differs from raw performance evidence",
    )
    observation = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": profile_name(candidate_mode, matrix_mode, zoom),
        "candidate": candidate_mode,
        "candidate_identity": candidate,
        "quality": quality,
        "scale_factor": scale_factor,
        "probe_status": captured_status,
        "screenshot": screenshot_evidence,
        "performance": performance,
    }
    native.atomic_write_json(root / "observation.json", observation)
    return observation


def verify_observation(
    *,
    root: Path,
    repo: Path,
    manifest: dict[str, Any],
    candidate_mode: bool,
    candidate: dict[str, Any] | None,
    zoom: str = "standard",
    matrix_mode: bool,
    quality: str,
    scale_factor: float,
) -> dict[str, Any]:
    observation = native.read_json(root / "observation.json")
    status = validate_probe_status(
        observation.get("probe_status"),
        nonce=observation["probe_status"]["session_nonce"],
        candidate=candidate_mode,
        quality=quality,
        scale_factor=scale_factor,
        zoom=zoom,
    )
    native.require(
        observation.get("profile") == profile_name(candidate_mode, matrix_mode, zoom)
        and observation.get("candidate") == candidate_mode
        and observation.get("candidate_identity") == candidate
        and observation.get("quality", "high") == quality
        and observation.get("scale_factor", WINDOW_SCALE_FACTOR) == scale_factor,
        "Wall observation identity differs",
    )
    if candidate_mode:
        native.require(
            status["gallery"]["manifest_sha256"] == candidate["manifest_sha256"]
            and status["gallery"]["asset_set_generation"]
            == candidate["asset_set_generation"]
            and status["gallery"]["authority"] == candidate["authority"],
            "Wall runtime candidate identity differs",
        )
    screenshot = observation.get("screenshot")
    native.require(
        isinstance(screenshot, dict) and screenshot.get("file") == SCREENSHOT,
        "Wall screenshot evidence differs",
    )
    image = read_image(root / SCREENSHOT)
    native.require(
        sha256(root / SCREENSHOT) == screenshot.get("sha256"),
        "Wall screenshot hash differs",
    )
    recalculated = image_evidence(image, status)
    if zoom == "farthest":
        recalculated["straight_run"] = straight_run_evidence(image, status)
    for field, value in recalculated.items():
        native.require(
            screenshot.get(field) == value, f"Wall screenshot {field} differs"
        )
    performance = verify_performance(
        repo=repo,
        output=root / "performance",
        adapter=manifest["adapter"],
        subject_commit=manifest["subject_commit"],
        source_fingerprint=manifest["source_fingerprint"],
        binary_sha256=manifest["binary_sha256"],
        quality=quality,
        scale_factor=scale_factor,
    )
    native.require(
        performance == observation.get("performance"),
        "Wall performance evidence differs",
    )
    native.require(
        performance["fixture"]["layout_checksum"]
        == status["fixture"]["layout_checksum"],
        "Wall fixture link differs",
    )
    return observation


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    candidate_mode = manifest.get("candidate")
    native.require(type(candidate_mode) is bool, "Wall candidate mode is invalid")
    matrix_mode = manifest.get("matrix", False)
    native.require(type(matrix_mode) is bool, "Wall matrix mode is invalid")
    zoom = manifest.get("zoom", "standard")
    native.require(zoom in ZOOM_MODES, "Wall zoom mode is invalid")
    candidate = manifest.get("candidate_identity")
    native.require(
        manifest.get("profile") == profile_name(candidate_mode, matrix_mode, zoom),
        "Wall manifest profile differs",
    )
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION, "Wall manifest schema differs"
    )
    native.require(
        manifest.get("status") == "pass",
        "Wall manifest is invalid",
    )
    repo = native.validate_repo(manifest["repo"])
    density.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"],
        "Wall source changed",
    )
    native.require(
        native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"],
        "Wall harness changed",
    )
    native.require(
        density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"],
        "Wall asset view changed",
    )
    if not candidate_mode:
        native.require(candidate is None, "Fallback calibration has candidate identity")
    else:
        native.require(
            candidate == candidate_identity(repo), "Wall candidate identity changed"
        )
    binary = repo / "target/profiling/bevy_app"
    native.require(sha256(binary) == manifest["binary_sha256"], "Wall binary changed")
    specs = matrix_cases() if matrix_mode else [
        {"id": "single", "quality": "high", "scale_factor": WINDOW_SCALE_FACTOR}
    ]
    if matrix_mode:
        native.require(
            manifest.get("cases") == specs,
            "Wall matrix case contract differs",
        )
    screenshot_hashes: dict[str, str] = {}
    for spec in specs:
        case_root = root / "cases" / spec["id"] if matrix_mode else root
        observation = verify_observation(
            root=case_root,
            repo=repo,
            manifest=manifest,
            candidate_mode=candidate_mode,
            candidate=candidate,
            matrix_mode=matrix_mode,
            zoom=zoom,
            quality=spec["quality"],
            scale_factor=spec["scale_factor"],
        )
        screenshot_hashes[spec["id"]] = observation["screenshot"]["sha256"]
    if matrix_mode:
        native.require(
            manifest.get("screenshot_sha256") == screenshot_hashes,
            "Wall matrix screenshot hashes differ",
        )
    else:
        native.require(
            manifest.get("screenshot_sha256") == screenshot_hashes["single"],
            "Wall screenshot manifest hash differs",
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": profile_name(candidate_mode, matrix_mode, zoom),
        "candidate": candidate_mode,
        "zoom": zoom,
        "root": str(root),
        "screenshots": len(specs),
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(
        f"Wall calibration is missing runtime asset {path}"
        for path in density.missing_runtime_assets(repo)
    )
    candidate: dict[str, Any] | None = None
    if args.matrix and not args.candidate:
        failures.append("Wall matrix requires --candidate")
    if args.candidate:
        try:
            candidate = candidate_identity(repo)
        except native.AcceptanceError as error:
            failures.append(str(error))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    try:
        density.assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = (
        Path(args.job_root).resolve()
        if args.job_root
        else native.unique_job_root(repo, "wall-art")
    )
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
    ]
    if args.candidate and candidate is not None:
        command.extend(
            [
                "--candidate",
                "--candidate-generation",
                str(candidate["asset_set_generation"]),
                "--candidate-manifest-sha256",
                candidate["manifest_sha256"],
            ]
        )
    if args.matrix:
        command.append("--matrix")
    if args.zoom != "standard":
        command.extend(["--zoom", args.zoom])
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": profile_name(args.candidate, args.matrix, args.zoom),
            "candidate": args.candidate,
            "matrix": args.matrix,
            "zoom": args.zoom,
            "candidate_identity": candidate,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "status_command": [
                "python3",
                str(Path(__file__).resolve()),
                "status",
                "--job-root",
                str(root),
            ],
            "verify_command": [
                "python3",
                str(Path(__file__).resolve()),
                "verify",
                "--job-root",
                str(root),
            ],
            "execution_contract": {
                "actual_window_required": True,
                "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
                "screenshots": len(matrix_cases()) if args.matrix else 1,
                "cases": matrix_cases() if args.matrix else None,
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Wall calibration must use planned kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(
        native.git_subject(repo) == args.subject_commit, "Wall subject changed"
    )
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "Wall source changed",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Wall harness changed",
    )
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        not args.matrix or args.candidate,
        "Wall matrix requires candidate mode",
    )
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed",
    )
    candidate: dict[str, Any] | None = None
    if not args.candidate:
        native.require(
            args.candidate_generation is None
            and args.candidate_manifest_sha256 is None,
            "Fallback calibration received candidate identity",
        )
    else:
        candidate = candidate_identity(repo)
        native.require(
            args.candidate_generation is not None
            and int(args.candidate_generation) == candidate["asset_set_generation"]
            and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
            "Planned Wall candidate identity changed",
        )
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": profile_name(args.candidate, args.matrix, args.zoom),
        "candidate": args.candidate,
        "matrix": args.matrix,
        "zoom": args.zoom,
        "candidate_identity": candidate,
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
        native.require(
            binary.is_file() and not binary.is_symlink(),
            "Wall profiling binary is missing",
        )
        specs = matrix_cases() if args.matrix else [
            {"id": "single", "quality": "high", "scale_factor": WINDOW_SCALE_FACTOR}
        ]
        observations: dict[str, dict[str, Any]] = {}
        for index, spec in enumerate(specs):
            case_root = root / "cases" / spec["id"] if args.matrix else root
            case_root.mkdir(parents=True, exist_ok=not args.matrix)
            state.update(
                {
                    "current_case": spec["id"],
                    "cases_completed": index,
                }
            )
            native.atomic_write_json(root / "job.json", state)
            observations[spec["id"]] = run_calibration(
                repo=repo,
                root=case_root,
                binary=binary,
                adapter=args.adapter,
                subject_commit=args.subject_commit,
                source_fingerprint=args.source_fingerprint,
                state=state,
                candidate_mode=args.candidate,
                candidate=candidate,
                matrix_mode=args.matrix,
                quality=spec["quality"],
                scale_factor=spec["scale_factor"],
                zoom=args.zoom,
                job_file=root / "job.json",
            )
            state["cases_completed"] = index + 1
            native.atomic_write_json(root / "job.json", state)
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": profile_name(args.candidate, args.matrix, args.zoom),
            "candidate": args.candidate,
            "matrix": args.matrix,
            "zoom": args.zoom,
            "candidate_identity": candidate,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "adapter": args.adapter,
            "binary_sha256": sha256(binary),
            "screenshot_sha256": (
                {
                    spec["id"]: observations[spec["id"]]["screenshot"]["sha256"]
                    for spec in specs
                }
                if args.matrix
                else observations["single"]["screenshot"]["sha256"]
            ),
            "cases": specs if args.matrix else None,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update(
            {"status": "valid", "completed_at": native.utc_now(), "child_pid": None}
        )
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
    return (
        2
        if job.get("status") == "running"
        else 0
        if job.get("status") == "valid"
        else 1
    )


def verify(args: argparse.Namespace) -> int:
    native.print_json(verify_root(Path(args.job_root).resolve()))
    return 0


def self_test() -> int:
    command = calibration_command(Path("/repo"), Path("/artifact"), "Intel")
    native.require(
        "--wall-actual-window" in command,
        "Wall calibration command lacks its single-case contract",
    )
    cases = matrix_cases()
    native.require(
        len(cases) == 9
        and cases[0]
        == {"id": "quality-high-dpi-1p0", "quality": "high", "scale_factor": 1.0}
        and cases[-1]
        == {"id": "quality-low-dpi-2p0", "quality": "low", "scale_factor": 2.0},
        "Wall matrix case contract differs",
    )
    matrix_command = calibration_command(
        Path("/repo"),
        Path("/artifact"),
        "Intel",
        matrix_mode=True,
        quality="low",
        scale_factor=2.0,
    )
    native.require(
        matrix_command[matrix_command.index("--rtt-quality") + 1] == "low"
        and matrix_command[matrix_command.index("--window-scale-factor") + 1] == "2.0",
        "Wall matrix command differs",
    )
    native.require(
        "--wall-art-matrix" in matrix_command
        and "--wall-art-matrix" not in command,
        "Wall matrix authorization flag differs",
    )
    farthest_command = calibration_command(
        Path("/repo"),
        Path("/artifact"),
        "Intel",
        matrix_mode=True,
        quality="high",
        scale_factor=1.0,
        zoom="farthest",
    )
    native.require(
        farthest_command[farthest_command.index("--wall-art-zoom") + 1] == "farthest"
        and "--wall-art-zoom" not in matrix_command,
        "Wall farthest zoom authorization flag differs",
    )
    native.require(
        profile_name(True, True, "farthest") == FARTHEST_PROFILE
        and profile_name(True, True) == MATRIX_PROFILE,
        "Wall farthest zoom profile differs",
    )
    straight = {
        "ordinal": 3,
        "grid": [7, 17],
        "mask": "0011",
        "viewport_center": {"x": 640.0, "y": 360.0},
        "roi": {"x": 637, "y": 353, "width": 6, "height": 14},
        "terrain_margin": 4,
    }
    validate_straight_probe(straight)
    try:
        validate_straight_probe({**straight, "mask": "1100"})
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Wall straight probe accepted a north-south mask")
    # A dark two-pixel run over noisy terrain must pass, and the same terrain
    # without the run must fail.
    def synthetic(with_wall: bool) -> Image:
        width, height = WINDOW_WIDTH, WINDOW_HEIGHT
        pixels = bytearray(width * height * 3)
        for y in range(height):
            for x in range(width):
                offset = (y * width + x) * 3
                base = 90 + ((x * 7 + y * 13) % 9)
                wall_row = with_wall and 359 <= y <= 360
                value = 12 if wall_row else base
                pixels[offset : offset + 3] = bytes((value, value, value))
        return width, height, bytes(pixels)

    probe_status = {"probe": {"straight": straight}}
    evidence = straight_run_evidence(synthetic(True), probe_status)
    native.require(
        evidence["columns"] == 6 and evidence["weakest_column_sigma"] >= 3.0,
        "Wall straight run evidence differs",
    )
    try:
        straight_run_evidence(synthetic(False), probe_status)
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Wall straight run accepted terrain without a wall")
    nonce = "0123456789abcdef0123456789abcdef"
    fixture_hash = density.sha256(CONTRACT_PATH)
    status = {
        "schema_version": 1,
        "status": "ready",
        "session_nonce": nonce,
        "phase": PHASE,
        "generation": 1,
        "fixture": {
            "contract_id": "wall-density-v1",
            "contract_sha256": fixture_hash,
            "layout_checksum": "a" * 64,
            "target_size": "N",
            "wall_phase": "completed",
            "subject_ordinal": 64,
            "subject_grid": [22, 17],
            "subject_mask": "0000",
        },
        "window": {"physical_width": 1280, "physical_height": 720, "scale_factor": 1.0},
        "render": {
            "backend": "vulkan",
            "render3d": "visible",
            "rtt_quality": "high",
            "camera_scale": 5.0,
            "fallback_mesh_resident": True,
        },
        "probe": {
            "viewport_center": {"x": 464.0, "y": 580.0},
            "roi": {"x": 416, "y": 532, "width": 96, "height": 96},
            "world_position": {"x": 80.0, "y": 16.0, "z": -80.0},
        },
    }
    validate_probe_status(status, nonce=nonce)
    candidate_status = json.loads(json.dumps(status))
    candidate_status["render"]["camera_scale"] = 1.0
    candidate_status["render"]["fallback_mesh_resident"] = False
    candidate_status["probe"]["viewport_center"] = {"x": 640.0, "y": 350.0}
    candidate_status["probe"]["roi"] = {
        "x": 592,
        "y": 302,
        "width": 96,
        "height": 96,
    }
    candidate_status["gallery"] = {
        "lighting": "lit",
        "asset_set_generation": 2,
        "authority": "isolated_candidate",
        "manifest_sha256": "b" * 64,
        "layout_checksum": "a" * 64,
        "wall_phase": "completed",
        "target_wall_count": 96,
        "connector_count": 192,
        "production_count": 96,
        "fallback_count": 0,
        "distinct_meshes": 6,
        "distinct_materials": 1,
        "mask_counts": {f"{mask:04b}": 6 for mask in range(16)},
    }
    candidate_status["capture_view"] = {
        "focus": "subject",
        "hidden_ui_roots": 7,
        "visible_ui_roots": 0,
        "hidden_connector_visuals": 192,
        "visible_connector_visuals": 0,
    }
    validate_probe_status(candidate_status, nonce=nonce, candidate=True)
    for case in cases:
        matrix_status = json.loads(json.dumps(candidate_status))
        matrix_status["window"]["scale_factor"] = case["scale_factor"]
        matrix_status["render"]["rtt_quality"] = case["quality"]
        validate_probe_status(
            matrix_status,
            nonce=nonce,
            candidate=True,
            quality=case["quality"],
            scale_factor=case["scale_factor"],
        )
    candidate_status["gallery"]["lighting"] = "unlit"
    try:
        validate_probe_status(candidate_status, nonce=nonce, candidate=True)
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Wall gallery accepted the removed unlit mode")
    native.require(
        acknowledgement(status)["generation"] == 1, "Wall acknowledgement differs"
    )
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profiles": [PROFILE, CANDIDATE_PROFILE, MATRIX_PROFILE, FARTHEST_PROFILE],
        }
    )
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    plan_parser.add_argument("--candidate", action="store_true")
    plan_parser.add_argument("--matrix", action="store_true")
    plan_parser.add_argument("--zoom", choices=ZOOM_MODES, default="standard")
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
    run_parser.add_argument("--candidate", action="store_true")
    run_parser.add_argument("--matrix", action="store_true")
    run_parser.add_argument("--zoom", choices=ZOOM_MODES, default="standard")
    run_parser.add_argument("--candidate-generation")
    run_parser.add_argument("--candidate-manifest-sha256")
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
