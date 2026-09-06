"""Seal the six-file Door technical candidate without art authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import validate_door_manifest as validator


class SealError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SealError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def file_record(path: Path, root: Path) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"file is absent: {path}")
    return {"path": path.relative_to(root).as_posix(), "sha256": digest(path)}


def production_record(path: Path, root: Path, role: str) -> dict[str, Any]:
    return {**file_record(path, root), "bytes": path.stat().st_size, "role": role}


def git(repo: Path, *args: str) -> bytes:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def working_tree_fingerprint(repo: Path) -> str:
    state = hashlib.sha256()
    state.update(git(repo, "diff", "--binary", "HEAD"))
    untracked = git(repo, "ls-files", "--others", "--exclude-standard", "-z")
    for raw in sorted(item for item in untracked.split(b"\0") if item):
        state.update(raw)
        state.update((repo / raw.decode()).read_bytes())
    return state.hexdigest()


def build_candidate(*, asset_root: Path, repo: Path, generation: int) -> dict[str, Any]:
    require(generation > 0, "candidate generation is invalid")
    staging = asset_root / "staging"
    exports = staging / "exports"
    reports = staging / "reports"
    blend = staging / "blend"
    core = [production_record(exports / path, exports, role) for path, role in validator.CORE]
    mesh_reports = []
    for state in validator.STATES:
        relative = f"models/buildings/door/door_{state}.glb"
        mesh_reports.append({
            "export": file_record(reports / f"{relative}.export.json", reports),
            "khronos": file_record(reports / f"{relative}.khronos.json", reports),
            "post_export": file_record(reports / f"door_{state}.post-export.json", reports),
            "state": state,
        })
    commit = git(repo, "rev-parse", "HEAD").decode().strip()
    tree = git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
    diff_hash = working_tree_fingerprint(repo)
    source_values = [commit, tree, diff_hash, *(record["sha256"] for record in core)]
    source_fingerprint = hashlib.sha256("".join(source_values).encode()).hexdigest()
    return {
        "art_review": {"artifact": None, "notes": "Technical double-leaf Door candidate; player art approval is pending.", "reviewed_at_utc": "", "reviewer": "", "status": "candidate"},
        "asset_set_generation": generation,
        "asset_set_id": validator.ASSET_SET_ID,
        "created_at_utc": datetime.now(UTC).isoformat().replace("+00:00", "Z"),
        "license": {"file": file_record(asset_root / "licenses/door-production-v1.md", asset_root / "licenses")},
        "manifest_mode": "candidate",
        "mesh_reports": mesh_reports,
        "normal_decision": "not_used_by_design",
        "preview_report": file_record(reports / "door-production-v1.previews.json", reports),
        "production": {"core": core, "optional": []},
        "provenance": {"mesh": "procedural-blender-5.1.1", "previews": "blender-eevee-fixed-camera", "texture": "openai-built-in-imagegen"},
        "schema_version": 1,
        "source": {
            "blend": file_record(blend / "door-production-v1.blend", blend),
            "geometry_contract": file_record(repo / "tools/blender_ai_workflow/fixtures/door-production-v1.geometry.json", repo),
            "runtime_subject": "pending",
            "scene_report": file_record(reports / "door-production-v1.scene-create.json", reports),
            "source_fingerprint": source_fingerprint,
            "texture_prompt": file_record(reports / "door-production-v1.texture-prompt.txt", reports),
            "tool_commit": commit,
            "tool_tree": tree,
            "tool_versions": {"blender": "5.1.1", "exporter": "Khronos glTF Blender I/O v5.1.19", "khronos_validator": "2.0.0-dev.3.10"},
            "working_tree_diff_sha256": diff_hash,
        },
        "texture_report": file_record(reports / "door-production-v1.textures.json", reports),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    asset_root = args.asset_root.resolve()
    output = args.output.resolve()
    require(output.parent == (asset_root / "staging/reports").resolve(), "candidate output must be in staging reports")
    output.write_text(json.dumps(build_candidate(asset_root=asset_root, repo=args.repo.resolve(), generation=args.generation), indent=2, sort_keys=True) + "\n", encoding="utf-8")
    result = validator.validate_manifest(output, exports_root=asset_root / "staging/exports", reports_root=asset_root / "staging/reports", blend_root=asset_root / "staging/blend", licenses_root=asset_root / "licenses", repo_root=args.repo.resolve())
    print(f"DOOR_CANDIDATE status=pass generation={result['asset_set_generation']} manifest_sha256={result['manifest_sha256']}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (SealError, validator.ManifestError) as error:
        print(f"Door candidate seal failed: {error}")
        raise SystemExit(1) from error

