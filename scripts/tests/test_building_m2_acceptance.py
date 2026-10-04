"""Observation inventory must not turn feedback into acceptance."""

import copy
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts import building_m2_acceptance as m2


class M2AcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.identity = {"kind": "Tank", "generation": 1, "authority": "art_preview",
                         "manifest_sha256": "a" * 64}
        self.plan = {"scope": "m2-feedback-plan-not-acceptance",
                     "numeric_candidate_sha256": m2.NUMERIC_SHA256,
                     "source_sha256": "b" * 64, "kinds": {"Tank": {"identity": self.identity}}}
        sample = {"identity": self.identity, "paused": False,
                  "owners": [{"entity": "owner"}], "roots": [{"state": "TankFull"}], "items": [],
                  "expected_images": {"world": "world", "catalog": "catalog"},
                  "consumers": [{"role": "ghost", "image": "world"},
                                {"role": "catalog", "image": "stale"}]}
        self.trace = {"schema_version": 1, "scope": "m2-feedback-observations-only", "identity": self.identity,
                      "failure": None, "accepted": False, "performance_evidence": False,
                      "samples": [copy.deepcopy(sample), copy.deepcopy(sample)]}

    def inspect(self, *, trace_text=None, plan_bytes=None):
        with tempfile.TemporaryDirectory() as directory:
            plan, trace = Path(directory) / "plan.json", Path(directory) / "trace.json"
            plan.write_bytes(m2.pipeline.canonical(self.plan) if plan_bytes is None else plan_bytes)
            # Match the producer's compact JSON without a canonical trailing LF.
            trace.write_text(json.dumps(self.trace, separators=(",", ":"))
                             if trace_text is None else trace_text)
            return m2.inspect(plan, "Tank", trace)

    def test_inventory_excludes_mismatched_consumer_and_cannot_accept(self):
        result = self.inspect()
        self.assertFalse(result["accepted"])
        self.assertEqual(result["matching_consumers_seen"], ["ghost"])
        self.assertEqual(result["unpaused_samples"], 2)
        self.assertIn("independent gates", result["remaining"])

    def test_wrong_generation_or_overflow_is_not_inventory(self):
        self.trace["identity"] = {**self.identity, "generation": 2}
        with self.assertRaises(ValueError):
            self.inspect()
        self.trace["identity"] = self.identity
        self.trace["failure"] = "sample cap reached"
        with self.assertRaises(ValueError):
            self.inspect()

    def test_probe_cannot_claim_performance_evidence(self):
        self.trace["performance_evidence"] = True
        with self.assertRaises(ValueError):
            self.inspect()

    def test_noncanonical_plan_is_still_rejected(self):
        with self.assertRaisesRegex(ValueError, "metadata is not canonical"):
            self.inspect(plan_bytes=json.dumps(self.plan).encode())

    def test_duplicate_fields_and_nonfinite_numbers_in_trace_are_rejected(self):
        payload = json.dumps(self.trace, separators=(",", ":"))
        for replacement, message in (
            ('"failure":null,"failure":null', "duplicate field"),
            ('"failure":NaN', "non-finite JSON number"),
        ):
            with self.subTest(replacement=replacement), self.assertRaisesRegex(ValueError, message):
                self.inspect(trace_text=payload.replace('"failure":null', replacement))

    def test_wrong_trace_schema_is_rejected(self):
        self.trace["schema_version"] = 2
        with self.assertRaisesRegex(ValueError, "unsupported observation schema"):
            self.inspect()

    def test_inventory_uses_the_same_revision_rejection(self):
        self.plan["kinds"]["Tank"]["revision"] = "v2"
        with self.assertRaisesRegex(ValueError, "revision"):
            self.inspect()


class RuntimeIntegrityTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.codec = self.root / "codec"
        self.codec.write_bytes(b"fixture codec")
        self.subject = {"source": "b" * 64}
        self.plan_path = self.root / "plan.json"
        self.value = {"schema_version": 1, "scope": "m2-feedback-plan-not-acceptance",
                      "numeric_candidate_sha256": m2.NUMERIC_SHA256, "accepted": False,
                      "runtime_published": False, "repo": str(self.root), "subject": self.subject,
                      "source_sha256": self.subject["source"], "codec": str(self.codec),
                      "codec_sha256": m2.pipeline.digest(self.codec.read_bytes()),
                      "independent_gates": {"art": None}, "performance": {"accepted": False}, "kinds": {}}
        geometry = (m2.ROOT / "tools/blender_ai_workflow/fixtures/building-m2-v1.numeric-candidate.json").read_bytes()
        for kind in ("Tank", "MudMixer"):
            root = self.root / kind
            source = kind.encode()
            artifact = {"role": "image:albedo", "path": "albedo.png", "bytes": 3,
                        "sha256": m2.pipeline.digest(b"png")}
            manifest = {"identity": {"kind": kind, "generation": 1, "authority": "art_preview",
                                     "manifest_sha256": "a" * 64},
                        "source_sha256": m2.pipeline.digest(source),
                        "geometry_contract_sha256": m2.NUMERIC_SHA256,
                        "artifacts": [artifact], "export_sha256": m2.pipeline.digest(m2.pipeline.canonical([artifact])),
                        "receipt": None, "art_approval_sha256": None}
            text = json.dumps(manifest)
            for path, data in (("provenance/source", source), ("provenance/geometry.json", geometry),
                               ("albedo.png", b"png"), (m2.pipeline.locator(kind), text.encode())):
                m2.pipeline.put(root / path, data)
            self.value["kinds"][kind] = {"source_root": str(root), "identity": manifest["identity"],
                                          "manifest_sha256": m2.pipeline.digest(text.encode())}
        self.plan_path.write_bytes(m2.pipeline.canonical(self.value))

    def verify(self, subject=None):
        # Codec schema parsing is tested in M1-c; all file/provenance/digest reads
        # in this test remain real, including the second kind and frozen plan.
        with patch.object(m2, "frozen_subject", return_value=subject or self.subject), \
                patch.object(m2, "candidate_geometry"), patch.object(m2.pipeline, "codec"):
            return m2.verify(self.plan_path)

    def test_reloads_both_sets_without_granting_acceptance(self):
        result = self.verify()
        self.assertEqual(result["status"], "pass")
        self.assertEqual(set(result["identities"]), {"Tank", "MudMixer"})
        self.assertFalse(result["accepted"])
        self.assertFalse(result["numeric_freeze"])

    def test_mutated_second_kind_artifact_is_rejected(self):
        (self.root / "MudMixer/albedo.png").write_bytes(b"bad")
        with self.assertRaises(ValueError):
            self.verify()

    def test_mutated_geometry_provenance_is_rejected(self):
        (self.root / "Tank/provenance/geometry.json").write_bytes(b"{}")
        with self.assertRaisesRegex(ValueError, "provenance"):
            self.verify()

    def test_codec_and_source_drift_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "subject"):
            self.verify(subject={"source": "c" * 64})
        self.codec.write_bytes(b"changed codec")
        with self.assertRaisesRegex(ValueError, "codec"):
            self.verify()

    def test_self_approval_is_rejected(self):
        self.value["independent_gates"]["art"] = "approved"
        self.plan_path.write_bytes(m2.pipeline.canonical(self.value))
        with self.assertRaisesRegex(ValueError, "approval"):
            self.verify()


    def set_revision(self, kind, revision, *, update_manifest=True, update_provenance=True):
        path, digest = m2.numeric_contract(kind, revision)
        spec = self.value["kinds"][kind]
        spec.update(revision=revision, numeric_candidate_sha256=digest)
        root = Path(spec["source_root"])
        if update_manifest:
            manifest_path = root / m2.pipeline.locator(kind)
            manifest = json.loads(manifest_path.read_text())
            manifest["geometry_contract_sha256"] = digest
            text = json.dumps(manifest)
            manifest_path.write_text(text)
            spec["manifest_sha256"] = m2.pipeline.digest(text.encode())
        if update_provenance:
            (root / "provenance/geometry.json").write_bytes(path.read_bytes())
        self.plan_path.write_bytes(m2.pipeline.canonical(self.value))

    def test_legacy_and_explicit_v2_have_exact_per_kind_proposals(self):
        legacy = self.verify()
        for kind in ("Tank", "MudMixer"):
            self.assertEqual(legacy["numeric_candidates"][kind],
                             {"revision": "v1", "numeric_candidate_sha256": m2.NUMERIC_SHA256})
        self.set_revision("Tank", "v1")
        self.set_revision("MudMixer", "v2")
        result = self.verify()
        self.assertEqual(result["numeric_candidates"]["MudMixer"],
                         {"revision": "v2", "numeric_candidate_sha256": m2.numeric_contract("MudMixer", "v2")[1]})
        self.assertEqual(result["numeric_candidates"]["Tank"]["revision"], "v1")
        self.assertFalse(result["accepted"])
        self.assertFalse(result["numeric_freeze"])
        self.assertFalse(result["runtime_published"])

    def test_v2_requires_explicit_matching_hash_and_revision(self):
        original = copy.deepcopy(self.value)
        for kind, revision, digest in (
            ("MudMixer", None, m2.NUMERIC_SHA256),
            ("MudMixer", 2, m2.NUMERIC_SHA256),
            ("MudMixer", "v3", m2.NUMERIC_SHA256),
            ("Tank", "v2", m2.NUMERIC_SHA256),
            ("MudMixer", "v2", None),
            ("MudMixer", "v2", m2.NUMERIC_SHA256),
            ("MudMixer", "v1", m2.numeric_contract("MudMixer", "v2")[1]),
        ):
            with self.subTest(kind=kind, revision=revision, digest=digest):
                self.value = copy.deepcopy(original)
                self.value["kinds"][kind]["revision"] = revision
                if digest is not None:
                    self.value["kinds"][kind]["numeric_candidate_sha256"] = digest
                self.plan_path.write_bytes(m2.pipeline.canonical(self.value))
                with self.assertRaises(ValueError):
                    self.verify()

    def test_v2_rejects_manifest_only_or_provenance_only_binding(self):
        self.set_revision("MudMixer", "v2", update_manifest=False)
        with self.assertRaisesRegex(ValueError, "numeric binding"):
            self.verify()
        self.set_revision("MudMixer", "v2")
        legacy, _ = m2.numeric_contract("MudMixer", "v1")
        (self.root / "MudMixer/provenance/geometry.json").write_bytes(legacy.read_bytes())
        with self.assertRaisesRegex(ValueError, "provenance"):
            self.verify()

    def test_plan_propagates_explicit_mixer_revision_without_authority(self):
        self.set_revision("MudMixer", "v2")
        args = SimpleNamespace(repo=str(self.root), subject_sha256=self.subject["source"],
                               tank_runtime=self.root / "Tank", mixer_runtime=self.root / "MudMixer",
                               mixer_revision="v2", codec=self.codec, output=self.root / "new-plan.json")
        with patch.object(m2, "frozen_subject", return_value=self.subject), \
                patch.object(m2, "candidate_geometry") as geometry, patch.object(m2.pipeline, "codec"):
            result = m2.plan(args)
        self.assertEqual(result["kinds"]["MudMixer"]["revision"], "v2")
        self.assertEqual(result["kinds"]["Tank"]["revision"], "v1")
        geometry.assert_any_call("MudMixer", "v2")
        self.assertFalse(result["accepted"])
        self.assertTrue(all(value is None for value in result["independent_gates"].values()))

    def test_native_recipe_cannot_admit_a_mismatched_v2_proposal(self):
        from scripts import building_m2_native_acceptance as recipe

        self.set_revision("MudMixer", "v2", update_manifest=False)
        value = {"status": "ready", "profile": recipe.PROFILE, "scope": recipe.SCOPE,
                 "kind": "MudMixer", "leg": "refining", "duration_seconds": 60,
                 "accepted": False, "promotion_authority": False, "adapter": "Intel",
                 "runtime_plan": str(self.plan_path),
                 "runtime_plan_sha256": m2.pipeline.digest(self.plan_path.read_bytes())}
        with patch.object(m2, "frozen_subject", return_value=self.subject), \
                patch.object(m2, "candidate_geometry"), patch.object(m2.pipeline, "codec"):
            with self.assertRaisesRegex(ValueError, "numeric binding"):
                recipe.check_plan(value)


if __name__ == "__main__":
    unittest.main()
