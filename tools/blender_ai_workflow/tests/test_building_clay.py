"""Pilot geometry and post-export gates; no Blender or runtime asset mirror required."""

from __future__ import annotations

import copy
import json
import math
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from building_clay_geometry import build, face_uv, load_pilot, read_json
import build_building_clay as bundle
from check_building_art_contract import REPO, check
from validate_building_clay_glb import validate
from validate_wall_glb import ContractError

WORKFLOW = Path(__file__).resolve().parents[1]


def encoded_geometry(geometry):
    points, normals, uvs, indices = [], [], [], []
    for face in geometry.faces:
        polygon = [geometry.vertices[i] for i in face]
        a, b, c = polygon[:3]
        u = [b[i] - a[i] for i in range(3)]
        v = [c[i] - a[i] for i in range(3)]
        cross = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2],
                 u[0] * v[1] - u[1] * v[0])
        length = math.sqrt(sum(x * x for x in cross))
        start = len(points)
        points.extend(polygon)
        normals.extend([tuple(x / length for x in cross)] * len(face))
        uvs.extend(face_uv(polygon))
        for i in range(1, len(face) - 1):
            indices.extend((start, start + i, start + i + 1))
    binary = bytearray()
    views, accessors = [], []
    for values, fmt, shape, component in ((points, "fff", "VEC3", 5126),
                                         (normals, "fff", "VEC3", 5126),
                                         (uvs, "ff", "VEC2", 5126),
                                         ([(i,) for i in indices], "I", "SCALAR", 5125)):
        data = b"".join(struct.pack("<" + fmt, *row) for row in values)
        views.append({"buffer": 0, "byteOffset": len(binary), "byteLength": len(data)})
        accessors.append({"bufferView": len(views) - 1, "componentType": component,
                          "type": shape, "count": len(values)})
        binary.extend(data)
    return {"asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}], "meshes": [{"primitives": [{
                "attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": 3}]}],
            "buffers": [{"byteLength": len(binary)}], "bufferViews": views, "accessors": accessors}, binary


def write_glb(path, document, binary):
    data = json.dumps(document).encode()
    data += b" " * (-len(data) % 4)
    binary = bytes(binary) + b"\0" * (-len(binary) % 4)
    path.write_bytes(struct.pack("<4sII", b"glTF", 2, 28 + len(data) + len(binary)) +
                     struct.pack("<II", len(data), 0x4E4F534A) + data +
                     struct.pack("<II", len(binary), 0x004E4942) + binary)


