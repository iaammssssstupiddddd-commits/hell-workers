"""Validate the Wall asset-set manifest v2 and every referenced artifact."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import subprocess
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any

PROJECT_ROOT = Path(__file__).resolve().parents[3]
GEOMETRY_CONTRACT = (
    PROJECT_ROOT / "tools/blender_ai_workflow/fixtures/wall-production-v1.geometry.json"
)

FAMILIES = {
    "isolated": ("Wall_Isolated", "models/buildings/wall/wall_isolated.glb"),
    "end": ("Wall_End", "models/buildings/wall/wall_end.glb"),
    "straight": ("Wall_Straight", "models/buildings/wall/wall_straight.glb"),
    "corner": ("Wall_Corner", "models/buildings/wall/wall_corner.glb"),
    "t_junction": ("Wall_TJunction", "models/buildings/wall/wall_t_junction.glb"),
    "cross": ("Wall_Cross", "models/buildings/wall/wall_cross.glb"),
}
TEXTURES = {
    "albedo": "textures/buildings/wall/wall_albedo.png",
    "emissive": "textures/buildings/wall/wall_emissive.png",
    "normal": "textures/buildings/wall/wall_normal.png",
}
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")
FILE_FIELDS = {"path", "sha256"}
PRODUCTION_FILE_FIELDS = {"path", "role", "bytes", "sha256"}
TOP_FIELDS = {
    "schema_version",
    "asset_set_id",
    "manifest_mode",
    "asset_set_generation",
    "created_at_utc",
    "source",
    "meshes",
    "production",
    "normal_decision",
    "texture_report",
    "set_reports",
    "provenance",
    "license",
    "art_review",
}


class ManifestError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ManifestError(message)


def no_duplicate_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        require(key not in value, f"duplicate JSON field: {key}")
        value[key] = item
    return value


def read_json(path: Path) -> Any:
    return json.loads(
        path.read_text(encoding="utf-8"), object_pairs_hook=no_duplicate_object
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_path(value: Any, label: str) -> PurePosixPath:
    require(isinstance(value, str) and value, f"{label} path is empty")
    path = PurePosixPath(value)
    require(
        not path.is_absolute() and ".." not in path.parts and "." not in path.parts,
        f"{label} path escapes its root",
    )
    require("\\" not in value, f"{label} path must use POSIX separators")
    return path


def regular_file(root: Path, relative: PurePosixPath, label: str) -> Path:
    root = root.resolve()
    path = root.joinpath(*relative.parts)
    require(
        path.is_file() and not path.is_symlink(), f"{label} file is absent: {relative}"
    )
    require(path.resolve().is_relative_to(root), f"{label} file escapes its root")
    return path


def validate_file_record(value: Any, root: Path, label: str) -> tuple[str, str, Path]:
    require(
        isinstance(value, dict) and set(value) == FILE_FIELDS,
        f"{label} file fields differ",
    )
    relative = relative_path(value["path"], label)
    digest = value["sha256"]
    require(
        isinstance(digest, str) and HEX64.fullmatch(digest) is not None,
        f"{label} sha256 is invalid",
    )
    path = regular_file(root, relative, label)
    require(sha256(path) == digest, f"{label} sha256 differs")
    return relative.as_posix(), digest, path


def validate_production_record(
    value: Any, root: Path, label: str
) -> tuple[str, str, str, Path]:
    require(
        isinstance(value, dict) and set(value) == PRODUCTION_FILE_FIELDS,
        f"{label} file fields differ",
    )
    relative = relative_path(value["path"], label)
    role = value["role"]
    digest = value["sha256"]
    byte_length = value["bytes"]
    require(isinstance(role, str) and role, f"{label} role is empty")
    require(
        isinstance(digest, str) and HEX64.fullmatch(digest) is not None,
        f"{label} sha256 is invalid",
    )
    require(
        type(byte_length) is int and byte_length > 0, f"{label} byte length is invalid"
    )
    path = regular_file(root, relative, label)
    require(path.stat().st_size == byte_length, f"{label} byte length differs")
    require(sha256(path) == digest, f"{label} sha256 differs")
    return relative.as_posix(), role, digest, path


def validate_timestamp(value: Any, label: str, *, allow_empty: bool = False) -> None:
    if allow_empty and value == "":
        return
    require(isinstance(value, str) and value, f"{label} timestamp is empty")
    try:
        datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise ManifestError(f"{label} timestamp is invalid") from error


def validate_png(path: Path, label: str) -> None:
    payload = path.read_bytes()
    require(
        payload.startswith(b"\x89PNG\r\n\x1a\n") and len(payload) >= 24,
        f"{label} is not PNG",
    )
    width, height = struct.unpack(">II", payload[16:24])
    require((width, height) == (1024, 1024), f"{label} PNG size differs")


def git_value(repo: Path, expression: str) -> str:
    completed = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", expression],
        check=False,
        capture_output=True,
        text=True,
    )
    require(completed.returncode == 0, f"cannot resolve git {expression}")
    return completed.stdout.strip()


def validate_manifest(
    manifest_path: Path,
    *,
    mode: str,
    blend_root: Path,
    exports_root: Path,
    reports_root: Path,
    licenses_root: Path,
    repo: Path | None = None,
) -> dict[str, Any]:
    require(
        manifest_path.is_file() and not manifest_path.is_symlink(),
        "asset-set manifest is absent",
    )
    manifest = read_json(manifest_path)
    require(
        isinstance(manifest, dict) and set(manifest) == TOP_FIELDS,
        "asset-set manifest fields differ",
    )
    require(
        manifest["schema_version"] == 2
        and manifest["asset_set_id"] == "wall-production-v1",
        "asset-set identity differs",
    )
    require(
        mode in {"candidate", "final"} and manifest["manifest_mode"] == mode,
        "manifest mode differs",
    )
    require(
        type(manifest["asset_set_generation"]) is int
        and manifest["asset_set_generation"] > 0,
        "asset-set generation is invalid",
    )
    validate_timestamp(manifest["created_at_utc"], "manifest")

    source = manifest["source"]
    require(
        isinstance(source, dict)
        and set(source)
        == {
            "blend",
            "geometry_contract",
            "tool_commit",
            "tool_tree",
            "tool_versions",
            "runtime_subject",
            "source_fingerprint",
        },
        "source fields differ",
    )
    validate_file_record(source["blend"], blend_root, "source blend")
    geometry_contract = source["geometry_contract"]
    require(
        isinstance(geometry_contract, dict)
        and set(geometry_contract) == {"asset_set_id", "schema_version", "sha256"}
        and geometry_contract["asset_set_id"] == "wall-production-v1"
        and geometry_contract["schema_version"] == 1
        and geometry_contract["sha256"] == sha256(GEOMETRY_CONTRACT),
        "geometry contract identity differs",
    )
    require(
        isinstance(source["tool_commit"], str)
        and HEX40.fullmatch(source["tool_commit"]),
        "tool commit is invalid",
    )
    require(
        isinstance(source["tool_tree"], str) and HEX40.fullmatch(source["tool_tree"]),
        "tool tree is invalid",
    )
    require(
        isinstance(source["source_fingerprint"], str)
        and HEX64.fullmatch(source["source_fingerprint"]),
        "source fingerprint is invalid",
    )
    tool_versions = source["tool_versions"]
    require(
        isinstance(tool_versions, dict)
        and set(tool_versions) == {"blender", "exporter", "khronos_validator"}
        and all(isinstance(value, str) and value for value in tool_versions.values()),
        "tool versions differ",
    )
    runtime_subject = source["runtime_subject"]
    if mode == "candidate" and runtime_subject == "pending":
        pass
    else:
        require(
            isinstance(runtime_subject, str) and HEX40.fullmatch(runtime_subject),
            "runtime subject is invalid",
        )
    if repo is not None:
        require(
            source["tool_commit"] == git_value(repo, "HEAD"),
            "tool commit differs from repository HEAD",
        )
        require(
            source["tool_tree"] == git_value(repo, "HEAD^{tree}"),
            "tool tree differs from repository HEAD",
        )

    meshes = manifest["meshes"]
    require(
        isinstance(meshes, list) and len(meshes) == 6,
        "mesh inventory must contain six families",
    )
    seen_families: set[str] = set()
    seen_report_paths: set[str] = set()
    mesh_outputs: dict[str, str] = {}
    report_hashes: list[str] = []
    for mesh in meshes:
        require(
            isinstance(mesh, dict)
            and set(mesh) == {"family", "collection", "output", "reports"},
            "mesh fields differ",
        )
        family = mesh["family"]
        require(
            family in FAMILIES and family not in seen_families,
            "mesh family is unknown or duplicated",
        )
        seen_families.add(family)
        expected_collection, expected_output = FAMILIES[family]
        require(
            mesh["collection"] == expected_collection, f"{family} collection differs"
        )
        output_path, output_hash, _ = validate_file_record(
            mesh["output"], exports_root, f"{family} output"
        )
        require(output_path == expected_output, f"{family} output path differs")
        mesh_outputs[output_path] = output_hash
        reports = mesh["reports"]
        require(
            isinstance(reports, dict)
            and set(reports) == {"scene", "export", "khronos", "post_export"},
            f"{family} report roles differ",
        )
        loaded_reports: dict[str, Any] = {}
        for role, record in reports.items():
            report_relative, report_hash, report_path = validate_file_record(
                record, reports_root, f"{family} {role} report"
            )
            require(
                report_relative not in seen_report_paths, "report path is duplicated"
            )
            seen_report_paths.add(report_relative)
            report_hashes.append(report_hash)
            loaded_reports[role] = read_json(report_path)
        scene = loaded_reports["scene"]
        require(
            scene.get("summary", {}).get("errors") == 0
            and scene.get("mesh_count") == 1,
            f"{family} scene report failed",
        )
        require(
            scene.get("selection", {}).get("collection") == expected_collection,
            f"{family} scene selector differs",
        )
        export = loaded_reports["export"]
        require(
            export.get("status") == "exported"
            and export.get("collection") == expected_collection,
            f"{family} export report failed",
        )
        require(
            export.get("sha256") == output_hash, f"{family} export hash link differs"
        )
        khronos = loaded_reports["khronos"]
        require(
            khronos.get("issues", {}).get("numErrors") == 0
            and khronos.get("issues", {}).get("numWarnings") == 0,
            f"{family} Khronos report failed",
        )
        post = loaded_reports["post_export"]
        require(
            post.get("status") == "pass" and post.get("family") == family,
            f"{family} post-export report failed",
        )
        require(
            post.get("glb_sha256") == output_hash,
            f"{family} post-export hash link differs",
        )
        require(
            post.get("contract_sha256") == geometry_contract["sha256"],
            f"{family} geometry contract hash link differs",
        )
    require(seen_families == set(FAMILIES), "mesh family inventory differs")

    decision = manifest["normal_decision"]
    require(decision in {"pending", "adopted", "rejected"}, "normal decision differs")
    if mode == "final":
        require(
            decision in {"adopted", "rejected"},
            "final manifest rejects pending normal decision",
        )
    expected_core = {path for _, path in FAMILIES.values()} | {
        TEXTURES["albedo"],
        TEXTURES["emissive"],
    }
    expected_optional: set[str] = set()
    if decision == "pending":
        expected_optional = {TEXTURES["normal"]}
    elif decision == "adopted":
        expected_core.add(TEXTURES["normal"])
    production = manifest["production"]
    require(
        isinstance(production, dict) and set(production) == {"core", "optional"},
        "production fields differ",
    )
    expected_roles = {
        **{path: f"mesh:{family}" for family, (_, path) in FAMILIES.items()},
        TEXTURES["albedo"]: "texture:albedo",
        TEXTURES["emissive"]: "texture:emissive",
        TEXTURES["normal"]: "texture:normal",
    }
    production_records: dict[str, str] = {}
    for selection in ("core", "optional"):
        records = production[selection]
        require(isinstance(records, list), f"production {selection} must be a list")
        paths: set[str] = set()
        for ordinal, record in enumerate(records):
            path_value, role, digest, path = validate_production_record(
                record, exports_root, f"production {selection}[{ordinal}]"
            )
            require(
                path_value not in production_records, "production path is duplicated"
            )
            require(role == expected_roles.get(path_value), "production role differs")
            paths.add(path_value)
            production_records[path_value] = digest
            if path_value in TEXTURES.values():
                validate_png(path, path_value)
        require(
            paths == (expected_core if selection == "core" else expected_optional),
            f"production {selection} inventory differs",
        )
    require(
        all(
            production_records[path] == digest for path, digest in mesh_outputs.items()
        ),
        "mesh and production hashes differ",
    )

    texture_report_relative, texture_report_hash, texture_report_path = (
        validate_file_record(
            manifest["texture_report"], reports_root, "texture validation report"
        )
    )
    require(
        texture_report_relative not in seen_report_paths,
        "report path is duplicated",
    )
    seen_report_paths.add(texture_report_relative)
    report_hashes.append(texture_report_hash)
    texture_report = read_json(texture_report_path)
    expected_texture_roles = {"albedo", "emissive"}
    if decision in {"pending", "adopted"}:
        expected_texture_roles.add("normal")
    expected_texture_report_fields = {
        "schema_version",
        "status",
        "asset_set_id",
        "normal_decision",
        "textures",
        "emissive",
        "albedo_mean_rgb",
    }
    if decision in {"pending", "adopted"}:
        expected_texture_report_fields.add("normal")
    require(
        isinstance(texture_report, dict)
        and set(texture_report) == expected_texture_report_fields
        and texture_report["schema_version"] == 1
        and texture_report["status"] == "pass"
        and texture_report["asset_set_id"] == "wall-production-v1"
        and texture_report["normal_decision"] == decision,
        "texture validation report identity differs",
    )
    reported_textures = texture_report["textures"]
    require(
        isinstance(reported_textures, dict)
        and set(reported_textures) == expected_texture_roles,
        "texture validation report inventory differs",
    )
    texture_record_fields = {"path", "bytes", "sha256", "width", "height", "mode"}
    for role, record in reported_textures.items():
        production_path = TEXTURES[role]
        require(
            isinstance(record, dict)
            and set(record) == texture_record_fields
            and record["path"] == PurePosixPath(production_path).name
            and record["bytes"]
            == regular_file(exports_root, PurePosixPath(production_path), role)
            .stat()
            .st_size
            and record["sha256"] == production_records[production_path]
            and record["width"] == 1024
            and record["height"] == 1024
            and record["mode"] == "RGB",
            f"texture validation report {role} link differs",
        )

    set_reports = manifest["set_reports"]
    require(
        isinstance(set_reports, list) and len(set_reports) == 2,
        "set report inventory differs",
    )
    roles: set[str] = set()
    for report in set_reports:
        require(
            isinstance(report, dict) and set(report) == {"role", "file"},
            "set report fields differ",
        )
        role = report["role"]
        require(
            role in {"reference_board", "rebuild"} and role not in roles,
            "set report role differs",
        )
        roles.add(role)
        report_relative, report_hash, report_path = validate_file_record(
            report["file"], reports_root, f"set report {role}"
        )
        require(report_relative not in seen_report_paths, "report path is duplicated")
        seen_report_paths.add(report_relative)
        report_hashes.append(report_hash)
        payload = read_json(report_path)
        require(
            payload.get("status") == "pass"
            and payload.get("asset_set_id") == "wall-production-v1",
            f"set report {role} failed",
        )

    provenance = manifest["provenance"]
    require(
        isinstance(provenance, dict) and set(provenance) == {"generators"},
        "provenance fields differ",
    )
    generators = provenance["generators"]
    require(
        isinstance(generators, list) and len(generators) == 2,
        "generator inventory differs",
    )
    generator_roles: set[str] = set()
    for generator in generators:
        require(
            isinstance(generator, dict)
            and set(generator)
            == {
                "role",
                "service",
                "model",
                "version",
                "prompt_sha256",
                "reference_sha256",
            },
            "generator fields differ",
        )
        require(
            generator["role"] in {"mesh", "texture"}
            and generator["role"] not in generator_roles,
            "generator role differs",
        )
        generator_roles.add(generator["role"])
        require(
            all(
                isinstance(generator[field], str) and generator[field]
                for field in ("service", "model", "version")
            ),
            "generator identity is empty",
        )
        require(
            isinstance(generator["prompt_sha256"], str)
            and HEX64.fullmatch(generator["prompt_sha256"]),
            "generator prompt hash differs",
        )
        require(
            isinstance(generator["reference_sha256"], list),
            "generator references must be a list",
        )
        require(
            all(
                isinstance(value, str) and HEX64.fullmatch(value)
                for value in generator["reference_sha256"]
            ),
            "generator reference hash differs",
        )

    license_value = manifest["license"]
    require(
        isinstance(license_value, dict)
        and set(license_value) == {"name", "terms_url", "checked_at_utc", "file"},
        "license fields differ",
    )
    require(
        isinstance(license_value["name"], str) and license_value["name"],
        "license name is empty",
    )
    require(
        isinstance(license_value["terms_url"], str) and license_value["terms_url"],
        "license terms URL is empty",
    )
    validate_timestamp(license_value["checked_at_utc"], "license")
    validate_file_record(license_value["file"], licenses_root, "license")

    review = manifest["art_review"]
    require(
        isinstance(review, dict)
        and set(review)
        == {"status", "reviewer", "reviewed_at_utc", "notes", "artifact"},
        "art review fields differ",
    )
    require(
        review["status"] in {"candidate", "art_approved", "rejected"},
        "art review status differs",
    )
    if mode == "final":
        require(
            review["status"] == "art_approved",
            "final manifest requires approved art review",
        )
        require(
            isinstance(review["reviewer"], str) and review["reviewer"],
            "art reviewer is empty",
        )
        validate_timestamp(review["reviewed_at_utc"], "art review")
        validate_file_record(review["artifact"], reports_root, "art review artifact")
    else:
        require(
            review["status"] == "candidate",
            "candidate manifest requires candidate art review",
        )
        validate_timestamp(review["reviewed_at_utc"], "art review", allow_empty=True)
        require(
            review["artifact"] is None, "candidate art review artifact must be null"
        )
    require(isinstance(review["notes"], str), "art review notes differ")

    return {
        "schema_version": 2,
        "status": "pass",
        "asset_set_id": "wall-production-v1",
        "manifest_mode": mode,
        "asset_set_generation": manifest["asset_set_generation"],
        "normal_decision": decision,
        "core_files": len(production["core"]),
        "optional_files": len(production["optional"]),
        "mesh_reports": len(report_hashes) - 3,
        "texture_reports": 1,
        "set_reports": 2,
        "manifest_sha256": sha256(manifest_path),
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--mode", required=True, choices=("candidate", "final"))
    parser.add_argument("--blend-root", required=True, type=Path)
    parser.add_argument("--exports-root", required=True, type=Path)
    parser.add_argument("--reports-root", required=True, type=Path)
    parser.add_argument("--licenses-root", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    report = validate_manifest(
        args.manifest.resolve(),
        mode=args.mode,
        blend_root=args.blend_root.resolve(),
        exports_root=args.exports_root.resolve(),
        reports_root=args.reports_root.resolve(),
        licenses_root=args.licenses_root.resolve(),
        repo=args.repo.resolve(),
    )
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (
        ManifestError,
        OSError,
        ValueError,
        KeyError,
        json.JSONDecodeError,
    ) as error:
        print(f"Wall asset-set manifest validation failed: {error}")
        raise SystemExit(1) from error
