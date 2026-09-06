"""Provision an immutable Door ArtPreview asset view outside the repository."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

import project_door_preview as projector
import validate_door_manifest as validator


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
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--destination", required=True, type=Path)
    args = parser.parse_args()
    root = args.asset_root.resolve()
    destination = args.destination.resolve()
    require((root / "staging/validation").resolve() in destination.parents, "destination must be under staging/validation")
    manifest_path = args.manifest.resolve()
    validator.validate_manifest(manifest_path, exports_root=root / "staging/exports", reports_root=root / "staging/reports", blend_root=root / "staging/blend", licenses_root=root / "licenses", repo_root=args.repo.resolve())
    manifest = validator.read_json(manifest_path)
    copied = []
    for record in manifest["production"]["core"]:
        copy_exclusive(root / "staging/exports" / record["path"], destination / "assets" / record["path"], record["sha256"])
        copied.append({"path": record["path"], "sha256": record["sha256"]})
    runtime = projector.project(manifest_path)
    locator = destination / "assets/manifests/door-production-v1.doorset"
    locator.parent.mkdir(parents=True, exist_ok=True)
    runtime_bytes = projector.canonical_bytes(runtime)
    if locator.exists():
        require(locator.read_bytes() == runtime_bytes, "runtime locator already differs")
    else:
        locator.write_bytes(runtime_bytes)
    receipt = {"asset_set_generation": runtime["asset_set_generation"], "asset_view_sha256": hashlib.sha256("".join(record["sha256"] for record in copied).encode()).hexdigest(), "authority": "art_preview", "core": copied, "manifest_sha256": runtime["manifest_sha256"], "schema_version": 1, "status": "provisioned"}
    (destination / "provision.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"DOOR_PREVIEW_PROVISIONED destination={destination} manifest_sha256={runtime['manifest_sha256']}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProvisionError, projector.ProjectionError, validator.ManifestError) as error:
        print(f"Door preview provisioning failed: {error}")
        raise SystemExit(1) from error

