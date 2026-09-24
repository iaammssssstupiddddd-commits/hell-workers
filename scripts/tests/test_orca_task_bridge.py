"""Installed 1.4.205 wire fixtures; no real Task, provider or credentials used."""

from __future__ import annotations

import copy
import json
import os
import shutil
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import uuid
from pathlib import Path
from unittest.mock import Mock, patch

from scripts import (host_coordination, orca_cursor_bridge_hook as cursor_hook,
                     orca_roles as roles, orca_task_bridge as bridge)
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
        endpoint_incarnation = self.row["worktreeId"] + "@@fixture:" + INCARNATION
        self.latest = {"id": AUTHORITY["dispatch"], "task_id": AUTHORITY["task"], "status": "dispatched"}
        self.observed = {
            "dispatch": {"id": AUTHORITY["dispatch"], "taskId": AUTHORITY["task"], "runId": AUTHORITY["run"],
                         "assigneeHandle": HANDLE, "processIncarnation": endpoint_incarnation,
                         "status": "dispatched", "capabilityRevokedAt": None},
            "worker": {"dispatchId": AUTHORITY["dispatch"], "runtimeEpoch": RUNTIME,
                       "agentTerminalHandle": HANDLE, "worktreeId": self.row["worktreeId"], "state": "ready"},
            "observation": {"exactWorker": True, "status": "live"},
            "terminal": copy.deepcopy(self.row),
            "terminalResource": {"terminalHandle": HANDLE, "worktreeId": self.row["worktreeId"],
                                 "ownerDispatchId": AUTHORITY["dispatch"],
                                 "endpointIncarnation": endpoint_incarnation},
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
                result["message"].setdefault("body", "")
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
        for module in (bridge, host_coordination, roles, roles.bindings):
            patcher = patch.object(module, "state_root", return_value=self.repo / "state/coordination")
            patcher.start()
            self.addCleanup(patcher.stop)
        patcher = patch.object(bridge, "root", return_value=self.root)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.policy = self.new_policy()

    def new_policy(self, *, cursor_hooks=False):
        directory = self.root / str(uuid.uuid4())
        directory.mkdir(mode=0o700)
        bridge.write_ledger(directory / "arm.json", AUTHORITY)
        return bridge.TaskPolicy(self.runtime, self.binding, directory, self.subject,
                                 cursor_hooks=cursor_hooks)

    @staticmethod
    def cursor_preamble(capability=CAP, *, task=AUTHORITY["task"], dispatch=AUTHORITY["dispatch"]):
        return f"""You are working inside Orca, a multi-agent IDE. You are a dispatched worker.
Your coordinator's terminal handle is: {COORDINATOR}
Your task ID is: {task}

=== CLI COMMANDS ===
  orca orchestration send --from {HANDLE} --dispatch-capability {capability} --type worker_done --subject "<short status>" --body "<summary>" --task-id {task} --dispatch-id {dispatch} --outcome succeeded
  orca orchestration send --from {HANDLE} --dispatch-capability {capability} --type heartbeat --subject alive --task-id {task} --dispatch-id {dispatch} --phase reviewing

=== TASK ===
Read the two requested files without editing them.
"""

    def hook(self, policy, event, **values):
        generation = values.pop("generation_id", "cursor-generation")
        params = {"conversation_id": "cursor-conversation", "generation_id": generation,
                  "hook_event_name": event, "cursor_version": "fixture",
                  "workspace_roots": [str(self.repo)], "user_email": None,
                  "transcript_path": None, **values}
        return {"id": str(uuid.uuid4()), "authToken": policy.cursor_hook_token,
                "method": bridge.CURSOR_HOOK_METHOD, "params": params}

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

    def test_review_format_retry_happens_before_settlement_and_accepts_same_dispatch(self):
        subject = {"ticket": "review-fixture", "base": "a" * 40, "head": "b" * 40,
                   "source_sha256": "c" * 64, "validation_evidence": "d" * 64}
        self.policy.review_subject = subject
        request = self.done()
        request["params"]["body"] = 'ORCA_REVIEW_JSON:{"acceptance":"normalize_label(" AbC ")"}'
        result = self.policy.handle(request)
        self.assertEqual(result["error"]["code"], "review_format_retry")
        self.assertEqual(self.policy.phase, "active")
        self.assertFalse(self.policy.revoked)
        self.assertEqual(self.mutations(), [])
        self.assertEqual(self.policy.operations, {})
        request = self.done()
        request["params"]["body"] = "ORCA_REVIEW_JSON:" + json.dumps({**subject,
            "verdict": "changes_requested", "blocking_findings": [{"id": "lowercase",
            "message": "case is unchanged", "acceptance": 'normalize_label(" AbC ") == "abc"'}]})
        self.assertTrue(self.policy.handle(request)["ok"])
        self.assertEqual(self.policy.phase, "settled")
        self.assertEqual(len(self.mutations()), 1)

    def test_review_retries_are_bounded_and_wrong_subject_is_not_retryable(self):
        self.policy.review_subject = {"ticket": "correct"}
        for _ in range(2):
            self.assertEqual(self.policy.handle(self.done())["error"]["code"], "review_format_retry")
        self.assertEqual(self.policy.handle(self.done())["error"]["code"], "task_bridge_refused")
        self.assertEqual(self.mutations(), [])
        self.policy = self.new_policy()
        self.policy.review_subject = {"ticket": "correct"}
        request = self.done()
        request["params"]["body"] = "ORCA_REVIEW_JSON:" + json.dumps({"ticket": "wrong",
            "base": "a", "head": "b", "source_sha256": "c", "validation_evidence": "d",
            "verdict": "approved", "blocking_findings": []})
        self.assertEqual(self.policy.handle(request)["error"]["code"], "task_bridge_refused")
        self.assertEqual(self.mutations(), [])

    def test_cursor_denies_arbitrary_prompt_without_revoking_bootstrap(self):
        policy = self.new_policy(cursor_hooks=True)
        reply = policy.handle_cursor_hook(self.hook(policy, "beforeSubmitPrompt", prompt="Edit result.py now"))
        self.assertEqual(cursor_hook.hook_output({"hook_event_name": "beforeSubmitPrompt"}, reply),
                         {"continue": False})
        self.assertEqual(policy.phase, "bootstrap")
        self.assertIsNone(policy.cursor_authority)
        self.assertEqual(self.mutations(), [])

    def test_cursor_dispatch_prompt_requires_host_arm_before_admission(self):
        policy = self.new_policy(cursor_hooks=True)
        (policy.directory / "arm.json").unlink()
        with patch.object(bridge, "ARM_SECONDS", 0):
            reply = policy.handle_cursor_hook(self.hook(policy, "beforeSubmitPrompt", prompt=self.cursor_preamble()))
        self.assertFalse(reply["ok"])
        self.assertEqual(policy.phase, "unknown")
        self.assertEqual(self.mutations(), [])

    def test_standard_completion_files_and_report_metadata_are_accepted(self):
        request = self.done()
        payload = json.loads(request["params"]["payload"])
        payload.update(filesModified=["scripts/tests/test_storage.py"], reportPath="report.md")
        request["params"]["payload"] = json.dumps(payload)
        result = self.policy.handle(request)
        self.assertTrue(result["ok"], result)
        self.assertEqual(self.policy.phase, "settled")

    def test_completion_file_metadata_does_not_grant_outside_paths(self):
        request = self.done()
        payload = json.loads(request["params"]["payload"])
        payload["filesModified"] = ["../outside"]
        request["params"]["payload"] = json.dumps(payload)
        self.assertFalse(self.policy.handle(request)["ok"])
        self.assertEqual(self.mutations(), [])

    def test_cursor_hook_settles_one_read_only_result_without_exposing_capability(self):
        policy = self.new_policy(cursor_hooks=True)
        observed = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt=self.cursor_preamble(), attachments=[]))
        self.assertTrue(observed["ok"], observed)
        response = json.dumps({
            "outcome": "succeeded", "subject": "Cursor B inspection complete",
            "body": "The requested source was inspected read-only. The two entry points use the guarded bridge. No work remains for this leaf task.",
        })
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", text=response))["ok"])
        settled = policy.handle_cursor_hook(self.hook(
            policy, "stop", status="completed", loop_count=0))
        self.assertTrue(settled["ok"], settled)
        self.assertEqual(settled["result"]["outcome"], "succeeded")
        self.assertEqual([call[1].get("type") for call in self.mutations()],
                         ["heartbeat", None, "worker_done"])
        journal = (policy.directory / "journal.json").read_text()
        self.assertNotIn(CAP, journal)
        self.assertNotIn(policy.cursor_hook_token, journal)
        self.assertEqual(json.loads(journal)["settled_status"], "completed")

    def test_cursor_stop_waits_for_concurrent_final_response_before_settlement(self):
        self.assertTrue(issubclass(bridge.Proxy, socketserver.ThreadingMixIn))
        policy = self.new_policy(cursor_hooks=True)
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt=self.cursor_preamble(), attachments=[]))["ok"])
        final = json.dumps({
            "outcome": "succeeded", "subject": "Cursor B inspection complete",
            "body": "The requested source was inspected read-only. The guarded hooks were confirmed. No work remains.",
        })
        stopped = []
        thread = threading.Thread(target=lambda: stopped.append(policy.handle_cursor_hook(
            self.hook(policy, "stop", status="completed", loop_count=0),
            deadline=time.monotonic() + 2)))
        thread.start()
        time.sleep(0.05)
        response = policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", text=final), deadline=time.monotonic() + 2)
        thread.join(timeout=2)
        self.assertFalse(thread.is_alive())
        self.assertTrue(response["ok"], response)
        self.assertTrue(stopped[0]["ok"], stopped[0])
        self.assertEqual(stopped[0]["result"]["outcome"], "succeeded")
        self.assertEqual(policy.phase, "settled")
        self.assertEqual(policy.cursor_stage, "settled")

    def test_cursor_hook_ignores_stale_bootstrap_completion_after_dispatch_arrives(self):
        policy = self.new_policy(cursor_hooks=True)
        bootstrap = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", generation_id="bootstrap-generation",
            prompt="Wait for the supervised task.", attachments=[]))
        self.assertEqual(bootstrap["result"], {"observed": False, "continue": False})
        dispatched = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", generation_id="dispatch-generation",
            prompt=self.cursor_preamble(), attachments=[]))
        self.assertEqual(dispatched["result"], {"observed": True, "continue": True})
        stale_response = policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", generation_id="bootstrap-generation",
            text='{"outcome":"failed","subject":"Stale","body":"Bootstrap turn only."}'))
        stale_stop = policy.handle_cursor_hook(self.hook(
            policy, "stop", generation_id="bootstrap-generation", status="completed", loop_count=0))
        self.assertEqual(stale_response["result"], {"observed": False})
        self.assertEqual(stale_stop["result"], {"observed": False})
        final = json.dumps({"outcome": "succeeded", "subject": "Current Dispatch",
                            "body": "Read only. The requested entry points were inspected. Nothing remains."})
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", generation_id="dispatch-generation", text=final))["ok"])
        settled = policy.handle_cursor_hook(self.hook(
            policy, "stop", generation_id="dispatch-generation", status="completed", loop_count=0))
        self.assertTrue(settled["ok"], settled)
        self.assertEqual(settled["result"]["outcome"], "succeeded")

    def test_cursor_hook_retries_non_json_result_once_without_lifecycle_mutation(self):
        policy = self.new_policy(cursor_hooks=True)
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", generation_id="dispatch-generation",
            prompt=self.cursor_preamble(), attachments=[]))["ok"])
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", generation_id="dispatch-generation",
            text="Completed the requested inspection."))["ok"])
        retry = policy.handle_cursor_hook(self.hook(
            policy, "stop", generation_id="dispatch-generation", status="completed", loop_count=0))
        self.assertTrue(retry["ok"], retry)
        self.assertEqual(retry["result"], {"followup_message": bridge.CURSOR_RESULT_FOLLOWUP})
        self.assertEqual(self.mutations(), [])
        self.assertEqual(policy.phase, "active")
        self.assertEqual(cursor_hook.hook_output(
            {"hook_event_name": "stop"}, retry),
            {"followup_message": cursor_hook.CURSOR_RESULT_FOLLOWUP})
        followup = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", generation_id="retry-generation",
            prompt=bridge.CURSOR_RESULT_FOLLOWUP, attachments=[]))
        self.assertEqual(followup["result"], {"observed": True, "continue": True})
        final = json.dumps({"outcome": "succeeded", "subject": "Current Dispatch",
                            "body": "Read only. The requested entry points were inspected. Nothing remains."})
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", generation_id="retry-generation", text=final))["ok"])
        settled = policy.handle_cursor_hook(self.hook(
            policy, "stop", generation_id="retry-generation", status="completed", loop_count=1))
        self.assertTrue(settled["ok"], settled)
        self.assertEqual(settled["result"]["outcome"], "succeeded")
        self.assertEqual([call[1].get("type") for call in self.mutations()],
                         ["heartbeat", None, "worker_done"])

    def test_cursor_initial_prompt_marks_hook_ready_without_admission(self):
        policy = self.new_policy(cursor_hooks=True)
        result = policy.handle_cursor_hook(self.hook(policy, 'beforeSubmitPrompt',
            prompt="BOOTSTRAP ONLY: Wait for the host's live supervised Dispatch. Do not edit.", attachments=[]))
        self.assertEqual(result['result'], {'observed': False, 'continue': False})
        self.assertEqual(policy.cursor_stage, 'bootstrap_rejected')
        self.assertIsNone(policy.authority)
        self.assertEqual(self.mutations(), [])

    def test_cursor_automatic_format_followup_without_before_submit(self):
        policy = self.new_policy(cursor_hooks=True)
        policy.handle_cursor_hook(self.hook(policy, "beforeSubmitPrompt",
            generation_id="initial", prompt=self.cursor_preamble(), attachments=[]))
        policy.handle_cursor_hook(self.hook(policy, "afterAgentResponse",
            generation_id="initial", text="Non-JSON result"))
        retry = policy.handle_cursor_hook(self.hook(policy, "stop",
            generation_id="initial", status="completed", loop_count=0))
        self.assertEqual(retry["result"]["followup_message"], bridge.CURSOR_RESULT_FOLLOWUP)
        final = json.dumps({"outcome": "succeeded", "subject": "Done",
                            "body": "Completed. Scope unchanged. No tests run."})
        response = policy.handle_cursor_hook(self.hook(policy, "afterAgentResponse",
            generation_id="automatic-retry", text=final))
        self.assertTrue(response["result"]["observed"])
        settled = policy.handle_cursor_hook(self.hook(policy, "stop",
            generation_id="automatic-retry", status="completed", loop_count=1))
        self.assertEqual(settled["result"]["outcome"], "succeeded")

    def test_cursor_hook_ignores_bootstrap_turn_but_rejects_changed_or_pending_authority(self):
        policy = self.new_policy(cursor_hooks=True)
        bootstrap = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt="Wait for the supervised task.", attachments=[]))
        self.assertEqual(bootstrap["result"], {"observed": False, "continue": False})
        ignored = policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", text="Waiting."))
        self.assertEqual(ignored["result"], {"observed": False})
        bad = policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt=self.cursor_preamble(dispatch="dispatch_other"), attachments=[]))
        self.assertFalse(bad["ok"])
        policy.cursor_response = json.dumps({"outcome": "succeeded", "subject": "Done",
                                             "body": "Read only. Found the entry. Nothing remains."})
        refused = policy.handle_cursor_hook(self.hook(
            policy, "stop", status="completed", loop_count=0))
        self.assertFalse(refused["ok"])
        self.assertEqual(policy.phase, "unknown")

        policy = self.new_policy(cursor_hooks=True)
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt=self.cursor_preamble(), attachments=[]))["ok"])
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", text='{"outcome":"succeeded","subject":"Done","body":"Complete","extra":true}'))["ok"])
        retry = policy.handle_cursor_hook(self.hook(
            policy, "stop", status="completed", loop_count=0))
        self.assertTrue(retry["ok"], retry)
        self.assertEqual(retry["result"], {"followup_message": bridge.CURSOR_RESULT_FOLLOWUP})
        self.assertEqual(policy.cursor_stage, "result_retry")

        policy = self.new_policy(cursor_hooks=True)
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "beforeSubmitPrompt", prompt=self.cursor_preamble(), attachments=[]))["ok"])
        self.assertTrue(policy.handle_cursor_hook(self.hook(
            policy, "afterAgentResponse", text=json.dumps({
                "outcome": "succeeded", "subject": "Done",
                "body": "Read only. Found the entry. Nothing remains.",
            })))["ok"])
        self.runtime.check_result.update(deliveryId="delivery_pending", count=1, messages=[{
            "id": "msg_pending", "run_id": AUTHORITY["run"], "from_handle": COORDINATOR,
            "to_handle": "dispatch:" + AUTHORITY["dispatch"], "type": "status",
            "subject": "Please inspect one more file",
        }])
        refused = policy.handle_cursor_hook(self.hook(
            policy, "stop", status="completed", loop_count=0))
        self.assertFalse(refused["ok"])
        self.assertEqual(policy.phase, "unknown")

    def test_pending_journal_precedes_exact_capability_forward_and_no_secrets_escape(self):
        def inspect():
            journal = json.loads((self.policy.directory / "journal.json").read_text())
            self.assertEqual([r["phase"] for r in journal["operations"].values()], ["pending"])
        self.runtime.before_mutation = inspect
        reply = self.policy.handle(self.request())
        self.assertTrue(reply["ok"], reply)
        self.assertNotIn("body", reply["result"]["message"])
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
        for section, field in (("dispatch", "processIncarnation"),
                               ("terminal", "incarnationId"),
                               ("terminalResource", "endpointIncarnation"),
                               ("terminalResource", "ownerDispatchId")):
            with self.subTest(section=section, field=field):
                self.runtime = Runtime(self.repo)
                self.runtime.observed[section][field] = "other"
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

    def wire_session(self, *, cursor_hooks=False):
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
        return bridge.Session(executable, config, HANDLE, self.repo, self.subject,
                              cursor_hooks=cursor_hooks)

    def test_session_arm_is_host_only_exact_once_and_teardown_removes_transport(self):
        session = self.wire_session()
        with session:
            bridge.arm(session.identifier, AUTHORITY)
            with self.assertRaises(bridge.wire.Refused):
                bridge.arm(session.identifier, AUTHORITY)
            self.assertEqual(set(p.name for p in session.public.iterdir()), {"orca-runtime.json", "rpc.sock", "orca"})
            self.assertEqual(session.policy.phase, "bootstrap")
        self.assertEqual(list(session.public.iterdir()), [])
        self.assertEqual(session.policy.phase, "unknown")
        self.assertLessEqual(len(os.fsencode(session.public / "rpc.sock")), 107)

    def test_arm_uses_live_incarnation_lease_not_host_pid_visibility(self):
        session = self.wire_session()
        with session, patch.object(bridge.os, "kill", side_effect=ProcessLookupError):
            bridge.arm(session.identifier, AUTHORITY)
            self.assertTrue((session.directory / "arm.json").is_file())

    def test_arm_refuses_an_exited_launcher_lease(self):
        session = self.wire_session()
        with session:
            session.terminal_lease.close()
            with self.assertRaisesRegex(bridge.wire.Refused, "lease is not live"):
                bridge.arm(session.identifier, AUTHORITY)

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

    def test_failed_dispatch_reconciliation_preserves_fixed_reviewer_without_approval(self):
        bridge_id = str(uuid.uuid4())
        directory = self.root / bridge_id
        directory.mkdir(mode=0o700)
        bridge.write_ledger(directory / "journal.json", {
            "schema": 1, "phase": "unknown", "revoked": True, "authority": AUTHORITY,
            "capability_sha256": None, "operations": {}, "settled_status": None,
        })
        bridge.write_ledger(directory / "identity.json", {
            "runtime": RUNTIME, "terminal": HANDLE, "incarnation": INCARNATION,
            "repo": str(self.repo), "pid": 999_999_999,
        })
        bridge.write_ledger(directory / "arm.json", AUTHORITY)
        self.runtime.latest.update(id="dispatch_retry", status="failed",
                                   retry_of_dispatch_id=AUTHORITY["dispatch"])
        self.runtime.observed["dispatch"].update(status="failed", capabilityRevokedAt=1)
        self.runtime.observed["worker"].update(state="abandoned", stage="abandoned")
        session_id = str(uuid.uuid4())
        previous = {"key": "fixed-reviewer", "ticket_sha256": "1" * 64,
                    "subject": {"repo": str(self.repo), "common": str(self.repo / ".git"),
                                "branch": "task", "base": "1" * 40},
                    "origin": str(self.repo), "source_sha256": "2" * 64,
                    "session_id": session_id, "session_sha256": "3" * 64}
        attempt_id = str(uuid.uuid4())
        observed_source = "4" * 64
        data = {"schema": 1, "slot": "reviewer", "provider": "codex",
                "tasks": {"fixed-reviewer": previous},
                "last": {"attempt_id": attempt_id, "key": "fixed-reviewer", "phase": "unknown",
                         "process_exited": True, "exit_code": 0, "source_before": observed_source,
                         "terminal": HANDLE, "orca_bridge": bridge_id}}
        ticket = {"id": "review", "repo": str(self.repo), "branch": "task", "base": "1" * 40,
                  "read_only": True, "allowed_directories": [], "prompt": "review"}
        snapshot = {"session_id": session_id, "session_sha256": "5" * 64}
        with patch.object(roles, "acquire_host"), patch.object(roles, "validate_ticket"), \
                patch.object(roles, "git", return_value=str(self.repo / ".git")), \
                patch.object(roles, "fingerprint", return_value="6" * 64), \
                patch.object(roles, "prepare_runtime", return_value=self.repo / "runtime"), \
                patch.object(roles.bindings, "read_state", return_value=data), \
                patch.object(roles.bindings, "session_snapshot", return_value=snapshot), \
                patch.object(roles.bindings, "save_state") as save, \
                patch.object(bridge.Upstream, "load", return_value=self.runtime):
            roles.reconcile_bridge(ticket, "reviewer", attempt_id, observed_source,
                                   "CLI could not reach the private bridge", self.root)
        save.assert_called_once_with(data)
        self.assertEqual(previous["session_sha256"], snapshot["session_sha256"])
        self.assertEqual(data["last"]["phase"], "recorded")
        self.assertEqual(data["last"]["bridge_reconciliation"]["dispatch"], AUTHORITY["dispatch"])
        self.assertEqual(previous["ticket_sha256"], "1" * 64)
        self.assertNotIn("approved", json.dumps(data))

    def test_interrupted_cursor_launcher_reconciles_ambiguous_send_after_dispatch_is_fenced(self):
        bridge_id = str(uuid.uuid4())
        operation = str(uuid.uuid4())
        directory = self.root / bridge_id
        directory.mkdir(mode=0o700)
        bridge.write_ledger(directory / "journal.json", {
            "schema": 1, "phase": "unknown", "revoked": True, "authority": AUTHORITY,
            "capability_sha256": "1" * 64,
            "operations": {operation: {"signature": "2" * 64, "phase": "pending"}},
            "settled_status": None,
        })
        bridge.write_ledger(directory / "identity.json", {
            "runtime": RUNTIME, "terminal": HANDLE, "incarnation": INCARNATION,
            "repo": str(self.repo), "pid": 999_999_999,
        })
        bridge.write_ledger(directory / "arm.json", AUTHORITY)
        self.runtime.latest.update(status="failed")
        self.runtime.observed["dispatch"].update(status="failed", capabilityRevokedAt=1)
        self.runtime.observed["worker"].update(state="abandoned", stage="abandoned")
        self.runtime.observed["observation"]["status"] = "exited"
        attempt_id = str(uuid.uuid4())
        observed_source = "4" * 64
        ticket = {"id": "read-only-worker", "repo": str(self.repo), "branch": "task",
                  "base": "1" * 40, "read_only": True, "allowed_directories": [],
                  "prompt": "inspect", "provider": "cursor", "complexity": "simple",
                  "task_kind": "acceptance-probe", "complexity_reason": "read-only probe",
                  "acceptance": "Report the inspected source without edits."}
        task_key = roles.bindings.digest({"common": str(self.repo / ".git"), "id": ticket["id"]})
        data = {"schema": 1, "slot": "worker-b", "provider": "cursor", "tasks": {},
                "last": {"attempt_id": attempt_id, "key": task_key, "phase": "starting",
                         "process_exited": False, "exit_code": None,
                         "source_before": observed_source, "terminal": HANDLE,
                         "orca_bridge": bridge_id}}
        with patch.object(roles, "acquire_host"), patch.object(roles, "validate_ticket"), \
                patch.object(roles, "git", return_value=str(self.repo / ".git")), \
                patch.object(roles, "fingerprint", return_value=observed_source), \
                patch.object(roles, "prepare_runtime", return_value=self.repo / "runtime"), \
                patch.object(roles.bindings, "read_state", return_value=data), \
                patch.object(roles.bindings, "history_exists", return_value=False), \
                patch.object(roles.bindings, "session_snapshot") as snapshot, \
                patch.object(roles.bindings, "save_state") as save, \
                patch.object(bridge.Upstream, "load", return_value=self.runtime):
            roles.reconcile_bridge(ticket, "worker-b", attempt_id, observed_source,
                                   "Heartbeat landed but its response was rejected", self.root)
        snapshot.assert_not_called()
        save.assert_called_once_with(data)
        last = data["last"]
        self.assertEqual(last["phase"], "abandoned")
        self.assertTrue(last["process_exited"])
        self.assertIsNone(last["exit_code"])
        evidence = last["bridge_reconciliation"]
        self.assertEqual(evidence["exit_observation"], "external_terminal_exited")
        self.assertEqual(evidence["ambiguous_operations"], [operation])
        self.assertIs(evidence["session_absent"], True)
        self.assertNotIn("session_id", evidence)
        self.assertTrue(roles.bindings.valid_bridge_reconciliation(evidence))
        self.assertEqual(data["abandoned"][task_key], last)
        self.assertNotIn("approved", json.dumps(data))
        roles.bindings.save_state(data)
        self.assertEqual(roles.bindings.read_state("worker-b", "cursor"), data)

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
    def test_bridge_client_carries_private_transport_into_role_shell(self):
        session = self.wire_session()
        private = self.repo / "state"
        runtime = private / "agents/reviewer"
        for name in ("codex", "tmp"):
            (runtime / name).mkdir(parents=True, exist_ok=True)
        ticket = {"repo": str(self.repo), "read_only": True, "allowed_directories": []}
        with session, patch.object(roles, "git", return_value=str(self.repo / ".git")), \
                patch.dict(os.environ, {"CODEX_HOME": str(self.repo / "no-auth"),
                                        "ORCA_TERMINAL_HANDLE": HANDLE}):
            command = roles.sandbox_command(
                ticket, "reviewer", runtime, [str(session.client), "status", "--json"], bridge=session)
            completed = bridge.wire.run_cli(command, 8)
            output = bridge.wire.decode(completed.stdout)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertTrue(output["result"]["runtime"]["reachable"])
            self.assertFalse(session.policy.revoked)

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

    def test_dry_run_bridge_is_rejected_before_launch(self):
        settings = (Path("/usr/bin/true"), self.root)
        ticket = {"read_only": True}
        with self.assertRaisesRegex(ValueError, "real approved"):
            roles.launch(ticket, "worker-a", dry_run=True, bridge_settings=settings)

    @unittest.skipUnless(CLI.is_file() and shutil.which("bwrap"), "installed Orca and bubblewrap required")
    def test_codex_editing_worker_keeps_bridge_and_write_scope(self):
        session = self.wire_session()
        runtime = self.repo / "codex-edit-runtime"
        for name in ("codex", "tmp"):
            (runtime / name).mkdir(parents=True, exist_ok=True)
        allowed = self.repo / "src"
        blocked = self.repo / "docs"
        allowed.mkdir()
        blocked.mkdir()
        ticket = {"repo": str(self.repo), "read_only": False,
                  "allowed_directories": ["src"]}
        script = """import json, pathlib, subprocess, sys
allowed, blocked, client = sys.argv[1:]
pathlib.Path(allowed).write_text('allowed')
try:
    pathlib.Path(blocked).write_text('forbidden')
    raise AssertionError('write escaped assigned scope')
except OSError:
    pass
status = subprocess.run([client, 'status', '--json'], capture_output=True, text=True, check=True)
assert json.loads(status.stdout)['result']['runtime']['reachable'] is True
print('scoped')
"""
        with session, patch.object(roles, "git", return_value=str(self.repo / ".git")), \
                patch.dict(os.environ, {"CODEX_HOME": str(self.repo / "no-auth"),
                                        "ORCA_TERMINAL_HANDLE": HANDLE}):
            command = roles.sandbox_command(
                ticket, "worker", runtime,
                [sys.executable, "-c", script, str(allowed / "change.txt"),
                 str(blocked / "escape.txt"), str(session.client)], bridge=session)
            completed = bridge.wire.run_cli(command, 8)
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(completed.stdout.strip(), b"scoped")
            self.assertEqual((allowed / "change.txt").read_text(), "allowed")
            self.assertFalse((blocked / "escape.txt").exists())

    @unittest.skipUnless(shutil.which("bwrap"), "bubblewrap required")
    def test_cursor_editing_worker_reaches_only_private_bridge_and_scope(self):
        session = self.wire_session(cursor_hooks=True)
        runtime = self.repo / "cursor-runtime"
        for child in ("cursor", "cursor-data", "xdg/cursor", "cache", "tmp", "codex"):
            (runtime / child).mkdir(parents=True, exist_ok=True)
        (self.repo / ".cursor").mkdir()
        script_dir = self.repo / "scripts"
        script_dir.mkdir()
        source = Path(__file__).resolve().parents[1] / "orca_cursor_bridge_hook.py"
        shutil.copyfile(source, script_dir / source.name)
        (self.repo / "src").mkdir()
        ticket = {"repo": str(self.repo), "read_only": False, "allowed_directories": ["src"]}
        with session, patch.object(roles, "git", return_value=str(self.repo / ".git")):
            policy_dir = session.public / "cursor-policy"
            policy_dir.mkdir(mode=0o700)
            denied_reads = (str(session.public), "/proc")
            policy = policy_dir / "cli.json"
            roles.write_cursor_policy(ticket, policy, project=True, denied_reads=denied_reads)
            roles.write_cursor_hooks(self.repo, policy_dir / "hooks.json")
            command = roles.sandbox_command(
                ticket, "worker", runtime, [sys.executable, str(script_dir / source.name)],
                provider="cursor", policy=policy, bridge=session)
            event = {"conversation_id": "fixture-conversation", "generation_id": "fixture-generation",
                     "hook_event_name": "beforeSubmitPrompt", "cursor_version": "fixture",
                     "workspace_roots": [str(self.repo)], "user_email": None,
                     "transcript_path": None, "prompt": "bootstrap", "attachments": [],
                     "future_controller_field": "must-not-cross-the-bridge"}
            captured = []
            original = session.policy.handle_cursor_hook

            def capture(request, **kwargs):
                captured.append(request)
                return original(request, **kwargs)

            with patch.object(session.policy, "handle_cursor_hook", side_effect=capture):
                completed = subprocess.run(command, input=json.dumps(event), text=True,
                                           capture_output=True, timeout=8, check=True)
            self.assertEqual(json.loads(completed.stdout), {"continue": False})
            self.assertNotIn("future_controller_field", captured[0]["params"])
            policy_text = policy.read_text()
            self.assertIn("Shell(*)", policy_text)
            self.assertIn("Mcp(*:*)", policy_text)
            self.assertIn("Write(src/**)", policy_text)
            self.assertNotIn("Write(**)", policy_text)
            self.assertNotIn(session.policy.cursor_hook_token, policy_text)

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
