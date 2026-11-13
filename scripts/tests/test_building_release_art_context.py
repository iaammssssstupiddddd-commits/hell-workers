"""Prepared rejection tests for new released-only context, not native evidence."""

import copy
import unittest

from scripts import building_release_art_observation as oracle


def fixture():
    camera = [
        1 / 960,
        0,
        0,
        0,
        0,
        1 / 840,
        0,
        0,
        0,
        0,
        0.001,
        0,
        1072 / 960,
        688 / 840,
        0.5,
        1,
    ]
    projection = {
        "main_clip_from_world": camera,
        "records": [{"shapes": [{"entity": 9000}]}],
    }
    frozen = {"records": [{"gallery_owner": 1000}]}

    def anchor(entity, xy, z=0.0):
        point = [*oracle.grid_point(xy), z]
        return {
            "entity": entity,
            "position": point,
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [1.0, 1.0, 1.0],
            "physical_anchor": oracle.common.project(camera, point, [1920, 1680]),
        }

    def mesh(entity, name, material, *, soul=False):
        return {
            "entity": entity,
            "mesh": name,
            "structural_material": None if soul else material,
            "standard_material": material if soul else None,
            "world_matrix": [
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
                0.0,
                0.0,
                0.0,
                1.0,
            ],
            "aabb_min": [-1.0, -1.0, -1.0],
            "aabb_max": [1.0, 1.0, 1.0],
            "layers": [1] if soul else [1, 5],
            "visible": True,
        }

    structures = []
    for index, (kind, grid) in enumerate(
        (("Wall", [7, 43]), ("Wall", [9, 43]), ("Floor", [10, 43]))
    ):
        row = anchor(100 + index, grid)
        row.update(
            kind=kind,
            visible_meshes=[
                mesh(
                    200 + index,
                    "wall0" if kind == "Wall" else "floor",
                    "wall-material" if kind == "Wall" else "floor-material",
                )
            ],
        )
        structures.append(row)
    soul = anchor(400, [11, 34])
    soul["visible_meshes"] = [mesh(401, "soul", "soul-material", soul=True)]
    icon = anchor(403, [11, 34], 0.5)
    icon["position"][1] += 20.0
    icon["physical_anchor"] = oracle.common.project(
        camera, icon["position"], [1920, 1680]
    )
    icon.update(
        image="wood-image",
        size=[12.0, 12.0],
        color=[1.0, 1.0, 1.0, 1.0],
        worker=400,
        item=402,
        world_matrix=oracle.m6.art.foreground.matrix(
            {
                "translation": icon["position"],
                "rotation": icon["rotation"],
                "scale": icon["scale"],
            }
        ),
    )
    identity = {
        "asset_set_generation": 13,
        "authority": "release_approved",
        "manifest_sha256": "a" * 64,
    }
    context = {
        "scope": "seeded-release-storyboard-context",
        "projection_scope": "context-anchors-only; full appearance independently inspected",
        "core_souls": list(range(15)),
        "total_souls": 16,
        "structures": structures,
        "wall_identity": identity,
        "wall_meshes": ["wall" + str(i) for i in range(6)],
        "wall_material": "wall-material",
        "floor_mesh": "floor",
        "floor_material": "floor-material",
        "carrier_mesh": "soul",
        "carrier_grid": [11, 34],
        "floor_grid": [10, 43],
        "carrier": soul,
        "held_resource": "Wood",
        "item": 402,
        "item_hidden": True,
        "icon": icon,
    }
    return context, projection, frozen, copy.deepcopy(identity)


class ReleasedContextTests(unittest.TestCase):
    def check(self, values):
        return oracle.context(*values)

    def test_exact_released_context_is_separate_from_core_and_targets(self):
        self.check(fixture())

    def test_rejects_population_authority_item_and_identity_aliases(self):
        changes = [
            ("total_souls", 15),
            ("total_souls", True),
            ("scope", "natural-haul"),
            ("held_resource", "Bone"),
            ("item_hidden", False),
            ("item", 1000),
            ("floor_grid", [11, 43]),
            ("floor_grid", [10.0, 43]),
            ("carrier_grid", [10, 34]),
            ("core_souls", list(range(14)) + [0]),
        ]
        for key, value in changes:
            with self.subTest(key=key, value=value):
                data = fixture()
                data[0][key] = value
                with self.assertRaises(ValueError):
                    self.check(data)
        data = fixture()
        data[0]["wall_identity"]["authority"] = "candidate"
        with self.assertRaises(ValueError):
            self.check(data)

    def test_rejects_wrong_or_bool_layers_and_duplicate_visuals(self):
        for layers in ([1], [1, True], [1, 5.0], [1, 5, 6]):
            with self.subTest(layers=layers):
                data = fixture()
                data[0]["structures"][0]["visible_meshes"][0]["layers"] = layers
                with self.assertRaises(ValueError):
                    self.check(data)
        data = fixture()
        data[0]["structures"][0]["visible_meshes"] *= 2
        with self.assertRaises(ValueError):
            self.check(data)
        data = fixture()
        data[0]["structures"][0]["visible_meshes"][0]["entity"] = 9000
        with self.assertRaises(ValueError):
            self.check(data)

    def test_rejects_foreign_or_unloaded_material_and_mesh(self):
        for key, value in [
            ("mesh", "unadmitted"),
            ("structural_material", None),
            ("standard_material", "foreign"),
            ("visible", False),
            ("aabb_max", [-2.0, -2.0, -2.0]),
        ]:
            with self.subTest(key=key):
                data = fixture()
                data[0]["structures"][0]["visible_meshes"][0][key] = value
                with self.assertRaises(ValueError):
                    self.check(data)

    def test_rejects_bool_or_nonfinite_pose_and_projection_forgery(self):
        for key, index, value in [
            ("rotation", 3, True),
            ("scale", 0, True),
            ("position", 0, float("nan")),
            ("physical_anchor", 0, -1.0),
        ]:
            with self.subTest(key=key):
                data = fixture()
                data[0]["structures"][0][key][index] = value
                with self.assertRaises(ValueError):
                    self.check(data)

    def test_rejects_fake_carried_icon_or_duplicate_actor(self):
        for key, value in [
            ("worker", 1),
            ("item", 3),
            ("size", [True, 12.0]),
            ("color", [True, 1.0, 1.0, 1.0]),
            ("entity", 200),
        ]:
            with self.subTest(key=key):
                data = fixture()
                data[0]["icon"][key] = value
                with self.assertRaises(ValueError):
                    self.check(data)
        data = fixture()
        data[0]["carrier"]["entity"] = 1000
        with self.assertRaises(ValueError):
            self.check(data)

    def test_meshes_cannot_alias_own_later_context_or_core_owner(self):
        for identity in (100, 102, 400, 1):
            with self.subTest(identity=identity):
                data = fixture()
                data[0]["structures"][0]["visible_meshes"][0]["entity"] = identity
                with self.assertRaises(ValueError):
                    self.check(data)

    def test_bevy_white_f32_srgba_conversion_is_admitted_but_gray_is_not(self):
        data = fixture()
        data[0]["icon"]["color"] = [0.9999999403953552] * 3 + [1.0]
        self.check(data)
        for color in ([0.999, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.99999994]):
            data = fixture()
            data[0]["icon"]["color"] = color
            with self.assertRaises(ValueError):
                self.check(data)

    def test_rejects_unknown_context_fields(self):
        data = fixture()
        data[0]["accepted"] = True
        with self.assertRaises(ValueError):
            self.check(data)


if __name__ == "__main__":
    unittest.main()
