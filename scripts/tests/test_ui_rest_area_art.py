"""Synthetic feedback predicates, never independent art or release decisions."""
import copy
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

from scripts.tests import test_ui_rest_area_save as saved
from scripts.tests import test_ui_rest_area_completion as completed

art = completed.acceptance.rest_area_art


class OrdinaryArtTests(unittest.TestCase):
    def test_uv_ink_requires_actual_mode_and_atlas_through_save_and_load(self):
        raw, entries, client, payload, contract = self.fixture()
        contract["surface_paint_contract"] = {"method": "uv_ink_v1"}
        for sample in raw["samples"]:
            for root in sample["art_feedback"]["roots"]:
                for part in root["parts"]:
                    part["body_paint"] = {"mode": 4., "ink_base": True}
        self.verify(raw, entries, client, payload, contract)
        for attack in ("mode", "ink_base", "missing", "foreign-atlas", "undeclared"):
            changed, adopted = copy.deepcopy((raw, contract))
            if attack == "undeclared":
                del adopted["surface_paint_contract"]
            for sample in changed["samples"]:
                for root in sample["art_feedback"]["roots"]:
                    for part in root["parts"]:
                        if attack == "mode":
                            part["body_paint"]["mode"] = 0.
                        elif attack == "ink_base":
                            part["body_paint"]["ink_base"] = False
                        elif attack == "missing":
                            del part["body_paint"]
                        elif attack == "foreign-atlas":
                            part["albedo"] = "foreign"
            with self.subTest(attack=attack), self.assertRaises((RuntimeError, ValueError)):
                self.verify(changed, entries, client, payload, adopted)

    def test_candidate_lifecycle_has_separate_explicit_scope_and_authority(self):
        raw, entries, client, payload, contract = self.fixture()
        contract["scope"] = raw["scope"] = art.CANDIDATE_SCOPE
        contract["identity"]["authority"] = "isolated_candidate"
        self.verify(raw, entries, client, payload, contract)
        before = copy.deepcopy((raw, contract))
        for field, value in (("scope", art.SCOPE), ("scope", "production-raw-observations-v1"),
                             ("authority", "art_preview"), ("authority", "release_approved"),
                             ("kind", "SoulSpa")):
            changed_raw, changed_contract = copy.deepcopy(before)
            if field == "scope":
                changed_raw["scope"] = value
            else:
                changed_contract["identity"][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                self.verify(changed_raw, entries, client, payload, changed_contract)
        driver = object()
        with patch.object(art.usage, "exercise_lifecycle") as exercise:
            art.exercise_lifecycle(driver, candidate=True)
            exercise.assert_called_once_with(driver, raw_scope=art.CANDIDATE_SCOPE)
        setting = art.session_setting(Path("/owned"), "b" * 32, contract)
        self.assertEqual(art.json.loads(setting)["mode"], "candidate")

    def test_candidate_source_requires_packaged_approval_and_exact_profile(self):
        manifest = {"identity": {"authority": "isolated_candidate"}, "artifacts": [], "parts": [],
                    "source_sha256": "s", "geometry_contract_sha256": "g"}
        with (patch.object(art.pipeline, "no_symlinks", side_effect=lambda path: path),
              patch.object(art.pipeline, "verify_set") as verify,
              patch.object(art.pipeline, "load_set", return_value=(manifest, "manifest", None)),
              patch.object(Path, "read_bytes", return_value=b"codec"),
              patch.object(art.art, "asset_view_fingerprint", return_value="base")):
            contract = art.source_contract(Path("/repo"), Path("/candidate"), Path("/codec"), candidate=True)
            self.assertEqual(contract["scope"], art.CANDIDATE_SCOPE)
            verify.assert_called_once_with(Path("/candidate"), "RestArea", Path("/codec"))
            with self.assertRaises(RuntimeError):
                art.source_contract(Path("/repo"), Path("/candidate"), Path("/codec"))
            verify.side_effect = RuntimeError("numeric approval missing or altered")
            with self.assertRaisesRegex(RuntimeError, "numeric approval"):
                art.source_contract(Path("/repo"), Path("/candidate"), Path("/codec"), candidate=True)

    def test_candidate_profile_cannot_silently_use_fallback_or_preview_mode(self):
        args = SimpleNamespace(rest_area_art_source="/candidate", art_codec="/codec", art_contract_sha256=None,
                               case="rest-area-candidate-lifecycle", smoke=True, input_backend="portal", layout_menu=False)
        with patch.object(art, "source_contract", return_value={"scope": art.CANDIDATE_SCOPE}) as source:
            self.assertEqual(art.planned_contract(args, Path("/repo")), {"scope": art.CANDIDATE_SCOPE})
            source.assert_called_once_with(Path("/repo"), Path("/candidate"), Path("/codec"), candidate=True)
        args.rest_area_art_source = args.art_codec = None
        with self.assertRaises(RuntimeError):
            art.planned_contract(args, Path("/repo"))

    def test_arrival_wait_is_bounded_and_requires_a_real_occupant(self):
        driver = MagicMock()
        state = {"frame": 10, "rest_area_planning": {"buildings": [99]}}
        driver.last = state
        driver.checkpoints = [{"value": state}]
        row = {"entity": "soul", "dream": 2.5}
        waits = []

        def wait(predicate, **kwargs):
            waits.append(kwargs)
            if len(waits) == 1:
                with patch.object(art.usage, "candidates", return_value=[]):
                    self.assertFalse(predicate(state))
                with patch.object(art.usage, "candidates", return_value=[({}, {**row, "dream": 1.9})]):
                    self.assertFalse(predicate(state))
                self.assertTrue(predicate(state))
                return state
            raise RuntimeError("stop after arrival")

        driver.wait.side_effect = wait
        with (patch.object(art.usage.planning, "exercise"),
              patch.object(art.usage.planning, "set_refund_paused"),
              patch.object(art.usage, "context", return_value={}),
              patch.object(art.usage, "candidates", return_value=[({}, row)]),
              patch.object(art.usage, "wait_for_raw_checkpoint")):
            with self.assertRaisesRegex(RuntimeError, "stop after arrival"):
                art.usage.exercise(driver)
        self.assertEqual(waits[0], {"after": 10, "timeout": 240})
        self.assertEqual(waits[1]["timeout"], 15)

    def test_live_art_lifecycle_forwards_scope_to_save_without_changing_formal_default(self):
        driver = object()
        with patch.object(art.usage, "exercise_save", side_effect=RuntimeError("stop before further input")) as save:
            with self.assertRaisesRegex(RuntimeError, "stop before further input"):
                art.exercise_lifecycle(driver)
            save.assert_called_once_with(driver, raw_scope=art.SCOPE)
            save.reset_mock()
            with self.assertRaisesRegex(RuntimeError, "stop before further input"):
                art.usage.exercise_lifecycle(driver)
            save.assert_called_once_with(driver, raw_scope="production-raw-observations-v1")

    def test_live_save_checker_receives_only_explicit_scope_not_raw_self_declaration(self):
        for explicit in (False, True):
            driver = MagicMock()
            driver.root = Path("/unused-owned-job")
            state = {"frame": 10, "world_epoch": 0, "nonce": "owned", "pid": 123}
            driver.last = state
            calls = []

            def wait(predicate, **kwargs):
                calls.append(kwargs)
                if len(calls) == 5:
                    self.assertTrue(predicate(state))
                return state

            driver.wait.side_effect = wait
            raw = {"nonce": "owned", "process_id": 123, "failure": None,
                   "scope": art.SCOPE, "events": [{"operation": "DurableLoad"}]}
            with (patch.object(art.usage, "exercise"),
                  patch.object(art.usage.planning, "catalog_control", return_value="owned-control"),
                  patch.object(art.usage.planning, "set_refund_paused"),
                  patch.object(Path, "is_file", return_value=True),
                  patch.object(art.usage.native, "read_json", return_value=raw),
                  patch.object(art.usage, "check_save", return_value="loaded-soul") as checker):
                result = art.usage.exercise_save(driver, **({"raw_scope": art.SCOPE} if explicit else {}))
            self.assertEqual(result, "loaded-soul")
            self.assertEqual(checker.call_count, 1)
            self.assertEqual(checker.call_args.kwargs,
                             {"raw_scope": art.SCOPE if explicit else "production-raw-observations-v1"})

    def fixture(self):
        raw, entries, client, payload = saved.OrdinaryRestSaveTests().lifecycle_fixture()
        identity = {"kind": "RestArea", "authority": "art_preview", "generation": 1, "manifest_sha256": "a" * 64}
        part = {"name": "body", "mesh_role": "body", "material_role": "opaque_albedo",
                "translation_wu": [0, 0, 0], "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}
        contract = {"scope": art.SCOPE, "identity": identity, "parts": [part]}
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

    def test_explicit_eis_region_is_validated_without_relative_fallback(self):
        client = {"input_backend": "portal-eis", "nonce": "owned", "portal_devices": 3,
                  "monitor_mapping": [1, 2.0], "input_transport": "portal-eis-absolute-v1",
                  "eis_region": {"rect": [0, 0, 1440, 900], "scale": 2.0, "root_size": [2880, 1800]}}
        manifest = {"input_backend": "portal-eis", "case": "rest-area-lifecycle"}
        completed.acceptance.check_input_client(manifest, {"nonce": "owned"}, client)
        for field, value in (("eis_region", None), ("input_transport", "portal-notify-relative-v1"),
                             ("monitor_mapping", [1, 1.0]), ("input_backend", "portal"), ("portal_devices", 1)):
            invalid = copy.deepcopy(client)
            invalid[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                completed.acceptance.check_input_client(manifest, {"nonce": "owned"}, invalid)
        for field, value in (("rect", [0, 0, 2880, 1800]), ("rect", [True, 0, 1440, 900]),
                             ("scale", float("nan")), ("root_size", [True, 1800])):
            invalid = copy.deepcopy(client)
            invalid["eis_region"][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(RuntimeError):
                art.check_eis_client(invalid)
        from scripts.perf_tool import execution, renderdoc_capture
        for inventory in (art.native.NATIVE_HARNESS_FILES, execution.MEASUREMENT_HARNESS_FILES,
                          renderdoc_capture.MEASUREMENT_HARNESS_FILES):
            self.assertEqual(inventory.count("scripts/native_ui_eis.py"), 1)


if __name__ == "__main__":
    unittest.main()
