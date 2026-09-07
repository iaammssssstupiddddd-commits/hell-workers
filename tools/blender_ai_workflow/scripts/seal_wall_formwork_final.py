"""Seal an approved Wall formwork candidate as an immutable final manifest."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
from typing import Any

import seal_wall_candidate as git_sealer
import seal_wall_formwork_candidate as candidate_sealer
import validate_wall_formwork_manifest as validator
from record_wall_formwork_approval import ApprovalError, HEX40, HEX64, timestamp


class FinalSealError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FinalSealError(message)


def validate_approval(
    path: Path, *, candidate: dict[str, Any], candidate_hash: str
) -> dict[str, Any]:
    approval = validator.read_json(path)
    require(
        isinstance(approval, dict)
        and approval.get("schema_version") == 1
        and approval.get("asset_set_id") == validator.ASSET_SET_ID
        and approval.get("asset_set_generation") == candidate["asset_set_generation"]
        and approval.get("manifest_sha256") == candidate_hash
        and approval.get("decision") == "opaque_formwork_approved"
        and approval.get("evidence_kind") == "art_preview"
        and approval.get("formwork_normal_decision") == "not_used_by_design",
        "formwork approval identity differs",
    )
    decision = approval.get("approval")
    preview = approval.get("preview")
    require(
        isinstance(decision, dict)
        and set(decision) == {"approved_at_utc", "approved_by", "statement"}
        and decision["approved_by"] == "user"
        and bool(decision["statement"]),
        "formwork approval decision differs",
    )
    timestamp(decision["approved_at_utc"])
    require(
        isinstance(preview, dict)
        and preview.get("profile") == "wall-formwork-art-preview-v1"
        and preview.get("verify_status") == "pass"
        and HEX40.fullmatch(preview.get("subject_commit", "")) is not None
        and all(
            HEX64.fullmatch(preview.get(field, "")) is not None
            for field in (
                "asset_view_fingerprint",
                "harness_fingerprint",
                "screenshot_sha256",
                "source_fingerprint",
            )
        ),
        "formwork approval preview differs",
    )
    return approval


def seal_final(
    *,
    asset_root: Path,
    repo: Path,
    candidate_manifest_path: Path,
    completed_manifest_path: Path,
    approval_path: Path,
    generation: int,
    reviewer: str,
    notes: str,
) -> dict[str, Any]:
    require(type(generation) is int and generation > 0, "generation is invalid")
    require(bool(reviewer) and bool(notes), "review metadata is empty")
    runtime_subject, runtime_tree = git_sealer.clean_git_identity(repo)
    reports = (asset_root / "staging/reports").resolve()
    require(candidate_manifest_path.parent == reports, "candidate manifest is outside staging reports")
    require(approval_path.parent == reports, "approval is outside staging reports")
    validation = validator.validate_manifest(
        candidate_manifest_path,
        exports_root=asset_root / "staging/exports",
        reports_root=reports,
        blend_root=asset_root / "staging/blend",
        licenses_root=asset_root / "licenses",
        completed_manifest_path=completed_manifest_path,
    )
    candidate = validator.read_json(candidate_manifest_path)
    require(generation > candidate["asset_set_generation"], "final generation must advance candidate")
    allocated = [
        int(path.name)
        for path in (asset_root / "generations").glob("*")
        if path.is_dir() and path.name.isdigit()
    ]
    if allocated:
        require(generation > max(allocated), "final generation is already allocated")
    approval = validate_approval(
        approval_path,
        candidate=candidate,
        candidate_hash=validation["manifest_sha256"],
    )
    final = copy.deepcopy(candidate)
    final["asset_set_generation"] = generation
    final["created_at_utc"] = approval["approval"]["approved_at_utc"]
    final["manifest_mode"] = "final"
    final["source"]["runtime_subject"] = runtime_subject
    final["source"]["tool_commit"] = runtime_subject
    final["source"]["tool_tree"] = runtime_tree
    final["source"]["working_tree_diff_sha256"] = "0" * 64
    source_hash = hashlib.sha256()
    for value in (
        runtime_subject,
        runtime_tree,
        "0" * 64,
        validator.sha256(completed_manifest_path),
        *(record["sha256"] for record in final["production"]["core"][8:]),
    ):
        source_hash.update(value.encode())
    final["source"]["source_fingerprint"] = source_hash.hexdigest()
    final["art_review"] = {
        "artifact": candidate_sealer.file_record(approval_path, reports),
        "notes": notes,
        "reviewed_at_utc": approval["approval"]["approved_at_utc"],
        "reviewer": reviewer,
        "status": "art_approved",
    }
    return final


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--approval", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--reviewer", default="user")
    parser.add_argument("--notes", default="Opaque wooden formwork approved; normal map is not used by design.")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    asset_root = args.asset_root.resolve()
    output = args.output.resolve()
    reports = (asset_root / "staging/reports").resolve()
    require(output.parent == reports, "final output must be in staging reports")
    final = seal_final(
        asset_root=asset_root,
        repo=args.repo.resolve(),
        candidate_manifest_path=args.candidate_manifest.resolve(),
        completed_manifest_path=args.completed_manifest.resolve(),
        approval_path=args.approval.resolve(),
        generation=args.generation,
        reviewer=args.reviewer,
        notes=args.notes,
    )
    temporary = output.with_name(f".{output.name}.sealing")
    require(not temporary.exists(), "stale final manifest temporary exists")
    try:
        temporary.write_text(
            json.dumps(final, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        validation = validator.validate_manifest(
            temporary,
            exports_root=asset_root / "staging/exports",
            reports_root=reports,
            blend_root=asset_root / "staging/blend",
            licenses_root=asset_root / "licenses",
            completed_manifest_path=args.completed_manifest.resolve(),
        )
        temporary.replace(output)
    finally:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
    print(
        "WALL_FORMWORK_FINAL "
        f"status=pass generation={args.generation} "
        f"manifest_sha256={validation['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ApprovalError,
        FinalSealError,
        git_sealer.SealError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wall formwork final seal failed: {error}")
        raise SystemExit(1) from error
