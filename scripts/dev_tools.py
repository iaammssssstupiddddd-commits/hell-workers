"""Pinned quality tools; diagnosis and checks never install software."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import tomllib
from pathlib import Path
from typing import Mapping, Sequence

TOOL_NAMES = ("cargo-deny", "ruff", "actionlint")
INSTALL_HELP = (
    "See docs/DEVELOPMENT.md (quality tools). Linux x86_64: "
    "python3 scripts/install_dev_tools.py --bin-dir ~/.local/bin; "
    "put that directory on PATH."
)


def load_manifest(repo: Path) -> dict[str, dict[str, str]]:
    with (repo / "scripts/dev-tools.toml").open("rb") as source:
        manifest = tomllib.load(source)
    if set(manifest) != set(TOOL_NAMES):
        raise ValueError("dev-tools.toml must define cargo-deny, ruff and actionlint")
    for name, spec in manifest.items():
        if not isinstance(spec, dict) or not re.fullmatch(
            r"\d+\.\d+\.\d+", str(spec.get("version", ""))
        ):
            raise ValueError(f"invalid pinned version for {name}")
    return manifest


def resolve_tool(name: str, environment: Mapping[str, str]) -> str | None:
    search = environment.get("PATH", "")
    if name == "cargo-deny":
        # Match Cargo's external-subcommand precedence, not just the shell PATH.
        search = os.pathsep.join((str(Path(environment["CARGO_HOME"]) / "bin"), search))
    return shutil.which(name, path=search)


def probe_tool(name: str, version: str, environment: Mapping[str, str]) -> str:
    binary = resolve_tool(name, environment)
    if binary is None:
        raise RuntimeError(f"{name} {version} is missing. {INSTALL_HELP}")
    result = subprocess.run(
        [binary, "--version"], env=dict(environment), check=False,
        capture_output=True, text=True, timeout=10,
    )
    output = result.stdout.strip()
    match = re.match(rf"(?:{re.escape(name)} )?(\d+\.\d+\.\d+)(?:\s|$)", output)
    if result.returncode or match is None or match[1] != version:
        raise RuntimeError(
            f"{name}: expected {version}, found {output or result.stderr.strip()!r} "
            f"at {binary}. {INSTALL_HELP}"
        )
    print(f"{name} {version}: {binary}")
    return binary


def preflight(
    repo: Path, environment: Mapping[str, str], names: Sequence[str] = TOOL_NAMES,
) -> dict[str, str]:
    manifest = load_manifest(repo)
    if "ruff" in names:
        with (repo / "ruff.toml").open("rb") as source:
            required = tomllib.load(source).get("required-version")
        if required != f"=={manifest['ruff']['version']}":
            raise RuntimeError("ruff.toml required-version differs from dev-tools.toml")
    return {name: probe_tool(name, manifest[name]["version"], environment) for name in names}


def lint_commands(repo: Path, binaries: Mapping[str, str]) -> list[list[str]]:
    return [
        [binaries["ruff"], "check", "--no-cache", "--config", str(repo / "ruff.toml"), "scripts"],
        [binaries["actionlint"], "-shellcheck=", "-pyflakes="],
    ]


def deps_command(repo: Path, binary: str, *, offline: bool) -> list[str]:
    # cargo-deny also supports direct execution; use the exact probed binary.
    command = [binary, "--manifest-path", str(repo / "Cargo.toml"),
               "--config", str(repo / "deny.toml"), "--workspace", "--locked"]
    if offline:
        command.append("--offline")
    return [*command, "check", "--hide-inclusion-graph"]
