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
    import dev_tools
    import ci_scope
    import ci_result
    import quality
except ModuleNotFoundError:
    from scripts import ci_result, ci_scope, dev_tools, quality

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

try:
    from validation_storage import require_mutable
except ModuleNotFoundError:
    from scripts.validation_storage import require_mutable


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
    incremental: bool | None = None,
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
            require_mutable(REPO_ROOT)
            activity = acquire_activity(REPO_ROOT, "shared")
            require_cargo_memory()
        if command and Path(command[0]).name == "cargo":
            env = cargo_environment(
                REPO_ROOT,
                namespace=".dev-tmp",
                environment=env,
                incremental=incremental,
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
    print("==> Pinned quality tools", flush=True)
    dev_tools.preflight(REPO_ROOT, quality_environment())
    quality.run_groups(list(quality.GROUPS), quality_runner())
    print("All quality gates passed.")


def quality_runner(*, base: str | None = None, pairs: list[list[str]] | None = None,
                   local: bool = False) -> quality.Runner:
    def hygiene() -> None:
        if pairs is None:
            run_command(diff_hygiene_command())
            ci_scope.diff_hygiene(REPO_ROOT, [], local=True)
        else:
            ci_scope.diff_hygiene(REPO_ROOT, pairs, local=local)

    return quality.Runner(
        command=run_command, tools=run_quality_tools,
        suppressions=check_clippy_suppressions, hygiene=hygiene,
        environment={"HELL_WORKERS_DIFF_BASE": base} if base else {},
    )


def run_quality_group(group: str, plan_json: str | None) -> None:
    if plan_json is None:
        if os.environ.get("CI", "").lower() in {"1", "true", "yes"}:
            raise ValueError("CI quality groups require --plan-json")
        quality.run_group(group, quality_runner())
        return
    plan = ci_scope.load_json(plan_json)
    ci_scope.verify_github_plan(REPO_ROOT, plan, os.environ)
    if not plan["groups"][group]:
        raise ValueError(f"quality group was not selected: {group}")
    quality.run_group(group, quality_runner(base=plan["help_base_sha"], pairs=plan["diff_pairs"]))
    ci_scope.write_outputs(os.environ.get("GITHUB_OUTPUT"), {
        "tested_sha": plan["tested_sha"], "plan_sha256": ci_scope.plan_digest(plan),
    })


def run_ci(args: argparse.Namespace) -> None:
    if args.ci_command == "plan":
        event = ci_scope.load_json(Path(args.github_event).read_text(encoding="utf-8"))
        plan = ci_scope.github_plan(REPO_ROOT, event, os.environ)
        encoded = ci_scope.canonical_json(plan)
        ci_scope.write_outputs(args.github_output, {
            "plan_json": encoded, "tested_sha": plan["tested_sha"],
            "plan_sha256": ci_scope.plan_digest(plan),
            **{name: str(value).lower() for name, value in plan["groups"].items()},
        })
        print(encoded)
    elif args.ci_command == "result":
        plan = ci_scope.load_json(args.plan_json)
        ci_scope.verify_github_plan(REPO_ROOT, plan, os.environ)
        plan["run_attempt"] = os.environ["GITHUB_RUN_ATTEMPT"]
        needs = ci_scope.load_json(args.needs_json)
        failures = ci_result.evaluate(plan, needs)
        run_url = (
            f"{os.environ.get('GITHUB_SERVER_URL', 'https://github.com')}/"
            f"{os.environ.get('GITHUB_REPOSITORY', '')}/actions/runs/{plan['run_id']}"
        )
        report = ci_result.summary(plan, needs, failures, run_url)
        summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary_path:
            with Path(summary_path).open("a", encoding="utf-8") as stream:
                stream.write(report)
        print(report)
        if failures:
            raise ValueError("Required quality jobs did not pass")
    else:
        if os.environ.get("CI", "").lower() in {"1", "true", "yes"}:
            raise ValueError("ci check is a local worktree command; CI must use immutable plans")
        fingerprint = ci_scope.source_fingerprint(REPO_ROOT)
        head = ci_scope.commit(REPO_ROOT, ci_scope.git(REPO_ROOT, "rev-parse", "HEAD").decode().strip())
        base = ci_scope.merge_base(REPO_ROOT, ci_scope.commit(REPO_ROOT, args.base), head)
        pairs = [[base, head]]
        paths = sorted(set(ci_scope.changed_paths(REPO_ROOT, pairs) + ci_scope.worktree_paths(REPO_ROOT)))
        groups, reasons = ci_scope.classify(paths, full=args.mode == "full")
        selected = [name for name in quality.GROUPS if groups[name]]
        print(f"Selected quality groups: {', '.join(selected)} ({', '.join(reasons)})", flush=True)
        quality.run_groups(selected, quality_runner(base=base, pairs=pairs, local=True))
        if ci_scope.source_fingerprint(REPO_ROOT) != fingerprint:
            raise ValueError("source changed during verification; reclassify and verify the new worktree")
        print(f"Selected quality groups passed. HEAD={head} base={base} source={fingerprint}")


def quality_environment(*, create_temp_dir: bool = False) -> dict[str, str]:
    environment = cargo_environment(
        REPO_ROOT, namespace=".dev-tmp", incremental=None,
        create_temp_dir=create_temp_dir,
    )
    environment["RUSTUP_AUTO_INSTALL"] = "0"
    environment["PYTHONDONTWRITEBYTECODE"] = "1"
    return environment


def run_quality_tools(*, lint: bool = False, deps: bool = False, offline: bool = False) -> None:
    environment = quality_environment(create_temp_dir=deps)
    names = (["ruff", "actionlint"] if lint else []) + (["cargo-deny"] if deps else [])
    binaries = dev_tools.preflight(REPO_ROOT, environment, names)
    commands = dev_tools.lint_commands(REPO_ROOT, binaries) if lint else []
    if deps:
        if offline:
            print("Offline dependency diagnosis: cached data only; not an online audit.", flush=True)
        commands.append(dev_tools.deps_command(REPO_ROOT, binaries["cargo-deny"], offline=offline))
    # Audits can use CPU/registry resources, so keep them out of native/perf captures.
    activity = acquire_activity(REPO_ROOT, "shared") if deps else None
    try:
        for command in commands:
            print(f"+ {command_text(command)}", flush=True)
            subprocess.run(command, cwd=REPO_ROOT, env=environment, check=True)
    finally:
        if activity is not None:
            activity.close()


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


def feedback(*, build_only: bool, arguments: Sequence[str]) -> None:
    """Reuse dev artifacts for iteration, including the profiling-only visual probes."""
    game_arguments = list(arguments)
    if game_arguments[:1] == ["--"]:
        game_arguments.pop(0)
    if build_only and game_arguments:
        raise RuntimeError("feedback --build-only does not accept game arguments")
    print("Feedback only: dev profile, incremental=1; not formal acceptance or performance evidence.", flush=True)
    command = [
        "cargo", "build" if build_only else "run", "--locked",
        "-p", "bevy_app@0.1.0", "--bin", "bevy_app", "--profile", "dev",
        "--no-default-features", "--features", "profiling",
    ]
    if not build_only:
        command.extend(["--", *game_arguments])
    run_command(command, incremental=True)


def load_toml(path: Path) -> dict[str, object]:
    with path.open("rb") as source:
        return tomllib.load(source)


def doctor() -> int:
    """Report required and optional development dependencies without mutation."""
    errors: list[str] = []
    warnings: list[str] = []
    probe_environment = quality_environment()

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
            env=probe_environment,
            timeout=10,
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

    tool_errors: list[str] = []
    for name in dev_tools.TOOL_NAMES:
        try:
            dev_tools.preflight(REPO_ROOT, probe_environment, [name])
        except (RuntimeError, OSError, ValueError, subprocess.TimeoutExpired) as error:
            tool_errors.append(str(error))
    if tool_errors:
        warnings.extend(tool_errors)
        print("Full verification tools: not ready")
    else:
        print("Full verification tools: ready")

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
    validation_parser = subparsers.add_parser(
        "validation", help="manage validation retention through the primary coordinator"
    )
    validation_parser.add_argument("arguments", nargs=argparse.REMAINDER)

    check_parser = subparsers.add_parser("check", help="run a fast compile check")
    check_parser.add_argument("--package", help="limit the check to one workspace package")
    check_parser.add_argument(
        "--tests",
        action="store_true",
        help="run matching tests after the compile check",
    )

    subparsers.add_parser("verify", help="run the complete local/CI quality gate")
    quality_parser = subparsers.add_parser("quality", help="run one shared quality group")
    quality_parser.add_argument("--group", required=True, choices=quality.GROUPS)
    quality_parser.add_argument("--plan-json", help="immutable GitHub plan (required in CI)")
    ci_parser = subparsers.add_parser("ci", help="select and aggregate change-aware verification")
    ci_subparsers = ci_parser.add_subparsers(dest="ci_command", required=True)
    plan_parser = ci_subparsers.add_parser("plan", help="classify a checked-out GitHub event")
    plan_parser.add_argument("--github-event", required=True)
    plan_parser.add_argument("--github-output")
    result_parser = ci_subparsers.add_parser("result", help="verify all required job results")
    result_parser.add_argument("--plan-json", required=True)
    result_parser.add_argument("--needs-json", required=True)
    local_parser = ci_subparsers.add_parser("check", help="verify committed and dirty local changes")
    local_parser.add_argument("--base", required=True, help="full comparison commit SHA")
    local_parser.add_argument("--mode", choices=("auto", "full"), default="auto")
    subparsers.add_parser("lint", help="run pinned Ruff and actionlint checks")
    deps_parser = subparsers.add_parser("deps", help="audit workspace dependencies with cargo-deny")
    deps_parser.add_argument("--offline", action="store_true", help="diagnose using cached data only")

    build_parser = subparsers.add_parser("build", help="build without implicit cleanup")
    build_parser.add_argument("--release", action="store_true", help="build release mode")

    feedback_parser = subparsers.add_parser(
        "feedback", help="build/run the reusable dev binary for visual and interaction feedback"
    )
    feedback_parser.add_argument("--build-only", action="store_true", help="build without opening the game")
    feedback_parser.add_argument("arguments", nargs=argparse.REMAINDER, help="game arguments after --")

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
        if args.command == "validation":
            return subprocess.run(
                [sys.executable, str(SCRIPTS_DIR / "validation_storage.py"), *args.arguments],
                cwd=REPO_ROOT, check=False,
            ).returncode
        if args.command == "check":
            fast_check(args.package, run_tests=args.tests)
        elif args.command == "verify":
            verify()
        elif args.command == "quality":
            run_quality_group(args.group, args.plan_json)
        elif args.command == "ci":
            run_ci(args)
        elif args.command == "lint":
            run_quality_tools(lint=True)
        elif args.command == "deps":
            run_quality_tools(deps=True, offline=args.offline)
        elif args.command == "build":
            build(release=args.release)
        elif args.command == "feedback":
            feedback(build_only=args.build_only, arguments=args.arguments)
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
    except subprocess.TimeoutExpired as error:
        print(f"Tool version probe timed out: {error.cmd}", file=sys.stderr)
        return 1
    except FileNotFoundError as error:
        print(f"Required command not found: {error.filename}", file=sys.stderr)
        return 127
    except (RuntimeError, ValueError, KeyError, TypeError) as error:
        print(str(error), file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("Interrupted.", file=sys.stderr)
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
