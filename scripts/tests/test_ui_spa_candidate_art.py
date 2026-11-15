"""Synthetic exact-candidate observers; no art/approval/native evidence is created."""
import copy
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

from scripts import ui_spa_candidate_art as art
from scripts.tests import test_ui_spa_occupied_save as fixtures
import ui_usability_acceptance as runner


def contract():
    return {"scope": art.SCOPE, "identity": {"kind": "SoulSpa", "authority": "isolated_candidate",
            "generation": 2, "manifest_sha256": "a" * 64},
            "parts": [{"name": name, "mesh_role": "body" if name == "body" else "slot",
                       "material_role": "opaque_albedo" if name == "body" else "spa_slot",
                       "translation_wu": [0., .6, 0.], "rotation_xyzw": [0., 0., 0., 1.],
                       "scale": [1., 1., 1.]} for name in ("body", "slot0", "slot1", "slot2", "slot3")],
            "relative_tiles": [[0, 0], [1, 0], [0, -1], [1, -1]]}


def attach(sample, value, adopted):
    state = sample["spa_context"]
    if not state["sites"]:
        return
    owner = state["sites"][0]["entity"]
    descriptor = {"identity": adopted["identity"], "parts": art.runtime_parts(adopted["parts"]),
                  **{key: key + "-handle" for key in ("body_mesh", "slot_mesh", "albedo", "slot_emissive", "world_preview", "catalog")}}
    anchor = value["spa_planning"]["anchor"]
    parts = []
    for spec in adopted["parts"]:
        name = spec["name"]
        if name == "body":
            active = False
        else:
            grid = [a + b for a, b in zip(anchor, adopted["relative_tiles"][int(name[-1])], strict=True)]
            tiles = [tile for tile in state["tiles"] if tile["grid"] == grid]
            active = bool(tiles and tiles[0]["workers"])
        parts.append({"entity": owner + name, "name": name, "parent": owner + "-root", "visible": True,
                      "mesh": descriptor["body_mesh" if name == "body" else "slot_mesh"],
                      "material": "material-" + name, "albedo": descriptor["albedo"],
                      "pose": {key: spec[source] for key, source in (("translation", "translation_wu"), ("rotation", "rotation_xyzw"), ("scale", "scale"))},
                      "material_values": {"white": True, "opaque": True, "lit": True,
                                          "roughness": 1., "reflectance": 0.,
                                          "emissive_white": active, "emissive_black": not active,
                                          "emissive_texture": descriptor["slot_emissive"] if active else None,
                                          "emissive_texture_absent": not active}})
    sample["art_feedback"] = {"requested": adopted["identity"], "active": adopted["identity"], "pending": None,
                              "descriptor": descriptor, "roots": [{"owner": owner, "entity": owner + "-root",
                              "identity": adopted["identity"], "owner_pose_matches": True, "parts": parts}]}


