"""Seal newly approved completed surfaces with byte-identical released formwork.

Builds a self-contained authoring v3 payload and an isolated candidate view under
staging/validation. It never writes a release receipt or a canonical pointer.
Every completed GLB is re-exported from the current blend and must reproduce the
approved preview bytes. Historical completed-art approval is not inherited.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import os
import shutil
import subprocess
from pathlib import Path

import asset_release_manifest as release
import project_wall_formwork_candidate as projector
import record_wall_surface_approval as approval_tool
import validate_wall_textures as textures
from pack_wall_core_atlas import verify_packed
from project_wall_formwork_preview import canonical_bytes
from provision_wall_formwork_preview import copy_exclusive
from provision_wall_surface_preview import geometry_signature, source_file
from seal_wall_candidate import clean_git_identity, file_record, require, sha256
from wall_surface_uv import PROFILE, validate_surface_uv_glb
from workflow_common import write_json_atomic


def record(path: Path, root: Path) -> dict:
    return file_record(path, root, path.name)


def copy_tree(source: Path, target: Path) -> None:
    require(source.is_dir() and not source.is_symlink(), "source directory is absent")
    require(not any(path.is_symlink() for path in source.rglob("*")), "source contains symlinks")
    # copyfile, not copy2: published source files may be read-only; the new,
    # exclusively owned staging copy must remain writable during sealing.
    shutil.copytree(source, target, copy_function=shutil.copyfile)


def validate_revision_inputs(base_path: Path, preview_path: Path, approval_path: Path,
                             generation: int, repo: Path, standard: Path, farthest: Path) -> tuple[dict, dict]:
    release.validate(base_path, repo=repo)
    base = release.wall.read_json(base_path)
    require(base.get("asset_set_id") == release.WALL_ID and base.get("schema_version") == 3,
            "surface revision requires a final Wall v3 base")
    preview = approval_tool.preview_identity(preview_path)
    require(preview["base_manifest_sha256"] == sha256(base_path), "released base differs from preview")
    require(type(generation) is int and generation > max(base["asset_set_generation"], preview["asset_set_generation"]),
            "revision generation must advance base and preview")
    require(preview["production"]["core"][8:] == base["production"]["core"][8:],
            "released formwork bytes changed")
    approval = release.wall.read_json(approval_path)
    rebuilt = approval_tool.build_approval(
        preview_path=preview_path, standard=standard, farthest=farthest, repo=repo,
        approved_at=approval["approval"]["recorded_at_utc"], statement=approval["approval"]["statement"])
    require(approval == rebuilt, "surface approval binding differs")
    return base, approval


def rebuild_meshes(*, root: Path, repo: Path, prepared: Path, completed: dict, approved_core: list) -> list:
    reports, exports = prepared / "reports", prepared / "exports"
    blend = root / "staging/blend/wall-production-v1.blend"
    environment = {**os.environ, "HELL_WORKERS_ASSET_ROOT": str(root), "BLENDER_SAFE_NO_NETWORK": "1"}
    families = []
    versions = set()
    for mesh, core in zip(completed["meshes"], approved_core[:6], strict=True):
        family = mesh["family"]
        relative = f"models/buildings/wall/wall_{family}.glb"
        require(core["path"] == relative and core["role"] == f"mesh:{family}", "mesh ordering differs")
        subprocess.run([str(repo / "tools/blender_ai_workflow/bin/export-staging-glb"), str(blend),
                        relative, "72", "--collection", mesh["collection"], "--geometry-scale", "32",
                        "--materials-mode", "placeholder"], env=environment, check=True)
        glb = source_file(exports, core)  # Re-export must match the approved bytes.
        post = validate_surface_uv_glb(glb, family)
        export_path = reports / f"{relative}.export.json"
        export = release.wall.read_json(export_path)
        khronos = release.wall.read_json(reports / f"{relative}.khronos.json")
        versions.add((str(export["blender_version"]), str(khronos["info"]["generator"]), str(khronos["validatorVersion"])))
        scene_path, post_path = reports / f"wall_{family}.scene.json", reports / f"wall_{family}.post-export.json"
        write_json_atomic(scene_path, export["validation"])
        write_json_atomic(post_path, post)
        mesh["output"] = record(glb, exports)
        mesh["reports"] = {"export": record(export_path, reports), "scene": record(scene_path, reports),
                           "post_export": record(post_path, reports),
                           "khronos": record(reports / f"{relative}.khronos.json", reports)}
        families.append({"family": family, "approved_sha256": core["sha256"], "rebuilt_sha256": sha256(glb),
                         "surface_uv": post["surface_uv"]})
    require(len(versions) == 1, "rebuild tool versions differ")
    completed["source"]["tool_versions"] = dict(zip(("blender", "exporter", "khronos_validator"), versions.pop(), strict=True))
    return families


def seal(*, base_path: Path, art_root: Path, preview_path: Path, approval_path: Path,
         standard: Path, farthest: Path, destination: Path, generation: int, repo: Path) -> dict:
    commit, tree = clean_git_identity(repo)
    require(destination.is_relative_to(art_root / "staging/validation") and not destination.exists(),
            "revision destination must be new and under staging/validation")
    base, approval = validate_revision_inputs(base_path, preview_path, approval_path, generation, repo, standard, farthest)
    base_roots = release.roots_for(base_path)
    completed_path = release.completed_source(base_path, base)
    completed = release.wall.read_json(completed_path)
    original_completed = copy.deepcopy(completed)
    approved_core = approval["production"]["core"]
    preview_exports = preview_path.parent / "assets"
    packing_path = art_root / "staging/reports/core-atlas.json"
    packing = release.wall.read_json(packing_path)
    preview = release.wall.read_json(preview_path)
    require(sha256(packing_path) == preview["core_atlas_report_sha256"] and packing["profile"] == PROFILE,
            "approved packing report differs")
    for role in ("stone", "core"):
        require(sha256(art_root / f"{role}-art.png") == packing["artwork"][role]["sha256"],
                "generated artwork differs from packing report")
    for core in approved_core[:6]:
        require(geometry_signature(source_file(preview_exports, core)) ==
                geometry_signature(base_roots["exports_root"] / core["path"]), "completed geometry/normals changed")
    for core in approved_core[6:8]:
        verify_packed(base_roots["exports_root"] / core["path"], source_file(preview_exports, core),
                      emissive=core["role"] == "texture:emissive")

    destination.mkdir(parents=True)
    prepared, payload = destination / "staging", destination / "payload"
    prepared.mkdir()
    blend = prepared / "blend/wall-production-v1.blend"
    blend.parent.mkdir()
    shutil.copyfile(art_root / "staging/blend/wall-production-v1.blend", blend)
    # Export in the new isolated root; the approved preview is never overwritten.
    families = rebuild_meshes(root=destination, repo=repo, prepared=prepared,
                              completed=completed, approved_core=approved_core)
    for key, source in release.roots_for(completed_path).items():
        subdirectory = {"blend_root": "source/blender", "exports_root": "exports",
                        "reports_root": "reports", "licenses_root": "licenses"}[key]
        copy_tree(source, payload / "completed" / subdirectory)
    for key, source in base_roots.items():
        subdirectory = {"blend_root": "source/blender", "exports_root": "exports",
                        "reports_root": "reports", "licenses_root": "licenses"}[key]
        copy_tree(source, payload / subdirectory)
    complete_root = payload / "completed"
    complete_reports = complete_root / "reports"
    for source in (prepared / "reports").rglob("*.json"):
        target = complete_reports / source.relative_to(prepared / "reports")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    for core in approved_core[:8]:
        for exports in (payload / "exports", complete_root / "exports"):
            shutil.copyfile(source_file(preview_exports, core), exports / core["path"])
    shutil.copyfile(blend, complete_root / "source/blender/wall-production-v1.blend")
    contract = complete_root / "source/contracts/wall-production-v1.geometry.json"
    contract.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(release.wall.GEOMETRY_CONTRACT, contract)
    created = approval["approval"]["recorded_at_utc"]
    review_path = complete_reports / "wall-surface.art-approval.json"
    write_json_atomic(review_path, approval)
    write_json_atomic(payload / "reports/wall-surface.art-approval.json", approval)
    texture_path = complete_reports / "wall-surface.textures.json"
    write_json_atomic(texture_path, textures.validate_textures(complete_root / "exports/textures/buildings/wall",
                      normal_decision="rejected", normal_sampling=None, normal_convention=None))
    rebuild_path = complete_reports / "wall-surface.rebuild.json"
    write_json_atomic(rebuild_path, {"status": "pass", "asset_set_id": release.WALL_ID,
                      "method": "fresh_blend_export_equals_approved_preview", "families": families,
                      "blend_sha256": sha256(blend), "tool_commit": commit})
    board_path = complete_reports / "wall-surface.reference-board.json"
    shutil.copyfile(art_root / "staging/reports/wall-production-v1.reference-board-neutral-standard.json", board_path)
    prompt = art_root / "prompts.md"
    require(prompt.is_file(), "surface generation prompts are absent")
    completed.update(asset_set_generation=generation, created_at_utc=created,
                     production={"core": approved_core[:8], "optional": []}, normal_decision="rejected",
                     texture_report=record(texture_path, complete_reports),
                     set_reports=[{"role": "reference_board", "file": record(board_path, complete_reports)},
                                  {"role": "rebuild", "file": record(rebuild_path, complete_reports)}])
    completed["source"].update(blend=record(complete_root / "source/blender/wall-production-v1.blend", complete_root / "source/blender"),
                               runtime_subject=commit, tool_commit=commit, tool_tree=tree)
    generators = {entry["role"]: entry for entry in completed["provenance"]["generators"]}
    generators["texture"].update(prompt_sha256=sha256(prompt), version=created[:10], reference_sha256=[])
    # The old mesh prompt is not the authoring source for the new UV mapping.
    generators["mesh"].update(prompt_sha256=sha256(repo / "tools/blender_ai_workflow/scripts/create_wall_production_scene.py"),
                              reference_sha256=[sha256(repo / "tools/blender_ai_workflow/scripts/wall_surface_uv.py")])
    review = {"status": "art_approved", "reviewer": "user", "reviewed_at_utc": created,
              "notes": "New surface approval; unchanged formwork retains its original source and bytes. No release authority.",
              "artifact": record(review_path, complete_reports)}
    completed["art_review"] = review
    fingerprint = hashlib.sha256(canonical_bytes({"commit": commit, "tree": tree, "approval": sha256(approval_path),
                                                "core": approved_core, "blend": sha256(blend)})).hexdigest()
    completed["source"]["source_fingerprint"] = fingerprint
    completed_output = complete_root / "manifest/wall-production-v1.asset-set.json"
    write_json_atomic(completed_output, completed)
    final = copy.deepcopy(base)
    final.update(asset_set_generation=generation, created_at_utc=created,
                 production=approval["production"], art_review=review)
    final["source"].update(completed={"asset_set_generation": generation, "manifest_sha256": sha256(completed_output)},
                           runtime_subject=commit, tool_commit=commit, tool_tree=tree,
                           working_tree_diff_sha256="0" * 64, source_fingerprint=fingerprint)
    artifacts = {}
    for role, source in {"prompt": prompt, "stone_art": art_root / "stone-art.png", "core_art": art_root / "core-art.png",
                         "packing": packing_path, "standard_capture": standard / "current-wall.png",
                         "farthest_capture": farthest / "current-wall.png", "standard_manifest": standard / "manifest.json",
                         "farthest_manifest": farthest / "manifest.json", "standard_job": standard / "job.json",
                         "farthest_job": farthest / "job.json"}.items():
        target = payload / "reports/surface" / f"{role}{source.suffix}"
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        artifacts[role] = record(target, payload / "reports")
    final["provenance"]["surface_revision"] = {"profile": PROFILE, "base_manifest_sha256": sha256(base_path),
        "original_formwork_review": base["art_review"],
        "original_formwork_approval": release.wall.read_json(base_roots["reports_root"] / base["art_review"]["artifact"]["path"]),
        "artifacts": artifacts, "superseded_completed_manifest_sha256": sha256(completed_path),
        "superseded_completed_generation": original_completed["asset_set_generation"]}
    output = payload / "manifest/wall-production-v1.asset-set.json"
    write_json_atomic(output, final)
    release.validate(output, repo=repo)
    runtime = projector.project(output)
    for core in approved_core:
        copy_exclusive(payload / "exports" / core["path"], destination / "candidate/assets" / core["path"], core["sha256"])
    locator = destination / "candidate/assets/manifests/wall-production-v1.wallset"
    locator.parent.mkdir(parents=True)
    locator.write_bytes(canonical_bytes(runtime))
    return {"status": "pass", "generation": generation, "manifest": str(output),
            "manifest_sha256": sha256(output), "authority": runtime["authority"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("base-manifest", "art-root", "preview-manifest", "approval", "standard-job", "farthest-job", "destination", "repo"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--generation", type=int, required=True)
    args = parser.parse_args()
    print(seal(base_path=args.base_manifest.resolve(), art_root=args.art_root.resolve(),
               preview_path=args.preview_manifest.resolve(), approval_path=args.approval.resolve(),
               standard=args.standard_job.resolve(), farthest=args.farthest_job.resolve(),
               destination=args.destination.resolve(), generation=args.generation, repo=args.repo.resolve()))


if __name__ == "__main__":
    main()
