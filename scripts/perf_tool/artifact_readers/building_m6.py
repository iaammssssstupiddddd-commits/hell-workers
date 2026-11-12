"""Independent M6 paused oracle. A valid sidecar is not lifecycle/art acceptance."""

from __future__ import annotations

import hashlib
import json
import math
import re
import struct
from pathlib import PurePosixPath

try:
    from ... import building_foreground_observation as foreground
except ImportError:
    import building_foreground_observation as foreground

from .building_art_static import KINDS, expected_records
from ..model import Case

CONTRACT = "building-art-m6-nine-v1"
PRE_RELEASE_CONTRACT = "building-art-pre-release-nine-v1"
GROUP_CONTRACT = "building-art-group-nine-v1"
GROUP_TARGETS = {
    "m2": ("Tank", "MudMixer"),
    "m3": ("RestArea", "SoulSpa"),
    "m5": ("WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"),
}
GROUP_FIXED = {"m2": (), "m3": GROUP_TARGETS["m2"],
               "m5": (*GROUP_TARGETS["m2"], *GROUP_TARGETS["m3"])}
GROUP_SLUGS = dict(zip(
    (*GROUP_TARGETS["m2"], *GROUP_TARGETS["m3"], *GROUP_TARGETS["m5"]),
    ("tank", "mud-mixer", "rest-area", "soul-spa", "wheelbarrow-parking", "sand-pile", "bone-pile", "outdoor-lamp"), strict=True))
GROUP_ARTIFACT_COUNTS = {"Tank": (2, 3), "MudMixer": (2, 3), "RestArea": (1, 3),
                         "SoulSpa": (2, 4), "WheelbarrowParking": (0, 2),
                         "SandPile": (0, 2), "BonePile": (0, 2), "OutdoorLamp": (0, 3)}
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


def validate_group_declaration(value):
    """Independent structural oracle; the coordinator also binds exact manifest bytes."""
    require(isinstance(value, dict) and set(value) == {"group", "fixed", "candidates"}
            and value["group"] in GROUP_TARGETS, "group declaration differs")
    group = value["group"]
    for field, kinds, authority in (("fixed", GROUP_FIXED[group], "release_approved"),
                                     ("candidates", GROUP_TARGETS[group], "isolated_candidate")):
        bindings = value[field]
        require(isinstance(bindings, list) and len(bindings) == len(kinds), "group kind count differs")
        seen = []
        for binding in bindings:
            require(isinstance(binding, dict) and set(binding) == {"identity", "locator", "paths"},
                    "group binding fields differ")
            identity = binding["identity"]
            require(isinstance(identity, dict) and set(identity) == {"kind", "generation", "authority", "manifest_sha256"}
                    and identity["kind"] in kinds and identity["authority"] == authority
                    and type(identity["generation"]) is int and 0 < identity["generation"] < 2**64
                    and isinstance(identity["manifest_sha256"], str)
                    and re.fullmatch(r"[0-9a-f]{64}", identity["manifest_sha256"]), "group identity differs")
            kind = identity["kind"]
            seen.append(kind)
            slug = GROUP_SLUGS[kind]
            require(binding["locator"] == f"manifests/building-{slug}-v1.buildingset", "group locator differs")
            paths = binding["paths"]
            meshes, images = GROUP_ARTIFACT_COUNTS[kind]
            require(isinstance(paths, list) and len(paths) == meshes + images
                    and all(isinstance(p, str) for p in paths) and len(set(paths)) == len(paths),
                    "group artifact path inventory differs")
            for path in paths:
                require(len(path.encode()) <= 512 and path.startswith(f"building_sets/{slug}/{identity['generation']}/")
                        and not any(c in path for c in ("\\", "#", ":"))
                        and not any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in path)
                        and all(part not in {"", ".", ".."} for part in path.split("/"))
                        and not PurePosixPath(path).is_absolute(), "group artifact path differs")
            require(sum(p.endswith(".glb") for p in paths) == meshes
                    and sum(p.endswith(".png") for p in paths) == images, "group artifact types differ")
        require(set(seen) == set(kinds) and len(set(seen)) == len(seen), "group kinds differ")
    return value


def validate_group_sidecar(value, case):
    return _validate_sidecar(value, case, pre_release=True, group=True)


def _validate_sidecar(value, case, *, pre_release, group=False):
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
        and value["contract_id"] == (GROUP_CONTRACT if group else PRE_RELEASE_CONTRACT if pre_release else CONTRACT)
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
        == ({
            "mode",
            "identities",
            "door_identity",
            "target_mesh_entities",
            "pool_active",
            "pool_pending",
            "resources",
            "gpu_allocation_bytes",
            "active_state_acceptance",
        } | ({"group", "loaded_identities", "target_path_observations"} if group else set())),
        "M6 fields differ",
    )
    require(m6["mode"] in {"legacy-control", "candidate"}, "invalid M6 mode")
    candidate = m6["mode"] == "candidate"
    if group:
        declaration = validate_group_declaration(m6["group"])
        all_bindings = [*declaration["fixed"], *declaration["candidates"]]
        loaded = [*declaration["fixed"], *(declaration["candidates"] if candidate else [])]
        require(canonical(m6["identities"]) == canonical([b["identity"] for b in all_bindings])
                and canonical(m6["loaded_identities"]) == canonical([b["identity"] for b in loaded]),
                "group loaded identities differ")
        expected_paths = [] if candidate else [{"path": path, "managed": False}
            for binding in declaration["candidates"] for path in (binding["locator"], *binding["paths"])]
        require(canonical(m6["target_path_observations"]) == canonical(expected_paths),
                "group control prefetched or omitted target paths")
        loaded_kinds = {b["identity"]["kind"] for b in loaded}
    else:
        identities(m6["identities"], pre_release=pre_release)
        loaded_kinds = set(SETS) if candidate else set()
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
        "target_active_unique_meshes": 1 + int(any(k not in loaded_kinds for k in PARTS))
            + sum(len(set(PARTS.get(k, []))) for k in loaded_kinds),
        "souls": copies // 4 * 15,
        "completion_effects": 0,
    }
    for key, expected in expected_counts.items():
        require(
            type(initial[key]) is int and initial[key] == expected, f"incorrect {key}"
        )
    for key, expected in {
        "target_mesh_entities": copies * (5 + int("Tank" in loaded_kinds)
            + int("MudMixer" in loaded_kinds) + 4 * int("SoulSpa" in loaded_kinds)),
        "pool_active": len(loaded_kinds),
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
        present = logical["kind"] in loaded_kinds
        extras = {"parts"} if present else set()
        if present and logical["kind"] in foreground.KINDS:
            extras.update({"owner", "foreground"})
        if present and logical["kind"] in {"Tank", "MudMixer"}:
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
