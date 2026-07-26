from __future__ import annotations

import subprocess
import tempfile
import unittest
from unittest import mock
from pathlib import Path

from scripts import check_help_impact

HELP_SOURCE = "crates/bevy_app/src/interface/ui/help_content/coverage.rs"
HELP_RENDER_SOURCE = "crates/hw_ui/src/help.rs"
HELP_SNAPSHOT = check_help_impact.HELP_APPROVAL_SNAPSHOT


class HelpImpactPathTests(unittest.TestCase):
    def test_docs_tests_and_non_text_assets_are_not_production(self) -> None:
        for path in (
            "docs/help-screen.md",
            "scripts/tests/test_check_help_impact.py",
            "crates/hw_ui/src/tests/help.rs",
            "crates/hw_ui/src/tests.rs",
            "crates/hw_ui/src/help_tests.rs",
            "crates/hw_ui/src/test_support.rs",
            "crates/hw_ui/src/test_support/fixture.rs",
            "crates/hw_ui/tests/help.rs",
            "assets/textures/help.png",
            "assets/audio/click.ogg",
            "assets/shaders/world.wgsl",
        ):
            self.assertFalse(check_help_impact.is_production_path(path), path)

    def test_rust_cargo_and_runtime_text_data_are_production(self) -> None:
        for path in (
            "crates/bevy_app/src/lib.rs",
            "crates/bevy_app/src/input_actions/model.rs",
            "crates/hw_ui/build.rs",
            "Cargo.toml",
            "crates/hw_ui/Cargo.toml",
            "Cargo.lock",
            "assets/catalog/help.ron",
            "crates/hw_ui/assets/labels.ftl",
            "settings/defaults.toml",
        ):
            self.assertTrue(check_help_impact.is_production_path(path), path)

    def test_root_catalog_and_typed_renderer_count_as_help_sources(self) -> None:
        self.assertTrue(
            check_help_impact.is_help_update_path(
                "crates/bevy_app/src/interface/ui/help_content/providers/familiars.rs"
            )
        )
        self.assertTrue(check_help_impact.is_help_update_path(HELP_RENDER_SOURCE))
        self.assertFalse(
            check_help_impact.is_help_update_path(
                "crates/hw_ui/src/setup/help_panel.rs"
            )
        )
        for path in (
            "crates/bevy_app/src/interface/ui/help_content/tests.rs",
            "crates/bevy_app/src/interface/ui/help_content/README.md",
            "crates/bevy_app/src/interface/ui/help_content/fixture.ron",
            "crates/bevy_app/src/interface/ui/help_content/coverage_approval.snap",
        ):
            self.assertFalse(check_help_impact.is_help_update_path(path), path)
        self.assertTrue(
            check_help_impact.has_help_update((HELP_SOURCE, HELP_SNAPSHOT))
        )
        self.assertTrue(
            check_help_impact.has_help_update(
                (HELP_RENDER_SOURCE, HELP_SNAPSHOT)
            )
        )
        self.assertFalse(check_help_impact.has_help_update((HELP_SOURCE,)))
        self.assertFalse(
            check_help_impact.has_help_update((HELP_RENDER_SOURCE,))
        )
        self.assertFalse(check_help_impact.has_help_update((HELP_SNAPSHOT,)))


class HelpImpactTrailerTests(unittest.TestCase):
    def test_exact_final_no_impact_trailers_are_accepted(self) -> None:
        self.assertEqual(
            check_help_impact.parse_no_impact_reason(
                "Subject\n\nBody\n\n"
                "Help-Impact: none\n"
                "Help-Impact-Reason: Internal cache only\n"
            ),
            "Internal cache only",
        )

    def test_body_text_empty_duplicate_and_wrong_impact_are_rejected(self) -> None:
        messages = (
            "Help-Impact: none\nHelp-Impact-Reason: body only\n\nBody continues",
            "Subject\n\nHelp-Impact: none\nHelp-Impact-Reason:",
            (
                "Subject\n\nHelp-Impact: none\nHelp-Impact: none\n"
                "Help-Impact-Reason: Duplicate"
            ),
            "Subject\n\nHelp-Impact: changed\nHelp-Impact-Reason: Wrong value",
        )
        for message in messages:
            with self.subTest(message=message):
                self.assertIsNone(
                    check_help_impact.parse_no_impact_reason(message)
                )


