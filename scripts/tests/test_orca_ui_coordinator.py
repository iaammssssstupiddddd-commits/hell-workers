from __future__ import annotations

import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import (host_coordination, orca_frontdesk, orca_issue_context,
                     orca_ui_coordinator as ui)


WORKSPACE = "eeac8301-ddb2-4c31-8a6c-e2a7f2fc7efb"
ISSUE = "884ddcd2-cef6-4869-a88d-14512684cce7"
DIGEST = "a" * 64
REQUEST = str(uuid.uuid5(
    orca_issue_context.REQUEST_NAMESPACE, f"{WORKSPACE}\n{ISSUE}\n{DIGEST}"
))
TERMINAL = "term_175c1be5-9f01-4a44-8268-a0542fa4e781"


class UiCoordinatorTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="orca-ui-coordinator-", dir=target)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for module in (host_coordination, orca_frontdesk, orca_issue_context, ui):
            mock = patch.object(module, "state_root", return_value=self.root / "coordination")
            mock.start()
            self.addCleanup(mock.stop)
        orca_frontdesk.submit("immutable Linear request", REQUEST)
        orca_frontdesk.write_ledger(orca_issue_context.ledger_path(), {
            "schema": 1,
            "imports": [{
                "request_id": REQUEST,
                "workspace_id": WORKSPACE,
                "issue_id": ISSUE,
                "identifier": "HW-42",
                "snapshot_sha256": DIGEST,
                "created_at": "2026-09-22T00:00:00+00:00",
            }],
        })
        environment = {
            "ORCA_TERMINAL_HANDLE": TERMINAL,
            "ORCA_WORKTREE_ID": f"fixture::{ui.REPO}",
        }
        env = patch.dict(ui.os.environ, environment, clear=False)
        env.start()
        self.addCleanup(env.stop)

    def test_prepare_and_acknowledge_bind_visible_terminal(self) -> None:
        imported = {"request_id": REQUEST, "linear_identifier": "HW-42"}
        with patch.object(ui.intake, "import_current_issue", return_value=imported):
            result, state = ui.prepare()

        self.assertEqual(result, imported)
        self.assertEqual(state["phase"], "starting")
        ready = ui.acknowledge(REQUEST)
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

    def test_provider_is_workspace_sandboxed_and_has_project_control_paths(self) -> None:
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
        binds = [command[index + 1] for index, value in enumerate(command) if value == "--bind"]
        self.assertEqual(binds, [str(self.root), str(ui.REPO.parent), str(ui.REPO)])
        self.assertEqual(command[command.index("--chdir") + 1], str(ui.REPO))


if __name__ == "__main__":
    unittest.main()
