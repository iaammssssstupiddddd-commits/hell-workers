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

    def test_cursor_permissions_limit_writes_and_disable_shell_mcp(self) -> None:
        config = providers.cursor_permissions(self.simple())
        self.assertEqual(config["version"], 1)
        self.assertEqual(config["editor"], {"vimMode": False})
        policy = config["permissions"]
        self.assertEqual(policy["deny"], ["Shell(*)", "Mcp(*:*)", "WebFetch(*)"])
        self.assertEqual(policy["allow"], ["Read(**)", "Write(crates/hw_ui/src/interaction/help/**)"])


if __name__ == "__main__":
    unittest.main()
