from __future__ import annotations

import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path

from scripts import update_docs_index


class IndexRenderingTests(unittest.TestCase):
    def test_current_plans_render_is_idempotent(self) -> None:
        readme = update_docs_index.PLANS_DIR / "README.md"
        content = readme.read_text(encoding="utf-8")
        first, _, _ = update_docs_index.render_plans_readme(content)
        second, _, _ = update_docs_index.render_plans_readme(first)
        self.assertEqual(first, second)

    def test_check_mode_does_not_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "README.md"
            path.write_text("before\n", encoding="utf-8")
            with redirect_stderr(StringIO()):
                stale = update_docs_index.apply_index(
                    path,
                    "after\n",
                    write=False,
                    current_count=1,
                    archive_count=0,
                )
            self.assertTrue(stale)
            self.assertEqual(path.read_text(encoding="utf-8"), "before\n")

    def test_replace_section_is_idempotent(self) -> None:
        content = "# Index\n\n## Current\n\n| old |\n\n## Next\n\ntext\n"
        rendered = update_docs_index.replace_section(
            content,
            "## Current",
            "| Document | Status | Notes |",
            "|---|---|---|",
            ["| [a.md](a.md) | Draft | A |"],
        )
        rerendered = update_docs_index.replace_section(
            rendered,
            "## Current",
            "| Document | Status | Notes |",
            "|---|---|---|",
            ["| [a.md](a.md) | Draft | A |"],
        )
        self.assertEqual(rendered, rerendered)


if __name__ == "__main__":
    unittest.main()
