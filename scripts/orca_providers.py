"""Provider selection and conservative task admission for guarded Orca roles."""

from __future__ import annotations

import shutil
import json
import os
import shlex
import stat
import sys
import tempfile
from pathlib import Path


PROVIDERS = {"worker-a": "codex", "worker-b": "cursor", "reviewer": "codex"}
SIMPLE_KINDS = {"local-fix", "mechanical-change", "test-addition"}
CURSOR_EDIT_ACCEPTANCE_SCOPE = "scripts/tests/fixtures/orca_edit_acceptance/worker-b"


def provider_for(ticket: dict, slot: str) -> str:
    provider = PROVIDERS[slot]
    if ticket.get("provider", provider) != provider:
        raise ValueError(f"{slot} requires provider {provider}")
    if slot == "worker-b":
        read_only = ticket.get("read_only") is True
        kinds = SIMPLE_KINDS | {"acceptance-edit"} | ({"acceptance-probe"} if read_only else set())
        if ticket.get("complexity") != "simple" or ticket.get("task_kind") not in kinds:
            raise ValueError("Cursor worker-b requires a simple, classified leaf task; route complex work to A")
        for key in ("complexity_reason", "acceptance"):
            if not isinstance(ticket.get(key), str) or not ticket[key].strip():
                raise ValueError(f"Cursor worker-b requires {key}")
        scopes = ticket.get("allowed_directories", [])
        if ticket.get("task_kind") == "acceptance-edit":
            if read_only or scopes != [CURSOR_EDIT_ACCEPTANCE_SCOPE]:
                raise ValueError("Cursor edit acceptance is restricted to its dedicated fixture")
        else:
            # Infrastructure, shared contracts, save and renderer work stay coordinator/A-owned.
            for scope in scopes:
                parts = Path(scope).parts
                if (len(parts) < 4 or parts[:1] != ("crates",) or parts[2] != "src"
                        or parts[1] in {"hw_core", "hw_jobs", "hw_world", "hw_visual", "visual_test"}
                        or any(word in part.lower() for part in parts
                               for word in ("save", "load", "render", "asset", "startup", "plugin"))):
                    raise ValueError(f"Cursor worker-b needs a narrow non-shared leaf scope: {scope}")
    return provider


def command_for(provider: str, repo: Path, role: str, prompt: str,
                resume_session: str | None = None, *, read_only: bool = False,
                externally_sandboxed: bool = False) -> list[str]:
    executable = shutil.which("cursor-agent" if provider == "cursor" else "codex")
    if not executable:
        raise RuntimeError(f"{provider} CLI is not installed")
    if provider == "cursor":
        if externally_sandboxed:
            raise ValueError("Cursor cannot bypass its own sandbox")
        if role != "worker":
            raise ValueError("Cursor is worker-b only")
        return [executable, "--workspace", str(repo), "--sandbox", "enabled", "--trust",
                *(["--mode", "ask"] if read_only else []),
                *(["--resume", resume_session] if resume_session else []), prompt]
    if externally_sandboxed and role != "reviewer" and not read_only:
        raise ValueError("Codex may rely on the outer sandbox only for read-only roles")
    isolation = (["--dangerously-bypass-approvals-and-sandbox"] if externally_sandboxed else
                 ["--sandbox", "read-only" if role == "reviewer" or read_only else "workspace-write",
                  "--ask-for-approval", "never"])
    return [executable, *(["resume", resume_session] if resume_session else []),
            "--cd", str(repo), *isolation,
            "--disable", "multi_agent", "--no-alt-screen", prompt]


def cursor_permissions(ticket: dict, *, denied_reads: tuple[str, ...] = ()) -> dict:
    """Permissions are an additional fence, not a replacement for mount isolation."""
    # Cursor's global config schema requires both fields. An incomplete config
    # triggers repair; a read-only repair failure falls back to default permissions.
    read_only = ticket.get("read_only") is True
    return {"version": 1, "editor": {"vimMode": False}, "permissions": {
        "allow": ["Read(**)", *([] if read_only else
                   [f"Write({scope}/**)" for scope in ticket["allowed_directories"]])],
        "deny": ["Shell(*)", "Mcp(*:*)", "WebFetch(*)",
                 *(f"Read({path}/**)" for path in denied_reads),
                 *(["Write(**)"] if read_only else [])],
    }}


def write_cursor_policy(ticket: dict, path: Path, *, project: bool = False,
                        denied_reads: tuple[str, ...] = ()) -> None:
    if path.exists() or path.is_symlink():
        info = path.lstat()
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                or info.st_mode & 0o077 or info.st_nlink != 1):
            raise RuntimeError("unsafe Cursor permission policy")
    fd, temporary = tempfile.mkstemp(prefix=".cursor-policy-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            config = cursor_permissions(ticket, denied_reads=denied_reads)
            json.dump({"permissions": config["permissions"]} if project else config, handle)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def cursor_hook_config(repo: Path) -> dict:
    """Controller-owned lifecycle hooks; they do not grant an agent tool."""
    command = f"{shlex.quote(sys.executable)} {shlex.quote(str(repo / 'scripts/orca_cursor_bridge_hook.py'))}"
    return {"version": 1, "hooks": {
        "beforeSubmitPrompt": [{"command": command, "timeout": 5, "failClosed": True}],
        "afterAgentResponse": [{"command": command, "timeout": 5, "failClosed": True}],
        "stop": [{"command": command, "timeout": 50, "failClosed": True}],
    }}


def write_cursor_hooks(repo: Path, path: Path) -> None:
    if path.exists() or path.is_symlink():
        raise RuntimeError("Cursor hook policy must be created once per bridge")
    fd, temporary = tempfile.mkstemp(prefix=".cursor-hooks-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(cursor_hook_config(repo), handle)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
