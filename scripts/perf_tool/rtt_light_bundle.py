"""Fail-closed RtT-light attempt assembly and baseline registration."""

from __future__ import annotations

import csv
import json
import math
import os
import re
import statistics
import tempfile
import uuid
from dataclasses import asdict
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from .artifacts import sha256, validate_run
from .execution import (
    read_native_memory,
    read_resource_usage,
    require_persistent_output,
)
from .model import Case, REPO_ROOT, SESSION_MANIFEST_SCHEMA_VERSION, Validation
from .policy import determinism_signature, validate_session_artifact_set
from .rtt_light_contract import (
    GATE_UNIT_TYPES,
    build_fixture_layout,
    contract_fingerprints,
    expected_formal_cases,
    expected_gate_result_rows,
    load_rtt_light_contract,
    projection_field_applicability,
    validate_gate_result_rows,
    validate_projection_rows,
)
from .summary import behavior_timeline_signature


ATTEMPT_SCHEMA_VERSION = 1
BASELINE_INDEX_SCHEMA_VERSION = 1
RENDERDOC_MANIFEST_SCHEMA_VERSION = 1
RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION = 2
RENDERDOC_EXTRACTION_SCHEMA_VERSION = 2
RENDERDOC_LOG_PROBLEM_RE = re.compile(
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
SOURCE_CHECKPOINTS_CURRENT = (
    "start",
    "after-audit",
    "after-behavior",
    "after-capture",
    "after-renderdoc",
    "after-memory",
    "before-registration",
)
SESSION_LEGS = frozenset({"audit", "behavior", "capture", "memory", "field-core", "consumer-core"})
WINDOW_LOCK_FIELDS = (
    "logical_width",
    "logical_height",
    "physical_width",
    "physical_height",
    "scale_factor",
    "rtt_quality",
    "scene_target_width",
    "scene_target_height",
    "mask_target_width",
    "mask_target_height",
    "target_scale_factor",
)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key {key!r}")
        result[key] = value
    return result


def read_json_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as error:
        raise RuntimeError(f"cannot read JSON object {path}: {error}") from error
    if not isinstance(value, dict):
        raise RuntimeError(f"JSON artifact is not an object: {path}")
    return value


def atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def write_csv_exclusive(
    path: Path, *, columns: list[str], rows: list[dict[str, str]]
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(descriptor, "w", newline="", encoding="utf-8") as handle:
            writer = csv.DictWriter(handle, fieldnames=columns)
            writer.writeheader()
            writer.writerows(rows)
            handle.flush()
            os.fsync(handle.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def read_exact_csv(path: Path, columns: list[str]) -> list[dict[str, str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            fieldnames = reader.fieldnames or []
    except (OSError, UnicodeError, csv.Error) as error:
        raise RuntimeError(f"cannot parse {path}: {error}") from error
    if fieldnames != columns or any(None in row for row in rows):
        raise RuntimeError(f"{path} columns or row width differ from the contract")
    return rows


def _is_sha256(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _is_commit(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value) is not None


def _relative_file(path: str, *, root: Path) -> Path:
    if not isinstance(path, str) or not path:
        raise RuntimeError(f"artifact path is not a nonempty string: {path!r}")
    candidate = Path(path)
    if candidate.is_absolute() or ".." in candidate.parts or candidate == Path("."):
        raise RuntimeError(f"artifact path is not a safe relative path: {path!r}")
    lexical = root
    for part in candidate.parts:
        lexical /= part
        if lexical.is_symlink():
            raise RuntimeError(f"artifact path traverses a symlink: {path!r}")
    resolved = lexical.resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as error:
        raise RuntimeError(f"artifact path escapes its root: {path!r}") from error
    return resolved


def directory_inventory(path: Path, *, relative_to: Path) -> list[dict[str, Any]]:
    if not path.is_dir() or path.is_symlink():
        raise RuntimeError(f"artifact directory is missing or symlinked: {path}")
    rows: list[dict[str, Any]] = []
    for candidate in sorted(path.rglob("*")):
        if candidate.is_symlink():
            raise RuntimeError(f"artifact tree contains a symlink: {candidate}")
        if not candidate.is_file():
            continue
        relative = candidate.relative_to(relative_to).as_posix()
        rows.append(
            {
                "path": relative,
                "bytes": candidate.stat().st_size,
                "sha256": sha256(candidate),
            }
        )
    return rows


def directory_digest(rows: list[dict[str, Any]]) -> str:
    payload = json.dumps(
        rows, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    import hashlib

    return hashlib.sha256(payload).hexdigest()


def _expected_leg_order(contract: dict[str, Any], stage: str) -> list[str]:
    order: list[str] = []
    for case in expected_formal_cases(contract, stage):
        if case["leg_id"] not in order:
            order.append(case["leg_id"])
    return order


def _validate_attempt_location(
    attempt: Path, *, contract_id: str, stage: str, commit: str, attempt_id: str
) -> tuple[Path, Path]:
    attempt = attempt.resolve()
    if attempt.name != attempt_id:
        raise RuntimeError("attempt directory name differs from job attempt_id")
    try:
        parsed = uuid.UUID(attempt_id)
    except ValueError as error:
        raise RuntimeError("job attempt_id is not a UUID") from error
    if parsed.version != 4 or str(parsed) != attempt_id:
        raise RuntimeError("job attempt_id must be a canonical UUIDv4")
    if attempt.parent.name != "attempts":
        raise RuntimeError("attempt directory is not below an attempts directory")
    generation = attempt.parent.parent
    if generation.name != f"{stage}-{commit[:16]}":
        raise RuntimeError("generation directory differs from stage and subject commit")
    baseline_root = generation.parent
    expected_root = (REPO_ROOT / "target/perf-runs/rtt-light" / contract_id).resolve()
    require_persistent_output(expected_root)
    require_persistent_output(attempt)
    if baseline_root.resolve() != expected_root:
        raise RuntimeError("attempt is outside the canonical RtT-light baseline root")
    return generation, baseline_root


def _validate_job(
    attempt: Path, contract: dict[str, Any]
) -> tuple[dict[str, Any], Path, Path, dict[str, Any]]:
    job = read_json_object(attempt / "job.json")
    required_keys = {
        "schema_version",
        "profile",
        "measurement_kind",
        "contract_id",
        "stage_id",
        "attempt_id",
        "subject_commit",
        "prerequisite_commits",
        "adapter_filter",
        "window_backend",
        "leg_order",
        "completed_legs",
        "source_checks",
        "tooling",
        "status",
    }
    if set(job) != required_keys:
        raise RuntimeError("job.json keys differ from attempt schema v1")
    if job["schema_version"] != ATTEMPT_SCHEMA_VERSION:
        raise RuntimeError("job.json schema_version is not 1")
    if (
        job["profile"] != "rtt-light"
        or job["measurement_kind"] != "formal"
        or job["contract_id"] != contract["contract_id"]
        or job["stage_id"] not in contract["stages"]
        or job["status"] != "completed"
    ):
        raise RuntimeError("job.json identity or status differs from a formal attempt")
    if contract["lifecycle"].get("formal_registration_allowed") is not True:
        raise RuntimeError("the RtT-light contract does not allow formal registration")
    commit = job["subject_commit"]
    if not _is_commit(commit):
        raise RuntimeError("job subject_commit is not a full commit SHA")
    generation, baseline_root = _validate_attempt_location(
        attempt,
        contract_id=contract["contract_id"],
        stage=job["stage_id"],
        commit=commit,
        attempt_id=job["attempt_id"],
    )
    expected_order = _expected_leg_order(contract, job["stage_id"])
    if job["leg_order"] != expected_order or job["completed_legs"] != expected_order:
        raise RuntimeError("job leg order or completion order differs from the formal matrix")
    if not isinstance(job["tooling"], dict) or set(job["tooling"]) != {
        "native_helper_sha256",
        "native_skill_sha256",
        "perf_runner_sha256",
        "renderdoccmd_sha256",
        "qrenderdoc_sha256",
        "librenderdoc_sha256",
        "renderdoc_version",
        "qrenderdoc_version",
        "renderdoc_api_version",
        "renderdoc_capture_helper_sha256",
        "renderdoc_extractor_sha256",
    } or any(
        not _is_sha256(value)
        for key, value in job["tooling"].items()
        if key
        not in {"renderdoc_version", "qrenderdoc_version", "renderdoc_api_version"}
    ) or any(
        not isinstance(job["tooling"][key], str) or not job["tooling"][key]
        for key in {"renderdoc_version", "qrenderdoc_version", "renderdoc_api_version"}
    ):
        raise RuntimeError("job tooling provenance is invalid")
    checks = job["source_checks"]
    expected_checkpoints = (
        SOURCE_CHECKPOINTS_CURRENT
        if job["stage_id"] == "current"
        else tuple(["start", *(f"after-{leg}" for leg in expected_order), "before-registration"])
    )
    if (
        not isinstance(checks, list)
        or any(not isinstance(check, dict) for check in checks)
        or [check.get("checkpoint") for check in checks] != list(expected_checkpoints)
    ):
        raise RuntimeError("job source checks differ from the exact checkpoint order")
    fingerprints: set[str] = set()
    for check in checks:
        if set(check) != {"checkpoint", "commit", "clean", "fingerprint"}:
            raise RuntimeError("job source check keys differ from schema v1")
        if check["commit"] != commit or check["clean"] is not True or not _is_sha256(check["fingerprint"]):
            raise RuntimeError("job source check is dirty, changed, or malformed")
        fingerprints.add(check["fingerprint"])
    if len(fingerprints) != 1:
        raise RuntimeError("job source fingerprint changed during the attempt")
    if not isinstance(job["adapter_filter"], str) or not job["adapter_filter"]:
        raise RuntimeError("job adapter_filter must be nonempty")
    if job["window_backend"] not in {"x11", "wayland"}:
        raise RuntimeError("job window_backend must resolve to x11 or wayland")
    if not isinstance(job["prerequisite_commits"], list) or any(
        not _is_commit(value) for value in job["prerequisite_commits"]
    ) or len(job["prerequisite_commits"]) != len(set(job["prerequisite_commits"])):
        raise RuntimeError("job prerequisite commits are invalid")
    environment_lock = read_json_object(generation / "environment-lock.json")
    return job, generation, baseline_root, environment_lock


def _expected_case(contract: dict[str, Any], formal: dict[str, Any]) -> Case:
    behavior_case = (
        formal["case_id"].removeprefix("behavior-")
        if formal["leg_id"] == "behavior"
        else None
    )
    return Case(
        "indoor-light",
        formal["size"],
        formal["render"],
        contract["formal_matrix"]["seed"],
        None,
        None,
        behavior_case=behavior_case,
    )


def _expected_session_cases(
    contract: dict[str, Any], stage: str, leg_id: str
) -> list[dict[str, Any]]:
    return [
        asdict(_expected_case(contract, formal))
        | {"id": _expected_case(contract, formal).identifier}
        for formal in expected_formal_cases(contract, stage)
        if formal["leg_id"] == leg_id
    ]


def _expected_rtt_selection(
    contract: dict[str, Any], stage: str, leg_id: str
) -> dict[str, Any]:
    lane = "behavior" if leg_id == "behavior" else leg_id if leg_id in {"field-core", "consumer-core"} else "static"
    sizes = [
        case["size"]
        for case in expected_formal_cases(contract, stage)
        if case["leg_id"] == leg_id
    ]
    sizes = list(dict.fromkeys(sizes))
    return {
        "contract_id": contract["contract_id"],
        "stage_id": stage,
        "lane": lane,
        **contract_fingerprints(contract),
        "fixture_id": contract["fixture"]["fixture_id"],
        "layout_checksums": {
            size: build_fixture_layout(contract, size)["layout_checksum"] for size in sizes
        },
        "lifecycle": contract["lifecycle"],
    }


def _expected_matrix(
    contract: dict[str, Any],
    stage: str,
    leg_id: str,
    environment_lock_path: Path,
) -> dict[str, Any]:
    formal = contract["formal_matrix"]
    cases = _expected_session_cases(contract, stage, leg_id)
    sizes = list(dict.fromkeys(case["size"] for case in cases))
    renders = list(dict.fromkeys(case["render"] for case in cases))
    fixed = leg_id in {"audit", "behavior", "field-core", "consumer-core"}
    behavior = leg_id == "behavior"
    windowed = leg_id in {"capture", "memory"}
    lane = "behavior" if behavior else leg_id if leg_id in {"field-core", "consumer-core"} else "static"
    leg_matrix = formal.get(leg_id.replace("-", "_"), {})
    return {
        "workload": "indoor-light",
        "sizes": sizes,
        "renders": renders,
        "seed": formal["seed"],
        "repeat": 3,
        "warmup_secs": None if fixed else leg_matrix["warmup_secs"],
        "measure_secs": None if fixed else leg_matrix["measure_secs"],
        "fixed_hz": formal["fixed_hz"] if fixed else None,
        "warmup_ticks": formal["audit"]["warmup_ticks"] if fixed else None,
        "audit_ticks": formal["audit"]["audit_ticks"] if fixed else None,
        "preflight_runs": leg_matrix.get("preflight_runs", 0),
        "souls": None,
        "familiars": None,
        "familiar_policies": ["baseline"],
        "operation_dialog_modes": ["hidden"],
        "dashboard_modes": ["hidden"],
        "behavior_cases": [case["behavior_case"] for case in cases if case["behavior_case"] is not None],
        "capture_kind": (
            "fixed-step-behavior"
            if behavior
            else "fixed-step-determinism"
            if fixed
            else "frame-time"
        ),
        "clock_mode": "fixed-behavior" if behavior else "fixed" if fixed else "realtime",
        "warmup_checksum_policy": None if fixed else "record",
        "measure_end_checksum_policy": None if fixed else "record",
        "allow_log_patterns": (
            contract["allow_log_patterns"]["headless_audit"]
            if fixed
            else contract["allow_log_patterns"]["windowed"]
        ),
        "tracy_capture_secs": None,
        "window_width": formal["window"]["physical_width"] if windowed else None,
        "window_height": formal["window"]["physical_height"] if windowed else None,
        "window_scale_factor": formal["window"]["scale_factor"] if windowed else None,
        "rtt_quality": formal["window"]["rtt_quality"],
        "environment_lock": str(environment_lock_path.resolve()) if windowed else None,
        "rtt_light_contract": _expected_rtt_selection(contract, stage, leg_id),
    }


def _expected_requested_environment(
    *,
    contract: dict[str, Any],
    leg_id: str,
    job: dict[str, Any],
) -> dict[str, str]:
    values = {
        "BEVY_ASSET_ROOT": str(REPO_ROOT),
        "HW_PRESENT_MODE": contract["formal_matrix"]["present_mode"],
        "HW_WINDOW_BACKEND": (
            "headless" if leg_id in {"audit", "behavior", "field-core", "consumer-core"} else job["window_backend"]
        ),
        "WGPU_BACKEND": contract["formal_matrix"]["backend"],
    }
    if leg_id in {"capture", "memory"}:
        values["WGPU_ADAPTER_NAME"] = job["adapter_filter"]
    return values


def _validate_environment_lock(
    lock: dict[str, Any],
    *,
    contract: dict[str, Any],
    job: dict[str, Any],
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
    if set(lock) != expected_keys or lock["schema_version"] != 1:
        raise RuntimeError("environment-lock.json differs from schema v1")
    source_fingerprint = job["source_checks"][0]["fingerprint"]
    if (
        lock["contract_id"] != contract["contract_id"]
        or lock["stage_id"] != job["stage_id"]
        or lock["subject_commit"] != job["subject_commit"]
        or lock["source_fingerprint"] != source_fingerprint
        or lock["resolved_window_backend"] != job["window_backend"]
        or lock["adapter_backend"] != contract["formal_matrix"]["backend"]
        or lock["requested_present_mode"] != "auto_no_vsync"
        or lock["effective_present_mode"] not in {"immediate", "mailbox", "fifo"}
        or not _is_sha256(lock["capture_binary_sha256"])
    ):
        raise RuntimeError("environment-lock.json identity or render environment differs")
    host = lock["host"]
    if (
        not isinstance(host, dict)
        or set(host) != {"platform", "python", "cpu", "hostname", "cargo", "rustc"}
        or any(not isinstance(value, str) or not value for value in host.values())
    ):
        raise RuntimeError("environment-lock.json host tuple is invalid")
    adapter = lock["adapter"]
    if not isinstance(adapter, dict) or set(adapter) != {"name", "driver", "driver_info", "backend"}:
        raise RuntimeError("environment-lock.json adapter tuple is invalid")
    if any(not isinstance(value, str) for value in adapter.values()) or not adapter["name"]:
        raise RuntimeError("environment-lock.json adapter values are invalid")
    if (
        job["adapter_filter"].casefold() not in adapter["name"].casefold()
        or adapter["backend"].casefold() != contract["formal_matrix"]["backend"]
    ):
        raise RuntimeError("environment-lock.json adapter differs from the formal selector")
    formal_window = contract["formal_matrix"]["window"]
    expected_window = {
        "logical_width": f"{formal_window['logical_width']:.6f}",
        "logical_height": f"{formal_window['logical_height']:.6f}",
        "physical_width": str(formal_window["physical_width"]),
        "physical_height": str(formal_window["physical_height"]),
        "scale_factor": f"{formal_window['scale_factor']:.6f}",
        "rtt_quality": formal_window["rtt_quality"],
        "scene_target_width": str(formal_window["scene_target_width"]),
        "scene_target_height": str(formal_window["scene_target_height"]),
        "mask_target_width": str(formal_window["scene_target_width"]),
        "mask_target_height": str(formal_window["scene_target_height"]),
        "target_scale_factor": f"{formal_window['scale_factor']:.6f}",
    }
    if lock["window"] != expected_window:
        raise RuntimeError("environment-lock.json window tuple differs from the formal matrix")


def _validate_session_file_set(session: Path, leg_id: str) -> None:
    expected = {"matrix.json", "manifest.json", "aggregate.csv", "report.md", "cases"}
    actual = {path.name for path in session.iterdir()}
    if actual != expected:
        raise RuntimeError(
            f"{leg_id} session root artifact set differs: {sorted(actual ^ expected)}"
        )


def _validate_run_file_set(
    run_dir: Path, *, leg_id: str, behavior_case: str | None
) -> None:
    data_files = {
        "window.csv",
        "indoor_light_fixture.csv",
        "indoor_light_layout.csv",
        "indoor_light_presentation.csv",
    }
    root_files = {
        "command.txt",
        "requested-environment.json",
        "run.log",
        "validation.json",
        "run-metadata.json",
        "data",
    }
    if leg_id == "audit":
        data_files |= {"determinism.csv", "determinism_records.csv"}
    elif leg_id == "behavior":
        data_files.add("timeline.json")
        if behavior_case == "load-normal-v1":
            data_files.add("behavior-save.scn.ron")
    elif leg_id in {"capture", "memory"}:
        data_files |= {
            "summary.csv",
            "frames.csv",
            "scene_roots.csv",
            "render_inventory.csv",
        }
        if leg_id == "memory":
            data_files.add("memory.csv")
            root_files |= {"profile-artifact.json", "resource-usage.txt"}
    else:
        raise RuntimeError(f"session file-set validator does not support leg {leg_id}")
    actual_root = {path.name for path in run_dir.iterdir()}
    actual_data = {path.name for path in (run_dir / "data").iterdir()}
    if actual_root != root_files:
        raise RuntimeError(
            f"{run_dir} root artifact set differs: {sorted(actual_root ^ root_files)}"
        )
    if actual_data != data_files:
        raise RuntimeError(
            f"{run_dir} data artifact set differs: {sorted(actual_data ^ data_files)}"
        )
    if any(path.is_symlink() for path in run_dir.rglob("*")):
        raise RuntimeError(f"{run_dir} contains a symlink")


def _load_memory_profile(run_dir: Path, validation: Validation) -> dict[str, Any]:
    try:
        samples = int((validation.summary or {})["samples"])
    except (KeyError, TypeError, ValueError) as error:
        raise RuntimeError(f"{run_dir} has no valid frame sample count") from error
    allocation, allocation_errors = read_native_memory(
        run_dir / "data" / "memory.csv", frame_samples=samples
    )
    process, process_errors = read_resource_usage(run_dir / "resource-usage.txt")
    if allocation_errors or process_errors:
        raise RuntimeError(
            f"{run_dir} memory profile is invalid: "
            + "; ".join([*allocation_errors, *process_errors])
        )
    observed = {
        "instrumentation": "memory",
        "allocation_memory": allocation,
        "process_memory": process,
    }
    if read_json_object(run_dir / "profile-artifact.json") != observed:
        raise RuntimeError(f"{run_dir} profile-artifact.json differs from raw memory data")
    return observed


def _revalidate_run(
    *,
    run_dir: Path,
    expected_case: Case,
    contract: dict[str, Any],
    stage: str,
    leg_id: str,
    job: dict[str, Any],
    environment_lock: dict[str, Any],
    preflight: bool,
) -> Validation:
    _validate_run_file_set(
        run_dir, leg_id=leg_id, behavior_case=expected_case.behavior_case
    )
    metadata = read_json_object(run_dir / "run-metadata.json")
    if metadata.get("case") != asdict(expected_case) or metadata.get("preflight") is not preflight:
        raise RuntimeError(f"{run_dir} run metadata differs from its formal case")
    returncode = metadata.get("returncode")
    if not isinstance(returncode, int):
        raise RuntimeError(f"{run_dir} run metadata returncode is invalid")
    formal = contract["formal_matrix"]
    fixed = leg_id in {"audit", "behavior"}
    windowed = leg_id in {"capture", "memory"}
    validation = validate_run(
        run_dir,
        returncode=returncode,
        expected_case=expected_case,
        expected_adapter=environment_lock["adapter"]["name"] if windowed else None,
        expected_backend=formal["backend"],
        allow_log_patterns=(
            contract["allow_log_patterns"]["headless_audit"]
            if fixed
            else contract["allow_log_patterns"]["windowed"]
        ),
        capture_kind=(
            "fixed-step-behavior"
            if leg_id == "behavior"
            else "fixed-step-determinism"
            if leg_id == "audit"
            else "frame-time"
        ),
        expected_warmup_secs=(formal[leg_id]["warmup_secs"] if windowed else None),
        expected_measure_secs=(formal[leg_id]["measure_secs"] if windowed else None),
        expected_fixed_hz=formal["fixed_hz"] if fixed else None,
        expected_warmup_ticks=formal["audit"]["warmup_ticks"] if fixed else None,
        expected_audit_ticks=formal["audit"]["audit_ticks"] if fixed else None,
        expected_window_backend="headless" if fixed else job["window_backend"],
        expected_present_mode=formal["present_mode"],
        expected_window_width=formal["window"]["physical_width"] if windowed else None,
        expected_window_height=formal["window"]["physical_height"] if windowed else None,
        expected_window_scale_factor=formal["window"]["scale_factor"] if windowed else None,
        expected_rtt_quality=formal["window"]["rtt_quality"],
        expected_contract=contract["contract_id"],
        expected_stage=stage,
        expected_lane="behavior" if leg_id == "behavior" else "static",
    )
    if leg_id == "memory":
        validation.profile_artifact = _load_memory_profile(run_dir, validation)
    stored = read_json_object(run_dir / "validation.json")
    if stored.get("valid") is not True or stored.get("reasons") != []:
        raise RuntimeError(f"{run_dir} stored validation is not valid")
    if leg_id == "memory" and stored.get("profile_artifact") != validation.profile_artifact:
        raise RuntimeError(f"{run_dir} stored memory profile differs from raw artifacts")
    comparable_fields = (
        "summary",
        "adapter",
        "warning_lines",
        "teardown_warning_lines",
        "determinism",
        "determinism_records",
        "scene_roots",
        "render_inventory",
        "window",
        "indoor_light_fixture",
        "indoor_light_layout",
        "indoor_light_presentation",
        "timeline",
        "behavior_save_artifact",
    )
    calculated = validation.to_json()
    for field in comparable_fields:
        if stored.get(field) != calculated.get(field):
            raise RuntimeError(f"{run_dir} stored {field} differs from raw artifact validation")
    if not validation.valid:
        raise RuntimeError(f"{run_dir} raw validation failed: {'; '.join(validation.reasons)}")
    if windowed:
        window = validation.window or {}
        observed_environment = {
            "host": None,
            "adapter": validation.adapter,
            "resolved_window_backend": window.get("resolved_window_backend"),
            "adapter_backend": window.get("adapter_backend"),
            "requested_present_mode": window.get("requested_present_mode"),
            "effective_present_mode": window.get("effective_present_mode"),
            "window": {field: window.get(field) for field in WINDOW_LOCK_FIELDS},
        }
        expected_environment = {
            key: environment_lock[key]
            for key in observed_environment
            if key != "host"
        }
        if {key: value for key, value in observed_environment.items() if key != "host"} != expected_environment:
            raise RuntimeError(f"{run_dir} differs from environment-lock.json")
    return validation


def _load_session_evidence(
    *,
    attempt: Path,
    leg_id: str,
    contract: dict[str, Any],
    stage: str,
    job: dict[str, Any],
    environment_lock: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    session = attempt / leg_id
    _validate_session_file_set(session, leg_id)
    manifest = read_json_object(session / "manifest.json")
    if manifest.get("schema_version") != SESSION_MANIFEST_SCHEMA_VERSION:
        raise RuntimeError(f"{leg_id} manifest schema is not current")
    if manifest.get("status") != "valid" or manifest.get("artifact_set_errors"):
        raise RuntimeError(f"{leg_id} manifest is not valid")
    matrix = _expected_matrix(
        contract, stage, leg_id, attempt.parent.parent / "environment-lock.json"
    )
    if manifest.get("matrix") != matrix or read_json_object(session / "matrix.json") != matrix:
        raise RuntimeError(f"{leg_id} matrix differs from the formal contract")
    expected_cases = _expected_session_cases(contract, stage, leg_id)
    if manifest.get("cases") != expected_cases:
        raise RuntimeError(f"{leg_id} manifest cases differ from the formal matrix")
    if manifest.get("requested_environment") != _expected_requested_environment(
        contract=contract, leg_id=leg_id, job=job
    ):
        raise RuntimeError(f"{leg_id} requested environment differs from the formal contract")
    git = manifest.get("git")
    if (
        not isinstance(git, dict)
        or set(git) != {"commit", "short_commit", "dirty_paths"}
        or git["commit"] != job["subject_commit"]
        or not isinstance(git["short_commit"], str)
        or not 7 <= len(git["short_commit"]) <= 40
        or not job["subject_commit"].startswith(git["short_commit"])
        or git["dirty_paths"] != []
    ):
        raise RuntimeError(f"{leg_id} session was not captured from the clean subject commit")
    source = manifest.get("source")
    fingerprint = job["source_checks"][0]["fingerprint"]
    if not isinstance(source, dict) or (
        source.get("algorithm") != "hell-workers-source-v1"
        or source.get("fingerprint_start") != fingerprint
        or source.get("fingerprint_end") != fingerprint
        or source.get("unchanged") is not True
    ):
        raise RuntimeError(f"{leg_id} session source provenance differs from the attempt")
    if manifest.get("host") != environment_lock["host"]:
        raise RuntimeError(f"{leg_id} host differs from environment-lock.json")
    expected_instrumentation = "memory" if leg_id == "memory" else "capture"
    binary = manifest.get("binary")
    if not isinstance(binary, dict) or binary.get("instrumentation") != expected_instrumentation or not _is_sha256(binary.get("sha256")):
        raise RuntimeError(f"{leg_id} binary provenance is invalid")
    errors = validate_session_artifact_set(session, manifest)
    if errors:
        raise RuntimeError(f"{leg_id} session artifact set failed: {'; '.join(errors)}")

    formal_by_case = {
        case["case_id"]: case
        for case in expected_formal_cases(contract, stage)
        if case["leg_id"] == leg_id
    }
    runner_to_formal = {
        expected["id"]: formal
        for expected, formal in zip(expected_cases, formal_by_case.values(), strict=True)
    }
    evidence: dict[str, dict[str, Any]] = {}
    repeat = matrix["repeat"]
    preflight_runs = matrix["preflight_runs"]
    for runner_case_id, formal in runner_to_formal.items():
        case_dir = session / "cases" / runner_case_id
        expected_case = _expected_case(contract, formal)
        validations: list[Validation] = []
        run_dirs: list[Path] = []
        for preflight in (True, False):
            count = preflight_runs if preflight else repeat
            for index in range(1, count + 1):
                label = ("preflight" if preflight else "run") + f"-{index:03d}"
                run_dir = case_dir / label
                validation = _revalidate_run(
                    run_dir=run_dir,
                    expected_case=expected_case,
                    contract=contract,
                    stage=stage,
                    leg_id=leg_id,
                    job=job,
                    environment_lock=environment_lock,
                    preflight=preflight,
                )
                if not preflight:
                    validations.append(validation)
                    run_dirs.append(run_dir)
        if len(validations) != repeat:
            raise RuntimeError(f"{formal['case_id']} does not have {repeat} measured runs")
        if leg_id == "audit" and len(
            {determinism_signature(value.determinism or []) for value in validations}
        ) != 1:
            raise RuntimeError(f"{formal['case_id']} determinism signatures diverge")
        if leg_id == "behavior" and len(
            {behavior_timeline_signature(value.timeline or []) for value in validations}
        ) != 1:
            raise RuntimeError(f"{formal['case_id']} behavior timelines diverge")
        evidence[formal["case_id"]] = {
            "formal": formal,
            "session": session,
            "case_dir": case_dir,
            "validations": validations,
            "run_dirs": run_dirs,
            "unexpected_log_lines": sum(len(value.warning_lines) for value in validations),
            "environment_contract_match": True,
            "required_sidecars_valid": True,
        }
    return manifest, evidence


def _validate_render_inventory_json(value: object) -> dict[str, str]:
    columns = (
        "scene_target_count",
        "mask_target_count",
        "camera_3d_rtt_count",
        "camera_2d_count",
        "layer_2d_pass_count",
        "soul_proxy_3d",
        "soul_mask_proxy_3d",
        "soul_shadow_proxy_3d",
        "familiar_proxy_3d",
    )
    if not isinstance(value, dict) or set(value) != set(columns):
        raise RuntimeError("RenderDoc render_inventory keys differ from schema v1")
    result: dict[str, str] = {}
    for column in columns:
        observed = value[column]
        if not isinstance(observed, int) or isinstance(observed, bool) or observed < 0:
            raise RuntimeError(f"RenderDoc render_inventory {column} is invalid")
        result[column] = str(observed)
    if result["scene_target_count"] != "1":
        raise RuntimeError("RenderDoc must observe exactly one Scene target")
    if int(result["camera_3d_rtt_count"]) != (
        int(result["scene_target_count"]) + int(result["mask_target_count"])
    ):
        raise RuntimeError("RenderDoc camera count differs from Scene + mask targets")
    return result


def _validate_render_resources(value: object) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {
        "scene_target_label",
        "mask_target_label",
        "composite_draw_count",
        "composite_texture_bindings",
        "composite_sampler_bindings",
    }:
        raise RuntimeError("RenderDoc render_resources differs from schema v2")
    for label in (value["scene_target_label"], value["mask_target_label"]):
        if not isinstance(label, str) or not label:
            raise RuntimeError("RenderDoc resource target label is invalid")
    if (
        not isinstance(value["composite_draw_count"], int)
        or isinstance(value["composite_draw_count"], bool)
        or value["composite_draw_count"] != 1
    ):
        raise RuntimeError("RenderDoc composite draw count is invalid")
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
            raise RuntimeError(f"RenderDoc {key} differs from schema v2")
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
                raise RuntimeError(f"RenderDoc {key} has an invalid binding")
            if key == "composite_texture_bindings" and (
                not isinstance(row["target"], str) or not row["target"]
            ):
                raise RuntimeError("RenderDoc composite texture target is invalid")
    if value != EXPECTED_RENDER_RESOURCES:
        raise RuntimeError("RenderDoc composite bindings differ from the current source")
    return value


def _validate_composite_topology(
    value: object,
    *,
    render_resources: dict[str, Any],
    tracked_resources: dict[str, dict[str, str]],
    bindings: list[dict[str, Any]],
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {"draw_count", "draws"}:
        raise RuntimeError("RenderDoc composite topology differs from schema v2")
    draws = value["draws"]
    if (
        not isinstance(value["draw_count"], int)
        or isinstance(value["draw_count"], bool)
        or value["draw_count"] != render_resources["composite_draw_count"]
        or not isinstance(draws, list)
        or len(draws) != value["draw_count"]
        or len(draws) != 1
    ):
        raise RuntimeError("RenderDoc composite draw count differs from the current source")
    draw = draws[0]
    if not isinstance(draw, dict) or set(draw) != {
        "pass_id",
        "event_id",
        "texture_bindings",
        "sampler_bindings",
    }:
        raise RuntimeError("RenderDoc composite draw differs from schema v2")
    if (
        not isinstance(draw["pass_id"], str)
        or not draw["pass_id"]
        or not isinstance(draw["event_id"], int)
        or isinstance(draw["event_id"], bool)
        or draw["event_id"] <= 0
    ):
        raise RuntimeError("RenderDoc composite draw identity is invalid")
    expected_textures = [
        {
            **binding,
            "resource_id": tracked_resources[binding["target"]]["resource_id"],
        }
        for binding in render_resources["composite_texture_bindings"]
    ]
    if (
        not isinstance(draw["texture_bindings"], list)
        or draw["texture_bindings"] != expected_textures
    ):
        raise RuntimeError("RenderDoc composite texture bindings differ from current source")
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
        raise RuntimeError("RenderDoc composite sampler bindings differ from current source")
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
    for texture in draw["texture_bindings"]:
        if (
            draw["pass_id"],
            draw["event_id"],
            f"{texture['stage']}:read-only",
            texture["fixed_bind_set_or_space"],
            texture["fixed_bind_number"],
            texture["resource_id"],
        ) not in raw_bindings:
            raise RuntimeError("RenderDoc composite texture is absent from raw bindings")
    for sampler in samplers:
        if (
            draw["pass_id"],
            draw["event_id"],
            f"{sampler['stage']}:sampler",
            sampler["fixed_bind_set_or_space"],
            sampler["fixed_bind_number"],
            sampler["resource_id"],
        ) not in raw_bindings:
            raise RuntimeError("RenderDoc composite sampler is absent from raw bindings")
    return draw


def _load_renderdoc_evidence(
    *,
    attempt: Path,
    contract: dict[str, Any],
    stage: str,
    job: dict[str, Any],
    environment_lock: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    directory = attempt / "renderdoc"
    manifest = read_json_object(directory / "manifest.json")
    expected_keys = {
        "schema_version",
        "status",
        "contract_id",
        "stage_id",
        "case_id",
        "size",
        "render",
        "source",
        "binary",
        "tool",
        "replay_tool",
        "library",
        "capture_helper",
        "extractor",
        "environment",
        "checkpoint",
        "capture",
        "extraction",
        "runtime_checkpoint",
        "log",
        "fixture",
        "unexpected_log_lines",
    }
    if set(manifest) != expected_keys or manifest["schema_version"] != RENDERDOC_MANIFEST_SCHEMA_VERSION:
        raise RuntimeError("RenderDoc manifest differs from schema v1")
    if (
        manifest["status"] != "valid"
        or manifest["contract_id"] != contract["contract_id"]
        or manifest["stage_id"] != stage
        or manifest["case_id"] != "renderdoc-medium-gpu"
        or manifest["size"] != "medium"
        or manifest["render"] != "gpu"
        or manifest["unexpected_log_lines"] != 0
    ):
        raise RuntimeError("RenderDoc manifest identity or status differs")
    source = manifest["source"]
    fingerprint = job["source_checks"][0]["fingerprint"]
    if source != {
        "commit": job["subject_commit"],
        "clean": True,
        "fingerprint": fingerprint,
    }:
        raise RuntimeError("RenderDoc source provenance differs from the attempt")
    binary = manifest["binary"]
    if not isinstance(binary, dict) or set(binary) != {"path", "sha256"} or not _is_sha256(binary["sha256"]):
        raise RuntimeError("RenderDoc binary provenance is invalid")
    if not isinstance(binary["path"], str) or not binary["path"]:
        raise RuntimeError("RenderDoc binary path is invalid")
    tool = manifest["tool"]
    if not isinstance(tool, dict) or set(tool) != {"path", "version", "sha256"}:
        raise RuntimeError("RenderDoc tool provenance is invalid")
    if (
        tool["version"] != job["tooling"]["renderdoc_version"]
        or tool["sha256"] != job["tooling"]["renderdoccmd_sha256"]
        or not isinstance(tool["path"], str)
        or not tool["path"]
    ):
        raise RuntimeError("RenderDoc tool differs from job provenance")
    replay_tool = manifest["replay_tool"]
    if (
        not isinstance(replay_tool, dict)
        or set(replay_tool) != {"path", "version", "sha256"}
        or not isinstance(replay_tool["path"], str)
        or not replay_tool["path"]
        or replay_tool["version"] != job["tooling"]["qrenderdoc_version"]
        or replay_tool["sha256"] != job["tooling"]["qrenderdoc_sha256"]
    ):
        raise RuntimeError("RenderDoc replay tool differs from job provenance")
    library = manifest["library"]
    if (
        not isinstance(library, dict)
        or set(library) != {"path", "sha256", "api_version"}
        or not isinstance(library["path"], str)
        or not library["path"]
        or library["sha256"] != job["tooling"]["librenderdoc_sha256"]
        or library["api_version"] != job["tooling"]["renderdoc_api_version"]
    ):
        raise RuntimeError("librenderdoc provenance differs from the formal job")
    capture_helper = manifest["capture_helper"]
    if (
        not isinstance(capture_helper, dict)
        or set(capture_helper) != {"path", "sha256"}
        or not isinstance(capture_helper["path"], str)
        or not capture_helper["path"]
        or capture_helper["sha256"]
        != job["tooling"]["renderdoc_capture_helper_sha256"]
    ):
        raise RuntimeError("RenderDoc capture helper differs from job provenance")
    extractor = manifest["extractor"]
    if (
        not isinstance(extractor, dict)
        or set(extractor) != {"path", "sha256"}
        or not isinstance(extractor["path"], str)
        or not extractor["path"]
        or extractor["sha256"] != job["tooling"]["renderdoc_extractor_sha256"]
    ):
        raise RuntimeError("RenderDoc extractor differs from job provenance")
    expected_environment = {
        "host": environment_lock["host"],
        "adapter": environment_lock["adapter"],
        "resolved_window_backend": environment_lock["resolved_window_backend"],
        "adapter_backend": environment_lock["adapter_backend"],
        "requested_present_mode": environment_lock["requested_present_mode"],
        "effective_present_mode": environment_lock["effective_present_mode"],
        "window": environment_lock["window"],
    }
    if manifest["environment"] != expected_environment:
        raise RuntimeError("RenderDoc environment differs from environment-lock.json")
    renderdoc_contract = contract["formal_matrix"]["renderdoc"]
    checkpoint = manifest["checkpoint"]
    if not isinstance(checkpoint, dict) or set(checkpoint) != {
        "name",
        "simulation_tick",
        "settle_frames",
        "capture_frame",
        "render_frame_index",
        "validated_frames",
    }:
        raise RuntimeError("RenderDoc checkpoint differs from schema v1")
    if (
        checkpoint["name"] != "indoor-light-fixture-ready-v1"
        or checkpoint["settle_frames"] != renderdoc_contract["settle_frames"]
        or checkpoint["capture_frame"] != renderdoc_contract["capture_frame"]
        or checkpoint["validated_frames"] != 1
        or not isinstance(checkpoint["simulation_tick"], int)
        or isinstance(checkpoint["simulation_tick"], bool)
        or checkpoint["simulation_tick"] < 0
        or not isinstance(checkpoint["render_frame_index"], int)
        or isinstance(checkpoint["render_frame_index"], bool)
        or checkpoint["render_frame_index"] < renderdoc_contract["capture_frame"]
    ):
        raise RuntimeError("RenderDoc fixed checkpoint differs from the contract")
    capture = manifest["capture"]
    extraction = manifest["extraction"]
    runtime_checkpoint = manifest["runtime_checkpoint"]
    capture_log = manifest["log"]
    artifact_paths: dict[str, Path] = {}
    for label, value, expected_suffix in (
        ("capture", capture, ".rdc"),
        ("extraction", extraction, ".json"),
        ("runtime_checkpoint", runtime_checkpoint, ".json"),
        ("log", capture_log, ".log"),
    ):
        if not isinstance(value, dict) or set(value) != {"path", "bytes", "sha256"}:
            raise RuntimeError(f"RenderDoc {label} locator differs from schema v1")
        artifact = _relative_file(value["path"], root=directory)
        if not artifact.is_file() or artifact.suffix != expected_suffix:
            raise RuntimeError(f"RenderDoc {label} artifact is missing")
        if value["bytes"] != artifact.stat().st_size or value["bytes"] <= 0 or value["sha256"] != sha256(artifact):
            raise RuntimeError(f"RenderDoc {label} size or hash differs")
        artifact_paths[label] = artifact
    raw_files = list((directory / "raw").glob("*.rdc")) if (directory / "raw").is_dir() else []
    if len(raw_files) != 1 or raw_files[0].resolve() != _relative_file(capture["path"], root=directory):
        raise RuntimeError("RenderDoc leg must contain exactly one raw .rdc")
    extracted_path = artifact_paths["extraction"]
    expected_inventory = {
        "manifest.json",
        capture["path"],
        extraction["path"],
        runtime_checkpoint["path"],
        capture_log["path"],
    }
    actual_inventory = {
        row["path"] for row in directory_inventory(directory, relative_to=directory)
    }
    if actual_inventory != expected_inventory:
        raise RuntimeError("RenderDoc artifact set differs from schema v1")
    runtime = read_json_object(artifact_paths["runtime_checkpoint"])
    if set(runtime) != {
        "schema_version",
        "status",
        "checkpoint",
        "render_inventory",
        "render_resources",
        "fixture",
        "capture_path",
        "renderdoc_api_version",
    } or runtime["schema_version"] != RENDERDOC_RUNTIME_CHECKPOINT_SCHEMA_VERSION or runtime["status"] != "valid":
        raise RuntimeError("RenderDoc runtime checkpoint differs from schema v2")
    if (
        runtime["checkpoint"] != checkpoint
        or runtime["fixture"] != manifest["fixture"]
        or runtime["renderdoc_api_version"]
        != job["tooling"]["renderdoc_api_version"]
        or not isinstance(runtime["capture_path"], str)
        or Path(runtime["capture_path"]).name != artifact_paths["capture"].name
    ):
        raise RuntimeError("RenderDoc runtime checkpoint differs from manifest evidence")
    render_resources = _validate_render_resources(runtime["render_resources"])
    log_text = artifact_paths["log"].read_text(encoding="utf-8")
    allowed_log_patterns = [
        re.compile(pattern) for pattern in contract["allow_log_patterns"]["windowed"]
    ]
    unexpected = [
        line
        for line in log_text.splitlines()
        if RENDERDOC_LOG_PROBLEM_RE.search(line)
        and not any(pattern.search(line) for pattern in allowed_log_patterns)
    ]
    if unexpected or manifest["unexpected_log_lines"] != len(unexpected):
        raise RuntimeError("RenderDoc log contains an unexpected warning or error")

    extracted = read_json_object(extracted_path)
    if set(extracted) != {
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
    } or extracted["schema_version"] != RENDERDOC_EXTRACTION_SCHEMA_VERSION:
        raise RuntimeError("RenderDoc extraction differs from schema v1")
    if (
        extracted["api"] != "vulkan"
        or extracted["capture_sha256"] != capture["sha256"]
        or extracted["validated_frames"] != 1
        or not isinstance(extracted["event_count"], int)
        or isinstance(extracted["event_count"], bool)
        or extracted["event_count"] <= 0
        or not isinstance(extracted["draw_count"], int)
        or isinstance(extracted["draw_count"], bool)
        or extracted["draw_count"] <= 0
    ):
        raise RuntimeError("RenderDoc extraction frame metadata is invalid")
    passes = extracted["passes"]
    attachments = extracted["attachments"]
    bindings = extracted["bindings"]
    schemas = {
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
    for name, rows in (
        ("passes", passes),
        ("attachments", attachments),
        ("bindings", bindings),
    ):
        if (
            not isinstance(rows, list)
            or not rows
            or any(not isinstance(row, dict) or set(row) != schemas[name] for row in rows)
        ):
            raise RuntimeError(f"RenderDoc extraction {name} differs from schema v1")
    if [row["pass_id"] for row in passes] != [
        f"pass-{index:04d}" for index in range(1, len(passes) + 1)
    ] or [row["attachment_id"] for row in attachments] != [
        f"attachment-{index:06d}" for index in range(1, len(attachments) + 1)
    ] or [row["binding_id"] for row in bindings] != [
        f"binding-{index:07d}" for index in range(1, len(bindings) + 1)
    ]:
        raise RuntimeError("RenderDoc extraction identifiers are not canonical")
    pass_ranges: dict[str, tuple[int, int]] = {}
    for row in passes:
        if (
            not isinstance(row["name"], str)
            or not row["name"]
            or any(
                not isinstance(row[field], int) or isinstance(row[field], bool)
                for field in ("first_event", "last_event", "draw_count")
            )
            or row["first_event"] <= 0
            or row["last_event"] < row["first_event"]
            or row["draw_count"] <= 0
        ):
            raise RuntimeError("RenderDoc pass row is invalid")
        pass_ranges[row["pass_id"]] = (row["first_event"], row["last_event"])
    if sum(row["draw_count"] for row in passes) != extracted["draw_count"]:
        raise RuntimeError("RenderDoc pass draw total differs from draw_count")
    for name, rows in (("attachment", attachments), ("binding", bindings)):
        for row in rows:
            event_range = pass_ranges.get(row["pass_id"])
            numeric_fields = (
                ("slot",)
                if name == "attachment"
                else ("fixed_bind_set_or_space", "fixed_bind_number")
            )
            if (
                event_range is None
                or not isinstance(row["event_id"], int)
                or isinstance(row["event_id"], bool)
                or not event_range[0] <= row["event_id"] <= event_range[1]
                or any(
                    not isinstance(row[field], int)
                    or isinstance(row[field], bool)
                    or row[field] < 0
                    for field in numeric_fields
                )
                or not isinstance(row["resource_id"], str)
                or not row["resource_id"]
            ):
                raise RuntimeError(f"RenderDoc {name} row is invalid")
            if name == "attachment" and row["kind"] not in {"color", "depth"}:
                raise RuntimeError("RenderDoc attachment kind is invalid")
            if name == "binding" and (
                not isinstance(row["category"], str) or ":" not in row["category"]
            ):
                raise RuntimeError("RenderDoc binding category is invalid")
    if extracted["event_count"] < extracted["draw_count"]:
        raise RuntimeError("RenderDoc event count is smaller than draw_count")
    tracked_resources = extracted["tracked_resources"]
    if not isinstance(tracked_resources, dict) or set(tracked_resources) != {
        "scene_target",
        "mask_target",
    }:
        raise RuntimeError("RenderDoc tracked-resource schema differs from v1")
    expected_labels = {
        "scene_target": render_resources["scene_target_label"],
        "mask_target": render_resources["mask_target_label"],
    }
    tracked_ids: dict[str, str] = {}
    for key, label in expected_labels.items():
        tracked = tracked_resources[key]
        if (
            not isinstance(tracked, dict)
            or set(tracked) != {"label", "resource_id"}
            or tracked["label"] != label
            or not isinstance(tracked["resource_id"], str)
            or not tracked["resource_id"]
        ):
            raise RuntimeError(f"RenderDoc {key} target was not resolved by its runtime label")
        tracked_ids[key] = tracked["resource_id"]
    composite_draw = _validate_composite_topology(
        extracted["composite_topology"],
        render_resources=render_resources,
        tracked_resources=tracked_resources,
        bindings=bindings,
    )
    replay_structure = extracted["replay_structure"]
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
        not isinstance(replay_structure, dict)
        or set(replay_structure) != expected_structure_keys
        or any(
            not isinstance(value, int) or isinstance(value, bool) or value < 0
            for value in replay_structure.values()
        )
        or replay_structure["render_pass_count"] != len(passes)
        or replay_structure["attachment_count"] != len(attachments)
        or replay_structure["binding_count"] != len(bindings)
        or replay_structure["render_pass_count"] < 2
        or replay_structure["composite_draw_count"] != 1
        or replay_structure["composite_texture_binding_count"] != 2
        or replay_structure["composite_sampler_binding_count"] != 2
        or composite_draw["event_id"] <= 0
    ):
        raise RuntimeError("RenderDoc replay structure differs from extracted evidence")
    for key, resource_id in tracked_ids.items():
        attachment_count = sum(
            row["resource_id"] == resource_id for row in attachments
        )
        binding_count = sum(row["resource_id"] == resource_id for row in bindings)
        if (
            replay_structure[f"{key}_attachment_count"] != attachment_count
            or replay_structure[f"{key}_binding_count"] != binding_count
            or attachment_count < 1
            or binding_count < 1
        ):
            raise RuntimeError(
                f"RenderDoc replay does not prove {key} attachment and binding topology"
            )
    render_inventory = _validate_render_inventory_json(runtime["render_inventory"])
    fixture_layout = build_fixture_layout(contract, "medium")
    expected_fixture = {
        "fixture_checksum": fixture_layout["layout_checksum"],
        "rooms": fixture_layout["counts"]["rooms"],
        "completed_floors": fixture_layout["counts"]["completed_floors"],
        "completed_walls": fixture_layout["counts"]["completed_walls"],
        "doors": fixture_layout["counts"]["doors"],
        "supplied_lamp_candidates": fixture_layout["counts"]["supplied_lamp_candidates"],
        "unsupplied_lamp_candidates": fixture_layout["counts"]["unsupplied_lamp_candidates"],
    }
    if manifest["fixture"] != expected_fixture:
        raise RuntimeError("RenderDoc fixture evidence differs from the contract")
    evidence = {
        "renderdoc-medium-gpu": {
            "formal": next(
                case
                for case in expected_formal_cases(contract, stage)
                if case["case_id"] == "renderdoc-medium-gpu"
            ),
            "session": directory,
            "case_dir": directory,
            "validations": [],
            "run_dirs": [],
            "fixture": manifest["fixture"],
            "render_inventory": render_inventory,
            "validated_frames": 1,
            "unexpected_log_lines": 0,
            "environment_contract_match": True,
            "required_sidecars_valid": True,
        }
    }
    return manifest, evidence


def collect_attempt_evidence(
    attempt: Path,
) -> tuple[
    dict[str, Any],
    dict[str, Any],
    Path,
    Path,
    dict[str, Any],
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
]:
    attempt = attempt.resolve()
    preliminary = read_json_object(attempt / "job.json")
    contract_id = preliminary.get("contract_id")
    if not isinstance(contract_id, str):
        raise RuntimeError("job.json has no contract_id")
    contract = load_rtt_light_contract(contract_id)
    job, generation, baseline_root, environment_lock = _validate_job(attempt, contract)
    if job["stage_id"] != "current":
        raise RuntimeError("the implemented formal bundle assembler currently supports current")
    _validate_environment_lock(environment_lock, contract=contract, job=job)
    manifests: dict[str, dict[str, Any]] = {}
    cases: dict[str, dict[str, Any]] = {}
    for leg_id in job["leg_order"]:
        if leg_id == "renderdoc":
            manifest, leg_cases = _load_renderdoc_evidence(
                attempt=attempt,
                contract=contract,
                stage=job["stage_id"],
                job=job,
                environment_lock=environment_lock,
            )
        elif leg_id in SESSION_LEGS:
            manifest, leg_cases = _load_session_evidence(
                attempt=attempt,
                leg_id=leg_id,
                contract=contract,
                stage=job["stage_id"],
                job=job,
                environment_lock=environment_lock,
            )
        else:
            raise RuntimeError(f"unsupported formal leg {leg_id}")
        manifests[leg_id] = manifest
        duplicates = set(cases) & set(leg_cases)
        if duplicates:
            raise RuntimeError("formal cases are duplicated: " + ", ".join(sorted(duplicates)))
        cases.update(leg_cases)
    expected_ids = [case["case_id"] for case in expected_formal_cases(contract, job["stage_id"])]
    if list(cases) != expected_ids:
        raise RuntimeError("attempt case set or order differs from the formal matrix")
    capture_sha = manifests["capture"]["binary"]["sha256"]
    for leg_id in ("audit", "behavior"):
        if manifests[leg_id]["binary"]["sha256"] != capture_sha:
            raise RuntimeError(f"{leg_id} did not use the Capture binary")
    if manifests["renderdoc"]["binary"]["sha256"] != capture_sha:
        raise RuntimeError("RenderDoc did not use the Capture binary")
    memory_sha = manifests["memory"]["binary"]["sha256"]
    if memory_sha == capture_sha:
        raise RuntimeError("Memory must use a distinct profiling-memory binary")
    if environment_lock["capture_binary_sha256"] != capture_sha:
        raise RuntimeError("Capture binary differs from environment-lock.json")
    return (
        contract,
        job,
        generation,
        baseline_root,
        environment_lock,
        manifests,
        cases,
    )


def _format_nonnegative_float(value: float) -> str:
    if not math.isfinite(value) or value < 0.0:
        raise RuntimeError("projection metric is not a finite nonnegative float")
    return format(value, ".17g")


def _only_equal(values: list[Any], *, label: str) -> Any:
    if not values:
        raise RuntimeError(f"{label} has no measured values")
    first = values[0]
    if any(value != first for value in values[1:]):
        raise RuntimeError(f"{label} differs across repeated runs")
    return first


def _fixture_projection(evidence: dict[str, Any]) -> dict[str, str]:
    validations: list[Validation] = evidence["validations"]
    if validations:
        fixture = _only_equal(
            [validation.indoor_light_fixture for validation in validations],
            label=f"{evidence['formal']['case_id']} fixture sidecar",
        )
        if not isinstance(fixture, dict):
            raise RuntimeError("formal run has no indoor-light fixture sidecar")
        checksum = fixture.get("layout_checksum")
        result = {
            "fixture_checksum": checksum,
            "rooms": fixture.get("rooms"),
            "completed_floors": fixture.get("completed_floors"),
            "completed_walls": fixture.get("completed_walls"),
            "doors": fixture.get("doors"),
            "supplied_lamp_candidates": fixture.get("supplied_lamp_candidates"),
            "unsupplied_lamp_candidates": fixture.get(
                "unsupplied_lamp_candidates"
            ),
        }
    else:
        fixture = evidence.get("fixture")
        if not isinstance(fixture, dict):
            raise RuntimeError("RenderDoc evidence has no fixture identity")
        result = {
            "fixture_checksum": fixture.get("fixture_checksum"),
            "rooms": str(fixture.get("rooms")),
            "completed_floors": str(fixture.get("completed_floors")),
            "completed_walls": str(fixture.get("completed_walls")),
            "doors": str(fixture.get("doors")),
            "supplied_lamp_candidates": str(
                fixture.get("supplied_lamp_candidates")
            ),
            "unsupplied_lamp_candidates": str(
                fixture.get("unsupplied_lamp_candidates")
            ),
        }
    if any(not isinstance(value, str) or not value for value in result.values()):
        raise RuntimeError("fixture projection contains an empty or non-string value")
    return result


def _render_inventory_projection(evidence: dict[str, Any]) -> dict[str, str]:
    validations: list[Validation] = evidence["validations"]
    if validations:
        inventory = _only_equal(
            [validation.render_inventory for validation in validations],
            label=f"{evidence['formal']['case_id']} render inventory",
        )
        if not isinstance(inventory, dict):
            raise RuntimeError("windowed formal run has no render inventory")
        return {
            column: inventory[column]
            for column in (
                "scene_target_count",
                "mask_target_count",
                "camera_3d_rtt_count",
                "camera_2d_count",
                "layer_2d_pass_count",
                "soul_proxy_3d",
                "soul_mask_proxy_3d",
                "soul_shadow_proxy_3d",
                "familiar_proxy_3d",
            )
        }
    inventory = evidence.get("render_inventory")
    if not isinstance(inventory, dict):
        raise RuntimeError("RenderDoc evidence has no render inventory")
    return dict(inventory)


def build_projection_rows(
    contract: dict[str, Any],
    stage: str,
    cases: dict[str, dict[str, Any]],
) -> list[dict[str, str]]:
    columns = [column["name"] for column in contract["projection"]["columns"]]
    rows: list[dict[str, str]] = []
    for formal in expected_formal_cases(contract, stage):
        evidence = cases.get(formal["case_id"])
        if evidence is None:
            raise RuntimeError(f"missing formal evidence for {formal['case_id']}")
        row = {column: "" for column in columns}
        row.update(
            {
                "schema_version": str(contract["projection"]["schema_version"]),
                "contract_id": contract["contract_id"],
                "stage_id": stage,
                "lane": formal["lane"],
                "leg_id": formal["leg_id"],
                "case_id": formal["case_id"],
            }
        )
        applicability = projection_field_applicability(
            contract, stage, formal["leg_id"], formal["case_id"]
        )
        for group_name, group in contract["projection"]["field_groups"].items():
            row[group["availability_column"]] = applicability[group_name]
        row.update(_fixture_projection(evidence))

        if applicability["render_inventory"] == "available":
            row.update(_render_inventory_projection(evidence))
        if applicability["wall_frame"] == "available":
            summaries = [
                validation.summary for validation in evidence["validations"]
            ]
            if any(not isinstance(summary, dict) for summary in summaries):
                raise RuntimeError(
                    f"{formal['case_id']} has no wall-frame summary in every run"
                )
            for source, target in (
                ("p50_ms", "wall_frame_p50_ms"),
                ("p95_ms", "wall_frame_p95_ms"),
                ("p99_ms", "wall_frame_p99_ms"),
                ("max_ms", "wall_frame_max_ms"),
            ):
                row[target] = _format_nonnegative_float(
                    statistics.median(float(summary[source]) for summary in summaries)
                )
        if applicability["memory"] == "available":
            profiles = [
                validation.profile_artifact
                for validation in evidence["validations"]
            ]
            if any(not isinstance(profile, dict) for profile in profiles):
                raise RuntimeError(
                    f"{formal['case_id']} has no memory profile in every run"
                )
            rss_values = [
                int(profile["process_memory"]["max_rss_kib"])
                for profile in profiles
            ]
            live_values = [
                int(profile["allocation_memory"]["peak_live_bytes"])
                for profile in profiles
            ]
            rss_median = statistics.median(rss_values)
            live_median = statistics.median(live_values)
            if not isinstance(rss_median, int) or not isinstance(live_median, int):
                raise RuntimeError("three-run memory median is not an integer")
            row["process_max_rss_kib"] = str(rss_median)
            row["allocation_peak_live_bytes"] = str(live_median)
        rows.append(row)
    validate_projection_rows(contract, stage, rows)
    return rows


def _parse_gate_value(value: str, value_type: str) -> bool | int | float | str:
    if value_type == "boolean":
        if value not in {"true", "false"}:
            raise RuntimeError(f"invalid boolean gate value {value!r}")
        return value == "true"
    if value_type in {"u64", "i64"}:
        return int(value)
    if value_type == "f64":
        parsed = float(value)
        if not math.isfinite(parsed):
            raise RuntimeError(f"invalid floating gate value {value!r}")
        return parsed
    if value_type == "sha256":
        return value
    raise RuntimeError(f"unsupported gate value type {value_type}")


def _comparison_passes(
    observed: str, threshold: str, comparator: str, value_type: str
) -> bool:
    left = _parse_gate_value(observed, value_type)
    right = _parse_gate_value(threshold, value_type)
    if comparator == "eq":
        return left == right
    if isinstance(left, (bool, str)) or isinstance(right, (bool, str)):
        raise RuntimeError(f"gate comparator {comparator} requires numeric values")
    if comparator == "le":
        return left <= right
    if comparator == "ge":
        return left >= right
    raise RuntimeError(f"unsupported gate comparator {comparator}")


def _gate_observed(
    expected: dict[str, str],
    cases: dict[str, dict[str, Any]],
) -> str:
    case_id = expected["case_id"]
    metric_id = expected["metric_id"]
    if case_id == "attempt":
        if metric_id in {"clean_commit", "source_fingerprint_unchanged"}:
            return "true"
        raise RuntimeError(f"unsupported attempt validity metric {metric_id}")
    evidence = cases.get(case_id)
    if evidence is None:
        raise RuntimeError(f"gate metric refers to missing formal case {case_id}")
    validations: list[Validation] = evidence["validations"]
    if metric_id == "valid_runs":
        return str(len(validations))
    if metric_id == "invalid_runs":
        return "0"
    if metric_id == "validated_frames":
        return str(evidence.get("validated_frames", 0))
    if metric_id == "determinism_signature_count":
        return str(
            len(
                {
                    determinism_signature(validation.determinism or [])
                    for validation in validations
                }
            )
        )
    if metric_id == "unexpected_log_lines":
        return str(evidence["unexpected_log_lines"])
    if metric_id == "environment_contract_match":
        return "true" if evidence["environment_contract_match"] else "false"
    if metric_id == "required_sidecars_valid":
        return "true" if evidence["required_sidecars_valid"] else "false"
    raise RuntimeError(f"stage {expected['stage_id']} gate metric is not implemented: {metric_id}")


def build_gate_result_rows(
    contract: dict[str, Any],
    stage: str,
    cases: dict[str, dict[str, Any]],
) -> list[dict[str, str]]:
    columns = contract["gate_result"]["columns"]
    rows: list[dict[str, str]] = []
    for expected in expected_gate_result_rows(contract, stage):
        observed = _gate_observed(expected, cases)
        passed = _comparison_passes(
            observed,
            expected["threshold"],
            expected["comparator"],
            expected["value_type"],
        )
        rows.append(
            {
                "gate_id": expected["gate_id"],
                "stage_id": stage,
                "case_id": expected["case_id"],
                "metric_id": expected["metric_id"],
                "status": "pass" if passed else "fail",
                "unit": expected["unit"],
                "observed": observed,
                "comparator": expected["comparator"],
                "threshold": expected["threshold"],
                "reference_artifact": expected["reference_artifact"],
                "subject_artifact": expected["subject_artifact"],
                "reason_code": (
                    "none"
                    if passed
                    else "value_mismatch"
                    if expected["comparator"] == "eq"
                    else "threshold_exceeded"
                ),
            }
        )
    if any(list(row) != columns for row in rows):
        raise RuntimeError("generated gate result column order differs from the contract")
    validate_gate_result_rows(contract, stage, rows, require_pass=True)
    return rows


def _validate_attempt_file_set(
    attempt: Path, *, leg_order: list[str], finalized: bool
) -> None:
    expected = {"job.json", "orchestrator.log", *leg_order}
    if finalized:
        expected |= {"data", "attempt-manifest.json"}
    actual = {path.name for path in attempt.iterdir()}
    if actual != expected:
        raise RuntimeError(
            "attempt root artifact set differs: " + ", ".join(sorted(actual ^ expected))
        )
    if not (attempt / "orchestrator.log").is_file():
        raise RuntimeError("attempt orchestrator.log is missing")
    if any(path.is_symlink() for path in attempt.rglob("*")):
        raise RuntimeError("attempt artifact tree contains a symlink")


def _raw_attempt_inventory(
    attempt: Path, *, finalized: bool
) -> list[dict[str, Any]]:
    rows = directory_inventory(attempt, relative_to=attempt)
    if finalized:
        rows = [
            row
            for row in rows
            if row["path"] != "attempt-manifest.json"
            and not row["path"].startswith("data/")
        ]
    return rows


def _case_index(
    *, attempt: Path, cases: dict[str, dict[str, Any]]
) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    for case_id, evidence in cases.items():
        case_dir: Path = evidence["case_dir"]
        inventory = directory_inventory(case_dir, relative_to=attempt)
        formal = evidence["formal"]
        result[case_id] = {
            "leg_id": formal["leg_id"],
            "lane": formal["lane"],
            "size": formal["size"],
            "render": formal["render"],
            "repeat": formal["repeat"],
            "status": "valid",
            "path": case_dir.relative_to(attempt).as_posix(),
            "artifact_count": len(inventory),
            "directory_sha256": directory_digest(inventory),
        }
    return result


def _attempt_manifest(
    *,
    attempt: Path,
    contract: dict[str, Any],
    job: dict[str, Any],
    generation: Path,
    environment_lock: dict[str, Any],
    manifests: dict[str, dict[str, Any]],
    cases: dict[str, dict[str, Any]],
    raw_inventory: list[dict[str, Any]],
) -> dict[str, Any]:
    fingerprints = contract_fingerprints(contract)
    projection_path = attempt / contract["projection"]["file"]
    gate_path = attempt / contract["gate_result"]["file"]
    case_index = _case_index(attempt=attempt, cases=cases)
    case_index["attempt"] = {
        "leg_id": "attempt",
        "lane": "static",
        "size": "not_applicable",
        "render": "not_applicable",
        "repeat": 1,
        "status": "valid",
        "path": ".",
        "artifact_count": len(raw_inventory),
        "directory_sha256": directory_digest(raw_inventory),
        "inventory_scope": "raw-artifacts",
    }
    return {
        "schema_version": 1,
        "status": "valid",
        "contract_id": contract["contract_id"],
        "stage_id": job["stage_id"],
        "attempt_id": job["attempt_id"],
        "subject_commit": job["subject_commit"],
        "prerequisite_commits": job["prerequisite_commits"],
        "source_fingerprint": job["source_checks"][0]["fingerprint"],
        **fingerprints,
        "environment_lock": {
            "path": os.path.relpath(generation / "environment-lock.json", attempt),
            "sha256": sha256(generation / "environment-lock.json"),
            "value": environment_lock,
        },
        "job_sha256": sha256(attempt / "job.json"),
        "tooling": job["tooling"],
        "binaries": {
            "capture_sha256": manifests["capture"]["binary"]["sha256"],
            "memory_sha256": manifests["memory"]["binary"]["sha256"],
        },
        "projection": {
            "path": projection_path.relative_to(attempt).as_posix(),
            "sha256": sha256(projection_path),
        },
        "gate_results": {
            "path": gate_path.relative_to(attempt).as_posix(),
            "sha256": sha256(gate_path),
        },
        "cases": case_index,
        "raw_artifacts": raw_inventory,
        "raw_directory_sha256": directory_digest(raw_inventory),
        "finalized_at": datetime.now(UTC).isoformat(),
    }


def _baseline_stage_entry(
    *, attempt: Path, manifest: dict[str, Any]
) -> dict[str, Any]:
    baseline_root = attempt.parent.parent.parent
    attempt_prefix = attempt.relative_to(baseline_root).as_posix()

    def rooted(locator: dict[str, Any]) -> dict[str, Any]:
        return {
            **locator,
            "path": f"{attempt_prefix}/{locator['path']}",
        }

    cases = {
        case_id: {
            **case,
            "path": f"{attempt_prefix}/{case['path']}",
        }
        for case_id, case in manifest["cases"].items()
    }
    return {
        "status": "valid",
        "attempt_id": manifest["attempt_id"],
        "subject_commit": manifest["subject_commit"],
        "prerequisite_commits": manifest["prerequisite_commits"],
        "source_fingerprint": manifest["source_fingerprint"],
        "measurement_contract_sha256": manifest["measurement_contract_sha256"],
        "fixture_contract_sha256": manifest["fixture_contract_sha256"],
        "attempt_manifest": {
            "path": f"{attempt_prefix}/attempt-manifest.json",
            "sha256": sha256(attempt / "attempt-manifest.json"),
        },
        "projection": rooted(manifest["projection"]),
        "gate_results": rooted(manifest["gate_results"]),
        "cases": cases,
    }


def _atomic_write_text(path: Path, value: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(value)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def _registered_files(
    baseline_root: Path, index: dict[str, Any] | None = None
) -> list[Path]:
    baseline_root = baseline_root.resolve()
    index_path = baseline_root / "baseline-index.json"
    index = read_json_object(index_path) if index is None else index
    stages = index.get("stages")
    if not isinstance(stages, dict) or not stages:
        raise RuntimeError("baseline index has no registered stages")
    files = {index_path}
    for stage_id, stage_entry in stages.items():
        if not isinstance(stage_id, str) or not isinstance(stage_entry, dict):
            raise RuntimeError("baseline index stage entry is invalid")
        locator = stage_entry.get("attempt_manifest")
        if not isinstance(locator, dict) or set(locator) != {"path", "sha256"}:
            raise RuntimeError("baseline attempt manifest locator is invalid")
        attempt_manifest = _relative_file(locator["path"], root=baseline_root)
        if (
            not attempt_manifest.is_file()
            or attempt_manifest.name != "attempt-manifest.json"
            or sha256(attempt_manifest) != locator["sha256"]
        ):
            raise RuntimeError("registered attempt manifest is missing or changed")
        attempt = attempt_manifest.parent
        for row in directory_inventory(attempt, relative_to=baseline_root):
            files.add(_relative_file(row["path"], root=baseline_root))
        manifest = read_json_object(attempt_manifest)
        environment = manifest.get("environment_lock")
        if not isinstance(environment, dict) or set(environment) != {
            "path",
            "sha256",
            "value",
        }:
            raise RuntimeError("attempt environment-lock locator is invalid")
        environment_path = (attempt / environment["path"]).resolve()
        try:
            environment_relative = environment_path.relative_to(baseline_root)
        except ValueError as error:
            raise RuntimeError("attempt environment-lock escapes the baseline root") from error
        environment_path = _relative_file(
            environment_relative.as_posix(), root=baseline_root
        )
        if (
            not environment_path.is_file()
            or sha256(environment_path) != environment["sha256"]
        ):
            raise RuntimeError("registered environment-lock is missing or changed")
        files.add(environment_path)
    return sorted(files)


def _checksum_text(
    baseline_root: Path, index: dict[str, Any] | None = None
) -> str:
    rows = []
    for path in _registered_files(baseline_root, index):
        rows.append(f"{sha256(path)}  {path.relative_to(baseline_root).as_posix()}")
    return "\n".join(rows) + "\n"


def finalize_attempt(attempt: Path) -> dict[str, Any]:
    try:
        import fcntl
    except ImportError as error:
        raise RuntimeError(
            "formal RtT-light registration currently requires POSIX file locking"
        ) from error
    (
        contract,
        job,
        generation,
        baseline_root,
        environment_lock,
        manifests,
        cases,
    ) = collect_attempt_evidence(attempt)
    attempt = attempt.resolve()
    _validate_attempt_file_set(attempt, leg_order=job["leg_order"], finalized=False)
    projection_rows = build_projection_rows(contract, job["stage_id"], cases)
    gate_rows = build_gate_result_rows(contract, job["stage_id"], cases)
    raw_inventory = _raw_attempt_inventory(attempt, finalized=False)

    data_dir = attempt / "data"
    if data_dir.exists() or (attempt / "attempt-manifest.json").exists():
        raise RuntimeError("attempt already contains finalized ledger artifacts")
    write_csv_exclusive(
        data_dir / Path(contract["projection"]["file"]).name,
        columns=[column["name"] for column in contract["projection"]["columns"]],
        rows=projection_rows,
    )
    write_csv_exclusive(
        data_dir / Path(contract["gate_result"]["file"]).name,
        columns=contract["gate_result"]["columns"],
        rows=gate_rows,
    )
    manifest = _attempt_manifest(
        attempt=attempt,
        contract=contract,
        job=job,
        generation=generation,
        environment_lock=environment_lock,
        manifests=manifests,
        cases=cases,
        raw_inventory=raw_inventory,
    )
    descriptor = os.open(
        attempt / "attempt-manifest.json", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644
    )
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())

    lock_path = Path("/tmp") / f"hell-workers-{contract['contract_id']}-baseline.lock"
    lock_descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o600)
    try:
        fcntl.flock(lock_descriptor, fcntl.LOCK_EX)
        index_path = baseline_root / "baseline-index.json"
        checksum_path = baseline_root / "SHA256SUMS"
        transaction_path = baseline_root / ".registration-in-progress"
        if transaction_path.exists():
            raise RuntimeError("an interrupted baseline registration marker exists")
        previous_index = index_path.read_bytes() if index_path.exists() else None
        previous_checksum = checksum_path.read_bytes() if checksum_path.exists() else None
        descriptor = os.open(
            transaction_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600
        )
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(f"{job['stage_id']} {job['attempt_id']}\n")
            handle.flush()
            os.fsync(handle.fileno())
        try:
            if index_path.exists():
                index = read_json_object(index_path)
            else:
                index = {
                    "schema_version": BASELINE_INDEX_SCHEMA_VERSION,
                    "contract_id": contract["contract_id"],
                    **contract_fingerprints(contract),
                    "stages": {},
                }
            expected_index_keys = {
                "schema_version",
                "contract_id",
                "measurement_contract_sha256",
                "fixture_contract_sha256",
                "stages",
            }
            if (
                set(index) != expected_index_keys
                or index["schema_version"] != BASELINE_INDEX_SCHEMA_VERSION
                or index["contract_id"] != contract["contract_id"]
                or {
                    key: index[key]
                    for key in (
                        "measurement_contract_sha256",
                        "fixture_contract_sha256",
                    )
                }
                != contract_fingerprints(contract)
                or not isinstance(index["stages"], dict)
            ):
                raise RuntimeError("baseline-index.json differs from schema v1")
            if job["stage_id"] in index["stages"]:
                raise RuntimeError(
                    f"baseline stage {job['stage_id']} is already registered"
                )
            index["stages"][job["stage_id"]] = _baseline_stage_entry(
                attempt=attempt, manifest=manifest
            )
            atomic_write_json(index_path, index)
            _atomic_write_text(checksum_path, _checksum_text(baseline_root, index))
            transaction_path.unlink()
            verify_attempt(attempt)
        except BaseException:
            if previous_index is None:
                index_path.unlink(missing_ok=True)
            else:
                _atomic_write_text(index_path, previous_index.decode("utf-8"))
            if previous_checksum is None:
                checksum_path.unlink(missing_ok=True)
            else:
                _atomic_write_text(checksum_path, previous_checksum.decode("utf-8"))
            transaction_path.unlink(missing_ok=True)
            raise
    finally:
        fcntl.flock(lock_descriptor, fcntl.LOCK_UN)
        os.close(lock_descriptor)
    return manifest


def _verify_attempt_manifest(
    *,
    attempt: Path,
    contract: dict[str, Any],
    job: dict[str, Any],
    generation: Path,
    baseline_root: Path,
    environment_lock: dict[str, Any],
    manifests: dict[str, dict[str, Any]],
    cases: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    manifest = read_json_object(attempt / "attempt-manifest.json")
    volatile = manifest.get("finalized_at")
    if not isinstance(volatile, str) or not volatile:
        raise RuntimeError("attempt manifest finalized_at is invalid")
    raw_inventory = _raw_attempt_inventory(attempt, finalized=True)
    expected = _attempt_manifest(
        attempt=attempt,
        contract=contract,
        job=job,
        generation=generation,
        environment_lock=environment_lock,
        manifests=manifests,
        cases=cases,
        raw_inventory=raw_inventory,
    )
    expected["finalized_at"] = volatile
    if manifest != expected:
        raise RuntimeError("attempt-manifest.json differs from regenerated evidence")
    return manifest


def verify_attempt(attempt: Path) -> dict[str, Any]:
    (
        contract,
        job,
        generation,
        baseline_root,
        environment_lock,
        manifests,
        cases,
    ) = collect_attempt_evidence(attempt)
    if (baseline_root / ".registration-in-progress").exists():
        raise RuntimeError("baseline registration is incomplete")
    attempt = attempt.resolve()
    _validate_attempt_file_set(attempt, leg_order=job["leg_order"], finalized=True)
    projection_columns = [
        column["name"] for column in contract["projection"]["columns"]
    ]
    projection_path = attempt / contract["projection"]["file"]
    observed_projection = read_exact_csv(projection_path, projection_columns)
    expected_projection = build_projection_rows(contract, job["stage_id"], cases)
    if observed_projection != expected_projection:
        raise RuntimeError("stored migration projection differs from raw evidence")
    gate_path = attempt / contract["gate_result"]["file"]
    observed_gates = read_exact_csv(gate_path, contract["gate_result"]["columns"])
    expected_gates = build_gate_result_rows(contract, job["stage_id"], cases)
    if observed_gates != expected_gates:
        raise RuntimeError("stored gate results differ from raw evidence")
    manifest = _verify_attempt_manifest(
        attempt=attempt,
        contract=contract,
        job=job,
        generation=generation,
        baseline_root=baseline_root,
        environment_lock=environment_lock,
        manifests=manifests,
        cases=cases,
    )
    index = read_json_object(baseline_root / "baseline-index.json")
    stage_entry = index.get("stages", {}).get(job["stage_id"])
    if stage_entry != _baseline_stage_entry(attempt=attempt, manifest=manifest):
        raise RuntimeError("baseline index stage entry differs from the valid attempt")
    checksum_path = baseline_root / "SHA256SUMS"
    try:
        checksum_text = checksum_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise RuntimeError(f"cannot read SHA256SUMS: {error}") from error
    if checksum_text != _checksum_text(baseline_root):
        raise RuntimeError("SHA256SUMS differs from the registered baseline tree")
    _verify_stage_gate_locators(
        baseline_root=baseline_root,
        index=index,
        stage_id=job["stage_id"],
        attempt=attempt,
        manifest=manifest,
        gate_rows=observed_gates,
    )
    return manifest


def resolve_baseline_locator(
    baseline_root: Path, index: dict[str, Any], locator: str
) -> Any:
    contract_id = index.get("contract_id")
    prefix = f"{contract_id}/baseline-index.json#"
    if not isinstance(locator, str) or not locator.startswith(prefix):
        raise RuntimeError(f"baseline locator has the wrong index prefix: {locator!r}")
    pointer = locator.removeprefix(prefix)
    if not pointer.startswith("/"):
        raise RuntimeError(f"baseline locator is not an absolute JSON pointer: {locator!r}")
    value: Any = index
    for encoded in pointer[1:].split("/"):
        token = encoded.replace("~1", "/").replace("~0", "~")
        if isinstance(value, dict) and token in value:
            value = value[token]
        elif isinstance(value, list) and token.isdigit() and int(token) < len(value):
            value = value[int(token)]
        else:
            raise RuntimeError(f"baseline locator does not resolve: {locator}")
    return value


def _verify_case_entry(
    *,
    baseline_root: Path,
    attempt: Path,
    manifest: dict[str, Any],
    case_id: str,
    entry: Any,
) -> None:
    expected = manifest["cases"].get(case_id)
    if not isinstance(entry, dict) or expected is None:
        raise RuntimeError(f"baseline locator case {case_id} is missing")
    attempt_prefix = attempt.relative_to(baseline_root).as_posix()
    expected_rooted = {
        **expected,
        "path": f"{attempt_prefix}/{expected['path']}",
    }
    if entry != expected_rooted:
        raise RuntimeError(f"baseline locator case {case_id} differs from its attempt")
    artifact = _relative_file(entry["path"], root=baseline_root)
    if case_id == "attempt":
        if (
            artifact != attempt
            or entry.get("inventory_scope") != "raw-artifacts"
            or entry.get("artifact_count") != len(manifest["raw_artifacts"])
            or entry.get("directory_sha256") != manifest["raw_directory_sha256"]
        ):
            raise RuntimeError("attempt pseudo-case differs from its raw artifact ledger")
        return
    inventory = directory_inventory(artifact, relative_to=attempt)
    if (
        entry.get("artifact_count") != len(inventory)
        or entry.get("directory_sha256") != directory_digest(inventory)
    ):
        raise RuntimeError(f"baseline case {case_id} directory digest differs")


def _verify_stage_gate_locators(
    *,
    baseline_root: Path,
    index: dict[str, Any],
    stage_id: str,
    attempt: Path,
    manifest: dict[str, Any],
    gate_rows: list[dict[str, str]],
) -> None:
    prefix = f"{index['contract_id']}/baseline-index.json#/stages/"
    checked: set[str] = set()
    for row in gate_rows:
        for field in ("subject_artifact", "reference_artifact"):
            locator = row[field]
            if not locator:
                continue
            if locator in checked:
                continue
            if not locator.startswith(prefix):
                raise RuntimeError(f"gate {field} is not a canonical baseline locator")
            parts = locator.removeprefix(prefix).split("/")
            if len(parts) != 3 or parts[1] != "cases" or not all(parts):
                raise RuntimeError(f"gate {field} does not target one case entry")
            locator_stage, _, case_id = parts
            entry = resolve_baseline_locator(baseline_root, index, locator)
            stage_entry = index.get("stages", {}).get(locator_stage)
            if not isinstance(stage_entry, dict):
                raise RuntimeError(f"gate locator stage {locator_stage} is unregistered")
            locator_manifest_path = _relative_file(
                stage_entry["attempt_manifest"]["path"], root=baseline_root
            )
            locator_manifest = read_json_object(locator_manifest_path)
            locator_attempt = locator_manifest_path.parent
            _verify_case_entry(
                baseline_root=baseline_root,
                attempt=locator_attempt,
                manifest=locator_manifest,
                case_id=case_id,
                entry=entry,
            )
            if field == "subject_artifact" and locator_stage != stage_id:
                raise RuntimeError("subject gate locator targets a different stage")
            if field == "subject_artifact" and locator_attempt != attempt:
                raise RuntimeError("subject gate locator targets a different attempt")
            checked.add(locator)


def verify_baseline(baseline_root: Path) -> dict[str, Any]:
    baseline_root = baseline_root.resolve()
    require_persistent_output(baseline_root)
    if (baseline_root / ".registration-in-progress").exists():
        raise RuntimeError("baseline registration is incomplete")
    index = read_json_object(baseline_root / "baseline-index.json")
    expected_keys = {
        "schema_version",
        "contract_id",
        "measurement_contract_sha256",
        "fixture_contract_sha256",
        "stages",
    }
    if set(index) != expected_keys or index["schema_version"] != BASELINE_INDEX_SCHEMA_VERSION:
        raise RuntimeError("baseline-index.json differs from schema v1")
    contract = load_rtt_light_contract(index["contract_id"])
    if {
        key: index[key]
        for key in ("measurement_contract_sha256", "fixture_contract_sha256")
    } != contract_fingerprints(contract):
        raise RuntimeError("baseline index contract fingerprints differ")
    stages = index["stages"]
    if not isinstance(stages, dict) or not stages:
        raise RuntimeError("baseline index has no stages")
    verified: dict[str, str] = {}
    for stage_id, stage_entry in stages.items():
        if stage_id not in contract["stages"] or not isinstance(stage_entry, dict):
            raise RuntimeError(f"baseline stage {stage_id!r} is invalid")
        manifest_path = _relative_file(
            stage_entry["attempt_manifest"]["path"], root=baseline_root
        )
        manifest = verify_attempt(manifest_path.parent)
        if manifest["stage_id"] != stage_id:
            raise RuntimeError("baseline stage id differs from its attempt manifest")
        verified[stage_id] = manifest["attempt_id"]
    checksum_path = baseline_root / "SHA256SUMS"
    observed_checksum = checksum_path.read_text(encoding="utf-8")
    expected_checksum = _checksum_text(baseline_root, index)
    if observed_checksum != expected_checksum:
        raise RuntimeError("baseline checksum ledger differs from registered payload")
    return {
        "schema_version": BASELINE_INDEX_SCHEMA_VERSION,
        "status": "valid",
        "contract_id": index["contract_id"],
        "stages": verified,
        "registered_file_count": len(_registered_files(baseline_root, index)),
        "sha256sums_sha256": sha256(checksum_path),
    }
