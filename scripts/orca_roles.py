"""Linux-only, opt-in Orca worker/reviewer launcher with mount write boundaries.

Workers edit existing, explicitly assigned directories; Git and authoritative
docs stay read-only. The coordinator builds, reviews evidence and commits.
No model/effort choice, publishing, agent nesting or automatic task dispatch.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from contextlib import ExitStack
from pathlib import Path

try:
    from host_coordination import acquire_host, state_root
except ModuleNotFoundError:
    from scripts.host_coordination import acquire_host, state_root


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def load_ticket(path: Path) -> dict:
    ticket = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(ticket, dict) or ticket.get("schema") != 1:
        raise ValueError("ticket schema must be 1")
    for key in ("id", "branch", "base", "prompt"):
        if not isinstance(ticket.get(key), str) or not ticket[key].strip():
            raise ValueError(f"ticket requires {key}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", ticket["id"]):
        raise ValueError("invalid ticket id")
    repo = Path(ticket["repo"])
    if not repo.is_absolute() or repo.resolve() != repo:
        raise ValueError("ticket repo must be a canonical absolute path")
    if git(repo, "rev-parse", "--show-toplevel") != str(repo):
        raise ValueError("ticket must name the checkout root")
    common = Path(git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir"))
    if common.parent == repo:
        raise ValueError("agents must use a linked worktree, never the primary checkout")
    if git(repo, "branch", "--show-current") != ticket["branch"]:
        raise ValueError("ticket branch differs from checkout")
    if not re.fullmatch(r"[a-f0-9]{40}", ticket["base"]):
        raise ValueError("ticket base must be a full SHA")
    if git(repo, "rev-parse", "HEAD") != ticket["base"]:
        raise ValueError("ticket base changed; reissue the ticket and review")
    allowed = ticket.get("allowed_directories", [])
    if not isinstance(allowed, list):
        raise ValueError("allowed_directories must be a list")
    paths: list[Path] = []
    for value in allowed:
        if not isinstance(value, str):
            raise ValueError("allowed directory must be a string")
        relative = Path(value)
        if (relative.is_absolute() or not relative.parts or
                any(part in {"..", ".git", ".codex", ".agents"} for part in relative.parts)
                or relative.parts[0] in {"docs", "target", ".cursor", ".github"}):
            raise ValueError(f"protected/non-relative allowed directory: {value}")
        candidate = repo / relative
        if candidate.resolve() != candidate or not candidate.is_dir():
            raise ValueError(f"allow only existing non-symlink directories: {value}")
        for child in candidate.rglob("*"):
            if child.is_symlink() or (child.is_file() and child.stat().st_nlink != 1):
                raise ValueError(f"symlink/hardlink in writable scope: {child}")
        if any(candidate.is_relative_to(other) or other.is_relative_to(candidate) for other in paths):
            raise ValueError("overlapping allowed directories")
        paths.append(candidate)
    return ticket


def fingerprint(repo: Path) -> str:
    """Bind review to HEAD and exact tracked/untracked non-ignored source bytes."""
    digest = hashlib.sha256(git(repo, "rev-parse", "HEAD").encode())
    names = subprocess.check_output([
        "git", "-C", str(repo), "ls-files", "-z", "--cached", "--others", "--exclude-standard"
    ]).split(b"\0")
    for name in sorted(set(names) - {b""}):
        path = repo / os.fsdecode(name)
        digest.update(name + b"\0")
        if path.is_symlink():
            digest.update(b"link\0" + os.fsencode(os.readlink(path)))
        elif path.is_file():
            digest.update(str(path.stat().st_mode & 0o777).encode() + b"\0")
            with path.open("rb") as handle:
                for block in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(block)
        else:
            digest.update(b"deleted")
    digest.update(subprocess.check_output(["git", "-C", str(repo), "diff", "--cached", "--binary"]))
    return digest.hexdigest()


def sandbox_command(ticket: dict, role: str, runtime: Path, command: list[str]) -> list[str]:
    """Outer mount namespace constrains every process, including MCPs and hooks."""
    bwrap = shutil.which("bwrap")
    if not bwrap or sys.platform != "linux":
        raise RuntimeError("role isolation requires Linux bubblewrap; no unsafe fallback")
    repo = Path(ticket["repo"])
    result = [bwrap, "--die-with-parent", "--new-session", "--unshare-pid",
              "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev",
              "--tmpfs", "/tmp", "--tmpfs", "/run", "--clearenv"]
    resolv = Path("/etc/resolv.conf").resolve(strict=True)
    # systemd-resolved often stores this under /run, which we hide above.
    result.extend(["--ro-bind", str(resolv), str(resolv)])
    for key in ("PATH", "HOME", "USER", "LOGNAME", "TERM", "LANG", "COLORTERM"):
        if key in os.environ:
            result.extend(["--setenv", key, os.environ[key]])
    result.extend(["--bind", str(runtime), str(runtime),
                   "--setenv", "CODEX_HOME", str(runtime / "codex"),
                   "--setenv", "TMPDIR", str(runtime / "tmp"),
                   "--setenv", "PYTHONDONTWRITEBYTECODE", "1"])
    # Account credentials are mounted read-only, never copied or printed.
    auth = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "auth.json"
    if auth.is_file():
        result.extend(["--ro-bind", str(auth), str(runtime / "codex/auth.json")])
    # Hide inherited config/MCP transports; the agent gets a clean Codex home.
    for path in (Path.home() / ".codex", Path.home() / ".orca",
                 Path.home() / ".config/orca", repo / ".codex"):
        if path.is_dir() and not runtime.is_relative_to(path):
            result.extend(["--tmpfs", str(path)])
    if role == "worker":
        for relative in ticket["allowed_directories"]:
            path = repo / relative
            result.extend(["--bind", str(path), str(path)])
    result.extend(["--chdir", str(repo), "--", *command])
    return result


def prepare_runtime(slot: str) -> Path:
    runtime = state_root().parent / "agents" / slot
    if runtime.resolve() != runtime:
        raise RuntimeError("agent runtime must not contain symlinks")
    for path in (runtime, runtime / "codex", runtime / "tmp"):
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        if path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o077:
            raise RuntimeError(f"unsafe agent runtime permissions: {path}")
    return runtime


def verify_review(ticket: dict, record: dict) -> None:
    repo = Path(ticket["repo"])
    expected = {"ticket": ticket["id"], "base": ticket["base"],
                "head": git(repo, "rev-parse", "HEAD"),
                "source_sha256": fingerprint(repo), "verdict": "approved"}
    if any(record.get(key) != value for key, value in expected.items()):
        raise ValueError("review is stale or not approved for this exact subject")
    if not record.get("reviewer_session") or not record.get("validation_evidence"):
        raise ValueError("review requires fixed reviewer session and same-subject validation evidence")
    if record.get("blocking_findings") != []:
        raise ValueError("review has unresolved or unrecorded blocking findings")


def launch(ticket: dict, slot: str, *, dry_run: bool, resume_session: str | None = None) -> int:
    role = "reviewer" if slot == "reviewer" else "worker"
    if resume_session and (role != "reviewer" or not re.fullmatch(r"[a-f0-9-]{36}", resume_session)):
        raise ValueError("resume requires a reviewer session UUID")
    if role == "worker" and not ticket.get("allowed_directories"):
        raise ValueError("worker needs a nonempty writable directory scope")
    repo = Path(ticket["repo"])
    if role == "worker" and git(repo, "status", "--porcelain"):
        raise ValueError("worker launch requires a clean checkout; preserve existing changes")
    prompt = (
        f"Role: {role}. Ticket: {ticket['id']}. Read AGENTS.md. No subagents, no commit, "
        "no push, no changes outside assigned directories. Do not start builds/tests/analysis "
        "servers; ask the coordinator for validation. Report findings and stop. "
        f"Allowed directories: {ticket['allowed_directories'] if role == 'worker' else []}.\n"
        + ticket["prompt"]
    )
    codex = shutil.which("codex")
    if not codex:
        raise RuntimeError("codex is not installed")
    command = [codex, *(["resume", resume_session] if resume_session else []),
               "--cd", str(repo), "--sandbox", "read-only" if role == "reviewer" else "workspace-write",
               "--ask-for-approval", "never", "--disable", "multi_agent", "--no-alt-screen", prompt]
    if dry_run:
        print(json.dumps({"slot": slot, "role": role, "repo": str(repo),
                          "allowed_directories": ticket["allowed_directories"] if role == "worker" else [],
                          "command": command}, ensure_ascii=False, indent=2))
        return 0
    workspace_slot = "workspace-" + hashlib.sha256(str(repo).encode()).hexdigest()
    with ExitStack() as leases:
        leases.enter_context(acquire_host(slot, inherit=False))
        leases.enter_context(acquire_host(workspace_slot, inherit=False))
        runtime = prepare_runtime(slot)
        if role == "reviewer" and not resume_session and any((runtime / "codex/sessions").glob("**/*.jsonl")):
            raise ValueError("reviewer history exists; reuse the terminal or pass --resume-session")
        before = fingerprint(repo)
        if role == "reviewer" and ticket.get("source_sha256") != before:
            raise ValueError("review ticket needs the current source_sha256; regenerate after changes")
        result = subprocess.run(sandbox_command(ticket, role, runtime, command), check=False)
        if role == "reviewer" and fingerprint(repo) != before:
            raise RuntimeError("source changed during review; review is invalid")
        print(json.dumps({"ticket": ticket["id"], "role": role, "exit_code": result.returncode,
                          "source_sha256": fingerprint(repo), "approved": False}))
        return result.returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("launch", "fingerprint", "verify-review"))
    parser.add_argument("--ticket", type=Path, required=True)
    parser.add_argument("--slot", choices=("worker-a", "worker-b", "reviewer"), default="reviewer")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--resume-session")
    parser.add_argument("--review-record", type=Path)
    args = parser.parse_args()
    try:
        ticket = load_ticket(args.ticket)
        if args.action == "fingerprint":
            print(fingerprint(Path(ticket["repo"])))
            return 0
        if args.action == "verify-review":
            if args.review_record is None:
                raise ValueError("--review-record is required")
            verify_review(ticket, json.loads(args.review_record.read_text()))
            print("Review matches the exact source; this command does not integrate or publish.")
            return 0
        return launch(ticket, args.slot, dry_run=args.dry_run, resume_session=args.resume_session)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Orca role refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
