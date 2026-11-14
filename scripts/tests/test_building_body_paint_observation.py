"""Wrong UV/mode, canvas, matte, foreign part and legacy rejection coverage."""

import copy
import unittest
from scripts import building_body_paint_observation as paint


class BodyPaintTests(unittest.TestCase):
    def test_exact_material_and_water_separation(self):
        contract, witness, handles = self.fixture()
        self.assertEqual(
            paint.expected_texture(contract, "body", handles, witness), "paint"
        )
        self.assertEqual(
            paint.expected_texture(contract, "water", handles, {"mode": 0.82}), "atlas"
        )
        legacy = {"identity": {"kind": "Tank"}}
        self.assertEqual(
            paint.expected_texture(legacy, "body", handles, {"mode": 0.82}), "atlas"
        )
        with self.assertRaises(ValueError):
            paint.expected_texture(legacy, "body", handles, witness)

    def fixture(self):
        return (
            {
                "identity": {"kind": "Tank"},
                "body_paint_contract": {
                    "projection": {
                        "method": "world_preview_v1",
                        "matte_linear_rgb": [0.125, 0.09375, 0.078125],
                    },
                    "world_preview": {
                        "image_role": "world_preview",
                        "canvas_wu": [96, 96],
                        "canvas_px": [256, 256],
                        "anchor_px": [128, 160],
                    },
                },
            },
            {
                "mode": 3.0,
                "canvas": [96, 96, 0.5, 0.625],
                "matte": [0.125, 0.09375, 0.078125, 1],
                "standard": True,
            },
            {"albedo": "atlas", "world_preview": "paint"},
        )

    def test_wrong_mode_uniforms_and_material_missing_or_nonfinite_rejected(self):
        contract, witness, handles = self.fixture()
        for field, value in (
            ("mode", 0.0),
            ("mode", 2.0),
            ("mode", True),
            ("standard", False),
            ("canvas", [96, 96, 0.5, 0.75]),
            ("matte", [float("nan"), 0, 0, 1]),
        ):
            changed = copy.deepcopy(witness)
            changed[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                paint.expected_texture(contract, "body", handles, changed)
        with self.assertRaises(ValueError):
            paint.expected_texture(contract, "body", handles, None)
        with self.assertRaises(ValueError):
            paint.expected_texture(contract, "water", handles, witness)
        contract["identity"]["kind"] = "MudMixer"
        with self.assertRaises(ValueError):
            paint.expected_texture(contract, "body", handles, witness)
