"""Synthetic raster fixtures test the packer's preservation contract, not art."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from pack_wall_core_atlas import verify_packed
from wall_surface_uv import CORE_PACK_RECT


class WallCoreAtlasTests(unittest.TestCase):
    def test_only_core_slot_may_change_and_emission_must_be_black(self):
        with tempfile.TemporaryDirectory() as directory:
            source, packed = [Path(directory) / name for name in ("source.png", "packed.png")]
            fixture = Image.new("RGB", (1024, 1024), (34, 30, 28))
            fixture.save(source)
            x, y, w, h = CORE_PACK_RECT
            fixture.paste((0, 0, 0), (x, y, x + w, y + h))
            fixture.save(packed)
            self.assertTrue(verify_packed(source, packed, emissive=True)["exterior_pixels_unchanged"])
            fixture.putpixel((x, y), (1, 0, 0))
            fixture.save(packed)
            with self.assertRaisesRegex(ValueError, "emit light"):
                verify_packed(source, packed, emissive=True)
            fixture.putpixel((x - 1, y), (1, 0, 0))
            fixture.save(packed)
            with self.assertRaisesRegex(ValueError, "protected exterior"):
                verify_packed(source, packed, emissive=False)

    def test_wrong_dimensions_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "small.png"
            Image.new("RGB", (256, 256)).save(path)
            with self.assertRaisesRegex(ValueError, "1024x1024"):
                verify_packed(path, path, emissive=False)


if __name__ == "__main__":
    unittest.main()
