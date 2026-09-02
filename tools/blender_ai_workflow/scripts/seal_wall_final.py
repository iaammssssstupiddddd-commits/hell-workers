"""Seal an approved Wall candidate as a new immutable final manifest."""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
if str(SCRIPT_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPT_ROOT))

import seal_wall_candidate as candidate_sealer
import validate_asset_set_manifest as validator

ASSET_SET_ID = "wall-production-v1"
FINAL_NAME = f"{ASSET_SET_ID}.asset-set-final.json"


class FinalSealError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FinalSealError(message)


def validate_timestamp(value: str, label: str) -> str:
    require(bool(value), f"{label} timestamp is empty")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise FinalSealError(f"{label} timestamp is invalid") from error
    require(parsed.tzinfo is not None, f"{label} timestamp requires timezone")
    return value


def validate_approval(
    path: Path, *, candidate_manifest: dict[str, Any], candidate_hash: str
) -> None:
    approval = candidate_sealer.read_json(path, "art approval artifact")
    require(
        isinstance(approval, dict)
        and approval.get("schema_version") == 1
        and approval.get("asset_set_id") == ASSET_SET_ID
        and approval.get("asset_set_generation")
        == candidate_manifest["asset_set_generation"]
        and approval.get("manifest_sha256") == candidate_hash
        and approval.get("normal_decision")
        == "rejected_by_missing_mesh_tangents"
        and approval.get("decision") == "lit_approved",
        "art approval identity differs",
    )
    selected = approval.get("approval")
    lit = approval.get("comparisons", {}).get("lit", {})
    require(
        isinstance(selected, dict)
        and selected.get("approved_by") == "user"
        and selected.get("selection") == "lit"
        and selected.get("selected_job") == lit.get("job")
        and selected.get("selected_screenshot_sha256")
        == lit.get("screenshot_sha256")
        and re.fullmatch(r"[0-9a-f]{64}", lit.get("screenshot_sha256", ""))
        is not None,
        "art approval selection differs",
    )
    validate_timestamp(selected.get("approved_at_utc", ""), "art approval")


def seal_final(
    *,
    asset_root: Path,
    repo: Path,
    candidate_manifest_path: Path,
    texture_report_path: Path,
    approval_path: Path,
    generation: int,
    created_at_utc: str,
    reviewer: str,
    notes: str,
) -> dict[str, Any]:
    require(type(generation) is int and generation > 0, "generation is invalid")
    validate_timestamp(created_at_utc, "manifest")
    require(bool(reviewer), "reviewer is empty")
    runtime_subject, _ = candidate_sealer.clean_git_identity(repo)

    staging = asset_root / "staging"
    reports_root = staging / "reports"
    candidate_manifest_path = candidate_manifest_path.resolve()
    require(
        candidate_manifest_path.parent == reports_root.resolve(),
        "candidate manifest is outside staging reports",
    )
    candidate_validation = validator.validate_manifest(
        candidate_manifest_path,
        mode="candidate",
        blend_root=staging / "blend",
        exports_root=staging / "exports",
        reports_root=reports_root,
        licenses_root=asset_root / "licenses",
        repo=repo,
    )
    candidate = validator.read_json(candidate_manifest_path)
    require(
        generation > candidate["asset_set_generation"],
        "final generation must advance the candidate generation",
    )
    allocated = [
        int(path.name)
        for path in (asset_root / "generations").glob("*")
        if path.is_dir() and path.name.isdigit()
    ]
    if allocated:
        require(generation > max(allocated), "final generation is already allocated")

    texture_report_path = texture_report_path.resolve()
    approval_path = approval_path.resolve()
    require(
        texture_report_path.parent == reports_root.resolve()
        and approval_path.parent == reports_root.resolve(),
        "final evidence is outside staging reports",
    )
    texture_report = candidate_sealer.read_json(
        texture_report_path, "final texture report"
    )
    require(
        isinstance(texture_report, dict)
        and texture_report.get("status") == "pass"
        and texture_report.get("asset_set_id") == ASSET_SET_ID
        and texture_report.get("normal_decision") == "rejected"
        and set(texture_report.get("textures", {})) == {"albedo", "emissive"}
        and "normal" not in texture_report,
        "final texture report retains the rejected normal",
    )
    validate_approval(
        approval_path,
        candidate_manifest=candidate,
        candidate_hash=candidate_validation["manifest_sha256"],
    )

    final = copy.deepcopy(candidate)
    final["manifest_mode"] = "final"
    final["asset_set_generation"] = generation
    final["created_at_utc"] = created_at_utc
    final["source"]["runtime_subject"] = runtime_subject
    final["production"]["optional"] = []
    final["normal_decision"] = "rejected"
    final["texture_report"] = candidate_sealer.file_record(
        texture_report_path, reports_root, "final texture report"
    )
    final["art_review"] = {
        "status": "art_approved",
        "reviewer": reviewer,
        "reviewed_at_utc": created_at_utc,
        "notes": notes,
        "artifact": candidate_sealer.file_record(
            approval_path, reports_root, "art approval artifact"
        ),
    }
    return final


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--texture-report", required=True, type=Path)
    parser.add_argument("--approval", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--created-at-utc", required=True)
    parser.add_argument("--reviewer", default="user")
    parser.add_argument("--notes", default="Lit wall art approved; normal rejected.")
    parser.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    asset_root = args.asset_root.resolve()
    output = args.output.resolve()
    require(
        output.parent == (asset_root / "staging/reports").resolve()
        and output.name == FINAL_NAME,
        f"final manifest output must be staging/reports/{FINAL_NAME}",
    )
    final = seal_final(
        asset_root=asset_root,
        repo=args.repo.resolve(),
        candidate_manifest_path=args.candidate_manifest,
        texture_report_path=args.texture_report,
        approval_path=args.approval,
        generation=args.generation,
        created_at_utc=args.created_at_utc,
        reviewer=args.reviewer,
        notes=args.notes,
    )
    temporary = output.with_name(f".{output.name}.sealing")
    require(not temporary.exists(), "stale final manifest temporary exists")
    try:
        candidate_sealer.write_json_atomic(temporary, final)
        validation = validator.validate_manifest(
            temporary,
            mode="final",
            blend_root=asset_root / "staging/blend",
            exports_root=asset_root / "staging/exports",
            reports_root=asset_root / "staging/reports",
            licenses_root=asset_root / "licenses",
            repo=args.repo.resolve(),
        )
        temporary.replace(output)
    finally:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
    print(
        f"WALL_FINAL_SEAL status=pass generation={validation['asset_set_generation']} "
        f"manifest_sha256={validation['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        FinalSealError,
        candidate_sealer.SealError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wall final seal failed: {error}")
        raise SystemExit(1) from error
