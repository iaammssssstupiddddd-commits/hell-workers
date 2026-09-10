"""Asset-specific validation and payload collection for immutable releases.

Wall v2 keeps its existing layout. Wall v3 carries the completed v2 source in
``completed/``; Door carries its repository geometry contract in ``source/repo/``.
Neither release needs the original staging directory to be revalidated.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import validate_asset_set_manifest as wall
import validate_door_manifest as door
import validate_wall_formwork_manifest as formwork

WALL_ID = "wall-production-v1"
DOOR_ID = "door-production-v1"
ASSET_IDS = (WALL_ID, DOOR_ID)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def identity(manifest: dict[str, Any]) -> tuple[str, int]:
    pair = (manifest.get("asset_set_id"), manifest.get("schema_version"))
    require(pair in {(WALL_ID, 2), (WALL_ID, 3), (DOOR_ID, 1)}, "unsupported release manifest identity")
    require(manifest.get("manifest_mode") == "final", "release requires a final manifest")
    return pair


def generation_directory(asset_id: str) -> Path:
    require(asset_id in ASSET_IDS, "unsupported asset-set ID")
    return Path("generations") if asset_id == WALL_ID else Path("asset_sets") / asset_id / "generations"


def quarantine_directory(asset_id: str) -> Path:
    return generation_directory(asset_id).with_name("quarantine")


def active_pointer(asset_id: str) -> Path:
    require(asset_id in ASSET_IDS, "unsupported asset-set ID")
    return Path("authority") / f"{asset_id}.active.json"


def roots_for(manifest_path: Path) -> dict[str, Path]:
    parent = manifest_path.resolve().parent
    if parent.name == "manifest":
        root = parent.parent
        return {
            "blend_root": root / "source/blender",
            "exports_root": root / "exports",
            "reports_root": root / "reports",
            "licenses_root": root / "licenses",
        }
    staging = parent.parent
    return {
        "blend_root": staging / "blend",
        "exports_root": staging / "exports",
        "reports_root": parent,
        "licenses_root": staging.parent / "licenses",
    }


def completed_source(manifest_path: Path, manifest: dict[str, Any]) -> Path:
    if manifest_path.parent.name == "manifest":
        root = manifest_path.parent.parent / "completed"
    else:
        generation = manifest["source"]["completed"]["asset_set_generation"]
        require(type(generation) is int and generation > 0, "completed source generation differs")
        root = manifest_path.resolve().parents[2] / "generations" / str(generation)
    return root / "manifest/wall-production-v1.asset-set.json"


def validate(
    manifest_path: Path,
    *,
    roots: dict[str, Path] | None = None,
    repo: Path | None,
) -> dict[str, Any]:
    manifest = wall.read_json(manifest_path)
    asset_id, version = identity(manifest)
    roots = roots or roots_for(manifest_path)
    if asset_id == WALL_ID and version == 2:
        return wall.validate_manifest(manifest_path, mode="final", repo=repo, **roots)
    formwork.file_record(manifest["art_review"]["artifact"], roots["reports_root"], "release art approval")
    if asset_id == WALL_ID:
        completed_path = completed_source(manifest_path, manifest)
        # The formwork validator binds the completed manifest, but does not read
        # the completed GLBs/reports. Revalidate those bytes independently too.
        validate(completed_path, repo=repo)
        result = formwork.validate_manifest(
            manifest_path, completed_manifest_path=completed_path, **roots
        )
        completed = wall.read_json(completed_path)
        if manifest_path.parent.name == "manifest":
            contract = completed_path.parent.parent / "source/contracts/wall-production-v1.geometry.json"
            require(contract.is_file() and not contract.is_symlink(), "completed geometry contract is absent")
            require(wall.sha256(contract) == completed["source"]["geometry_contract"]["sha256"], "completed geometry contract differs")
        for record in completed["production"]["core"]:
            path = roots["exports_root"] / record["path"]
            require(path.is_file() and not path.is_symlink(), "completed core file is absent")
            require(path.stat().st_size == record["bytes"] and wall.sha256(path) == record["sha256"], "completed core bytes differ")
    else:
        contract_root = (
            manifest_path.parent.parent / "source/repo"
            if manifest_path.parent.name == "manifest"
            else repo
        )
        require(contract_root is not None, "Door validation requires the source repository")
        result = door.validate_manifest(manifest_path, repo_root=contract_root, **roots)
    return {**result, "asset_set_id": asset_id}


def additional_payload(
    manifest_path: Path, manifest: dict[str, Any], *, repo: Path | None
) -> list[tuple[Path, str, str]]:
    """Return all files for v3 Wall / Door (not the legacy Wall v2 payload)."""
    asset_id, version = identity(manifest)
    require((asset_id, version) != (WALL_ID, 2), "legacy payload has its own collector")
    roots = roots_for(manifest_path)
    entries: list[tuple[Path, str, str]] = []

    def add(record: dict[str, Any], root: Path, prefix: str) -> None:
        relative = formwork.safe_relative(record["path"], "release payload")
        entries.append((root / relative, f"{prefix}/{record['path']}", record["sha256"]))

    for record in manifest["production"]["core"]:
        add(record, roots["exports_root"], "exports")
    add(manifest["license"]["file"], roots["licenses_root"], "licenses")
    add(manifest["art_review"]["artifact"], roots["reports_root"], "reports")
    if asset_id == WALL_ID:
        source = manifest["source"]["formwork"]
        add(source["blend"], roots["blend_root"], "source/blender")
        for field in ("geometry_contract", "texture_prompt"):
            add(source[field], roots["reports_root"], "reports")
        for mesh in manifest["formwork_meshes"]:
            for record in mesh["reports"].values():
                add(record, roots["reports_root"], "reports")
        add(manifest["formwork_texture_report"], roots["reports_root"], "reports")
    else:
        source = manifest["source"]
        require(repo is not None, "Door payload requires the source repository")
        add(source["blend"], roots["blend_root"], "source/blender")
        add(source["geometry_contract"], repo, "source/repo")
        for field in ("scene_report", "texture_prompt"):
            add(source[field], roots["reports_root"], "reports")
        for field in ("texture_report", "preview_report"):
            add(manifest[field], roots["reports_root"], "reports")
        for mesh in manifest["mesh_reports"]:
            for field in ("export", "khronos", "post_export"):
                add(mesh[field], roots["reports_root"], "reports")
    entries.append((manifest_path, f"manifest/{asset_id}.asset-set.json", wall.sha256(manifest_path)))
    return entries
