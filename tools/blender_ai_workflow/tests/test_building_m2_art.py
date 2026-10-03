"""Candidate authoring regressions; never substitute for independent art review."""

import struct
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from building_m2_art import candidate_geometry, face_layout, paint
from build_building_m2_candidate import check_uv
from test_building_clay import encoded_geometry, write_glb


class M2ArtTests(unittest.TestCase):
    def test_roles_share_one_atlas_with_unique_face_cells_and_gutters(self):
        for kind in ("Tank", "MudMixer"):
            geometry, layout = candidate_geometry(kind), face_layout(kind)
            cells = []
            for role, faces in layout.items():
                self.assertEqual(len(faces), len(geometry[role].faces))
                for face in faces:
                    cell = face["cell"]
                    cells.append(cell)
                    x, y = (cell % 16) * 32, (cell // 16) * 32
                    for px, py in face["polygon_px"]:
                        self.assertGreaterEqual(px, x + 4)
                        self.assertGreaterEqual(py, y + 4)
                        self.assertLessEqual(px, x + 27 + 1e-9)
                        self.assertLessEqual(py, y + 27 + 1e-9)
            self.assertEqual(len(cells), len(set(cells)))
            self.assertLessEqual(len(cells), 256)

    def test_paint_stages_are_distinct_deterministic_opaque_and_within_budget(self):
        with tempfile.TemporaryDirectory() as directory:
            for kind in ("Tank", "MudMixer"):
                images = []
                for stage in ("planes", "strokes", "strokes"):
                    path = Path(directory) / f"{kind}-{len(images)}.png"
                    paint(kind, path, stage)
                    with Image.open(path) as image:
                        self.assertEqual(image.size, (512, 512))
                        self.assertEqual(image.getchannel("A").getextrema(), (255, 255))
                        images.append(image.tobytes())
                self.assertNotEqual(images[0], images[1])
                self.assertEqual(images[1], images[2])

    def test_exported_uvs_must_match_painted_face_cells(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "role.glb"
            for kind in ("Tank", "MudMixer"):
                layout = face_layout(kind)
                for role, geometry in candidate_geometry(kind).items():
                    document, binary = encoded_geometry(geometry)
                    offset = document["bufferViews"][2]["byteOffset"]
                    uv = [point for face in layout[role] for point in face["uv"]]
                    for index, (u, v) in enumerate(uv):
                        struct.pack_into("<ff", binary, offset + index * 8, u, 1 - v)
                    write_glb(path, document, binary)
                    check_uv(path, geometry, layout[role])
                    struct.pack_into("<ff", binary, offset, 0, 0)
                    write_glb(path, document, binary)
                    with self.assertRaises(ValueError):
                        check_uv(path, geometry, layout[role])


if __name__ == "__main__":
    unittest.main()
