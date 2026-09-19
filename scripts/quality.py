"""Shared quality groups for local development and GitHub Actions."""

from __future__ import annotations

import platform
import sys
from dataclasses import dataclass
from typing import Callable

GROUPS = ("contracts", "tooling", "deps", "rust")


@dataclass(frozen=True)
class Runner:
    command: Callable[..., None]
    tools: Callable[..., None]
    suppressions: Callable[[], None]
    hygiene: Callable[[], None]
    environment: dict[str, str]

    def python(self, *arguments: str) -> None:
        self.command([sys.executable, *arguments], extra_env=self.environment)


def run_group(group: str, runner: Runner) -> None:
    if group not in GROUPS:
        raise ValueError(f"unknown quality group: {group}")
    print(f"==> Quality group: {group}", flush=True)
    if group == "contracts":
        for script in (
            "validation_storage", "check_agent_rules", "check_help_impact",
            "check_repo_hygiene", "check_crate_dependencies",
        ):
            arguments = ("check",) if script == "validation_storage" else ()
            runner.python(f"scripts/{script}.py", *arguments)
        runner.python("scripts/update_docs_index.py", "--check")
        runner.python("scripts/check_docs.py")
        runner.suppressions()
        runner.hygiene()
    elif group == "tooling":
        runner.tools(lint=True)
        for directory in ("scripts/tests", "tools/blender_ai_workflow/tests"):
            runner.python("-m", "unittest", "discover", "-s", directory, "-p", "test_*.py")
        runner.python("scripts/perf.py", "self-test")
    elif group == "deps":
        runner.tools(deps=True)
    else:
        runner.command(["cargo", "fmt", "--all", "--check"])
        runner.command(["cargo", "check", "--workspace", "--locked"])
        runner.command([
            "cargo", "test", "--workspace", "--no-default-features",
            "--features", "profiling", "--locked",
        ])
        features = ["profiling-memory", "profiling-tracy"]
        if platform.system() in {"Linux", "Windows"}:
            features.append("profiling-renderdoc")
        for feature in features:
            runner.command([
                "cargo", "check", "-p", "bevy_app@0.1.0", "--lib",
                "--no-default-features", "--features", feature, "--locked",
            ])
        runner.command([
            "cargo", "clippy", "--workspace", "--all-targets", "--locked",
            "--", "-D", "warnings",
        ])
        runner.command(["cargo", "test", "--workspace", "--locked"])
    print(f"Quality group passed: {group}", flush=True)


def run_groups(groups: list[str], runner: Runner) -> None:
    if not groups or set(groups) - set(GROUPS) or len(groups) != len(set(groups)):
        raise ValueError("quality groups must be a non-empty set of known groups")
    for group in GROUPS:
        if group in groups:
            run_group(group, runner)
