from __future__ import annotations

import json
import shutil
import sqlite3
import subprocess
import sys
import unittest
from contextlib import nullcontext
from types import SimpleNamespace
from unittest.mock import patch

from scripts import orca_role_state as state, orca_roles as roles
from scripts.tests import test_orca_roles as fixtures


SESSION = "e1fd2684-d55a-4794-9741-903c92b7dbea"
OTHER = "c379e7f4-f1da-4f9b-844e-398072a325dd"
FAKE = r'''
import hashlib, json, os, pathlib, sqlite3, sys
options = json.loads(sys.argv[1])
repo = pathlib.Path.cwd()
runtime = pathlib.Path(os.environ['CODEX_HOME']).parent
session = options['session']
if options['history']:
    if options['provider'] == 'codex':
        path = runtime / 'codex/sessions' / ('rollout-' + session + '.jsonl')
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists():
            path.write_text(json.dumps({'type': 'session_meta', 'payload': {
                'id': session, 'cwd': str(repo)}}) + '\n')
        with path.open('a') as stream:
            stream.write(json.dumps({'type': 'fixture-turn'}) + '\n')
    else:
        key = hashlib.md5(str(repo).encode()).hexdigest()
        path = runtime / 'cursor/chats' / key / session / 'store.db'
        path.parent.mkdir(parents=True, exist_ok=True)
        connection = sqlite3.connect(path)
        connection.execute('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT)')
        meta = {'agentId': session, 'latestRootBlobId': '0102'}
        connection.execute('INSERT OR REPLACE INTO meta VALUES (?, ?)', ('0', json.dumps(meta).encode().hex()))
        connection.commit()
        connection.close()
if options['edit']:
    (repo / options['edit']).write_text('fixture edit')
if options['control']:
    try:
        pathlib.Path(options['control']).write_text('escape')
    except OSError:
        pass
    else:
        raise AssertionError('control state was writable')
sys.exit(options['code'])
'''


@unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap"), "Linux bubblewrap required")
class RoleContinuationTests(unittest.TestCase):
    run_git = staticmethod(fixtures.OrcaRoleTests.run_git)
    load = fixtures.OrcaRoleTests.load

    def setUp(self) -> None:
        fixtures.OrcaRoleTests.setUp(self)
        self.coordination = self.root / "state/coordination"
        self.coordination.mkdir(parents=True, mode=0o700)
        self.options = {"session": SESSION, "history": True, "edit": None, "code": 0}
        self.commands = []
        for module in (state, roles):
            patcher = patch.object(module, "state_root", return_value=self.coordination)
            patcher.start()
            self.addCleanup(patcher.stop)
        patcher = patch.object(roles, "acquire_host", side_effect=lambda *a, **k: nullcontext())
        patcher.start()
        self.addCleanup(patcher.stop)
        patcher = patch.object(roles, "command_for", side_effect=self.command)
        patcher.start()
        self.addCleanup(patcher.stop)

    def command(self, provider, repo, role, prompt, resume, *, read_only=False,
                externally_sandboxed=False):
        self.commands.append((provider, role, prompt, resume, externally_sandboxed))
        return [sys.executable, "-c", FAKE, json.dumps({**self.options, "provider": provider,
                "control": str(state.state_path("reviewer" if role == "reviewer" else
                                                "worker-b" if provider == "cursor" else "worker-a"))})]

    def launch(self, slot="worker-a", resume=None, follow_up=None):
        return roles.launch(self.load(), slot, dry_run=False, resume_session=resume, follow_up=follow_up)

    def reviewer(self):
        self.ticket["allowed_directories"] = []
        self.ticket["source_sha256"] = roles.fingerprint(self.repo)

    def cursor(self):
        (self.repo / ".cursor").mkdir(exist_ok=True)
        scope = "crates/hw_ui/src/interaction/help"
        path = self.repo / scope
        path.mkdir(parents=True)
        (path / "content.txt").write_text("original")
        self.run_git(self.repo, "add", scope)
        self.run_git(self.repo, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "-qm", "cursor fixture")
        self.ticket.update(base=roles.git(self.repo, "rev-parse", "HEAD"), allowed_directories=[scope],
                           complexity="simple", task_kind="test-addition", complexity_reason="fixture",
                           acceptance="fixture assertion")
        return scope + "/content.txt"

    def test_dirty_codex_resume_requires_same_task_uuid_and_explicit_followup(self):
        self.options["edit"] = "src/content.txt"
        self.assertEqual(self.launch(), 0)
        self.assertTrue(self.commands[-1][4])
        self.assertTrue(roles.git(self.repo, "status", "--porcelain"))
        for resume, follow in ((None, None), (OTHER, "next"), (SESSION, None)):
            with self.subTest(resume=resume, follow=follow), self.assertRaises(ValueError):
                self.launch(resume=resume, follow_up=follow)
        self.assertEqual(self.launch(resume=SESSION, follow_up="Only inspect prior edits."), 0)
        self.assertNotIn(self.ticket["prompt"], self.commands[-1][2])
        self.assertEqual(self.commands[-1][3], SESSION)

    def test_cursor_dirty_resume_and_metadata_validation(self):
        self.options["edit"] = self.cursor()
        self.assertEqual(self.launch("worker-b"), 0)
        self.assertEqual(self.launch("worker-b", SESSION, "Check previous changes."), 0)
        data = state.read_state("worker-b", "cursor")
        bound = next(iter(data["tasks"].values()))
        runtime = roles.prepare_runtime("worker-b/tasks/" + bound["key"])
        first = state.session_snapshot(runtime, "cursor", self.repo)
        self.assertEqual(first, state.session_snapshot(runtime, "cursor", self.repo))
        self.assertEqual(first["session_id"], SESSION)

    def test_read_only_workers_bind_dirty_source_and_cannot_gain_write_access(self):
        for slot in ("worker-a", "worker-b"):
            with self.subTest(slot=slot):
                if slot == "worker-b":
                    self.cursor()
                write_scope = list(self.ticket["allowed_directories"])
                (self.repo / "src/content.txt").write_text("coordinator-owned dirty input")
                self.ticket.update(id="readonly-" + slot, read_only=True, allowed_directories=[],
                                   complexity="simple", task_kind="test-addition",
                                   complexity_reason="only checks conversation continuity",
                                   acceptance="same session, unchanged source",
                                   source_sha256=roles.fingerprint(self.repo))
                self.assertEqual(self.launch(slot), 0)
                self.assertEqual(self.launch(slot, SESSION, "Recall prior turn, no edits."), 0)
                self.ticket["read_only"] = False
                self.ticket["allowed_directories"] = write_scope
                with self.assertRaisesRegex(ValueError, "exact same task"):
                    self.launch(slot, SESSION, "Cannot change access.")

    def test_read_only_worker_rejects_stale_source_before_claim(self):
        self.ticket.update(read_only=True, allowed_directories=[], source_sha256="0" * 64)
        with self.assertRaisesRegex(ValueError, "current source_sha256"):
            self.launch()
        self.assertFalse((self.coordination.parent / "role-state/assignments").exists())

    def test_unsettled_bridge_prevents_checkpoint_and_fresh_or_resumed_role(self):
        self.ticket.update(read_only=True, allowed_directories=[], source_sha256=roles.fingerprint(self.repo))
        channel = SimpleNamespace(identifier=OTHER, client=self.root / "bridge-client",
                                  policy=SimpleNamespace(phase="unknown"), mounts=lambda: [])
        with patch.object(roles.task_bridge, "Session", return_value=nullcontext(channel)):
            with self.assertRaisesRegex(RuntimeError, "Task bridge outcome unknown"):
                roles.launch(self.load(), "worker-a", dry_run=False, bridge_settings=(self.root, self.root))
        data = state.read_state("worker-a", "codex", allow_pending=True)
        self.assertEqual(data["last"]["phase"], "unknown")
        self.assertEqual(data["last"]["orca_bridge"], OTHER)
        self.assertTrue(data["last"]["process_exited"])
        self.assertEqual(data["last"]["exit_code"], 0)
        self.assertFalse(data["tasks"])
        self.assertTrue(self.commands[-1][4])
        for resume, follow_up in ((None, None), (SESSION, "Continue")):
            with self.subTest(resume=resume), self.assertRaises(ValueError):
                self.launch(resume=resume, follow_up=follow_up)

    def test_read_only_worker_mount_blocks_fake_provider_edit(self):
        self.ticket.update(read_only=True, allowed_directories=[], source_sha256=roles.fingerprint(self.repo))
        self.options["edit"] = "src/content.txt"
        self.assertNotEqual(self.launch(), 0)
        self.assertEqual((self.repo / "src/content.txt").read_text(), "original")

    def failed_read_only_start(self):
        self.ticket.update(read_only=True, allowed_directories=[], source_sha256=roles.fingerprint(self.repo))
        self.options.update(history=False, code=1)
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.launch()
        return state.read_state("worker-a", "codex", allow_pending=True)

    def test_abandon_preserves_failed_empty_start_and_refuses_replay(self):
        failed = self.failed_read_only_start()
        attempt = failed["last"]["attempt_id"]
        source = roles.fingerprint(self.repo)
        with self.assertRaisesRegex(ValueError, "observed source"):
            roles.abandon_start(self.load(), "worker-a", attempt, "0" * 64, "Inspected startup failure")
        original = dict(self.ticket)
        self.ticket["prompt"] = "different instruction"
        with self.assertRaisesRegex(ValueError, "ownership differs"):
            roles.abandon_start(self.load(), "worker-a", attempt, source, "Inspected startup failure")
        self.ticket = original
        with self.assertRaisesRegex(ValueError, "exact failed"):
            roles.abandon_start(self.load(), "worker-a", OTHER, source, "Inspected startup failure")
        roles.abandon_start(self.load(), "worker-a", attempt, source, "Inspected startup failure")
        data = state.read_state("worker-a", "codex")
        self.assertEqual(data["last"]["phase"], "abandoned")
        self.assertEqual(data["abandoned"][failed["last"]["key"]]["attempt_id"], attempt)
        self.assertEqual(data["last"]["source_before"], failed["last"]["source_before"])
        self.assertEqual(data["last"]["exit_code"], 1)
        with self.assertRaisesRegex(ValueError, "abandoned ticket"):
            self.launch()
        self.ticket["id"] = "explicit-new-inspection"
        self.options.update(history=True, code=0)
        self.assertEqual(self.launch(), 0)
        self.assertIn(failed["last"]["key"], state.read_state("worker-a", "codex")["abandoned"])

    def test_abandon_refuses_unknown_exit_success_history_or_reviewer(self):
        failed = self.failed_read_only_start()
        attempt = failed["last"]["attempt_id"]
        source = roles.fingerprint(self.repo)
        bridge_attempt = json.loads(json.dumps(failed))
        bridge_attempt["last"]["orca_bridge"] = OTHER
        state.save_state(bridge_attempt)
        with self.assertRaisesRegex(ValueError, "supervised lifecycle"):
            roles.abandon_start(self.load(), "worker-a", attempt, source, "No provider history but Task may exist")
        state.save_state(failed)
        for change in ({"process_exited": False}, {"exit_code": 0}, {"exit_code": None}, {"phase": "starting"}):
            mutated = json.loads(json.dumps(failed))
            mutated["last"].update(change)
            state.save_state(mutated)
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "positive process-exit"):
                roles.abandon_start(self.load(), "worker-a", attempt, source, "Inspected")
        state.save_state(failed)
        with self.assertRaisesRegex(ValueError, "fixed reviewer"):
            roles.abandon_start(self.load(), "reviewer", attempt, source, "Inspected")
        runtime = roles.prepare_runtime("worker-a/tasks/" + failed["last"]["key"])
        path = runtime / "codex/sessions/unbound.jsonl"
        path.parent.mkdir()
        path.write_text('{}')
        with self.assertRaisesRegex(ValueError, "history exists"):
            roles.abandon_start(self.load(), "worker-a", attempt, source, "Inspected")
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_abandon_preserves_failed_source_unchanged_edit_start(self):
        self.options.update(history=False, code=1)
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.launch()
        failed = state.read_state("worker-a", "codex", allow_pending=True)
        source = roles.fingerprint(self.repo)
        roles.abandon_start(self.load(), "worker-a", failed["last"]["attempt_id"], source,
                            "Fresh Codex configuration failed before editing")
        data = state.read_state("worker-a", "codex")
        self.assertEqual(data["last"]["phase"], "abandoned")
        self.assertEqual(data["last"]["observed_source_sha256"], source)
        self.assertFalse(data["tasks"])

    def test_abandon_save_failure_never_starts_agent(self):
        failed = self.failed_read_only_start()
        with patch.object(state, "save_state", side_effect=OSError("disk")), \
                patch.object(roles, "run_provider") as run:
            with self.assertRaises(OSError):
                roles.abandon_start(self.load(), "worker-a", failed["last"]["attempt_id"],
                                    roles.fingerprint(self.repo), "Inspected startup failure")
            run.assert_not_called()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_abandon_dry_run_flag_refuses_without_mutation(self):
        failed = self.failed_read_only_start()
        self.load()
        with patch.object(sys, "argv", ["orca_roles", "abandon-start", "--ticket", str(self.path),
                                       "--slot", "worker-a", "--dry-run",
                                       "--attempt-id", failed["last"]["attempt_id"],
                                       "--observed-source", roles.fingerprint(self.repo),
                                       "--reason", "Inspected startup failure"]), \
                patch.object(state, "save_state") as save, patch.object(roles, "run_provider") as run:
            self.assertEqual(roles.main(), 1)
            save.assert_not_called()
            run.assert_not_called()
        self.assertEqual(state.read_state("worker-a", "codex", allow_pending=True), failed)

    def test_cursor_wal_is_read_and_snapshot_is_stable_after_close(self):
        self.cursor()
        self.launch("worker-b")
        bound = next(iter(state.read_state("worker-b", "cursor")["tasks"].values()))
        runtime = roles.prepare_runtime("worker-b/tasks/" + bound["key"])
        path = next((runtime / "cursor/chats").glob("*/*/store.db"))
        before = state.session_snapshot(runtime, "cursor", self.repo)
        connection = sqlite3.connect(path)
        try:
            connection.execute("PRAGMA journal_mode=WAL")
            metadata = {"agentId": OTHER, "latestRootBlobId": "010203"}
            connection.execute("UPDATE meta SET value = ?", (json.dumps(metadata).encode().hex(),))
            connection.commit()
            with self.assertRaisesRegex(ValueError, "identity"):
                state.session_snapshot(runtime, "cursor", self.repo)
            metadata["agentId"] = SESSION
            connection.execute("UPDATE meta SET value = ?", (json.dumps(metadata).encode().hex(),))
            connection.commit()
            with_wal = state.session_snapshot(runtime, "cursor", self.repo)
            self.assertNotEqual(before["session_sha256"], with_wal["session_sha256"])
            self.assertEqual(with_wal, state.session_snapshot(runtime, "cursor", self.repo))
        finally:
            connection.close()
        after_close = state.session_snapshot(runtime, "cursor", self.repo)
        self.assertEqual(after_close, state.session_snapshot(runtime, "cursor", self.repo))

    def test_changed_ticket_or_source_or_index_cannot_be_adopted(self):
        self.launch()
        original = dict(self.ticket)
        for field, value in (("prompt", "different"), ("allowed_directories", ["src", "docs"]),
                             ("acceptance", "different")):
            with self.subTest(field=field):
                self.ticket = {**original, field: value}
                with self.assertRaises(ValueError):
                    self.launch(resume=SESSION, follow_up="next")
        self.ticket = original
        (self.repo / "src/content.txt").write_text("external edit")
        with self.assertRaisesRegex(ValueError, "source/index"):
            self.launch(resume=SESSION, follow_up="next")
        self.run_git(self.repo, "add", "src")
        with self.assertRaises(ValueError):
            self.launch(resume=SESSION, follow_up="next")

    def test_missing_session_and_multiple_sessions_leave_unknown(self):
        self.options["history"] = False
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.launch()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_provider_fork_on_resume_leaves_unknown(self):
        self.launch()
        self.options["session"] = OTHER
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.launch(resume=SESSION, follow_up="next")
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch(resume=SESSION, follow_up="next")

    def test_spawn_failure_is_unknown_not_a_clean_retry(self):
        real = roles.run_provider
        def fail(command, data):
            with patch.object(roles.subprocess, "Popen", side_effect=OSError("spawn failed")):
                return real(command, data)
        with patch.object(roles, "run_provider", side_effect=fail):
            with self.assertRaises(OSError):
                self.launch()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_interrupt_kills_own_group_waits_and_preserves_unknown(self):
        data = {"schema": 1, "slot": "worker-a", "provider": "codex", "tasks": {},
                "last": {"phase": "starting", "process_exited": False}}
        class Child:
            pid = 123456789
            returncode = None
            calls = 0
            def poll(self):
                return self.returncode
            def wait(self, timeout=None):
                self.calls += 1
                if self.calls == 1:
                    raise KeyboardInterrupt
                if self.calls == 2:
                    raise subprocess.TimeoutExpired("fixture", timeout)
                self.returncode = -9
                return self.returncode
        child = Child()
        with patch.object(roles.subprocess, "Popen", return_value=child), \
                patch.object(roles.os, "killpg") as kill:
            with self.assertRaises(KeyboardInterrupt):
                roles.run_provider(["fixture"], data)
        self.assertEqual(child.calls, 3)
        self.assertEqual([call.args[1] for call in kill.call_args_list],
                         [roles.signal.SIGTERM, roles.signal.SIGKILL])
        self.assertTrue(data["last"]["process_exited"])
        self.assertEqual(data["last"]["phase"], "unknown")
        with self.assertRaisesRegex(ValueError, "unknown"):
            state.read_state("worker-a", "codex")

    def test_pending_attempt_without_identity_is_rejected_without_mutation(self):
        data = {"schema": 1, "slot": "worker-a", "provider": "codex", "tasks": {},
                "last": {"phase": "unknown", "process_exited": True, "exit_code": -9}}
        state.save_state(data)
        path = state.state_path("worker-a")
        original = path.read_bytes()
        with self.assertRaisesRegex(ValueError, "canonical UUID"):
            state.read_state("worker-a", "codex", allow_pending=True)
        self.assertEqual(path.read_bytes(), original)

    def test_missing_barrier_and_broken_subject_are_not_resumable(self):
        self.launch()
        original = state.read_state("worker-a", "codex")
        for mutation in ("missing", "null", "subject", "origin"):
            data = json.loads(json.dumps(original))
            task = next(iter(data["tasks"].values()))
            if mutation == "missing":
                del data["last"]
            elif mutation == "null":
                data["last"] = None
            elif mutation == "subject":
                task["subject"] = {}
            else:
                task["origin"] = "relative"
            state.save_state(data)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.launch(resume=SESSION, follow_up="next")

    def test_post_exit_save_failure_blocks_duplicate_launch(self):
        real = state.save_state
        calls = 0
        def save(data):
            nonlocal calls
            calls += 1
            if calls == 3:
                raise OSError("final save failed")
            real(data)
        with patch.object(state, "save_state", side_effect=save):
            with self.assertRaises(OSError):
                self.launch()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_nonzero_exit_is_recorded_not_approval_and_can_resume_same_task(self):
        self.options["code"] = 7
        self.assertEqual(self.launch(), 7)
        data = state.read_state("worker-a", "codex")
        self.assertEqual(data["last"]["exit_code"], 7)
        self.assertTrue(data["last"]["process_exited"])
        self.options["code"] = 0
        self.assertEqual(self.launch(resume=SESSION, follow_up="Inspect failure; do not repeat."), 0)

    def test_fixed_reviewer_cross_worktree_and_review_record_binding(self):
        self.reviewer()
        self.launch("reviewer")
        with self.assertRaisesRegex(ValueError, "fixed session"):
            self.launch("reviewer", OTHER)
        second = self.root / "second-candidate"
        self.run_git(self.primary, "worktree", "add", "-qb", "second", str(second))
        self.repo = second
        self.ticket.update(repo=str(second), branch="second", id="second-task")
        self.reviewer()
        self.assertEqual(self.launch("reviewer", SESSION), 0)
        record = {"ticket": self.ticket["id"], "base": self.ticket["base"], "head": self.ticket["base"],
                  "source_sha256": roles.fingerprint(second), "verdict": "approved",
                  "reviewer_session": SESSION, "validation_evidence": "fixture-pass", "blocking_findings": []}
        roles.verify_review(self.load(), record)
        with self.assertRaisesRegex(ValueError, "fixed reviewer"):
            roles.verify_review(self.load(), {**record, "reviewer_session": OTHER})

    def test_arbitrary_existing_history_is_not_adopted(self):
        runtime = roles.prepare_runtime("reviewer")
        path = runtime / "codex/sessions/old.jsonl"
        path.parent.mkdir()
        path.write_text("{}")
        self.reviewer()
        with self.assertRaisesRegex(ValueError, "unbound"):
            self.launch("reviewer")

    def test_same_task_cannot_switch_slots(self):
        self.cursor()
        self.launch()
        with self.assertRaisesRegex(ValueError, "different ownership"):
            self.launch("worker-b")

    def test_scope_checks_both_rename_paths_and_nul_filenames(self):
        (self.repo / "src/content.txt").rename(self.repo / "docs/moved\nname.txt")
        with self.assertRaisesRegex(ValueError, "outside"):
            roles.worker_scope(self.load(), initial=False)

    def test_session_corruption_and_symlinks_are_refused(self):
        self.launch()
        data = state.read_state("worker-a", "codex")
        key = next(iter(data["tasks"]))
        runtime = roles.prepare_runtime("worker-a/tasks/" + key)
        path = next((runtime / "codex/sessions").glob("*.jsonl"))
        original = path.read_text()
        path.write_text(original.replace(str(self.repo), str(self.primary)))
        with self.assertRaisesRegex(ValueError, "origin"):
            self.launch(resume=SESSION, follow_up="next")
        path.write_text(original)
        copy = self.root / "history-copy"
        copy.write_text(original)
        path.unlink()
        path.symlink_to(copy)
        with self.assertRaisesRegex(ValueError, "unsafe"):
            self.launch(resume=SESSION, follow_up="next")

    def test_external_change_during_process_is_not_checkpointed(self):
        real = roles.run_provider
        def run(command, data):
            code = real(command, data)
            (self.repo / "docs/content.txt").write_text("parallel edit")
            return code
        with patch.object(roles, "run_provider", side_effect=run):
            with self.assertRaisesRegex(ValueError, "outside"):
                self.launch()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()

    def test_external_change_during_read_only_worker_keeps_unknown(self):
        self.ticket.update(read_only=True, allowed_directories=[], source_sha256=roles.fingerprint(self.repo))
        real = roles.run_provider
        def run(command, data):
            code = real(command, data)
            (self.repo / "src/content.txt").write_text("parallel edit")
            return code
        with patch.object(roles, "run_provider", side_effect=run):
            with self.assertRaisesRegex(RuntimeError, "source changed during read-only"):
                self.launch()
        with self.assertRaisesRegex(ValueError, "unknown"):
            self.launch()


class RoleStateSchemaTests(unittest.TestCase):
    def test_canonical_uuid_and_hash_are_strict(self):
        for value in (None, "latest", "../x", SESSION.upper()):
            with self.subTest(value=value), self.assertRaises(ValueError):
                state.identity(value)
        self.assertEqual(state.digest({"a": 1, "b": 2}), state.digest({"b": 2, "a": 1}))


if __name__ == "__main__":
    unittest.main()
