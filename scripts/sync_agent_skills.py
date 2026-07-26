#!/usr/bin/env python3
"""Check or sync repository Agent Skill bodies from canonical Cursor copies."""

from __future__ import annotations

import argparse
import re
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import TextIO


REPO_ROOT = Path(__file__).resolve().parent.parent


@dataclass(frozen=True)
class SkillDocument:
    path: Path
    expected_name: str


@dataclass(frozen=True)
class SkillSync:
    canonical: SkillDocument
    targets: tuple[SkillDocument, ...]


@dataclass(frozen=True)
class CodexSkillMetadata:
    path: Path
    skill_name: str


def _skill(relative_path: str, expected_name: str) -> SkillDocument:
    return SkillDocument(REPO_ROOT / relative_path, expected_name)


SKILL_SYNCS = (
    SkillSync(
        canonical=_skill(
            ".cursor/skills/hell-workers-update-docs/SKILL.md",
            "hell-workers-update-docs",
        ),
        targets=(
            _skill(
                ".codex/skills/hell-workers-update-docs/SKILL.md",
                "hell-workers-update-docs",
            ),
            _skill(
                ".gemini/skills/hell-workers-update-docs/SKILL.md",
                "hell-workers-update-docs",
            ),
            _skill(
                ".claude-plugin/skills/update-docs/SKILL.md",
                "update-docs",
            ),
        ),
    ),
    SkillSync(
        canonical=_skill(
            ".cursor/skills/hell-workers-review-help-impact/SKILL.md",
            "hell-workers-review-help-impact",
        ),
        targets=(
            _skill(
                ".codex/skills/hell-workers-review-help-impact/SKILL.md",
                "hell-workers-review-help-impact",
            ),
            _skill(
                ".gemini/skills/hell-workers-review-help-impact/SKILL.md",
                "hell-workers-review-help-impact",
            ),
            _skill(
                ".claude-plugin/skills/review-help-impact/SKILL.md",
                "review-help-impact",
            ),
        ),
    ),
)

CODEX_SKILL_METADATA = (
    CodexSkillMetadata(
        REPO_ROOT
        / ".codex/skills/hell-workers-update-docs/agents/openai.yaml",
        "hell-workers-update-docs",
    ),
    CodexSkillMetadata(
        REPO_ROOT
        / ".codex/skills/hell-workers-review-help-impact/agents/openai.yaml",
        "hell-workers-review-help-impact",
    ),
)

_BLOCK_SCALAR_MARKERS = frozenset({">", ">-", "|", "|-"})
_SKILL_NAME_PATTERN = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


def split_frontmatter(content: str) -> tuple[str, str]:
    if not content.startswith("---\n"):
        raise ValueError("SKILL.md must start with YAML frontmatter")
    end = content.find("\n---\n", 4)
    if end < 0:
        raise ValueError("SKILL.md frontmatter is not terminated")
    boundary = end + len("\n---\n")
    return content[:boundary], content[boundary:].lstrip("\n")


def validate_skill_frontmatter(content: str, expected_name: str) -> None:
    frontmatter, _ = split_frontmatter(content)
    fields: dict[str, str] = {}
    active_block: str | None = None

    for line in frontmatter.splitlines()[1:-1]:
        if not line.strip():
            continue
        if line[0].isspace():
            if active_block is None:
                raise ValueError("unexpected indented Skill frontmatter content")
            fields[active_block] = " ".join(
                part for part in (fields[active_block], line.strip()) if part
            )
            continue

        active_block = None
        match = re.fullmatch(r"(name|description):[ \t]*(.*)", line)
        if match is None:
            raise ValueError(f"unsupported or malformed Skill frontmatter: {line}")
        field, raw_value = match.groups()
        if field in fields:
            raise ValueError(f"duplicate Skill frontmatter field: {field}")

        value = raw_value.strip()
        if value in _BLOCK_SCALAR_MARKERS:
            if field != "description":
                raise ValueError(f"{field} cannot use a block scalar")
            fields[field] = ""
            active_block = field
            continue
        if value.startswith(("'", '"')):
            if len(value) < 2 or value[-1] != value[0]:
                raise ValueError(f"unterminated quoted Skill {field}")
            value = value[1:-1]
        elif value.startswith(("[", "{", "&", "*", "!")) or ": " in value:
            raise ValueError(f"unsupported Skill {field} scalar")
        fields[field] = value

    missing = {"name", "description"} - fields.keys()
    if missing:
        raise ValueError(
            f"missing Skill frontmatter field(s): {', '.join(sorted(missing))}"
        )

    name = fields["name"].strip()
    if not _SKILL_NAME_PATTERN.fullmatch(name):
        raise ValueError(f"invalid Skill name: {name!r}")
    if name != expected_name:
        raise ValueError(
            f"unexpected Skill name {name!r}; expected {expected_name!r}"
        )

    description = fields["description"].strip()
    if not description:
        raise ValueError("Skill description must not be empty")
    if len(description) > 1024:
        raise ValueError("Skill description exceeds 1024 characters")
    if "<" in description or ">" in description:
        raise ValueError("Skill description cannot contain angle brackets")


