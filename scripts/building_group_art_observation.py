"""Independent group gallery oracle; never performance, art or release approval."""
from __future__ import annotations

import hashlib
import itertools
import math
from pathlib import Path

from PIL import Image

if __package__:
    from . import building_m6_acceptance as m6
    from .perf_tool.artifact_readers.building_m6 import (
        GROUP_GALLERY_CONTRACT, GROUP_TARGETS, canonical, validate_group_gallery_sidecar,
    )
else:
    import building_m6_acceptance as m6
    from perf_tool.artifact_readers.building_m6 import (
        GROUP_GALLERY_CONTRACT, GROUP_TARGETS, canonical, validate_group_gallery_sidecar,
    )

PROFILE = "building-group-art-gallery-v1"
WIDTH, HEIGHT = 1920, 1680
require = m6.pipeline.require


def vector(value, length):
    require(isinstance(value, list) and len(value) == length
            and all(type(n) in (int, float) and math.isfinite(n) for n in value), "nonfinite/invalid projection vector")
    return value


def transform(matrix, point):
    vector(matrix, 16)
    vector(point, 4)
    return [sum(matrix[column * 4 + row] * point[column] for column in range(4)) for row in range(4)]


def project(matrix, point, extent):
    x, y, z, w = transform(matrix, [*point, 1.0])
    require(w > 0 and math.isfinite(w) and 0 <= z / w <= 1, "point outside camera depth")
    return [(x / w + 1) * extent[0] / 2, (1 - y / w) * extent[1] / 2]


def bounds(points):
    require(bool(points), "empty projected geometry")
    for point in points:
        vector(point, 2)
    return [min(p[0] for p in points), min(p[1] for p in points),
            max(p[0] for p in points), max(p[1] for p in points)]


def close(actual, expected):
    # Actual camera calculations are f32; this is projection arithmetic only.
    vector(actual, len(expected))
    require(all(abs(a - b) <= .25 for a, b in zip(actual, expected, strict=True)), "projected geometry differs")


def area(points, view):
    box = bounds(points)
    side, minimum = (4, 64) if view == "normal" else (2, 16)
    require(box[0] >= 16 and box[1] >= 16 and box[2] <= WIDTH - 16 and box[3] <= HEIGHT - 16
            and box[2] - box[0] >= side and box[3] - box[1] >= side
            and (box[2] - box[0]) * (box[3] - box[1]) >= minimum,
            "group target clipped or below minimum pixel footprint")
    return box


def fixture(probe, final, group):
    value = {"schema_version": 1, "contract_id": GROUP_GALLERY_CONTRACT,
             "evidence_kind": "paused-static-only", "active_simulation_evidence": False,
             "camera_scale": 1.0 if probe["session"]["view"] == "normal" else 2.0,
             "stable_frames": probe["stable_frames"],
             "layout_sha256": hashlib.sha256(canonical(probe["fixture"])).hexdigest(),
             "initial": probe["fixture"], "final": final["fixture"]}
    observed = validate_group_gallery_sidecar(value, m6.Case("building-art-static", "small", "gpu", 20260920, 15, 0))
    require(observed["m6"]["group"]["group"] == group, "gallery group differs")
    return observed


