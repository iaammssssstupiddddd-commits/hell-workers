"""Fail-closed change selection with immutable GitHub revision provenance."""

from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
from pathlib import Path, PurePosixPath
from typing import Mapping

try:
    from check_agent_rules import ROOT_RULE_FILES
    from check_help_impact import is_production_path
except ModuleNotFoundError:
    from scripts.check_agent_rules import ROOT_RULE_FILES
    from scripts.check_help_impact import is_production_path

GROUP_IDS = ("contracts", "tooling", "rust", "deps")
SCHEMA_KEYS = {
    "schema_version", "event_name", "mode", "base_sha", "head_sha", "tested_sha",
    "help_base_sha", "diff_pairs", "groups", "reason_codes", "path_count",
    "paths_sha256", "run_id", "run_attempt",
}
CONTROL_MODULES = {
    "dev", "quality", "ci_scope", "ci_result", "cargo_runtime", "build_lane",
    "build_coordination", "validation_storage", "dev_tools", "install_dev_tools",
    "sync_agent_skills", "update_docs_index",
}
CONTROL_PATHS = {
    *ROOT_RULE_FILES, "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "deny.toml",
    "ruff.toml", ".gitignore", ".gitattributes", "scripts/dev-tools.toml",
    "docs/plans/plan-template.md", "docs/DEVELOPMENT.md",
    "docs/development-infra/validation-storage-workflow.md",
}
CONTROL_ROOTS = {
    ".github", ".cargo", ".agent", ".cursor", ".codex", ".gemini", ".claude-plugin",
}
RULE_NAMES = {"AGENTS.md", "CLAUDE.md", "GEMINI.md", "_rules.md"}
SHA = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})\Z")


def git(root: Path, *arguments: str) -> bytes:
    return subprocess.run(
        ["git", *arguments], cwd=root, check=True, capture_output=True,
    ).stdout


def sha_value(value: object) -> str:
    if not isinstance(value, str) or SHA.fullmatch(value) is None or set(value) == {"0"}:
        raise ValueError(f"expected a non-zero full commit SHA, got {value!r}")
    return value


def commit(root: Path, value: object) -> str:
    sha = sha_value(value)
    resolved = git(root, "rev-parse", "--verify", f"{sha}^{{commit}}").decode().strip()
    if resolved != sha:
        raise ValueError("revision must name the commit itself")
    return sha


def merge_base(root: Path, first: str, second: str) -> str:
    bases = git(root, "merge-base", "--all", first, second).decode().splitlines()
    if len(bases) != 1:
        raise ValueError("a unique merge base is required")
    return sha_value(bases[0])


def fetch_event_history(root: Path, environment: Mapping[str, str]) -> None:
    """Resolve event SHAs explicitly; never silently substitute a newer branch tip."""
    event = load_json(Path(environment["GITHUB_EVENT_PATH"]).read_text())
    revisions = [environment["GITHUB_SHA"]]
    name = environment["GITHUB_EVENT_NAME"]
    if name == "pull_request":
        revisions.extend(event["pull_request"][side]["sha"] for side in ("base", "head"))
    elif name == "push":
        revisions.extend(event[key] for key in ("before", "after"))
    elif name == "workflow_dispatch":
        inputs = event.get("inputs") or {}
        if inputs.get("base_sha"):
            revisions.append(inputs["base_sha"])
    for revision in revisions:
        sha = sha_value(revision)
        try:
            commit(root, sha)
        except subprocess.CalledProcessError:
            git(root, "fetch", "--no-tags", "origin", sha)
            commit(root, sha)


def normalize_path(path: str) -> str:
    parts = PurePosixPath(path).parts
    if (not parts or path.startswith("/") or ".." in parts or "\0" in path
            or PurePosixPath(path).as_posix() != path):
        raise ValueError(f"invalid repository-relative path: {path!r}")
    return path


