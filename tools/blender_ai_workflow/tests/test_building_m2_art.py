"""Candidate authoring regressions; never substitute for independent art review."""

import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import copy
from collections import Counter
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from building_m2_art import candidate_geometry, face_layout, paint, project_xz
from build_building_m2_candidate import check_uv
if __package__:
    from .test_building_clay import encoded_geometry, write_glb
else:
    from test_building_clay import encoded_geometry, write_glb


class M2ArtTests(unittest.TestCase):
    def test_tank_floor_is_one_color_and_iron_stays_on_outer_wall(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tank.png"
            layout = face_layout("Tank")
            paint("Tank", path)
            with Image.open(path) as image:
                floor_colors = set()
                for role, faces in layout.items():
                    for face in faces:
                        cell = face["cell"]
                        x, y = (cell % 16) * 32, (cell // 16) * 32
                        pixels = image.crop((x, y, x + 32, y + 32)).get_flattened_data()
                        iron = (78, 76, 70, 255)
                        rust = (132, 78, 53, 255)
                        if role == "body" and face["surface"] == "vessel:band1":
                            self.assertIn(iron, pixels)
                            rows = sorted({index // 32 for index, color in enumerate(pixels) if color == iron})
                            clusters = 1 + sum(right - left > 2 for left, right in zip(rows, rows[1:]))
                            self.assertEqual(clusters, 2)
                        else:
                            self.assertNotIn(iron, pixels)
                            self.assertNotIn(rust, pixels)
                        if face["surface"] == "vessel:band4":
                            mask = Image.new("L", (32, 32))
                            ImageDraw.Draw(mask).polygon(
                                [(px - x, py - y) for px, py in face["polygon_px"]], fill=255)
                            colors = Counter(image.getpixel((x + ix, y + iy))
                                             for iy in range(32) for ix in range(32)
                                             if mask.getpixel((ix, iy)))
                            self.assertTrue(colors)
                            floor_colors.add(colors.most_common(1)[0][0])
                self.assertEqual(len(floor_colors), 1)

    def test_tank_rejects_unknown_surface_and_degenerate_horizontal_projection(self):
        with tempfile.TemporaryDirectory() as directory:
            layout = copy.deepcopy(face_layout("Tank"))
            layout["body"][0]["surface"] = "vessel:unknown"
            with patch("building_m2_art.face_layout", return_value=layout):
                with self.assertRaisesRegex(ValueError, "unknown Tank vessel surface"):
                    paint("Tank", Path(directory) / "invalid.png")
        with self.assertRaisesRegex(ValueError, "X/Z projection"):
            project_xz([(0, 0, 0), (1, 0, 0), (2, 0, 0)],
                       [(0, 0), (1, 0), (2, 0)], 1, 0)

    def test_albedo_does_not_bake_directional_face_lighting(self):
        import building_m2_art as art
        with tempfile.TemporaryDirectory() as directory:
            for kind in ("Tank", "MudMixer"):
                original = Path(directory) / (kind + ".png")
                changed = Path(directory) / (kind + "-normal.png")
                art.paint(kind, original)
                layout = copy.deepcopy(art.face_layout(kind))
                for faces in layout.values():
                    for face in faces:
                        face["normal"] = [-n for n in face["normal"]]
                with patch.object(art, "face_layout", return_value=layout):
                    art.paint(kind, changed)
                self.assertEqual(original.read_bytes(), changed.read_bytes())

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
