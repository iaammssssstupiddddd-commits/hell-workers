from __future__ import annotations

import copy
import unittest
from unittest.mock import patch

from scripts import orca_notice_recovery as recovery
from scripts.tests import test_orca_review_loop as fixtures

L = recovery.loop


class NoticeRecoveryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.ReviewLoopTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.before = self.fixture.register()
        self.before.update(phase="paused", reason="inbox: unsupported message")
        self.before["run"] = {"phase": "ready", "context": {"id": "run_fixture", "consumer_generation": 1}}
        self.before["inbox"]["operation"] = {"kind": "check", "ack": None}
        L.save(self.before)
        self.row = {"id": "msg_notice", "run_id": "run_fixture", "to_handle": "run:run_fixture",
                    "from_handle": "term_origin", "type": "status", "subject": "diagnosis",
                    "body": "inspected status, not lifecycle", "payload": None, "thread_id": None}
        self.expected = {self.row["id"]: recovery.notice_hash(self.row)}
        self.acks = []
        self.fail_ack = False
        self.next_rows = []
        for target, name, options in ((L.dispatch, "checked_run", {}),
                                      (L.dispatch, "run_cli", {"side_effect": self.call})):
            p = patch.object(target, name, **options)
            p.start()
            self.addCleanup(p.stop)

    def call(self, cli, argv, operation):
        if operation == "notice-replay":
            return {"runId": "run_fixture", "acknowledged": None, "deliveryId": "delivery_notice",
                    "messages": [self.row], "count": 1}
        self.assertEqual(operation, "notice-ack")
        self.acks.append(argv[argv.index("--retry-request") + 1])
        if self.fail_ack:
            self.fail_ack = False
            raise RuntimeError("lost ACK reply")
        return {"runId": "run_fixture", "acknowledged": "delivery_notice",
                "deliveryId": "delivery_next" if self.next_rows else None,
                "messages": self.next_rows, "count": len(self.next_rows)}

    def recover(self):
        return recovery.recover(self.before["request_id"], self.expected, "Inspected root status", self.fixture.cli)

    def test_inspected_notice_is_preserved_and_acknowledged_once(self):
        result = self.recover()
        after = L.load(self.before["request_id"])
        self.assertEqual(after["phase"], "active")
        self.assertEqual(after["run"], self.before["run"])
        self.assertIn(self.row["id"], after["inbox"]["handled"])
        receipt = L.STORAGE.read_private_json(recovery.Path(result["receipt"]), {})
        self.assertEqual(receipt["before"], self.before)
        self.assertEqual(receipt["messages"], [self.row])
        self.assertEqual(self.recover(), result)
        self.assertEqual(len(self.acks), 1)

    def test_changed_notice_is_not_acknowledged(self):
        self.row["body"] = "different content"
        with self.assertRaisesRegex(ValueError, "differs"):
            self.recover()
        self.assertEqual(self.acks, [])
        self.assertEqual(L.load(self.before["request_id"]), self.before)

    def test_lifecycle_cannot_be_discarded_even_if_explicitly_hashed(self):
        self.row["type"] = "worker_done"
        self.expected[self.row["id"]] = recovery.notice_hash(self.row)
        with self.assertRaisesRegex(ValueError, "differs"):
            self.recover()
        self.assertEqual(self.acks, [])

    def test_lost_ack_replays_exact_retry_identity(self):
        self.fail_ack = True
        with self.assertRaisesRegex(RuntimeError, "lost ACK"):
            self.recover()
        self.assertTrue(self.recover()["recovered"])
        self.assertEqual(len(self.acks), 2)
        self.assertEqual(self.acks[0], self.acks[1])

    def test_following_lifecycle_batch_is_preserved_for_normal_decoder(self):
        row = copy.deepcopy(self.row)
        row.update(id="msg_done", type="worker_done", payload="{}")
        self.next_rows = [row]
        self.recover()
        inbox = L.load(self.before["request_id"])["inbox"]
        self.assertEqual(inbox["delivery"], "delivery_next")
        self.assertEqual(inbox["messages"]["msg_done"]["row"], row)

    def test_second_unsupported_batch_is_not_acknowledged(self):
        self.next_rows = [{**self.row, "id": "msg_other"}]
        with self.assertRaisesRegex(ValueError, "unsupported"):
            self.recover()
        self.assertEqual(L.load(self.before["request_id"]), self.before)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            self.recover()
        self.assertEqual(len(self.acks), 1)


if __name__ == "__main__":
    unittest.main()