class BuildingClayTests(unittest.TestCase):
    def test_inventory_matches_current_rust_shapes_and_all_preview_entrypoints(self):
        self.assertEqual(check(REPO)["kinds"], 10)

    def test_all_four_roles_roundtrip_post_export_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "role.glb"
            for kind in ("Tank", "MudMixer"):
                for role, geometry in build(kind, load_pilot(kind)).items():
                    with self.subTest(kind=kind, role=role):
                        write_glb(path, *encoded_geometry(geometry))
                        result = validate(path, kind, role)
                        self.assertEqual(result["triangles"], geometry.triangles())
                        self.assertEqual(result["evidence_kind"], "technical_clay_only")

    def test_shells_have_outward_winding_and_no_open_edges(self):
        for kind in ("Tank", "MudMixer"):
            for role, geometry in build(kind, load_pilot(kind)).items():
                with self.subTest(kind=kind, role=role):
                    volume = 0
                    edges = {}
                    for face in geometry.faces:
                        for a, b in zip(face, face[1:] + face[:1], strict=True):
                            key = tuple(sorted((a, b)))
                            edges[key] = edges.get(key, 0) + (1 if a < b else -1)
                        a = geometry.vertices[face[0]]
                        for i in range(1, len(face) - 1):
                            b, c = geometry.vertices[face[i]], geometry.vertices[face[i + 1]]
                            volume += a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0])
                    self.assertGreater(volume, 0)
                    self.assertTrue(all(count == 0 for count in edges.values()))

    def test_tank_water_states_fit_real_cavity(self):
        contract = load_pilot("Tank")
        self.assertFalse(contract["states"]["Empty"]["water_visible"])
        for state in ("Partial", "Full"):
            y = contract["states"][state]["water_y_wu"]
            self.assertGreater(y - 0.25, 4)
            self.assertLess(y + 0.25, 28)
            # Concentric, identically oriented regular polygons, not unrelated circles.
            config = contract["construction"]
            inner = config["inner_radius_bottom"] + (y - 0.25 - 4) / 24 * (
                config["inner_radius_top"] - config["inner_radius_bottom"])
            gap = inner - config["water_radius"]
            self.assertGreater(gap, 0)
            self.assertLessEqual(gap, 0.1, "water must not look like a floating disk")

    def test_mixer_entire_rotor_sweep_clears_trough_and_meets_support(self):
        contract = load_pilot("MudMixer")
        rotor = build("MudMixer", contract)["rotor"]
        for degree in range(360):
            angle = math.radians(degree)
            for x, y, z in rotor.vertices:
                rx, rz = x * math.cos(angle) + z * math.sin(angle), -x * math.sin(angle) + z * math.cos(angle)
                self.assertLess(abs(rx), 22)
                self.assertLess(abs(rz), 20)
                self.assertGreater(y + 28, 12)
                self.assertLessEqual(y + 28, 42)
        self.assertEqual(max(y for _, y, _ in rotor.vertices) + 28, 42)

    def test_unsafe_glb_variants_fail_closed(self):
        geometry = build("Tank", load_pilot("Tank"))["body"]
        original, binary = encoded_geometry(geometry)
        mutations = {
            "translation": lambda d: d["nodes"][0].update(translation=[0, 1, 0]),
            "scale": lambda d: d["nodes"][0].update(scale=[32, 32, 32]),
            "children": lambda d: d["nodes"][0].update(children=[0]),
            "animation": lambda d: d.update(animations=[{}]),
            "skin": lambda d: d.update(skins=[{}]),
            "image": lambda d: d.update(images=[{}]),
            "morph": lambda d: d["meshes"][0]["primitives"][0].update(targets=[{}]),
            "multi_primitive": lambda d: d["meshes"][0]["primitives"].append({}),
            "line_mode": lambda d: d["meshes"][0]["primitives"][0].update(mode=1),
            "bad_uv": lambda d: d["accessors"][2].update(type="VEC3"),
            "external_buffer": lambda d: d["buffers"][0].update(uri="elsewhere.bin"),
            "missing_normal": lambda d: d["meshes"][0]["primitives"][0]["attributes"].pop("NORMAL"),
            "sparse": lambda d: d["accessors"][0].update(sparse={}),
            "bad_index_accessor": lambda d: d["meshes"][0]["primitives"][0].update(indices=-1),
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.glb"
            for name, mutate in mutations.items():
                with self.subTest(name=name):
                    doc = copy.deepcopy(original)
                    mutate(doc)
                    write_glb(path, doc, binary)
                    with self.assertRaises((ValueError, ContractError)):
                        validate(path, "Tank", "body")

    def test_nonfinite_vertices_wrong_units_zero_normals_and_zero_uv_fail(self):
        doc, binary = encoded_geometry(build("Tank", load_pilot("Tank"))["body"])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.glb"
            for offset, value in ((0, float("nan")), (0, 3200),
                                  (doc["bufferViews"][1]["byteOffset"] + 4, 0)):
                changed = bytearray(binary)
                struct.pack_into("<f", changed, offset, value)
                write_glb(path, doc, changed)
                with self.assertRaises((ValueError, ContractError)):
                    validate(path, "Tank", "body")
            changed = bytearray(binary)
            view = doc["bufferViews"][2]
            changed[view["byteOffset"]:view["byteOffset"] + view["byteLength"]] = b"\0" * view["byteLength"]
            write_glb(path, doc, changed)
            with self.assertRaisesRegex(ValueError, "degenerate UV"):
                validate(path, "Tank", "body")

    def test_draft_never_represents_a_frozen_art_contract(self):
        for kind in ("Tank", "MudMixer"):
            self.assertIsNone(load_pilot(kind)["final_art"])
            with self.assertRaises(ValueError):
                load_pilot("Door")

    def test_duplicate_and_nonfinite_contract_fields_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            for content in ('{"kind":1,"kind":2}', '{"height":NaN}'):
                path.write_text(content)
                with self.assertRaises(ValueError):
                    read_json(path)


class ClayBundleTests(unittest.TestCase):
    def create_bundle(self, name):
        kind = "Tank"
        blend, output, report, reports = bundle.paths(kind, name)
        for path in (blend.parent, output, report.parent, reports):
            path.mkdir(parents=True, exist_ok=True)
        blend.write_bytes(b"test source")
        Image.new("RGB", (16, 16), (150, 150, 150)).save(output / "albedo.png")
        contract = load_pilot(kind)
        payload = {"schema_version": 1, "kind": kind, "evidence_kind": "blender_clay_only",
                   "runtime_authority": None, "art_approval": None,
                   "blend": str(blend), "blend_sha256": bundle.digest(blend),
                   "generator_sha256": bundle.digest(WORKFLOW / "scripts/create_building_clay_scene.py"),
                   "geometry_generator_sha256": bundle.digest(WORKFLOW / "scripts/building_clay_geometry.py"),
                   "geometry_contract_sha256": bundle.digest(WORKFLOW / "fixtures/building-tank-v1.geometry.json"),
                   "ocio": {"fallback": False}, "preview": contract["preview"], "roles": {}, "renders": {},
                   "albedo": {"path": str(output / "albedo.png"), "sha256": bundle.digest(output / "albedo.png")}}
        for state in ("Empty", "Partial", "Full"):
            path = output / f"{state}.png"
            image = Image.new("RGBA", (256, 256))
            image.paste((150, 150, 150, 255), (64, 64, 192, 192))
            image.save(path)
            payload["renders"][state] = {"path": str(path), "sha256": bundle.digest(path),
                "projection": {"all_visible_vertices_in_canvas": True,
                    "samples_px": [[128, 160], [128 + 32 * 256 / 96, 160],
                                   [128, 160 - 19.2 * 256 / 96], [128, 160 + 32 * 256 / 96]]}}
        payload["catalog"] = payload["renders"]["Empty"]
        for role, geometry in build(kind, contract).items():
            glb = output / f"{role}.glb"
            write_glb(glb, *encoded_geometry(geometry))
            payload["roles"][role] = {}
            (reports / f"{role}.glb.export.json").write_text(json.dumps({
                "status": "exported", "sha256": bundle.digest(glb), "bytes": glb.stat().st_size,
                "source_blend": str(blend), "output": str(glb), "collection": contract["roles"][role]["collection"],
                "geometry_scale": 32, "materials_mode": "placeholder",
                "validation": {"summary": {"errors": 0}, "mesh_count": 1}}))
        report.write_text(json.dumps(payload))
        return report, payload

    def test_verify_binds_bytes_sources_states_and_rechecks_khronos(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, HELL_WORKERS_ASSET_ROOT=root):
            self.create_bundle("clay-test")
            with patch.object(bundle.subprocess, "run") as run:
                self.assertEqual(bundle.verify("Tank", "clay-test")["status"], "pass")
                self.assertEqual(run.call_count, 2)

    def test_stale_or_false_authority_report_is_rejected(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, HELL_WORKERS_ASSET_ROOT=root):
            report, payload = self.create_bundle("clay-test")
            for key, value in (("runtime_authority", "release_approved"), ("art_approval", {}),
                               ("generator_sha256", "0" * 64), ("blend_sha256", "0" * 64),
                               ("renders", {}), ("catalog", {})):
                with self.subTest(key=key):
                    altered = copy.deepcopy(payload)
                    altered[key] = value
                    report.write_text(json.dumps(altered))
                    with self.assertRaises(ValueError):
                        bundle.verify("Tank", "clay-test")

    def test_no_clobber_and_path_escape_before_any_subprocess(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, HELL_WORKERS_ASSET_ROOT=root):
            self.create_bundle("clay-test")
            with patch.object(bundle.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "already used"):
                    bundle.build_bundle("Tank", "clay-test")
                with self.assertRaises(ValueError):
                    bundle.paths("Tank", "../escape")
                run.assert_not_called()

    def test_source_or_image_byte_change_is_rejected(self):
        with tempfile.TemporaryDirectory() as root, patch.dict(os.environ, HELL_WORKERS_ASSET_ROOT=root):
            self.create_bundle("clay-test")
            blend, output, _, _ = bundle.paths("Tank", "clay-test")
            source = blend.read_bytes()
            blend.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "blend identity"):
                bundle.verify("Tank", "clay-test")
            blend.write_bytes(source)
            (output / "Full.png").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "state image"):
                bundle.verify("Tank", "clay-test")


if __name__ == "__main__":
    unittest.main()
