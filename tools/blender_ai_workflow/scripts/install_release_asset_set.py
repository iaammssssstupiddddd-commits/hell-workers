"""Install an active canonical Wall/Door release; switch the runtime locator last.

Dry-run is the default. This is a mirror operation, not release approval or
promotion: the canonical active pointer and immutable receipt must already exist.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any

import asset_release_manifest as release
import project_doorset
import project_wallset
import promote_asset_set as promotion

PROJECT_ROOT = Path(__file__).resolve().parents[3]
if str(PROJECT_ROOT / "scripts") not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT / "scripts"))
import sync_external_assets as sync


def locator_path(asset_id: str) -> Path:
    extension = "wallset" if asset_id == release.WALL_ID else "doorset"
    return Path("manifests") / f"{asset_id}.{extension}"


def validate_locator_destination(destination: Path) -> None:
    for path in (destination, *destination.parents):
        promotion.require(not path.is_symlink(), f"runtime locator path is a symlink: {path}")
    promotion.require(not destination.exists() or destination.is_file(), "runtime locator is not a file")


def install(
    manifest_path: Path, dest_root: Path, *, repo: Path | None, apply: bool
) -> dict[str, Any]:
    promotion.require(manifest_path.parent.name == "manifest", "install requires a promoted manifest")
    manifest = release.wall.read_json(manifest_path)
    asset_id, _version = release.identity(manifest)
    generation_root = manifest_path.parent.parent
    receipt = generation_root / "authority/promotion-receipt.json"
    projector = project_wallset if asset_id == release.WALL_ID else project_doorset
    projection = projector.project_release(manifest_path, receipt)
    locator = dest_root / locator_path(asset_id)
    validate_locator_destination(locator)
    receipt_dest = sync.ensure_safe_destination(dest_root, Path(projection["receipt"]["path"]))
    runtime_generation = receipt_dest.parent.parent
    allowed = {dest_root / record["path"] for record in [*projection["core"], projection["receipt"]]}
    for path in tuple(allowed):
        allowed.update(parent for parent in path.parents if parent.is_relative_to(runtime_generation))
    if runtime_generation.exists():
        for path in runtime_generation.rglob("*"):
            promotion.require(path in allowed and not path.is_symlink(), f"unexpected runtime generation entry: {path}")
    promotion.require(
        not receipt_dest.exists() or (receipt_dest.is_file() and promotion.sha256(receipt_dest) == projection["receipt"]["sha256"]),
        "immutable runtime receipt differs",
    )
    arguments = dict(source_root=generation_root / "exports", dest_root=dest_root,
        manifest_path=manifest_path, selection="core", receipt_path=receipt, repo=repo)
    # Validate the whole copy set, receipt, pointer and locator before any write.
    copies = sync.sync_manifest_assets(**arguments, dry_run=True)
    payload = promotion.canonical_json(projection)
    previous = promotion.sha256(locator) if locator.exists() else None
    changed = not locator.exists() or locator.read_bytes() != payload
    if apply:
        sync.sync_manifest_assets(**arguments, dry_run=False)
        if not receipt_dest.exists():
            promotion.copy_verified(receipt, receipt_dest, projection["receipt"]["sha256"])
        promotion.fsync_tree(runtime_generation, lambda _stage: None)
        # Seal the directory entries up to the runtime asset root, not just the
        # immutable files, before making the locator point at them.
        ancestor = runtime_generation.parent
        while ancestor.is_relative_to(dest_root):
            promotion.fsync_directory(ancestor)
            if ancestor == dest_root:
                break
            ancestor = ancestor.parent
        promotion.require((promotion.sha256(locator) if locator.exists() else None) == previous,
            "runtime locator changed during installation")
        sync.validate_receipt(receipt, manifest_path, release.wall)
        if changed:
            promotion.write_atomic(locator, payload, f"install-g{manifest['asset_set_generation']}")
            promotion.fsync_directory(locator.parent)
            promotion.fsync_directory(dest_root)
    return {"status": "installed" if apply else "planned", "asset_set_id": asset_id,
        "asset_set_generation": manifest["asset_set_generation"], "core_copies": copies,
        "locator": str(locator), "locator_changed": changed,
        "previous_locator_sha256": previous, "locator_sha256": promotion.bytes_sha256(payload)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--dest", required=True, type=Path)
    parser.add_argument("--repo", type=Path, default=PROJECT_ROOT)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    # Preserve the destination spelling until symlink checks have run.
    result = install(args.manifest.absolute(), args.dest.absolute(), repo=args.repo.resolve(), apply=args.apply)
    print(promotion.canonical_json(result).decode(), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
