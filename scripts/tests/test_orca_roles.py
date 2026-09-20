from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts import orca_roles as roles


class OrcaRoleTests(unittest.TestCase):
    def setUp(self) -> None:
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=target)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.primary = self.root / "primary"
        self.repo = self.root / "candidate"
        self.primary.mkdir()
        self.run_git(self.primary, "init", "-q")
        for directory in ("src", "docs"):
            (self.primary / directory).mkdir()
            (self.primary / directory / "content.txt").write_text("original")
        self.run_git(self.primary, "add", ".")
        self.run_git(self.primary, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                     "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")
        self.run_git(self.primary, "worktree", "add", "-qb", "task", str(self.repo))
        self.ticket = {"schema": 1, "id": "test-task", "repo": str(self.repo),
                       "branch": "task", "base": roles.git(self.repo, "rev-parse", "HEAD"),
                       "allowed_directories": ["src"], "prompt": "Read the fixture."}
        self.path = self.root / "ticket.json"

    @staticmethod
    def run_git(repo: Path, *args: str) -> None:
        subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True)

    def load(self) -> dict:
        self.path.write_text(json.dumps(self.ticket))
        return roles.load_ticket(self.path)

    def test_ticket_rejects_primary_wrong_branch_and_protected_paths(self) -> None:
        self.assertEqual(self.load()["id"], "test-task")
        for scope in (["."], ["../primary"], ["docs"], [".git"], ["src/content.txt"], ["src", "src"]):
            self.ticket["allowed_directories"] = scope
            with self.subTest(scope=scope), self.assertRaises(ValueError):
                self.load()
        self.ticket["allowed_directories"] = ["src"]
        self.ticket["repo"] = str(self.primary)
        with self.assertRaisesRegex(ValueError, "primary"):
            self.load()
        self.ticket["repo"] = str(self.repo)
        self.ticket["branch"] = "wrong"
        with self.assertRaisesRegex(ValueError, "branch"):
            self.load()

    def test_scope_rejects_symlinks_and_hardlinks(self) -> None:
        link = self.repo / "src/link"
        link.symlink_to(self.primary / "src/content.txt")
        with self.assertRaisesRegex(ValueError, "symlink/hardlink"):
            self.load()
        link.unlink()
        os.link(self.primary / "src/content.txt", link)
        with self.assertRaisesRegex(ValueError, "symlink/hardlink"):
            self.load()

    def test_fingerprint_invalidates_dirty_untracked_mode_and_index(self) -> None:
        before = roles.fingerprint(self.repo)
        (self.repo / "src/new.txt").write_text("new")
        after = roles.fingerprint(self.repo)
        self.assertNotEqual(before, after)
        (self.repo / "src/content.txt").chmod(0o755)
        self.assertNotEqual(after, roles.fingerprint(self.repo))
        before_index = roles.fingerprint(self.repo)
        self.run_git(self.repo, "add", "src")
        self.assertNotEqual(before_index, roles.fingerprint(self.repo))

    def test_stale_review_and_missing_evidence_fail_closed(self) -> None:
        ticket = self.load()
        record = {"ticket": ticket["id"], "base": ticket["base"], "head": ticket["base"],
                  "source_sha256": roles.fingerprint(self.repo), "verdict": "approved",
                  "reviewer_session": "fixture-reviewer", "validation_evidence": "fixture-pass",
                  "blocking_findings": []}
        roles.verify_review(ticket, record)
        with self.assertRaisesRegex(ValueError, "blocking"):
            roles.verify_review(ticket, {**record, "blocking_findings": ["bug"]})
        with self.assertRaisesRegex(ValueError, "evidence"):
            roles.verify_review(ticket, {**record, "validation_evidence": ""})
        (self.repo / "src/content.txt").write_text("changed after approval")
        with self.assertRaisesRegex(ValueError, "stale"):
            roles.verify_review(ticket, record)

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap"), "Linux bubblewrap required")
    def test_real_mount_boundary_blocks_reviewer_and_worker_escape(self) -> None:
        ticket = self.load()
        runtime = self.root / "runtime"
        (runtime / "codex").mkdir(parents=True)
        (runtime / "tmp").mkdir()
        paths = [self.repo / "src/content.txt", self.repo / "docs/content.txt",
                 self.primary / "src/content.txt", self.primary / ".git/config"]
        script = """import json, pathlib, sys
assert pathlib.Path('/etc/resolv.conf').read_text()
results = []
for name in sys.argv[1:]:
    try:
        pathlib.Path(name).write_text('changed')
        results.append(True)
    except OSError:
        results.append(False)
print(json.dumps(results))
"""
        for role, expected in (("reviewer", [False] * 4), ("worker", [True, False, False, False])):
            with self.subTest(role=role):
                command = roles.sandbox_command(ticket, role, runtime,
                                                [sys.executable, "-c", script, *map(str, paths)])
                result = subprocess.run(command, capture_output=True, text=True, check=True)
                self.assertEqual(json.loads(result.stdout), expected)
        self.assertEqual((self.primary / "src/content.txt").read_text(), "original")


if __name__ == "__main__":
    unittest.main()
