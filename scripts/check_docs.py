#!/usr/bin/env python3
"""Check active Markdown links and the root documentation index."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote


REPO_ROOT = Path(__file__).resolve().parent.parent
DOCS_ROOT = REPO_ROOT / "docs"
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^]]*]\((?P<target>[^)]+)\)")
EXCLUDED_PARTS = {"archive", "archived", "rejected"}
STALE_TASK_REFERENCES = {
    "crates/bevy_app/src/systems/soul_ai/execute/task_execution/types.rs": (
        "AssignedTask is defined in crates/hw_jobs/src/tasks/mod.rs"
    ),
    "hw_jobs::assigned_task": "use hw_jobs::tasks or hw_jobs::AssignedTask",
}


def repository_markdown_files() -> list[Path]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.md",
        ],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    files: list[Path] = []
    for entry in result.stdout.splitlines():
        path = REPO_ROOT / entry
        if not path.is_file():
            continue
        relative = path.relative_to(REPO_ROOT)
        if relative.parts[0] == "docs" and EXCLUDED_PARTS.intersection(relative.parts):
            continue
        if relative.parts[0] in {".codex", ".cursor", ".gemini", ".claude-plugin"}:
            continue
        if relative.name in {"AGENTS.md", "CLAUDE.md", "GEMINI.md"} and len(relative.parts) > 1:
            continue
        if relative.parts[0] == "docs" or relative.name in {
            "README.md",
            "AGENTS.md",
            "CLAUDE.md",
            "GEMINI.md",
        }:
            files.append(path)
    return sorted(set(files))


def normalized_target(raw_target: str) -> str:
    target = raw_target.strip()
    if target.startswith("<") and ">" in target:
        target = target[1 : target.index(">")]
    elif " " in target:
        target = target.split(" ", 1)[0]
    return unquote(target).split("#", 1)[0].split("?", 1)[0]


def find_violations() -> list[str]:
    violations: list[str] = []
    for path in repository_markdown_files():
        relative = path.relative_to(REPO_ROOT)
        content = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(content.splitlines(), start=1):
            for stale, guidance in STALE_TASK_REFERENCES.items():
                if stale in line:
                    violations.append(
                        f"{relative}:{line_number}: stale task reference {stale!r}; {guidance}"
                    )
            for match in MARKDOWN_LINK.finditer(line):
                raw_target = match.group("target")
                target = normalized_target(raw_target)
                if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                    continue
                if target.startswith(("file://", "/home/", "/Users/")):
                    violations.append(
                        f"{relative}:{line_number}: non-portable link: {raw_target}"
                    )
                    continue
                resolved = (path.parent / target).resolve()
                if not resolved.exists():
                    violations.append(
                        f"{relative}:{line_number}: missing link target: {raw_target}"
                    )

    index = (DOCS_ROOT / "README.md").read_text(encoding="utf-8")
    for path in sorted(DOCS_ROOT.glob("*.md")):
        if path.name == "README.md":
            continue
        if re.search(rf"\]\((?:\./)?{re.escape(path.name)}(?:[#)])", index):
            continue
        violations.append(f"docs/README.md: missing root index entry for {path.name}")

    return sorted(set(violations))


def main() -> int:
    violations = find_violations()
    if violations:
        print("Documentation contract violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1
    print("Documentation links and root index: pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
