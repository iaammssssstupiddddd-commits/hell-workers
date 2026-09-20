"""Provider selection and conservative task admission for guarded Orca roles."""

from __future__ import annotations

import shutil
import json
import os
import stat
import tempfile
from pathlib import Path


PROVIDERS = {"worker-a": "codex", "worker-b": "cursor", "reviewer": "codex"}
SIMPLE_KINDS = {"local-fix", "mechanical-change", "test-addition"}


def provider_for(ticket: dict, slot: str) -> str:
    provider = PROVIDERS[slot]
    if ticket.get("provider", provider) != provider:
        raise ValueError(f"{slot} requires provider {provider}")
    if slot == "worker-b":
        kinds = SIMPLE_KINDS | ({"acceptance-probe"} if ticket.get("read_only") is True else set())
        if ticket.get("complexity") != "simple" or ticket.get("task_kind") not in kinds:
            raise ValueError("Cursor worker-b requires a simple, classified leaf task; route complex work to A")
        for key in ("complexity_reason", "acceptance"):
            if not isinstance(ticket.get(key), str) or not ticket[key].strip():
                raise ValueError(f"Cursor worker-b requires {key}")
        # Infrastructure, shared contracts, save and renderer work stay coordinator/A-owned.
        for scope in ticket.get("allowed_directories", []):
            parts = Path(scope).parts
            if (len(parts) < 4 or parts[:1] != ("crates",) or parts[2] != "src"
                    or parts[1] in {"hw_core", "hw_jobs", "hw_world", "hw_visual", "visual_test"}
                    or any(word in part.lower() for part in parts
                           for word in ("save", "load", "render", "asset", "startup", "plugin"))):
                raise ValueError(f"Cursor worker-b needs a narrow non-shared leaf scope: {scope}")
    return provider


def command_for(provider: str, repo: Path, role: str, prompt: str,
                resume_session: str | None = None, *, read_only: bool = False) -> list[str]:
    executable = shutil.which("cursor-agent" if provider == "cursor" else "codex")
    if not executable:
        raise RuntimeError(f"{provider} CLI is not installed")
    if provider == "cursor":
        if role != "worker":
            raise ValueError("Cursor is worker-b only")
        return [executable, "--workspace", str(repo), "--sandbox", "enabled", "--trust",
                *(["--mode", "ask"] if read_only else []),
                *(["--resume", resume_session] if resume_session else []), prompt]
    return [executable, *(["resume", resume_session] if resume_session else []),
            "--cd", str(repo), "--sandbox",
            "read-only" if role == "reviewer" or read_only else "workspace-write",
            "--ask-for-approval", "never", "--disable", "multi_agent", "--no-alt-screen", prompt]


def cursor_permissions(ticket: dict) -> dict:
    """Permissions are an additional fence, not a replacement for mount isolation."""
    # Cursor's global config schema requires both fields. An incomplete config
    # triggers repair; a read-only repair failure falls back to default permissions.
    read_only = ticket.get("read_only") is True
    return {"version": 1, "editor": {"vimMode": False}, "permissions": {
        "allow": ["Read(**)", *([] if read_only else
                   [f"Write({scope}/**)" for scope in ticket["allowed_directories"]])],
        "deny": ["Shell(*)", "Mcp(*:*)", "WebFetch(*)", *(["Write(**)"] if read_only else [])],
    }}


def write_cursor_policy(ticket: dict, path: Path, *, project: bool = False) -> None:
    if path.exists() or path.is_symlink():
        info = path.lstat()
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
                or info.st_mode & 0o077 or info.st_nlink != 1):
            raise RuntimeError("unsafe Cursor permission policy")
    fd, temporary = tempfile.mkstemp(prefix=".cursor-policy-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            config = cursor_permissions(ticket)
            json.dump({"permissions": config["permissions"]} if project else config, handle)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
