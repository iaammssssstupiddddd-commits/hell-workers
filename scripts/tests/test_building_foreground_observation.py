"""Fail-closed passive foreground evidence; no game or window is launched."""
from __future__ import annotations

import copy
from pathlib import Path
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import building_foreground_observation as fg
import building_art_acceptance as art


class ForegroundTests(unittest.TestCase):
    def fixture(self, kind="WheelbarrowParking"):
        preview = {"image_role": "world_off" if kind == "OutdoorLamp" else "world",
                   "canvas_px": [256, 256], "canvas_wu": [64., 64.], "anchor_px": [128., 128.]}
        roles = [preview["image_role"]] + (["world_on"] if kind == "OutdoorLamp" else [])
        identity = {"kind": kind, "authority": "art_preview", "generation": 1, "manifest_sha256": "a" * 64}
        manifest = {"identity": identity, "world_preview": preview, "artifacts": [
            {"role": "image:" + role, "path": role + ".png", "sha256": "b" * 64, "bytes": 100} for role in roles]}
        value = {"kind": kind, "identity": identity, "variant": "gallery", "scope": "paused-static-presentation",
                 "promotion_authority": False, "foreground_observation": fg.descriptor(manifest)}
        records = []
        for index in range(4):
            owner_pose = {"translation": [index * 160., 32., .12], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            local_pose = {"translation": [0., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            role = "world_on" if kind == "OutdoorLamp" and index >= 2 else preview["image_role"]
            observation = {"schema": 1, "owner": 100 + index, "entity": 200 + index, "parent": 100 + index,
                           "identity": identity, "descriptor": preview, "image_role": role,
                           "artifact": next(a for a in manifest["artifacts"] if a["role"] == "image:" + role),
                           "image_handle": "Image(asset-123)", "image_matches": True, "color_srgba": [.2, .8, .1, .4],
                           "canvas_wu": [64., 64.], "anchor": [0., 0.], "visible": True, "layers": [0],
                           "owner_pose": owner_pose, "local_pose": local_pose, "global_pose": owner_pose,
                           "global_matrix": fg.matrix(owner_pose), "parent_local_matrix": fg.matrix(owner_pose)}
            records.append({"owner": 100 + index, "kind": kind, "ordinal": index,
                            "center": owner_pose["translation"][:2], "foreground": observation})
        return value, {"identity": identity, "fixture": {"foreground_schema": 1, "records": records}}

    def test_exact_pass_retains_nonwhite_tint_and_old_plans_do_not_gain_evidence(self):
        for kind in fg.KINDS:
            value, probe = self.fixture(kind)
            original = copy.deepcopy(probe)
            fg.validate(probe, value)
            self.assertEqual(probe, original)
        self.assertIsNone(fg.settings({}))
        fg.validate({}, {})

    def test_rejects_unknown_or_foreign_plan(self):
        value, _ = self.fixture()
        for key, changed in (("kind", "RestArea"), ("variant", "rest-area-art-context"),
                             ("scope", "ordinary-world"), ("promotion_authority", True)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                fg.settings({**value, key: changed})
        for schema in (True, 0, 2):
            with self.subTest(schema=schema), self.assertRaises(ValueError):
                fg.settings({**value, "foreground_observation": {**value["foreground_observation"], "schema": schema}})
        with self.assertRaises(ValueError):
            fg.settings({**value, "foreground_observation": {**value["foreground_observation"], "verifier_sha256": "f" * 64}})

    def test_rejects_wrong_owner_image_color_geometry_and_global_transform(self):
        value, probe = self.fixture()
        changes = [("owner", 101), ("entity", 100), ("parent", 999), ("identity", {}),
                   ("schema", True), ("descriptor", {}), ("image_role", "catalog"),
                   ("artifact", {}), ("image_matches", False), ("image_handle", ""),
                   ("canvas_wu", [32., 32.]), ("anchor", [0., .5]), ("visible", False),
                   ("layers", [1]), ("color_srgba", [float("nan"), 0, 0, 1]),
                   ("color_srgba", [0, 0, 0, 2]), ("color_srgba", [True, 0, 0, 1]),
                   ("global_matrix", [0.] * 16), ("parent_local_matrix", [0.] * 16),
                   ("global_pose", {"translation": [1, 2, 3], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1]})]
        for key, changed in changes:
            bad = copy.deepcopy(probe)
            bad["fixture"]["records"][0]["foreground"][key] = changed
            with self.subTest(key=key, changed=changed), self.assertRaises(ValueError):
                fg.validate(bad, value)
        bad = copy.deepcopy(probe)
        bad["fixture"]["records"] = bad["fixture"]["records"][:3]
        with self.assertRaises(ValueError):
            fg.validate(bad, value)
        bad = copy.deepcopy(probe)
        del bad["fixture"]["foreground_schema"]
        with self.assertRaises(ValueError):
            fg.validate(bad, value)

    def test_rotated_scaled_parent_child_composition_is_independent(self):
        parent = {"translation": [10., 20., .12], "rotation": [0., 0., 2**-.5, 2**-.5], "scale": [2., 2., 1.]}
        child = {"translation": [3., 4., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
        result = fg.product(fg.matrix(parent), fg.matrix(child))
        self.assertTrue(fg.close(result[12:15], [2., 26., .12]))

    def test_new_plan_freezes_foreground_descriptor_and_rejects_source_mismatch(self):
        value, _ = self.fixture()
        observation = value["foreground_observation"]
        manifest = {"identity": value["identity"], "world_preview": observation["descriptor"],
                    "artifacts": observation["artifacts"]}
        subject = {"commit": "a" * 40, "source": "b" * 64, "harness": "c" * 64,
                   "assets": "d" * 64, "driver": "e" * 64, "dirty": []}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            codec = root / "codec"
            codec.write_bytes(b"fixture")
            args = SimpleNamespace(repo=root, mode="art-preview", job_root=root / "job",
                                   source_root=root / "source", kind="WheelbarrowParking", codec=codec,
                                   adapter="fixture", output=root / "plan.json")
            with patch.object(art.native, "validate_repo", return_value=root), \
                    patch.object(art, "subject", return_value=subject), \
                    patch.object(art.static, "check_root"), \
                    patch.object(art.pipeline, "load_set", return_value=(manifest, "{}", None)), \
                    patch.object(art.native, "resource_snapshot", return_value={"failures": []}):
                planned = art.plan(args)
                self.assertEqual(planned["foreground_observation"], observation)
                art.check_plan(planned, codec)
                changed = copy.deepcopy(planned)
                changed["foreground_observation"]["artifacts"][0]["sha256"] = "f" * 64
                with self.assertRaisesRegex(ValueError, "foreground source"):
                    art.check_plan(changed, codec)

    def test_world_artifacts_are_exact_unique_and_not_catalog(self):
        value, _ = self.fixture()
        observation = value["foreground_observation"]
        manifest = {"identity": value["identity"], "world_preview": observation["descriptor"],
                    "artifacts": observation["artifacts"]}
        for artifacts in ([], observation["artifacts"] * 2,
                          [{**observation["artifacts"][0], "role": "image:catalog"}]):
            with self.subTest(artifacts=artifacts), self.assertRaises(ValueError):
                fg.descriptor({**manifest, "artifacts": artifacts})

    def test_json_type_aliases_cannot_change_exact_identity_or_inventory(self):
        value, probe = self.fixture()
        changes = [(("identity",), "generation", True), (("identity",), "generation", 1.0),
                   (("fixture", "records", 0), "ordinal", False),
                   (("fixture", "records", 0), "ordinal", 0.0),
                   (("fixture", "records", 0), "owner", 100.0),
                   (("fixture", "records", 0, "foreground"), "parent", 100.0),
                   (("fixture", "records", 0, "foreground"), "layers", [False]),
                   (("fixture", "records", 0, "foreground"), "layers", [0.0]),
                   (("fixture", "records", 0, "foreground", "identity"), "generation", True),
                   (("fixture", "records", 0, "foreground", "identity"), "generation", 1.0),
                   (("fixture", "records", 0, "foreground", "descriptor"), "anchor_px", [128, 128]),
                   (("fixture", "records", 0, "foreground", "descriptor"), "canvas_px", [256.0, 256.0]),
                   (("fixture", "records", 0, "foreground"), "anchor", [False, 0.]),
                   (("fixture", "records", 0, "foreground", "artifact"), "bytes", 100.0),
                   (("fixture", "records", 0, "foreground"), "canvas_wu", [64, 64])]
        for path, key, changed in changes:
            bad = copy.deepcopy(probe)
            target = bad
            for item in path:
                target = target[item]
            target[key] = changed
            with self.subTest(path=path, key=key, changed=changed), self.assertRaises(ValueError):
                fg.validate(bad, value)


if __name__ == "__main__":
    unittest.main()
