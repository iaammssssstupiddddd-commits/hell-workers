#!/usr/bin/env python3
from __future__ import annotations

import argparse
import filecmp
import hashlib
import importlib.util
import shutil
from collections.abc import Iterable
from pathlib import Path
from types import ModuleType
from typing import Any

ALLOWED_TOP_LEVEL_DIRS = ("textures", "models", "audio")
# A promoted asset set lands in its own generation directory, which the runtime
# projection names; the mirror sweep below never walks it.
ALLOWED_MANIFEST_TOP_LEVEL_DIRS = (*ALLOWED_TOP_LEVEL_DIRS, "wall_sets")
PROJECT_ROOT = Path(__file__).resolve().parents[1]
WALL_MANIFEST_VALIDATOR = (
    PROJECT_ROOT / "tools/blender_ai_workflow/scripts/validate_asset_set_manifest.py"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Sync exported assets from an external shared folder into the repository assets directory."
    )
    parser.add_argument(
        "--source",
        required=True,
        type=Path,
        help="Path to the external exports directory (for example ~/Sync/hell-workers-assets/exports).",
    )
    parser.add_argument(
        "--dest",
        default=Path("assets"),
        type=Path,
        help="Repository assets directory. Defaults to ./assets.",
    )
    parser.add_argument(
        "--delete-missing",
        action="store_true",
        help="Delete synced files from the destination when they no longer exist in the source.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print planned operations without copying or deleting files.",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        help="Wall asset-set manifest v2. Requires --selection and copies only its exact allowlist.",
    )
    parser.add_argument(
        "--selection",
        choices=("core",),
        help="Manifest inventory to copy. Requires --manifest.",
    )
    parser.add_argument(
        "--receipt",
        type=Path,
        help=(
            "Promotion receipt of a canonical generation. Required when the "
            "manifest lives in a promoted generation, refused otherwise."
        ),
    )
    return parser.parse_args()


def manifest_roots(manifest_path: Path, source_root: Path) -> dict[str, Path] | None:
    """Locate the roots of a promoted generation, or None for a staging tree.

    A generation keeps the blend under `source/blender` and carries its own
    reports and licenses, while staging shares one `licenses` directory with
    every other candidate.
    """
    generation = manifest_path.parent.parent
    if manifest_path.parent.name != "manifest" or source_root != generation / "exports":
        return None
    return {
        "blend_root": generation / "source" / "blender",
        "exports_root": source_root,
        "reports_root": generation / "reports",
        "licenses_root": generation / "licenses",
        "generation_root": generation,
    }


def ensure_valid_source(source_root: Path) -> None:
    if not source_root.exists():
        raise FileNotFoundError(f"Source directory does not exist: {source_root}")
    if not source_root.is_dir():
        raise NotADirectoryError(f"Source path is not a directory: {source_root}")


def iter_source_files(source_top: Path):
    if not source_top.exists():
        return
    for path in sorted(source_top.rglob("*")):
        if path.is_file():
            yield path


