from __future__ import annotations

import copy
import unittest
from unittest.mock import patch

from scripts import orca_prelaunch_recovery as recovery
from scripts.tests import test_orca_review_loop as fixtures

L = recovery.loop
B = recovery.B


class PrelaunchRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.ReviewLoopTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        f = self.fixture
        self.target = f.root / "integration"
        f.run_git(f.primary, "worktree", "add", "-qb", "hw-42-integration", str(self.target))
        f.spec["integration"] = {"target": {"repo": str(self.target), "branch": "hw-42-integration", "base": f.ticket["base"]},
                                 "validation": f.spec["lanes"][0]["validation"]}
        self.before = f.register()
        self.before["lanes"]["worker-a"]["phase"] = "dispatching"
        self.before["run"] = {"phase": "ready", "context": {"id": "run_fixture", "consumer_generation": 1}}
        L.save(self.before)
        self.request = self.before["request_id"]
        self.role = {"schema": 1, "slot": "worker-a", "provider": "codex", "tasks": {},
                     "last": {"phase": "unknown", "process_exited": True, "exit_code": -9}}
        B.save_state(self.role)
        self.attempt = {"phase": "unknown", "slot": "worker-a", "ticket_sha256": B.digest(f.ticket),
                        "shared_run": self.before["run"]["context"], "terminal": "term_refused",
                        "bridge_id": None, "task_id": None, "dispatch_id": None}
        self.attempt_path = L.dispatch.dispatch_path(self.request, f.ticket)
        L.dispatch.save(self.attempt_path, self.attempt)
        (f.primary / "scripts/fix.py").write_text("# recovery fix\n")
        f.run_git(f.primary, "add", ".")
        f.run_git(f.primary, "commit", "-qm", "fix tooling")
        self.spec = {"slot": "worker-a", "loop_sha256": B.digest(self.before), "role_sha256": B.digest(self.role),
                     "new_base": L.roles.git(f.primary, "rev-parse", "HEAD"), "reason": "Explicitly retry refused prelaunch",
                     "terminal_close": {"ok": True, "result": {"close": {"handle": "term_refused", "ptyKilled": True}}}}
        patcher = patch.object(L.dispatch, "run_cli", side_effect=self.call)
        patcher.start()
        self.addCleanup(patcher.stop)

    def call(self, cli, argv, operation):
        if operation == "recovery-status":
            return {"running": True}
        if operation == "run-current":
            return {"run": {**self.before["run"]["context"], "coordinator_handle": self.before["terminal"], "legacy": 0}}
        if operation == "recovery-workers":
            return {"workers": [], "scope": {"run": "run_fixture"}, "page": {"total": 0, "hasMore": False}}
        if operation == "recovery-terminals":
            return {"terminals": [], "truncated": False}
        self.fail(operation)

    def recover(self):
        return recovery.recover(self.request, self.spec, self.fixture.cli)

    def test_recovery_preserves_evidence_and_same_run_and_is_idempotent(self):
        result = self.recover()
        receipt = L.STORAGE.read_private_json(recovery.Path(result["receipt"]), {})
        self.assertEqual(receipt["before"], {"loop": self.before, "role": self.role, "attempt": self.attempt})
        self.assertEqual(L.load(self.request)["run"], self.before["run"])
        self.assertEqual(L.load(self.request)["lanes"]["worker-a"]["phase"], "planned")
        self.assertIsNone(B.read_state("worker-a", "codex")["last"])
        for repo in (self.fixture.repo, self.target):
            self.assertEqual(L.roles.git(repo, "rev-parse", "HEAD"), self.spec["new_base"])
        self.assertEqual(self.recover(), result)

    def test_closed_receipt_is_required(self):
        self.spec["terminal_close"]["result"]["close"]["ptyKilled"] = False
        with self.assertRaisesRegex(ValueError, "positive close"):
            self.recover()
        self.assertEqual(L.load(self.request), self.before)

    def test_dirty_workspace_is_not_overwritten(self):
        (self.fixture.repo / "src/content.txt").write_text("preserve this work")
        with self.assertRaisesRegex(ValueError, "workspace changed"):
            self.recover()
        self.assertEqual((self.fixture.repo / "src/content.txt").read_text(), "preserve this work")

    def test_role_with_identity_cannot_be_quarantined(self):
        data = copy.deepcopy(self.role)
        data["last"]["attempt_id"] = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        B.save_state(data)
        self.spec["role_sha256"] = B.digest(data)
        with self.assertRaisesRegex(ValueError, "identity-less"):
            self.recover()

    def test_dispatch_identity_cannot_be_retried_as_prelaunch(self):
        data = {**self.attempt, "dispatch_id": "ctx_live"}
        L.dispatch.save(self.attempt_path, data)
        with self.assertRaisesRegex(ValueError, "reached dispatch"):
            self.recover()

    def test_changed_state_is_not_overwritten(self):
        self.spec["role_sha256"] = "a" * 64
        with self.assertRaisesRegex(ValueError, "role changed"):
            self.recover()

    def test_active_role_lock_prevents_recovery(self):
        with L.acquire_host("worker-a", inherit=False):
            with self.assertRaises(L.HostBusyError):
                self.recover()

    def test_partial_state_write_is_replayable_from_preserved_journal(self):
        save = L.STORAGE.write_ledger
        failed = False
        def fail(path, data):
            nonlocal failed
            if path == self.attempt_path and not failed:
                failed = True
                raise OSError("power loss")
            return save(path, data)
        with patch.object(L.STORAGE, "write_ledger", side_effect=fail):
            with self.assertRaisesRegex(OSError, "power loss"):
                self.recover()
        self.assertTrue(self.recover()["recovered"])


if __name__ == "__main__":
    unittest.main()
