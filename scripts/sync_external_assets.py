#!/usr/bin/env python3
from __future__ import annotations

import argparse
import filecmp
import shutil
from pathlib import Path


ALLOWED_TOP_LEVEL_DIRS = ("textures", "models", "audio")


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
    return parser.parse_args()


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


def copy_if_needed(source_file: Path, source_top: Path, dest_top: Path, dry_run: bool) -> bool:
    relative_path = source_file.relative_to(source_top)
    dest_file = dest_top / relative_path
    needs_copy = not dest_file.exists() or not filecmp.cmp(source_file, dest_file, shallow=False)

    if not needs_copy:
        return False

    print(f"COPY {source_file} -> {dest_file}")
    if not dry_run:
        dest_file.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_file, dest_file)
    return True


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
