from __future__ import annotations

import copy
import sys
import signal
import subprocess
import hashlib
import uuid
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import Mock, patch

from scripts import host_coordination, orca_git_checkpoint as checkpoints, orca_role_state as state, orca_roles as roles
from scripts.tests import test_orca_roles as fixtures


class CheckpointTests(unittest.TestCase):
    run_git = staticmethod(fixtures.OrcaRoleTests.run_git)

    def setUp(self):
        fixtures.OrcaRoleTests.setUp(self)
        self.run_git(self.primary, "config", "user.name", "Fixture")
        self.run_git(self.primary, "config", "user.email", "fixture@example.invalid")
        for module in (state, roles, host_coordination):
            patcher = patch.object(module, "state_root", return_value=self.root / "state/coordination")
            patcher.start()
            self.addCleanup(patcher.stop)
        self.snapshot = {"session_id": str(uuid.uuid4()), "session_sha256": "a" * 64}
        patcher = patch.object(state, "session_snapshot", side_effect=lambda *args: dict(self.snapshot))
        patcher.start()
        self.addCleanup(patcher.stop)
        self.edit_and_record()

    def edit_and_record(self):
        (self.repo / "src/content.txt").write_text("changed")
        key = checkpoints.task_key(self.ticket)
        task = {"key": key, "ticket_sha256": state.digest(self.ticket), "subject": checkpoints.subject(self.ticket),
                "origin": str(self.repo), "source_sha256": roles.fingerprint(self.repo), **self.snapshot}
        self.data = {"schema": 1, "slot": "worker-a", "provider": "codex", "tasks": {key: task},
                     "last": {"key": key, "phase": "recorded", "process_exited": True, "exit_code": 0,
                              "attempt_id": str(uuid.uuid4())}}
        state.save_state(self.data)
        state.claim_task(key, "worker-a", "codex", self.ticket, task["subject"])

    def validate(self, command=None):
        return checkpoints.validate(self.ticket, "worker-a", command or [sys.executable, "-c", "pass"],
                                    "Fixture only; no player behavior changes")

    def test_commit_and_same_session_resume_without_relaxing_guards(self):
        evidence = self.validate()
        result = checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        self.assertEqual(roles.git(self.repo, "status", "--porcelain"), "")
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD^"), self.ticket["base"])
        next_ticket = result["next_ticket"]
        roles.validate_ticket(next_ticket)
        state.admit(self.data, next_ticket, checkpoints.subject(next_ticket), roles.fingerprint(self.repo),
                    self.snapshot["session_id"], "Fix the review finding")
        state.claim_task(checkpoints.task_key(next_ticket), "worker-a", "codex", next_ticket, checkpoints.subject(next_ticket))
        self.assertEqual(checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"]), result)
        with self.assertRaisesRegex(ValueError, "exact same task"):
            state.admit(self.data, {**next_ticket, "prompt": "broaden scope"}, checkpoints.subject(next_ticket),
                        roles.fingerprint(self.repo), self.snapshot["session_id"], "next")
        with self.assertRaisesRegex(ValueError, "explicit resume"):
            state.admit(self.data, next_ticket, checkpoints.subject(next_ticket), roles.fingerprint(self.repo),
                        str(uuid.uuid4()), "next")

    def test_recovers_after_ref_update_before_real_index_update(self):
        evidence = self.validate()
        original = checkpoints.git

        def crash(repo, *argv, **kwargs):
            if argv[0] == "read-tree" and "env" not in kwargs:
                raise OSError("simulated stop after ref update")
            return original(repo, *argv, **kwargs)

        with patch.object(checkpoints, "git", side_effect=crash), self.assertRaises(OSError):
            checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        committed = roles.git(self.repo, "rev-parse", "HEAD")
        result = checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        self.assertEqual(result["candidate"], committed)
        self.assertEqual(roles.git(self.repo, "rev-list", "--count", "HEAD"), "2")

    def test_rejects_dirty_source_failed_validation_and_changed_session(self):
        evidence = self.validate([sys.executable, "-c", "raise SystemExit(1)"])
        with self.assertRaisesRegex(ValueError, "successful same-source"):
            checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        evidence = self.validate()
        (self.repo / "src/content.txt").write_text("external edit")
        with self.assertRaises(ValueError):
            checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.ticket["base"])
        self.snapshot["session_sha256"] = "b" * 64
        with self.assertRaisesRegex(ValueError, "session changed"):
            self.validate()

    def test_untracked_addition_is_committed(self):
        (self.repo / "src/new.txt").write_text("new file")
        self.edit_and_record()
        result = checkpoints.checkpoint(self.ticket, "worker-a", self.validate()["id"])
        self.assertEqual(result["phase"], "committed")
        self.assertEqual(roles.git(self.repo, "status", "--porcelain"), "")

    def test_source_changes_during_validation_are_not_accepted(self):
        with self.assertRaisesRegex(ValueError, "validation modified"):
            self.validate([sys.executable, "-c", "from pathlib import Path; Path('src/content.txt').write_text('bad')"])

    def test_validation_exports_reviewed_no_impact_reason(self):
        reason = "Fixture only; no player behavior changes"
        evidence = checkpoints.validate(
            self.ticket,
            "worker-a",
            [sys.executable, "-c", "import os; assert os.environ['HELL_WORKERS_HELP_IMPACT_REASON'] == " + repr(reason)],
            reason,
        )
        self.assertEqual(evidence["exit_code"], 0)

    def test_repo_local_dev_validation_uses_current_host_runner(self):
        candidate = self.repo / "scripts/dev.py"
        candidate.parent.mkdir()
        candidate.write_text("raise SystemExit('stale runner must not execute')\n")
        command = [sys.executable, "scripts/dev.py", "ci", "check", "--base", self.ticket["base"]]
        execution, executor = checkpoints.validation_execution(self.repo, command)
        runner = Path(checkpoints.__file__).with_name("orca_host_validation.py").resolve()
        self.assertEqual(execution, [
            sys.executable, str(runner), "--repo", str(self.repo), "--",
            "ci", "check", "--base", self.ticket["base"],
        ])
        self.assertEqual(executor, {
            "schema": 1,
            "kind": "host-dev-runner",
            "sha256": hashlib.sha256(runner.read_bytes()).hexdigest(),
        })

    def test_non_repository_validation_command_is_not_rewritten(self):
        command = [sys.executable, "-c", "pass"]
        self.assertEqual(checkpoints.validation_execution(self.repo, command), (command, None))

    def test_checkpoint_accepts_only_sealed_same_source_validation_recovery(self):
        source = roles.fingerprint(self.repo)
        recovery_dir = state.storage.checked_directory(
            state.state_path("worker-a").parent / "loops" / "validation-resumes"
        )
        recovery_path = recovery_dir / "fixture-recovery.json"
        state.storage.write_ledger(recovery_path, {
            "schema": 1, "request_id": str(uuid.uuid4()), "run_id": "run_fixture",
            "subject": "worker-a", "evidence": "e" * 64, "source_sha256": source,
            "failed_settlement": "fixture", "reason": "fixture recovery",
        })
        self.data["last"].update(
            exit_code=1,
            coordinator_validation_recovery=str(recovery_path),
            settlement_exit={"outcome": "failed", "source_sha256": "0" * 64},
        )
        state.save_state(self.data)
        with (patch.object(roles, "require_completed_bridge"),
              self.assertRaisesRegex(ValueError, "exact successful worker exit")):
            self.validate()
        self.data["last"]["settlement_exit"]["source_sha256"] = source
        state.save_state(self.data)
        with patch.object(roles, "require_completed_bridge"):
            evidence = self.validate()
            result = checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        self.assertEqual(result["phase"], "committed")

    def test_recovery_preserves_unknown_index_and_external_source(self):
        evidence = self.validate()
        with patch.object(checkpoints, "finish", side_effect=OSError("crash")), self.assertRaises(OSError):
            checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        (self.repo / "src/content.txt").write_text("external")
        with self.assertRaisesRegex(ValueError, "source changed"):
            checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.ticket["base"])
        self.assertEqual((self.repo / "src/content.txt").read_text(), "external")

    def test_live_role_lease_refuses_validation_and_commit(self):
        evidence = self.validate()
        with host_coordination.acquire_host("worker-a", inherit=False):
            with self.assertRaisesRegex(RuntimeError, "busy"):
                self.validate()
            with self.assertRaisesRegex(RuntimeError, "busy"):
                checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])

    def test_provider_exit_zero_does_not_promote_failed_orca_settlement(self):
        bridge = str(uuid.uuid4())
        self.data["last"].update(orca_bridge=bridge, terminal="term_fixture")
        state.save_state(self.data)
        with patch.object(roles.task_bridge, "root", return_value=self.root / "bridges"):
            with self.assertRaisesRegex(ValueError, "settlement"):
                self.validate()
            directory = self.root / "bridges" / bridge
            authority = {"run": "run_fixture", "task": "task_fixture", "dispatch": "ctx_fixture", "coordinator": "term_coordinator"}
            state.storage.write_ledger(directory / "arm.json", authority)
            state.storage.write_ledger(directory / "identity.json", {"repo": str(self.repo), "terminal": "term_fixture"})
            journal = {"phase": "settled", "settled_status": "failed", "revoked": True,
                       "authority": authority, "operations": {}}
            state.storage.write_ledger(directory / "journal.json", journal)
            with self.assertRaisesRegex(ValueError, "settlement"):
                self.validate()
            journal.update(settled_status="completed", operations={"event": {"phase": "confirmed", "result": {
                "message": {"type": "worker_done", "run_id": "run_fixture", "from_handle": "term_fixture"},
                "lifecycle": {"action": "completed", "taskId": "task_fixture", "dispatchId": "ctx_fixture"}}}})
            state.storage.write_ledger(directory / "journal.json", journal)
            self.assertEqual(self.validate()["exit_code"], 0)

    def test_settled_idle_completion_stops_only_owned_child_and_records_real_exit(self):
        class Child:
            pid = 123456789
            returncode = None
            calls = 0

            def poll(self):
                return self.returncode

            def wait(self, timeout=None):
                self.calls += 1
                if self.calls == 1:
                    raise subprocess.TimeoutExpired("fixture", timeout)
                self.returncode = -signal.SIGTERM
                return self.returncode

        for outcome, expected in (("completed", 0), ("failed", 1)):
            data = copy.deepcopy(self.data)
            data["last"]["phase"] = "starting"
            child = Child()
            receipt = {"outcome": outcome, "source_sha256": roles.fingerprint(self.repo)}
            with patch.object(roles.subprocess, "Popen", return_value=child), patch.object(roles.os, "killpg") as kill:
                self.assertEqual(roles.run_provider(["fixture"], data, completion_probe=lambda: receipt), expected)
            kill.assert_called_once_with(child.pid, signal.SIGTERM)
            self.assertEqual(data["last"]["provider_exit_code"], -signal.SIGTERM)
            self.assertEqual(data["last"]["exit_code"], expected)
            self.assertTrue(data["last"]["process_exited"])
            self.assertEqual(data["last"]["phase"], "unknown")  # Only launch postconditions can seal it.

    def test_unknown_or_non_idle_completion_never_earns_success(self):
        with self.assertRaisesRegex(ValueError, "supervised bridge"):
            roles.launch(self.ticket, "worker-a", dry_run=False, exit_on_settlement=True)

    def test_settlement_probe_requires_confirmed_receipt_exact_terminal_and_idle(self):
        directory = self.root / "probe"
        authority = {"run": "run_fixture", "task": "task_fixture", "dispatch": "ctx_fixture", "coordinator": "term_coordinator"}
        binding = SimpleNamespace(terminal="term_fixture", repo=self.repo, validate=Mock())
        bridge = SimpleNamespace(directory=directory, policy=SimpleNamespace(authority=authority),
                                 upstream=SimpleNamespace(metadata_path=self.root / "metadata/runtime.json", runtime_id="epoch"),
                                 binding=binding, identifier=str(uuid.uuid4()))
        observer = Mock(runtime_id="epoch")
        journal = {"phase": "active"}
        state.storage.write_ledger(directory / "journal.json", journal)
        with patch.object(roles.task_bridge.wire.Upstream, "load", return_value=observer) as load:
            self.assertIsNone(roles.settlement_exit(bridge))
            load.assert_not_called()
            journal = {"phase": "settled", "settled_status": "completed", "authority": authority,
                       "operations": {"event": {"phase": "pending"}}}
            state.storage.write_ledger(directory / "journal.json", journal)
            self.assertIsNone(roles.settlement_exit(bridge))
            load.assert_not_called()
            journal["operations"]["event"] = {"phase": "confirmed", "result": {
                "message": {"type": "worker_done"},
                "lifecycle": {"action": "completed", "taskId": "task_fixture", "dispatchId": "ctx_fixture"}}}
            state.storage.write_ledger(directory / "journal.json", journal)
            for failure in (roles.task_bridge.wire.ObservationUnavailable("unconfirmed"), TimeoutError(), ConnectionError()):
                observer.call.side_effect = failure
                self.assertIsNone(roles.settlement_exit(bridge))
            for idle in (False, True):
                observer.call.side_effect = [{"terminal": {}}, {"wait": {
                    "handle": "term_fixture", "condition": "tui-idle", "satisfied": idle, "status": "running"}},
                    {"terminal": {}}]
                result = roles.settlement_exit(bridge)
                self.assertEqual(result is not None, idle)
            observer.call.side_effect = [{"terminal": {}}, {"wait": {
                "handle": "term_other", "condition": "tui-idle", "satisfied": True, "status": "running"}}]
            with self.assertRaisesRegex(RuntimeError, "readiness"):
                roles.settlement_exit(bridge)

    def test_cursor_checkpoint_keeps_provider_scope_and_same_session(self):
        scope = "scripts/tests/fixtures/orca_edit_acceptance/worker-b"
        path = self.repo / scope
        path.mkdir(parents=True)
        self.run_git(self.repo, "add", "src")
        (path / "content.txt").write_text("original")
        self.run_git(self.repo, "add", "scripts")
        self.run_git(self.repo, "-c", "commit.gpgsign=false", "commit", "-qm", "Cursor fixture")
        self.ticket.update(base=roles.git(self.repo, "rev-parse", "HEAD"), allowed_directories=[scope],
                           complexity="simple", task_kind="acceptance-edit", complexity_reason="isolated fixture",
                           acceptance="bounded edit only")
        (path / "content.txt").write_text("changed")
        key = checkpoints.task_key(self.ticket)
        task = {"key": key, "ticket_sha256": state.digest(self.ticket), "subject": checkpoints.subject(self.ticket),
                "origin": str(self.repo), "source_sha256": roles.fingerprint(self.repo), **self.snapshot}
        data = {"schema": 1, "slot": "worker-b", "provider": "cursor", "tasks": {key: task},
                "last": {"key": key, "phase": "recorded", "process_exited": True, "exit_code": 0,
                         "attempt_id": str(uuid.uuid4())}}
        state.save_state(data)
        # A distinct assignment represents the independently placed B task.
        self.ticket["id"] = "cursor-checkpoint"
        key = checkpoints.task_key(self.ticket)
        task.update(key=key, ticket_sha256=state.digest(self.ticket))
        data["tasks"] = {key: task}
        data["last"]["key"] = key
        state.save_state(data)
        state.claim_task(key, "worker-b", "cursor", self.ticket, task["subject"])
        evidence = checkpoints.validate(self.ticket, "worker-b", [sys.executable, "-c", "pass"], "Fixture only")
        result = checkpoints.checkpoint(self.ticket, "worker-b", evidence["id"])
        ticket = result["next_ticket"]
        state.admit(data, ticket, checkpoints.subject(ticket), roles.fingerprint(self.repo),
                    self.snapshot["session_id"], "Fix the fixture")
        self.assertEqual(result["provider"], "cursor")

    def test_scope_index_and_active_worker_are_rejected(self):
        (self.repo / "docs/content.txt").write_text("outside")
        with self.assertRaisesRegex(ValueError, "outside"):
            self.validate()
        (self.repo / "docs/content.txt").write_text("original")
        self.run_git(self.repo, "add", "src")
        with self.assertRaisesRegex(ValueError, "index"):
            self.validate()

    def test_review_receipt_survives_next_subject_but_not_source_change(self):
        evidence = self.validate()
        committed = checkpoints.checkpoint(self.ticket, "worker-a", evidence["id"])
        ticket = checkpoints.review_ticket(committed)
        self.assertNotEqual(ticket["base"], ticket["review_base"])
        bound = {"key": "fixed-reviewer", "ticket_sha256": state.digest(ticket), "subject": checkpoints.subject(ticket),
                 "origin": str(self.repo), "source_sha256": roles.fingerprint(self.repo), **self.snapshot}
        data = {"schema": 1, "slot": "reviewer", "provider": "codex", "tasks": {"fixed-reviewer": bound},
                "last": {"key": "fixed-reviewer", "phase": "recorded", "process_exited": True,
                         "exit_code": 0, "attempt_id": str(uuid.uuid4())}}
        state.save_state(data)
        record = {"ticket": ticket["id"], "base": ticket["review_base"], "head": ticket["base"],
                  "source_sha256": roles.fingerprint(self.repo), "verdict": "approved",
                  "blocking_findings": [], "reviewer_session": self.snapshot["session_id"],
                  "validation_evidence": evidence["id"]}
        sealed = roles.seal_review(ticket, record)
        other = copy.deepcopy(data)
        other["tasks"]["fixed-reviewer"].update(ticket_sha256="b" * 64, session_sha256="c" * 64)
        state.save_state(other)
        roles.verify_review(ticket, sealed)
        for phase in ("starting", "unknown"):
            other["last"]["phase"] = phase
            state.save_state(other)
            roles.verify_review(ticket, sealed)
            with self.assertRaisesRegex(ValueError, "unknown role attempt"):
                state.read_state("reviewer", "codex")
        other["tasks"]["fixed-reviewer"]["session_id"] = str(uuid.uuid4())
        state.save_state(other)
        with self.assertRaisesRegex(ValueError, "fixed reviewer"):
            roles.verify_review(ticket, sealed)
        state.save_state(data)
        with self.assertRaisesRegex(ValueError, "stale"):
            roles.verify_review(ticket, {**sealed, "validation_evidence": "forged"})
        (self.repo / "src/content.txt").write_text("changed after review")
        with self.assertRaisesRegex(ValueError, "stale"):
            roles.verify_review(ticket, sealed)


if __name__ == "__main__":
    unittest.main()
