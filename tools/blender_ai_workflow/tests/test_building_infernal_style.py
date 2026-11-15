"""Paint revisions preserve old pixels and all fixed runtime presentation bounds."""

import hashlib
import io
from pathlib import Path
import sys
import tempfile
import unittest

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_infernal_style as style
import building_m2_art as m2
import building_m3_art as m3
import building_m5_art as m5

# Decoded pixels from the accepted 0c4b9b05 source, not PNG encoder bytes.
LEGACY = {
    "Tank": "66f89e482cfaac5714a8972933e2b1054f5be08d24243ba37c7acf907fdbac00",
    "MudMixer": "862723642a5032cb1826d4816022723020e7117f8c562b80800ece5a0ce644a0",
    "RestArea": "1bbe5587aabf3b3b4ed9e7fc6df1582609a895f950255730450ebd8164958f88",
    "SoulSpa": "75c1ccce2f37f662694a8e7b11cfef5f6685d2be2e08c5edc53829cb1643ba39",
    "OutdoorLamp": "455e82fe327990c33031e1bba0591929867c5f645960d7f8cdc7b8d10cd45965",
}


def painted(kind, paint_style):
    with tempfile.TemporaryDirectory() as directory:
        out = Path(directory)
        if kind in ("Tank", "MudMixer"):
            path = out / "albedo.png"
            m2.paint(
                kind,
                path,
                revision="v3" if kind == "MudMixer" else "v1",
                style=paint_style,
            )
        elif kind in ("RestArea", "SoulSpa"):
            m3.paint(kind, out, "strokes", style=paint_style)
            path = out / "albedo.png"
        else:
            with Image.open(io.BytesIO(m5.render(kind, style=paint_style))) as image:
                return image.copy()
        with Image.open(path) as image:
            return image.copy()


class InfernalStyleTests(unittest.TestCase):
    def test_legacy_pixels_remain_identical_to_the_adopted_source(self):
        for kind, digest in LEGACY.items():
            with self.subTest(kind=kind):
                self.assertEqual(
                    hashlib.sha256(painted(kind, "legacy").tobytes()).hexdigest(),
                    digest,
                )

    def test_new_paint_is_deterministic_and_preserves_dimensions_alpha_and_layout(self):
        for kind in ("Tank", "MudMixer", "RestArea", "OutdoorLamp"):
            with self.subTest(kind=kind):
                before = painted(kind, "legacy")
                after = painted(kind, style.STYLE)
                self.assertEqual(before.mode, after.mode)
                self.assertEqual(before.size, after.size)
                self.assertEqual(
                    before.getchannel("A").tobytes(), after.getchannel("A").tobytes()
                )
                self.assertNotEqual(before.tobytes(), after.tobytes())
                self.assertEqual(after.tobytes(), painted(kind, style.STYLE).tobytes())

    def test_lamp_on_off_differ_in_color_but_never_footprint(self):
        with (
            Image.open(io.BytesIO(m5.render("OutdoorLamp", style=style.STYLE))) as a,
            Image.open(
                io.BytesIO(m5.render("OutdoorLamp", on=True, style=style.STYLE))
            ) as b,
        ):
            self.assertEqual(a.getchannel("A").tobytes(), b.getchannel("A").tobytes())
            self.assertNotEqual(a.tobytes(), b.tobytes())
            self.assertEqual(a.size, (256, 256))

    def test_spa_reference_paint_is_not_changed_by_rest_area_work(self):
        self.assertEqual(
            painted("SoulSpa", "legacy").tobytes(),
            painted("SoulSpa", style.STYLE).tobytes(),
        )

    def test_unknown_style_is_rejected_before_a_candidate_is_written(self):
        for kind in ("Tank", "MudMixer", "RestArea"):
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                painted(kind, "unknown")
        with self.assertRaises(ValueError):
            m5.render("OutdoorLamp", style="unknown")
        with self.assertRaises(ValueError):
            m5.render("BonePile", style=style.STYLE)
