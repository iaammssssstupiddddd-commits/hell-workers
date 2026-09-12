from __future__ import annotations

import copy
import json
import shutil
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from PIL import Image

WORKFLOW = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKFLOW / "scripts"))

import door_preview_projection as projection
import validate_door_textures as textures
import seal_door_preview_revision as revision
from test_building_release import door_fixture


def projection_report(hashes: dict[str, str]) -> dict:
    return {
        "anchor_px": list(projection.ANCHOR_PX),
        "projection": {"profile": projection.PROFILE, **projection.camera_settings()},
        "previews": {
            axis: {
                "sha256": hashes[axis],
                "projection": {
                    "profile": projection.PROFILE,
                    "all_vertices_in_canvas": True,
                    "samples_px": {
                        name: list(projection.project_glb(point, axis))
                        for name, point in projection.reference_points().items()
                    },
                },
            } for axis in ("ew", "ns")
        },
    }


class DoorProjectionTests(unittest.TestCase):
    def test_matches_runtime_contract(self) -> None:
        repo = WORKFLOW.parents[1]
        constants = (repo / "crates/hw_core/src/constants/render.rs").read_text()
        runtime = (repo / "crates/bevy_app/src/systems/visual/door_preview.rs").read_text()
        self.assertIn(f"VIEW_HEIGHT: f32 = {projection.VIEW_HEIGHT};", constants)
        self.assertIn(f"Z_OFFSET: f32 = {projection.Z_OFFSET};", constants)
        self.assertIn(f"PRODUCTION_PREVIEW_SIZE: Vec2 = Vec2::splat({projection.CANVAS_WU});", runtime)
        self.assertIn("PRODUCTION_PREVIEW_ANCHOR: Anchor = Anchor(Vec2::new(0.0, -0.25));", runtime)

    def test_axes_ground_anchor_and_one_tile_scale(self) -> None:
        for axis in ("ew", "ns"):
            self.assertEqual(projection.project_glb((0, -16, 0), axis), (128, 192))
        self.assertEqual(projection.project_glb((-16, -16, 0), "ew"), (64, 192))
        self.assertEqual(projection.project_glb((16, -16, 0), "ew"), (192, 192))
        self.assertEqual(projection.project_glb((-16, -16, 0), "ns"), (128, 256))
        self.assertEqual(projection.project_glb((16, -16, 0), "ns"), (128, 128))
        with self.assertRaises(ValueError):
            projection.project_glb((0, 0, 0), "diagonal")

    def test_all_geometry_contract_bounds_fit_canvas(self) -> None:
        # The source frame's ground extents must not be shrunk just to leave a margin.
        for axis in ("ew", "ns"):
            for x in (-16, 16):
                for y in (-16, 16):
                    for z in (-5.6, 5.6):
                        px, py = projection.project_glb((x, y, z), axis)
                        self.assertTrue(0 <= px <= 256 and 0 <= py <= 256)

    def test_skew_stale_hash_unknown_camera_and_cropped_geometry_rejected(self) -> None:
        hashes = {"ew": "a" * 64, "ns": "b" * 64}
        report = projection_report(hashes)
        projection.validate_report(report, hashes)
        for defect in ("skew", "hash", "camera", "crop", "nan", "missing"):
            with self.subTest(defect=defect):
                broken = copy.deepcopy(report)
                evidence = broken["previews"]["ns"]["projection"]
                if defect == "skew":
                    evidence["samples_px"]["left_ground"][0] += 3
                elif defect == "hash":
                    broken["previews"]["ns"]["sha256"] = "c" * 64
                elif defect == "camera":
                    broken["projection"]["profile"] = "unknown"
                elif defect == "crop":
                    evidence["all_vertices_in_canvas"] = False
                elif defect == "nan":
                    evidence["samples_px"]["ground_center"][0] = float("nan")
                else:
                    del evidence["samples_px"]["ground_center"]
                with self.assertRaises(ValueError):
                    projection.validate_report(broken, hashes)

    def test_south_edge_allowed_only_with_bound_topdown_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Image.new("RGB", (512, 512)).save(root / "door_albedo.png")
            for axis, bounds in (("ew", (64, 90, 192, 215)), ("ns", (100, 50, 151, 256))):
                image = Image.new("RGBA", (256, 256))
                image.paste((80, 60, 40, 255), bounds)
                image.save(root / f"door_preview_{axis}.png")
            with self.assertRaises(textures.TextureError):
                textures.validate(root)
            hashes = {axis: textures.digest(root / f"door_preview_{axis}.png") for axis in ("ew", "ns")}
            self.assertEqual(textures.validate(root, projection_report(hashes))["status"], "pass")


