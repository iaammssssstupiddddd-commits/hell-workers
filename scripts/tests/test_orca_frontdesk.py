from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination, orca_frontdesk as desk


class FrontdeskTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for module in (host_coordination, desk):
            mock = patch.object(module, "state_root", return_value=self.root / "coordination")
            mock.start()
            self.addCleanup(mock.stop)

    def test_submit_is_persistent_and_not_dispatched(self) -> None:
        item = desk.submit("独立したtestを追加。ゲーム仕様は変更しない。")
        self.assertEqual(desk.list_requests(), [item])
        self.assertEqual(item["status"], "queued")
        self.assertIsNone(item["task_id"])
        self.assertIsNone(item["coordinator_session"])
        self.assertEqual(desk.ledger_path().stat().st_mode & 0o777, 0o600)
        self.assertEqual(item["routing"]["worker-b"], "cursor-simple-only")

    def test_same_request_id_deduplicates_and_different_body_refuses(self) -> None:
        identity = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        first = desk.submit("task", identity)
        self.assertEqual(desk.submit("task", identity), first)
        self.assertEqual(len(desk.list_requests()), 1)
        with self.assertRaisesRegex(ValueError, "different request"):
            desk.submit("different", identity)

    def test_invalid_input_changes_nothing(self) -> None:
        for text in (" ", "x" * 32001):
            with self.assertRaises(ValueError):
                desk.submit(text)
        with self.assertRaises(ValueError):
            desk.submit("valid text", "not-a-uuid")
        self.assertEqual(desk.list_requests(), [])

    def test_corrupt_state_is_not_reset(self) -> None:
        path = desk.ledger_path()
        path.write_text("corrupt")
        path.chmod(0o600)
        with self.assertRaises(ValueError):
            desk.submit("do not reset")
        self.assertEqual(path.read_text(), "corrupt")

    def test_symlink_and_hardlink_ledgers_refuse(self) -> None:
        original = self.root / "original"
        original.write_text('{}')
        original.chmod(0o600)
        path = desk.ledger_path()
        path.symlink_to(original)
        with self.assertRaisesRegex(RuntimeError, "unsafe intake"):
            desk.submit("task")
        path.unlink()
        os.link(original, path)
        with self.assertRaisesRegex(RuntimeError, "unsafe intake"):
            desk.submit("task")
        self.assertEqual(original.read_text(), '{}')

    def test_atomic_failure_preserves_previous_ledger(self) -> None:
        first = desk.submit("first")
        with patch.object(desk.os, "replace", side_effect=OSError("simulated interruption")):
            with self.assertRaises(OSError):
                desk.submit("second")
        self.assertEqual(desk.list_requests(), [first])
        self.assertEqual(list(desk.ledger_path().parent.glob(".requests-*")), [])

    def test_unknown_or_duplicate_state_is_not_success(self) -> None:
        first = desk.submit("task")
        for entries in ([first, first], [{**first, "status": "completed"}]):
            desk.ledger_path().write_text(json.dumps({"schema": 1, "requests": entries}))
            with self.assertRaisesRegex(ValueError, "unknown/duplicate"):
                desk.list_requests()

    def test_busy_frontdesk_state_refuses_without_retry(self) -> None:
        with host_coordination.acquire_host("frontdesk-state", inherit=False):
            with self.assertRaises(host_coordination.HostBusyError):
                desk.submit("task")

    def test_declining_menu_start_does_not_start_coordinator(self) -> None:
        item = desk.submit("task")
        with patch("builtins.input", side_effect=[item["id"], "no"]), patch.object(desk, "coordinator_module") as module:
            desk.menu_action("3")
            module.return_value.consult.assert_not_called()

    def test_invalid_content_timestamp_and_dispatch_identity_preserved(self) -> None:
        first = desk.submit("task")
        for change in ({"request": " "}, {"request": 1}, {"created_at": "invalid"},
                       {"created_at": "2026-09-20T00:00:00"}, {"task_id": "unexpected"}):
            with self.subTest(change=change):
                raw = json.dumps({"schema": 1, "requests": [{**first, **change}]})
                desk.ledger_path().write_text(raw)
                with self.assertRaises(ValueError):
                    desk.list_requests()
                self.assertEqual(desk.ledger_path().read_text(), raw)

    def test_post_replace_sync_failure_retries_same_id_without_duplicate(self) -> None:
        identity = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        real_sync = os.fsync
        calls = 0

        def fail_directory_sync(fd):
            nonlocal calls
            calls += 1
            if calls == 2:
                raise OSError("directory sync interrupted after replace")
            real_sync(fd)

        with patch.object(desk.os, "fsync", side_effect=fail_directory_sync):
            with self.assertRaises(OSError):
                desk.submit("task", identity)
        with patch.object(desk.os, "fsync", wraps=real_sync) as sync:
            item = desk.submit("task", identity)
            sync.assert_called_once()
        self.assertEqual(desk.list_requests(), [item])


if __name__ == "__main__":
    unittest.main()
