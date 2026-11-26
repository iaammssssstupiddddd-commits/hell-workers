"""Read-only M0 inventory/shape/protected-image gate. Does not authorize runtime assets."""

from __future__ import annotations

import argparse
import hashlib
import re
from pathlib import Path

from building_clay_geometry import FIXTURES, PILOTS, build, load_pilot, read_json, require

REPO = FIXTURES.parents[2]


def check(repo: Path, *, check_images: bool = False) -> dict:
    contract = read_json(FIXTURES / "building-art-v1.contract.json")
    require(contract["schema_version"] == 1 and contract["contract_id"] == "building-art-v1", "contract identity differs")
    model = (repo / "crates/hw_jobs/src/model.rs").read_text()
    all_match = re.search(r"pub const ALL: \[Self; \d+\] = \[(.*?)\];", model, re.S)
    require(all_match is not None, "BuildingType::ALL declaration unavailable; review shape audit")
    equipment = re.findall(r"Self::Equipment\(crate::Equipment::(\w+)\)", all_match[1])
    catalog = (repo / "crates/hw_jobs/src/equipment.rs").read_text().split("pub const ALL:", 1)[1].split("];", 1)[0]
    require(equipment == re.findall(r"Self::(\w+)", catalog), "equipment inventory drift")
    kinds = re.findall(r"Self::(\w+),", all_match[1])
    # Built-in wall fittings use the shared structural mesh and ship their own tile art.
    embedded = {"ScreamingVent": {"shape": "single_tile"}}
    require((repo / "crates/bevy_app/src/assets/screaming_vent.png").is_file(), "vent grate image is missing")
    expected = set(contract["kinds"]) | set(contract["inherited"]) | set(contract["excluded"]) | set(embedded)
    require(len(kinds) == len(expected) and set(kinds) == expected, "BuildingType::ALL inventory drift")
    shapes = (repo / contract["shape_source"]).read_text()
    for name, shape in contract["shapes"].items():
        match = re.search(rf"const {name.upper()}:.*?= &\[(.*?)\];", shapes, re.S)
        require(match is not None, f"shape declaration unavailable: {name}")
        tiles = [[int(x), int(y)] for x, y in re.findall(r"\((-?\d+),\s*(-?\d+)\)", match[1])]
        require(tiles == shape["ordered_tiles"], f"ordered logical shape drift: {name}")
    # Audit the actual match arms, not just unused constant declarations.
    # Remove payload alternatives before checking unit variant match arms.
    shapes = re.sub(r"BuildingType::Equipment\(_\)\s*\|", "", shapes)
    arms = re.findall(r"((?:BuildingType::\w+\s*\|?\s*)+) => BuildingShape \{(.*?)\}", shapes, re.S)
    mapping = {kind: body for head, body in arms for kind in re.findall(r"BuildingType::(\w+)", head)}
    for kind, spec in (contract["kinds"] | contract["inherited"] | embedded).items():
        body = mapping.get(kind, "")
        shape = contract["shapes"][spec["shape"]]
        require(f'ordered_relative_tiles: {spec["shape"].upper()}' in body, f"shape assignment drift: {kind}")
        require(f'BuildingAnchorBasis::{shape["anchor_basis"]}' in body, f"anchor drift: {kind}")
        for key, field in (("center_tiles", "center_offset_tiles"), ("size_tiles", "size_tiles")):
            actual = re.search(rf"{field}: \(([-\d.]+), ([-\d.]+)\)", body)
            require(actual is not None and [float(actual[1]), float(actual[2])] == shape[key], f"{field} drift: {kind}")
    for path in contract["consumers"].values():
        require((repo / path).exists(), f"preview consumer moved: {path}")
    require(sum(len(spec["mesh_roles"]) for spec in contract["kinds"].values()) == 8, "mesh inventory differs")
    for spec in contract["kinds"].values():
        for field in ("mesh_roles", "image_roles"):
            require(len(spec[field]) == len(set(spec[field])), "duplicate role")
    pilots = {kind: {role: geo.triangles() for role, geo in build(kind, load_pilot(kind)).items()} for kind in PILOTS}
    if check_images:
        for path, expected_hash in read_json(FIXTURES / "building-art-protected-images-v1.json")["files"].items():
            require(hashlib.sha256((repo / path).read_bytes()).hexdigest() == expected_hash, f"protected image changed: {path}")
    return {"status": "pass", "kinds": 10, "new_sets": 9, "pilot_triangles": pilots,
            "protected_images": "pass" if check_images else "not_checked", "runtime_implemented": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=REPO)
    parser.add_argument("--check-images", action="store_true", help="Requires the external runtime asset mirror")
    args = parser.parse_args()
    print(check(args.repo, check_images=args.check_images))


if __name__ == "__main__":
    main()
