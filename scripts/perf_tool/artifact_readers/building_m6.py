"""Independent M6 paused oracle. A valid sidecar is not lifecycle/art acceptance."""

from __future__ import annotations

import hashlib
import json
import math
import re
import struct

try:
    from ... import building_foreground_observation as foreground
except ImportError:
    import building_foreground_observation as foreground

from .building_art_static import KINDS, expected_records
from ..model import Case

CONTRACT = "building-art-m6-nine-v1"
PRE_RELEASE_CONTRACT = "building-art-pre-release-nine-v1"
DOOR_HASH = "4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449"
SETS = tuple(kind for kind in KINDS if kind != "Door")
PARTS = {
    "Tank": ["body", "water"],
    "MudMixer": ["body", "rotor"],
    "RestArea": ["body"],
    "SoulSpa": ["body", "slot", "slot", "slot", "slot"],
}
RESOURCE_KEYS = {
    "resident_mesh_count",
    "resident_structural_material_count",
    "resident_image_count",
    "resident_image_cpu_bytes",
    "structural_material_shallow_bytes",
}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def canonical(value):
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()


def identities(values, *, pre_release=False):
    require(
        isinstance(values, list) and len(values) == 8,
        "M6 requires eight identities plus dedicated Door",
    )
    require(
        {value["kind"] for value in values} == set(SETS), "M6 kind inventory differs"
    )
    for value in values:
        require(
            set(value) == {"kind", "generation", "authority", "manifest_sha256"}
            and type(value["generation"]) is int
            and value["generation"] > 0
            and value["authority"] == ("isolated_candidate" if pre_release else "release_approved")
            and isinstance(value["manifest_sha256"], str)
            and re.fullmatch(r"[0-9a-f]{64}", value["manifest_sha256"]),
            "invalid release identity",
        )


def validate_sidecar(value, case):
    return _validate_sidecar(value, case, pre_release=False)


def validate_pre_release_sidecar(value, case):
    return _validate_sidecar(value, case, pre_release=True)


