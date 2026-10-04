"""Ownership/admission rejection tests; no game process or native launch."""
from __future__ import annotations

from contextlib import contextmanager
import copy
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_production_validation_context as context


class ValidationContextTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name).resolve()
        self.plan = self.repo / "plan.json"
        self.plan.write_text("{}")
        self.root = self.repo / "target/native-acceptance/ordinary-world"
        self.command = ["python3", str(self.repo / "runner.py"), "--plan", str(self.plan)]
        self.verifier = ["python3", str(self.repo / "runner.py"), "verify", str(self.root)]
        self.batch = {
            "phase": "running", "repo": str(self.repo), "owner": "main",
            "roots": [str(self.root)], "command": self.command, "verify_command": self.verifier,
            "pid": 100, "process_identity": "boot:parent", "child_pid": 101,
            "child_identity": "boot:child", "subject": "a" * 40,
            "subject_fingerprint": "b" * 64, "policy_sha256": "c" * 64,
            "coordinator_sha256": "d" * 64,
            "command_files": [{"path": str(self.plan), "sha256": "d" * 64}],
        }
        self.ledger = {"batches": {"ordinary-world": self.batch}}

        @contextmanager
        def locked(_):
            yield self.ledger

        for target, kwargs in (
            ("primary_for", {"return_value": self.repo}),
            ("locked", {"side_effect": locked}),
            ("git", {"return_value": "a" * 40}),
            ("subject_fingerprint", {"return_value": "b" * 64}),
            ("policy_hash", {"return_value": "c" * 64}),
            ("digest", {"return_value": "d" * 64}),
            ("live_identity", {"side_effect": lambda pid: {100: "boot:parent", 101: "boot:child"}.get(pid)}),
        ):
            handle = patch.object(context.storage, target, **kwargs)
            handle.start()
            self.addCleanup(handle.stop)
        for target, value in (("getpid", 101), ("getppid", 100)):
            handle = patch.object(context.os, target, return_value=value)
            handle.start()
            self.addCleanup(handle.stop)
        handle = patch.dict(os.environ, {context.storage.BATCH_ENV: "ordinary-world"})
        handle.start()
        self.addCleanup(handle.stop)

    def invoke(self, **changes):
        args = dict(repo=self.repo, plan=self.plan, root=self.root, owner="main",
                    command=self.command, verifier=self.verifier)
        return context.running_context(**(args | changes))

    def test_exact_live_child_reads_context_without_mutation_or_authority(self):
        before = copy.deepcopy(self.ledger)
        paths = set(self.repo.rglob("*"))
        value = self.invoke()
        self.assertEqual(value["head"], "a" * 40)
        self.assertEqual(value["runner_identity"], "boot:child")
        for key in ("accepted", "performance_evidence", "art_approved", "promotion_authority"):
            self.assertIs(value[key], False)
        self.assertEqual(before, self.ledger)
        self.assertEqual(paths, set(self.repo.rglob("*")))
        value["command"].append("foreign")
        self.assertEqual(before, self.ledger)

    def test_rejects_each_changed_ledger_binding(self):
        original = copy.deepcopy(self.batch)
        cases = {
            "phase": "finalized", "repo": "/foreign", "owner": "foreign",
            "roots": [str(self.root), "/foreign"], "command": ["arbitrary"],
            "verify_command": ["arbitrary"], "pid": 99, "process_identity": "stale",
            "child_pid": 99, "child_identity": "stale", "subject": "e" * 40,
            "subject_fingerprint": "e" * 64, "policy_sha256": "e" * 64,
            "coordinator_sha256": "e" * 64, "command_files": [],
        }
        for key, value in cases.items():
            with self.subTest(key=key):
                self.batch.clear()
                self.batch.update(copy.deepcopy(original) | {key: value})
                with self.assertRaises(RuntimeError):
                    self.invoke()

    def test_rejects_missing_controller_or_child_birth_identity(self):
        for pid in (100, 101):
            with self.subTest(pid=pid), patch.object(context.storage, "live_identity",
                    side_effect=lambda actual: None if actual == pid else {100: "boot:parent", 101: "boot:child"}[actual]):
                with self.assertRaises(RuntimeError):
                    self.invoke()

    def test_rejects_forged_environment_without_live_batch(self):
        self.ledger["batches"].clear()
        with self.assertRaises(RuntimeError):
            self.invoke()
        with patch.dict(os.environ, {}, clear=True), self.assertRaises(RuntimeError):
            self.invoke()

    def test_rejects_changed_plan_bytes_and_noncanonical_paths(self):
        self.batch["command_files"][0]["sha256"] = "e" * 64
        with self.assertRaises(RuntimeError):
            self.invoke()
        for changes in ({"plan": Path("relative.json")},
                        {"root": self.repo / "target/ordinary-world"}):
            with self.subTest(changes=changes), self.assertRaises(RuntimeError):
                self.invoke(**changes)

    def test_rejects_symlink_and_foreign_primary(self):
        link = self.repo / "alias.json"
        link.symlink_to(self.plan)
        with self.assertRaises(RuntimeError):
            self.invoke(plan=link)
        with patch.object(context.storage, "primary_for", return_value=Path("/foreign")), self.assertRaises(RuntimeError):
            self.invoke()

    def test_initial_publication_wait_releases_lock_and_does_not_retry_execution(self):
        child = self.batch.pop("child_pid")
        birth = self.batch.pop("child_identity")
        state = {"locked": False}

        @contextmanager
        def locked(_):
            self.assertFalse(state["locked"])
            state["locked"] = True
            try:
                yield self.ledger
            finally:
                state["locked"] = False

        def publish(_):
            self.assertFalse(state["locked"])
            self.batch.update(child_pid=child, child_identity=birth)

        with patch.object(context.storage, "locked", side_effect=locked), \
                patch.object(context.time, "sleep", side_effect=publish) as sleep:
            self.assertEqual(self.invoke()["runner_pid"], child)
            sleep.assert_called_once_with(0.02)

    def test_missing_publication_times_out_but_changed_parent_never_waits(self):
        self.batch.pop("child_pid")
        self.batch.pop("child_identity")
        with patch.object(context.time, "monotonic", side_effect=[0, 3]), \
                patch.object(context.time, "sleep") as sleep, self.assertRaises(RuntimeError):
            self.invoke()
        sleep.assert_not_called()
        self.batch["process_identity"] = "stale"
        with patch.object(context.time, "sleep") as sleep, self.assertRaises(RuntimeError):
            self.invoke()
        sleep.assert_not_called()

    def test_partial_or_foreign_publication_never_waits(self):
        for changes in ({"child_pid": None}, {"child_identity": None}, {"child_pid": 999}):
            original = copy.deepcopy(self.batch)
            self.batch.update(changes)
            with self.subTest(changes=changes), patch.object(context.time, "sleep") as sleep:
                with self.assertRaises(RuntimeError):
                    self.invoke()
                sleep.assert_not_called()
            self.batch.clear()
            self.batch.update(original)

    def test_returns_only_the_verified_plan_hash_not_a_second_unchecked_read(self):
        reads = []

        def digest(path):
            if path == self.plan:
                reads.append(path)
                return "d" * 64 if len(reads) == 1 else "e" * 64
            return "d" * 64

        with patch.object(context.storage, "digest", side_effect=digest):
            self.assertEqual(self.invoke()["plan_sha256"], "d" * 64)
        self.assertEqual(reads, [self.plan])


if __name__ == "__main__":
    unittest.main()
