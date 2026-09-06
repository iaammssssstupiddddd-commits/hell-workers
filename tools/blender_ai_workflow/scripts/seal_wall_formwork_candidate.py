"""Seal a technical Wall formwork candidate without granting art authority."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import validate_wall_formwork_manifest as validator


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
    record = file_record(path, root)
    return {**record, "bytes": path.stat().st_size, "role": role}


def git(repo: Path, *args: str) -> bytes:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout


def working_tree_fingerprint(repo: Path) -> str:
    digest_state = hashlib.sha256()
    digest_state.update(git(repo, "diff", "--binary", "HEAD"))
    untracked = git(repo, "ls-files", "--others", "--exclude-standard", "-z")
    for raw in sorted(item for item in untracked.split(b"\0") if item):
        relative = raw.decode()
        digest_state.update(raw)
        digest_state.update((repo / relative).read_bytes())
    return digest_state.hexdigest()


def build_candidate(
    *, asset_root: Path, repo: Path, completed_manifest_path: Path, generation: int
) -> dict[str, Any]:
    require(generation > 0, "candidate generation is invalid")
    staging = asset_root / "staging"
    exports = staging / "exports"
    reports = staging / "reports"
    blend = staging / "blend"
    completed = json.loads(completed_manifest_path.read_text(encoding="utf-8"))
    require(
        completed.get("schema_version") == 2
        and completed.get("manifest_mode") == "final"
        and completed.get("asset_set_id") == validator.ASSET_SET_ID
        and completed.get("normal_decision") == "rejected"
        and generation > completed.get("asset_set_generation", 0),
        "completed source manifest differs",
    )
    completed_core = completed.get("production", {}).get("core")
    require(isinstance(completed_core, list) and len(completed_core) == 8, "completed core differs")
    for record in completed_core:
        path = exports / record["path"]
        require(
            path.is_file()
            and path.stat().st_size == record["bytes"]
            and digest(path) == record["sha256"],
            f"completed bytes differ: {record['path']}",
        )

    formwork_meshes = []
    formwork_core = []
    for family, collection in validator.FAMILIES.items():
        relative = validator.FORMWORK_PATHS[family]
        output = exports / relative
        export_report = reports / f"{relative}.export.json"
        khronos_report = reports / f"{relative}.khronos.json"
        post_report = reports / f"wall_formwork_{family}.post-export.json"
        formwork_meshes.append(
            {
                "collection": collection,
                "family": family,
                "output": file_record(output, exports),
                "reports": {
                    "export": file_record(export_report, reports),
                    "khronos": file_record(khronos_report, reports),
                    "post_export": file_record(post_report, reports),
                },
            }
        )
        formwork_core.append(
            production_record(output, exports, f"mesh:formwork:{family}")
        )
    texture = exports / validator.FORMWORK_TEXTURE
    formwork_core.append(production_record(texture, exports, "texture:formwork_albedo"))

    commit = git(repo, "rev-parse", "HEAD").decode().strip()
    tree = git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
    diff_hash = working_tree_fingerprint(repo)
    source_hash = hashlib.sha256()
    for value in [commit, tree, diff_hash, digest(completed_manifest_path)]:
        source_hash.update(value.encode())
    for record in formwork_core:
        source_hash.update(record["sha256"].encode())
    created = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    return {
        "art_review": {
            "artifact": None,
            "notes": "Technical formwork candidate; player art approval is pending.",
            "reviewed_at_utc": "",
            "reviewer": "",
            "status": "candidate",
        },
        "asset_set_generation": generation,
        "asset_set_id": validator.ASSET_SET_ID,
        "created_at_utc": created,
        "formwork_meshes": formwork_meshes,
        "formwork_texture_report": file_record(
            reports / "wall-formwork-v1.texture.json", reports
        ),
        "license": completed["license"],
        "manifest_mode": "candidate",
        "normal_decision": "rejected",
        "production": {"core": completed_core + formwork_core, "optional": []},
        "provenance": {
            "completed": completed["provenance"],
            "formwork": {
                "mesh": "procedural-blender-5.1.1",
                "texture": "openai-built-in-imagegen",
            },
        },
        "schema_version": 3,
        "source": {
            "completed": {
                "asset_set_generation": completed["asset_set_generation"],
                "manifest_sha256": digest(completed_manifest_path),
            },
            "formwork": {
                "blend": file_record(blend / "wall-formwork-v1.blend", blend),
                "geometry_contract": file_record(
                    reports / "wall-formwork-v1.geometry.json", reports
                ),
                "texture_prompt": file_record(
                    reports / "wall-formwork-v1.texture-prompt.txt", reports
                ),
            },
            "runtime_subject": "pending",
            "source_fingerprint": source_hash.hexdigest(),
            "tool_commit": commit,
            "tool_tree": tree,
            "tool_versions": {
                "blender": "5.1.1",
                "exporter": "Khronos glTF Blender I/O v5.1.19",
                "khronos_validator": "2.0.0-dev.3.10",
            },
            "working_tree_diff_sha256": diff_hash,
        },
    }


def write_atomic(path: Path, value: dict[str, Any]) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--completed-manifest", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    asset_root = args.asset_root.resolve()
    output = args.output.resolve()
    reports = (asset_root / "staging/reports").resolve()
    require(output.parent == reports, "candidate output must be in staging reports")
    candidate = build_candidate(
        asset_root=asset_root,
        repo=args.repo.resolve(),
        completed_manifest_path=args.completed_manifest.resolve(),
        generation=args.generation,
    )
    write_atomic(output, candidate)
    validation = validator.validate_manifest(
        output,
        exports_root=asset_root / "staging/exports",
        reports_root=reports,
        blend_root=asset_root / "staging/blend",
        licenses_root=asset_root / "licenses",
        completed_manifest_path=args.completed_manifest.resolve(),
    )
    print(
        "WALL_FORMWORK_CANDIDATE "
        f"status=pass generation={validation['asset_set_generation']} "
        f"manifest_sha256={validation['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (SealError, validator.ManifestError) as error:
        print(f"Wall formwork candidate seal failed: {error}")
        raise SystemExit(1) from error
