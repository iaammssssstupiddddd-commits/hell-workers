"""Numeric proposal regression coverage; execution belongs to the coordinator."""

from __future__ import annotations

import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from building_clay_geometry import FIXTURES, read_json
from validate_building_numeric_candidate import validate


class NumericCandidateTests(unittest.TestCase):
    def setUp(self):
        self.path = FIXTURES / "building-m2-v1.numeric-candidate.json"
        self.proposal = read_json(self.path)

    def check_changed(self, proposal):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "candidate.json"
            path.write_text(json.dumps(proposal))
            return validate(path)

    def test_arithmetic_does_not_grant_acceptance(self):
        result = validate(self.path)
        self.assertFalse(result["numeric_freeze"])
        self.assertFalse(result["runtime_published"])
        self.assertIn("actual-window", result["excludes"])
        self.assertEqual(result["texture_base_bytes_per_kind"], 1572864)
        self.assertEqual(result["texture_full_mip_bytes_per_kind"], 2097148)

    def test_changed_fixture_identity_and_state_transform_rejected(self):
        for kind in ("Tank", "MudMixer"):
            changed = copy.deepcopy(self.proposal)
            changed["kinds"][kind]["geometry_sha256"] = "0" * 64
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                self.check_changed(changed)
        changed = copy.deepcopy(self.proposal)
        changed["kinds"]["Tank"]["states"]["Full"]["translation_wu"][1] = 12
        with self.assertRaises(ValueError):
            self.check_changed(changed)

    def test_static_rotor_bounds_cannot_replace_continuous_sweep(self):
        changed = copy.deepcopy(self.proposal)
        changed["kinds"]["MudMixer"]["rotor_sweep"]["radius_wu"] = 14
        with self.assertRaises(ValueError):
            self.check_changed(changed)

    def test_proposal_cannot_self_approve_or_publish(self):
        for key, value in (("numeric_freeze", True), ("runtime_published", True),
                           ("decision", "approved")):
            changed = copy.deepcopy(self.proposal)
            changed[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check_changed(changed)

    def test_texture_allocation_change_requires_new_proposal(self):
        changed = copy.deepcopy(self.proposal)
        changed["texture_budget_per_kind"]["albedo_px"] = [1024, 1024]
        with self.assertRaises(ValueError):
            self.check_changed(changed)


if __name__ == "__main__":
    unittest.main()
