from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from scripts import host_coordination, orca_role_tabs as tabs


class RoleTabTests(unittest.TestCase):
    def setUp(self):
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=target)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = str(self.root / "repo")
        self.row = {"handle": "term_fixture", "incarnationId": "instance", "worktreeId": "repo::" + self.repo,
                    "worktreePath": self.repo, "executionHostId": "local", "orphaned": False,
                    "connected": True, "writable": True}
        self.rows = []
        self.commands = []
        for module in (tabs, host_coordination):
            p = patch.object(module, "state_root", return_value=self.root / "coordination")
            p.start()
            self.addCleanup(p.stop)
        p = patch.object(tabs, "idle_shell", return_value={"pid": 2147483647, "start": "1"})
        self.idle = p.start()
        self.addCleanup(p.stop)

    def call(self, cli, argv, operation):
        self.commands.append(operation)
        if operation == "role-tab-list":
            return {"terminals": self.rows, "visualLayouts": [], "truncated": False,
                    "hostScope": {"omittedHostIds": []}}
        if operation == "terminal-create":
            self.rows = [self.row]
            return {"terminal": self.row}
        if operation == "role-tab-show":
            return {"terminal": self.row}
        if operation == "role-tab-send":
            return {"send": {"handle": "term_fixture", "accepted": True}}
        if operation == "role-tab-rename":
            return {}
        if operation == "role-tab-read":
            return {"terminal": {"handle": self.row["handle"], "tail": ["old output"],
                                 "limited": False, "truncated": False}}
        if operation == "role-tab-close":
            self.rows = []
            return {"close": {"handle": self.row["handle"], "ptyKilled": True}}
        raise AssertionError(operation)

    def launch(self):
        return tabs.launch(self.call, Path("orca"), "request", self.repo, "worker-a", "true")

    def test_retry_and_revision_reuse_one_tab(self):
        first = self.launch()
        second = self.launch()
        third = self.launch()
        self.assertEqual(first["handle"], second["handle"])
        self.assertEqual(second["handle"], third["handle"])
        self.assertNotEqual(first["launch_id"], second["launch_id"])
        self.assertEqual(self.commands.count("terminal-create"), 1)
        self.assertEqual(self.commands.count("role-tab-send"), 2)

    def test_busy_shell_never_creates_or_sends(self):
        self.launch()
        self.commands.clear()
        self.idle.side_effect = ValueError("busy")
        with self.assertRaisesRegex(ValueError, "busy"):
            self.launch()
        self.assertEqual(self.commands, ["role-tab-list"])

    def test_changed_incarnation_or_missing_handle_is_not_replaced(self):
        self.launch()
        self.row["incarnationId"] = "new-instance"
        with self.assertRaisesRegex(ValueError, "incarnation"):
            self.launch()
        self.rows = []
        with self.assertRaisesRegex(ValueError, "missing"):
            self.launch()
        self.assertEqual(self.commands.count("terminal-create"), 1)

    def test_unknown_create_blocks_second_allocation(self):
        call = Mock(side_effect=[{"terminals": [], "visualLayouts": [], "truncated": False,
                                "hostScope": {"omittedHostIds": []}}, RuntimeError("timeout")])
        with self.assertRaises(RuntimeError):
            tabs.launch(call, Path("orca"), "request", self.repo, "worker-a", "true")
        with self.assertRaisesRegex(ValueError, "unresolved"):
            self.launch()
        self.assertEqual(self.commands, [])

    def test_reconciled_positive_close_permits_replacement_not_missing_inference(self):
        self.launch()
        self.rows = []
        closed = {"ok": True, "result": {"close": {"handle": "term_other", "ptyKilled": True}}}
        with self.assertRaisesRegex(ValueError, "missing"):
            tabs.launch(self.call, Path("orca"), "request", self.repo, "worker-a", "true", closed_receipt=closed)
        closed["result"]["close"]["handle"] = self.row["handle"]
        tabs.launch(self.call, Path("orca"), "request", self.repo, "worker-a", "true", closed_receipt=closed)
        self.assertEqual(self.commands.count("terminal-create"), 2)
        self.assertEqual(len(self.rows), 1)

    def test_unknown_send_blocks_replay(self):
        self.launch()
        original = self.call

        def call(cli, argv, operation):
            if operation == "role-tab-send":
                return {"send": {"accepted": False}}
            return original(cli, argv, operation)
        with self.assertRaisesRegex(ValueError, "input unknown"):
            tabs.launch(call, Path("orca"), "request", self.repo, "worker-a", "true")
        with self.assertRaisesRegex(ValueError, "unresolved"):
            self.launch()

    def test_stale_completion_does_not_relabel_new_generation(self):
        first = self.launch()
        self.launch()
        call = Mock()
        tabs.label(call, Path("orca"), {"request_id": "request", "repo": self.repo, "slot": "worker-a",
                   "terminal": first["handle"], "tab_launch_id": first["launch_id"]}, "終了")
        call.assert_not_called()

    def test_partial_inventory_and_remote_identity_refused(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            tabs.inventory(Mock(return_value={"terminals": [], "visualLayouts": []}), Path("orca"), self.repo)
        self.row["executionHostId"] = "remote"
        with self.assertRaisesRegex(ValueError, "identity"):
            tabs.identity(self.row, self.repo)

    def test_default_shell_only_no_coordinator_or_split_adoption(self):
        pane = {"type": "terminal", "handle": self.row["handle"]}
        self.assertEqual(tabs.default_shell([self.row], [{"title": "作業シェル", "panes": pane}]), self.row)
        self.assertIsNone(tabs.default_shell([self.row], [{"title": "統括", "panes": pane}]))
        self.assertIsNone(tabs.default_shell([self.row], [{"title": "作業シェル", "panes": {"type": "split"}}]))

    def test_empty_authoritative_inventory_may_omit_visual_layouts(self):
        def empty(_cli, _argv, _purpose):
            return {"terminals": [], "totalCount": 0, "truncated": False,
                    "hostScope": {"omittedHostIds": []}}
        self.assertEqual(tabs.inventory(empty, Path("orca"), self.repo), ([], []))

    def test_explicit_adoption_preserves_identity_and_refuses_overwrite(self):
        self.rows = [self.row]
        owned = tabs.identity(self.row, self.repo)
        tabs.adopt(self.call, Path("orca"), "request", self.repo, "worker-a", owned)
        self.launch()
        self.assertNotIn("terminal-create", self.commands)
        with self.assertRaisesRegex(ValueError, "overwrite"):
            tabs.adopt(self.call, Path("orca"), "request", self.repo, "worker-a", owned)

    def test_retire_archives_output_and_never_replays_close(self):
        self.rows = [self.row]
        owned = tabs.identity(self.row, self.repo)
        path = tabs.retire(self.call, Path("orca"), self.repo, owned)
        self.assertEqual(tabs.storage.read_private_json(path, {})["output"]["tail"], ["old output"])
        self.assertEqual(self.commands.count("role-tab-close"), 1)
        with self.assertRaisesRegex(ValueError, "already attempted"):
            tabs.retire(self.call, Path("orca"), self.repo, owned)
        self.assertEqual(self.commands.count("role-tab-close"), 1)

    def test_shell_stopped_rejects_invalid_journal_identity(self):
        with self.assertRaisesRegex(ValueError, "incomplete"):
            tabs.shell_stopped({"pid": 42})

    def test_retire_pages_a_character_limited_preview(self):
        self.rows = [self.row]
        owned = tabs.identity(self.row, self.repo)
        original = self.call

        def paged(cli, argv, operation):
            if operation == "role-tab-read":
                if "--cursor" in argv:
                    return {"terminal": {"handle": owned["handle"], "tail": ["one", "two"],
                                         "truncated": False, "latestCursor": "2",
                                         "nextCursor": "2", "returnedLineCount": 2}}
                return {"terminal": {"handle": owned["handle"], "tail": ["two"],
                                     "limited": True, "truncated": False,
                                     "oldestCursor": "0", "latestCursor": "2"}}
            return original(cli, argv, operation)

        path = tabs.retire(paged, Path("orca"), self.repo, owned)
        self.assertEqual(tabs.storage.read_private_json(path, {})["output"]["tail"], ["one", "two"])
        self.assertEqual(self.commands.count("role-tab-close"), 1)

    def test_retire_preserves_entire_retained_range_and_marks_prior_ring_eviction(self):
        preview = {'handle': 'term_fixture', 'tail': ['two'], 'limited': True,
                   'truncated': True, 'oldestCursor': '400', 'latestCursor': '402'}
        page = {**preview, 'tail': ['one', 'two'], 'truncated': False,
                'nextCursor': '402', 'returnedLineCount': 2}
        call = Mock(side_effect=[{'terminal': preview}, {'terminal': page}, {'terminal': preview}])
        result = tabs.complete_output(call, Path('orca'), 'term_fixture')
        self.assertEqual(result['tail'], ['one', 'two'])
        self.assertTrue(result['retainedRangeComplete'])
        self.assertTrue(result['truncated'])
        self.assertEqual(result['droppedBeforeCursor'], '400')
        self.assertIn('400', call.call_args_list[1].args[1])
        moved = {**preview, 'oldestCursor': '401'}
        call = Mock(side_effect=[{'terminal': preview}, {'terminal': page}, {'terminal': moved}])
        with self.assertRaisesRegex(ValueError, 'changed'):
            tabs.complete_output(call, Path('orca'), 'term_fixture')

    def test_retire_refuses_busy_or_unpreserved_output(self):
        owned = tabs.identity(self.row, self.repo)
        self.idle.side_effect = ValueError("busy")
        with self.assertRaisesRegex(ValueError, "busy"):
            tabs.retire(self.call, Path("orca"), self.repo, owned)
        self.assertNotIn("role-tab-close", self.commands)
        self.idle.side_effect = None
        original = self.call

        def limited(cli, argv, operation):
            if operation == "role-tab-read":
                return {"terminal": {"handle": owned["handle"], "limited": True, "truncated": False}}
            return original(cli, argv, operation)
        with self.assertRaisesRegex(ValueError, "incomplete"):
            tabs.retire(limited, Path("orca"), self.repo, owned)
        self.assertNotIn("role-tab-close", self.commands)

    def test_default_title_without_ownership_record_is_not_adopted(self):
        self.rows = [self.row]
        original = self.call

        def call(cli, argv, operation):
            result = original(cli, argv, operation)
            if operation == "role-tab-list":
                result["visualLayouts"] = [{"title": "作業シェル", "panes": {
                    "type": "terminal", "handle": self.row["handle"]}}]
            return result
        tabs.launch(call, Path("orca"), "request", self.repo, "worker-a", "true")
        self.assertIn("terminal-create", self.commands)
        self.assertNotIn("role-tab-send", self.commands)

    def test_owned_default_shell_is_adopted_and_unmanaged_role_refused(self):
        self.rows = [self.row]
        original = self.call
        title = "作業シェル"

        def call(cli, argv, operation):
            result = original(cli, argv, operation)
            if operation == "role-tab-list":
                result["visualLayouts"] = [{"title": title, "panes": {
                    "type": "terminal", "handle": self.row["handle"]}}]
            return result
        tabs.record_default(self.row, self.repo)
        tabs.launch(call, Path("orca"), "request", self.repo, "worker-a", "true")
        self.assertNotIn("terminal-create", self.commands)
        title = tabs.TITLES["worker-a"]
        with self.assertRaisesRegex(ValueError, "unregistered"):
            tabs.launch(call, Path("orca"), "another-request", self.repo, "worker-a", "true")


class ShellProbeTests(unittest.TestCase):
    def test_fixture_foreground_shell_and_child_rejection(self):
        with tempfile.TemporaryDirectory() as directory:
            proc = Path(directory)
            shell = proc / "42"
            shell.mkdir()
            # /proc/stat fields 3..22, including start time.
            fields = ["S", "1", "42", "42", "100", "42"] + ["0"] * 13 + ["123"]
            (shell / "stat").write_text("42 (bash) " + " ".join(fields))
            (shell / "comm").write_text("bash\n")
            (shell / "environ").write_bytes(b"ORCA_TERMINAL_HANDLE=term_fixture\0")
            (shell / "cwd").symlink_to(proc, target_is_directory=True)
            self.assertEqual(tabs.idle_shell("term_fixture", str(proc), proc)["pid"], 42)
            child = proc / "43"
            child.mkdir()
            (child / "stat").write_text("43 (codex) " + " ".join(fields))
            (child / "environ").write_bytes(b"")
            with self.assertRaisesRegex(ValueError, "idle"):
                tabs.idle_shell("term_fixture", str(proc), proc)


if __name__ == "__main__":
    unittest.main()
