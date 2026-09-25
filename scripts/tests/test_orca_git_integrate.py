from __future__ import annotations

import copy
import json
import subprocess
import sys
import unittest
from unittest.mock import patch

from scripts import (host_coordination, orca_git_integrate as integration,
                     orca_review_loop as loop, orca_roles as roles, orca_settlement_recovery as recovery)
from scripts.tests import test_orca_review_loop as fixtures
from scripts.tests.test_orca_dispatch import REQUEST, COORDINATOR


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = fixtures.ReviewLoopTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.repo = self.fixture.root / "integration"
        self.fixture.run_git(self.fixture.primary, "worktree", "add", "-qb", "hw-42-integration", str(self.repo))
        self.target = {"repo": str(self.repo), "branch": "hw-42-integration", "base": self.fixture.ticket["base"]}

    def approvals(self):
        self.fixture.register()
        result = self.fixture.finish()
        self.assertEqual(result["phase"], "approved", result)
        lane = result["lanes"]["worker-a"]
        return [{"slot": "worker-a", "ticket": lane["review_ticket"], "review": lane["review"]}]

    def enable(self):
        self.fixture.spec["integration"] = {"target": self.target,
                                            "validation": self.fixture.spec["lanes"][0]["validation"]}

    def second_approval(self, first, *, conflict=False):
        repo = self.fixture.root / "worker-b"
        self.fixture.run_git(self.fixture.primary, "worktree", "add", "-qb", "task-b", str(repo), self.target["base"])
        (repo / ("src/content.txt" if conflict else "scripts/new.txt")).write_text("B change")
        self.fixture.run_git(repo, "add", ".")
        self.fixture.run_git(repo, "-c", "commit.gpgsign=false", "commit", "-qm", "B fixture")
        ticket = {**first["ticket"], "id": "review-b", "repo": str(repo), "branch": "task-b",
                  "base": roles.git(repo, "rev-parse", "HEAD"), "source_sha256": roles.fingerprint(repo)}
        self.fixture.record(ticket, "reviewer")
        record = {"ticket": ticket["id"], "base": ticket["review_base"], "head": ticket["base"],
                  "source_sha256": ticket["source_sha256"], "validation_evidence": ticket["validation_evidence"],
                  "reviewer_session": self.fixture.sessions["reviewer"], "verdict": "approved", "blocking_findings": []}
        return {"slot": "worker-b", "ticket": ticket, "review": roles.seal_review(ticket, record)}

    def test_two_worker_commits_are_serially_integrated(self):
        approvals = self.approvals()
        approvals.append(self.second_approval(approvals[0]))
        result = integration.integrate(self.target, approvals)
        self.assertEqual(len(result["commits"]), 2)
        self.assertEqual(result["commits"][1]["parents"][0], result["commits"][0]["head"])
        self.assertEqual((self.repo / "scripts/new.txt").read_text(), "B change")
        self.assertEqual((self.repo / "src/content.txt").read_text(), "revision 1")

    def test_second_merge_conflict_leaves_target_entirely_untouched(self):
        approvals = self.approvals()
        approvals.append(self.second_approval(approvals[0], conflict=True))
        source = roles.fingerprint(self.repo)
        with self.assertRaises(subprocess.CalledProcessError):
            integration.integrate(self.target, approvals)
        self.assertEqual(roles.fingerprint(self.repo), source)
        self.assertEqual(roles.git(self.repo, "status", "--porcelain"), "")

    def test_ignored_local_collision_is_not_overwritten(self):
        approvals = self.approvals()
        approvals.append(self.second_approval(approvals[0]))
        exclude = self.fixture.primary / ".git/info/exclude"
        exclude.write_text("scripts/new.txt\n")
        (self.repo / "scripts/new.txt").write_text("local ignored work")
        with self.assertRaisesRegex(ValueError, "existing local path"):
            integration.integrate(self.target, approvals)
        self.assertEqual((self.repo / "scripts/new.txt").read_text(), "local ignored work")
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.target["base"])

    def test_integration_keeps_worker_sha_and_requires_fresh_combined_validation(self):
        approvals = self.approvals()
        result = integration.integrate(self.target, approvals)
        self.assertEqual(result["phase"], "committed")
        self.assertEqual(roles.git(self.repo, "status", "--porcelain"), "")
        self.assertEqual(roles.git(self.repo, "show", "-s", "--format=%P", "HEAD"),
                         self.target["base"] + " " + approvals[0]["ticket"]["base"])
        self.assertEqual((self.repo / "src/content.txt").read_text(), "revision 1")
        self.assertEqual(integration.integrate(self.target, approvals), result)
        with self.assertRaisesRegex(ValueError, "same-integration"):
            integration.review_ticket(result, {"id": "a" * 64})
        evidence = integration.validate(result, self.fixture.spec["lanes"][0]["validation"])
        ticket = integration.review_ticket(result, evidence)
        self.assertEqual(ticket["base"], result["head"])
        self.assertNotEqual(ticket["validation_evidence"], approvals[0]["ticket"]["validation_evidence"])

    def test_recovers_checkout_and_ref_crashes_without_duplicate_merge(self):
        approvals = self.approvals()
        original = integration.git
        for crash_at in ("update-ref", "after-update-ref"):
            def crash(repo, *args, **kwargs):
                if args[0] == "update-ref":
                    if crash_at == "after-update-ref":
                        original(repo, *args, **kwargs)
                    raise OSError("simulated crash")
                return original(repo, *args, **kwargs)
            with patch.object(integration, "git", side_effect=crash), self.assertRaisesRegex(OSError, "simulated"):
                integration.integrate(self.target, approvals)
        head = roles.git(self.repo, "rev-parse", "HEAD")
        result = integration.integrate(self.target, approvals)
        self.assertEqual(result["head"], head)
        self.assertEqual(roles.git(self.repo, "rev-list", "--count", "HEAD"), "4")

    def test_dirty_target_and_stale_worker_approval_do_not_change_target(self):
        approvals = self.approvals()
        (self.repo / "src/content.txt").write_text("user edit")
        with self.assertRaisesRegex(ValueError, "clean target"):
            integration.integrate(self.target, approvals)
        self.assertEqual((self.repo / "src/content.txt").read_text(), "user edit")
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.target["base"])
        (self.repo / "src/content.txt").write_text("original")
        (self.fixture.repo / "src/content.txt").write_text("unreviewed edit")
        with self.assertRaisesRegex(ValueError, "stale"):
            integration.integrate(self.target, approvals)
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.target["base"])

    def test_recovery_preserves_external_edit_and_unknown_index(self):
        approvals = self.approvals()
        original = integration.git
        def crash(repo, *args, **kwargs):
            if args[0] == "update-ref":
                raise OSError("crash")
            return original(repo, *args, **kwargs)
        with patch.object(integration, "git", side_effect=crash), self.assertRaises(OSError):
            integration.integrate(self.target, approvals)
        (self.repo / "src/content.txt").write_text("external")
        with self.assertRaises(subprocess.CalledProcessError):
            integration.integrate(self.target, approvals)
        self.assertEqual((self.repo / "src/content.txt").read_text(), "external")
        self.fixture.run_git(self.repo, "add", "src/content.txt")
        with self.assertRaisesRegex(ValueError, "index is unknown"):
            integration.integrate(self.target, approvals)
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.target["base"])

    def test_real_loop_integrates_then_uses_same_fixed_reviewer_again(self):
        self.enable()
        self.fixture.register()
        result = self.fixture.finish()
        self.assertEqual(result["phase"], "approved", result)
        final = result["integration"]
        self.assertEqual(final["phase"], "approved")
        self.assertEqual(len(self.fixture.reviews), 3)
        self.assertEqual(final["review"]["reviewer_session"], self.fixture.sessions["reviewer"])
        self.assertEqual(final["review"]["head"], roles.git(self.repo, "rev-parse", "HEAD"))
        self.assertEqual(self.fixture.tick(), result)
        (self.repo / "src/content.txt").write_text("after approval")
        self.assertEqual(self.fixture.tick()["phase"], "paused")

    def test_settled_final_review_recovery_resumes_same_driver_without_new_dispatch(self):
        self.enable()
        self.fixture.register()
        for _ in range(60):
            data = self.fixture.tick()
            if data["integration"]["phase"] == "reviewing":
                break
        else:
            self.fail("final reviewer did not start")
        state = loop.bindings.read_state("reviewer", "codex")
        state["last"].update(phase="unknown", process_exited=True, exit_code=-15,
                             orca_bridge=data["integration"]["attempt"]["bridge_id"],
                             terminal=data["integration"]["attempt"]["terminal"])
        loop.bindings.save_state(state)
        loop.transition(data, "worker-a", "paused",
                        reason="approval invalidated: unknown role attempt; reconcile before any new launch")
        data["phase"] = "paused"
        loop.save(data)
        starts = len(self.fixture.starts)
        def reconcile(ticket, expected, metadata):
            self.assertEqual(expected, loop.bindings.digest(state))
            restored = copy.deepcopy(state)
            restored["last"].update(phase="recorded", exit_code=0, provider_exit_code=-15)
            loop.bindings.save_state(restored)
            return {"outcome": "completed"}
        with patch.object(recovery, "recover_role", side_effect=reconcile), \
                patch.object(loop.dispatch.ui_coordinator, "read_registered_state",
                             return_value={"phase": "ready", "terminal": COORDINATOR}):
            with self.assertRaisesRegex(ValueError, "exact paused"):
                recovery.recover(REQUEST, "0" * 64, loop.bindings.digest(state), self.fixture.root)
            recovery.recover(REQUEST, loop.bindings.digest(data), loop.bindings.digest(state), self.fixture.root)
        # This suite's start/completion adapter does not instantiate wire bridges;
        # the recovery suite separately checks real journals and exact read-back.
        with patch.object(roles, "require_completed_bridge"):
            result = self.fixture.finish()
        self.assertEqual(result["phase"], "approved", result["integration"].get("reason"))
        self.assertEqual(len(self.fixture.starts), starts)
        self.assertEqual(result["integration"]["review"]["head"], data["integration"]["receipt"]["head"])

    def test_combined_failure_or_findings_never_reuses_worker_approval(self):
        self.enable()
        self.fixture.register()
        original = self.fixture.completed
        def finding(ticket, slot, attempt):
            result = original(ticket, slot, attempt)
            if ticket["id"].startswith("integration-") and ticket["generation"] == 1:
                record = loop.parse_review(result["body"], result["session"])
                record.pop("reviewer_session")
                record.update(verdict="changes_requested", blocking_findings=[
                    {"id": "C1", "message": "Combined issue", "acceptance": "Combined case passes"}])
                result["body"] = "Summary. Combined. Done.\nORCA_REVIEW_JSON: " + json.dumps(record)
            return result
        with patch.object(loop, "completion", side_effect=finding):
            for _ in range(60):
                result = self.fixture.tick()
                if result["integration"]["phase"] == "correction_required":
                    break
            else:
                self.fail("combined findings not surfaced")
            self.assertEqual(result["phase"], "active", result)
            self.assertEqual(result["integration"]["review"]["verdict"], "changes_requested")
            self.assertTrue(result["attempts"][result["integration"]["attempt"]["dispatch_id"]]["released"])
            attention = loop.watch(REQUEST, COORDINATOR, timeout=0)["attention"]
            self.assertEqual(attention[0]["type"], "combined_review")
            routing = {"head": attention[0]["head"], "slot": "worker-a", "reason": "C1 is confined to src"}
            with self.assertRaisesRegex(ValueError, "this settled"):
                loop.route_correction(REQUEST, COORDINATOR, {**routing, "head": "a" * 40})
            first = loop.route_correction(REQUEST, COORDINATOR, routing)
            self.assertEqual(loop.route_correction(REQUEST, COORDINATOR, routing), first)
            result = self.fixture.finish()
        self.assertEqual(result["phase"], "approved", result)
        self.assertEqual(result["integration"]["review_ticket"]["generation"], 2)
        self.assertEqual(len(self.fixture.starts), 8)
        last_worker = [row for row in self.fixture.starts if row[0] == "worker-a"][-1]
        self.assertEqual(last_worker[2]["resume_session"], self.fixture.sessions["worker-a"])
        self.assertIn(routing["head"], last_worker[2]["follow_up"])
        self.assertEqual(result["integration"]["receipt"]["start"], routing["head"])
        self.assertEqual(roles.git(self.repo, "merge-base", result["integration"]["receipt"]["head"], routing["head"]), routing["head"])

    def test_combined_validation_failure_is_recorded_without_final_review(self):
        self.enable()
        config = copy.deepcopy(self.fixture.spec["integration"]["validation"])
        config["argv"][-1] = "raise SystemExit(7)"
        self.fixture.spec["integration"]["validation"] = config
        self.fixture.register()
        result = self.fixture.finish()
        self.assertEqual(result["phase"], "paused", result)
        self.assertEqual(result["integration"]["evidence"]["exit_code"], 7)
        self.assertEqual(len(self.fixture.reviews), 2)

    def test_busy_final_reviewer_does_not_starve_questions(self):
        self.enable()
        self.fixture.register()
        for _ in range(60):
            result = self.fixture.tick()
            if result["integration"]["phase"] == "reviewing":
                break
        else:
            self.fail("final review not dispatched")
        with (host_coordination.acquire_host("reviewer", inherit=False),
              patch.object(loop.mail, "poll", return_value=True) as poll,
              patch.object(loop, "completion", return_value=None)):
            self.fixture.tick()
        poll.assert_called_once()

    def test_final_approval_waits_for_final_done_ack(self):
        self.enable()
        self.fixture.register()
        for _ in range(60):
            result = self.fixture.tick()
            if result["integration"]["phase"] == "deciding":
                break
        else:
            self.fail("final verdict not ready")
        with patch.object(loop.mail, "drained", return_value=False):
            result = self.fixture.tick()
        self.assertEqual(result["integration"]["phase"], "approved")
        self.assertEqual(result["phase"], "active")
        self.assertEqual(self.fixture.tick()["phase"], "approved")

    def test_combined_validation_cannot_change_subject(self):
        result = integration.integrate(self.target, self.approvals())
        config = copy.deepcopy(self.fixture.spec["lanes"][0]["validation"])
        config["argv"][-1] = "from pathlib import Path; Path('src/content.txt').write_text('unexpected')"
        with self.assertRaisesRegex(ValueError, "modified source"):
            integration.validate(result, config)
        self.assertEqual((self.repo / "src/content.txt").read_text(), "unexpected")

    def test_combined_validation_exports_reviewed_no_impact_reason(self):
        result = integration.integrate(self.target, self.approvals())
        reason = "Fixture only; no player behavior changes"
        config = {
            "argv": [sys.executable, "-c", "import os; assert os.environ['HELL_WORKERS_HELP_IMPACT_REASON'] == " + repr(reason)],
            "help_reason": reason,
            "help_decision": "none",
        }
        evidence = integration.validate(result, config)
        self.assertEqual(evidence["exit_code"], 0)

    def test_combined_validation_uses_current_host_runner_without_outer_heavy(self):
        result = integration.integrate(self.target, self.approvals())
        command = [sys.executable, "scripts/dev.py", "ci", "check", "--base", self.target["base"]]
        execution = [sys.executable, "/fixture/orca_host_validation.py", "--repo", str(self.repo),
                     "--", "ci", "check", "--base", self.target["base"]]
        executor = {"schema": 1, "kind": "host-dev-runner", "sha256": "a" * 64}
        completed = subprocess.CompletedProcess(execution, 0, b"", b"")
        acquired = []
        original = integration.acquire_host
        original_run = subprocess.run

        def tracked(name="heavy", **kwargs):
            acquired.append(name)
            return original(name, **kwargs)

        def run_command(argv, *args, **kwargs):
            if argv == execution:
                return completed
            return original_run(argv, *args, **kwargs)

        with (
            patch.object(integration.checkpoints, "validation_execution", return_value=(execution, executor)),
            patch.object(integration, "acquire_host", side_effect=tracked),
            patch.object(integration.subprocess, "run", side_effect=run_command) as run,
        ):
            evidence = integration.validate(result, {
                "argv": command,
                "help_reason": "Fixture only; no player behavior changes",
                "help_decision": "none",
            })
        self.assertEqual(evidence["executor"], executor)
        self.assertNotIn("heavy", acquired)
        validation_call = next(item for item in run.call_args_list if item.args[0] == execution)
        self.assertEqual(validation_call.kwargs["timeout"], 7200)
        self.assertNotIn(host_coordination.HOST_FD_ENV, validation_call.kwargs["env"])
        self.assertEqual(validation_call.kwargs["pass_fds"], ())

    def test_cancellation_before_integration_preserves_target(self):
        self.enable()
        self.fixture.register()
        for _ in range(60):
            result = self.fixture.tick()
            if result["integration"]["phase"] == "integrating":
                break
        else:
            self.fail("approved workers not ready")
        with patch.object(loop, "active_issue", return_value=False):
            result = self.fixture.tick()
        self.assertEqual(result["phase"], "paused")
        self.assertEqual(roles.git(self.repo, "rev-parse", "HEAD"), self.target["base"])

    def test_registration_rejects_non_issue_same_worker_or_wrong_base_target(self):
        self.enable()
        original = copy.deepcopy(self.target)
        for target in ({**original, "branch": "task"},
                       {**original, "repo": str(self.fixture.repo), "branch": "task"},
                       {**original, "base": "a" * 40}):
            self.fixture.spec["integration"]["target"] = target
            with self.subTest(target=target), self.assertRaises(ValueError):
                self.fixture.register()
