from __future__ import annotations

import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import orca_providers as providers


class ProviderTests(unittest.TestCase):
    def simple(self) -> dict:
        return {"complexity": "simple", "task_kind": "test-addition",
                "complexity_reason": "existing pattern, one leaf, no API change",
                "acceptance": "focused test added; coordinator runs it",
                "allowed_directories": ["crates/hw_ui/src/interaction/help"]}

    def test_fixed_provider_mapping(self) -> None:
        self.assertEqual(providers.provider_for({}, "worker-a"), "codex")
        self.assertEqual(providers.provider_for({}, "reviewer"), "codex")
        self.assertEqual(providers.provider_for(self.simple(), "worker-b"), "cursor")
        with self.assertRaisesRegex(ValueError, "requires provider"):
            providers.provider_for({"provider": "cursor"}, "reviewer")

    def test_b_rejects_missing_or_complex_classification(self) -> None:
        for change in ({"complexity": "complex"}, {"complexity_reason": ""},
                       {"acceptance": " "}, {"task_kind": "architecture"}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                providers.provider_for({**self.simple(), **change}, "worker-b")
        with self.assertRaises(ValueError):
            providers.provider_for({}, "worker-b")

    def test_b_rejects_shared_infrastructure_and_broad_scopes(self) -> None:
        for scope in ("scripts/tests", "crates/hw_ui/src", "crates/hw_core/src/events",
                      "crates/hw_jobs/src/tasks", "crates/bevy_app/src/plugins/startup",
                      "crates/bevy_app/src/save", "crates/hw_visual/src/visual3d"):
            with self.subTest(scope=scope), self.assertRaisesRegex(ValueError, "leaf scope"):
                providers.provider_for({**self.simple(), "allowed_directories": [scope]}, "worker-b")

    def test_b_acceptance_probe_requires_explicit_read_only(self) -> None:
        ticket = {**self.simple(), "allowed_directories": [], "task_kind": "acceptance-probe"}
        self.assertEqual(providers.provider_for({**ticket, "read_only": True}, "worker-b"), "cursor")
        for value in (False, "true", None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                providers.provider_for({**ticket, "read_only": value}, "worker-b")

    def test_b_edit_acceptance_is_restricted_to_dedicated_fixture(self) -> None:
        root = Path(__file__).resolve().parents[2]
        for slot in ("worker-a", "worker-b"):
            fixture = root / f"scripts/tests/fixtures/orca_edit_acceptance/{slot}/result.fixture"
            self.assertTrue(fixture.read_text().startswith("READY:"))
        ticket = {**self.simple(), "task_kind": "acceptance-edit",
                  "allowed_directories": [providers.CURSOR_EDIT_ACCEPTANCE_SCOPE]}
        self.assertEqual(providers.provider_for(ticket, "worker-b"), "cursor")
        for change in (
            {"read_only": True},
            {"allowed_directories": ["scripts/tests/fixtures/orca_edit_acceptance/worker-a"]},
            {"allowed_directories": [providers.CURSOR_EDIT_ACCEPTANCE_SCOPE,
                                     "crates/hw_ui/src/interaction/help"]},
        ):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, "dedicated fixture"):
                providers.provider_for({**ticket, **change}, "worker-b")

    @patch("scripts.orca_providers.shutil.which", side_effect=lambda name: f"/bin/{name}")
    def test_commands_never_fall_back_to_another_provider(self, _) -> None:
        cursor = providers.command_for("cursor", Path("/repo"), "worker", "task")
        self.assertEqual(cursor[0], "/bin/cursor-agent")
        self.assertIn("enabled", cursor)
        self.assertNotIn("--force", cursor)
        self.assertNotIn("--yolo", cursor)
        codex = providers.command_for("codex", Path("/repo"), "reviewer", "review")
        self.assertIn("read-only", codex)
        self.assertNotIn("--model", codex)
        session = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        resumed = providers.command_for("cursor", Path("/repo"), "worker", "next", session)
        self.assertEqual(resumed[resumed.index("--resume") + 1], session)
        self.assertNotIn("--continue", resumed)
        with self.assertRaises(ValueError):
            providers.command_for("cursor", Path("/repo"), "reviewer", "wrong")

    @patch("scripts.orca_providers.shutil.which", return_value=None)
    def test_missing_provider_stops(self, _) -> None:
        with self.assertRaisesRegex(RuntimeError, "cursor CLI"):
            providers.command_for("cursor", Path("/repo"), "worker", "task")

    @patch("scripts.orca_providers.shutil.which", side_effect=lambda name: f"/bin/{name}")
    def test_read_only_workers_keep_provider_and_exact_resume(self, _) -> None:
        session = "e1fd2684-d55a-4794-9741-903c92b7dbea"
        cursor = providers.command_for("cursor", Path("/repo"), "worker", "next", session, read_only=True)
        self.assertEqual(cursor[cursor.index("--mode") + 1], "ask")
        self.assertEqual(cursor[cursor.index("--resume") + 1], session)
        codex = providers.command_for("codex", Path("/repo"), "worker", "next", session, read_only=True)
        self.assertEqual(codex[codex.index("--sandbox") + 1], "read-only")
        self.assertEqual(codex[1:3], ["resume", session])
        policy = providers.cursor_permissions({**self.simple(), "read_only": True})["permissions"]
        self.assertEqual(policy["allow"], ["Read(**)"])
        self.assertIn("Write(**)", policy["deny"])

    @patch("scripts.orca_providers.shutil.which", side_effect=lambda name: f"/bin/{name}")
    def test_codex_bridge_relies_on_the_outer_sandbox(self, _) -> None:
        codex = providers.command_for(
            "codex", Path("/repo"), "reviewer", "review", externally_sandboxed=True)
        self.assertIn("--dangerously-bypass-approvals-and-sandbox", codex)
        self.assertNotIn("--sandbox", codex)
        self.assertNotIn("--ask-for-approval", codex)
        with self.assertRaisesRegex(ValueError, "Cursor cannot bypass"):
            providers.command_for(
                "cursor", Path("/repo"), "worker", "task", externally_sandboxed=True)
        with self.assertRaisesRegex(ValueError, "read-only roles"):
            providers.command_for(
                "codex", Path("/repo"), "worker", "task", externally_sandboxed=True)

    def test_cursor_permissions_limit_writes_and_disable_shell_mcp(self) -> None:
        config = providers.cursor_permissions(self.simple())
        self.assertEqual(config["version"], 1)
        self.assertEqual(config["editor"], {"vimMode": False})
        policy = config["permissions"]
        self.assertEqual(policy["deny"], ["Shell(*)", "Mcp(*:*)", "WebFetch(*)"])
        self.assertEqual(policy["allow"], ["Read(**)", "Write(crates/hw_ui/src/interaction/help/**)"])

    def test_cursor_hook_policy_keeps_agent_tools_denied(self) -> None:
        ticket = {**self.simple(), "read_only": True, "allowed_directories": []}
        policy = providers.cursor_permissions(
            ticket, denied_reads=("/private/bridge", "/proc"))["permissions"]
        self.assertEqual(policy["allow"], ["Read(**)"])
        for denied in ("Shell(*)", "Mcp(*:*)", "WebFetch(*)", "Write(**)",
                       "Read(/private/bridge/**)", "Read(/proc/**)"):
            self.assertIn(denied, policy["deny"])
        hooks = providers.cursor_hook_config(Path("/repo"))
        self.assertEqual(set(hooks["hooks"]), {"beforeSubmitPrompt", "afterAgentResponse", "stop"})
        for rows in hooks["hooks"].values():
            self.assertEqual(len(rows), 1)
            self.assertTrue(rows[0]["failClosed"])
            self.assertIn("orca_cursor_bridge_hook.py", rows[0]["command"])


if __name__ == "__main__":
    unittest.main()
