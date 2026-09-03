from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from xml.etree import ElementTree

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


def load_reference_locator_verifier():
    spec = importlib.util.spec_from_file_location(
        "verify_wall_reference_locators",
        SCRIPTS_ROOT / "verify_wall_reference_locators.py",
    )
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load verify_wall_reference_locators.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WallGeometryContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.contract_path = FIXTURES_ROOT / "wall-production-v1.geometry.json"
        cls.payload = json.loads(cls.contract_path.read_text(encoding="utf-8"))

    def test_geometry_contract_covers_each_mask_once(self) -> None:
        payload = self.payload
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
        payload = self.payload
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

    def test_geometry_contract_rejects_redundant_edge_subdivision(self) -> None:
        mesh = self.payload["mesh_contract"]
        self.assertEqual(mesh["target_triangle_range_per_mesh"], [24, 72])
        self.assertEqual(mesh["hard_triangle_cap_per_mesh"], 72)

    def test_bounds_contract_distinguishes_envelope_pivot_and_placement(self) -> None:
        bounds = self.payload["bounds"]
        self.assertEqual(bounds["contract_kind"], "maximum_cell_envelope")
        self.assertEqual(bounds["origin"], [0.0, 0.0, 0.0])
        self.assertEqual(bounds["required_vertical_min_max_wu"], [-16.0, 16.0])
        self.assertEqual(bounds["horizontal_boundary_planes_wu"], [-16.0, 16.0])
        self.assertEqual(bounds["placement_center_y_wu"], 16.0)
        self.assertEqual(bounds["world_y_after_placement"], [0.0, 32.0])
        self.assertEqual(
            [
                bounds["required_vertical_min_max_wu"][0]
                + bounds["placement_center_y_wu"],
                bounds["required_vertical_min_max_wu"][1]
                + bounds["placement_center_y_wu"],
            ],
            bounds["world_y_after_placement"],
        )
        self.assertEqual(
            bounds["node_transform"],
            {
                "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0],
                "translation": [0.0, 0.0, 0.0],
            },
        )

    def test_positive_y_rotation_derives_all_topology_mappings(self) -> None:
        orientation = self.payload["canonical_orientation"]
        cycle = orientation["direction_cycle_positive_quarter_turn"]
        self.assertEqual(cycle, ["N", "W", "S", "E"])
        self.assertEqual(orientation["positive_quarter_turn_axis"], "+Y")
        self.assertEqual(orientation["positive_quarter_turn_degrees"], 90)
        self.assertEqual(
            orientation["positive_quarter_turn_top_view"], "counterclockwise"
        )
        canonical = {
            entry["family"]: entry for entry in orientation["canonical_families"]
        }
        self.assertEqual(
            set(canonical),
            {"isolated", "end", "straight", "corner", "t_junction", "cross"},
        )
        bit_order = self.payload["topology"]["mask_bit_order"]
        for mapping in self.payload["topology"]["mappings"]:
            family = canonical[mapping["family"]]
            turns = mapping["quarter_turns_y"]
            rotated = {
                cycle[(cycle.index(direction) + turns) % len(cycle)]
                for direction in family["arms"]
            }
            derived_mask = "".join(
                "1" if direction in rotated else "0" for direction in bit_order
            )
            self.assertEqual(derived_mask, mapping["mask"], mapping)

    def test_orientation_svg_is_bound_to_the_geometry_contract(self) -> None:
        svg_path = FIXTURES_ROOT / "wall-production-v1.orientation.svg"
        root = ElementTree.fromstring(svg_path.read_text(encoding="utf-8"))
        contract_sha256 = hashlib.sha256(self.contract_path.read_bytes()).hexdigest()
        self.assertEqual(root.attrib["data-contract-id"], self.payload["asset_set_id"])
        self.assertEqual(root.attrib["data-contract-sha256"], contract_sha256)
        namespace = {"svg": "http://www.w3.org/2000/svg"}
        groups = {
            group.attrib["data-family"]: group
            for group in root.findall("svg:g", namespace)
            if "data-family" in group.attrib
        }
        expected = {
            entry["family"]: entry
            for entry in self.payload["canonical_orientation"]["canonical_families"]
        }
        self.assertEqual(set(groups), set(expected))
        for family, entry in expected.items():
            self.assertEqual(groups[family].attrib["data-canonical-mask"], entry["mask"])
            self.assertEqual(groups[family].attrib["data-arms"].split(), entry["arms"])
        bounds_views = [
            group
            for group in root.findall("svg:g", namespace)
            if group.attrib.get("data-view") == "bounds-and-pivot"
        ]
        self.assertEqual(len(bounds_views), 1)


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


class WallReferenceLocatorContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = load_reference_locator_verifier()
        cls.contract = json.loads(
            (FIXTURES_ROOT / "wall-reference-locators-v1.json").read_text(
                encoding="utf-8"
            )
        )

    def test_contract_keeps_historical_and_current_roles_disjoint(self) -> None:
        self.assertEqual(self.contract["schema_version"], 1)
        self.assertEqual(self.contract["contract_id"], "wall-reference-locators-v1")
        historical = self.contract["historical_p02"]
        current = self.contract["current_wall"]
        self.assertEqual(historical["role"], "historical-p02-presentation-contract")
        self.assertIn("current-wall-pixels", historical["not_valid_for"])
        self.assertEqual(current["role"], "current-fallback-wall-visual-reference")
        self.assertIn("historical-p02-performance", current["not_valid_for"])
        self.assertIn("production-wall-art-approval", current["not_valid_for"])

    def test_checksum_ledger_rejects_duplicate_locators(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            ledger = Path(directory) / "SHA256SUMS"
            digest = "a" * 64
            ledger.write_text(
                f"{digest}  evidence.json\n{digest}  evidence.json\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                self.verifier.ContractError, "duplicate checksum locator"
            ):
                self.verifier.parse_checksum_ledger(ledger)

    def test_relative_path_rejects_parent_traversal(self) -> None:
        with self.assertRaisesRegex(self.verifier.ContractError, "stay below its root"):
            self.verifier.relative_path("../evidence.json", "fixture")


class WallColorCalibrationVerifierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = load_verifier()
        cls.contract_path = FIXTURES_ROOT / "wall-color-calibration-v1.json"
        cls.contract = json.loads(cls.contract_path.read_text(encoding="utf-8"))

    def test_wall_ocio_config_freezes_exact_srgb_transfer(self) -> None:
        config = (FIXTURES_ROOT / "wall-calibration-v2.ocio").read_text(
            encoding="utf-8"
        )
        self.assertIn("ocio_profile_version: 2.1", config)
        self.assertIn("strictparsing: true", config)
        self.assertIn("active_displays: [sRGB]", config)
        self.assertIn("active_views: [Standard]", config)
        self.assertIn(
            "from_scene_reference: !<ExponentWithLinearTransform> "
            "{gamma: 2.4, offset: 0.055, direction: inverse}",
            config,
        )

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
            metadata["blender_projection"] = {
                "horizontal_world_span": self.contract["image"]["width"],
                "ortho_scale": self.contract["image"]["width"],
                "type": "orthographic",
                "vertical_world_span": self.contract["image"]["height"],
            }
            metadata["ocio"] = {
                "active_config_cache_id": "fixture-cache-id",
                "active_config_matches": True,
                "config_cache_id": "fixture-cache-id",
                "config_path": "/fixture/config.ocio",
                "config_sha256": "a" * 64,
                "config_version": "2.1",
                "fallback": False,
                "runtime_version": "2.4.2",
                "validation_status": "pass",
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

    def test_reference_ocio_active_config_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference = root / "reference.png"
            candidate = root / "candidate.png"
            reference_metadata = root / "reference.json"
            candidate_metadata = root / "candidate.json"
            self.render_fixture_image(reference)
            self.render_fixture_image(candidate)
            metadata = self.metadata(reference, "blender")
            metadata["ocio"]["active_config_cache_id"] = "unexpected-cache-id"
            self.write_metadata(reference_metadata, metadata)
            self.write_metadata(candidate_metadata, self.metadata(candidate, "bevy-client"))

            with self.assertRaisesRegex(
                self.verifier.ContractError, "active config must match"
            ):
                self.verifier.verify_calibration(
                    contract_path=self.contract_path,
                    reference_path=reference,
                    reference_metadata_path=reference_metadata,
                    candidate_path=candidate,
                    candidate_metadata_path=candidate_metadata,
                )

    def test_reference_blender_projection_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reference = root / "reference.png"
            candidate = root / "candidate.png"
            reference_metadata = root / "reference.json"
            candidate_metadata = root / "candidate.json"
            self.render_fixture_image(reference)
            self.render_fixture_image(candidate)
            metadata = self.metadata(reference, "blender")
            metadata["blender_projection"]["ortho_scale"] = self.contract["image"][
                "height"
            ]
            self.write_metadata(reference_metadata, metadata)
            self.write_metadata(candidate_metadata, self.metadata(candidate, "bevy-client"))

            with self.assertRaisesRegex(
                self.verifier.ContractError, "projection differs"
            ):
                self.verifier.verify_calibration(
                    contract_path=self.contract_path,
                    reference_path=reference,
                    reference_metadata_path=reference_metadata,
                    candidate_path=candidate,
                    candidate_metadata_path=candidate_metadata,
                )


if __name__ == "__main__":
    unittest.main()
