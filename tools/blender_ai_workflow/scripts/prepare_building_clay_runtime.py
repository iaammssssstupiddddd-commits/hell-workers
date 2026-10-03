"""Package verified M2 neutral clay through M1-c; never grant art/release approval.

Coordinator entrypoint after build_building_clay.py build. No build or native
process is started here except the explicitly supplied, already-built codec and
the existing clay bundle validators. Each kind gets a separate isolated view.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import build_building_clay as clay
import building_asset_pipeline as pipeline
from building_clay_geometry import FIXTURES, PILOTS, load_pilot
from validate_building_clay_glb import validate
from validate_wall_glb import accessor_values, read_glb


def state_geometry(kind: str, output: Path) -> dict:
    """Recompute state extents and continuous sweep from the actual exported GLB."""
    contract = load_pilot(kind)
    roles = {role: validate(output / f"{role}.glb", kind, role) for role in contract["roles"]}
    moving = "water" if kind == "Tank" else "rotor"
    document, binary = read_glb(output / f"{moving}.glb")
    accessor = document["meshes"][0]["primitives"][0]["attributes"]["POSITION"]
    points = accessor_values(document, binary, accessor)
    if kind == "Tank":
        states = {}
        for state, value in contract["states"].items():
            height = value["water_y_wu"]
            minimum = min(point[1] for point in points) + height
            maximum = max(point[1] for point in points) + height
            pipeline.require(contract["construction"]["floor_y"] < minimum < maximum
                             < contract["construction"]["rim_y"], "water state escapes cavity")
            states[state] = {"visible": value["water_visible"], "water_y_wu": height,
                             "surface_bounds_y_wu": [minimum, maximum]}
        return {"roles": roles, "states": states}
    # Maximum radius of a linear triangle lies at a vertex, so this bounds the
    # complete continuous revolution, rather than just sampled degree poses.
    radius = max(math.hypot(point[0], point[2]) for point in points)
    pivot = contract["roles"]["rotor"]["translation_wu"]
    minimum = min(point[1] for point in points) + pivot[1]
    maximum = max(point[1] for point in points) + pivot[1]
    boxes = contract["construction"]["body_boxes"]
    floor = boxes[0]["center"][1] + boxes[0]["size"][1] / 2
    inner_x = boxes[2]["center"][0] - boxes[2]["size"][0] / 2
    inner_z = boxes[4]["center"][2] - boxes[4]["size"][2] / 2
    support_bottom = boxes[-1]["center"][1] - boxes[-1]["size"][1] / 2
    pipeline.require(pivot[0] == pivot[2] == 0 and radius < min(inner_x, inner_z)
                     and minimum > floor and abs(maximum - support_bottom) <= contract["tolerance_wu"],
                     "rotor sweep intersects trough or misses shaft support")
    return {"roles": roles, "rotor_sweep": {
        "axis": "local_y", "pivot_wu": pivot, "radius_wu": radius,
        "bounds_y_wu": [minimum, maximum], "clearance_x_wu": inner_x - radius,
        "clearance_z_wu": inner_z - radius, "clearance_floor_wu": minimum - floor,
        "support_contact_y_wu": support_bottom, "continuous_full_turn": True}}


def recipe(kind: str, generation: int, source: Path) -> dict:
    contract = load_pilot(kind)
    pipeline.identity(kind, generation, "art_preview")
    representative = contract["preview"]["representative_state"]

    def record(name):
        return pipeline.record(name, (source / name).read_bytes())

    artifacts = [{"role": "mesh:" + role, **record(role + ".glb")} for role in contract["roles"]]
    artifacts.extend({"role": "image:" + role, **record(name)} for role, name in (
        ("albedo", "albedo.png"), ("world_preview", representative + ".png"),
        ("catalog", representative + ".png")))
    preview = {"image_role": "world_preview", **contract["preview"]}
    # The existing square clay representative is reused; the catalog's center
    # anchor is a UI contract, not the world ground-contact anchor.
    catalog = {**preview, "image_role": "catalog",
               "anchor_px": [value / 2 for value in preview["canvas_px"]]}
    return {"schema_version": 1, "kind": kind, "generation": generation,
            "source": record("source.blend"), "geometry_contract": record("geometry.json"),
            "artifacts": artifacts, "parts": [
                {"name": role, "mesh_role": role, "material_role": "opaque_albedo",
                 "translation_wu": spec["translation_wu"], "rotation_xyzw": [0, 0, 0, 1],
                 "scale": [1, 1, 1]} for role, spec in contract["roles"].items()],
            "world_preview": preview, "catalog_preview": catalog}


def prepare(kind: str, name: str, generation: int, destination: Path, codec: Path) -> dict:
    pipeline.identity(kind, generation, "art_preview")
    destination = pipeline.no_symlinks(destination)
    pipeline.require(not destination.exists(), "use a fresh clay runtime output directory")
    verification = clay.verify(kind, name)
    blend, output, report, _ = clay.paths(kind, name)
    geometry = state_geometry(kind, output)
    contract = load_pilot(kind)
    files = {"source.blend": blend.read_bytes(),
             "geometry.json": (FIXTURES / f"building-{PILOTS[kind]}-v1.geometry.json").read_bytes(),
             "clay-report.json": report.read_bytes(), "albedo.png": (output / "albedo.png").read_bytes()}
    for role in contract["roles"]:
        files[role + ".glb"] = (output / f"{role}.glb").read_bytes()
    for state in json.loads(report.read_bytes())["renders"]:
        files[state + ".png"] = (output / f"{state}.png").read_bytes()
    source = destination / "authoring"
    for name, payload in files.items():
        pipeline.put(pipeline.rooted(source, name), payload)
    recipe_path = destination / "recipe.json"
    pipeline.put(recipe_path, pipeline.canonical(recipe(kind, generation, source)))
    exported = pipeline.export(recipe_path, source, destination / "runtime", codec, authority="art_preview")
    result = {"schema_version": 1, "scope": "m2-neutral-clay-only", "kind": kind,
              "identity": exported["identity"], "runtime_root": str(destination / "runtime"),
              "technical_bundle": verification, "geometry": geometry,
              "art_approved": False, "production_candidate": False, "runtime_published": False,
              "actual_window_approved": False, "numeric_freeze": False,
              "remaining": ["coordinator native clay comparison", "independent clay decision",
                            "numeric freeze before color/UV", "dynamic and lifecycle native acceptance",
                            "performance budget and comparison", "art approval and release"]}
    pipeline.put(destination / "clay-runtime.json", pipeline.canonical(result))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=tuple(PILOTS))
    parser.add_argument("--name", required=True)
    parser.add_argument("--generation", required=True, type=int)
    parser.add_argument("--destination", required=True, type=Path)
    parser.add_argument("--codec", required=True, type=Path)
    args = parser.parse_args()
    print(json.dumps(prepare(args.kind, args.name, args.generation, args.destination, args.codec), indent=2))


if __name__ == "__main__":
    main()
