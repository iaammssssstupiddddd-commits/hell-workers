from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination, orca_frontdesk as desk, orca_coordinator as coordinator
from scripts import orca_roles as roles


SESSION = "01995c6d-7536-7595-bdd0-f693f398db23"
OTHER = "01995c6d-7536-7595-bdd0-f693f398db24"
FAKE = r'''
import json, os, pathlib, sys, time
session, repo, mode, control = sys.argv[1:]
prompt = sys.stdin.read()
home = pathlib.Path(os.environ['CODEX_HOME'])
sessions = home / 'sessions'
sessions.mkdir(exist_ok=True)
(sessions / ('rollout-' + session + '.jsonl')).write_text(json.dumps({
  'type': 'session_meta', 'payload': {'id': session, 'cwd': repo}}) + '\n')
if mode == 'malformed':
    print('not json', flush=True)
    sys.exit(0)
if mode == 'truncated':
    print('{"type":', end='', flush=True)
    sys.exit(0)
if mode == 'oversized':
    print('x' * 5000, flush=True)
    sys.exit(0)
if mode == 'timeout':
    time.sleep(20)
    sys.exit(0)
if mode != 'missing-thread':
    print(json.dumps({'type': 'thread.started', 'thread_id': session}), flush=True)
if mode == 'duplicate-thread':
    print(json.dumps({'type': 'thread.started', 'thread_id': session}), flush=True)
print(json.dumps({'type': 'turn.started'}), flush=True)
if mode == 'duplicate-start':
    print(json.dumps({'type': 'turn.started'}), flush=True)
if mode == 'readonly':
    for path in (pathlib.Path(repo) / 'src/content.txt', pathlib.Path(control)):
        try:
            path.write_text('escape')
        except OSError:
            continue
        raise AssertionError('write escaped isolation')
if mode == 'failed':
    print(json.dumps({'type': 'turn.failed'}), flush=True)
    sys.exit(1)
print(json.dumps({'type': 'item.completed', 'item': {'type': 'agent_message', 'text': '相談のみ・未dispatch'}}), flush=True)
if mode != 'missing-completed':
    print(json.dumps({'type': 'turn.completed'}), flush=True)
if mode == 'after-completed':
    print(json.dumps({'type': 'turn.started'}), flush=True)
sys.exit(9 if mode == 'bad-exit' else 0)
'''


@unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap"), "Linux bubblewrap required")
class CoordinatorTests(unittest.TestCase):
    def setUp(self):
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.primary = self.root / "primary"
        self.primary.mkdir()
        self.git(self.primary, "init", "-q")
        (self.primary / "src").mkdir()
        (self.primary / "src/content.txt").write_text("original")
        self.git(self.primary, "add", ".")
        self.git(self.primary, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                 "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
        self.repo = self.root / "candidate"
        self.git(self.primary, "worktree", "add", "-qb", "task", str(self.repo))
        self.mocks = []
        for module in (desk, host_coordination, roles):
            self.start_patch(patch.object(module, "state_root", return_value=self.root / "state/coordination"))
        self.start_patch(patch.object(coordinator, "REPO", self.repo))
        self.calls = []
        self.mode = "readonly"
        self.start_patch(patch.object(coordinator, "provider_command", side_effect=self.provider))
        self.item = desk.submit("環境のread-only相談。実装しない。")

    def start_patch(self, mock):
        result = mock.start()
        self.addCleanup(mock.stop)
        return result

    @staticmethod
    def git(repo, *args):
        subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)

    def provider(self, session):
        self.calls.append(session)
        return [sys.executable, "-u", "-c", FAKE, session or SESSION,
                str(self.repo), self.mode, str(coordinator.state_path(self.item["id"]))]

    def test_initial_and_followup_use_same_session_and_never_dispatch(self):
        before = roles.fingerprint(self.repo)
        first = coordinator.consult(self.item["id"])
        self.assertEqual(first["phase"], "succeeded")
        self.assertEqual(first, coordinator.consult(self.item["id"]))
        turn_id = str(uuid.uuid4())
        followup = coordinator.consult(self.item["id"], "前回の相談を継続", turn_id)
        self.assertEqual(self.calls, [None, SESSION])
        self.assertEqual(coordinator.consult(self.item["id"], "前回の相談を継続", turn_id), followup)
        self.assertEqual(coordinator.read_state(self.item["id"])["session_id"], SESSION)
        self.assertEqual(desk.list_requests(), [self.item])
        self.assertEqual(roles.fingerprint(self.repo), before)
        self.assertEqual(coordinator.state_path(self.item["id"]).stat().st_mode & 0o777, 0o600)

    def test_different_message_reusing_turn_id_is_rejected(self):
        first = coordinator.consult(self.item["id"])
        with self.assertRaisesRegex(ValueError, "different message"):
            coordinator.consult(self.item["id"], "different", first["id"])

    def test_failed_or_unproven_turn_stays_unknown_and_is_not_resent(self):
        for mode in ("malformed", "truncated", "missing-completed", "failed", "bad-exit"):
            with self.subTest(mode=mode):
                self.item = desk.submit(mode)
                self.mode = mode
                with self.assertRaises((RuntimeError, ValueError)):
                    coordinator.consult(self.item["id"])
                state = coordinator.read_state(self.item["id"])
                self.assertEqual(state["turns"][-1]["phase"], "unknown")
                count = len(self.calls)
                with self.assertRaisesRegex(RuntimeError, "never automatically resend"):
                    coordinator.consult(self.item["id"])
                self.assertEqual(len(self.calls), count)

    def test_oversized_event_refuses_without_unbounded_buffering(self):
        self.mode = "oversized"
        with patch.object(coordinator, "MAX_EVENT_BYTES", 1024):
            with self.assertRaisesRegex(ValueError, "oversized"):
                coordinator.consult(self.item["id"])

    def test_identity_save_failure_preserves_unknown_and_reaps_child(self):
        real_save = coordinator.save_state
        calls = 0

        def interrupt_identity(data):
            nonlocal calls
            calls += 1
            if calls == 2:
                raise OSError("identity write interrupted")
            real_save(data)

        with patch.object(coordinator, "save_state", side_effect=interrupt_identity):
            with self.assertRaisesRegex(OSError, "identity write"):
                coordinator.consult(self.item["id"])
        data = coordinator.read_state(self.item["id"])
        self.assertEqual(data["session_id"], SESSION)
        self.assertEqual(data["turns"][-1]["phase"], "unknown")
        self.assertTrue(data["turns"][-1]["process_exited"])

    def test_final_save_failure_does_not_relaunch_an_uncertain_turn(self):
        real_save = coordinator.save_state

        def interrupt_final(data):
            if data["turns"][-1]["phase"] == "succeeded":
                raise OSError("final write interrupted")
            real_save(data)

        with patch.object(coordinator, "save_state", side_effect=interrupt_final):
            with self.assertRaises(OSError):
                coordinator.consult(self.item["id"])
        self.assertEqual(coordinator.read_state(self.item["id"])["turns"][-1]["phase"], "running")
        with self.assertRaisesRegex(RuntimeError, "never automatically resend"):
            coordinator.consult(self.item["id"])
        self.assertEqual(self.calls, [None])

    def test_followup_prompt_does_not_resend_original_request(self):
        initial = coordinator.prompt_for(self.item, self.item["request"], {}, initial=True)
        followup = coordinator.prompt_for(self.item, "追加の質問", {}, initial=False)
        self.assertIn(self.item["request"], initial)
        self.assertNotIn(self.item["request"], followup)

    def test_wrong_thread_on_resume_fails_closed(self):
        coordinator.consult(self.item["id"])
        self.calls.clear()
        with patch.object(coordinator, "provider_command", return_value=self.provider(OTHER)):
            with self.assertRaisesRegex(ValueError, "different session"):
                coordinator.consult(self.item["id"], "follow up", str(uuid.uuid4()))

    def test_resume_requires_this_attempts_ordered_identity_and_events(self):
        for mode in ("missing-thread", "duplicate-thread", "duplicate-start", "after-completed"):
            with self.subTest(mode=mode):
                self.item = desk.submit(mode)
                self.mode = "readonly"
                coordinator.consult(self.item["id"])
                self.mode = mode
                with self.assertRaises(ValueError):
                    coordinator.consult(self.item["id"], "follow up", str(uuid.uuid4()))
                self.assertEqual(coordinator.read_state(self.item["id"])["turns"][-1]["phase"], "unknown")

    def test_recorded_success_without_identity_is_not_trusted(self):
        coordinator.consult(self.item["id"])
        path = coordinator.state_path(self.item["id"])
        data = json.loads(path.read_text())
        data["turns"][0]["thread_seen"] = False
        path.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, "unproven"):
            coordinator.consult(self.item["id"])

    def test_branch_change_does_not_reuse_previous_conversation(self):
        coordinator.consult(self.item["id"])
        self.git(self.repo, "switch", "-c", "different")
        with self.assertRaisesRegex(ValueError, "branch/common"):
            coordinator.consult(self.item["id"], "follow up", str(uuid.uuid4()))

    def test_missing_session_does_not_fall_back_to_new_session(self):
        coordinator.consult(self.item["id"])
        with patch.object(coordinator, "session_exists", side_effect=ValueError("session missing")):
            with self.assertRaisesRegex(ValueError, "session missing"):
                coordinator.consult(self.item["id"], "follow up", str(uuid.uuid4()))
        self.assertEqual(self.calls, [None])

    def test_explicit_recovery_is_a_new_turn_in_same_session_after_proven_exit(self):
        self.mode = "failed"
        with self.assertRaises(RuntimeError):
            coordinator.consult(self.item["id"])
        failed = coordinator.read_state(self.item["id"])["turns"][-1]
        self.assertTrue(failed["process_exited"])
        self.mode = "readonly"
        result = coordinator.consult(self.item["id"], "前回の到達点だけ確認", str(uuid.uuid4()), recover=True)
        self.assertEqual(result["recovery_of"], failed["id"])
        self.assertEqual(self.calls, [None, SESSION])
        self.assertEqual(result["phase"], "succeeded")

    def test_recovery_without_positive_process_exit_is_refused(self):
        self.mode = "failed"
        with self.assertRaises(RuntimeError):
            coordinator.consult(self.item["id"])
        data = coordinator.read_state(self.item["id"])
        data["turns"][-1]["process_exited"] = False
        coordinator.save_state(data)
        with self.assertRaisesRegex(RuntimeError, "proven child exit"):
            coordinator.consult(self.item["id"], "前回を確認", str(uuid.uuid4()), recover=True)
        self.assertEqual(self.calls, [None])

    def test_busy_slot_does_not_spawn_or_block_intake(self):
        with host_coordination.acquire_host("coordinator", inherit=False):
            with self.assertRaises(host_coordination.HostBusyError):
                coordinator.consult(self.item["id"])
            desk.submit("intake still accepts")
        self.assertEqual(self.calls, [])
        self.assertEqual(len(desk.list_requests()), 2)

    def test_timeout_reaps_child_before_releasing_slot(self):
        self.mode = "timeout"
        children = []
        real_popen = subprocess.Popen

        def spawn(*args, **kwargs):
            child = real_popen(*args, **kwargs)
            children.append(child)
            return child

        with patch.object(coordinator, "TURN_TIMEOUT", 0.05), patch.object(coordinator.subprocess, "Popen", spawn):
            with self.assertRaises(TimeoutError):
                coordinator.consult(self.item["id"])
        self.assertTrue(all(child.poll() is not None for child in children))
        with host_coordination.acquire_host("coordinator", inherit=False):
            pass

    def test_spawn_error_preserves_intent_without_retry(self):
        with patch.object(coordinator, "execute", side_effect=OSError("spawn failed")):
            with self.assertRaises(OSError):
                coordinator.consult(self.item["id"])
        state = coordinator.read_state(self.item["id"])
        self.assertIsNone(state["session_id"])
        self.assertEqual(state["turns"][0]["phase"], "unknown")

    def test_unrecorded_runtime_and_corrupt_state_are_not_reset(self):
        runtime = roles.prepare_runtime(f"coordinator/{self.item['id']}")
        (runtime / "codex/sessions").mkdir()
        (runtime / "codex/sessions/orphan.jsonl").write_text("orphan")
        with self.assertRaisesRegex(ValueError, "unrecorded session"):
            coordinator.consult(self.item["id"])
        path = coordinator.state_path(self.item["id"])
        path.write_text("corrupt")
        path.chmod(0o600)
        with self.assertRaises(ValueError):
            coordinator.consult(self.item["id"])
        self.assertEqual(path.read_text(), "corrupt")

    def test_source_change_is_explicit_not_an_approval(self):
        real_execute = coordinator.execute

        def mutate_after(*args):
            code = real_execute(*args)
            (self.repo / "src/content.txt").write_text("parallel change")
            return code

        with patch.object(coordinator, "execute", side_effect=mutate_after):
            turn = coordinator.consult(self.item["id"])
        self.assertTrue(turn["source_changed"])
        self.assertEqual(desk.list_requests()[0]["status"], "queued")


class CoordinatorCommandTests(unittest.TestCase):
    @patch("scripts.orca_coordinator.shutil.which", return_value="/bin/codex")
    def test_exact_resume_and_readonly_flags(self, _):
        command = coordinator.provider_command(SESSION)
        self.assertLess(command.index("--sandbox"), command.index("resume"))
        self.assertIn("read-only", command)
        self.assertIn(SESSION, command)
        for flag in ("--last", "--ephemeral", "--dangerously-bypass-approvals-and-sandbox", "--model"):
            self.assertNotIn(flag, command)


if __name__ == "__main__":
    unittest.main()