class SpaCandidateArtTests(unittest.TestCase):
    def test_uv_ink_requires_all_actual_parts_and_keeps_worker_emission_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = fixtures.campaign_fixture(save)
            raw["scope"] = art.SCOPE
            adopted = contract()
            adopted["surface_paint_contract"] = {"method": "uv_ink_v1"}
            for entry in entries[2:]:
                sample = art.spa.raw_checkpoint(raw, entry["value"])
                attach(sample, entry["value"], adopted)
                for root in sample["art_feedback"]["roots"]:
                    for part in root["parts"]:
                        part["body_paint"] = {"mode": 4., "ink_base": True}
            art.verify(raw, entries, client, save, inputs, adopted)
            for name in ("body", "slot0", "slot1", "slot2", "slot3"):
                for attack in ("mode", "missing", "foreign-atlas", "emission"):
                    changed = copy.deepcopy(raw)
                    for sample in changed["samples"]:
                        for root in sample.get("art_feedback", {}).get("roots", []):
                            for part in root["parts"]:
                                if part["name"] != name:
                                    continue
                                if attack == "mode":
                                    part["body_paint"]["mode"] = 0.
                                elif attack == "missing":
                                    del part["body_paint"]
                                elif attack == "foreign-atlas":
                                    part["albedo"] = "foreign"
                                else:
                                    part["material_values"]["emissive_white"] = not part["material_values"]["emissive_white"]
                    with self.subTest(name=name, attack=attack), self.assertRaises((RuntimeError, ValueError)):
                        art.verify(changed, entries, client, save, inputs, adopted)

    def test_f32_projection_is_exact_not_a_relaxed_descriptor_comparison(self):
        adopted = contract()
        self.assertEqual(art.runtime_parts(adopted["parts"])[1]["translation_wu"][1], .6000000238418579)
        for value in (True, float("nan"), float("inf"), 1e40):
            changed = copy.deepcopy(adopted["parts"])
            changed[1]["translation_wu"][1] = value
            with self.subTest(value=value), self.assertRaises(RuntimeError):
                art.runtime_parts(changed)

    def test_eis_candidate_retains_exact_region_and_no_scroll_client_guards(self):
        manifest = {"case": art.CASE, "input_backend": "portal-eis"}
        session = {"case": art.CASE, "nonce": "owned"}
        client = {"nonce": "owned", "input_backend": "portal-eis", "portal_devices": 3,
                  "monitor_mapping": [1, 2.], "input_transport": "portal-eis-absolute-v1", "eis_scroll": False,
                  "eis_region": {"scale": 2., "root_size": [2880, 1800], "rect": [0, 0, 1440, 900]}}
        runner.check_input_client(manifest, session, client)
        for field, value in (("input_transport", "portal-notify-relative-v1"), ("eis_scroll", True),
                             ("portal_devices", 1), ("monitor_mapping", [1, 1.]), ("nonce", "foreign")):
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                runner.check_input_client(manifest, session, {**client, field: value})

    def test_exact_candidate_uses_the_complete_occupied_campaign_and_rejects_foreign_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = fixtures.campaign_fixture(save)
            raw["scope"] = art.SCOPE
            adopted = contract()
            for entry in entries[2:]:
                sample = art.spa.raw_checkpoint(raw, entry["value"])
                attach(sample, entry["value"], adopted)
            art.verify(raw, entries, client, save, inputs, adopted)
            for key, value in (("translation_wu", [0., .6, 0.]),
                               ("translation_wu", [0., .6000000834465027, 0.]),
                               ("mesh_role", "foreign"), ("material_role", "foreign")):
                changed = copy.deepcopy(raw)
                descriptor = art.spa.raw_checkpoint(changed, entries[-1]["value"])["art_feedback"]["descriptor"]
                descriptor["parts"][1][key] = value
                with self.subTest(key=key, value=value), self.assertRaisesRegex(RuntimeError, "descriptor"):
                    art.verify(changed, entries, client, save, inputs, adopted)
            with self.assertRaises(RuntimeError):
                art.occupied.verify(raw, entries, client, save, inputs)
            changed = copy.deepcopy(raw)
            changed["scope"] = "production-raw-observations-v1"
            with self.assertRaises(RuntimeError):
                art.verify(changed, entries, client, save, inputs, adopted)
            for field, value in (("mesh", "foreign"), ("parent", "old-root"), ("visible", False)):
                changed = copy.deepcopy(raw)
                part = art.spa.raw_checkpoint(changed, entries[-1]["value"])["art_feedback"]["roots"][0]["parts"][1]
                part[field] = value
                with self.subTest(field=field), self.assertRaises(RuntimeError):
                    art.verify(changed, entries, client, save, inputs, adopted)
            changed = copy.deepcopy(raw)
            part = art.spa.raw_checkpoint(changed, entries[-1]["value"])["art_feedback"]["roots"][0]["parts"][1]
            part["material_values"].update(emissive_white=True, emissive_black=False,
                                           emissive_texture="slot_emissive-handle", emissive_texture_absent=False)
            with self.assertRaisesRegex(RuntimeError, "worker/emission"):
                art.verify(changed, entries, client, save, inputs, adopted)
            for entry in entries[2:]:
                for field, value in (("roughness", 0.), ("roughness", True), ("roughness", float("nan")),
                                     ("reflectance", .5), ("reflectance", False), ("reflectance", float("inf"))):
                    changed = copy.deepcopy(raw)
                    values = art.spa.raw_checkpoint(changed, entry["value"])["art_feedback"]["roots"][0]["parts"][0]["material_values"]
                    values[field] = value
                    with self.subTest(case=entry["case"], field=field, value=value), self.assertRaises(RuntimeError):
                        art.verify(changed, entries, client, save, inputs, adopted)

    def test_dedicated_plan_is_required_and_foreign_mode_cannot_borrow_authority(self):
        args = SimpleNamespace(case=art.CASE, soul_spa_art_source="/candidate", art_codec="/codec",
                               rest_area_art_source=None, smoke=True, input_backend="portal", layout_menu=False,
                               art_contract_sha256=None)
        with patch.object(art, "source_contract", return_value=contract()):
            self.assertEqual(art.planned_contract(args, Path("/repo")), contract())
            self.assertEqual(art.planned_contract(SimpleNamespace(**{**vars(args), "input_backend": "portal-eis"}), Path("/repo")), contract())
            for field, value in (("case", "soul-spa-occupied-save"), ("soul_spa_art_source", None),
                                 ("rest_area_art_source", "/foreign"), ("input_backend", "none"),
                                 ("input_backend", "xtest"), ("smoke", False), ("art_contract_sha256", "foreign")):
                changed = SimpleNamespace(**{**vars(args), field: value})
                with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                    art.planned_contract(changed, Path("/repo"))


if __name__ == "__main__":
    unittest.main()
