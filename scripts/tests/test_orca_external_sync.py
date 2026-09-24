from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import host_coordination
from scripts import orca_external_sync as sync


REQUEST = "4252a97e-396b-4779-8ed8-a032f6f0dcda"
SENT = {"exitCode": 0, "errorCode": None, "definitive": False}


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
        self.cli = self.root / "orca-ide"
        self.cli.touch()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        (self.repo / ".git").write_text("gitdir: fixture\n")

    def test_linear_queue_is_idempotent_and_ordered(self) -> None:
        first = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手")
        self.assertEqual(first, sync.queue_linear_status(self.root, REQUEST, "TAK-14", "started", "着手"))
        second = sync.queue_linear_status(self.root, REQUEST, "TAK-14", "review", "レビュー待ち")
        self.assertEqual((first["sequence"], second["sequence"]), (1, 2))
        self.assertEqual(first["payload"]["targetState"], "In Progress")
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

    def test_executor_sends_once_then_requires_read_back(self) -> None:
        operation = sync.queue_linear_comment(self.root, REQUEST, "TAK-99", "完了しました")
        verified = {"provider": "linear", "issue": "TAK-99", "commentId": "comment-1"}
        with patch.object(sync, "_read_back", side_effect=[None, verified]) as read_back, \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=SENT) as send:
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "confirmed")
        self.assertEqual(result["operation"]["id"], operation["id"])
        self.assertEqual(read_back.call_count, 2)
        send.assert_called_once()

    def test_unknown_operation_is_read_back_but_never_replayed(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        sync.transition(self.root, REQUEST, operation["id"], "sending")
        sync.transition(self.root, REQUEST, operation["id"], "unknown", {"readBack": "required"})
        with patch.object(sync, "_read_back", return_value=None), \
                patch.object(sync, "_send") as send:
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "unknown")
        send.assert_not_called()

    def test_unknown_operation_can_be_confirmed_by_later_read_back(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        sync.transition(self.root, REQUEST, operation["id"], "sending")
        sync.transition(self.root, REQUEST, operation["id"], "unknown", {"readBack": "required"})
        verified = {"provider": "linear", "issue": "TAK-99", "state": "In Progress"}
        with patch.object(sync, "_read_back", return_value=verified), \
                patch.object(sync, "_send") as send:
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "confirmed")
        send.assert_not_called()

    def test_unknown_linear_status_blocks_a_new_status_identity(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        sync.transition(self.root, REQUEST, operation["id"], "sending")
        sync.transition(self.root, REQUEST, operation["id"], "unknown", {"readBack": "required"})
        with self.assertRaisesRegex(ValueError, "must be reconciled"):
            sync.queue_linear_status(self.root, REQUEST, "TAK-99", "review", "レビュー")

    def test_executor_processes_only_the_oldest_operation(self) -> None:
        first = sync.queue_linear_comment(self.root, REQUEST, "TAK-99", "first")
        sync.queue_linear_comment(self.root, REQUEST, "TAK-99", "second")
        verified = {"provider": "linear", "issue": "TAK-99", "commentId": "comment-1"}
        with patch.object(sync, "_read_back", side_effect=[None, verified]), \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=SENT):
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["operation"]["id"], first["id"])
        ledger = sync.read(self.root, REQUEST)
        self.assertEqual([item["phase"] for item in ledger["operations"]],
                         ["confirmed", "queued"])

    def test_draft_executor_metadata_is_all_or_nothing(self) -> None:
        with self.assertRaisesRegex(ValueError, "metadata must be complete"):
            sync.queue_github_draft(
                self.root, REQUEST, branch="candidate", base="a" * 40,
                head="b" * 40, tested_sha="b" * 40, review_sha="b" * 40,
                publication_authorized=True, repository="owner/repo",
            )

    def test_linear_comment_uses_marker_not_a_fresh_provider_retry_id(self) -> None:
        operation = sync.queue_linear_comment(self.root, REQUEST, "TAK-99", "body")
        with patch.object(sync, "_run", return_value=(0, {"ok": True})) as run:
            self.assertEqual(sync._send(operation, self.cli, self.repo), SENT)
        command = run.call_args.args[0]
        self.assertNotIn("--write-id", command)
        self.assertIn(f"<!-- orca-op:{operation['id']} -->", run.call_args.kwargs["stdin"])

    def test_blocked_draft_can_be_authorized_once_with_a_receipt(self) -> None:
        operation = sync.queue_github_draft(
            self.root, REQUEST, branch="candidate", base="a" * 40,
            head="b" * 40, tested_sha="b" * 40, review_sha="b" * 40,
            publication_authorized=False, repository="owner/repo", base_branch="base",
            title="trial", body="body",
        )
        queued = sync.authorize_publication(
            self.root, REQUEST, operation["id"], "user-thread-2026-09-22",
        )
        self.assertEqual(queued["phase"], "queued")
        verified = {"provider": "github", "number": 42, "url": "https://example/pr/42"}
        with patch.object(sync, "_read_back", side_effect=[None, verified]), \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=SENT):
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "confirmed")
        self.assertEqual(result["operation"]["result"]["authorityReceipt"],
                         "user-thread-2026-09-22")

    def test_authorized_draft_executor_metadata_requires_authority_receipt(self) -> None:
        with self.assertRaisesRegex(ValueError, "authority receipt"):
            sync.queue_github_draft(
                self.root, REQUEST, branch="candidate", base="a" * 40,
                head="b" * 40, tested_sha="b" * 40, review_sha="b" * 40,
                publication_authorized=True, repository="owner/repo", base_branch="base",
                title="trial", body="body",
            )

    def test_linear_preflight_blocks_review_to_started_regression(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        issue = {"issue": {"state": {"name": "In Review", "type": "started"}}}
        with patch.object(sync, "_linear_issue", return_value=issue):
            allowed, reason = sync._preflight(operation, self.cli, self.repo)
        self.assertFalse(allowed)
        self.assertEqual(reason, "linear_state_would_regress")

    def test_linear_preflight_blocks_terminal_state_changes(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "review", "review")
        issue = {"issue": {"state": {"name": "Done", "type": "completed"}}}
        with patch.object(sync, "_linear_issue", return_value=issue):
            allowed, reason = sync._preflight(operation, self.cli, self.repo)
        self.assertFalse(allowed)
        self.assertEqual(reason, "linear_state_would_regress")

    def test_linear_preflight_blocks_unordered_custom_started_state(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "review", "review")
        issue = {"issue": {"state": {"name": "Ready to Deploy", "type": "started"}}}
        with patch.object(sync, "_linear_issue", return_value=issue):
            allowed, reason = sync._preflight(operation, self.cli, self.repo)
        self.assertFalse(allowed)
        self.assertEqual(reason, "linear_state_order_unproven")

    def test_github_preflight_requires_exact_remote_base_and_head(self) -> None:
        operation = sync.queue_github_draft(
            self.root, REQUEST, branch="candidate", base="a" * 40,
            head="b" * 40, tested_sha="b" * 40, review_sha="b" * 40,
            publication_authorized=True, publication_receipt="user-thread-2026-09-22",
            repository="owner/repo", base_branch="base", title="trial", body="body",
        )
        with patch.object(sync, "_github_ref", side_effect=["a" * 40, "c" * 40]):
            allowed, reason = sync._preflight(operation, self.cli, self.repo)
        self.assertFalse(allowed)
        self.assertEqual(reason, "github_head_sha_changed")

    def test_preflight_block_is_durable_and_never_sends(self) -> None:
        operation = sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        with patch.object(sync, "_read_back", return_value=None), \
                patch.object(sync, "_preflight",
                             return_value=(False, "linear_state_would_regress")), \
                patch.object(sync, "_send") as send:
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "blocked_policy")
        self.assertEqual(result["operation"]["id"], operation["id"])
        send.assert_not_called()

    def test_definitive_linear_rejection_is_policy_block_not_unknown(self) -> None:
        sync.queue_linear_status(self.root, REQUEST, "TAK-99", "started", "着手")
        rejected = {"exitCode": 1, "errorCode": "linear_invalid_state", "definitive": True}
        with patch.object(sync, "_read_back", side_effect=[None, None]), \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=rejected):
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "blocked_policy")
        self.assertEqual(result["operation"]["result"]["errorCode"], "linear_invalid_state")

    def test_unknown_github_write_preserves_publication_receipt(self) -> None:
        operation = sync.queue_github_draft(
            self.root, REQUEST, branch="candidate", base="a" * 40,
            head="b" * 40, tested_sha="b" * 40, review_sha="b" * 40,
            publication_authorized=False, repository="owner/repo", base_branch="base",
            title="trial", body="body",
        )
        sync.authorize_publication(
            self.root, REQUEST, operation["id"], "user-thread-2026-09-22",
        )
        with patch.object(sync, "_read_back", side_effect=[None, None]), \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=SENT):
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["state"], "unknown")
        self.assertEqual(result["operation"]["result"]["authorityReceipt"],
                         "user-thread-2026-09-22")

    def test_policy_block_can_be_superseded_by_same_target_successor(self) -> None:
        blocked = sync.queue_linear_status(
            self.root, REQUEST, "TAK-99", "started", "着手", target_state="Old state",
        )
        sync.transition(
            self.root, REQUEST, blocked["id"], "blocked_policy",
            {"reason": "provider_rejected"},
        )
        successor = sync.queue_linear_status(
            self.root, REQUEST, "TAK-99", "started", "着手", target_state="In Progress",
        )
        old = sync.supersede_policy_block(
            self.root, REQUEST, blocked["id"], successor["id"],
            "Workflow state name was corrected after read-back",
        )
        self.assertEqual(old["phase"], "superseded")
        self.assertEqual(old["result"]["supersededBy"], successor["id"])
        self.assertEqual(
            sync.supersede_policy_block(
                self.root, REQUEST, blocked["id"], successor["id"],
                "Workflow state name was corrected after read-back",
            ),
            old,
        )
        with self.assertRaisesRegex(ValueError, "different intent"):
            sync.supersede_policy_block(
                self.root, REQUEST, blocked["id"], successor["id"],
                "A different retry reason",
            )
        verified = {"provider": "linear", "issue": "TAK-99", "state": "In Progress"}
        with patch.object(sync, "_read_back", side_effect=[None, verified]), \
                patch.object(sync, "_preflight", return_value=(True, None)), \
                patch.object(sync, "_send", return_value=SENT):
            result = sync.execute_next(
                self.root, REQUEST, orca_cli=self.cli, repo=self.repo,
            )
        self.assertEqual(result["operation"]["id"], successor["id"])
        self.assertEqual(result["state"], "confirmed")

    def test_policy_block_cannot_be_superseded_by_another_target(self) -> None:
        blocked = sync.queue_linear_status(
            self.root, REQUEST, "TAK-99", "started", "着手", target_state="Old state",
        )
        sync.transition(
            self.root, REQUEST, blocked["id"], "blocked_policy",
            {"reason": "provider_rejected"},
        )
        successor = sync.queue_linear_status(
            self.root, REQUEST, "TAK-100", "started", "着手", target_state="In Progress",
        )
        with self.assertRaisesRegex(ValueError, "changes external target"):
            sync.supersede_policy_block(
                self.root, REQUEST, blocked["id"], successor["id"], "wrong target",
            )


if __name__ == "__main__":
    unittest.main()
