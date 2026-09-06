"""Seal an approved production Door candidate as an immutable final manifest."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

import seal_wall_candidate as git_sealer
import validate_door_manifest as validator


class FinalSealError(RuntimeError):
    pass


HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FinalSealError(message)


def timestamp(value: Any) -> str:
    require(isinstance(value, str) and bool(value), "approval timestamp is empty")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise FinalSealError("approval timestamp is invalid") from error
    require(parsed.tzinfo is not None, "approval timestamp requires timezone")
    return value


def file_record(path: Path, root: Path) -> dict[str, str]:
    require(path.is_file() and not path.is_symlink(), f"file is absent: {path}")
    resolved = path.resolve()
    resolved_root = root.resolve()
    require(resolved.is_relative_to(resolved_root), "approval escapes reports root")
    return {
        "path": resolved.relative_to(resolved_root).as_posix(),
        "sha256": validator.sha256(resolved),
    }


def validate_approval(
    path: Path, *, candidate: dict[str, Any], candidate_hash: str
) -> dict[str, Any]:
    approval = validator.read_json(path)
    require(
        isinstance(approval, dict)
        and approval.get("schema_version") == 1
        and approval.get("asset_set_id") == validator.ASSET_SET_ID
        and approval.get("asset_set_generation")
        == candidate["asset_set_generation"]
        and approval.get("manifest_sha256") == candidate_hash
        and approval.get("decision") == "double_leaf_approved"
        and approval.get("evidence_kind") == "art_preview"
        and approval.get("normal_decision") == "not_used_by_design",
        "Door approval identity differs",
    )
    decision = approval.get("approval")
    preview = approval.get("preview")
    require(
        isinstance(decision, dict)
        and set(decision) == {"approved_at_utc", "approved_by", "statement"}
        and decision["approved_by"] == "user"
        and bool(decision["statement"]),
        "Door approval decision differs",
    )
    timestamp(decision["approved_at_utc"])
    require(
        isinstance(preview, dict)
        and preview.get("profile") == "door-art-preview-v1"
        and preview.get("verify_status") == "pass"
        and HEX40.fullmatch(preview.get("source_commit", "")) is not None
        and HEX64.fullmatch(preview.get("evidence_manifest_sha256", ""))
        is not None,
        "Door approval preview differs",
    )
    for label in ("capture", "review_crop"):
        record = preview.get(label)
        require(
            isinstance(record, dict)
            and set(record) == {"bytes", "filename", "sha256"}
            and type(record["bytes"]) is int
            and record["bytes"] > 0
            and HEX64.fullmatch(record["sha256"]) is not None,
            f"Door approval {label} differs",
        )
    return approval


def seal_final(
    *,
    asset_root: Path,
    repo: Path,
    candidate_manifest_path: Path,
    approval_path: Path,
    generation: int,
    reviewer: str,
    notes: str,
) -> dict[str, Any]:
    require(type(generation) is int and generation > 0, "generation is invalid")
    require(bool(reviewer) and bool(notes), "review metadata is empty")
    runtime_subject, runtime_tree = git_sealer.clean_git_identity(repo)
    reports = (asset_root / "staging/reports").resolve()
    require(
        candidate_manifest_path.parent == reports,
        "candidate manifest is outside staging reports",
    )
    require(approval_path.parent == reports, "approval is outside staging reports")
    validation = validator.validate_manifest(
        candidate_manifest_path,
        exports_root=asset_root / "staging/exports",
        reports_root=reports,
        blend_root=asset_root / "staging/blend",
        licenses_root=asset_root / "licenses",
        repo_root=repo,
    )
    candidate = validator.read_json(candidate_manifest_path)
    require(
        generation > candidate["asset_set_generation"],
        "final generation must advance candidate",
    )
    allocated = [
        int(path.name)
        for path in (asset_root / "door_sets").glob("*")
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
        *(record["sha256"] for record in final["production"]["core"]),
    ):
        source_hash.update(value.encode())
    final["source"]["source_fingerprint"] = source_hash.hexdigest()
    final["art_review"] = {
        "artifact": file_record(approval_path, reports),
        "notes": notes,
        "reviewed_at_utc": approval["approval"]["approved_at_utc"],
        "reviewer": reviewer,
        "status": "double_leaf_approved",
    }
    return final


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--approval", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--reviewer", default="user")
    parser.add_argument(
        "--notes",
        default="Symmetric double-leaf Door approved; normal map is not used by design.",
    )
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
            repo_root=args.repo.resolve(),
        )
        temporary.replace(output)
    finally:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
    print(
        "DOOR_FINAL "
        f"status=pass generation={args.generation} "
        f"manifest_sha256={validation['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        FinalSealError,
        git_sealer.SealError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Door final seal failed: {error}")
        raise SystemExit(1) from error
