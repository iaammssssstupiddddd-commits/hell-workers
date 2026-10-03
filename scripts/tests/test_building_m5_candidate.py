"""M5 artwork packaging boundaries; synthetic pixels are not art approval."""
import io
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import building_m5_acceptance as acceptance

m5 = acceptance.candidate


def png(color=(155, 140, 115, 255), size=(256, 256), box=(70, 40, 180, 220)):
    from PIL import Image, ImageDraw
    image = Image.new("RGBA", size)
    ImageDraw.Draw(image).rectangle(box, fill=color)
    stream = io.BytesIO()
    image.save(stream, format="PNG")
    return stream.getvalue()


class M5CandidateTests(unittest.TestCase):
    def test_roles_and_world_geometry_remain_separate_from_contain_catalog(self):
        from PIL import Image
        payload = png()
        card = m5.originals("SandPile", {"world": payload})
        with Image.open(io.BytesIO(card)) as image:
            bounds = image.getchannel("A").getbbox()
            self.assertEqual(image.size, (256, 256))
            self.assertEqual(bounds[1], 16)
            self.assertEqual(bounds[3], 240)
            self.assertAlmostEqual((bounds[0] + bounds[2]) / 2, 128, delta=1)
        for kind in m5.KINDS:
            world, catalog = m5.previews(kind)
            self.assertEqual(world["anchor_px"], [128, 128])
            self.assertEqual(catalog["image_role"], "catalog")
            self.assertEqual(world["canvas_wu"], [64, 64] if kind == "WheelbarrowParking" else [32, 32])
        self.assertEqual(m5.previews("OutdoorLamp")[0]["image_role"], "world_off")

    def test_rejects_wrong_canvas_clipping_and_magenta(self):
        for payload in (png(size=(128, 256)), png(box=(0, 0, 30, 30)), png(color=(255, 0, 255, 255))):
            with self.assertRaises(ValueError):
                m5.originals("SandPile", {"world": payload})

    def test_lamp_requires_visible_state_difference_with_identical_alpha(self):
        off = png()
        with self.assertRaisesRegex(ValueError, "no visible state"):
            m5.originals("OutdoorLamp", {"world_off": off, "world_on": off})
        with self.assertRaisesRegex(ValueError, "silhouette"):
            m5.originals("OutdoorLamp", {"world_off": off, "world_on": png(box=(60, 40, 180, 220))})
        self.assertTrue(m5.originals("OutdoorLamp", {"world_off": off, "world_on": png(color=(210, 180, 240, 255))}))

    def test_staging_boundary_rejects_product_paths_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"HELL_WORKERS_ASSET_ROOT": directory}):
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, "staging"):
                m5.staged(root / "assets")
            (root / "staging").mkdir()
            (root / "staging/link").symlink_to(root, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                m5.staged(root / "staging/link/image.png")
            with self.assertRaises(ValueError):
                m5.destination("../product")

    def test_prepare_preserves_world_bytes_and_cannot_grant_approval(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {"HELL_WORKERS_ASSET_ROOT": directory}):
            root = Path(directory)
            source = root / "staging/originals/parking"
            source.mkdir(parents=True)
            payload = png()
            (source / "world.png").write_bytes(payload)
            with patch.object(m5.pipeline, "export", return_value={"identity": {"authority": "art_preview"}}) as export:
                with patch.object(m5, "verify"):
                    result = m5.prepare("WheelbarrowParking", source, "parking-v1", 1, root / "codec")
            output = m5.destination("parking-v1")
            self.assertEqual((output / "authoring/world.png").read_bytes(), payload)
            recipe = m5.pipeline.read(output / "recipe.json")
            self.assertEqual([entry["role"] for entry in recipe["artifacts"]], ["image:world", "image:catalog"])
            self.assertEqual(recipe["parts"], [])
            self.assertEqual(export.call_args.kwargs, {"authority": "art_preview"})
            self.assertFalse(result["art_approved"])
            self.assertFalse(result["runtime_published"])
            with self.assertRaisesRegex(ValueError, "fresh"):
                m5.prepare("WheelbarrowParking", source, "parking-v1", 2, root / "codec")

    def test_release_bindings_require_every_explicit_m5_authority(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def pointer(_root, kind):
                return {"identity": {"kind": kind, "authority": "release_approved"}}
            def load(_root, kind, _codec):
                return ({"identity": pointer(_root, kind)["identity"]}, "", None)
            with patch.object(m5.pipeline, "pointer", side_effect=pointer):
                with patch.object(m5.pipeline, "load_set", side_effect=load):
                    result = m5.pipeline.release_bindings(root, root / "codec", "m5")
                    self.assertEqual(len(result), 8)
                    self.assertEqual(len(m5.pipeline.release_bindings(root, root / "codec")), 2)
            with patch.object(m5.pipeline, "pointer", return_value=None):
                with self.assertRaisesRegex(ValueError, "no active release"):
                    m5.pipeline.release_bindings(root, root / "codec", "m5")

    def test_protected_inventory_detects_resource_and_vehicle_change(self):
        import json
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = json.loads(m5.CONTRACT.read_bytes())["protected_images"]
            for relative in paths:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"original")
            before = acceptance.protected(root)
            (root / paths[0]).write_bytes(b"replacement")
            self.assertNotEqual(before, acceptance.protected(root))

    def test_native_legs_do_not_claim_gameplay_or_release_acceptance(self):
        for kind in m5.KINDS:
            legs = acceptance.legs(kind)
            self.assertTrue(all(not leg["accepted"] and not leg["evidence"] for leg in legs))
            self.assertIn("save-load", {leg["id"] for leg in legs})
        self.assertIn("power", {leg["id"] for leg in acceptance.legs("OutdoorLamp")})
        self.assertIn("vehicle", {leg["id"] for leg in acceptance.legs("WheelbarrowParking")})


if __name__ == "__main__":
    unittest.main()
