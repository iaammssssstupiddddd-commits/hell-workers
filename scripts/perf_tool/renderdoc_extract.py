#!/usr/bin/env python3
"""Replay one formal RtT-light RenderDoc capture and extract stable structure.

This file is executed by qrenderdoc's ``--python`` option.  qrenderdoc owns
RenderDoc replay initialisation and shutdown; this script owns only the capture
handle and replay controller that it opens.
"""

from __future__ import annotations

import hashlib
import json
import os
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = 2
RUNTIME_CHECKPOINT_SCHEMA_VERSION = 3
CAPTURE_ENV = "HW_RENDERDOC_CAPTURE"
OUTPUT_ENV = "HW_RENDERDOC_EXTRACTION"
CHECKPOINT_ENV = "HW_RENDERDOC_RUNTIME_CHECKPOINT"
FAILURE_ENV = "HW_RENDERDOC_EXTRACTION_FAILURE"

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
}


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


def _validate_checkpoint(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != {
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
    }:
        raise RuntimeError("runtime checkpoint keys differ from schema v3")
    if (
        value.get("schema_version") != RUNTIME_CHECKPOINT_SCHEMA_VERSION
        or value.get("status") != "valid"
        or value.get("contract_id") != "rtt-light-v1"
        or value.get("stage_id") not in EXPECTED_RENDER_RESOURCES_BY_STAGE
        or not isinstance(value.get("generation"), int)
        or isinstance(value.get("generation"), bool)
        or value["generation"] < 1
        or value.get("requested_renderdoc_api_version") != "1.6.0"
        or not isinstance(value.get("returned_renderdoc_api_version"), str)
    ):
        raise RuntimeError("runtime checkpoint identity differs from schema v3")
    inventory = value.get("render_inventory")
    if not isinstance(inventory, dict) or not inventory:
        raise RuntimeError("runtime checkpoint has no render inventory")
    if not isinstance(value.get("checkpoint"), dict):
        raise RuntimeError("runtime checkpoint has no frame checkpoint")
    if not isinstance(value.get("selector"), dict):
        raise RuntimeError("runtime checkpoint has no selector evidence")
    if not isinstance(value.get("gpu_ready"), dict):
        raise RuntimeError("runtime checkpoint has no GPU-ready evidence")
    artifact = value.get("capture_artifact")
    if (
        not isinstance(artifact, dict)
        or set(artifact) != {"sha256", "bytes"}
        or not isinstance(artifact.get("sha256"), str)
        or len(artifact["sha256"]) != 64
        or not isinstance(artifact.get("bytes"), int)
        or isinstance(artifact.get("bytes"), bool)
        or artifact["bytes"] <= 0
    ):
        raise RuntimeError("runtime checkpoint has no capture artifact evidence")
    return value


def _read_checkpoint(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"cannot read runtime checkpoint: {error}") from error
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


def _binding_metadata(pipeline: Any, stage: Any, category: str, used: Any) -> tuple[int, int]:
    """Resolve a stable Vulkan ``(set, binding)`` pair from reflection.

    ``UsedDescriptor.access`` identifies the reflection-array index.  It does
    not itself carry a ``fixedBindNumber``; using the ordinal of an
    ``onlyUsed`` list would silently renumber bindings when a shader stops
    accessing an earlier descriptor.
    """

    access = getattr(used, "access", None)
    index = getattr(access, "index", None)
    if not isinstance(index, int) or isinstance(index, bool) or index < 0:
        raise RuntimeError(f"RenderDoc {category} descriptor has no reflection index")
    reflection = pipeline.GetShaderReflection(stage)
    fields = {
        "constant-block": "constantBlocks",
        "read-only": "readOnlyResources",
        "read-write": "readWriteResources",
        "sampler": "samplers",
    }
    entries = getattr(reflection, fields[category], None)
    if entries is None or index >= len(entries):
        raise RuntimeError(
            f"RenderDoc {category} descriptor reflection index is out of range"
        )
    fixed_bind_set_or_space = getattr(entries[index], "fixedBindSetOrSpace", None)
    fixed_bind_number = getattr(entries[index], "fixedBindNumber", None)
    if any(
        not isinstance(value, int) or isinstance(value, bool) or value < 0
        for value in (fixed_bind_set_or_space, fixed_bind_number)
    ):
        raise RuntimeError(
            f"RenderDoc {category} descriptor has no valid fixed set/binding"
        )
    return fixed_bind_set_or_space, fixed_bind_number


def _binding_resource_id(rd: Any, category: str, used: Any) -> str:
    source = getattr(used, "sampler" if category == "sampler" else "descriptor", None)
    resource_id = _resource_id(rd, source)
    if resource_id is None:
        raise RuntimeError(f"RenderDoc used {category} descriptor has no resource")
    return resource_id


def _draw_passes(rd: Any, roots: Any) -> tuple[list[tuple[Any, list[Any]]], list[Any]]:
    flattened = _flatten(roots)
    groups: list[tuple[Any, list[Any]]] = []
    active: tuple[Any, list[Any]] | None = None
    for action in flattened:
        flags = action.flags
        is_command_buffer_boundary = bool(
            flags & rd.ActionFlags.CommandBufferBoundary
        )
        begins = bool(flags & rd.ActionFlags.BeginPass) and not is_command_buffer_boundary
        ends = bool(flags & rd.ActionFlags.EndPass) and not is_command_buffer_boundary
        # RenderDoc represents Vulkan vkCmdNextSubpass as one action carrying
        # both flags.  Close the old subpass before opening the next one.
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


def _render_resources(checkpoint: dict[str, Any]) -> dict[str, Any]:
    expected = checkpoint.get("render_resources")
    if not isinstance(expected, dict) or set(expected) != {
        "scene_target_label",
        "mask_target_label",
        "composite_draw_count",
        "composite_texture_bindings",
        "composite_sampler_bindings",
    }:
        raise RuntimeError("runtime checkpoint render resources differ from schema v3")
    for key in ("scene_target_label", "mask_target_label"):
        label = expected[key]
        if key == "mask_target_label" and label is None:
            continue
        if not isinstance(label, str) or not label:
            raise RuntimeError("runtime checkpoint render resource label is invalid")
    if (
        not isinstance(expected["composite_draw_count"], int)
        or isinstance(expected["composite_draw_count"], bool)
        or expected["composite_draw_count"] != 1
    ):
        raise RuntimeError("runtime checkpoint composite draw count is invalid")
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
        rows = expected[key]
        if (
            not isinstance(rows, list)
            or not rows
            or any(not isinstance(row, dict) or set(row) != expected_keys for row in rows)
        ):
            raise RuntimeError(f"runtime checkpoint {key} differs from schema v3")
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
                raise RuntimeError(f"runtime checkpoint {key} has an invalid binding")
            if key == "composite_texture_bindings" and (
                not isinstance(row["target"], str) or not row["target"]
            ):
                raise RuntimeError("runtime checkpoint texture binding has an invalid target")
    stage_id = checkpoint.get("stage_id")
    if expected != EXPECTED_RENDER_RESOURCES_BY_STAGE.get(stage_id):
        raise RuntimeError(
            f"runtime checkpoint composite bindings differ from the {stage_id} source contract"
        )
    return expected


def _tracked_resources(
    rd: Any, controller: Any, render_resources: dict[str, Any]
) -> dict[str, dict[str, str]]:
    labels = {
        key.removesuffix("_label"): label
        for key, label in render_resources.items()
        if key.endswith("_target_label") and label is not None
    }
    matches: dict[str, list[str]] = {key: [] for key in labels}
    for resource in controller.GetResources():
        resource_id = _resource_id(rd, getattr(resource, "resourceId", resource))
        name = str(getattr(resource, "name", "")).strip()
        if resource_id is None:
            continue
        for key, label in labels.items():
            if name == label:
                matches[key].append(resource_id)
    tracked: dict[str, dict[str, str]] = {}
    for key, label in labels.items():
        ids = matches[key]
        if len(ids) != 1:
            raise RuntimeError(
                f"RenderDoc resource label {label!r} resolved to {len(ids)} resources"
            )
        tracked[key] = {"label": label, "resource_id": ids[0]}
    return tracked


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def _composite_topology(
    *,
    render_resources: dict[str, Any],
    bindings: list[dict[str, Any]],
    tracked_resources: dict[str, dict[str, str]],
) -> dict[str, Any]:
    """Find the one source-defined draw that samples the stage's RtT targets.

    The match is per draw, not a global resource count.  In particular, two
    references to the same sampler across separate draws must not satisfy the
    two-sampler requirement.
    """

    expected_textures = [
        {
            **binding,
            "resource_id": tracked_resources[binding["target"]]["resource_id"],
        }
        for binding in render_resources["composite_texture_bindings"]
    ]
    expected_samplers = render_resources["composite_sampler_bindings"]
    expected_texture_signatures = {
        (
            binding["stage"],
            binding["fixed_bind_set_or_space"],
            binding["fixed_bind_number"],
            binding["resource_id"],
        )
        for binding in expected_textures
    }
    expected_sampler_signatures = {
        (
            binding["stage"],
            binding["fixed_bind_set_or_space"],
            binding["fixed_bind_number"],
        )
        for binding in expected_samplers
    }
    bindings_by_draw: dict[tuple[str, int], list[dict[str, Any]]] = {}
    for binding in bindings:
        key = (binding["pass_id"], binding["event_id"])
        bindings_by_draw.setdefault(key, []).append(binding)

    draws: list[dict[str, Any]] = []
    for (pass_id, event_id), draw_bindings in bindings_by_draw.items():
        texture_rows = [
            row for row in draw_bindings if row["category"] == "fragment:read-only"
        ]
        sampler_rows = [
            row for row in draw_bindings if row["category"] == "fragment:sampler"
        ]
        texture_signatures = {
            (
                row["category"].split(":", 1)[0],
                row["fixed_bind_set_or_space"],
                row["fixed_bind_number"],
                row["resource_id"],
            )
            for row in texture_rows
        }
        sampler_signatures = {
            (
                row["category"].split(":", 1)[0],
                row["fixed_bind_set_or_space"],
                row["fixed_bind_number"],
            )
            for row in sampler_rows
        }
        if (
            len(texture_rows) != len(expected_textures)
            or len(texture_signatures) != len(texture_rows)
            or texture_signatures != expected_texture_signatures
            or len(sampler_rows) != len(expected_samplers)
            or len(sampler_signatures) != len(sampler_rows)
            or sampler_signatures != expected_sampler_signatures
        ):
            continue
        draws.append(
            {
                "pass_id": pass_id,
                "event_id": event_id,
                "texture_bindings": [
                    {
                        "target": binding["target"],
                        "stage": binding["stage"],
                        "fixed_bind_set_or_space": binding["fixed_bind_set_or_space"],
                        "fixed_bind_number": binding["fixed_bind_number"],
                        "resource_id": binding["resource_id"],
                    }
                    for binding in expected_textures
                ],
                "sampler_bindings": [
                    {
                        "stage": binding["stage"],
                        "fixed_bind_set_or_space": binding["fixed_bind_set_or_space"],
                        "fixed_bind_number": binding["fixed_bind_number"],
                        "resource_id": next(
                            row["resource_id"]
                            for row in sampler_rows
                            if (
                                row["category"] == f"{binding['stage']}:sampler"
                                and row["fixed_bind_set_or_space"]
                                == binding["fixed_bind_set_or_space"]
                                and row["fixed_bind_number"]
                                == binding["fixed_bind_number"]
                            )
                        ),
                    }
                    for binding in expected_samplers
                ],
            }
        )
    if len(draws) != render_resources["composite_draw_count"]:
        raise RuntimeError(
            "RenderDoc replay does not prove the exact source-defined RtT composite draw"
        )
    return {"draw_count": len(draws), "draws": draws}


def _replay_structure(
    *,
    passes: list[dict[str, Any]],
    attachments: list[dict[str, Any]],
    bindings: list[dict[str, Any]],
    tracked_resources: dict[str, dict[str, str]],
    composite_topology: dict[str, Any],
) -> dict[str, int]:
    result = {
        "render_pass_count": len(passes),
        "attachment_count": len(attachments),
        "binding_count": len(bindings),
        "composite_draw_count": composite_topology["draw_count"],
        "composite_texture_binding_count": sum(
            len(draw["texture_bindings"])
            for draw in composite_topology["draws"]
        ),
        "composite_sampler_binding_count": sum(
            len(draw["sampler_bindings"])
            for draw in composite_topology["draws"]
        ),
    }
    for key, value in tracked_resources.items():
        resource_id = value["resource_id"]
        result[f"{key}_attachment_count"] = sum(
            row["resource_id"] == resource_id for row in attachments
        )
        result[f"{key}_binding_count"] = sum(
            row["resource_id"] == resource_id for row in bindings
        )
    if (
        result["render_pass_count"] < 2
        or result["composite_draw_count"] != 1
        or result["composite_texture_binding_count"]
        != len(composite_topology["draws"][0]["texture_bindings"])
        or result["composite_sampler_binding_count"]
        != len(composite_topology["draws"][0]["sampler_bindings"])
        or any(
            result[f"{key}_{kind}_count"] < 1
            for key in tracked_resources
            for kind in ("attachment", "binding")
        )
    ):
        raise RuntimeError(
            "RenderDoc replay does not prove the stage targets and exact composite topology"
        )
    return result


def _extract(rd: Any, controller: Any, checkpoint: dict[str, Any]) -> dict[str, Any]:
    properties = controller.GetAPIProperties()
    if properties.pipelineType != rd.GraphicsAPI.Vulkan:
        raise RuntimeError(f"capture API is not Vulkan: {properties.pipelineType}")

    roots = controller.GetRootActions()
    groups, flattened = _draw_passes(rd, roots)
    structured_file = controller.GetStructuredFile()
    render_resources = _render_resources(checkpoint)
    tracked_resources = _tracked_resources(rd, controller, render_resources)
    passes: list[dict[str, Any]] = []
    attachments: list[dict[str, Any]] = []
    bindings: list[dict[str, Any]] = []

    binding_getters = (
        ("constant-block", "GetConstantBlocks"),
        ("read-only", "GetReadOnlyResources"),
        ("read-write", "GetReadWriteResources"),
        ("sampler", "GetSamplers"),
    )
    stages = (
        ("vertex", rd.ShaderStage.Vertex),
        ("fragment", rd.ShaderStage.Fragment),
        ("compute", rd.ShaderStage.Compute),
    )

    for pass_index, (root, draws) in enumerate(groups, start=1):
        pass_id = f"pass-{pass_index:04d}"
        event_ids = [int(draw.eventId) for draw in draws]
        passes.append(
            {
                "pass_id": pass_id,
                "name": _action_name(root, structured_file),
                "first_event": min(event_ids),
                "last_event": max(event_ids),
                "draw_count": len(draws),
            }
        )
        for draw in draws:
            event_id = int(draw.eventId)
            if event_id <= 0:
                raise RuntimeError("draw action has a nonpositive event id")
            controller.SetFrameEvent(event_id, True)
            pipeline = controller.GetPipelineState()

            for slot, descriptor in enumerate(pipeline.GetOutputTargets()):
                resource_id = _resource_id(rd, descriptor)
                if resource_id is None:
                    continue
                attachments.append(
                    {
                        "attachment_id": f"attachment-{len(attachments) + 1:06d}",
                        "pass_id": pass_id,
                        "event_id": event_id,
                        "slot": slot,
                        "kind": "color",
                        "resource_id": resource_id,
                    }
                )
            depth_id = _resource_id(rd, pipeline.GetDepthTarget())
            if depth_id is not None:
                attachments.append(
                    {
                        "attachment_id": f"attachment-{len(attachments) + 1:06d}",
                        "pass_id": pass_id,
                        "event_id": event_id,
                        "slot": 0,
                        "kind": "depth",
                        "resource_id": depth_id,
                    }
                )

            for stage_name, stage in stages:
                for category, getter_name in binding_getters:
                    getter = getattr(pipeline, getter_name)
                    for used in getter(stage, True):
                        fixed_bind_set_or_space, fixed_bind_number = _binding_metadata(
                            pipeline, stage, category, used
                        )
                        bindings.append(
                            {
                                "binding_id": f"binding-{len(bindings) + 1:07d}",
                                "pass_id": pass_id,
                                "event_id": event_id,
                                "category": f"{stage_name}:{category}",
                                "fixed_bind_set_or_space": fixed_bind_set_or_space,
                                "fixed_bind_number": fixed_bind_number,
                                "resource_id": _binding_resource_id(
                                    rd, category, used
                                ),
                            }
                        )

    event_ids = {
        int(action.eventId) for action in flattened if int(action.eventId) > 0
    }
    draw_count = sum(row["draw_count"] for row in passes)
    if len(event_ids) < draw_count:
        raise RuntimeError("capture event count is smaller than its draw count")
    if not attachments:
        raise RuntimeError("capture contains no draw attachments")
    if not bindings:
        raise RuntimeError("capture contains no used draw bindings")
    composite_topology = _composite_topology(
        render_resources=render_resources,
        bindings=bindings,
        tracked_resources=tracked_resources,
    )
    replay_structure = _replay_structure(
        passes=passes,
        attachments=attachments,
        bindings=bindings,
        tracked_resources=tracked_resources,
        composite_topology=composite_topology,
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "api": "vulkan",
        "capture_sha256": _sha256(_required_path(CAPTURE_ENV, must_exist=True)),
        "validated_frames": 1,
        "event_count": len(event_ids),
        "draw_count": draw_count,
        "passes": passes,
        "attachments": attachments,
        "bindings": bindings,
        "tracked_resources": tracked_resources,
        "composite_topology": composite_topology,
        "replay_structure": replay_structure,
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
    """Exercise replay-schema logic without requiring a RenderDoc install.

    The formal capture must still replay through qrenderdoc.  This narrow test
    protects the API-shape assumptions that can otherwise regress before that
    environment is available: pass boundaries, reflection binding numbers,
    sampler-resource resolution, label lookup, and topology accounting.
    """

    from enum import IntFlag
    from types import SimpleNamespace

    checkpoint = {
        "schema_version": 3,
        "status": "valid",
        "contract_id": "rtt-light-v1",
        "stage_id": "current",
        "generation": 1,
        "checkpoint": {"ready_frame_ordinal": 4},
        "render_inventory": {"scene_target_count": 1},
        "render_resources": CURRENT_RENDER_RESOURCES,
        "fixture": {"rooms": 4},
        "capture_path": "/diagnostic/capture.rdc",
        "requested_renderdoc_api_version": "1.6.0",
        "returned_renderdoc_api_version": "1.7.0",
        "selector": {"strategy": "wgpu_device_null_window"},
        "gpu_ready": {"pre_capture": {}, "post_capture": {}},
        "capture_artifact": {"sha256": "a" * 64, "bytes": 1},
    }
    _require(
        _validate_checkpoint(checkpoint) is checkpoint,
        "runtime checkpoint schema v3 validation regressed",
    )
    try:
        _validate_checkpoint({**checkpoint, "schema_version": 2})
    except RuntimeError:
        pass
    else:
        raise RuntimeError("runtime checkpoint schema v2 must be rejected")

    class ResourceId:
        def __init__(self, value: int):
            self.value = value

        @staticmethod
        def Null() -> "ResourceId":
            return ResourceId(0)

        def __eq__(self, other: object) -> bool:
            return isinstance(other, ResourceId) and self.value == other.value

        def __str__(self) -> str:
            return f"ResourceId::{self.value}"

    class ActionFlags(IntFlag):
        BeginPass = 1
        EndPass = 2
        Drawcall = 4
        CommandBufferBoundary = 8

    rd = SimpleNamespace(ResourceId=ResourceId, ActionFlags=ActionFlags)

    def action(event_id: int, flags: ActionFlags, children: list[Any] | None = None) -> Any:
        return SimpleNamespace(
            eventId=event_id,
            flags=flags,
            children=[] if children is None else children,
        )

    roots = [
        action(0, ActionFlags.BeginPass | ActionFlags.CommandBufferBoundary),
        action(1, ActionFlags.BeginPass, [action(2, ActionFlags.Drawcall)]),
        action(
            3,
            ActionFlags.EndPass | ActionFlags.BeginPass,
            [action(4, ActionFlags.Drawcall)],
        ),
        action(5, ActionFlags.EndPass),
        action(6, ActionFlags.EndPass | ActionFlags.CommandBufferBoundary),
    ]
    passes, flattened = _draw_passes(rd, roots)
    _require(
        len(passes) == 2 and len(flattened) == 7,
        "subpass-boundary grouping regressed",
    )

    reflection = SimpleNamespace(
        constantBlocks=[SimpleNamespace(fixedBindSetOrSpace=2, fixedBindNumber=2)],
        readOnlyResources=[SimpleNamespace(fixedBindSetOrSpace=2, fixedBindNumber=3)],
        readWriteResources=[SimpleNamespace(fixedBindSetOrSpace=2, fixedBindNumber=4)],
        samplers=[SimpleNamespace(fixedBindSetOrSpace=2, fixedBindNumber=5)],
    )
    pipeline = SimpleNamespace(GetShaderReflection=lambda _stage: reflection)
    used_sampler = SimpleNamespace(
        access=SimpleNamespace(index=0), sampler=SimpleNamespace(object=ResourceId(11))
    )
    used_read_only = SimpleNamespace(
        access=SimpleNamespace(index=0), descriptor=SimpleNamespace(resource=ResourceId(12))
    )
    _require(
        _binding_metadata(pipeline, "fragment", "sampler", used_sampler) == (2, 5),
        "sampler reflection set/binding lookup regressed",
    )
    _require(
        _binding_metadata(pipeline, "fragment", "read-only", used_read_only)
        == (2, 3),
        "read-only reflection set/binding lookup regressed",
    )
    _require(
        _binding_resource_id(rd, "sampler", used_sampler) == "ResourceId::11",
        "sampler object resource lookup regressed",
    )
    _require(
        _binding_resource_id(rd, "read-only", used_read_only) == "ResourceId::12",
        "descriptor resource lookup regressed",
    )

    resources = [
        SimpleNamespace(resourceId=ResourceId(21), name="hell-workers-rtt-scene"),
        SimpleNamespace(resourceId=ResourceId(22), name="hell-workers-rtt-soul-mask"),
    ]
    controller = SimpleNamespace(GetResources=lambda: resources)
    render_resources = _render_resources(
        {"stage_id": "current", "render_resources": CURRENT_RENDER_RESOURCES}
    )
    tracked = _tracked_resources(
        rd,
        controller,
        render_resources,
    )
    _require(
        tracked["scene_target"]["resource_id"] == "ResourceId::21"
        and tracked["mask_target"]["resource_id"] == "ResourceId::22",
        "resource-label lookup regressed",
    )
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
    composite_topology = _composite_topology(
        render_resources=render_resources,
        bindings=bindings,
        tracked_resources=tracked,
    )
    _require(
        composite_topology["draw_count"] == 1
        and composite_topology["draws"][0]["sampler_bindings"][0]["resource_id"]
        == "ResourceId::11",
        "exact composite topology accounting regressed",
    )
    duplicate_bindings = [
        *bindings,
        *[
            {
                **binding,
                "binding_id": f"binding-duplicate-{index}",
                "pass_id": "pass-0001",
                "event_id": 2,
            }
            for index, binding in enumerate(bindings, start=1)
        ],
    ]
    try:
        _composite_topology(
            render_resources=render_resources,
            bindings=duplicate_bindings,
            tracked_resources=tracked,
        )
    except RuntimeError:
        pass
    else:
        raise RuntimeError("duplicate composite draws must not satisfy topology")
    structure = _replay_structure(
        passes=[{"pass_id": "pass-0001"}, {"pass_id": "pass-0002"}],
        attachments=[
            {"resource_id": "ResourceId::21"},
            {"resource_id": "ResourceId::22"},
        ],
        bindings=bindings,
        tracked_resources=tracked,
        composite_topology=composite_topology,
    )
    _require(
        structure["scene_target_attachment_count"] == 1
        and structure["mask_target_binding_count"] == 1
        and structure["composite_sampler_binding_count"] == 2,
        "RtT replay topology accounting regressed",
    )
    p01_checkpoint = {
        **checkpoint,
        "stage_id": "p01",
        "render_resources": P01_RENDER_RESOURCES,
    }
    _require(
        _validate_checkpoint(p01_checkpoint) is p01_checkpoint,
        "P01 runtime checkpoint validation regressed",
    )
    p01_resources = _render_resources(p01_checkpoint)
    p01_tracked = _tracked_resources(
        rd,
        SimpleNamespace(GetResources=lambda: resources[:1]),
        p01_resources,
    )
    p01_bindings = [bindings[0], bindings[2]]
    p01_topology = _composite_topology(
        render_resources=p01_resources,
        bindings=p01_bindings,
        tracked_resources=p01_tracked,
    )
    p01_structure = _replay_structure(
        passes=[{"pass_id": "pass-0001"}, {"pass_id": "pass-0002"}],
        attachments=[{"resource_id": "ResourceId::21"}],
        bindings=p01_bindings,
        tracked_resources=p01_tracked,
        composite_topology=p01_topology,
    )
    _require(
        set(p01_tracked) == {"scene_target"}
        and p01_structure["scene_target_attachment_count"] == 1
        and p01_structure["composite_texture_binding_count"] == 1
        and p01_structure["composite_sampler_binding_count"] == 1,
        "P01 Scene-only replay topology accounting regressed",
    )
    print("renderdoc_extract self-test: PASS")
    return 0


def main() -> int:
    capture = _required_path(CAPTURE_ENV, must_exist=True)
    output = _required_path(OUTPUT_ENV, must_exist=False)
    checkpoint_path = _required_path(CHECKPOINT_ENV, must_exist=True)
    if output.exists():
        raise RuntimeError(f"extraction output already exists: {output}")
    if capture.stat().st_size <= 0:
        raise RuntimeError("capture is empty")

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
        payload = _extract(rd, controller, _read_checkpoint(checkpoint_path))
        _write_json_exclusive(output, payload)
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
        message = f"renderdoc extraction failed: {error}"
        print(message, file=sys.stderr)
        failure_value = os.environ.get(FAILURE_ENV)
        if failure_value:
            try:
                _write_json_exclusive(
                    Path(failure_value).resolve(),
                    {"schema_version": 1, "error": message},
                )
            except Exception as failure_error:
                print(
                    f"renderdoc extraction failure evidence failed: {failure_error}",
                    file=sys.stderr,
                )
        raise SystemExit(1) from error
