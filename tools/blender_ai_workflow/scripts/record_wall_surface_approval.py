"""Bind user approval to both verified native surface previews, not release authority."""

from __future__ import annotations

import argparse
import subprocess
import sys
from datetime import datetime
from pathlib import Path

from provision_wall_surface_preview import source_file
from record_wall_formwork_approval import HEX40, HEX64, read_object, require, timestamp
from seal_wall_candidate import sha256
from validate_wall_formwork_manifest import FAMILIES
from wall_surface_uv import PROFILE
from workflow_common import write_json_atomic

VERIFY = Path(".codex/skills/hell-workers-run-native-acceptance/scripts/wall_art_acceptance.py")
FINGERPRINTS = ("source_fingerprint", "harness_fingerprint", "asset_view_fingerprint")


def preview_identity(path: Path) -> dict:
    preview = read_object(path, "surface preview")
    require(preview.get("schema_version") == 1
            and preview.get("manifest_mode") == "surface_art_preview"
            and preview.get("asset_set_id") == "wall-production-v1"
            and preview.get("review_status") == "art_preview"
            and preview.get("promotion_authority") is False
            and preview.get("surface_uv_profile") == PROFILE,
            "surface preview identity differs")
    require(type(preview.get("asset_set_generation")) is int
            and preview["asset_set_generation"] > 0, "preview generation differs")
    require(HEX40.fullmatch(preview.get("subject_commit", "")) is not None,
            "preview subject differs")
    roles = ([f"mesh:{family}" for family in FAMILIES] + ["texture:albedo", "texture:emissive"]
             + [f"mesh:formwork:{family}" for family in FAMILIES] + ["texture:formwork_albedo"])
    production = preview.get("production", {})
    require(production.get("optional") == []
            and [record.get("role") for record in production.get("core", [])] == roles,
            "preview core inventory differs")
    for record in production["core"]:
        source_file(path.parent / "assets", record)
    return preview


def preview_evidence(root: Path, zoom: str, preview: dict, manifest_hash: str) -> dict:
    job = read_object(root / "job.json", "native preview job")
    evidence = read_object(root / "manifest.json", "native preview evidence")
    image = root / "current-wall.png"
    require(image.is_file() and not image.is_symlink(), "preview screenshot is absent")
    identity = {"asset_set_generation": preview["asset_set_generation"],
                "authority": "art_preview", "manifest_sha256": manifest_hash}
    require(job.get("status") == "valid" and evidence.get("status") == "pass",
            "native preview did not pass")
    for payload in (job, evidence):
        require(payload.get("profile") == f"wall-surface-art-preview-v1-{zoom}"
                and payload.get("zoom") == zoom
                and payload.get("authority") == payload.get("evidence_kind") == "art_preview"
                and payload.get("candidate_identity") == identity
                and payload.get("subject_commit") == preview["subject_commit"],
                "native preview binding differs")
    for field in FINGERPRINTS:
        require(job.get(field) == evidence.get(field)
                and HEX64.fullmatch(job.get(field, "")) is not None,
                f"native {field} differs")
    require(evidence.get("screenshot_sha256") == sha256(image), "screenshot hash differs")
    timestamp(evidence.get("completed_at", ""))
    return {"job": root.name, "profile": job["profile"], "zoom": zoom,
            "subject_commit": job["subject_commit"], "completed_at": evidence["completed_at"],
            "manifest_sha256": sha256(root / "manifest.json"),
            "job_sha256": sha256(root / "job.json"),
            "screenshot_sha256": sha256(image), "screenshot_bytes": image.stat().st_size,
            "verify_status": "pass", **{field: job[field] for field in FINGERPRINTS}}


def build_approval(*, preview_path: Path, standard: Path, farthest: Path,
                   approved_at: str, statement: str, repo: Path) -> dict:
    require(bool(statement.strip()), "user approval statement is empty")
    timestamp(approved_at)
    preview = preview_identity(preview_path)
    digest = sha256(preview_path)
    records = {}
    for zoom, root in (("standard", standard), ("farthest", farthest)):
        records[zoom] = preview_evidence(root, zoom, preview, digest)
        require(datetime.fromisoformat(approved_at.replace("Z", "+00:00")) >=
                datetime.fromisoformat(records[zoom]["completed_at"].replace("Z", "+00:00")),
                "approval predates preview completion")
        # Revalidate the complete native artifact bundle, not just its pass label.
        result = subprocess.run([sys.executable, str(repo / VERIFY), "verify", "--job-root", str(root)],
                                capture_output=True, text=True, check=False)
        require(result.returncode == 0, f"native {zoom} independent verify failed: {result.stdout[-1000:]} {result.stderr[-1000:]}")
    require(all(records["standard"][field] == records["farthest"][field] for field in FINGERPRINTS),
            "preview pair fingerprints differ")
    return {"schema_version": 1, "asset_set_id": preview["asset_set_id"],
            "asset_set_generation": preview["asset_set_generation"], "manifest_sha256": digest,
            "surface_uv_profile": PROFILE, "decision": "surface_art_approved",
            "evidence_kind": "art_preview", "promotion_authority": False,
            "normal_decision": "rejected", "production": preview["production"],
            "base_manifest_sha256": preview["base_manifest_sha256"],
            "approval": {"approved_by": "user", "recorded_at_utc": approved_at, "statement": statement,
                         "scope": "Surface appearance only; formal acceptance and release remain separate."},
            "previews": records}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("preview-manifest", "standard-job", "farthest-job", "repo", "output"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--recorded-at-utc", required=True)
    parser.add_argument("--statement", required=True)
    args = parser.parse_args()
    output = args.output.absolute()
    require(not output.exists() and not output.is_symlink(), "approval output must be new")
    approval = build_approval(preview_path=args.preview_manifest.resolve(),
                              standard=args.standard_job.resolve(), farthest=args.farthest_job.resolve(),
                              approved_at=args.recorded_at_utc, statement=args.statement, repo=args.repo.resolve())
    write_json_atomic(output, approval)
    print(f"WALL_SURFACE_APPROVAL status=pass sha256={sha256(output)} output={output}")


if __name__ == "__main__":
    main()
