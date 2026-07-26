#!/usr/bin/env python3
"""Require a current Help decision for player-facing production changes."""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


REPO_ROOT = Path(__file__).resolve().parent.parent
EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
RUNTIME_DATA_EXTENSIONS = {".ron", ".json", ".yaml", ".yml", ".toml", ".ftl"}
HELP_UPDATE_PREFIXES = ("crates/bevy_app/src/interface/ui/help_content/",)
HELP_RENDER_SOURCE_PATHS = frozenset({"crates/hw_ui/src/help.rs"})
HELP_APPROVAL_SNAPSHOT = (
    "crates/bevy_app/src/interface/ui/help_content/coverage_approval.snap"
)


class HelpImpactError(RuntimeError):
    pass


@dataclass(frozen=True)
class CommitChange:
    revision: str
    paths: tuple[str, ...]
    help_impact_reason: str | None = None

    @property
    def production_paths(self) -> tuple[str, ...]:
        return tuple(path for path in self.paths if is_production_path(path))

    @property
    def has_production_change(self) -> bool:
        return bool(self.production_paths)

    @property
    def has_help_update(self) -> bool:
        return has_help_update(self.paths)

    @property
    def has_decision(self) -> bool:
        return self.has_help_update or self.help_impact_reason is not None


@dataclass(frozen=True)
class ImpactDecision:
    passed: bool
    message: str


def _is_test_only_rust_path(parts: tuple[str, ...]) -> bool:
    relative = parts[3:]
    if "tests" in relative or "test_support" in relative:
        return True
    filename = relative[-1]
    return (
        filename == "tests.rs"
        or filename.endswith("_tests.rs")
        or filename == "test_support.rs"
    )


def is_production_path(path: str) -> bool:
    normalized = PurePosixPath(path)
    parts = normalized.parts
    if not parts:
        return False

    if path in {"Cargo.toml", "Cargo.lock"}:
        return True
    if (
        len(parts) == 3
        and parts[0] == "crates"
        and parts[2] in {"Cargo.toml", "build.rs"}
    ):
        return True
    if (
        len(parts) >= 4
        and parts[0] == "crates"
        and parts[2] == "src"
        and normalized.suffix == ".rs"
    ):
        return not _is_test_only_rust_path(parts)

    in_runtime_data_root = (
        parts[0] in {"assets", "settings"}
        or (len(parts) >= 3 and parts[0] == "crates" and parts[2] == "assets")
    )
    return in_runtime_data_root and normalized.suffix.lower() in RUNTIME_DATA_EXTENSIONS


def is_help_update_path(path: str) -> bool:
    return is_production_path(path) and (
        path in HELP_RENDER_SOURCE_PATHS
        or any(path.startswith(prefix) for prefix in HELP_UPDATE_PREFIXES)
    )


def has_help_update(paths: Sequence[str]) -> bool:
    """Require both a runtime Help source and its exact approval snapshot."""

    return HELP_APPROVAL_SNAPSHOT in paths and any(
        is_help_update_path(path) for path in paths
    )


