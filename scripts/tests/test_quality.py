from __future__ import annotations

import unittest
from unittest.mock import Mock, patch

from scripts import dev, quality


class QualityGroupTests(unittest.TestCase):
    def runner(self):
        return quality.Runner(Mock(), Mock(), Mock(), Mock(), {"HELL_WORKERS_DIFF_BASE": "base"})

    def test_contracts_do_not_install_tools_or_invoke_cargo(self):
        runner = self.runner()
        quality.run_group("contracts", runner)
        runner.tools.assert_not_called()
        runner.hygiene.assert_called_once()
        runner.suppressions.assert_called_once()
        self.assertTrue(all(call.args[0][0] != "cargo" for call in runner.command.call_args_list))
        help_call = next(call for call in runner.command.call_args_list if "scripts/check_help_impact.py" in call.args[0])
        self.assertEqual(help_call.kwargs["extra_env"], {"HELL_WORKERS_DIFF_BASE": "base"})

    def test_first_failure_stops_expensive_later_groups(self):
        runner = self.runner()
        runner.command.side_effect = RuntimeError("contract failed")
        with self.assertRaises(RuntimeError):
            quality.run_groups(list(quality.GROUPS), runner)
        runner.tools.assert_not_called()
        self.assertEqual(runner.command.call_count, 1)

    def test_tooling_and_dependencies_have_separate_preflight(self):
        runner = self.runner()
        quality.run_group("tooling", runner)
        runner.tools.assert_called_once_with(lint=True)
        self.assertEqual(runner.command.call_count, 3)
        self.assertTrue(all(call.args[0][0] != "cargo" for call in runner.command.call_args_list))
        runner.tools.reset_mock()
        quality.run_group("deps", runner)
        runner.tools.assert_called_once_with(deps=True)

    def test_rust_keeps_all_features_tests_locked_and_zero_warning_gate(self):
        runner = self.runner()
        with patch.object(quality.platform, "system", return_value="Linux"):
            quality.run_group("rust", runner)
        commands = [call.args[0] for call in runner.command.call_args_list]
        self.assertEqual(len(commands), 8)
        self.assertTrue(all("--locked" in command for command in commands if command[1] != "fmt"))
        self.assertEqual(sum(command[1] == "test" for command in commands), 2)
        self.assertTrue(any(command[-3:] == ["--", "-D", "warnings"] for command in commands))
        for feature in ("profiling-memory", "profiling-tracy", "profiling-renderdoc"):
            self.assertTrue(any(feature in command for command in commands))

    def test_driver_uses_its_existing_guarded_cargo_runner(self):
        with patch.object(dev, "run_command") as run:
            quality.run_group("rust", dev.quality_runner())
        self.assertTrue(any(call.args[0][1] == "clippy" for call in run.call_args_list))

    def test_ci_cannot_run_an_unbound_group_or_local_check(self):
        with patch.dict(dev.os.environ, {"CI": "true"}):
            self.assertEqual(dev.main(["quality", "--group", "rust"]), 1)
            self.assertEqual(dev.main(["ci", "check", "--base", "a" * 40]), 1)


if __name__ == "__main__":
    unittest.main()
