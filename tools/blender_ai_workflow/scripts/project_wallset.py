"""Project a validated Wall asset-set manifest into canonical runtime JSON."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
if str(SCRIPT_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPT_ROOT))

import validate_asset_set_manifest as validator


class ProjectionError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProjectionError(message)


def canonical_bytes(payload: dict[str, Any]) -> bytes:
    return (
        json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode()


def project_candidate(manifest_path: Path) -> dict[str, Any]:
    manifest = validator.read_json(manifest_path)
    require(
        isinstance(manifest, dict)
        and manifest.get("schema_version") == 2
        and manifest.get("asset_set_id") == "wall-production-v1"
        and manifest.get("manifest_mode") == "candidate"
        and manifest.get("normal_decision") == "pending",
        "candidate manifest identity differs",
    )
    core = manifest.get("production", {}).get("core")
    optional = manifest.get("production", {}).get("optional")
    require(
        isinstance(core, list) and len(core) == 8, "candidate core inventory differs"
    )
    require(
        isinstance(optional, list)
        and len(optional) == 1
        and optional[0].get("role") == "texture:normal",
        "candidate normal inventory differs",
    )
    return {
        "schema_version": 1,
        "asset_set_id": "wall-production-v1",
        "asset_set_generation": manifest["asset_set_generation"],
        "authority": "candidate",
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": "pending",
        "core": core,
        "candidate_normal": optional[0],
        "receipt": None,
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--blend-root", required=True, type=Path)
    parser.add_argument("--exports-root", required=True, type=Path)
    parser.add_argument("--reports-root", required=True, type=Path)
    parser.add_argument("--licenses-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    manifest_path = args.manifest.resolve()
    validator.validate_manifest(
        manifest_path,
        mode="candidate",
        blend_root=args.blend_root.resolve(),
        exports_root=args.exports_root.resolve(),
        reports_root=args.reports_root.resolve(),
        licenses_root=args.licenses_root.resolve(),
        repo=args.repo.resolve(),
    )
    projection = project_candidate(manifest_path)
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    temporary.write_bytes(canonical_bytes(projection))
    temporary.replace(output)
    print(
        f"WALLSET_PROJECT status=pass generation={projection['asset_set_generation']} "
        f"output={output}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ProjectionError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wallset projection failed: {error}")
        raise SystemExit(1) from error
