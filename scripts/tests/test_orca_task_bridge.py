"""Installed 1.4.205 wire fixtures; no real Task, provider or credentials used."""

from __future__ import annotations

import copy
import json
import os
import shutil
import socket
import socketserver
import sys
import tempfile
import threading
import time
import unittest
import uuid
from pathlib import Path
from unittest.mock import Mock, patch

from scripts import host_coordination, orca_roles as roles, orca_task_bridge as bridge
from scripts.tests.test_orca_preflight import CLI, HANDLE, INCARNATION, RUNTIME, SECRET


CAP = "dcap_fixture_only_not_a_live_capability"
COORDINATOR = "term_175c1be5-9f01-4a44-8268-a0542fa4e781"
AUTHORITY = {"run": "run_fixture", "task": "task_fixture", "dispatch": "dispatch_fixture",
             "coordinator": COORDINATOR}
FLAGS = {"timedOut": False, "cancelled": False, "connectionLost": False}


class Runtime:
    """Wire field names from installed dispatchShow, workerShow, $jn and ask."""
    runtime_id, token = RUNTIME, SECRET

    def __init__(self, repo):
        self.calls = []
        self.row = {"handle": HANDLE, "incarnationId": INCARNATION,
                    "worktreeId": "fixture::" + str(repo), "worktreePath": str(repo),
                    "executionHostId": "local", "connected": True, "orphaned": False}
        self.latest = {"id": AUTHORITY["dispatch"], "task_id": AUTHORITY["task"], "status": "dispatched"}
        self.observed = {
            "dispatch": {"id": AUTHORITY["dispatch"], "taskId": AUTHORITY["task"], "runId": AUTHORITY["run"],
                         "assigneeHandle": HANDLE, "processIncarnation": INCARNATION,
                         "status": "dispatched", "capabilityRevokedAt": None},
            "worker": {"dispatchId": AUTHORITY["dispatch"], "runtimeEpoch": RUNTIME,
                       "agentTerminalHandle": HANDLE, "worktreeId": self.row["worktreeId"], "state": "ready"},
            "observation": {"exactWorker": True, "status": "live"},
        }
        self.check_result = {"runId": AUTHORITY["run"], "dispatchId": AUTHORITY["dispatch"],
                             "deliveryId": None, "messages": [], "count": 0, "acknowledged": None, **FLAGS}
        self.ask_result = {"messageId": "msg_question", "threadId": "msg_question", "answer": None,
                           "timeoutMs": 1000, **FLAGS, "timedOut": True}
        self.before_mutation = lambda: None
        self.change = lambda method, result: None

    def call(self, method, params=None, envelope=None):
        self.calls.append((method, copy.deepcopy(params), copy.deepcopy(envelope)))
        if method == "status.get":
            result = {"runtimeId": RUNTIME, "appVersion": bridge.wire.VERSION, "graphStatus": "ready",
                      "capabilities": [bridge.CONTRACT], "authToken": SECRET}
        elif method == "terminal.show":
            result = {"terminal": copy.deepcopy(self.row)}
        elif method == "orchestration.dispatchShow":
            result = {"dispatch": copy.deepcopy(self.latest)}
        elif method == "orchestration.workerShow":
            result = copy.deepcopy(self.observed)
        else:
            self.before_mutation()
            if method == "orchestration.send":
                result = {"message": {"id": "msg_sent", "run_id": AUTHORITY["run"], "from_handle": HANDLE,
                          "to_handle": "run:" + AUTHORITY["run"],
                          **{k: params[k] for k in ("type", "subject", "body", "payload") if k in params}}}
                if params["type"] == "worker_done":
                    outcome = json.loads(params["payload"])["outcome"]
                    state = "completed" if outcome == "succeeded" else "failed"
                    self.latest["status"] = state
                    self.observed["dispatch"].update(status=state, capabilityRevokedAt=1)
                    self.observed["worker"].update(state=outcome, stage="settled")
                    result["lifecycle"] = {"action": state, "taskId": AUTHORITY["task"], "dispatchId": AUTHORITY["dispatch"]}
            elif method == "orchestration.check":
                result = copy.deepcopy(self.check_result)
            elif method == "orchestration.ask":
                result = copy.deepcopy(self.ask_result)
            else:
                raise AssertionError(method)
            result["mutation"] = {"requestId": envelope["orchestrationRequestId"], "replayed": False}
        self.change(method, result)
        return result


