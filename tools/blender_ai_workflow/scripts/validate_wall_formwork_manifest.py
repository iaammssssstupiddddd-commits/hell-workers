"""Validate Wall authoring manifest v3 with completed and formwork sources."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any

import validate_asset_set_manifest as completed_validator

ASSET_SET_ID = "wall-production-v1"
FAMILIES = {
    "isolated": "Wall_Formwork_Isolated",
    "end": "Wall_Formwork_End",
    "straight": "Wall_Formwork_Straight",
    "corner": "Wall_Formwork_Corner",
    "t_junction": "Wall_Formwork_TJunction",
    "cross": "Wall_Formwork_Cross",
}
FORMWORK_PATHS = {
    family: f"models/buildings/wall/wall_formwork_{family}.glb"
    for family in FAMILIES
}
FORMWORK_TEXTURE = "textures/buildings/wall/wall_formwork_albedo.png"
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
TOP_FIELDS = {
    "art_review", "asset_set_generation", "asset_set_id", "created_at_utc",
    "formwork_meshes", "formwork_texture_report", "license", "manifest_mode",
    "normal_decision", "production", "provenance", "schema_version", "source",
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


def timestamp(value: Any) -> None:
    require(isinstance(value, str) and value, "created timestamp is empty")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    require(parsed.tzinfo is not None, "created timestamp requires timezone")


def safe_relative(value: Any, label: str) -> PurePosixPath:
    require(isinstance(value, str) and value, f"{label} path is empty")
    path = PurePosixPath(value)
    require(not path.is_absolute() and ".." not in path.parts, f"{label} path is unsafe")
    return path


def file_record(value: Any, root: Path, label: str) -> Path:
    require(isinstance(value, dict) and set(value) == {"path", "sha256"}, f"{label} record differs")
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
    completed_manifest_path: Path,
) -> dict[str, Any]:
    manifest = read_json(manifest_path)
    require(isinstance(manifest, dict) and set(manifest) == TOP_FIELDS, "manifest fields differ")
    require(
        manifest["schema_version"] == 3
        and manifest["asset_set_id"] == ASSET_SET_ID
        and manifest["manifest_mode"] in {"candidate", "final"}
        and type(manifest["asset_set_generation"]) is int
        and manifest["asset_set_generation"] > 0,
        "manifest identity differs",
    )
    timestamp(manifest["created_at_utc"])
    require(manifest["normal_decision"] == "rejected", "formwork normal decision differs")

    completed = completed_validator.read_json(completed_manifest_path)
    require(
        completed.get("schema_version") == 2
        and completed.get("manifest_mode") == "final"
        and completed.get("asset_set_id") == ASSET_SET_ID,
        "completed source manifest differs",
    )
    source = manifest["source"]
    require(
        isinstance(source, dict)
        and set(source)
        == {
            "completed", "formwork", "runtime_subject", "source_fingerprint",
            "tool_commit", "tool_tree", "tool_versions", "working_tree_diff_sha256",
        },
        "source fields differ",
    )
    require(
        source["completed"]
        == {
            "asset_set_generation": completed["asset_set_generation"],
            "manifest_sha256": sha256(completed_manifest_path),
        },
        "completed source binding differs",
    )
    require(HEX40.fullmatch(source["tool_commit"]) is not None, "tool commit differs")
    require(HEX40.fullmatch(source["tool_tree"]) is not None, "tool tree differs")
    require(HEX64.fullmatch(source["source_fingerprint"]) is not None, "source fingerprint differs")
    require(HEX64.fullmatch(source["working_tree_diff_sha256"]) is not None, "working tree hash differs")
    formwork = source["formwork"]
    require(isinstance(formwork, dict) and set(formwork) == {"blend", "geometry_contract", "texture_prompt"}, "formwork source fields differ")
    file_record(formwork["blend"], blend_root, "formwork blend")
    file_record(formwork["geometry_contract"], reports_root, "formwork geometry contract")
    file_record(formwork["texture_prompt"], reports_root, "formwork texture prompt")

    mesh_records = manifest["formwork_meshes"]
    require(isinstance(mesh_records, list) and len(mesh_records) == 6, "formwork mesh inventory differs")
    for entry, (family, collection) in zip(mesh_records, FAMILIES.items(), strict=True):
        require(
            isinstance(entry, dict)
            and set(entry) == {"collection", "family", "output", "reports"}
            and entry["family"] == family
            and entry["collection"] == collection,
            f"formwork mesh identity differs for {family}",
        )
        output = file_record(entry["output"], exports_root, f"formwork {family} output")
        require(output.relative_to(exports_root).as_posix() == FORMWORK_PATHS[family], f"formwork {family} output path differs")
        reports = entry["reports"]
        require(set(reports) == {"export", "khronos", "post_export"}, f"formwork {family} report inventory differs")
        for role, record in reports.items():
            report = file_record(record, reports_root, f"formwork {family} {role}")
            payload = read_json(report)
            if role == "post_export":
                require(payload.get("status") == "pass" and payload.get("family") == family and payload.get("glb_sha256") == sha256(output), f"formwork {family} post-export report differs")
            elif role == "khronos":
                require(payload.get("issues", {}).get("numErrors") == 0 and payload.get("issues", {}).get("numWarnings") == 0, f"formwork {family} Khronos report differs")
            else:
                require(payload.get("status") == "exported" and payload.get("sha256") == sha256(output), f"formwork {family} export report differs")

    texture_report = file_record(manifest["formwork_texture_report"], reports_root, "formwork texture report")
    texture_payload = read_json(texture_report)
    require(texture_payload.get("status") == "pass" and texture_payload.get("role") == "texture:formwork_albedo" and texture_payload.get("opaque") is True, "formwork texture report differs")

    core = manifest["production"].get("core")
    require(manifest["production"].get("optional") == [] and isinstance(core, list) and len(core) == 15, "production inventory differs")
    completed_core = completed["production"]["core"]
    require(core[:8] == completed_core, "completed production bytes changed")
    expected = [
        (FORMWORK_PATHS[family], f"mesh:formwork:{family}") for family in FAMILIES
    ] + [(FORMWORK_TEXTURE, "texture:formwork_albedo")]
    for record, (path, role) in zip(core[8:], expected, strict=True):
        production_record(record, exports_root, path, role)

    file_record(manifest["license"]["file"], licenses_root, "license")
    require(isinstance(manifest["provenance"], dict), "provenance differs")
    review = manifest["art_review"]
    require(review.get("status") in {"candidate", "art_approved"}, "art review status differs")
    if manifest["manifest_mode"] == "candidate":
        require(review["status"] == "candidate", "candidate art review differs")
    else:
        require(review["status"] == "art_approved", "final art review differs")
        require(HEX40.fullmatch(source["runtime_subject"]) is not None, "final runtime subject differs")
        require(source["working_tree_diff_sha256"] == "0" * 64, "final source must be clean")
    return {
        "asset_set_generation": manifest["asset_set_generation"],
        "manifest_sha256": sha256(manifest_path),
        "status": "pass",
    }
