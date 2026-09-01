"""Seal the exact staged production Wall candidate as asset-set manifest v2."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
if str(SCRIPT_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPT_ROOT))

import validate_asset_set_manifest as manifest_validator

ASSET_SET_ID = "wall-production-v1"


class SealError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SealError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path, label: str) -> Any:
    require(path.is_file() and not path.is_symlink(), f"{label} is absent")
    return json.loads(path.read_text(encoding="utf-8"))


def file_record(path: Path, root: Path, label: str) -> dict[str, Any]:
    require(path.is_file() and not path.is_symlink(), f"{label} is absent")
    resolved_root = root.resolve()
    resolved = path.resolve()
    require(resolved.is_relative_to(resolved_root), f"{label} escapes its root")
    relative = resolved.relative_to(resolved_root).as_posix()
    return {"path": relative, "sha256": sha256(resolved)}


def production_record(path: Path, root: Path, role: str, label: str) -> dict[str, Any]:
    record = file_record(path, root, label)
    return {**record, "role": role, "bytes": path.stat().st_size}


def git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    require(completed.returncode == 0, f"git {' '.join(args)} failed")
    return completed.stdout.strip()


def clean_git_identity(repo: Path) -> tuple[str, str]:
    require(git(repo, "status", "--porcelain=v1") == "", "repository is dirty")
    return git(repo, "rev-parse", "HEAD"), git(repo, "rev-parse", "HEAD^{tree}")


def validate_timestamp(value: Any, label: str) -> str:
    require(isinstance(value, str) and value, f"{label} timestamp is empty")
    try:
        timestamp = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise SealError(f"{label} timestamp is invalid") from error
    require(timestamp.tzinfo is not None, f"{label} timestamp requires timezone")
    return value


def provenance_payload(path: Path, asset_root: Path) -> tuple[dict[str, Any], str]:
    payload = read_json(path, "provenance metadata")
    require(
        isinstance(payload, dict)
        and set(payload) == {"schema_version", "asset_set_id", "generators"}
        and payload["schema_version"] == 1
        and payload["asset_set_id"] == ASSET_SET_ID,
        "provenance metadata identity differs",
    )
    generators = payload["generators"]
    require(
        isinstance(generators, list) and len(generators) == 2,
        "generator inventory differs",
    )
    result = []
    roles: set[str] = set()
    for generator in generators:
        require(
            isinstance(generator, dict)
            and set(generator)
            == {"role", "service", "model", "version", "prompt", "references"},
            "generator metadata fields differ",
        )
        role = generator["role"]
        require(
            role in {"mesh", "texture"} and role not in roles, "generator role differs"
        )
        roles.add(role)
        require(
            all(
                isinstance(generator[key], str) and generator[key]
                for key in ("service", "model", "version")
            ),
            "generator identity is empty",
        )
        prompt = asset_root / manifest_validator.relative_path(
            generator["prompt"], f"{role} prompt"
        )
        prompt_record = file_record(prompt, asset_root, f"{role} prompt")
        references = generator["references"]
        require(isinstance(references, list), "generator references must be a list")
        reference_hashes = []
        for ordinal, value in enumerate(references):
            reference = asset_root / manifest_validator.relative_path(
                value, f"{role} reference[{ordinal}]"
            )
            reference_hashes.append(
                file_record(reference, asset_root, f"{role} reference[{ordinal}]")[
                    "sha256"
                ]
            )
        result.append(
            {
                "role": role,
                "service": generator["service"],
                "model": generator["model"],
                "version": generator["version"],
                "prompt_sha256": prompt_record["sha256"],
                "reference_sha256": reference_hashes,
            }
        )
    return {"generators": result}, sha256(path)


def license_payload(
    path: Path, asset_root: Path, licenses_root: Path
) -> tuple[dict[str, Any], str]:
    payload = read_json(path, "license metadata")
    require(
        isinstance(payload, dict)
        and set(payload)
        == {
            "schema_version",
            "asset_set_id",
            "name",
            "terms_url",
            "checked_at_utc",
            "file",
        }
        and payload["schema_version"] == 1
        and payload["asset_set_id"] == ASSET_SET_ID,
        "license metadata identity differs",
    )
    require(
        isinstance(payload["name"], str) and payload["name"], "license name is empty"
    )
    require(
        isinstance(payload["terms_url"], str)
        and payload["terms_url"].startswith("https://"),
        "license terms URL differs",
    )
    validate_timestamp(payload["checked_at_utc"], "license")
    relative = manifest_validator.relative_path(payload["file"], "license")
    license_file = licenses_root.joinpath(*relative.parts)
    record = file_record(license_file, licenses_root, "license file")
    return {
        "name": payload["name"],
        "terms_url": payload["terms_url"],
        "checked_at_utc": payload["checked_at_utc"],
        "file": record,
    }, sha256(path)


def seal_candidate(
    *,
    asset_root: Path,
    repo: Path,
    generation: int,
    created_at_utc: str,
    provenance_path: Path,
    license_metadata_path: Path,
) -> dict[str, Any]:
    require(type(generation) is int and generation > 0, "generation is invalid")
    validate_timestamp(created_at_utc, "manifest")
    tool_commit, tool_tree = clean_git_identity(repo)
    staging = asset_root / "staging"
    blend_root = staging / "blend"
    exports_root = staging / "exports"
    reports_root = staging / "reports"
    licenses_root = asset_root / "licenses"
    blend = file_record(
        blend_root / f"{ASSET_SET_ID}.blend", blend_root, "source blend"
    )
    geometry_hash = sha256(manifest_validator.GEOMETRY_CONTRACT)
    meshes = []
    core = []
    exporter_versions: set[str] = set()
    blender_versions: set[str] = set()
    validator_versions: set[str] = set()
    fingerprint_records: list[dict[str, Any]] = []
    for family, (collection, output_relative) in manifest_validator.FAMILIES.items():
        output_path = exports_root / output_relative
        output = file_record(output_path, exports_root, f"{family} output")
        stem = Path(output_relative).name.removesuffix(".glb")
        report_paths = {
            "scene": reports_root / f"{stem}.scene.json",
            "export": reports_root / f"{output_relative}.export.json",
            "khronos": reports_root / f"{output_relative}.khronos.json",
            "post_export": reports_root / f"{stem}.post-export.json",
        }
        reports = {
            role: file_record(path, reports_root, f"{family} {role} report")
            for role, path in report_paths.items()
        }
        export_report = read_json(report_paths["export"], f"{family} export report")
        khronos_report = read_json(report_paths["khronos"], f"{family} Khronos report")
        blender_versions.add(str(export_report.get("blender_version", "")))
        exporter_versions.add(str(khronos_report.get("info", {}).get("generator", "")))
        validator_versions.add(str(khronos_report.get("validatorVersion", "")))
        meshes.append(
            {
                "family": family,
                "collection": collection,
                "output": output,
                "reports": reports,
            }
        )
        production = production_record(
            output_path, exports_root, f"mesh:{family}", f"{family} output"
        )
        core.append(production)
        fingerprint_records.append(
            {"family": family, "output": production, "reports": reports}
        )
    require(
        len(blender_versions) == len(exporter_versions) == len(validator_versions) == 1
        and all(
            next(iter(values))
            for values in (blender_versions, exporter_versions, validator_versions)
        ),
        "tool versions differ across mesh reports",
    )
    for role in ("albedo", "emissive"):
        relative = manifest_validator.TEXTURES[role]
        core.append(
            production_record(
                exports_root / relative,
                exports_root,
                f"texture:{role}",
                f"{role} texture",
            )
        )
    normal_relative = manifest_validator.TEXTURES["normal"]
    optional = [
        production_record(
            exports_root / normal_relative,
            exports_root,
            "texture:normal",
            "normal texture",
        )
    ]
    texture_report = file_record(
        reports_root / f"{ASSET_SET_ID}.textures.json", reports_root, "texture report"
    )
    set_reports = [
        {
            "role": role,
            "file": file_record(reports_root / name, reports_root, f"{role} report"),
        }
        for role, name in (
            ("reference_board", f"{ASSET_SET_ID}.reference-board.json"),
            ("rebuild", f"{ASSET_SET_ID}.rebuild.json"),
        )
    ]
    provenance, provenance_hash = provenance_payload(provenance_path, asset_root)
    license_value, license_metadata_hash = license_payload(
        license_metadata_path, asset_root, licenses_root
    )
    fingerprint = {
        "tool_commit": tool_commit,
        "tool_tree": tool_tree,
        "blend": blend,
        "geometry_contract_sha256": geometry_hash,
        "meshes": fingerprint_records,
        "textures": core[6:] + optional,
        "texture_report": texture_report,
        "set_reports": set_reports,
        "provenance_metadata_sha256": provenance_hash,
        "license_metadata_sha256": license_metadata_hash,
        "license": license_value,
    }
    fingerprint_hash = hashlib.sha256(
        json.dumps(fingerprint, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    return {
        "schema_version": 2,
        "asset_set_id": ASSET_SET_ID,
        "manifest_mode": "candidate",
        "asset_set_generation": generation,
        "created_at_utc": created_at_utc,
        "source": {
            "blend": blend,
            "geometry_contract": {
                "asset_set_id": ASSET_SET_ID,
                "schema_version": 1,
                "sha256": geometry_hash,
            },
            "tool_commit": tool_commit,
            "tool_tree": tool_tree,
            "tool_versions": {
                "blender": next(iter(blender_versions)),
                "exporter": next(iter(exporter_versions)),
                "khronos_validator": next(iter(validator_versions)),
            },
            "runtime_subject": "pending",
            "source_fingerprint": fingerprint_hash,
        },
        "meshes": meshes,
        "production": {"core": core, "optional": optional},
        "normal_decision": "pending",
        "texture_report": texture_report,
        "set_reports": set_reports,
        "provenance": provenance,
        "license": license_value,
        "art_review": {
            "status": "candidate",
            "reviewer": "",
            "reviewed_at_utc": "",
            "notes": "",
            "artifact": None,
        },
    }


def write_json_atomic(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(path)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asset-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--created-at-utc")
    parser.add_argument("--provenance", required=True, type=Path)
    parser.add_argument("--license-metadata", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    created_at = args.created_at_utc or datetime.now(timezone.utc).isoformat()
    asset_root = args.asset_root.resolve()
    output = args.output.resolve()
    reports_root = (asset_root / "staging/reports").resolve()
    require(
        output.parent == reports_root,
        "candidate manifest output must be in staging reports",
    )
    require(
        output.name == f"{ASSET_SET_ID}.asset-set.json",
        "candidate manifest filename differs",
    )
    manifest = seal_candidate(
        asset_root=asset_root,
        repo=args.repo.resolve(),
        generation=args.generation,
        created_at_utc=created_at,
        provenance_path=args.provenance.resolve(),
        license_metadata_path=args.license_metadata.resolve(),
    )
    candidate = output.with_name(f".{output.name}.sealing")
    require(not candidate.exists(), "stale candidate manifest temporary exists")
    try:
        write_json_atomic(candidate, manifest)
        validation = manifest_validator.validate_manifest(
            candidate,
            mode="candidate",
            blend_root=asset_root / "staging/blend",
            exports_root=asset_root / "staging/exports",
            reports_root=reports_root,
            licenses_root=asset_root / "licenses",
            repo=args.repo.resolve(),
        )
        candidate.replace(output)
    finally:
        if candidate.is_file() and not candidate.is_symlink():
            candidate.unlink()
    print(
        f"WALL_CANDIDATE_SEAL status=pass generation={validation['asset_set_generation']} "
        f"manifest_sha256={validation['manifest_sha256']}"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        SealError,
        manifest_validator.ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wall candidate sealing failed: {error}")
        raise SystemExit(1) from error
