"""Record a user approval for one verified production Door ArtPreview."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

import validate_door_manifest as validator


class ApprovalError(RuntimeError):
    pass


HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
PROFILE = "door-art-preview-v1"


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


def evidence_file(
    job_root: Path, record: Any, *, expected_path: str, label: str
) -> dict[str, Any]:
    require(
        isinstance(record, dict)
        and record.get("path") == expected_path
        and HEX64.fullmatch(record.get("sha256", "")) is not None,
        f"{label} record differs",
    )
    path = job_root / expected_path
    require(
        path.is_file()
        and not path.is_symlink()
        and path.stat().st_size > 0
        and digest(path) == record["sha256"],
        f"{label} bytes differ",
    )
    return {
        "bytes": path.stat().st_size,
        "filename": expected_path,
        "sha256": record["sha256"],
    }


def build_approval(
    *,
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
        candidate.get("schema_version") == 1
        and candidate.get("asset_set_id") == validator.ASSET_SET_ID
        and candidate.get("manifest_mode") == "candidate"
        and candidate.get("art_review", {}).get("status") == "candidate",
        "candidate manifest identity differs",
    )

    evidence_path = preview_job_root / "manifest.json"
    evidence = read_object(evidence_path, "preview evidence manifest")
    identity = {
        "asset_set_generation": candidate["asset_set_generation"],
        "authority": "ArtPreview",
        "manifest_sha256": candidate_hash,
    }
    source = evidence.get("source")
    gallery = evidence.get("gallery")
    require(
        evidence.get("schema_version") == 1
        and evidence.get("status") == "awaiting_art_approval"
        and evidence.get("evidence_kind") == "art_preview"
        and evidence.get("candidate_identity") == identity
        and evidence.get("window_backend") == "x11"
        and isinstance(evidence.get("adapter"), dict)
        and evidence["adapter"].get("backend") == "Vulkan"
        and isinstance(source, dict)
        and source.get("commit") == candidate.get("source", {}).get("tool_commit")
        and source.get("unchanged") is True
        and source.get("dirty_paths") == []
        and isinstance(gallery, dict)
        and gallery.get("production_count") == 6
        and gallery.get("fallback_count") == 0
        and gallery.get("axes") == ["EastWest", "NorthSouth"]
        and gallery.get("states") == ["Closed", "Open", "Locked"],
        "preview evidence binding differs",
    )
    require(
        HEX40.fullmatch(source["commit"]) is not None,
        "preview subject differs",
    )
    capture = evidence_file(
        preview_job_root,
        evidence.get("capture"),
        expected_path="current-door.png",
        label="preview capture",
    )
    review_crop = evidence_file(
        preview_job_root,
        evidence.get("review_crop"),
        expected_path="current-door-review-crop.png",
        label="preview review crop",
    )
    sidecar = evidence.get("sidecar")
    require(
        isinstance(sidecar, dict)
        and HEX64.fullmatch(sidecar.get("status_sha256", "")) is not None
        and HEX64.fullmatch(sidecar.get("ack_sha256", "")) is not None
        and digest(preview_job_root / "door-status.json") == sidecar["status_sha256"]
        and digest(preview_job_root / "door-ack.json") == sidecar["ack_sha256"],
        "preview sidecar binding differs",
    )
    return {
        "approval": {
            "approved_at_utc": approved_at_utc,
            "approved_by": "user",
            "statement": statement,
        },
        "asset_set_generation": candidate["asset_set_generation"],
        "asset_set_id": validator.ASSET_SET_ID,
        "decision": "double_leaf_approved",
        "evidence_kind": "art_preview",
        "manifest_sha256": candidate_hash,
        "normal_decision": "not_used_by_design",
        "preview": {
            "capture": capture,
            "evidence_manifest_sha256": digest(evidence_path),
            "job": preview_job_root.name,
            "profile": PROFILE,
            "review_crop": review_crop,
            "source_commit": source["commit"],
            "verify_status": "pass",
        },
        "schema_version": 1,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
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
        repo_root=args.repo.resolve(),
    )
    approval = build_approval(
        candidate_manifest_path=args.candidate_manifest.resolve(),
        preview_job_root=args.preview_job_root.resolve(),
        approved_at_utc=args.approved_at_utc,
        statement=args.statement,
    )
    temporary = output.with_name(f".{output.name}.tmp")
    require(not temporary.exists(), "stale approval temporary exists")
    try:
        temporary.write_text(
            json.dumps(approval, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        temporary.replace(output)
    finally:
        if temporary.is_file() and not temporary.is_symlink():
            temporary.unlink()
    print(
        "DOOR_ART_APPROVAL "
        f"status=pass manifest_sha256={approval['manifest_sha256']} output={output}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ApprovalError,
        validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Door approval failed: {error}")
        raise SystemExit(1) from error