class HelpImpactDecisionTests(unittest.TestCase):
    def commit(
        self,
        revision: str,
        *paths: str,
        reason: str | None = None,
    ) -> check_help_impact.CommitChange:
        return check_help_impact.CommitChange(revision, tuple(paths), reason)

    def test_multiple_production_commits_then_help_update_passes(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [
                self.commit("a", "crates/hw_core/src/lib.rs"),
                self.commit("b", "crates/hw_ui/src/components.rs"),
                self.commit(
                    "c",
                    "crates/bevy_app/src/interface/ui/help_content/manifest.rs",
                    HELP_SNAPSHOT,
                ),
            ],
            [],
        )
        self.assertTrue(decision.passed)

    def test_batch_no_impact_trailer_passes(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [
                self.commit("a", "crates/hw_core/src/lib.rs"),
                self.commit("b", "README.md", reason="Internal cache only"),
            ],
            [],
        )
        self.assertTrue(decision.passed)

    def test_missing_decision_fails(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [self.commit("a", "crates/hw_core/src/lib.rs")],
            [],
        )
        self.assertFalse(decision.passed)
        self.assertIn("crates/hw_core/src/lib.rs", decision.message)

    def test_production_after_help_or_decision_invalidates_it(self) -> None:
        for earlier in (
            self.commit(
                "a",
                HELP_SOURCE,
                HELP_SNAPSHOT,
            ),
            self.commit("a", "README.md", reason="Earlier batch"),
        ):
            with self.subTest(earlier=earlier):
                decision = check_help_impact.evaluate_batch(
                    [earlier, self.commit("b", "crates/hw_core/src/jobs.rs")],
                    [],
                )
                self.assertFalse(decision.passed)

    def test_decision_must_descend_from_every_production_commit(self) -> None:
        commits = [
            self.commit("left-production", "crates/hw_core/src/jobs.rs"),
            self.commit(
                "left-help",
                HELP_SOURCE,
                HELP_SNAPSHOT,
            ),
            self.commit("right-production", "crates/hw_ui/src/components.rs"),
        ]
        ancestors = {
            ("left-production", "left-production"),
            ("left-production", "left-help"),
            ("left-help", "left-help"),
            ("right-production", "right-production"),
        }
        decision = check_help_impact.evaluate_batch(
            commits,
            [],
            is_ancestor=lambda left, right: (left, right) in ancestors,
        )
        self.assertFalse(decision.passed)

    def test_worktree_production_invalidates_a_committed_decision(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [
                self.commit("a", "crates/hw_core/src/jobs.rs"),
                self.commit(
                    "b",
                    HELP_SOURCE,
                    HELP_SNAPSHOT,
                ),
            ],
            ["crates/hw_ui/src/components.rs"],
        )
        self.assertFalse(decision.passed)

    def test_dirty_local_override_passes_but_ci_rejects_it(self) -> None:
        local = check_help_impact.evaluate_batch(
            [],
            ["crates/hw_core/src/lib.rs"],
            local_reason="Internal type move",
        )
        ci = check_help_impact.evaluate_batch(
            [],
            ["crates/hw_core/src/lib.rs"],
            local_reason="Internal type move",
            ci=True,
        )
        self.assertTrue(local.passed)
        self.assertFalse(ci.passed)

    def test_same_worktree_help_source_and_snapshot_cover_dirty_production(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [],
            [
                "crates/hw_core/src/lib.rs",
                "crates/bevy_app/src/interface/ui/help_content/mod.rs",
                HELP_SNAPSHOT,
            ],
        )
        self.assertTrue(decision.passed)

    def test_help_source_snapshot_and_test_only_changes_are_not_interchangeable(
        self,
    ) -> None:
        incomplete_decisions = (
            [HELP_SOURCE],
            ["crates/hw_core/src/lib.rs", HELP_SNAPSHOT],
            [
                "crates/hw_core/src/lib.rs",
                "crates/bevy_app/src/interface/ui/help_content/tests.rs",
                HELP_SNAPSHOT,
            ],
        )
        for paths in incomplete_decisions:
            with self.subTest(paths=paths):
                self.assertFalse(
                    check_help_impact.evaluate_batch([], paths).passed
                )

    def test_untracked_source_is_a_worktree_production_change(self) -> None:
        decision = check_help_impact.evaluate_batch(
            [],
            ["crates/new_feature/src/lib.rs"],
        )
        self.assertFalse(decision.passed)


