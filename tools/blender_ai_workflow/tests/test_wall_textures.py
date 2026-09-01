from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

from PIL import Image

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/validate_wall_textures.py"


def load_validator():
    spec = importlib.util.spec_from_file_location("validate_wall_textures", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load validate_wall_textures.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WallTextureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = load_validator()

    def fixture(self, root: Path) -> None:
        Image.new("RGB", (1024, 1024), (33, 27, 27)).save(root / "wall_albedo.png")
        emissive = Image.new("RGB", (1024, 1024), (0, 0, 0))
        for y in range(800, 820):
            for x in range(100, 120):
                emissive.putpixel((x, y), (190, 0, 190))
        emissive.save(root / "wall_emissive.png")
        Image.new("RGB", (1024, 1024), (128, 128, 255)).save(root / "wall_normal.png")

    def test_valid_texture_set_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            report = self.validator.validate_textures(
                root, normal_sampling="linear", normal_convention="+Y"
            )
            self.assertEqual(report["status"], "pass")
            self.assertEqual(report["normal"]["convention"], "+Y")

    def test_emissive_leakage_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            emissive = Image.open(root / "wall_emissive.png")
            emissive.putpixel((900, 100), (190, 0, 190))
            emissive.save(root / "wall_emissive.png")
            with self.assertRaisesRegex(
                self.validator.TextureError, "escapes the purple UV region"
            ):
                self.validator.validate_textures(
                    root, normal_sampling="linear", normal_convention="+Y"
                )

    def test_wrong_normal_convention_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            with self.assertRaisesRegex(self.validator.TextureError, "OpenGL"):
                self.validator.validate_textures(
                    root, normal_sampling="linear", normal_convention="-Y"
                )

    def test_back_facing_normal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root)
            Image.new("RGB", (1024, 1024), (128, 128, 0)).save(root / "wall_normal.png")
            with self.assertRaisesRegex(self.validator.TextureError, "back-facing"):
                self.validator.validate_textures(
                    root, normal_sampling="linear", normal_convention="+Y"
                )


if __name__ == "__main__":
    unittest.main()
