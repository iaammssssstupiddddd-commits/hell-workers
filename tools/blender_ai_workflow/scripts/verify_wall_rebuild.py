"""Capture and verify a deterministic rebuild of the six production Wall GLBs."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

FAMILIES = {
    "isolated": "wall_isolated",
    "end": "wall_end",
    "straight": "wall_straight",
    "corner": "wall_corner",
    "t_junction": "wall_t_junction",
    "cross": "wall_cross",
}
STRUCTURAL_FIELDS = {
    "schema_version",
    "status",
    "asset_set_id",
    "contract_sha256",
    "family",
    "arms",
    "mesh_count",
    "primitive_count",
    "triangle_count",
    "vertex_count",
    "raw_primitive_local_bounds",
    "node_transform",
    "uv0_present",
    "tangent_present",
    "embedded_images",
    "external_images",
    "cross_sections",
}


class RebuildError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RebuildError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> Any:
    require(path.is_file() and not path.is_symlink(), f"report is absent: {path}")
    return json.loads(path.read_text(encoding="utf-8"))


def snapshot(exports_root: Path, reports_root: Path) -> dict[str, Any]:
    families: dict[str, Any] = {}
    contract_hash: str | None = None
    for family, stem in FAMILIES.items():
        glb = exports_root / f"models/buildings/wall/{stem}.glb"
        post_path = reports_root / f"{stem}.post-export.json"
        require(glb.is_file() and not glb.is_symlink(), f"{family} GLB is absent")
        post = read_json(post_path)
        require(
            isinstance(post, dict)
            and post.get("status") == "pass"
            and post.get("asset_set_id") == "wall-production-v1"
            and post.get("family") == family,
            f"{family} post-export report failed",
        )
        digest = sha256(glb)
        require(post.get("glb_sha256") == digest, f"{family} report hash differs")
        structure = {field: post.get(field) for field in sorted(STRUCTURAL_FIELDS)}
        require(
            all(field in post for field in STRUCTURAL_FIELDS),
            f"{family} structural report fields are incomplete",
        )
        current_contract = post.get("contract_sha256")
        require(isinstance(current_contract, str), f"{family} contract hash is absent")
        if contract_hash is None:
            contract_hash = current_contract
        require(current_contract == contract_hash, "geometry contract hash differs")
        families[family] = {
            "path": f"models/buildings/wall/{stem}.glb",
            "bytes": glb.stat().st_size,
            "sha256": digest,
            "post_export_sha256": sha256(post_path),
            "structure": structure,
        }
    return {
        "geometry_contract_sha256": contract_hash,
        "families": families,
    }


def capture(exports_root: Path, reports_root: Path) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "status": "baseline",
        "asset_set_id": "wall-production-v1",
        **snapshot(exports_root, reports_root),
    }


def verify(
    baseline_path: Path, exports_root: Path, reports_root: Path
) -> dict[str, Any]:
    baseline = read_json(baseline_path)
    require(
        isinstance(baseline, dict)
        and set(baseline)
        == {
            "schema_version",
            "status",
            "asset_set_id",
            "geometry_contract_sha256",
            "families",
        }
        and baseline["schema_version"] == 1
        and baseline["status"] == "baseline"
        and baseline["asset_set_id"] == "wall-production-v1",
        "rebuild baseline identity differs",
    )
    rebuilt = snapshot(exports_root, reports_root)
    require(
        baseline["geometry_contract_sha256"] == rebuilt["geometry_contract_sha256"],
        "rebuild geometry contract differs",
    )
    require(baseline["families"] == rebuilt["families"], "rebuild output differs")
    return {
        "schema_version": 1,
        "status": "pass",
        "asset_set_id": "wall-production-v1",
        "baseline_sha256": sha256(baseline_path),
        "geometry_contract_sha256": rebuilt["geometry_contract_sha256"],
        "families": rebuilt["families"],
    }


def write_report(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", required=True, choices=("capture", "verify"))
    parser.add_argument("--exports-root", required=True, type=Path)
    parser.add_argument("--reports-root", required=True, type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    if args.mode == "capture":
        require(args.baseline is None, "capture mode rejects --baseline")
        payload = capture(args.exports_root.resolve(), args.reports_root.resolve())
    else:
        require(args.baseline is not None, "verify mode requires --baseline")
        payload = verify(
            args.baseline.resolve(),
            args.exports_root.resolve(),
            args.reports_root.resolve(),
        )
    write_report(args.output.resolve(), payload)
    print(f"WALL_REBUILD status={payload['status']} output={args.output.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RebuildError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"Wall rebuild verification failed: {error}")
        raise SystemExit(1) from error