class DoorPreviewRevisionTests(unittest.TestCase):
    def prepare(self, root: Path) -> tuple[Path, Path, Path]:
        original, updated, repo = root / "original", root / "updated", root / "repo"
        base = door_fixture(original, repo)
        albedo = original / "staging/exports/textures/buildings/door/door_albedo.png"
        Image.new("RGB", (512, 512)).save(albedo)
        manifest = json.loads(base.read_text())
        manifest["production"]["core"][3].update(sha256=textures.digest(albedo), bytes=albedo.stat().st_size)
        base.write_text(json.dumps(manifest))
        shutil.copytree(original, updated)
        texture_root = updated / "staging/exports/textures/buildings/door"
        for axis, bounds in (("ew", (64, 90, 192, 215)), ("ns", (100, 50, 151, 256))):
            image = Image.new("RGBA", (256, 256))
            image.paste((80, 60, 40, 255), bounds)
            image.save(texture_root / f"door_preview_{axis}.png")
        hashes = {axis: textures.digest(texture_root / f"door_preview_{axis}.png") for axis in ("ew", "ns")}
        report = projection_report(hashes)
        report.update(status="rendered", ocio={"fallback": False, "validation_status": "pass"})
        (updated / "staging/reports/door/preview.json").write_text(json.dumps(report))
        return base, updated, repo

    def run_seal(self, base: Path, updated: Path, repo: Path) -> dict:
        with mock.patch.object(revision.final.git_sealer, "clean_git_identity", return_value=("e" * 40, "f" * 40)):
            return revision.seal(base_path=base, root=updated, repo=repo, generation=2,
                                 approved_at="2026-09-12T00:00:00Z", instruction="fix projection",
                                 authorization="commit and release", output=updated / "staging/reports/revision.json")

    def test_revision_preserves_art_and_separates_correction_authority(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base, updated, repo = self.prepare(Path(directory))
            self.assertEqual(self.run_seal(base, updated, repo)["status"], "pass")
            original = json.loads(base.read_text())
            result = json.loads((updated / "staging/reports/revision.json").read_text())
            self.assertEqual(result["production"]["core"][:4], original["production"]["core"][:4])
            self.assertEqual(result["art_review"]["artifact"], original["art_review"]["artifact"])
            evidence = json.loads((updated / "staging/reports/door/preview.json").read_text())["revision"]
            self.assertEqual(evidence["kind"], "user_directed_projection_correction")
            self.assertEqual(evidence["base_manifest_sha256"], textures.digest(base))
            self.assertEqual(result["source"]["runtime_subject"], "e" * 40)

    def test_non_preview_core_and_original_evidence_changes_fail_closed(self) -> None:
        for path in ("exports/models/buildings/door/door_open.glb",
                     "exports/textures/buildings/door/door_albedo.png",
                     "blend/door.blend", "reports/door/art.json"):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as directory:
                base, updated, repo = self.prepare(Path(directory))
                (updated / "staging" / path).write_bytes(b"tampered")
                with self.assertRaises(RuntimeError):
                    self.run_seal(base, updated, repo)
                self.assertFalse((updated / "staging/reports/revision.json").exists())


if __name__ == "__main__":
    unittest.main()
