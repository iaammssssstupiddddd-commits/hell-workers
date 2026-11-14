"""Released-only nine-kind image oracle. No performance or release authority."""

from __future__ import annotations

import hashlib

if __package__:
    from . import building_group_art_observation as common
    from .perf_tool.artifact_readers.building_m6 import (
        RELEASE_GALLERY_CONTRACT,
        validate_release_gallery_sidecar,
    )
else:
    import building_group_art_observation as common
    from perf_tool.artifact_readers.building_m6 import (
        RELEASE_GALLERY_CONTRACT,
        validate_release_gallery_sidecar,
    )

m6 = common.m6
require, canonical = m6.pipeline.require, common.canonical
PROFILE = "building-release-art-gallery-v1"
WIDTH, HEIGHT = common.WIDTH, common.HEIGHT
PROJECTION_KEYS = {
    "window",
    "rtt",
    "main_clip_from_world",
    "rtt_clip_from_world",
    "overlay_clip_from_world",
    "composite_world",
    "visible_ui_nodes",
    "records",
}


def entity(value):
    require(type(value) is int and value >= 0, "invalid context entity")
    return value


def handle(value):
    require(
        isinstance(value, str) and 0 < len(value) <= 256, "invalid context asset handle"
    )
    return value


def grid_point(grid):
    require(
        isinstance(grid, list)
        and len(grid) == 2
        and all(type(n) is int and 0 <= n < 100 for n in grid),
        "invalid context grid",
    )
    return [(n - 49.5) * 32 for n in grid]


def anchor(value, projection, *, extra):
    require(
        set(value)
        == {"entity", "position", "rotation", "scale", "physical_anchor"} | extra,
        "context anchor fields differ",
    )
    entity(value["entity"])
    common.vector(value["position"], 3)
    common.vector(value["rotation"], 4)
    common.vector(value["scale"], 3)
    require(
        value["rotation"] == [0.0, 0.0, 0.0, 1.0] and value["scale"] == [1.0, 1.0, 1.0],
        "context owner transform differs",
    )
    expected = common.project(
        projection["main_clip_from_world"], value["position"], [WIDTH, HEIGHT]
    )
    common.close(value["physical_anchor"], expected)
    require(
        16 <= expected[0] <= WIDTH - 16 and 16 <= expected[1] <= HEIGHT - 16,
        "context anchor clipped",
    )


def meshes(rows, *, allowed, material_key, material, entities, layers):
    require(
        isinstance(rows, list) and 0 < len(rows) <= 256,
        "missing/oversized context geometry",
    )
    for row in rows:
        require(
            set(row)
            == {
                "entity",
                "mesh",
                "structural_material",
                "standard_material",
                "world_matrix",
                "aabb_min",
                "aabb_max",
                "layers",
                "visible",
            },
            "context mesh fields differ",
        )
        identity = entity(row["entity"])
        require(identity not in entities, "duplicate context visual")
        entities.add(identity)
        require(
            handle(row["mesh"]) in allowed
            and row["visible"] is True
            and isinstance(row["layers"], list)
            and all(type(n) is int for n in row["layers"])
            and row["layers"] == layers,
            "context mesh not actual visible production geometry",
        )
        require(
            row[material_key] == material and material is not None,
            "context material differs",
        )
        other = (
            "standard_material"
            if material_key == "structural_material"
            else "structural_material"
        )
        require(row[other] is None, "context material authority is ambiguous")
        common.vector(row["world_matrix"], 16)
        lo, hi = common.vector(row["aabb_min"], 3), common.vector(row["aabb_max"], 3)
        require(
            all(a <= b for a, b in zip(lo, hi, strict=True))
            and any(a < b for a, b in zip(lo, hi, strict=True)),
            "empty/inverted context mesh bounds",
        )


