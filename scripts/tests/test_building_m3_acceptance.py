"""M3 recipes cannot replace same-subject lifecycle/native acceptance."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import building_m3_acceptance as m3


class M3AcceptanceTests(unittest.TestCase):
    def test_both_paths_include_specific_gameplay_and_native_observations(self):
        rest = {leg["id"]: leg for leg in m3.legs("RestArea")}
        spa = {leg["id"]: leg for leg in m3.legs("SoulSpa")}
        self.assertIn("occupancy-dream", rest)
        self.assertIn("all-masks", spa)
        self.assertIn("construction-cancel", spa)
        self.assertIn("operational-energy-walk", spa)
        for legs in (rest, spa):
            self.assertIn("save-load", legs)
            self.assertIn("failure-generation", legs)
            self.assertIn("cleanup", legs)
            self.assertTrue(all(leg["accepted"] is False and leg["evidence"] == [] for leg in legs.values()))

    def test_recipe_rejects_adopted_approval_before_reading_source(self):
        for field in ("accepted", "runtime_published"):
            value = {"schema_version": 1, "scope": "m3-coordinator-recipe-not-acceptance",
                     "accepted": False, "runtime_published": False, "performance": {"accepted": False}}
            value[field] = True
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "plan.json"
                path.write_bytes(m3.pipeline.canonical(value))
                with self.assertRaisesRegex(ValueError, "cannot claim acceptance"):
                    m3.verify(path)

    def test_runtime_rejects_wrong_slot_mapping_or_candidate_authority(self):
        manifest = {"identity": {"authority": "art_preview"}, "receipt": None, "art_approval_sha256": None,
                    "geometry_contract_sha256": m3.pipeline.digest(m3.CONTRACT.read_bytes()),
                    "parts": m3.parts("SoulSpa")}
        manifest["parts"][1]["translation_wu"][0] = 16
        with patch.object(m3.pipeline, "load_set", return_value=(manifest, "", None)):
            with self.assertRaisesRegex(ValueError, "transforms differ"):
                m3.runtime("SoulSpa", Path("unused"), Path("codec"))
        manifest["identity"]["authority"] = "isolated_candidate"
        with patch.object(m3.pipeline, "load_set", return_value=(manifest, "", None)):
            with self.assertRaisesRegex(ValueError, "ArtPreview"):
                m3.runtime("SoulSpa", Path("unused"), Path("codec"))