def classify_path(path: str) -> tuple[set[str], str]:
    path = normalize_path(path)
    parts = PurePosixPath(path).parts
    name = parts[-1]
    all_groups = set(GROUP_IDS)
    if path in CONTROL_PATHS or parts[0] in CONTROL_ROOTS or name in RULE_NAMES:
        return all_groups, "control"
    if parts[0] == "crates" and name in {"Cargo.toml", "Cargo.lock", "build.rs"}:
        return all_groups, "build-control"
    if parts[0] == "scripts":
        module = name.removesuffix(".py")
        if len(parts) == 3 and parts[1] == "tests":
            module = module.removeprefix("test_")
        if name.endswith(".py") and (module in CONTROL_MODULES or module.startswith("check_")):
            return all_groups, "quality-control"
    if (parts[0] in {"assets", "settings"}
            or len(parts) >= 3 and parts[0] == "crates" and parts[2] == "assets"):
        return all_groups, "runtime-assets"
    if parts[0] == "crates" and (
        name.endswith((".rs", ".snap")) or "proptest-regressions" in parts
    ):
        return {"contracts", "tooling", "rust"}, "rust-with-tooling-contracts"
    if parts[0] == "docs" and name.endswith(".md") or name == "README.md":
        return {"contracts"}, "documentation"
    if (parts[0] == "scripts" and name.endswith(".py")
            or parts[:3] == ("scripts", "perf_tool", "contracts")
            or len(parts) >= 4 and parts[:2] == ("tools", "blender_ai_workflow")
            and parts[2] in {"scripts", "tests", "bin", "fixtures", "templates"}):
        return {"contracts", "tooling"}, "tooling"
    return all_groups, "unknown-path"


def classify(paths: list[str], *, full: bool = False) -> tuple[dict[str, bool], list[str]]:
    selected = {"contracts"}
    reasons = set()
    for path in paths:
        groups, reason = classify_path(path)
        selected.update(groups)
        reasons.add(reason)
        if is_production_path(path):
            reasons.add("help-review-required")
    if full:
        selected.update(GROUP_IDS)
        reasons.add("full-requested")
    if not paths:
        reasons.add("empty-tree-diff")
    return {name: name in selected for name in GROUP_IDS}, sorted(reasons)


def parse_changes(output: bytes) -> list[str]:
    if not output:
        return []
    if not output.endswith(b"\0"):
        raise ValueError("truncated Git change list")
    items = output[:-1].split(b"\0")
    if len(items) % 2:
        raise ValueError("malformed Git change list")
    paths = []
    for status, path in zip(items[::2], items[1::2], strict=True):
        if status not in {b"A", b"M", b"D", b"T"}:
            raise ValueError(f"unsupported Git change status: {status!r}")
        paths.append(normalize_path(os.fsdecode(path)))
    return paths


def changed_paths(root: Path, pairs: list[list[str]]) -> list[str]:
    paths = set()
    for first, second in pairs:
        paths.update(parse_changes(git(
            root, "diff", "--name-status", "--no-renames", "--no-ext-diff", "-z",
            first, second, "--",
        )))
    return sorted(paths)


def untracked_paths(root: Path) -> list[str]:
    return [normalize_path(os.fsdecode(value)) for value in git(
        root, "ls-files", "--others", "--exclude-standard", "-z",
    ).split(b"\0") if value]


def worktree_paths(root: Path) -> list[str]:
    if git(root, "ls-files", "--unmerged", "-z"):
        raise ValueError("resolve the unmerged index before verification")
    paths = set(untracked_paths(root))
    # Keep staged changes even when an unstaged edit restores the HEAD content.
    for arguments in (("--cached", "HEAD"), ()):
        paths.update(parse_changes(git(
            root, "diff", *arguments, "--name-status", "--no-renames", "--no-ext-diff", "-z", "--",
        )))
    return sorted(paths)


def path_digest(paths: list[str]) -> str:
    return hashlib.sha256(b"".join(os.fsencode(p) + b"\0" for p in sorted(set(paths)))).hexdigest()


