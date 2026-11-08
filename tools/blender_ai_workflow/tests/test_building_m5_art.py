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
            "OutdoorLamp": "a8958577a785ccde040cfbe1a311bf7dac56c24740acdbe66923fb064aa86592",
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

    def test_lamp_readable_cage_preserves_anchor_margin_and_only_interior_changes(self):
        from PIL import Image, ImageChops
        with Image.open(io.BytesIO(art.render("OutdoorLamp"))) as off, \
                Image.open(io.BytesIO(art.render("OutdoorLamp", on=True))) as on:
            bounds = off.getchannel("A").getbbox()
            self.assertGreaterEqual(bounds[1], 8)
            self.assertGreaterEqual(bounds[2] - bounds[0], 108)
            self.assertLessEqual(bounds[3], 145)
            # Changes are enclosed inside the iron rim, above the support.
            difference = ImageChops.difference(off.convert("RGB"), on.convert("RGB"))
            changed = difference.getbbox()
            self.assertTrue(80 <= changed[0] < changed[2] <= 176)
            self.assertTrue(15 <= changed[1] < changed[3] <= 75)
            self.assertGreaterEqual(changed[3] - changed[1], 30)
            # Background stays visible between iron and light; the bone joint
            # and long shaft remain separate from the two illuminated panels.
            self.assertEqual(off.getpixel((96, 36))[3], 0)
            self.assertEqual(off.getpixel((158, 36))[3], 0)
            self.assertEqual(off.getpixel((128, 88))[3], 255)
            self.assertEqual(off.getpixel((128, 104))[3], 255)
            # A broad unlit center bar separates the two illuminated panels.
            self.assertEqual(off.getpixel((128, 40)), on.getpixel((128, 40)))
            self.assertNotEqual(off.getpixel((106, 40)), on.getpixel((106, 40)))
            self.assertNotEqual(off.getpixel((150, 40)), on.getpixel((150, 40)))


if __name__ == "__main__":
    unittest.main()
