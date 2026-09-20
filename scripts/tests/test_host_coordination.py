from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import build_coordination, dev, host_coordination as host


class HostCoordinationTests(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).resolve().parents[2] / "target"
        root.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=root)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        patcher = patch.object(host, "state_root", return_value=self.root / "coordination")
        patcher.start()
        self.addCleanup(patcher.stop)
        environment = patch.dict(os.environ)
        environment.start()
        os.environ.pop(host.HOST_FD_ENV, None)
        self.addCleanup(environment.stop)

    def test_cross_checkout_and_clone_contention(self) -> None:
        first = build_coordination.acquire_activity(self.root / "one", "shared")
        try:
            with self.assertRaises(build_coordination.ActivityBusyError):
                build_coordination.acquire_activity(self.root / "two", "shared")
        finally:
            first.close()
        with build_coordination.acquire_activity(self.root / "two", "exclusive"):
            pass

    def test_only_exact_locked_description_can_be_borrowed(self) -> None:
        with host.acquire_host() as owner:
            with patch.dict(os.environ, owner.environment({})):
                with host.acquire_host() as borrowed:
                    self.assertTrue(borrowed.borrowed)
            fake = host.open_lock(owner.path)
            try:
                with patch.dict(os.environ, {host.HOST_FD_ENV: str(fake)}):
                    with self.assertRaisesRegex(RuntimeError, "does not own"):
                        host.acquire_host()
            finally:
                os.close(fake)
        unlocked = host.open_lock(host.lock_path())
        try:
            with patch.dict(os.environ, {host.HOST_FD_ENV: str(unlocked)}):
                with self.assertRaisesRegex(RuntimeError, "not locked"):
                    host.acquire_host()
        finally:
            os.close(unlocked)

    def test_child_keeps_slot_after_parent_descriptor_closes(self) -> None:
        owner = host.acquire_host()
        child = subprocess.Popen([sys.executable, "-c", "import sys; print('ready', flush=True); sys.stdin.read()"],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                                 pass_fds=(owner.fd,))
        try:
            self.assertEqual(child.stdout.readline().strip(), "ready")
            owner.close()
            with self.assertRaises(host.HostBusyError):
                host.acquire_host()
        finally:
            child.communicate(timeout=5)
            owner.close()
        with host.acquire_host():
            pass

    def test_bad_fd_and_symlink_fail_closed(self) -> None:
        with patch.dict(os.environ, {host.HOST_FD_ENV: "garbage"}):
            with self.assertRaisesRegex(RuntimeError, "invalid inherited"):
                host.acquire_host()
        path = host.lock_path()
        path.symlink_to(self.root / "other")
        with self.assertRaises(OSError):
            host.acquire_host()

    def test_role_slots_are_bounded_and_independent_of_heavy_work(self) -> None:
        with host.acquire_host("reviewer"), host.acquire_host("worker-a"), host.acquire_host("worker-b"):
            with self.assertRaises(host.HostBusyError):
                host.acquire_host("reviewer")
            with host.acquire_host():
                pass

    def test_cli_fanout_overrides_are_rejected_before_spawn(self) -> None:
        for arguments in (["check", "-j8"], ["check", "--jobs", "8"],
                          ["+nightly", "check"], ["--release", "check"],
                          ["test", "--", "--test-threads=8"],
                          ["check", "--config", "build.jobs=8"]):
            with self.subTest(arguments=arguments), patch.object(dev.subprocess, "run") as run:
                with self.assertRaises(RuntimeError):
                    dev.run_command(["cargo", *arguments])
                run.assert_not_called()

    def test_aliases_and_external_subcommands_take_host_admission(self) -> None:
        for name in ("b", "c", "t", "r", "visual-test", "external-plugin"):
            with self.subTest(name=name), patch.object(dev, "require_mutable"), \
                    patch.object(dev, "acquire_activity", side_effect=host.HostBusyError("busy")) as acquire, \
                    patch.object(dev.subprocess, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "busy"):
                    dev.run_command(["cargo", name])
                acquire.assert_called_once()
                run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