def snapshot(value, session, frozen, view_root):
    require(set(value) == {"window", "rtt", "main_clip_from_world", "rtt_clip_from_world",
            "overlay_clip_from_world", "composite_world", "visible_ui_nodes", "records"}, "gallery projection keys differ")
    dpi = session["scale_factor"]
    require(type(dpi) in (int, float) and dpi in (1, 1.5, 2), "unsupported gallery DPI")
    logical = [WIDTH / dpi, HEIGHT / dpi]
    require(m6.pipeline.exact_json(value["window"], {"physical": [WIDTH, HEIGHT], "logical": logical, "dpi": dpi})
            and type(value["visible_ui_nodes"]) is int and value["visible_ui_nodes"] == 0, "gallery window/UI differs")
    scale = {"high": 1.0, "medium": .75, "low": .5}[session["quality"]]
    require(m6.pipeline.exact_json(value["rtt"], {"physical": [WIDTH * scale, HEIGHT * scale], "logical": logical,
            "target_scale_factor": scale * dpi, "quality": session["quality"]}), "gallery RTT differs")
    for name in ("main_clip_from_world", "rtt_clip_from_world", "overlay_clip_from_world", "composite_world"):
        vector(value[name], 16)
    selected = (*GROUP_TARGETS[session["bindings"]["group"]], "Door")
    expected = {(kind, ordinal) for kind in selected for ordinal in range(4)}
    rows = value["records"]
    require(isinstance(rows, list) and len(rows) == len(expected)
            and {(r["kind"], r["ordinal"]) for r in rows} == expected, "gallery target coverage differs")
    owners, entities = [], []
    for row in rows:
        require(set(row) == {"kind", "ordinal", "owner", "physical_bounds", "shapes"}
                and type(row["ordinal"]) is int and type(row["owner"]) is int, "gallery owner fields differ")
        owners.append(row["owner"])
        original = next(r for r in frozen["records"] if (r["kind"], r["ordinal"]) == (row["kind"], row["ordinal"]))
        structural = row["kind"] in {"Tank", "MudMixer", "RestArea", "SoulSpa", "Door"}
        require(row["owner"] == original["gallery_owner"], "projection owner differs from actual fixture")
        require(len(row["shapes"]) == (sum(p["visible"] for p in original["parts"]) if "parts" in original and structural else 1),
                "gallery shape coverage differs")
        unmatched_parts = [p for p in original.get("parts", []) if p["visible"] is True] if structural else []
        points, texels = [], []
        for shape in row["shapes"]:
            require(type(shape["entity"]) is int, "invalid shape entity")
            entities.append(shape["entity"])
            matrix = shape["world_matrix"]
            vector(matrix, 16)
            if structural:
                require(set(shape) == {"type", "entity", "mesh", "aabb_min", "aabb_max", "world_matrix",
                        "physical_corners", "rtt_texel_corners"} and shape["type"] == "mesh3d", "gallery mesh fields differ")
                resident = [part for part in original["gallery_geometry"] if part["entity"] == shape["entity"]]
                require(len(resident) == 1 and resident[0]["visible"] is True, "projection leaf differs from resident consumer")
                resident = resident[0]
                require(shape["mesh"] == resident["mesh"] and shape["aabb_min"] == resident["aabb_min"]
                    and shape["aabb_max"] == resident["aabb_max"], "projection mesh/AABB differs from resident source")
                if row["kind"] == "Door":
                    expected_pose = {"translation": [original["center"][0], 16., -original["center"][1]],
                                     "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
                    require(resident["mesh_role"] == "door" and resident["local_pose"] == expected_pose,
                            "gallery Door frame transform differs")
                    expected_matrix = m6.art.foreground.matrix(expected_pose)
                else:
                    pose = resident["local_pose"]
                    matches = [p for p in unmatched_parts if p["mesh_role"] == resident["mesh_role"]
                        and p["translation_wu"] == pose["translation"] and p["rotation_xyzw"] == pose["rotation"]
                        and p["scale"] == pose["scale"] and p["visible"] is True]
                    require(bool(matches), "gallery leaf differs from unconsumed authored actual part")
                    unmatched_parts.remove(matches[0])
                    root_matrix = m6.art.foreground.matrix({"translation": [original["center"][0], 0., -original["center"][1]],
                                     "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]})
                    expected_matrix = m6.art.foreground.product(root_matrix, m6.art.foreground.matrix(pose))
                require(m6.art.foreground.close(matrix, expected_matrix), "projection world transform differs from admitted root/part")
                lo, hi = vector(shape["aabb_min"], 3), vector(shape["aabb_max"], 3)
                require(all(a <= b for a, b in zip(lo, hi, strict=True)), "inverted mesh bounds")
                expected_points, expected_texels = [], []
                for local in itertools.product(*zip(lo, hi, strict=True)):
                    world = transform(matrix, [*local, 1])
                    require(abs(world[3] - 1) < 1e-6, "non-affine model transform")
                    p = project(value["rtt_clip_from_world"], world[:3], logical)
                    uv = [p[i] / logical[i] for i in range(2)]
                    require(all(0 <= n <= 1 for n in uv), "mesh outside RTT texture")
                    composed = transform(value["composite_world"], [uv[0] - .5, .5 - uv[1], 0, 1])
                    expected_points.append(project(value["overlay_clip_from_world"], composed[:3], [WIDTH, HEIGHT]))
                    expected_texels.append([uv[i] * value["rtt"]["physical"][i] for i in range(2)])
                require(len(shape["physical_corners"]) == 8 and len(shape["rtt_texel_corners"]) == 8, "mesh corner coverage differs")
                for actual, point in zip(shape["physical_corners"], expected_points, strict=True):
                    close(actual, point)
                for actual, point in zip(shape["rtt_texel_corners"], expected_texels, strict=True):
                    close(actual, point)
                points.extend(expected_points)
                texels.extend(expected_texels)
            else:
                require(set(shape) == {"type", "entity", "image", "world_matrix", "canvas_wu", "anchor",
                        "image_size", "opaque_bounds_px", "physical_corners"} and shape["type"] == "sprite", "gallery sprite fields differ")
                foreground = original["foreground"]
                require(row["owner"] == foreground["owner"] and shape["entity"] == foreground["entity"]
                    and shape["image"] == foreground["image_handle"] and matrix == foreground["global_matrix"]
                    and shape["canvas_wu"] == foreground["canvas_wu"] and shape["anchor"] == foreground["anchor"],
                    "projection sprite differs from admitted foreground consumer")
                artifact = original["foreground"]["artifact"]
                path = m6.pipeline.rooted(Path(view_root) / "assets", artifact["path"])
                require(hashlib.sha256(path.read_bytes()).hexdigest() == artifact["sha256"], "gallery sprite artifact changed")
                with Image.open(path) as image:
                    image = image.convert("RGBA")
                    require(list(image.size) == shape["image_size"]
                            and list(image.getchannel("A").getbbox() or ()) == shape["opaque_bounds_px"], "gallery alpha footprint differs")
                vector(shape["canvas_wu"], 2)
                vector(shape["anchor"], 2)
                lo_x, lo_y, hi_x, hi_y = shape["opaque_bounds_px"]
                expected_points = []
                for x, y in itertools.product((lo_x, hi_x), (lo_y, hi_y)):
                    local = [(x / shape["image_size"][0] - .5 - shape["anchor"][0]) * shape["canvas_wu"][0],
                             (.5 - y / shape["image_size"][1] - shape["anchor"][1]) * shape["canvas_wu"][1], 0, 1]
                    expected_points.append(project(value["main_clip_from_world"], transform(matrix, local)[:3], [WIDTH, HEIGHT]))
                require(len(shape["physical_corners"]) == 4, "sprite corner coverage differs")
                for actual, point in zip(shape["physical_corners"], expected_points, strict=True):
                    close(actual, point)
                points.extend(expected_points)
        if structural:
            require(not unmatched_parts, "gallery authored visible parts were not observed exactly once")
            box = bounds(texels)
            require((box[2] - box[0]) * (box[3] - box[1]) >= 16, "insufficient RTT texel footprint")
        close(row["physical_bounds"], area(points, session["view"]))
    require(len(set(owners)) == len(owners) and len(set(entities)) == len(entities), "duplicate gallery owner/shape")


def validate(probe, final, session, value, view_root):
    require(set(probe) == {"schema_version", "profile", "status", "nonce", "pid", "session", "stable_frames",
            "ready_at_unix_ms", "fixture", "snapshot", "performance_evidence", "accepted", "promotion_authority"}
        and set(final) == {"schema_version", "profile", "status", "nonce", "pid", "probe_sha256", "ack_sha256", "ack",
            "stable_frames", "completed_at_unix_ms", "fixture", "snapshot", "performance_evidence", "accepted", "promotion_authority"},
        "gallery probe/final fields differ")
    require(type(probe["schema_version"]) is int and type(final["schema_version"]) is int
            and probe["schema_version"] == final["schema_version"] == 1
            and probe["profile"] == final["profile"] == PROFILE
            and probe["status"] == "ready" and final["status"] == "complete"
            and m6.pipeline.exact_json(probe["session"], session)
            and probe["nonce"] == final["nonce"] == session["nonce"]
            and type(probe["pid"]) is int and type(final["pid"]) is int and probe["pid"] > 0 and probe["pid"] == final["pid"]
            and type(probe["stable_frames"]) is int and probe["stable_frames"] >= 60
            and type(final["stable_frames"]) is int and final["stable_frames"] >= probe["stable_frames"]
            and all(obj[key] is False for obj in (probe, final) for key in ("accepted", "performance_evidence", "promotion_authority")),
            "gallery runtime scope/readiness differs")
    require(all(type(obj[key]) is int and obj[key] > 0 for obj, key in (
        (probe, "ready_at_unix_ms"), (final, "completed_at_unix_ms"))), "invalid gallery clock wire type")
    observed = fixture(probe, final, session["bindings"]["group"])
    require(canonical(observed["m6"]["group"]) == canonical(value["group_declaration"])
            and canonical(probe["snapshot"]) == canonical(final["snapshot"]), "gallery identity/snapshot drift")
    m6.validate_authored_parts(value, observed)
    snapshot(probe["snapshot"], session, observed, view_root)
