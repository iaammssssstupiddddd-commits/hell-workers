"""Record a user approval for one verified Wall formwork ArtPreview."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

import validate_wall_formwork_manifest as validator


class ApprovalError(RuntimeError):
    pass


HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
PROFILE = "wall-formwork-art-preview-v1"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ApprovalError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def timestamp(value: str) -> str:
    require(bool(value), "approval timestamp is empty")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise ApprovalError("approval timestamp is invalid") from error
    require(parsed.tzinfo is not None, "approval timestamp requires timezone")
    return value


def read_object(path: Path, label: str) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"{label} is absent")
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{label} is not an object")
    return value


def build_approval(
    *,
    asset_root: Path,
    candidate_manifest_path: Path,
    preview_job_root: Path,
    approved_at_utc: str,
    statement: str,
) -> dict[str, Any]:
    require(bool(statement), "approval statement is empty")
    timestamp(approved_at_utc)
    candidate = read_object(candidate_manifest_path, "candidate manifest")
    candidate_hash = digest(candidate_manifest_path)
    require(
        candidate.get("schema_version") == 3
        and candidate.get("asset_set_id") == validator.ASSET_SET_ID
        and candidate.get("manifest_mode") == "candidate"
        and candidate.get("art_review", {}).get("status") == "candidate",
        "candidate manifest identity differs",
    )

    job = read_object(preview_job_root / "job.json", "preview job")
    evidence = read_object(preview_job_root / "manifest.json", "preview manifest")
    screenshot = preview_job_root / "current-wall.png"
    require(screenshot.is_file() and not screenshot.is_symlink(), "preview screenshot is absent")
    identity = {
        "asset_set_generation": candidate["asset_set_generation"],
        "authority": "art_preview",
        "manifest_sha256": candidate_hash,
    }
    require(
        job.get("status") == "valid"
        and evidence.get("status") == "pass"
        and job.get("profile") == evidence.get("profile") == PROFILE
        and job.get("authority") == evidence.get("authority") == "art_preview"
        and job.get("evidence_kind") == evidence.get("evidence_kind") == "art_preview"
        and job.get("candidate_identity") == evidence.get("candidate_identity") == identity
        and job.get("subject_commit") == evidence.get("subject_commit")
        and job.get("source_fingerprint") == evidence.get("source_fingerprint")
        and job.get("harness_fingerprint") == evidence.get("harness_fingerprint")
        and job.get("asset_view_fingerprint") == evidence.get("asset_view_fingerprint")
        and evidence.get("screenshot_sha256") == digest(screenshot)
        and evidence.get("zoom") == "standard",
        "preview evidence binding differs",
    )
    require(HEX40.fullmatch(job["subject_commit"]) is not None, "preview subject differs")
    for field in ("source_fingerprint", "harness_fingerprint", "asset_view_fingerprint"):
        require(HEX64.fullmatch(job[field]) is not None, f"preview {field} differs")

    resolved_asset_root = asset_root.resolve()
    resolved_job = preview_job_root.resolve()
    require(resolved_job.is_relative_to(resolved_asset_root), "preview job is outside asset root")
    return {
        "approval": {
            "approved_at_utc": approved_at_utc,
            "approved_by": "user",
            "statement": statement,
        },
        "asset_set_generation": candidate["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "decision": "opaque_formwork_approved",
        "evidence_kind": "art_preview",
        "formwork_normal_decision": "not_used_by_design",
        "manifest_sha256": candidate_hash,
        "preview": {
            "asset_view_fingerprint": job["asset_view_fingerprint"],
            "completed_at": evidence["completed_at"],
            "harness_fingerprint": job["harness_fingerprint"],
            "job": preview_job_root.name,
            "job_root": resolved_job.relative_to(resolved_asset_root).as_posix(),
            "profile": PROFILE,
            "screenshot": screenshot.resolve().relative_to(resolved_asset_root).as_posix(),
            "screenshot_bytes": screenshot.stat().st_size,
            "screenshot_sha256": digest(screenshot),
            "source_fingerprint": job["source_fingerprint"],
            "subject_commit": job["subject_commit"],
            "verify_status": "pass",
        },
        "schema_version": 1,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--preview-job-root", required=True, type=Path)
    parser.add_argument("--approved-at-utc", required=True)
    parser.add_argument("--statement", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    asset_root = args.asset_root.resolve()
    reports = (asset_root / "staging/reports").resolve()
    output = args.output.resolve()
    require(output.parent == reports, "approval output must be in staging reports")
    validator.validate_manifest(
        args.candidate_manifest.resolve(),
        exports_root=asset_root / "staging/exports",
        reports_root=reports,
        blend_root=asset_root / "staging/blend",
        licenses_root=asset_root / "licenses",
        completed_manifest_path=args.completed_manifest.resolve(),
    )
    approval = build_approval(
        asset_root=asset_root,
        candidate_manifest_path=args.candidate_manifest.resolve(),
        preview_job_root=args.preview_job_root.resolve(),
        approved_at_utc=args.approved_at_utc,
        statement=args.statement,
    )
    temporary = output.with_name(f".{output.name}.tmp")
    temporary.write_text(json.dumps(approval, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(output)
    print(
        "WALL_FORMWORK_ART_APPROVAL "
        f"status=pass manifest_sha256={approval['manifest_sha256']} output={output}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ApprovalError, validator.ManifestError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"Wall formwork approval failed: {error}")
        raise SystemExit(1) from error
