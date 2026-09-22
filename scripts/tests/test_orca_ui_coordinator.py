from __future__ import annotations

import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import (
    host_coordination,
    orca_frontdesk,
    orca_issue_context,
    orca_ui_coordinator as ui,
)


WORKSPACE = "eeac8301-ddb2-4c31-8a6c-e2a7f2fc7efb"
ISSUE = "884ddcd2-cef6-4869-a88d-14512684cce7"
DIGEST = "a" * 64
REQUEST = str(
    uuid.uuid5(orca_issue_context.REQUEST_NAMESPACE, f"{WORKSPACE}\n{ISSUE}\n{DIGEST}")
)
TERMINAL = "term_175c1be5-9f01-4a44-8268-a0542fa4e781"


class UiCoordinatorTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(
            prefix="orca-ui-coordinator-", dir=target
        )
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for module in (host_coordination, orca_frontdesk, orca_issue_context, ui):
            mock = patch.object(
                module, "state_root", return_value=self.root / "coordination"
            )
            mock.start()
            self.addCleanup(mock.stop)
        orca_frontdesk.submit("immutable Linear request", REQUEST)
        orca_frontdesk.write_ledger(
            orca_issue_context.ledger_path(),
            {
                "schema": 1,
                "imports": [
                    {
                        "request_id": REQUEST,
                        "workspace_id": WORKSPACE,
                        "issue_id": ISSUE,
                        "identifier": "HW-42",
                        "snapshot_sha256": DIGEST,
                        "created_at": "2026-09-22T00:00:00+00:00",
                    }
                ],
            },
        )
        environment = {
            "ORCA_TERMINAL_HANDLE": TERMINAL,
            "ORCA_WORKTREE_ID": f"fixture::{ui.REPO}",
        }
        env = patch.dict(ui.os.environ, environment, clear=False)
        env.start()
        self.addCleanup(env.stop)

    def ready_coordinator(self) -> None:
        imported = {"request_id": REQUEST, "linear_identifier": "HW-42"}
        with patch.object(ui.intake, "import_current_issue", return_value=imported):
            ui.prepare()
        with patch.object(ui, "pin_coordinator_title") as pin:
            ui.acknowledge(REQUEST)
        pin.assert_called_once_with(TERMINAL)

    def handoff_body(
        self, content: str = "目的: 新仕様を実装する\n受入条件: 既存仕様を置換する"
    ) -> Path:
        path = self.root / "handoff.md"
        path.write_text(content, encoding="utf-8")
        path.chmod(0o600)
        return path

    @staticmethod
    def issue_response(identifier: str = "HW-43") -> dict:
        return {
            "ok": True,
            "result": {
                "issue": {
                    "identifier": identifier,
                    "url": f"https://linear.app/example/issue/{identifier}",
                },
            },
        }

    @staticmethod
    def worktree(identifier: str = "HW-43") -> dict:
        path = "/tmp/orca-hw-43"
        return {
            "id": f"fixture::{path}",
            "path": path,
            "repoId": "fixture",
            "linkedLinearIssue": identifier,
        }

    @staticmethod
    def coordinator_terminal() -> dict:
        return {
            "handle": "term_8e4142eb-9cc8-4271-885f-4146b1d312fd",
            "worktreeId": "fixture::/tmp/orca-hw-43",
            "title": "統括",
            "orphaned": False,
        }

    def test_prepare_and_acknowledge_bind_visible_terminal(self) -> None:
        imported = {"request_id": REQUEST, "linear_identifier": "HW-42"}
        with patch.object(ui.intake, "import_current_issue", return_value=imported):
            result, state = ui.prepare()

        self.assertEqual(result, imported)
        self.assertEqual(state["phase"], "starting")
        with patch.object(ui, "pin_coordinator_title") as pin:
            ready = ui.acknowledge(REQUEST)
        pin.assert_called_once_with(TERMINAL)
        self.assertTrue(ready["ready"])
        self.assertEqual(ui.require_ready(REQUEST, TERMINAL)["phase"], "ready")
        with self.assertRaisesRegex(ui.UiCoordinatorError, "配車元"):
            ui.require_ready(REQUEST, "term_other")

    def test_request_view_keeps_routing_internal(self) -> None:
        view = ui.request_view(REQUEST)
        self.assertEqual(view["linear_identifier"], "HW-42")
        self.assertIn("immutable Linear request", view["immutable_request"])
        self.assertIn("Cursor CLI", view["routing"]["implementation_b"])

    def test_prompt_forbids_requesting_internal_ids_from_user(self) -> None:
        value = ui.prompt(REQUEST, "HW-42", ui.REPO)
        self.assertIn("workspace UUID", value)
        self.assertIn("入力・選択させてはいけません", value)
        self.assertIn("実装BはCursor CLI", value)
        self.assertIn("acknowledge", value)
        self.assertIn("利用者へ課題作成や", value)
        self.assertIn("orca_ui_coordinator.py handoff", value)
        self.assertIn("旧タブは\n自動終了", value)

    def test_handoff_creates_child_issue_and_activated_worktree(self) -> None:
        self.ready_coordinator()
        body = self.handoff_body()
        responses = [
            (0, self.issue_response()),
            (0, {"ok": True, "result": {"worktrees": []}}),
            (0, {"ok": True, "result": {}}),
            (0, {"ok": True, "result": {"worktrees": [self.worktree()]}}),
            (0, {"ok": True, "result": {"terminals": []}}),
            (0, {"ok": True, "result": {}}),
            (
                0,
                {
                    "ok": True,
                    "result": {"terminals": [self.coordinator_terminal()]},
                },
            ),
        ]
        with (
            patch.object(
                ui, "resolve_source_ref", return_value=("feature/source", "b" * 40)
            ),
            patch.object(ui, "run_orca_response", side_effect=responses) as run,
        ):
            result = ui.handoff(
                REQUEST, "新仕様へ切り替える", str(body), "feature/source"
            )

        self.assertTrue(result["ready"])
        self.assertEqual(result["linear_identifier"], "HW-43")
        linear_args, linear_body = run.call_args_list[0].args
        self.assertEqual(linear_args[:2], ["linear", "create"])
        self.assertNotIn("--parent", linear_args)
        self.assertEqual(linear_args[linear_args.index("--workspace") + 1], WORKSPACE)
        write_id = uuid.UUID(linear_args[linear_args.index("--write-id") + 1])
        self.assertEqual(write_id.version, 4)
        self.assertIn("目的: 新仕様を実装する", linear_body)
        create_args = run.call_args_list[2].args[0]
        self.assertIn("--linear-issue", create_args)
        self.assertIn("--activate", create_args)
        self.assertIn("--no-parent", create_args)
        terminal_args = run.call_args_list[5].args[0]
        self.assertEqual(terminal_args[:2], ["terminal", "create"])
        self.assertEqual(terminal_args[terminal_args.index("--title") + 1], "統括")
        self.assertIn(
            "launch-wait", terminal_args[terminal_args.index("--command") + 1]
        )
        self.assertIn("--focus", terminal_args)
        self.assertEqual(run.call_count, 7)

        with (
            patch.object(
                ui, "resolve_source_ref", return_value=("feature/source", "b" * 40)
            ),
            patch.object(ui, "run_orca_response") as second_run,
        ):
            repeated = ui.handoff(
                REQUEST, "新仕様へ切り替える", str(body), "feature/source"
            )
        self.assertEqual(repeated, result)
        second_run.assert_not_called()

    def test_handoff_reuses_existing_linked_worktree(self) -> None:
        self.ready_coordinator()
        body = self.handoff_body()
        responses = [
            (0, self.issue_response()),
            (0, {"ok": True, "result": {"worktrees": [self.worktree()]}}),
            (
                0,
                {
                    "ok": True,
                    "result": {"terminals": [self.coordinator_terminal()]},
                },
            ),
        ]
        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui, "run_orca_response", side_effect=responses) as run,
        ):
            result = ui.handoff(REQUEST, "局所修正", str(body), None)
        self.assertTrue(result["ready"])
        self.assertEqual(run.call_count, 3)

    def test_handoff_retries_unconfirmed_write_once_and_stops(self) -> None:
        self.ready_coordinator()
        body = self.handoff_body()

        def unconfirmed(
            arguments: list[str], _input: str | None = None
        ) -> tuple[int, dict]:
            write_id = arguments[arguments.index("--write-id") + 1]
            return 1, {
                "ok": False,
                "error": {"code": "linear_write_unconfirmed", "writeId": write_id},
            }

        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui, "run_orca_response", side_effect=unconfirmed) as run,
        ):
            with self.assertRaisesRegex(ui.UiCoordinatorError, "確定できません"):
                ui.handoff(REQUEST, "重複させない", str(body), None)
        self.assertEqual(run.call_count, 2)
        state_file = next(
            ui.handoff_root().glob("????????-????-????-????-????????????.json")
        )
        state = orca_frontdesk.read_private_json(state_file, {})
        self.assertEqual(state["phase"], "unknown")
        self.assertEqual(state["last_error_code"], "linear_write_unconfirmed")
        self.assertIsNone(state["issue_identifier"])

        responses = [
            (0, self.issue_response()),
            (0, {"ok": True, "result": {"worktrees": [self.worktree()]}}),
            (
                0,
                {
                    "ok": True,
                    "result": {"terminals": [self.coordinator_terminal()]},
                },
            ),
        ]
        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui, "run_orca_response", side_effect=responses) as resumed,
        ):
            result = ui.handoff(REQUEST, "重複させない", str(body), None)
        self.assertTrue(result["ready"])
        resumed_args = resumed.call_args_list[0].args[0]
        self.assertEqual(
            resumed_args[resumed_args.index("--write-id") + 1],
            state["linear_write_id"],
        )

    def test_handoff_marks_confirmed_write_failure_without_retry(self) -> None:
        self.ready_coordinator()
        body = self.handoff_body()
        response = (1, {"ok": False, "error": {"code": "linear_write_failed"}})
        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui, "run_orca_response", return_value=response) as run,
        ):
            with self.assertRaisesRegex(ui.UiCoordinatorError, "linear_write_failed"):
                ui.handoff(REQUEST, "確定失敗", str(body), None)
        self.assertEqual(run.call_count, 1)
        state_file = next(
            ui.handoff_root().glob("????????-????-????-????-????????????.json")
        )
        state = orca_frontdesk.read_private_json(state_file, {})
        self.assertEqual(state["phase"], "failed")
        self.assertEqual(state["last_error_code"], "linear_write_failed")

    def test_handoff_recovers_legacy_uuid5_unknown_state(self) -> None:
        self.ready_coordinator()
        body = self.handoff_body()
        legacy_write_id = str(uuid.uuid5(uuid.NAMESPACE_URL, "legacy"))

        def unconfirmed(
            arguments: list[str], _input: str | None = None
        ) -> tuple[int, dict]:
            write_id = arguments[arguments.index("--write-id") + 1]
            return 1, {
                "ok": False,
                "error": {
                    "code": "linear_write_unconfirmed",
                    "writeId": write_id,
                },
            }

        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui, "run_orca_response", side_effect=unconfirmed),
        ):
            with self.assertRaises(ui.UiCoordinatorError):
                ui.handoff(REQUEST, "旧状態復旧", str(body), None)
        state_file = next(
            ui.handoff_root().glob("????????-????-????-????-????????????.json")
        )
        state = orca_frontdesk.read_private_json(state_file, {})
        self.assertEqual(state["phase"], "unknown")
        state["schema"] = 1
        state["linear_write_id"] = legacy_write_id
        del state["last_error_code"]
        del state["coordinator_terminal"]
        orca_frontdesk.write_ledger(state_file, state)

        recovered_write_id = uuid.UUID("58d5bf9b-25c6-43ff-8939-c329182ff729")
        responses = [
            (0, self.issue_response()),
            (0, {"ok": True, "result": {"worktrees": [self.worktree()]}}),
            (
                0,
                {
                    "ok": True,
                    "result": {"terminals": [self.coordinator_terminal()]},
                },
            ),
        ]
        with (
            patch.object(ui, "resolve_source_ref", return_value=(None, "b" * 40)),
            patch.object(ui.uuid, "uuid4", return_value=recovered_write_id),
            patch.object(ui, "run_orca_response", side_effect=responses) as run,
        ):
            result = ui.handoff(REQUEST, "旧状態復旧", str(body), None)
        self.assertTrue(result["ready"])
        args = run.call_args_list[0].args[0]
        self.assertEqual(args[args.index("--write-id") + 1], str(recovered_write_id))

    def test_handoff_body_rejects_internal_context(self) -> None:
        body = self.handoff_body("内部: /home/example/session")
        with self.assertRaisesRegex(ui.UiCoordinatorError, "ローカルパス"):
            ui.read_handoff_body(str(body))

    def test_registered_coordinator_survives_orca_title_rename(self) -> None:
        self.ready_coordinator()
        response = {
            "ok": True,
            "result": {
                "terminals": [
                    {
                        "handle": TERMINAL,
                        "worktreeId": f"fixture::{ui.REPO}",
                        "title": "確認する HW-42依頼 | fixture",
                        "orphaned": False,
                    }
                ]
            },
        }
        with patch.object(ui, "run_orca_response", return_value=(0, response)):
            terminals = ui.list_coordinator_terminals(f"fixture::{ui.REPO}")
        self.assertEqual(terminals, [{"handle": TERMINAL}])

    def test_launch_wait_retries_only_busy_coordinator(self) -> None:
        with (
            patch.object(
                ui, "launch", side_effect=[host_coordination.HostBusyError("busy"), 0]
            ) as launch,
            patch.object(ui.time, "sleep") as sleep,
            patch.object(ui.time, "monotonic", side_effect=[0.0, 0.0]),
        ):
            self.assertEqual(ui.launch_wait(timeout_seconds=5), 0)
        self.assertEqual(launch.call_count, 2)
        sleep.assert_called_once_with(1)

    def test_provider_is_workspace_sandboxed_and_has_project_control_paths(
        self,
    ) -> None:
        provider = ui.provider_command("/bin/codex", "prompt")
        self.assertIn("--dangerously-bypass-approvals-and-sandbox", provider)
        self.assertIn("gpt-5.6-sol", provider)
        self.assertIn('model_reasoning_effort="high"', provider)
        self.assertIn("mcp_servers.rust-analyzer-mcp.enabled=false", provider)
        runtime = ui.prepare_runtime(ui.REPO)
        config = runtime / "codex/config.toml"
        self.assertEqual(config.stat().st_mode & 0o777, 0o600)
        self.assertIn(f'[projects."{ui.REPO}"]', config.read_text(encoding="utf-8"))
        with patch.object(ui.shutil, "which", return_value="/usr/bin/bwrap"):
            command = ui.sandbox_command(provider, ui.REPO, runtime)
        self.assertEqual(command[0], "/usr/bin/bwrap")
        self.assertIn("--ro-bind", command)
        self.assertNotIn("--unshare-net", command)
        binds = [
            command[index + 1]
            for index, value in enumerate(command)
            if value == "--bind"
        ]
        self.assertEqual(binds, [str(self.root), str(ui.REPO.parent), str(ui.REPO)])
        self.assertEqual(command[command.index("--chdir") + 1], str(ui.REPO))


if __name__ == "__main__":
    unittest.main()
