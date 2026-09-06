"""Provision an immutable ArtPreview asset view outside the repository."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path
from typing import Any

import project_wall_formwork_preview as projector
import validate_wall_formwork_manifest as validator


class ProvisionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProvisionError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy_exclusive(source: Path, destination: Path, expected_hash: str) -> None:
    require(source.is_file() and digest(source) == expected_hash, f"source hash differs: {source}")
    if destination.exists():
        require(destination.is_file() and digest(destination) == expected_hash, f"destination already differs: {destination}")
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(f".{destination.name}.provisioning")
    require(not temporary.exists(), f"stale provisioning file exists: {temporary}")
    shutil.copyfile(source, temporary)
    require(digest(temporary) == expected_hash, f"copied hash differs: {destination}")
    temporary.replace(destination)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--destination", required=True, type=Path)
    args = parser.parse_args()
    asset_root = args.asset_root.resolve()
    destination = args.destination.resolve()
    validation_root = (asset_root / "staging/validation").resolve()
    require(validation_root in destination.parents, "destination must be under staging/validation")
    assets = destination / "assets"
    manifest_path = args.manifest.resolve()
    reports = asset_root / "staging/reports"
    exports = asset_root / "staging/exports"
    validator.validate_manifest(
        manifest_path,
        exports_root=exports,
        reports_root=reports,
        blend_root=asset_root / "staging/blend",
        licenses_root=asset_root / "licenses",
        completed_manifest_path=args.completed_manifest.resolve(),
    )
    manifest = validator.read_json(manifest_path)
    copied: list[dict[str, Any]] = []
    for record in manifest["production"]["core"]:
        source = exports / record["path"]
        target = assets / record["path"]
        copy_exclusive(source, target, record["sha256"])
        copied.append({"path": record["path"], "sha256": record["sha256"]})
    runtime = projector.project(manifest_path)
    runtime_bytes = projector.canonical_bytes(runtime)
    locator = assets / "manifests/wall-production-v1.wallset"
    if locator.exists():
        require(locator.read_bytes() == runtime_bytes, "runtime locator already differs")
    else:
        locator.parent.mkdir(parents=True, exist_ok=True)
        locator.write_bytes(runtime_bytes)
    receipt = {
        "asset_set_generation": runtime["asset_set_generation"],
        "asset_view_sha256": hashlib.sha256(
            "".join(record["sha256"] for record in copied).encode()
        ).hexdigest(),
        "authority": "art_preview",
        "core": copied,
        "manifest_sha256": runtime["manifest_sha256"],
        "schema_version": 1,
        "status": "provisioned",
    }
    receipt_path = destination / "provision.json"
    receipt_path.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"WALL_FORMWORK_PREVIEW_PROVISIONED destination={destination} manifest_sha256={runtime['manifest_sha256']}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProvisionError, projector.ProjectionError, validator.ManifestError) as error:
        print(f"Wall formwork preview provisioning failed: {error}")
        raise SystemExit(1) from error
