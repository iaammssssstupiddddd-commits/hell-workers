from __future__ import annotations

import hashlib
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from PIL import Image

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = WORKFLOW_ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS_ROOT))

import project_wall_formwork_preview as projector
import project_wall_formwork_candidate as candidate_projector
import provision_wall_formwork_preview as provisioner
import record_wall_formwork_approval as approval_recorder
import seal_wall_formwork_final as final_sealer
import validate_wall_formwork_manifest as manifest_validator
import validate_wall_formwork_texture as texture_validator


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def file_record(path: Path, root: Path) -> dict[str, str]:
    return {"path": path.relative_to(root).as_posix(), "sha256": digest(path)}


def production_record(path: Path, root: Path, role: str) -> dict[str, object]:
    return {
        **file_record(path, root),
        "bytes": path.stat().st_size,
        "role": role,
    }


class WallFormworkTextureTests(unittest.TestCase):
    def test_opaque_512_texture_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "formwork.png"
            image = Image.new("RGB", (512, 512), (54, 45, 40))
            image.putpixel((0, 0), (75, 62, 51))
            image.save(path)
            report = texture_validator.validate(path)
            self.assertTrue(report["opaque"])
            self.assertEqual(report["sha256"], digest(path))

    def test_transparency_and_wrong_extent_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            transparent = root / "transparent.png"
            Image.new("RGBA", (512, 512), (1, 2, 3, 254)).save(transparent)
            with self.assertRaisesRegex(texture_validator.TextureError, "opaque"):
                texture_validator.validate(transparent)
            small = root / "small.png"
            Image.new("RGB", (64, 64), (1, 2, 3)).save(small)
            with self.assertRaisesRegex(texture_validator.TextureError, "512x512"):
                texture_validator.validate(small)


