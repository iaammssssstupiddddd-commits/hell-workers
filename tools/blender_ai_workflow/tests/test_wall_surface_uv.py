from __future__ import annotations

import math
import importlib.util
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from wall_surface_uv import CORE_PACK_RECT, PACK_RECTS, PROFILE, SURFACES, face_uv, validate_surface_uv_glb
import wall_preview_projection as projection
from validate_wall_glb import ContractError
from test_wall_m1_contracts import box_geometry, fixture_document, write_glb


def surface_box(path: Path, mutation: str = "") -> None:
    positions, indices = box_geometry(4.8, 16)
    points, normals, uvs = [], [], []
    for offset in range(0, len(indices), 3):
        triangle = [positions[i] for i in indices[offset:offset + 3]]
        authored = [(x / 32, -z / 32, y / 32) for x, y, z in triangle]
        cap = len({p[2] for p in authored}) == 1
        pairs = [(a, b) for a in authored for b in authored
                 if math.hypot(a[0] - b[0], a[1] - b[1]) > 1e-7]
        a, b = pairs[0]
        if not cap:
            a, b = next((a, b) for a, b in pairs
                        if (a[0] == b[0]) != (a[1] == b[1]))
        for original, point in zip(triangle, authored):
            uv = face_uv("core" if cap else "stone", a[:2], b[:2], point)
            if mutation == "legacy":
                uv = (.03 + .6 * (point[0] + .5), .32 + .64 * (point[1] + .5))
            elif mutation == "decorated_cap" and cap:
                uv = (.03 + .6 * (point[0] + .5), .34 + .6 * (point[1] + .5))
            elif mutation == "warm_cap" and cap:
                uv = (330.5 / 1024 + 16 / 1024 * (point[0] + .5),
                      962.5 / 1024 + 16 / 1024 * (point[1] + .5))
            elif mutation == "stretched_side" and not cap:
                uv = (uv[0], .34 + (uv[1] - .34) * .5)
            elif mutation == "iron_side" and not cap:
                uv = (.68 + .28 * (point[0] + .5), .34 + .28 * (point[2] + .5))
            elif mutation == "purple_side" and not cap:
                uv = (.03 + .22 * (point[0] + .5), .04 + .22 * (point[2] + .5))
            elif mutation == "nan":
                uv = (float("nan"), uv[1])
            points.append(original)
            normals.append((0., 1., 0.))
            uvs.append((uv[0], 1 - uv[1]))
    document, _ = fixture_document(4.8, 16)
    chunks = [b"".join(struct.pack("<3f", *p) for p in points),
              b"".join(struct.pack("<3f", *n) for n in normals),
              b"".join(struct.pack("<2f", *uv) for uv in uvs),
              b"".join(struct.pack("<H", i) for i in range(len(points)))]
    binary = b""
    for view, accessor, chunk in zip(document["bufferViews"], document["accessors"], chunks):
        view.update(byteOffset=len(binary), byteLength=len(chunk))
        accessor["count"] = len(points)
        binary += chunk
        binary += b"\0" * (-len(binary) % 4)
    document["buffers"][0]["byteLength"] = len(binary)
    write_glb(path, document, binary)


