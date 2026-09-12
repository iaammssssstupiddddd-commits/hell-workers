from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import record_wall_surface_approval as approval
import seal_wall_surface_revision as revision
from record_wall_formwork_approval import ApprovalError
from seal_wall_candidate import SealError, sha256
from workflow_common import write_json_atomic


class SurfaceApprovalTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.preview_path = self.root / "preview/surface-preview.json"
        roles = ([f"mesh:{family}" for family in approval.FAMILIES] + ["texture:albedo", "texture:emissive"]
                 + [f"mesh:formwork:{family}" for family in approval.FAMILIES] + ["texture:formwork_albedo"])
        core = []
        for i, role in enumerate(roles):
            path = self.preview_path.parent / f"assets/{i}.bin"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(role.encode())
            core.append({"role": role, "path": path.name, "bytes": path.stat().st_size, "sha256": sha256(path)})
        self.preview = {"schema_version": 1, "asset_set_id": "wall-production-v1", "asset_set_generation": 12,
                        "manifest_mode": "surface_art_preview", "review_status": "art_preview",
                        "promotion_authority": False, "surface_uv_profile": approval.PROFILE,
                        "subject_commit": "a" * 40, "base_manifest_sha256": "b" * 64,
                        "production": {"core": core, "optional": []}}
        write_json_atomic(self.preview_path, self.preview)
        for zoom in ("standard", "farthest"):
            root = self.root / zoom
            root.mkdir()
            image = root / "current-wall.png"
            image.write_bytes(b"fixture screenshot " + zoom.encode())
            common = {"profile": f"wall-surface-art-preview-v1-{zoom}", "zoom": zoom,
                      "authority": "art_preview", "evidence_kind": "art_preview",
                      "subject_commit": self.preview["subject_commit"],
                      "candidate_identity": {"asset_set_generation": 12, "authority": "art_preview",
                                             "manifest_sha256": sha256(self.preview_path)},
                      "completed_at": "2026-09-12T14:55:00Z",
                      **{field: "c" * 64 for field in approval.FINGERPRINTS}}
            write_json_atomic(root / "job.json", {**common, "status": "valid"})
            write_json_atomic(root / "manifest.json", {**common, "status": "pass", "screenshot_sha256": sha256(image)})
        self.verifier = patch.object(approval.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "pass", ""))
        self.run_verify = self.verifier.start()
        self.addCleanup(self.verifier.stop)

    def build(self, **overrides):
        return approval.build_approval(**{"preview_path": self.preview_path, "repo": self.root,
            "standard": self.root / "standard", "farthest": self.root / "farthest",
            "approved_at": "2026-09-12T15:00:00Z", "statement": "OKです。進めてください", **overrides})

    def mutate(self, zoom, filename, change):
        path = self.root / zoom / filename
        payload = json.loads(path.read_text())
        payload.update(change)
        write_json_atomic(path, payload)

    def test_verified_pair_binds_exact_core_without_release_authority(self):
        result = self.build()
        self.assertEqual(result["production"], self.preview["production"])
        self.assertEqual(result["decision"], "surface_art_approved")
        self.assertIs(result["promotion_authority"], False)
        self.assertEqual(self.run_verify.call_count, 2)
        for call, zoom in zip(self.run_verify.call_args_list, ("standard", "farthest"), strict=True):
            self.assertEqual(call.args[0][-3:], ["verify", "--job-root", str(self.root / zoom)])

    def test_failed_job_is_rejected(self):
        self.mutate("standard", "job.json", {"status": "invalid"})
        with self.assertRaisesRegex(ApprovalError, "did not pass"):
            self.build()

    def test_failed_independent_verify_is_rejected(self):
        self.run_verify.return_value = subprocess.CompletedProcess([], 1, "invalid fixture", "")
        with self.assertRaisesRegex(ApprovalError, "independent verify failed"):
            self.build()

    def test_wrong_subject_candidate_and_authority_are_rejected(self):
        path = self.root / "standard/manifest.json"
        original = json.loads(path.read_text())
        for change in ({"subject_commit": "d" * 40}, {"candidate_identity": {}},
                       {"authority": "isolated_candidate"}, {"evidence_kind": "formal"},
                       {"profile": "wall-formwork-art-preview-v1"}):
            write_json_atomic(path, {**original, **change})
            with self.subTest(change=change), self.assertRaisesRegex(ApprovalError, "binding differs"):
                self.build()

    def test_farthest_cannot_reuse_standard_job(self):
        with self.assertRaisesRegex(ApprovalError, "binding differs"):
            self.build(farthest=self.root / "standard")

    def test_modified_screenshot_is_rejected(self):
        (self.root / "standard/current-wall.png").write_bytes(b"modified")
        with self.assertRaisesRegex(ApprovalError, "screenshot hash"):
            self.build()

    def test_modified_asset_is_rejected(self):
        (self.preview_path.parent / "assets/0.bin").write_bytes(b"changed approved asset")
        with self.assertRaisesRegex(SealError, "source bytes"):
            self.build()

    def test_pair_fingerprint_mismatch_is_rejected(self):
        for filename in ("job.json", "manifest.json"):
            self.mutate("farthest", filename, {"asset_view_fingerprint": "d" * 64})
        with self.assertRaisesRegex(ApprovalError, "pair fingerprints"):
            self.build()

    def test_empty_or_backdated_approval_is_rejected(self):
        for overrides in ({"statement": " "}, {"approved_at": "2026-09-11T00:00:00Z"}, {"approved_at": "2026-09-12"}):
            with self.subTest(overrides=overrides), self.assertRaises(ApprovalError):
                self.build(**overrides)

    def test_revision_rejects_reused_generation_and_changed_formwork(self):
        base_path = self.root / "base.json"
        base = {"schema_version": 3, "asset_set_id": "wall-production-v1", "manifest_mode": "final",
                "asset_set_generation": 10, "production": copy.deepcopy(self.preview["production"])}
        write_json_atomic(base_path, base)
        preview = {**self.preview, "base_manifest_sha256": sha256(base_path)}
        with patch.object(revision.release, "validate"), patch.object(approval, "preview_identity", return_value=preview):
            with self.assertRaisesRegex(SealError, "generation must advance"):
                revision.validate_revision_inputs(base_path, self.preview_path, self.root / "absent", 12,
                                                   self.root, self.root / "standard", self.root / "farthest")
            preview["production"]["core"][8]["sha256"] = "f" * 64
            with self.assertRaisesRegex(SealError, "formwork bytes changed"):
                revision.validate_revision_inputs(base_path, self.preview_path, self.root / "absent", 13,
                                                   self.root, self.root / "standard", self.root / "farthest")

    def test_revision_destination_must_be_new_and_isolated(self):
        with patch.object(revision, "clean_git_identity", return_value=("a" * 40, "b" * 40)):
            for destination in (self.root / "assets", self.root / "staging/validation/existing"):
                destination.mkdir(parents=True)
                with self.assertRaisesRegex(SealError, "destination must be new"):
                    revision.seal(base_path=self.root / "absent", art_root=self.root, preview_path=self.preview_path,
                                  approval_path=self.root / "absent", standard=self.root / "standard",
                                  farthest=self.root / "farthest", destination=destination, generation=13, repo=self.root)

    def test_rebuild_prepares_blend_relative_textures_before_export(self):
        blend = self.root / "staging/blend/wall-production-v1.blend"
        blend.parent.mkdir(parents=True)
        blend.write_bytes(b"fixture blend")
        prepared = self.root / "isolated/staging"
        copied = revision.prepare_rebuild_sources(self.root, prepared, self.preview_path.parent / "assets", self.preview["production"]["core"])
        self.assertEqual(copied.read_bytes(), blend.read_bytes())
        for texture in self.preview["production"]["core"][6:8]:
            self.assertEqual(sha256(prepared / "exports" / texture["path"]), texture["sha256"])

    def test_published_read_only_directory_modes_are_not_inherited(self):
        source = self.root / "published"
        reports = source / "reports"
        reports.mkdir(parents=True)
        (reports / "old.json").write_bytes(b"immutable evidence")
        (reports / "old.json").chmod(0o444)
        reports.chmod(0o555)
        source.chmod(0o555)
        try:
            target = self.root / "new-staging"
            revision.copy_tree(source, target)
            (target / "reports/new.json").write_bytes(b"new evidence")
            self.assertEqual((target / "reports/old.json").read_bytes(), b"immutable evidence")
            self.assertEqual(reports.stat().st_mode & 0o777, 0o555)
        finally:
            source.chmod(0o755)
            reports.chmod(0o755)


    def test_release_revalidates_source_art_prompt_and_approved_screenshots(self):
        reports = self.root / "reports"
        reports.mkdir()
        artifacts = {}
        for role in revision.release.SURFACE_ARTIFACT_ROLES:
            path = reports / role
            path.write_bytes(role.encode())
            artifacts[role] = revision.record(path, reports)
        decision = self.build()
        for zoom in ("standard", "farthest"):
            decision["previews"][zoom]["screenshot_sha256"] = artifacts[f"{zoom}_capture"]["sha256"]
            for kind in ("manifest", "job"):
                decision["previews"][zoom][f"{kind}_sha256"] = artifacts[f"{zoom}_{kind}"]["sha256"]
        write_json_atomic(reports / "approval.json", decision)
        packing = {"profile": approval.PROFILE, "artwork": {role: {"sha256": artifacts[f"{role}_art"]["sha256"]} for role in ("stone", "core")},
                   "textures": {record["path"]: {"sha256": record["sha256"]} for record in self.preview["production"]["core"][6:8]}}
        write_json_atomic(reports / "packing", packing)
        artifacts["packing"] = revision.record(reports / "packing", reports)
        rev = {"profile": approval.PROFILE, "artifacts": artifacts, "base_manifest_sha256": self.preview["base_manifest_sha256"]}
        manifest = {"production": self.preview["production"], "art_review": {"artifact": revision.record(reports / "approval.json", reports)}}
        completed = {"art_review": manifest["art_review"], "production": self.preview["production"], "provenance": {"generators": [{"role": "texture", "prompt_sha256": artifacts["prompt"]["sha256"]}]}}
        revision.release.validate_surface_revision(rev, manifest, completed, reports)
        (reports / "stone_art").write_bytes(b"corrupted raw image")
        with self.assertRaisesRegex(revision.release.formwork.ManifestError, "hash differs"):
            revision.release.validate_surface_revision(rev, manifest, completed, reports)
        artifacts["stone_art"] = revision.record(reports / "stone_art", reports)
        with self.assertRaisesRegex(RuntimeError, "source artwork differs"):
            revision.release.validate_surface_revision(rev, manifest, completed, reports)


if __name__ == "__main__":
    unittest.main()
