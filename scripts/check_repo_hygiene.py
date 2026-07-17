#!/usr/bin/env python3
"""Validate secret hygiene, generated files, and executable script modes."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
UNSAFE_CREDENTIAL_PATTERNS = (
    re.compile(r"git\s+config[^\n]*credential\.helper\s+store"),
    re.compile(r"https://[^\s/@]*(?:token|TOKEN|GITHUB_TOKEN)[^\s@]*@"),
    re.compile(r"GITHUB_TOKEN=[^\s]+\s*>\s*\.env"),
)


def git_index() -> dict[str, str]:
    result = subprocess.run(
        ["git", "ls-files", "-s"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    entries: dict[str, str] = {}
    for line in result.stdout.splitlines():
        metadata, path = line.split("\t", 1)
        entries[path] = metadata.split()[0]
    return entries


def iter_json_strings(value: object):
    if isinstance(value, str):
        yield value
    elif isinstance(value, list):
        for item in value:
            yield from iter_json_strings(item)
    elif isinstance(value, dict):
        for item in value.values():
            yield from iter_json_strings(item)


def is_local_env_file(path: str) -> bool:
    """Return whether a path names a local dotenv file that must not be tracked."""
    name = Path(path).name
    return name == ".env" or (name.startswith(".env.") and name != ".env.example")


def find_violations() -> list[str]:
    violations: list[str] = []
    index = git_index()

    for path, mode in sorted(index.items()):
        parts = Path(path).parts
        absolute = REPO_ROOT / path
        if absolute.exists() and (
            "__pycache__" in parts or path.endswith((".pyc", ".pyo"))
        ):
            violations.append(f"{path}: generated Python bytecode is tracked")
        if is_local_env_file(path):
            violations.append(f"{path}: local secret file is tracked")
        if path.startswith("scripts/") and absolute.is_file():
            try:
                first_line = absolute.open(encoding="utf-8").readline()
            except UnicodeDecodeError:
                continue
            if first_line.startswith("#!") and mode != "100755":
                violations.append(
                    f"{path}: shebang script must have Git executable mode 100755"
                )

    gitignore = (REPO_ROOT / ".gitignore").read_text(encoding="utf-8")
    for required in (".env", ".env.*", "!.env.example", "__pycache__/", "*.py[cod]"):
        if required not in gitignore.splitlines():
            violations.append(f".gitignore: missing required pattern {required}")

    if (REPO_ROOT / "scripts" / "update-git-credentials.sh").exists():
        violations.append("scripts/update-git-credentials.sh: unsafe credential helper remains")

    for path in sorted((REPO_ROOT / "scripts").rglob("*")):
        if not path.is_file() or path.name == Path(__file__).name:
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for pattern in UNSAFE_CREDENTIAL_PATTERNS:
            if pattern.search(content):
                violations.append(f"{path.relative_to(REPO_ROOT)}: unsafe credential pattern")

    for relative in (".mcp.json", ".gemini/settings.json"):
        path = REPO_ROOT / relative
        data = json.loads(path.read_text(encoding="utf-8"))
        for value in iter_json_strings(data):
            if value.startswith(("/home/", "/Users/")):
                violations.append(f"{relative}: personal absolute path {value}")

    return sorted(set(violations))


def main() -> int:
    violations = find_violations()
    if violations:
        print("Repository hygiene violations:", file=sys.stderr)
        for violation in violations:
            print(f"- {violation}", file=sys.stderr)
        return 1
    print("Repository secret, generated-file, and script-mode hygiene: pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