def context(value, projection, frozen, expected_wall):
    require(
        set(value)
        == {
            "scope",
            "projection_scope",
            "core_souls",
            "total_souls",
            "structures",
            "wall_identity",
            "wall_meshes",
            "wall_material",
            "floor_mesh",
            "floor_material",
            "carrier_mesh",
            "carrier_grid",
            "floor_grid",
            "carrier",
            "held_resource",
            "item",
            "item_hidden",
            "icon",
        },
        "released context fields differ",
    )
    require(
        value["scope"] == "seeded-release-storyboard-context"
        and value["projection_scope"]
        == "context-anchors-only; full appearance independently inspected"
        and type(value["total_souls"]) is int
        and value["total_souls"] == 16
        and value["held_resource"] == "Wood"
        and value["item_hidden"] is True,
        "context scope/population/resource differs",
    )
    require(
        m6.pipeline.exact_json(value["wall_identity"], expected_wall)
        and expected_wall["authority"] == "release_approved",
        "context Wall authority differs",
    )
    require(
        isinstance(value["core_souls"], list)
        and len(value["core_souls"]) == 15
        and len(set(entity(n) for n in value["core_souls"])) == 15,
        "context core Souls differ",
    )
    require(
        isinstance(value["wall_meshes"], list)
        and len(value["wall_meshes"]) == 6
        and len(set(handle(n) for n in value["wall_meshes"])) == 6,
        "context Wall family coverage differs",
    )
    for name in ("wall_material", "floor_mesh", "floor_material", "carrier_mesh"):
        handle(value[name])
    # The normal nine-kind layout has fixed first EW Door (8,43). These are its
    # actual existing supports; no topology edit is permitted for the picture.
    grid_point(value["floor_grid"])
    grid_point(value["carrier_grid"])
    require(
        value["floor_grid"] == [10, 43] and value["carrier_grid"] == [11, 34],
        "context declared cells differ",
    )
    targets = {row["gallery_owner"] for row in frozen["records"]}
    residents = {part["resident"][key] for row in frozen["records"]
                for part in row.get("parts", []) if "resident" in part
                for key in ("root", "entity")}
    require(not (targets | residents) & set(value["core_souls"]), "context core Soul overlaps target")
    occupied = set(value["core_souls"]) | targets
    visuals = {
        shape["entity"] for row in projection["records"] for shape in row["shapes"]
    } | residents
    structures = value["structures"]
    require(
        isinstance(structures, list)
        and [row["kind"] for row in structures] == ["Wall", "Wall", "Floor"],
        "context structure coverage differs",
    )
    context_owners = [entity(row["entity"]) for row in structures] + [
        entity(value["carrier"]["entity"])
    ]
    require(
        len(set(context_owners)) == len(context_owners)
        and not set(context_owners) & (occupied | residents),
        "context owner identity overlaps",
    )
    all_owners = occupied | set(context_owners)
    for row, grid in zip(structures, ([7, 43], [9, 43], [10, 43]), strict=True):
        anchor(row, projection, extra={"kind", "visible_meshes"})
        require(
            row["position"][:2] == grid_point(grid), "context structure cell differs"
        )
        identity = row["entity"]
        require(identity not in occupied, "context owner overlaps target/core Soul")
        occupied.add(identity)
        wall = row["kind"] == "Wall"
        require(len(row["visible_meshes"]) == 1, "context structural visual duplicated")
        meshes(
            row["visible_meshes"],
            allowed=value["wall_meshes"] if wall else [value["floor_mesh"]],
            material_key="structural_material",
            material=value["wall_material"] if wall else value["floor_material"],
            entities=visuals,
            layers=[1, 5],
        )
    soul = value["carrier"]
    anchor(soul, projection, extra={"visible_meshes"})
    require(
        soul["entity"] not in occupied
        and soul["position"][:2] == grid_point(value["carrier_grid"]),
        "context extra carrier differs",
    )
    occupied.add(soul["entity"])
    require(len(soul["visible_meshes"]) == 1, "context carrier geometry differs")
    material = handle(soul["visible_meshes"][0]["standard_material"])
    meshes(
        soul["visible_meshes"],
        allowed=[value["carrier_mesh"]],
        material_key="standard_material",
        material=material,
        entities=visuals,
        layers=[1],
    )
    item = entity(value["item"])
    require(
        item not in occupied and item not in visuals,
        "context held item overlaps another entity",
    )
    icon = value["icon"]
    anchor(
        icon,
        projection,
        extra={"image", "size", "color", "world_matrix", "worker", "item"},
    )
    common.vector(icon["size"], 2)
    common.vector(icon["color"], 4)
    require(
        icon["entity"] not in occupied | visuals | {item}
        and icon["worker"] == soul["entity"]
        and icon["item"] == item
        and icon["size"] == [12.0, 12.0]
        and all(abs(c - 1.0) <= 1e-6 for c in icon["color"][:3])
        and icon["color"][3] == 1.0,
        "context actual inventory icon differs",
    )
    require(not visuals & all_owners, "context mesh overlaps actual owner/core Soul")
    handle(icon["image"])
    common.close(
        icon["position"],
        [soul["position"][0], soul["position"][1] + 20.0, soul["position"][2] + 0.5],
    )
    common.close(
        icon["world_matrix"],
        m6.art.foreground.matrix(
            {
                "translation": icon["position"],
                "rotation": icon["rotation"],
                "scale": icon["scale"],
            }
        ),
    )


