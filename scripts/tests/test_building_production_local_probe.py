"""Guard tests for the technical-only ordinary-world producer; no game launch."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_production_local_probe as probe


class LocalProbeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name).resolve()
        (self.repo / "target/native-acceptance").mkdir(parents=True)
        (self.repo / "target/profiling").mkdir(parents=True)
        self.binary = self.repo / "target/profiling/bevy_app"
        self.binary.write_bytes(b"test binary")
        self.binary.chmod(0o755)
        self.root = self.repo / "target/native-acceptance/technical-probe"
        self.plan = self.repo / "target/native-acceptance/technical-probe-plan.json"
        self.value = {
            "schema": 1, "profile": probe.PROFILE, "repo": str(self.repo),
            "job_root": str(self.root), "head": "a" * 40, "source": "b" * 64,
            "harness": "c" * 64, "driver": probe.digest(Path(probe.__file__)),
            "binary": str(self.binary), "binary_sha256": probe.digest(self.binary),
            "owner": probe.OWNER, "accepted": False, "promotion_authority": False,
        }
        mocks = (
            (probe.native, "validate_repo", self.repo),
            (probe.native, "git_dirty_paths", []),
            (probe.native, "git_subject", "a" * 40),
            (probe.native, "source_fingerprint", "b" * 64),
            (probe.native, "native_harness_fingerprint", "c" * 64),
        )
        for module, name, result in mocks:
            handle = patch.object(module, name, return_value=result)
            handle.start()
            self.addCleanup(handle.stop)

    def test_exact_plan_is_read_only_and_has_no_authority(self):
        self.plan.write_text("{}")
        self.assertEqual(probe.check_plan(self.value, self.plan),
                         (self.repo, self.root, self.binary))
        self.assertEqual(probe.command(self.plan, self.repo, self.root)[-6:],
                         ["--plan", str(self.plan), "--repo", str(self.repo),
                          "--job-root", str(self.root)])
        self.assertIs(self.value["accepted"], False)

    def test_rejects_authority_source_and_output_changes(self):
        self.plan.write_text("{}")
        for key, replacement in (("accepted", True), ("promotion_authority", True),
                                 ("head", "d" * 40), ("source", "d" * 64),
                                 ("driver", "d" * 64), ("binary_sha256", "d" * 64),
                                 ("owner", "foreign"), ("job_root", str(self.repo))):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.value)
                changed[key] = replacement
                with self.assertRaises(RuntimeError):
                    probe.check_plan(changed, self.plan)
        self.root.mkdir()
        with self.assertRaises(RuntimeError):
            probe.check_plan(self.value, self.plan)

    def test_plan_does_not_publish_a_file_on_rejection(self):
        args = argparse.Namespace(repo=self.repo, job_root=self.root, output=self.plan)
        with patch.object(probe.native, "resource_snapshot", return_value={"failures": []}):
            changed = patch.object(probe.native, "git_dirty_paths", return_value=["dirty"])
            with changed, self.assertRaises(RuntimeError):
                probe.plan(args)
        self.assertFalse(self.plan.exists())

    def test_raw_requires_exact_identity_and_lossless_sample_event_order(self):
        raw = {"schema_version": 2, "scope": "production-raw-observations-v1",
               "nonce": "a" * 32, "process_id": 123, "failure": None,
               "accepted": False, "performance_evidence": False,
               "promotion_authority": False,
               "samples": [{"sample_index": i, "real_seconds": float(i)} for i in range(3)],
               "events": [{"event_index": 0, "sample_index": 2}]}
        probe.check_raw(raw, "a" * 32, 123)
        for change in ({"nonce": "b" * 32}, {"process_id": 124},
                       {"failure": "overflow"}, {"accepted": True},
                       {"events": [{"event_index": 1, "sample_index": 2}]},
                       {"samples": [{"sample_index": 0, "real_seconds": 0.0}] * 3}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.check_raw(raw | change, "a" * 32, 123)

    def test_registered_context_rejects_foreign_or_failed_batch(self):
        self.plan.write_text("{}")
        context = {
            "schema_version": 1, "scope": "normal-validation-runner-ownership-only",
            "batch": "technical-probe", "repo": str(self.repo), "owner": probe.OWNER,
            "root": str(self.root), "head": "a" * 40, "source": "b" * 64,
            "command": probe.command(self.plan, self.repo, self.root),
            "verifier": probe.verifier(self.root), "plan_sha256": probe.digest(self.plan),
            "runner_pid": 123, "runner_identity": "boot:child",
            "coordinator_pid": 122, "coordinator_identity": "boot:parent",
            "accepted": False, "performance_evidence": False,
            "art_approved": False, "promotion_authority": False,
        }
        batch = {
            "phase": "sealing", "repo": str(self.repo), "owner": probe.OWNER,
            "roots": [str(self.root)], "subject": "a" * 40,
            "subject_fingerprint": "b" * 64, "command": context["command"],
            "verify_command": context["verifier"], "child_pid": 123,
            "child_identity": "boot:child", "exit_code": 0,
            "pid": 122, "process_identity": "boot:parent",
            "command_files": [{"path": str(self.plan), "sha256": probe.digest(self.plan)}],
        }
        ledger = {"batches": {"technical-probe": batch}}

        @contextmanager
        def locked(_):
            yield ledger

        with patch.object(probe.ownership.storage, "primary_for", return_value=self.repo), \
                patch.object(probe.ownership.storage, "locked", side_effect=locked), \
                patch.object(probe.os, "getpid", return_value=123), \
                patch.object(probe.os, "getppid", return_value=122), \
                patch.object(probe.ownership.storage, "live_identity",
                             side_effect=lambda pid: {122: "boot:parent", 123: "boot:child"}.get(pid)):
            probe.check_registered_context(self.repo, self.root, self.plan, context)
            for key, wrong in (("exit_code", 1), ("subject", "d" * 40),
                               ("child_identity", "foreign"), ("roots", ["/foreign"])):
                with self.subTest(key=key):
                    original = batch[key]
                    batch[key] = wrong
                    with self.assertRaises(RuntimeError):
                        probe.check_registered_context(self.repo, self.root, self.plan, context)
                    batch[key] = original
            with self.assertRaises(RuntimeError):
                probe.check_registered_context(self.repo, self.root, self.plan,
                                               context | {"art_approved": True})
            # A real seal replaces execution child identity with verifier child.
            batch["child_pid"], batch["child_identity"] = 124, "boot:verifier"
            with patch.object(probe.os, "getpid", return_value=124), \
                    patch.object(probe.ownership.storage, "live_identity",
                                 side_effect=lambda pid: {122: "boot:parent", 124: "boot:verifier"}.get(pid)):
                probe.check_registered_context(self.repo, self.root, self.plan, context)

            # Popen may start verifier before the coordinator publishes child.
            batch["child_pid"], batch["child_identity"] = 123, "boot:child"

            def publish(_):
                batch["child_pid"], batch["child_identity"] = 124, "boot:verifier"

            with patch.object(probe.os, "getpid", return_value=124), \
                    patch.object(probe.ownership.storage, "live_identity",
                                 side_effect=lambda pid: {122: "boot:parent", 124: "boot:verifier"}.get(pid)), \
                    patch.object(probe.time, "sleep", side_effect=publish) as sleep:
                probe.check_registered_context(self.repo, self.root, self.plan, context)
                sleep.assert_called_once_with(0.02)
            for pid, identity in ((None, None), (999, "foreign"), (124, None)):
                batch["child_pid"], batch["child_identity"] = pid, identity
                with patch.object(probe.os, "getpid", return_value=124), \
                        patch.object(probe.ownership.storage, "live_identity",
                                     side_effect=lambda actual: {122: "boot:parent", 124: "boot:verifier"}.get(actual)), \
                        patch.object(probe.time, "sleep") as sleep, self.assertRaises(RuntimeError):
                    probe.check_registered_context(self.repo, self.root, self.plan, context)
                sleep.assert_not_called()
            batch["child_pid"], batch["child_identity"] = 124, None
            with patch.object(probe.os, "getpid", return_value=124), \
                    patch.object(probe.ownership.storage, "live_identity",
                                 side_effect=lambda pid: "boot:parent" if pid == 122 else None), \
                    self.assertRaises(RuntimeError):
                probe.check_registered_context(self.repo, self.root, self.plan, context)


if __name__ == "__main__":
    unittest.main()
