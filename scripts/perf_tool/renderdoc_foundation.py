"""RenderDoc measurement foundation: state machine, capsules, and validators."""

from __future__ import annotations

import errno
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Final, Iterable, Literal, IO

try:
    from build_coordination import acquire_activity
except ModuleNotFoundError:
    from scripts.build_coordination import acquire_activity

from .artifacts import sha256 as file_sha256

RENDERDOC_REQUESTED_API_VERSION: Final = "1.6.0"
RUNTIME_CHECKPOINT_SCHEMA_V3: Final = 3
RUNTIME_CHECKPOINT_SCHEMA_V4: Final = 4
ENVIRONMENT_LOCK_SCHEMA_V2: Final = 2
CAPSULE_SCHEMA_VERSION: Final = 1

DIAGNOSTIC_NAMESPACE = "target/native-acceptance/renderdoc-foundation"
FORMAL_RENDERDOC_REL = "renderdoc"
FORMAL_RENDERDOC_FAILED_REL = "renderdoc-failed"

FoundationState = Literal[
    "static_preflight_valid",
    "build_valid",
    "candidate_sealed",
    "native_canary_valid",
    "rd0_valid",
    "final_sealed",
    "formal_captured",
    "replay_valid",
    "render_gate_valid",
    "published",
    "static_preflight_failed",
    "build_capsule_failed",
    "native_canary_failed",
    "renderdoc_runtime_failed",
    "replay_integrity_failed",
    "render_gate_failed",
    "publish_failed",
    "formal_timeout",
    "application_checkpoint_invalid",
    "diagnostic_only",
]

PUBLISHABLE_STATES: Final[frozenset[str]] = frozenset({"render_gate_valid"})

FOUNDATION_TRANSITIONS: Final[dict[str, frozenset[str]]] = {
    "static_preflight_valid": frozenset(
        {"build_valid", "static_preflight_failed"}
    ),
    "build_valid": frozenset({"candidate_sealed", "build_capsule_failed"}),
    "candidate_sealed": frozenset({"native_canary_valid", "native_canary_failed"}),
    "native_canary_valid": frozenset({"rd0_valid", "renderdoc_runtime_failed"}),
    "rd0_valid": frozenset({"final_sealed", "replay_integrity_failed"}),
    "final_sealed": frozenset({"formal_captured", "formal_timeout"}),
    "formal_captured": frozenset({"replay_valid", "replay_integrity_failed"}),
    "replay_valid": frozenset({"render_gate_valid", "render_gate_failed"}),
    "render_gate_valid": frozenset({"published", "publish_failed"}),
    "static_preflight_failed": frozenset({"diagnostic_only"}),
    "build_capsule_failed": frozenset({"diagnostic_only"}),
    "native_canary_failed": frozenset({"diagnostic_only"}),
    "application_checkpoint_invalid": frozenset({"diagnostic_only"}),
    "renderdoc_runtime_failed": frozenset({"diagnostic_only"}),
    "formal_timeout": frozenset({"diagnostic_only"}),
    "replay_integrity_failed": frozenset({"diagnostic_only"}),
    "render_gate_failed": frozenset({"diagnostic_only"}),
    "publish_failed": frozenset({"diagnostic_only"}),
    "diagnostic_only": frozenset(),
    "published": frozenset(),
}

FailureClass = Literal[
    "static_preflight_failed",
    "build_capsule_failed",
    "native_canary_failed",
    "application_checkpoint_invalid",
    "renderdoc_runtime_failed",
    "formal_timeout",
    "replay_integrity_failed",
    "render_gate_failed",
    "publish_failed",
]

