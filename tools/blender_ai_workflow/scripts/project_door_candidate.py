"""Project an approved Door manifest into isolated-candidate runtime JSON."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import project_door_preview as preview_projector
import validate_door_manifest as validator


class ProjectionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProjectionError(message)


def project(manifest_path: Path) -> dict[str, Any]:
    manifest = validator.read_json(manifest_path)
    require(
        manifest.get("schema_version") == 1
        and manifest.get("manifest_mode") == "final"
        and manifest.get("asset_set_id") == validator.ASSET_SET_ID
        and manifest.get("normal_decision") == "not_used_by_design"
        and manifest.get("art_review", {}).get("status")
        == "double_leaf_approved",
        "Door candidate identity differs",
    )
    core = manifest.get("production", {}).get("core")
    require(
        isinstance(core, list)
        and len(core) == 6
        and manifest.get("production", {}).get("optional") == [],
        "Door candidate inventory differs",
    )
    return {
        "asset_set_generation": manifest["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "authority": "isolated_candidate",
        "core": core,
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": "not_used_by_design",
        "receipt": None,
        "review_status": "art_approved",
        "schema_version": 1,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = args.asset_root.resolve()
    manifest = args.manifest.resolve()
    validator.validate_manifest(
        manifest,
        exports_root=root / "staging/exports",
        reports_root=root / "staging/reports",
        blend_root=root / "staging/blend",
        licenses_root=root / "licenses",
        repo_root=args.repo.resolve(),
    )
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    try:
        temporary.write_bytes(preview_projector.canonical_bytes(project(manifest)))
        temporary.replace(output)
    finally:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
    print(f"DOOR_CANDIDATE_PROJECTED output={output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ProjectionError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Door candidate projection failed: {error}")
        raise SystemExit(1) from error
