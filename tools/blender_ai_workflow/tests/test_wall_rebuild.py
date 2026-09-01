from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = WORKFLOW_ROOT / "scripts/verify_wall_rebuild.py"


def load_verifier():
    spec = importlib.util.spec_from_file_location("verify_wall_rebuild", SCRIPT_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load verify_wall_rebuild.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WallRebuildTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = load_verifier()

    def fixture(self, root: Path) -> tuple[Path, Path]:
        exports = root / "exports"
        reports = root / "reports"
        reports.mkdir()
        for family, stem in self.verifier.FAMILIES.items():
            glb = exports / f"models/buildings/wall/{stem}.glb"
            glb.parent.mkdir(parents=True, exist_ok=True)
            glb.write_bytes(f"glb:{family}".encode())
            payload = {
                "schema_version": 1,
                "status": "pass",
                "asset_set_id": "wall-production-v1",
                "contract_sha256": "a" * 64,
                "family": family,
                "arms": [],
                "mesh_count": 1,
                "primitive_count": 1,
                "triangle_count": 1,
                "vertex_count": 3,
                "raw_primitive_local_bounds": {"min": [0, 0, 0], "max": [1, 1, 1]},
                "node_transform": {},
                "uv0_present": True,
                "tangent_present": False,
                "embedded_images": 0,
                "external_images": [],
                "cross_sections": [],
                "glb_sha256": self.verifier.sha256(glb),
            }
            (reports / f"{stem}.post-export.json").write_text(
                json.dumps(payload), encoding="utf-8"
            )
        return exports, reports

    def test_exact_rebuild_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            exports, reports = self.fixture(root)
            baseline_path = root / "baseline.json"
            self.verifier.write_report(
                baseline_path, self.verifier.capture(exports, reports)
            )
            result = self.verifier.verify(baseline_path, exports, reports)
            self.assertEqual(result["status"], "pass")
            self.assertEqual(len(result["families"]), 6)

    def test_changed_glb_is_rejected_even_with_updated_post_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            exports, reports = self.fixture(root)
            baseline_path = root / "baseline.json"
            self.verifier.write_report(
                baseline_path, self.verifier.capture(exports, reports)
            )
            stem = self.verifier.FAMILIES["isolated"]
            glb = exports / f"models/buildings/wall/{stem}.glb"
            glb.write_bytes(b"different deterministic output")
            post_path = reports / f"{stem}.post-export.json"
            post = json.loads(post_path.read_text())
            post["glb_sha256"] = self.verifier.sha256(glb)
            post_path.write_text(json.dumps(post), encoding="utf-8")
            with self.assertRaisesRegex(self.verifier.RebuildError, "output differs"):
                self.verifier.verify(baseline_path, exports, reports)

    def test_changed_structure_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            exports, reports = self.fixture(root)
            baseline_path = root / "baseline.json"
            self.verifier.write_report(
                baseline_path, self.verifier.capture(exports, reports)
            )
            stem = self.verifier.FAMILIES["cross"]
            post_path = reports / f"{stem}.post-export.json"
            post = json.loads(post_path.read_text())
            post["triangle_count"] = 2
            post_path.write_text(json.dumps(post), encoding="utf-8")
            with self.assertRaisesRegex(self.verifier.RebuildError, "output differs"):
                self.verifier.verify(baseline_path, exports, reports)

    def test_missing_family_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            exports, reports = self.fixture(root)
            stem = self.verifier.FAMILIES["end"]
            (exports / f"models/buildings/wall/{stem}.glb").unlink()
            with self.assertRaisesRegex(self.verifier.RebuildError, "GLB is absent"):
                self.verifier.capture(exports, reports)


if __name__ == "__main__":
    unittest.main()