def source_fingerprint(root: Path) -> str:
    """Include HEAD, index and actual source; ignore only untracked build output."""
    digest = hashlib.sha256(git(root, "rev-parse", "HEAD"))
    digest.update(git(root, "ls-files", "--stage", "-z"))
    paths = set(os.fsdecode(p) for p in git(root, "ls-files", "-z").split(b"\0") if p)
    paths.update(untracked_paths(root))
    for name in sorted(paths):
        path = root / normalize_path(name)
        digest.update(os.fsencode(name) + b"\0")
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            digest.update(b"deleted\0")
            continue
        digest.update(str(mode).encode() + b"\0")
        if stat.S_ISLNK(mode):
            digest.update(os.fsencode(os.readlink(path)))
        elif stat.S_ISREG(mode):
            with path.open("rb") as stream:
                while chunk := stream.read(1024 * 1024):
                    digest.update(chunk)
        else:
            raise ValueError(f"unsupported source file type: {name!r}")
        digest.update(b"\0")
    return digest.hexdigest()


def github_plan(root: Path, event: dict, environment: Mapping[str, str]) -> dict:
    event_name = environment.get("GITHUB_EVENT_NAME", "")
    tested = commit(root, environment.get("GITHUB_SHA"))
    if git(root, "rev-parse", "HEAD").decode().strip() != tested:
        raise ValueError("checkout HEAD differs from the event tested SHA")
    mode = "auto"
    force_full = False
    if event_name == "pull_request":
        pr = event["pull_request"]
        base, head = commit(root, pr["base"]["sha"]), commit(root, pr["head"]["sha"])
        parents = git(root, "rev-list", "--parents", "-n", "1", tested).decode().split()[1:]
        if parents != [base, head]:
            raise ValueError("PR merge parents differ from the event base/head")
        common = merge_base(root, base, head)
        pairs, help_base = [[common, head], [base, tested]], base
    elif event_name == "push":
        base, head = commit(root, event["before"]), commit(root, event["after"])
        if tested != head:
            raise ValueError("push checkout differs from after SHA")
        common = merge_base(root, base, head)
        pairs, help_base = [[base, head]], base
        if common != base:
            pairs.extend([[common, base], [common, head]])
            help_base, force_full = common, True
    elif event_name == "workflow_dispatch":
        inputs = event.get("inputs") or {}
        mode = inputs.get("mode", "auto")
        if mode not in {"auto", "full"}:
            raise ValueError("manual quality mode must be auto or full")
        base, head = commit(root, inputs.get("base_sha")), tested
        if base == tested or merge_base(root, base, tested) != base:
            raise ValueError("manual base must be a strict ancestor of tested SHA")
        pairs, help_base = [[base, tested]], base
    else:
        raise ValueError(f"unsupported quality event: {event_name!r}")
    pairs = [list(pair) for pair in sorted(set(map(tuple, pairs)))]
    paths = changed_paths(root, pairs)
    groups, reasons = classify(paths, full=force_full or mode == "full")
    if force_full:
        reasons = sorted({*reasons, "non-fast-forward"})
    plan = {
        "schema_version": 1, "event_name": event_name, "mode": mode,
        "base_sha": base, "head_sha": head, "tested_sha": tested,
        "help_base_sha": help_base, "diff_pairs": pairs, "groups": groups,
        "reason_codes": reasons, "path_count": len(paths), "paths_sha256": path_digest(paths),
        "run_id": environment.get("GITHUB_RUN_ID", ""),
        "run_attempt": environment.get("GITHUB_RUN_ATTEMPT", ""),
    }
    validate_plan(plan)
    return plan


