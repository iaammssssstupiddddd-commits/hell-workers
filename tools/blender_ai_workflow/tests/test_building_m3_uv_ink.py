"""Illustrated M3 emission isolation and verified-source packaging boundaries."""

import json
from contextlib import ExitStack
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_m3_art as art
import build_building_m3_candidate as builder


class M3InkTests(unittest.TestCase):
    def test_every_actual_uv_vertex_including_bone_caps_and_slots_inverts_to_geometry(
        self,
    ):
        from building_mixer_uv_ink import local_point
        import math

        for kind in art.KINDS:
            meshes = art.geometry(kind)
            for role, faces in art.face_layout(kind).items():
                for face, indices in zip(faces, meshes[role].faces, strict=True):
                    points = [meshes[role].vertices[i] for i in indices]
                    for pixel, expected in zip(face["polygon_px"], points, strict=True):
                        actual = local_point(pixel, face["polygon_px"], points)
                        self.assertTrue(
                            all(
                                math.isclose(a, b, abs_tol=1e-9)
                                for a, b in zip(actual, expected, strict=True)
                            )
                        )
                    x, y = face["cell"] % 16 * 32, face["cell"] // 16 * 32
                    for pixel in ((x, y), (x + 31, y + 31)):
                        self.assertTrue(
                            all(
                                math.isfinite(v)
                                for v in local_point(pixel, face["polygon_px"], points)
                            )
                        )

    def test_new_diffuse_does_not_repaint_worker_emission_or_change_alpha(self):
        for kind in art.KINDS:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                output = Path(directory)
                art.paint(kind, output, "strokes", style="legacy")
                old = (output / "albedo.png").read_bytes()
                emission = (
                    (output / "slot_emissive.png").read_bytes()
                    if kind == "SoulSpa"
                    else None
                )
                art.paint(kind, output, "strokes", style="illustrated-v1")
                current = (output / "albedo.png").read_bytes()
                self.assertNotEqual(old, current)
                with Image.open(output / "albedo.png") as image:
                    self.assertEqual(image.mode, "RGBA")
                    self.assertEqual(image.size, (512, 512))
                    self.assertEqual(image.getchannel("A").getextrema(), (255, 255))
                if emission is not None:
                    self.assertEqual(
                        (output / "slot_emissive.png").read_bytes(), emission
                    )
                art.paint(kind, output, "strokes", style="illustrated-v1")
                self.assertEqual((output / "albedo.png").read_bytes(), current)

    def test_legacy_style_selection_is_unchanged_and_unknown_style_is_rejected(self):
        self.assertEqual(builder.selected_style("RestArea", None), "infernal-v1")
        self.assertEqual(builder.selected_style("SoulSpa", None), "legacy")
        for kind, style in (("SoulSpa", "infernal-v1"), ("RestArea", "arbitrary")):
            with self.assertRaises(ValueError):
                builder.selected_style(kind, style)

    def test_packaging_rejects_after_verify_copy_and_export_input_changes(self):
        for kind in art.KINDS:
            for attack in (
                None,
                "after-verify-atlas",
                "after-verify-report",
                "during-copy-glb",
                "export-source",
                "export-copy",
            ):
                with (
                    self.subTest(kind=kind, attack=attack),
                    tempfile.TemporaryDirectory() as temp,
                    ExitStack() as stack,
                ):
                    root = Path(temp)
                    blend, output, report, reports = (
                        root / "source.blend",
                        root / "output",
                        root / "report.json",
                        root / "reports",
                    )
                    output.mkdir()
                    reports.mkdir()
                    _, spec = art.contract(kind)
                    names = (
                        [f"{role}.glb" for role in spec["roles"]]
                        + [f"{state}.png" for state in art.states(kind)]
                        + ["albedo.png", "catalog.png"]
                        + (["slot_emissive.png"] if kind == "SoulSpa" else [])
                    )
                    report.write_text(json.dumps({"stage": "strokes"}))
                    for path in (blend, *(output / name for name in names)):
                        path.write_bytes(b"synthetic unapproved M3 fixture")
                    verified = {
                        "stage": "strokes",
                        "geometry_contract_sha256": builder.digest(art.CONTRACT),
                        "source_inventory": builder.illustrated_outputs(
                            blend, output, report, reports
                        ),
                        "source_toolchain": {},
                    }
                    stack.enter_context(
                        patch.object(
                            builder,
                            "paths",
                            return_value=(blend, output, report, reports),
                        )
                    )
                    stack.enter_context(
                        patch.object(builder, "illustrated_toolchain", return_value={})
                    )
                    stack.enter_context(
                        patch.object(builder.pipeline, "verify_set", return_value={})
                    )

                    def verify(*_args):
                        if attack == "after-verify-atlas":
                            (output / "albedo.png").write_bytes(b"unverified atlas")
                        if attack == "after-verify-report":
                            report.write_text("{}")
                        return verified.copy()

                    stack.enter_context(
                        patch.object(builder, "verify", side_effect=verify)
                    )
                    original_put = builder.pipeline.put

                    def put(path, payload, **kwargs):
                        original_put(path, payload, **kwargs)
                        if attack == "during-copy-glb" and path.name == "body.glb":
                            (output / "catalog.png").write_bytes(b"changed during copy")

                    stack.enter_context(
                        patch.object(builder.pipeline, "put", side_effect=put)
                    )
                    captured = {}

                    def export(recipe_path, _source, _runtime, _codec, **_kwargs):
                        captured.update(builder.pipeline.read(recipe_path))
                        if attack == "export-source":
                            (output / "body.glb").write_bytes(b"changed during export")
                        if attack == "export-copy":
                            (_source / "body.glb").write_bytes(b"changed copied body")
                        return {"identity": {"kind": kind, "authority": "art_preview"}}

                    stack.enter_context(
                        patch.object(builder.pipeline, "export", side_effect=export)
                    )
                    if attack is None:
                        result = builder.prepare(
                            kind,
                            "synthetic",
                            4,
                            root / "prepared",
                            root / "codec",
                            "illustrated-v1",
                        )
                        self.assertEqual(result["identity"]["authority"], "art_preview")
                        self.assertEqual(
                            captured["surface_paint"], {"method": "uv_ink_v1"}
                        )
                    else:
                        with self.assertRaises(ValueError):
                            builder.prepare(
                                kind,
                                "synthetic",
                                4,
                                root / "prepared",
                                root / "codec",
                                "illustrated-v1",
                            )
