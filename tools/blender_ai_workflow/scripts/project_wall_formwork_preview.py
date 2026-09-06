"""Project a technical formwork candidate into an ArtPreview runtime wallset."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import validate_wall_formwork_manifest as validator


class ProjectionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProjectionError(message)


def canonical_bytes(payload: dict[str, Any]) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode()


def project(manifest_path: Path) -> dict[str, Any]:
    manifest = validator.read_json(manifest_path)
    require(
        manifest.get("schema_version") == 3
        and manifest.get("manifest_mode") == "candidate"
        and manifest.get("asset_set_id") == validator.ASSET_SET_ID
        and manifest.get("normal_decision") == "rejected"
        and manifest.get("art_review", {}).get("status") == "candidate",
        "formwork preview identity differs",
    )
    core = manifest.get("production", {}).get("core")
    require(
        isinstance(core, list)
        and len(core) == 15
        and manifest.get("production", {}).get("optional") == [],
        "formwork preview inventory differs",
    )
    return {
        "asset_set_generation": manifest["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "authority": "art_preview",
        "candidate_normal": None,
        "core": core,
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": "rejected",
        "receipt": None,
        "review_status": "art_preview",
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
    validator.validate_manifest(
        args.manifest.resolve(),
        exports_root=args.exports_root.resolve(),
        reports_root=args.reports_root.resolve(),
        blend_root=args.blend_root.resolve(),
        licenses_root=args.licenses_root.resolve(),
        completed_manifest_path=args.completed_manifest.resolve(),
    )
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    temporary.write_bytes(canonical_bytes(project(args.manifest.resolve())))
    temporary.replace(output)
    print(f"WALL_FORMWORK_PREVIEW_PROJECTED output={output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProjectionError, validator.ManifestError) as error:
        print(f"Wall formwork preview projection failed: {error}")
        raise SystemExit(1) from error
