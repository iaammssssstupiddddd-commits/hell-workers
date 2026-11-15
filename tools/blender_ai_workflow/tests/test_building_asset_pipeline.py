"""Transaction tests isolate the Rust codec; Rust projection tests own its ABI.

The byte payloads below are fixtures, never actual art or release evidence.
"""
from __future__ import annotations

import json
import contextlib
import io
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
    def test_uv_ink_survives_export_and_rejects_old_art_approval_and_dropping_codec(self):
        self.candidate("MudMixer")
        source = self.root / "author-MudMixer-1"
        recipe = source / "recipe.json"
        value = pipeline.read(recipe)
        value["surface_paint"] = {"method": "uv_ink_v1"}
        recipe.write_bytes(pipeline.canonical(value))
        output = self.root / "uv-ink-preview"
        pipeline.export(recipe, source, output, self.binary, authority="art_preview")
        decoded = pipeline.read(output / pipeline.locator("MudMixer"))
        self.assertEqual(decoded["surface_paint"], value["surface_paint"])
        rejected = self.root / "unreviewed-uv-ink"
        with self.assertRaises(ValueError):
            pipeline.export(recipe, source, rejected, self.binary, authority="isolated_candidate",
                            approval_path=source / "art-approval.json", numeric_approval_path=source / "numeric-approval.json")
        self.assertFalse(rejected.exists())
        def dropping(binary, operation, text, receipt=None):
            manifest = json.loads(text); manifest.pop("surface_paint", None)
            return fixture_codec(binary, operation, json.dumps(manifest), receipt)
        with patch.object(pipeline, "codec", side_effect=dropping), self.assertRaises(ValueError):
            pipeline.export(recipe, source, rejected, self.binary, authority="art_preview")
        self.assertFalse(rejected.exists())

    def body_paint_recipe(self):
        self.candidate("Tank")
        source = self.root / "author-Tank-1"
        path = source / "recipe.json"
        value = pipeline.read(path)
        value["body_paint_projection"] = {"method": "world_preview_v1",
                                         "matte_linear_rgb": [0.125, 0.09375, 0.078125]}
        path.write_bytes(pipeline.canonical(value))
        return source, path, value

    def test_body_paint_metadata_survives_codec_without_borrowing_old_art_approval(self):
        source, recipe, value = self.body_paint_recipe()
        output = self.root / "body-paint-preview"
        pipeline.export(recipe, source, output, self.binary, authority="art_preview")
        decoded = pipeline.read(output / pipeline.locator("Tank"))
        self.assertEqual(decoded["body_paint_projection"], value["body_paint_projection"])
        self.assertEqual([r["role"] for r in decoded["artifacts"]], [r["role"] for r in value["artifacts"]])
        self.assertIsNone(decoded["art_approval_sha256"])
        rejected = self.root / "unreviewed-projection"
        with self.assertRaises(ValueError):
            pipeline.export(recipe, source, rejected, self.binary, authority="isolated_candidate",
                            approval_path=source / "art-approval.json",
                            numeric_approval_path=source / "numeric-approval.json")
        self.assertFalse(rejected.exists())

    def test_codec_cannot_drop_projection_or_accept_null_as_an_opt_in(self):
        source, recipe, value = self.body_paint_recipe()
        def dropping(binary, operation, text, receipt=None):
            manifest = json.loads(text)
            manifest.pop("body_paint_projection", None)
            return fixture_codec(binary, operation, json.dumps(manifest), receipt)
        rejected = self.root / "dropped-projection"
        with patch.object(pipeline, "codec", side_effect=dropping), self.assertRaises(ValueError):
            pipeline.export(recipe, source, rejected, self.binary, authority="art_preview")
        self.assertFalse(rejected.exists())
        value["body_paint_projection"] = None
        recipe.write_bytes(pipeline.canonical(value))
        with self.assertRaises(ValueError):
            pipeline.export(recipe, source, rejected, self.binary, authority="art_preview")
        self.assertFalse(rejected.exists())

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
        numeric_module = (pipeline.building_m3_numeric if kind in pipeline.building_m3_numeric.KINDS else
                          pipeline.building_m5_numeric if kind in pipeline.building_m5_numeric.KINDS else None)
        geometry = numeric_module.CONTRACT.read_bytes() if numeric_module else b"geometry fixture"
        (source / "geometry").write_bytes(geometry)
        recipe = {"schema_version": 1, "kind": kind, "generation": generation,
            "source": pipeline.record("source", b"source fixture"),
            "geometry_contract": pipeline.record("geometry", geometry),
            "artifacts": entries, "parts": [], "world_preview": {}, "catalog_preview": {}}
        if kind in pipeline.building_m5_numeric.KINDS:
            numeric = pipeline.building_m5_numeric.numeric_contract(geometry, kind)
            common = {key: numeric[key] for key in ("canvas_px", "canvas_wu", "anchor_px", "representative_state")}
            for key in ("canvas_wu", "anchor_px"):
                common[key] = [float(value) for value in common[key]]
            recipe["world_preview"] = {"image_role": numeric["world_roles"][0], **common}
            recipe["catalog_preview"] = {"image_role": "catalog", **common}
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
        elif numeric_module is not None:
            numeric_path = source / "numeric-approval.json"
            numeric_path.write_bytes(pipeline.canonical({"schema_version": 1,
                "decision": ("building_m5_numeric_approved" if kind in pipeline.building_m5_numeric.KINDS
                             else "building_m3_numeric_approved"), "art_preview_identity": preview["identity"],
                "art_approval_sha256": pipeline.digest(approval_path.read_bytes()),
                "geometry_contract_sha256": pipeline.digest(geometry),
                "numeric_contract": numeric_module.numeric_contract(geometry, kind)}))
        output = self.root / f"candidate-{kind}-{generation}"
        pipeline.export(recipe_path, source, output, self.binary, authority="isolated_candidate",
                        approval_path=approval_path, numeric_approval_path=numeric_path)
        manifest = pipeline.read(output / pipeline.locator(kind))
        if numeric_path is not None:
            if kind in {"Tank", "MudMixer"}:
                self.assertEqual(manifest["production_state"], state)
            else:
                self.assertNotIn("production_state", manifest)
            self.assertEqual(manifest["numeric_approval_sha256"], pipeline.digest(numeric_path.read_bytes()))
        else:
            self.assertNotIn("production_state", manifest)
            self.assertNotIn("numeric_approval_sha256", manifest)
        return output

    def assert_numeric_release(self, root, kind, expected):
        manifest, _, receipt_text = pipeline.load_set(root, kind, self.binary)
        receipt = json.loads(receipt_text)
        if kind in pipeline.NUMERIC_KINDS:
            self.assertEqual(manifest.get("production_state"), expected.get("production_state"))
            self.assertEqual(manifest["numeric_approval_sha256"], expected["numeric_approval_sha256"])
            self.assertEqual(receipt["numeric_approval_sha256"], expected["numeric_approval_sha256"])
            numeric_path = root / f"building_sets/{pipeline.KINDS[kind]['slug']}/{manifest['identity']['generation']}/provenance/numeric-approval.json"
            self.assertEqual(pipeline.digest(numeric_path.read_bytes()), expected["numeric_approval_sha256"])
        else:
            self.assertNotIn("production_state", manifest)
            self.assertNotIn("numeric_approval_sha256", manifest)
            self.assertNotIn("numeric_approval_sha256", receipt)

    def test_art_approval_generation_is_exact_json_not_python_numeric_equality(self):
        for kind in ("Tank", "RestArea", "SoulSpa"):
            self.candidate(kind)
            source = self.root / f"author-{kind}-1"
            original_art = pipeline.read(source / "art-approval.json")
            original_numeric = pipeline.read(source / "numeric-approval.json")
            for index, value in enumerate((True, 1.0)):
                art = json.loads(json.dumps(original_art))
                art["art_preview_identity"]["generation"] = value
                (source / "art-approval.json").write_bytes(pipeline.canonical(art))
                # The numeric identity remains the correct integer; rebinding
                # its art hash must not sanitize the malformed art decision.
                numeric = {**original_numeric, "art_approval_sha256": pipeline.digest(pipeline.canonical(art))}
                (source / "numeric-approval.json").write_bytes(pipeline.canonical(numeric))
                output = self.root / f"bad-art-{kind}-{index}"
                with self.subTest(kind=kind, value=value), self.assertRaises(ValueError):
                    pipeline.export(source / "recipe.json", source, output, self.binary,
                                    authority="isolated_candidate", approval_path=source / "art-approval.json",
                                    numeric_approval_path=source / "numeric-approval.json")
                self.assertFalse(output.exists())

    def test_m2_numeric_identity_and_values_reject_boolean_aliases(self):
        root = self.candidate("Tank")
        decision = pipeline.read(root / "provenance/numeric-approval.json")
        identity = dict(decision["art_preview_identity"])
        for value in (True, 1.0):
            changed = {**decision, "art_preview_identity": {**identity, "generation": value}}
            with self.subTest(value=value), self.assertRaises(ValueError):
                pipeline.approved_numeric_state(changed, identity, decision["art_approval_sha256"])
        changed = {**decision, "production_state": {"kind": "Tank", "partial_y_wu": 0.5, "full_y_wu": True}}
        with self.assertRaises(ValueError):
            pipeline.approved_numeric_state(changed, identity, decision["art_approval_sha256"])

    def test_release_approval_generation_rejects_numeric_alias_before_promotion(self):
        for generation, value in ((1, True), (2, 2.0)):
            path, assets, evidence, approval = self.promotion("Tank", generation)
            changed = pipeline.read(approval)
            changed["candidate_identity"]["generation"] = value
            approval.write_bytes(pipeline.canonical(changed))
            with self.subTest(value=value), self.assertRaises(ValueError):
                pipeline.apply(path, self.binary, evidence, approval)
            self.assertIsNone(pipeline.pointer(assets, "Tank"))

    def test_rehashed_art_provenance_still_rejects_numeric_identity_aliases(self):
        for kind in ("Tank", "RestArea", "SoulSpa", "SandPile"):
            for generation, value in ((1, True), (2, 2.0)):
                root = self.candidate(kind, generation)
                art_path = root / "provenance/art-approval.json"
                art = pipeline.read(art_path)
                art["art_preview_identity"]["generation"] = value
                art_path.write_bytes(pipeline.canonical(art))
                manifest_path = root / pipeline.locator(kind)
                manifest = pipeline.read(manifest_path)
                manifest["art_approval_sha256"] = pipeline.digest(art_path.read_bytes())
                numeric_path = root / "provenance/numeric-approval.json"
                if numeric_path.exists():
                    numeric = pipeline.read(numeric_path)
                    numeric["art_approval_sha256"] = manifest["art_approval_sha256"]
                    numeric_path.write_bytes(pipeline.canonical(numeric))
                    manifest["numeric_approval_sha256"] = pipeline.digest(numeric_path.read_bytes())
                manifest_path.write_text(fixture_codec(self.binary, "seal", json.dumps(manifest))["manifest"])
                with self.subTest(kind=kind, value=value, route="plan"), self.assertRaises(ValueError):
                    pipeline.plan(root, self.root / "assets", kind, self.binary)
                with self.subTest(kind=kind, value=value, route="verify"), self.assertRaises(ValueError):
                    pipeline.verify_set(root, kind, self.binary)

    def test_candidate_evidence_identity_rejects_numeric_alias_with_rebound_approval(self):
        for generation, value in ((1, True), (2, 2.0)):
            path, assets, evidence, approval = self.promotion("Tank", generation)
            changed = pipeline.read(evidence)
            changed["identity"]["generation"] = value
            evidence.write_bytes(pipeline.canonical(changed))
            release = pipeline.read(approval)
            release["evidence_sha256"] = pipeline.digest(evidence.read_bytes())
            approval.write_bytes(pipeline.canonical(release))
            with self.subTest(value=value), self.assertRaises(ValueError):
                pipeline.apply(path, self.binary, evidence, approval)
            self.assertIsNone(pipeline.pointer(assets, "Tank"))

    def test_install_rejects_rehashed_release_approval_identity_alias(self):
        for kind, value in (("Tank", True), ("RestArea", 1.0)):
            path, assets, evidence, approval = self.promotion(kind)
            pipeline.apply(path, self.binary, evidence, approval)
            active = pipeline.pointer(assets, kind)
            slug = pipeline.KINDS[kind]["slug"]
            journal_path = assets / f"transactions/building-{slug}-1.json"
            journal = pipeline.read(journal_path)
            journal["approval"]["candidate_identity"]["generation"] = value
            active["approval_sha256"] = pipeline.digest(pipeline.canonical(journal["approval"]))
            journal["new"] = active
            journal_path.write_bytes(pipeline.canonical(journal))
            (assets / f"authority/building-{slug}.json").write_bytes(pipeline.canonical(active))
            destination = self.root / f"bad-mirror-{kind}"
            with self.subTest(kind=kind, value=value), self.assertRaises(ValueError):
                pipeline.install(assets, destination, kind, self.binary, execute=True)
            self.assertFalse((destination / pipeline.locator(kind)).exists())

    def test_apply_rejects_coordinated_plan_evidence_and_approval_aliases(self):
        for kind, value in (("Tank", True), ("RestArea", 1.0)):
            path, assets, evidence, approval = self.promotion(kind)
            plan = pipeline.read(path)
            plan["candidate_identity"]["generation"] = value
            path.write_bytes(pipeline.canonical(plan))
            sample = pipeline.read(evidence)
            sample["identity"]["generation"] = value
            evidence.write_bytes(pipeline.canonical(sample))
            decision = pipeline.read(approval)
            decision["candidate_identity"]["generation"] = value
            decision["evidence_sha256"] = pipeline.digest(evidence.read_bytes())
            approval.write_bytes(pipeline.canonical(decision))
            with self.subTest(kind=kind, value=value), self.assertRaises(ValueError):
                pipeline.checked_plan(path, self.binary)
            with self.assertRaises(ValueError):
                pipeline.apply(path, self.binary, evidence, approval)
            self.assertIsNone(pipeline.pointer(assets, kind))

    def test_install_rejects_coordinated_rehashed_journal_identity_aliases(self):
        for kind, value in (("Tank", True), ("RestArea", 1.0)):
            path, assets, evidence, approval = self.promotion(kind)
            pipeline.apply(path, self.binary, evidence, approval)
            active = pipeline.pointer(assets, kind)
            slug = pipeline.KINDS[kind]["slug"]
            journal_path = assets / f"transactions/building-{slug}-1.json"
            journal = pipeline.read(journal_path)
            for container, key in ((journal["plan"], "candidate_identity"),
                                   (journal["evidence"], "identity"), (journal["approval"], "candidate_identity")):
                container[key]["generation"] = value
            active["plan_sha256"] = pipeline.digest(pipeline.canonical(journal["plan"]))
            active["evidence_sha256"] = pipeline.digest(pipeline.canonical(journal["evidence"]))
            journal["approval"]["evidence_sha256"] = active["evidence_sha256"]
            active["approval_sha256"] = pipeline.digest(pipeline.canonical(journal["approval"]))
            journal["plan_sha256"] = active["plan_sha256"]
            journal["new"] = active
            journal_path.write_bytes(pipeline.canonical(journal))
            (assets / f"authority/building-{slug}.json").write_bytes(pipeline.canonical(active))
            destination = self.root / f"coordinated-bad-mirror-{kind}"
            with self.subTest(kind=kind, value=value), self.assertRaises(ValueError):
                pipeline.install(assets, destination, kind, self.binary, execute=True)
            self.assertFalse((destination / pipeline.locator(kind)).exists())

    def test_m3_export_requires_separate_exact_numeric_decision_before_writes(self):
        for kind in ("RestArea", "SoulSpa"):
            self.candidate(kind)
            source = self.root / f"author-{kind}-1"
            output = self.root / f"rejected-{kind}"
            for field, value in ((None, None), ("art_approval_sha256", "f" * 64),
                                 ("geometry_contract_sha256", "e" * 64)):
                original = pipeline.read(source / "numeric-approval.json")
                numeric_path = None
                if field is not None:
                    numeric_path = source / "changed-decision.json"
                    numeric_path.write_bytes(pipeline.canonical({**original, field: value}))
                with self.subTest(kind=kind, field=field), self.assertRaises(ValueError):
                    pipeline.export(source / "recipe.json", source, output, self.binary,
                                    authority="isolated_candidate", approval_path=source / "art-approval.json",
                                    numeric_approval_path=numeric_path)
                self.assertFalse(output.exists())

    def test_m3_provenance_tamper_blocks_planning(self):
        for kind in ("RestArea", "SoulSpa"):
            candidate = self.candidate(kind)
            path = candidate / "provenance/numeric-approval.json"
            original = path.read_bytes()
            changed = pipeline.read(path)
            changed["numeric_contract"]["geometry_scale"] = 33
            path.write_bytes(pipeline.canonical(changed))
            with self.assertRaises(ValueError):
                pipeline.plan(candidate, self.root / "assets", kind, self.binary)
            path.write_bytes(original)
            # Missing decision is not repaired or synthesized from manifest digest.
            path.unlink()
            with self.assertRaises(FileNotFoundError):
                pipeline.plan(candidate, self.root / "assets", kind, self.binary)

    def test_m5_numeric_decision_is_required_before_writes_and_rechecked_on_plan(self):
        for kind in sorted(pipeline.building_m5_numeric.KINDS):
            candidate = self.candidate(kind)
            source = self.root / f"author-{kind}-1"
            output = self.root / f"rejected-{kind}"
            for numeric_path in (None, source / "art-approval.json"):
                with self.subTest(kind=kind, path=numeric_path), self.assertRaises(ValueError):
                    pipeline.export(source / "recipe.json", source, output, self.binary,
                                    authority="isolated_candidate", approval_path=source / "art-approval.json",
                                    numeric_approval_path=numeric_path)
                self.assertFalse(output.exists())
            path = candidate / "provenance/numeric-approval.json"
            changed = pipeline.read(path)
            changed["numeric_contract"]["anchor_px"][0] += 1
            path.write_bytes(pipeline.canonical(changed))
            # Rehashing only the manifest must not make changed numeric values valid.
            manifest_path = candidate / pipeline.locator(kind)
            manifest = pipeline.read(manifest_path)
            manifest["numeric_approval_sha256"] = pipeline.digest(path.read_bytes())
            manifest_path.write_text(fixture_codec(self.binary, "seal", json.dumps(manifest))["manifest"])
            with self.assertRaises(ValueError):
                pipeline.plan(candidate, self.root / "assets", kind, self.binary)

    def promotion(self, kind="Tank", generation=1):
        candidate = self.candidate(kind, generation)
        assets = self.root / "assets"
        value = pipeline.plan(candidate, assets, kind, self.binary)
        candidate_manifest = pipeline.read(candidate / pipeline.locator(kind))
        projected = json.loads(value["projection"]["manifest"])
        receipt = json.loads(value["projection"]["receipt"])
        if kind in pipeline.NUMERIC_KINDS:
            self.assertEqual(projected.get("production_state"), candidate_manifest.get("production_state"))
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

    def test_read_only_verification_preserves_files_and_cannot_promote(self):
        root = self.candidate("RestArea")
        before = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}
        result = pipeline.verify_set(root, "RestArea", self.binary)
        self.assertEqual(result["scope"], "packaged-byte-validity-only")
        self.assertFalse(result["promotion_authority"])
        self.assertEqual(before, {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()})
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(pipeline.main(["verify", "--kind", "RestArea", "--source-root", str(root),
                                           "--codec", str(self.binary)]), 0)
        with self.assertRaisesRegex(ValueError, "read-only"):
            pipeline.main(["verify", "--kind", "RestArea", "--source-root", str(root),
                           "--codec", str(self.binary), "--apply"])

    def test_read_only_verification_rejects_malformed_approval_metadata(self):
        root = self.candidate("RestArea")
        approval = root / "provenance/art-approval.json"
        original = approval.read_bytes()
        record = pipeline.read(approval)
        boolean_schema = {**record, "schema_version": True}
        for payload in (pipeline.canonical(boolean_schema), original + b"\n",
                        original.rstrip()[:-1] + b',"schema_version":1}'):
            with self.subTest(payload=payload):
                approval.write_bytes(payload)
                path = root / pipeline.locator("RestArea")
                manifest = pipeline.read(path)
                manifest["art_approval_sha256"] = pipeline.digest(payload)
                path.write_bytes(pipeline.canonical(manifest))
                with self.assertRaisesRegex(ValueError, "metadata"):
                    pipeline.verify_set(root, "RestArea", self.binary)

    def test_read_only_verification_rejects_source_and_art_identity_drift(self):
        root = self.candidate("RestArea")
        source = root / "provenance/source"
        original = source.read_bytes()
        source.write_bytes(b"changed source")
        with self.assertRaisesRegex(ValueError, "source provenance"):
            pipeline.verify_set(root, "RestArea", self.binary)
        source.write_bytes(original)
        approval = root / "provenance/art-approval.json"
        record = pipeline.read(approval)
        record["art_preview_identity"]["generation"] += 1
        approval.write_bytes(pipeline.canonical(record))
        path = root / pipeline.locator("RestArea")
        manifest = pipeline.read(path)
        manifest["art_approval_sha256"] = pipeline.digest(approval.read_bytes())
        path.write_bytes(pipeline.canonical(manifest))
        with self.assertRaisesRegex(ValueError, "approval identity"):
            pipeline.verify_set(root, "RestArea", self.binary)


if __name__ == "__main__":
    unittest.main()
