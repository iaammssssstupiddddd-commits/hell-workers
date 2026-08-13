#!/usr/bin/env python3
"""Run and fail-closed-verify the P02 production actual-window matrix.

The matrix deliberately drives the normal indoor-light fixture.  It never
uses ``visual_test`` or a synthetic building/Soul route: Rust publishes a
phase-and-ROI sidecar only after the production state is ready, then this
launcher captures one X11 client frame for each phase.  Verification decodes
the saved PNGs again, recomputes their visual predicates, and revalidates the
raw performance run rather than trusting an observation summary.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import tempfile
import time
import zlib
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402


SCHEMA_VERSION = 2
PROFILE = "p02-presentation-actual-window-v2"
QUALITIES = ("high", "medium", "low")
SCALE_FACTORS = (1.0, 1.5, 2.0)
VISIBILITY = (("visible", "gpu"), ("hidden", "cpu"))
EXPECTED_CASES = tuple(
    (quality, scale, visibility, render)
    for quality in QUALITIES
    for scale in SCALE_FACTORS
    for visibility, render in VISIBILITY
)
PHASES = (
    "door-open",
    "door-closed",
    "door-locked",
    "soul-front",
    "soul-behind",
    "bridge",
    "foreground-a",
    "foreground-b",
)
SCREENSHOT_NAMES = {phase: f"{phase}.png" for phase in PHASES}
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WARMUP_SECONDS = 10.0
MEASURE_SECONDS = 10.0
RUN_TIMEOUT_SECONDS = 150.0
POLL_INTERVAL_SECONDS = 0.10


Image = tuple[int, int, bytes]


def case_id(quality: str, scale: float, visibility: str) -> str:
    return f"quality-{quality}-dpi-{str(scale).replace('.', 'p')}-render3d-{visibility}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_object(value: Any, label: str, fields: set[str]) -> dict[str, Any]:
    native.require(isinstance(value, dict), f"{label} must be an object")
    actual = set(value)
    native.require(
        actual == fields,
        f"{label} fields differ: expected {sorted(fields)}, got {sorted(actual)}",
    )
    return value


def require_int(value: Any, label: str, *, minimum: int | None = None) -> int:
    native.require(type(value) is int, f"{label} must be an integer")
    if minimum is not None:
        native.require(value >= minimum, f"{label} must be at least {minimum}")
    return value


def require_number(value: Any, label: str) -> float:
    native.require(type(value) in {int, float}, f"{label} must be a number")
    number = float(value)
    native.require(math.isfinite(number), f"{label} must be finite")
    return number


def read_single_csv(path: Path) -> dict[str, str]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            rows = list(csv.DictReader(handle))
    except OSError as error:
        raise native.AcceptanceError(f"cannot read CSV artifact {path}: {error}") from error
    native.require(len(rows) == 1, f"{path.name} must contain exactly one row")
    return rows[0]


def read_image(path: Path) -> Image:
    native.require(path.is_file() and not path.is_symlink(), f"missing non-symlink PNG {path}")
    payload = path.read_bytes()
    native.validate_png_structure(payload)
    width, height, pixels = native.decode_png_rgb(payload)
    native.require(len(pixels) == width * height * 3, f"{path.name} RGB decode length differs")
    return width, height, pixels


def image_statistics(image: Image) -> dict[str, float | int]:
    width, height, pixels = image
    count = width * height
    native.require(count > 0, "P02 screenshot has no pixels")
    total = sum(pixels)
    mean = total / len(pixels)
    variance = sum((value - mean) ** 2 for value in pixels) / len(pixels)
    non_black = 0
    detail = 0
    for red, green, blue in zip(pixels[::3], pixels[1::3], pixels[2::3]):
        if max(red, green, blue) >= 18:
            non_black += 1
        if max(red, green, blue) - min(red, green, blue) >= 18:
            detail += 1
    native.require(width >= 640 and height >= 360, "P02 client screenshot is unexpectedly small")
    native.require(non_black >= count // 100, "P02 client screenshot is effectively black")
    native.require(detail >= count // 1000, "P02 client screenshot has no credible scene detail")
    return {
        "width": width,
        "height": height,
        "mean": round(mean, 6),
        "standard_deviation": round(math.sqrt(variance), 6),
        "non_black_pixels": non_black,
        "detail_pixels": detail,
    }


def require_roi(value: Any, *, width: int, height: int, label: str) -> dict[str, int]:
    roi = require_object(value, label, {"x", "y", "width", "height"})
    result = {
        key: require_int(roi[key], f"{label}.{key}", minimum=0)
        for key in ("x", "y", "width", "height")
    }
    native.require(result["width"] > 0 and result["height"] > 0, f"{label} must have area")
    native.require(
        result["x"] + result["width"] <= width
        and result["y"] + result["height"] <= height,
        f"{label} lies outside the physical client window",
    )
    return result


def expected_phase_probe_fields(phase: str, visibility: str) -> set[str]:
    if phase.startswith("door-"):
        return {"kind", "expected_state", "presentation_state", "roi"}
    if phase.startswith("soul-"):
        return (
            {"kind", "render3d", "roi", "relation", "wall_depth", "soul_depth"}
            if visibility == "visible"
            else {"kind", "render3d", "roi"}
        )
    if phase == "bridge":
        return {"kind", "owner_3d_visual", "render3d", "roi"}
    if phase.startswith("foreground-"):
        return {"kind", "phase_scale", "roi"}
    raise native.AcceptanceError(f"unknown P02 phase {phase}")


def validate_probe_status(value: Any, *, phase: str, visibility: str) -> dict[str, Any]:
    status = require_object(
        value,
        f"P02 {phase} probe status",
        {
            "schema_version",
            "status",
            "phase",
            "generation",
            "fixture_layout_checksum",
            "render3d",
            "camera_target",
            "window",
            "probe",
        },
    )
    native.require(status["schema_version"] == SCHEMA_VERSION, "P02 probe schema differs")
    native.require(status["status"] == "ready", f"P02 {phase} probe was not ready")
    native.require(status["phase"] == phase, f"P02 probe phase is not {phase}")
    require_int(status["generation"], "P02 probe generation", minimum=1)
    native.require(
        isinstance(status["fixture_layout_checksum"], str)
        and re.fullmatch(r"[0-9a-f]{64}", status["fixture_layout_checksum"]) is not None,
        "P02 probe fixture checksum is invalid",
    )
    native.require(status["render3d"] == visibility, "P02 probe Render3d visibility differs")
    target = require_object(status["camera_target"], "P02 probe camera target", {"x", "y"})
    require_number(target["x"], "P02 probe camera_target.x")
    require_number(target["y"], "P02 probe camera_target.y")
    window = require_object(
        status["window"], "P02 probe window", {"physical_width", "physical_height"}
    )
    window_width = require_int(window["physical_width"], "P02 probe physical width", minimum=1)
    window_height = require_int(window["physical_height"], "P02 probe physical height", minimum=1)
    probe = require_object(status["probe"], f"P02 {phase} probe", expected_phase_probe_fields(phase, visibility))
    native.require(isinstance(probe["kind"], str), f"P02 {phase} probe kind is invalid")
    roi = require_roi(probe["roi"], width=window_width, height=window_height, label=f"P02 {phase} ROI")
    if phase.startswith("door-"):
        expected_state = phase.removeprefix("door-")
        native.require(probe["kind"] == "door", "P02 Door probe kind differs")
        native.require(probe["expected_state"] == expected_state, "P02 Door semantic state differs")
        native.require(
            probe["presentation_state"] == expected_state,
            "P02 Door active presentation state differs",
        )
    elif phase.startswith("soul-"):
        native.require(probe["kind"] == "soul-depth", "P02 Soul probe kind differs")
        native.require(probe["render3d"] == visibility, "P02 Soul probe visibility differs")
        if visibility == "visible":
            relation = phase.removeprefix("soul-")
            native.require(probe["relation"] == relation, "P02 Soul depth relation differs")
            wall_depth = require_number(probe["wall_depth"], "P02 wall depth")
            soul_depth = require_number(probe["soul_depth"], "P02 Soul depth")
            native.require(
                soul_depth < wall_depth if relation == "front" else soul_depth > wall_depth,
                "P02 Soul depth values do not prove the requested ordering",
            )
    elif phase == "bridge":
        native.require(probe["kind"] == "bridge", "P02 Bridge probe kind differs")
        native.require(probe["owner_3d_visual"] is True, "P02 Bridge lacks its production 3D owner")
        native.require(probe["render3d"] == visibility, "P02 Bridge visibility differs")
    else:
        native.require(probe["kind"] == "foreground", "P02 Foreground probe kind differs")
        expected_scale = 1.0 if phase == "foreground-a" else 1.18
        native.require(
            abs(require_number(probe["phase_scale"], "P02 foreground scale") - expected_scale) < 1e-6,
            "P02 Foreground phase scale differs",
        )
    status["probe"]["roi"] = roi
    return status


def capture_client_window(
    destination: Path,
    *,
    root_pid: int,
    status: dict[str, Any],
) -> tuple[dict[str, Any], Image] | None:
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
    image = read_image(destination)
    expected_window = status["window"]
    native.require(
        image[0] == expected_window["physical_width"]
        and image[1] == expected_window["physical_height"],
        "P02 screenshot dimensions differ from the Rust probe sidecar",
    )
    return (
        {
            "file": destination.name,
            **image_statistics(image),
            "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
            "window_id": window_id,
            "window_pid": window_pid,
            "sha256": sha256(destination),
        },
        image,
    )


def roi_colour_metrics(image: Image, roi: dict[str, int]) -> dict[str, float | int]:
    width, _, pixels = image
    total = roi["width"] * roi["height"]
    native.require(total > 0, "P02 ROI has no pixels")
    channel_total = 0
    detail = 0
    green = 0
    red = 0
    warm = 0
    brown = 0
    for y in range(roi["y"], roi["y"] + roi["height"]):
        start = (y * width + roi["x"]) * 3
        end = start + roi["width"] * 3
        row = pixels[start:end]
        for red_value, green_value, blue_value in zip(row[::3], row[1::3], row[2::3]):
            channel_total += red_value + green_value + blue_value
            if max(red_value, green_value, blue_value) - min(red_value, green_value, blue_value) >= 18:
                detail += 1
            if green_value >= max(red_value, blue_value) + 12 and green_value >= 48:
                green += 1
            if red_value >= max(green_value, blue_value) + 15 and red_value >= 48:
                red += 1
            if red_value >= 56 and green_value >= 28 and red_value >= blue_value + 12:
                warm += 1
            if (
                red_value >= 56
                and green_value >= 28
                and red_value >= green_value + 8
                and green_value >= blue_value + 8
            ):
                brown += 1
    return {
        "pixels": total,
        "mean_channel": round(channel_total / (total * 3), 6),
        "detail_pixels": detail,
        "green_pixels": green,
        "red_pixels": red,
        "warm_pixels": warm,
        "brown_pixels": brown,
    }


def roi_difference(first: Image, second: Image, roi: dict[str, int]) -> dict[str, float | int]:
    native.require(first[:2] == second[:2], "P02 screenshot dimensions differ within a case")
    width, _, first_pixels = first
    _, _, second_pixels = second
    changed = 0
    min_x = roi["width"]
    min_y = roi["height"]
    max_x = -1
    max_y = -1
    for local_y, y in enumerate(range(roi["y"], roi["y"] + roi["height"])):
        first_row = (y * width + roi["x"]) * 3
        second_row = first_row
        for local_x in range(roi["width"]):
            start = first_row + local_x * 3
            if max(
                abs(first_pixels[start] - second_pixels[second_row + local_x * 3]),
                abs(first_pixels[start + 1] - second_pixels[second_row + local_x * 3 + 1]),
                abs(first_pixels[start + 2] - second_pixels[second_row + local_x * 3 + 2]),
            ) >= 20:
                changed += 1
                min_x = min(min_x, local_x)
                min_y = min(min_y, local_y)
                max_x = max(max_x, local_x)
                max_y = max(max_y, local_y)
    bbox_width = 0 if max_x < min_x else max_x - min_x + 1
    bbox_height = 0 if max_y < min_y else max_y - min_y + 1
    bbox_area = bbox_width * bbox_height
    area = roi["width"] * roi["height"]
    return {
        "changed_pixels": changed,
        "changed_ratio": round(changed / area, 6),
        "changed_bbox_width": bbox_width,
        "changed_bbox_height": bbox_height,
        "changed_bbox_area": bbox_area,
        "changed_bbox_fill_ratio": round(changed / bbox_area, 6) if bbox_area else 0.0,
    }


def phase_roi(phases: dict[str, dict[str, Any]], phase: str) -> dict[str, int]:
    return phases[phase]["probe_status"]["probe"]["roi"]


def require_changed(metrics: dict[str, float | int], label: str, *, minimum: int, maximum_ratio: float) -> None:
    native.require(metrics["changed_pixels"] >= minimum, f"{label} has insufficient changed pixels")
    native.require(metrics["changed_ratio"] <= maximum_ratio, f"{label} changed an implausibly broad ROI")


def evaluate_case_images(
    images: dict[str, Image], phases: dict[str, dict[str, Any]], *, visibility: str
) -> dict[str, Any]:
    native.require(set(images) == set(PHASES), "P02 image phase set differs")
    native.require(set(phases) == set(PHASES), "P02 sidecar phase set differs")
    checksums = {phases[phase]["probe_status"]["fixture_layout_checksum"] for phase in PHASES}
    native.require(len(checksums) == 1, "P02 phases used different production fixture layouts")

    door = {
        phase: roi_colour_metrics(images[phase], phase_roi(phases, phase))
        for phase in ("door-open", "door-closed", "door-locked")
    }
    for phase, metrics in door.items():
        native.require(metrics["detail_pixels"] >= 64, f"P02 {phase} Door ROI lacks scene detail")
    native.require(door["door-open"]["green_pixels"] >= 16, "P02 Open Door has no green material evidence")
    native.require(door["door-closed"]["brown_pixels"] >= 16, "P02 Closed Door has no brown material evidence")
    native.require(door["door-locked"]["red_pixels"] >= 16, "P02 Locked Door has no red material evidence")
    door_differences = {
        "open_closed": roi_difference(
            images["door-open"], images["door-closed"], phase_roi(phases, "door-open")
        ),
        "open_locked": roi_difference(
            images["door-open"], images["door-locked"], phase_roi(phases, "door-open")
        ),
        "closed_locked": roi_difference(
            images["door-closed"], images["door-locked"], phase_roi(phases, "door-closed")
        ),
    }
    for label, metrics in door_differences.items():
        require_changed(metrics, f"P02 Door {label}", minimum=32, maximum_ratio=0.95)

    front_roi = phase_roi(phases, "soul-front")
    behind_roi = phase_roi(phases, "soul-behind")
    native.require(front_roi == behind_roi, "P02 Soul depth phases do not share one Wall ROI")
    soul_difference = roi_difference(images["soul-front"], images["soul-behind"], front_roi)
    if visibility == "visible":
        require_changed(soul_difference, "P02 alpha/depth probe", minimum=64, maximum_ratio=0.55)
        native.require(
            soul_difference["changed_bbox_area"] > 0
            and soul_difference["changed_bbox_fill_ratio"] < 0.90,
            "P02 Soul depth difference is an opaque rectangle, not an alpha-masked billboard",
        )
    else:
        native.require(
            soul_difference["changed_ratio"] <= 0.005,
            "P02 hidden Render3d Soul depth frames changed despite having no billboard",
        )

    bridge = roi_colour_metrics(images["bridge"], phase_roi(phases, "bridge"))
    if visibility == "visible":
        native.require(bridge["detail_pixels"] >= 64, "P02 visible Bridge ROI lacks structure")
        native.require(bridge["warm_pixels"] >= 12, "P02 visible Bridge ROI lacks its material")

    foreground_roi = phase_roi(phases, "foreground-a")
    native.require(
        foreground_roi == phase_roi(phases, "foreground-b"),
        "P02 Foreground phases do not share one ROI",
    )
    foreground_difference = roi_difference(
        images["foreground-a"], images["foreground-b"], foreground_roi
    )
    require_changed(foreground_difference, "P02 Foreground animation", minimum=32, maximum_ratio=0.70)

    return {
        "fixture_layout_checksum": checksums.pop(),
        "doors": door,
        "door_differences": door_differences,
        "soul_depth_difference": soul_difference,
        "bridge": bridge,
        "foreground_difference": foreground_difference,
    }


def load_perf_modules(repo: Path) -> tuple[Any, Any, Any, Any]:
    root = str(repo)
    if root not in sys.path:
        sys.path.insert(0, root)
    from scripts.perf_tool.artifacts import validate_run
    from scripts.perf_tool.model import Case
    from scripts.perf_tool.policy import validate_session_artifact_set
    from scripts.perf_tool.rtt_light_contract import load_rtt_light_contract

    return validate_run, Case, validate_session_artifact_set, load_rtt_light_contract


def locate_run(output: Path) -> Path:
    runs = list(output.glob("cases/*/run-001"))
    native.require(len(runs) == 1, f"P02 case expected one measured run, found {len(runs)}")
    return runs[0]


def revalidate_performance(
    *,
    repo: Path,
    output: Path,
    render: str,
    quality: str,
    scale: float,
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    validate_run, Case, validate_session_artifact_set, load_rtt_light_contract = load_perf_modules(repo)
    session = native.read_json(output / "manifest.json")
    native.require(session.get("status") == "valid", "P02 performance session is not valid")
    native.require(not validate_session_artifact_set(output, session), "P02 performance session file set is invalid")
    binary = require_object(session.get("binary"), "P02 performance binary", {"path", "sha256", "instrumentation"})
    native.require(binary["sha256"] == binary_sha256, "P02 performance binary hash differs from matrix binary")
    native.require(binary["instrumentation"] == "capture", "P02 performance instrumentation differs")
    git = require_object(session.get("git"), "P02 performance git provenance", {"commit", "short_commit", "dirty_paths"})
    native.require(git["commit"] == subject_commit and git["dirty_paths"] == [], "P02 performance subject differs")
    source = require_object(
        session.get("source"),
        "P02 performance source provenance",
        {"algorithm", "fingerprint_start", "fingerprint_end", "started_at", "finished_at", "unchanged"},
    )
    native.require(
        source["fingerprint_start"] == source_fingerprint
        and source["fingerprint_end"] == source_fingerprint
        and source["unchanged"] is True,
        "P02 performance source fingerprint differs",
    )
    matrix = session.get("matrix")
    native.require(isinstance(matrix, dict), "P02 performance matrix is invalid")
    expected_matrix = {
        "workload": "indoor-light",
        "sizes": ["medium"],
        "renders": [render],
        "seed": 20260803,
        "repeat": 1,
        "preflight_runs": 0,
        "capture_kind": "frame-time",
        "clock_mode": "realtime",
        "warmup_secs": WARMUP_SECONDS,
        "measure_secs": MEASURE_SECONDS,
        "window_width": WINDOW_WIDTH,
        "window_height": WINDOW_HEIGHT,
        "window_scale_factor": scale,
        "rtt_quality": quality,
    }
    for field, expected in expected_matrix.items():
        native.require(matrix.get(field) == expected, f"P02 performance matrix {field} differs")
    selection = matrix.get("rtt_light_contract")
    native.require(
        isinstance(selection, dict)
        and (selection.get("contract_id"), selection.get("stage_id"), selection.get("lane"))
        == ("rtt-light-v1", "p02", "static"),
        "P02 performance selected the wrong production contract",
    )
    run_dir = locate_run(output)
    metadata = native.read_json(run_dir / "run-metadata.json")
    expected_case = Case("indoor-light", "medium", render, 20260803, None, None)
    native.require(metadata.get("case") == expected_case.__dict__, "P02 run metadata case differs")
    native.require(metadata.get("returncode") == 0, "P02 run metadata return code differs")
    contract = load_rtt_light_contract("rtt-light-v1")
    validation = validate_run(
        run_dir,
        returncode=0,
        expected_case=expected_case,
        expected_adapter=adapter,
        expected_backend="vulkan",
        allow_log_patterns=contract["allow_log_patterns"]["windowed"],
        capture_kind="frame-time",
        expected_warmup_secs=WARMUP_SECONDS,
        expected_measure_secs=MEASURE_SECONDS,
        expected_window_backend="x11",
        expected_present_mode="novsync",
        expected_window_width=WINDOW_WIDTH,
        expected_window_height=WINDOW_HEIGHT,
        expected_window_scale_factor=scale,
        expected_rtt_quality=quality,
        expected_contract="rtt-light-v1",
        expected_stage="p02",
        expected_lane="static",
    )
    calculated = validation.to_json()
    stored = native.read_json(run_dir / "validation.json")
    native.require(stored == calculated, "P02 stored validation differs from raw performance artifacts")
    native.require(validation.valid, "P02 raw performance validation failed: " + "; ".join(validation.reasons))
    presentation = validation.p02_presentation
    window = validation.window
    fixture = validation.indoor_light_fixture
    native.require(
        presentation is not None and window is not None and fixture is not None,
        "P02 raw performance evidence lacks presentation/window/fixture sidecars",
    )
    semantic = {
        "presentation": presentation,
        "window": window,
        "fixture": fixture,
        "run_metadata": metadata,
    }
    provenance = {
        "performance_binary_sha256": binary["sha256"],
        "performance_subject_commit": git["commit"],
        "performance_source_fingerprint": source["fingerprint_end"],
        "performance_source_unchanged": source["unchanged"],
    }
    return semantic, calculated, provenance


def case_provenance_is_valid(observation: dict[str, Any]) -> bool:
    return (
        observation.get("production_subject") == "bevy_app:indoor-light/rtt-light-v1/p02/static"
        and observation.get("capture_scope") == native.SAVE_CATALOG_CAPTURE_SCOPE
    )


def run_case(
    *,
    repo: Path,
    binary: Path,
    binary_sha256: str,
    artifact: Path,
    quality: str,
    scale: float,
    visibility: str,
    render: str,
    adapter: str,
    subject_commit: str,
    source_fingerprint: str,
) -> dict[str, Any]:
    output = artifact / "performance"
    status_path = artifact / "probe-status.json"
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
        "--binary", str(binary), "--skip-build", "--warmup-secs", str(WARMUP_SECONDS),
        "--measure-secs", str(MEASURE_SECONDS), "--timeout-secs", str(int(RUN_TIMEOUT_SECONDS)),
    ]
    artifact.mkdir(parents=True, exist_ok=False)
    log_path = artifact / "run.log"
    environment = os.environ.copy()
    environment["HW_P02_PRESENTATION_ACTUAL_WINDOW"] = "1"
    environment["HW_P02_PRESENTATION_STATUS_PATH"] = str(status_path)
    captures: dict[str, dict[str, Any]] = {}
    images: dict[str, Image] = {}
    deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
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
        try:
            while process.poll() is None:
                if status_path.is_file() and not status_path.is_symlink():
                    try:
                        status_value = native.read_json(status_path)
                    except Exception:
                        status_value = None
                    if status_value is not None:
                        phase = status_value.get("phase") if isinstance(status_value, dict) else None
                        if isinstance(phase, str) and phase in PHASES and phase not in captures:
                            status = validate_probe_status(status_value, phase=phase, visibility=visibility)
                            captured = capture_client_window(
                                artifact / SCREENSHOT_NAMES[phase],
                                root_pid=process.pid,
                                status=status,
                            )
                            if captured is not None:
                                evidence, image = captured
                                captures[phase] = {"probe_status": status, "screenshot": evidence}
                                images[phase] = image
                if time.monotonic() >= deadline:
                    native.stop_command_process(process)
                    raise native.AcceptanceError(f"P02 case {artifact.name} timed out")
                time.sleep(POLL_INTERVAL_SECONDS)
            returncode = process.wait()
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
            status_path.unlink(missing_ok=True)
    native.require(returncode == 0, f"P02 case {artifact.name} exited with {returncode}")
    native.require(tuple(captures) == PHASES, f"P02 case {artifact.name} did not capture every storyboard phase")
    semantic, performance_validation, performance_provenance = revalidate_performance(
        repo=repo,
        output=output,
        render=render,
        quality=quality,
        scale=scale,
        adapter=adapter,
        subject_commit=subject_commit,
        source_fingerprint=source_fingerprint,
        binary_sha256=binary_sha256,
    )
    image_checks = evaluate_case_images(images, captures, visibility=visibility)
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
        "phase_order": list(PHASES),
        "phases": captures,
        "image_checks": image_checks,
        "semantic": semantic,
        "performance_validation": performance_validation,
        "performance_provenance": performance_provenance,
    }
    native.atomic_write_json(artifact / "observation.json", result)
    return result


def verify_screenshot_evidence(
    *,
    path: Path,
    evidence: Any,
    status: dict[str, Any],
    phase: str,
) -> Image:
    fields = {
        "file",
        "width",
        "height",
        "mean",
        "standard_deviation",
        "non_black_pixels",
        "detail_pixels",
        "capture_scope",
        "window_id",
        "window_pid",
        "sha256",
    }
    evidence = require_object(evidence, f"P02 {phase} screenshot evidence", fields)
    native.require(evidence["file"] == SCREENSHOT_NAMES[phase], f"P02 {phase} screenshot filename differs")
    native.require(evidence["capture_scope"] == native.SAVE_CATALOG_CAPTURE_SCOPE, f"P02 {phase} capture scope differs")
    native.require(isinstance(evidence["window_id"], str) and evidence["window_id"], f"P02 {phase} window id is invalid")
    require_int(evidence["window_pid"], f"P02 {phase} window pid", minimum=1)
    native.require(isinstance(evidence["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", evidence["sha256"]) is not None, f"P02 {phase} screenshot hash is invalid")
    image = read_image(path)
    native.require(sha256(path) == evidence["sha256"], f"P02 {phase} screenshot hash differs")
    native.require(
        image[:2] == (status["window"]["physical_width"], status["window"]["physical_height"]),
        f"P02 {phase} screenshot dimensions differ from its sidecar",
    )
    statistics = image_statistics(image)
    for field, value in statistics.items():
        native.require(evidence.get(field) == value, f"P02 {phase} screenshot {field} differs")
    return image


def expected_case_tuple(identifier: str) -> tuple[str, float, str, str]:
    matches = [item for item in EXPECTED_CASES if case_id(item[0], item[1], item[2]) == identifier]
    native.require(len(matches) == 1, f"P02 case identifier is not in the matrix: {identifier}")
    return matches[0]


def verify_case(
    *,
    root: Path,
    repo: Path,
    observation: Any,
    subject_commit: str,
    source_fingerprint: str,
    binary_sha256: str,
    adapter: str,
) -> tuple[dict[str, Any], dict[str, Image]]:
    required = {
        "schema_version",
        "status",
        "case_id",
        "quality",
        "scale_factor",
        "render3d",
        "render_mode",
        "production_subject",
        "capture_scope",
        "phase_order",
        "phases",
        "image_checks",
        "semantic",
        "performance_validation",
        "performance_provenance",
    }
    observation = require_object(observation, "P02 observation", required)
    native.require(observation["schema_version"] == SCHEMA_VERSION, "P02 observation schema differs")
    native.require(observation["status"] == "pass", "P02 observation is not pass")
    native.require(case_provenance_is_valid(observation), "P02 observation is not production client-window evidence")
    identifier = observation["case_id"]
    native.require(isinstance(identifier, str), "P02 observation case id is invalid")
    quality, scale, visibility, render = expected_case_tuple(identifier)
    native.require(
        observation["quality"] == quality
        and observation["scale_factor"] == scale
        and observation["render3d"] == visibility
        and observation["render_mode"] == render,
        f"P02 {identifier} metadata differs from its matrix case",
    )
    native.require(observation["phase_order"] == list(PHASES), f"P02 {identifier} phase order differs")
    phases = observation["phases"]
    native.require(isinstance(phases, dict) and set(phases) == set(PHASES), f"P02 {identifier} phase set differs")
    case_root = root / "cases" / identifier
    expected_files = {"performance", "run.log", "observation.json", *SCREENSHOT_NAMES.values()}
    actual_files = {path.name for path in case_root.iterdir()}
    native.require(actual_files == expected_files, f"P02 {identifier} artifact file set differs")
    native.require(not any(path.is_symlink() for path in case_root.rglob("*")), f"P02 {identifier} contains a symlink")
    images: dict[str, Image] = {}
    normalized_phases: dict[str, dict[str, Any]] = {}
    for phase in PHASES:
        entry = require_object(phases[phase], f"P02 {identifier} {phase}", {"probe_status", "screenshot"})
        status = validate_probe_status(entry["probe_status"], phase=phase, visibility=visibility)
        image = verify_screenshot_evidence(
            path=case_root / SCREENSHOT_NAMES[phase],
            evidence=entry["screenshot"],
            status=status,
            phase=phase,
        )
        images[phase] = image
        normalized_phases[phase] = {"probe_status": status, "screenshot": entry["screenshot"]}
    calculated_checks = evaluate_case_images(images, normalized_phases, visibility=visibility)
    native.require(calculated_checks == observation["image_checks"], f"P02 {identifier} image predicates differ")
    semantic, validation, provenance = revalidate_performance(
        repo=repo,
        output=case_root / "performance",
        render=render,
        quality=quality,
        scale=scale,
        adapter=adapter,
        subject_commit=subject_commit,
        source_fingerprint=source_fingerprint,
        binary_sha256=binary_sha256,
    )
    native.require(semantic == observation["semantic"], f"P02 {identifier} semantic evidence differs from raw run")
    native.require(validation == observation["performance_validation"], f"P02 {identifier} raw validation differs")
    native.require(provenance == observation["performance_provenance"], f"P02 {identifier} performance provenance differs")
    return observation, images


def evaluate_cross_case_bridge(
    *,
    observations: dict[str, dict[str, Any]],
    images: dict[str, dict[str, Image]],
) -> dict[str, Any]:
    checks: dict[str, Any] = {}
    for quality in QUALITIES:
        for scale in SCALE_FACTORS:
            visible = case_id(quality, scale, "visible")
            hidden = case_id(quality, scale, "hidden")
            visible_roi = phase_roi(observations[visible]["phases"], "bridge")
            hidden_roi = phase_roi(observations[hidden]["phases"], "bridge")
            native.require(visible_roi == hidden_roi, f"P02 Bridge ROI differs across {quality}/{scale}")
            difference = roi_difference(images[visible]["bridge"], images[hidden]["bridge"], visible_roi)
            require_changed(difference, f"P02 Bridge Render3d {quality}/{scale}", minimum=48, maximum_ratio=0.80)
            visible_detail = observations[visible]["image_checks"]["bridge"]["detail_pixels"]
            hidden_detail = observations[hidden]["image_checks"]["bridge"]["detail_pixels"]
            native.require(
                visible_detail > hidden_detail,
                f"P02 visible Bridge has no extra local structure at {quality}/{scale}",
            )
            checks[f"quality-{quality}-dpi-{str(scale).replace('.', 'p')}"] = difference
    return checks


def verify_root(
    root: Path,
    *,
    subject_commit: str | None = None,
    source_fingerprint: str | None = None,
    harness_fingerprint: str | None = None,
) -> dict[str, Any]:
    native.require(root.is_dir() and not root.is_symlink(), "P02 artifact root is invalid")
    expected_root_files = {"job.json", "build.log", "manifest.json", "cases"}
    native.require({path.name for path in root.iterdir()} == expected_root_files, "P02 root artifact file set differs")
    native.require(not any(path.is_symlink() for path in root.rglob("*")), "P02 root contains a symlink")
    manifest = require_object(
        native.read_json(root / "manifest.json"),
        "P02 manifest",
        {
            "schema_version",
            "status",
            "profile",
            "repo",
            "subject_commit",
            "source_fingerprint",
            "harness_fingerprint",
            "profiling_binary_sha256",
            "capture_scope",
            "cases",
            "cross_case_checks",
            "completed_at",
        },
    )
    job = require_object(
        native.read_json(root / "job.json"),
        "P02 job",
        {
            "schema_version",
            "status",
            "profile",
            "repo",
            "subject_commit",
            "source_fingerprint",
            "harness_fingerprint",
            "profiling_binary_sha256",
            "started_at",
            "heartbeat_at",
            "cases_completed",
            "current_case",
            "completed_at",
        },
    )
    native.require(manifest["schema_version"] == SCHEMA_VERSION, "P02 manifest schema differs")
    native.require(manifest["status"] == "pass" and manifest["profile"] == PROFILE, "P02 manifest is not the v2 pass profile")
    native.require(job["schema_version"] == SCHEMA_VERSION and job["status"] == "valid" and job["profile"] == PROFILE, "P02 job is not valid v2 evidence")
    for field in ("repo", "subject_commit", "source_fingerprint", "harness_fingerprint", "profiling_binary_sha256"):
        native.require(job[field] == manifest[field], f"P02 job/manifest {field} differs")
    native.require(job["cases_completed"] == len(EXPECTED_CASES) and job["current_case"] is None, "P02 job case completion differs")
    if subject_commit is not None:
        native.require(manifest["subject_commit"] == subject_commit, "P02 matrix subject differs")
    if source_fingerprint is not None:
        native.require(manifest["source_fingerprint"] == source_fingerprint, "P02 matrix source differs")
    if harness_fingerprint is not None:
        native.require(manifest["harness_fingerprint"] == harness_fingerprint, "P02 matrix harness differs")
    repo = native.validate_repo(manifest["repo"])
    native.require(native.git_subject(repo) == manifest["subject_commit"], "P02 current subject differs from artifact")
    native.require(native.source_fingerprint(repo) == manifest["source_fingerprint"], "P02 current source differs from artifact")
    native.require(native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"], "P02 current harness differs from artifact")
    binary = repo / "target/profiling/bevy_app"
    native.require(binary.is_file() and not binary.is_symlink(), "P02 profiling binary is missing for revalidation")
    native.require(sha256(binary) == manifest["profiling_binary_sha256"], "P02 profiling binary hash differs")
    native.require(manifest["capture_scope"] == native.SAVE_CATALOG_CAPTURE_SCOPE, "P02 manifest capture scope differs")
    cases = manifest["cases"]
    native.require(isinstance(cases, list) and len(cases) == len(EXPECTED_CASES), "P02 matrix is incomplete")
    expected_ids = {case_id(quality, scale, visibility) for quality, scale, visibility, _ in EXPECTED_CASES}
    indexed = {item.get("case_id"): item for item in cases if isinstance(item, dict)}
    native.require(set(indexed) == expected_ids and len(indexed) == len(cases), "P02 manifest case set differs")
    observations: dict[str, dict[str, Any]] = {}
    images: dict[str, dict[str, Image]] = {}
    for identifier in sorted(expected_ids):
        observation = native.read_json(root / "cases" / identifier / "observation.json")
        native.require(indexed[identifier] == observation, f"P02 manifest case {identifier} differs from observation")
        checked, checked_images = verify_case(
            root=root,
            repo=repo,
            observation=observation,
            subject_commit=manifest["subject_commit"],
            source_fingerprint=manifest["source_fingerprint"],
            binary_sha256=manifest["profiling_binary_sha256"],
            adapter="Intel",
        )
        observations[identifier] = checked
        images[identifier] = checked_images
    cross_case_checks = evaluate_cross_case_bridge(observations=observations, images=images)
    native.require(cross_case_checks == manifest["cross_case_checks"], "P02 cross-case Bridge predicates differ")
    return {
        "status": "pass",
        "schema_version": SCHEMA_VERSION,
        "profile": PROFILE,
        "cases": len(expected_ids),
        "cross_case_checks": cross_case_checks,
        "root": str(root),
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    for command in ("xprop", "import"):
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
    native.print_json(
        {
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
                "phases": list(PHASES),
                "actual_window_required": True,
                "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
                "parallel_game_processes": 1,
                "actual_feature_builds": 1,
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "P02 run must be launched by the planned direct kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "P02 subject changed before launch")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "P02 source changed before launch")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "P02 harness changed before launch")
    native.assert_clean_subject(repo, args.subject_commit)
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": PROFILE,
        "repo": str(repo),
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "profiling_binary_sha256": None,
        "started_at": native.utc_now(),
        "heartbeat_at": None,
        "cases_completed": 0,
        "current_case": None,
        "completed_at": None,
    }
    native.atomic_write_json(root / "job.json", state)
    build = [
        "python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
        "--no-default-features", "--features", "profiling",
    ]
    native.run_command(
        "build", build, repo=repo, env=os.environ.copy(), log_path=root / "build.log", job_file=root / "job.json", state=state
    )
    binary = repo / "target/profiling/bevy_app"
    native.require(binary.is_file() and not binary.is_symlink(), "P02 profiling binary is missing")
    binary_hash = sha256(binary)
    state["profiling_binary_sha256"] = binary_hash
    results = []
    for quality, scale, visibility, render in EXPECTED_CASES:
        identifier = case_id(quality, scale, visibility)
        state["current_case"] = identifier
        native.atomic_write_json(root / "job.json", {**state, "heartbeat_at": native.utc_now()})
        results.append(
            run_case(
                repo=repo,
                binary=binary,
                binary_sha256=binary_hash,
                artifact=root / "cases" / identifier,
                quality=quality,
                scale=scale,
                visibility=visibility,
                render=render,
                adapter=args.adapter,
                subject_commit=args.subject_commit,
                source_fingerprint=args.source_fingerprint,
            )
        )
        state["cases_completed"] = len(results)
    result_index = {result["case_id"]: result for result in results}
    image_index = {
        identifier: {phase: read_image(root / "cases" / identifier / SCREENSHOT_NAMES[phase]) for phase in PHASES}
        for identifier in result_index
    }
    cross_case_checks = evaluate_cross_case_bridge(observations=result_index, images=image_index)
    manifest = {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "repo": str(repo),
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "profiling_binary_sha256": binary_hash,
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "cases": results,
        "cross_case_checks": cross_case_checks,
        "completed_at": native.utc_now(),
    }
    native.atomic_write_json(root / "manifest.json", manifest)
    state.update(
        {
            "status": "valid",
            "current_case": None,
            "completed_at": native.utc_now(),
        }
    )
    native.atomic_write_json(root / "job.json", state)
    verification = verify_root(
        root,
        subject_commit=args.subject_commit,
        source_fingerprint=args.source_fingerprint,
        harness_fingerprint=args.harness_fingerprint,
    )
    native.print_json(verification)
    return 0


def encode_rgb_png(width: int, height: int, pixels: bytes) -> bytes:
    native.require(len(pixels) == width * height * 3, "self-test RGB payload length differs")
    raw = b"".join(
        b"\x00" + pixels[row * width * 3 : (row + 1) * width * 3]
        for row in range(height)
    )
    ihdr = (
        width.to_bytes(4, "big")
        + height.to_bytes(4, "big")
        + b"\x08\x02\x00\x00\x00"
    )
    return b"\x89PNG\r\n\x1a\n" + native.png_chunk(b"IHDR", ihdr) + native.png_chunk(b"IDAT", zlib.compress(raw)) + native.png_chunk(b"IEND", b"")


def expect_failure(action: Any, label: str) -> None:
    try:
        action()
    except native.AcceptanceError:
        return
    raise native.AcceptanceError(f"P02 negative self-test unexpectedly passed: {label}")


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
    with tempfile.TemporaryDirectory(prefix="p02-presentation-self-test-") as temporary:
        root = Path(temporary)
        width, height = 640, 360
        pixels = bytes([48, 96, 64] * width * height)
        path = root / "door-open.png"
        path.write_bytes(encode_rgb_png(width, height, pixels))
        image = read_image(path)
        native.require(image == (width, height, pixels), "P02 PNG RGB decoder self-test differs")
        roi = {"x": 2, "y": 2, "width": 8, "height": 8}
        metrics = roi_colour_metrics(image, roi)
        native.require(metrics["green_pixels"] == 64, "P02 ROI green predicate self-test differs")
        modified = bytearray(pixels)
        modified[(4 * width + 4) * 3 : (4 * width + 4) * 3 + 3] = b"\xff\x00\x00"
        difference = roi_difference(image, (width, height, bytes(modified)), roi)
        native.require(difference["changed_pixels"] == 1, "P02 ROI difference self-test differs")
        evidence = {
            "file": "door-open.png",
            **image_statistics(image),
            "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
            "window_id": "0x12",
            "window_pid": 1,
            "sha256": sha256(path),
        }
        status = {
            "schema_version": SCHEMA_VERSION,
            "status": "ready",
            "phase": "door-open",
            "generation": 1,
            "fixture_layout_checksum": "0" * 64,
            "render3d": "visible",
            "camera_target": {"x": 0.0, "y": 0.0},
            "window": {"physical_width": width, "physical_height": height},
            "probe": {
                "kind": "door",
                "expected_state": "open",
                "presentation_state": "open",
                "roi": roi,
            },
        }
        validated = validate_probe_status(status, phase="door-open", visibility="visible")
        verify_screenshot_evidence(path=path, evidence=evidence, status=validated, phase="door-open")
        expect_failure(
            lambda: verify_screenshot_evidence(
                path=path,
                evidence={**evidence, "sha256": "1" * 64},
                status=validated,
                phase="door-open",
            ),
            "screenshot hash mutation",
        )
        expect_failure(
            lambda: validate_probe_status(
                {**status, "schema_version": 1}, phase="door-open", visibility="visible"
            ),
            "old probe schema",
        )
        expect_failure(
            lambda: validate_probe_status(
                {**status, "probe": {**status["probe"], "roi": {"x": 636, "y": 356, "width": 8, "height": 8}}},
                phase="door-open",
                visibility="visible",
            ),
            "out of bounds ROI",
        )
    native.print_json({"status": "pass", "schema_version": SCHEMA_VERSION, "cases": len(EXPECTED_CASES)})
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
        for value, label, length in (
            (args.subject_commit, "subject", 40),
            (args.source_fingerprint, "source fingerprint", 64),
            (args.harness_fingerprint, "harness fingerprint", 64),
        ):
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
