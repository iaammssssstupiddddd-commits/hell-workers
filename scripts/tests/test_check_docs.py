"""The documentation gate checks owned docs, not installed dependencies."""

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from scripts import check_docs


class DocumentationScopeTests(unittest.TestCase):
    def test_vendor_readmes_excluded_but_untracked_project_docs_retained(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            paths = [
                "README.md", "docs/new.md", "tools/example/README.md",
                "node_modules/debug/README.md", "docs/node_modules/README.md",
            ]
            for relative in paths:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("[missing](absent.md)\n", encoding="utf-8")
            result = subprocess.CompletedProcess([], 0, "\n".join(paths), "")
            with patch.object(check_docs, "REPO_ROOT", root), patch.object(
                check_docs.subprocess, "run", return_value=result
            ):
                selected = check_docs.repository_markdown_files()
            self.assertEqual(
                {str(path.relative_to(root)) for path in selected},
                set(paths) - {"node_modules/debug/README.md"},
            )


if __name__ == "__main__":
    unittest.main()
