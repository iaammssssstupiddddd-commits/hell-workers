"""Exact seeded Spa pages are technical evidence, never lifecycle/release authority."""
from __future__ import annotations

import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import building_art_acceptance as art


def fixture(page):
    identity = {"kind": "SoulSpa", "generation": 2, "authority": "isolated_candidate", "manifest_sha256": "a" * 64}
    parts = [{"name": name, "mesh_role": "body" if name == "body" else "slot",
              "material_role": "opaque_albedo" if name == "body" else "spa_slot",
              "translation_wu": [0., 0., 0.], "rotation_xyzw": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
             for name in ("body", "slot0", "slot1", "slot2", "slot3")]
    value = {"kind": "SoulSpa", "mode": "candidate", "identity": identity, "scope": art.SPA_PAGE_SCOPE,
             "spa_mask_page": page, "promotion_authority": False, "view": "normal", "spa_parts": parts}
    records = []
    for kind in ("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking", "SandPile", "BonePile", "Door", "OutdoorLamp"):
        for ordinal in range(4):
            record = {"kind": kind, "ordinal": ordinal}
            if kind == "SoulSpa":
                mask = page * 4 + ordinal
                record.update(center=[32., 64.], root_identity=identity, root_translation_wu=[32., 0., -64.],
                              owner=ordinal * 10, root=ordinal * 10 + 1,
                              root_rotation_xyzw=[0., 0., 0., 1.], root_scale=[1., 1., 1.],
                              state={"mask": mask, "operational": True}, parts=[])
                for index, part in enumerate(parts):
                    active = part["name"] != "body" and bool(mask & (1 << int(part["name"][-1])))
                    record["parts"].append({**copy.deepcopy(part), "entity": ordinal * 10 + index + 2,
                        "parent": ordinal * 10 + 1, "visible": True, "slot_active": active,
                        "emissive_white": active, "emissive_black": not active, "emissive_texture_matches": True,
                        "white_opaque_lit": True, "roughness": 1., "metallic": 0., "reflectance": 0.,
                        "material_values": "actual material observation"})
            records.append(record)
    probe = {"profile": art.PROFILE, "status": "ready", "mode": "candidate", "identity": identity,
             "nonce": "a" * 32, "scope": art.SPA_PAGE_SCOPE, "stable_frames": 30, "spa_mask_page": page,
             "view": "normal", "camera_scale": 1., "fixture": {"spa_mask_page": page,
             "scope": art.SPA_PAGE_SCOPE, "souls": art.actor_count(value), "target_count": 36,
             "view": "normal", "camera_scale": 1., "records": records}}
    return value, probe


class SpaPageTests(unittest.TestCase):
    def test_four_pages_cover_every_mask_and_exact_population(self):
        masks = []
        for page, population in enumerate((12, 16, 16, 20)):
            value, probe = fixture(page)
            art.validate_probe(probe, value, probe["nonce"])
            command = art.game_command(Path("binary"), Path("job"), value)
            self.assertEqual(command[command.index("--spawn-souls") + 1], str(population))
            masks.extend(r["state"]["mask"] for r in probe["fixture"]["records"] if r["kind"] == "SoulSpa")
            env = art.game_environment({}, Path("view"), {**value, "adapter": "Intel"}, "nonce", Path("job"))
            import json
            self.assertEqual(json.loads(env["HW_BUILDING_ART_SESSION"])["spa_mask_page"], page)
        self.assertEqual(masks, list(range(16)))
        self.assertEqual(art.actor_count({}), 15)

    def test_wrong_scope_authority_and_unbounded_pages_are_rejected(self):
        value, _ = fixture(0)
        for key, change in (("spa_mask_page", True), ("spa_mask_page", 1.0), ("spa_mask_page", -1),
                            ("spa_mask_page", 4), ("kind", "RestArea"), ("mode", "feedback"),
                            ("view", "overview"), ("lighting", "dim"), ("promotion_authority", True),
                            ("variant", "rest-area-art-context"), ("scope", "paused-static-presentation"),
                            ("identity", {"authority": "art_preview"})):
            with self.subTest(key=key, change=change), self.assertRaises(ValueError):
                art.spa_page({**value, key: change})

    def test_raw_mask_parts_materials_root_and_inventory_fail_closed(self):
        value, original = fixture(1)
        spa_index = next(i for i, r in enumerate(original["fixture"]["records"]) if r["kind"] == "SoulSpa")
        root = ("fixture", "records", spa_index)
        part = (*root, "parts", 1)
        mutations = [((), "spa_mask_page", True), (("fixture",), "souls", 15),
            (("fixture",), "scope", "paused-static-presentation"),
            (("fixture", "records", 0), "kind", "Bridge"),
            (("fixture", "records", 0), "ordinal", 1),
            (root, "root_identity", {**value["identity"], "generation": 1}),
            (root, "root", 0), (part, "entity", 0), (part, "parent", 31),
            (root, "root_translation_wu", [32., 0., 64.]), (root, "root_scale", [2., 1., 1.]),
            (root, "root_rotation_xyzw", [0., 0., 1., 0.]), (root, "center", [float("nan"), 64.]),
            ((*root, "state"), "mask", 1), ((*root, "state"), "operational", False),
            (part, "name", "slot1"), (part, "mesh_role", "foreign"), (part, "material_role", "opaque_albedo"),
            (part, "visible", False), (part, "slot_active", True), (part, "emissive_white", True),
            (part, "emissive_black", False), (part, "emissive_texture_matches", False),
            (part, "white_opaque_lit", False), (part, "roughness", True), (part, "metallic", .1),
            (part, "reflectance", .5), (part, "material_values", ""),
            (part, "translation_wu", [1., 0., 0.]), (part, "scale", [1., float("inf"), 1.])]
        for path, key, change in mutations:
            bad = copy.deepcopy(original)
            target = bad
            for component in path:
                target = target[component]
            target[key] = change
            with self.subTest(path=path, key=key), self.assertRaises(ValueError):
                art.validate_probe(bad, value, bad["nonce"])


if __name__ == "__main__":
    unittest.main()
