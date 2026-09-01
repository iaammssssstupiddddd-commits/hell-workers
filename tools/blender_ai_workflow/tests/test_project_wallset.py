from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/project_wallset.py"


def load_projector():
    spec = importlib.util.spec_from_file_location("project_wallset", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load project_wallset.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def record(path: str, role: str) -> dict[str, object]:
    return {"path": path, "role": role, "bytes": 1, "sha256": "a" * 64}


class WallsetProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.projector = load_projector()

    def manifest(self, root: Path) -> Path:
        core = [
            record(f"models/buildings/wall/wall_{family}.glb", f"mesh:{family}")
            for family in (
                "isolated",
                "end",
                "straight",
                "corner",
                "t_junction",
                "cross",
            )
        ]
        core += [
            record("textures/buildings/wall/wall_albedo.png", "texture:albedo"),
            record("textures/buildings/wall/wall_emissive.png", "texture:emissive"),
        ]
        payload = {
            "schema_version": 2,
            "asset_set_id": "wall-production-v1",
            "manifest_mode": "candidate",
            "asset_set_generation": 3,
            "normal_decision": "pending",
            "production": {
                "core": core,
                "optional": [
                    record("textures/buildings/wall/wall_normal.png", "texture:normal")
                ],
            },
        }
        path = root / "candidate.json"
        path.write_text(json.dumps(payload), encoding="utf-8")
        return path

    def test_candidate_projection_is_canonical_and_exact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = self.manifest(Path(directory))
            projection = self.projector.project_candidate(manifest)
            encoded = self.projector.canonical_bytes(projection)
            self.assertEqual(
                encoded, self.projector.canonical_bytes(json.loads(encoded))
            )
            self.assertEqual(projection["asset_set_generation"], 3)
            self.assertEqual(projection["authority"], "isolated_candidate")
            self.assertEqual(projection["review_status"], "candidate")
            self.assertEqual(
                projection["manifest_sha256"],
                hashlib.sha256(manifest.read_bytes()).hexdigest(),
            )
            self.assertEqual(len(projection["core"]), 8)

    def test_rejected_or_adopted_manifest_is_not_candidate_projection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = self.manifest(Path(directory))
            payload = json.loads(manifest.read_text())
            payload["normal_decision"] = "adopted"
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaisesRegex(
                self.projector.ProjectionError, "identity differs"
            ):
                self.projector.project_candidate(manifest)

    def test_candidate_inventory_is_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = self.manifest(Path(directory))
            payload = json.loads(manifest.read_text())
            payload["production"]["core"].append(
                record("audio/extra.ogg", "audio:extra")
            )
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaisesRegex(
                self.projector.ProjectionError, "core inventory differs"
            ):
                self.projector.project_candidate(manifest)

    def test_release_projection_binds_receipt_and_generation_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self.manifest(root)
            payload = json.loads(manifest.read_text())
            payload["manifest_mode"] = "final"
            payload["normal_decision"] = "rejected"
            payload["production"]["optional"] = []
            payload["art_review"] = {"review_status": "art_approved"}
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            manifest_hash = hashlib.sha256(manifest.read_bytes()).hexdigest()
            receipt_payload = {
                "approval": {
                    "approved_at_utc": "2026-09-01T00:00:00Z",
                    "path": "evidence/release-approval.json",
                    "sha256": "1" * 64,
                },
                "asset_set_generation": 3,
                "asset_set_id": "wall-production-v1",
                "m5_evidence_bundle": {
                    "path": "evidence/m5-evidence-bundle.json",
                    "sha256": "2" * 64,
                },
                "manifest_sha256": manifest_hash,
                "new_active": {
                    "asset_set_generation": 3,
                    "asset_set_id": "wall-production-v1",
                    "manifest_sha256": manifest_hash,
                },
                "previous_active": {"status": "absent"},
                "promotion_plan_sha256": "3" * 64,
                "receipt_id": "receipt-release-01",
                "schema_version": 1,
                "tool_commit": "4" * 40,
                "tool_tree": "5" * 40,
            }
            receipt = root / "promotion-receipt.json"
            receipt.write_bytes(self.projector.canonical_bytes(receipt_payload))

            projection = self.projector.project_release(manifest, receipt)

            self.assertEqual(projection["authority"], "release_approved")
            self.assertEqual(projection["review_status"], "art_approved")
            self.assertIsNone(projection["candidate_normal"])
            self.assertEqual(
                projection["core"][0]["path"],
                "wall_sets/3/models/wall_isolated.glb",
            )
            self.assertEqual(
                projection["receipt"]["path"],
                "wall_sets/3/authority/promotion-receipt.json",
            )

    def test_release_projection_rejects_receipt_for_another_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self.manifest(root)
            payload = json.loads(manifest.read_text())
            payload["manifest_mode"] = "final"
            payload["normal_decision"] = "rejected"
            payload["production"]["optional"] = []
            payload["art_review"] = {"review_status": "art_approved"}
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            receipt = root / "promotion-receipt.json"
            receipt.write_bytes(
                self.projector.canonical_bytes(
                    {
                        "asset_set_generation": 3,
                        "asset_set_id": "wall-production-v1",
                        "manifest_sha256": "f" * 64,
                        "new_active": {},
                        "schema_version": 1,
                    }
                )
            )
            with self.assertRaisesRegex(
                self.projector.ProjectionError, "receipt binding differs"
            ):
                self.projector.project_release(manifest, receipt)


if __name__ == "__main__":
    unittest.main()
