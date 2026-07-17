#!/usr/bin/env python3
"""Validate active AI instructions against repository-owned contracts."""

from __future__ import annotations

import re
import subprocess
import sys
import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent

ROOT_RULE_FILES = (
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".kilocoderules",
    ".github/copilot-instructions.md",
)

SKILL_FILES = (
    ".codex/skills/hell-workers-update-docs/SKILL.md",
    ".cursor/skills/hell-workers-update-docs/SKILL.md",
    ".gemini/skills/hell-workers-update-docs/SKILL.md",
    ".claude-plugin/skills/update-docs/SKILL.md",
)

CANONICAL_PATHS = (
    "crates/hw_jobs/src/tasks/mod.rs",
    "crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs",
    "crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs",
    "docs/plans/README.md",
    "docs/plans/plan-template.md",
)

STALE_PATTERNS = (
    (
        "removed task model path",
        re.compile(
            r"(?:crates/bevy_app/)?src/systems/soul_ai/(?:execute/)?"
            r"task_execution/(?:types|context)\.rs"
        ),
    ),
    (
        "removed GameAssets adapter path",
        re.compile(r"(?:crates/)?bevy_app/src/entities/game_assets\.rs"),
    ),
    (
        "removed AI rules plan",
        re.compile(r"multi-tool-ai-rules-plan(?:\.md)?"),
    ),
    (
        "incorrect tracked-plan policy",
        re.compile(r"docs/plans/[^\n]*(?:gitignored|gitignore対象)", re.IGNORECASE),
    ),
    (
        "mandatory milestone stop conflicts with directive scope",
        re.compile(r"(?:Stop\s*&\s*Wait|必ず作業を停止)", re.IGNORECASE),
    ),
)


def bevy_version() -> str:
    with (REPO_ROOT / "Cargo.toml").open("rb") as source:
        cargo = tomllib.load(source)
    dependency = cargo["workspace"]["dependencies"]["bevy"]
    if isinstance(dependency, str):
        return dependency
    return str(dependency["version"])


def active_rule_files() -> list[Path]:
    files = [REPO_ROOT / path for path in ROOT_RULE_FILES]
    files.extend(sorted((REPO_ROOT / ".agent" / "rules").glob("*.md")))
    files.extend(sorted((REPO_ROOT / ".cursor" / "rules").glob("*.mdc")))
    files.extend(sorted((REPO_ROOT / ".cursor" / "docs").glob("*.md")))
    files.extend(REPO_ROOT / path for path in SKILL_FILES)
    files.extend(sorted((REPO_ROOT / "crates").rglob("_rules.md")))
    return files


def find_violations() -> list[str]:
    expected_bevy = bevy_version()
    violations: list[str] = []

    for relative in CANONICAL_PATHS:
        if not (REPO_ROOT / relative).is_file():
            violations.append(f"{relative}: canonical file is missing")

    for path in active_rule_files():
        relative = path.relative_to(REPO_ROOT)
        if not path.is_file():
            violations.append(f"{relative}: active rule file is missing")
            continue

        content = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(content.splitlines(), start=1):
            if "/home/" in line or "/Users/" in line:
                violations.append(
                    f"{relative}:{line_number}: personal absolute path in active rule"
                )
            for label, pattern in STALE_PATTERNS:
                if pattern.search(line):
                    violations.append(f"{relative}:{line_number}: {label}")

            declares_current_bevy = any(
                marker in line
                for marker in ("Engine", "本プロジェクトは", "project uses Bevy")
            )
            version_matches = (
                re.finditer(r"Bevy\s+\*{0,2}(0\.\d+)", line)
                if declares_current_bevy
                else ()
            )
            for match in version_matches:
                if match.group(1) != expected_bevy:
                    violations.append(
                        f"{relative}:{line_number}: Bevy {match.group(1)} "
                        f"does not match Cargo.toml {expected_bevy}"
                    )
            for match in re.finditer(r"docs\.rs/bevy/(0\.\d+)(?:\.\d+)?", line):
                if match.group(1) != expected_bevy:
                    violations.append(
                        f"{relative}:{line_number}: docs.rs Bevy {match.group(1)} "
                        f"does not match Cargo.toml {expected_bevy}"
                    )
            match = re.search(r"Bevy API[^\n]*(0\.\d+)\s*系", line)
            if match and match.group(1) != expected_bevy:
                violations.append(
                    f"{relative}:{line_number}: Bevy API {match.group(1)} "
                    f"does not match Cargo.toml {expected_bevy}"
                )

            if (
                "AssignedTask::None" in line
                and "OnTaskCompleted" in line
                and ("発火させる" in line or "emits" in line)
            ):
                violations.append(
                    f"{relative}:{line_number}: contradicts docs/invariants.md I-S3"
                )

    for path in sorted((REPO_ROOT / "crates").rglob("*")):
        if path.name not in {"AGENTS.md", "CLAUDE.md"} or not path.is_symlink():
            continue
        if not path.exists():
            violations.append(
                f"{path.relative_to(REPO_ROOT)}: broken rule symlink -> {path.readlink()}"
            )

    skill_sync = subprocess.run(
        [sys.executable, "scripts/sync_agent_skills.py", "--check"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if skill_sync.returncode != 0:
        violations.append(
            "agent docs skills differ from .cursor canonical; "
            "run python3 scripts/sync_agent_skills.py --write"
        )

    return sorted(set(violations))


def main() -> int:
    violations = find_violations()
    if violations:
        print("AI rule contract violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1
    print(f"AI rule contracts: pass (Bevy {bevy_version()})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