def parse_no_impact_reason(message: str) -> str | None:
    """Parse one exact no-impact decision from the final Git trailer block."""

    parsed = subprocess.run(
        ["git", "interpret-trailers", "--parse"],
        input=message,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    trailers: dict[str, list[str]] = {}
    for line in parsed.splitlines():
        key, separator, value = line.partition(":")
        if not separator:
            continue
        trailers.setdefault(key.strip(), []).append(value.strip())

    impacts = trailers.get("Help-Impact", [])
    reasons = trailers.get("Help-Impact-Reason", [])
    if impacts != ["none"] or len(reasons) != 1 or not reasons[0]:
        return None
    return reasons[0]


def _linear_ancestor_predicate(
    commits: Sequence[CommitChange],
) -> Callable[[str, str], bool]:
    positions = {commit.revision: index for index, commit in enumerate(commits)}
    return lambda ancestor, descendant: positions[ancestor] <= positions[descendant]


def evaluate_batch(
    commits: Sequence[CommitChange],
    worktree_paths: Sequence[str],
    *,
    local_reason: str = "",
    ci: bool = False,
    is_ancestor: Callable[[str, str], bool] | None = None,
) -> ImpactDecision:
    production_commits = [commit for commit in commits if commit.has_production_change]
    worktree_production = tuple(
        path for path in worktree_paths if is_production_path(path)
    )
    worktree_help = has_help_update(worktree_paths)

    if not production_commits and not worktree_production:
        return ImpactDecision(True, "Help impact: no production changes")

    # A dirty Help catalog is the newest decision in the local batch and covers
    # both committed and worktree production changes.
    if worktree_help:
        return ImpactDecision(
            True,
            "Help impact: current production batch includes a Help catalog update",
        )

    reason = local_reason.strip()
    if worktree_production:
        if reason and not ci:
            return ImpactDecision(
                True,
                f"Help impact: local no-impact override accepted ({reason})",
            )
        suffix = (
            "CI does not accept HELL_WORKERS_HELP_IMPACT_REASON."
            if ci and reason
            else (
                "Update the Help catalog and exact approval snapshot, "
                "or record a local no-impact reason."
            )
        )
        return ImpactDecision(
            False,
            _failure_message(worktree_production, suffix),
        )

    ancestor = is_ancestor or _linear_ancestor_predicate(commits)
    for candidate in (commit for commit in commits if commit.has_decision):
        if all(
            ancestor(production.revision, candidate.revision)
            for production in production_commits
        ):
            if candidate.has_help_update:
                return ImpactDecision(
                    True,
                    "Help impact: committed production batch includes a Help catalog update",
                )
            return ImpactDecision(
                True,
                "Help impact: no-impact decision accepted "
                f"({candidate.help_impact_reason})",
            )

    uncovered_paths = tuple(
        path for commit in production_commits for path in commit.production_paths
    )
    return ImpactDecision(
        False,
        _failure_message(
            uncovered_paths,
            (
                "Update the Help catalog and exact approval snapshot, "
                "or add exact Help-Impact trailers."
            ),
        ),
    )


def _failure_message(paths: Sequence[str], suffix: str) -> str:
    unique_paths = sorted(set(paths))
    shown = ", ".join(unique_paths[:8])
    if len(unique_paths) > 8:
        shown += f", ... (+{len(unique_paths) - 8})"
    return (
        "Help impact: production changes are newer than the latest Help decision"
        f" [{shown}]. {suffix}"
    )


def git(
    arguments: Sequence[str],
    *,
    root: Path = REPO_ROOT,
    check: bool = True,
) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=root,
        check=check,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def git_nul_paths(
    arguments: Sequence[str],
    *,
    root: Path = REPO_ROOT,
    environment: Mapping[str, str] | None = None,
) -> tuple[str, ...]:
    result = subprocess.run(
        ["git", *arguments],
        cwd=root,
        check=True,
        capture_output=True,
        env=environment,
    )
    return tuple(
        value.decode("utf-8", errors="surrogateescape")
        for value in result.stdout.split(b"\0")
        if value
    )


def revision_exists(revision: str, *, root: Path = REPO_ROOT) -> bool:
    return (
        subprocess.run(
            ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        ).returncode
        == 0
    )


def merge_base(left: str, right: str, *, root: Path = REPO_ROOT) -> str | None:
    result = subprocess.run(
        ["git", "merge-base", left, right],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    value = result.stdout.strip()
    return value if result.returncode == 0 and value else None


def resolve_diff_base(
    environment: Mapping[str, str] | None = None,
    *,
    root: Path = REPO_ROOT,
) -> str:
    env = os.environ if environment is None else environment
    explicit = env.get("HELL_WORKERS_DIFF_BASE", "").strip()
    ci = env.get("CI", "").strip().lower() in {"1", "true", "yes"}

    if explicit:
        if explicit.strip("0") == "":
            if ci:
                raise HelpImpactError("CI requires a non-zero HELL_WORKERS_DIFF_BASE")
        else:
            if not revision_exists(explicit, root=root):
                raise HelpImpactError(f"diff base cannot be resolved: {explicit}")
            resolved = merge_base("HEAD", explicit, root=root)
            if resolved is None:
                raise HelpImpactError(
                    f"diff base has no merge-base with HEAD: {explicit}"
                )
            return resolved

    if ci:
        raise HelpImpactError("CI requires HELL_WORKERS_DIFF_BASE")

    resolved = merge_base("HEAD", "origin/master", root=root)
    if resolved is not None:
        return resolved

    if not revision_exists("HEAD", root=root):
        raise HelpImpactError("HEAD cannot be resolved")
    if commit_parents("HEAD", root=root):
        raise HelpImpactError(
            "local diff base cannot be resolved; fetch origin/master or set "
            "HELL_WORKERS_DIFF_BASE"
        )
    return EMPTY_TREE


def commit_parents(revision: str, *, root: Path = REPO_ROOT) -> tuple[str, ...]:
    return tuple(git(["show", "-s", "--format=%P", revision], root=root).split())


def _merge_specific_paths(
    revision: str,
    parents: Sequence[str],
    *,
    root: Path = REPO_ROOT,
) -> tuple[str, ...]:
    """Return only edits added by a two-parent merge itself.

    Git's canonical automatic merge tree already contains changes transported
    from both parents. Comparing the recorded merge tree against that temporary
    tree avoids reclassifying parent changes while retaining manual conflict
    resolution or `git merge --no-commit` edits.
    """

    if len(parents) != 2:
        raise HelpImpactError(
            f"octopus merge is unsupported by the Help impact gate: {revision}"
        )

    objects_path = Path(git(["rev-parse", "--git-path", "objects"], root=root))
    if not objects_path.is_absolute():
        objects_path = root / objects_path
    objects_path = objects_path.resolve()

    with tempfile.TemporaryDirectory(prefix="help-impact-objects-") as temp_dir:
        temporary_objects = Path(temp_dir) / "objects"
        temporary_objects.mkdir()
        environment = os.environ.copy()
        environment["GIT_OBJECT_DIRECTORY"] = str(temporary_objects)
        environment["GIT_ALTERNATE_OBJECT_DIRECTORIES"] = str(objects_path)
        result = subprocess.run(
            ["git", "merge-tree", "--write-tree", parents[0], parents[1]],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )
        first_line = result.stdout.splitlines()[0] if result.stdout else ""
        if result.returncode not in {0, 1} or re.fullmatch(
            r"[0-9a-fA-F]{40,64}", first_line
        ) is None:
            details = result.stderr.strip() or result.stdout.strip()
            raise HelpImpactError(
                f"cannot reconstruct automatic merge tree for {revision}: {details}"
            )
        return git_nul_paths(
            [
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                first_line,
                f"{revision}^{{tree}}",
            ],
            root=root,
            environment=environment,
        )


def changed_paths_for_commit(revision: str, *, root: Path = REPO_ROOT) -> tuple[str, ...]:
    parents = commit_parents(revision, root=root)
    if len(parents) > 1:
        return tuple(sorted(set(_merge_specific_paths(revision, parents, root=root))))

    return tuple(
        sorted(
            set(
                git_nul_paths(
                    [
                        "diff-tree",
                        "--root",
                        "--no-commit-id",
                        "--name-only",
                        "--no-renames",
                        "-r",
                        "-z",
                        revision,
                    ],
                    root=root,
                )
            )
        )
    )


def collect_commits(base: str, *, root: Path = REPO_ROOT) -> list[CommitChange]:
    revision_range = "HEAD" if base == EMPTY_TREE else f"{base}..HEAD"
    output = git(["rev-list", "--topo-order", "--reverse", revision_range], root=root)
    commits = []
    for revision in output.splitlines():
        message = git(["show", "-s", "--format=%B", revision], root=root)
        commits.append(
            CommitChange(
                revision=revision,
                paths=changed_paths_for_commit(revision, root=root),
                help_impact_reason=parse_no_impact_reason(message),
            )
        )
    return commits


def collect_worktree_paths(*, root: Path = REPO_ROOT) -> tuple[str, ...]:
    paths = set(
        git_nul_paths(["diff", "--name-only", "--no-renames", "-z"], root=root)
    )
    paths.update(
        git_nul_paths(
            ["diff", "--cached", "--name-only", "--no-renames", "-z"],
            root=root,
        )
    )
    paths.update(
        git_nul_paths(
            ["ls-files", "--others", "--exclude-standard", "-z"],
            root=root,
        )
    )
    return tuple(sorted(paths))


def git_is_ancestor(ancestor: str, descendant: str, *, root: Path = REPO_ROOT) -> bool:
    return (
        subprocess.run(
            ["git", "merge-base", "--is-ancestor", ancestor, descendant],
            cwd=root,
            check=False,
            capture_output=True,
        ).returncode
        == 0
    )


def main() -> int:
    env = os.environ
    ci = env.get("CI", "").strip().lower() in {"1", "true", "yes"}
    try:
        base = resolve_diff_base(env)
        decision = evaluate_batch(
            collect_commits(base),
            collect_worktree_paths(),
            local_reason=env.get("HELL_WORKERS_HELP_IMPACT_REASON", ""),
            ci=ci,
            is_ancestor=lambda ancestor, descendant: git_is_ancestor(
                ancestor, descendant
            ),
        )
    except (HelpImpactError, subprocess.CalledProcessError, OSError) as error:
        print(f"Help impact gate failed: {error}", file=sys.stderr)
        return 1

    stream = sys.stdout if decision.passed else sys.stderr
    print(decision.message, file=stream)
    print(f"Help impact diff base: {base}", file=stream)
    return 0 if decision.passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