class WallSurfaceUvTests(unittest.TestCase):
    def test_projection_constants_match_runtime_contract(self):
        repo = Path(__file__).resolve().parents[3]
        constants = (repo / "crates/hw_core/src/constants/render.rs").read_text()
        world = (repo / "crates/hw_core/src/constants/world.rs").read_text()
        composite = (repo / "crates/bevy_app/src/plugins/startup/rtt_composite.rs").read_text()
        self.assertIn(f"VIEW_HEIGHT: f32 = {projection.VIEW_HEIGHT};", constants)
        self.assertIn(f"Z_OFFSET: f32 = {projection.Z_OFFSET};", constants)
        self.assertIn(f"TILE_SIZE: f32 = {projection.AUTHORING_SCALE};", world)
        self.assertIn("size.y * topdown_rtt_vertical_compensation()", composite)

    def test_preview_material_has_no_metallic_or_specular_tint(self):
        script = Path(__file__).resolve().parents[1] / "scripts/create_wall_production_scene.py"
        spec = importlib.util.spec_from_file_location("wall_scene_for_test", script)
        module = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"bpy": SimpleNamespace()}):
            spec.loader.exec_module(module)
        shader = SimpleNamespace(inputs={
            name: SimpleNamespace(default_value=-1.0)
            for name in ("Roughness", "Metallic", "Specular IOR Level")
        })
        module.configure_surface_shader(shader)
        self.assertEqual(shader.inputs["Roughness"].default_value, 1.0)
        self.assertEqual(shader.inputs["Metallic"].default_value, 0.0)
        self.assertEqual(shader.inputs["Specular IOR Level"].default_value, 0.0)

    def test_both_side_axes_have_equal_nonzero_density(self):
        for surface in ("stone",):
            for a, b in (((-.5, -.15), (.5, -.15)), ((.15, -.5), (.15, .5))):
                low = face_uv(surface, a, b, (*a, -.5))
                along = face_uv(surface, a, b, (*b, -.5))
                high = face_uv(surface, a, b, (*a, .5))
                self.assertAlmostEqual(abs(along[0] - low[0]), high[1] - low[1])
                self.assertGreater(high[1] - low[1], 0)
                self.assertEqual(along[1], low[1])
                self.assertEqual(high[0], low[0])

    def test_short_edges_crop_instead_of_stretching(self):
        for width in (.3, .35, .65, 1.):
            a, b = (-.5, .15), (-.5 + width, .15)
            lo, hi = [face_uv("stone", a, b, (*p, 0)) for p in (a, b)]
            self.assertAlmostEqual(hi[0] - lo[0], SURFACES["stone"][2] * width)

    def test_mid_height_does_not_restart_stone(self):
        a, b = (-.5, .15), (.5, .15)
        samples = [face_uv("stone", a, b, (0, .15, z))[1] for z in (-.5, 0, .5)]
        self.assertAlmostEqual(samples[1] - samples[0], samples[2] - samples[1])

    def test_cap_uses_only_dedicated_core_tile(self):
        u0, v0, scale = SURFACES["core"]
        for x in (-.5, 0, .5):
            for y in (-.5, 0, .5):
                uv = face_uv("core", (0, 0), (0, 0), (x, y, .5))
                self.assertTrue(u0 <= uv[0] <= u0 + scale)
                self.assertTrue(v0 <= uv[1] <= v0 + scale)
        self.assertEqual(scale, 255 / 1024)

    def test_core_packing_cannot_overlap_any_exterior_uv_rectangle(self):
        x, y, w, h = CORE_PACK_RECT
        for name in ("stone",):
            u, v, scale = SURFACES[name]
            left, right = u * 1024, (u + scale) * 1024
            top, bottom = (1 - v - scale) * 1024, (1 - v) * 1024
            self.assertTrue(right < x or left > x + w or bottom < y or top > y + h)

    def test_all_surface_uvs_stay_inside_their_filter_gutters(self):
        self.assertEqual(set(SURFACES), {"stone", "core"})
        for name, (u, v, scale) in SURFACES.items():
            x, y, w, h = PACK_RECTS[name]
            self.assertGreaterEqual(u * 1024, x + 2)
            self.assertLessEqual((u + scale) * 1024, x + w - 2)
            self.assertGreaterEqual((1 - v - scale) * 1024, y + 2)
            self.assertLessEqual((1 - v) * 1024, y + h - 2)

    def test_projection_matches_game_tile_height_and_both_thickness_axes(self):
        for zoom in (1, 5):
            samples = {name: projection.project(point, 256, 192, zoom)
                       for name, point in projection.reference_points().items()}
            projection.validate_samples(samples, 256, 192, zoom)
            x, y = samples["origin"]
            self.assertAlmostEqual(samples["thickness_ns"][0] - x, 9.6 / zoom)
            self.assertAlmostEqual(y - samples["thickness_ew"][1], 9.6 / zoom)
            self.assertAlmostEqual(y - samples["wall_height"][1], 19.2 / zoom)
            samples["tile_y"] = (x, y - 32 / zoom / 1.16619038)
            with self.assertRaises(ValueError):
                projection.validate_samples(samples, 256, 192, zoom)

    def test_invalid_edges_fail_closed(self):
        for end in ((0, 0), (1, 1), (float("nan"), 0)):
            with self.assertRaises(ValueError):
                face_uv("stone", (0, 0), end, (0, 0, 0))

    def test_exported_profile_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "wall.glb"
            surface_box(path)
            report = validate_surface_uv_glb(path, "straight")
            self.assertEqual(report["surface_uv"]["profile"], PROFILE)
            self.assertEqual(report["surface_uv"]["triangles_by_surface"]["core"], 4)

    def test_export_rejects_old_uv_stretch_and_decorated_caps(self):
        for mutation in ("legacy", "decorated_cap", "warm_cap", "stretched_side",
                         "iron_side", "purple_side", "nan"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "wall.glb"
                surface_box(path, mutation)
                with self.assertRaises(ContractError):
                    validate_surface_uv_glb(path, "straight")


if __name__ == "__main__":
    unittest.main()
