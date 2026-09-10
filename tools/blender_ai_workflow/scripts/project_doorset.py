"""Project a validated final Door manifest and receipt into release runtime JSON."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import Any

import asset_release_manifest as release_manifest
import project_door_candidate as candidate
import project_wallset as common


def runtime_path(generation: int, record: dict[str, Any]) -> str:
    source = Path(record["path"])
    relative = Path("models") / source.name if record["role"].startswith("mesh:") else source
    return (Path("door_sets") / str(generation) / relative).as_posix()


def project_release(manifest_path: Path, receipt_path: Path) -> dict[str, Any]:
    projection = candidate.project(manifest_path)
    common.validate_release_receipt(manifest_path, receipt_path, release_manifest.DOOR_ID)
    generation = projection["asset_set_generation"]
    return {
        **projection,
        "authority": "release_approved",
        "core": [{**record, "path": runtime_path(generation, record)} for record in projection["core"]],
        "receipt": {
            "bytes": receipt_path.stat().st_size,
            "path": f"door_sets/{generation}/authority/promotion-receipt.json",
            "sha256": common.validator.sha256(receipt_path),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    manifest = args.manifest.resolve()
    release_manifest.validate(manifest, repo=args.repo.resolve())
    projection = project_release(manifest, args.receipt.resolve())
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    temporary.write_bytes(common.canonical_bytes(projection))
    temporary.replace(output)
    print(f"DOORSET_PROJECT status=pass generation={projection['asset_set_generation']} output={output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
