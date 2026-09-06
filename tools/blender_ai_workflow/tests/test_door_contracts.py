from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKFLOW_ROOT / "scripts"))

import project_door_preview as projector
import provision_door_preview as provisioner
import validate_door_manifest as manifest_validator
import validate_door_textures as texture_validator


class DoorGeometryRevisionTests(unittest.TestCase):
    def test_thin_top_frame_and_symmetric_open_pose_are_frozen(self) -> None:
        contract = json.loads(
            (WORKFLOW_ROOT / "fixtures/door-production-v1.geometry.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual(contract["frame"]["top_y_range_wu"], [14.4, 16.0])
        self.assertEqual(contract["frame"]["top_z_range_wu"], [-3.6, 3.6])
        self.assertEqual(
            contract["open_leaf_envelope_wu"]["swing_angles_degrees"],
            {"left": -78.0, "right": 78.0},
        )
        self.assertEqual(
            contract["open_leaf_envelope_wu"]["left_x"], [-13.2, -8.149594]
        )
        self.assertEqual(
            contract["open_leaf_envelope_wu"]["right_x"], [8.149594, 13.2]
        )
        self.assertEqual(
            contract["open_leaf_envelope_wu"]["left_z"], [-3.698988, 9.515919]
        )
        self.assertEqual(
            contract["open_leaf_envelope_wu"]["right_z"], [-3.698988, 9.515919]
        )
        self.assertEqual(
            contract["states"],
            {
                "closed": {"triangles": 204},
                "open": {"triangles": 204},
                "locked": {"triangles": 216},
            },
        )


class DoorTextureTests(unittest.TestCase):
    def test_exact_opaque_albedo_and_distinct_safe_previews_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Image.new("RGB", (512, 512), (45, 36, 31)).save(root / "door_albedo.png")
            ew = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
            ew.paste((80, 65, 54, 255), (32, 24, 220, 220))
            ew.save(root / "door_preview_ew.png")
            ns = Image.new("RGBA", (256, 256), (0, 0, 0, 0))
            ns.paste((75, 62, 50, 255), (48, 20, 204, 216))
            ns.save(root / "door_preview_ns.png")

            report = texture_validator.validate(root)

            self.assertEqual(report["status"], "pass")
            self.assertTrue(report["albedo"]["opaque"])

    def test_transparent_albedo_and_cropped_preview_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Image.new("RGBA", (512, 512), (1, 2, 3, 254)).save(root / "door_albedo.png")
            for axis in ("ew", "ns"):
                Image.new("RGBA", (256, 256), (1, 2, 3, 255)).save(
                    root / f"door_preview_{axis}.png"
                )
            with self.assertRaises(texture_validator.TextureError):
                texture_validator.validate(root)


class DoorAuthorityTests(unittest.TestCase):
    def candidate(self) -> dict[str, object]:
        return {
            "art_review": {"status": "candidate"},
            "asset_set_generation": 1,
            "asset_set_id": manifest_validator.ASSET_SET_ID,
            "schema_version": 1,
            "manifest_mode": "candidate",
            "production": {
                "core": [
                    {"bytes": 1, "path": path, "role": role, "sha256": "a" * 64}
                    for path, role in manifest_validator.CORE
                ],
                "optional": [],
            },
        }

    def test_candidate_projects_to_canonical_art_preview_only(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "candidate.json"
            manifest.write_text(json.dumps(self.candidate()), encoding="utf-8")

            runtime = projector.project(manifest)
            encoded = projector.canonical_bytes(runtime)

            self.assertEqual(runtime["authority"], "art_preview")
            self.assertEqual(runtime["review_status"], "art_preview")
            self.assertEqual(runtime["normal_decision"], "not_used_by_design")
            self.assertEqual(len(runtime["core"]), 6)
            self.assertEqual(encoded, projector.canonical_bytes(json.loads(encoded)))

    def test_final_manifest_cannot_be_projected_as_preview(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            payload = self.candidate()
            payload["manifest_mode"] = "final"
            manifest = Path(directory) / "final.json"
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaises(projector.ProjectionError):
                projector.project(manifest)

    def test_existing_different_provisioned_byte_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            destination = root / "destination"
            source.write_bytes(b"source")
            destination.write_bytes(b"other")
            with self.assertRaises(provisioner.ProvisionError):
                provisioner.copy_exclusive(
                    source,
                    destination,
                    "41cf6794ba4200b839c3e1ec173e19738d3b1f3e0d1acee1f238aa2f0ba88afa",
                )

    def test_parent_segments_are_not_safe_manifest_paths(self) -> None:
        with self.assertRaises(manifest_validator.ManifestError):
            manifest_validator.safe_relative("../door.glb", "Door mesh")


if __name__ == "__main__":
    unittest.main()
