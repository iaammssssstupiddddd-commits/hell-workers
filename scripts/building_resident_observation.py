"""Independent M6 resident-part facts; no admission or art authority."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

if __package__:
    from . import building_body_paint_observation as paint
else:
    import building_body_paint_observation as paint

require = paint.require
ROLES = {
    "Tank": {"body": "body", "water": "water"},
    "MudMixer": {"body": "body", "rotor": "rotor"},
    "RestArea": {"body": "body"},
    "SoulSpa": {"body": "body", **{f"slot{i}": "slot" for i in range(4)}},
}
FIELDS = {"entity", "parent", "root", "owner", "root_identity", "name", "root_pose",
          "mesh", "material", "texture", "meshes", "images", "body_paint", "spa"}


def dependencies():
    return {"resident": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "body_paint": paint.verifier_sha256()}


def exact(left, right):
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(right, sort_keys=True, allow_nan=False)


def entity(value):
    require(type(value) is int and 0 <= value < 2**64, "resident entity differs")
    return value


def handle(value):
    require(isinstance(value, str) and 0 < len(value) <= 512, "resident handle missing")
    return value


def vector(value, count):
    require(isinstance(value, list) and len(value) == count
            and all(type(v) in (float, int) and math.isfinite(v) for v in value), "resident vector differs")
    return value


def near(left, right):
    return all(abs(a-b) <= 1e-5 for a, b in zip(left, right, strict=True))


def validate_shape(record, identity):
    """Close the raw schema and actual owner/root/part/handle relationships."""
    kind = record["kind"]
    require(kind in ROLES and identity["kind"] == kind, "resident structural kind differs")
    owner = entity(record.get("owner"))
    if "gallery_owner" in record:
        require(owner == record["gallery_owner"], "resident gallery owner differs")
    parts = record["parts"]
    require(len(parts) == len(ROLES[kind]), "resident part count differs")
    names, leaves, roots, root_poses, descriptors = [], [], set(), [], []
    for part in parts:
        value = part.get("resident")
        require(isinstance(value, dict) and set(value) == FIELDS, "resident fields missing or unknown")
        name = value["name"]
        require(isinstance(name, str) and name in ROLES[kind] and part["mesh_role"] == ROLES[kind][name], "resident actual part name/role differs")
        names.append(name)
        leaf, root = entity(value["entity"]), entity(value["root"])
        require(entity(value["parent"]) == root and entity(value["owner"]) == owner
                and owner != root and leaf not in (root, owner), "resident parent/root/owner differs")
        require(exact(value["root_identity"], identity), "resident observed root identity differs")
        roots.add(root)
        leaves.append(leaf)
        pose = value["root_pose"]
        require(isinstance(pose, dict) and set(pose) == {"translation", "rotation", "scale"}, "resident root pose missing")
        for key, count in (("translation", 3), ("rotation", 4), ("scale", 3)):
            vector(pose[key], count)
        require(near(pose["translation"], [record["center"][0], 0., -record["center"][1]])
                and near(pose["rotation"], [0., 0., 0., 1.]) and near(pose["scale"], [1., 1., 1.]),
                "resident root is detached from the logical owner")
        if "root_translation_wu" in record:
            require(near(pose["translation"], vector(record["root_translation_wu"], 3)), "resident root checkpoint differs")
        root_poses.append(pose)
        meshes, images = value["meshes"], value["images"]
        image_roles = {"albedo", "world_preview", "catalog"} | ({"slot_emissive"} if kind == "SoulSpa" else set())
        require(isinstance(meshes, dict) and set(meshes) == set(ROLES[kind].values())
                and isinstance(images, dict) and set(images) == image_roles, "resident descriptor roles differ")
        descriptors.append({"meshes": meshes, "images": images})
        for handles in (meshes, images):
            for item in handles.values():
                handle(item)
        require(handle(value["mesh"]) == meshes[part["mesh_role"]], "resident actual mesh differs")
        handle(value["material"])
        handle(value["texture"])
        facts = value["body_paint"]
        require(isinstance(facts, dict) and set(facts) == {"mode", "canvas", "matte", "standard", "ink_base"}
                and type(facts["mode"]) in (int, float) and math.isfinite(facts["mode"])
                and type(facts["standard"]) is bool and type(facts["ink_base"]) is bool, "resident paint facts missing")
        vector(facts["canvas"], 4)
        vector(facts["matte"], 4)
        spa = value["spa"]
        if kind == "SoulSpa":
            require(isinstance(spa, dict) and set(spa) == {"slot_active", "emissive_white", "emissive_black", "texture"}
                    and all(type(spa[k]) is bool for k in ("slot_active", "emissive_white", "emissive_black")),
                    "resident Spa emission facts missing")
            mask = record["state"]["mask"]
            require(type(mask) is int and 0 <= mask < 16, "resident Spa mask differs")
            active = name != "body" and bool(mask & (1 << int(name[-1])))
            require(spa["slot_active"] is active and spa["emissive_white"] is active
                    and spa["emissive_black"] is (not active)
                    and spa["texture"] == (images["slot_emissive"] if active else None), "resident Spa worker/emission differs")
        else:
            require(spa is None, "resident non-Spa emission facts forbidden")
        if "gallery_geometry" in record:
            geometry = [p for p in record["gallery_geometry"] if p["entity"] == leaf]
            require(len(geometry) == 1 and geometry[0]["mesh"] == value["mesh"]
                    and geometry[0]["mesh_role"] == part["mesh_role"], "resident gallery geometry belongs to another part")
    require(len(set(names)) == len(names) and set(names) == set(ROLES[kind])
            and len(set(leaves)) == len(leaves) and len(roots) == 1
            and all(exact(root_poses[0], pose) for pose in root_poses)
            and all(exact(descriptors[0], descriptor) for descriptor in descriptors), "resident parts duplicate or share foreign roots")
    return [owner, *roots, *leaves]


def validate_record(manifest, record, expected_parts):
    """Derive paint/part expectations from the frozen codec-verified manifest."""
    ids = validate_shape(record, manifest["identity"])
    declared = {part["name"]: part for part in expected_parts}
    require(len(declared) == len(expected_parts), "resident frozen names duplicate")
    contract = {"identity": manifest["identity"], **paint.fields(manifest)}
    for part in record["parts"]:
        actual = part["resident"]
        require(actual["name"] in declared, "resident name absent from frozen manifest")
        expected = declared[actual["name"]]
        require(part["mesh_role"] == expected["mesh_role"], "resident frozen mesh role differs")
        for key, count in (("translation_wu", 3), ("rotation_xyzw", 4), ("scale", 3)):
            require(vector(part[key], count) == vector(expected[key], count), "resident frozen named part pose differs")
        texture = paint.expected_texture(contract, actual["name"], actual["images"], actual["body_paint"])
        require(actual["texture"] == texture, "resident texture differs from frozen paint contract")
    return ids
