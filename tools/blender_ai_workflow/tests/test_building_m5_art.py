"""Fixed-canvas original proposals are not release/art approval evidence."""
import io
import hashlib
import sys
import unittest
import tempfile
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_m5_art as art


class M5ArtTests(unittest.TestCase):
    def test_parking_worn_strokes_contrast_without_changing_alpha_or_other_kinds(self):
        from PIL import Image
        with Image.open(io.BytesIO(art.render("WheelbarrowParking"))) as image:
            self.assertEqual(hashlib.sha256(image.getchannel("A").tobytes()).hexdigest(),
                             "4c618a8a814d182bafdffe3ee55dff21ee95472dbc250cc88fea6a0dce226a91")
            self.assertEqual(image.getchannel("A").getbbox(), (68, 60, 189, 181))
            # Only existing wear/rut strokes change, not the dark timber face.
            self.assertEqual(image.getpixel((74, 102)), (76, 62, 49, 255))
            for point in ((76, 100), (100, 110), (128, 62)):
                r, g, b, alpha = image.getpixel(point)
                self.assertEqual(alpha, 255)
                self.assertGreater(0.2126*r + 0.7152*g + 0.0722*b, 120)
        for kind, expected in {
            "SandPile": "44dbd1b7ca87502887771cb0db829f7183925b0be4c9f048638e65747608fca8",
            "BonePile": "2483a20c8e9bde63a85e63b511434d4f339d9075688263b48fd0e974f9581226",
        }.items():
            self.assertEqual(hashlib.sha256(art.render(kind)).hexdigest(), expected)

    def test_staging_only_fresh_outputs_and_origin_drift_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(art, "asset_root", return_value=root), \
                    patch("build_building_m5_candidate.asset_root", return_value=root):
                result = art.build("fixture")
                self.assertFalse(result["art_approved"])
                self.assertFalse(result["runtime_published"])
                self.assertEqual(result, art.verify("fixture"))
                with self.assertRaisesRegex(ValueError, "fresh"):
                    art.build("fixture")
                report = art.path("fixture") / "proposal.json"
                value = art.pipeline.read(report)
                value["generator_sha256"] = "0"*64
                report.write_bytes(art.pipeline.canonical(value))
                with self.assertRaisesRegex(ValueError, "origin"):
                    art.verify("fixture")

    def test_fixed_canvas_deterministic_originals_and_state_roles(self):
        from PIL import Image
        for kind in art.KINDS:
            with self.subTest(kind=kind):
                _, spec = art.contract(kind)
                payloads = {role: art.render(kind, on=role == "world_on") for role in spec["world_roles"]}
                art.originals(kind, payloads)
                for role, payload in payloads.items():
                    self.assertEqual(payload, art.render(kind, on=role == "world_on"))
                    with Image.open(io.BytesIO(payload)) as image:
                        self.assertEqual(image.mode, "RGBA")
                        self.assertEqual(image.size, (256, 256))
                if kind == "OutdoorLamp":
                    with Image.open(io.BytesIO(payloads["world_off"])) as off, \
                            Image.open(io.BytesIO(payloads["world_on"])) as on:
                        self.assertEqual(off.getchannel("A").tobytes(), on.getchannel("A").tobytes())
                        self.assertNotEqual(off.tobytes(), on.tobytes())

    def test_unknown_kind_and_nonlamp_on_are_rejected(self):
        with self.assertRaises(ValueError):
            art.render("Bridge")
        with self.assertRaises(ValueError):
            art.render("SandPile", on=True)
        for name in ("../escape", "/absolute", "", "x"*81):
            with self.assertRaises(ValueError):
                art.path(name)

    def test_tall_work_light_keeps_ground_anchor_and_only_head_changes(self):
        from PIL import Image, ImageChops
        value, spec = art.contract("OutdoorLamp")
        self.assertEqual(value["anchor_px"], [128, 208])
        self.assertEqual(spec["canvas_wu"], [64, 64])
        with Image.open(io.BytesIO(art.render("OutdoorLamp"))) as off, \
                Image.open(io.BytesIO(art.render("OutdoorLamp", on=True))) as on:
            bounds = off.getchannel("A").getbbox()
            self.assertGreater(bounds[1], 10)
            self.assertLess(bounds[3], 256)
            self.assertGreater(bounds[3]-bounds[1], 180)
            self.assertEqual(off.getchannel("A").tobytes(), on.getchannel("A").tobytes())
            changed = ImageChops.difference(off.convert("RGB"), on.convert("RGB")).getbbox()
            self.assertTrue(90 <= changed[0] < changed[2] <= 166)
            self.assertTrue(20 <= changed[1] < changed[3] <= 50)
            # The pole and tripod extend far below the head without lighting up.
            for point in ((128, 100), (128, 160), (128, 180)):
                self.assertEqual(off.getpixel(point)[3], 255)
                self.assertEqual(off.getpixel(point), on.getpixel(point))


if __name__ == "__main__":
    unittest.main()
