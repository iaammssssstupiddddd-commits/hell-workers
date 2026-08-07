from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import MagicMock, patch

from scripts import build_coordination, build_lane, dev
from scripts.perf_tool import cli as perf_cli


class BuildCoordinationTests(unittest.TestCase):
    def test_shared_leases_can_coexist_but_exclusive_cannot(self) -> None:
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parents[2] / "target"
        ) as directory:
            repo = Path(directory)
            first = build_coordination.acquire_activity(repo, "shared")
            second = build_coordination.acquire_activity(repo, "shared")
            try:
                self.assertEqual(first.mode, "shared")
                self.assertEqual(second.mode, "shared")
                with self.assertRaises(build_coordination.ActivityBusyError):
                    build_coordination.acquire_activity(repo, "exclusive")
            finally:
                second.close()
                first.close()

            with build_coordination.acquire_activity(repo, "exclusive"):
                with self.assertRaises(build_coordination.ActivityBusyError):
                    build_coordination.acquire_activity(repo, "shared")

    def test_invalid_mode_and_tmp_workspace_fail_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "invalid Cargo activity mode"):
            build_coordination.acquire_activity(Path.cwd(), "unknown")  # type: ignore[arg-type]
        with self.assertRaisesRegex(RuntimeError, "must not be placed under /tmp"):
            build_coordination.acquire_activity(Path("/tmp/hell-workers-activity-test"), "shared")

    def test_dev_compile_refuses_before_child_when_native_recipe_is_active(self) -> None:
        with build_coordination.acquire_activity(dev.REPO_ROOT, "exclusive"):
            with patch.object(dev, "require_cargo_memory"), patch.object(
                dev.subprocess, "run"
            ) as run:
                with self.assertRaises(build_coordination.ActivityBusyError):
                    dev.run_command(["cargo", "check", "--workspace"])
            run.assert_not_called()

    def test_idle_lane_does_not_block_native_or_performance_activity(self) -> None:
        with build_lane.acquire_lane(dev.REPO_ROOT):
            with build_coordination.acquire_activity(dev.REPO_ROOT, "exclusive") as lease:
                self.assertEqual(lease.mode, "exclusive")

    def test_activity_lease_releases_after_exception(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "sentinel"):
            with build_coordination.acquire_activity(dev.REPO_ROOT, "exclusive"):
                raise RuntimeError("sentinel")
        with build_coordination.acquire_activity(dev.REPO_ROOT, "shared"):
            pass

    def test_performance_recipe_uses_exclusive_lease_but_dry_run_does_not(self) -> None:
        lease = MagicMock()
        with patch.object(perf_cli, "acquire_activity", return_value=lease) as acquire:
            with patch.object(perf_cli, "_run_suite", return_value=3) as run:
                args = SimpleNamespace(dry_run=False)
                self.assertEqual(perf_cli.run_suite(args), 3)
                acquire.assert_called_once_with(perf_cli.REPO_ROOT, "exclusive")
                run.assert_called_once_with(args)

        lease = MagicMock()
        with patch.object(perf_cli, "acquire_activity", return_value=lease) as acquire:
            with patch.object(perf_cli, "_run_suite", return_value=0) as run:
                args = SimpleNamespace(dry_run=True)
                self.assertEqual(perf_cli.run_suite(args), 0)
                acquire.assert_not_called()
                run.assert_called_once_with(args)


if __name__ == "__main__":
    unittest.main()
