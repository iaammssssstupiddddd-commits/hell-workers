from __future__ import annotations

import json
import hashlib
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKFLOW_ROOT / "scripts"))

import project_door_preview as projector
import project_door_candidate as candidate_projector
import provision_door_preview as provisioner
import record_door_approval as approval_recorder
import seal_door_final as final_sealer
import validate_door_manifest as manifest_validator
import validate_door_textures as texture_validator


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


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
        self.assertEqual(
            manifest_validator.STATES,
            {"closed": 204, "open": 204, "locked": 216},
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

    def test_approved_final_projects_to_isolated_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            payload = self.candidate()
            payload["manifest_mode"] = "final"
            payload["art_review"] = {"status": "double_leaf_approved"}
            payload["normal_decision"] = "not_used_by_design"
            manifest = Path(directory) / "final.json"
            manifest.write_text(json.dumps(payload), encoding="utf-8")

            runtime = candidate_projector.project(manifest)

            self.assertEqual(runtime["authority"], "isolated_candidate")
            self.assertEqual(runtime["review_status"], "art_approved")
            self.assertIsNone(runtime["receipt"])

    def test_candidate_manifest_cannot_be_projected_as_isolated_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "candidate.json"
            manifest.write_text(json.dumps(self.candidate()), encoding="utf-8")
            with self.assertRaises(candidate_projector.ProjectionError):
                candidate_projector.project(manifest)

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


class DoorApprovalTests(unittest.TestCase):
    def test_approval_binds_candidate_to_exact_native_preview(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate = root / "candidate.json"
            candidate_payload = {
                "schema_version": 1,
                "asset_set_id": manifest_validator.ASSET_SET_ID,
                "asset_set_generation": 5,
                "manifest_mode": "candidate",
                "art_review": {"status": "candidate"},
                "source": {"tool_commit": "a" * 40},
            }
            candidate.write_text(json.dumps(candidate_payload), encoding="utf-8")
            job = root / "door-preview-job"
            job.mkdir()
            capture = job / "current-door.png"
            crop = job / "current-door-review-crop.png"
            status = job / "door-status.json"
            ack = job / "door-ack.json"
            capture.write_bytes(b"capture")
            crop.write_bytes(b"crop")
            status.write_bytes(b"status")
            ack.write_bytes(b"ack")
            evidence = {
                "schema_version": 1,
                "status": "awaiting_art_approval",
                "evidence_kind": "art_preview",
                "candidate_identity": {
                    "asset_set_generation": 5,
                    "authority": "ArtPreview",
                    "manifest_sha256": digest(candidate),
                },
                "window_backend": "x11",
                "adapter": {"backend": "Vulkan"},
                "source": {"commit": "a" * 40, "unchanged": True, "dirty_paths": []},
                "gallery": {
                    "production_count": 6,
                    "fallback_count": 0,
                    "axes": ["EastWest", "NorthSouth"],
                    "states": ["Closed", "Open", "Locked"],
                },
                "capture": {
                    "bytes": capture.stat().st_size,
                    "path": capture.name,
                    "sha256": digest(capture),
                },
                "review_crop": {
                    "bytes": crop.stat().st_size,
                    "path": crop.name,
                    "sha256": digest(crop),
                },
                "sidecar": {
                    "status_sha256": digest(status),
                    "ack_sha256": digest(ack),
                },
            }
            (job / "manifest.json").write_text(json.dumps(evidence), encoding="utf-8")

            approval = approval_recorder.build_approval(
                candidate_manifest_path=candidate,
                preview_job_root=job,
                approved_at_utc="2026-09-06T22:00:00+07:00",
                statement="OKです",
            )

            self.assertEqual(approval["decision"], "double_leaf_approved")
            self.assertEqual(approval["preview"]["review_crop"]["sha256"], digest(crop))
            self.assertEqual(approval["manifest_sha256"], digest(candidate))

    def test_final_seal_rebinds_approval_to_clean_runtime_subject(self) -> None:
        candidate = {
            "asset_set_generation": 5,
            "production": {"core": [{"sha256": "a" * 64}]},
            "source": {},
        }
        approval = {
            "approval": {
                "approved_at_utc": "2026-09-06T22:00:00+07:00",
                "approved_by": "user",
                "statement": "OKです",
            },
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            reports = root / "staging/reports"
            reports.mkdir(parents=True)
            candidate_path = reports / "candidate.json"
            approval_path = reports / "approval.json"
            candidate_path.write_text("{}", encoding="utf-8")
            approval_path.write_text("{}", encoding="utf-8")
            with (
                mock.patch.object(
                    final_sealer.git_sealer,
                    "clean_git_identity",
                    return_value=("b" * 40, "c" * 40),
                ),
                mock.patch.object(
                    final_sealer.validator,
                    "validate_manifest",
                    return_value={"manifest_sha256": "d" * 64},
                ),
                mock.patch.object(
                    final_sealer.validator,
                    "read_json",
                    return_value=candidate,
                ),
                mock.patch.object(
                    final_sealer,
                    "validate_approval",
                    return_value=approval,
                ),
            ):
                final = final_sealer.seal_final(
                    asset_root=root,
                    repo=root,
                    candidate_manifest_path=candidate_path,
                    approval_path=approval_path,
                    generation=6,
                    reviewer="user",
                    notes="Approved Door.",
                )

            self.assertEqual(final["manifest_mode"], "final")
            self.assertEqual(final["source"]["runtime_subject"], "b" * 40)
            self.assertEqual(final["source"]["working_tree_diff_sha256"], "0" * 64)
            self.assertEqual(final["art_review"]["status"], "double_leaf_approved")


if __name__ == "__main__":
    unittest.main()
