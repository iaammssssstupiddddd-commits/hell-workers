"""UV seams, physical rotation marks and unchanged numeric/UV layout boundaries."""

import math
import json
from contextlib import ExitStack
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_mixer_uv_ink as ink
import building_m2_art as art
import build_building_m2_candidate as builder


class MixerInkTests(unittest.TestCase):
    def test_node_startup_and_search_path_injection_are_removed(self):
        with patch.object(builder.shutil, "which", return_value="/declared/bin/node"), patch.dict(
            builder.os.environ,
            {"NODE_OPTIONS": "--require /untrusted.js", "NODE_PATH": "/untrusted"},
        ):
            environment = builder.illustrated_environment()
        self.assertNotIn("NODE_OPTIONS", environment)
        self.assertNotIn("NODE_PATH", environment)

    def test_prepare_rejects_changes_after_verify_during_copy_and_during_export(self):
        for attack in (
            None,
            "after-verify-atlas",
            "after-verify-report",
            "during-copy-glb",
            "export-source",
            "export-copy",
        ):
            with (
                self.subTest(attack=attack),
                tempfile.TemporaryDirectory() as temp,
                ExitStack() as stack,
            ):
                root = Path(temp)
                blend, report, output, reports = (
                    root / "source.blend",
                    root / "report.json",
                    root / "output",
                    root / "reports",
                )
                output.mkdir()
                reports.mkdir()
                report.write_text(json.dumps({"renders": {"IdleAngleZero": {}}}))
                for path in (
                    blend,
                    output / "albedo.png",
                    output / "body.glb",
                    output / "rotor.glb",
                    output / "IdleAngleZero.png",
                ):
                    path.write_bytes(b"synthetic unapproved source fixture")
                numeric, numeric_sha = builder.numeric_contract("MudMixer", "v3")
                verified = {
                    "stage": "strokes",
                    "numeric_candidate_sha256": numeric_sha,
                    "source_inventory": builder.illustrated_outputs(
                        blend, output, report, reports
                    ),
                    "source_toolchain": {},
                }
                destination = root / "prepared"
                stack.enter_context(
                    patch.object(
                        builder, "paths", return_value=(blend, output, report, reports)
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
                        (output / "albedo.png").write_bytes(b"unverified new atlas")
                    if attack == "after-verify-report":
                        report.write_text(json.dumps({"renders": {}}))
                    return verified.copy()

                stack.enter_context(patch.object(builder, "verify", side_effect=verify))
                original_put = builder.pipeline.put

                def put(path, payload, **kwargs):
                    original_put(path, payload, **kwargs)
                    if attack == "during-copy-glb" and path.name == "body.glb":
                        (output / "rotor.glb").write_bytes(
                            b"changed after verification"
                        )

                stack.enter_context(
                    patch.object(builder.pipeline, "put", side_effect=put)
                )

                def export(*_args, **_kwargs):
                    if attack == "export-source":
                        (output / "body.glb").write_bytes(
                            b"changed during codec export"
                        )
                    if attack == "export-copy":
                        (destination / "authoring/albedo.png").write_bytes(
                            b"changed copied atlas"
                        )
                    return {"identity": {"test_fixture_only": True}}

                exporter = stack.enter_context(
                    patch.object(builder.pipeline, "export", side_effect=export)
                )
                if attack is None:
                    result = builder.prepare(
                        "MudMixer",
                        "fixture",
                        4,
                        destination,
                        root / "codec",
                        "v3",
                        ink.STYLE,
                    )
                    self.assertFalse(result["production_candidate"])
                    self.assertEqual(
                        builder.pipeline.read(destination / "recipe.json")[
                            "surface_paint"
                        ],
                        {"method": "uv_ink_v1"},
                    )
                else:
                    with self.assertRaises(ValueError):
                        builder.prepare(
                            "MudMixer",
                            "fixture",
                            4,
                            destination,
                            root / "codec",
                            "v3",
                            ink.STYLE,
                        )
                    if attack.startswith("after-verify"):
                        exporter.assert_not_called()

    def test_same_surface_coordinates_cross_face_and_angle_seams_without_face_lighting(
        self,
    ):
        for angle in (-math.pi, -0.9, 0.2, math.pi):
            point = (14 * math.cos(angle), -4, 14 * math.sin(angle))
            self.assertEqual(
                ink.color("rotor", "drum:band1", point),
                ink.color("rotor", "drum:band2", point),
            )
        point = (17, 16, 0)
        self.assertEqual(
            ink.color("body", "frame:left", point),
            ink.color("body", "frame:front", point),
        )
        a, b = (
            (14 * math.cos(t), 1, 14 * math.sin(t))
            for t in (-math.pi + 1e-9, math.pi - 1e-9)
        )
        self.assertEqual(
            ink.color("rotor", "drum:band2", a), ink.color("rotor", "drum:band2", b)
        )

    def test_one_chalk_mark_is_not_half_turn_symmetric(self):
        def at(angle):
            return ink.color(
                "rotor", "drum:band2", (14 * math.cos(angle), 0, 14 * math.sin(angle))
            )

        self.assertEqual(at(-1.0), (*ink.palette.WORN_EDGE, 255))
        self.assertNotEqual(at(-1.0), at(-1.0 + math.pi))

    def test_face_inverse_and_full_cell_gutters_keep_the_existing_uv_mapping(self):
        triangle = [(3, 4), (15, 4), (3, 16)]
        points = [(0, 0, 0), (12, 0, 0), (0, 0, 12)]
        self.assertEqual(ink.local_point((9, 10), triangle, points), (6.0, 0.0, 6.0))
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "ink.png"
            before = art.face_layout("MudMixer", "v3")
            self.assertEqual(ink.paint(path), before)
            with Image.open(path) as image:
                self.assertEqual(image.size, (512, 512))
                self.assertEqual(image.getchannel("A").getextrema(), (255, 255))
                first = before["rotor"][12]
                mesh = art.candidate_geometry("MudMixer", "v3")["rotor"]
                physical = [mesh.vertices[i] for i in mesh.faces[12]]
                x, y = first["cell"] % 16 * 32, first["cell"] // 16 * 32
                # Corners outside the polygon are intentionally painted, not left
                # as black/transparent background which filtering would expose.
                self.assertEqual(
                    image.getpixel((x, y)),
                    ink.color(
                        "rotor",
                        first["surface"],
                        ink.local_point(
                            (x + 0.5, y + 0.5), first["polygon_px"], physical
                        ),
                    ),
                )

    def test_foreign_kind_revision_and_unknown_surface_reject_before_launch(self):
        with patch.object(builder.subprocess, "run") as launch:
            for kind, revision in (("Tank", "v1"), ("MudMixer", "v1")):
                with self.assertRaises(ValueError):
                    builder.build_bundle(
                        kind, "not-launched", "strokes", revision, ink.STYLE
                    )
                with self.assertRaises(ValueError):
                    art.paint(
                        kind,
                        Path("/not-created.png"),
                        revision=revision,
                        style=ink.STYLE,
                    )
            launch.assert_not_called()
        with self.assertRaises(ValueError):
            ink.color("rotor", "foreign:band0", (0, 0, 0))

    def test_output_inventory_detects_external_validator_mutations(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            output, reports = root / "output", root / "reports"
            output.mkdir()
            reports.mkdir()
            blend, report = root / "source.blend", root / "report.json"
            for path in (
                blend,
                report,
                output / "body.glb",
                reports / "body.export.json",
            ):
                path.write_bytes(b"initial immutable fixture")
            before = builder.illustrated_outputs(blend, output, report, reports)
            (output / "body.glb").write_bytes(b"changed by external verifier")
            self.assertNotEqual(
                before, builder.illustrated_outputs(blend, output, report, reports)
            )
