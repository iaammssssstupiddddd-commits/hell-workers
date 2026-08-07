#!/usr/bin/env python3
"""Portable development entrypoint for hell-workers.

The commands in this module are intentionally implemented with the Python
standard library so environment diagnosis and quality checks do not require an
additional task runner.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import platform
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Sequence

try:
    from cargo_runtime import cargo_environment, require_cargo_memory
except ModuleNotFoundError:
    from scripts.cargo_runtime import cargo_environment, require_cargo_memory

try:
    from build_lane import lane_states, run_lane_shell, validate_inherited_lease
except ModuleNotFoundError:
    from scripts.build_lane import lane_states, run_lane_shell, validate_inherited_lease

try:
    from build_coordination import acquire_activity
except ModuleNotFoundError:
    from scripts.build_coordination import acquire_activity


REPO_ROOT = Path(__file__).resolve().parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
RUST_ATTRIBUTE = re.compile(r"#\s*!?\[(?P<body>.*?)]", re.DOTALL)
CLIPPY_SUPPRESSION = re.compile(
    r"\b(?:allow|expect)\s*\([^)]*\bclippy::", re.DOTALL
)
CARGO_OUTPUT_CONFIG = re.compile(
    r"(?:target-dir|build-dir)",
    re.IGNORECASE,
)


def command_text(command: Sequence[str]) -> str:
    """Return a shell-readable representation without invoking a shell."""
    return " ".join(subprocess.list2cmdline([part]) for part in command)


def reject_cargo_output_overrides(arguments: Sequence[str]) -> None:
    """Keep Cargo CLI options from bypassing the controlled output roots."""
    for index, argument in enumerate(arguments):
        if argument == "--target-dir" or argument.startswith("--target-dir="):
            raise RuntimeError(
                "Cargo target-dir overrides are not supported; use the controlled workspace or lane"
            )
        config_value: str | None = None
        if argument == "--config":
            if index + 1 < len(arguments):
                config_value = arguments[index + 1]
        elif argument.startswith("--config="):
            config_value = argument.removeprefix("--config=")
        if config_value is None:
            continue
        config_text = config_value.strip()
        config_override = CARGO_OUTPUT_CONFIG.search(config_text) is not None
        config_path = Path(config_text).expanduser()
        if not config_override and config_path.is_file():
            try:
                config_override = (
                    CARGO_OUTPUT_CONFIG.search(config_path.read_text(encoding="utf-8"))
                    is not None
                )
            except OSError as error:
                raise RuntimeError(
                    f"cannot inspect Cargo config override {config_path}: {error}"
                ) from error
        if config_override:
            raise RuntimeError(
                "Cargo target/build-dir config overrides are not supported; use the controlled workspace or lane"
            )


def run_command(
    command: Sequence[str],
    *,
    extra_env: dict[str, str] | None = None,
) -> None:
    """Run a command from the repository root and fail on a non-zero status."""
    print(f"+ {command_text(command)}", flush=True)
    env = os.environ.copy()
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    if extra_env:
        env.update(extra_env)
    lane = None
    requires_activity = False
    if command and Path(command[0]).name == "cargo":
        reject_cargo_output_overrides(command[1:])
        lane = validate_inherited_lease(REPO_ROOT, env)
        if len(command) > 1 and command[1] in {
            "bench",
            "build",
            "check",
            "clippy",
            "doc",
            "fix",
            "install",
            "run",
            "rustc",
            "test",
        }:
            requires_activity = True
    activity = None
    try:
        if requires_activity:
            activity = acquire_activity(REPO_ROOT, "shared")
            require_cargo_memory()
        if command and Path(command[0]).name == "cargo":
            env = cargo_environment(
                REPO_ROOT,
                namespace=".dev-tmp",
                environment=env,
                incremental=None,
                lane=lane,
            )
        subprocess.run(command, cwd=REPO_ROOT, env=env, check=True)
    finally:
        if activity is not None:
            activity.close()


def run_python_script(*arguments: str) -> None:
    run_command([sys.executable, *arguments])


def run_docs(*, write: bool) -> None:
    mode = "--write" if write else "--check"
    run_python_script(str(SCRIPTS_DIR / "update_docs_index.py"), mode)
    run_python_script(str(SCRIPTS_DIR / "check_docs.py"))


def find_clippy_suppressions(root: Path = REPO_ROOT) -> list[str]:
    """Return deterministic locations of repository-owned Clippy suppressions."""
    violations: list[str] = []
    for path in sorted((root / "crates").rglob("*.rs")):
        content = path.read_text(encoding="utf-8")
        for attribute in RUST_ATTRIBUTE.finditer(content):
            if not CLIPPY_SUPPRESSION.search(attribute.group("body")):
                continue
            line_number = content.count("\n", 0, attribute.start()) + 1
            relative = path.relative_to(root)
            snippet = " ".join(attribute.group(0).split())
            violations.append(f"{relative}:{line_number}:{snippet}")
    return violations


def check_clippy_suppressions() -> None:
    """Reject repository-owned Clippy allow/expect attributes."""
    violations = find_clippy_suppressions()

    if violations:
        print("Clippy suppressions are not allowed:", file=sys.stderr)
        print("\n".join(violations), file=sys.stderr)
        raise SystemExit(1)
    print("Clippy suppression check: pass")


def diff_hygiene_command(environment: dict[str, str] | None = None) -> list[str]:
    """Build a diff check for a local worktree or a CI event range."""
    env = os.environ if environment is None else environment
    base = env.get("HELL_WORKERS_DIFF_BASE", "").strip()
    if base and not re.fullmatch(r"0+", base):
        return ["git", "diff", "--check", f"{base}...HEAD"]
    return ["git", "diff", "HEAD", "--check"]


def verify() -> None:
    """Run the complete local/CI quality gate."""
    print("==> Python tooling", flush=True)
    run_python_script(
        "-m",
        "unittest",
        "discover",
        "-s",
        "scripts/tests",
        "-p",
        "test_*.py",
    )
    run_python_script(str(SCRIPTS_DIR / "perf.py"), "self-test")

    print("==> Repository contracts", flush=True)
    run_python_script(str(SCRIPTS_DIR / "check_agent_rules.py"))
    run_python_script(str(SCRIPTS_DIR / "check_help_impact.py"))
    run_python_script(str(SCRIPTS_DIR / "check_repo_hygiene.py"))
    run_docs(write=False)

    print("==> Rust quality gates", flush=True)
    run_command(["cargo", "fmt", "--all", "--check"])
    run_command(["cargo", "check", "--workspace", "--locked"])
    run_command(
        [
            "cargo",
            "check",
            "-p",
            "bevy_app@0.1.0",
            "--lib",
            "--no-default-features",
            "--features",
            "profiling",
            "--locked",
        ]
    )
    run_command(
        [
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]
    )
    check_clippy_suppressions()
    run_command(["cargo", "test", "--workspace", "--locked"])

    print("==> Diff hygiene", flush=True)
    run_command(diff_hygiene_command())
    print("All quality gates passed.")


def fast_check(package: str | None, *, run_tests: bool) -> None:
    """Run the fast repository gate, optionally followed by focused tests."""
    run_command(["cargo", "fmt", "--all", "--check"])
    check_clippy_suppressions()
    run_python_script(str(SCRIPTS_DIR / "check_agent_rules.py"))

    check_command = ["cargo", "check", "--locked"]
    if package:
        check_command.extend(["--package", package])
    else:
        check_command.append("--workspace")
    run_command(check_command)

    if run_tests:
        test_command = ["cargo", "test", "--locked"]
        if package:
            test_command.extend(["--package", package])
        else:
            test_command.append("--workspace")
        run_command(test_command)


def build(*, release: bool) -> None:
    """Build the workspace without implicit cleanup or output redirection."""
    command = ["cargo", "build", "--locked"]
    if release:
        command.append("--release")
    run_command(command)


def load_toml(path: Path) -> dict[str, object]:
    with path.open("rb") as source:
        return tomllib.load(source)


def doctor() -> int:
    """Report required and optional development dependencies without mutation."""
    errors: list[str] = []
    warnings: list[str] = []

    print(f"Repository: {REPO_ROOT}")
    print(f"Python: {platform.python_version()} ({sys.executable})")
    if sys.version_info < (3, 11):
        errors.append("Python 3.11 or newer is required (tomllib is used).")

    required_commands = ["git", "cargo", "rustc", "rg"]
    for command in required_commands:
        resolved = shutil.which(command)
        if resolved:
            print(f"required {command}: {resolved}")
        else:
            errors.append(f"required command is missing: {command}")

    toolchain = load_toml(REPO_ROOT / "rust-toolchain.toml")
    expected_channel = str(toolchain["toolchain"]["channel"])
    rust_host: str | None = None
    if shutil.which("rustc"):
        result = subprocess.run(
            ["rustc", "-vV"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        details = result.stdout.strip()
        actual = details.splitlines()[0] if details else ""
        rust_host = next(
            (
                line.removeprefix("host: ")
                for line in details.splitlines()
                if line.startswith("host: ")
            ),
            None,
        )
        print(f"Rust: {actual}")
        if result.returncode != 0 or expected_channel not in actual:
            errors.append(
                f"rustc does not match rust-toolchain.toml channel {expected_channel}"
            )

    if rust_host == "x86_64-unknown-linux-gnu":
        mold = shutil.which("mold")
        if mold:
            print(f"required mold: {mold}")
        else:
            errors.append("required command is missing: mold (configured Linux linker)")

    optional_commands = [
        "bacon",
        "cargo-deny",
        "cargo-expand",
        "docsrs-mcp",
        "rust-analyzer-mcp",
        "trunk",
        "gh",
    ]
    for command in optional_commands:
        resolved = shutil.which(command)
        status = resolved if resolved else "not installed"
        print(f"optional {command}: {status}")

    pillow = importlib.util.find_spec("PIL")
    print(f"optional Pillow: {'installed' if pillow else 'not installed'}")
    if pillow is None:
        warnings.append("Pillow is required only for image conversion scripts.")

    cargo_config = load_toml(REPO_ROOT / ".cargo" / "config.toml")
    target_dir = cargo_config.get("build", {}).get("target-dir")  # type: ignore[union-attr]
    if target_dir != "target":
        errors.append(".cargo/config.toml must keep build.target-dir at workspace target")
    else:
        print("Cargo target directory: workspace target/")

    assets = sum(1 for path in (REPO_ROOT / "assets").rglob("*") if path.is_file())
    print(f"Assets present: {assets} files")
    if assets == 0:
        warnings.append("No runtime assets are present; run the documented asset sync flow.")

    if errors:
        print("\nErrors:", file=sys.stderr)
        for message in errors:
            print(f"- {message}", file=sys.stderr)
    if warnings:
        print("\nWarnings:")
        for message in warnings:
            print(f"- {message}")

    if errors:
        return 1
    print("\nDevelopment environment: ready")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("doctor", help="diagnose the local environment")

    check_parser = subparsers.add_parser("check", help="run a fast compile check")
    check_parser.add_argument("--package", help="limit the check to one workspace package")
    check_parser.add_argument(
        "--tests",
        action="store_true",
        help="run matching tests after the compile check",
    )

    subparsers.add_parser("verify", help="run the complete local/CI quality gate")

    build_parser = subparsers.add_parser("build", help="build without implicit cleanup")
    build_parser.add_argument("--release", action="store_true", help="build release mode")

    cargo_parser = subparsers.add_parser(
        "cargo",
        help="run a Cargo subcommand through the persistent-storage guard",
    )
    cargo_parser.add_argument(
        "arguments",
        nargs=argparse.REMAINDER,
        help="arguments after `cargo --`, for example `-- test -p hw_world`",
    )

    docs_parser = subparsers.add_parser("docs", help="check or update docs indexes")
    docs_mode = docs_parser.add_mutually_exclusive_group(required=True)
    docs_mode.add_argument("--check", action="store_true", help="check without writing")
    docs_mode.add_argument("--write", action="store_true", help="update generated indexes")

    lane_parser = subparsers.add_parser(
        "lane",
        help="inspect or enter a session-fixed interactive Cargo build lane",
    )
    lane_subparsers = lane_parser.add_subparsers(dest="lane_command", required=True)
    lane_subparsers.add_parser("status", help="show whether lane a and b are free")
    shell_parser = lane_subparsers.add_parser(
        "shell",
        help="enter a shell that keeps one lane until the shell exits",
    )
    shell_parser.add_argument(
        "arguments",
        nargs=argparse.REMAINDER,
        help="optional command after `--`, for example `-- python3 scripts/dev.py check`",
    )

    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "doctor":
            return doctor()
        if args.command == "check":
            fast_check(args.package, run_tests=args.tests)
        elif args.command == "verify":
            verify()
        elif args.command == "build":
            build(release=args.release)
        elif args.command == "cargo":
            cargo_arguments = list(args.arguments)
            if cargo_arguments[:1] == ["--"]:
                cargo_arguments.pop(0)
            if not cargo_arguments:
                raise RuntimeError(
                    "cargo requires a Cargo subcommand, for example: cargo -- check --workspace"
                )
            run_command(["cargo", *cargo_arguments])
        elif args.command == "docs":
            run_docs(write=args.write)
        elif args.command == "lane":
            if args.lane_command == "status":
                for lane, state, path in lane_states(REPO_ROOT):
                    print(f"{lane}: {state} ({path})")
            else:
                command = list(args.arguments)
                if command[:1] == ["--"]:
                    command.pop(0)
                return run_lane_shell(REPO_ROOT, command or None)
    except subprocess.CalledProcessError as error:
        return error.returncode
    except FileNotFoundError as error:
        print(f"Required command not found: {error.filename}", file=sys.stderr)
        return 127
    except (RuntimeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("Interrupted.", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
