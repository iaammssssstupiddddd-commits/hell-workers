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


SCRIPTS_ROOT = Path(__file__).resolve().parents[1]
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

from cargo_runtime import persistent_storage_error
from perf_tool.renderdoc_foundation import (
    CAPTURE_CHILD_DEADLINE_SECONDS,
    classify_renderdoc_log_lines,
    assert_disk_headroom,
    disk_reservation_bytes,
    run_with_deadline,
    validate_runtime_checkpoint_v3,
    verify_capsule_hash,
    DIAGNOSTIC_NAMESPACE,
)


SCHEMA_VERSION = 1
RUNTIME_CHECKPOINT_SCHEMA_VERSION = 3
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
MEASUREMENT_HARNESS_FILES = (
    ".codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py",
    "scripts/build_coordination.py",
    "scripts/cargo_runtime.py",
    "scripts/perf_tool/execution.py",
    "scripts/perf_tool/renderdoc_capture.py",
    "scripts/perf_tool/renderdoc_foundation.py",
    "scripts/perf_tool/rtt_light_bundle.py",
)
RENDERDOC_API_VERSION = "1.6.0"

CURRENT_RENDER_RESOURCES = {
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

P01_RENDER_RESOURCES = {
    "scene_target_label": "hell-workers-rtt-scene",
    "mask_target_label": None,
    "composite_draw_count": 1,
    "composite_texture_bindings": [
        {
            "target": "scene_target",
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 1,
        },
    ],
    "composite_sampler_bindings": [
        {
            "stage": "fragment",
            "fixed_bind_set_or_space": 2,
            "fixed_bind_number": 2,
        },
    ],
}

EXPECTED_RENDER_RESOURCES_BY_STAGE = {
    "current": CURRENT_RENDER_RESOURCES,
    "p01": P01_RENDER_RESOURCES,
    "p02": P01_RENDER_RESOURCES,
}


class CaptureError(RuntimeError):
    """A formal capture prerequisite or evidence validation failed."""


def workspace_temp_dir(repo: Path) -> Path:
    return (repo / DIAGNOSTIC_NAMESPACE).resolve()


class RetainedFailureDirectory(tempfile.TemporaryDirectory):
    """Delete successful scratch space, but retain partial failure evidence."""

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> bool | None:
        if exc_type is None:
            return super().__exit__(exc_type, exc, traceback)
        self._finalizer.detach()
        failure = Path(self.name) / "failure.json"
        try:
            failure.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "status": "diagnostic_only",
                        "reason": str(exc),
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
        except OSError:
            pass
        print(f"RenderDoc failure evidence retained at {self.name}", file=sys.stderr)
        return False


def require_within(path: Path, root: Path, *, label: str) -> None:
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise CaptureError(f"{label} must be under {root}: {path}") from error


def require_persistent_storage(path: Path, *, label: str) -> None:
    storage_error = persistent_storage_error(path, label=label)
    if storage_error:
        raise CaptureError(storage_error)


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


def normalized_json_digest(path: Path) -> str:
    payload = read_json(path)

    def normalize(value: Any) -> Any:
        if isinstance(value, dict):
            return {
                key: normalize(child)
                for key, child in value.items()
                if key
                not in {
                    "capture_sha256",
                    "resource_id",
                    "binding_id",
                    "attachment_id",
                }
            }
        if isinstance(value, list):
            return [normalize(child) for child in value]
        return value

    encoded = json.dumps(
        normalize(payload), sort_keys=True, separators=(",", ":")
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


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


def _probe_environment(*, headless_qt: bool) -> dict[str, str]:
    environment = os.environ.copy()
    if headless_qt:
        environment["QT_QPA_PLATFORM"] = "offscreen"
    return environment


def _tool_version(path: Path, *, headless_qt: bool = False) -> str:
    failures: list[str] = []
    argument_options = (
        (("--version",),) if headless_qt else (("version",), ("--version",))
    )
    for arguments in argument_options:
        try:
            completed = subprocess.run(
                [str(path), *arguments],
                check=False,
                capture_output=True,
                text=True,
                timeout=30,
                env=_probe_environment(headless_qt=headless_qt),
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


def _help_text(
    path: Path, arguments: tuple[str, ...], *, headless_qt: bool = False
) -> str:
    completed = subprocess.run(
        [str(path), *arguments],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env=_probe_environment(headless_qt=headless_qt),
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
    qrenderdoc_version = _tool_version(qrenderdoc, headless_qt=True)
    if _version_tuple(renderdoc_version) != _version_tuple(qrenderdoc_version):
        raise CaptureError(
            "renderdoccmd and qrenderdoc major/minor versions differ: "
            f"{renderdoc_version!r} != {qrenderdoc_version!r}"
        )
    capture_help = _help_text(renderdoccmd, ("capture", "--help"))
    for option in ("--capture-file", "--wait-for-exit", "--working-dir"):
        if option not in capture_help:
            raise CaptureError(f"renderdoccmd capture does not advertise {option}")
    if "--python" not in _help_text(qrenderdoc, ("--help",), headless_qt=True):
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
        subject_bytes: bytes | None = None
        if relative in MEASUREMENT_HARNESS_FILES:
            completed = subprocess.run(
                ["git", "show", f"HEAD:{relative}"],
                cwd=repo,
                check=False,
                capture_output=True,
            )
            if completed.returncode != 0:
                continue
            subject_bytes = completed.stdout
        if relative in SOURCE_FILES or relative.startswith(SOURCE_PREFIXES):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            if subject_bytes is not None:
                digest.update(subject_bytes)
            else:
                with source.open("rb") as handle:
                    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                        digest.update(chunk)
        elif relative.startswith(ASSET_PREFIX):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
    return digest.hexdigest()


def _assert_clean_source(repo: Path, commit: str, fingerprint: str) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise CaptureError("subject commit is not a full lowercase SHA")
    if _command_output(["git", "rev-parse", "HEAD"], cwd=repo) != commit:
        raise CaptureError("subject commit changed before RenderDoc capture")
    status = _command_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"], cwd=repo
    )
    dirty: list[str] = []
    for entry in status.splitlines() if status else []:
        fields = entry.split(maxsplit=1)
        path = fields[1] if len(fields) == 2 else entry
        paths = path.split(" -> ", 1) if " -> " in path else [path]
        if paths and all(candidate in MEASUREMENT_HARNESS_FILES for candidate in paths):
            continue
        dirty.append(entry)
    if dirty:
        raise CaptureError("formal RenderDoc subject is dirty: " + dirty[0])
    actual = source_fingerprint(repo)
    if actual != fingerprint:
        raise CaptureError(
            f"source fingerprint differs: expected {fingerprint}, observed {actual}"
        )


def _load_contract(repo: Path, contract_id: str, stage: str) -> dict[str, Any]:
    contract = read_json(repo / CONTRACT_FILE)
    if (
        contract.get("contract_id") != contract_id
        or stage not in EXPECTED_RENDER_RESOURCES_BY_STAGE
    ):
        raise CaptureError("RenderDoc capture identity differs from rtt-light-v1/current|p01|p02")
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
    stage: str,
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
        "renderdoc_binary_sha256",
        "memory_binary_sha256",
    }
    if set(lock) != expected_keys or lock.get("schema_version") != 2:
        raise CaptureError("environment-lock.json differs from schema v2")
    matrix = contract["formal_matrix"]
    if (
        lock["contract_id"] != contract["contract_id"]
        or lock["stage_id"] != stage
        or lock["subject_commit"] != commit
        or lock["source_fingerprint"] != fingerprint
        or lock["resolved_window_backend"] != window_backend
        or lock["adapter_backend"] != matrix["backend"]
        or lock["requested_present_mode"] != "auto_no_vsync"
        or lock["effective_present_mode"] not in {"immediate", "mailbox", "fifo"}
        or lock["renderdoc_binary_sha256"] != binary_sha256
        or not isinstance(lock["capture_binary_sha256"], str)
        or re.fullmatch(r"[0-9a-f]{64}", lock["capture_binary_sha256"]) is None
        or lock["capture_binary_sha256"] == binary_sha256
        or lock["memory_binary_sha256"] is not None
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
        "mask_target_width": str(window["scene_target_width"] if stage == "current" else 0),
        "mask_target_height": str(window["scene_target_height"] if stage == "current" else 0),
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
    return classify_renderdoc_log_lines(text, allow_patterns)[1]


def _validate_render_resources(value: Any, *, stage_id: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {
        "scene_target_label",
        "mask_target_label",
        "composite_draw_count",
        "composite_texture_bindings",
        "composite_sampler_bindings",
    }:
        raise CaptureError("runtime RenderDoc resources differ from schema v2")
    for key in ("scene_target_label", "mask_target_label"):
        label = value[key]
        if key == "mask_target_label" and label is None:
            continue
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
            or not rows
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
    if value != EXPECTED_RENDER_RESOURCES_BY_STAGE.get(stage_id):
        raise CaptureError(
            f"runtime RenderDoc composite bindings differ from the {stage_id} source contract"
        )
    return value


def _runtime_checkpoint(
    path: Path, *, contract: dict[str, Any], stage: str, capture_path: Path
) -> dict[str, Any]:
    value = read_json(path)
    try:
        validate_runtime_checkpoint_v3(
            value,
            contract=contract,
            stage_id=stage,
            capture_path=capture_path,
            rdc_sha256=sha256(capture_path),
            rdc_bytes=capture_path.stat().st_size,
        )
    except (OSError, ValueError) as error:
        raise CaptureError(f"runtime RenderDoc checkpoint differs from schema v3: {error}") from error
    labels = ["render_inventory", "render_resources", "fixture"]
    if stage == "p02":
        labels.append("p02_presentation")
    for label in labels:
        if not isinstance(value[label], dict) or not value[label]:
            raise CaptureError(f"runtime checkpoint {label} evidence is empty")
    _validate_render_resources(value["render_resources"], stage_id=stage)
    return value


def _manifest_stage_id(*, requested_stage: str, runtime: dict[str, Any]) -> str:
    runtime_stage = runtime.get("stage_id")
    if runtime_stage != requested_stage:
        raise CaptureError(
            "RenderDoc manifest stage differs from the validated runtime checkpoint"
        )
    return requested_stage


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


def _record_process_status(path: Path, *, stage: str, status: Any) -> None:
    payload = read_json(path) if path.exists() else {"schema_version": 1, "processes": []}
    payload["processes"].append(
        {
            "stage": stage,
            "pid": status.pid,
            "pgid": status.pgid,
            "returncode": status.returncode,
            "signal": status.signal,
            "deadline_reason": status.deadline_reason,
            "orphan_probe_count": status.orphan_probe_count,
        }
    )
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _record_disk_status(path: Path, *, stage: str, filesystem: Path, required: int) -> None:
    payload = read_json(path) if path.exists() else {"schema_version": 1, "checks": []}
    payload["checks"].append(
        {
            "stage": stage,
            "filesystem": str(filesystem.resolve()),
            "required_bytes": required,
            "free_bytes": shutil.disk_usage(filesystem).free,
        }
    )
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


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
    render_resources = _validate_render_resources(
        runtime.get("render_resources"), stage_id=runtime.get("stage_id")
    )
    expected_labels = {
        key.removesuffix("_label"): label
        for key, label in render_resources.items()
        if key.endswith("_target_label") and label is not None
    }
    if not isinstance(tracked, dict) or set(tracked) != set(expected_labels):
        raise CaptureError("RenderDoc extraction tracked resources are invalid")
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
    }
    expected_structure_keys.update(
        f"{key}_{kind}_count"
        for key in expected_labels
        for kind in ("attachment", "binding")
    )
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
        or structure["composite_texture_binding_count"]
        != len(render_resources["composite_texture_bindings"])
        or structure["composite_sampler_binding_count"]
        != len(render_resources["composite_sampler_bindings"])
        or any(
            structure[f"{key}_{kind}_count"] < 1
            for key in expected_labels
            for kind in ("attachment", "binding")
        )
        or composite_draw["event_id"] <= 0
    ):
        raise CaptureError("RenderDoc replay structure does not prove stage RtT topology")


def _capture_command(
    *,
    tools: dict[str, Any],
    repo: Path,
    binary: Path,
    runtime_dir: Path,
    capture_template: Path,
    contract: dict[str, Any],
    stage: str,
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
        stage,
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
    binary = _regular_file(args.binary, "RenderDoc profiling binary", executable=True)
    require_within(binary, repo / "target", label="RenderDoc profiling binary")
    require_persistent_storage(binary, label="RenderDoc profiling binary")
    capsule_manifest_path = Path(args.capsule_manifest).resolve()
    if capsule_manifest_path != binary.parent / "capsule-manifest.json":
        raise CaptureError("RenderDoc binary and capsule manifest are not co-located")
    try:
        capsule = verify_capsule_hash(binary.parent)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        raise CaptureError(f"RenderDoc binary capsule is invalid: {error}") from error
    if (
        capsule.leg != "renderdoc"
        or capsule.profile != "profiling-renderdoc"
        or capsule.features != "profiling-renderdoc"
        or capsule.binary_sha256 != sha256(binary)
    ):
        raise CaptureError("RenderDoc binary capsule identity differs from the formal recipe")
    output = Path(args.output).resolve()
    output_root = (
        repo / "target" / "perf-runs"
        if args.mode == "formal"
        else repo / "target" / "native-acceptance" / "renderdoc-foundation"
    )
    require_within(output, output_root, label=f"RenderDoc {args.mode} output")
    require_persistent_storage(output, label="RenderDoc output")
    if output.exists() or output.name != "renderdoc" or not output.parent.is_dir():
        raise CaptureError(f"RenderDoc output must be a new attempt/renderdoc path: {output}")
    environment_lock_path = Path(args.environment_lock).resolve()
    if args.mode == "formal" and environment_lock_path.parent != output.parent.parent.parent:
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
        stage=args.stage,
    )
    capture_session = (
        Path(args.capture_session).resolve()
        if args.capture_session
        else output.parent / "capture" / "manifest.json"
    )
    require_within(capture_session, repo / "target", label="Capture session manifest")
    _validate_capture_session(
        read_json(capture_session),
        commit=args.subject_commit,
        fingerprint=args.source_fingerprint,
        binary_hash=environment_lock["capture_binary_sha256"],
    )

    temporary_root = workspace_temp_dir(repo)
    require_persistent_storage(temporary_root, label="RenderDoc temporary directory")
    temporary_root.mkdir(parents=True, exist_ok=True)
    with RetainedFailureDirectory(
        prefix=f"{uuid.uuid4()}-",
        dir=temporary_root,
    ) as temporary_name:
        work = Path(temporary_name)
        process_status_path = work / "process-status.json"
        disk_status_path = work / "disk-status.json"
        initial_reservation = disk_reservation_bytes(max_rdc_bytes=0, rd0_rdc_bytes=0)
        _record_disk_status(
            disk_status_path,
            stage="before-capture",
            filesystem=temporary_root,
            required=initial_reservation,
        )
        assert_disk_headroom(temporary_root, required_bytes=initial_reservation)
        raw_dir = work / "raw"
        runtime_dir = work / "runtime"
        raw_dir.mkdir()
        runtime_dir.mkdir()
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
                "RUST_BACKTRACE": "1",
                "TMPDIR": str(work),
                "TMP": str(work),
                "TEMP": str(work),
            }
        )
        command = _capture_command(
            tools=tools,
            repo=repo,
            binary=binary,
            runtime_dir=runtime_dir,
            capture_template=capture_template,
            contract=contract,
            stage=args.stage,
        )
        with combined_log.open("xb") as log_handle:
            completed = run_with_deadline(
                command,
                cwd=repo,
                env=environment,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                deadline_seconds=CAPTURE_CHILD_DEADLINE_SECONDS,
                deadline_reason="renderdoc_capture_deadline",
            )
        _record_process_status(
            process_status_path, stage="capture", status=completed
        )
        if completed.deadline_reason is not None or completed.orphan_probe_count != 0:
            raise CaptureError(
                "renderdoccmd capture exceeded its deadline or left a process group"
            )
        if completed.returncode != 0:
            raise CaptureError(f"renderdoccmd capture failed with {completed.returncode}")
        captures = [
            path
            for path in raw_dir.rglob("*.rdc")
            if path.is_file() and not path.is_symlink() and path.stat().st_size > 0
        ]
        if len(captures) != 1:
            raise CaptureError(f"RenderDoc produced {len(captures)} nonempty .rdc files")
        capture = captures[0].resolve()
        checkpoint_path = runtime_dir / "renderdoc-checkpoint.json"
        runtime = _runtime_checkpoint(
            checkpoint_path, contract=contract, stage=args.stage, capture_path=capture
        )
        extraction_path = work / "extraction.json"
        extraction_failure_path = work / "extraction-failure.json"
        replay_environment = environment.copy()
        replay_environment.update(
            {
                "HW_RENDERDOC_CAPTURE": str(capture),
                "HW_RENDERDOC_EXTRACTION": str(extraction_path),
                "HW_RENDERDOC_EXTRACTION_FAILURE": str(extraction_failure_path),
                "HW_RENDERDOC_RUNTIME_CHECKPOINT": str(checkpoint_path),
            }
        )
        with combined_log.open("ab") as log_handle:
            replay = run_with_deadline(
                [
                    str(tools["qrenderdoc"]),
                    "--python",
                    str(tools["extractor"]),
                    str(capture),
                ],
                cwd=repo,
                env=replay_environment,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                deadline_seconds=CAPTURE_CHILD_DEADLINE_SECONDS,
                deadline_reason="renderdoc_replay_deadline",
            )
        _record_process_status(process_status_path, stage="replay-1", status=replay)
        if replay.deadline_reason is not None or replay.orphan_probe_count != 0:
            raise CaptureError(
                "qrenderdoc replay exceeded its deadline or left a process group"
            )
        if replay.returncode != 0 or not extraction_path.is_file():
            failure_detail = ""
            if extraction_failure_path.is_file():
                failure = read_json(extraction_failure_path)
                if isinstance(failure.get("error"), str):
                    failure_detail = f": {failure['error']}"
            raise CaptureError(
                "qrenderdoc extraction did not produce JSON "
                f"(process exit {replay.returncode}){failure_detail}"
            )
        capture_hash = sha256(capture)
        _validate_extraction(
            extraction_path, capture_hash=capture_hash, runtime=runtime
        )
        replay_check_path = work / "extraction-replay.json"
        replay_check_failure_path = work / "extraction-replay-failure.json"
        replay_check_environment = replay_environment.copy()
        replay_check_environment["HW_RENDERDOC_EXTRACTION"] = str(replay_check_path)
        replay_check_environment["HW_RENDERDOC_EXTRACTION_FAILURE"] = str(
            replay_check_failure_path
        )
        with combined_log.open("ab") as log_handle:
            replay_check = run_with_deadline(
                [
                    str(tools["qrenderdoc"]),
                    "--python",
                    str(tools["extractor"]),
                    str(capture),
                ],
                cwd=repo,
                env=replay_check_environment,
                stdout=log_handle,
                stderr=subprocess.STDOUT,
                deadline_seconds=CAPTURE_CHILD_DEADLINE_SECONDS,
                deadline_reason="renderdoc_second_replay_deadline",
            )
        _record_process_status(
            process_status_path, stage="replay-2", status=replay_check
        )
        if (
            replay_check.deadline_reason is not None
            or replay_check.orphan_probe_count != 0
            or replay_check.returncode != 0
            or not replay_check_path.is_file()
        ):
            raise CaptureError("second qrenderdoc replay failed or left a process group")
        _validate_extraction(
            replay_check_path, capture_hash=capture_hash, runtime=runtime
        )
        replay_digest = normalized_json_digest(extraction_path)
        if normalized_json_digest(replay_check_path) != replay_digest:
            raise CaptureError("two local replays produced different normalized evidence")
        _assert_clean_source(repo, args.subject_commit, args.source_fingerprint)
        log_text = combined_log.read_text(encoding="utf-8", errors="replace")
        problems = unexpected_log_lines(
            log_text, contract["allow_log_patterns"]["windowed"]
        )
        if problems:
            raise CaptureError("unexpected RenderDoc log line: " + problems[0])

        staging = output.parent / f".renderdoc-{uuid.uuid4()}.tmp"
        copy_reservation = disk_reservation_bytes(
            max_rdc_bytes=capture.stat().st_size,
            rd0_rdc_bytes=capture.stat().st_size if args.mode == "rd0" else 0,
        )
        _record_disk_status(
            disk_status_path,
            stage="before-staging-copy",
            filesystem=output.parent,
            required=copy_reservation,
        )
        assert_disk_headroom(output.parent, required_bytes=copy_reservation)
        if staging.exists():
            raise CaptureError(f"RenderDoc staging path already exists: {staging}")
        staging.mkdir(mode=0o755)
        try:
            (staging / "raw").mkdir()
            final_capture = staging / "raw" / capture.name
            final_extraction = staging / "extraction.json"
            final_checkpoint = staging / "runtime-checkpoint.json"
            final_log = staging / "capture.log"
            final_capsule_manifest = staging / "capsule-manifest.json"
            final_process_status = staging / "process-status.json"
            final_disk_status = staging / "disk-status.json"
            _copy_regular(capture, final_capture)
            _copy_regular(extraction_path, final_extraction)
            _copy_regular(checkpoint_path, final_checkpoint)
            _copy_regular(combined_log, final_log)
            _copy_regular(capsule_manifest_path, final_capsule_manifest)
            _copy_regular(process_status_path, final_process_status)
            _copy_regular(disk_status_path, final_disk_status)
            manifest = {
                "schema_version": SCHEMA_VERSION,
                "status": "valid",
                "contract_id": contract["contract_id"],
                "stage_id": _manifest_stage_id(
                    requested_stage=args.stage, runtime=runtime
                ),
                "case_id": "renderdoc-medium-gpu",
                "size": "medium",
                "render": "gpu",
                "source": {
                    "commit": args.subject_commit,
                    "clean": True,
                    "fingerprint": args.source_fingerprint,
                },
                "binary": {"path": str(binary), "sha256": binary_hash},
                "capsule": {
                    "capsule_id": capsule.capsule_id,
                    "logical_locator": capsule.logical_locator,
                    "manifest_sha256": sha256(capsule_manifest_path),
                    "build_fingerprint": capsule.build_fingerprint,
                    "profile": capsule.profile,
                    "features": capsule.features,
                    "manifest": _locator(final_capsule_manifest, root=staging),
                },
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
                "process_status": _locator(final_process_status, root=staging),
                "disk_status": _locator(final_disk_status, root=staging),
                "fixture": runtime["fixture"],
                "unexpected_log_lines": 0,
                "replay_digest": replay_digest,
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
    temporary_root = workspace_temp_dir(repo)
    temporary_root.mkdir(parents=True, exist_ok=True)
    try:
        require_within(
            Path("/tmp/hell-workers-renderdoc-self-test"),
            repo / "target" / "perf-runs",
            label="RenderDoc output",
        )
    except CaptureError:
        pass
    else:
        raise CaptureError("RenderDoc output outside target/perf-runs was accepted")
    try:
        require_persistent_storage(
            Path("/tmp/hell-workers-renderdoc-self-test"),
            label="RenderDoc temporary directory",
        )
    except CaptureError:
        pass
    else:
        raise CaptureError("RenderDoc temporary directory under /tmp was accepted")
    with tempfile.TemporaryDirectory(
        prefix="renderdoc-capture-self-test-",
        dir=temporary_root,
    ) as name:
        root = Path(name)
        renderdoccmd = root / "renderdoccmd"
        qrenderdoc = root / "qrenderdoc"
        library = root / "librenderdoc.so"
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
            "if [ \"$QT_QPA_PLATFORM\" != offscreen ]; then exit 9; fi\n"
            "if [ \"$1\" = --version ]; then echo 'QRenderDoc v1.99'; exit 0; fi\n"
            "if [ \"$1\" = --help ]; then echo '--python'; exit 0; fi\n"
            "exit 1\n",
            encoding="utf-8",
        )
        renderdoccmd.chmod(0o755)
        qrenderdoc.chmod(0o755)
        library.write_bytes(b"\x7fELFformal-probe")
        tools = inspect_tools(str(renderdoccmd), str(qrenderdoc), str(library))
        if (
            tools["renderdoc_version"] != "RenderDoc v1.99"
            or tools["qrenderdoc_version"] != "QRenderDoc v1.99"
        ):
            raise CaptureError("tool probe versions differ in self-test")
        capture = root / "capture.rdc"
        capture.write_bytes(b"RenderDoc self-test capture")
        capture_hash = sha256(capture)
        contract = _load_contract(repo, "rtt-light-v1", "current")
        gpu_signature = {
            "pipeline_count": 8,
            "scene_camera_count": 1,
            "mask_camera_count": 1,
            "window_camera_count": 1,
        }
        runtime_checkpoint = {
            "schema_version": RUNTIME_CHECKPOINT_SCHEMA_VERSION,
            "status": "valid",
            "contract_id": "rtt-light-v1",
            "stage_id": "current",
            "generation": 1,
            "checkpoint": {
                "name": "indoor-light-fixture-ready-v1",
                "simulation_tick": 4,
                "simulation_tick_source": "perf_capture.fixed_update_tick",
                "settle_frames": 4,
                "ready_frame_ordinal": 4,
                "capture_frame": 4,
                "capture_begin_frame": 12,
                "capture_end_frame": 12,
                "render_frame_index": 12,
                "frame_count_before_capture": 12,
            },
            "render_inventory": {"scene_target_count": 1},
            "render_resources": CURRENT_RENDER_RESOURCES,
            "fixture": {"fixture_checksum": "self-test"},
            "capture_path": str(capture),
            "requested_renderdoc_api_version": RENDERDOC_API_VERSION,
            "returned_renderdoc_api_version": "1.7.2",
            "selector": {
                "strategy": "wgpu_device_null_window",
                "device_selector": "wgpu::Device::start_graphics_debugger_capture",
                "window_selector": "null",
                "window_count": 1,
                "primary_window": 1,
            },
            "gpu_ready": {
                "pre_capture": gpu_signature,
                "post_capture": gpu_signature,
            },
            "capture_artifact": {
                "sha256": capture_hash,
                "bytes": capture.stat().st_size,
            },
        }
        runtime_path = root / "runtime-checkpoint.json"
        runtime_path.write_text(json.dumps(runtime_checkpoint), encoding="utf-8")
        _runtime_checkpoint(
            runtime_path, contract=contract, stage="current", capture_path=capture
        )
        validate_runtime_checkpoint_v3(
            runtime_checkpoint,
            contract=contract,
            stage_id="current",
            capture_path=None,
            rdc_sha256=capture_hash,
            rdc_bytes=capture.stat().st_size,
        )
        p01_signature = {**gpu_signature, "mask_camera_count": 0}
        p01_checkpoint = {
            **runtime_checkpoint,
            "stage_id": "p01",
            "render_resources": P01_RENDER_RESOURCES,
            "gpu_ready": {
                "pre_capture": p01_signature,
                "post_capture": p01_signature,
            },
        }
        p01_runtime_path = root / "p01-runtime-checkpoint.json"
        p01_runtime_path.write_text(json.dumps(p01_checkpoint), encoding="utf-8")
        _runtime_checkpoint(
            p01_runtime_path, contract=contract, stage="p01", capture_path=capture
        )
        if (
            _manifest_stage_id(requested_stage="current", runtime=runtime_checkpoint)
            != "current"
            or _manifest_stage_id(requested_stage="p01", runtime=p01_checkpoint)
            != "p01"
        ):
            raise CaptureError("RenderDoc manifest stage identity was not preserved")
        try:
            _manifest_stage_id(requested_stage="current", runtime=p01_checkpoint)
        except CaptureError:
            pass
        else:
            raise CaptureError("RenderDoc manifest accepted a mismatched runtime stage")
        relative_runtime_checkpoint = {
            **runtime_checkpoint,
            "capture_path": "raw/indoor-light_capture.rdc",
        }
        try:
            validate_runtime_checkpoint_v3(
                relative_runtime_checkpoint,
                contract=contract,
                stage_id="current",
                capture_path=None,
                rdc_sha256=capture_hash,
                rdc_bytes=capture.stat().st_size,
            )
        except ValueError:
            pass
        else:
            raise CaptureError("relative runtime capture path was accepted")
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
            runtime={"stage_id": "current", "render_resources": CURRENT_RENDER_RESOURCES},
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
                runtime={"stage_id": "current", "render_resources": CURRENT_RENDER_RESOURCES},
            )
        except CaptureError:
            pass
        else:
            raise CaptureError("RenderDoc set identity regression was accepted")
    if unexpected_log_lines("INFO ready\nERROR broken\n", []) != ["ERROR broken"]:
        raise CaptureError("RenderDoc log classifier did not fail closed")
    if unexpected_log_lines("WARNING allowed\n", [r"allowed$"]):
        raise CaptureError("RenderDoc log allowlist did not match exactly")
    if unexpected_log_lines(
        "ICU4X data error: No segmentation model for language: ja\n", []
    ):
        raise CaptureError("known ICU4X Japanese fallback was not classified")
    if unexpected_log_lines(
        "ICU4X data error: No segmentation model for language: ja extra\n", []
    ) != ["ICU4X data error: No segmentation model for language: ja extra"]:
        raise CaptureError("near-match ICU4X diagnostic was accepted")
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
    capture.add_argument("--capsule-manifest", required=True)
    capture.add_argument("--mode", choices=("formal", "rd0"), default="formal")
    capture.add_argument("--capture-session")
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
