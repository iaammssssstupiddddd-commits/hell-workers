from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import orca_host_validation as host


class HostValidationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name).resolve()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        (self.repo / "scripts").mkdir()
        (self.repo / "scripts/dev.py").write_text("# frozen fixture\n")

    def test_current_runner_targets_exact_frozen_worktree(self):
        original = (host.dev.REPO_ROOT, host.dev.SCRIPTS_DIR)
        observed = {}

        def invoke(arguments):
            observed.update(root=host.dev.REPO_ROOT, scripts=host.dev.SCRIPTS_DIR,
                            arguments=list(arguments))
            return 7

        with patch.object(host.dev, "main", side_effect=invoke):
            self.assertEqual(host.run(self.repo, ["ci", "check", "--base", "a" * 40]), 7)
        self.assertEqual(observed, {
            "root": self.repo,
            "scripts": self.repo / "scripts",
            "arguments": ["ci", "check", "--base", "a" * 40],
        })
        self.assertEqual((host.dev.REPO_ROOT, host.dev.SCRIPTS_DIR), original)

    def test_uncontrolled_command_and_noncanonical_repo_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "guarded development"):
            host.run(self.repo, ["cargo", "test"])
        with self.assertRaisesRegex(ValueError, "absolute canonical"):
            host.checked_repo(Path("relative"))


if __name__ == "__main__":
    unittest.main()
