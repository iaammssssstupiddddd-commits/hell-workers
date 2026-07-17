#!/usr/bin/env python3
"""Check or sync docs-skill bodies from the repository canonical copy."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
CANONICAL = REPO_ROOT / ".cursor/skills/hell-workers-update-docs/SKILL.md"
TARGETS = (
    REPO_ROOT / ".codex/skills/hell-workers-update-docs/SKILL.md",
    REPO_ROOT / ".gemini/skills/hell-workers-update-docs/SKILL.md",
    REPO_ROOT / ".claude-plugin/skills/update-docs/SKILL.md",
)


def split_frontmatter(content: str) -> tuple[str, str]:
    if not content.startswith("---\n"):
        raise ValueError("SKILL.md must start with YAML frontmatter")
    end = content.find("\n---\n", 4)
    if end < 0:
        raise ValueError("SKILL.md frontmatter is not terminated")
    boundary = end + len("\n---\n")
    return content[:boundary], content[boundary:].lstrip("\n")


def expected_target(content: str, canonical_body: str) -> str:
    frontmatter, _ = split_frontmatter(content)
    return f"{frontmatter}\n{canonical_body}"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="validate without writing")
    mode.add_argument("--write", action="store_true", help="sync target skill bodies")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    _, canonical_body = split_frontmatter(CANONICAL.read_text(encoding="utf-8"))
    stale: list[Path] = []

    for path in TARGETS:
        content = path.read_text(encoding="utf-8")
        expected = expected_target(content, canonical_body)
        if content == expected:
            print(f"OK {path.relative_to(REPO_ROOT)}")
            continue
        stale.append(path)
        if args.write:
            path.write_text(expected, encoding="utf-8")
            print(f"Updated {path.relative_to(REPO_ROOT)}")
        else:
            print(f"Stale {path.relative_to(REPO_ROOT)}", file=sys.stderr)

    return 1 if stale and args.check else 0


if __name__ == "__main__":
    raise SystemExit(main())
