"""Independent raw material/image witness for the closed Tank projection pilot."""

from __future__ import annotations
import hashlib
import math
from PIL import Image

try:
    from . import building_release_art_observation as released
    from .perf_tool.artifact_readers import building_m6 as geometry
except ImportError:
    import building_release_art_observation as released
    from perf_tool.artifact_readers import building_m6 as geometry
common = released.common
m6 = common.m6
require, canonical = m6.pipeline.require, common.canonical
PROFILE = "building-tank-illustration-pilot-v1"
WIDTH, HEIGHT = released.WIDTH, released.HEIGHT


def vector(value, n):
    require(
        isinstance(value, list)
        and len(value) == n
        and all(type(x) in (int, float) and math.isfinite(x) for x in value),
        "pilot finite vector differs",
    )


def matrix_pose(matrix, translation):
    vector(matrix, 16)
    vector(translation, 3)
    expected = [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        *translation,
        1.0,
    ]
    require(
        all(abs(x - y) < 0.001 for x, y in zip(matrix, expected, strict=True)),
        "pilot actual global transform differs",
    )


def validate_witness(witness, session, fixture, view_root, projection):
    paint = session["paint"]
    base = session["gallery"]
    require(
        set(witness)
        == {
            "profile",
            "accepted",
            "promotion_authority",
            "performance_evidence",
            "paint_image",
            "water_image",
            "owners",
        }
        and witness["profile"] == PROFILE
        and all(
            witness[k] is False
            for k in ("accepted", "promotion_authority", "performance_evidence")
        ),
        "pilot witness scope differs",
    )
    tank = next(
        b["identity"] for b in base["bindings"] if b["identity"]["kind"] == "Tank"
    )
    manifest = m6.pipeline.read(
        view_root / "assets/manifests/building-tank-v1.buildingset"
    )
    atlas = next(
        a["path"] for a in manifest["artifacts"] if a["role"] == "image:albedo"
    )
    with Image.open(view_root / "assets" / atlas) as im:
        rgba = im.convert("RGBA")
        water_rgba = m6.pipeline.digest(rgba.tobytes())
        water_size = list(rgba.size)
    images = [
        (
            witness["paint_image"],
            paint["path"],
            paint["dimensions"],
            paint["rgba_sha256"],
        ),
        (witness["water_image"], atlas, water_size, water_rgba),
    ]
    for observed, path, size, sha in images:
        require(
            set(observed)
            == {
                "handle",
                "asset_path",
                "width",
                "height",
                "format",
                "mip_levels",
                "rgba_sha256",
                "bytes",
            },
            "pilot image witness fields differ",
        )
        released.handle(observed["handle"])
        require(
            all(
                type(observed[k]) is int
                for k in ("width", "height", "mip_levels", "bytes")
            ),
            "pilot image wire integers differ",
        )
        require(
            observed["asset_path"] == path
            and [observed["width"], observed["height"]] == size
            and observed["format"] == "Rgba8UnormSrgb"
            and observed["mip_levels"] == 1
            and observed["rgba_sha256"] == sha
            and observed["bytes"] == size[0] * size[1] * 4,
            "pilot decoded image differs",
        )
    records = {r["gallery_owner"]: r for r in fixture["records"] if r["kind"] == "Tank"}
    rows = witness["owners"]
    edges = projection["tank_paint_hierarchy"]
    geometry_ids = {
        g["entity"] for r in records.values() for g in r["gallery_geometry"]
    }
    require(
        isinstance(edges, list) and len(edges) == len(geometry_ids) == 8,
        "pilot actual parent coverage differs",
    )
    for edge in edges:
        require(
            set(edge) == {"entity", "parent", "owner"}, "pilot parent fields differ"
        )
        for key in edge:
            released.entity(edge[key])
    require(
        len({e["entity"] for e in edges}) == 8
        and {e["entity"] for e in edges} == geometry_ids,
        "pilot actual parent entities differ",
    )
    hierarchy = {e["entity"]: e for e in edges}
    require(
        isinstance(rows, list)
        and len(rows) == 4
        and len({r["owner"] for r in rows}) == 4
        and {r["owner"] for r in rows} == set(records),
        "pilot owner coverage differs",
    )
    roots = set()
    entities = set()
    body_materials = set()
    water_materials = set()
    meshes = {}
    for row in rows:
        require(
            set(row)
            == {"owner", "root", "identity", "root_pose", "root_global", "parts"}
            and row["identity"] == tank,
            "pilot root identity fields differ",
        )
        released.entity(row["root"])
        require(row["root"] not in roots, "pilot duplicate Root")
        roots.add(row["root"])
        record = records[row["owner"]]
        root_position = [record["center"][0], 0.0, -record["center"][1]]
        pose = row["root_pose"]
        for key, n in [("translation", 3), ("rotation", 4), ("scale", 3)]:
            vector(pose[key], n)
        require(
            set(pose) == {"translation", "rotation", "scale"}
            and pose["translation"] == root_position
            and pose["rotation"] == [0.0, 0.0, 0.0, 1.0]
            and pose["scale"] == [1.0, 1.0, 1.0],
            "pilot Root origin differs",
        )
        matrix_pose(row["root_global"], root_position)
        geometry_rows = record["gallery_geometry"]
        require(
            len(geometry_rows) == 2
            and {g["mesh_role"] for g in geometry_rows} == {"body", "water"},
            "pilot actual geometry coverage differs",
        )
        actual_geometry = {g["mesh_role"]: g for g in geometry_rows}
        projected = [
            r
            for r in projection["records"]
            if r["kind"] == "Tank" and r["owner"] == row["owner"]
        ]
        require(len(projected) == 1, "pilot projected owner missing")
        actual_shapes = {shape["entity"]: shape for shape in projected[0]["shapes"]}
        parts = row["parts"]
        require(
            len(parts) == 2 and {p["role"] for p in parts} == {"body", "water"},
            "pilot parts differ",
        )
        for part in parts:
            require(
                set(part)
                == {
                    "entity",
                    "role",
                    "mesh",
                    "material",
                    "mode",
                    "unlit",
                    "alpha",
                    "base_color",
                    "texture",
                    "projection",
                    "matte",
                    "local",
                    "global",
                    "visible",
                },
                "pilot material witness fields differ",
            )
            released.entity(part["entity"])
            require(part["entity"] not in entities, "pilot reused part")
            entities.add(part["entity"])
            for key in ("mesh", "material", "texture"):
                released.handle(part[key])
            require(
                part["unlit"] is False and part["alpha"] == "Opaque",
                "pilot surface flags changed",
            )
            vector(part["base_color"], 4)
            vector(part["projection"], 4)
            vector(part["matte"], 4)
            role = part["role"]
            meshes.setdefault(role, set()).add(part["mesh"])
            local = part["local"]
            for key, n in [("translation", 3), ("rotation", 4), ("scale", 3)]:
                vector(local[key], n)
            require(
                set(local) == {"translation", "rotation", "scale"}
                and local["rotation"] == [0.0, 0.0, 0.0, 1.0]
                and local["scale"] == [1.0, 1.0, 1.0],
                "pilot local pose changed",
            )
            actual = actual_geometry[role]
            edge = hierarchy[part["entity"]] if part["entity"] in hierarchy else None
            require(
                edge is not None
                and edge["parent"] == row["root"]
                and edge["owner"] == row["owner"]
                and part["entity"] == actual["entity"]
                and part["mesh"] == actual["mesh"]
                and m6.pipeline.exact_json(local, actual["local_pose"])
                and part["visible"] is actual["visible"],
                "pilot material is not bound to the observed geometry",
            )
            if part["visible"]:
                shape = actual_shapes.get(part["entity"])
                require(
                    shape is not None
                    and shape["type"] == "mesh3d"
                    and shape["mesh"] == part["mesh"]
                    and m6.pipeline.exact_json(shape["world_matrix"], part["global"]),
                    "pilot material is not bound to the rendered shape",
                )
            else:
                require(
                    part["entity"] not in actual_shapes,
                    "hidden pilot water was rendered",
                )
            count = record["state"]["stored_water"]
            height = 0.0 if role == "body" else 22.0 if count == 50 else 12.0
            require(
                local["translation"] == [0.0, height, 0.0]
                and part["visible"] is (role == "body" or count != 0),
                "pilot water state/pose differs",
            )
            matrix_pose(part["global"], [root_position[0], height, root_position[2]])
            if role == "body":
                require(
                    part["mode"] == 2.0
                    and part["texture"] == witness["paint_image"]["handle"]
                    and part["projection"] == [*paint["canvas_wu"], *paint["anchor_uv"]]
                    and part["matte"] == [*paint["matte_linear_rgb"], 1.0]
                    and all(abs(x - 1.0) <= 1e-6 for x in part["base_color"][:3])
                    and part["base_color"][3] == 1.0,
                    "pilot body paint not actually applied",
                )
                body_materials.add(part["material"])
            else:
                require(
                    abs(part["mode"] - 0.82) < 1e-6
                    and part["texture"] == witness["water_image"]["handle"],
                    "pilot paint mixed into water",
                )
                expected = (
                    [1.0, 1.0, 1.0, 1.0]
                    if count in (0, 50)
                    else [0.58, 0.70, 0.73, 1.0]
                )
                require(
                    all(
                        abs(x - y) < 1e-5
                        for x, y in zip(part["base_color"], expected, strict=True)
                    ),
                    "pilot water color state changed",
                )
                water_materials.add(part["material"])
    require(
        not (roots & set(records))
        and not (roots & entities)
        and not (entities & set(records))
        and len(body_materials) == 1
        and not (body_materials & water_materials)
        and all(len(ids) == 1 for ids in meshes.values())
        and not (meshes["body"] & meshes["water"]),
        "pilot sharing/mesh roles differ",
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
        "pilot raw fields differ",
    )
    for obj in (probe, final):
        require(
            type(obj["schema_version"]) is int
            and obj["schema_version"] == 1
            and type(obj["pid"]) is int
            and type(obj["stable_frames"]) is int
            and obj["profile"] == PROFILE
            and all(
                obj[k] is False
                for k in ("performance_evidence", "accepted", "promotion_authority")
            ),
            "pilot raw scope differs",
        )
    require(
        set(session) == {"schema_version", "contract_id", "gallery", "paint"}
        and type(session["schema_version"]) is int
        and session["schema_version"] == 1
        and session["contract_id"] == PROFILE
        and m6.pipeline.exact_json(session["paint"], value["paint"]),
        "pilot session differs",
    )
    base = session["gallery"]
    require(
        probe["status"] == "ready"
        and final["status"] == "complete"
        and m6.pipeline.exact_json(probe["session"], session)
        and probe["nonce"] == final["nonce"] == base["nonce"]
        and type(probe["pid"]) is int
        and probe["pid"] > 0
        and probe["pid"] == final["pid"]
        and 60 <= probe["stable_frames"] <= final["stable_frames"]
        and type(probe["ready_at_unix_ms"]) is int
        and type(final["completed_at_unix_ms"]) is int
        and 0 < probe["ready_at_unix_ms"] <= final["completed_at_unix_ms"],
        "pilot capture readiness differs",
    )
    require(
        canonical(probe["snapshot"]) == canonical(final["snapshot"]),
        "pilot actual witness changed",
    )
    sidecar = {
        "schema_version": 1,
        "contract_id": geometry.TANK_PILOT_CONTRACT,
        "evidence_kind": "paused-static-only",
        "active_simulation_evidence": False,
        "camera_scale": 1.0 if base["view"] == "normal" else 2.0,
        "stable_frames": probe["stable_frames"],
        "layout_sha256": hashlib.sha256(canonical(probe["fixture"])).hexdigest(),
        "initial": probe["fixture"],
        "final": final["fixture"],
    }
    frozen = geometry.validate_tank_pilot_sidecar(
        sidecar, m6.Case("building-art-static", "small", "gpu", 20260920, 15, 0)
    )
    require(
        m6.pipeline.exact_json(
            frozen["m6"]["identities"], [b["identity"] for b in base["bindings"]]
        )
        and frozen["m6"]["door_identity"] == base["door_identity"],
        "pilot base identities differ",
    )
    m6.validate_authored_parts(value, frozen)
    projection = probe["snapshot"]
    require(
        set(projection)
        == released.PROJECTION_KEYS | {"tank_paint", "tank_paint_hierarchy"},
        "pilot projection/witness coverage differs",
    )
    common.released_snapshot(
        {k: projection[k] for k in released.PROJECTION_KEYS}, base, frozen, view_root
    )
    validate_witness(projection["tank_paint"], session, frozen, view_root, projection)
