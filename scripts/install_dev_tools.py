"""Explicitly install the three pinned Linux x86_64 quality-tool binaries."""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import platform
import tarfile
import tempfile
import urllib.request
from pathlib import Path

try:
    from dev_tools import TOOL_NAMES, load_manifest
    from cargo_runtime import persistent_storage_error
except ModuleNotFoundError:
    from scripts.dev_tools import TOOL_NAMES, load_manifest
    from scripts.cargo_runtime import persistent_storage_error


def install_archive(name: str, spec: dict[str, str], destination: Path) -> None:
    with urllib.request.urlopen(spec["url"], timeout=60) as response:
        archive = response.read()
    if hashlib.sha256(archive).hexdigest() != spec["sha256"]:
        raise RuntimeError(f"{name}: archive SHA-256 mismatch")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as bundle:
        members = [m for m in bundle.getmembers() if m.isfile() and Path(m.name).name == name]
        if len(members) != 1:
            raise RuntimeError(f"{name}: expected one regular binary in archive")
        source = bundle.extractfile(members[0])
        if source is None:
            raise RuntimeError(f"{name}: missing archive payload")
        binary = source.read()
    destination.mkdir(parents=True, exist_ok=True)
    # Extract just the named binary; never interpret archive paths on disk.
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination, prefix=f".{name}-", delete=False) as output:
            temporary = Path(output.name)
            output.write(binary)
            output.flush()
            os.fsync(output.fileno())
        temporary.chmod(0o755)
        temporary.replace(destination / name)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    print(f"Installed {name} {spec['version']}: {destination / name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bin-dir", required=True, type=Path)
    parser.add_argument("--tool", choices=TOOL_NAMES, action="append")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() not in {"x86_64", "AMD64"}:
        parser.error("these archives support Linux x86_64 only; see docs/DEVELOPMENT.md")
    destination = args.bin_dir.expanduser().resolve()
    error = persistent_storage_error(destination, label="quality tools")
    if error:
        parser.error(error)
    manifest = load_manifest(Path(__file__).resolve().parent.parent)
    for name in args.tool or TOOL_NAMES:
        install_archive(name, manifest[name], destination)


if __name__ == "__main__":
    main()
