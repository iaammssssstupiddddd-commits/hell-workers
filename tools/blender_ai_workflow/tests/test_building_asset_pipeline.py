"""Transaction tests isolate the Rust codec; Rust projection tests own its ABI.

The byte payloads below are fixtures, never actual art or release evidence.
"""
from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_asset_pipeline as pipeline
import install_release_asset_set as legacy_install
import promote_asset_set as legacy_promote


def fixture_codec(_binary, operation, text, receipt=None):
    manifest = json.loads(text)
    if operation == "validate":
        return {"manifest": text, "receipt": receipt}
    manifest["identity"]["manifest_sha256"] = ""
    manifest["receipt"] = None
    manifest["identity"]["manifest_sha256"] = pipeline.digest(pipeline.canonical(manifest))
    if manifest["identity"]["authority"] == "release_approved":
        receipt_value = {"schema_version": 1, "identity": manifest["identity"],
            "art_approval_sha256": manifest["art_approval_sha256"], "decision": "release_approved"}
        if "numeric_approval_sha256" in manifest:
            receipt_value["numeric_approval_sha256"] = manifest["numeric_approval_sha256"]
        receipt = pipeline.canonical(receipt_value).decode()
        manifest["receipt"] = {"role": "authority:receipt", **pipeline.record(
            f"building_sets/{pipeline.KINDS[manifest['identity']['kind']]['slug']}/{manifest['identity']['generation']}/{pipeline.digest(receipt.encode())}.json", receipt.encode())}
    return {"manifest": pipeline.canonical(manifest).decode(), "receipt": receipt}


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = self.root / "codec"
        self.binary.write_bytes(b"test codec protocol fixture")
        patcher = patch.object(pipeline, "codec", side_effect=fixture_codec)
        patcher.start()
        self.addCleanup(patcher.stop)
        patcher = patch.object(pipeline, "verify_candidate_evidence", side_effect=pipeline.read)
        patcher.start()
        self.addCleanup(patcher.stop)

    def candidate(self, kind="Tank", generation=1):
        source = self.root / f"author-{kind}-{generation}"
        source.mkdir()
        contract = pipeline.KINDS[kind]
        entries = []
        for role in ["mesh:" + r for r in contract["mesh_roles"]] + ["image:" + r for r in contract["image_roles"]]:
            payload = role.encode()
            name = role.replace(":", "-")
            (source / name).write_bytes(payload)
            entries.append({"role": role, **pipeline.record(name, payload)})
        (source / "source").write_bytes(b"source fixture")
        (source / "geometry").write_bytes(b"geometry fixture")
        recipe = {"schema_version": 1, "kind": kind, "generation": generation,
            "source": pipeline.record("source", b"source fixture"),
            "geometry_contract": pipeline.record("geometry", b"geometry fixture"),
            "artifacts": entries, "parts": [], "world_preview": {}, "catalog_preview": {}}
        recipe_path = source / "recipe.json"
        recipe_path.write_bytes(pipeline.canonical(recipe))
        preview_root = self.root / f"preview-{kind}-{generation}"
        preview = pipeline.export(recipe_path, source, preview_root, self.binary, authority="art_preview")
        preview_manifest = pipeline.read(preview_root / pipeline.locator(kind))
        self.assertNotIn("production_state", preview_manifest)
        self.assertNotIn("numeric_approval_sha256", preview_manifest)
        approval_path = source / "art-approval.json"
        approval_path.write_bytes(pipeline.canonical({"schema_version": 1, "decision": "building_art_approved",
            "art_preview_identity": preview["identity"]}))
        numeric_path = None
        if kind in {"Tank", "MudMixer"}:
            # Separate synthetic decision input, never actual numeric/art authority.
            state = ({"kind": "Tank", "partial_y_wu": 9.0, "full_y_wu": 19.0} if kind == "Tank" else
                     {"kind": "MudMixer", "axis": [0.0, 0.0, 1.0], "radians_per_second": 0.7})
            numeric_path = source / "numeric-approval.json"
            numeric_path.write_bytes(pipeline.canonical({"schema_version": 1,
                "decision": "building_numeric_approved", "art_preview_identity": preview["identity"],
                "art_approval_sha256": pipeline.digest(approval_path.read_bytes()), "production_state": state}))
        output = self.root / f"candidate-{kind}-{generation}"
        pipeline.export(recipe_path, source, output, self.binary, authority="isolated_candidate",
                        approval_path=approval_path, numeric_approval_path=numeric_path)
        manifest = pipeline.read(output / pipeline.locator(kind))
        if numeric_path is not None:
            self.assertEqual(manifest["production_state"], state)
            self.assertEqual(manifest["numeric_approval_sha256"], pipeline.digest(numeric_path.read_bytes()))
        else:
            self.assertNotIn("production_state", manifest)
            self.assertNotIn("numeric_approval_sha256", manifest)
        return output

    def assert_numeric_release(self, root, kind, expected):
        manifest, _, receipt_text = pipeline.load_set(root, kind, self.binary)
        receipt = json.loads(receipt_text)
        if kind in {"Tank", "MudMixer"}:
            self.assertEqual(manifest["production_state"], expected["production_state"])
            self.assertEqual(manifest["numeric_approval_sha256"], expected["numeric_approval_sha256"])
            self.assertEqual(receipt["numeric_approval_sha256"], expected["numeric_approval_sha256"])
            numeric_path = root / f"building_sets/{pipeline.KINDS[kind]['slug']}/{manifest['identity']['generation']}/provenance/numeric-approval.json"
            self.assertEqual(pipeline.digest(numeric_path.read_bytes()), expected["numeric_approval_sha256"])
        else:
            self.assertNotIn("production_state", manifest)
            self.assertNotIn("numeric_approval_sha256", manifest)
            self.assertNotIn("numeric_approval_sha256", receipt)

    def promotion(self, kind="Tank", generation=1):
        candidate = self.candidate(kind, generation)
        assets = self.root / "assets"
        value = pipeline.plan(candidate, assets, kind, self.binary)
        candidate_manifest = pipeline.read(candidate / pipeline.locator(kind))
        projected = json.loads(value["projection"]["manifest"])
        receipt = json.loads(value["projection"]["receipt"])
        if kind in {"Tank", "MudMixer"}:
            self.assertEqual(projected["production_state"], candidate_manifest["production_state"])
            self.assertEqual(projected["numeric_approval_sha256"], candidate_manifest["numeric_approval_sha256"])
            self.assertEqual(receipt["numeric_approval_sha256"], candidate_manifest["numeric_approval_sha256"])
        else:
            self.assertNotIn("numeric_approval_sha256", projected)
            self.assertNotIn("numeric_approval_sha256", receipt)
        plan_path = self.root / f"{kind}-{generation}.plan.json"
        plan_path.write_bytes(pipeline.canonical(value))
        evidence = self.root / f"evidence-{kind}-{generation}"
        evidence.mkdir()
        evidence = evidence / "manifest.json"
        evidence.write_bytes(pipeline.canonical({"schema_version": 1, "profile": "building-art", "mode": "candidate",
            "status": "pass", "identity": value["candidate_identity"], "feedback_only": False}))
        approval = self.root / f"{kind}-{generation}.approval.json"
        approval.write_bytes(pipeline.canonical({"schema_version": 1, "decision": "building_release_approved",
            "candidate_identity": value["candidate_identity"], "evidence_sha256": pipeline.digest(evidence.read_bytes())}))
        return plan_path, assets, evidence, approval

    def test_all_nine_kinds_plan_apply_install_rollback_are_idempotent(self):
        for kind in pipeline.KINDS:
            with self.subTest(kind=kind):
                path, assets, evidence, approval = self.promotion(kind)
                value = pipeline.read(path)
                self.assertEqual(value, pipeline.plan(Path(value["source_root"]), assets, kind, self.binary))
                expected = pipeline.apply(path, self.binary, evidence, approval)
                self.assertEqual(expected, pipeline.apply(path, self.binary, evidence, approval))
                projected = json.loads(value["projection"]["manifest"])
                self.assert_numeric_release(assets, kind, projected)
                destination = self.root / "mirror"
                self.assertEqual(pipeline.install(assets, destination, kind, self.binary, execute=False)["status"], "planned")
                installed = pipeline.install(assets, destination, kind, self.binary, execute=True)
                self.assertEqual(installed, pipeline.install(assets, destination, kind, self.binary, execute=True))
                self.assert_numeric_release(destination, kind, projected)
                restored = pipeline.rollback(path, self.binary, execute=True)
                self.assertEqual(restored, pipeline.rollback(path, self.binary, execute=True))
                self.assertIsNone(pipeline.pointer(assets, kind))
                self.assertEqual(pipeline.install(assets, destination, kind, self.binary,
                    execute=True, allow_rollback=True)["status"], "rolled_back")
                self.assertEqual(pipeline.install(assets, destination, kind, self.binary,
                    execute=True, allow_rollback=True)["status"], "rolled_back")
                with self.assertRaisesRegex(ValueError, "cannot be reapplied"):
                    pipeline.apply(path, self.binary, evidence, approval)

    def test_recovery_replays_each_interruption_without_new_authority(self):
        for stage in ("after_snapshot", "after_payload", "after_authority", "after_locator"):
            kind = {"after_snapshot": "Tank", "after_payload": "MudMixer", "after_authority": "RestArea", "after_locator": "SoulSpa"}[stage]
            path, assets, evidence, approval = self.promotion(kind)
            def interrupt(current):
                if current == stage:
                    raise RuntimeError("interruption")
            with self.assertRaisesRegex(RuntimeError, "interruption"):
                pipeline.apply(path, self.binary, evidence, approval, interrupt)
            pipeline.recover(path, self.binary, execute=True)
            self.assertEqual(pipeline.recover(path, self.binary, execute=True)["status"], "applied")
            self.assertEqual(pipeline.pointer(assets, kind)["identity"]["authority"], "release_approved")
            self.assert_numeric_release(assets, kind, json.loads(pipeline.read(path)["projection"]["manifest"]))

    def test_rollback_restores_previous_generation_and_preserves_other_kind(self):
        first, assets, evidence, approval = self.promotion("Tank")
        pipeline.apply(first, self.binary, evidence, approval)
        destination = self.root / "mirror"
        pipeline.install(assets, destination, "Tank", self.binary, execute=True)
        previous = (assets / pipeline.locator("Tank")).read_bytes()
        previous_manifest = json.loads(previous)
        other, _, evidence, approval = self.promotion("BonePile")
        pipeline.apply(other, self.binary, evidence, approval)
        other_bytes = (assets / pipeline.locator("BonePile")).read_bytes()
        second, _, evidence, approval = self.promotion("Tank", 2)
        pipeline.apply(second, self.binary, evidence, approval)
        pipeline.install(assets, destination, "Tank", self.binary, execute=True)
        pipeline.rollback(second, self.binary, execute=True)
        with self.assertRaisesRegex(ValueError, "regression"):
            pipeline.install(assets, destination, "Tank", self.binary, execute=True)
        pipeline.install(assets, destination, "Tank", self.binary, execute=True, allow_rollback=True)
        self.assertEqual(previous, (destination / pipeline.locator("Tank")).read_bytes())
        self.assertEqual(previous, (assets / pipeline.locator("Tank")).read_bytes())
        self.assertEqual(other_bytes, (assets / pipeline.locator("BonePile")).read_bytes())
        self.assert_numeric_release(assets, "Tank", previous_manifest)
        self.assert_numeric_release(destination, "Tank", previous_manifest)

    def test_root_escape_symlinks_and_noncanonical_paths_fail_before_write(self):
        for path in ("../outside", "/outside", "a/../outside", "a//b", "a/./b", "a\\b", "url:x", "."):
            with self.subTest(path=path), self.assertRaises(ValueError):
                pipeline.rooted(self.root, path)
        outside = self.root / "outside"
        outside.mkdir()
        (self.root / "link").symlink_to(outside, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            pipeline.rooted(self.root, "link/mutation")
        self.assertEqual(list(outside.iterdir()), [])

    def test_tampered_approval_evidence_and_payload_fail_before_journal(self):
        path, assets, evidence, approval = self.promotion()
        original = approval.read_bytes()
        value = pipeline.read(approval)
        value["candidate_identity"]["kind"] = "BonePile"
        approval.write_bytes(pipeline.canonical(value))
        with self.assertRaisesRegex(ValueError, "approval/evidence"):
            pipeline.apply(path, self.binary, evidence, approval)
        self.assertFalse((assets / "transactions").exists())
        self.assertFalse((assets / "authority").exists())
        approval.write_bytes(original)
        value = pipeline.read(evidence)
        value["mode"] = "feedback"
        evidence.write_bytes(pipeline.canonical(value))
        with self.assertRaisesRegex(ValueError, "candidate evidence"):
            pipeline.apply(path, self.binary, evidence, approval)
        self.assertFalse((assets / "transactions").exists())
        self.assertFalse((assets / "authority").exists())
        source = Path(pipeline.read(path)["source_root"])
        manifest = json.loads((source / pipeline.locator("Tank")).read_bytes())
        (source / manifest["artifacts"][0]["path"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "bytes/hash"):
            pipeline.checked_plan(path, self.binary)

    def test_receipt_and_canonical_metadata_tampering_is_rejected(self):
        path, assets, evidence, approval = self.promotion()
        pipeline.apply(path, self.binary, evidence, approval)
        pointer = assets / "authority/building-tank.json"
        value = pipeline.read(pointer)
        value["identity"]["authority"] = "art_preview"
        pointer.write_bytes(pipeline.canonical(value))
        with self.assertRaisesRegex(ValueError, "active identity"):
            pipeline.install(assets, self.root / "mirror", "Tank", self.binary, execute=True)
        path.write_text(json.dumps(pipeline.read(path), indent=2))
        with self.assertRaisesRegex(ValueError, "canonical"):
            pipeline.checked_plan(path, self.binary)

    def test_completed_recovery_and_rollback_do_not_need_staging(self):
        path, assets, evidence, approval = self.promotion()
        pipeline.apply(path, self.binary, evidence, approval)
        source = Path(pipeline.read(path)["source_root"])
        source.rename(self.root / "retired-staging")
        self.assertEqual(pipeline.recover(path, self.binary, execute=True)["status"], "applied")
        self.assert_numeric_release(assets, "Tank", json.loads(pipeline.read(path)["projection"]["manifest"]))
        self.assertEqual(pipeline.rollback(path, self.binary, execute=True)["status"], "rolled_back")
        self.assertIsNone(pipeline.pointer(assets, "Tank"))

    def test_legacy_dispatch_without_kind_keeps_existing_entrypoints(self):
        with patch.object(legacy_promote, "build_parser", side_effect=RuntimeError("legacy parser")):
            with patch.object(sys, "argv", ["promote", "plan"]), self.assertRaisesRegex(RuntimeError, "legacy parser"):
                legacy_promote.main()
        with patch.object(pipeline, "main", return_value=0) as dispatch:
            with patch.object(sys, "argv", ["install", "--kind", "Tank"]):
                self.assertEqual(legacy_install.main(), 0)
            dispatch.assert_called_once_with(["install", "--kind", "Tank"])


if __name__ == "__main__":
    unittest.main()
