"""Synthetic M5 decisions only; passing tests never grant art/numeric authority."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_m5_numeric as numeric


class M5NumericTests(unittest.TestCase):
    def fixture(self, kind):
        raw = numeric.CONTRACT.read_bytes()
        preview = {"kind": kind, "generation": 1, "authority": "art_preview", "manifest_sha256": "a" * 64}
        values = numeric.numeric_contract(raw, kind)
        approval = {"schema_version": 1, "decision": "building_m5_numeric_approved",
                    "art_preview_identity": preview, "art_approval_sha256": "b" * 64,
                    "geometry_contract_sha256": hashlib.sha256(raw).hexdigest(), "numeric_contract": values}
        common = {key: values[key] for key in ("canvas_px", "canvas_wu", "anchor_px", "representative_state")}
        for key in ("canvas_wu", "anchor_px"):
            common[key] = [float(value) for value in common[key]]
        manifest = {"world_preview": {"image_role": values["world_roles"][0], **common},
                    "catalog_preview": {"image_role": "catalog", **common}, "parts": [],
                    "artifacts": [{"role": "image:" + role} for role in values["world_roles"] + ["catalog"]]}
        return raw, preview, approval, manifest

    def test_tall_lamp_revision_keeps_historical_contract_and_separate_catalog_anchor(self):
        old = numeric.numeric_contract(numeric.CONTRACT.read_bytes(), "OutdoorLamp")
        raw = numeric.CONTRACT.with_name("building-m5-v2.contract.json").read_bytes()
        revised = numeric.numeric_contract(raw, "OutdoorLamp")
        self.assertEqual(old["anchor_px"], [128, 128])
        self.assertEqual(revised["anchor_px"], [128, 208])
        self.assertEqual(revised["canvas_wu"], [64, 64])
        preview = {"kind": "OutdoorLamp", "generation": 2, "authority": "art_preview", "manifest_sha256": "a"*64}
        approval = {"schema_version": 1, "decision": "building_m5_numeric_approved",
                    "art_preview_identity": preview, "art_approval_sha256": "b"*64,
                    "geometry_contract_sha256": hashlib.sha256(raw).hexdigest(), "numeric_contract": revised}
        common = {key: revised[key] for key in ("canvas_px", "canvas_wu", "anchor_px", "representative_state")}
        for key in ("canvas_wu", "anchor_px"):
            common[key] = list(map(float, common[key]))
        manifest = {"world_preview": {"image_role": "world_off", **common},
                    "catalog_preview": {"image_role": "catalog", **common, "anchor_px": [128.0, 128.0]},
                    "parts": [], "artifacts": [{"role": "image:"+role} for role in ("world_off", "world_on", "catalog")]}
        numeric.validate(approval, preview, "b"*64, raw, manifest)
        with self.assertRaises(ValueError):
            numeric.validate(approval, preview, "b"*64, raw,
                             {**manifest, "catalog_preview": {**manifest["catalog_preview"], "anchor_px": [128.0,208.0]}})

    def test_exact_four_kinds_and_explicit_numeric_decisions(self):
        for kind in sorted(numeric.KINDS):
            raw, preview, approval, manifest = self.fixture(kind)
            numeric.validate(approval, preview, "b" * 64, raw, manifest)
            self.assertEqual(json.loads(raw)["status"], "unapproved-production-proposal")
            for field, value in (("schema_version", True), ("schema_version", 1.0), ("decision", "approved"),
                                 ("art_approval_sha256", "f" * 64), ("geometry_contract_sha256", "c" * 64)):
                with self.subTest(kind=kind, field=field), self.assertRaises(ValueError):
                    numeric.validate({**approval, field: value}, preview, "b" * 64, raw, manifest)
            for field in approval:
                changed = dict(approval)
                del changed[field]
                with self.assertRaises(ValueError):
                    numeric.validate(changed, preview, "b" * 64, raw, manifest)
            with self.assertRaises(ValueError):
                numeric.validate({**approval, "extra": 1}, preview, "b" * 64, raw, manifest)

    def test_identity_aliases_and_changed_original_cannot_reuse_decision(self):
        raw, preview, approval, manifest = self.fixture("SandPile")
        for field, value in (("generation", True), ("generation", 1.0), ("generation", 2),
                             ("kind", "BonePile"), ("authority", "isolated_candidate"),
                             ("manifest_sha256", "f" * 64)):
            changed = copy.deepcopy(approval)
            changed["art_preview_identity"][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                numeric.validate(changed, preview, "b" * 64, raw, manifest)
        changed = json.loads(raw)
        changed["canvas_px"][0] = True
        changed = numeric.canonical(changed)
        with self.assertRaises(ValueError):
            numeric.validate({**approval, "geometry_contract_sha256": hashlib.sha256(changed).hexdigest()},
                             preview, "b" * 64, changed, manifest)
        with self.assertRaises(ValueError):
            numeric.numeric_contract(raw.replace(b'"schema_version": 1', b'"schema_version": 1, "schema_version": 1'), "SandPile")

    def test_numeric_values_and_actual_descriptors_cannot_drift(self):
        for kind in sorted(numeric.KINDS):
            raw, preview, approval, manifest = self.fixture(kind)
            for field in ("canvas_px", "canvas_wu", "anchor_px"):
                changed = copy.deepcopy(approval)
                changed["numeric_contract"][field][0] = True
                with self.assertRaises(ValueError):
                    numeric.validate(changed, preview, "b" * 64, raw, manifest)
                for descriptor in ("world_preview", "catalog_preview"):
                    altered = copy.deepcopy(manifest)
                    altered[descriptor][field][0] += 1
                    with self.assertRaises(ValueError):
                        numeric.validate(approval, preview, "b" * 64, raw, altered)
            for field, value in (("parts", [{}]), ("production_state", {"kind": "Tank"}),
                                 ("artifacts", manifest["artifacts"][::-1])):
                with self.assertRaises(ValueError):
                    numeric.validate(approval, preview, "b" * 64, raw, {**manifest, field: value})


if __name__ == "__main__":
    unittest.main()