class RuntimeHandler(socketserver.BaseRequestHandler):
    def handle(self):
        request = bridge.wire.receive(self.request, time.monotonic() + 2)
        if request["authToken"] != SECRET:
            raise AssertionError("fixture transport authentication missing")
        result = self.server.runtime.call(request["method"], request.get("params"), request)
        try:
            self.request.sendall(json.dumps({"id": request["id"], "ok": True, "result": result,
                                            "_meta": {"runtimeId": RUNTIME}}).encode() + b"\n")
        except BrokenPipeError:
            pass  # Absolute-deadline negative test deliberately closes first.


class TaskBridgeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="hwt-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        self.storage = tempfile.TemporaryDirectory(prefix="task-bridge-", dir=target)
        self.addCleanup(self.storage.cleanup)
        self.repo = Path(self.storage.name)
        self.runtime = Runtime(self.repo)
        self.binding = bridge.wire.Binding.discover(self.runtime, HANDLE, self.repo)
        self.subject = Mock()
        for module in (bridge, host_coordination, roles):
            patcher = patch.object(module, "state_root", return_value=self.repo / "state/coordination")
            patcher.start()
            self.addCleanup(patcher.stop)
        patcher = patch.object(bridge, "root", return_value=self.root)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.policy = self.new_policy()

    def new_policy(self):
        directory = self.root / str(uuid.uuid4())
        directory.mkdir(mode=0o700)
        bridge.write_ledger(directory / "arm.json", AUTHORITY)
        return bridge.TaskPolicy(self.runtime, self.binding, directory, self.subject)

    def request(self, method="orchestration.send", params=None, **changes):
        operation = str(uuid.uuid4())
        if params is None:
            params = {"from": HANDLE, "subject": "Alive", "type": "heartbeat", "devMode": False,
                      "payload": json.dumps({"taskId": AUTHORITY["task"], "dispatchId": AUTHORITY["dispatch"], "phase": "reading"})}
        result = {"id": str(uuid.uuid4()), "authToken": self.policy.token, "method": method, "params": params,
                  "orchestrationContractVersion": 1, "orchestrationRequestId": operation,
                  "compatibilityInvocationId": operation}
        if method in {"orchestration.send", "orchestration.ask"}:
            result["orchestrationCapability"] = CAP
        return {**result, **changes}

    def check(self, **extra):
        return self.request("orchestration.check", {"terminal": HANDLE, "compatibilityCliCommand": "orca-ide", **extra})

    def ask(self, **extra):
        return self.request("orchestration.ask", {"from": HANDLE, "compatibilityCliCommand": "orca-ide", "timeoutMs": 1000, **extra})

    def done(self, outcome="succeeded"):
        request = self.request()
        request["params"].update(type="worker_done", subject="Complete", body="Read source. No edits. Checks passed.",
                                 waitForLifecycleSettlement=True, payload=json.dumps({"taskId": AUTHORITY["task"],
                                 "dispatchId": AUTHORITY["dispatch"], "outcome": outcome}))
        return request

    def mutations(self):
        return [c for c in self.runtime.calls if c[0] in bridge.METHODS]

    def test_pending_journal_precedes_exact_capability_forward_and_no_secrets_escape(self):
        def inspect():
            journal = json.loads((self.policy.directory / "journal.json").read_text())
            self.assertEqual([r["phase"] for r in journal["operations"].values()], ["pending"])
        self.runtime.before_mutation = inspect
        reply = self.policy.handle(self.request())
        self.assertTrue(reply["ok"], reply)
        self.assertEqual(self.mutations()[0][2]["orchestrationCapability"], CAP)
        journal = (self.policy.directory / "journal.json").read_text()
        for secret in (SECRET, CAP, self.policy.token):
            self.assertNotIn(secret, json.dumps(reply) + journal)

    def test_send_receipt_accepts_equivalent_json_serialization(self):
        request = self.request()
        request["params"]["type"] = "escalation"
        request["params"]["subject"] = "Need a decision"
        request["params"]["payload"] = json.dumps({
            "taskId": AUTHORITY["task"],
            "dispatchId": AUTHORITY["dispatch"],
            "phase": "blocked",
        }, indent=2)

        def compact_payload(method, result):
            if method == "orchestration.send":
                result["message"]["payload"] = (
                    '{"phase":"\\u0062locked","dispatchId":"dispatch_fixture",'
                    '"taskId":"task_fixture"}'
                )

        self.runtime.change = compact_payload
        reply = self.policy.handle(request)
        self.assertTrue(reply["ok"], reply)
        self.assertEqual(len(self.mutations()), 1)

    def test_send_receipt_rejects_json_content_or_type_change(self):
        for replacement in (
            {"taskId": AUTHORITY["task"], "dispatchId": AUTHORITY["dispatch"], "phase": "changed"},
            {"taskId": "other", "dispatchId": AUTHORITY["dispatch"], "phase": "reading"},
            [
                {"taskId": AUTHORITY["task"], "dispatchId": AUTHORITY["dispatch"], "phase": "reading"}
            ],
        ):
            with self.subTest(replacement=replacement):
                self.runtime = Runtime(self.repo)
                self.policy = self.new_policy()

                def change_payload(method, result, replacement=replacement):
                    if method == "orchestration.send":
                        result["message"]["payload"] = json.dumps(replacement)

                self.runtime.change = change_payload
                reply = self.policy.handle(self.request())
                self.assertFalse(reply["ok"])
                self.assertEqual(self.policy.phase, "unknown")
                self.assertEqual(len(self.mutations()), 1)

    def test_settlement_requires_exact_live_verdict_and_supports_failed_outcome(self):
        for outcome, state in (("succeeded", "completed"), ("failed", "failed")):
            with self.subTest(outcome=outcome):
                self.runtime = Runtime(self.repo)
                self.policy = self.new_policy()
                request = self.done(outcome)
                reply = self.policy.handle(request)
                self.assertTrue(reply["ok"], reply)
                self.assertEqual(reply["result"]["lifecycle"]["action"], state)
                self.assertEqual(self.policy.phase, "settled")
                self.assertTrue(self.policy.handle(request)["ok"])
                self.assertEqual(len(self.mutations()), 1)
                self.assertFalse(self.policy.handle(self.request())["ok"])

    def test_completion_without_lifecycle_verdict_is_unknown_not_success(self):
        def change(method, result):
            if method == "orchestration.send":
                result.pop("lifecycle", None)
        self.runtime.change = change
        self.assertFalse(self.policy.handle(self.done())["ok"])
        self.assertEqual(self.policy.phase, "unknown")
        self.assertEqual(len(self.mutations()), 1)

    def test_replay_is_local_but_revalidates_current_generation(self):
        request = self.request()
        self.assertTrue(self.policy.handle(request)["ok"])
        self.assertTrue(self.policy.handle(request)["ok"])
        self.assertEqual(len(self.mutations()), 1)
        self.runtime.row["incarnationId"] = str(uuid.uuid4())
        self.assertFalse(self.policy.handle(request)["ok"])
        self.assertEqual(len(self.mutations()), 1)

    def test_changed_replay_and_unknown_upstream_replay_are_refused(self):
        request = self.request()
        self.assertTrue(self.policy.handle(request)["ok"])
        request["params"]["subject"] = "Changed"
        self.assertFalse(self.policy.handle(request)["ok"])
        self.assertEqual(len(self.mutations()), 1)
        self.policy = self.new_policy()
        def replay(method, result):
            if "mutation" in result:
                result["mutation"]["replayed"] = True
        self.runtime.change = replay
        self.assertFalse(self.policy.handle(self.request())["ok"])

    def test_scope_and_envelope_fail_before_mutation(self):
        changes = [{"params": {"run": "other"}}, {"method": "terminal.send"},
                   {"orchestrationCompatibilityEvidence": {}}, {"orchestrationCapability": "wrong"},
                   {"orchestrationContractVersion": True}, {"compatibilityInvocationId": str(uuid.uuid4())}]
        for change in changes:
            with self.subTest(change=change):
                self.policy = self.new_policy()
                self.assertFalse(self.policy.handle(self.request(**change))["ok"])
                self.assertEqual(self.mutations(), [])

    def test_pending_question_can_only_resume_itself_not_old_answered_question(self):
        self.runtime.ask_result.update(answer="yes", answerMessageId="msg_answer", timedOut=False)
        self.assertTrue(self.policy.handle(self.ask(question="First?"))["ok"])
        self.runtime.ask_result.update(messageId="msg_second", threadId="msg_second", answer=None, timedOut=True)
        self.runtime.ask_result.pop("answerMessageId")
        self.assertTrue(self.policy.handle(self.ask(question="Second?"))["ok"])
        self.assertEqual(self.policy.pending_question, "msg_second")
        self.assertFalse(self.policy.handle(self.ask(resume="msg_question"))["ok"])
        self.assertEqual(self.policy.pending_question, "msg_second")
        self.assertEqual(len(self.mutations()), 2)

    def test_ask_timeout_resume_and_accepted_are_not_answer_or_approval(self):
        self.runtime.ask_result.update(accepted=True, timedOut=False)
        self.assertTrue(self.policy.handle(self.ask(question="Proceed?"))["ok"])
        self.assertEqual(self.policy.pending_question, "msg_question")
        self.runtime.ask_result.update(answer="yes", answerMessageId="msg_answer")
        self.assertTrue(self.policy.handle(self.ask(resume="msg_question"))["ok"])
        self.assertIsNone(self.policy.pending_question)
        self.assertEqual(self.policy.phase, "active")

    def test_unresolved_question_or_delivery_prevents_done(self):
        for kind in ("pending_question", "delivery"):
            with self.subTest(kind=kind):
                self.policy = self.new_policy()
                setattr(self.policy, kind, "unresolved")
                self.assertFalse(self.policy.handle(self.done())["ok"])
                self.assertEqual(self.mutations(), [])

    def test_check_wait_arrival_and_empty_arrival_normalize_absent_flags(self):
        for messages in ([], [{"id": "msg_in", "run_id": AUTHORITY["run"], "from_handle": COORDINATOR,
                              "to_handle": "dispatch:" + AUTHORITY["dispatch"], "type": "message", "subject": "Read this"}]):
            with self.subTest(messages=messages):
                self.policy = self.new_policy()
                self.runtime.check_result.update(messages=messages, count=len(messages), deliveryId="delivery_one" if messages else None)
                for flag in FLAGS:
                    self.runtime.check_result.pop(flag, None)
                reply = self.policy.handle(self.check(wait=True, timeoutMs=1000))
                self.assertTrue(reply["ok"], reply)
                self.assertEqual({f: reply["result"][f] for f in FLAGS}, FLAGS)
        self.runtime.check_result.update(messages=[], count=0, deliveryId=None, acknowledged="delivery_one", **FLAGS)
        self.assertTrue(self.policy.handle(self.check(ack="delivery_one"))["ok"])
        self.assertIsNone(self.policy.delivery)

    def test_ack_unseen_delivery_foreign_mail_and_oversized_wait_are_denied(self):
        for request in (self.check(ack="unseen"), self.check(run="other"), self.ask(question="Wait?", timeoutMs=600000)):
            self.policy = self.new_policy()
            request["authToken"] = self.policy.token
            self.assertFalse(self.policy.handle(request)["ok"])
            self.assertEqual(self.mutations(), [])
        self.policy = self.new_policy()
        self.runtime.check_result["dispatchId"] = "other"
        self.assertFalse(self.policy.handle(self.check())["ok"])

    def test_live_source_worker_and_dispatch_changes_refuse_before_forwarding(self):
        for field, value in (("runtimeEpoch", "other"), ("state", "starting"), ("agentTerminalHandle", "other")):
            with self.subTest(field=field):
                self.runtime = Runtime(self.repo)
                self.runtime.observed["worker"][field] = value
                self.policy = self.new_policy()
                self.assertFalse(self.policy.handle(self.request())["ok"])
                self.assertEqual(self.mutations(), [])
        self.runtime = Runtime(self.repo)
        self.policy = self.new_policy()
        self.subject.side_effect = ValueError("source changed")
        self.assertFalse(self.policy.handle(self.request())["ok"])
        self.assertEqual(self.mutations(), [])

    def test_exception_after_pending_never_retries_or_leaks(self):
        self.runtime.before_mutation = Mock(side_effect=RuntimeError(SECRET))
        request = self.request()
        reply = self.policy.handle(request)
        self.assertFalse(reply["ok"])
        self.assertNotIn(SECRET, json.dumps(reply))
        self.assertEqual(self.policy.operations[request["orchestrationRequestId"]]["phase"], "pending")
        self.assertFalse(self.policy.handle(request)["ok"])
        self.assertEqual(len(self.mutations()), 1)

    def test_unarmed_bridge_has_bounded_wait_and_never_forwards(self):
        (self.policy.directory / "arm.json").unlink()
        with patch.object(bridge, "ARM_SECONDS", 0):
            self.assertFalse(self.policy.handle(self.request())["ok"])
        self.assertEqual(self.mutations(), [])

    def wire_session(self):
        config = self.root / "upstream"
        config.mkdir(mode=0o700, exist_ok=True)
        server = socketserver.UnixStreamServer(str(self.root / "upstream.sock"), RuntimeHandler)
        server.runtime = self.runtime
        thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
        thread.start()
        def stop():
            server.shutdown()
            thread.join()
            server.server_close()
        self.addCleanup(stop)
        bridge.write_ledger(config / "orca-runtime.json", {"runtimeId": RUNTIME, "authToken": SECRET,
                            "transports": [{"kind": "unix", "endpoint": str(self.root / "upstream.sock")}]})
        executable = CLI if CLI.is_file() else Path("/usr/bin/true")
        return bridge.Session(executable, config, HANDLE, self.repo, self.subject)

    def test_session_arm_is_host_only_exact_once_and_teardown_removes_transport(self):
        session = self.wire_session()
        with session:
            bridge.arm(session.identifier, AUTHORITY)
            with self.assertRaises(bridge.wire.Refused):
                bridge.arm(session.identifier, AUTHORITY)
            self.assertEqual(set(p.name for p in session.public.iterdir()), {"orca-runtime.json", "rpc.sock"})
            self.assertEqual(session.policy.phase, "bootstrap")
        self.assertEqual(list(session.public.iterdir()), [])
        self.assertEqual(session.policy.phase, "unknown")
        self.assertLessEqual(len(os.fsencode(session.public / "rpc.sock")), 107)

    def test_enter_failure_drains_server_and_removes_credentials(self):
        session = self.wire_session()
        with patch("builtins.print", side_effect=BrokenPipeError), self.assertRaises(BrokenPipeError):
            session.__enter__()
        self.assertFalse(session.thread.is_alive())
        self.assertEqual(list(session.public.iterdir()), [])
        self.assertEqual(session.terminal_lease.fd, -1)

    def test_start_failure_before_thread_cleans_transport_without_shutdown_deadlock(self):
        session = self.wire_session()
        with patch.object(bridge, "Proxy", side_effect=OSError("fixture bind failure")), self.assertRaises(OSError):
            session.__enter__()
        self.assertIsNone(session.thread)
        self.assertEqual(list(session.public.iterdir()), [])
        self.assertEqual(session.terminal_lease.fd, -1)

    def test_same_terminal_cannot_open_another_session_before_first_drains(self):
        session = self.wire_session()
        duplicate = bridge.Session(session.executable, session.upstream.metadata_path.parent, HANDLE, self.repo, self.subject)
        with session, self.assertRaises(host_coordination.HostBusyError):
            duplicate.__enter__()

    def test_non_ascii_auth_token_revokes_and_records_unknown_without_forwarding(self):
        session = self.wire_session()
        with session:
            with socket.socket(socket.AF_UNIX) as client:
                client.settimeout(2)
                client.connect(str(session.public / "rpc.sock"))
                client.sendall(json.dumps({"id": str(uuid.uuid4()), "authToken": "不正", "method": "status.get"}).encode() + b"\n")
                self.assertEqual(client.recv(1024), b"")
            self.assertTrue(session.policy.revoked)
            journal = json.loads((session.directory / "journal.json").read_text())
            self.assertEqual(journal["phase"], "unknown")
            self.assertEqual(self.mutations(), [])

    @unittest.skipUnless(shutil.which("bwrap"), "Linux bubblewrap required")
    def test_role_mounts_only_own_runtime_and_proxy_not_other_roles_or_control(self):
        session = self.wire_session()
        private = self.repo / "state"
        own = private / "agents/worker-a"
        for name in ("codex", "tmp"):
            (own / name).mkdir(parents=True, exist_ok=True)
        other = private / "agents/worker-b"
        other.mkdir()
        (other / "secret").write_text("fixture-other-role-secret")
        ticket = {"repo": str(self.repo), "read_only": True, "allowed_directories": []}
        script = """import json, os, pathlib, socket, sys
own, other, public, control, upstream = map(pathlib.Path, sys.argv[1:])
assert own.is_dir() and not other.exists()
assert not (control / 'journal.json').exists()
assert set(k for k in os.environ if k.startswith('ORCA_')) == {'ORCA_USER_DATA_PATH'}
try:
    (public / 'orca-runtime.json').write_text('forbidden')
    raise AssertionError('proxy metadata writable')
except OSError:
    pass
assert not upstream.is_socket()
metadata = json.loads((public / 'orca-runtime.json').read_text())
request = {'id':'f8c5a495-fc16-40b5-83c1-05620bf2c55c','authToken':metadata['authToken'],'method':'status.get'}
with socket.socket(socket.AF_UNIX) as stream:
    stream.connect(metadata['transports'][0]['endpoint'])
    stream.sendall(json.dumps(request).encode() + b'\\n')
    reply = json.loads(stream.makefile().readline())
assert reply['ok'] is True
print('isolated')
"""
        with session, patch.object(roles, "git", return_value=str(self.repo / ".git")), \
                patch.dict(os.environ, {"CODEX_HOME": str(self.repo / "no-auth"), "ORCA_TERMINAL_HANDLE": HANDLE}):
            command = roles.sandbox_command(ticket, "worker", own, [sys.executable, "-c", script,
                        *map(str, (own, other, session.public, session.directory, session.upstream.endpoint))], bridge=session)
            result = bridge.wire.run_cli(command, 8)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.strip(), b"isolated")

    @unittest.skipUnless(CLI.is_file() and shutil.which("bwrap"), "installed Orca and bubblewrap required")
    def test_stock_cli_messages_checks_asks_and_settlement_through_proxy(self):
        session = self.wire_session()
        with session:
            bridge.arm(session.identifier, AUTHORITY)
            commands = [
                ["send", "--from", HANDLE, "--dispatch-capability", CAP, "--type", "heartbeat", "--subject", "Alive",
                 "--task-id", AUTHORITY["task"], "--dispatch-id", AUTHORITY["dispatch"], "--phase", "reading"],
                ["check", "--terminal", HANDLE, "--wait", "--timeout-ms", "1000"],
                ["ask", "--from", HANDLE, "--dispatch-capability", CAP, "--question", "Proceed?", "--timeout-ms", "1000"],
                ["ask", "--from", HANDLE, "--dispatch-capability", CAP, "--resume", "msg_question", "--timeout-ms", "1000"],
                ["send", "--from", HANDLE, "--dispatch-capability", CAP, "--type", "worker_done", "--subject", "Complete",
                 "--body", "Read source. No edits. Complete.", "--task-id", AUTHORITY["task"], "--dispatch-id", AUTHORITY["dispatch"],
                 "--outcome", "succeeded"],
            ]
            for index, argv in enumerate(commands):
                if index == 3:
                    self.runtime.ask_result.update(answer="yes", answerMessageId="msg_answer", timedOut=False)
                command = bridge.wire.sandbox(CLI, session.public, session.upstream, self.repo, ["orchestration", *argv, "--json"])
                completed = bridge.wire.run_cli(command, 8)
                output = bridge.wire.decode(completed.stdout)
                self.assertTrue(output.get("ok"), output)
                self.assertEqual(completed.returncode, 1 if index == 2 else 0, (index, output, completed.stderr))
                self.assertFalse(session.policy.revoked)
            self.assertEqual(session.policy.phase, "settled")
        self.assertEqual(len(self.mutations()), 5)

    def test_cursor_editing_and_dry_run_bridges_are_rejected_before_launch(self):
        settings = (Path("/usr/bin/true"), self.root)
        ticket = {"read_only": True}
        for slot, dry_run, read_only in (("worker-b", False, True), ("worker-a", True, True), ("worker-a", False, False)):
            ticket["read_only"] = read_only
            with self.subTest(slot=slot, dry_run=dry_run), self.assertRaises(ValueError):
                roles.launch(ticket, slot, dry_run=dry_run, bridge_settings=settings)

    @unittest.skipUnless(CLI.is_file() and shutil.which("bwrap"), "installed Orca and bubblewrap required")
    def test_delayed_arm_and_ask_do_not_exhaust_stock_cli_inactivity_budget(self):
        session = self.wire_session()
        self.runtime.before_mutation = lambda: time.sleep(3)
        with session:
            def delayed_arm():
                time.sleep(3.5)
                bridge.arm(session.identifier, AUTHORITY)
            thread = threading.Thread(target=delayed_arm)
            thread.start()
            command = bridge.wire.sandbox(CLI, session.public, session.upstream, self.repo,
                ["orchestration", "ask", "--from", HANDLE, "--dispatch-capability", CAP,
                 "--question", "Proceed?", "--timeout-ms", "1000", "--json"])
            started = time.monotonic()
            try:
                completed = bridge.wire.run_cli(command, 12)
            finally:
                thread.join()
            self.assertGreater(time.monotonic() - started, 6)
            output = bridge.wire.decode(completed.stdout)
            self.assertTrue(output.get("ok"), output)
            self.assertTrue(output["result"]["timedOut"])
            self.assertEqual(completed.returncode, 1)
            self.assertFalse(session.policy.revoked)
            self.assertEqual(session.policy.pending_question, "msg_question")
        self.assertEqual(session.policy.phase, "unknown")

    @unittest.skipUnless(CLI.is_file() and shutil.which("bwrap"), "installed Orca and bubblewrap required")
    def test_keepalive_cannot_extend_absolute_host_deadline(self):
        session = self.wire_session()
        self.runtime.before_mutation = lambda: time.sleep(0.4)
        with session, patch.object(bridge, "REQUEST_SECONDS", 0.2), patch.object(bridge, "KEEPALIVE_SECONDS", 0.05):
            bridge.arm(session.identifier, AUTHORITY)
            command = bridge.wire.sandbox(CLI, session.public, session.upstream, self.repo,
                ["orchestration", "ask", "--from", HANDLE, "--dispatch-capability", CAP,
                 "--question", "Proceed?", "--timeout-ms", "1000", "--json"])
            started = time.monotonic()
            completed = bridge.wire.run_cli(command, 4)
            self.assertLess(time.monotonic() - started, 2)
            self.assertFalse(bridge.wire.decode(completed.stdout).get("ok"))
            self.assertNotEqual(completed.returncode, 0)
            self.assertTrue(session.policy.revoked)
        self.assertEqual(session.policy.phase, "unknown")


if __name__ == "__main__":
    unittest.main()