class HelpImpactBaseTests(unittest.TestCase):
    def test_ci_requires_a_resolvable_nonzero_explicit_base(self) -> None:
        with self.assertRaises(check_help_impact.HelpImpactError):
            check_help_impact.resolve_diff_base(
                {"CI": "true", "HELL_WORKERS_DIFF_BASE": "0000"}
            )
        with mock.patch.object(
            check_help_impact, "revision_exists", return_value=False
        ):
            with self.assertRaises(check_help_impact.HelpImpactError):
                check_help_impact.resolve_diff_base(
                    {"CI": "true", "HELL_WORKERS_DIFF_BASE": "missing"}
                )

    def test_local_invalid_explicit_base_fails_instead_of_falling_back(self) -> None:
        with mock.patch.object(
            check_help_impact, "revision_exists", return_value=False
        ):
            with self.assertRaises(check_help_impact.HelpImpactError):
                check_help_impact.resolve_diff_base(
                    {"HELL_WORKERS_DIFF_BASE": "missing"}
                )

    def test_explicit_base_resolves_to_its_merge_base_with_head(self) -> None:
        with (
            mock.patch.object(
                check_help_impact, "revision_exists", return_value=True
            ),
            mock.patch.object(
                check_help_impact, "merge_base", return_value="merge-base"
            ) as merge_base,
        ):
            self.assertEqual(
                check_help_impact.resolve_diff_base(
                    {"HELL_WORKERS_DIFF_BASE": "abc123"}
                ),
                "merge-base",
            )
        merge_base.assert_called_once()

    def test_local_missing_origin_base_fails_closed_for_existing_history(self) -> None:
        with (
            mock.patch.object(
                check_help_impact, "merge_base", return_value=None
            ),
            mock.patch.object(
                check_help_impact, "revision_exists", return_value=True
            ),
            mock.patch.object(
                check_help_impact,
                "commit_parents",
                return_value=("parent",),
            ),
        ):
            with self.assertRaisesRegex(
                check_help_impact.HelpImpactError,
                "local diff base cannot be resolved",
            ):
                check_help_impact.resolve_diff_base({})

    def test_local_root_commit_uses_empty_tree_when_origin_is_unavailable(
        self,
    ) -> None:
        with (
            mock.patch.object(
                check_help_impact, "merge_base", return_value=None
            ),
            mock.patch.object(
                check_help_impact, "revision_exists", return_value=True
            ),
            mock.patch.object(
                check_help_impact, "commit_parents", return_value=()
            ),
        ):
            self.assertEqual(
                check_help_impact.resolve_diff_base({}),
                check_help_impact.EMPTY_TREE,
            )

    def test_initial_repository_collects_root_commit_without_tree_range(self) -> None:
        calls: list[list[str]] = []

        def fake_git(arguments: list[str], **_: object) -> str:
            calls.append(arguments)
            return "root-commit" if arguments[0] == "rev-list" else "Subject"

        with (
            mock.patch.object(check_help_impact, "git", side_effect=fake_git),
            mock.patch.object(
                check_help_impact,
                "changed_paths_for_commit",
                return_value=("crates/hw_core/src/lib.rs",),
            ),
            mock.patch.object(
                check_help_impact,
                "parse_no_impact_reason",
                return_value=None,
            ),
        ):
            commits = check_help_impact.collect_commits(
                check_help_impact.EMPTY_TREE
            )

        self.assertEqual(calls[0][-1], "HEAD")
        self.assertEqual(commits[0].revision, "root-commit")


class HelpImpactGitDagTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory(prefix="help-impact-git-test-")
        self.root = Path(self.temp_dir.name)
        self.git("init", "-q", "-b", "master")
        self.git("config", "user.email", "help-impact@example.com")
        self.git("config", "user.name", "Help Impact Test")

    def tearDown(self) -> None:
        self.temp_dir.cleanup()

    def git(
        self,
        *arguments: str,
        check: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", *arguments],
            cwd=self.root,
            check=check,
            capture_output=True,
            text=True,
        )

    def write(self, path: str, content: str) -> None:
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")

    def commit(
        self,
        path: str,
        content: str,
        subject: str,
        *,
        no_impact_reason: str | None = None,
        extra_files: tuple[tuple[str, str], ...] = (),
    ) -> str:
        self.write(path, content)
        paths = [path]
        for extra_path, extra_content in extra_files:
            self.write(extra_path, extra_content)
            paths.append(extra_path)
        self.git("add", "--", *paths)
        arguments = ["commit", "-q", "-m", subject]
        if no_impact_reason is not None:
            arguments.extend(
                [
                    "-m",
                    "Help-Impact: none\n"
                    f"Help-Impact-Reason: {no_impact_reason}",
                ]
            )
        self.git(*arguments)
        return self.git("rev-parse", "HEAD").stdout.strip()

    def evaluate_from(self, base: str) -> check_help_impact.ImpactDecision:
        commits = check_help_impact.collect_commits(base, root=self.root)
        return check_help_impact.evaluate_batch(
            commits,
            [],
            is_ancestor=lambda ancestor, descendant: (
                check_help_impact.git_is_ancestor(
                    ancestor,
                    descendant,
                    root=self.root,
                )
            ),
        )

    def test_synthetic_merge_does_not_recount_parent_changes(self) -> None:
        base = self.commit(
            "crates/hw_core/src/lib.rs",
            "one\ntwo\nthree\n",
            "base",
        )
        self.git("switch", "-q", "-c", "feature")
        self.commit(
            "crates/hw_core/src/lib.rs",
            "ONE\ntwo\nthree\n",
            "feature production",
        )
        self.commit(
            "README.md",
            "feature decision\n",
            "record Help decision",
            no_impact_reason="Internal implementation only",
        )
        feature_tip = self.git("rev-parse", "HEAD").stdout.strip()

        self.git("switch", "-q", "master")
        target_tip = self.commit(
            "crates/hw_core/src/lib.rs",
            "one\ntwo\nTHREE\n",
            "target production",
        )
        self.git("merge", "-q", "--no-ff", "--no-edit", feature_tip)

        merge_revision = self.git("rev-parse", "HEAD").stdout.strip()
        self.assertEqual(
            check_help_impact.changed_paths_for_commit(
                merge_revision,
                root=self.root,
            ),
            (),
        )
        self.assertTrue(self.evaluate_from(target_tip).passed)
        self.assertNotEqual(base, target_tip)

    def test_parallel_old_help_and_new_production_do_not_merge_into_a_decision(
        self,
    ) -> None:
        base = self.commit("README.md", "base\n", "base")
        self.git("switch", "-q", "-c", "help")
        self.commit(
            HELP_SOURCE,
            "// old help\n",
            "old Help update",
            extra_files=((HELP_SNAPSHOT, "old exact snapshot\n"),),
        )
        help_tip = self.git("rev-parse", "HEAD").stdout.strip()

        self.git("switch", "-q", "master")
        self.commit(
            "crates/hw_core/src/lib.rs",
            "// parallel production\n",
            "parallel production",
        )
        self.git("merge", "-q", "--no-ff", "--no-edit", help_tip)

        self.assertFalse(self.evaluate_from(base).passed)

    def test_merge_resolution_requires_a_descendant_decision(self) -> None:
        base = self.commit(
            "crates/hw_core/src/lib.rs",
            "base\n",
            "base",
        )
        self.git("switch", "-q", "-c", "left")
        self.commit(
            "crates/hw_core/src/lib.rs",
            "left\n",
            "left production",
        )
        left_tip = self.commit(
            "LEFT.md",
            "decision\n",
            "left decision",
            no_impact_reason="Left implementation only",
        )

        self.git("switch", "-q", "master")
        self.commit(
            "crates/hw_core/src/lib.rs",
            "right\n",
            "right production",
        )
        self.commit(
            "RIGHT.md",
            "decision\n",
            "right decision",
            no_impact_reason="Right implementation only",
        )
        merge = self.git(
            "merge",
            "-q",
            "--no-ff",
            "--no-edit",
            left_tip,
            check=False,
        )
        self.assertNotEqual(merge.returncode, 0)
        self.write("crates/hw_core/src/lib.rs", "resolved\n")
        self.git("add", "--", "crates/hw_core/src/lib.rs")
        self.git("commit", "-q", "-m", "merge resolution")

        merge_revision = self.git("rev-parse", "HEAD").stdout.strip()
        self.assertEqual(
            check_help_impact.changed_paths_for_commit(
                merge_revision,
                root=self.root,
            ),
            ("crates/hw_core/src/lib.rs",),
        )
        self.assertFalse(self.evaluate_from(base).passed)

        self.commit(
            "README.md",
            "post-merge decision\n",
            "post-merge Help decision",
            no_impact_reason="Reviewed merged behavior",
        )
        self.assertTrue(self.evaluate_from(base).passed)


if __name__ == "__main__":
    unittest.main()
