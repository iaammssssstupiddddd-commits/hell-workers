from __future__ import annotations

import tempfile
import unittest
import uuid
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts import host_coordination, orca_frontdesk as frontdesk
from scripts import orca_supervision as supervision


RUNTIME = "81ee13d4-6612-4253-9d56-9af03b390c09"


class SupervisionTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-supervision-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.state = self.root / "panel"
        for module in (host_coordination, frontdesk):
            mock = patch.object(module, "state_root", return_value=self.root / "coordination")
            mock.start()
            self.addCleanup(mock.stop)

    def request(self, text: str = "保存領域の修正を依頼します") -> dict:
        operation = str(uuid.uuid4())
        value = {"schema": 1, "runtimeId": RUNTIME, "expectedRuntimeId": RUNTIME,
                 "operationId": operation, "workflowId": supervision.RECEPTION_ID,
                 "revision": "1", "action": "submit", "text": text}
        _, requests, _ = supervision.directories(self.state)
        frontdesk.write_ledger(requests / f"{operation}.json", value)
        return value

    def lifecycle_request(self, workflow_id: str, action: str, revision: str = "1") -> dict:
        operation = str(uuid.uuid4())
        value = {"schema": 1, "runtimeId": RUNTIME, "expectedRuntimeId": RUNTIME,
                 "operationId": operation, "workflowId": workflow_id,
                 "revision": revision, "action": action, "text": ""}
        _, requests, _ = supervision.directories(self.state)
        frontdesk.write_ledger(requests / f"{operation}.json", value)
        return value

    def accepted_implementation(self) -> str:
        request = self.request()
        request["action"] = "implement"
        frontdesk.write_ledger(self.state / "requests" / f"{request['operationId']}.json", request)
        supervision.tick(self.state, RUNTIME)
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request["operationId"]), {
            "schema": 1, "requestId": request["operationId"], "phase": "ready"})
        return request["operationId"]

    def test_reception_publishes_without_an_agent_or_terminal(self) -> None:
        snapshot = supervision.tick(self.state, RUNTIME)
        self.assertEqual(len(snapshot["workflows"]), 1)
        self.assertEqual(snapshot["workflows"][0]["actions"], ["submit", "implement"])
        self.assertTrue(all(role["terminal"] is None for role in snapshot["workflows"][0]["roles"]))
        self.assertEqual(frontdesk.list_requests(), [])
        self.assertEqual((self.state / "snapshot.json").stat().st_mode & 0o777, 0o600)

    def test_submit_is_durable_and_replay_does_not_create_a_second_intake(self) -> None:
        request = self.request()
        first = supervision.tick(self.state, RUNTIME)
        second = supervision.tick(self.state, RUNTIME)
        self.assertEqual(first["workflows"][1]["state"], "queued")
        self.assertEqual(len(second["workflows"]), 2)
        self.assertEqual([item["id"] for item in frontdesk.list_requests()], [request["operationId"]])
        receipt = frontdesk.read_private_json(
            self.state / "receipts" / f"{request['operationId']}.json", {})
        self.assertEqual(receipt["phase"], "accepted")
        self.assertEqual(receipt["intakeId"], request["operationId"])

    def test_implementation_intent_is_distinct_from_consultation(self) -> None:
        request = self.request()
        request["action"] = "implement"
        frontdesk.write_ledger(
            self.state / "requests" / f"{request['operationId']}.json", request)
        first = supervision.tick(self.state, RUNTIME)
        self.assertEqual(first["workflows"][1]["state"], "queued")
        self.assertFalse(supervision.consult_intent(self.state, request["operationId"]))
        self.assertEqual(frontdesk.read_private_json(
            self.state / "receipts" / f"{request['operationId']}.json", {})["action"],
            "implement")

    def test_pause_resume_uses_revisioned_receipts_without_new_tabs(self) -> None:
        request_id = self.accepted_implementation()
        with patch.object(supervision, "preflight_pause") as preflight, \
                patch.object(supervision, "route_view", return_value=("working", "作業中", supervision.roles())):
            pause = self.lifecycle_request(request_id, "pause")
            paused = supervision.tick(self.state, RUNTIME)["workflows"][1]
            self.assertEqual((paused["state"], paused["revision"], paused["actions"]),
                             ("paused", "2", ["resume", "close"]))
            self.assertEqual(preflight.call_count, 1)
            self.assertEqual(supervision.tick(self.state, RUNTIME)["workflows"][1]["revision"], "2")
            self.assertEqual(frontdesk.read_private_json(
                self.state / "receipts" / f"{pause['operationId']}.json", {})["phase"], "accepted")
            self.lifecycle_request(request_id, "resume", "2")
            active = supervision.tick(self.state, RUNTIME)["workflows"][1]
            self.assertEqual((active["state"], active["revision"], active["actions"]),
                             ("working", "3", ["pause", "close"]))
            self.assertEqual(preflight.call_count, 2)

    def test_busy_close_is_rejected_without_closing_any_terminal(self) -> None:
        request_id = self.accepted_implementation()
        with patch.object(supervision, "preflight_close", side_effect=ValueError("agent is busy")), \
                patch.object(supervision, "finish_close") as finish, \
                patch.object(supervision, "route_view", return_value=("working", "作業中", supervision.roles())):
            close = self.lifecycle_request(request_id, "close")
            workflow = supervision.tick(self.state, RUNTIME)["workflows"][1]
            self.assertEqual((workflow["state"], workflow["revision"]), ("working", "2"))
            self.assertIn("agent is busy", workflow["detail"])
            self.assertEqual(frontdesk.read_private_json(
                self.state / "receipts" / f"{close['operationId']}.json", {})["phase"], "rejected")
            finish.assert_not_called()

    def test_lost_pause_receipt_recovers_without_repeating_preflight(self) -> None:
        request_id = self.accepted_implementation()
        request = self.lifecycle_request(request_id, "pause")
        write = frontdesk.write_ledger

        def interrupt(path: Path, value: dict) -> None:
            if path == self.state / "receipts" / f"{request['operationId']}.json":
                raise OSError("receipt write interrupted")
            write(path, value)

        with patch.object(supervision, "preflight_pause") as preflight, \
                patch.object(supervision, "route_view", return_value=("working", "作業中", supervision.roles())):
            with patch.object(frontdesk, "write_ledger", side_effect=interrupt):
                with self.assertRaisesRegex(OSError, "receipt write interrupted"):
                    supervision.tick(self.state, RUNTIME)
            self.assertEqual(supervision.lifecycle(self.state, request_id)["phase"], "paused")
            supervision.tick(self.state, RUNTIME)
            preflight.assert_called_once()
            self.assertEqual(supervision.task_revision(self.state, request_id), "2")

    def test_closed_consultation_is_retained_without_a_terminal(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        supervision.write_consult_phase(self.state, request["operationId"], "settled")
        self.lifecycle_request(request["operationId"], "close")
        workflow = supervision.tick(self.state, RUNTIME)["workflows"][1]
        self.assertEqual((workflow["state"], workflow["revision"], workflow["actions"]),
                         ("closed", "2", []))
        self.assertTrue(all(role["terminal"] is None for role in workflow["roles"]))

    def test_clean_exited_coordinator_can_preflight_close(self) -> None:
        request_id = self.accepted_implementation()
        child = str(uuid.uuid4())
        worktree_id = f"repo::{self.root}"
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "phase": "ready",
            "childRequestId": child, "worktreeId": worktree_id})
        state = {"phase": "exited", "exit_code": 0,
                 "worktree_id": worktree_id, "terminal": "term_fixture"}
        identity = {"handle": "term_fixture", "worktreeId": worktree_id,
                    "incarnationId": "inc_fixture"}
        responses = [(0, {"ok": True, "result": {"terminal": {"handle": "term_fixture"}}}),
                     (0, {"ok": True, "result": {
                         "terminals": [{"handle": "term_fixture"}],
                         "visualLayouts": [{"root": {"tabs": [{}]}}], "truncated": False,
                         "hostScope": {"omittedHostIds": []}}})]
        with patch.object(supervision.ui, "read_registered_state", return_value=state), \
                patch.object(supervision.ui, "run_orca_response", side_effect=responses), \
                patch.object(supervision.role_tabs, "identity", return_value=identity), \
                patch.object(supervision.role_tabs, "idle_shell"), \
                patch.object(supervision.subprocess, "run", return_value=SimpleNamespace(
                    returncode=0, stdout="", stderr="")):
            supervision.preflight_close(self.state, request_id, {"action": "implement"})

    def test_close_refuses_a_replacement_default_tab(self) -> None:
        request_id = self.accepted_implementation()
        child = str(uuid.uuid4())
        worktree_id = f"repo::{self.root}"
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "phase": "ready",
            "childRequestId": child, "worktreeId": worktree_id})
        state = {"worktree_id": worktree_id, "terminal": "term_fixture"}
        responses = [(0, {"ok": True, "result": {"terminal": {"handle": "term_fixture"}}}),
                     (0, {"ok": True, "result": {"terminals": [{"handle": "new_default"}],
                                              "truncated": False,
                                              "hostScope": {"omittedHostIds": []}}})]
        with patch.object(supervision.ui, "read_registered_state", return_value=state), \
                patch.object(supervision.ui, "run_orca_response", side_effect=responses), \
                patch.object(supervision.role_tabs, "identity", return_value={
                    "handle": "term_fixture", "worktreeId": worktree_id}), \
                patch.object(supervision.role_tabs, "retire") as retire:
            with self.assertRaisesRegex(ValueError, "not tab-free"):
                supervision.finish_close(self.state, request_id, {"schema": 1}, {"action": "implement"})
            retire.assert_called_once()

    def test_uncertain_close_never_replays_an_existing_retirement(self) -> None:
        request_id = self.accepted_implementation()
        operation = str(uuid.uuid4())
        worktree_id = f"repo::{self.root}"
        child = str(uuid.uuid4())
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "phase": "ready",
            "childRequestId": child, "worktreeId": worktree_id})
        frontdesk.write_ledger(supervision.lifecycle_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "revision": 2,
            "operationId": operation, "phase": "unknown", "outcome": "accepted"})
        frontdesk.write_ledger(self.state / "receipts" / f"{operation}.json", {
            "operationId": operation, "workflowId": request_id,
            "action": "close", "phase": "accepted"})
        identity = {"handle": "term_fixture", "worktreeId": worktree_id,
                    "incarnationId": "inc_fixture"}
        retired_root = self.root / "role-tabs"
        frontdesk.write_ledger(retired_root / "retired" /
                               f"{supervision.role_tabs.bindings.digest(identity)}.json",
                               {"phase": "prepared", "identity": identity})
        with patch.object(supervision, "preflight_close"), \
                patch.object(supervision.ui, "read_registered_state", return_value={
                    "worktree_id": worktree_id, "terminal": "term_fixture"}), \
                patch.object(supervision.ui, "run_orca_response", return_value=(0, {
                    "ok": True, "result": {"terminal": {"handle": "term_fixture"}}})), \
                patch.object(supervision.role_tabs, "identity", return_value=identity), \
                patch.object(supervision.role_tabs, "root", return_value=retired_root), \
                patch.object(supervision, "finish_close") as finish:
            with self.assertRaisesRegex(ValueError, "close result is uncertain"):
                supervision.reconcile_unfinished_close(self.state, request_id)
            finish.assert_not_called()

    def test_returned_close_receipt_completes_by_readback_without_replay(self) -> None:
        request_id = self.accepted_implementation()
        operation = str(uuid.uuid4())
        worktree_id = f"repo::{self.root}"
        child = str(uuid.uuid4())
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "phase": "ready",
            "childRequestId": child, "worktreeId": worktree_id})
        frontdesk.write_ledger(supervision.lifecycle_path(self.state, request_id), {
            "schema": 1, "requestId": request_id, "revision": 2,
            "operationId": operation, "phase": "unknown", "outcome": "accepted"})
        frontdesk.write_ledger(self.state / "receipts" / f"{operation}.json", {
            "operationId": operation, "workflowId": request_id,
            "action": "close", "phase": "accepted"})
        retired_root = self.root / "role-tabs"
        frontdesk.write_ledger(retired_root / "retired" / "fixture.json", {
            "identity": {"handle": "term_fixture", "worktreeId": worktree_id},
            "phase": "close-returned", "receipt": {"close": {
                "handle": "term_fixture", "ptyKilled": True}}})
        with patch.object(supervision.ui, "read_registered_state", return_value={
                    "worktree_id": worktree_id, "terminal": "term_fixture"}), \
                patch.object(supervision.role_tabs, "root", return_value=retired_root), \
                patch.object(supervision, "verify_tab_free") as verified, \
                patch.object(supervision, "finish_close") as finish:
            result = supervision.reconcile_unfinished_close(self.state, request_id)
        self.assertEqual(result["phase"], "closed")
        verified.assert_called_once_with(self.root)
        finish.assert_not_called()

    def test_implementation_route_exposes_only_a_verified_coordinator_terminal(self) -> None:
        request = self.request()
        request["action"] = "implement"
        frontdesk.write_ledger(
            self.state / "requests" / f"{request['operationId']}.json", request)
        supervision.tick(self.state, RUNTIME)
        route_path = supervision.routing.route_path(self.state, request["operationId"])
        frontdesk.write_ledger(route_path, {
            "schema": 1, "requestId": request["operationId"], "phase": "worktree_created",
            "childRequestId": "6c42fd6e-1930-4d79-a3e5-845ccf478b75",
            "worktreeId": "repo::/tmp/implementation", "issueIdentifier": "TAK-99"})
        state_path = self.root / "coordinator.json"
        state_path.write_text("registered")
        state = {"phase": "ready", "worktree_id": "repo::/tmp/implementation",
                 "linear_identifier": "TAK-99", "terminal": "term-1"}
        terminal = {"handle": "term-1", "incarnationId": "inc-1",
                    "worktreeId": state["worktree_id"], "executionHostId": "local",
                    "orphaned": False, "connected": True}
        with patch.object(supervision.ui, "state_path", return_value=state_path), \
                patch.object(supervision.ui, "read_registered_state", return_value=state), \
                patch.object(supervision.ui, "run_orca_response", return_value=(0, {
                    "ok": True, "result": {"terminal": terminal}})):
            workflow = supervision.tick(self.state, RUNTIME)["workflows"][1]
        self.assertEqual(workflow["state"], "working")
        self.assertEqual(workflow["roles"][0]["terminal"], {
            key: terminal[key] for key in
            ("handle", "incarnationId", "worktreeId", "executionHostId")})
        self.assertTrue(all(role["terminal"] is None for role in workflow["roles"][1:]))

    def test_implementation_creation_unknown_never_spawns_another_router(self) -> None:
        request = self.request()
        request["action"] = "implement"
        frontdesk.write_ledger(
            self.state / "requests" / f"{request['operationId']}.json", request)
        supervision.tick(self.state, RUNTIME)
        frontdesk.write_ledger(supervision.routing.route_path(self.state, request["operationId"]), {
            "schema": 1, "requestId": request["operationId"], "phase": "issue_creating"})
        with patch.object(supervision.subprocess, "Popen") as spawn:
            self.assertIsNone(supervision.start_route(self.state))
            spawn.assert_not_called()

    def test_explicit_implementation_starts_only_the_guarded_router(self) -> None:
        request = self.request()
        request["action"] = "implement"
        frontdesk.write_ledger(
            self.state / "requests" / f"{request['operationId']}.json", request)
        supervision.tick(self.state, RUNTIME)
        child = unittest.mock.Mock()
        with patch.object(supervision.ui, "primary_repo", return_value=self.root), \
                patch.object(supervision.subprocess, "Popen", return_value=child) as spawn:
            self.assertEqual(supervision.start_route(self.state), (request["operationId"], child))
        command = spawn.call_args.args[0]
        self.assertEqual(Path(command[1]).name, "orca_supervision_route.py")
        self.assertIn(request["operationId"], command)
        self.assertNotIn("codex", command)
        self.assertNotIn("cursor", command)

    def test_failed_receipt_write_replays_the_same_intake(self) -> None:
        request = self.request()
        actual = frontdesk.write_ledger

        def interrupt(path: Path, value: dict) -> None:
            if path.parent.name == "receipts":
                raise OSError("receipt fsync interrupted")
            actual(path, value)

        with patch.object(frontdesk, "write_ledger", side_effect=interrupt):
            with self.assertRaisesRegex(OSError, "receipt fsync interrupted"):
                supervision.tick(self.state, RUNTIME)
        self.assertEqual(len(frontdesk.list_requests()), 1)
        self.assertEqual(supervision.tick(self.state, RUNTIME)["workflows"][1]["id"],
                         request["operationId"])
        self.assertEqual(len(frontdesk.list_requests()), 1)

    def test_other_runtime_and_changed_record_block_all_new_intake(self) -> None:
        request = self.request()
        with self.assertRaisesRegex(ValueError, "another runtime"):
            supervision.tick(self.state, str(uuid.uuid4()))
        self.assertEqual(frontdesk.list_requests(), [])
        request["runtimeId"] = str(uuid.uuid4())
        frontdesk.write_ledger(
            self.state / "requests" / f"{request['operationId']}.json", request)
        with self.assertRaisesRegex(ValueError, "invalid"):
            supervision.tick(self.state, RUNTIME)
        self.assertEqual(frontdesk.list_requests(), [])

    def test_confirmed_prior_boot_is_reconciled_without_second_intake(self) -> None:
        request = self.request()
        _, _, receipts = supervision.directories(self.state)
        frontdesk.submit(request["text"], request["operationId"])
        frontdesk.write_ledger(receipts / f"{request['operationId']}.json", {
            "schema": 1, "runtimeId": RUNTIME, "operationId": request["operationId"],
            "workflowId": supervision.RECEPTION_ID, "revision": "1", "action": "submit",
            "text": request["text"], "intakeId": request["operationId"], "phase": "accepted",
        })
        supervision.tick(self.state, str(uuid.uuid4()))
        self.assertEqual(len(frontdesk.list_requests()), 1)
        self.assertEqual(list((self.state / "requests").iterdir()), [])

    def test_unknown_entry_and_unsafe_request_are_not_skipped(self) -> None:
        _, requests, _ = supervision.directories(self.state)
        (requests / "unexpected").write_text("{}")
        with self.assertRaisesRegex(ValueError, "unexpected"):
            supervision.tick(self.state, RUNTIME)
        (requests / "unexpected").unlink()
        request = self.request()
        path = requests / f"{request['operationId']}.json"
        path.chmod(0o644)
        with self.assertRaisesRegex(RuntimeError, "unsafe"):
            supervision.tick(self.state, RUNTIME)
        self.assertEqual(frontdesk.list_requests(), [])

    def test_consultation_launch_is_durable_and_not_relaunched_after_restart(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                         "ready")
        child = unittest.mock.Mock()
        child.poll.return_value = None
        with patch.object(supervision.subprocess, "Popen", return_value=child) as spawn:
            started = supervision.start_consult(self.state)
            self.assertEqual(started, (request["operationId"], child))
            self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                             "launching")
            self.assertIsNone(supervision.start_consult(self.state))
            self.assertEqual(spawn.call_count, 1)
        supervision.reconcile_unowned_consults(self.state)
        self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                         "unknown")
        self.assertEqual(supervision.tick(self.state, RUNTIME)["workflows"][1]["state"],
                         "unknown")

    def test_consultation_success_requires_proven_provider_state(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        supervision.write_consult_phase(self.state, request["operationId"], "launching")
        child = unittest.mock.Mock(returncode=0)
        child.poll.return_value = 0
        with patch.object(supervision.coordinator, "read_state", return_value={
            "turns": [{"phase": "succeeded", "response": "相談回答"}]}) as read_state:
            supervision.settle_consult(self.state, request["operationId"], child)
            workflow = supervision.tick(self.state, RUNTIME)["workflows"][1]
            self.assertEqual(workflow["state"], "feedback")
            self.assertEqual(workflow["detail"], "相談回答")
            self.assertEqual(workflow["roles"][0]["state"], "waiting")
            self.assertTrue(read_state.called)

    def test_failed_provider_launch_is_unknown_and_never_retried(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        with patch.object(supervision.subprocess, "Popen", side_effect=OSError("denied")):
            with self.assertRaises(OSError):
                supervision.start_consult(self.state)
        self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                         "unknown")
        self.assertIsNone(supervision.start_consult(self.state))

    def test_test_state_requires_hidden_electron_and_child_of_private_panel(self) -> None:
        with patch.dict(supervision.os.environ, {
            "ORCA_SUPERVISION_TEST_STATE_ROOT": str(self.root / "outside"),
            "ORCA_E2E_HEADLESS": "1",
        }):
            with self.assertRaisesRegex(RuntimeError, "not proven"):
                supervision.configure_test_state(self.state)
        with patch.dict(supervision.os.environ, {
            "ORCA_SUPERVISION_TEST_STATE_ROOT": str(self.state / "test-frontdesk"),
            "ORCA_E2E_HEADLESS": "0",
        }):
            with self.assertRaisesRegex(RuntimeError, "not proven"):
                supervision.configure_test_state(self.state)

    def test_stop_only_signals_the_owned_consultation_process_group(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        supervision.write_consult_phase(self.state, request["operationId"], "launching")
        child = unittest.mock.Mock(pid=4321)
        child.poll.return_value = None
        with patch.object(supervision.os, "getpgid", return_value=9876), \
                patch.object(supervision.os, "killpg") as kill_group:
            with self.assertRaisesRegex(RuntimeError, "not owned"):
                supervision.stop_owned_consult(self.state, request["operationId"], child)
            kill_group.assert_not_called()
        self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                         "launching")

    def test_stop_marks_consultation_unknown_after_owned_child_exits(self) -> None:
        request = self.request()
        supervision.tick(self.state, RUNTIME)
        supervision.write_consult_phase(self.state, request["operationId"], "launching")
        child = unittest.mock.Mock(pid=4321)
        child.poll.return_value = None
        with patch.object(supervision.os, "getpgid", return_value=4321), \
                patch.object(supervision.os, "killpg") as kill_group:
            supervision.stop_owned_consult(self.state, request["operationId"], child)
        kill_group.assert_called_once_with(4321, supervision.signal.SIGTERM)
        child.wait.assert_called_once_with(timeout=5)
        self.assertEqual(supervision.consult_intent(self.state, request["operationId"])["phase"],
                         "unknown")


if __name__ == "__main__":
    unittest.main()
