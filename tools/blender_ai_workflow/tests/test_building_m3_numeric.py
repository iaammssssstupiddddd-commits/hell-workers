"""Synthetic decisions only: no independent art/numeric or release evidence."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import building_m3_numeric as numeric


class M3NumericTests(unittest.TestCase):
    def fixture(self, kind="RestArea"):
        payload = numeric.CONTRACT.read_bytes()
        preview = {"kind": kind, "generation": 1, "authority": "art_preview", "manifest_sha256": "a" * 64}
        approval = {"schema_version": 1, "decision": "building_m3_numeric_approved",
                    "art_preview_identity": preview, "art_approval_sha256": "b" * 64,
                    "geometry_contract_sha256": hashlib.sha256(payload).hexdigest(),
                    "numeric_contract": numeric.numeric_contract(payload, kind)}
        return payload, preview, approval

    def test_exact_kind_specific_values_and_original_are_required(self):
        for kind in sorted(numeric.KINDS):
            payload, preview, approval = self.fixture(kind)
            numeric.validate(approval, preview, "b" * 64, payload)
            self.assertEqual(approval["numeric_contract"]["kind"], kind)
            self.assertNotIn("palette", approval["numeric_contract"]["geometry"])
            self.assertFalse(json.loads(payload)["numeric_freeze"])
            self.assertNotIn("production_state", approval)
            for field, value in (("schema_version", True), ("schema_version", 1.0),
                                 ("decision", "building_numeric_approved"),
                                 ("art_approval_sha256", "c" * 64),
                                 ("geometry_contract_sha256", "d" * 64)):
                with self.subTest(kind=kind, field=field, value=value):
                    changed = {**approval, field: value}
                    with self.assertRaises(ValueError):
                        numeric.validate(changed, preview, "b" * 64, payload)
            for field in approval:
                changed = dict(approval)
                del changed[field]
                with self.assertRaises(ValueError):
                    numeric.validate(changed, preview, "b" * 64, payload)
            with self.assertRaises(ValueError):
                numeric.validate({**approval, "extra": 1}, preview, "b" * 64, payload)

    def test_foreign_identity_and_numeric_values_are_rejected(self):
        payload, preview, approval = self.fixture()
        for field, value in (("kind", "SoulSpa"), ("kind", "Tank"), ("generation", 2),
                             ("generation", True), ("authority", "release_approved"),
                             ("manifest_sha256", "f" * 64)):
            changed = copy.deepcopy(approval)
            changed["art_preview_identity"][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                numeric.validate(changed, preview, "b" * 64, payload)
        for value in (True, 33, float("nan"), float("inf")):
            changed = copy.deepcopy(approval)
            changed["numeric_contract"]["geometry_scale"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                numeric.validate(changed, preview, "b" * 64, payload)
        changed = copy.deepcopy(approval)
        changed["numeric_contract"]["extra"] = 0
        with self.assertRaises(ValueError):
            numeric.validate(changed, preview, "b" * 64, payload)

    def test_geometry_shape_flags_and_nonfinite_values_fail_closed(self):
        payload, _, _ = self.fixture()
        for field, value in (("extra", 1), ("numeric_freeze", True), ("art_approved", True),
                             ("runtime_published", True), ("geometry_scale", True),
                             ("geometry_scale", float("nan")), ("tolerance_wu", float("inf")),
                             ("ordered_relative_tiles", []), ("stage", "approved")):
            changed = {**json.loads(payload), field: value}
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                numeric.numeric_contract(json.dumps(changed).encode(), "RestArea")
        with self.assertRaises(ValueError):
            numeric.numeric_contract(payload.replace(b'"schema_version": 1', b'"schema_version": 1, "schema_version": 1'), "RestArea")

    def test_changed_geometry_cannot_reuse_decision_even_with_rebound_hash(self):
        payload, preview, approval = self.fixture()
        changed = json.loads(payload)
        changed["RestArea"]["roof"]["thickness_wu"] += 0.25
        changed = numeric.canonical(changed)
        for candidate in (approval, {**approval, "geometry_contract_sha256": hashlib.sha256(changed).hexdigest()}):
            with self.assertRaises(ValueError):
                numeric.validate(candidate, preview, "b" * 64, changed)
        # Reformatting is also a different raw original despite equal numeric projection.
        with self.assertRaises(ValueError):
            numeric.validate(approval, preview, "b" * 64, numeric.canonical(json.loads(payload)))


if __name__ == "__main__":
    unittest.main()
