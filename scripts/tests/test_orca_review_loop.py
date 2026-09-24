from __future__ import annotations

import copy
import json
import sys
import threading
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import (host_coordination, orca_dispatch as dispatch, orca_git_checkpoint as checkpoints,
                     orca_review_loop as loop, orca_role_state as bindings, orca_roles as roles)
from scripts.tests import test_orca_dispatch as fixtures

RELEASE = loop.release
COMPLETION = loop.completion
ENSURE_RUN = loop.mail.ensure_run
POLL_MAIL = loop.mail.poll
MAIL_DRAINED = loop.mail.drained

class ReviewLoopTests(unittest.TestCase):
    run_git = staticmethod(fixtures.OrcaDispatchTests.run_git)

    def setUp(self):
        fixtures.OrcaDispatchTests.setUp(self)
        patcher = patch.object(roles.task_bridge, "state_root", return_value=self.root / "coordination")
        patcher.start()
        self.addCleanup(patcher.stop)
        self.run_git(self.primary, "config", "user.name", "Fixture")
        self.run_git(self.primary, "config", "user.email", "fixture@example.invalid")
        self.spec = {"lanes": [{"slot": "worker-a", "ticket": self.ticket,
                                "validation": {"argv": [sys.executable, "-c", "pass"],
                                               "help_decision": "none", "help_reason": "Test fixture only"}}]}
        self.sessions = {"worker-a": str(uuid.uuid4()), "reviewer": str(uuid.uuid4())}
        self.snapshots = {slot: {"session_id": session, "session_sha256": "a" * 64}
                          for slot, session in self.sessions.items()}
        self.starts, self.reviews = [], []
        for target, name, kwargs in (
            (dispatch.intake, "read_issue", {"return_value": {"issue_id": fixtures.ISSUE, "state": {"type": "started"}}}),
            (bindings, "session_snapshot", {"side_effect": self.snapshot}),
            (dispatch, "start", {"side_effect": self.start}),
            (loop, "completion", {"side_effect": self.completed}),
            (loop, "release", {}),
            (loop.mail, "ensure_run", {"return_value": {"id": "run_fixture", "consumer_generation": 1}}),
            (loop.mail, "poll", {"return_value": False}),
            (loop.mail, "drained", {"return_value": True}),
        ):
            patcher = patch.object(target, name, **kwargs)
            patcher.start()
            self.addCleanup(patcher.stop)

    def snapshot(self, runtime, *_):
        slot = "reviewer" if Path(runtime).name == "reviewer" else "worker-a"
        return dict(self.snapshots[slot])

    def register(self):
        return loop.register(fixtures.REQUEST, fixtures.COORDINATOR, self.spec)

    def tick(self):
        return loop.tick(fixtures.REQUEST, fixtures.COORDINATOR)

    def record(self, ticket, slot):
        data = bindings.read_state(slot, "codex")
        key = "fixed-reviewer" if slot == "reviewer" else checkpoints.task_key(ticket)
        snapshot = self.snapshots[slot]
        data["tasks"][key] = {"key": key, "ticket_sha256": bindings.digest(ticket),
                              "subject": checkpoints.subject(ticket), "origin": ticket["repo"],
                              "source_sha256": roles.fingerprint(Path(ticket["repo"])), **snapshot}
        data["last"] = {"key": key, "phase": "recorded", "process_exited": True,
                        "exit_code": 0, "attempt_id": str(uuid.uuid4())}
        bindings.save_state(data)
        if slot != "reviewer":
            bindings.claim_task(key, slot, "codex", ticket, checkpoints.subject(ticket))

    def start(self, request_id, path, slot, terminal, **kwargs):
        ticket = loop.STORAGE.read_private_json(path, {})
        self.starts.append((slot, copy.deepcopy(ticket), kwargs))
        if slot != "reviewer":
            data = bindings.read_state(slot, "codex")
            bindings.admit(data, ticket, checkpoints.subject(ticket), roles.fingerprint(self.repo),
                           kwargs["resume_session"], kwargs["follow_up"])
            (self.repo / "src/content.txt").write_text(f"revision {ticket.get('generation', 0)}")
        else:
            self.reviews.append(ticket)
        self.snapshots[slot]["session_sha256"] = bindings.digest({"turn": len(self.starts)})
        self.record(ticket, slot)
        return {"phase": "armed", "bridge_id": str(uuid.uuid4()), "terminal": f"term_fixture{len(self.starts)}",
                "run_id": "run_fixture", "task_id": f"task_fixture{len(self.starts)}", "dispatch_id": f"ctx_fixture{len(self.starts)}"}

    def completed(self, ticket, slot, attempt):
        record = {"ticket": ticket["id"], "base": ticket.get("review_base", ticket["base"]),
                  "head": ticket["base"], "source_sha256": roles.fingerprint(Path(ticket["repo"])),
                  "validation_evidence": ticket.get("validation_evidence", "a" * 64),
                  "verdict": "approved", "blocking_findings": []}
        if slot == "reviewer" and len(self.reviews) == 1:
            record.update(verdict="changes_requested", blocking_findings=[
                {"id": "R1", "message": "Fix the bounded text", "acceptance": "The next revision is present"}])
        return {"session": self.sessions[slot], "body": "Summary. Scope checked. Review complete.\nORCA_REVIEW_JSON: " + json.dumps(record),
                "outcome": "completed", "attempt_id": str(uuid.uuid4()), "source": roles.fingerprint(Path(ticket["repo"]))}

    def finish(self):
        for _ in range(60):
            data = self.tick()
            if data["phase"] != "active":
                return data
        self.fail("bounded fixture loop did not settle")

    def finish_integrated(self):
        target_repo = self.root / "integration"
        base = self.ticket["base"]
        self.run_git(self.primary, "worktree", "add", "-qb", "hw-42-integration", str(target_repo), base)
        validation = {"argv": [sys.executable, "-c", "pass"],
                      "help_decision": "none", "help_reason": "Test fixture only"}
        self.spec["integration"] = {
            "target": {"repo": str(target_repo), "branch": "hw-42-integration", "base": base},
            "validation": validation,
        }
        self.register()
        with patch.object(loop, "is_production_path", return_value=False):
            data = self.finish()
        self.assertEqual(data["phase"], "approved", data)
        data["run"] = {"phase": "ready", "context": {
            "id": "run_fixture", "consumer_generation": 1,
        }}
        for attempt in data["attempts"].values():
            attempt["completion_acknowledged"] = True
        data["ui_cleanup"] = {"phase": "complete", "receipts": [], "at_ms": loop.event_time_ms()}
        loop.save(data)
        return loop.load(fixtures.REQUEST), target_repo

    def test_review_revision_round_trip_real_git_same_worker_and_fixed_reviewer(self):
        self.register()
        data = self.finish()
        self.assertEqual(data["phase"], "approved", data)
        lane = data["lanes"]["worker-a"]
        self.assertEqual(lane["revisions"], 1)
        self.assertEqual([row[0] for row in self.starts], ["worker-a", "reviewer", "worker-a", "reviewer"])
        self.assertEqual(self.starts[2][2]["resume_session"], self.sessions["worker-a"])
        self.assertIn("R1", self.starts[2][2]["follow_up"])
        self.assertNotIn(self.ticket["prompt"], self.starts[2][2]["follow_up"])
        self.assertTrue(all(row[2]["exit_on_settlement"] for row in self.starts))
        self.assertEqual(roles.git(self.repo, "rev-list", "--count", "HEAD"), "3")
        self.assertEqual(roles.git(self.repo, "status", "--porcelain"), "")
        self.assertEqual(self.tick(), data)

    def test_settled_integrated_loop_starts_one_successor_on_approved_head_and_same_run(self):
        previous, target_repo = self.finish_integrated()
        expected = loop.inspection_digest(previous)
        approved_head = previous["integration"]["receipt"]["head"]
        next_repo = self.root / "next-worker"
        self.run_git(self.primary, "worktree", "add", "-qb", "hw-42-next", str(next_repo), approved_head)
        ticket = {
            **self.ticket,
            "id": "edit-next-leaf",
            "repo": str(next_repo),
            "branch": "hw-42-next",
            "base": approved_head,
            "prompt": "Continue the next bounded milestone.",
        }
        validation = {"argv": [sys.executable, "-c", "pass"],
                      "help_decision": "none", "help_reason": "Test fixture only"}
        spec = {
            "lanes": [{"slot": "worker-a", "ticket": ticket, "validation": validation}],
            "integration": {
                "target": {"repo": str(target_repo), "branch": "hw-42-integration", "base": approved_head},
                "validation": validation,
            },
        }
        with patch.object(dispatch, "checked_run", return_value=previous["run"]["context"]):
            context = loop.successor_preflight(fixtures.REQUEST, fixtures.COORDINATOR)
            wrong = copy.deepcopy(spec)
            wrong["integration"]["target"]["branch"] = "hw-42-other"
            with self.assertRaisesRegex(ValueError, "exact approved integration"):
                loop.register_successor(fixtures.REQUEST, fixtures.COORDINATOR, wrong, expected)
            self.assertEqual(list(loop.history_root(fixtures.REQUEST).glob("*.json")), [])
            current = loop.register_successor(fixtures.REQUEST, fixtures.COORDINATOR, spec, expected)
            repeated = loop.register_successor(fixtures.REQUEST, fixtures.COORDINATOR, spec, expected)
        self.assertEqual(context["mode"], "successor")
        self.assertEqual(context["base"], approved_head)
        self.assertEqual(current["loop_generation"], 2)
        self.assertEqual(current["run"], previous["run"])
        self.assertEqual(current["predecessor"]["loop_sha256"], expected)
        self.assertEqual(repeated, current)
        archive = Path(current["predecessor"]["archive"])
        archived = loop.STORAGE.read_private_json(archive, {})
        self.assertEqual(archived["loop"], previous)
        self.assertEqual(archived["loop_sha256"], expected)
        loop.STORAGE.write_ledger(archive, {"schema": 1, "request_id": fixtures.REQUEST})
        with self.assertRaisesRegex(ValueError, "predecessor archive changed"):
            loop.load(fixtures.REQUEST)

    def test_successor_rejects_unsettled_or_wrong_target_without_archiving(self):
        data = self.register()
        data["phase"] = "approved"
        loop.save(data)
        with patch.object(dispatch, "checked_run"):
            with self.assertRaisesRegex(ValueError, "settled integrated approval"):
                loop.successor_preflight(fixtures.REQUEST, fixtures.COORDINATOR)
        self.assertEqual(list(loop.history_root(fixtures.REQUEST).glob("*.json")), [])

    def test_round_trip_uses_one_run_and_drains_every_real_mail_adapter_delivery(self):
        queued, acknowledgements, creates = [], [], []
        run = {"id": "run_fixture", "consumer_generation": 1,
               "coordinator_handle": fixtures.COORDINATOR, "legacy": 0}

        def start(request, path, slot, terminal, **kwargs):
            attempt = self.start(request, path, slot, terminal, **kwargs)
            ticket = loop.STORAGE.read_private_json(path, {})
            row = {"id": f"msg_done{len(self.starts)}", "type": "worker_done", "subject": "fixture",
                   "run_id": run["id"], "from_handle": attempt["terminal"], "to_handle": "run:" + run["id"],
                   "body": self.completed(ticket, slot, attempt)["body"]}
            authority = {"run": run["id"], "task": attempt["task_id"],
                         "dispatch": attempt["dispatch_id"], "coordinator": terminal}
            loop.STORAGE.write_ledger(roles.task_bridge.root() / attempt["bridge_id"] / "journal.json",
                {"phase": "settled", "authority": authority,
                 "operations": {"done": {"phase": "confirmed", "result": {"message": row}}}})
            queued.append(row)
            return attempt

        def call(executable, argv, operation):
            if operation == "run-current":
                return {"run": run if creates else None}
            if operation == "run-create":
                creates.append(1)
                return {"run": run}
            self.assertEqual(operation, "loop-check")
            ack = argv[argv.index("--ack") + 1] if "--ack" in argv else None
            if ack:
                acknowledgements.append(ack)
            rows = list(queued)
            queued.clear()
            return {"runId": run["id"], "messages": rows, "count": len(rows), "acknowledged": ack,
                    "deliveryId": f"delivery_{len(self.starts)}" if rows else None}

        with (patch.object(loop.mail, "ensure_run", side_effect=ENSURE_RUN),
              patch.object(loop.mail, "poll", side_effect=POLL_MAIL),
              patch.object(loop.mail, "drained", side_effect=MAIL_DRAINED),
              patch.object(dispatch, "start", side_effect=start),
              patch.object(dispatch, "run_cli", side_effect=call)):
            self.register()
            data = self.finish()
        self.assertEqual(data["phase"], "approved", data)
        self.assertEqual(len(creates), 1)
        self.assertEqual(len(acknowledgements), 4)
        self.assertEqual(len(data["attempts"]), 4)
        self.assertTrue(all(item["completion_acknowledged"] for item in data["attempts"].values()))

    def test_approved_lanes_wait_for_missing_mail_and_watch_does_not_consume(self):
        self.register()
        with patch.object(loop.mail, "drained", return_value=False):
            for _ in range(50):
                data = self.tick()
                if data["lanes"]["worker-a"]["phase"] == "approved":
                    break
            self.assertEqual(data["phase"], "active")
            with patch.object(dispatch, "run_cli") as cli:
                observed = loop.watch(fixtures.REQUEST, fixtures.COORDINATOR, timeout=0)
                cli.assert_not_called()
            self.assertEqual(observed["lanes"]["worker-a"]["phase"], "approved")

    def test_finalization_closes_each_finished_role_tab_once_and_keeps_coordinator(self):
        self.register()
        data = self.finish()
        self.assertEqual(data["phase"], "approved")
        for attempt in data["attempts"].values():
            attempt["repo"] = str(self.repo)
            attempt["released"] = True
            attempt["completion_acknowledged"] = True
        loop.save(data)
        identities = {}
        for attempt in data["attempts"].values():
            slot = "reviewer" if attempt["role"] == "reviewer" else attempt["slot"]
            key = (str(self.repo), slot)
            identities.setdefault(key, {
                "handle": attempt["terminal"],
                "incarnationId": "incarnation-" + slot,
                "worktreeId": "repo::" + str(self.repo),
            })
        for (repo, slot), identity in identities.items():
            loop.STORAGE.write_ledger(
                dispatch.role_tabs.registry_path(fixtures.REQUEST, repo, slot),
                {"phase": "known", "identity": identity},
            )

        retired = []

        def inventory(_call, _cli, repo):
            rows = [{"handle": identity["handle"]} for (path, _), identity in identities.items()
                    if path == repo]
            return rows, []

        def retire(_call, _cli, repo, identity):
            retired.append((repo, identity["handle"]))
            return self.root / (identity["handle"] + ".json")

        expected = loop.inspection_digest(data)
        with (patch.object(dispatch, "checked_coordinator"),
              patch.object(dispatch.role_tabs, "inventory", side_effect=inventory),
              patch.object(dispatch.role_tabs, "retire_settled", side_effect=retire)):
            finalized = loop.finalize_tabs(fixtures.REQUEST, fixtures.COORDINATOR, expected)
            repeated = loop.finalize_tabs(fixtures.REQUEST, fixtures.COORDINATOR,
                                          loop.inspection_digest(finalized))
        self.assertEqual(finalized["ui_cleanup"]["phase"], "complete")
        self.assertEqual(len(retired), len(identities))
        self.assertNotIn(fixtures.COORDINATOR, {handle for _, handle in retired})
        self.assertEqual(repeated, finalized)

    def test_failed_validation_retries_without_commit_and_keeps_same_session(self):
        self.spec["lanes"][0]["validation"]["argv"] = [sys.executable, "-c",
            "from pathlib import Path; raise SystemExit(0 if 'revision 1' in Path('src/content.txt').read_text() else 1)"]
        self.register()
        for _ in range(15):
            data = self.tick()
            if data["lanes"]["worker-a"]["revisions"]:
                break
        lane = data["lanes"]["worker-a"]
        self.assertEqual(lane["phase"], "planned")
        self.assertEqual(lane["ticket"]["base"], self.ticket["base"])
        self.assertEqual(lane["ticket"]["generation"], 1)
        self.assertEqual(roles.git(self.repo, "rev-list", "--count", "HEAD"), "1")
        self.tick()
        self.tick()
        self.assertEqual(self.starts[-1][2]["resume_session"], self.sessions["worker-a"])

    def test_registration_is_idempotent_and_cannot_reassign_dirty_ticket(self):
        original = self.register()
        self.assertEqual(self.register(), original)
        changed = copy.deepcopy(self.spec)
        changed["lanes"][0]["ticket"]["prompt"] = "another task"
        with self.assertRaisesRegex(ValueError, "another specification"):
            loop.register(fixtures.REQUEST, fixtures.COORDINATOR, changed)
        self.assertEqual(self.starts, [])

    def test_source_change_before_dispatch_stops_without_start(self):
        self.register()
        (self.repo / "src/content.txt").write_text("external")
        data = self.tick()
        self.assertEqual(data["phase"], "paused")
        self.assertEqual(self.starts, [])
        self.assertEqual((self.repo / "src/content.txt").read_text(), "external")

    def test_unknown_dispatch_is_not_retried(self):
        self.register()
        self.tick()
        with patch.object(dispatch, "start", side_effect=RuntimeError("unknown mutation")) as start:
            self.assertEqual(self.tick()["phase"], "paused")
            self.tick()
            self.assertEqual(start.call_count, 1)

    def test_busy_slot_waits_without_pausing_or_new_dispatch(self):
        self.register()
        self.tick()
        with host_coordination.acquire_host("worker-a", inherit=False):
            self.assertEqual(self.tick()["lanes"]["worker-a"]["phase"], "dispatching")
        self.assertEqual(self.starts, [])
        self.tick()
        self.assertEqual(len(self.starts), 1)

    def test_transient_linear_runtime_failure_waits_without_pausing(self):
        before = self.register()
        history = list(before["lanes"]["worker-a"]["history"])
        with patch.object(loop, "active_issue", side_effect=dispatch.intake.LinearRuntimeUnavailable(
                "Orca Linear read failed (runtime_unavailable)")):
            data = self.tick()
        self.assertEqual(data["phase"], "active")
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "planned")
        self.assertEqual(data["lanes"]["worker-a"]["history"], history)
        self.assertEqual(self.starts, [])

    def test_exact_legacy_linear_runtime_pause_resumes_pre_command_phase(self):
        data = self.register()
        loop.transition(data, "worker-a", "paused",
                        reason="LinearIntakeError: Orca Linear read failed (runtime_unavailable)")
        data.update(phase="paused", reason=None)
        loop.save(data)
        expected = loop.inspection_digest(loop.load(fixtures.REQUEST))
        with patch.object(loop, "active_issue", return_value=True):
            resumed = loop.resume_linear_runtime(fixtures.REQUEST, fixtures.COORDINATOR, expected)
        self.assertEqual(resumed["phase"], "active")
        self.assertEqual(resumed["lanes"]["worker-a"]["phase"], "planned")
        self.assertIsNone(resumed["lanes"]["worker-a"]["reason"])

    def test_validation_busy_is_not_an_unknown_command(self):
        self.register()
        for _ in range(4):
            data = self.tick()
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "validating")
        with host_coordination.acquire_host("heavy", inherit=False):
            self.assertEqual(self.tick()["lanes"]["worker-a"]["phase"], "validating")
        self.assertEqual(self.tick()["lanes"]["worker-a"]["phase"], "checkpointing")

    def test_second_worker_uses_a_disjoint_worktree_and_rejects_overlap(self):
        other = self.root / "worker-b"
        self.run_git(self.primary, "worktree", "add", "-qb", "other-task", str(other))
        item = copy.deepcopy(self.spec["lanes"][0])
        item["slot"] = "worker-b"
        item["ticket"].update(repo=str(other), branch="other-task", id="second-leaf", provider="cursor", complexity="simple-leaf")
        self.spec["lanes"].append(item)
        with patch.object(roles, "provider_for", return_value="cursor"), self.assertRaisesRegex(ValueError, "overlap"):
            self.register()

    def test_pending_role_finalization_waits_for_the_role_lock(self):
        self.register()
        self.tick()
        data = self.tick()
        attempt = data["lanes"]["worker-a"]["attempt"]
        with host_coordination.acquire_host("worker-a", inherit=False), self.assertRaises(host_coordination.HostBusyError):
            COMPLETION(self.ticket, "worker-a", attempt)

    def test_exact_bridge_authority_and_session_are_required_for_completion(self):
        self.register()
        self.tick()
        data = self.tick()
        attempt = data["lanes"]["worker-a"]["attempt"]
        state = bindings.read_state("worker-a", "codex")
        state["last"].update(orca_bridge=attempt["bridge_id"], terminal=attempt["terminal"])
        bindings.save_state(state)
        authority = {"run": attempt["run_id"], "task": attempt["task_id"],
                     "dispatch": attempt["dispatch_id"], "coordinator": fixtures.COORDINATOR}
        directory = roles.task_bridge.root() / attempt["bridge_id"]
        loop.STORAGE.write_ledger(directory / "arm.json", authority)
        loop.STORAGE.write_ledger(directory / "identity.json", {"repo": str(self.repo), "terminal": attempt["terminal"]})
        journal = {"phase": "settled", "settled_status": "completed", "revoked": True, "authority": authority,
                   "operations": {"done": {"phase": "confirmed", "result": {
                       "message": {"type": "worker_done", "run_id": attempt["run_id"], "from_handle": attempt["terminal"]},
                       "lifecycle": {"action": "completed", "taskId": attempt["task_id"], "dispatchId": attempt["dispatch_id"]}}}}}
        loop.STORAGE.write_ledger(directory / "journal.json", journal)
        self.assertEqual(COMPLETION(self.ticket, "worker-a", attempt)["outcome"], "completed")
        journal["authority"]["dispatch"] = "ctx_stale"
        loop.STORAGE.write_ledger(directory / "journal.json", journal)
        with self.assertRaisesRegex(ValueError, "authority"):
            COMPLETION(self.ticket, "worker-a", attempt)

    def test_unexpected_driver_exception_is_reported_without_retry(self):
        self.register()
        with patch.object(loop, "tick", side_effect=KeyError("attempt_id")) as tick:
            with loop.Driver(fixtures.REQUEST, fixtures.COORDINATOR) as driver:
                driver.thread.join(timeout=5)
                self.assertFalse(driver.thread.is_alive())
                observed = loop.watch(fixtures.REQUEST, fixtures.COORDINATOR, timeout=0)
                self.assertEqual(observed["driver"]["phase"], "failed")
                self.assertIn("KeyError", observed["driver"]["reason"])
        tick.assert_called_once()

    def test_resumed_driver_waits_for_ui_acknowledgement(self):
        ui = dispatch.ui_coordinator
        path = self.root / "starting-ui.json"
        path.write_text("{}")
        states = [{"terminal": fixtures.COORDINATOR, "phase": phase} for phase in ("starting", "ready")]
        with (patch.object(ui, "state_path", return_value=path),
              patch.object(ui, "load_state", side_effect=states) as read,
              patch.object(loop, "tick", side_effect=RuntimeError("end fixture")) as tick):
            with loop.Driver(fixtures.REQUEST, fixtures.COORDINATOR) as driver:
                driver.thread.join(timeout=5)
                self.assertFalse(driver.thread.is_alive())
        self.assertEqual(read.call_count, 2)
        tick.assert_called_once()

    def test_duplicate_driver_does_not_run_another_tick(self):
        with patch.object(loop, "tick"):
            with loop.Driver(fixtures.REQUEST, fixtures.COORDINATOR):
                with self.assertRaises(host_coordination.HostBusyError):
                    with loop.Driver(fixtures.REQUEST, fixtures.COORDINATOR):
                        self.fail("second driver started")

    def test_ui_does_not_launch_provider_before_acquiring_driver(self):
        ui = dispatch.ui_coordinator
        with (patch.object(ui.shutil, "which", return_value="/fixture/codex"),
              patch.object(ui, "prepare", return_value=({"request_id": fixtures.REQUEST},
                  {"terminal": fixtures.COORDINATOR, "linear_identifier": "FIX-1"})),
              patch.object(ui, "primary_repo", return_value=self.primary),
              patch.object(ui, "provider_command", return_value=["/fixture/codex"]),
              patch.object(ui, "prompt", return_value="fixture"),
              patch.object(ui, "prepare_runtime", return_value=self.root / "runtime"),
              patch.object(ui, "sandbox_command", return_value=["/fixture/codex"]),
              patch.object(loop, "Driver", side_effect=host_coordination.HostBusyError("existing driver")),
              patch.object(ui.subprocess, "Popen") as spawn,
              self.assertRaises(host_coordination.HostBusyError)):
            ui.launch()
        spawn.assert_not_called()

    def test_interrupted_validation_does_not_rerun_command(self):
        data = self.register()
        loop.transition(data, "worker-a", "validation_running")
        with patch.object(checkpoints, "validate") as validate:
            self.assertEqual(self.tick()["phase"], "paused")
            validate.assert_not_called()

    def test_cancellation_prevents_dispatch_without_touching_worker(self):
        self.register()
        with patch.object(loop, "active_issue", return_value=False):
            self.assertEqual(self.tick()["phase"], "paused")
        self.assertEqual(self.starts, [])

    def test_changed_source_invalidates_previously_approved_state(self):
        self.register()
        self.assertEqual(self.finish()["phase"], "approved")
        (self.repo / "src/content.txt").write_text("external after approval")
        self.assertEqual(self.tick()["phase"], "paused")

    def test_cancellation_before_checkpoint_keeps_dirty_source_without_commit(self):
        self.register()
        for _ in range(5):
            data = self.tick()
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "checkpointing")
        with patch.object(loop, "active_issue", return_value=False):
            self.assertEqual(self.tick()["phase"], "paused")
        self.assertEqual(roles.git(self.repo, "rev-list", "--count", "HEAD"), "1")
        self.assertEqual((self.repo / "src/content.txt").read_text(), "revision 0")

    def test_invalid_review_is_released_before_pausing_without_new_dispatch(self):
        self.register()
        for _ in range(8):
            data = self.tick()
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "reviewing")
        with patch.object(loop, "completion", return_value={"outcome": "completed", "body": "Looks good",
                                                           "session": self.sessions["reviewer"]}):
            self.assertEqual(self.tick()["lanes"]["worker-a"]["phase"], "review_release")
        with patch.object(loop, "release") as release:
            data = self.tick()
            release.assert_called_once()
        self.assertEqual(data["phase"], "paused")
        self.assertIn("invalid review verdict", data["lanes"]["worker-a"]["reason"])
        self.assertEqual(len(self.starts), 2)

    def test_paused_loop_cannot_be_bypassed_by_a_new_request(self):
        data = self.register()
        data["phase"] = "paused"
        loop.save(data)
        with patch.object(dispatch, "checked_coordinator"), patch.object(dispatch, "linear_record"):
            with self.assertRaisesRegex(ValueError, "unresolved loop"):
                loop.register(str(uuid.uuid4()), fixtures.COORDINATOR, self.spec)

    def test_paused_loop_polls_mail_without_dispatching_or_integrating(self):
        data = self.register()
        data["phase"] = "paused"
        loop.save(data)
        with patch.object(loop.mail, "poll", return_value=False) as poll, \
                patch.object(loop, "step") as step, patch.object(loop, "integration_step") as integrate:
            result = self.tick()
        self.assertEqual(result["phase"], "paused")
        poll.assert_called_once()
        step.assert_not_called()
        integrate.assert_not_called()

    def test_auxiliary_specs_do_not_own_loop_bindings(self):
        loop.STORAGE.write_ledger(loop.root() / "recovery-spec.json", {"purpose": "maintenance"})
        self.register()
        for _ in range(8):
            data = self.tick()
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "reviewing")

    def test_canonical_invalid_ledger_still_refuses_registration(self):
        loop.STORAGE.write_ledger(loop.root() / f"{uuid.uuid4()}.json", {})
        with self.assertRaisesRegex(ValueError, "invalid review loop ledger"):
            self.register()

    def test_reconcile_only_inspected_pause_before_review_dispatch(self):
        self.register()
        for _ in range(6):
            data = self.tick()
        self.assertEqual(data["lanes"]["worker-a"]["phase"], "review_pending")
        loop.transition(data, "worker-a", "paused", reason="scheduler catalog error")
        data["phase"] = "paused"
        loop.save(data)
        expected = bindings.digest(data)
        with self.assertRaisesRegex(ValueError, "exact inspected"):
            loop.resume_review_wait(fixtures.REQUEST, fixtures.COORDINATOR, "worker-a", "0" * 64)
        result = loop.resume_review_wait(fixtures.REQUEST, fixtures.COORDINATOR, "worker-a", expected)
        self.assertEqual(result["lanes"]["worker-a"]["phase"], "review_pending")
        self.assertEqual(len(self.starts), 1)
        self.assertTrue((loop.root() / "review-wait-recoveries" / f"{expected}.json").exists())
        with self.assertRaisesRegex(ValueError, "exact inspected"):
            loop.resume_review_wait(fixtures.REQUEST, fixtures.COORDINATOR, "worker-a", expected)

    def test_production_help_review_is_not_replaced_by_predeclared_text(self):
        self.register()
        for _ in range(4):
            self.tick()
        with patch.object(loop, "is_production_path", return_value=True), patch.object(checkpoints, "validate") as validate:
            data = self.tick()
            validate.assert_not_called()
        self.assertEqual(data["phase"], "paused")
        self.assertIn("fresh coordinator Help review", data["lanes"]["worker-a"]["reason"])

    def test_exact_post_implementation_help_review_resumes_same_run(self):
        self.register()
        self.assertIsInstance(self.tick()["registered_at_ms"], int)
        for _ in range(3):
            self.tick()
        with patch.object(loop, "is_production_path", return_value=True):
            data = self.tick()
        subject = loop.help_review_subject(data, "worker-a")
        spec = {"loop_sha256": loop.inspection_digest(data), "subject": "worker-a", "decision": "none",
                "reason": "Internal typed asset residency only; no player-visible input, label, or workflow changed.",
                "source_sha256": subject["source_sha256"], "paths": subject["paths"]}
        with patch.object(loop, "is_production_path", return_value=True):
            resumed = loop.submit_help_review(fixtures.REQUEST, fixtures.COORDINATOR, spec)
        self.assertEqual(resumed["phase"], "active")
        self.assertEqual(loop.owned_run_id(resumed), "run_fixture")
        self.assertEqual(resumed["lanes"]["worker-a"]["phase"], "validating")
        review = resumed["lanes"]["worker-a"]["help_review"]
        self.assertEqual(review["source_sha256"], subject["source_sha256"])
        self.assertTrue(Path(review["receipt"]).is_file())
        evidence = {"id": "e" * 64, "exit_code": 0}
        with (patch.object(loop, "is_production_path", return_value=True),
              patch.object(checkpoints, "validate", return_value=evidence) as validate):
            progressed = self.tick()
        self.assertEqual(progressed["lanes"]["worker-a"]["phase"], "checkpointing")
        self.assertEqual(validate.call_args.args[-2:], (spec["reason"], "none"))

    def test_late_coordinator_pause_reuses_completed_validation_recovery(self):
        data = self.register()
        source = roles.fingerprint(self.repo)
        evidence_id = "e" * 64
        replacement = ["git", "diff", "--check", self.ticket["base"]]
        reason = "The unchanged source uses the explicitly narrowed validation command."
        recovery_dir = loop.STORAGE.checked_directory(loop.root() / "validation-resumes")
        recovery_path = recovery_dir / "late-pause.json"
        loop.STORAGE.write_ledger(recovery_path, {
            "schema": 1, "request_id": fixtures.REQUEST, "run_id": "run_fixture",
            "subject": "worker-a", "evidence": evidence_id, "source_sha256": source,
            "failed_settlement": "fixture", "reason": reason, "replacement_argv": replacement,
        })
        lane = data["lanes"]["worker-a"]
        lane.update(phase="validating", evidence={"id": evidence_id, "source_sha256": source},
                    validation_recovery=str(recovery_path))
        lane["validation"]["argv"] = replacement
        data["run"] = {"phase": "ready", "context": {"id": "run_fixture", "consumer_generation": 1}}
        data.update(phase="paused", reason="coordinator escalation decision: late response")
        loop.save(data)
        inspected = loop.load(fixtures.REQUEST)
        spec = {"loop_sha256": loop.inspection_digest(inspected), "subject": "worker-a",
                "validation_evidence": evidence_id, "source_sha256": source,
                "reason": reason, "replacement_argv": replacement}
        with patch.object(dispatch.ui_coordinator, "read_registered_state", return_value={
                "terminal": fixtures.COORDINATOR, "phase": "ready"}):
            resumed = loop.resume_coordinator_validation(fixtures.REQUEST, fixtures.COORDINATOR, spec)
        self.assertEqual(resumed["phase"], "active")
        self.assertIsNone(resumed["reason"])
        self.assertEqual(resumed["lanes"]["worker-a"]["phase"], "validating")
        resumed["lanes"]["worker-a"].update(
            phase="paused", reason="ValueError: checkpoint requires exact successful worker exit"
        )
        resumed.update(phase="paused", reason=None)
        loop.save(resumed)
        inspected = loop.load(fixtures.REQUEST)
        spec["loop_sha256"] = loop.inspection_digest(inspected)
        with patch.object(dispatch.ui_coordinator, "read_registered_state", return_value={
                "terminal": fixtures.COORDINATOR, "phase": "ready"}):
            retried = loop.resume_coordinator_validation(fixtures.REQUEST, fixtures.COORDINATOR, spec)
        self.assertEqual(retried["phase"], "active")
        self.assertEqual(retried["lanes"]["worker-a"]["phase"], "validating")

    def test_help_review_refuses_old_loop_and_invalidates_changed_source(self):
        self.register()
        for _ in range(4):
            self.tick()
        with patch.object(loop, "is_production_path", return_value=True):
            data = self.tick()
        subject = loop.help_review_subject(data, "worker-a")
        spec = {"loop_sha256": "0" * 64, "subject": "worker-a", "decision": "none",
                "reason": "Internal fixture only; no player-visible behavior changed.",
                "source_sha256": subject["source_sha256"], "paths": subject["paths"]}
        with self.assertRaisesRegex(ValueError, "exact inspected"):
            loop.submit_help_review(fixtures.REQUEST, fixtures.COORDINATOR, spec)
        spec["loop_sha256"] = loop.inspection_digest(data)
        with patch.object(loop, "is_production_path", return_value=True):
            loop.submit_help_review(fixtures.REQUEST, fixtures.COORDINATOR, spec)
        (self.repo / "src/content.txt").write_text("changed after Help review")
        with (patch.object(loop, "is_production_path", return_value=True),
              patch.object(checkpoints, "validate") as validate):
            paused = self.tick()
        validate.assert_not_called()
        self.assertEqual(paused["phase"], "paused")
        self.assertNotIn("help_review", paused["lanes"]["worker-a"])

    def test_inspection_exposes_exact_help_review_resume_inputs(self):
        self.register()
        for _ in range(4):
            self.tick()
        with patch.object(loop, "is_production_path", return_value=True):
            data = self.tick()
        observed = loop.inspection(data)
        self.assertEqual(observed["loop_sha256"], loop.inspection_digest(data))
        self.assertEqual(observed["loop"], data)
        self.assertEqual(observed["pending_help_reviews"], [loop.help_review_subject(data, "worker-a")])

    def test_help_review_inspection_ignores_activity_clock_only(self):
        self.register()
        for _ in range(4):
            self.tick()
        with patch.object(loop, "is_production_path", return_value=True):
            data = self.tick()
        observed = loop.inspection(data)
        data["updated_at_ms"] += 1
        self.assertEqual(loop.inspection(data)["loop_sha256"], observed["loop_sha256"])
        data["reason"] = "different semantic state"
        self.assertNotEqual(loop.inspection(data)["loop_sha256"], observed["loop_sha256"])

    def test_corrupt_ledger_is_preserved(self):
        self.register()
        path = loop.state_path(fixtures.REQUEST)
        path.write_text('{"data": {"schema": 999}}')
        before = path.read_bytes()
        with self.assertRaises(ValueError):
            self.tick()
        self.assertEqual(path.read_bytes(), before)

    def test_coordinator_change_is_not_silent_takeover(self):
        self.register()
        with self.assertRaisesRegex(ValueError, "coordinator changed"):
            loop.tick(fixtures.REQUEST, "term_other")

    def test_only_structured_exact_review_is_accepted(self):
        for body in ("looks good", "ORCA_REVIEW_JSON: {}", "ORCA_REVIEW_JSON: {}\nORCA_REVIEW_JSON: {}",
                     'ORCA_REVIEW_JSON: {"verdict":"approved","verdict":"changes_requested"}'):
            with self.subTest(body=body), self.assertRaises((ValueError, RuntimeError)):
                loop.parse_review(body, self.sessions["reviewer"])

    def test_driver_stops_with_its_context_and_does_not_run_detached(self):
        called = threading.Event()
        with patch.object(loop, "tick", side_effect=lambda *_: called.set()):
            with loop.Driver(fixtures.REQUEST, fixtures.COORDINATOR) as driver:
                self.assertTrue(called.wait(2))
            self.assertFalse(driver.thread.is_alive())

    def test_release_requires_exact_dispatch_and_confirmed_cleanup(self):
        attempt = {"dispatch_id": "ctx_fixture", "run_id": "run_fixture",
                   "task_id": "task_fixture", "outcome": "completed"}
        observed = {"dispatch": {"id": "ctx_fixture", "runId": "run_fixture", "taskId": "task_fixture",
                                 "status": "completed"},
                    "projection": {"resource": {"terminalState": "retained", "releaseState": "not_requested"}}}
        with patch.object(dispatch, "run_cli", side_effect=[observed, {}, observed]) as cli:
            RELEASE(attempt)
            self.assertEqual(cli.call_count, 3)
        wrong = copy.deepcopy(observed)
        wrong["dispatch"]["id"] = "ctx_old_generation"
        with patch.object(dispatch, "run_cli", return_value=wrong) as cli, self.assertRaises(ValueError):
            RELEASE(attempt)
        self.assertEqual(cli.call_count, 1)
        pending = copy.deepcopy(observed)
        pending["projection"]["resource"]["terminalState"] = "release_pending"
        with patch.object(dispatch, "run_cli", side_effect=[observed, {}, pending]), self.assertRaises(ValueError):
            RELEASE(attempt)


if __name__ == "__main__":
    unittest.main()
