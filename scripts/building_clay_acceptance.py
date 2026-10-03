"""Independent M2 clay observations on top of the M1-c native recipe.

Run the existing building_art_acceptance.py plan/run no-prompt launcher first,
once per kind in feedback or art-preview mode. This verifier checks all Tank
levels or the Mixer's paused initial pose from that exact actual-window job.
It is not an art decision, animation/lifecycle test, or performance acceptance.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
import building_asset_pipeline as pipeline  # noqa: E402
from building_clay_geometry import FIXTURES, PILOTS, load_pilot  # noqa: E402


def close_vector(actual, expected, tolerance=0.001):
    return (isinstance(actual, list) and len(actual) == len(expected)
            and all(type(a) in (int, float) and math.isfinite(a) and abs(a - e) <= tolerance
                    for a, e in zip(actual, expected, strict=True)))


def inspect_records(kind: str, records: list, contract: dict) -> dict:
    pipeline.require(kind in PILOTS and contract["kind"] == kind, "clay kind differs")
    selected = [row for row in records if row.get("kind") == kind]
    pipeline.require(len(selected) == 4 and {row["ordinal"] for row in selected} == set(range(4)),
                     "clay requires the four exact small-fixture specimens")
    observed_states = set()
    for row in selected:
        center = row["center"]
        pipeline.require(close_vector(row.get("root_translation_wu"), [center[0], 0, -center[1]]),
                         "clay feet differ from logical owner")
        parts = row.get("parts", [])
        pipeline.require(len(parts) == 2 and {part["mesh_role"] for part in parts} == set(contract["roles"]),
                         "clay role inventory differs")
        for part in parts:
            role = part["mesh_role"]
            position = list(contract["roles"][role]["translation_wu"])
            visible = True
            if kind == "Tank" and role == "water":
                count, capacity = row["state"]["stored_water"], row["state"]["capacity"]
                state = "Empty" if count == 0 else ("Full" if capacity > 0 and count >= capacity else "Partial")
                observed_states.add(state)
                position[1] = contract["states"][state]["water_y_wu"]
                visible = contract["states"][state]["water_visible"]
            elif kind == "MudMixer":
                observed_states.add("RefiningPaused" if row["state"]["refining"] else "IdlePaused")
            pipeline.require(part.get("visible") is visible and close_vector(part.get("translation_wu"), position)
                             and close_vector(part.get("rotation_xyzw"), [0, 0, 0, 1])
                             and close_vector(part.get("scale"), [1, 1, 1]), "clay role pose/visibility differs")
    expected = {"Empty", "Partial", "Full"} if kind == "Tank" else {"IdlePaused", "RefiningPaused"}
    pipeline.require(observed_states == expected, "clay state coverage differs")
    return {"kind": kind, "specimens": 4, "states": sorted(observed_states)}


def verify(job_root: Path) -> dict:
    import building_art_acceptance

    evidence = building_art_acceptance.verify(job_root)
    kind = evidence["identity"]["kind"]
    pipeline.require(kind in PILOTS and evidence["identity"]["authority"] == "art_preview"
                     and evidence["mode"] in {"feedback", "art-preview"}, "not an unapproved clay job")
    source = Path(evidence["plan"]["source_root"])
    expected_geometry = (FIXTURES / f"building-{PILOTS[kind]}-v1.geometry.json").read_bytes()
    pipeline.require(pipeline.rooted(source, "provenance/geometry.json").read_bytes() == expected_geometry,
                     "native geometry is not the current clay fixture")
    manifest = json.loads(pipeline.rooted(source, pipeline.locator(kind)).read_bytes())
    pipeline.require(manifest["geometry_contract_sha256"] == pipeline.digest(expected_geometry),
                     "native manifest geometry binding differs")
    # Match exported role transforms and preview descriptors to the same fixture,
    # then check observed ECS values, not just the requested manifest.
    contract = load_pilot(kind)
    for part in manifest["parts"]:
        pipeline.require(close_vector(part["translation_wu"], contract["roles"][part["name"]]["translation_wu"])
                         and close_vector(part["rotation_xyzw"], [0, 0, 0, 1])
                         and close_vector(part["scale"], [1, 1, 1]), "manifest clay pose differs")
    pipeline.require(manifest["world_preview"] == {"image_role": "world_preview", **contract["preview"]},
                     "clay preview descriptor differs")
    observation = inspect_records(kind, evidence["probe"]["fixture"]["records"], contract)
    return {"status": "pass", "scope": "m2-paused-clay-observations-only", "observation": observation,
            "native_manifest_sha256": pipeline.digest((job_root / "manifest.json").read_bytes()),
            "identity": evidence["identity"], "feedback_only": evidence["feedback_only"],
            "art_approved": False, "numeric_freeze": False, "promotion_authority": False,
            "animation_verified": False, "lifecycle_verified": False, "performance_verified": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--job-root", required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.job_root), indent=2))


if __name__ == "__main__":
    main()
