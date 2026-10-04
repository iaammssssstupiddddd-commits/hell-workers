"""Synthetic transaction fixtures; Rust tests own actual codec validation."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

MODULE = Path(__file__).resolve().parents[2] / "tools/blender_ai_workflow/scripts/building_asset_pipeline.py"
SPEC = importlib.util.spec_from_file_location("m2_pipeline_fixture", MODULE)
pipeline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pipeline)


def fixture_codec(_binary, operation, text, receipt=None):
    manifest = json.loads(text)
    if operation == "validate":
        return {"manifest": text, "receipt": receipt}
    manifest["identity"]["manifest_sha256"] = ""
    manifest["receipt"] = None
    manifest["identity"]["manifest_sha256"] = pipeline.digest(pipeline.canonical(manifest))
    if manifest["identity"]["authority"] == "release_approved":
        record = {"schema_version": 1, "identity": manifest["identity"],
                  "art_approval_sha256": manifest["art_approval_sha256"], "decision": "release_approved"}
        if "numeric_approval_sha256" in manifest:
            record["numeric_approval_sha256"] = manifest["numeric_approval_sha256"]
        receipt = pipeline.canonical(record).decode()
        relative = f"building_sets/{pipeline.KINDS[manifest['identity']['kind']]['slug']}/{manifest['identity']['generation']}/{pipeline.digest(receipt.encode())}.json"
        manifest["receipt"] = {"role": "authority:receipt", **pipeline.record(relative, receipt.encode())}
    return {"manifest": pipeline.canonical(manifest).decode(), "receipt": receipt}


class M2ReleasePipelineTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.binary = self.root / "codec"
        self.binary.write_bytes(b"protocol fixture, not executable")
        codec = patch.object(pipeline, "codec", side_effect=fixture_codec)
        codec.start()
        self.addCleanup(codec.stop)

    def inputs(self, kind):
        source = self.root / kind
        source.mkdir()
        records = []
        for role in [*("mesh:" + r for r in pipeline.KINDS[kind]["mesh_roles"]),
                     *("image:" + r for r in pipeline.KINDS[kind]["image_roles"])]:
            name = role.replace(":", "-")
            (source / name).write_bytes(role.encode())
            records.append({"role": role, **pipeline.record(name, role.encode())})
        for name in ("source", "geometry"):
            (source / name).write_bytes(name.encode())
        recipe = {"schema_version": 1, "kind": kind, "generation": 1,
                  "source": pipeline.record("source", b"source"), "geometry_contract": pipeline.record("geometry", b"geometry"),
                  "artifacts": records, "parts": [], "world_preview": {}, "catalog_preview": {}}
        recipe_path = source / "recipe.json"
        recipe_path.write_bytes(pipeline.canonical(recipe))
        preview = pipeline.export(recipe_path, source, source / "preview", self.binary, authority="art_preview")
        art = source / "art.json"
        art.write_bytes(pipeline.canonical({"schema_version": 1, "decision": "building_art_approved",
                                           "art_preview_identity": preview["identity"]}))
        state = {"kind": "Tank", "partial_y_wu": 9.0, "full_y_wu": 19.0} if kind == "Tank" else {
            "kind": "MudMixer", "axis": [0.0, 0.0, 1.0], "radians_per_second": 0.7}
        numeric = source / "numeric.json"
        numeric.write_bytes(pipeline.canonical({"schema_version": 1, "decision": "building_numeric_approved",
            "art_preview_identity": preview["identity"], "art_approval_sha256": pipeline.digest(art.read_bytes()),
            "production_state": state}))
        return source, recipe_path, art, numeric

    def export(self, source, recipe, art, numeric, output):
        return pipeline.export(recipe, source, output, self.binary, authority="isolated_candidate",
                               approval_path=art, numeric_approval_path=numeric)

    def test_m2_provenance_survives_projection_recovery_install(self):
        for kind in ("Tank", "MudMixer"):
            source, recipe, art, numeric = self.inputs(kind)
            candidate = self.root / (kind + "-candidate")
            self.export(source, recipe, art, numeric, candidate)
            assets = self.root / (kind + "-assets")
            value = pipeline.plan(candidate, assets, kind, self.binary)
            projected = json.loads(value["projection"]["manifest"])
            receipt = json.loads(value["projection"]["receipt"])
            self.assertEqual(projected["production_state"], pipeline.read(numeric)["production_state"])
            self.assertEqual(receipt["numeric_approval_sha256"], pipeline.digest(numeric.read_bytes()))
            path = self.root / (kind + ".plan.json")
            path.write_bytes(pipeline.canonical(value))
            evidence = self.root / (kind + ".evidence.json")
            evidence.write_bytes(pipeline.canonical({"schema_version": 1, "profile": "building-art",
                "mode": "candidate", "status": "pass", "identity": value["candidate_identity"], "feedback_only": False}))
            approval = self.root / (kind + ".release.json")
            approval.write_bytes(pipeline.canonical({"schema_version": 1, "decision": "building_release_approved",
                "candidate_identity": value["candidate_identity"], "evidence_sha256": pipeline.digest(evidence.read_bytes())}))

            def interrupt(stage):
                if stage == "after_locator":
                    raise RuntimeError("fixture interruption")

            with patch.object(pipeline, "verify_candidate_evidence", side_effect=pipeline.read):
                with self.assertRaises(RuntimeError):
                    pipeline.apply(path, self.binary, evidence, approval, interrupt)
                pipeline.recover(path, self.binary, execute=True)
            pipeline.checked_plan(path, self.binary, source_required=False)
            mirror = self.root / (kind + "-mirror")
            pipeline.install(assets, mirror, kind, self.binary, execute=True)
            installed, _, _ = pipeline.load_set(mirror, kind, self.binary)
            self.assertEqual(installed["production_state"], projected["production_state"])
            self.assertEqual(installed["numeric_approval_sha256"], projected["numeric_approval_sha256"])
            provenance = assets / f"building_sets/{pipeline.KINDS[kind]['slug']}/1/provenance/numeric-approval.json"
            provenance.write_bytes(pipeline.canonical({}))
            with self.assertRaises(ValueError):
                pipeline.checked_plan(path, self.binary, source_required=False)

    def test_missing_stale_cross_kind_and_malformed_decisions_write_nothing(self):
        source, recipe, art, numeric = self.inputs("Tank")
        original = pipeline.read(numeric)
        for name, change in (
            ("stale", lambda n: n["art_preview_identity"].update(generation=2)),
            ("art", lambda n: n.update(art_approval_sha256="0" * 64)),
            ("kind", lambda n: n["production_state"].update(kind="MudMixer")),
            ("decision", lambda n: n.update(decision="technical_pass")),
            ("extra", lambda n: n.update(approved=True)),
        ):
            altered = copy.deepcopy(original)
            change(altered)
            numeric.write_bytes(pipeline.canonical(altered))
            output = self.root / name
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.export(source, recipe, art, numeric, output)
            self.assertFalse(output.exists())
        for approval in (None, numeric):
            if approval is not None:
                numeric.write_text('{"production_state":NaN}')
            output = self.root / "absent-or-nonfinite"
            with self.assertRaises(ValueError):
                self.export(source, recipe, art, approval, output)
            self.assertFalse(output.exists())
        numeric.write_bytes(pipeline.canonical(original))
        with self.assertRaises(ValueError):
            pipeline.export(recipe, source, self.root / "draft", self.binary, authority="art_preview",
                            numeric_approval_path=numeric)
        self.assertFalse((self.root / "draft").exists())

    def test_approved_values_change_identity_without_changing_preview(self):
        source, recipe, art, numeric = self.inputs("Tank")
        first = self.export(source, recipe, art, numeric, self.root / "first")
        changed = pipeline.read(numeric)
        changed["production_state"]["full_y_wu"] = 20.0
        numeric.write_bytes(pipeline.canonical(changed))
        second = self.export(source, recipe, art, numeric, self.root / "second")
        self.assertNotEqual(first["identity"], second["identity"])
        manifest, _, _ = pipeline.load_set(self.root / "second", "Tank", self.binary)
        self.assertEqual(pipeline.preview_projection(manifest, self.binary)["identity"],
                         pipeline.read(art)["art_preview_identity"])

    def test_non_m2_omits_numeric_fields_and_rejects_numeric_input(self):
        source, recipe, art, numeric = self.inputs("RestArea")
        output = self.root / "rest-candidate"
        self.export(source, recipe, art, None, output)
        manifest, _, _ = pipeline.load_set(output, "RestArea", self.binary)
        self.assertNotIn("production_state", manifest)
        self.assertNotIn("numeric_approval_sha256", manifest)
        with self.assertRaises(ValueError):
            self.export(source, recipe, art, numeric, self.root / "bad-rest")
        self.assertFalse((self.root / "bad-rest").exists())

    def test_runtime_codec_rejection_precedes_all_destination_writes(self):
        source, recipe, art, numeric = self.inputs("Tank")
        decision = pipeline.read(numeric)
        decision["production_state"]["full_y_wu"] = -1.0
        numeric.write_bytes(pipeline.canonical(decision))

        def refusing_codec(binary, operation, text, receipt=None):
            manifest = json.loads(text)
            if manifest["identity"]["authority"] == "isolated_candidate":
                self.assertEqual(manifest["production_state"], decision["production_state"])
                raise ValueError("runtime codec rejected numeric bounds")
            return fixture_codec(binary, operation, text, receipt)

        with patch.object(pipeline, "codec", side_effect=refusing_codec):
            with self.assertRaisesRegex(ValueError, "numeric bounds"):
                self.export(source, recipe, art, numeric, self.root / "bad-bounds")
        self.assertFalse((self.root / "bad-bounds").exists())


if __name__ == "__main__":
    unittest.main()
