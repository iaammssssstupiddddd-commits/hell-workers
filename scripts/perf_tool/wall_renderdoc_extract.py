#!/usr/bin/env python3
"""Replay one wall-density RenderDoc capture and extract its wall draw group.

qrenderdoc owns replay initialisation and shutdown.  This script validates the
Rust runtime checkpoint, then identifies wall draws from the checkpointed mesh
index count and the named Scene RtT color target.  GPU resource IDs are replay
evidence only; they are never used as cross-capture identity.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 1
RUNTIME_CHECKPOINT_SCHEMA_VERSION = 1
CONTRACT_ID = "wall-density-v1"
CONTRACT_SHA256 = "7b32f4e0ecd9cdb9223cde1b7f5aae93460e3ec861b2e3ae0e1eb2e2b87419c8"
CHECKPOINT_NAME = "wall-density-fixture-ready-v1"
SCENE_TARGET_LABEL = "hell-workers-rtt-scene"
CAPTURE_ENV = "HW_RENDERDOC_CAPTURE"
OUTPUT_ENV = "HW_RENDERDOC_EXTRACTION"
CHECKPOINT_ENV = "HW_RENDERDOC_RUNTIME_CHECKPOINT"
FAILURE_ENV = "HW_RENDERDOC_EXTRACTION_FAILURE"


def _required_path(name: str, *, must_exist: bool) -> Path:
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"{name} is required")
    path = Path(value).resolve()
    if must_exist and (not path.is_file() or path.is_symlink()):
        raise RuntimeError(f"{name} is not a regular file: {path}")
    return path


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _is_uint(value: Any, *, positive: bool = False) -> bool:
    return (
        isinstance(value, int)
        and not isinstance(value, bool)
        and value >= (1 if positive else 0)
    )


def _is_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def _validate_checkpoint(value: Any) -> dict[str, Any]:
    required_keys = {
        "schema_version",
        "status",
        "contract_id",
        "stage_id",
        "generation",
        "checkpoint",
        "render_inventory",
        "wall_density",
        "render_resources",
        "fixture",
        "capture_path",
        "requested_renderdoc_api_version",
        "returned_renderdoc_api_version",
        "selector",
        "gpu_ready",
        "capture_artifact",
    }
    if not isinstance(value, dict) or set(value) != required_keys:
        raise RuntimeError("wall runtime checkpoint keys differ from schema v1")
    if (
        value["schema_version"] != RUNTIME_CHECKPOINT_SCHEMA_VERSION
        or value["status"] != "valid"
        or value["contract_id"] != CONTRACT_ID
        or not _is_uint(value["generation"], positive=True)
        or value["requested_renderdoc_api_version"] != "1.6.0"
        or not isinstance(value["returned_renderdoc_api_version"], str)
        or not value["returned_renderdoc_api_version"]
    ):
        raise RuntimeError("wall runtime checkpoint identity differs from schema v1")

    wall = value["wall_density"]
    wall_keys = {
        "schema_version",
        "contract_sha256",
        "layout_checksum",
        "target_size",
        "perf_size",
        "phase",
        "target_wall_count",
        "connector_count",
        "mask_counts",
        "wall_mesh_index_count",
        "mesh_handle_match_count",
        "material_handle_match_count",
        "structural_visual_count",
        "mesh_resident",
        "material_resident",
    }
    if not isinstance(wall, dict) or set(wall) != wall_keys:
        raise RuntimeError("wall runtime evidence keys differ from schema v1")
    expected_by_size = {
        "small": ("N", 96, 192, 6),
        "medium": ("4N", 384, 768, 24),
    }
    expected = expected_by_size.get(wall.get("perf_size"))
    phase = wall.get("phase")
    if expected is None or phase not in {"completed", "provisional"}:
        raise RuntimeError("wall runtime size or phase is invalid")
    target_size, target_count, connector_count, mask_repetitions = expected
    expected_masks = {f"{mask:04b}": mask_repetitions for mask in range(16)}
    if (
        wall["schema_version"] != 1
        or wall["contract_sha256"] != CONTRACT_SHA256
        or not _is_sha256(wall["layout_checksum"])
        or wall["target_size"] != target_size
        or wall["target_wall_count"] != target_count
        or wall["connector_count"] != connector_count
        or wall["mask_counts"] != expected_masks
        or not _is_uint(wall["wall_mesh_index_count"], positive=True)
        or wall["mesh_handle_match_count"] != target_count
        or wall["material_handle_match_count"] != target_count
        or wall["structural_visual_count"] != target_count
        or wall["mesh_resident"] is not True
        or wall["material_resident"] is not True
    ):
        raise RuntimeError("wall runtime evidence differs from its frozen fixture")
    if value["stage_id"] != f"wall-density-{wall['perf_size']}-{phase}":
        raise RuntimeError("wall runtime stage differs from its fixture evidence")

    checkpoint = value["checkpoint"]
    if (
        not isinstance(checkpoint, dict)
        or checkpoint.get("name") != CHECKPOINT_NAME
        or not _is_uint(checkpoint.get("simulation_tick"))
        or checkpoint.get("settle_frames") != 4
        or checkpoint.get("ready_frame_ordinal") != 4
    ):
        raise RuntimeError("wall frame checkpoint differs from schema v1")
    inventory = value["render_inventory"]
    expected_inventory = {
        "scene_target_count": 1,
        "mask_target_count": 0,
        "camera_3d_rtt_count": 1,
        "camera_2d_count": 2,
        "layer_2d_pass_count": 1,
        "soul_proxy_3d": 0,
        "soul_mask_proxy_3d": 0,
        "soul_shadow_proxy_3d": 0,
        "familiar_proxy_3d": 0,
    }
    if inventory != expected_inventory:
        raise RuntimeError("wall render inventory differs from schema v1")
    artifact = value["capture_artifact"]
    if (
        not isinstance(artifact, dict)
        or set(artifact) != {"sha256", "bytes"}
        or not _is_sha256(artifact.get("sha256"))
        or not _is_uint(artifact.get("bytes"), positive=True)
    ):
        raise RuntimeError("wall runtime checkpoint has no capture artifact evidence")
    fixture = value["fixture"]
    if (
        not isinstance(fixture, dict)
        or fixture.get("fixture_checksum") != wall["layout_checksum"]
        or fixture.get("completed_walls")
        != (target_count if phase == "completed" else 0)
    ):
        raise RuntimeError("wall fixture summary differs from wall runtime evidence")
    return value


def _read_checkpoint(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"cannot read wall runtime checkpoint: {error}") from error
    return _validate_checkpoint(value)


def _flatten(actions: Any) -> list[Any]:
    result: list[Any] = []

    def visit(action: Any) -> None:
        result.append(action)
        for child in action.children:
            visit(child)

    for action in actions:
        visit(action)
    return result


def _draw_passes(rd: Any, roots: Any) -> tuple[list[tuple[Any, list[Any]]], list[Any]]:
    flattened = _flatten(roots)
    groups: list[tuple[Any, list[Any]]] = []
    active: tuple[Any, list[Any]] | None = None
    for action in flattened:
        flags = action.flags
        boundary = bool(flags & rd.ActionFlags.CommandBufferBoundary)
        begins = bool(flags & rd.ActionFlags.BeginPass) and not boundary
        ends = bool(flags & rd.ActionFlags.EndPass) and not boundary
        if ends:
            if active is None:
                raise RuntimeError("RenderDoc capture has an unmatched render-pass end")
            begin, draws = active
            if draws:
                groups.append((begin, draws))
            active = None
        if begins:
            if active is not None:
                raise RuntimeError("RenderDoc capture has nested render-pass boundaries")
            active = (action, [])
        if bool(flags & rd.ActionFlags.Drawcall):
            if active is None:
                raise RuntimeError("RenderDoc draw action is outside an explicit render pass")
            active[1].append(action)
    if active is not None:
        raise RuntimeError("RenderDoc capture has an unterminated render pass")
    if not groups:
        raise RuntimeError("capture contains no explicit render pass with draw actions")
    return groups, flattened


def _action_name(action: Any, structured_file: Any) -> str:
    custom = str(action.customName).strip()
    if custom:
        return custom
    try:
        generated = str(action.GetName(structured_file)).strip()
    except Exception:
        generated = ""
    return generated or f"event-{int(action.eventId)}"


def _resource_id(rd: Any, value: Any) -> str | None:
    candidate = getattr(value, "resource", value)
    candidate = getattr(candidate, "object", candidate)
    candidate = getattr(candidate, "resourceId", candidate)
    try:
        if candidate == rd.ResourceId.Null():
            return None
    except Exception:
        pass
    text = str(candidate).strip()
    if not text or text in {"0", "ResourceId::0", "ResourceId()"}:
        return None
    return text


def _scene_target_resource(rd: Any, controller: Any) -> str:
    matches = []
    for resource in controller.GetResources():
        if str(getattr(resource, "name", "")).strip() != SCENE_TARGET_LABEL:
            continue
        resource_id = _resource_id(rd, getattr(resource, "resourceId", resource))
        if resource_id is not None:
            matches.append(resource_id)
    if len(matches) != 1:
        raise RuntimeError(
            f"RenderDoc Scene target label resolved to {len(matches)} resources"
        )
    return matches[0]


def _action_uint(action: Any, field: str) -> int:
    value = getattr(action, field, None)
    if not _is_uint(value):
        raise RuntimeError(f"RenderDoc draw action has no valid {field}")
    return value


def _action_int(action: Any, field: str) -> int:
    value = getattr(action, field, None)
    if not isinstance(value, int) or isinstance(value, bool):
        raise RuntimeError(f"RenderDoc draw action has no valid {field}")
    return value


def _select_wall_draw_group(
    draws: list[dict[str, Any]],
    *,
    scene_target_resource_id: str,
    wall_mesh_index_count: int,
    target_wall_count: int,
) -> dict[str, Any]:
    candidates_by_pass: dict[str, list[dict[str, Any]]] = {}
    for draw in draws:
        if (
            draw["indexed"] is True
            and draw["num_indices"] == wall_mesh_index_count
            and draw["num_instances"] > 0
            and scene_target_resource_id in draw["color_resource_ids"]
            and draw["depth_resource_id"] is not None
            and draw["fragment_shader_present"] is True
        ):
            candidates_by_pass.setdefault(draw["pass_id"], []).append(draw)
    if len(candidates_by_pass) != 1:
        raise RuntimeError(
            "wall draw identity did not resolve to exactly one Scene color+depth pass: "
            f"candidates={sorted(candidates_by_pass)}"
        )
    pass_id, candidates = next(iter(candidates_by_pass.items()))
    rendered_instance_count = sum(draw["num_instances"] for draw in candidates)
    if rendered_instance_count > target_wall_count:
        raise RuntimeError(
            "wall draw group renders more instances than the checkpointed wall owners"
        )
    return {
        "pass_id": pass_id,
        "draw_group_count": len(candidates),
        "rendered_instance_count": rendered_instance_count,
        "checkpointed_owner_count": target_wall_count,
        "wall_mesh_index_count": wall_mesh_index_count,
        "draws": candidates,
    }


def _extract(rd: Any, controller: Any, checkpoint: dict[str, Any]) -> dict[str, Any]:
    properties = controller.GetAPIProperties()
    if properties.pipelineType != rd.GraphicsAPI.Vulkan:
        raise RuntimeError(f"capture API is not Vulkan: {properties.pipelineType}")
    scene_target_resource_id = _scene_target_resource(rd, controller)
    groups, flattened = _draw_passes(rd, controller.GetRootActions())
    structured_file = controller.GetStructuredFile()
    passes: list[dict[str, Any]] = []
    draws: list[dict[str, Any]] = []
    for pass_index, (root, pass_draws) in enumerate(groups, start=1):
        pass_id = f"pass-{pass_index:04d}"
        event_ids = [int(draw.eventId) for draw in pass_draws]
        passes.append(
            {
                "pass_id": pass_id,
                "name": _action_name(root, structured_file),
                "first_event": min(event_ids),
                "last_event": max(event_ids),
                "draw_count": len(pass_draws),
            }
        )
        for draw in pass_draws:
            event_id = int(draw.eventId)
            if event_id <= 0:
                raise RuntimeError("RenderDoc draw action has a nonpositive event id")
            controller.SetFrameEvent(event_id, True)
            pipeline = controller.GetPipelineState()
            color_resource_ids = sorted(
                resource_id
                for descriptor in pipeline.GetOutputTargets()
                if (resource_id := _resource_id(rd, descriptor)) is not None
            )
            depth_resource_id = _resource_id(rd, pipeline.GetDepthTarget())
            fragment_shader_present = (
                pipeline.GetShaderReflection(rd.ShaderStage.Fragment) is not None
            )
            draws.append(
                {
                    "pass_id": pass_id,
                    "event_id": event_id,
                    "indexed": bool(draw.flags & rd.ActionFlags.Indexed),
                    "num_indices": _action_uint(draw, "numIndices"),
                    "num_instances": _action_uint(draw, "numInstances"),
                    "index_offset": _action_uint(draw, "indexOffset"),
                    "vertex_offset": _action_int(draw, "vertexOffset"),
                    "instance_offset": _action_uint(draw, "instanceOffset"),
                    "color_resource_ids": color_resource_ids,
                    "depth_resource_id": depth_resource_id,
                    "fragment_shader_present": fragment_shader_present,
                }
            )
    event_ids = {
        int(action.eventId) for action in flattened if int(action.eventId) > 0
    }
    wall = checkpoint["wall_density"]
    wall_draw_group = _select_wall_draw_group(
        draws,
        scene_target_resource_id=scene_target_resource_id,
        wall_mesh_index_count=wall["wall_mesh_index_count"],
        target_wall_count=wall["target_wall_count"],
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "contract_id": CONTRACT_ID,
        "stage_id": checkpoint["stage_id"],
        "api": "vulkan",
        "capture_sha256": _sha256(_required_path(CAPTURE_ENV, must_exist=True)),
        "runtime_checkpoint_sha256": _sha256(
            _required_path(CHECKPOINT_ENV, must_exist=True)
        ),
        "validated_frames": 1,
        "event_count": len(event_ids),
        "draw_count": len(draws),
        "scene_target": {
            "label": SCENE_TARGET_LABEL,
            "resource_id": scene_target_resource_id,
        },
        "wall_density": wall,
        "passes": passes,
        "wall_draw_group": wall_draw_group,
    }


def _write_json_exclusive(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
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


def self_test() -> int:
    from enum import IntFlag
    from types import SimpleNamespace

    wall = {
        "schema_version": 1,
        "contract_sha256": CONTRACT_SHA256,
        "layout_checksum": "a" * 64,
        "target_size": "N",
        "perf_size": "small",
        "phase": "completed",
        "target_wall_count": 96,
        "connector_count": 192,
        "mask_counts": {f"{mask:04b}": 6 for mask in range(16)},
        "wall_mesh_index_count": 36,
        "mesh_handle_match_count": 96,
        "material_handle_match_count": 96,
        "structural_visual_count": 96,
        "mesh_resident": True,
        "material_resident": True,
    }
    checkpoint = {
        "schema_version": 1,
        "status": "valid",
        "contract_id": CONTRACT_ID,
        "stage_id": "wall-density-small-completed",
        "generation": 1,
        "checkpoint": {
            "name": CHECKPOINT_NAME,
            "simulation_tick": 1,
            "settle_frames": 4,
            "ready_frame_ordinal": 4,
        },
        "render_inventory": {
            "scene_target_count": 1,
            "mask_target_count": 0,
            "camera_3d_rtt_count": 1,
            "camera_2d_count": 2,
            "layer_2d_pass_count": 1,
            "soul_proxy_3d": 0,
            "soul_mask_proxy_3d": 0,
            "soul_shadow_proxy_3d": 0,
            "familiar_proxy_3d": 0,
        },
        "wall_density": wall,
        "render_resources": {"scene_target_label": SCENE_TARGET_LABEL},
        "fixture": {"fixture_checksum": "a" * 64, "completed_walls": 96},
        "capture_path": "/diagnostic/capture.rdc",
        "requested_renderdoc_api_version": "1.6.0",
        "returned_renderdoc_api_version": "1.7.0",
        "selector": {"strategy": "wgpu_device_null_window"},
        "gpu_ready": {"pre_capture": {}, "post_capture": {}},
        "capture_artifact": {"sha256": "b" * 64, "bytes": 1},
    }
    if _validate_checkpoint(checkpoint) is not checkpoint:
        raise RuntimeError("wall checkpoint schema validation regressed")
    bad_masks = {**wall, "mask_counts": {"0000": 96}}
    try:
        _validate_checkpoint({**checkpoint, "wall_density": bad_masks})
    except RuntimeError:
        pass
    else:
        raise RuntimeError("wall checkpoint accepted a nonuniform mask fixture")

    draws = [
        {
            "pass_id": "pass-0001",
            "event_id": 10,
            "indexed": True,
            "num_indices": 36,
            "num_instances": 80,
            "index_offset": 0,
            "vertex_offset": 0,
            "instance_offset": 0,
            "color_resource_ids": ["ResourceId::7"],
            "depth_resource_id": "ResourceId::8",
            "fragment_shader_present": True,
        },
        {
            "pass_id": "pass-0002",
            "event_id": 20,
            "indexed": True,
            "num_indices": 36,
            "num_instances": 16,
            "index_offset": 0,
            "vertex_offset": 0,
            "instance_offset": 80,
            "color_resource_ids": ["ResourceId::7"],
            "depth_resource_id": None,
            "fragment_shader_present": False,
        },
    ]
    selected = _select_wall_draw_group(
        draws,
        scene_target_resource_id="ResourceId::7",
        wall_mesh_index_count=36,
        target_wall_count=96,
    )
    if selected["pass_id"] != "pass-0001" or selected["draw_group_count"] != 1:
        raise RuntimeError("wall main-pass draw selection regressed")

    class ActionFlags(IntFlag):
        BeginPass = 1
        EndPass = 2
        Drawcall = 4
        CommandBufferBoundary = 8
        Indexed = 16

    def action(event_id: int, flags: ActionFlags, children: list[Any] | None = None) -> Any:
        return SimpleNamespace(
            eventId=event_id,
            flags=flags,
            children=[] if children is None else children,
        )

    roots = [
        action(1, ActionFlags.BeginPass, [action(2, ActionFlags.Drawcall)]),
        action(3, ActionFlags.EndPass),
    ]
    rd = SimpleNamespace(ActionFlags=ActionFlags)
    passes, flattened = _draw_passes(rd, roots)
    if len(passes) != 1 or len(flattened) != 3:
        raise RuntimeError("wall render-pass grouping regressed")
    print("wall_renderdoc_extract self-test: PASS")
    return 0


def main() -> int:
    capture = _required_path(CAPTURE_ENV, must_exist=True)
    output = _required_path(OUTPUT_ENV, must_exist=False)
    checkpoint_path = _required_path(CHECKPOINT_ENV, must_exist=True)
    if output.exists():
        raise RuntimeError(f"extraction output already exists: {output}")
    if capture.stat().st_size <= 0:
        raise RuntimeError("capture is empty")
    checkpoint = _read_checkpoint(checkpoint_path)
    if checkpoint["capture_artifact"]["sha256"] != _sha256(capture):
        raise RuntimeError("capture bytes differ from the wall runtime checkpoint")
    if checkpoint["capture_artifact"]["bytes"] != capture.stat().st_size:
        raise RuntimeError("capture size differs from the wall runtime checkpoint")

    import renderdoc as rd

    capture_file = rd.OpenCaptureFile()
    controller = None
    try:
        result = capture_file.OpenFile(str(capture), "", None)
        if result != rd.ResultCode.Succeeded:
            raise RuntimeError(f"cannot open capture: {result}")
        if not capture_file.LocalReplaySupport():
            raise RuntimeError("capture does not support local replay")
        result, controller = capture_file.OpenCapture(rd.ReplayOptions(), None)
        if result != rd.ResultCode.Succeeded:
            raise RuntimeError(f"cannot initialise capture replay: {result}")
        _write_json_exclusive(output, _extract(rd, controller, checkpoint))
    finally:
        if controller is not None:
            controller.Shutdown()
        capture_file.Shutdown()
    return 0


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--self-test"]:
            raise SystemExit(self_test())
        raise SystemExit(main())
    except Exception as error:
        message = f"wall RenderDoc extraction failed: {error}"
        print(message, file=sys.stderr)
        failure_value = os.environ.get(FAILURE_ENV)
        if failure_value:
            try:
                _write_json_exclusive(
                    Path(failure_value).resolve(),
                    {"schema_version": 1, "status": "invalid", "error": message},
                )
            except Exception as write_error:
                print(f"cannot write extraction failure: {write_error}", file=sys.stderr)
        raise SystemExit(1)
