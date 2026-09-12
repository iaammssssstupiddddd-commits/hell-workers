"""Seal a cut-core revision into an isolated ArtPreview view, never a release.

Unlike the historical formwork sealer, this explicitly changes completed art
while preserving every released formwork byte. Neither old art approval nor a
promotion receipt is inherited by the candidate.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from pack_wall_core_atlas import verify_packed
from provision_wall_formwork_preview import copy_exclusive
from project_wall_formwork_preview import canonical_bytes
from seal_wall_candidate import clean_git_identity, require, sha256
from validate_wall_formwork_manifest import FAMILIES, FORMWORK_PATHS, FORMWORK_TEXTURE, safe_relative
from validate_wall_glb import accessor_values, index_values, read_glb
from wall_surface_uv import PROFILE, validate_surface_uv_glb
from workflow_common import asset_root, write_json_atomic


def geometry_signature(path: Path) -> list:
    document, binary = read_glb(path)
    primitive = document["meshes"][0]["primitives"][0]
    attributes = primitive["attributes"]
    positions = accessor_values(document, binary, attributes["POSITION"])
    normals = accessor_values(document, binary, attributes["NORMAL"])
    indices = index_values(document, binary, primitive["indices"])
    return sorted(tuple(sorted(tuple(round(value, 5) for value in (*positions[i], *normals[i]))
                               for i in indices[offset:offset + 3]))
                  for offset in range(0, len(indices), 3))


def source_file(root: Path, record: dict) -> Path:
    path = root / safe_relative(record["path"], "source")
    require(path.resolve().is_relative_to(root.resolve()), "source escapes asset root")
    require(path.is_file() and not path.is_symlink(), f"source absent: {path}")
    require(path.stat().st_size == record["bytes"] and sha256(path) == record["sha256"],
            f"source bytes differ: {path}")
    return path


def seal(root: Path, repo: Path, generation: int, destination: Path) -> dict:
    subject, tree = clean_git_identity(repo)
    staging = root / "staging"
    require(destination.resolve().is_relative_to((staging / "validation").resolve()),
            "preview must stay under staging/validation")
    require(not destination.exists(), "preview destination already exists")
    base_path = repo / "assets/manifests/wall-production-v1.wallset"
    base = json.loads(base_path.read_text())
    require(base.get("authority") == "release_approved" and base.get("schema_version") == 2
            and base.get("normal_decision") == "rejected" and base.get("review_status") == "art_approved",
            "surface revision requires a released schema-2 source")
    require(type(generation) is int and generation > base["asset_set_generation"], "generation must advance")
    receipt_path = source_file(repo / "assets", base["receipt"])
    receipt = json.loads(receipt_path.read_text())
    require(receipt["manifest_sha256"] == base["manifest_sha256"]
            and receipt["asset_set_generation"] == base["asset_set_generation"], "base receipt differs")
    roles = ([f"mesh:{family}" for family in FAMILIES] + ["texture:albedo", "texture:emissive"]
             + [f"mesh:formwork:{family}" for family in FAMILIES] + ["texture:formwork_albedo"])
    require([record["role"] for record in base["core"]] == roles, "base inventory differs")
    original = {record["role"]: source_file(repo / "assets", record) for record in base["core"]}
    packing_path = staging / "reports/core-atlas.json"
    packing = json.loads(packing_path.read_text())
    require(packing["profile"] == PROFILE, "atlas profile differs")
    source_paths, records, geometry = {}, [], {}
    for role in roles:
        if role.startswith("mesh:formwork:"):
            relative = FORMWORK_PATHS[role.rsplit(":", 1)[1]]
            source = original[role]
        elif role == "texture:formwork_albedo":
            relative, source = FORMWORK_TEXTURE, original[role]
        elif role.startswith("mesh:"):
            family = role.split(":")[1]
            name = f"wall_{family}.glb"
            source = staging / "exports/models" / name
            relative = f"models/buildings/wall/{name}"
            geometry[family] = validate_surface_uv_glb(source, family)
            require(geometry_signature(source) == geometry_signature(original[role]),
                    f"released geometry or normals changed: {family}")
            export = json.loads((staging / f"reports/models/{name}.export.json").read_text())
            require(export["sha256"] == sha256(source) and export["status"] == "exported"
                    and export["geometry_scale"] == 32 and export["materials_mode"] == "placeholder"
                    and export["validation"]["summary"]["errors"] == 0,
                    f"export evidence differs: {family}")
        else:
            name = f"wall_{role.split(':')[1]}.png"
            relative = f"textures/buildings/wall/{name}"
            source = staging / "exports" / relative
            require(packing["textures"][name]["sha256"] == sha256(source)
                    and packing["textures"][name]["source_sha256"] == sha256(original[role]),
                    "atlas/source binding differs")
            verify_packed(original[role], source, emissive=role == "texture:emissive")
        source_paths[relative] = source
        records.append({"path": relative, "role": role, "bytes": source.stat().st_size,
                        "sha256": sha256(source)})
    manifest = {
        "schema_version": 1, "manifest_mode": "surface_art_preview", "asset_set_id": base["asset_set_id"],
        "asset_set_generation": generation, "review_status": "art_preview", "promotion_authority": False,
        "subject_commit": subject, "subject_tree": tree, "surface_uv_profile": PROFILE,
        "base_locator_sha256": sha256(base_path), "base_manifest_sha256": base["manifest_sha256"],
        "core_atlas_report_sha256": sha256(packing_path), "production": {"core": records, "optional": []},
        "geometry": geometry,
    }
    destination.mkdir(parents=True)
    manifest_path = destination / "surface-preview.json"
    write_json_atomic(manifest_path, manifest)
    for record in records:
        copy_exclusive(source_paths[record["path"]], destination / "assets" / record["path"], record["sha256"])
    runtime = {"asset_set_generation": generation, "asset_set_id": base["asset_set_id"],
               "authority": "art_preview", "candidate_normal": None, "core": records,
               "manifest_sha256": sha256(manifest_path), "normal_decision": "rejected", "receipt": None,
               "review_status": "art_preview", "schema_version": 2}
    locator = destination / "assets/manifests/wall-production-v1.wallset"
    locator.parent.mkdir(parents=True, exist_ok=True)
    locator.write_bytes(canonical_bytes(runtime))
    return runtime


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--generation", type=int, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    runtime = seal(asset_root(), args.repo.resolve(), args.generation, args.destination.resolve())
    print(json.dumps({"status": "pass", "authority": runtime["authority"],
                      "manifest_sha256": runtime["manifest_sha256"], "destination": str(args.destination)}))


if __name__ == "__main__":
    main()
