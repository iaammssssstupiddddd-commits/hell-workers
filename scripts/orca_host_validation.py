#!/usr/bin/env python3
"""Run a frozen worktree's quality command with the current host controller.

The repository under test remains the source of contracts, tests and product
code.  Only the process/lease runner comes from the active Orca controller so
long-lived worktrees do not retain obsolete host coordination behavior.
"""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path
from typing import Sequence

if __package__:
    from . import dev
else:
    import dev


ALLOWED_COMMANDS = {"check", "verify", "quality", "ci", "lint", "deps", "docs"}


def checked_repo(value: Path) -> Path:
    if not value.is_absolute():
        raise ValueError("host validation requires an absolute canonical repository with scripts/dev.py")
    try:
        repo = value.resolve(strict=True)
    except OSError as error:
        raise ValueError(
            "host validation requires an absolute canonical repository with scripts/dev.py"
        ) from error
    if repo != value or not (repo / "scripts/dev.py").is_file():
        raise ValueError("host validation requires an absolute canonical repository with scripts/dev.py")
    top = subprocess.check_output(
        ["git", "-C", str(repo), "rev-parse", "--show-toplevel"], text=True,
    ).strip()
    if Path(top).resolve() != repo:
        raise ValueError("host validation repository is not the exact Git worktree root")
    return repo


def run(repo: Path, arguments: Sequence[str]) -> int:
    checked = checked_repo(repo)
    argv = list(arguments)
    if not argv or argv[0] not in ALLOWED_COMMANDS:
        raise ValueError("host validation accepts only guarded development commands")
    original_root, original_scripts = dev.REPO_ROOT, dev.SCRIPTS_DIR
    try:
        dev.REPO_ROOT = checked
        dev.SCRIPTS_DIR = checked / "scripts"
        return dev.main(argv)
    finally:
        dev.REPO_ROOT, dev.SCRIPTS_DIR = original_root, original_scripts


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    arguments = list(args.arguments)
    if arguments[:1] == ["--"]:
        arguments.pop(0)
    try:
        return run(args.repo, arguments)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(str(error), file=dev.sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
