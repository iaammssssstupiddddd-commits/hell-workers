from __future__ import annotations

import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination, orca_frontdesk as desk
from scripts import orca_intake_decision as decision


REQUEST = "4252a97e-396b-4779-8ed8-a032f6f0dcda"
WORKSPACE = "68abc67b-ca1b-407b-be63-99dd91321b26"
ISSUE_ID = "d8ce9446-3f9f-44e4-a523-75085033b48a"


class IntakeDecisionTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-intake-decision-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for module in (host_coordination, desk):
            mocked = patch.object(module, "state_root", return_value=self.root / "coordination")
            mocked.start()
            self.addCleanup(mocked.stop)
        desk.submit("表示接続を実装してください", REQUEST)
        self.issue = {"workspaceId": WORKSPACE, "issueId": ISSUE_ID, "identifier": "TAK-14"}

    def test_consultation_never_creates_an_issue(self) -> None:
        result = decision.fixed_reception_decision(self.root, REQUEST, "submit")
        self.assertEqual(result["disposition"], "none")
        self.assertIsNone(result["writeId"])
        self.assertEqual(decision.read(self.root, REQUEST), result)

    def test_fixed_reception_implementation_is_standalone_and_idempotent(self) -> None:
        first = decision.fixed_reception_decision(self.root, REQUEST, "implement")
        second = decision.fixed_reception_decision(self.root, REQUEST, "implement")
        self.assertEqual(first, second)
        self.assertEqual(first["disposition"], "create_standalone")
        self.assertEqual(str(uuid.UUID(first["writeId"])), first["writeId"])

    def test_existing_and_child_decisions_have_distinct_contracts(self) -> None:
        existing = decision.record(
            self.root, REQUEST, "continue_existing", "same objective", "coordinator",
            existing_issue=self.issue,
        )
        self.assertEqual(existing["existingIssue"], self.issue)
        self.assertIsNone(existing["writeId"])

    def test_child_creation_binds_parent_and_write_id(self) -> None:
        child = decision.record(
            self.root, REQUEST, "create_child", "independent deliverable", "coordinator",
            parent_issue=self.issue,
        )
        self.assertEqual(child["parentIssue"], self.issue)
        self.assertIsNotNone(child["writeId"])

    def test_decision_cannot_be_replaced_to_bypass_existing_work(self) -> None:
        decision.fixed_reception_decision(self.root, REQUEST, "implement")
        with self.assertRaisesRegex(ValueError, "immutable"):
            decision.record(
                self.root, REQUEST, "create_child", "try another route", "coordinator",
                parent_issue=self.issue,
            )

    def test_changed_intake_content_invalidates_decision(self) -> None:
        decision.fixed_reception_decision(self.root, REQUEST, "implement")
        path = desk.ledger_path()
        ledger = desk.read_ledger(path)
        ledger["requests"][0]["request"] = "changed"
        desk.write_ledger(path, ledger)
        with self.assertRaisesRegex(ValueError, "digest changed"):
            decision.read(self.root, REQUEST)


if __name__ == "__main__":
    unittest.main()
