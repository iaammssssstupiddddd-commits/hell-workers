"""Closed pilot rejection tests; these are not native/art acceptance evidence."""

import copy
import json
from pathlib import Path
import tempfile
import unittest
from PIL import Image
from scripts import building_tank_illustration_pilot as pilot
from scripts import building_tank_illustration_observation as oracle


def matrix(translation):
    return [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        *translation,
        1.0,
    ]


def pose(translation):
    return {
        "translation": translation,
        "rotation": [0.0, 0.0, 0.0, 1.0],
        "scale": [1.0, 1.0, 1.0],
    }


class PilotWitnessTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.view = Path(self.tmp.name)
        assets = self.view / "assets"
        (assets / "manifests").mkdir(parents=True)
        (assets / "pilot").mkdir()
        self.paint_path = self.view / "source-paint.png"
        Image.new("RGBA", (2, 2), (94, 82, 79, 255)).save(self.paint_path)
        self.paint, prepared = pilot.prepare_paint(self.paint_path.resolve())
        (assets / "pilot/tank-body.png").write_bytes(prepared)
        Image.new("RGBA", (2, 2), (66, 87, 90, 255)).save(assets / "water.png")
        identity = {
            "kind": "Tank",
            "generation": 3,
            "authority": "release_approved",
            "manifest_sha256": "3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2",
        }
        self.manifests = [
            {
                "identity": identity.copy(),
                "artifacts": [{"role": "image:albedo", "path": "water.png"}],
            }
        ]
        # The valid codec wire key order need not equal Python canonical order.
        (assets / "manifests/building-tank-v1.buildingset").write_text(
            json.dumps(self.manifests[0])
        )
        self.session = {
            "schema_version": 1,
            "contract_id": pilot.PROFILE,
            "paint": self.paint,
            "gallery": {"bindings": [{"identity": identity}], "nonce": "b" * 32},
        }

        def image(handle, path):
            with Image.open(assets / path) as im:
                return {
                    "handle": handle,
                    "asset_path": path,
                    "width": im.width,
                    "height": im.height,
                    "format": "Rgba8UnormSrgb",
                    "mip_levels": 1,
                    "rgba_sha256": pilot.pipeline.digest(im.tobytes()),
                    "bytes": im.width * im.height * 4,
                }

        self.witness = {
            "profile": pilot.PROFILE,
            "accepted": False,
            "promotion_authority": False,
            "performance_evidence": False,
            "paint_image": image("paint", "pilot/tank-body.png"),
            "water_image": image("water", "water.png"),
            "owners": [],
        }
        self.fixture = {"records": []}
        for i, count in enumerate([0, 5, 25, 50]):
            origin = [i * 160.0, 0.0, 0.0]
            owner = 100 + i
            self.fixture["records"].append(
                {
                    "kind": "Tank",
                    "gallery_owner": owner,
                    "center": [origin[0], -origin[2]],
                    "state": {"stored_water": count},
                }
            )
            parts = []
            for role in ["body", "water"]:
                height = 0.0 if role == "body" else 22.0 if count == 50 else 12.0
                local = pose([0.0, height, 0.0])
                body = role == "body"
                parts.append(
                    {
                        "entity": 1000 + i * 2 + int(not body),
                        "role": role,
                        "mesh": role + "-mesh",
                        "material": "body-mat"
                        if body
                        else "water-full"
                        if count in (0, 50)
                        else "water-partial",
                        "mode": 2.0 if body else 0.82,
                        "unlit": False,
                        "alpha": "Opaque",
                        "base_color": [0.99999994, 0.99999994, 0.99999994, 1.0]
                        if body
                        else [1.0, 1.0, 1.0, 1.0]
                        if count in (0, 50)
                        else [0.58, 0.70, 0.73, 1.0],
                        "texture": "paint" if body else "water",
                        "projection": [
                            *self.paint["canvas_wu"],
                            *self.paint["anchor_uv"],
                        ]
                        if body
                        else [1.0, 2.0, 3.0, 4.0],
                        "matte": [*self.paint["matte_linear_rgb"], 1.0]
                        if body
                        else [1.0, 1.0, 1.0, 1.0],
                        "local": local,
                        "global": matrix([origin[0], height, origin[2]]),
                        "visible": body or count != 0,
                    }
                )
            self.witness["owners"].append(
                {
                    "owner": owner,
                    "root": 200 + i,
                    "identity": identity.copy(),
                    "root_pose": pose(origin),
                    "root_global": matrix(origin),
                    "parts": parts,
                }
            )

        self.projection = {"records": [], "tank_paint_hierarchy": []}
        for record, row in zip(
            self.fixture["records"], self.witness["owners"], strict=True
        ):
            record["gallery_geometry"] = [
                {
                    "mesh_role": p["role"],
                    "entity": p["entity"],
                    "mesh": p["mesh"],
                    "local_pose": copy.deepcopy(p["local"]),
                    "visible": p["visible"],
                }
                for p in row["parts"]
            ]
            self.projection["tank_paint_hierarchy"].extend(
                {"entity": p["entity"], "parent": row["root"], "owner": row["owner"]}
                for p in row["parts"]
            )
            self.projection["records"].append(
                {
                    "kind": "Tank",
                    "owner": row["owner"],
                    "shapes": [
                        {
                            "entity": p["entity"],
                            "mesh": p["mesh"],
                            "type": "mesh3d",
                            "world_matrix": copy.deepcopy(p["global"]),
                        }
                        for p in row["parts"]
                        if p["visible"]
                    ],
                }
            )

    def validate(self, witness=None):
        oracle.validate_witness(
            witness or self.witness,
            self.session,
            self.fixture,
            self.view,
            self.projection,
            self.manifests,
        )

    def test_real_white_rounding_and_all_three_water_states_are_accepted(self):
        self.validate()

    def test_only_exact_frozen_codec_manifest_identity_supplies_water_image(self):
        self.manifests[0]["identity"]["generation"] = 2
        with self.assertRaises(ValueError):
            self.validate()

    def test_wrong_owner_root_origin_identity_and_duplicate_part_are_rejected(self):
        mutations = [
            lambda w: w["owners"][0].update(owner=999),
            lambda w: w["owners"][0]["root_pose"]["translation"].__setitem__(1, 1.0),
            lambda w: w["owners"][0]["identity"].update(generation=2),
            lambda w: w["owners"][1]["parts"][0].update(entity=1000),
        ]
        for change in mutations:
            w = copy.deepcopy(self.witness)
            change(w)
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.validate(w)

    def test_paint_cannot_be_missing_applied_to_water_or_replaced_by_lighting_only(
        self,
    ):
        for role, key, value in [
            ("body", "texture", None),
            ("body", "texture", "water"),
            ("body", "mode", 1.0),
            ("water", "texture", "paint"),
            ("water", "mode", 2.0),
        ]:
            w = copy.deepcopy(self.witness)
            p = next(p for p in w["owners"][0]["parts"] if p["role"] == role)
            p[key] = value
            with self.subTest(role=role, key=key), self.assertRaises(ValueError):
                self.validate(w)

    def test_canvas_anchor_surface_flags_and_color_are_pinned(self):
        for key, value in [
            ("projection", [95.0, 96.0, 0.5, 0.625]),
            ("matte", [0.0, 0.0, 0.0, 1.0]),
            ("unlit", True),
            ("alpha", "Mask"),
            ("base_color", [0.95, 1.0, 1.0, 1.0]),
        ]:
            w = copy.deepcopy(self.witness)
            w["owners"][0]["parts"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(w)

    def test_water_pose_visibility_and_color_are_not_inferred_from_paint(self):
        for ordinal, key, value in [
            (0, "visible", True),
            (1, "local", pose([0.0, 22.0, 0.0])),
            (3, "base_color", [0.58, 0.7, 0.73, 1.0]),
        ]:
            w = copy.deepcopy(self.witness)
            w["owners"][ordinal]["parts"][1][key] = value
            with self.subTest(ordinal=ordinal), self.assertRaises(ValueError):
                self.validate(w)

    def test_actual_image_path_decoded_bytes_and_descriptor_must_match(self):
        for key, value in [
            ("asset_path", "other.png"),
            ("rgba_sha256", "0" * 64),
            ("width", 1),
            ("format", "Rgba8Unorm"),
            ("mip_levels", 2),
            ("bytes", 15),
            ("bytes", True),
        ]:
            w = copy.deepcopy(self.witness)
            w["paint_image"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(w)

    def test_foreign_mesh_and_per_owner_material_growth_are_rejected(self):
        for change in [
            lambda w: w["owners"][0]["parts"][0].update(mesh="water-mesh"),
            lambda w: w["owners"][1]["parts"][0].update(material="extra-body"),
        ]:
            w = copy.deepcopy(self.witness)
            change(w)
            with self.assertRaises(ValueError):
                self.validate(w)

    def test_self_consistent_foreign_entity_or_mesh_cannot_replace_rendered_geometry(
        self,
    ):
        for key, value in [("entity", 9999), ("mesh", "foreign-body")]:
            w = copy.deepcopy(self.witness)
            if key == "mesh":
                for row in w["owners"]:
                    row["parts"][0][key] = value
            else:
                w["owners"][0]["parts"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate(w)

    def test_nonfinite_and_boolean_pose_components_are_rejected(self):
        for v in [float("nan"), False]:
            w = copy.deepcopy(self.witness)
            w["owners"][0]["root_pose"]["translation"][1] = v
            with self.assertRaises(ValueError):
                self.validate(w)

    def test_root_must_match_actual_childof_with_disjoint_complete_hierarchy(self):
        for root in (9999, 100, 1000):
            w = copy.deepcopy(self.witness)
            w["owners"][0]["root"] = root
            with self.subTest(root=root), self.assertRaises(ValueError):
                self.validate(w)
        for key in ("parent", "owner"):
            before = copy.deepcopy(self.projection)
            self.projection["tank_paint_hierarchy"][1][key] = 9999
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.validate()
            self.projection = before
        self.projection["tank_paint_hierarchy"][-1] = self.projection[
            "tank_paint_hierarchy"
        ][0]
        with self.assertRaises(ValueError):
            self.validate()
        self.projection["tank_paint_hierarchy"].pop()
        with self.assertRaises(ValueError):
            self.validate()

    def test_paint_input_requires_png_rgba_and_is_hash_bound(self):
        before = pilot.paint_input(self.paint_path.resolve())
        Image.new("RGBA", (2, 2), (94, 82, 78, 255)).save(self.paint_path)
        self.assertNotEqual(
            before["rgba_sha256"],
            pilot.paint_input(self.paint_path.resolve())["rgba_sha256"],
        )
        Image.new("RGB", (2, 2), (94, 82, 79)).save(self.paint_path)
        with self.assertRaises(ValueError):
            pilot.paint_input(self.paint_path.resolve())

    def test_transparent_black_rgb_is_matted_before_filtering_and_source_is_unchanged(
        self,
    ):
        Image.new("RGBA", (4, 4), (0, 0, 0, 0)).save(self.paint_path)
        before = self.paint_path.read_bytes()
        spec, raw = pilot.prepare_paint(self.paint_path.resolve())
        import io

        with Image.open(io.BytesIO(raw)) as im:
            self.assertEqual(
                im.getextrema(), ((99, 99), (86, 86), (79, 79), (255, 255))
            )
        self.assertEqual(self.paint_path.read_bytes(), before)
        self.assertEqual(spec["dimensions"], [256, 256])

    def test_pilot_contract_cannot_enter_normal_released_or_performance_oracles(self):
        v = {
            "schema_version": 1,
            "contract_id": oracle.geometry.TANK_PILOT_CONTRACT,
            "evidence_kind": "paused-static-only",
            "active_simulation_evidence": False,
            "camera_scale": 1.0,
            "stable_frames": 60,
            "layout_sha256": "a" * 64,
            "initial": {},
            "final": {},
        }
        case = pilot.m6.Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        for validate in [
            oracle.geometry.validate_release_gallery_sidecar,
            oracle.geometry.validate_sidecar,
        ]:
            with self.assertRaises(ValueError):
                validate(v, case)


if __name__ == "__main__":
    unittest.main()