def copy_if_needed(
    source_file: Path, source_top: Path, dest_top: Path, dry_run: bool
) -> bool:
    relative_path = source_file.relative_to(source_top)
    dest_file = dest_top / relative_path
    needs_copy = not dest_file.exists() or not filecmp.cmp(
        source_file, dest_file, shallow=False
    )

    if not needs_copy:
        return False

    print(f"COPY {source_file} -> {dest_file}")
    if not dry_run:
        dest_file.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_file, dest_file)
    return True


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_wall_manifest_validator() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "validate_asset_set_manifest", WALL_MANIFEST_VALIDATOR
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("Cannot load Wall asset-set manifest validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ensure_safe_destination(dest_root: Path, relative: Path) -> Path:
    if relative.is_absolute() or ".." in relative.parts or "." in relative.parts:
        raise ValueError(f"Manifest destination path escapes asset root: {relative}")
    if relative.parts[0] not in ALLOWED_MANIFEST_TOP_LEVEL_DIRS:
        raise ValueError(f"Manifest destination top-level is not allowed: {relative}")

    resolved_root = dest_root.resolve()
    destination = dest_root / relative
    if not destination.resolve(strict=False).is_relative_to(resolved_root):
        raise ValueError(f"Manifest destination path escapes asset root: {relative}")
    current = dest_root
    for part in relative.parts[:-1]:
        current /= part
        if current.is_symlink():
            raise ValueError(f"Manifest destination parent is a symlink: {current}")
    if destination.is_symlink():
        raise ValueError(f"Manifest destination is a symlink: {destination}")
    return destination


def selected_manifest_records(
    manifest: dict[str, Any], selection: str
) -> Iterable[dict[str, str]]:
    if selection != "core":
        raise ValueError("Only the approved core inventory may be synchronized")
    return manifest["production"]["core"]


def load_wallset_projector() -> ModuleType:
    path = PROJECT_ROOT / "tools/blender_ai_workflow/scripts/project_wallset.py"
    spec = importlib.util.spec_from_file_location("project_wallset", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("Cannot load Wall runtime projector")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_receipt(receipt_path: Path, manifest_path: Path, validator) -> dict:
    """Bind the receipt to this generation, its manifest bytes and its pointer."""
    receipt = validator.read_json(receipt_path)
    manifest_hash = sha256(manifest_path)
    generation_root = manifest_path.parent.parent
    asset_root = generation_root.parent.parent
    if receipt.get("manifest_sha256") != manifest_hash:
        raise ValueError("Promotion receipt does not describe this manifest")
    if str(receipt.get("asset_set_generation")) != generation_root.name:
        raise ValueError("Promotion receipt generation differs from its directory")
    pointer_path = asset_root / "authority" / "wall-production-v1.active.json"
    pointer = validator.read_json(pointer_path)
    if (
        pointer.get("manifest_sha256") != manifest_hash
        or pointer.get("asset_set_generation") != receipt.get("asset_set_generation")
        or pointer.get("receipt_sha256") != sha256(receipt_path)
    ):
        raise ValueError("Active pointer does not reference this receipt")
    return receipt


def sync_manifest_assets(
    *,
    source_root: Path,
    dest_root: Path,
    manifest_path: Path,
    selection: str,
    dry_run: bool,
    repo: Path | None,
    receipt_path: Path | None = None,
) -> int:
    if dest_root.is_symlink():
        raise ValueError(f"Destination asset root is a symlink: {dest_root}")
    if dest_root.exists() and not dest_root.is_dir():
        raise NotADirectoryError(
            f"Destination asset root is not a directory: {dest_root}"
        )
    validator = load_wall_manifest_validator()
    manifest = validator.read_json(manifest_path)
    mode = manifest.get("manifest_mode") if isinstance(manifest, dict) else None
    if mode != "final":
        raise ValueError(
            "Manifest sync accepts only a final art-approved candidate; release requires a promotion receipt"
        )

    roots = manifest_roots(manifest_path, source_root)
    if roots is None:
        if receipt_path is not None:
            raise ValueError("--receipt applies only to a promoted generation")
        staging_root = source_root.parent
        roots = {
            "blend_root": staging_root / "blend",
            "exports_root": source_root,
            "reports_root": staging_root / "reports",
            "licenses_root": staging_root.parent / "licenses",
        }
    elif receipt_path is None:
        raise ValueError("A promoted generation requires --receipt")
    else:
        validate_receipt(receipt_path, manifest_path, validator)
    validation = validator.validate_manifest(
        manifest_path,
        mode="final",
        blend_root=roots["blend_root"],
        exports_root=roots["exports_root"],
        reports_root=roots["reports_root"],
        licenses_root=roots["licenses_root"],
        repo=repo,
    )
    records = selected_manifest_records(manifest, selection)
    # A released generation keeps its own directory in the repository so the
    # runtime projection can name one immutable generation, while a staging
    # candidate keeps the manifest-relative layout. The destination has to be
    # the projector's own mapping, not a copy of its shape.
    projector = load_wallset_projector()
    generation = manifest["asset_set_generation"]

    def destination_for(record: dict) -> Path:
        if receipt_path is None:
            return Path(record["path"])
        return Path(projector.runtime_path(generation, record))

    if not dry_run:
        dest_root.mkdir(parents=True, exist_ok=True)
    copied = 0
    for record in records:
        relative = destination_for(record)
        source_file = source_root / record["path"]
        destination = ensure_safe_destination(dest_root, relative)
        needs_copy = (
            not destination.is_file() or sha256(destination) != record["sha256"]
        )
        if not needs_copy:
            continue
        print(f"COPY {source_file} -> {destination}")
        if not dry_run:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_file, destination)
            if sha256(destination) != record["sha256"]:
                raise OSError(f"Copied asset hash differs: {destination}")
        copied += 1
    print(
        "MANIFEST "
        f"asset_set_id={validation['asset_set_id']} "
        f"generation={validation['asset_set_generation']} "
        f"sha256={validation['manifest_sha256']} selection={selection}"
    )
    return copied


def delete_missing_files(source_top: Path, dest_top: Path, dry_run: bool) -> int:
    removed = 0
    if not dest_top.exists():
        return removed

    for dest_file in sorted(dest_top.rglob("*")):
        if not dest_file.is_file():
            continue

        relative_path = dest_file.relative_to(dest_top)
        source_file = source_top / relative_path
        if source_file.exists():
            continue

        print(f"DELETE {dest_file}")
        if not dry_run:
            dest_file.unlink()
        removed += 1

    return removed


def main() -> int:
    args = parse_args()
    source_root = args.source.expanduser().resolve()
    dest_root = args.dest.expanduser().resolve()

    ensure_valid_source(source_root)

    if (args.manifest is None) != (args.selection is None):
        raise ValueError("--manifest and --selection must be specified together")
    if args.manifest is not None:
        if args.delete_missing:
            raise ValueError("--delete-missing is not supported with manifest sync")
        copied = sync_manifest_assets(
            source_root=source_root,
            dest_root=dest_root,
            manifest_path=args.manifest.expanduser().resolve(),
            selection=args.selection,
            dry_run=args.dry_run,
            repo=PROJECT_ROOT,
            receipt_path=(
                None if args.receipt is None else args.receipt.expanduser().resolve()
            ),
        )
        print(
            f"DONE copied={copied} removed=0 dry_run={'yes' if args.dry_run else 'no'}"
        )
        return 0

    copied = 0
    removed = 0

    for top_level in ALLOWED_TOP_LEVEL_DIRS:
        source_top = source_root / top_level
        dest_top = dest_root / top_level

        if not source_top.exists():
            print(f"SKIP missing source directory: {source_top}")
            continue

        for source_file in iter_source_files(source_top):
            if copy_if_needed(source_file, source_top, dest_top, args.dry_run):
                copied += 1

        if args.delete_missing:
            removed += delete_missing_files(source_top, dest_top, args.dry_run)

    print(
        "DONE "
        f"copied={copied} removed={removed} "
        f"dry_run={'yes' if args.dry_run else 'no'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
