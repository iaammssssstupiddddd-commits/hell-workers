from __future__ import annotations

import json
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import (host_coordination, orca_coordinator, orca_dispatch as dispatch,
                     orca_frontdesk, orca_issue_context, orca_role_state, orca_roles,
                     orca_ui_coordinator)


WORKSPACE = "eeac8301-ddb2-4c31-8a6c-e2a7f2fc7efb"
ISSUE = "884ddcd2-cef6-4869-a88d-14512684cce7"
REQUEST = str(uuid.uuid5(
    orca_issue_context.REQUEST_NAMESPACE, f"{WORKSPACE}\n{ISSUE}\n{'a' * 64}"
))
COORDINATOR = "term_175c1be5-9f01-4a44-8268-a0542fa4e781"
TERMINAL = "term_9064b7ad-64e3-40db-b6c9-cd74d6fc21ab"
BRIDGE = "319674b0-1f30-4257-81dd-70f24553f24b"


class OrcaDispatchTests(unittest.TestCase):
    def setUp(self) -> None:
        # Dispatch protocol fixtures isolate the separately tested tab allocator.
        def launch(call, cli, request, repo, slot, command, **kwargs):
            return call(cli, ["terminal", "create", "--worktree", f"path:{repo}",
                             "--title", dispatch.role_tabs.TITLES[slot] + " | edit-leaf",
                             "--command", command], "terminal-create")["terminal"]
        for name, options in (("launch", {"side_effect": launch}), ("label", {})):
            patcher = patch.object(dispatch.role_tabs, name, **options)
            patcher.start()
            self.addCleanup(patcher.stop)
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-dispatch-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.primary = self.root / "primary"
        self.repo = self.root / "candidate"
        self.primary.mkdir()
        self.run_git(self.primary, "init", "-q")
        (self.primary / "src").mkdir()
        (self.primary / "src/content.txt").write_text("original")
        (self.primary / "scripts").mkdir()
        (self.primary / "scripts/orca_roles.py").write_text("# fixture")
        self.run_git(self.primary, "add", ".")
        self.run_git(self.primary, "-c", "user.name=Fixture", "-c",
                     "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
                     "commit", "-qm", "fixture")
        self.run_git(self.primary, "worktree", "add", "-qb", "task", str(self.repo))
        self.ticket = {
            "schema": 1, "id": "edit-leaf", "repo": str(self.repo), "branch": "task",
            "base": orca_roles.git(self.repo, "rev-parse", "HEAD"),
            "allowed_directories": ["src"], "prompt": "Update the bounded leaf.",
        }
        self.ticket_path = self.root / "ticket.json"
        self.ticket_path.write_text(json.dumps(self.ticket))
        self.cli = self.root / "orca"
        self.cli.write_text("#!/bin/sh\nexit 1\n")
        self.cli.chmod(0o700)
        self.metadata = self.root / "metadata"
        self.metadata.mkdir()
        for module in (host_coordination, orca_frontdesk, orca_issue_context,
                       orca_role_state, orca_roles, orca_ui_coordinator, dispatch):
            mock = patch.object(module, "state_root", return_value=self.root / "coordination")
            mock.start()
            self.addCleanup(mock.stop)
        orca_frontdesk.submit("Linear snapshot", REQUEST)
        orca_frontdesk.write_ledger(orca_issue_context.ledger_path(), {
            "schema": 1,
            "imports": [{
                "request_id": REQUEST, "workspace_id": WORKSPACE, "issue_id": ISSUE,
                "identifier": "HW-42", "snapshot_sha256": "a" * 64,
                "created_at": "2026-09-21T00:00:00+00:00",
            }],
        })
        orca_coordinator.save_state({
            "schema": 1, "request_id": REQUEST, "repo": str(orca_coordinator.REPO),
            "session_id": "e1fd2684-d55a-4794-9741-903c92b7dbea",
            "turns": [{
                "id": "e230e066-b2af-4888-9032-9264802af0c5", "message": "plan",
                "phase": "succeeded", "exit_code": 0, "completed": True,
                "thread_seen": True, "session_id": "e1fd2684-d55a-4794-9741-903c92b7dbea",
                "response": "approved split",
            }],
        })

    @staticmethod
    def run_git(repo: Path, *args: str) -> None:
        import subprocess
        subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)

    def test_cursor_bootstrap_requires_new_controller_hook_not_old_idle_text(self):
        with patch.object(dispatch.frontdesk, 'read_private_json', return_value={
                'phase': 'bootstrap', 'authority': None, 'cursor_stage': 'initial'}):
            with self.assertRaisesRegex(dispatch.DispatchError, 'initial prompt hook'):
                dispatch.wait_for_cursor_bootstrap(BRIDGE, timeout=0)
        with patch.object(dispatch.frontdesk, 'read_private_json', return_value={
                'phase': 'bootstrap', 'authority': None, 'cursor_stage': 'bootstrap_rejected'}):
            dispatch.wait_for_cursor_bootstrap(BRIDGE, timeout=0)
        with patch.object(dispatch.frontdesk, 'read_private_json', return_value={
                'phase': 'active', 'authority': {'dispatch': 'ctx_other'}, 'cursor_stage': 'bootstrap_rejected'}):
            with self.assertRaisesRegex(dispatch.DispatchError, 'authority'):
                dispatch.wait_for_cursor_bootstrap(BRIDGE, timeout=0)

    def test_unknown_role_is_rejected_before_external_mutations(self) -> None:
        orca_role_state.save_state({"schema": 1, "slot": "worker-a", "provider": "codex",
                                   "tasks": {}, "last": {"phase": "unknown"}})
        with patch.object(dispatch, "run_cli") as cli:
            with self.assertRaisesRegex(ValueError, "unknown role attempt"):
                dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata)
        cli.assert_not_called()
        self.assertFalse(dispatch.dispatch_path(REQUEST, self.ticket).exists())

    def test_start_creates_run_controlled_terminal_dispatch_then_arms(self) -> None:
        receipts = [
            {"run": {"id": "run_fixture"}},
            {"terminal": {"handle": TERMINAL}},
            {"wait": {"satisfied": True}},
            {"runId": "run_fixture", "taskId": "task_fixture",
             "dispatchId": "ctx_fixture", "state": "ready", "stage": "input_accepted"},
        ]
        with patch.object(dispatch, "run_cli", side_effect=receipts) as cli, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), \
                patch.object(dispatch.task_bridge, "arm") as arm:
            result = dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                                    orca_cli=self.cli, metadata=self.metadata)

        self.assertEqual(result["phase"], "armed")
        self.assertEqual(result["task_id"], "task_fixture")
        self.assertEqual(result["dispatch_id"], "ctx_fixture")
        arm.assert_called_once_with(BRIDGE, {
            "run": "run_fixture", "task": "task_fixture", "dispatch": "ctx_fixture",
            "coordinator": COORDINATOR,
        })
        commands = [entry.args[1] for entry in cli.call_args_list]
        self.assertEqual(commands[0][:2], ["orchestration", "run-create"])
        self.assertEqual(commands[1][:2], ["terminal", "create"])
        self.assertEqual(
            commands[1][commands[1].index("--title") + 1],
            "実装A（Codex） | edit-leaf",
        )
        self.assertIn("--bridge-orca", commands[1][commands[1].index("--command") + 1])
        self.assertEqual(commands[2][:2], ["terminal", "wait"])
        self.assertEqual(commands[3][:2], ["orchestration", "worker-start"])
        self.assertIn("--terminal", commands[3])
        self.assertIn(TERMINAL, commands[3])

        with patch.object(dispatch, "run_cli") as repeated:
            self.assertEqual(
                dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata),
                result,
            )
        repeated.assert_not_called()

    def test_unknown_mutation_is_persisted_and_never_retried(self) -> None:
        with patch.object(dispatch, "run_cli", side_effect=dispatch.DispatchError("unknown")):
            with self.assertRaisesRegex(dispatch.DispatchError, "unknown"):
                dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata)
        state = orca_frontdesk.read_private_json(dispatch.dispatch_path(REQUEST, self.ticket), {})
        self.assertEqual(state["phase"], "unknown")
        with self.assertRaisesRegex(dispatch.DispatchError, "previous dispatch"):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata)

    def test_busy_bootstrap_never_receives_task_input(self):
        receipts = [{"run": {"id": "run_fixture"}}, {"terminal": {"handle": TERMINAL}},
                    {"wait": {"satisfied": False}}]
        with patch.object(dispatch, "run_cli", side_effect=receipts) as cli, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), \
                patch.object(dispatch.task_bridge, "arm") as arm:
            with self.assertRaisesRegex(dispatch.DispatchError, "not idle"):
                dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata)
        self.assertEqual(len(cli.call_args_list), 3)
        arm.assert_not_called()
        record = orca_frontdesk.read_private_json(dispatch.dispatch_path(REQUEST, self.ticket), {})
        self.assertEqual(record["phase"], "unknown")
        self.assertIsNone(record.get("task_id"))

    def test_shared_run_does_not_create_or_rebind_a_run(self):
        run = {"id": "run_fixture", "consumer_generation": 1}
        receipts = [{"run": {**run, "coordinator_handle": COORDINATOR, "legacy": 0}},
                    {"terminal": {"handle": TERMINAL}},
                    {"wait": {"satisfied": True}},
                    {"runId": "run_fixture", "taskId": "task_fixture", "dispatchId": "ctx_fixture",
                     "state": "ready", "stage": "input_accepted"}]
        with patch.object(dispatch, "run_cli", side_effect=receipts) as cli, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), patch.object(dispatch.task_bridge, "arm"):
            result = dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                                    orca_cli=self.cli, metadata=self.metadata, run_context=run)
        self.assertEqual(result["shared_run"], run)
        self.assertEqual([call.args[2] for call in cli.call_args_list],
                         ["run-current", "terminal-create", "terminal-idle", "worker-start"])
        with patch.object(dispatch, "run_cli", return_value={"run": {**run, "coordinator_handle": "term_other", "legacy": 0}}):
            with self.assertRaisesRegex(dispatch.DispatchError, "consumer changed"):
                dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                               orca_cli=self.cli, metadata=self.metadata, run_context=run)

    def test_generation_resume_is_explicit_and_followup_replaces_original_prompt(self) -> None:
        self.ticket["generation"] = 1
        self.ticket_path.write_text(json.dumps(self.ticket))
        session = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        receipts = [
            {"run": {"id": "run_fixture"}},
            {"terminal": {"handle": TERMINAL}},
            {"wait": {"satisfied": True}},
            {"runId": "run_fixture", "taskId": "task_fixture",
             "dispatchId": "ctx_fixture", "state": "ready", "stage": "input_accepted"},
        ]
        with patch.object(dispatch.bindings, "admit") as admit, \
                patch.object(dispatch, "run_cli", side_effect=receipts) as cli, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), \
                patch.object(dispatch.task_bridge, "arm"):
            result = dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                                    orca_cli=self.cli, metadata=self.metadata,
                                    resume_session=session, follow_up="Fix finding R1 only")
        admit.assert_called_once()
        self.assertEqual(result["generation"], 1)
        commands = [entry.args[1] for entry in cli.call_args_list]
        launcher = commands[1][commands[1].index("--command") + 1]
        self.assertIn("--resume-session " + session, launcher)
        self.assertIn("--follow-up-json", launcher)
        spec = commands[3][commands[3].index("--spec") + 1]
        self.assertIn("Fix finding R1 only", spec)
        self.assertNotIn(self.ticket["prompt"], spec)
        self.assertNotEqual(dispatch.dispatch_path(REQUEST, self.ticket),
                            dispatch.dispatch_path(REQUEST, {**self.ticket, "generation": 0}))
        with patch.object(dispatch, "run_cli") as repeated, self.assertRaisesRegex(dispatch.DispatchError, "different"):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                           orca_cli=self.cli, metadata=self.metadata,
                           resume_session=session, follow_up="different fix")
        repeated.assert_not_called()

    def test_reconciled_input_failure_retries_original_task_without_new_spec(self):
        run = {"id": "run_fixture", "consumer_generation": 1}
        receipt = self.root / "recovery.json"
        orca_frontdesk.write_ledger(receipt, {"phase": "complete", "request_id": REQUEST,
            "after": {"loop": {"lanes": {"worker-a": {"ticket": self.ticket}},
                               "terminal": COORDINATOR, "run": {"context": run}}},
            "before": {"attempt": {"exit_on_settlement": True}},
            "failed_input": {"observed": {"dispatch": {"id": "ctx_previous", "taskId": "task_original",
                                                      "status": "failed", "lastFailure": "agent_prompt_blocked"}}}})
        dispatch.save(dispatch.dispatch_path(REQUEST, self.ticket), {"phase": "retry_ready", "recovery": str(receipt),
                      "retry": {"task": "task_original", "dispatch": "ctx_previous"}})
        receipts = [{"run": {**run, "coordinator_handle": COORDINATOR, "legacy": 0}},
                    {"terminal": {"handle": TERMINAL}},
                    {"wait": {"satisfied": True}},
                    {"runId": "run_fixture", "taskId": "task_original", "dispatchId": "ctx_retry",
                     "state": "ready", "stage": "input_accepted"}]
        with patch.object(dispatch, "run_cli", side_effect=receipts) as cli, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), patch.object(dispatch.task_bridge, "arm"):
            result = dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                                    orca_cli=self.cli, metadata=self.metadata, run_context=run, exit_on_settlement=True)
        self.assertEqual(result["task_id"], "task_original")
        argv = cli.call_args_list[-1].args[1]
        self.assertEqual(argv[argv.index("--retry-of") + 1], "ctx_previous")
        self.assertNotIn("--spec", argv)

    def test_settled_reviewer_tab_retry_reuses_run_and_close_proof(self):
        run = {"id": "run_fixture", "consumer_generation": 1}
        ticket = {**self.ticket, "id": "review-fixture", "generation": 1,
                  "read_only": True, "allowed_directories": [],
                  "source_sha256": orca_roles.fingerprint(self.repo),
                  "validation_evidence": "a" * 64}
        ticket_path = self.root / "review-ticket.json"
        ticket_path.write_text(json.dumps(ticket))
        confirmation = {"ok": True, "result": {"close": {
            "handle": "term_old", "ptyKilled": True}}, "receipt_path": "/proof"}
        recovery = self.root / "tab-recovery.json"
        current = {"phase": "tab_retry_ready", "recovery": str(recovery)}
        orca_frontdesk.write_ledger(recovery, {
            "schema": 1, "request_id": REQUEST, "ticket": ticket, "slot": "reviewer",
            "coordinator": COORDINATOR, "run": run, "exit_on_settlement": True,
            "terminal_close": confirmation, "after": current,
        })
        dispatch.save(dispatch.dispatch_path(REQUEST, ticket), current)
        receipts = [{"run": {**run, "coordinator_handle": COORDINATOR, "legacy": 0}},
                    {"wait": {"satisfied": True}},
                    {"runId": "run_fixture", "taskId": "task_review", "dispatchId": "ctx_review",
                     "state": "ready", "stage": "input_accepted"}]
        with patch.object(dispatch, "run_cli", side_effect=receipts), \
                patch.object(dispatch.role_tabs, "launch", return_value={
                    "handle": TERMINAL, "launch_id": "launch_review"}) as launch, \
                patch.object(dispatch, "wait_for_bridge", return_value=BRIDGE), \
                patch.object(dispatch.task_bridge, "arm"):
            result = dispatch.start(REQUEST, ticket_path, "reviewer", COORDINATOR,
                                    orca_cli=self.cli, metadata=self.metadata,
                                    run_context=run, exit_on_settlement=True)
        self.assertEqual(result["dispatch_id"], "ctx_review")
        self.assertEqual(launch.call_args.kwargs["closed_receipt"], confirmation)

    def test_authorize_settled_reviewer_tab_retry_requires_preterminal_attempt(self):
        run = {"id": "run_fixture", "consumer_generation": 1}
        ticket = {**self.ticket, "id": "review-fixture", "generation": 1,
                  "read_only": True, "allowed_directories": [],
                  "source_sha256": orca_roles.fingerprint(self.repo),
                  "validation_evidence": "a" * 64}
        attempt = {"schema": 1, "request_id": REQUEST, "ticket_sha256": orca_role_state.digest(ticket),
                   "slot": "reviewer", "repo": str(self.repo), "phase": "unknown",
                   "run_id": run["id"], "shared_run": run, "exit_on_settlement": True,
                   "terminal": None, "bridge_id": None, "task_id": None, "dispatch_id": None}
        dispatch.save(dispatch.dispatch_path(REQUEST, ticket), attempt)
        registry = self.root / "role-tab.json"
        identity = {"handle": "term_old", "incarnationId": "old", "worktreeId": "old-worktree"}
        orca_frontdesk.write_ledger(registry, {"request": REQUEST, "repo": str(self.repo),
                                              "slot": "reviewer", "phase": "known",
                                              "identity": identity})
        confirmation = {"ok": True, "result": {"close": {
            "handle": "term_old", "ptyKilled": True}}, "receipt_path": "/proof"}
        with patch.object(dispatch.ui_coordinator, "read_registered_state", return_value={
                    "phase": "ready", "terminal": COORDINATOR, "repo": str(self.repo)}), \
                patch.object(dispatch, "linear_record"), patch.object(dispatch, "checked_run"), \
                patch.object(dispatch.intake, "default_orca_cli", return_value=self.cli), \
                patch.object(dispatch.intake, "checked_orca_cli", return_value=self.cli), \
                patch.object(dispatch.role_tabs, "registry_path", return_value=registry), \
                patch.object(dispatch.role_tabs, "settled_close_confirmation",
                             return_value=confirmation):
            receipt = dispatch.authorize_settled_tab_retry(
                REQUEST, ticket, "reviewer", COORDINATOR, run, True)
        self.assertEqual(receipt["terminal_close"], confirmation)
        saved = orca_frontdesk.read_private_json(dispatch.dispatch_path(REQUEST, ticket), {})
        self.assertEqual(saved["phase"], "tab_retry_ready")

    def test_forged_generation_is_refused_before_external_mutation(self) -> None:
        self.ticket["generation"] = 1
        self.ticket_path.write_text(json.dumps(self.ticket))
        with patch.object(dispatch, "run_cli") as cli, self.assertRaises(ValueError):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                           orca_cli=self.cli, metadata=self.metadata,
                           resume_session="e1fd2684-d55a-4794-9741-903c92b7dbea", follow_up="fix")
        cli.assert_not_called()
        with patch.object(dispatch, "run_cli") as cli, self.assertRaisesRegex(dispatch.DispatchError, "same session"):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                           orca_cli=self.cli, metadata=self.metadata)
        cli.assert_not_called()

    def test_requires_linear_intake_and_successful_consultation(self) -> None:
        ledger = orca_issue_context.ledger_path()
        orca_frontdesk.write_ledger(ledger, {"schema": 1, "imports": []})
        with self.assertRaisesRegex(dispatch.DispatchError, "Linear intake"):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                           orca_cli=self.cli, metadata=self.metadata)

        orca_frontdesk.write_ledger(ledger, {
            "schema": 1,
            "imports": [{
                "request_id": REQUEST, "workspace_id": WORKSPACE, "issue_id": ISSUE,
                "identifier": "HW-42", "snapshot_sha256": "a" * 64,
                "created_at": "2026-09-21T00:00:00+00:00",
            }],
        })
        state = orca_coordinator.read_state(REQUEST)
        state["turns"][-1]["phase"] = "unknown"
        orca_coordinator.save_state(state)
        with self.assertRaisesRegex(dispatch.DispatchError, "consultation"):
            dispatch.start(REQUEST, self.ticket_path, "worker-a", COORDINATOR,
                           orca_cli=self.cli, metadata=self.metadata)

    def test_visible_ui_coordinator_replaces_hidden_consultation_gate(self) -> None:
        state = orca_coordinator.read_state(REQUEST)
        state["turns"][-1]["phase"] = "unknown"
        orca_coordinator.save_state(state)
        orca_frontdesk.write_ledger(orca_ui_coordinator.state_path(REQUEST), {
            "schema": 1,
            "request_id": REQUEST,
            "repo": str(orca_ui_coordinator.REPO),
            "terminal": COORDINATOR,
            "worktree_id": f"fixture::{orca_ui_coordinator.REPO}",
            "linear_identifier": "HW-42",
            "phase": "ready",
            "created_at": "2026-09-22T00:00:00+00:00",
            "acknowledged_at": "2026-09-22T00:00:01+00:00",
            "exited_at": None,
            "exit_code": None,
        })

        self.assertEqual(
            dispatch.checked_coordinator(REQUEST, COORDINATOR)["terminal"],
            COORDINATOR,
        )
        with self.assertRaisesRegex(dispatch.DispatchError, "配車元"):
            dispatch.checked_coordinator(REQUEST, "term_other")

    def test_task_spec_keeps_validation_review_and_commit_with_coordinator(self) -> None:
        spec = dispatch.task_spec(self.ticket, "worker-a")
        for heading in ("Target:", "Change:", "Constraints:", "Ownership:",
                        "Observable acceptance:"):
            self.assertIn(heading, spec)
        self.assertIn("no subagents, builds, tests, commit, push, PR", spec)
        self.assertIn("validation, review, commit, and integration remain coordinator-owned", spec)


if __name__ == "__main__":
    unittest.main()
