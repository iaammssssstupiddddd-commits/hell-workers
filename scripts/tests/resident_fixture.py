"""Synthetic resident relationships, never native or asset-approval evidence."""
import copy

from scripts import building_resident_observation as resident


def bind(row, identity, index):
    kind = row["kind"]
    if kind not in resident.ROLES:
        return
    owner = row.setdefault("owner", 50000 + index * 16)
    root = 50001 + index * 16
    names = list(resident.ROLES[kind])
    meshes = {role: "synthetic-mesh-" + kind + role for role in set(resident.ROLES[kind].values())}
    images = {role: "synthetic-image-" + kind + role for role in
              ("albedo", "world_preview", "catalog", *(("slot_emissive",) if kind == "SoulSpa" else ()))}
    for i, part in enumerate(row["parts"]):
        name = names[i]
        active = kind == "SoulSpa" and name != "body" and bool(row["state"]["mask"] & (1 << int(name[-1])))
        geometry = row.get("gallery_geometry", [])[i] if "gallery_geometry" in row else None
        leaf = geometry["entity"] if geometry else root + i + 1
        if geometry:
            geometry["mesh"] = meshes[part["mesh_role"]]
        part["resident"] = {
            "entity": leaf, "parent": root, "root": root, "owner": owner,
            "root_identity": copy.deepcopy(identity), "name": name,
            "root_pose": {"translation": [row["center"][0], 0., -row["center"][1]],
                          "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]},
            "mesh": meshes[part["mesh_role"]], "material": "synthetic-material-" + str(leaf),
            "texture": images["albedo"], "meshes": copy.deepcopy(meshes), "images": copy.deepcopy(images),
            "body_paint": {"mode": .82 if kind == "Tank" else 0., "canvas": [0., 0., 0., 0.],
                           "matte": [0., 0., 0., 0.], "standard": True, "ink_base": True},
            "spa": {"slot_active": active, "emissive_white": active, "emissive_black": not active,
                    "texture": images["slot_emissive"] if active else None} if kind == "SoulSpa" else None,
        }
