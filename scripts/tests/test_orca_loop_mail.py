from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import orca_loop_mail as mail


class LoopMailTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="orca-mail-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.saved = []
        self.run = {"id": "run_fixture", "consumer_generation": 1,
                    "coordinator_handle": "term_coordinator", "legacy": 0}
        self.attempt = {"run_id": "run_fixture", "task_id": "task_fixture", "dispatch_id": "ctx_fixture",
                        "terminal": "term_worker", "slot": "worker-a", "released": False,
                        "bridge_id": "319674b0-1f30-4257-81a4-41542d5b48a8"}
        self.data = {"request_id": "fixture", "terminal": "term_coordinator", "phase": "active",
                     "run": {"phase": "ready", "context": {"id": "run_fixture", "consumer_generation": 1}},
                     "attempts": {"ctx_fixture": self.attempt},
                     "inbox": {"delivery": None, "messages": {}, "handled": {}, "operation": None}}
        self.journal = {"phase": "active", "authority": {
            "run": "run_fixture", "task": "task_fixture", "dispatch": "ctx_fixture", "coordinator": "term_coordinator"},
            "operations": {}}
        patcher = patch.object(mail.dispatch.task_bridge, "root", return_value=self.root)
        patcher.start()
        self.addCleanup(patcher.stop)
        patcher = patch.object(mail.dispatch, "run_cli", side_effect=self.call)
        self.cli = patcher.start()
        self.addCleanup(patcher.stop)
        self.batches = []
        self.acks = []
        self.replies = []
        self.bound = True
        self.write_journal()

    def save(self, data):
        self.saved.append(copy.deepcopy(data))

    def write_journal(self):
        mail.storage.write_ledger(self.root / self.attempt["bridge_id"] / "journal.json", self.journal)

    def row(self, kind, key):
        row = {"id": key, "run_id": "run_fixture", "from_handle": "term_worker", "to_handle": "run:run_fixture",
               "type": kind, "subject": "fixture", "body": "untrusted text", "payload": "{}"}
        result = {"messageId": key} if kind == "question" else {"message": copy.deepcopy(row)}
        if kind == "question":
            row.update(from_handle="dispatch:ctx_fixture", thread_id=key,
                       payload=json.dumps({"taskId": "task_fixture", "dispatchId": "ctx_fixture",
                                           "question": row["body"], "options": []}))
        self.journal["operations"][key] = {"phase": "confirmed", "result": result}
        self.write_journal()
        return row

    def call(self, executable, argv, operation):
        if operation == "run-create":
            self.bound = True
            return {"run": self.run}
        if operation == "run-current":
            if not self.bound:
                return {"run": None}
            return {"run": self.run}
        if operation == "loop-check":
            ack = argv[argv.index("--ack") + 1] if "--ack" in argv else None
            self.acks.append(ack)
            self.assertEqual(self.saved[-1]["inbox"]["operation"], {"kind": "check", "ack": ack})
            if ack:
                self.assertTrue(all(row["phase"] == "processed" for row in self.saved[-1]["inbox"]["messages"].values()))
            rows = self.batches.pop(0) if self.batches else []
            return {"runId": "run_fixture", "deliveryId": f"delivery_{len(self.acks)}" if rows else None,
                    "messages": rows, "count": len(rows), "acknowledged": ack}
        if operation == "loop-reply":
            key, body = argv[argv.index("--id") + 1], argv[argv.index("--body") + 1]
            self.assertEqual(self.saved[-1]["inbox"]["operation"]["kind"], "reply")
            self.replies.append(key)
            return {"question": {"message_id": key, "run_id": "run_fixture", "dispatch_id": "ctx_fixture",
                                 "status": "answered", "answer_body": body, "answer_message_id": "msg_answer",
                                 "answered_by_generation": 1},
                    "message": {"id": "msg_answer", "run_id": "run_fixture", "thread_id": key, "body": body,
                                "from_handle": "run:run_fixture", "to_handle": "dispatch:ctx_fixture"}}
        raise AssertionError(operation)

    def poll(self):
        return mail.poll(self.data, self.save)

    def test_one_run_is_created_once_and_reused(self):
        self.data["run"] = {"phase": "planned"}
        self.bound = False
        first = mail.ensure_run(self.data, self.save)
        self.assertEqual(mail.ensure_run(self.data, self.save), first)
        self.assertEqual(sum(call.args[2] == "run-create" for call in self.cli.call_args_list), 1)
        self.assertEqual(self.saved[0]["run"]["phase"], "creating")

    def test_reused_terminal_is_correlated_by_confirmed_receipt(self):
        row = self.row('worker_done', 'msg_current')
        old = {**self.attempt, 'dispatch_id': 'ctx_previous', 'task_id': 'task_previous',
               'bridge_id': 'fded5f49-0111-4f4b-89e8-84dab2b6ca43', 'released': True}
        self.data['attempts'][old['dispatch_id']] = old
        journal = {'phase': 'settled', 'authority': {
            'run': old['run_id'], 'task': old['task_id'], 'dispatch': old['dispatch_id'],
            'coordinator': self.data['terminal']}, 'operations': {}}
        mail.storage.write_ledger(self.root / old['bridge_id'] / 'journal.json', journal)
        self.assertEqual(mail.message_subject(self.data, row), self.attempt)
        journal['operations'] = copy.deepcopy(self.journal['operations'])
        mail.storage.write_ledger(self.root / old['bridge_id'] / 'journal.json', journal)
        with self.assertRaisesRegex(ValueError, 'multiple confirmed'):
            mail.message_subject(self.data, row)

    def test_inbox_resume_requires_exact_verified_pause_and_does_not_ack(self):
        from contextlib import nullcontext
        from scripts import orca_review_loop as loop
        row = self.row('worker_done', 'msg_current')
        self.data.update(phase='paused', reason='inbox: message has no unique owned Dispatch; coordinator reconciliation required')
        self.data['inbox']['messages'] = {row['id']: {'row': row, 'phase': 'received'}}
        expected = loop.bindings.digest(self.data)
        with patch.object(loop, 'load', return_value=self.data), \
             patch.object(loop, 'save', side_effect=self.save), \
             patch.object(loop, 'root', return_value=self.root), \
             patch.object(loop, 'acquire_host', return_value=nullcontext()), \
             patch.object(loop.dispatch, 'checked_coordinator'):
            with self.assertRaises(ValueError):
                loop.resume_inbox('fixture', 'term_coordinator', 'wrong')
            restored = loop.resume_inbox('fixture', 'term_coordinator', expected)
            self.assertEqual(restored['phase'], 'active')
            self.assertEqual(self.acks, [])
            self.assertEqual(restored['inbox']['messages'][row['id']]['phase'], 'received')

    def test_unknown_run_creation_is_not_replayed(self):
        self.data["run"] = {"phase": "planned"}
        self.bound = False
        real = self.call
        def fail(exe, argv, op):
            if op == "run-create":
                raise RuntimeError("lost reply")
            return real(exe, argv, op)
        self.cli.side_effect = fail
        with self.assertRaises(RuntimeError):
            mail.ensure_run(self.data, self.save)
        with self.assertRaisesRegex(ValueError, "outcome unknown"):
            mail.ensure_run(self.data, self.save)
        self.assertEqual(sum(call.args[2] == "run-create" for call in self.cli.call_args_list), 1)

    def test_existing_unowned_run_is_not_replaced(self):
        self.data["run"] = {"phase": "planned"}
        with self.assertRaisesRegex(ValueError, "already owns"):
            mail.ensure_run(self.data, self.save)
        self.assertEqual(self.data["run"]["phase"], "planned")
        self.assertEqual([call.args[2] for call in self.cli.call_args_list], ["run-current"])

    def test_run_consumer_changes_stop_before_mail_mutation(self):
        for field, value in (("consumer_generation", 2), ("coordinator_handle", "term_other"), ("id", "run_other")):
            original = self.run[field]
            self.run[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(RuntimeError, "consumer changed"):
                self.poll()
            self.run[field] = original
        self.assertEqual(self.acks, [])

    def test_question_and_done_whole_delivery_waits_for_answer_and_release(self):
        self.batches = [[self.row("question", "msg_question"), self.row("worker_done", "msg_done")]]
        self.assertTrue(self.poll())
        self.assertTrue(self.poll())
        self.assertEqual(mail.pending(self.data)[0]["id"], "msg_question")
        mail.decide(self.data, "msg_question", "Keep the scope", "reply", self.save)
        self.assertFalse(self.poll())
        self.assertEqual(self.acks, [None])
        self.attempt["released"] = True
        self.poll()
        self.assertEqual(self.acks, [None, "delivery_1"])
        self.assertEqual(set(self.data["inbox"]["handled"]), {"msg_question", "msg_done"})
        self.assertTrue(mail.drained(self.data))

    def test_missing_done_delivery_is_not_treated_as_drained(self):
        self.attempt["released"] = True
        self.poll()
        self.assertFalse(mail.drained(self.data))

    def test_answer_is_idempotent_locally_but_changed_answer_is_rejected(self):
        self.batches = [[self.row("question", "msg_question")]]
        self.poll()
        self.poll()
        for _ in range(2):
            mail.decide(self.data, "msg_question", "answer", "reply", self.save)
        self.assertEqual(self.replies, ["msg_question"])
        with self.assertRaisesRegex(ValueError, "different decision"):
            mail.decide(self.data, "msg_question", "different", "reply", self.save)

    def test_unknown_answer_never_auto_replies_or_acknowledges(self):
        self.batches = [[self.row("question", "msg_question")]]
        self.poll()
        self.poll()
        real = self.call
        self.cli.side_effect = lambda exe, argv, op: {} if op == "loop-reply" else real(exe, argv, op)
        with self.assertRaisesRegex(ValueError, "receipt"):
            mail.decide(self.data, "msg_question", "answer", "reply", self.save)
        with self.assertRaisesRegex(ValueError, "outcome unknown"):
            mail.decide(self.data, "msg_question", "answer", "reply", self.save)
        with self.assertRaisesRegex(ValueError, "outcome unknown"):
            self.poll()
        self.assertEqual(self.acks, [None])

    def test_unknown_check_is_preserved_without_replay(self):
        real = self.call
        def fail(exe, argv, op):
            if op == "loop-check":
                raise RuntimeError("lost Delivery")
            return real(exe, argv, op)
        self.cli.side_effect = fail
        with self.assertRaises(RuntimeError):
            self.poll()
        with self.assertRaisesRegex(ValueError, "outcome unknown"):
            self.poll()
        self.assertEqual(sum(call.args[2] == "loop-check" for call in self.cli.call_args_list), 1)

    def test_escalation_requires_explicit_decision(self):
        self.batches = [[self.row("escalation", "msg_blocker")]]
        self.poll()
        self.poll()
        self.assertEqual(self.acks, [None])
        mail.decide(self.data, "msg_blocker", "Needs a scope decision", "pause", self.save)
        self.assertEqual(self.data["phase"], "paused")
        self.assertEqual(self.replies, [])

    def test_pending_ask_waits_for_confirmed_receipt(self):
        row = self.row("question", "msg_question")
        self.journal["operations"]["msg_question"] = {"phase": "pending"}
        self.write_journal()
        self.batches = [[row]]
        self.poll()
        self.assertTrue(self.poll())
        self.assertEqual(mail.pending(self.data), [])
        self.row("question", "msg_question")
        self.poll()
        self.assertEqual(len(mail.pending(self.data)), 1)

    def test_question_uses_dispatch_address_and_exact_task_thread(self):
        row = self.row("question", "msg_question")
        self.assertEqual(mail.message_subject(self.data, row), self.attempt)
        for field, value in (("from_handle", "term_worker"), ("thread_id", "msg_old"),
                             ("payload", '{"taskId":"task_wrong"}')):
            with self.subTest(field=field), self.assertRaises(ValueError):
                mail.message_subject(self.data, {**row, field: value})

    def test_foreign_and_modified_messages_do_not_earn_ack(self):
        row = self.row("heartbeat", "msg_beat")
        for field, value in (("from_handle", "term_foreign"), ("run_id", "run_other"), ("body", "tampered")):
            changed = {**row, field: value}
            with self.subTest(field=field), self.assertRaises(ValueError):
                mail.message_subject(self.data, changed)
        self.assertEqual(self.acks, [])

    def test_bad_batches_do_not_overwrite_pending_operation(self):
        row = self.row("heartbeat", "msg_beat")
        for rows in ([row, row], [{**row, "type": "unknown"}], [{**row, "body": "x" * 32001}]):
            self.data["inbox"]["operation"] = {"kind": "check", "ack": None}
            with self.assertRaises(ValueError):
                mail.accept_batch(self.data, {"runId": "run_fixture", "messages": rows, "deliveryId": "delivery_1",
                                             "count": len(rows), "acknowledged": None}, None)
            self.assertIsNotNone(self.data["inbox"]["operation"])


if __name__ == "__main__":
    unittest.main()
