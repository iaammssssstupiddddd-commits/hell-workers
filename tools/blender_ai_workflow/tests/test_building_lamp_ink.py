"""Lamp illustration keeps its existing foreground/off-on/anchor contract."""

import io
from pathlib import Path
import sys
import unittest
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_m5_art as art


class LampInkTests(unittest.TestCase):
    def test_flat_pole_uses_original_silhouette_and_same_off_on_geometry(self):
        images = {}
        for style in ("infernal-v1", "illustrated-v1"):
            for on in (False, True):
                payload = art.render("OutdoorLamp", on=on, style=style)
                self.assertEqual(payload, art.render("OutdoorLamp", on=on, style=style))
                with Image.open(io.BytesIO(payload)) as image:
                    images[style, on] = image.copy()
        alpha = images["infernal-v1", False].getchannel("A").tobytes()
        for image in images.values():
            self.assertEqual(image.size, (256, 256))
            self.assertEqual(image.mode, "RGBA")
            self.assertEqual(image.getchannel("A").tobytes(), alpha)
        self.assertNotEqual(
            images["illustrated-v1", False].tobytes(),
            images["infernal-v1", False].tobytes(),
        )
        off, on = images["illustrated-v1", False], images["illustrated-v1", True]
        changes = [
            (x, y)
            for y in range(256)
            for x in range(256)
            if off.getpixel((x, y)) != on.getpixel((x, y))
        ]
        self.assertTrue(changes)
        # Both the emitted glass and its state change stay at the high pole head.
        self.assertLess(max(y for _, y in changes), 90)
        self.assertEqual(art.contract("OutdoorLamp")[0]["anchor_px"], [128, 208])
        art.originals(
            "OutdoorLamp",
            {
                name: art.render("OutdoorLamp", on=on, style="illustrated-v1")
                for name, on in (("world_off", False), ("world_on", True))
            },
        )

    def test_illustration_is_closed_to_lamp_and_invalid_states_are_rejected(self):
        for kind in ("SandPile", "BonePile", "WheelbarrowParking"):
            with self.assertRaises(ValueError):
                art.render(kind, style="illustrated-v1")
        with self.assertRaises(ValueError):
            art.render("OutdoorLamp", style="arbitrary")
