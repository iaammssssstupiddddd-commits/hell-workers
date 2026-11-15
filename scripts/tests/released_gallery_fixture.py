"""Synthetic complete 36-owner wire fixture; grants no native/art authority."""

import copy
import itertools
import json
from pathlib import Path

from PIL import Image
from scripts import building_release_art_observation as oracle
from scripts.tests.test_building_m6_acceptance import sidecar
from scripts.tests.resident_fixture import bind as bind_resident
from scripts.tests.test_building_release_art_context import fixture as context_fixture


def full_fixture(view):
    common, m6 = oracle.common, oracle.m6
    frozen = sidecar(True)["initial"]
    manifests = []
    for kind in m6.pipeline.RELEASE_GROUPS["m5"]:
        row = next(r for r in frozen["records"] if r["kind"] == kind)
        parts = copy.deepcopy(row["parts"])
        for i, part in enumerate(parts):
            if kind == "SoulSpa":
                part["translation_wu"][0] = float(i * 20)
        manifest = {
            "identity": next(
                i for i in frozen["m6"]["identities"] if i["kind"] == kind
            ),
            "parts": [dict({k: v for k, v in p.items() if k not in {"visible", "resident"}},
                           **({"name": p["resident"]["name"]} if "resident" in p else {})) for p in parts],
            "production_state": {
                "kind": "Tank",
                "partial_y_wu": 12.0,
                "full_y_wu": 22.0,
            },
        }
        if "foreground" in row:
            manifest["world_preview"] = copy.deepcopy(row["foreground"]["descriptor"])
            manifest["artifacts"] = []
            for role in (
                ("world_off", "world_on") if kind == "OutdoorLamp" else ("world",)
            ):
                path = Path(view) / "assets" / kind / (role + ".png")
                path.parent.mkdir(parents=True, exist_ok=True)
                image = Image.new("RGBA", (256, 256))
                image.paste((100, 100, 100, 255), (8, 8, 248, 248))
                image.save(path)
                payload = path.read_bytes()
                manifest["artifacts"].append(
                    {
                        "role": "image:" + role,
                        "path": f"{kind}/{role}.png",
                        "bytes": len(payload),
                        "sha256": m6.pipeline.digest(payload),
                    }
                )
        manifests.append(manifest)
    for index, row in enumerate(frozen["records"]):
        owner = row.get("owner", 2000 + index)
        row["gallery_owner"] = owner
        if row["kind"] != "Door":
            manifest = next(
                m for m in manifests if m["identity"]["kind"] == row["kind"]
            )
            expected = m6.projected_parts(manifest, row)
            row["parts"] = [
                dict({k: v for k, v in p.items() if k != "name"}, visible=actual["visible"])
                for p, actual in zip(expected, row["parts"], strict=True)
            ]
        if row["kind"] in {*m6.art.foreground.KINDS}:
            role = row["foreground"]["image_role"]
            row["foreground"]["artifact"] = copy.deepcopy(
                next(a for a in manifest["artifacts"] if a["role"] == "image:" + role)
            )
        if row["kind"] in {"Tank", "MudMixer", "RestArea", "SoulSpa", "Door"}:
            parts = row.get(
                "parts",
                [
                    {
                        "mesh_role": "door",
                        "translation_wu": [row["center"][0], 16.0, -row["center"][1]],
                        "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
                        "scale": [1.0, 1.0, 1.0],
                        "visible": True,
                    }
                ],
            )
            row["gallery_geometry"] = [
                {
                    "entity": 10000 + index * 8 + i,
                    "mesh": f"mesh-{index}-{i}",
                    "mesh_role": p["mesh_role"],
                    "visible": p["visible"],
                    "local_pose": {
                        "translation": p["translation_wu"][:],
                        "rotation": p["rotation_xyzw"][:],
                        "scale": p["scale"][:],
                    },
                    "aabb_min": [0.0, 0.0, 0.0],
                    "aabb_max": [10.0, 10.0, 10.0],
                }
                for i, p in enumerate(parts)
            ]
        if row["kind"] in m6.resident.ROLES:
            bind_resident(row, manifest["identity"], index)
    main = [
        1 / 960,
        0,
        0,
        0,
        0,
        1 / 840,
        0,
        0,
        0,
        0,
        0.001,
        0,
        1072 / 960,
        688 / 840,
        0.5,
        1,
    ]
    rtt = [
        1 / 960,
        0,
        0,
        0,
        0,
        0.5 / 840,
        0.001,
        0,
        0,
        -1 / 840,
        0,
        0,
        1072 / 960,
        688 / 840,
        0.5,
        1,
    ]
    overlay = [1 / 960, 0, 0, 0, 0, 1 / 840, 0, 0, 0, 0, 0.001, 0, 0, 0, 0.5, 1]
    composite = [1920, 0, 0, 0, 0, 1680, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    projection = {
        "window": {
            "physical": [1920, 1680],
            "logical": [1920.0, 1680.0],
            "dpi": 1.0,
            "decorations": False,
        },
        "rtt": {
            "physical": [1920.0, 1680.0],
            "logical": [1920.0, 1680.0],
            "target_scale_factor": 1.0,
            "quality": "high",
        },
        "main_clip_from_world": main,
        "rtt_clip_from_world": rtt,
        "overlay_clip_from_world": overlay,
        "composite_world": composite,
        "visible_ui_nodes": 0,
        "records": [],
    }
    for row in frozen["records"]:
        record = {
            "kind": row["kind"],
            "ordinal": row["ordinal"],
            "owner": row["gallery_owner"],
            "shapes": [],
        }
        points = []
        if "gallery_geometry" in row:
            root = m6.art.foreground.matrix(
                {
                    "translation": [row["center"][0], 0.0, -row["center"][1]],
                    "rotation": [0.0, 0.0, 0.0, 1.0],
                    "scale": [1.0, 1.0, 1.0],
                }
            )
            for geometry in row["gallery_geometry"]:
                if not geometry["visible"]:
                    continue
                local = m6.art.foreground.matrix(geometry["local_pose"])
                matrix = (
                    local
                    if row["kind"] == "Door"
                    else m6.art.foreground.product(root, local)
                )
                shape = {
                    "type": "mesh3d",
                    "entity": geometry["entity"],
                    "mesh": geometry["mesh"],
                    "aabb_min": geometry["aabb_min"],
                    "aabb_max": geometry["aabb_max"],
                    "world_matrix": matrix,
                    "physical_corners": [],
                    "rtt_texel_corners": [],
                }
                for corner in itertools.product((0.0, 10.0), repeat=3):
                    world = common.transform(matrix, [*corner, 1.0])
                    texel = common.project(rtt, world[:3], [1920, 1680])
                    shape["rtt_texel_corners"].append(texel)
                    uv = [texel[0] / 1920, texel[1] / 1680]
                    composed = common.transform(
                        composite, [uv[0] - 0.5, 0.5 - uv[1], 0, 1]
                    )
                    shape["physical_corners"].append(
                        common.project(overlay, composed[:3], [1920, 1680])
                    )
                points.extend(shape["physical_corners"])
                record["shapes"].append(shape)
        else:
            fg = row["foreground"]
            matrix = fg["global_matrix"]
            shape = {
                "type": "sprite",
                "entity": fg["entity"],
                "image": fg["image_handle"],
                "world_matrix": matrix,
                "canvas_wu": fg["canvas_wu"],
                "anchor": fg["anchor"],
                "image_size": [256, 256],
                "opaque_bounds_px": [8, 8, 248, 248],
                "physical_corners": [],
            }
            for x, y in itertools.product((8, 248), repeat=2):
                local = [
                    (x / 256 - 0.5) * fg["canvas_wu"][0],
                    (0.5 - y / 256) * fg["canvas_wu"][1],
                    0,
                    1,
                ]
                shape["physical_corners"].append(
                    common.project(
                        main, common.transform(matrix, local)[:3], [1920, 1680]
                    )
                )
            points.extend(shape["physical_corners"])
            record["shapes"].append(shape)
        record["physical_bounds"] = common.bounds(points)
        projection["records"].append(record)
    ctx, _, _, wall = context_fixture()
    projection["context"] = ctx
    value = {
        "assets": {
            "bindings": [{"identity": m["identity"]} for m in manifests],
            "manifests": manifests,
        },
        "wall_identity": wall,
    }
    session = {
        "schema_version": 1,
        "contract_id": oracle.PROFILE,
        "bindings": value["assets"]["bindings"],
        "door_identity": frozen["m6"]["door_identity"],
        "nonce": "a" * 32,
        "status_path": str(Path(view) / "probe.json"),
        "view": "normal",
        "quality": "high",
        "scale_factor": 1.0,
    }
    probe = {
        "schema_version": 1,
        "profile": oracle.PROFILE,
        "status": "ready",
        "nonce": session["nonce"],
        "pid": 123,
        "session": session,
        "stable_frames": 60,
        "ready_at_unix_ms": 1000,
        "fixture": frozen,
        "snapshot": projection,
        "performance_evidence": False,
        "accepted": False,
        "promotion_authority": False,
    }
    final = {
        k: copy.deepcopy(v)
        for k, v in probe.items()
        if k not in ("session", "ready_at_unix_ms")
    }
    final.update(
        status="complete",
        completed_at_unix_ms=1200,
        probe_sha256="0" * 64,
        ack_sha256="1" * 64,
        ack={},
    )
    # Remove all accidental Python list aliases, like real independent JSON files.
    return json.loads(json.dumps((probe, final, session, value)))
