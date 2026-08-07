#!/usr/bin/env python3
"""Fail-closed formal RenderDoc capture helper for the RtT-light baseline."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import uuid
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
RUNTIME_CHECKPOINT_SCHEMA_VERSION = 2
EXTRACTION_SCHEMA_VERSION = 2
CONTRACT_FILE = "scripts/perf_tool/contracts/rtt_light_migration_v1.json"
SOURCE_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "scripts/perf.py",
}
SOURCE_PREFIXES = ("crates/", "scripts/perf_tool/")
ASSET_PREFIX = "assets/"
RENDERDOC_API_VERSION = "1.6.0"
RENDERDOC_VULKAN_MANIFEST = Path(
    "share/vulkan/implicit_layer.d/renderdoc_capture.json"
)
LOG_PROBLEM_RE = re.compile(
    r"\b(?:WARN(?:ING)?|ERROR|FATAL|CRITICAL|panicked)\b|bevy_ecs::error::handler",
    re.IGNORECASE,
)

EXPECTED_RENDER_RESOURCES = {
    "scene_target_label": "hell-workers-rtt-scene",
    "mask_target_label": "hell-workers-rtt-soul-mask",
    "composite_draw_count": 1,
    "composite_texture_bindings": [
        {
            "target": "scene_target",
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 1,
        },
        {
            "target": "mask_target",
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 3,
        },
    ],
    "composite_sampler_bindings": [
        {
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 2,
        },
        {
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 4,
        },
    ],
}


class CaptureError(RuntimeError):
    """A formal capture prerequisite or evidence validation failed."""


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise CaptureError(f"cannot read JSON object {path}: {error}") from error
    if not isinstance(value, dict):
        raise CaptureError(f"JSON artifact is not an object: {path}")
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _regular_file(value: str, label: str, *, executable: bool = False) -> Path:
    path = Path(value).resolve()
    if not path.is_file():
        raise CaptureError(f"{label} is not a regular file: {path}")
    if executable and not os.access(path, os.X_OK):
        raise CaptureError(f"{label} is not executable: {path}")
    return path


def _command_output(command: list[str], *, cwd: Path | None = None) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip()
        raise CaptureError(
            f"command failed ({' '.join(command)}): {detail or completed.returncode}"
        )
    return completed.stdout.strip()


def _tool_version(path: Path) -> str:
    failures: list[str] = []
    for arguments in (("version",), ("--version",)):
        try:
            completed = subprocess.run(
                [str(path), *arguments],
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
            )
        except subprocess.SubprocessError as error:
            failures.append(f"{' '.join(arguments)}: {error}")
            continue
        output = (completed.stdout or completed.stderr).strip()
        if completed.returncode == 0 and output:
            return output.splitlines()[0]
        failures.append(f"{' '.join(arguments)} rc={completed.returncode}")
    raise CaptureError(f"cannot query tool version from {path}: {'; '.join(failures)}")


def _version_tuple(value: str) -> tuple[int, int]:
    match = re.search(r"(?<!\d)(\d+)\.(\d+)(?:\.\d+)?(?!\d)", value)
    if match is None:
        raise CaptureError(f"cannot parse RenderDoc major/minor version: {value!r}")
    return int(match.group(1)), int(match.group(2))


def _help_text(path: Path, arguments: tuple[str, ...]) -> str:
    completed = subprocess.run(
        [str(path), *arguments],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    output = "\n".join(filter(None, (completed.stdout, completed.stderr)))
    if completed.returncode != 0 or not output:
        raise CaptureError(f"cannot inspect {' '.join((str(path), *arguments))}")
    return output


def inspect_tools(
    renderdoccmd_value: str,
    qrenderdoc_value: str,
    library_value: str,
) -> dict[str, Any]:
    renderdoccmd = _regular_file(renderdoccmd_value, "renderdoccmd", executable=True)
    qrenderdoc = _regular_file(qrenderdoc_value, "qrenderdoc", executable=True)
    library = _regular_file(library_value, "librenderdoc")
    if library.stat().st_size <= 0 or library.read_bytes()[:4] != b"\x7fELF":
        raise CaptureError(f"librenderdoc is not a nonempty ELF file: {library}")
    renderdoc_version = _tool_version(renderdoccmd)
    qrenderdoc_version = _tool_version(qrenderdoc)
    if _version_tuple(renderdoc_version) != _version_tuple(qrenderdoc_version):
        raise CaptureError(
            "renderdoccmd and qrenderdoc major/minor versions differ: "
            f"{renderdoc_version!r} != {qrenderdoc_version!r}"
        )
    capture_help = _help_text(renderdoccmd, ("capture", "--help"))
    for option in ("--capture-file", "--wait-for-exit", "--working-dir"):
        if option not in capture_help:
            raise CaptureError(f"renderdoccmd capture does not advertise {option}")
    if "--python" not in _help_text(qrenderdoc, ("--help",)):
        raise CaptureError("qrenderdoc does not advertise --python")
    extractor = Path(__file__).resolve().with_name("renderdoc_extract.py")
    if not extractor.is_file():
        raise CaptureError(f"RenderDoc extractor is missing: {extractor}")
    return {
        "renderdoccmd": renderdoccmd,
        "qrenderdoc": qrenderdoc,
        "library": library,
        "extractor": extractor,
        "renderdoc_version": renderdoc_version,
        "qrenderdoc_version": qrenderdoc_version,
    }


def _portable_vulkan_manifest_source(tools: dict[str, Any]) -> Path:
    """Locate the manifest shipped with the selected RenderDoc toolchain."""

    renderdoccmd = tools["renderdoccmd"]
    library = tools["library"]
    candidates = (
        renderdoccmd.parent.parent / RENDERDOC_VULKAN_MANIFEST,
        library.parent.parent.parent / RENDERDOC_VULKAN_MANIFEST,
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise CaptureError(
        "RenderDoc Vulkan implicit-layer manifest is unavailable next to the "
        f"selected tools: {candidates[0]}"
    )


def prepare_portable_vulkan_layer(tools: dict[str, Any], work: Path) -> Path:
    """Write a per-run manifest that points at the inspected RenderDoc library.

    Portable RenderDoc bundles retain the distribution's absolute library path
    in their manifest.  That path is invalid after the bundle is unpacked in
    the workspace, so do not register it globally.  The Vulkan loader instead
    discovers this short-lived, rewritten manifest through
    ``VK_ADD_IMPLICIT_LAYER_PATH`` for the capture child only.
    """

    source = _portable_vulkan_manifest_source(tools)
    manifest = read_json(source)
    layer = manifest.get("layer")
    if (
        not isinstance(layer, dict)
        or layer.get("name") != "VK_LAYER_RENDERDOC_Capture"
        or layer.get("type") != "GLOBAL"
        or not isinstance(layer.get("library_path"), str)
        or not layer["library_path"]
    ):
        raise CaptureError(f"RenderDoc Vulkan manifest is invalid: {source}")
    layer["library_path"] = str(tools["library"])
    directory = work / "vulkan" / "implicit_layer.d"
    directory.mkdir(parents=True)
    _write_json_exclusive(directory / "renderdoc_capture.json", manifest)
    return directory


def _retain_renderdoc_diagnostics(work: Path, output: Path, message: str) -> CaptureError:
    """Keep transient capture evidence when RenderDoc fails before publication."""

    destination = output.parent / f"renderdoc-failure-{uuid.uuid4()}"
    try:
        shutil.copytree(work, destination)
    except OSError:
        return CaptureError(message)
    return CaptureError(f"{message}; retained RenderDoc diagnostics: {destination}")


def _tracked_paths(repo: Path) -> list[str]:
    output = _command_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=repo,
    )
    return sorted(filter(None, output.splitlines()))


def source_fingerprint(repo: Path) -> str:
    digest = hashlib.sha256()
    for relative in _tracked_paths(repo):
        source = repo / relative
        if not source.is_file():
            continue
        if relative in SOURCE_FILES or relative.startswith(SOURCE_PREFIXES):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif relative.startswith(ASSET_PREFIX):
            stats = source.stat()
            digest.update(b"asset-stat\0")
            digest.update(relative.encode())
            digest.update(f"\0{stats.st_size}\0{stats.st_mtime_ns}\0".encode())
    return digest.hexdigest()


def _assert_clean_source(repo: Path, commit: str, fingerprint: str) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise CaptureError("subject commit is not a full lowercase SHA")
    if _command_output(["git", "rev-parse", "HEAD"], cwd=repo) != commit:
        raise CaptureError("subject commit changed before RenderDoc capture")
    status = _command_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=repo
    )
    if status:
        raise CaptureError("formal RenderDoc subject is dirty: " + status.splitlines()[0])
    actual = source_fingerprint(repo)
    if actual != fingerprint:
        raise CaptureError(
            f"source fingerprint differs: expected {fingerprint}, observed {actual}"
        )


def _load_contract(repo: Path, contract_id: str, stage: str) -> dict[str, Any]:
    contract = read_json(repo / CONTRACT_FILE)
    if contract.get("contract_id") != contract_id or stage != "current":
        raise CaptureError("RenderDoc capture identity differs from rtt-light-v1/current")
    if contract.get("lifecycle") != {
        "status": "frozen",
        "formal_registration_allowed": True,
        "freeze_blockers": [],
    }:
        raise CaptureError("RtT-light contract is not frozen for formal capture")
    renderdoc = contract.get("formal_matrix", {}).get("renderdoc")
    if renderdoc != {
        "size": "medium",
        "render": "gpu",
        "settle_frames": 4,
        "capture_frame": 4,
        "repeat": 1,
    }:
        raise CaptureError("RenderDoc formal matrix differs from the implemented checkpoint")
    return contract


def _validate_environment_lock(
    lock: dict[str, Any],
    *,
    contract: dict[str, Any],
    commit: str,
    fingerprint: str,
    adapter_filter: str,
    window_backend: str,
    binary_sha256: str,
) -> None:
    expected_keys = {
        "schema_version",
        "contract_id",
        "stage_id",
        "subject_commit",
        "source_fingerprint",
        "host",
        "adapter",
        "resolved_window_backend",
        "adapter_backend",
        "requested_present_mode",
        "effective_present_mode",
        "window",
        "capture_binary_sha256",
    }
    if set(lock) != expected_keys or lock.get("schema_version") != 1:
        raise CaptureError("environment-lock.json differs from schema v1")
    matrix = contract["formal_matrix"]
    if (
        lock["contract_id"] != contract["contract_id"]
        or lock["stage_id"] != "current"
        or lock["subject_commit"] != commit
        or lock["source_fingerprint"] != fingerprint
        or lock["resolved_window_backend"] != window_backend
        or lock["adapter_backend"] != matrix["backend"]
        or lock["requested_present_mode"] != "auto_no_vsync"
        or lock["effective_present_mode"] not in {"immediate", "mailbox", "fifo"}
        or lock["capture_binary_sha256"] != binary_sha256
    ):
        raise CaptureError("environment lock identity or renderer tuple differs")
    adapter = lock["adapter"]
    if (
        not isinstance(adapter, dict)
        or set(adapter) != {"name", "driver", "driver_info", "backend"}
        or not isinstance(adapter.get("name"), str)
        or adapter_filter.casefold() not in adapter["name"].casefold()
        or str(adapter.get("backend", "")).casefold() != matrix["backend"]
    ):
        raise CaptureError("environment lock adapter differs from the selector")
    window = matrix["window"]
    expected_window = {
        "logical_width": f"{window['logical_width']:.6f}",
        "logical_height": f"{window['logical_height']:.6f}",
        "physical_width": str(window["physical_width"]),
        "physical_height": str(window["physical_height"]),
        "scale_factor": f"{window['scale_factor']:.6f}",
        "rtt_quality": window["rtt_quality"],
        "scene_target_width": str(window["scene_target_width"]),
        "scene_target_height": str(window["scene_target_height"]),
        "mask_target_width": str(window["scene_target_width"]),
        "mask_target_height": str(window["scene_target_height"]),
        "target_scale_factor": f"{window['scale_factor']:.6f}",
    }
    if lock["window"] != expected_window:
        raise CaptureError("environment lock window tuple differs from the formal matrix")


def _validate_capture_session(
    manifest: dict[str, Any], *, commit: str, fingerprint: str, binary_hash: str
) -> None:
    source = manifest.get("source", {})
    if (
        manifest.get("status") != "valid"
        or manifest.get("artifact_set_errors")
        or manifest.get("git", {}).get("commit") != commit
        or manifest.get("binary", {}).get("sha256") != binary_hash
        or source.get("fingerprint_start") != fingerprint
        or source.get("fingerprint_end") != fingerprint
        or source.get("unchanged") is not True
    ):
        raise CaptureError("Capture session provenance is not valid for RenderDoc reuse")


def unexpected_log_lines(text: str, allow_patterns: Iterable[str]) -> list[str]:
    compiled = [re.compile(pattern) for pattern in allow_patterns]
    return [
        line
        for line in text.splitlines()
        if LOG_PROBLEM_RE.search(line)
        and not any(pattern.search(line) for pattern in compiled)
    ]


def _validate_render_resources(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {
        "scene_target_label",
        "mask_target_label",
        "composite_draw_count",
        "composite_texture_bindings",
        "composite_sampler_bindings",
    }:
        raise CaptureError("runtime RenderDoc resources differ from schema v2")
    for label in (value["scene_target_label"], value["mask_target_label"]):
        if not isinstance(label, str) or not label:
            raise CaptureError("runtime RenderDoc target label is invalid")
    if (
        not isinstance(value["composite_draw_count"], int)
        or isinstance(value["composite_draw_count"], bool)
        or value["composite_draw_count"] != 1
    ):
        raise CaptureError("runtime RenderDoc composite draw count is invalid")
    for key, expected_keys in (
        (
            "composite_texture_bindings",
            {"target", "stage", "fixed_bind_set_or_space", "fixed_bind_number"},
        ),
        (
            "composite_sampler_bindings",
            {"stage", "fixed_bind_set_or_space", "fixed_bind_number"},
        ),
    ):
        rows = value[key]
        if (
            not isinstance(rows, list)
            or len(rows) != 2
            or any(not isinstance(row, dict) or set(row) != expected_keys for row in rows)
        ):
            raise CaptureError(f"runtime RenderDoc {key} differs from schema v2")
        for row in rows:
            if (
                not isinstance(row["stage"], str)
                or not row["stage"]
                or any(
                    not isinstance(row[field], int)
                    or isinstance(row[field], bool)
                    or row[field] < 0
                    for field in ("fixed_bind_set_or_space", "fixed_bind_number")
                )
            ):
                raise CaptureError(f"runtime RenderDoc {key} has an invalid binding")
            if key == "composite_texture_bindings" and (
                not isinstance(row["target"], str) or not row["target"]
            ):
                raise CaptureError("runtime RenderDoc texture target is invalid")
    if value != EXPECTED_RENDER_RESOURCES:
        raise CaptureError("runtime RenderDoc composite bindings differ from the current source")
    return value


def _runtime_checkpoint(
    path: Path, *, contract: dict[str, Any], capture_path: Path
) -> dict[str, Any]:
    value = read_json(path)
    if set(value) != {
        "schema_version",
        "status",
        "checkpoint",
        "render_inventory",
        "render_resources",
        "fixture",
        "capture_path",
        "renderdoc_api_version",
    } or value.get("schema_version") != RUNTIME_CHECKPOINT_SCHEMA_VERSION or value.get("status") != "valid":
        raise CaptureError("runtime RenderDoc checkpoint differs from schema v2")
    checkpoint = value["checkpoint"]
    renderdoc = contract["formal_matrix"]["renderdoc"]
    if (
        not isinstance(checkpoint, dict)
        or set(checkpoint)
        != {
            "name",
            "simulation_tick",
            "settle_frames",
            "capture_frame",
            "render_frame_index",
            "validated_frames",
        }
        or checkpoint["name"] != "indoor-light-fixture-ready-v1"
        or checkpoint["settle_frames"] != renderdoc["settle_frames"]
        or checkpoint["capture_frame"] != renderdoc["capture_frame"]
        or checkpoint["validated_frames"] != 1
        or not isinstance(checkpoint["simulation_tick"], int)
        or isinstance(checkpoint["simulation_tick"], bool)
        or checkpoint["simulation_tick"] < 0
        or not isinstance(checkpoint["render_frame_index"], int)
        or isinstance(checkpoint["render_frame_index"], bool)
        or checkpoint["render_frame_index"] < renderdoc["capture_frame"]
        or value["renderdoc_api_version"] != RENDERDOC_API_VERSION
    ):
        raise CaptureError("runtime RenderDoc fixed checkpoint differs from the contract")
    try:
        observed_capture = Path(value["capture_path"]).resolve()
    except TypeError as error:
        raise CaptureError("runtime checkpoint capture path is invalid") from error
    if observed_capture != capture_path.resolve():
        raise CaptureError("runtime checkpoint points at a different .rdc capture")
    for label in ("render_inventory", "render_resources", "fixture"):
        if not isinstance(value[label], dict) or not value[label]:
            raise CaptureError(f"runtime checkpoint {label} evidence is empty")
    _validate_render_resources(value["render_resources"])
    return value


def _locator(path: Path, *, root: Path) -> dict[str, Any]:
    return {
        "path": path.relative_to(root).as_posix(),
        "bytes": path.stat().st_size,
        "sha256": sha256(path),
    }


def _write_json_exclusive(path: Path, value: dict[str, Any]) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def _copy_regular(source: Path, destination: Path) -> None:
    if not source.is_file() or source.is_symlink() or source.stat().st_size <= 0:
        raise CaptureError(f"capture artifact is not a nonempty regular file: {source}")
    with source.open("rb") as reader, destination.open("xb") as writer:
        shutil.copyfileobj(reader, writer, length=1024 * 1024)
        writer.flush()
        os.fsync(writer.fileno())


def _validate_composite_topology(
    value: Any,
    *,
    render_resources: dict[str, Any],
    tracked_resources: dict[str, dict[str, str]],
    bindings: list[dict[str, Any]],
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {"draw_count", "draws"}:
        raise CaptureError("RenderDoc composite topology differs from schema v2")
    draws = value["draws"]
    if (
        not isinstance(value["draw_count"], int)
        or isinstance(value["draw_count"], bool)
        or value["draw_count"] != render_resources["composite_draw_count"]
        or not isinstance(draws, list)
        or len(draws) != value["draw_count"]
        or len(draws) != 1
    ):
        raise CaptureError("RenderDoc composite draw count differs from the current source")
    draw = draws[0]
    if not isinstance(draw, dict) or set(draw) != {
        "pass_id",
        "event_id",
        "texture_bindings",
        "sampler_bindings",
    }:
        raise CaptureError("RenderDoc composite draw differs from schema v2")
    if (
        not isinstance(draw["pass_id"], str)
        or not draw["pass_id"]
        or not isinstance(draw["event_id"], int)
        or isinstance(draw["event_id"], bool)
        or draw["event_id"] <= 0
    ):
        raise CaptureError("RenderDoc composite draw identity is invalid")
    expected_textures = [
        {
            **binding,
            "resource_id": tracked_resources[binding["target"]]["resource_id"],
        }
        for binding in render_resources["composite_texture_bindings"]
    ]
    textures = draw["texture_bindings"]
    if (
        not isinstance(textures, list)
        or textures != expected_textures
    ):
        raise CaptureError("RenderDoc composite texture bindings differ from the current source")
    sampler_keys = {
        "stage",
        "fixed_bind_set_or_space",
        "fixed_bind_number",
        "resource_id",
    }
    samplers = draw["sampler_bindings"]
    if (
        not isinstance(samplers, list)
        or len(samplers) != len(render_resources["composite_sampler_bindings"])
        or any(not isinstance(row, dict) or set(row) != sampler_keys for row in samplers)
        or any(
            not isinstance(row["resource_id"], str) or not row["resource_id"]
            for row in samplers
        )
        or [
            {
                key: row[key]
                for key in ("stage", "fixed_bind_set_or_space", "fixed_bind_number")
            }
            for row in samplers
        ]
        != render_resources["composite_sampler_bindings"]
    ):
        raise CaptureError("RenderDoc composite sampler bindings differ from the current source")
    raw_bindings = {
        (
            row["pass_id"],
            row["event_id"],
            row["category"],
            row["fixed_bind_set_or_space"],
            row["fixed_bind_number"],
            row["resource_id"],
        )
        for row in bindings
    }
    for texture in textures:
        if (
            draw["pass_id"],
            draw["event_id"],
            f"{texture['stage']}:read-only",
            texture["fixed_bind_set_or_space"],
            texture["fixed_bind_number"],
            texture["resource_id"],
        ) not in raw_bindings:
            raise CaptureError("RenderDoc composite texture is absent from raw bindings")
    for sampler in samplers:
        if (
            draw["pass_id"],
            draw["event_id"],
            f"{sampler['stage']}:sampler",
            sampler["fixed_bind_set_or_space"],
            sampler["fixed_bind_number"],
            sampler["resource_id"],
        ) not in raw_bindings:
            raise CaptureError("RenderDoc composite sampler is absent from raw bindings")
    return draw


def _validate_extraction(path: Path, *, capture_hash: str, runtime: dict[str, Any]) -> None:
    value = read_json(path)
    expected_keys = {
        "schema_version",
        "api",
        "capture_sha256",
        "validated_frames",
        "event_count",
        "draw_count",
        "passes",
        "attachments",
        "bindings",
        "tracked_resources",
        "composite_topology",
        "replay_structure",
    }
    if (
        set(value) != expected_keys
        or value.get("schema_version") != EXTRACTION_SCHEMA_VERSION
        or value.get("api") != "vulkan"
        or value.get("capture_sha256") != capture_hash
        or value.get("validated_frames") != 1
    ):
        raise CaptureError("RenderDoc replay extraction identity differs")
    for field in ("event_count", "draw_count"):
        observed = value.get(field)
        if not isinstance(observed, int) or isinstance(observed, bool) or observed <= 0:
            raise CaptureError(f"RenderDoc extraction {field} is invalid")
    row_schemas = {
        "passes": {"pass_id", "name", "first_event", "last_event", "draw_count"},
        "attachments": {
            "attachment_id",
            "pass_id",
            "event_id",
            "slot",
            "kind",
            "resource_id",
        },
        "bindings": {
            "binding_id",
            "pass_id",
            "event_id",
            "category",
            "fixed_bind_set_or_space",
            "fixed_bind_number",
            "resource_id",
        },
    }
    for field, schema in row_schemas.items():
        rows = value[field]
        if (
            not isinstance(rows, list)
            or not rows
            or any(not isinstance(row, dict) or set(row) != schema for row in rows)
        ):
            raise CaptureError(f"RenderDoc extraction {field} is empty or invalid")
    bindings = value["bindings"]
    for row in bindings:
        if (
            not isinstance(row["pass_id"], str)
            or not row["pass_id"]
            or not isinstance(row["event_id"], int)
            or isinstance(row["event_id"], bool)
            or row["event_id"] <= 0
            or not isinstance(row["category"], str)
            or ":" not in row["category"]
            or any(
                not isinstance(row[field], int)
                or isinstance(row[field], bool)
                or row[field] < 0
                for field in ("fixed_bind_set_or_space", "fixed_bind_number")
            )
            or not isinstance(row["resource_id"], str)
            or not row["resource_id"]
        ):
            raise CaptureError("RenderDoc binding row is invalid")
    tracked = value["tracked_resources"]
    if not isinstance(tracked, dict) or set(tracked) != {"scene_target", "mask_target"}:
        raise CaptureError("RenderDoc extraction tracked resources are invalid")
    render_resources = _validate_render_resources(runtime.get("render_resources"))
    expected_labels = {
        "scene_target": render_resources["scene_target_label"],
        "mask_target": render_resources["mask_target_label"],
    }
    for key, label in expected_labels.items():
        resource = tracked.get(key)
        if (
            not isinstance(resource, dict)
            or set(resource) != {"label", "resource_id"}
            or resource["label"] != label
            or not isinstance(resource["resource_id"], str)
            or not resource["resource_id"]
        ):
            raise CaptureError(f"RenderDoc replay did not resolve {key} by its runtime label")
    composite_draw = _validate_composite_topology(
        value["composite_topology"],
        render_resources=render_resources,
        tracked_resources=tracked,
        bindings=bindings,
    )
    structure = value["replay_structure"]
    expected_structure_keys = {
        "render_pass_count",
        "attachment_count",
        "binding_count",
        "composite_draw_count",
        "composite_texture_binding_count",
        "composite_sampler_binding_count",
        "scene_target_attachment_count",
        "scene_target_binding_count",
        "mask_target_attachment_count",
        "mask_target_binding_count",
    }
    if (
        not isinstance(structure, dict)
        or set(structure) != expected_structure_keys
        or any(
            not isinstance(number, int) or isinstance(number, bool) or number < 0
            for number in structure.values()
        )
        or structure["render_pass_count"] != len(value["passes"])
        or structure["attachment_count"] != len(value["attachments"])
        or structure["binding_count"] != len(bindings)
        or structure["render_pass_count"] < 2
        or structure["composite_draw_count"] != 1
        or structure["composite_texture_binding_count"] != 2
        or structure["composite_sampler_binding_count"] != 2
        or any(
            structure[f"{key}_{kind}_count"] < 1
            for key in ("scene_target", "mask_target")
            for kind in ("attachment", "binding")
        )
        or composite_draw["event_id"] <= 0
    ):
        raise CaptureError("RenderDoc replay structure does not prove current RtT topology")


def _capture_command(
    *,
    tools: dict[str, Any],
    repo: Path,
    binary: Path,
    runtime_dir: Path,
    capture_template: Path,
    contract: dict[str, Any],
) -> list[str]:
    matrix = contract["formal_matrix"]
    window = matrix["window"]
    return [
        str(tools["renderdoccmd"]),
        "capture",
        "--capture-file",
        str(capture_template),
        "--wait-for-exit",
        "--working-dir",
        str(repo),
        str(binary),
        "--perf-scenario",
        "--perf-workload",
        "indoor-light",
        "--perf-size",
        "medium",
        "--perf-render",
        "gpu",
        "--perf-clock",
        "fixed",
        "--perf-fixed-hz",
        str(matrix["fixed_hz"]),
        "--perf-warmup-ticks",
        str(matrix["audit"]["warmup_ticks"]),
        "--perf-audit-ticks",
        str(matrix["audit"]["audit_ticks"]),
        "--perf-seed",
        str(matrix["seed"]),
        "--perf-output-dir",
        str(runtime_dir),
        "--perf-contract",
        contract["contract_id"],
        "--perf-stage",
        "current",
        "--perf-lane",
        "static",
        "--perf-window-width",
        str(window["physical_width"]),
        "--perf-window-height",
        str(window["physical_height"]),
        "--perf-window-scale-factor",
        str(window["scale_factor"]),
        "--perf-rtt-quality",
        window["rtt_quality"],
        "--perf-renderdoc-capture",
    ]


def run_capture(args: argparse.Namespace) -> dict[str, Any]:
    repo = Path(args.repo).resolve()
    if not (repo / "Cargo.toml").is_file():
        raise CaptureError(f"not a repository root: {repo}")
    binary = _regular_file(args.binary, "Capture profiling binary", executable=True)
    output = Path(args.output).resolve()
    if output.exists() or output.name != "renderdoc" or not output.parent.is_dir():
        raise CaptureError(f"RenderDoc output must be a new attempt/renderdoc path: {output}")
    environment_lock_path = Path(args.environment_lock).resolve()
    if environment_lock_path.parent != output.parent.parent.parent:
        raise CaptureError("environment lock is outside the RenderDoc attempt generation")
    contract = _load_contract(repo, args.contract, args.stage)
    _assert_clean_source(repo, args.subject_commit, args.source_fingerprint)
    tools = inspect_tools(args.renderdoccmd, args.qrenderdoc, args.renderdoc_library)
    if (
        tools["renderdoc_version"] != args.renderdoc_version
        or tools["qrenderdoc_version"] != args.qrenderdoc_version
    ):
        raise CaptureError("RenderDoc tool versions changed after native planning")
    binary_hash = sha256(binary)
    environment_lock = read_json(environment_lock_path)
    _validate_environment_lock(
        environment_lock,
        contract=contract,
        commit=args.subject_commit,
        fingerprint=args.source_fingerprint,
        adapter_filter=args.adapter,
        window_backend=args.window_backend,
        binary_sha256=binary_hash,
    )
    capture_session = output.parent / "capture" / "manifest.json"
    _validate_capture_session(
        read_json(capture_session),
        commit=args.subject_commit,
        fingerprint=args.source_fingerprint,
        binary_hash=binary_hash,
    )

    with tempfile.TemporaryDirectory(prefix="hell-workers-renderdoc-") as temporary_name:
        work = Path(temporary_name)
        raw_dir = work / "raw"
        runtime_dir = work / "runtime"
        raw_dir.mkdir()
        runtime_dir.mkdir()
        vulkan_layer_directory = prepare_portable_vulkan_layer(tools, work)
        capture_template = raw_dir / "indoor-light"
        combined_log = work / "capture.log"
        environment = os.environ.copy()
        environment.update(
            {
                "BEVY_ASSET_ROOT": str(repo),
                "HW_PRESENT_MODE": contract["formal_matrix"]["present_mode"],
                "HW_WINDOW_BACKEND": args.window_backend,
                "WGPU_BACKEND": contract["formal_matrix"]["backend"],
                "WGPU_ADAPTER_NAME": args.adapter,
                "HW_RENDERDOC_LIBRARY": str(tools["library"]),
                "HW_RENDERDOC_CAPTURE_TEMPLATE": str(capture_template),
                "VK_ADD_IMPLICIT_LAYER_PATH": str(vulkan_layer_directory),
                "ENABLE_VULKAN_RENDERDOC_CAPTURE": "1",
                "RUST_BACKTRACE": "1",
            }
        )
        command = _capture_command(
            tools=tools,
            repo=repo,
            binary=binary,
            runtime_dir=runtime_dir,
            capture_template=capture_template,
            contract=contract,
        )
        with combined_log.open("xb") as log_handle:
            completed = subprocess.run(
                command,
                cwd=repo,
                env=environment,
                check=False,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                timeout=600,
            )
        if completed.returncode != 0:
            raise _retain_renderdoc_diagnostics(
                work,
                output,
                f"renderdoccmd capture failed with {completed.returncode}",
            )
        captures = [
            path
            for path in raw_dir.rglob("*.rdc")
            if path.is_file() and not path.is_symlink() and path.stat().st_size > 0
        ]
        if len(captures) != 1:
            raise _retain_renderdoc_diagnostics(
                work,
                output,
                f"RenderDoc produced {len(captures)} nonempty .rdc files",
            )
        capture = captures[0].resolve()
        checkpoint_path = runtime_dir / "renderdoc-checkpoint.json"
        runtime = _runtime_checkpoint(
            checkpoint_path, contract=contract, capture_path=capture
        )
        extraction_path = work / "extraction.json"
        replay_environment = environment.copy()
        replay_environment.update(
            {
                "HW_RENDERDOC_CAPTURE": str(capture),
                "HW_RENDERDOC_EXTRACTION": str(extraction_path),
                "HW_RENDERDOC_RUNTIME_CHECKPOINT": str(checkpoint_path),
            }
        )
        with combined_log.open("ab") as log_handle:
            replay = subprocess.run(
                [
                    str(tools["qrenderdoc"]),
                    "--python",
                    str(tools["extractor"]),
                    str(capture),
                ],
                cwd=repo,
                env=replay_environment,
                check=False,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                timeout=600,
            )
        if replay.returncode != 0 or not extraction_path.is_file():
            raise _retain_renderdoc_diagnostics(
                work,
                output,
                f"qrenderdoc extraction failed with {replay.returncode}",
            )
        capture_hash = sha256(capture)
        _validate_extraction(
            extraction_path, capture_hash=capture_hash, runtime=runtime
        )
        _assert_clean_source(repo, args.subject_commit, args.source_fingerprint)
        log_text = combined_log.read_text(encoding="utf-8", errors="replace")
        problems = unexpected_log_lines(
            log_text, contract["allow_log_patterns"]["windowed"]
        )
        if problems:
            raise CaptureError("unexpected RenderDoc log line: " + problems[0])

        staging = output.parent / f".renderdoc-{uuid.uuid4()}.tmp"
        if staging.exists():
            raise CaptureError(f"RenderDoc staging path already exists: {staging}")
        staging.mkdir(mode=0o755)
        try:
            (staging / "raw").mkdir()
            final_capture = staging / "raw" / capture.name
            final_extraction = staging / "extraction.json"
            final_checkpoint = staging / "runtime-checkpoint.json"
            final_log = staging / "capture.log"
            _copy_regular(capture, final_capture)
            _copy_regular(extraction_path, final_extraction)
            _copy_regular(checkpoint_path, final_checkpoint)
            _copy_regular(combined_log, final_log)
            manifest = {
                "schema_version": SCHEMA_VERSION,
                "status": "valid",
                "contract_id": contract["contract_id"],
                "stage_id": "current",
                "case_id": "renderdoc-medium-gpu",
                "size": "medium",
                "render": "gpu",
                "source": {
                    "commit": args.subject_commit,
                    "clean": True,
                    "fingerprint": args.source_fingerprint,
                },
                "binary": {"path": str(binary), "sha256": binary_hash},
                "tool": {
                    "path": str(tools["renderdoccmd"]),
                    "version": tools["renderdoc_version"],
                    "sha256": sha256(tools["renderdoccmd"]),
                },
                "replay_tool": {
                    "path": str(tools["qrenderdoc"]),
                    "version": tools["qrenderdoc_version"],
                    "sha256": sha256(tools["qrenderdoc"]),
                },
                "library": {
                    "path": str(tools["library"]),
                    "sha256": sha256(tools["library"]),
                    "api_version": RENDERDOC_API_VERSION,
                },
                "capture_helper": {
                    "path": str(Path(__file__).resolve()),
                    "sha256": sha256(Path(__file__).resolve()),
                },
                "extractor": {
                    "path": str(tools["extractor"]),
                    "sha256": sha256(tools["extractor"]),
                },
                "environment": {
                    "host": environment_lock["host"],
                    "adapter": environment_lock["adapter"],
                    "resolved_window_backend": environment_lock[
                        "resolved_window_backend"
                    ],
                    "adapter_backend": environment_lock["adapter_backend"],
                    "requested_present_mode": environment_lock[
                        "requested_present_mode"
                    ],
                    "effective_present_mode": environment_lock[
                        "effective_present_mode"
                    ],
                    "window": environment_lock["window"],
                },
                "checkpoint": runtime["checkpoint"],
                "capture": _locator(final_capture, root=staging),
                "extraction": _locator(final_extraction, root=staging),
                "runtime_checkpoint": _locator(final_checkpoint, root=staging),
                "log": _locator(final_log, root=staging),
                "fixture": runtime["fixture"],
                "unexpected_log_lines": 0,
            }
            _write_json_exclusive(staging / "manifest.json", manifest)
            os.replace(staging, output)
        except BaseException:
            shutil.rmtree(staging, ignore_errors=True)
            raise
    return read_json(output / "manifest.json")


def self_test() -> int:
    repo = Path(__file__).resolve().parents[2]
    sys.path.insert(0, str(repo / "scripts"))
    from perf_tool import execution as perf_execution

    if (
        SOURCE_FILES != perf_execution.SOURCE_FINGERPRINT_FILES
        or SOURCE_PREFIXES != perf_execution.SOURCE_FINGERPRINT_PREFIXES
        or ASSET_PREFIX != perf_execution.SOURCE_FINGERPRINT_ASSET_PREFIX
        or source_fingerprint(repo) != perf_execution.source_fingerprint()
    ):
        raise CaptureError("capture and perf source fingerprint boundaries differ")
    with tempfile.TemporaryDirectory(prefix="renderdoc-capture-self-test-") as name:
        root = Path(name)
        prefix = root / "portable" / "usr"
        renderdoccmd = prefix / "bin" / "renderdoccmd"
        qrenderdoc = prefix / "bin" / "qrenderdoc"
        library = prefix / "lib64" / "renderdoc" / "librenderdoc.so"
        manifest_source = prefix / RENDERDOC_VULKAN_MANIFEST
        renderdoccmd.parent.mkdir(parents=True)
        library.parent.mkdir(parents=True)
        manifest_source.parent.mkdir(parents=True)
        renderdoccmd.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = version ]; then echo 'RenderDoc v1.99'; exit 0; fi\n"
            "if [ \"$1\" = capture ] && [ \"$2\" = --help ]; then "
            "echo '--capture-file --wait-for-exit --working-dir'; exit 0; fi\n"
            "exit 1\n",
            encoding="utf-8",
        )
        qrenderdoc.write_text(
            "#!/bin/sh\n"
            "if [ \"$1\" = --version ]; then echo 'QRenderDoc v1.99'; exit 0; fi\n"
            "if [ \"$1\" = --help ]; then echo '--python'; exit 0; fi\n"
            "exit 1\n",
            encoding="utf-8",
        )
        renderdoccmd.chmod(0o755)
        qrenderdoc.chmod(0o755)
        library.write_bytes(b"\x7fELFformal-probe")
        manifest_source.write_text(
            json.dumps(
                {
                    "file_format_version": "1.1.2",
                    "layer": {
                        "name": "VK_LAYER_RENDERDOC_Capture",
                        "type": "GLOBAL",
                        "library_path": "/usr/lib64/renderdoc/librenderdoc.so",
                    },
                }
            ),
            encoding="utf-8",
        )
        tools = inspect_tools(str(renderdoccmd), str(qrenderdoc), str(library))
        if (
            tools["renderdoc_version"] != "RenderDoc v1.99"
            or tools["qrenderdoc_version"] != "QRenderDoc v1.99"
        ):
            raise CaptureError("tool probe versions differ in self-test")
        layer_directory = prepare_portable_vulkan_layer(tools, root / "layer")
        rewritten_manifest = read_json(layer_directory / "renderdoc_capture.json")
        if rewritten_manifest["layer"]["library_path"] != str(library):
            raise CaptureError("portable Vulkan manifest did not use inspected library")
        failure_work = root / "failure-work"
        failure_work.mkdir()
        (failure_work / "capture.log").write_text("capture failure\n", encoding="utf-8")
        failure_output = root / "attempt" / "renderdoc"
        failure_output.parent.mkdir()
        failure = _retain_renderdoc_diagnostics(
            failure_work, failure_output, "expected self-test failure"
        )
        retained = sorted(failure_output.parent.glob("renderdoc-failure-*"))
        if len(retained) != 1 or not (retained[0] / "capture.log").is_file():
            raise CaptureError("RenderDoc failure diagnostics were not retained")
        if str(retained[0]) not in str(failure):
            raise CaptureError("RenderDoc failure diagnostics path was not reported")
        capture = root / "capture.rdc"
        capture.write_bytes(b"RenderDoc self-test capture")
        capture_hash = sha256(capture)
        bindings = [
            {
                "binding_id": "binding-0000001",
                "pass_id": "pass-0002",
                "event_id": 4,
                "category": "fragment:read-only",
                "fixed_bind_set_or_space": 2,
                "fixed_bind_number": 1,
                "resource_id": "ResourceId::21",
            },
            {
                "binding_id": "binding-0000002",
                "pass_id": "pass-0002",
                "event_id": 4,
                "category": "fragment:read-only",
                "fixed_bind_set_or_space": 2,
                "fixed_bind_number": 3,
                "resource_id": "ResourceId::22",
            },
            {
                "binding_id": "binding-0000003",
                "pass_id": "pass-0002",
                "event_id": 4,
                "category": "fragment:sampler",
                "fixed_bind_set_or_space": 2,
                "fixed_bind_number": 2,
                "resource_id": "ResourceId::11",
            },
            {
                "binding_id": "binding-0000004",
                "pass_id": "pass-0002",
                "event_id": 4,
                "category": "fragment:sampler",
                "fixed_bind_set_or_space": 2,
                "fixed_bind_number": 4,
                "resource_id": "ResourceId::11",
            },
        ]
        tracked_resources = {
            "scene_target": {
                "label": "hell-workers-rtt-scene",
                "resource_id": "ResourceId::21",
            },
            "mask_target": {
                "label": "hell-workers-rtt-soul-mask",
                "resource_id": "ResourceId::22",
            },
        }
        extraction = {
            "schema_version": EXTRACTION_SCHEMA_VERSION,
            "api": "vulkan",
            "capture_sha256": capture_hash,
            "validated_frames": 1,
            "event_count": 5,
            "draw_count": 2,
            "passes": [
                {
                    "pass_id": "pass-0001",
                    "name": "scene",
                    "first_event": 2,
                    "last_event": 2,
                    "draw_count": 1,
                },
                {
                    "pass_id": "pass-0002",
                    "name": "composite",
                    "first_event": 4,
                    "last_event": 4,
                    "draw_count": 1,
                },
            ],
            "attachments": [
                {
                    "attachment_id": "attachment-000001",
                    "pass_id": "pass-0001",
                    "event_id": 2,
                    "slot": 0,
                    "kind": "color",
                    "resource_id": "ResourceId::21",
                },
                {
                    "attachment_id": "attachment-000002",
                    "pass_id": "pass-0002",
                    "event_id": 4,
                    "slot": 0,
                    "kind": "color",
                    "resource_id": "ResourceId::22",
                },
            ],
            "bindings": bindings,
            "tracked_resources": tracked_resources,
            "composite_topology": {
                "draw_count": 1,
                "draws": [
                    {
                        "pass_id": "pass-0002",
                        "event_id": 4,
                        "texture_bindings": [
                            {
                                "target": "scene_target",
                                "stage": "fragment",
                                "fixed_bind_set_or_space": 2,
                                "fixed_bind_number": 1,
                                "resource_id": "ResourceId::21",
                            },
                            {
                                "target": "mask_target",
                                "stage": "fragment",
                                "fixed_bind_set_or_space": 2,
                                "fixed_bind_number": 3,
                                "resource_id": "ResourceId::22",
                            },
                        ],
                        "sampler_bindings": [
                            {
                                "stage": "fragment",
                                "fixed_bind_set_or_space": 2,
                                "fixed_bind_number": 2,
                                "resource_id": "ResourceId::11",
                            },
                            {
                                "stage": "fragment",
                                "fixed_bind_set_or_space": 2,
                                "fixed_bind_number": 4,
                                "resource_id": "ResourceId::11",
                            },
                        ],
                    }
                ],
            },
            "replay_structure": {
                "render_pass_count": 2,
                "attachment_count": 2,
                "binding_count": 4,
                "composite_draw_count": 1,
                "composite_texture_binding_count": 2,
                "composite_sampler_binding_count": 2,
                "scene_target_attachment_count": 1,
                "scene_target_binding_count": 1,
                "mask_target_attachment_count": 1,
                "mask_target_binding_count": 1,
            },
        }
        extraction_path = root / "extraction.json"
        extraction_path.write_text(json.dumps(extraction), encoding="utf-8")
        _validate_extraction(
            extraction_path,
            capture_hash=capture_hash,
            runtime={"render_resources": EXPECTED_RENDER_RESOURCES},
        )
        invalid_extraction = json.loads(json.dumps(extraction))
        invalid_extraction["composite_topology"]["draws"][0]["sampler_bindings"][1][
            "fixed_bind_set_or_space"
        ] = 1
        invalid_path = root / "invalid-extraction.json"
        invalid_path.write_text(json.dumps(invalid_extraction), encoding="utf-8")
        try:
            _validate_extraction(
                invalid_path,
                capture_hash=capture_hash,
                runtime={"render_resources": EXPECTED_RENDER_RESOURCES},
            )
        except CaptureError:
            pass
        else:
            raise CaptureError("RenderDoc set identity regression was accepted")
    if unexpected_log_lines("INFO ready\nERROR broken\n", []) != ["ERROR broken"]:
        raise CaptureError("RenderDoc log classifier did not fail closed")
    if unexpected_log_lines("WARNING allowed\n", [r"allowed$"]):
        raise CaptureError("RenderDoc log allowlist did not match exactly")
    print("renderdoc_capture self-test: PASS")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    probe = subparsers.add_parser("probe")
    for target in (probe,):
        target.add_argument("--renderdoccmd", required=True)
        target.add_argument("--qrenderdoc", required=True)
        target.add_argument("--renderdoc-library", required=True)
    capture = subparsers.add_parser("capture")
    capture.add_argument("--repo", required=True)
    capture.add_argument("--binary", required=True)
    capture.add_argument("--output", required=True)
    capture.add_argument("--environment-lock", required=True)
    capture.add_argument("--contract", required=True)
    capture.add_argument("--stage", required=True)
    capture.add_argument("--adapter", required=True)
    capture.add_argument("--window-backend", choices=("x11", "wayland"), required=True)
    capture.add_argument("--subject-commit", required=True)
    capture.add_argument("--source-fingerprint", required=True)
    capture.add_argument("--renderdoccmd", required=True)
    capture.add_argument("--qrenderdoc", required=True)
    capture.add_argument("--renderdoc-library", required=True)
    capture.add_argument("--renderdoc-version", required=True)
    capture.add_argument("--qrenderdoc-version", required=True)
    subparsers.add_parser("self-test")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.command == "probe":
        tools = inspect_tools(
            args.renderdoccmd, args.qrenderdoc, args.renderdoc_library
        )
        print(
            json.dumps(
                {
                    "status": "ready",
                    "renderdoc_version": tools["renderdoc_version"],
                    "qrenderdoc_version": tools["qrenderdoc_version"],
                    "renderdoccmd_sha256": sha256(tools["renderdoccmd"]),
                    "qrenderdoc_sha256": sha256(tools["qrenderdoc"]),
                    "librenderdoc_sha256": sha256(tools["library"]),
                    "renderdoc_api_version": RENDERDOC_API_VERSION,
                    "extractor_sha256": sha256(tools["extractor"]),
                },
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    if args.command == "self-test":
        return self_test()
    manifest = run_capture(args)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"RenderDoc capture failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
