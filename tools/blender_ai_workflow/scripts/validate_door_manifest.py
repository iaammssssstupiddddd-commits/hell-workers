"""Validate the exact Door production authoring manifest and source evidence."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any

ASSET_SET_ID = "door-production-v1"
STATES = {"closed": 204, "open": 204, "locked": 216}
CORE = [
    ("models/buildings/door/door_closed.glb", "mesh:closed"),
    ("models/buildings/door/door_open.glb", "mesh:open"),
    ("models/buildings/door/door_locked.glb", "mesh:locked"),
    ("textures/buildings/door/door_albedo.png", "texture:albedo"),
    ("textures/buildings/door/door_preview_ew.png", "preview:ew"),
    ("textures/buildings/door/door_preview_ns.png", "preview:ns"),
]
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
TOP_FIELDS = {
    "art_review",
    "asset_set_generation",
    "asset_set_id",
    "created_at_utc",
    "license",
    "manifest_mode",
    "mesh_reports",
    "normal_decision",
    "preview_report",
    "production",
    "provenance",
    "schema_version",
    "source",
    "texture_report",
}


class ManifestError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ManifestError(message)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def safe_relative(value: Any, label: str) -> PurePosixPath:
    require(isinstance(value, str) and value, f"{label} path is empty")
    path = PurePosixPath(value)
    require(not path.is_absolute() and ".." not in path.parts, f"{label} path is unsafe")
    return path


def file_record(value: Any, root: Path, label: str) -> Path:
    require(
        isinstance(value, dict) and set(value) == {"path", "sha256"},
        f"{label} record differs",
    )
    path = root / safe_relative(value["path"], label)
    require(path.is_file() and not path.is_symlink(), f"{label} file is absent")
    require(value["sha256"] == sha256(path), f"{label} hash differs")
    return path


def production_record(value: Any, root: Path, path: str, role: str) -> None:
    require(
        isinstance(value, dict)
        and set(value) == {"bytes", "path", "role", "sha256"}
        and value["path"] == path
        and value["role"] == role,
        f"production record differs for {role}",
    )
    actual = root / path
    require(actual.is_file() and not actual.is_symlink(), f"production file is absent for {role}")
    require(value["bytes"] == actual.stat().st_size, f"production byte length differs for {role}")
    require(value["sha256"] == sha256(actual), f"production hash differs for {role}")


def validate_manifest(
    manifest_path: Path,
    *,
    exports_root: Path,
    reports_root: Path,
    blend_root: Path,
    licenses_root: Path,
    repo_root: Path,
) -> dict[str, Any]:
    manifest = read_json(manifest_path)
    require(isinstance(manifest, dict) and set(manifest) == TOP_FIELDS, "manifest fields differ")
    require(
        manifest["schema_version"] == 1
        and manifest["asset_set_id"] == ASSET_SET_ID
        and manifest["manifest_mode"] in {"candidate", "final"}
        and type(manifest["asset_set_generation"]) is int
        and manifest["asset_set_generation"] > 0,
        "manifest identity differs",
    )
    parsed = datetime.fromisoformat(manifest["created_at_utc"].replace("Z", "+00:00"))
    require(parsed.tzinfo is not None, "created timestamp requires timezone")
    require(manifest["normal_decision"] == "not_used_by_design", "normal decision differs")
    review = manifest["art_review"]
    expected_review = "candidate" if manifest["manifest_mode"] == "candidate" else "double_leaf_approved"
    require(isinstance(review, dict) and review.get("status") == expected_review, "art review differs")

    production = manifest["production"]
    core = production.get("core")
    require(production.get("optional") == [] and isinstance(core, list) and len(core) == 6, "production inventory differs")
    for record, (path, role) in zip(core, CORE, strict=True):
        production_record(record, exports_root, path, role)

    mesh_reports = manifest["mesh_reports"]
    require(isinstance(mesh_reports, list) and len(mesh_reports) == 3, "mesh report inventory differs")
    for record, (state, triangles) in zip(mesh_reports, STATES.items(), strict=True):
        require(isinstance(record, dict) and record.get("state") == state and set(record) == {"export", "khronos", "post_export", "state"}, f"mesh reports differ for {state}")
        output = exports_root / f"models/buildings/door/door_{state}.glb"
        export = read_json(file_record(record["export"], reports_root, f"{state} export"))
        require(export.get("status") == "exported" and export.get("sha256") == sha256(output), f"{state} export report differs")
        khronos = read_json(file_record(record["khronos"], reports_root, f"{state} Khronos"))
        require(khronos.get("issues", {}).get("numErrors") == 0 and khronos.get("issues", {}).get("numWarnings") == 0, f"{state} Khronos report differs")
        post = read_json(file_record(record["post_export"], reports_root, f"{state} post export"))
        require(post.get("status") == "pass" and post.get("state") == state and post.get("triangles") == triangles and post.get("glb_sha256") == sha256(output), f"{state} post-export report differs")

    textures = read_json(file_record(manifest["texture_report"], reports_root, "texture report"))
    require(textures.get("status") == "pass" and textures.get("albedo", {}).get("opaque") is True and textures.get("albedo", {}).get("size") == [512, 512], "texture report differs")
    previews = read_json(file_record(manifest["preview_report"], reports_root, "preview report"))
    require(previews.get("status") == "rendered" and previews.get("anchor_px") == [128, 192] and previews.get("ocio", {}).get("fallback") is False and previews.get("ocio", {}).get("validation_status") == "pass", "preview report differs")
    for axis, role in (("ew", "preview:ew"), ("ns", "preview:ns")):
        record = core[4 if axis == "ew" else 5]
        require(previews["previews"][axis]["sha256"] == record["sha256"] and record["role"] == role, f"{axis} preview binding differs")

    file_record(manifest["license"]["file"], licenses_root, "license")
    source = manifest["source"]
    require(isinstance(source, dict) and set(source) == {"blend", "geometry_contract", "runtime_subject", "scene_report", "source_fingerprint", "texture_prompt", "tool_commit", "tool_tree", "tool_versions", "working_tree_diff_sha256"}, "source fields differ")
    file_record(source["blend"], blend_root, "Door blend")
    file_record(source["geometry_contract"], repo_root, "geometry contract")
    file_record(source["scene_report"], reports_root, "scene report")
    file_record(source["texture_prompt"], reports_root, "texture prompt")
    require(HEX40.fullmatch(source["tool_commit"]) is not None and HEX40.fullmatch(source["tool_tree"]) is not None, "tool identity differs")
    require(HEX64.fullmatch(source["source_fingerprint"]) is not None and HEX64.fullmatch(source["working_tree_diff_sha256"]) is not None, "source fingerprint differs")
    if manifest["manifest_mode"] == "final":
        require(HEX40.fullmatch(source["runtime_subject"]) is not None, "final runtime subject differs")
        require(source["working_tree_diff_sha256"] == "0" * 64, "final source must be clean")
    else:
        require(source["runtime_subject"] == "pending", "candidate runtime subject differs")
    return {"asset_set_generation": manifest["asset_set_generation"], "manifest_sha256": sha256(manifest_path), "status": "pass"}
