"""Immutable executable input for formal art evidence across Memory rebuilds."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import shutil

CONTRACT = "building-native-executable-snapshot-v1"


def verifier_sha256():
    return digest(Path(__file__))


def digest(path: Path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def snapshot_path(root: Path):
    if not root.is_absolute() or root.resolve() != root or not root.is_dir():
        raise ValueError("executable snapshot requires a canonical existing job root")
    return root / "binary" / "bevy_app"


def freeze(source: Path, root: Path):
    destination = snapshot_path(root)
    if source.is_symlink() or not source.is_file() or not os.access(source, os.X_OK):
        raise ValueError("snapshot source must be a regular executable")
    before = digest(source)
    destination.parent.mkdir(mode=0o700, exist_ok=False)
    with source.open("rb") as input_file, destination.open("xb") as output:
        shutil.copyfileobj(input_file, output, length=1024 * 1024)
        output.flush()
        os.fsync(output.fileno())
        os.fchmod(output.fileno(), 0o555)
    if digest(destination) != before or digest(source) != before:
        raise ValueError("executable changed while creating its immutable input")
    validate(destination, root, before)
    return destination


def validate(path: Path, root: Path, expected_sha256: str):
    if (path != snapshot_path(root) or path.is_symlink() or path.resolve() != path
            or not path.is_file() or not os.access(path, os.X_OK) or path.stat().st_mode & 0o222
            or not isinstance(expected_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_sha256)
            or digest(path) != expected_sha256):
        raise ValueError("formal executable snapshot path/bytes/permissions changed")


def validate_cache(path: Path, repo: Path, feedback: bool, expected_sha256: str):
    expected = repo / "target" / ("debug" if feedback else "profiling") / "bevy_app"
    if (path != expected or path.is_symlink() or path.resolve() != path
            or not path.is_file() or not os.access(path, os.X_OK)
            or not isinstance(expected_sha256, str) or not re.fullmatch(r"[0-9a-f]{64}", expected_sha256)
            or digest(path) != expected_sha256):
        raise ValueError("shared executable cache path/bytes changed")
