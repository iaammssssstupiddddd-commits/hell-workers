#!/usr/bin/env python3
"""Validate active AI instructions against repository-owned contracts."""

from __future__ import annotations

import re
import subprocess
import sys
import tomllib
from collections.abc import Iterable
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent

ROOT_RULE_FILES = (
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursorrules",
    ".kilocoderules",
    ".github/copilot-instructions.md",
    ".gemini/antigravity/project_rules.md",
)

MANDATORY_HELP_REVIEW_RULE = (
    "You MUST use the repository `hell-workers-review-help-impact` Skill after "
    "implementing, changing, or removing functionality, code, or runtime data "
    "and before reporting completion, committing, or publishing."
)
MANDATORY_HELP_REVIEW_DECISION_RULE = (
    "Complete the Skill's `Update required` / `No impact` decision from the "
    "actual player-visible path; a passing Help impact gate alone does not "
    "count as the review."
)
MANDATORY_HELP_REVIEW_FALLBACK_RULE = (
    "If the current product does not expose that Skill natively, read and "
    "follow `.cursor/skills/hell-workers-review-help-impact/SKILL.md` directly "
    "before completion."
)
MANDATORY_HELP_REVIEW_FILES = (
    *ROOT_RULE_FILES,
    ".agent/rules/help-impact.md",
    ".cursor/rules/help-impact.mdc",
    ".agent/workflows/task-lifecycle.md",
    ".cursor/workflows/task-lifecycle.md",
)
MANDATORY_HELP_REVIEW_MARKERS = (
    MANDATORY_HELP_REVIEW_RULE,
    MANDATORY_HELP_REVIEW_DECISION_RULE,
    MANDATORY_HELP_REVIEW_FALLBACK_RULE,
)
MANDATORY_NATIVE_ACCEPTANCE_RULE = (
    "You MUST use the repository `hell-workers-run-native-acceptance` Skill "
    "whenever a task requires real-machine or native acceptance, actual-window, "
    "renderer/GPU/backend, or native performance verification, including requests "
    "for `実機確認` or `実機テスト`."
)
MANDATORY_NATIVE_ACCEPTANCE_LAUNCHER_RULE = (
    "Use the Skill's established no-prompt launcher and fail-closed artifact "
    "verification; do not ask the user for repeated display or GUI permissions "
    "while that launcher is available."
)
MANDATORY_NATIVE_ACCEPTANCE_FALLBACK_RULE = (
    "If the current product does not expose that Skill natively, read and follow "
    "`.cursor/skills/hell-workers-run-native-acceptance/SKILL.md` directly."
)
MANDATORY_NATIVE_ACCEPTANCE_MARKERS = (
    MANDATORY_NATIVE_ACCEPTANCE_RULE,
    MANDATORY_NATIVE_ACCEPTANCE_LAUNCHER_RULE,
    MANDATORY_NATIVE_ACCEPTANCE_FALLBACK_RULE,
)

SKILL_FILES = (
    ".codex/skills/hell-workers-update-docs/SKILL.md",
    ".cursor/skills/hell-workers-update-docs/SKILL.md",
    ".gemini/skills/hell-workers-update-docs/SKILL.md",
    ".claude-plugin/skills/update-docs/SKILL.md",
    ".codex/skills/hell-workers-review-help-impact/SKILL.md",
    ".codex/skills/hell-workers-review-help-impact/agents/openai.yaml",
    ".cursor/skills/hell-workers-review-help-impact/SKILL.md",
    ".gemini/skills/hell-workers-review-help-impact/SKILL.md",
    ".claude-plugin/skills/review-help-impact/SKILL.md",
    ".codex/skills/hell-workers-run-native-acceptance/SKILL.md",
    ".codex/skills/hell-workers-run-native-acceptance/agents/openai.yaml",
    ".cursor/skills/hell-workers-run-native-acceptance/SKILL.md",
    ".gemini/skills/hell-workers-run-native-acceptance/SKILL.md",
    ".claude-plugin/skills/run-native-acceptance/SKILL.md",
    ".codex/skills/hell-workers-update-docs/agents/openai.yaml",
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


def missing_mandatory_help_review_rules(
    paths: Iterable[Path],
) -> tuple[Path, ...]:
    missing = []
    for path in paths:
        if not path.is_file():
            missing.append(path)
            continue
        content = path.read_text(encoding="utf-8")
        if any(marker not in content for marker in MANDATORY_HELP_REVIEW_MARKERS):
            missing.append(path)
    return tuple(missing)


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

    mandatory_help_review_paths = tuple(
        REPO_ROOT / path for path in MANDATORY_HELP_REVIEW_FILES
    )
    for path in missing_mandatory_help_review_rules(
        mandatory_help_review_paths
    ):
        violations.append(
            f"{path.relative_to(REPO_ROOT)}: mandatory Help impact review "
            "rule is missing"
        )

    for path in (REPO_ROOT / relative for relative in ROOT_RULE_FILES):
        if not path.is_file():
            violations.append(
                f"{path.relative_to(REPO_ROOT)}: mandatory native acceptance rule is missing"
            )
            continue
        content = path.read_text(encoding="utf-8")
        if any(marker not in content for marker in MANDATORY_NATIVE_ACCEPTANCE_MARKERS):
            violations.append(
                f"{path.relative_to(REPO_ROOT)}: mandatory native acceptance rule is missing"
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
            "agent skills differ from .cursor canonical; "
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
