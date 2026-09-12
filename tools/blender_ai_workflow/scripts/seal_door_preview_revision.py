"""Seal a preview-only correction, retaining approval solely for unchanged 3D art.

This is not a new art approval. A user-directed projection correction is recorded
in the hash-bound preview report, separately from the original double-leaf review.
The original final generation is fully revalidated and its four non-preview core
files, blend, geometry, material evidence and license must remain byte-identical.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path

import asset_release_manifest as release
import door_preview_projection as projection
import seal_door_final as final
import validate_door_manifest as validator
import validate_door_textures as textures
from workflow_common import write_json_atomic


def unchanged_art(base: dict, exports: Path) -> None:
    for record in base["production"]["core"][:4]:
        validator.production_record(record, exports, record["path"], record["role"])


def seal(*, base_path: Path, root: Path, repo: Path, generation: int,
         approved_at: str, instruction: str, authorization: str, output: Path) -> dict:
    commit, tree = final.git_sealer.clean_git_identity(repo)
    release.validate(base_path, repo=repo)
    base = validator.read_json(base_path)
    final.require(base["asset_set_id"] == validator.ASSET_SET_ID, "base must be a final Door")
    final.require(generation > base["asset_set_generation"], "revision must advance generation")
    final.require(bool(instruction.strip()) and bool(authorization.strip()), "user direction is required")
    final.timestamp(approved_at)
    reports = root / "staging/reports"
    exports = root / "staging/exports"
    final.require(output.parent == reports and not output.exists(), "revision output must be new and in reports")
    unchanged_art(base, exports)
    preview_path = reports / base["preview_report"]["path"]
    preview = validator.read_json(preview_path)
    hashes = {axis: validator.sha256(exports / f"textures/buildings/door/door_preview_{axis}.png")
              for axis in ("ew", "ns")}
    projection.validate_report(preview, hashes)
    for index, axis in ((4, "ew"), (5, "ns")):
        final.require(hashes[axis] != base["production"]["core"][index]["sha256"],
                      f"{axis} preview was not corrected")
    preview["revision"] = {
        "kind": "user_directed_projection_correction",
        "base_generation": base["asset_set_generation"],
        "base_manifest_sha256": validator.sha256(base_path),
        "unchanged_art_core": base["production"]["core"][:4],
        "original_art_review": base["art_review"],
        "approval_scope": "Original review covers unchanged 3D art only; new PNGs are a user-directed bugfix, not a new visual approval.",
        "user_instruction": instruction,
        "release_authorization": authorization,
        "authorized_at_utc": approved_at,
        "tool_commit": commit,
    }
    write_json_atomic(preview_path, preview)
    texture_path = reports / base["texture_report"]["path"]
    write_json_atomic(texture_path, textures.validate(exports / "textures/buildings/door", preview))
    result = copy.deepcopy(base)
    result["asset_set_generation"] = generation
    result["created_at_utc"] = approved_at
    for index, axis in ((4, "ew"), (5, "ns")):
        record = result["production"]["core"][index]
        record.update(sha256=hashes[axis], bytes=(exports / record["path"]).stat().st_size)
    for key, path in (("preview_report", preview_path), ("texture_report", texture_path)):
        result[key]["sha256"] = validator.sha256(path)
    result["provenance"]["previews"] = projection.PROFILE
    result["art_review"]["notes"] = base["art_review"].get("notes", "") + " Retained for unchanged meshes/albedo only; preview_report records the separate projection correction authorization."
    source = result["source"]
    source.update(runtime_subject=commit, tool_commit=commit, tool_tree=tree, working_tree_diff_sha256="0" * 64)
    source["source_fingerprint"] = hashlib.sha256("".join((
        commit, tree, "0" * 64, *(record["sha256"] for record in result["production"]["core"])
    )).encode()).hexdigest()
    # Full validation also checks unchanged blend, geometry, original approval,
    # mesh reports, texture provenance and license against their original hashes.
    temporary = output.with_name(f".{output.name}.sealing")
    final.require(not temporary.exists(), "stale revision temporary exists")
    write_json_atomic(temporary, result)
    release.validate(temporary, repo=repo)
    temporary.rename(output)
    return {"status": "pass", "generation": generation, "manifest_sha256": validator.sha256(output)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("base-manifest", "asset-root", "repo", "output"):
        parser.add_argument(f"--{name}", required=True, type=Path)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--authorized-at-utc", required=True)
    parser.add_argument("--user-instruction", required=True)
    parser.add_argument("--release-authorization", required=True)
    args = parser.parse_args()
    print(json.dumps(seal(base_path=args.base_manifest.resolve(), root=args.asset_root.resolve(),
                          repo=args.repo.resolve(), generation=args.generation,
                          approved_at=args.authorized_at_utc, instruction=args.user_instruction,
                          authorization=args.release_authorization, output=args.output.resolve())))


if __name__ == "__main__":
    main()
