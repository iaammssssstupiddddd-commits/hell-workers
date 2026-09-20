from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

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

    @patch("scripts.orca_roles.command_for", return_value=["/fake/codex"])
    def test_admission_rechecks_ticket_after_leases(self, _) -> None:
        ticket = self.load()
        with patch.object(roles, "acquire_host"), patch.object(roles, "validate_ticket",
                side_effect=ValueError("branch changed")) as validate:
            with self.assertRaisesRegex(ValueError, "branch changed"):
                roles.launch(ticket, "worker-a", dry_run=False)
        validate.assert_called_once_with(ticket)

    @patch("scripts.orca_roles.command_for", return_value=["/fake/codex"])
    def test_unknown_reviewer_session_refuses_before_provider_launch(self, _) -> None:
        ticket = self.load()
        runtime = self.root / "empty-runtime"
        runtime.mkdir()
        with patch.object(roles, "acquire_host"), patch.object(roles, "prepare_runtime", return_value=runtime):
            with self.assertRaisesRegex(ValueError, "does not exist"):
                roles.launch(ticket, "reviewer", dry_run=False,
                             resume_session="e1fd2684-d55a-4794-9741-903c92b7dbea")

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap"), "Linux bubblewrap required")
    def test_cursor_mount_hides_project_config_and_policy_is_read_only(self) -> None:
        ticket = self.load()
        runtime = self.root / "cursor-runtime"
        for child in ("cursor", "cursor-data", "xdg/cursor", "cache", "tmp", "codex"):
            (runtime / child).mkdir(parents=True, exist_ok=True)
        for child in (".cursor", ".claude"):
            directory = self.repo / child
            directory.mkdir()
            (directory / "mcp.json").write_text('{"fixture":true}')
        policy = self.root / "policy.json"
        roles.write_cursor_policy(ticket, policy)
        original_config = self.root / "original-config"
        (original_config / "cursor").mkdir(parents=True)
        original_auth = original_config / "cursor/auth.json"
        original_auth.write_text('{"fixture": "not-a-credential"}')
        script = """import json, os, pathlib, sys
repo, runtime = map(pathlib.Path, sys.argv[1:])
assert os.environ['CURSOR_CONFIG_DIR'] == str(runtime / 'cursor')
assert os.environ['CURSOR_DATA_DIR'] == str(runtime / 'cursor-data')
assert 'CURSOR_API_KEY' not in os.environ
assert not (repo / '.cursor/mcp.json').exists()
assert not (repo / '.claude/mcp.json').exists()
policy = runtime / 'cursor/cli-config.json'
assert 'Shell(*)' in json.loads(policy.read_text())['permissions']['deny']
auth = runtime / 'xdg/cursor/auth.json'
assert json.loads(auth.read_text()) == {'fixture': 'not-a-credential'}
for path in (policy, auth, repo / 'docs/content.txt'):
    try:
        path.write_text('escape')
    except OSError:
        continue
    raise AssertionError('write escaped: ' + str(path))
(repo / 'src/content.txt').write_text('allowed')
print('pass')
"""
        with patch.dict(os.environ, {"XDG_CONFIG_HOME": str(original_config), "CURSOR_API_KEY": "fixture"}):
            command = roles.sandbox_command(ticket, "worker", runtime,
                                           [sys.executable, "-c", script, str(self.repo), str(runtime)],
                                           provider="cursor", policy=policy)
            result = subprocess.run(command, capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.strip(), "pass")
        self.assertEqual((self.repo / "docs/content.txt").read_text(), "original")
        self.assertEqual(original_auth.read_text(), '{"fixture": "not-a-credential"}')

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
