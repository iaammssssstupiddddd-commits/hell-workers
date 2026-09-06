"""Project an approved Wall formwork manifest into isolated-candidate runtime JSON."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import project_wall_formwork_preview as preview_projector
import validate_wall_formwork_manifest as validator


class ProjectionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProjectionError(message)


def project(manifest_path: Path) -> dict[str, Any]:
    manifest = validator.read_json(manifest_path)
    require(
        manifest.get("schema_version") == 3
        and manifest.get("manifest_mode") == "final"
        and manifest.get("asset_set_id") == validator.ASSET_SET_ID
        and manifest.get("normal_decision") == "rejected"
        and manifest.get("art_review", {}).get("status") == "art_approved",
        "formwork candidate identity differs",
    )
    core = manifest.get("production", {}).get("core")
    require(
        isinstance(core, list)
        and len(core) == 15
        and manifest.get("production", {}).get("optional") == [],
        "formwork candidate inventory differs",
    )
    return {
        "asset_set_generation": manifest["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "authority": "isolated_candidate",
        "candidate_normal": None,
        "core": core,
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": "rejected",
        "receipt": None,
        "review_status": "art_approved",
        "schema_version": 2,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--blend-root", required=True, type=Path)
    parser.add_argument("--exports-root", required=True, type=Path)
    parser.add_argument("--reports-root", required=True, type=Path)
    parser.add_argument("--licenses-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    manifest = args.manifest.resolve()
    validator.validate_manifest(
        manifest,
        exports_root=args.exports_root.resolve(),
        reports_root=args.reports_root.resolve(),
        blend_root=args.blend_root.resolve(),
        licenses_root=args.licenses_root.resolve(),
        completed_manifest_path=args.completed_manifest.resolve(),
    )
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    runtime = project(manifest)
    temporary.write_bytes(preview_projector.canonical_bytes(runtime))
    temporary.replace(output)
    print(
        "WALL_FORMWORK_CANDIDATE_PROJECTED "
        f"generation={runtime['asset_set_generation']} output={output}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProjectionError, validator.ManifestError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"Wall formwork candidate projection failed: {error}")
        raise SystemExit(1) from error