def validate_plan(plan: object) -> dict:
    if not isinstance(plan, dict) or set(plan) != SCHEMA_KEYS:
        raise ValueError("invalid CI plan keys")
    if type(plan["schema_version"]) is not int or plan["schema_version"] != 1:
        raise ValueError("unsupported CI plan schema")
    if plan["event_name"] not in {"pull_request", "push", "workflow_dispatch"}:
        raise ValueError("invalid plan event")
    if plan["mode"] not in {"auto", "full"}:
        raise ValueError("invalid plan mode")
    for name in ("base_sha", "head_sha", "tested_sha", "help_base_sha"):
        sha_value(plan[name])
    groups = plan["groups"]
    if (not isinstance(groups, dict) or set(groups) != set(GROUP_IDS)
            or any(type(value) is not bool for value in groups.values())
            or not groups["contracts"] or groups["rust"] and not groups["tooling"]
            or plan["mode"] == "full" and not all(groups.values())):
        raise ValueError("invalid selected quality groups")
    pairs = plan["diff_pairs"]
    if not isinstance(pairs, list) or not pairs or len(pairs) > 3:
        raise ValueError("invalid plan diff pairs")
    for pair in pairs:
        if not isinstance(pair, list) or len(pair) != 2:
            raise ValueError("invalid diff pair")
        for revision in pair:
            sha_value(revision)
    reasons = plan["reason_codes"]
    if (not isinstance(reasons, list) or not reasons
            or any(not isinstance(reason, str) or re.fullmatch(r"[a-z-]+", reason) is None for reason in reasons)):
        raise ValueError("invalid plan reason codes")
    if type(plan["path_count"]) is not int or plan["path_count"] < 0:
        raise ValueError("invalid plan path count")
    if not isinstance(plan["paths_sha256"], str) or re.fullmatch(r"[0-9a-f]{64}", plan["paths_sha256"]) is None:
        raise ValueError("invalid path digest")
    for name in ("run_id", "run_attempt"):
        if not isinstance(plan[name], str) or re.fullmatch(r"[1-9][0-9]*", plan[name]) is None:
            raise ValueError(f"invalid {name}")
    return plan


def canonical_json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def plan_digest(plan: dict) -> str:
    validate_plan(plan)
    identity = {key: value for key, value in plan.items() if key != "run_attempt"}
    return hashlib.sha256(canonical_json(identity).encode()).hexdigest()


def load_json(text: str) -> dict:
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    value = json.loads(text, object_pairs_hook=unique_keys)
    if not isinstance(value, dict):
        raise ValueError("expected a JSON object")
    return value


def verify_github_plan(root: Path, plan: dict, environment: Mapping[str, str]) -> None:
    validate_plan(plan)
    event_path = environment.get("GITHUB_EVENT_PATH", "")
    if not event_path:
        raise ValueError("GITHUB_EVENT_PATH is required to verify a CI plan")
    event = load_json(Path(event_path).read_text(encoding="utf-8"))
    actual = github_plan(root, event, environment)
    if plan_digest(actual) != plan_digest(plan):
        raise ValueError("CI plan differs from the current event/checkout classification")


def write_outputs(path: str | None, values: dict[str, str]) -> None:
    if not path:
        return
    with Path(path).open("a", encoding="utf-8") as stream:
        for key, value in values.items():
            if re.fullmatch(r"[a-z_][a-z_0-9]*", key) is None or "\n" in value or "\r" in value:
                raise ValueError("unsafe GitHub output")
            stream.write(f"{key}={value}\n")


def diff_hygiene(root: Path, pairs: list[list[str]], *, local: bool = False) -> None:
    for first, second in pairs:
        git(root, "diff", "--check", first, second, "--")
    if local:
        git(root, "diff", "--check", "HEAD", "--")
        git(root, "diff", "--cached", "--check", "HEAD", "--")
        git(root, "diff", "--check", "--")
        for path in untracked_paths(root):
            absolute = root / path
            if not absolute.is_file() or absolute.is_symlink():
                continue
            result = subprocess.run(
                ["git", "diff", "--no-index", "--check", "--", os.devnull, str(absolute)],
                cwd=root, capture_output=True,
            )
            if result.returncode not in {0, 1} or result.stdout or result.stderr:
                raise ValueError(f"untracked diff hygiene failed: {path!r}: {os.fsdecode(result.stdout + result.stderr)}")