def _validate_sidecar(value, case, *, pre_release):
    copies = {"small": 4, "medium": 16}[case.size]
    require(
        case
        == Case("building-art-static", case.size, "gpu", 20260920, copies // 4 * 15, 0),
        "M6 logical case differs",
    )
    require(
        set(value)
        == {
            "schema_version",
            "contract_id",
            "evidence_kind",
            "active_simulation_evidence",
            "camera_scale",
            "stable_frames",
            "layout_sha256",
            "initial",
            "final",
        },
        "sidecar keys differ",
    )
    require(
        type(value["schema_version"]) is int
        and value["schema_version"] == 1
        and value["contract_id"] == (PRE_RELEASE_CONTRACT if pre_release else CONTRACT)
        and value["evidence_kind"] == "paused-static-only"
        and value["active_simulation_evidence"] is False
        and value["camera_scale"] == 5.0
        and type(value["stable_frames"]) is int
        and value["stable_frames"] >= 30,
        "M6 scope/readiness differs",
    )
    initial = value["initial"]
    require(
        canonical(initial) == canonical(value["final"])
        and hashlib.sha256(canonical(initial)).hexdigest() == value["layout_sha256"],
        "M6 checkpoint drift",
    )
    require(
        set(initial)
        == ({
            "records",
            "target_count",
            "target_structural_roots",
            "target_foreground_owners",
            "target_active_unique_meshes",
            "souls",
            "completion_effects",
            "m6",
        } | ({"pre_release_comparison"} if pre_release else set())),
        "inventory keys differ",
    )
    if pre_release:
        require(initial["pre_release_comparison"] is True, "pre-release scope differs")
    m6 = initial["m6"]
    require(
        set(m6)
        == {
            "mode",
            "identities",
            "door_identity",
            "target_mesh_entities",
            "pool_active",
            "pool_pending",
            "resources",
            "gpu_allocation_bytes",
            "active_state_acceptance",
        },
        "M6 fields differ",
    )
    require(m6["mode"] in {"legacy-control", "candidate"}, "invalid M6 mode")
    candidate = m6["mode"] == "candidate"
    identities(m6["identities"], pre_release=pre_release)
    require(
        m6["door_identity"]
        == {
            "asset_set_generation": 7,
            "authority": "release_approved",
            "manifest_sha256": DOOR_HASH,
        },
        "Door authority differs",
    )
    require(
        m6["active_state_acceptance"] is False and m6["gpu_allocation_bytes"] is None,
        "false acceptance/measurement claim",
    )
    expected_counts = {
        "target_count": copies * 9,
        "target_structural_roots": copies * 5,
        "target_foreground_owners": copies * 4,
        "target_active_unique_meshes": 8 if candidate else 2,
        "souls": copies // 4 * 15,
        "completion_effects": 0,
    }
    for key, expected in expected_counts.items():
        require(
            type(initial[key]) is int and initial[key] == expected, f"incorrect {key}"
        )
    for key, expected in {
        "target_mesh_entities": copies * (11 if candidate else 5),
        "pool_active": 8 if candidate else 0,
        "pool_pending": 0,
    }.items():
        require(type(m6[key]) is int and m6[key] == expected, f"incorrect {key}")
    require(set(m6["resources"]) == RESOURCE_KEYS, "resource fields differ")
    require(
        all(type(number) is int and number > 0 for number in m6["resources"].values()),
        "invalid resources",
    )
    expected = expected_records(copies)
    require(len(initial["records"]) == len(expected), "owner count differs")
    foreground_entities = []
    for observed, logical in zip(initial["records"], expected, strict=True):
        extras = {"parts"} if candidate and logical["kind"] != "Door" else set()
        if candidate and logical["kind"] in foreground.KINDS:
            extras.update({"owner", "foreground"})
        if candidate and logical["kind"] in {"Tank", "MudMixer"}:
            extras.add("root_translation_wu")
            require(
                observed["root_translation_wu"]
                == [logical["center"][0], 0.0, -logical["center"][1]],
                "root feet differ",
            )
        require(
            set(observed) == set(logical) | extras
            and canonical({key: observed[key] for key in logical})
            == canonical(logical),
            "logical state/layout differs",
        )
        if "foreground" in extras:
            actual = observed["foreground"]
            require(isinstance(actual, dict), "M6 foreground object missing")
            preview = actual.get("descriptor")
            require(isinstance(preview, dict) and set(preview) == {
                "anchor_px", "canvas_px", "canvas_wu", "image_role", "representative_state"},
                "M6 foreground descriptor keys differ")
            role = "world_on" if logical["kind"] == "OutdoorLamp" and logical["ordinal"] % 4 >= 2 else preview["image_role"]
            identity = next(value for value in m6["identities"] if value["kind"] == logical["kind"])
            artifact = actual.get("artifact")
            require(isinstance(artifact, dict) and set(artifact) == {"role", "path", "bytes", "sha256"}
                    and artifact["role"] == "image:" + role
                    and type(artifact["bytes"]) is int and artifact["bytes"] > 0
                    and isinstance(artifact["path"], str) and artifact["path"]
                    and isinstance(artifact["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", artifact["sha256"]),
                    "M6 foreground artifact fields differ")
            foreground_entities.extend(foreground.validate_record(observed, identity, preview, {artifact["role"]: artifact}, role))
            # Production completion uses Z_BUILDING_FLOOR for resource piles,
            # Z_BUILDING_STRUCT for Parking/Lamp. Project the actual f32 rules.
            owner_z = .05 if logical["kind"] in {"SandPile", "BonePile"} else .12
            owner_pose = {"translation": [*logical["center"], struct.unpack("<f", struct.pack("<f", owner_z))[0]],
                          "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            local_pose = {"translation": [0., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            require(canonical(actual["owner_pose"]) == canonical(owner_pose)
                    and canonical(actual["local_pose"]) == canonical(local_pose)
                    and canonical(actual["global_pose"]) == canonical(owner_pose)
                    and canonical(actual["global_matrix"]) == canonical(foreground.matrix(owner_pose))
                    and canonical(actual["parent_local_matrix"]) == canonical(foreground.matrix(owner_pose))
                    and all(abs(number - 1.) <= 1e-6 for number in actual["color_srgba"][:3])
                    and actual["color_srgba"][3] == 1., "M6 foreground static pose/color differs")
        if "parts" in extras:
            parts = observed["parts"]
            require(
                [part["mesh_role"] for part in parts] == PARTS.get(logical["kind"], []),
                "part role/count differs",
            )
            for part in parts:
                require(
                    set(part)
                    == {
                        "mesh_role",
                        "translation_wu",
                        "rotation_xyzw",
                        "scale",
                        "visible",
                    },
                    "part fields differ",
                )
                for key, length in (
                    ("translation_wu", 3),
                    ("rotation_xyzw", 4),
                    ("scale", 3),
                ):
                    require(
                        len(part[key]) == length
                        and all(
                            type(n) in (int, float) and math.isfinite(n)
                            for n in part[key]
                        ),
                        "invalid transform",
                    )
                hidden = (
                    logical["kind"] == "Tank"
                    and part["mesh_role"] == "water"
                    and logical["state"]["stored_water"] == 0
                )
                require(
                    type(part["visible"]) is bool and part["visible"] != hidden,
                    "part visibility differs",
                )
    require(len(set(foreground_entities)) == len(foreground_entities), "M6 duplicate foreground owner or sprite")
    return initial
