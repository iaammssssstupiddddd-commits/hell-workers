from __future__ import annotations

import copy
import json
import signal
import unittest
import uuid
from unittest.mock import Mock, patch

from scripts import host_coordination, orca_settlement_recovery as recovery
from scripts.tests import test_orca_roles as fixtures

roles, state, storage = recovery.roles, recovery.bindings, recovery.storage


class SettlementRecoveryTests(unittest.TestCase):
    run_git = staticmethod(fixtures.OrcaRoleTests.run_git)

    def setUp(self):
        fixtures.OrcaRoleTests.setUp(self)
        for module in (roles, state, host_coordination, roles.task_bridge):
            patcher = patch.object(module, "state_root", return_value=self.root / "state/coordination")
            patcher.start()
            self.addCleanup(patcher.stop)
        self.ticket.update(read_only=True, allowed_directories=[], review_base=self.ticket["base"],
                           source_sha256=roles.fingerprint(self.repo), validation_evidence="a" * 64)
        self.snapshot = {"session_id": str(uuid.uuid4()), "session_sha256": "b" * 64}
        patcher = patch.object(state, "session_snapshot", side_effect=lambda *args: dict(self.snapshot))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.bridge_id = str(uuid.uuid4())
        self.directory = roles.task_bridge.root() / self.bridge_id
        self.authority = {"run": "run_fixture", "task": "task_fixture", "dispatch": "ctx_fixture", "coordinator": "term_coordinator"}
        self.identity = {"runtime": "runtime", "terminal": "term_fixture", "repo": str(self.repo), "incarnation": "incarnation"}
        self.verdict = {"ticket": self.ticket["id"], "base": self.ticket["base"], "head": self.ticket["base"],
                        "source_sha256": self.ticket["source_sha256"], "validation_evidence": "a" * 64,
                        "verdict": "approved", "blocking_findings": []}
        self.journal = {"phase": "settled", "revoked": True, "settled_status": "completed", "authority": self.authority,
            "operations": {"done": {"phase": "confirmed", "result": {
                "message": {"type": "worker_done", "run_id": "run_fixture", "from_handle": "term_fixture",
                            "body": "Summary. Scope checked. Done.\nORCA_REVIEW_JSON:" + json.dumps(self.verdict)},
                "lifecycle": {"action": "completed", "taskId": "task_fixture", "dispatchId": "ctx_fixture"}}}}}
        for name, data in (("journal", self.journal), ("identity", self.identity), ("arm", self.authority)):
            storage.write_ledger(self.directory / f"{name}.json", data)
        self.before = {"schema": 1, "slot": "reviewer", "provider": "codex", "tasks": {"fixed-reviewer": {
            "key": "fixed-reviewer", "ticket_sha256": "c" * 64, "subject": recovery.loop.checkpoints.subject(self.ticket),
            "source_sha256": self.ticket["source_sha256"], "origin": str(self.repo), **self.snapshot}},
            "last": {"key": "fixed-reviewer", "phase": "unknown", "process_exited": True, "exit_code": -signal.SIGTERM,
                     "source_before": self.ticket["source_sha256"], "orca_bridge": self.bridge_id,
                     "terminal": "term_fixture", "attempt_id": str(uuid.uuid4())}}
        state.save_state(self.before)
        self.expected = state.digest(self.before)
        self.observed = {"dispatch": {"id": "ctx_fixture", "taskId": "task_fixture", "task_id": "task_fixture",
            "runId": "run_fixture", "assigneeHandle": "term_fixture", "status": "completed",
            "capabilityRevokedAt": "confirmed", "processIncarnation": "incarnation"},
            "worker": {"state": "succeeded", "dispatchId": "ctx_fixture", "runtimeEpoch": "runtime", "worktreeId": "wt"},
            "observation": {"exactWorker": True},
            "terminal": {"handle": "term_fixture", "incarnationId": "incarnation", "worktreeId": "wt"},
            "terminalResource": {"terminalHandle": "term_fixture", "worktreeId": "wt", "ownerDispatchId": "ctx_fixture"}}
        self.upstream = Mock(runtime_id="runtime")
        self.upstream.call.side_effect = lambda method, _: (self.observed if method == "orchestration.workerShow"
                                                           else {"dispatch": self.observed["dispatch"]})
        patcher = patch.object(roles.task_bridge.Upstream, "load", return_value=self.upstream)
        patcher.start()
        self.addCleanup(patcher.stop)

    def recover(self):
        return recovery.recover_role(self.ticket, self.expected, self.root)

    def test_exact_settlement_reconciles_without_message_or_new_session(self):
        result = self.recover()
        current = state.read_state("reviewer", "codex")
        self.assertEqual(current["last"]["provider_exit_code"], -signal.SIGTERM)
        self.assertEqual(current["last"]["exit_code"], 0)
        self.assertEqual(current["tasks"]["fixed-reviewer"]["session_id"], self.snapshot["session_id"])
        receipt = storage.read_private_json(self.directory / "settlement-recovery.json", {})
        self.assertEqual(receipt["before"], self.before)
        self.assertEqual(self.recover(), result)
        self.assertEqual({call.args[0] for call in self.upstream.call.call_args_list},
                         {"orchestration.workerShow", "orchestration.dispatchShow"})

    def test_stale_or_unexited_or_failed_process_cannot_be_normalized(self):
        for fields in ({"process_exited": False}, {"exit_code": 7}, {"phase": "starting"}):
            with self.subTest(fields=fields):
                changed = copy.deepcopy(self.before)
                changed["last"].update(fields)
                state.save_state(changed)
                with self.assertRaises(ValueError):
                    recovery.recover_role(self.ticket, state.digest(changed), self.root)
        state.save_state(self.before)
        with self.assertRaisesRegex(ValueError, "inspected"):
            recovery.recover_role(self.ticket, "0" * 64, self.root)

    def test_changed_source_or_session_or_dispatch_is_refused(self):
        self.snapshot["session_id"] = str(uuid.uuid4())
        with self.assertRaisesRegex(ValueError, "session changed"):
            self.recover()
        self.snapshot["session_id"] = self.before["tasks"]["fixed-reviewer"]["session_id"]
        self.observed["dispatch"]["processIncarnation"] = "other"
        with self.assertRaisesRegex(ValueError, "read-back"):
            self.recover()
        (self.repo / "src/content.txt").write_text("changed")
        with self.assertRaisesRegex(ValueError, "unchanged source"):
            self.recover()

    def test_pending_operation_is_not_accepted(self):
        self.journal["operations"]["pending"] = {"phase": "pending"}
        storage.write_ledger(self.directory / "journal.json", self.journal)
        with self.assertRaisesRegex(ValueError, "unconfirmed operation"):
            self.recover()

    def test_interruption_after_role_write_replays_only_exact_after_state(self):
        write = storage.write_ledger
        def interrupted(path, data):
            if path.name == "settlement-recovery.json" and data["phase"] == "complete":
                raise OSError("interrupted")
            return write(path, data)
        with patch.object(storage, "write_ledger", side_effect=interrupted), self.assertRaises(OSError):
            self.recover()
        self.recover()
        self.snapshot["session_sha256"] = "e" * 64
        with self.assertRaisesRegex(ValueError, "history changed"):
            self.recover()
