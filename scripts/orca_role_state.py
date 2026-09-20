"""Controller-owned role bindings, not Orca Task/Dispatch completion state.

Only an observed process exit with one intact provider session makes a launcher
checkpoint resumable. Unknown attempts are preserved for manual reconciliation.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import sqlite3
import stat
import uuid
from contextlib import closing
from pathlib import Path

try:
    import orca_frontdesk as storage
    from host_coordination import state_root
except ModuleNotFoundError:
    from scripts import orca_frontdesk as storage
    from scripts.host_coordination import state_root


def identity(value: str) -> str:
    if not isinstance(value, str) or str(uuid.UUID(value)) != value:
        raise ValueError("session must be a canonical UUID")
    return value


def digest(value: dict) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def state_path(slot: str) -> Path:
    if slot not in {"worker-a", "worker-b", "reviewer"}:
        raise ValueError("invalid role slot")
    return storage.checked_directory(state_root().parent / "role-state") / f"{slot}.json"


def read_state(slot: str, provider: str, *, allow_pending: bool = False) -> dict:
    data = storage.read_private_json(state_path(slot), {
        "schema": 1, "slot": slot, "provider": provider, "tasks": {}, "last": None,
    })
    if (not isinstance(data, dict) or data.get("schema") != 1 or data.get("slot") != slot
            or data.get("provider") != provider or not isinstance(data.get("tasks"), dict)):
        raise ValueError("invalid role state; preserve for reconciliation")
    last = data.get("last")
    abandoned = data.get("abandoned", {})
    if not isinstance(abandoned, dict):
        raise ValueError("invalid abandoned attempts")
    for key, attempt in abandoned.items():
        if (not isinstance(attempt, dict) or attempt.get("key") != key or key in data["tasks"]
                or attempt.get("phase") != "abandoned" or attempt.get("process_exited") is not True
                or type(attempt.get("exit_code")) is not int or attempt["exit_code"] == 0
                or not isinstance(attempt.get("reason"), str) or not attempt["reason"].strip()
                or not all(isinstance(attempt.get(name), str) and re.fullmatch(r"[a-f0-9]{64}", attempt[name])
                           for name in ("ticket_sha256", "observed_source_sha256"))):
            raise ValueError("invalid abandoned attempt evidence")
        identity(attempt["attempt_id"])
    if "last" not in data or (last is None and (data["tasks"] or abandoned)):
        raise ValueError("missing role attempt barrier; preserve for reconciliation")
    if last is not None:
        recorded = (isinstance(last, dict) and last.get("phase") == "recorded"
                    and last.get("process_exited") is True and type(last.get("exit_code")) is int
                    and last.get("key") in data["tasks"])
        reconciled = isinstance(last, dict) and last.get("phase") == "abandoned" and abandoned.get(last.get("key")) == last
        pending = allow_pending and isinstance(last, dict) and last.get("phase") in {"starting", "unknown"}
        if not (recorded or reconciled or pending):
            raise ValueError("unknown role attempt; reconcile before any new launch")
        identity(last["attempt_id"])
    for key, task in data["tasks"].items():
        if (not isinstance(task, dict) or task.get("key") != key
                or not isinstance(task.get("subject"), dict)
                or not all(isinstance(task.get(name), str) and re.fullmatch(r"[a-f0-9]{64}", task[name])
                           for name in ("ticket_sha256", "source_sha256", "session_sha256"))):
            raise ValueError("invalid role checkpoint")
        subject = task["subject"]
        if (set(subject) != {"repo", "common", "branch", "base"}
                or not all(isinstance(value, str) and value for value in subject.values())
                or not re.fullmatch(r"[a-f0-9]{40}", subject["base"])):
            raise ValueError("invalid role subject")
        for value in (subject["repo"], subject["common"], task.get("origin")):
            if not isinstance(value, str) or not Path(value).is_absolute() or str(Path(value).resolve()) != value:
                raise ValueError("invalid role origin/path")
        identity(task["session_id"])
    return data


def save_state(data: dict) -> None:
    storage.write_ledger(state_path(data["slot"]), data)


def claim_task(key: str, slot: str, provider: str, ticket: dict, subject: dict) -> None:
    """Caller holds the task lease, so two slots cannot replace ownership."""
    path = storage.checked_directory(state_path(slot).parent / "assignments") / f"{key}.json"
    expected = {"schema": 1, "slot": slot, "provider": provider,
                "ticket_sha256": digest(ticket), "subject": subject}
    current = storage.read_private_json(path, expected)
    if current != expected:
        raise ValueError("task is already bound to different ownership; do not reassign implicitly")
    storage.write_ledger(path, expected)


def safe_file(path: Path) -> None:
    info = path.lstat()
    if (path.resolve() != path or not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.getuid() or info.st_nlink != 1):
        raise ValueError("unsafe provider session file")


def session_snapshot(runtime: Path, provider: str, origin: Path) -> dict:
    """Read metadata only; do not load a provider, execute SQL writes or print transcripts."""
    if provider == "codex":
        files = list((runtime / "codex/sessions").glob("**/*.jsonl"))
    else:
        files = list((runtime / "cursor/chats").glob("*/*/store.db"))
    if len(files) != 1:
        raise ValueError("provider needs exactly one session; missing or ambiguous history")
    path = files[0]
    safe_file(path)
    auxiliary = []
    if provider == "codex":
        with path.open(encoding="utf-8") as handle:
            meta = json.loads(handle.readline(1024 * 1024))
        session_id = identity(meta.get("payload", {}).get("id"))
        if (meta.get("type") != "session_meta" or meta["payload"].get("cwd") != str(origin)
                or not path.name.endswith(f"-{session_id}.jsonl")):
            raise ValueError("Codex session identity/origin mismatch")
    else:
        session_id = identity(path.parent.name)
        # Cursor 2026.08.04: config/chats/md5(resolve(cwd))/UUID/store.db.
        workspace_key = hashlib.md5(str(origin).encode(), usedforsecurity=False).hexdigest()
        if path.parent.parent.name != workspace_key:
            raise ValueError("Cursor session workspace mismatch")
        for suffix in ("-wal", "-shm"):
            extra = Path(str(path) + suffix)
            if extra.exists() or extra.is_symlink():
                safe_file(extra)
                auxiliary.append(extra)
        try:
            # mode=ro reads committed WAL too; immutable=1 would silently ignore it.
            with closing(sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=0.1)) as connection:
                row = connection.execute("SELECT value FROM meta WHERE key = '0'").fetchone()
                if not row or not isinstance(row[0], str) or len(row[0]) > 2 * 1024 * 1024:
                    raise ValueError("invalid Cursor session metadata")
                meta = json.loads(bytes.fromhex(row[0]))
                if meta.get("agentId") != session_id or not meta.get("latestRootBlobId"):
                    raise ValueError("Cursor session identity or conversation missing")
        except sqlite3.Error as error:
            raise ValueError("unreadable Cursor session; preserve for reconciliation") from error
        # A read-only WAL connection may create sidecars in a writable directory.
        # Inventory again after close, so two snapshots hash the same content.
        auxiliary = []
        for suffix in ("-wal", "-shm"):
            extra = Path(str(path) + suffix)
            if extra.exists() or extra.is_symlink():
                safe_file(extra)
                auxiliary.append(extra)
    content = hashlib.sha256()
    # SHM is SQLite coordination, not conversation content; WAL is part of it.
    for file in (path, *(item for item in auxiliary if item.name.endswith("-wal"))):
        content.update(str(file.relative_to(runtime)).encode() + b"\0")
        with file.open("rb") as handle:
            for block in iter(lambda: handle.read(1024 * 1024), b""):
                content.update(block)
    return {"session_id": session_id, "session_sha256": content.hexdigest()}


def history_exists(runtime: Path) -> bool:
    return (any((runtime / "codex/sessions").glob("**/*.jsonl"))
            or any((runtime / "cursor/chats").glob("*/*/store.db")))


def admit(data: dict, ticket: dict, subject: dict, source: str,
          resume_session: str | None, follow_up: str | None) -> tuple[str, dict | None]:
    reviewer = data["slot"] == "reviewer"
    key = "fixed-reviewer" if reviewer else digest({"common": subject["common"], "id": ticket["id"]})
    if key in data.get("abandoned", {}):
        raise ValueError("abandoned ticket cannot be replayed; issue a new explicit task")
    previous = data["tasks"].get(key)
    if previous is None:
        if resume_session:
            raise ValueError("session does not exist in the controller binding; never adopt arbitrary history")
        if follow_up:
            raise ValueError("follow-up requires an existing worker session")
    else:
        if resume_session != previous["session_id"]:
            raise ValueError("explicit resume of the fixed session is required")
        if previous["subject"]["common"] != subject["common"]:
            raise ValueError("role session belongs to a different repository")
        if not reviewer:
            if previous["ticket_sha256"] != digest(ticket) or previous["subject"] != subject:
                raise ValueError("worker continuation must keep the exact same task and ownership")
            if previous["source_sha256"] != source:
                raise ValueError("source/index changed since worker exit; preserve changes and reconcile")
            if not follow_up or not follow_up.strip():
                raise ValueError("worker resume requires an explicit follow-up, never replay the original task")
    if reviewer and follow_up:
        raise ValueError("reviewer instructions belong in the current review ticket")
    return key, previous
