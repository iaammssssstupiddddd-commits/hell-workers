"""Project a validated Door technical candidate into ArtPreview authority."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

import validate_door_manifest as validator


class ProjectionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProjectionError(message)


def canonical_bytes(payload: dict[str, Any]) -> bytes:
    return (json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def project(manifest_path: Path) -> dict[str, Any]:
    manifest = validator.read_json(manifest_path)
    require(manifest.get("schema_version") == 1 and manifest.get("manifest_mode") == "candidate" and manifest.get("asset_set_id") == validator.ASSET_SET_ID and manifest.get("art_review", {}).get("status") == "candidate", "Door preview identity differs")
    core = manifest.get("production", {}).get("core")
    require(isinstance(core, list) and len(core) == 6 and manifest["production"].get("optional") == [], "Door preview inventory differs")
    return {
        "asset_set_generation": manifest["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "authority": "art_preview",
        "core": core,
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": "not_used_by_design",
        "receipt": None,
        "review_status": "art_preview",
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
    validator.validate_manifest(args.manifest.resolve(), exports_root=root / "staging/exports", reports_root=root / "staging/reports", blend_root=root / "staging/blend", licenses_root=root / "licenses", repo_root=args.repo.resolve())
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(canonical_bytes(project(args.manifest.resolve())))
    print(f"DOOR_PREVIEW_PROJECTED output={output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProjectionError, validator.ManifestError) as error:
        print(f"Door preview projection failed: {error}")
        raise SystemExit(1) from error

