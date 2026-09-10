#!/usr/bin/env python3
"""Run and verify the joint Wall/Door lifecycle and construction-preview storyboard."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path
from typing import Any
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import door_behavior_acceptance as door_art  # noqa: E402
import native_acceptance as native  # noqa: E402
import wall_art_acceptance as wall_art  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 1
STATUS_SCHEMA_VERSION = 2
PROFILE = "wall-door-joint-lifecycle-v2"
RELEASE_PROFILE = "wall-door-joint-release-v1"
COVERAGE = {
    "actual_window": ["both-axis-seams", "east-west-continuous-doors", "completion-in-place", "support-relocation",
                      "joint-paused-load", "construction-preview-axis-after-support-change", "corner-seams",
                      "north-south-continuous-doors", "support-removal-and-restoration"],
    "focused_audit": ["wall-paused-replacement", "wall-topology-door-removal", "preview-anchor"],
    "pending_j1": [],
}
SEED = 20_260_906
WINDOW = (1280, 720)
RUN_TIMEOUT_SECONDS = 120.0
POLL_SECONDS = 0.1
CHECKPOINTS = (
    ("wall-door-joint-framed", 1, "joint-framed.png", 3),
    ("wall-door-joint-completed", 2, "joint-completed.png", 1),
    ("wall-door-joint-support-changed", 3, "joint-support-changed.png", 1),
    ("wall-door-joint-support-removed", 4, "joint-support-removed.png", 1),
    ("wall-door-joint-support-restored", 5, "joint-support-restored.png", 1),
    ("wall-door-joint-loaded", 6, "joint-loaded.png", 1),
)
FIXED_TESTS = (
    "systems::save::transaction::tests::mixed_wall_presentation_recovers_after_normal_rollback_and_recovery_replacement",
    "systems::visual::wall_presentation::tests::real_topology_producer_updates_door_add_and_remove_in_the_same_frame",
    "systems::visual::door_preview::tests::production_preview_sets_canvas_anchor_without_changing_tint",
)
ENV_KEYS = (
    "HW_WALL_ART_PREVIEW", "HW_DOOR_ART_PREVIEW", "HW_WALL_ART_ACTUAL_WINDOW", "HW_DOOR_ART_ACTUAL_WINDOW",
    "HW_WALL_ART_PREVIEW_GENERATION", "HW_WALL_ART_PREVIEW_MANIFEST_SHA256",
    "HW_DOOR_ART_PREVIEW_GENERATION", "HW_DOOR_ART_PREVIEW_MANIFEST_SHA256",
    "BEVY_ASSET_ROOT",
    "HW_WINDOW_BACKEND",
    "HW_PRESENT_MODE",
    "WGPU_BACKEND",
    "WGPU_ADAPTER_NAME",
    "HW_WALL_CANDIDATE",
    "HW_WALL_CANDIDATE_GENERATION",
    "HW_WALL_CANDIDATE_MANIFEST_SHA256",
    "HW_DOOR_CANDIDATE",
    "HW_DOOR_CANDIDATE_GENERATION",
    "HW_DOOR_CANDIDATE_MANIFEST_SHA256",
    "HW_DOOR_PERF_PRESENTATION",
    "HW_WALL_DOOR_JOINT_ACTUAL_WINDOW",
    "HW_WALL_DOOR_JOINT_RELEASE",
    "HW_WALL_DOOR_JOINT_STATUS_PATH",
    "HW_WALL_DOOR_JOINT_ACK_PATH",
    "HW_WALL_DOOR_JOINT_SESSION_NONCE",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def assert_harness_matches(repo: Path) -> None:
    actual_root = Path(__file__).resolve().parents[4]
    for relative in native.NATIVE_HARNESS_FILES:
        actual, recorded = actual_root / relative, repo / relative
        native.require(actual.is_file() and recorded.is_file()
                       and not actual.is_symlink() and not recorded.is_symlink()
                       and sha256(actual) == sha256(recorded),
                       f"J1 executing harness differs from the recorded repository: {relative}")


def build_command() -> list[str]:
    return [
        "python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
        "--no-default-features", "--features", "profiling",
    ]


def fixed_test_command(test_name: str) -> list[str]:
    return [
        "python3", "scripts/dev.py", "cargo", "--", "test", "-p", "bevy_app@0.1.0",
        "--lib", "--profile", "profiling", "--no-default-features", "--features", "profiling",
        test_name, "--", "--exact", "--nocapture",
    ]


def game_command(repo: Path, root: Path, *, release: bool = False) -> list[str]:
    return [
        str(repo / "target/profiling/bevy_app"),
        "--perf-scenario",
        "--perf-wall-door-joint-actual-window",
        *(["--perf-wall-door-joint-release"] if release else []),
        "--perf-seed", str(SEED),
        "--perf-size", "small",
        "--perf-workload", "door-density",
        "--perf-render", "gpu",
        "--perf-clock", "realtime",
        "--perf-familiar-policy", "baseline",
        "--perf-operation-dialog", "hidden",
        "--perf-dashboard", "hidden",
        "--perf-output-dir", str(root / "data"),
        "--perf-warmup-secs", "30",
        "--perf-measure-secs", "60",
        "--spawn-souls", "0",
        "--spawn-familiars", "0",
        "--perf-door-presentation", "production",
        "--perf-window-width", str(WINDOW[0]),
        "--perf-window-height", str(WINDOW[1]),
        "--perf-window-scale-factor", "1.0",
        "--perf-rtt-quality", "high",
    ]


def release_identities(repo: Path) -> dict[str, dict[str, Any]]:
    """Validate the runtime mirror; canonical promotion approval is a separate gate."""
    result = {}
    for kind, schema, roles in (
        ("wall", 2, wall_art.FORMWORK_PREVIEW_ROLES),
        ("door", 1, ("mesh:closed", "mesh:open", "mesh:locked", "texture:albedo", "preview:ew", "preview:ns")),
    ):
        asset_id = f"{kind}-production-v1"
        locator = repo / f"assets/manifests/{asset_id}.{kind}set"
        native.require(locator.is_file() and not locator.is_symlink(), f"J1 {kind} release locator is absent")
        payload = native.read_json(locator)
        native.require(locator.read_bytes() == canonical_bytes(payload), "J1 release locator is not canonical")
        generation = payload.get("asset_set_generation")
        digest = payload.get("manifest_sha256")
        native.require(payload.get("schema_version") == schema and payload.get("asset_set_id") == asset_id
                       and payload.get("authority") == "release_approved" and payload.get("review_status") == "art_approved"
                       and type(generation) is int and generation > 0
                       and isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
                       f"J1 {kind} release identity differs")
        if kind == "wall":
            native.require(payload.get("normal_decision") == "rejected" and payload.get("candidate_normal") is None,
                           "J1 released formwork normal contract differs")
        core = payload.get("core")
        native.require(isinstance(core, list) and all(isinstance(item, dict) for item in core)
                       and tuple(item.get("role") for item in core) == tuple(roles), "J1 released core inventory differs")
        receipt = payload.get("receipt")
        native.require(isinstance(receipt, dict) and set(receipt) == {"path", "bytes", "sha256"}
                       and receipt["path"] == f"{kind}_sets/{generation}/authority/promotion-receipt.json",
                       "J1 release receipt reference differs")
        paths = []
        for record in [*core, receipt]:
            relative = Path(record.get("path", ""))
            native.require(not relative.is_absolute() and ".." not in relative.parts
                           and relative.is_relative_to(f"{kind}_sets/{generation}"), "J1 release path escapes its generation")
            asset = repo / "assets" / relative
            native.require(asset.is_file() and not any(part.is_symlink() for part in (asset, *asset.parents))
                           and type(record.get("bytes")) is int and asset.stat().st_size == record["bytes"]
                           and sha256(asset) == record.get("sha256"), f"J1 release bytes differ: {relative}")
            paths.append(relative)
        native.require(len(set(paths)) == len(paths), "J1 release paths are duplicated")
        receipt_path = repo / "assets" / receipt["path"]
        sealed = native.read_json(receipt_path)
        identity = {"asset_set_id": asset_id, "asset_set_generation": generation, "manifest_sha256": digest}
        native.require(receipt_path.read_bytes() == canonical_bytes(sealed) and sealed.get("schema_version") == 1
                       and all(sealed.get(key) == value for key, value in identity.items())
                       and sealed.get("new_active") == identity, "J1 release receipt describes another asset set")
        result[kind] = {"asset_set_generation": generation, "manifest_sha256": digest,
                        "authority": "release_approved", "locator_sha256": sha256(locator),
                        "receipt_sha256": receipt["sha256"]}
    return result


def canonical_bytes(payload: dict) -> bytes:
    return (json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def candidate_identities(repo: Path, *, release: bool = False) -> dict[str, dict[str, Any]]:
    if release:
        return release_identities(repo)
    result = {
        "wall": wall_art.candidate_identity(repo, require_formwork=True),
        "door": door_art.candidate_identity(repo),
    }
    wall = native.read_json(repo / "assets/manifests/wall-production-v1.wallset")
    for record in wall["core"]:
        relative = Path(record["path"])
        native.require(not relative.is_absolute() and ".." not in relative.parts, "Wall core path escapes assets")
        path = repo / "assets" / relative
        native.require(path.is_file() and not path.is_symlink()
                       and path.resolve().is_relative_to((repo / "assets").resolve())
                       and path.stat().st_size == record["bytes"]
                       and sha256(path) == record["sha256"], f"Wall core bytes differ: {relative}")
    return result


def clean_environment(
    repo: Path,
    adapter: str,
    identities: dict[str, dict[str, Any]],
    status_path: Path,
    ack_path: Path,
    nonce: str,
) -> dict[str, str]:
    environment = native.cargo_environment(repo)
    for key in ENV_KEYS:
        environment.pop(key, None)
    wall = identities["wall"]
    door = identities["door"]
    environment.update(
        {
            "BEVY_ASSET_ROOT": str(repo),
            "HW_WINDOW_BACKEND": "x11",
            "HW_PRESENT_MODE": "novsync",
            "WGPU_BACKEND": "vulkan",
            "WGPU_ADAPTER_NAME": adapter,
            "HW_WALL_CANDIDATE": "1",
            "HW_WALL_CANDIDATE_GENERATION": str(wall["asset_set_generation"]),
            "HW_WALL_CANDIDATE_MANIFEST_SHA256": wall["manifest_sha256"],
            "HW_DOOR_CANDIDATE": "1",
            "HW_DOOR_CANDIDATE_GENERATION": str(door["asset_set_generation"]),
            "HW_DOOR_CANDIDATE_MANIFEST_SHA256": door["manifest_sha256"],
            "HW_DOOR_PERF_PRESENTATION": "production",
            "HW_WALL_DOOR_JOINT_ACTUAL_WINDOW": "1",
            "HW_WALL_DOOR_JOINT_STATUS_PATH": str(status_path),
            "HW_WALL_DOOR_JOINT_ACK_PATH": str(ack_path),
            "HW_WALL_DOOR_JOINT_SESSION_NONCE": nonce,
        }
    )
    if wall.get("authority") == "release_approved":
        for key in ENV_KEYS:
            if key.startswith(("HW_WALL_CANDIDATE", "HW_DOOR_CANDIDATE")):
                environment.pop(key, None)
        environment["HW_WALL_DOOR_JOINT_RELEASE"] = "1"
    return environment


def expected_targets(generation: int) -> dict[tuple[int, int], dict[str, Any]]:
    result = {}
    for grid, state, axis in (
        ((14, 44), "Closed", "NorthSouth" if generation in {3, 5, 6} else "EastWest"),
        ((20, 44), "Open", "NorthSouth"), ((26, 44), "Open", "EastWest"),
        ((32, 44), "Locked", "NorthSouth"), ((38, 44), "Closed", "EastWest"),
        ((39, 44), "Open", "EastWest"), ((38, 50), "Closed", "NorthSouth"),
        ((38, 51), "Open", "NorthSouth"), ((26, 50), "Open", "EastWest"),
    ):
        result[grid] = {"kind": "door", "state": state, "axis": axis}
    walls = [(20, 43), (20, 45), (25, 44), (27, 44), (32, 43), (32, 45),
             (37, 44), (40, 44), (38, 49), (38, 52), (25, 50), (27, 50), (25, 51)]
    walls += [(13, 44), (15, 44), (13, 50), (15, 50)] if generation < 3 else [(14, 43), (14, 45), (14, 49), (14, 51)]
    if generation == 4:
        walls.remove((14, 45))
    for grid in walls:
        result[grid] = {"kind": "wall", "provisional": grid == (25, 51) or (generation == 1 and grid in {(25, 44), (32, 43)})}
    result[(14, 50)] = {"kind": "preview", "axis": "EastWest" if generation < 3 else "NorthSouth"}
    return result


def validate_status(
    value: Any,
    *,
    checkpoint: tuple[str, int, str, int],
    nonce: str,
    identities: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    native.require(isinstance(value, dict), "J1 status must be an object")
    phase, generation, _screenshot, provisional_count = checkpoint
    native.require(
        value.get("schema_version") == STATUS_SCHEMA_VERSION
        and value.get("status") == "ready"
        and value.get("phase") == phase
        and value.get("generation") == generation
        and value.get("session_nonce") == nonce,
        "J1 status identity differs",
    )
    native.require(value.get("window") == {"width": 1280, "height": 720, "scale_factor": 1.0}, "J1 window differs")
    native.require(value.get("render") == {"backend": "vulkan", "rtt_quality": "high"}, "J1 render contract differs")
    runtime = value.get("candidate_identity")
    native.require(isinstance(runtime, dict), "J1 runtime identity is absent")
    for kind in ("wall", "door"):
        expected = identities[kind]
        observed = runtime.get(kind)
        native.require(
            isinstance(observed, dict)
            and observed.get("authority") == ("ReleaseApproved" if identities["wall"].get("authority") == "release_approved" else "IsolatedCandidate")
            and observed.get("asset_set_generation") == expected["asset_set_generation"]
            and observed.get("manifest_sha256") == expected["manifest_sha256"],
            f"J1 {kind} runtime identity differs",
        )
    gallery = value.get("gallery")
    expected = expected_targets(generation)
    count = len(expected)
    native.require(type(value.get("world_epoch")) is int and value["world_epoch"] >= 0
                   and value.get("paused_load_complete") is (generation == 6), "J1 world replacement evidence differs")
    native.require(
        isinstance(gallery, dict)
        and gallery.get("door_count") == 9
        and gallery.get("wall_count") == (16 if generation == 4 else 17)
        and gallery.get("provisional_wall_count") == provisional_count
        and gallery.get("continuous_door_count") == 4
        and gallery.get("preview_count") == 1
        and gallery.get("both_axes") is True
        and gallery.get("owner_visual_identity_stable") is True,
        "J1 gallery contract differs",
    )
    targets = gallery.get("projected_targets")
    native.require(isinstance(targets, list) and len(targets) == count, "J1 projection coverage differs")
    for target in targets:
        native.require(
            isinstance(target, dict)
            and isinstance(target.get("grid"), list)
            and len(target["grid"]) == 2
            and all(type(target.get(axis)) in {int, float} and math.isfinite(float(target[axis])) for axis in ("x", "y")),
            "J1 projected target differs",
        )
        native.require(20 <= target["x"] < WINDOW[0] - 20 and 20 <= target["y"] < WINDOW[1] - 20, "J1 target is outside the client")
        identity = target.get("identity")
        native.require(isinstance(identity, dict) and identity.get("kind") in {"wall", "door", "preview"}
                       and isinstance(identity.get("owner"), str) and identity["owner"].isdigit()
                       and isinstance(identity.get("visual"), str) and identity["visual"].isdigit(), "J1 target identity differs")
        spec = expected.get(tuple(target["grid"]))
        native.require(spec is not None and all(identity.get(key) == item for key, item in spec.items()), "J1 target semantic contract differs")
    native.require(len({tuple(target["grid"]) for target in targets}) == count
                   and len({target["identity"]["owner"] for target in targets}) == count
                   and len({target["identity"]["visual"] for target in targets}) == count, "J1 target identities are duplicated")
    return value


def validate_transitions(observations: list[dict], identities: list[dict[str, str]]) -> None:
    native.require(identities[0] == identities[1] == identities[2], "J1 visual identity changed before teardown")
    removed = next(item["identity"]["owner"] for item in observations[2]["status"]["gallery"]["projected_targets"] if item["grid"] == [14, 45])
    native.require({owner: visual for owner, visual in identities[2].items() if owner != removed} == identities[3], "J1 teardown changed a surviving visual")
    restored = next(item["identity"]["owner"] for item in observations[4]["status"]["gallery"]["projected_targets"] if item["grid"] == [14, 45])
    native.require(restored != removed and set(identities[4]) - set(identities[3]) == {restored}
                   and {owner: visual for owner, visual in identities[4].items() if owner != restored} == identities[3], "J1 restoration did not create just one replacement")
    native.require(not (set(identities[4]) & set(identities[5]))
                   and not (set(identities[4].values()) & set(identities[5].values())), "J1 normal load retained old world entities")
    epochs = [item["status"]["world_epoch"] for item in observations]
    native.require(epochs[:5] == [epochs[0]] * 5 and epochs[5] == epochs[0] + 1, "J1 world epoch sequence differs")


def image_evidence(path: Path, status: dict[str, Any]) -> list[dict[str, Any]]:
    native.require(path.is_file() and not path.is_symlink(), "J1 screenshot is absent or a symlink")
    payload = path.read_bytes()
    native.validate_png_structure(payload)
    width, height, pixels = native.decode_png_rgb(payload)
    native.require((width, height) == WINDOW and pixels, "J1 screenshot dimensions differ")
    evidence = []
    for target in status["gallery"]["projected_targets"]:
        x, y = round(target["x"]), round(target["y"])
        crop = b"".join(pixels[((row * width) + x - 20) * 3:((row * width) + x + 20) * 3] for row in range(y - 20, y + 20))
        luminance = [0.2126 * crop[i] + 0.7152 * crop[i + 1] + 0.0722 * crop[i + 2] for i in range(0, len(crop), 3)]
        mean = sum(luminance) / len(luminance)
        deviation = math.sqrt(sum((item - mean) ** 2 for item in luminance) / len(luminance))
        native.require(deviation >= 2.0 and mean >= 5, f"J1 target {target['grid']} contains no credible detail")
        evidence.append({"grid": target["grid"], "roi_sha256": hashlib.sha256(crop).hexdigest(), "standard_deviation": round(deviation, 6)})
    return evidence


def capture_window(destination: Path, root_pid: int, status: dict[str, Any]) -> dict[str, Any] | None:
    candidates = native.x11_client_windows_for_process_tree(root_pid)
    if not candidates:
        return None
    native.require(len(candidates) == 1, "J1 exposed multiple X11 client windows")
    window_id, window_pid = candidates[0]
    import_command = shutil.which("import")
    native.require(import_command is not None, "J1 requires ImageMagick import")
    completed = native.run_bounded_capture_tool(
        [import_command, "-window", window_id, "-depth", "8", "-type", "TrueColor", str(destination)],
        label="J1 X11 client screenshot",
    )
    if completed.returncode != 0 or not destination.is_file():
        destination.unlink(missing_ok=True)
        return None
    return {
        "file": destination.name,
        "sha256": sha256(destination),
        "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
        "window_id": window_id,
        "window_pid": window_pid,
        "image_evidence": image_evidence(destination, status),
    }


def acknowledgement(status: dict[str, Any]) -> dict[str, Any]:
    return {
        "schema_version": STATUS_SCHEMA_VERSION,
        "status": "captured",
        "session_nonce": status["session_nonce"],
        "phase": status["phase"],
        "generation": status["generation"],
    }


def run_storyboard(
    *,
    repo: Path,
    root: Path,
    adapter: str,
    identities: dict[str, dict[str, Any]],
    job_file: Path,
    state: dict[str, Any],
) -> list[dict[str, Any]]:
    status_path = root / "probe-status.json"
    ack_path = root / "probe-ack.json"
    nonce = secrets.token_hex(16)
    command = game_command(repo, root, release=identities["wall"].get("authority") == "release_approved")
    environment = clean_environment(repo, adapter, identities, status_path, ack_path, nonce)
    state.setdefault("commands", []).append({"stage": "actual-window", "argv": command})
    native.atomic_write_json(job_file, state)
    observations: list[dict[str, Any]] = []
    deadline = time.monotonic() + RUN_TIMEOUT_SECONDS
    log_path = root / "actual-window.log"
    native.admit_stage_start("actual-window", state=state, job_file=job_file)
    with log_path.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            command, cwd=repo, env=environment, stdout=log, stderr=subprocess.STDOUT,
            text=True, start_new_session=True, pass_fds=native.activity_pass_fds(environment),
        )
        state["child_pid"] = process.pid
        native.atomic_write_json(job_file, state)
        try:
            while process.poll() is None:
                if status_path.is_file() and not status_path.is_symlink():
                    value = native.read_json(status_path)
                    if isinstance(value, dict) and value.get("status") == "failed":
                        raise native.AcceptanceError(f"J1 probe failed: {value.get('reason')}")
                    checkpoint = CHECKPOINTS[len(observations)] if len(observations) < len(CHECKPOINTS) else None
                    if checkpoint is not None and isinstance(value, dict) and value.get("phase") == checkpoint[0]:
                        status = validate_status(value, checkpoint=checkpoint, nonce=nonce, identities=identities)
                        screenshot = capture_window(root / checkpoint[2], process.pid, status)
                        if screenshot is not None:
                            observations.append({"status": status, "screenshot": screenshot})
                            native.atomic_write_json(ack_path, acknowledgement(status))
                if time.monotonic() >= deadline:
                    raise native.AcceptanceError("J1 actual-window storyboard timed out")
                state["heartbeat_at"] = native.utc_now()
                native.atomic_write_json(job_file, state)
                time.sleep(POLL_SECONDS)
        finally:
            if process.poll() is None:
                native.stop_command_process(process)
            ack_path.unlink(missing_ok=True)
    native.require(process.returncode == 0, f"J1 process exited with {process.returncode}")
    native.require(len(observations) == len(CHECKPOINTS), "J1 did not capture every checkpoint")
    verify_game_log(log_path, adapter)
    state["child_pid"] = None
    native.atomic_write_json(job_file, state)
    return observations


def verify_game_log(path: Path, adapter: str) -> dict[str, str]:
    text = path.read_text(encoding="utf-8")
    native.require(re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", text) is None, "J1 game log contains a warning or error")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', text)
    native.require(len(adapters) == 1 and adapter.casefold() in adapters[0][0].casefold()
                   and adapters[0][1] == "Vulkan", "J1 actual adapter/backend differs")
    return {"name": adapters[0][0], "backend": adapters[0][1]}


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    released = manifest.get("release", False)
    native.require(type(released) is bool, "J1 release selection differs")
    profile = RELEASE_PROFILE if released else PROFILE
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == profile,
        "J1 manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    assert_harness_matches(repo)
    native.require(manifest.get("coverage") == COVERAGE, "J1 seam scope differs")
    door_art.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"]
        and native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"]
        and density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"]
        and candidate_identities(repo, release=released) == manifest["candidate_identities"],
        "J1 provenance changed",
    )
    observations = manifest.get("observations")
    native.require(isinstance(observations, list) and len(observations) == len(CHECKPOINTS), "J1 observations differ")
    for checkpoint, observation in zip(CHECKPOINTS, observations, strict=True):
        validate_status(
            observation.get("status"), checkpoint=checkpoint,
            nonce=manifest["session_nonce"], identities=manifest["candidate_identities"],
        )
        screenshot = observation.get("screenshot")
        path = root / checkpoint[2]
        native.require(
            isinstance(screenshot, dict)
            and screenshot.get("capture_scope") == native.SAVE_CATALOG_CAPTURE_SCOPE
            and sha256(path) == screenshot.get("sha256"),
            "J1 screenshot evidence changed",
        )
        native.validate_png_structure(path.read_bytes())
        native.require(image_evidence(path, observation["status"]) == screenshot.get("image_evidence"), "J1 target pixels changed")
    identities = [{item["identity"]["owner"]: item["identity"]["visual"]
                   for item in observation["status"]["gallery"]["projected_targets"]} for observation in observations]
    validate_transitions(observations, identities)
    fixed = manifest.get("fixed_tests")
    native.require(isinstance(fixed, list) and [test.get("test") for test in fixed] == list(FIXED_TESTS), "J1 fixed test inventory differs")
    for index, test in enumerate(fixed):
        native.require(test.get("log") == f"fixed-{index + 1}.log", "J1 fixed audit path differs")
        path = root / test["log"]
        native.require(path.is_file() and sha256(path) == test["sha256"], "J1 fixed audit changed")
        text = path.read_text(encoding="utf-8")
        native.require("1 passed; 0 failed" in text, "J1 fixed audit no longer proves one exact test")
    native.require(sha256(repo / "target/profiling/bevy_app") == manifest["binary_sha256"], "J1 binary changed")
    native.require(sha256(root / "actual-window.log") == manifest["game_log_sha256"], "J1 game log changed")
    native.require(verify_game_log(root / "actual-window.log", manifest["adapter"]) == manifest["actual_adapter"], "J1 adapter evidence changed")
    final_status = native.read_json(root / "probe-status.json")
    native.require(final_status == {
        "schema_version": STATUS_SCHEMA_VERSION, "status": "complete",
        "session_nonce": manifest["session_nonce"], "phase": CHECKPOINTS[-1][0], "generation": CHECKPOINTS[-1][1],
    }, "J1 final checkpoint was not acknowledged")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": profile,
        "root": str(root),
        "fixed_tests": len(FIXED_TESTS),
        "screenshots": len(CHECKPOINTS),
        "coverage": COVERAGE,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    identities = None
    try:
        assert_harness_matches(repo)
        door_art.assert_fully_clean(repo, subject)
        identities = candidate_identities(repo, release=args.release)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "wall-door-joint")
    native.require(root.is_relative_to(repo / "target/native-acceptance"), "J1 job root must be under target/native-acceptance")
    native.require_persistent_storage(root, label="J1 job root")
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1",
        "python3", str(Path(__file__).resolve()), "run",
        "--repo", str(repo), "--job-root", str(root), "--subject-commit", subject,
        "--source-fingerprint", source, "--harness-fingerprint", harness,
        "--asset-view-fingerprint", assets, "--adapter", args.adapter,
        *(["--release"] if args.release else []),
    ]
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": RELEASE_PROFILE if args.release else PROFILE,
            "release": args.release,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "candidate_identities": identities,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "status_command": ["python3", str(Path(__file__).resolve()), "status", "--job-root", str(root)],
            "verify_command": ["python3", str(Path(__file__).resolve()), "verify", "--job-root", str(root)],
            "execution_contract": {
                "coverage": COVERAGE,
                "actual_window_required": True,
                "capture_scope": native.SAVE_CATALOG_CAPTURE_SCOPE,
                "parallel_game_processes": 1,
                "checkpoints": [item[0] for item in CHECKPOINTS],
                "fixed_tests": list(FIXED_TESTS),
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "J1 must use the planned direct kitty command")
    repo = native.validate_repo(args.repo)
    assert_harness_matches(repo)
    native.require(native.git_subject(repo) == args.subject_commit, "J1 subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "J1 source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "J1 harness changed")
    native.require(density.asset_view_fingerprint(repo) == args.asset_view_fingerprint, "J1 asset view changed")
    door_art.assert_fully_clean(repo, args.subject_commit)
    resources = native.resource_snapshot(repo, require_launcher=True)
    native.require(not resources["failures"], f"J1 resource preflight failed: {resources['failures']}")
    identities = candidate_identities(repo, release=args.release)
    root = Path(args.job_root).resolve()
    native.require(root.is_relative_to(repo / "target/native-acceptance"), "J1 job root must be under target/native-acceptance")
    native.require_persistent_storage(root, label="J1 job root")
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION, "status": "running", "profile": RELEASE_PROFILE if args.release else PROFILE,
        "release": args.release,
        "subject_commit": args.subject_commit, "current_stage": "fixed-audit",
        "child_pid": None, "heartbeat_at": native.utc_now(), "commands": [],
        "resources": resources,
    }
    job_file = root / "job.json"
    native.atomic_write_json(job_file, state)
    try:
        fixed_results = []
        for index, test_name in enumerate(FIXED_TESTS):
            log_path = root / f"fixed-{index + 1}.log"
            native.run_command(
                f"fixed-{index + 1}", fixed_test_command(test_name), repo=repo,
                env=native.cargo_environment(repo), log_path=log_path, job_file=job_file, state=state,
                timeout_seconds=3600.0,
            )
            text = log_path.read_text(encoding="utf-8")
            native.require("running 1 test" in text and "1 passed; 0 failed" in text, "J1 fixed audit did not execute exactly one passing test")
            fixed_results.append({"test": test_name, "log": log_path.name, "sha256": sha256(log_path)})
        native.run_command(
            "build", build_command(), repo=repo, env=native.cargo_environment(repo),
            log_path=root / "build.log", job_file=job_file, state=state, timeout_seconds=3600.0,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(binary.is_file() and not binary.is_symlink(), "J1 profiling binary is absent")
        state["current_stage"] = "actual-window"
        observations = run_storyboard(
            repo=repo, root=root, adapter=args.adapter, identities=identities,
            job_file=job_file, state=state,
        )
        manifest = {
            "schema_version": SCHEMA_VERSION, "status": "pass", "profile": RELEASE_PROFILE if args.release else PROFILE,
            "release": args.release,
            "repo": str(repo), "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "candidate_identities": identities,
            "coverage": COVERAGE,
            "actual_adapter": verify_game_log(root / "actual-window.log", args.adapter),
            "game_log_sha256": sha256(root / "actual-window.log"),
            "adapter": args.adapter, "binary_sha256": sha256(binary),
            "session_nonce": observations[0]["status"]["session_nonce"],
            "fixed_tests": fixed_results, "observations": observations,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        native.print_json(verify_root(root))
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(job_file, state)
        return 0
    except Exception as error:
        state.update({"status": "invalid", "failure": f"{type(error).__name__}: {error}", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(job_file, state)
        raise


def status(args: argparse.Namespace) -> int:
    value = native.read_json(Path(args.job_root).resolve() / "job.json")
    if value.get("status") == "running":
        heartbeat = value.get("heartbeat_at")
        native.require(isinstance(heartbeat, str), "J1 heartbeat is missing")
        age = (datetime.now(UTC) - native.parse_utc(heartbeat)).total_seconds()
        native.require(0 <= age <= 120, f"J1 heartbeat is stale: {age:.1f} seconds")
    native.print_json(value)
    return 2 if value.get("status") == "running" else 0 if value.get("status") == "valid" else 1


def verify(args: argparse.Namespace) -> int:
    native.print_json(verify_root(Path(args.job_root).resolve()))
    return 0


def self_test_release() -> None:
    with tempfile.TemporaryDirectory(prefix="joint-release-self-test-") as temporary:
        repo = Path(temporary).resolve()
        locators = {}
        for kind, schema, roles in (
            ("wall", 2, wall_art.FORMWORK_PREVIEW_ROLES),
            ("door", 1, ("mesh:closed", "mesh:open", "mesh:locked", "texture:albedo", "preview:ew", "preview:ns")),
        ):
            identity = {"asset_set_id": f"{kind}-production-v1", "asset_set_generation": 1,
                        "manifest_sha256": ("a" if kind == "wall" else "b") * 64}
            core = []
            for index, role in enumerate(roles):
                relative = f"{kind}_sets/1/core-{index}.bin"
                asset = repo / "assets" / relative
                asset.parent.mkdir(parents=True, exist_ok=True)
                asset.write_bytes(f"test-only {kind} {role}".encode())
                core.append({"path": relative, "bytes": asset.stat().st_size, "sha256": sha256(asset), "role": role})
            relative = f"{kind}_sets/1/authority/promotion-receipt.json"
            receipt = repo / "assets" / relative
            receipt.parent.mkdir(parents=True)
            receipt.write_bytes(canonical_bytes({"schema_version": 1, **identity, "new_active": identity}))
            payload = {"schema_version": schema, **identity, "authority": "release_approved", "review_status": "art_approved",
                       "normal_decision": "rejected", "candidate_normal": None, "core": core,
                       "receipt": {"path": relative, "bytes": receipt.stat().st_size, "sha256": sha256(receipt)}}
            locator = repo / f"assets/manifests/{kind}-production-v1.{kind}set"
            locator.parent.mkdir(parents=True, exist_ok=True)
            locator.write_bytes(canonical_bytes(payload))
            locators[kind] = (locator, payload)
        identities = release_identities(repo)
        native.require(set(identities) == {"wall", "door"}, "J1 release fixture inventory differs")
        with mock.patch.object(native, "cargo_environment", return_value={key: "inherited" for key in ENV_KEYS}):
            environment = clean_environment(repo, "Intel", identities, repo / "status", repo / "ack", "a" * 32)
            native.require(environment.get("HW_WALL_DOOR_JOINT_RELEASE") == "1"
                           and not any(key.startswith(("HW_WALL_CANDIDATE", "HW_DOOR_CANDIDATE", "HW_WALL_ART_PREVIEW", "HW_DOOR_ART_PREVIEW")) for key in environment),
                           "J1 release inherited a candidate or art-preview opt-in")
            candidate = copy.deepcopy(identities)
            candidate["wall"]["authority"] = "isolated_candidate"
            environment = clean_environment(repo, "Intel", candidate, repo / "status", repo / "ack", "a" * 32)
            native.require("HW_WALL_DOOR_JOINT_RELEASE" not in environment
                           and environment.get("HW_WALL_CANDIDATE") == environment.get("HW_DOOR_CANDIDATE") == "1",
                           "J1 candidate inherited release selection")
        for kind in ("wall", "door"):
            locator, original = locators[kind]
            for mutation in ("candidate", "wrong-generation", "wrong-receipt", "tampered-core", "duplicate-core", "symlink"):
                changed = copy.deepcopy(original)
                if mutation == "candidate":
                    changed["authority"] = "isolated_candidate"
                elif mutation == "wrong-generation":
                    changed["core"][0]["path"] = f"{kind}_sets/2/core-0.bin"
                elif mutation == "wrong-receipt":
                    changed["receipt"] = locators["door" if kind == "wall" else "wall"][1]["receipt"]
                elif mutation == "tampered-core":
                    changed["core"][0]["sha256"] = "f" * 64
                elif mutation == "duplicate-core":
                    changed["core"][1] = {**changed["core"][0], "role": changed["core"][1]["role"]}
                else:
                    link = repo / f"assets/{kind}_sets/1/link"
                    link.symlink_to(repo / f"assets/{kind}_sets/1", target_is_directory=True)
                    changed["core"][0]["path"] = f"{kind}_sets/1/link/core-0.bin"
                locator.write_bytes(canonical_bytes(changed))
                try:
                    release_identities(repo)
                except native.AcceptanceError:
                    pass
                else:
                    raise native.AcceptanceError(f"J1 released {kind} accepted {mutation}")
                locator.write_bytes(canonical_bytes(original))
            receipt = repo / "assets" / original["receipt"]["path"]
            saved_receipt = receipt.read_bytes()
            wrong = native.read_json(receipt)
            wrong["asset_set_id"] = "another-asset"
            receipt.write_bytes(canonical_bytes(wrong))
            changed = copy.deepcopy(original)
            changed["receipt"].update(bytes=receipt.stat().st_size, sha256=sha256(receipt))
            locator.write_bytes(canonical_bytes(changed))
            try:
                release_identities(repo)
            except native.AcceptanceError:
                pass
            else:
                raise native.AcceptanceError("J1 release accepted rehashed receipt for another asset")
            receipt.write_bytes(saved_receipt)
            locator.write_bytes(canonical_bytes(original))
        actual_root = Path(__file__).resolve().parents[4]
        for relative in native.NATIVE_HARNESS_FILES:
            target = repo / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(actual_root / relative, target)
        assert_harness_matches(repo)
        changed_helper = repo / Path(__file__).resolve().relative_to(actual_root)
        changed_helper.write_bytes(changed_helper.read_bytes() + b"\n# changed test harness\n")
        try:
            assert_harness_matches(repo)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError("J1 accepted an unrecorded executing helper")


def self_test() -> int:
    self_test_release()
    native.require([item[1] for item in CHECKPOINTS] == list(range(1, 7)), "J1 checkpoint order differs")
    command = game_command(Path("/repo"), Path("/job"))
    native.require(
        command[command.index("--perf-workload") + 1] == "door-density"
        and "--perf-wall-door-joint-actual-window" in command
        and command[command.index("--perf-door-presentation") + 1] == "production",
        "J1 game command differs",
    )
    native.require(len(FIXED_TESTS) == 3 and all("::tests::" in item for item in FIXED_TESTS), "J1 fixed audit differs")
    for test_name in FIXED_TESTS:
        audit = fixed_test_command(test_name)
        native.require("--lib" in audit and "--no-default-features" in audit
                       and audit[audit.index("--profile") + 1] == "profiling"
                       and audit[-4:] == [test_name, "--", "--exact", "--nocapture"],
                       "J1 fixed audit build contract differs")
    identities = {kind: {"asset_set_generation": generation, "manifest_sha256": digit * 64}
                  for kind, generation, digit in (("wall", 10, "a"), ("door", 6, "b"))}
    nonce = "0123456789abcdef0123456789abcdef"
    status_value = {
        "schema_version": STATUS_SCHEMA_VERSION, "status": "ready", "phase": CHECKPOINTS[0][0], "generation": 1,
        "world_epoch": 0, "paused_load_complete": False,
        "session_nonce": nonce, "window": {"width": 1280, "height": 720, "scale_factor": 1.0},
        "render": {"backend": "vulkan", "rtt_quality": "high"},
        "candidate_identity": {kind: {**value, "authority": "IsolatedCandidate"} for kind, value in identities.items()},
        "gallery": {"door_count": 9, "wall_count": 17, "provisional_wall_count": 3, "preview_count": 1,
                    "continuous_door_count": 4, "both_axes": True, "owner_visual_identity_stable": True,
                    "projected_targets": [{"grid": list(grid), "x": 100 + index * 30, "y": 320,
                        "identity": {**spec, "owner": str(index), "visual": str(index + 27)}} for index, (grid, spec) in enumerate(expected_targets(1).items())]},
    }
    validate_status(status_value, checkpoint=CHECKPOINTS[0], nonce=nonce, identities=identities)
    release_status = copy.deepcopy(status_value)
    release_identity = copy.deepcopy(identities)
    for kind in ("wall", "door"):
        release_identity[kind]["authority"] = "release_approved"
        release_status["candidate_identity"][kind]["authority"] = "ReleaseApproved"
    validate_status(release_status, checkpoint=CHECKPOINTS[0], nonce=nonce, identities=release_identity)
    for wrong_status, expected_identity in ((status_value, release_identity), (release_status, identities)):
        try:
            validate_status(wrong_status, checkpoint=CHECKPOINTS[0], nonce=nonce, identities=expected_identity)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError("J1 status accepted the other authority")
    native.require("--perf-wall-door-joint-release" not in command
                   and "--perf-wall-door-joint-release" in game_command(Path("/repo"), Path("/job"), release=True),
                   "J1 release command selection differs")
    for path, replacement in (
        (("generation",), 2), (("session_nonce",), "f" * 32),
        (("candidate_identity", "wall", "manifest_sha256"), "c" * 64),
        (("candidate_identity", "door", "authority"), "ArtPreview"),
        (("gallery", "projected_targets", 0, "x"), -20),
        (("gallery", "projected_targets", 0, "x"), float("nan")),
        (("gallery", "projected_targets", 0, "identity", "owner"), "1"),
        (("gallery", "owner_visual_identity_stable"), False),
        (("world_epoch",), -1), (("paused_load_complete",), True),
        (("gallery", "projected_targets", 0, "identity", "axis"), "NorthSouth"),
    ):
        changed = copy.deepcopy(status_value)
        parent = changed
        for key in path[:-1]:
            parent = parent[key]
        parent[path[-1]] = replacement
        try:
            validate_status(changed, checkpoint=CHECKPOINTS[0], nonce=nonce, identities=identities)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError(f"J1 status accepted changed {path}")
    observations = []
    owners = {grid: index for index, grid in enumerate(expected_targets(1))}
    for checkpoint in CHECKPOINTS:
        generation = checkpoint[1]
        if generation == 3:
            for before, after in (((13, 44), (14, 43)), ((15, 44), (14, 45)),
                                  ((13, 50), (14, 49)), ((15, 50), (14, 51))):
                owners[after] = owners.pop(before)
        if generation == 4:
            del owners[(14, 45)]
        if generation == 5:
            owners[(14, 45)] = 100
        if generation == 6:
            owners = {grid: owner + 1000 for grid, owner in owners.items()}
        status = copy.deepcopy(status_value)
        status.update(phase=checkpoint[0], generation=generation, world_epoch=int(generation == 6),
                      paused_load_complete=generation == 6)
        status["gallery"].update(wall_count=16 if generation == 4 else 17,
                                 provisional_wall_count=checkpoint[3], projected_targets=[
            {"grid": list(grid), "x": 100 + index * 30, "y": 320,
             "identity": {**spec, "owner": str(owners[grid]), "visual": str(owners[grid] + 5000)}}
            for index, (grid, spec) in enumerate(expected_targets(generation).items())])
        validate_status(status, checkpoint=checkpoint, nonce=nonce, identities=identities)
        observations.append({"status": status})
    entity_maps = [{item["identity"]["owner"]: item["identity"]["visual"]
                   for item in observation["status"]["gallery"]["projected_targets"]} for observation in observations]
    validate_transitions(observations, entity_maps)
    for index in (1, 3, 4, 5):
        changed = copy.deepcopy(entity_maps)
        owner = next(iter(changed[index]))
        changed[index][owner] = "999999" if index != 5 else next(iter(entity_maps[4].values()))
        try:
            validate_transitions(observations, changed)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError(f"J1 accepted invalid identity transition {index}")
    changed = copy.deepcopy(observations)
    changed[-1]["status"]["world_epoch"] = 0
    try:
        validate_transitions(changed, entity_maps)
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("J1 accepted load without world replacement")
    native.print_json({"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE})
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    plan_parser.add_argument("--release", action="store_true")
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--release", action="store_true")
    for name in ("repo", "job_root", "subject_commit", "source_fingerprint", "harness_fingerprint", "asset_view_fingerprint", "adapter"):
        run_parser.add_argument("--" + name.replace("_", "-"), required=True)
    for name in ("status", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        return {"plan": plan, "run": run, "status": status, "verify": verify, "self-test": lambda _: self_test()}[args.command](args)
    except (native.AcceptanceError, OSError, KeyError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
