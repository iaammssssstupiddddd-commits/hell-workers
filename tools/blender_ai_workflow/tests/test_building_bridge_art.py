"""Bridge proposal contract only; never substitutes for clay/native/art acceptance."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_bridge_art as art
import building_asset_pipeline as pipeline


class BridgeArtTests(unittest.TestCase):
    def test_bridge_exact_roles_low_deck_and_order(self):
        value, spec = art.contract("Bridge")
        self.assertEqual(pipeline.KINDS["Bridge"]["mesh_roles"], ["body"])
        self.assertEqual(pipeline.KINDS["Bridge"]["image_roles"], ["albedo", "world_preview", "catalog"])
        self.assertEqual(art.states("Bridge"), ["Complete"])
        self.assertEqual(value["ordered_relative_tiles"], [[x, y] for y in range(5) for x in range(2)])
        mesh = art.geometry("Bridge")["body"]
        self.assertEqual(mesh.bounds(), ([-32.0, 0.0, -80.0], [32.0, 0.9, 80.0]))
        self.assertLessEqual(mesh.triangles(), spec["roles"]["body"]["triangle_cap"])
        self.assertEqual(len(art.parts("Bridge")), 1)
        self.assertFalse(value["numeric_freeze"])
        self.assertFalse(value["art_approved"])
        self.assertFalse(value["runtime_published"])

    def test_uv_islands_have_finite_padded_coordinates(self):
        mesh = art.geometry("Bridge")["body"]
        faces = art.face_layout("Bridge")["body"]
        self.assertEqual(len(faces), len(mesh.faces))
        self.assertEqual(len({face["cell"] for face in faces}), len(faces))
        for face, indices in zip(faces, mesh.faces, strict=True):
            self.assertEqual(len(face["uv"]), len(indices))
            self.assertTrue(all(0 < x < 1 and 0 < y < 1 for x, y in face["uv"]))

    def test_other_kind_cannot_use_bridge_recipe(self):
        for kind in ("Tank", "SoulSpa", "Door", "Wall"):
            with self.assertRaises(ValueError):
                art.contract(kind)


if __name__ == "__main__":
    unittest.main()
