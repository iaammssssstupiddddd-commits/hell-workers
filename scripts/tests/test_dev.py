from __future__ import annotations

import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path

from scripts import dev


class ClippySuppressionTests(unittest.TestCase):
    def test_detects_direct_multiline_and_cfg_attr_suppressions(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates" / "sample" / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                """
#[allow(clippy::too_many_arguments)]
fn direct() {}

#[expect(
    clippy::type_complexity,
    reason = "test"
)]
fn multiline() {}

#[cfg_attr(test, allow(clippy::needless_return))]
fn conditional() {}
""",
                encoding="utf-8",
            )

            violations = dev.find_clippy_suppressions(root)

        self.assertEqual(len(violations), 3)
        self.assertTrue(all("crates/sample/src/lib.rs" in item for item in violations))

    def test_ignores_non_clippy_attributes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "crates" / "sample" / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#[allow(dead_code)]\nfn sample() {}\n", encoding="utf-8")
            self.assertEqual(dev.find_clippy_suppressions(root), [])


class CliTests(unittest.TestCase):
    def test_docs_requires_explicit_mode(self) -> None:
        parser = dev.build_parser()
        with redirect_stderr(StringIO()), self.assertRaises(SystemExit):
            parser.parse_args(["docs"])

    def test_build_release_is_parsed(self) -> None:
        args = dev.build_parser().parse_args(["build", "--release"])
        self.assertTrue(args.release)


class DiffHygieneTests(unittest.TestCase):
    def test_local_check_includes_worktree_changes(self) -> None:
        self.assertEqual(
            dev.diff_hygiene_command({}),
            ["git", "diff", "HEAD", "--check"],
        )

    def test_ci_check_uses_the_event_range(self) -> None:
        self.assertEqual(
            dev.diff_hygiene_command({"HELL_WORKERS_DIFF_BASE": "abc123"}),
            ["git", "diff", "--check", "abc123...HEAD"],
        )

    def test_zero_before_sha_falls_back_to_worktree(self) -> None:
        self.assertEqual(
            dev.diff_hygiene_command({"HELL_WORKERS_DIFF_BASE": "0" * 40}),
            ["git", "diff", "HEAD", "--check"],
        )


if __name__ == "__main__":
    unittest.main()
