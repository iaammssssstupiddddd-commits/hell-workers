from __future__ import annotations

import unittest
from unittest.mock import patch

from scripts import orca_input_recovery as recovery
from scripts.tests import test_orca_review_loop as fixtures

L = recovery.loop


class InputRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.f = fixtures.ReviewLoopTests()
        self.f.setUp()
        self.addCleanup(self.f.doCleanups)
        self.data = self.f.register()
        self.data.update(phase="paused")
        self.data["run"] = {"phase": "ready", "context": {"id": "run_fixture", "consumer_generation": 1}}
        self.data["lanes"]["worker-a"]["phase"] = "paused"
        L.save(self.data)
        self.ticket = self.f.ticket
        self.path = L.dispatch.dispatch_path(self.data["request_id"], self.ticket)
        self.attempt = {"phase": "unknown", "terminal": "term_current", "bridge_id": "fixture",
                        "ticket_sha256": L.bindings.digest(self.ticket), "shared_run": self.data["run"]["context"]}
        L.dispatch.save(self.path, self.attempt)
        prior = self.f.root / "prior.json"
        L.STORAGE.write_ledger(prior, {"phase": "complete", "request_id": self.data["request_id"],
            "after": {"attempt": {"retry": {"task": "task_original", "dispatch": "ctx_failed"}}, "loop": self.data}})
        self.record = {"last": {"phase": "starting", "terminal": "term_current", "orca_bridge": "fixture"},
                       "recovery_receipt": str(prior)}
        self.calls = []
        for target, name, options in ((L.bindings, "read_state", {"return_value": self.record}),
                                      (L.dispatch, "checked_run", {}),
                                      (L.dispatch, "run_cli", {"side_effect": self.call}),
                                      (L.dispatch.task_bridge, "arm", {})):
            p = patch.object(target, name, **options)
            p.start()
            self.addCleanup(p.stop)

    def call(self, cli, argv, operation):
        self.calls.append((argv, operation))
        if operation == "input-latest":
            return {"dispatch": {"id": "ctx_failed", "status": "failed", "run_id": "run_fixture"}}
        return {"state": "ready", "stage": "input_accepted", "runId": "run_fixture",
                "taskId": "task_original", "dispatchId": "ctx_retry"}

    def recover(self):
        return recovery.recover(self.data["request_id"], self.f.cli)

    def test_reuses_live_controlled_terminal_and_original_task_then_resumes_loop(self):
        self.assertTrue(self.recover()["recovered"])
        argv = self.calls[-1][0]
        self.assertIn("--retry-request", argv)
        self.assertNotIn("--spec", argv)
        self.assertEqual(argv[argv.index("--terminal") + 1], "term_current")
        after = L.load(self.data["request_id"])
        self.assertEqual(after["phase"], "active")
        self.assertEqual(after["lanes"]["worker-a"]["phase"], "implementing")

    def test_changed_source_is_refused(self):
        (self.f.repo / "src/content.txt").write_text("unexpected edit")
        with self.assertRaisesRegex(ValueError, "source/launcher"):
            self.recover()
        self.assertEqual(self.calls, [])

    def test_acknowledged_start_is_not_resent_after_arm_interruption(self):
        with patch.object(L.dispatch.task_bridge, "arm", side_effect=RuntimeError("interrupted")):
            with self.assertRaisesRegex(RuntimeError, "interrupted"):
                self.recover()
        self.recover()
        self.assertEqual(sum(op == "input-retry" for _, op in self.calls), 1)


if __name__ == "__main__":
    unittest.main()