def validate(probe, final, session, value, view_root):
    require(
        set(probe)
        == {
            "schema_version",
            "profile",
            "status",
            "nonce",
            "pid",
            "session",
            "stable_frames",
            "ready_at_unix_ms",
            "fixture",
            "snapshot",
            "performance_evidence",
            "accepted",
            "promotion_authority",
        }
        and set(final)
        == {
            "schema_version",
            "profile",
            "status",
            "nonce",
            "pid",
            "probe_sha256",
            "ack_sha256",
            "ack",
            "stable_frames",
            "completed_at_unix_ms",
            "fixture",
            "snapshot",
            "performance_evidence",
            "accepted",
            "promotion_authority",
        },
        "released gallery probe/final fields differ",
    )
    require(
        all(
            type(obj["schema_version"]) is int
            and obj["schema_version"] == 1
            and obj["profile"] == PROFILE
            and all(
                obj[key] is False
                for key in ("accepted", "performance_evidence", "promotion_authority")
            )
            for obj in (probe, final)
        ),
        "released gallery scope differs",
    )
    require(
        probe["status"] == "ready"
        and final["status"] == "complete"
        and m6.pipeline.exact_json(probe["session"], session)
        and probe["nonce"] == final["nonce"] == session["nonce"]
        and type(probe["pid"]) is int
        and type(final["pid"]) is int
        and probe["pid"] > 0
        and probe["pid"] == final["pid"]
        and type(probe["stable_frames"]) is int
        and type(final["stable_frames"]) is int
        and 60 <= probe["stable_frames"] <= final["stable_frames"]
        and type(probe["ready_at_unix_ms"]) is int
        and type(final["completed_at_unix_ms"]) is int
        and 0 < probe["ready_at_unix_ms"] <= final["completed_at_unix_ms"],
        "released gallery readiness differs",
    )
    require(
        set(session)
        == {
            "schema_version",
            "contract_id",
            "bindings",
            "door_identity",
            "nonce",
            "status_path",
            "view",
            "quality",
            "scale_factor",
        }
        and type(session["schema_version"]) is int
        and session["schema_version"] == 1
        and session["contract_id"] == PROFILE
        and session["quality"] == "high"
        and type(session["scale_factor"]) in (int, float)
        and session["scale_factor"] == 1
        and session["view"] in ("normal", "far")
        and m6.pipeline.exact_json(session["bindings"], value["assets"]["bindings"]),
        "released gallery session differs",
    )
    require(
        canonical(probe["snapshot"]) == canonical(final["snapshot"]),
        "released gallery snapshot drift",
    )
    sidecar = {
        "schema_version": 1,
        "contract_id": RELEASE_GALLERY_CONTRACT,
        "evidence_kind": "paused-static-only",
        "active_simulation_evidence": False,
        "camera_scale": 1.0 if session["view"] == "normal" else 2.0,
        "stable_frames": probe["stable_frames"],
        "layout_sha256": hashlib.sha256(canonical(probe["fixture"])).hexdigest(),
        "initial": probe["fixture"],
        "final": final["fixture"],
    }
    frozen = validate_release_gallery_sidecar(
        sidecar, m6.Case("building-art-static", "small", "gpu", 20260920, 15, 0)
    )
    require(
        m6.pipeline.exact_json(
            frozen["m6"]["identities"], [b["identity"] for b in session["bindings"]]
        )
        and m6.pipeline.exact_json(
            frozen["m6"]["door_identity"], session["door_identity"]
        ),
        "released gallery actual identities differ",
    )
    m6.validate_authored_parts(value, frozen)
    projection = probe["snapshot"]
    require(
        set(projection) == PROJECTION_KEYS | {"context"},
        "released gallery projection/context coverage differs",
    )
    selected = {key: projection[key] for key in PROJECTION_KEYS}
    common.released_snapshot(selected, session, frozen, view_root)
    context(projection["context"], selected, frozen, value["wall_identity"])
