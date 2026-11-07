"""Synthetic feedback predicates, never independent art or release decisions."""
import copy
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from scripts.tests import test_ui_rest_area_save as saved
from scripts.tests import test_ui_rest_area_completion as completed

art = completed.acceptance.rest_area_art


class OrdinaryArtTests(unittest.TestCase):
    def fixture(self):
        raw, entries, client, payload = saved.OrdinaryRestSaveTests().lifecycle_fixture()
        identity = {"kind": "RestArea", "authority": "art_preview", "generation": 1, "manifest_sha256": "a" * 64}
        part = {"name": "body", "mesh_role": "body", "material_role": "opaque_albedo",
                "translation_wu": [0, 0, 0], "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}
        contract = {"identity": identity, "parts": [part]}
        raw["scope"] = art.SCOPE
        completed_context = copy.deepcopy(raw["samples"][7]["rest_area_context"])
        completed_context.update(real_seconds=6.0, virtual_seconds=6.0)
        raw["samples"][6].update(rest_area_context=completed_context, paused=True, virtual_seconds=6.0)
        entries[2]["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(completed_context)
        for sample in raw["samples"]:
            if "rest_area_context" in sample:
                sample["owners"] = [{"entity": owner["entity"], "entity_bits": owner["entity_bits"],
                                     "kind": "RestArea", "provisional": False, "position": [20.0, 20.0, 0.0]}
                                    for owner in sample["rest_area_context"]["owners"]]
            roots = [{"entity": "root-" + owner["entity"], "owner": owner["entity"], "identity": identity, "owner_pose_matches": True,
                      "parts": [{"entity": "part-" + owner["entity"], "name": "body", "mesh": "mesh-body",
                                 "parent": "root-" + owner["entity"], "material": "material-body", "albedo": "image-albedo",
                                 "visible": True, "pose": {"translation": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1]},
                                 "material_values": {"white": True, "opaque": True, "lit": True, "roughness": 1.0,
                                                     "reflectance": 0.0, "emissive_black": True, "emissive_texture_absent": True}}]}
                     for owner in sample.get("owners", []) if owner.get("kind") == "RestArea"]
            sample["art_feedback"] = {"requested": identity, "active": identity, "pending": None,
                                      "descriptor": {"identity": identity, "body_mesh": "mesh-body", "albedo": "image-albedo",
                                                     "world_preview": "preview", "catalog": "catalog", "part_names": ["body"], "parts": [part]},
                                      "roots": roots}
        return raw, entries, client, payload, contract

    def verify(self, raw, entries, client, payload, contract):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "saves" / "manual-1.scn.ron"
            path.parent.mkdir()
            path.write_bytes(payload)
            art.check_lifecycle(raw, entries, client, path, contract)

    def test_exact_preview_lifecycle_is_not_accepted_by_formal_raw_validator(self):
        fixture = self.fixture()
        before = copy.deepcopy(fixture)
        self.verify(*fixture)
        self.assertEqual(fixture, before)
        with self.assertRaises(RuntimeError):
            saved.OrdinaryRestSaveTests().verify_lifecycle(*fixture[:4])

    def test_missing_wrong_generation_fallback_hidden_or_changed_parts_are_rejected(self):
        for mode in ("active", "pending", "descriptor", "root", "owner", "root_pose", "mesh", "texture", "material", "visible",
                     "parent", "translation", "rotation", "scale", "white", "opaque", "lit", "roughness", "reflectance",
                     "emissive_black", "emissive_texture_absent", "part_names", "alias", "scope", "authority"):
            raw, entries, client, payload, contract = copy.deepcopy(self.fixture())
            if mode == "scope":
                raw["scope"] = "production-raw-observations-v1"
            elif mode == "authority":
                raw["accepted"] = True
            else:
                for sample in raw["samples"]:
                    feedback = sample["art_feedback"]
                    if mode == "active":
                        feedback["active"] = None
                    elif mode == "pending":
                        feedback["pending"] = contract["identity"]
                    elif mode == "descriptor":
                        feedback["descriptor"] = None
                    elif mode == "part_names":
                        feedback["descriptor"]["part_names"] = []
                    elif mode == "alias":
                        feedback["active"] = {**feedback["active"], "generation": True}
                    for root in feedback["roots"]:
                        if mode == "root":
                            root["identity"] = None
                        elif mode == "root_pose":
                            root["owner_pose_matches"] = False
                        elif mode == "owner" and sample["world_epoch"] == 1:
                            root["owner"] = "pre-load-owner"
                        for part in root["parts"]:
                            if mode in ("mesh", "material", "parent"):
                                part[mode] = None
                            elif mode == "texture":
                                part["albedo"] = "foreign"
                            elif mode == "visible":
                                part["visible"] = False
                            elif mode in ("translation", "rotation", "scale"):
                                part["pose"][mode][0] = float("nan")
                            elif mode in part["material_values"]:
                                part["material_values"][mode] = False if mode not in ("roughness", "reflectance") else 0.25
            with self.subTest(mode=mode), self.assertRaises((RuntimeError, ValueError)):
                self.verify(raw, entries, client, payload, contract)

    def test_exact_owner_inventory_requires_a_real_root(self):
        raw, entries, client, payload, contract = self.fixture()
        for sample in raw["samples"]:
            sample["art_feedback"]["roots"] = []
        with self.assertRaises(RuntimeError):
            self.verify(raw, entries, client, payload, contract)

    def test_explicit_bounded_profile_and_frozen_contract_are_required(self):
        args = SimpleNamespace(rest_area_art_source="/source", art_codec="/codec", art_contract_sha256=None,
                               case="rest-area-lifecycle", smoke=True, input_backend="portal", layout_menu=False)
        with patch.object(art, "source_contract", return_value={"scope": art.SCOPE}):
            self.assertEqual(art.planned_contract(args, Path("/repo")), {"scope": art.SCOPE})
            for field, value in (("case", "navigation"), ("smoke", False), ("input_backend", "none"),
                                 ("layout_menu", True), ("art_codec", None), ("art_contract_sha256", "foreign")):
                changed = copy.copy(args)
                setattr(changed, field, value)
                with self.subTest(field=field), self.assertRaises(RuntimeError):
                    art.planned_contract(changed, Path("/repo"))

    def test_asset_view_preserves_base_files_and_only_the_declared_overlay(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            repo = root / "repo"
            view = repo / "target/native-acceptance/batch/viewport-0/asset-view"
            for directory in (repo / "assets", view / "assets/manifests"):
                directory.mkdir(parents=True)
            (repo / "assets/font").write_bytes(b"font")
            (view / "assets/font").write_bytes(b"font")
            (view / "assets/manifests/building-rest-area-v1.buildingset").write_bytes(b"manifest")
            contract = {"artifacts": [], "manifest_sha256": art.pipeline.digest(b"manifest")}
            art.check_view(repo, view, contract)
            for mutation in ("changed", "extra", "symlink"):
                path = view / "assets/font"
                if mutation == "changed":
                    path.write_bytes(b"foreign")
                elif mutation == "extra":
                    (view / "assets/extra").write_bytes(b"extra")
                else:
                    path.unlink()
                    path.symlink_to(repo / "assets/font")
                with self.subTest(mutation=mutation), self.assertRaises(RuntimeError):
                    art.check_view(repo, view, contract)
                if path.is_symlink():
                    path.unlink()
                path.write_bytes(b"font")
                (view / "assets/extra").unlink(missing_ok=True)

    def test_same_bytes_through_a_view_assets_or_ancestor_symlink_are_rejected(self):
        for component in ("view", "assets", "viewport"):
            with self.subTest(component=component), tempfile.TemporaryDirectory() as temporary:
                repo = Path(temporary) / "repo"
                view = repo / "target/native-acceptance/batch/viewport-0/asset-view"
                (repo / "assets").mkdir(parents=True)
                (view / "assets/manifests").mkdir(parents=True)
                locator = "manifests/building-rest-area-v1.buildingset"
                (view / "assets" / locator).write_bytes(b"manifest")
                contract = {"artifacts": [], "manifest_sha256": art.pipeline.digest(b"manifest")}
                art.check_view(repo, view, contract)
                target = view if component == "view" else view / "assets" if component == "assets" else view.parent
                preserved = Path(temporary) / "same-bytes"
                target.rename(preserved)
                target.symlink_to(preserved, target_is_directory=True)
                # Prove byte inventory alone still passes this same-bytes
                # substitution; rejection must come from the root boundary.
                with patch.object(art, "checked_view", return_value=view):
                    art.check_view(repo, view, contract)
                with self.assertRaises((RuntimeError, ValueError)):
                    art.check_view(repo, view, contract)
                if component != "assets":
                    with self.assertRaises((RuntimeError, ValueError)):
                        art.checked_view(repo, view, prepared=False)

    def test_new_helper_is_in_all_three_bound_harness_inventories(self):
        from scripts.perf_tool import execution, renderdoc_capture
        relative = ".codex/skills/hell-workers-run-native-acceptance/scripts/ui_rest_area_art.py"
        for inventory in (art.native.NATIVE_HARNESS_FILES, execution.MEASUREMENT_HARNESS_FILES,
                          renderdoc_capture.MEASUREMENT_HARNESS_FILES):
            self.assertEqual(inventory.count(relative), 1)


if __name__ == "__main__":
    unittest.main()