class WallFormworkManifestTests(unittest.TestCase):
    def fixture(self, root: Path) -> tuple[Path, dict[str, object], dict[str, Path]]:
        exports = root / "exports"
        reports = root / "reports"
        blends = root / "blend"
        licenses = root / "licenses"
        for directory in (exports, reports, blends, licenses):
            directory.mkdir()

        completed_core = []
        completed_roles = [
            *(f"mesh:{family}" for family in manifest_validator.FAMILIES),
            "texture:albedo",
            "texture:emissive",
        ]
        completed_paths = [
            *(f"models/buildings/wall/wall_{family}.glb" for family in manifest_validator.FAMILIES),
            "textures/buildings/wall/wall_albedo.png",
            "textures/buildings/wall/wall_emissive.png",
        ]
        for relative, role in zip(completed_paths, completed_roles, strict=True):
            path = exports / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(role.encode())
            completed_core.append(production_record(path, exports, role))

        license_path = licenses / "wall-production-v1.md"
        license_path.write_text("test license\n", encoding="utf-8")
        completed = {
            "asset_set_generation": 4,
            "asset_set_id": manifest_validator.ASSET_SET_ID,
            "license": {"file": file_record(license_path, licenses)},
            "manifest_mode": "final",
            "production": {"core": completed_core, "optional": []},
            "provenance": {},
            "schema_version": 2,
        }
        completed_path = root / "completed.json"
        completed_path.write_text(json.dumps(completed), encoding="utf-8")

        blend = blends / "wall-formwork-v1.blend"
        blend.write_bytes(b"blend")
        geometry = reports / "wall-formwork-v1.geometry.json"
        geometry.write_text("{}\n", encoding="utf-8")
        prompt = reports / "wall-formwork-v1.texture-prompt.txt"
        prompt.write_text("opaque wood\n", encoding="utf-8")

        formwork_meshes = []
        formwork_core = []
        for family, collection in manifest_validator.FAMILIES.items():
            output = exports / manifest_validator.FORMWORK_PATHS[family]
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(f"glb-{family}".encode())
            export_report = reports / f"{family}.export.json"
            export_report.write_text(
                json.dumps({"status": "exported", "sha256": digest(output)}),
                encoding="utf-8",
            )
            khronos_report = reports / f"{family}.khronos.json"
            khronos_report.write_text(
                json.dumps({"issues": {"numErrors": 0, "numWarnings": 0}}),
                encoding="utf-8",
            )
            post_report = reports / f"{family}.post.json"
            post_report.write_text(
                json.dumps(
                    {
                        "family": family,
                        "glb_sha256": digest(output),
                        "status": "pass",
                    }
                ),
                encoding="utf-8",
            )
            formwork_meshes.append(
                {
                    "collection": collection,
                    "family": family,
                    "output": file_record(output, exports),
                    "reports": {
                        "export": file_record(export_report, reports),
                        "khronos": file_record(khronos_report, reports),
                        "post_export": file_record(post_report, reports),
                    },
                }
            )
            formwork_core.append(
                production_record(output, exports, f"mesh:formwork:{family}")
            )

        texture = exports / manifest_validator.FORMWORK_TEXTURE
        texture.parent.mkdir(parents=True, exist_ok=True)
        Image.new("RGB", (512, 512), (50, 40, 30)).save(texture)
        formwork_core.append(
            production_record(texture, exports, "texture:formwork_albedo")
        )
        texture_report = reports / "texture.json"
        texture_report.write_text(
            json.dumps(
                {
                    "opaque": True,
                    "role": "texture:formwork_albedo",
                    "status": "pass",
                }
            ),
            encoding="utf-8",
        )

        payload: dict[str, object] = {
            "art_review": {"status": "candidate"},
            "asset_set_generation": 5,
            "asset_set_id": manifest_validator.ASSET_SET_ID,
            "created_at_utc": "2026-09-06T00:00:00Z",
            "formwork_meshes": formwork_meshes,
            "formwork_texture_report": file_record(texture_report, reports),
            "license": completed["license"],
            "manifest_mode": "candidate",
            "normal_decision": "rejected",
            "production": {"core": completed_core + formwork_core, "optional": []},
            "provenance": {},
            "schema_version": 3,
            "source": {
                "completed": {
                    "asset_set_generation": 4,
                    "manifest_sha256": digest(completed_path),
                },
                "formwork": {
                    "blend": file_record(blend, blends),
                    "geometry_contract": file_record(geometry, reports),
                    "texture_prompt": file_record(prompt, reports),
                },
                "runtime_subject": "pending",
                "source_fingerprint": "a" * 64,
                "tool_commit": "b" * 40,
                "tool_tree": "c" * 40,
                "tool_versions": {},
                "working_tree_diff_sha256": "d" * 64,
            },
        }
        manifest = reports / "candidate.json"
        manifest.write_text(json.dumps(payload), encoding="utf-8")
        return manifest, payload, {
            "blend": blends,
            "completed": completed_path,
            "exports": exports,
            "licenses": licenses,
            "reports": reports,
        }

    def validate(self, manifest: Path, roots: dict[str, Path]) -> dict[str, object]:
        return manifest_validator.validate_manifest(
            manifest,
            exports_root=roots["exports"],
            reports_root=roots["reports"],
            blend_root=roots["blend"],
            licenses_root=roots["licenses"],
            completed_manifest_path=roots["completed"],
        )

    def test_candidate_validates_and_projects_to_non_release_authority(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest, _, roots = self.fixture(Path(directory))
            self.assertEqual(self.validate(manifest, roots)["status"], "pass")
            runtime = projector.project(manifest)
            self.assertEqual(runtime["schema_version"], 2)
            self.assertEqual(runtime["authority"], "art_preview")
            self.assertEqual(runtime["review_status"], "art_preview")
            self.assertEqual(len(runtime["core"]), 15)

    def test_final_manifest_requires_clean_approved_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest, payload, roots = self.fixture(Path(directory))
            payload["manifest_mode"] = "final"
            payload["art_review"] = {"status": "art_approved"}
            payload["source"]["runtime_subject"] = "e" * 40
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            with self.assertRaisesRegex(
                manifest_validator.ManifestError, "final source must be clean"
            ):
                self.validate(manifest, roots)

    def test_verified_preview_records_approval_and_projects_final_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, payload, _ = self.fixture(root)
            job_root = root / "acceptance/wall-formwork-preview-test"
            job_root.mkdir(parents=True)
            screenshot = job_root / "current-wall.png"
            screenshot.write_bytes(b"approved screenshot")
            identity = {
                "asset_set_generation": 5,
                "authority": "art_preview",
                "manifest_sha256": digest(manifest),
            }
            common = {
                "asset_view_fingerprint": "a" * 64,
                "candidate_identity": identity,
                "evidence_kind": "art_preview",
                "harness_fingerprint": "b" * 64,
                "profile": approval_recorder.PROFILE,
                "source_fingerprint": "c" * 64,
                "subject_commit": "d" * 40,
            }
            (job_root / "job.json").write_text(
                json.dumps({**common, "authority": "art_preview", "status": "valid"}),
                encoding="utf-8",
            )
            (job_root / "manifest.json").write_text(
                json.dumps(
                    {
                        **common,
                        "authority": "art_preview",
                        "completed_at": "2026-09-06T00:00:00Z",
                        "screenshot_sha256": digest(screenshot),
                        "status": "pass",
                        "zoom": "standard",
                    }
                ),
                encoding="utf-8",
            )
            approval = approval_recorder.build_approval(
                asset_root=root,
                candidate_manifest_path=manifest,
                preview_job_root=job_root,
                approved_at_utc="2026-09-06T01:00:00Z",
                statement="OKです",
            )
            self.assertEqual(approval["decision"], "opaque_formwork_approved")
            self.assertEqual(approval["preview"]["screenshot_sha256"], digest(screenshot))
            approval_path = root / "approval.json"
            approval_path.write_text(json.dumps(approval), encoding="utf-8")
            final_sealer.validate_approval(
                approval_path,
                candidate=payload,
                candidate_hash=digest(manifest),
            )

            payload["manifest_mode"] = "final"
            payload["asset_set_generation"] = 6
            payload["art_review"] = {"status": "art_approved"}
            manifest.write_text(json.dumps(payload), encoding="utf-8")
            runtime = candidate_projector.project(manifest)
            self.assertEqual(runtime["schema_version"], 2)
            self.assertEqual(runtime["authority"], "isolated_candidate")
            self.assertEqual(runtime["review_status"], "art_approved")
            self.assertEqual(len(runtime["core"]), 15)

    def test_preview_approval_rejects_failed_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, _, _ = self.fixture(root)
            job_root = root / "acceptance/failed"
            job_root.mkdir(parents=True)
            (job_root / "current-wall.png").write_bytes(b"screenshot")
            identity = {
                "asset_set_generation": 5,
                "authority": "art_preview",
                "manifest_sha256": digest(manifest),
            }
            common = {
                "adapter": "Intel",
                "asset_view_fingerprint": "a" * 64,
                "authority": "art_preview",
                "candidate_identity": identity,
                "evidence_kind": "art_preview",
                "harness_fingerprint": "b" * 64,
                "profile": approval_recorder.PROFILE,
                "source_fingerprint": "c" * 64,
                "subject_commit": "d" * 40,
            }
            (job_root / "job.json").write_text(
                json.dumps({**common, "status": "valid"}), encoding="utf-8"
            )
            (job_root / "manifest.json").write_text(
                json.dumps(
                    {
                        **common,
                        "completed_at": "2026-09-06T00:00:00Z",
                        "screenshot_sha256": digest(job_root / "current-wall.png"),
                        "status": "failed",
                        "zoom": "standard",
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(approval_recorder.ApprovalError, "binding"):
                approval_recorder.build_approval(
                    asset_root=root,
                    candidate_manifest_path=manifest,
                    preview_job_root=job_root,
                    approved_at_utc="2026-09-06T01:00:00Z",
                    statement="OKです",
                )

    def test_final_seal_rebinds_approved_bytes_to_clean_runtime_subject(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            asset_root = Path(directory)
            staging = asset_root / "staging"
            staging.mkdir()
            manifest, _, roots = self.fixture(staging)
            shutil.move(staging / "licenses", asset_root / "licenses")
            roots["licenses"] = asset_root / "licenses"
            job_root = staging / "acceptance/wall-formwork-preview-test"
            job_root.mkdir(parents=True)
            screenshot = job_root / "current-wall.png"
            screenshot.write_bytes(b"approved screenshot")
            identity = {
                "asset_set_generation": 5,
                "authority": "art_preview",
                "manifest_sha256": digest(manifest),
            }
            common = {
                "asset_view_fingerprint": "a" * 64,
                "candidate_identity": identity,
                "evidence_kind": "art_preview",
                "harness_fingerprint": "b" * 64,
                "profile": approval_recorder.PROFILE,
                "source_fingerprint": "c" * 64,
                "subject_commit": "d" * 40,
            }
            (job_root / "job.json").write_text(
                json.dumps({**common, "authority": "art_preview", "status": "valid"}),
                encoding="utf-8",
            )
            (job_root / "manifest.json").write_text(
                json.dumps(
                    {
                        **common,
                        "authority": "art_preview",
                        "completed_at": "2026-09-06T00:00:00Z",
                        "screenshot_sha256": digest(screenshot),
                        "status": "pass",
                        "zoom": "standard",
                    }
                ),
                encoding="utf-8",
            )
            approval = approval_recorder.build_approval(
                asset_root=asset_root,
                candidate_manifest_path=manifest,
                preview_job_root=job_root,
                approved_at_utc="2026-09-06T01:00:00Z",
                statement="OKです",
            )
            approval_path = roots["reports"] / "approval.json"
            approval_path.write_text(json.dumps(approval), encoding="utf-8")
            with mock.patch.object(
                final_sealer.git_sealer,
                "clean_git_identity",
                return_value=("e" * 40, "f" * 40),
            ):
                final = final_sealer.seal_final(
                    asset_root=asset_root,
                    repo=asset_root,
                    candidate_manifest_path=manifest,
                    completed_manifest_path=roots["completed"],
                    approval_path=approval_path,
                    generation=6,
                    reviewer="user",
                    notes="Approved formwork.",
                )
            self.assertEqual(final["manifest_mode"], "final")
            self.assertEqual(final["source"]["runtime_subject"], "e" * 40)
            self.assertEqual(final["source"]["working_tree_diff_sha256"], "0" * 64)
            final_path = roots["reports"] / "final.json"
            final_path.write_text(json.dumps(final), encoding="utf-8")
            self.assertEqual(self.validate(final_path, roots)["status"], "pass")

    def test_provisioning_refuses_to_overwrite_different_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            destination = root / "destination"
            source.write_bytes(b"candidate")
            destination.write_bytes(b"other")
            with self.assertRaisesRegex(
                provisioner.ProvisionError, "destination already differs"
            ):
                provisioner.copy_exclusive(source, destination, digest(source))


if __name__ == "__main__":
    unittest.main()
