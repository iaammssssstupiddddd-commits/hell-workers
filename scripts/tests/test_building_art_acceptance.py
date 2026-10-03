"""Fail-closed recipe admission; no native processes are launched here."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import building_art_acceptance as art


class RecipeTests(unittest.TestCase):
    def source(self, digit="a"):
        return {"commit": digit * 40, "source": digit * 64, "harness": "b" * 64,
                "assets": "c" * 64, "driver": "d" * 64, "dirty": []}

    def test_feedback_cannot_become_clean_candidate_evidence(self):
        dirty = {**self.source(), "dirty": ["crates/bevy_app/src/main.rs"]}
        art.validate_subject(dirty, feedback=True)
        with self.assertRaisesRegex(ValueError, "clean"):
            art.validate_subject(dirty, feedback=False)
        missing = self.source()
        del missing["source"]
        with self.assertRaisesRegex(ValueError, "incomplete"):
            art.validate_subject(missing, feedback=False)
        value = {"mode": "candidate", "identity": {"authority": "isolated_candidate"}, "scope": "paused-static-presentation"}
        probe = {"profile": "building-art", "status": "ready", "mode": "art-preview", "identity": value["identity"],
                 "scope": value["scope"], "nonce": "a" * 32, "stable_frames": 30, "fixture": {"target_count": 36}}
        with self.assertRaisesRegex(ValueError, "acknowledgement"):
            art.validate_probe(probe, value, "a" * 32)

    def identities(self):
        left = {"subject_commit": "a" * 40, "source_fingerprint": "a" * 64,
                "harness_fingerprint": "b" * 64, "asset_view_fingerprint": "c" * 64, "driver_sha256": "d" * 64}
        right = {**left, "subject_commit": "e" * 40, "source_fingerprint": "e" * 64}
        return left, right

    def comparison(self, left_identity, right_identity, *, equal_binary=False, layout="f" * 64):
        left = {"identity": left_identity, "adapter": "fixture-adapter", "repo": "baseline"}
        right = {"identity": right_identity, "adapter": "fixture-adapter", "repo": "candidate"}
        before = {"sessions": [{"instrumentation": "capture", "size": "small", "binary_sha256": "1" * 64,
            "observations": [{"layout_sha256": "f" * 64, "adapter": "fixture-adapter"}], "aggregate": {"p95": {"median": 1.0}}}]}
        after = copy.deepcopy(before)
        after["sessions"][0]["binary_sha256"] = ("1" if equal_binary else "2") * 64
        after["sessions"][0]["observations"][0]["layout_sha256"] = layout
        with patch.object(art.native, "read_json", side_effect=[left, right]), \
                patch.object(art, "verify_frozen_static", side_effect=[before, after]) as verifier, \
                patch.object(art, "source_roles"):
            result = art.compare(Path("baseline"), Path("candidate"), left_identity, right_identity)
            self.assertEqual(verifier.call_count, 2)
            self.assertEqual(verifier.call_args_list[0].args, (Path("baseline"), left, left_identity))
            self.assertEqual(verifier.call_args_list[1].args, (Path("candidate"), right, right_identity))
            return result

    def test_separate_source_comparison_revalidates_both_raw_jobs_without_budget_claim(self):
        result = self.comparison(*self.identities())
        self.assertEqual(result["status"], "compared")
        self.assertEqual(result["budget_decision"], "not-evaluated")
        self.assertFalse(result["promotion_authority"])

    def test_same_source_same_binary_and_fabricated_fallback_are_rejected(self):
        left, right = self.identities()
        with self.assertRaisesRegex(ValueError, "distinct sources"):
            self.comparison(left, left)
        with self.assertRaisesRegex(ValueError, "binary pair"):
            self.comparison(left, right, equal_binary=True)
        with self.assertRaisesRegex(ValueError, "layout differs"):
            self.comparison(left, right, layout="0" * 64)
        with self.assertRaisesRegex(ValueError, "assets/adapter"):
            self.comparison(left, {**right, "asset_view_fingerprint": "0" * 64})

    def test_missing_or_malformed_full_identity_is_not_a_baseline(self):
        left, right = self.identities()
        for invalid in ({}, {**left, "source_fingerprint": "unknown"}):
            with self.assertRaises(ValueError):
                self.comparison(invalid, right)

    def test_two_post_m1_sources_do_not_replace_the_pre_m1_baseline(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline, candidate = Path(directory) / "baseline", Path(directory) / "candidate"
            marker = "crates/bevy_app/src/assets/building_asset_set/mod.rs"
            fixture = "crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/mod.rs"
            for root in (baseline, candidate):
                (root / fixture).parent.mkdir(parents=True)
                (root / fixture).write_text("fixture")
            (candidate / marker).parent.mkdir(parents=True)
            (candidate / marker).write_text("foundation")
            art.source_roles(baseline, candidate)
            (baseline / marker).parent.mkdir(parents=True)
            (baseline / marker).write_text("foundation")
            with self.assertRaisesRegex(ValueError, "pre-M1"):
                art.source_roles(baseline, candidate)


class FrozenVerifierTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def fixture(self, name):
        repo, job = self.root / name, self.root / (name + "-job")
        driver = repo / "scripts/building_art_static_acceptance.py"
        driver.parent.mkdir(parents=True)
        driver.write_text("# frozen driver " + name)
        job.mkdir()
        identity = {"subject_commit": name[0] * 40, "source_fingerprint": name[0] * 64,
                    "harness_fingerprint": art.pipeline.digest(name.encode()),
                    "asset_view_fingerprint": "c" * 64,
                    "driver_sha256": art.pipeline.digest(driver.read_bytes())}
        manifest = {"repo": str(repo), "profile": art.static.PROFILE, "identity": identity,
                    "sessions": [{"raw_inventory": name}]}
        (job / "manifest.json").write_text(json.dumps(manifest))
        result = {"status": "pass", "profile": art.static.PROFILE,
                  "subject_commit": identity["subject_commit"], "capture_runs": 6,
                  "memory_runs": 3, "active_simulation": "not-measured",
                  "sessions": manifest["sessions"]}
        return repo, job, driver, manifest, identity, result

    def invoke(self, fixture, *, result=None, returncode=0, dirty=None, mutate=None):
        repo, job, driver, manifest, identity, valid = fixture

        def child(command, **kwargs):
            self.assertEqual(command, [sys.executable, "-B", "-E", "-s", str(driver),
                                       "verify", "--job-root", str(job.resolve())])
            self.assertEqual(kwargs["cwd"], repo)
            if mutate:
                mutate()
            return subprocess.CompletedProcess(command, returncode,
                json.dumps(valid) if result is None else result, "raw job rejected")

        with patch.object(art.native, "validate_repo", return_value=repo), \
                patch.object(art.native, "git_subject", return_value=identity["subject_commit"]), \
                patch.object(art.native, "git_dirty_paths", return_value=dirty or []), \
                patch.object(art.subprocess, "run", side_effect=child) as process, \
                patch.object(art.static, "verify", side_effect=AssertionError("wrong source verifier")):
            try:
                return art.verify_frozen_static(job, manifest, identity)
            finally:
                self.launched = process.call_count

    def test_each_frozen_source_uses_its_own_driver_and_module_process(self):
        before, after = self.fixture("baseline"), self.fixture("candidate")
        self.assertNotEqual(before[4]["harness_fingerprint"], after[4]["harness_fingerprint"])
        self.assertNotEqual(before[4]["driver_sha256"], after[4]["driver_sha256"])
        for fixture in (before, after):
            self.assertEqual(self.invoke(fixture), fixture[5])
            self.assertEqual(self.launched, 1)

    def test_failed_raw_verification_never_falls_back_to_current_driver(self):
        with self.assertRaisesRegex(ValueError, "raw-job verification failed"):
            self.invoke(self.fixture("baseline"), returncode=1)

    def test_malformed_or_mismatched_verifier_result_is_rejected(self):
        fixture = self.fixture("baseline")
        for output in ("not json", "[]", json.dumps({**fixture[5], "status": "fail"}),
                       json.dumps({**fixture[5], "subject_commit": "f" * 40}),
                       json.dumps({**fixture[5], "sessions": []})):
            with self.subTest(output=output), self.assertRaises(ValueError):
                self.invoke(fixture, result=output)

    def test_dirty_or_modified_driver_is_rejected_before_launch(self):
        fixture = self.fixture("baseline")
        with self.assertRaisesRegex(ValueError, "dirty or changed"):
            self.invoke(fixture, dirty=["scripts/helper.py"])
        self.assertEqual(self.launched, 0)
        fixture[2].write_text("# changed driver")
        with self.assertRaisesRegex(ValueError, "driver identity differs"):
            self.invoke(fixture)
        self.assertEqual(self.launched, 0)

    def test_manifest_or_driver_changes_during_verification_are_rejected(self):
        for name in ("baseline", "candidate"):
            fixture = self.fixture(name)
            target = fixture[1] / "manifest.json" if name == "baseline" else fixture[2]
            with self.subTest(target=target), self.assertRaises(ValueError):
                self.invoke(fixture, mutate=lambda: target.write_text("{}"))
            self.assertEqual(self.launched, 1)


if __name__ == "__main__":
    unittest.main()
