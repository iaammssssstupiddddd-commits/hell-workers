"""Pure M3 authoring regressions; no Blender, approval, or native substitute."""
import json
import struct
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))
import building_m3_art as art
from build_building_m3_candidate import recipe
from build_building_m3_candidate import check_uv as check_m3_uv
from build_building_m2_candidate import check_uv as check_m2_uv
if __package__:
    from .test_building_clay import encoded_geometry, write_glb
else:
    from test_building_clay import encoded_geometry, write_glb


class M3ArtTests(unittest.TestCase):
    def test_m3_cap_faces_are_indexed_exactly_without_relaxing_m2(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "role.glb"
            for role, mesh in art.geometry("SoulSpa").items():
                layout = art.face_layout("SoulSpa")[role]
                document, binary = encoded_geometry(mesh)
                uv_offset = document["bufferViews"][2]["byteOffset"]
                index_offset = document["bufferViews"][3]["byteOffset"]
                for index, (u, v) in enumerate(point for face in layout for point in face["uv"]):
                    struct.pack_into("<ff", binary, uv_offset + index*8, u, 1-v)
                write_glb(path, document, binary)
                check_m3_uv(path, mesh, layout)
                if role == "body":
                    with self.assertRaisesRegex(ValueError, "unsupported sides"):
                        check_m2_uv(path, mesh, layout)
                else:
                    check_m2_uv(path, mesh, layout)
                first = struct.unpack_from("<III", binary, index_offset)
                second = struct.unpack_from("<III", binary, index_offset + 12)
                changes = {
                    "degenerate": (0, 0, 1), "reversed": first[::-1],
                    "duplicate": second, "cross-face": (0, 1, len(mesh.faces[0])),
                    "out-of-range": (0, 1, document["accessors"][0]["count"]),
                }
                for label, triangle in changes.items():
                    with self.subTest(role=role, mutation=label):
                        changed = bytearray(binary)
                        struct.pack_into("<III", changed, index_offset, *triangle)
                        write_glb(path, document, changed)
                        with self.assertRaises(ValueError):
                            check_m3_uv(path, mesh, layout)
                changed = bytearray(binary)
                struct.pack_into("<ff", changed, uv_offset, 0, 0)
                write_glb(path, document, changed)
                with self.assertRaises(ValueError):
                    check_m3_uv(path, mesh, layout)

    def test_every_upper_bone_shaft_has_grain_inside_its_actual_uv_polygon(self):
        from PIL import Image, ImageDraw
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            art.paint("SoulSpa", output, "planes")
            with Image.open(output / "albedo.png") as image:
                planes = image.copy()
            emission = (output / "slot_emissive.png").read_bytes()
            art.paint("SoulSpa", output, "strokes")
            with Image.open(output / "albedo.png") as image:
                strokes = image.copy()
            self.assertEqual(emission, (output / "slot_emissive.png").read_bytes())
            upper = [face for face in art.face_layout("SoulSpa")["body"]
                     if face["surface"] == "bone:shaft" and face["normal"][1] > 0.5]
            self.assertEqual(len(upper), 42)
            missing = []
            for face in upper:
                mask = Image.new("1", planes.size)
                ImageDraw.Draw(mask).polygon(face["polygon_px"], fill=1)
                left, top, right, bottom = mask.getbbox()
                pixels = [(x, y) for y in range(top, bottom) for x in range(left, right)
                          if mask.getpixel((x, y))]
                if not any(planes.getpixel(p) != strokes.getpixel(p) for p in pixels):
                    missing.append(face["cell"])
                self.assertTrue(all(planes.getpixel(p)[3] == strokes.getpixel(p)[3] for p in pixels))
            self.assertEqual(missing, [], "bone grain must reach every upper thin UV face")

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

    def test_restarea_linen_and_repairs_are_distinct_at_atlas_scale(self):
        from PIL import Image
        _, spec = art.contract("RestArea")
        cloth = spec["palette"]["cloth"]
        wood = spec["palette"]["wood"]
        self.assertGreater(sum(cloth) - sum(wood), 200)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            art.paint("RestArea", output, "strokes")
            with Image.open(output / "albedo.png") as image:
                pixels = image.convert("RGB")
                top = [face for face in art.face_layout("RestArea")["body"]
                       if face["surface"] == "cloth:canopy" and face["normal"][1] > 0.5]
                self.assertEqual(len(top), 4)
                colors = set()
                for face in top:
                    x, y = face["cell"] % 16 * 32, face["cell"] // 16 * 32
                    colors.update(pixels.getpixel((px, py))
                                  for px in range(x, x+32) for py in range(y, y+32))
                self.assertIn(tuple(cloth), colors)
                self.assertIn(tuple(min(255, c+24) for c in cloth), colors)
                self.assertIn(tuple(round(c*0.66) for c in cloth), colors)


if __name__ == "__main__":
    unittest.main()