def expected_target(content: str, canonical_body: str) -> str:
    frontmatter, _ = split_frontmatter(content)
    return f"{frontmatter}\n{canonical_body}"


def _display_path(path: Path) -> str:
    try:
        return str(path.relative_to(REPO_ROOT))
    except ValueError:
        return str(path)


def validate_codex_metadata(
    metadata_documents: Sequence[CodexSkillMetadata],
) -> None:
    for document in metadata_documents:
        if not document.path.is_file():
            raise FileNotFoundError(
                f"Codex Skill metadata is missing: {_display_path(document.path)}"
            )
        lines = [
            line
            for line in document.path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        if not lines or lines[0] != "interface:":
            raise ValueError(
                f"{_display_path(document.path)}: expected interface mapping"
            )

        fields: dict[str, str] = {}
        for line in lines[1:]:
            match = re.fullmatch(
                r'  (display_name|short_description|default_prompt): "([^"]+)"',
                line,
            )
            if match is None:
                raise ValueError(
                    f"{_display_path(document.path)}: "
                    f"unsupported or malformed metadata line: {line}"
                )
            field, value = match.groups()
            if field in fields:
                raise ValueError(
                    f"{_display_path(document.path)}: duplicate {field}"
                )
            fields[field] = value

        required = {"display_name", "short_description", "default_prompt"}
        missing = required - fields.keys()
        if missing:
            raise ValueError(
                f"{_display_path(document.path)}: missing metadata field(s): "
                f"{', '.join(sorted(missing))}"
            )
        if not 25 <= len(fields["short_description"]) <= 64:
            raise ValueError(
                f"{_display_path(document.path)}: short_description must be "
                "25-64 characters"
            )
        invocation = f"${document.skill_name}"
        if invocation not in fields["default_prompt"]:
            raise ValueError(
                f"{_display_path(document.path)}: default_prompt must mention "
                f"{invocation}"
            )


def sync_skills(
    skill_syncs: Sequence[SkillSync],
    *,
    write: bool,
    output: TextIO = sys.stdout,
    error: TextIO = sys.stderr,
) -> tuple[Path, ...]:
    seen_paths: set[Path] = set()
    prepared_targets: list[tuple[Path, str, str]] = []
    stale: list[Path] = []

    for skill_sync in skill_syncs:
        group_paths = (skill_sync.canonical, *skill_sync.targets)
        for document in group_paths:
            path = document.path
            if path in seen_paths:
                raise ValueError(
                    f"duplicate Agent Skill sync path: {_display_path(path)}"
                )
            seen_paths.add(path)

        canonical_path = skill_sync.canonical.path
        if not canonical_path.is_file():
            raise FileNotFoundError(
                f"canonical Agent Skill is missing: "
                f"{_display_path(canonical_path)}"
            )
        canonical_content = canonical_path.read_text(encoding="utf-8")
        validate_skill_frontmatter(
            canonical_content,
            skill_sync.canonical.expected_name,
        )
        _, canonical_body = split_frontmatter(canonical_content)

        for document in skill_sync.targets:
            path = document.path
            if not path.is_file():
                raise FileNotFoundError(
                    f"Agent Skill adapter is missing: {_display_path(path)}"
                )
            content = path.read_text(encoding="utf-8")
            validate_skill_frontmatter(content, document.expected_name)
            expected = expected_target(content, canonical_body)
            prepared_targets.append((path, content, expected))

    for path, content, expected in prepared_targets:
        if content == expected:
            print(f"OK {_display_path(path)}", file=output)
            continue
        stale.append(path)
        if write:
            path.write_text(expected, encoding="utf-8")
            print(f"Updated {_display_path(path)}", file=output)
        else:
            print(f"Stale {_display_path(path)}", file=error)

    return tuple(stale)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="validate without writing")
    mode.add_argument("--write", action="store_true", help="sync target skill bodies")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        validate_codex_metadata(CODEX_SKILL_METADATA)
        stale = sync_skills(SKILL_SYNCS, write=args.write)
    except (OSError, ValueError) as failure:
        print(f"Agent Skill sync failed: {failure}", file=sys.stderr)
        return 1
    return 1 if stale and args.check else 0


if __name__ == "__main__":
    raise SystemExit(main())
