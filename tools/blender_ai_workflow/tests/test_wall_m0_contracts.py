from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageColor, ImageDraw

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
FIXTURES_ROOT = WORKFLOW_ROOT / "fixtures"
SCRIPTS_ROOT = WORKFLOW_ROOT / "scripts"


def load_verifier():
    spec = importlib.util.spec_from_file_location(
        "verify_color_calibration",
        SCRIPTS_ROOT / "verify_color_calibration.py",
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load verify_color_calibration.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WallGeometryContractTests(unittest.TestCase):
    def test_geometry_contract_covers_each_mask_once(self) -> None:
        payload = json.loads(
            (FIXTURES_ROOT / "wall-production-v1.geometry.json").read_text(encoding="utf-8")
        )
        self.assertEqual(payload["schema_version"], 1)
        mappings = payload["topology"]["mappings"]
        masks = [mapping["mask"] for mapping in mappings]
        self.assertEqual(len(masks), 16)
        self.assertEqual(len(set(masks)), 16)
        self.assertEqual(set(masks), {f"{value:04b}" for value in range(16)})
        self.assertEqual(
            {mapping["family"] for mapping in mappings},
            {"isolated", "end", "straight", "corner", "t_junction", "cross"},
        )
        self.assertTrue(all(mapping["quarter_turns_y"] in range(4) for mapping in mappings))

    def test_geometry_contract_freezes_wall_dimensions(self) -> None:
        payload = json.loads(
            (FIXTURES_ROOT / "wall-production-v1.geometry.json").read_text(encoding="utf-8")
        )
        geometry = payload["geometry"]
        self.assertEqual(geometry["tile_size_wu"], 32.0)
        self.assertEqual(geometry["height_wu"], 32.0)
        self.assertEqual(geometry["nominal_and_minimum_continuous_thickness_wu"], 9.6)
        self.assertEqual(geometry["port_width_wu"], 9.6)
        self.assertEqual(geometry["port_half_width_wu"], 4.8)
        self.assertEqual(geometry["ornament_envelope_width_wu"], 12.8)
        self.assertEqual(geometry["port_collar_abs_s_wu"], [8.0, 16.0])
        self.assertEqual(payload["bounds"]["local_min"], [-16.0, -16.0, -16.0])
        self.assertEqual(payload["bounds"]["local_max"], [16.0, 16.0, 16.0])


class WallDensityContractTests(unittest.TestCase):
    def test_density_contract_has_exact_n_and_4n_distribution(self) -> None:
        payload = json.loads(
            (FIXTURES_ROOT / "wall-density-v1.json").read_text(encoding="utf-8")
        )
        self.assertEqual(payload["schema_version"], 1)
        self.assertEqual(payload["contract_id"], "wall-density-v1")
        masks = payload["fixture"]["masks"]
        self.assertEqual(masks, [f"{value:04b}" for value in range(16)])
        cases = {case["target_size"]: case for case in payload["cases"]}
        self.assertEqual(cases["N"]["mask_repetitions"] * len(masks), 96)
        self.assertEqual(cases["N"]["target_wall_count"], 96)
        self.assertEqual(cases["N"]["connector_count"], 192)
        self.assertEqual(cases["N"]["perf_size"], "small")
        self.assertEqual(cases["4N"]["mask_repetitions"] * len(masks), 384)
        self.assertEqual(cases["4N"]["target_wall_count"], 384)
        self.assertEqual(cases["4N"]["connector_count"], 768)
        self.assertEqual(cases["4N"]["perf_size"], "medium")
        self.assertEqual(payload["fixture"]["grid_layout"]["origin"], [2, 2])
        self.assertEqual(payload["fixture"]["grid_layout"]["stride"], [5, 5])
        self.assertEqual(payload["fixture"]["grid_layout"]["columns"], 20)
        self.assertEqual(payload["fixture"]["camera_scale"], 5.0)
        self.assertEqual(payload["fixture"]["target_phases"], ["completed", "provisional"])
        self.assertEqual(payload["environment"]["seed"], 20260901)
        self.assertEqual(payload["environment"]["runs"], 3)
        self.assertEqual(payload["environment"]["warmup_seconds"], 30)
        self.assertEqual(payload["environment"]["measure_seconds"], 60)


class WallColorCalibrationVerifierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = load_verifier()
        cls.contract_path = FIXTURES_ROOT / "wall-color-calibration-v1.json"
        cls.contract = json.loads(cls.contract_path.read_text(encoding="utf-8"))

    def render_fixture_image(self, path: Path, *, candidate_shift: int = 0) -> None:
        image_spec = self.contract["image"]
        image = Image.new("RGB", (image_spec["width"], image_spec["height"]), "black")
        draw = ImageDraw.Draw(image)
        patch_size = image_spec["patch_size_px"]
        half = patch_size // 2
        for patch in self.contract["base_patches"]:
            color = patch["input_hex_srgb"]
            if candidate_shift:
                values = [max(0, min(255, value + candidate_shift)) for value in ImageColor.getrgb(color)]
                color = tuple(values)
            center_x, center_y = patch["center_px"]
            draw.rectangle(
                (center_x - half, center_y - half, center_x + half - 1, center_y + half - 1),
                fill=color,
            )
        emissive = self.contract["emissive_sanity"]
        center_x, center_y = emissive["center_px"]
        draw.rectangle(
            (center_x - half, center_y - half, center_x + half - 1, center_y + half - 1),
            fill="#ff00ff",
        )
        image.save(path, format="PNG")

    def metadata(self, image_path: Path, renderer: str) -> dict[str, object]:
        pipeline = self.contract["color_pipeline"]
        patch_inputs = {
            patch["id"]: patch["input_hex_srgb"] for patch in self.contract["base_patches"]
        }
        emissive = self.contract["emissive_sanity"]
        patch_inputs[emissive["id"]] = emissive["input_hex_srgb"]
        metadata: dict[str, object] = {
            "schema_version": 1,
            "contract_id": self.contract["contract_id"],
            "renderer": renderer,
            "source_fingerprint": "fixture-source",
            "capture": {
                "encoded_format": "png",
                "height": self.contract["image"]["height"],
                "image_sha256": self.verifier.sha256_file(image_path),
                "rescaled": False,
                "srgb": True,
                "width": self.contract["image"]["width"],
            },
            "color": {
                "automatic_exposure": pipeline["automatic_exposure"],
                "display_device": pipeline["display_device"],
                "exposure": pipeline["exposure"],
                "gamma": pipeline["gamma"],
                "look": pipeline["look"],
                "tonemapping": pipeline["tonemapping"],
                "view_transform": pipeline["reference_view_transform"],
            },
            "patch_inputs": patch_inputs,
        }
        if renderer == "blender":
            metadata["ocio"] = {
                "config_path": "/fixture/config.ocio",
                "config_sha256": "a" * 64,
                "fallback": False,
                "runtime_version": "2.4.2",
            }
        return metadata

    def write_metadata(self, path: Path, payload: dict[str, object]) -> None:
        path.write_text(json.dumps(payload), encoding="utf-8")

    def test_ciede2000_matches_published_reference_vectors(self) -> None:
        vectors = [
            ((50.0, 2.6772, -79.7751), (50.0, 0.0, -82.7485), 2.0425),
            ((50.0, 3.1571, -77.2803), (50.0, 0.0, -82.7485), 2.8615),
            ((50.0, 2.8361, -74.0200), (50.0, 0.0, -82.7485), 3.4412),
            ((50.0, -1.3802, -84.2814), (50.0, 0.0, -82.7485), 1.0000),
        ]
        for first, second, expected in vectors:
            with self.subTest(expected=expected):
                self.assertAlmostEqual(self.verifier.delta_e_2000(first, second), expected, places=4)

    def test_identical_pngs_pass_with_positive_ocio_proof(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference = root / "reference.png"
            candidate = root / "candidate.png"
            reference_metadata = root / "reference.json"
            candidate_metadata = root / "candidate.json"
            self.render_fixture_image(reference)
            self.render_fixture_image(candidate)
            self.write_metadata(reference_metadata, self.metadata(reference, "blender"))
            self.write_metadata(candidate_metadata, self.metadata(candidate, "bevy-client"))

            report = self.verifier.verify_calibration(
                contract_path=self.contract_path,
                reference_path=reference,
                reference_metadata_path=reference_metadata,
                candidate_path=candidate,
                candidate_metadata_path=candidate_metadata,
            )

            self.assertEqual(report["status"], "pass")
            self.assertEqual(report["base_color_gate"]["mean_delta_e_2000"], 0.0)
            self.assertTrue(report["emissive_sanity_gate"]["passed"])

    def test_reference_ocio_fallback_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference = root / "reference.png"
            candidate = root / "candidate.png"
            reference_metadata = root / "reference.json"
            candidate_metadata = root / "candidate.json"
            self.render_fixture_image(reference)
            self.render_fixture_image(candidate)
            metadata = self.metadata(reference, "blender")
            metadata["ocio"]["fallback"] = True
            self.write_metadata(reference_metadata, metadata)
            self.write_metadata(candidate_metadata, self.metadata(candidate, "bevy-client"))

            with self.assertRaisesRegex(self.verifier.ContractError, "fallback must be false"):
                self.verifier.verify_calibration(
                    contract_path=self.contract_path,
                    reference_path=reference,
                    reference_metadata_path=reference_metadata,
                    candidate_path=candidate,
                    candidate_metadata_path=candidate_metadata,
                )


if __name__ == "__main__":
    unittest.main()