FAILURE_POLICY: Final[dict[str, dict[str, Any]]] = {
    "static_preflight_failed": {
        "owner": "harness / environment",
        "production_change": False,
        "retry": "input_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "build_capsule_failed": {
        "owner": "build / source",
        "production_change": False,
        "retry": "source_or_tool_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "native_canary_failed": {
        "owner": "application / renderer",
        "production_change": "diagnostic_only",
        "retry": "after_root_cause",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "application_checkpoint_invalid": {
        "owner": "application",
        "production_change": False,
        "retry": "checkpoint_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "renderdoc_runtime_failed": {
        "owner": "RenderDoc integration",
        "production_change": False,
        "retry": "tool_or_bridge_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "formal_timeout": {
        "owner": "launcher",
        "production_change": False,
        "retry": "after_cleanup_evidence",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "replay_integrity_failed": {
        "owner": "extractor / tool",
        "production_change": False,
        "retry": "parser_or_tool_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "render_gate_failed": {
        "owner": "stage contract",
        "production_change": False,
        "retry": "expected_topology_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
    "publish_failed": {
        "owner": "bundle / registry",
        "production_change": False,
        "retry": "registry_fix_only",
        "namespace": DIAGNOSTIC_NAMESPACE,
    },
}

# Runtime evidence is collected before arming the capture, so capture and
# replay retain bounded independent deadlines.
CAPTURE_CHILD_DEADLINE_SECONDS: Final = 600
REPLAY_CHILD_DEADLINE_SECONDS: Final = 600
RD0_OUTER_DEADLINE_SECONDS: Final = 1920
FORMAL_RENDERDOC_OUTER_DEADLINE_SECONDS: Final = 1320
PROCESS_TERM_GRACE_SECONDS: Final = 5

_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
_RENDERDOC_LOG_PROBLEM_RE = re.compile(
    r"\b(?:WARN(?:ING)?|ERROR|FATAL|CRITICAL|panicked)\b|bevy_ecs::error::handler",
    re.IGNORECASE,
)
_ICU4X_JAPANESE_FALLBACK = (
    "ICU4X data error: No segmentation model for language: ja"
)
_QRENDERDOC_GTK_THEME_WARNING_RE = re.compile(
    r"^\(qrenderdoc:\d+\): Gtk-WARNING \*\*: \d{2}:\d{2}:\d{2}\.\d{3}: "
    r"Theme parsing error: <data>:1:0: Expected a valid selector$"
)


@dataclass(frozen=True)
class ParsedApiVersion:
    major: int
    minor: int
    patch: int

    def as_string(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


def parse_api_version(value: str) -> ParsedApiVersion:
    parts = value.strip().split(".")
    if len(parts) != 3:
        raise ValueError(f"invalid RenderDoc API version: {value!r}")
    try:
        major, minor, patch = (int(part) for part in parts)
    except ValueError as error:
        raise ValueError(f"invalid RenderDoc API version: {value!r}") from error
    if major < 0 or minor < 0 or patch < 0:
        raise ValueError(f"invalid RenderDoc API version: {value!r}")
    return ParsedApiVersion(major, minor, patch)


def accepts_renderdoc_api_version(returned: str, *, requested: str = RENDERDOC_REQUESTED_API_VERSION) -> bool:
    observed = parse_api_version(returned)
    baseline = parse_api_version(requested)
    if observed.major != 1 or baseline.major != 1:
        return False
    return (observed.major, observed.minor, observed.patch) >= (
        baseline.major,
        baseline.minor,
        baseline.patch,
    )


def transition_foundation_state(current: FoundationState, next_state: FoundationState) -> None:
    allowed = FOUNDATION_TRANSITIONS.get(current)
    if allowed is None or next_state not in allowed:
        raise ValueError(
            f"illegal foundation transition {current!r} -> {next_state!r}"
        )


def assert_publish_allowed(state: FoundationState) -> None:
    if state not in PUBLISHABLE_STATES:
        raise ValueError(f"publish is not allowed from state {state!r}")


def failure_policy(failure_class: FailureClass) -> dict[str, Any]:
    policy = FAILURE_POLICY.get(failure_class)
    if policy is None:
        raise ValueError(f"unknown failure class {failure_class!r}")
    return dict(policy)


def is_sha256(value: Any) -> bool:
    return isinstance(value, str) and _SHA256_RE.fullmatch(value) is not None


def is_known_nonfatal_renderdoc_log_line(line: str) -> bool:
    """Return true only for exact upstream diagnostics proven non-fatal.

    ICU4X emits the Japanese line when Parley's non-complex line segmenter
    intentionally falls back to the end of the grapheme slice.  QRenderDoc may
    emit the GTK theme line after a successful replay and extraction.  Neither
    classification accepts a prefix, suffix, altered language, or other GTK
    warning.
    """
    return (
        line == _ICU4X_JAPANESE_FALLBACK
        or _QRENDERDOC_GTK_THEME_WARNING_RE.fullmatch(line) is not None
    )


def classify_renderdoc_log_lines(
    text: str, allow_patterns: Iterable[str]
) -> tuple[list[str], list[str]]:
    """Return (known non-fatal, unexpected) RenderDoc warning/error lines."""
    compiled_allow = [re.compile(pattern) for pattern in allow_patterns]
    known_nonfatal: list[str] = []
    unexpected: list[str] = []
    for line in text.splitlines():
        if _RENDERDOC_LOG_PROBLEM_RE.search(line) is None:
            continue
        if any(pattern.search(line) for pattern in compiled_allow):
            continue
        if is_known_nonfatal_renderdoc_log_line(line):
            known_nonfatal.append(line)
        else:
            unexpected.append(line)
    return known_nonfatal, unexpected


def logical_capsule_id(*, leg: str, features: str, binary_sha256: str) -> str:
    if not is_sha256(binary_sha256):
        raise ValueError("binary_sha256 must be a lowercase sha256 hex digest")
    return hashlib.sha256(f"{leg}:{features}:{binary_sha256}".encode()).hexdigest()


@dataclass(frozen=True)
class BinaryCapsuleManifest:
    schema_version: int
    capsule_id: str
    leg: str
    profile: str
    features: str
    cargo_lock_sha256: str
    binary_sha256: str
    build_fingerprint: str
    logical_locator: str
    resolved_path_diagnostic: str

    def to_json(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "capsule_id": self.capsule_id,
            "leg": self.leg,
            "profile": self.profile,
            "features": self.features,
            "cargo_lock_sha256": self.cargo_lock_sha256,
            "binary_sha256": self.binary_sha256,
            "build_fingerprint": self.build_fingerprint,
            "logical_locator": self.logical_locator,
            "resolved_path_diagnostic": self.resolved_path_diagnostic,
        }


def build_fingerprint(*, rustc_version: str, linker: str, env_allowlist: dict[str, str]) -> str:
    payload = json.dumps(
        {"rustc": rustc_version, "linker": linker, "env": env_allowlist},
        sort_keys=True,
        separators=(",", ":"),
    )
    return hashlib.sha256(payload.encode()).hexdigest()


def copy_binary_capsule(
    *,
    source_binary: Path,
    capsule_root: Path,
    leg: str,
    profile: str,
    features: str,
    cargo_lock_path: Path,
    rustc_version: str,
    linker: str,
    env_allowlist: dict[str, str],
) -> BinaryCapsuleManifest:
    if not source_binary.is_file() or source_binary.is_symlink():
        raise RuntimeError(f"capsule source is not a regular file: {source_binary}")
    capsule_root.mkdir(parents=True, exist_ok=True)
    destination = capsule_root / "bevy_app"
    if destination.exists():
        raise RuntimeError(f"capsule destination already exists: {destination}")
    shutil.copy2(source_binary, destination)
    os.chmod(destination, stat.S_IMODE(destination.stat().st_mode) & ~stat.S_IWUSR)
    digest = file_sha256(destination)
    capsule_id = logical_capsule_id(leg=leg, features=features, binary_sha256=digest)
    manifest = BinaryCapsuleManifest(
        schema_version=CAPSULE_SCHEMA_VERSION,
        capsule_id=capsule_id,
        leg=leg,
        profile=profile,
        features=features,
        cargo_lock_sha256=file_sha256(cargo_lock_path),
        binary_sha256=digest,
        build_fingerprint=build_fingerprint(
            rustc_version=rustc_version,
            linker=linker,
            env_allowlist=env_allowlist,
        ),
        logical_locator=f"capsule/{leg}/{capsule_id}/bevy_app",
        resolved_path_diagnostic=str(destination.resolve()),
    )
    manifest_path = capsule_root / "capsule-manifest.json"
    manifest_path.write_text(
        json.dumps(manifest.to_json(), ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.chmod(manifest_path, stat.S_IMODE(manifest_path.stat().st_mode) & ~stat.S_IWUSR)
    return manifest


def verify_capsule_hash(capsule_root: Path) -> BinaryCapsuleManifest:
    manifest_path = capsule_root / "capsule-manifest.json"
    binary_path = capsule_root / "bevy_app"
    if not manifest_path.is_file() or not binary_path.is_file():
        raise RuntimeError(f"incomplete capsule at {capsule_root}")
    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    observed = file_sha256(binary_path)
    if payload.get("binary_sha256") != observed:
        raise RuntimeError("capsule binary hash changed since seal")
    return BinaryCapsuleManifest(
        schema_version=int(payload["schema_version"]),
        capsule_id=str(payload["capsule_id"]),
        leg=str(payload["leg"]),
        profile=str(payload["profile"]),
        features=str(payload["features"]),
        cargo_lock_sha256=str(payload["cargo_lock_sha256"]),
        binary_sha256=str(payload["binary_sha256"]),
        build_fingerprint=str(payload["build_fingerprint"]),
        logical_locator=str(payload["logical_locator"]),
        resolved_path_diagnostic=str(payload["resolved_path_diagnostic"]),
    )


def guarded_build_activity(repo: Path):
    return acquire_activity(repo, "exclusive")


def environment_lock_v2_payload(
    *,
    base: dict[str, Any],
    renderdoc_binary_sha256: str | None = None,
    memory_binary_sha256: str | None = None,
) -> dict[str, Any]:
    payload = dict(base)
    payload["schema_version"] = ENVIRONMENT_LOCK_SCHEMA_V2
    if renderdoc_binary_sha256 is not None:
        if not is_sha256(renderdoc_binary_sha256):
            raise ValueError("renderdoc_binary_sha256 is invalid")
        payload["renderdoc_binary_sha256"] = renderdoc_binary_sha256
    if memory_binary_sha256 is not None:
        if not is_sha256(memory_binary_sha256):
            raise ValueError("memory_binary_sha256 is invalid")
        payload["memory_binary_sha256"] = memory_binary_sha256
    return payload


def validate_environment_lock_schema(lock: dict[str, Any]) -> None:
    version = lock.get("schema_version")
    if version not in {1, ENVIRONMENT_LOCK_SCHEMA_V2}:
        raise ValueError("unsupported environment-lock schema_version")
    if version == ENVIRONMENT_LOCK_SCHEMA_V2:
        for optional in ("renderdoc_binary_sha256", "memory_binary_sha256"):
            if optional in lock and lock[optional] is not None and not is_sha256(lock[optional]):
                raise ValueError(f"environment-lock {optional} is invalid")


def validate_runtime_checkpoint_v3(
    payload: dict[str, Any],
    *,
    contract: dict[str, Any],
    stage_id: str,
    capture_path: Path | None,
    rdc_sha256: str,
    rdc_bytes: int,
) -> None:
    schema_version = payload.get("schema_version")
    if schema_version not in {RUNTIME_CHECKPOINT_SCHEMA_V3, RUNTIME_CHECKPOINT_SCHEMA_V4}:
        raise ValueError("runtime checkpoint schema_version must be 3 or 4")
    if stage_id == "p08" and schema_version != RUNTIME_CHECKPOINT_SCHEMA_V4:
        raise ValueError("P08 runtime checkpoint schema_version must be 4")
    if payload.get("status") != "valid":
        raise ValueError("runtime checkpoint status must be valid")
    required_top = {
        "schema_version",
        "status",
        "contract_id",
        "stage_id",
        "generation",
        "checkpoint",
        "render_inventory",
        "render_resources",
        "fixture",
        "capture_path",
        "requested_renderdoc_api_version",
        "returned_renderdoc_api_version",
        "selector",
        "gpu_ready",
        "capture_artifact",
    }
    if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}:
        required_top.add("p02_presentation")
    if stage_id in {"p04", "p05", "p06", "p07", "p08"}:
        required_top.add("runtime_field")
    if stage_id in {"p06", "p08"}:
        required_top.add("gpu_light_field")
    if stage_id == "p08":
        required_top.add("cross_consumer")
    if set(payload) != required_top:
        raise ValueError("runtime checkpoint v3 has unexpected keys")
    if payload["contract_id"] != contract["contract_id"] or payload["stage_id"] != stage_id:
        raise ValueError("runtime checkpoint contract/stage mismatch")
    if stage_id in {"p02", "p03", "p04", "p05", "p06", "p07", "p08"}:
        presentation = payload["p02_presentation"]
        expected_presentation_keys = {
            "layer_2d_camera_count",
            "layer_2d_pass_count",
            "building_count",
            "duplicate_presentation_count",
            "building_exactly_one_presentation",
            "soul_count",
            "soul_billboard_count",
            "familiar_3d_count",
            "state_and_bounce_probes_pass",
        }
        if not isinstance(presentation, dict) or set(presentation) != expected_presentation_keys:
            raise ValueError("runtime checkpoint P02 presentation schema is invalid")
        for key in expected_presentation_keys - {
            "building_exactly_one_presentation",
            "state_and_bounce_probes_pass",
        }:
            if (
                not isinstance(presentation[key], int)
                or isinstance(presentation[key], bool)
                or presentation[key] < 0
            ):
                raise ValueError(f"runtime checkpoint P02 presentation {key} is invalid")
        for key in (
            "building_exactly_one_presentation",
            "state_and_bounce_probes_pass",
        ):
            if not isinstance(presentation[key], bool):
                raise ValueError(f"runtime checkpoint P02 presentation {key} is invalid")
    if stage_id in {"p04", "p05", "p06", "p07", "p08"}:
        runtime_field = payload["runtime_field"]
        expected_runtime_keys = {
            "typed_emitter_components",
            "eligible_supplied_emitters",
            "indoor_mask_cells",
            "indoor_mask_checksum",
        }
        if schema_version == RUNTIME_CHECKPOINT_SCHEMA_V4:
            expected_runtime_keys |= {"world_epoch", "field_revision", "field_checksum"}
        if not isinstance(runtime_field, dict) or set(runtime_field) != expected_runtime_keys:
            raise ValueError("runtime checkpoint P04 field schema is invalid")
        for key in expected_runtime_keys - {"indoor_mask_checksum", "field_checksum"}:
            value = runtime_field[key]
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise ValueError(f"runtime checkpoint P04 field {key} is invalid")
        checksum = runtime_field["indoor_mask_checksum"]
        if not isinstance(checksum, str) or len(checksum) != 64 or any(
            character not in "0123456789abcdef" for character in checksum
        ):
            raise ValueError("runtime checkpoint P04 mask checksum is invalid")
        if schema_version == RUNTIME_CHECKPOINT_SCHEMA_V4:
            field_checksum = runtime_field["field_checksum"]
            if not isinstance(field_checksum, str) or len(field_checksum) != 64 or any(
                character not in "0123456789abcdef" for character in field_checksum
            ):
                raise ValueError("runtime checkpoint field checksum is invalid")
    if stage_id in {"p06", "p08"}:
        gpu_field = payload["gpu_light_field"]
        expected_gpu_keys = {
            "schema_version", "availability", "field_image_count", "field_handle_count",
            "logical_payload_bytes", "staging_bytes", "upload_count",
            "uploads_per_changed_revision", "changed_revision_samples", "steady_updates",
            "steady_uploads", "steady_scoped_allocation_events",
            "steady_scoped_allocation_bytes", "upload_allocation_events",
            "upload_allocation_bytes", "old_epoch_uploads", "uploaded_epoch", "gpu_checksum",
            "receiver_pipeline_count", "receiver_material_count", "receiver_binding_count",
            "shared_field_image", "point_light_count_increment", "spot_light_count_increment",
            "shadow_map_count_increment", "local_light_pass_increment", "mask_pass_count",
            "duplicate_2d_pass_count", "cpu_golden_vectors_pass", "pixel_probes_pass",
            "field_texture_label", "field_width", "field_height", "pixel_probe_x",
            "pixel_probe_y", "pixel_probe_expected_rgba",
        }
        if schema_version == RUNTIME_CHECKPOINT_SCHEMA_V4:
            expected_gpu_keys.add("uploaded_revision")
        if not isinstance(gpu_field, dict) or set(gpu_field) != expected_gpu_keys:
            raise ValueError("runtime checkpoint P06 GPU field schema is invalid")
        if gpu_field["schema_version"] != 1 or gpu_field["availability"] != "available":
            raise ValueError("runtime checkpoint P06 GPU field is unavailable")
        boolean_keys = {
            "shared_field_image", "cpu_golden_vectors_pass", "pixel_probes_pass"
        }
        for key in boolean_keys:
            if not isinstance(gpu_field[key], bool):
                raise ValueError(f"runtime checkpoint P06 GPU field {key} is invalid")
        integer_keys = expected_gpu_keys - {
            "schema_version", "availability", "gpu_checksum", "field_texture_label",
            "pixel_probe_expected_rgba", *boolean_keys
        }
        for key in integer_keys:
            value = gpu_field[key]
            if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                raise ValueError(f"runtime checkpoint P06 GPU field {key} is invalid")
        checksum = gpu_field["gpu_checksum"]
        if not isinstance(checksum, str) or len(checksum) != 64 or any(
            character not in "0123456789abcdef" for character in checksum
        ):
            raise ValueError("runtime checkpoint P06 GPU checksum is invalid")
        label = gpu_field["field_texture_label"]
        expected_rgba = gpu_field["pixel_probe_expected_rgba"]
        if not isinstance(label, str) or not label:
            raise ValueError("runtime checkpoint P06 texture label is invalid")
        if (
            not isinstance(expected_rgba, list)
            or len(expected_rgba) != 4
            or any(
                not isinstance(value, int)
                or isinstance(value, bool)
                or not 0 <= value <= 255
                for value in expected_rgba
            )
        ):
            raise ValueError("runtime checkpoint P06 pixel probe is invalid")
        if (
            gpu_field["field_width"] <= 0
            or gpu_field["field_height"] <= 0
            or gpu_field["pixel_probe_x"] >= gpu_field["field_width"]
            or gpu_field["pixel_probe_y"] >= gpu_field["field_height"]
        ):
            raise ValueError("runtime checkpoint P06 pixel probe bounds are invalid")
    if stage_id == "p08":
        cross = payload["cross_consumer"]
        expected_cross_keys = {
            "schema_version", "availability", "world_epoch", "field_revision",
            "field_checksum", "gpu_uploaded_epoch", "gpu_uploaded_revision",
            "gpu_checksum", "soul_recovery_steps", "soul_count",
            "soul_sample_count", "soul_effect_count", "soul_mask_or_stale_effects",
            "room_count", "room_state_count", "room_world_epoch_match_count",
            "room_field_revision_match_count", "room_topology_match_count",
            "revision_epoch_consistency",
        }
        if not isinstance(cross, dict) or set(cross) != expected_cross_keys:
            raise ValueError("runtime checkpoint P08 cross-consumer schema is invalid")
        if cross["schema_version"] != 1 or cross["availability"] != "available":
            raise ValueError("runtime checkpoint P08 cross-consumer evidence is unavailable")
        integer_keys = expected_cross_keys - {
            "schema_version", "availability", "field_checksum", "gpu_checksum",
            "revision_epoch_consistency",
        }
        if any(
            not isinstance(cross[key], int) or isinstance(cross[key], bool) or cross[key] < 0
            for key in integer_keys
        ):
            raise ValueError("runtime checkpoint P08 cross-consumer integer is invalid")
        if any(
            not isinstance(cross[key], str)
            or len(cross[key]) != 64
            or any(character not in "0123456789abcdef" for character in cross[key])
            for key in ("field_checksum", "gpu_checksum")
        ):
            raise ValueError("runtime checkpoint P08 cross-consumer checksum is invalid")
        runtime_field = payload["runtime_field"]
        gpu_field = payload["gpu_light_field"]
        expected_counts = contract["fixture"]["sizes"]["medium"]["expected_counts"]
        raw_consistent = (
            cross["world_epoch"] == runtime_field["world_epoch"]
            == cross["gpu_uploaded_epoch"] == gpu_field["uploaded_epoch"]
            and cross["field_revision"] == runtime_field["field_revision"]
            == cross["gpu_uploaded_revision"] == gpu_field["uploaded_revision"]
            and cross["field_checksum"] == runtime_field["field_checksum"]
            == cross["gpu_checksum"] == gpu_field["gpu_checksum"]
            and cross["soul_recovery_steps"] > 0
            and cross["soul_count"] == expected_counts["souls"]
            and cross["soul_sample_count"]
            == cross["soul_count"] * cross["soul_recovery_steps"]
            and cross["soul_effect_count"] <= cross["soul_sample_count"]
            and cross["soul_mask_or_stale_effects"] == 0
            and cross["room_count"] == expected_counts["rooms"]
            and cross["room_state_count"] == cross["room_count"]
            and cross["room_world_epoch_match_count"] == cross["room_count"]
            and cross["room_field_revision_match_count"] == cross["room_count"]
            and cross["room_topology_match_count"] == cross["room_count"]
        )
        if cross["revision_epoch_consistency"] is not True or not raw_consistent:
            raise ValueError("runtime checkpoint P08 cross-consumer facts are inconsistent")
    generation = payload["generation"]
    if not isinstance(generation, int) or isinstance(generation, bool) or generation < 1:
        raise ValueError("runtime checkpoint generation is invalid")
    checkpoint = payload["checkpoint"]
    renderdoc = contract["formal_matrix"]["renderdoc"]
    expected_checkpoint_keys = {
        "name",
        "simulation_tick",
        "simulation_tick_source",
        "settle_frames",
        "ready_frame_ordinal",
        "capture_frame",
        "capture_begin_frame",
        "capture_end_frame",
        "render_frame_index",
        "frame_count_before_capture",
    }
    if not isinstance(checkpoint, dict) or set(checkpoint) != expected_checkpoint_keys:
        raise ValueError("runtime checkpoint.checkpoint schema is invalid")
    if (
        checkpoint["name"] != "indoor-light-fixture-ready-v1"
        or checkpoint["settle_frames"] != renderdoc["settle_frames"]
        or checkpoint["ready_frame_ordinal"] != renderdoc["capture_frame"]
        or checkpoint["capture_frame"] != renderdoc["capture_frame"]
        or checkpoint["capture_begin_frame"] != checkpoint["capture_end_frame"]
        or checkpoint["ready_frame_ordinal"] != checkpoint["capture_frame"]
        or not isinstance(checkpoint["simulation_tick"], int)
        or isinstance(checkpoint["simulation_tick"], bool)
        or checkpoint["simulation_tick"] < 0
        or checkpoint["simulation_tick_source"] != "perf_capture.fixed_update_tick"
        or not isinstance(checkpoint["render_frame_index"], int)
        or isinstance(checkpoint["render_frame_index"], bool)
        or checkpoint["render_frame_index"] < renderdoc["capture_frame"]
        or not isinstance(checkpoint["frame_count_before_capture"], int)
        or isinstance(checkpoint["frame_count_before_capture"], bool)
        or checkpoint["frame_count_before_capture"] < renderdoc["capture_frame"]
    ):
        raise ValueError("runtime checkpoint fixed gate values differ from contract")
    requested = payload["requested_renderdoc_api_version"]
    returned = payload["returned_renderdoc_api_version"]
    if requested != RENDERDOC_REQUESTED_API_VERSION:
        raise ValueError("requested RenderDoc API version mismatch")
    if not accepts_renderdoc_api_version(str(returned), requested=str(requested)):
        raise ValueError("returned RenderDoc API version is incompatible")
    selector = payload["selector"]
    if (
        not isinstance(selector, dict)
        or set(selector)
        != {
            "strategy",
            "device_selector",
            "window_selector",
            "window_count",
            "primary_window",
        }
        or selector["strategy"] != "wgpu_device_null_window"
        or selector["device_selector"]
        != "wgpu::Device::start_graphics_debugger_capture"
        or selector["window_selector"] != "null"
        or selector["window_count"] != 1
        or not isinstance(selector["primary_window"], int)
        or isinstance(selector["primary_window"], bool)
    ):
        raise ValueError("selector strategy is missing or invalid")
    gpu_ready = payload["gpu_ready"]
    if not isinstance(gpu_ready, dict) or set(gpu_ready) != {
        "pre_capture",
        "post_capture",
    }:
        raise ValueError("gpu_ready schema is invalid")
    for label in ("pre_capture", "post_capture"):
        block = gpu_ready.get(label)
        if not isinstance(block, dict):
            raise ValueError(f"gpu_ready.{label} is invalid")
        for key in (
            "pipeline_count",
            "scene_camera_count",
            "mask_camera_count",
            "window_camera_count",
        ):
            if (
                not isinstance(block.get(key), int)
                or isinstance(block[key], bool)
                or block[key] < 0
            ):
                raise ValueError(f"gpu_ready.{label}.{key} is invalid")
        if set(block) != {
            "pipeline_count",
            "scene_camera_count",
            "mask_camera_count",
            "window_camera_count",
        }:
            raise ValueError(f"gpu_ready.{label} schema is invalid")
    if gpu_ready["pre_capture"] != gpu_ready["post_capture"]:
        raise ValueError("pre/post GPU-ready signatures differ")
    artifact = payload["capture_artifact"]
    if (
        not isinstance(artifact, dict)
        or artifact.get("sha256") != rdc_sha256
        or artifact.get("bytes") != rdc_bytes
        or not is_sha256(artifact.get("sha256", ""))
    ):
        raise ValueError("capture_artifact does not match the sealed .rdc")
    try:
        observed_capture = Path(payload["capture_path"])
    except TypeError as error:
        raise ValueError("capture_path is invalid") from error
    if not observed_capture.is_absolute():
        raise ValueError("capture_path must be absolute")
    if capture_path is not None and observed_capture.resolve() != capture_path.resolve():
        raise ValueError("capture_path does not match the sealed capture file")


def disk_reservation_bytes(
    *,
    max_rdc_bytes: int,
    rd0_rdc_bytes: int,
    post_run_floor_gib: int = 15,
) -> int:
    gib = 1024**3
    floor = post_run_floor_gib * gib
    peak_copy = max(2 * max_rdc_bytes + gib, 2 * rd0_rdc_bytes + gib)
    return max(floor, peak_copy)


def assert_disk_headroom(path: Path, *, required_bytes: int) -> None:
    usage = shutil.disk_usage(path)
    if usage.free < required_bytes:
        raise RuntimeError(
            f"insufficient disk space under {path}: need {required_bytes}, free {usage.free}"
        )


@dataclass
class ProcessRunStatus:
    pid: int
    pgid: int
    returncode: int | None
    signal: str | None
    deadline_reason: str | None
    orphan_probe_count: int


def _terminate_process_group(pgid: int, *, grace_seconds: float) -> None:
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return
    deadline = time.monotonic() + grace_seconds
    while time.monotonic() < deadline:
        try:
            os.killpg(pgid, 0)
        except ProcessLookupError:
            return
        time.sleep(0.05)
    try:
        os.killpg(pgid, signal.SIGKILL)
    except ProcessLookupError:
        return


def run_with_deadline(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None,
    deadline_seconds: float,
    deadline_reason: str,
    stdout: int | IO[bytes] = subprocess.DEVNULL,
    stderr: int | IO[bytes] = subprocess.DEVNULL,
) -> ProcessRunStatus:
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        start_new_session=True,
        stdout=stdout,
        stderr=stderr,
    )
    pgid = os.getpgid(process.pid)
    deadline = time.monotonic() + deadline_seconds
    returncode: int | None = None
    signal_name: str | None = None
    reason: str | None = None
    while True:
        returncode = process.poll()
        if returncode is not None:
            break
        if time.monotonic() >= deadline:
            reason = deadline_reason
            _terminate_process_group(pgid, grace_seconds=PROCESS_TERM_GRACE_SECONDS)
            returncode = process.wait(timeout=30)
            signal_name = "SIGTERM/SIGKILL"
            break
        time.sleep(0.1)
    orphan_probe = 0
    try:
        os.killpg(pgid, 0)
        orphan_probe = 1
    except ProcessLookupError:
        orphan_probe = 0
    if orphan_probe != 0:
        _terminate_process_group(pgid, grace_seconds=0)
    return ProcessRunStatus(
        pid=process.pid,
        pgid=pgid,
        returncode=returncode,
        signal=signal_name,
        deadline_reason=reason,
        orphan_probe_count=orphan_probe,
    )


def read_external_evidence_capsule(path: Path) -> dict[str, Any] | None:
    manifest = path / "manifest.json"
    if not manifest.is_file():
        return None
    payload = json.loads(manifest.read_text(encoding="utf-8"))
    if payload.get("kind") != "external_evidence_capsule":
        raise ValueError("external evidence manifest has wrong kind")
    root = path.resolve()
    if "target" in root.parts:
        raise ValueError("external evidence capsule must not live under target/")
    return payload


def foundation_diagnostic_root(repo: Path) -> Path:
    return (repo / DIAGNOSTIC_NAMESPACE / str(uuid.uuid4())).resolve()


def run_self_test() -> None:
    transition_foundation_state("static_preflight_valid", "build_valid")
    try:
        transition_foundation_state("static_preflight_valid", "published")
    except ValueError:
        pass
    else:
        raise AssertionError("illegal transition unexpectedly succeeded")
    assert accepts_renderdoc_api_version("1.6.0")
    assert accepts_renderdoc_api_version("1.7.2")
    assert not accepts_renderdoc_api_version("1.5.9")
    assert not accepts_renderdoc_api_version("2.0.0")
    known, unexpected = classify_renderdoc_log_lines(
        "\n".join(
            (
                _ICU4X_JAPANESE_FALLBACK,
                "(qrenderdoc:1234): Gtk-WARNING **: 06:55:44.862: "
                "Theme parsing error: <data>:1:0: Expected a valid selector",
                "ERROR actual failure",
            )
        ),
        [],
    )
    assert known == [
        _ICU4X_JAPANESE_FALLBACK,
        "(qrenderdoc:1234): Gtk-WARNING **: 06:55:44.862: "
        "Theme parsing error: <data>:1:0: Expected a valid selector",
    ]
    assert unexpected == ["ERROR actual failure"]
    for near_match in (
        _ICU4X_JAPANESE_FALLBACK + " extra",
        "ICU4X data error: No segmentation model for language: th",
        "(qrenderdoc:1234): Gtk-WARNING **: 06:55:44.862: another warning",
    ):
        assert classify_renderdoc_log_lines(near_match, [])[1] == [near_match]
    try:
        assert_publish_allowed("replay_valid")
    except ValueError:
        pass
    else:
        raise AssertionError("publish from replay_valid unexpectedly allowed")
    assert_publish_allowed("render_gate_valid")
    reservation = disk_reservation_bytes(max_rdc_bytes=512 * 1024**2, rd0_rdc_bytes=256 * 1024**2)
    assert reservation >= 15 * 1024**3
    status = run_with_deadline(
        ["python3", "-c", "import time; time.sleep(0.05)"],
        cwd=Path.cwd(),
        env=os.environ.copy(),
        deadline_seconds=2,
        deadline_reason="self-test",
    )
    if status.returncode != 0 or status.orphan_probe_count != 0:
        raise AssertionError("deadline runner failed in self-test")
    timeout_status = run_with_deadline(
        ["python3", "-c", "import time; time.sleep(2)"],
        cwd=Path.cwd(),
        env=os.environ.copy(),
        deadline_seconds=0.2,
        deadline_reason="timeout-self-test",
    )
    if timeout_status.deadline_reason != "timeout-self-test":
        raise AssertionError("timeout path did not record deadline_reason")
    for failure_class in FAILURE_POLICY:
        policy = failure_policy(failure_class)  # type: ignore[arg-type]
        assert policy["owner"]
        assert "retry" in policy
