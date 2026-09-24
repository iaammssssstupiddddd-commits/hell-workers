from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination
from scripts import orca_external_sync as sync


REQUEST = "4252a97e-396b-4779-8ed8-a032f6f0dcda"


class ExternalSyncTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-external-sync-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        mocked = patch.object(host_coordination, "state_root", return_value=self.root / "coordination")
        mocked.start()
        self.addCleanup(mocked.stop)

    def test_linear_queue_is_idempotent_and_ordered(self) -> None:
        first = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手")
        self.assertEqual(first, sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手"))
        second = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "review", "レビュー待ち")
        self.assertEqual((first["sequence"], second["sequence"]), (1, 2))
        with self.assertRaisesRegex(ValueError, "stale"):
            sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "巻き戻し")

    def test_github_requires_same_tested_and_reviewed_head(self) -> None:
        with self.assertRaisesRegex(ValueError, "same tested"):
            sync.queue_github_draft(
                self.root, REQUEST, branch="candidate", base="a" * 40, head="b" * 40,
                tested_sha="c" * 40, review_sha="b" * 40, publication_authorized=True,
            )

    def test_missing_publication_authority_records_a_visible_block(self) -> None:
        operation = sync.queue_github_draft(
            self.root, REQUEST, branch="candidate", base="a" * 40, head="b" * 40,
            tested_sha="b" * 40, review_sha="b" * 40, publication_authorized=False,
        )
        self.assertEqual(operation["phase"], "blocked_authority")
        self.assertEqual(sync.projection(self.root, REQUEST)["state"], "blocked_authority")

    def test_unknown_write_cannot_be_requeued_with_a_new_identity(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手")
        sync.transition(self.root, REQUEST, operation["id"], "sending")
        sync.transition(self.root, REQUEST, operation["id"], "unknown", {"readBack": "required"})
        again = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手")
        self.assertEqual(again["id"], operation["id"])
        self.assertEqual(again["phase"], "unknown")

    def test_external_event_is_only_a_proposal(self) -> None:
        proposal = sync.propose(self.root, REQUEST, "linear", "status_changed", {"to": "Done"})
        self.assertEqual(proposal["phase"], "proposed")
        self.assertEqual(sync.projection(self.root, REQUEST)["state"], "not_required")


if __name__ == "__main__":
    unittest.main()
