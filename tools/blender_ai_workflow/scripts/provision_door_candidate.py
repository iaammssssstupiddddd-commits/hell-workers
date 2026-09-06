"""Provision an immutable Door isolated-candidate asset view outside the repository."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import project_door_candidate as projector
import project_door_preview as preview_projector
import provision_door_preview as preview_provisioner
import validate_door_manifest as validator


class ProvisionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProvisionError(message)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--destination", required=True, type=Path)
    args = parser.parse_args()
    root = args.asset_root.resolve()
    destination = args.destination.resolve()
    require(
        (root / "staging/validation").resolve() in destination.parents,
        "destination must be under staging/validation",
    )
    manifest_path = args.manifest.resolve()
    validator.validate_manifest(
        manifest_path,
        exports_root=root / "staging/exports",
        reports_root=root / "staging/reports",
        blend_root=root / "staging/blend",
        licenses_root=root / "licenses",
        repo_root=args.repo.resolve(),
    )
    manifest = validator.read_json(manifest_path)
    copied = []
    for record in manifest["production"]["core"]:
        preview_provisioner.copy_exclusive(
            root / "staging/exports" / record["path"],
            destination / "assets" / record["path"],
            record["sha256"],
        )
        copied.append({"path": record["path"], "sha256": record["sha256"]})
    runtime = projector.project(manifest_path)
    locator = destination / "assets/manifests/door-production-v1.doorset"
    locator.parent.mkdir(parents=True, exist_ok=True)
    runtime_bytes = preview_projector.canonical_bytes(runtime)
    if locator.exists():
        require(locator.read_bytes() == runtime_bytes, "runtime locator already differs")
    else:
        locator.write_bytes(runtime_bytes)
    receipt = {
        "asset_set_generation": runtime["asset_set_generation"],
        "asset_view_sha256": hashlib.sha256(
            "".join(record["sha256"] for record in copied).encode()
        ).hexdigest(),
        "authority": "isolated_candidate",
        "core": copied,
        "manifest_sha256": runtime["manifest_sha256"],
        "schema_version": 1,
        "status": "provisioned",
    }
    (destination / "provision.json").write_text(
        json.dumps(receipt, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(
        "DOOR_CANDIDATE_PROVISIONED "
        f"destination={destination} manifest_sha256={runtime['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ProvisionError,
        projector.ProjectionError,
        preview_provisioner.ProvisionError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Door candidate provisioning failed: {error}")
        raise SystemExit(1) from error
