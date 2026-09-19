from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from contextlib import redirect_stdout
from io import StringIO

from scripts import check_help_impact, ci_scope, dev


class ClassificationTests(unittest.TestCase):
    def test_path_priority_and_dependency_closure(self):
        cases = {
            "docs/building.md": {"contracts"},
            "crates/hw_world/README.md": {"contracts"},
            "scripts/convert_to_png.py": {"contracts", "tooling"},
            "tools/blender_ai_workflow/bin/export-staging-glb": {"contracts", "tooling"},
            "crates/hw_world/src/lib.rs": {"contracts", "tooling", "rust"},
            "crates/hw_world/proptest-regressions/pathfinding/tests/properties.txt": {"contracts", "tooling", "rust"},
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                actual, _ = ci_scope.classify([path])
                self.assertEqual({g for g, value in actual.items() if value}, expected)
        for path in (
            "Cargo.toml", "Cargo.lock", "crates/hw_world/Cargo.toml", "crates/hw_world/build.rs",
            "rust-toolchain.toml", "deny.toml", ".github/workflows/ci.yml", ".cargo/config.toml",
            "scripts/ci_scope.py", "scripts/tests/test_ci_scope.py", "scripts/tests/test_dev.py",
            "scripts/check_docs.py", "scripts/tests/test_check_docs.py", "scripts/dev-tools.toml",
            "AGENTS.md", "crates/hw_world/_rules.md", ".cursor/skills/example/SKILL.md",
            "docs/plans/plan-template.md", "docs/DEVELOPMENT.md", "assets/README.md",
            "settings/default.ron", "crates/hw_world/assets/fixture.json", "assets/test.wgsl",
            "docs/diagram.png", "new-root/new.extension",
        ):
            with self.subTest(path=path):
                self.assertTrue(all(ci_scope.classify([path])[0].values()))

    def test_mixed_empty_full_and_help_paths(self):
        groups, reasons = ci_scope.classify(["docs/building.md", "crates/hw_world/src/lib.rs"])
        self.assertTrue(groups["rust"] and groups["tooling"])
        self.assertFalse(groups["deps"])
        self.assertIn("help-review-required", reasons)
        self.assertEqual(ci_scope.classify([])[0], {g: g == "contracts" for g in ci_scope.GROUP_IDS})
        self.assertTrue(all(ci_scope.classify([], full=True)[0].values()))

    def test_no_rename_parser_accepts_special_paths_and_rejects_corruption(self):
        self.assertEqual(ci_scope.parse_changes(b"D\0crates/a.rs\0A\0docs/a\nb.md\0"),
                         ["crates/a.rs", "docs/a\nb.md"])
        for output in (b"M\0", b"M\0x", b"U\0a\0", b"R100\0a\0b\0", b"M\0../outside\0"):
            with self.subTest(output=output), self.assertRaises(ValueError):
                ci_scope.parse_changes(output)


class GitScopeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="ci-scope-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-b", "master")
        self.git("config", "user.name", "CI fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("config", "core.hooksPath", "/dev/null")
        self.write("README.md", "initial\n")
        self.base = self.commit()

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True).stdout.decode().strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "-m", "fixture")
        return self.git("rev-parse", "HEAD")

    def env(self, event="push"):
        return {"GITHUB_EVENT_NAME": event, "GITHUB_SHA": self.git("rev-parse", "HEAD"),
                "GITHUB_RUN_ID": "42", "GITHUB_RUN_ATTEMPT": "1"}

    def test_push_multiple_commits_and_deleted_rust(self):
        self.write("crates/hw_world/src/lib.rs", "fn example() {}\n")
        self.commit()
        self.write("docs/topic.md", "topic\n")
        head = self.commit()
        plan = ci_scope.github_plan(self.root, {"before": self.base, "after": head}, self.env())
        self.assertEqual(plan["path_count"], 2)
        self.assertTrue(plan["groups"]["rust"])
        self.git("mv", "crates/hw_world/src/lib.rs", "docs/renamed.md")
        after = self.commit()
        plan = ci_scope.github_plan(self.root, {"before": head, "after": after}, self.env())
        self.assertTrue(plan["groups"]["rust"])
        self.assertEqual(plan["path_count"], 2)

    def test_pr_base_only_changes_do_not_force_rust_but_all_jobs_bind_merge_sha(self):
        self.git("switch", "-c", "topic")
        self.write("docs/new.md", "topic\n")
        head = self.commit()
        self.git("switch", "master")
        self.write("crates/hw_world/src/lib.rs", "fn example() {}\n")
        base = self.commit()
        self.git("merge", "--no-ff", "topic", "-m", "synthetic merge")
        event = {"pull_request": {"base": {"sha": base}, "head": {"sha": head}}}
        plan = ci_scope.github_plan(self.root, event, self.env("pull_request"))
        self.assertFalse(plan["groups"]["rust"])
        self.assertNotEqual(plan["tested_sha"], head)
        self.assertEqual(plan["help_base_sha"], base)
        event["pull_request"]["base"]["sha"] = self.base
        with self.assertRaisesRegex(ValueError, "parents"):
            ci_scope.github_plan(self.root, event, self.env("pull_request"))

    def test_non_fast_forward_forces_full_and_uses_common_help_base(self):
        self.git("switch", "-c", "old")
        self.write("docs/old.md", "old\n")
        before = self.commit()
        self.git("switch", "master")
        self.write("docs/new.md", "new\n")
        after = self.commit()
        plan = ci_scope.github_plan(self.root, {"before": before, "after": after}, self.env())
        self.assertTrue(all(plan["groups"].values()))
        self.assertEqual(plan["help_base_sha"], self.base)

    def test_manual_requires_valid_strict_ancestor_even_for_full(self):
        self.write("docs/topic.md", "text\n")
        head = self.commit()
        for base in (None, "0" * 40, head, "master", "f" * 40):
            with self.subTest(base=base), self.assertRaises((ValueError, subprocess.CalledProcessError)):
                ci_scope.github_plan(self.root, {"inputs": {"base_sha": base, "mode": "full"}}, self.env("workflow_dispatch"))
        plan = ci_scope.github_plan(self.root, {"inputs": {"base_sha": self.base, "mode": "full"}}, self.env("workflow_dispatch"))
        self.assertTrue(all(plan["groups"].values()))

    def test_dirty_paths_include_staged_unstaged_untracked_and_detect_content_change(self):
        self.write(".gitignore", "generated/\n")
        self.commit()
        self.write("docs/staged.md", "staged\n")
        self.git("add", "docs/staged.md")
        self.write("README.md", "unstaged\n")
        self.write("crates/hw_world/src/lib.rs", "untracked\n")
        self.assertEqual(set(ci_scope.worktree_paths(self.root)), {"docs/staged.md", "README.md", "crates/hw_world/src/lib.rs"})
        first = ci_scope.source_fingerprint(self.root)
        self.write("generated/cache", "ignored\n")
        self.assertEqual(first, ci_scope.source_fingerprint(self.root))
        self.write("README.md", "changed again\n")
        self.assertNotEqual(first, ci_scope.source_fingerprint(self.root))

    def test_staged_change_restored_only_in_worktree_still_selects_rust(self):
        name = "crates/hw_world/src/lib.rs"
        self.write(name, "fn example() {}\n")
        self.commit()
        self.write(name, "fn changed() {} \n")
        self.git("add", name)
        self.write(name, "fn example() {}\n")
        self.assertEqual(self.git("diff", "HEAD", "--name-only"), "")
        self.assertEqual(ci_scope.worktree_paths(self.root), [name])
        with self.assertRaises(subprocess.CalledProcessError):
            ci_scope.diff_hygiene(self.root, [], local=True)

    def test_reverted_production_tree_still_requires_help_commit_review(self):
        name = "crates/hw_world/src/lib.rs"
        self.write(name, "fn temporary() {}\n")
        self.commit()
        self.git("rm", name)
        head = self.commit()
        plan = ci_scope.github_plan(self.root, {"before": self.base, "after": head}, self.env())
        self.assertFalse(plan["groups"]["rust"])
        self.assertTrue(plan["groups"]["contracts"])
        commits = check_help_impact.collect_commits(plan["help_base_sha"], root=self.root)
        decision = check_help_impact.evaluate_batch(commits, [], is_ancestor=lambda a, b:
            check_help_impact.git_is_ancestor(a, b, root=self.root))
        self.assertFalse(decision.passed)

    def test_large_diff_and_special_characters_are_not_truncated(self):
        for i in range(3050):
            self.write(f"docs/{i}.md", "text\n")
        self.write("docs/space and\nnewline.md", "text\n")
        head = self.commit()
        plan = ci_scope.github_plan(self.root, {"before": self.base, "after": head}, self.env())
        self.assertEqual(plan["path_count"], 3051)
        self.assertFalse(plan["groups"]["rust"])

    def test_local_ci_check_refuses_changed_source_after_success(self):
        self.write("docs/topic.md", "first\n")
        with patch.object(dev, "REPO_ROOT", self.root), patch.dict(os.environ, {"CI": ""}), patch.object(
            dev.quality, "run_groups", side_effect=lambda *_: self.write("docs/topic.md", "second\n")
        ):
            self.assertEqual(dev.main(["ci", "check", "--base", self.base]), 1)

    def test_local_source_change_during_classification_cannot_pass(self):
        self.write("docs/topic.md", "first\n")
        original = ci_scope.classify

        def changed_after_listing(paths, **kwargs):
            self.write("crates/hw_world/src/lib.rs", "fn missed() {}\n")
            return original(paths, **kwargs)

        with patch.object(dev, "REPO_ROOT", self.root), patch.dict(os.environ, {"CI": ""}), \
                patch.object(ci_scope, "classify", side_effect=changed_after_listing), \
                patch.object(dev.quality, "run_groups"):
            self.assertEqual(dev.main(["ci", "check", "--base", self.base]), 1)

    def test_untracked_whitespace_fails_and_read_only_plan_is_bound_to_event(self):
        self.write("docs/new.md", "bad trailing space \n")
        with self.assertRaisesRegex(ValueError, "hygiene"):
            ci_scope.diff_hygiene(self.root, [], local=True)
        self.write("docs/new.md", "good\n")
        head = self.commit()
        event = {"before": self.base, "after": head}
        environment = self.env()
        plan = ci_scope.github_plan(self.root, event, environment)
        event_path = self.root / "event.json"
        event_path.write_text(json.dumps(event))
        environment["GITHUB_EVENT_PATH"] = str(event_path)
        ci_scope.verify_github_plan(self.root, plan, environment)
        plan["groups"]["deps"] = True
        with self.assertRaisesRegex(ValueError, "differs"):
            ci_scope.verify_github_plan(self.root, plan, environment)

    def test_cli_plan_group_and_aggregate_share_actual_github_outputs(self):
        self.write("docs/topic.md", "topic\n")
        head = self.commit()
        # Keep event/output files outside source so checkout stays immutable.
        with tempfile.TemporaryDirectory() as directory:
            event_path = Path(directory) / "event.json"
            event_path.write_text(json.dumps({"before": self.base, "after": head}))
            output = Path(directory) / "output"
            summary = Path(directory) / "summary"
            env = {**self.env(), "GITHUB_EVENT_PATH": str(event_path), "GITHUB_OUTPUT": str(output),
                   "GITHUB_STEP_SUMMARY": str(summary), "CI": "true"}
            with patch.object(dev, "REPO_ROOT", self.root), patch.dict(os.environ, env), redirect_stdout(StringIO()):
                self.assertEqual(dev.main(["ci", "plan", "--github-event", str(event_path),
                                           "--github-output", str(output)]), 0)
                outputs = dict(line.split("=", 1) for line in output.read_text().splitlines())
                plan_json = outputs["plan_json"]
                plan = json.loads(plan_json)
                self.assertEqual(outputs["plan_sha256"], ci_scope.plan_digest(plan))
                output.unlink()
                with patch.object(dev.quality, "run_group") as run:
                    self.assertEqual(dev.main(["quality", "--group", "contracts", "--plan-json", plan_json]), 0)
                    self.assertEqual(run.call_args.args[0], "contracts")
                    self.assertEqual(run.call_args.args[1].environment["HELL_WORKERS_DIFF_BASE"], self.base)
                checked = dict(line.split("=", 1) for line in output.read_text().splitlines())
                needs = {"changes": {"result": "success", "outputs": outputs},
                         "contracts": {"result": "success", "outputs": checked},
                         **{name: {"result": "skipped"} for name in ("tooling", "dependencies", "rust")}}
                self.assertEqual(dev.main(["ci", "result", "--plan-json", plan_json,
                                           "--needs-json", json.dumps(needs)]), 0)
                self.assertIn("passed for the selected scope", summary.read_text())
                needs["contracts"]["result"] = "cancelled"
                self.assertEqual(dev.main(["ci", "result", "--plan-json", plan_json,
                                           "--needs-json", json.dumps(needs)]), 1)

    def test_push_rejects_missing_zero_unknown_and_wrong_checkout(self):
        self.write("docs/topic.md", "topic\n")
        head = self.commit()
        for before in (None, "0" * 40, "f" * 40):
            with self.subTest(before=before), self.assertRaises((ValueError, subprocess.CalledProcessError)):
                ci_scope.github_plan(self.root, {"before": before, "after": head}, self.env())
        with self.assertRaisesRegex(ValueError, "checkout"):
            ci_scope.github_plan(self.root, {"before": self.base, "after": self.base}, self.env())

    def test_audit_setup_accepts_inputless_events_but_quality_requires_base(self):
        with tempfile.TemporaryDirectory() as directory:
            event_path = Path(directory) / "event.json"
            for event in ({}, {"inputs": None}, {"inputs": {}}):
                event_path.write_text(json.dumps(event))
                for name in ("workflow_dispatch", "schedule"):
                    with self.subTest(event=event, name=name):
                        environment = {**self.env(name), "GITHUB_EVENT_PATH": str(event_path)}
                        ci_scope.fetch_event_history(self.root, environment)
                        self.assertEqual(self.git("rev-parse", "HEAD"), self.base)
                with self.assertRaises(ValueError):
                    ci_scope.github_plan(self.root, event, self.env("workflow_dispatch"))

    def test_setup_fetches_missing_event_commit_without_moving_checkout(self):
        self.write("docs/topic.md", "topic\n")
        head = self.commit()
        with tempfile.TemporaryDirectory() as directory:
            clone = Path(directory) / "checkout"
            subprocess.run(["git", "clone", "--depth", "1", self.root.as_uri(), str(clone)],
                           check=True, capture_output=True)
            event_path = Path(directory) / "event.json"
            event_path.write_text(json.dumps({"before": self.base, "after": head}))
            environment = {**self.env(), "GITHUB_EVENT_PATH": str(event_path)}
            with self.assertRaises(subprocess.CalledProcessError):
                ci_scope.commit(clone, self.base)
            ci_scope.fetch_event_history(clone, environment)
            self.assertEqual(ci_scope.commit(clone, self.base), self.base)
            self.assertEqual(ci_scope.git(clone, "rev-parse", "HEAD").decode().strip(), head)
            event_path.write_text(json.dumps({"before": "f" * 40, "after": head}))
            with self.assertRaises(subprocess.CalledProcessError):
                ci_scope.fetch_event_history(clone, environment)

    def test_unmerged_index_is_rejected_and_changed_source_does_not_hide_staged_tree(self):
        self.git("switch", "-c", "conflict")
        self.write("README.md", "other\n")
        self.commit()
        self.git("switch", "master")
        self.write("README.md", "ours\n")
        self.commit()
        with self.assertRaises(subprocess.CalledProcessError):
            self.git("merge", "conflict")
        with self.assertRaisesRegex(ValueError, "unmerged"):
            ci_scope.worktree_paths(self.root)


if __name__ == "__main__":
    unittest.main()
