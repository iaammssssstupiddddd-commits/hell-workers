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
import promote_asset_set as promotion


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
        and manifest.get("manifest_mode") == "final"
        and manifest.get("normal_decision") in {"adopted", "rejected"}
        and manifest.get("art_review", {}).get("status") == "art_approved",
        "candidate manifest identity differs",
    )
    core = manifest.get("production", {}).get("core")
    optional = manifest.get("production", {}).get("optional")
    expected_length = 9 if manifest["normal_decision"] == "adopted" else 8
    require(
        isinstance(core, list) and len(core) == expected_length,
        "candidate core inventory differs",
    )
    require(optional == [], "candidate optional inventory must be empty")
    return {
        "schema_version": 1,
        "asset_set_id": "wall-production-v1",
        "asset_set_generation": manifest["asset_set_generation"],
        "authority": "isolated_candidate",
        "manifest_sha256": validator.sha256(manifest_path),
        "normal_decision": manifest["normal_decision"],
        "review_status": "art_approved",
        "core": core,
        "candidate_normal": None,
        "receipt": None,
    }


def runtime_path(generation: int, record: dict[str, Any]) -> str:
    source = Path(record["path"])
    if str(record["role"]).startswith("mesh:"):
        relative = Path("models") / source.name
    else:
        relative = source
    return (Path("wall_sets") / str(generation) / relative).as_posix()


def project_release(manifest_path: Path, receipt_path: Path) -> dict[str, Any]:
    require(
        receipt_path.is_file() and not receipt_path.is_symlink(),
        "release receipt is absent",
    )
    manifest = validator.read_json(manifest_path)
    require(
        isinstance(manifest, dict)
        and manifest.get("schema_version") == 2
        and manifest.get("asset_set_id") == "wall-production-v1"
        and manifest.get("manifest_mode") == "final"
        and manifest.get("normal_decision") in {"adopted", "rejected"}
        and manifest.get("art_review", {}).get("status") == "art_approved",
        "release manifest identity differs",
    )
    core = manifest.get("production", {}).get("core")
    optional = manifest.get("production", {}).get("optional")
    expected_length = 9 if manifest["normal_decision"] == "adopted" else 8
    require(
        isinstance(core, list) and len(core) == expected_length,
        "release core inventory differs",
    )
    require(optional == [], "release optional inventory must be empty")
    generation = manifest["asset_set_generation"]
    manifest_hash = validator.sha256(manifest_path)
    receipt_bytes = receipt_path.read_bytes()
    receipt = validator.read_json(receipt_path)
    require(
        isinstance(receipt, dict)
        and set(receipt) == promotion.RECEIPT_FIELDS
        and receipt.get("schema_version") == 1
        and receipt.get("asset_set_id") == "wall-production-v1"
        and receipt.get("asset_set_generation") == generation
        and receipt.get("manifest_sha256") == manifest_hash
        and receipt.get("new_active")
        == {
            "asset_set_id": "wall-production-v1",
            "asset_set_generation": generation,
            "manifest_sha256": manifest_hash,
        },
        "release receipt binding differs",
    )
    require(
        isinstance(receipt["receipt_id"], str)
        and promotion.RECEIPT_ID.fullmatch(receipt["receipt_id"]) is not None
        and all(
            isinstance(receipt[field], str)
            and promotion.HEX64.fullmatch(receipt[field]) is not None
            for field in ("manifest_sha256", "promotion_plan_sha256")
        )
        and all(
            isinstance(receipt[field], str) and len(receipt[field]) == 40
            for field in ("tool_commit", "tool_tree")
        ),
        "release receipt authority fields differ",
    )
    for field, expected_path in (
        ("m5_evidence_bundle", "evidence/m5-evidence-bundle.json"),
        ("approval", "evidence/release-approval.json"),
    ):
        record = receipt[field]
        expected_fields = (
            {"approved_at_utc", "path", "sha256"}
            if field == "approval"
            else {"path", "sha256"}
        )
        require(
            isinstance(record, dict)
            and set(record) == expected_fields
            and record["path"] == expected_path
            and isinstance(record["sha256"], str)
            and promotion.HEX64.fullmatch(record["sha256"]) is not None,
            f"release receipt {field} differs",
        )
    promotion.validate_timestamp(receipt["approval"]["approved_at_utc"], "approval")
    previous = receipt["previous_active"]
    require(
        isinstance(previous, dict) and previous.get("status") in {"absent", "present"},
        "release receipt preimage differs",
    )
    if previous["status"] == "absent":
        require(
            previous == {"status": "absent"},
            "release receipt absent preimage differs",
        )
    else:
        require(
            set(previous)
            == {
                "asset_set_generation",
                "manifest_sha256",
                "receipt_sha256",
                "sha256",
                "status",
            }
            and type(previous["asset_set_generation"]) is int
            and previous["asset_set_generation"] > 0
            and all(
                isinstance(previous[field], str)
                and promotion.HEX64.fullmatch(previous[field]) is not None
                for field in ("manifest_sha256", "receipt_sha256", "sha256")
            ),
            "release receipt present preimage differs",
        )
    require(
        receipt_bytes == canonical_bytes(receipt),
        "release receipt is not canonical JSON",
    )
    projected_core = [
        {**record, "path": runtime_path(generation, record)} for record in core
    ]
    return {
        "schema_version": 1,
        "asset_set_id": "wall-production-v1",
        "asset_set_generation": generation,
        "authority": "release_approved",
        "manifest_sha256": manifest_hash,
        "normal_decision": manifest["normal_decision"],
        "core": projected_core,
        "candidate_normal": None,
        "receipt": {
            "bytes": len(receipt_bytes),
            "path": (
                Path("wall_sets")
                / str(generation)
                / "authority/promotion-receipt.json"
            ).as_posix(),
            "sha256": validator.sha256(receipt_path),
        },
        "review_status": "art_approved",
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--authority",
        choices=("isolated_candidate", "release_approved"),
        default="isolated_candidate",
    )
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--receipt", type=Path)
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
    mode = "final"
    validator.validate_manifest(
        manifest_path,
        mode=mode,
        blend_root=args.blend_root.resolve(),
        exports_root=args.exports_root.resolve(),
        reports_root=args.reports_root.resolve(),
        licenses_root=args.licenses_root.resolve(),
        repo=args.repo.resolve(),
    )
    if args.authority == "isolated_candidate":
        require(args.receipt is None, "candidate projection does not accept a receipt")
        projection = project_candidate(manifest_path)
    else:
        require(args.receipt is not None, "release projection requires --receipt")
        projection = project_release(manifest_path, args.receipt.resolve())
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
