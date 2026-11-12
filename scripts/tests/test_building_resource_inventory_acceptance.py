from __future__ import annotations

import os
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from scripts import building_resource_inventory_acceptance as supplement


class ResourceSupplementTests(unittest.TestCase):
    def test_origin_requires_distinct_committed_source_with_only_two_passive_paths(self):
        def command(paths, ancestor=0):
            return [SimpleNamespace(stdout="\n".join(paths)), SimpleNamespace(returncode=ancestor)]
        with patch.object(supplement.subprocess, "run", side_effect=command(supplement.ALLOWED)), \
             patch.object(supplement.native, "git_subject", return_value="a"*40):
            supplement.origin(Path("/reference"))
        for paths, ancestor, head in [(supplement.ALLOWED | {"crates/hw_jobs/src/lib.rs"}, 0, "a"*40),
                                      (supplement.ALLOWED, 1, "a"*40),
                                      (supplement.ALLOWED, 0, supplement.BASE)]:
            with self.subTest(paths=paths, ancestor=ancestor, head=head), \
                 patch.object(supplement.subprocess, "run", side_effect=command(paths, ancestor)), \
                 patch.object(supplement.native, "git_subject", return_value=head), \
                 self.assertRaises(supplement.native.AcceptanceError):
                supplement.origin(Path("/reference"))

    def test_historical_identity_uses_its_own_catalog_not_the_current_catalog(self):
        value = {"subject_commit": "a"*40, "source_fingerprint": "b"*64,
                 "harness_fingerprint": "c"*64, "asset_view_fingerprint": "d"*64,
                 "driver_sha256": supplement.static.REFERENCE_DRIVER_SHA256}
        completed = SimpleNamespace(returncode=0, stdout=supplement.json.dumps(value), stderr="")
        with patch.object(supplement.native, "git_dirty_paths", return_value=[]), \
             patch.object(supplement, "origin"), \
             patch.object(supplement.static, "sha256", return_value=supplement.static.REFERENCE_DRIVER_SHA256), \
             patch.object(supplement.subprocess, "run", return_value=completed) as called, \
             patch.object(supplement.static, "identity", side_effect=FileNotFoundError("19 current harness paths absent")):
            self.assertEqual(supplement.local_identity(Path("/historical")), value)
        argv = called.call_args.args[0]
        self.assertEqual(argv[:4], [supplement.sys.executable, "-I", "-B", "-c"])
        self.assertEqual(argv[-1], "/historical")
        self.assertIn("local.identity(repo)", argv[4])
        for bad in (SimpleNamespace(returncode=1, stdout="", stderr="changed source"),
                    SimpleNamespace(returncode=0, stdout=supplement.json.dumps(value | {"driver_sha256": "0"*64}), stderr="")):
            with patch.object(supplement.native, "git_dirty_paths", return_value=[]), \
                 patch.object(supplement, "origin"), \
                 patch.object(supplement.static, "sha256", return_value=supplement.static.REFERENCE_DRIVER_SHA256), \
                 patch.object(supplement.subprocess, "run", return_value=bad), \
                 self.assertRaises(supplement.native.AcceptanceError):
                supplement.local_identity(Path("/historical"))

    def test_changed_plan_is_rejected_before_inputs_and_outputs(self):
        args = SimpleNamespace(plan=Path("/private/plan"), frozen_plan={"job_root": "/admitted"})
        with patch.dict(os.environ, {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
             patch.object(supplement.native, "read_json", return_value={"job_root": "/changed"}), \
             patch.object(supplement, "check_plan") as checked:
            with self.assertRaisesRegex(supplement.native.AcceptanceError, "changed across activity"):
                supplement.run.__wrapped__(args)
            checked.assert_not_called()

    def test_reference_three_run_default_is_preserved_and_unknown_repeat_scope_rejected(self):
        for repeat in (0, 2, True):
            with self.subTest(repeat=repeat), self.assertRaises(supplement.native.AcceptanceError):
                supplement.static.verify_session(Path("/not-created"), "capture", "small", 15, {}, "Intel", repeats=repeat)
        self.assertEqual(supplement.static.verify_session.__kwdefaults__, {"repeats": 3})

    def test_result_cannot_change_root_or_claim_budget_acceptance(self):
        manifest = {"plan": {"job_root": "/foreign"}, "profile": supplement.PROFILE, "status": "pass"}
        with patch.object(supplement.native, "read_json", return_value=manifest), \
             patch.object(supplement, "observations") as observed:
            with self.assertRaises(supplement.native.AcceptanceError):
                supplement.verify(Path("/admitted"))
            observed.assert_not_called()
        manifest.update(plan={"job_root": "/admitted"}, binary={"path": "/admitted/binary/bevy_app", "sha256": "a"*64},
                        observations=[{"binary_sha256": "a"*64}], plateau={}, accepted=False,
                        promotion_authority=False, performance_budget_evidence=True)
        with patch.object(supplement.native, "read_json", return_value=manifest), \
             patch.object(supplement.executable, "validate"), \
             patch.object(supplement, "observations", return_value=(manifest["observations"], {})):
            with self.assertRaisesRegex(supplement.native.AcceptanceError, "result differs"):
                supplement.verify(Path("/admitted"))


if __name__ == "__main__":
    unittest.main()
