from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination, orca_frontdesk as desk
from scripts import orca_supervision_route as routing


REQUEST = "4252a97e-396b-4779-8ed8-a032f6f0dcda"
WORKSPACE = "68abc67b-ca1b-407b-be63-99dd91321b26"
REPO = "88983a13-d1dd-47a0-8ffb-3772ea15d3f1"
ISSUE = "d8ce9446-3f9f-44e4-a523-75085033b48a"
RUNTIME = "80ce35e7-20cd-48c3-af6b-73b2ccc17d23"
CHILD = "6c42fd6e-1930-4d79-a3e5-845ccf478b75"


class RouteTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-route-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.panel = self.root / "panel"
        self.primary = self.root / "primary"
        for module in (host_coordination, desk):
            mock = patch.object(module, "state_root", return_value=self.root / "coordination")
            mock.start()
            self.addCleanup(mock.stop)
        desk.submit("保存領域の修正を実装してください", REQUEST)
        desk.write_ledger(desk.checked_directory(self.panel / "receipts") / f"{REQUEST}.json", {
            "schema": 1, "operationId": REQUEST, "intakeId": REQUEST,
            "action": "implement", "phase": "accepted",
            "text": "保存領域の修正を実装してください"})
        self.calls = []
        self.fail_on = None
        imported = patch.object(routing.intake, "import_issue", return_value={"request_id": CHILD})
        imported.start()
        self.addCleanup(imported.stop)
        coordinator_state = patch.object(routing.ui, "state_path", return_value=self.root / "missing-state.json")
        coordinator_state.start()
        self.addCleanup(coordinator_state.stop)
        coordinator_launch = patch.object(routing.ui, "ensure_coordinator_terminal", return_value="term_fixture")
        self.coordinator_launch = coordinator_launch.start()
        self.addCleanup(coordinator_launch.stop)

    def call(self, args: list[str], text: str | None = None) -> tuple[int, dict]:
        self.calls.append((args, text))
        if self.fail_on and args[:len(self.fail_on)] == self.fail_on:
            return 1, {"ok": False, "error": {"code": "unconfirmed"}}
        meta = {"runtimeId": RUNTIME}
        if args[:3] == ["linear", "team", "list"]:
            return 0, {"ok": True, "result": {"teams": [{"key": "TAK", "workspace": {"id": WORKSPACE}}]}, "_meta": meta}
        if args[:2] == ["repo", "list"]:
            return 0, {"ok": True, "result": {"repos": [{"id": REPO, "path": str(self.primary)}]}, "_meta": meta}
        if args[:2] == ["linear", "create"]:
            write_id = args[args.index("--write-id") + 1]
            return 0, {"ok": True, "result": {"issue": {
                "id": ISSUE, "identifier": "TAK-99", "title": args[args.index("--title") + 1],
                "team": {"key": "TAK"}},
                "meta": {"workspaceId": WORKSPACE, "writeId": write_id}}, "_meta": meta}
        if args[:2] == ["worktree", "create"]:
            return 0, {"ok": True, "result": {"worktree": {
                "id": REPO + "::" + str(self.root / "tak-99"), "linkedLinearIssue": "TAK-99"}},
                "_meta": meta}
        raise AssertionError(f"unexpected Orca call: {args}")

    def test_one_issue_and_worktree_then_exact_journal_replay(self) -> None:
        first = routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(first["phase"], "terminal_starting")
        self.assertEqual(first["issueIdentifier"], "TAK-99")
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(self.calls[2][0][:2], ["linear", "create"])
        self.assertEqual(self.calls[2][1], "保存領域の修正を実装してください")
        self.assertEqual(self.calls[3][0][:2], ["worktree", "create"])
        self.coordinator_launch.assert_called_once_with(first["worktreeId"], focus=False)
        second = routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(second, first)
        self.assertEqual(len(self.calls), 4)
        self.coordinator_launch.assert_called_once()

    def test_only_the_matching_ready_coordinator_completes_route(self) -> None:
        first = routing.route(self.panel, REQUEST, self.primary, self.call)
        state_path = self.root / "coordinator.json"
        state_path.write_text("registered", encoding="utf-8")
        with patch.object(routing.ui, "state_path", return_value=state_path), \
                patch.object(routing.ui, "read_registered_state", return_value={
                    "worktree_id": first["worktreeId"], "linear_identifier": "TAK-99",
                    "phase": "ready"}):
            ready = routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(ready["phase"], "ready")
        self.assertEqual(len(self.calls), 4)
        self.coordinator_launch.assert_called_once()

    def test_unconfirmed_terminal_launch_does_not_launch_twice(self) -> None:
        self.coordinator_launch.side_effect = RuntimeError("terminal creation unconfirmed")
        with self.assertRaisesRegex(RuntimeError, "unconfirmed"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(desk.read_private_json(routing.route_path(self.panel, REQUEST), {})["phase"],
                         "terminal_starting")
        self.coordinator_launch.side_effect = None
        routing.route(self.panel, REQUEST, self.primary, self.call)
        self.coordinator_launch.assert_called_once()

    def test_unconfirmed_linear_write_is_not_retried(self) -> None:
        self.fail_on = ["linear", "create"]
        with self.assertRaisesRegex(RuntimeError, "unconfirmed"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        count = len(self.calls)
        with self.assertRaisesRegex(RuntimeError, "unresolved"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(len(self.calls), count)
        self.assertEqual(desk.read_private_json(routing.route_path(self.panel, REQUEST), {})["phase"],
                         "issue_creating")

    def test_unconfirmed_worktree_write_is_not_retried(self) -> None:
        self.fail_on = ["worktree", "create"]
        with self.assertRaisesRegex(RuntimeError, "unconfirmed"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        count = len(self.calls)
        with self.assertRaisesRegex(RuntimeError, "unresolved"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(len(self.calls), count)
        self.assertEqual(desk.read_private_json(routing.route_path(self.panel, REQUEST), {})["phase"],
                         "worktree_creating")

    def test_consultation_receipt_cannot_create_an_issue(self) -> None:
        receipt_path = self.panel / "receipts" / f"{REQUEST}.json"
        receipt = desk.read_private_json(receipt_path, {})
        desk.write_ledger(receipt_path, {**receipt, "action": "submit"})
        with self.assertRaisesRegex(ValueError, "explicit UI implementation intent"):
            routing.route(self.panel, REQUEST, self.primary, self.call)
        self.assertEqual(self.calls, [])


if __name__ == "__main__":
    unittest.main()
