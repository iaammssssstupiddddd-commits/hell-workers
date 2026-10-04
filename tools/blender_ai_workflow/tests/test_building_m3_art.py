"""Pure M3 authoring regressions; no Blender, approval, or native substitute."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import building_m3_art as art
from build_building_m3_candidate import recipe


class M3ArtTests(unittest.TestCase):
    def test_restarea_entry_is_recessed_without_changing_ground_envelope(self):
        _, spec = art.contract("RestArea")
        self.assertEqual(spec["roof"]["front_z_profile"], [29, 12, 6, 12, 29])
        body = art.geometry("RestArea")["body"]
        self.assertEqual(list(body.bounds()), spec["body_bounds_wu"])
        front = [v for face, surface in zip(body.faces, body.surfaces, strict=True)
                 if surface == "cloth:canopy" for v in (body.vertices[i] for i in face)]
        self.assertEqual(max(z for x, _, z in front if x == 0), 6)
        self.assertEqual(spec["posts"]["x"], [-25, 25])
        self.assertEqual(spec["posts"]["z"], [-24, 24])

    def test_logical_slot_order_and_shared_mesh(self):
        parts = art.parts("SoulSpa")
        self.assertEqual([part["name"] for part in parts], ["body", "slot0", "slot1", "slot2", "slot3"])
        self.assertEqual([part["translation_wu"] for part in parts[1:]],
                         [[-16, 0.6, -16], [16, 0.6, -16], [-16, 0.6, 16], [16, 0.6, 16]])
        self.assertEqual({part["mesh_role"] for part in parts[1:]}, {"slot"})
        self.assertEqual({part["material_role"] for part in parts[1:]}, {"spa_slot"})
        self.assertEqual(len(art.states("SoulSpa")), 17)
        self.assertEqual(art.states("SoulSpa")[1:], [f"Mask{i:02d}" for i in range(16)])

    def test_numeric_geometry_atlas_budget_and_projection(self):
        for kind in art.KINDS:
            value, spec = art.contract(kind)
            geometry = art.geometry(kind)
            self.assertEqual(set(geometry), set(spec["roles"]))
            layout = art.face_layout(kind)
            cells = []
            for role, mesh in geometry.items():
                self.assertLessEqual(mesh.triangles(), spec["roles"][role]["triangle_cap"])
                self.assertEqual(len(mesh.faces), len(layout[role]))
                for face in layout[role]:
                    cells.append(face["cell"])
                    self.assertTrue(all(0 < v < 1 for uv in face["uv"] for v in uv))
            self.assertEqual(len(cells), len(set(cells)))
            self.assertLessEqual(len(cells), (value["atlas"]["size_px"]//value["atlas"]["cell_px"])**2)
            for part in art.parts(kind):
                for vertex in geometry[part["mesh_role"]].vertices:
                    x, y, z = [v+t for v, t in zip(vertex, part["translation_wu"], strict=True)]
                    preview = spec["preview"]
                    scale = preview["canvas_px"][0] / preview["canvas_wu"][0]
                    for ax, ay in (preview["anchor_px"], (128, 128)):
                        self.assertTrue(0 < ax+x*scale < 256)
                        self.assertTrue(0 < ay+(z-0.6*y)*scale < 256)
            if kind == "RestArea":
                self.assertEqual(list(geometry["body"].bounds()), spec["body_bounds_wu"])
            else:
                self.assertLess(geometry["body"].bounds()[1][1], 8)

    def test_paint_is_deterministic_and_emission_is_slot_only(self):
        from PIL import Image
        for kind in art.KINDS:
            with tempfile.TemporaryDirectory() as directory:
                output = Path(directory)
                art.paint(kind, output, "strokes")
                first = (output / "albedo.png").read_bytes()
                art.paint(kind, output, "strokes")
                self.assertEqual(first, (output / "albedo.png").read_bytes())
                if kind == "SoulSpa":
                    with Image.open(output / "slot_emissive.png") as image:
                        self.assertIsNotNone(image.convert("RGB").getbbox())
                        for face in art.face_layout(kind)["body"]:
                            x, y = face["cell"] % 16 * 32, face["cell"] // 16 * 32
                            self.assertIsNone(image.crop((x, y, x+32, y+32)).convert("RGB").getbbox())
                else:
                    self.assertFalse((output / "slot_emissive.png").exists())

    def test_recipe_roles_representatives_and_no_authority(self):
        for kind in art.KINDS:
            with tempfile.TemporaryDirectory() as directory:
                source = Path(directory)
                names = ["source.blend", "geometry.json", "body.glb", "albedo.png", "catalog.png"]
                names += ["Empty.png"] if kind == "RestArea" else ["slot.glb", "slot_emissive.png", "Mask00.png"]
                for name in names:
                    (source / name).write_bytes(name.encode())
                value = recipe(kind, 1, source)
                expected = ["mesh:body"] + (["mesh:slot"] if kind == "SoulSpa" else [])
                expected += ["image:albedo"] + (["image:slot_emissive"] if kind == "SoulSpa" else [])
                expected += ["image:world_preview", "image:catalog"]
                self.assertEqual([item["role"] for item in value["artifacts"]], expected)
                self.assertEqual(value["parts"], art.parts(kind))
                self.assertEqual(value["catalog_preview"]["anchor_px"], [128, 128])
                self.assertEqual(value["world_preview"]["representative_state"],
                                 "Empty" if kind == "RestArea" else "OperationalMaskZero")
                self.assertNotIn("receipt", value)
                self.assertNotIn("authority", value)
        self.assertFalse(json.loads(art.CONTRACT.read_bytes())["art_approved"])

    def test_cloth_patch_is_distinct_from_wood_grain_without_geometry_changes(self):
        from PIL import Image
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            art.paint("RestArea", output, "planes")
            with Image.open(output / "albedo.png") as image:
                planes = image.copy()
            art.paint("RestArea", output, "strokes")
            with Image.open(output / "albedo.png") as image:
                strokes = image.copy()
            _, spec = art.contract("RestArea")
            light_cloth = 0
            for face in art.face_layout("RestArea")["body"]:
                x, y = face["cell"] % 16 * 32, face["cell"] // 16 * 32
                for px in range(x, x+32):
                    for py in range(y, y+32):
                        before, after = planes.getpixel((px, py)), strokes.getpixel((px, py))
                        self.assertEqual(before[3], after[3])
                        if face["surface"].startswith("wood"):
                            self.assertTrue(all(a <= b for a, b in zip(after[:3], before[:3])))
                        elif face["surface"].startswith("cloth") and after[0] > before[0]:
                            light_cloth += 1
            self.assertGreater(light_cloth, 0, "cloth needs a coherent lighter repair, not repeated wood strokes")
            self.assertEqual(list(art.geometry("RestArea")["body"].bounds()), spec["body_bounds_wu"])


if __name__ == "__main__":
    unittest.main()
