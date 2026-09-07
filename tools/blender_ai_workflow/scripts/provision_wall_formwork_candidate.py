"""Provision an immutable Wall formwork isolated-candidate asset view."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

import project_wall_formwork_candidate as projector
import project_wall_formwork_preview as preview_projector
import provision_wall_formwork_preview as preview_provisioner
import validate_wall_formwork_manifest as validator


class ProvisionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProvisionError(message)


def provision(
    *,
    asset_root: Path,
    manifest_path: Path,
    completed_manifest_path: Path,
    destination: Path,
) -> dict[str, Any]:
    root = asset_root.resolve()
    destination = destination.resolve()
    require(
        (root / "staging/validation").resolve() in destination.parents,
        "destination must be under staging/validation",
    )
    reports = root / "staging/reports"
    exports = root / "staging/exports"
    validator.validate_manifest(
        manifest_path.resolve(),
        exports_root=exports,
        reports_root=reports,
        blend_root=root / "staging/blend",
        licenses_root=root / "licenses",
        completed_manifest_path=completed_manifest_path.resolve(),
    )
    manifest = validator.read_json(manifest_path.resolve())
    runtime = projector.project(manifest_path.resolve())
    copied: list[dict[str, str]] = []
    for record in manifest["production"]["core"]:
        preview_provisioner.copy_exclusive(
            exports / record["path"],
            destination / "assets" / record["path"],
            record["sha256"],
        )
        copied.append({"path": record["path"], "sha256": record["sha256"]})

    locator = destination / "assets/manifests/wall-production-v1.wallset"
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
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--destination", required=True, type=Path)
    args = parser.parse_args()
    receipt = provision(
        asset_root=args.asset_root,
        manifest_path=args.manifest,
        completed_manifest_path=args.completed_manifest,
        destination=args.destination,
    )
    print(
        "WALL_FORMWORK_CANDIDATE_PROVISIONED "
        f"destination={args.destination.resolve()} "
        f"manifest_sha256={receipt['manifest_sha256']}"
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
        print(f"Wall formwork candidate provisioning failed: {error}")
        raise SystemExit(1) from error
