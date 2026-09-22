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
            if self.spec.get("failed_dispatch"):
                return {"workers": [{"dispatchId": "ctx_blocked", "taskId": "task_blocked",
                                     "workerState": self.failed["worker"]["state"]}], "scope": {"run": "run_fixture"},
                        "page": {"total": 1, "hasMore": False}}
            return {"workers": [], "scope": {"run": "run_fixture"}, "page": {"total": 0, "hasMore": False}}
        if operation == "recovery-failed-input":
            return self.failed
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

    def failed_input(self):
        ticket = self.fixture.ticket
        common = L.roles.git(self.fixture.repo, "rev-parse", "--path-format=absolute", "--git-common-dir")
        subject = {"repo": str(self.fixture.repo), "common": common, "branch": ticket["branch"], "base": ticket["base"]}
        key = B.digest({"common": common, "id": ticket["id"]})
        bridge_id = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        self.role["last"] = {"phase": "unknown", "process_exited": True, "exit_code": 0,
                             "key": key, "attempt_id": bridge_id, "terminal": "term_refused",
                             "orca_bridge": bridge_id, "source_before": self.before["lanes"]["worker-a"]["source"]}
        B.save_state(self.role)
        B.claim_task(key, "worker-a", "codex", ticket, subject)
        self.attempt["bridge_id"] = bridge_id
        L.dispatch.save(self.attempt_path, self.attempt)
        self.directory = L.dispatch.task_bridge.root() / bridge_id
        L.STORAGE.write_ledger(self.directory / "journal.json", {"phase": "closed", "revoked": True,
                                                               "authority": None, "operations": {}})
        L.STORAGE.write_ledger(self.directory / "identity.json", {"terminal": "term_refused",
                                                                "repo": str(self.fixture.repo), "pid": 12345})
        p = patch.object(recovery.os, "kill", side_effect=ProcessLookupError)
        p.start()
        self.addCleanup(p.stop)
        self.failed = {"dispatch": {"id": "ctx_blocked", "taskId": "task_blocked", "runId": "run_fixture",
                                   "assigneeHandle": "term_refused", "status": "failed", "capabilityRevokedAt": "now",
                                   "lastFailure": "agent_prompt_blocked"},
                       "worker": {"dispatchId": "ctx_blocked", "state": "failed", "stage": "dispatch_input",
                                  "agentTerminalHandle": "term_refused"}, "observation": {"exactWorker": True}}
        self.spec.update(failed_dispatch="ctx_blocked", terminal_close=None, role_sha256=B.digest(self.role))

    def test_failed_unarmed_input_preserves_original_task_for_retry(self):
        self.failed_input()
        result = self.recover()
        receipt = L.STORAGE.read_private_json(recovery.Path(result["receipt"]), {})
        self.assertEqual(receipt["after"]["attempt"]["retry"], {"task": "task_blocked", "dispatch": "ctx_blocked"})
        self.assertEqual(receipt["before"]["role"], self.role)
        self.assertEqual(receipt["after"]["assignment"]["subject"]["base"], self.spec["new_base"])
        self.assertEqual(self.recover(), result)

    def test_armed_input_cannot_be_recovered_as_unstarted(self):
        self.failed_input()
        L.STORAGE.write_ledger(self.directory / "arm.json", {"dispatch": "ctx_blocked"})
        with self.assertRaisesRegex(ValueError, "unarmed"):
            self.recover()

    def test_failure_must_be_positive_and_input_specific(self):
        self.failed_input()
        self.failed["dispatch"]["status"] = "dispatched"
        with self.assertRaisesRegex(ValueError, "failed input"):
            self.recover()

    def bootstrap_input(self):
        self.failed_input()
        self.spec["bootstrap_session"] = True
        self.failed["worker"].update(state="abandoned", stage="abandoned")
        self.failed["dispatch"]["lastFailure"] = None
        L.STORAGE.write_ledger(self.directory / "journal.json", {"phase": "unknown", "revoked": True,
                                                               "authority": None, "operations": {}})
        L.STORAGE.write_ledger(self.attempt_path.with_suffix(".input-recovery.json"), {
            "before": self.attempt, "result": {"state": "ready", "stage": "input_accepted", "runId": "run_fixture",
                                               "taskId": "task_blocked", "dispatchId": "ctx_blocked"}})

    def test_unarmed_bootstrap_preserves_the_exact_provider_session(self):
        self.bootstrap_input()
        result = self.recover()
        data = L.load(self.request)
        self.assertEqual(data["lanes"]["worker-a"]["session"], self.fixture.sessions["worker-a"])
        role = B.read_state("worker-a", "codex")
        self.assertEqual(role["tasks"][self.role["last"]["key"]]["session_id"], self.fixture.sessions["worker-a"])
        self.assertEqual(role["last"]["phase"], "recorded")
        self.assertEqual(self.recover(), result)

    def test_arbitrary_bootstrap_history_without_accepted_input_is_not_adopted(self):
        self.bootstrap_input()
        L.STORAGE.write_ledger(self.attempt_path.with_suffix(".input-recovery.json"), {})
        with self.assertRaisesRegex(ValueError, "accepted input"):
            self.recover()


if __name__ == "__main__":
    unittest.main()
