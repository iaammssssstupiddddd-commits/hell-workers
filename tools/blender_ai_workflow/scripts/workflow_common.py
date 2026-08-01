"""Shared path and report helpers for the Hell Workers Blender workflow."""

from __future__ import annotations

import argparse
import json
import os
import tempfile
from pathlib import Path
from typing import Any

DEFAULT_ASSET_ROOT = Path.home() / "Sync" / "hell-workers-assets"


def script_arguments() -> list[str]:
    """Return arguments after Blender's `--` separator."""
    import sys

    if "--" not in sys.argv:
        return []
    return sys.argv[sys.argv.index("--") + 1 :]


def asset_root() -> Path:
    configured = os.environ.get("HELL_WORKERS_ASSET_ROOT")
    return Path(configured).expanduser().resolve() if configured else DEFAULT_ASSET_ROOT


def is_within(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
    except ValueError:
        return False
    return True


def require_within(path: Path, root: Path, label: str) -> Path:
    resolved = path.expanduser().resolve()
    if not is_within(resolved, root):
        raise ValueError(f"{label} must be under {root}: {resolved}")
    return resolved


def staging_path(path: str | Path, kind: str) -> Path:
    root = asset_root() / "staging" / kind
    resolved = require_within(Path(path), root, f"{kind} output")
    resolved.parent.mkdir(parents=True, exist_ok=True)
    return resolved


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rendered = f"{json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True)}\n"
    descriptor, temporary_name = tempfile.mkstemp(
        dir=path.parent,
        prefix=f".{path.name}.",
        suffix=".tmp",
        text=True,
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            handle.write(rendered)
            handle.flush()
            os.fsync(handle.fileno())
        temporary_path.replace(path)
    finally:
        temporary_path.unlink(missing_ok=True)


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("must be greater than zero")
    return parsed
