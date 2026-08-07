"""Fail-closed coordination between interactive Cargo and native recipes."""

from __future__ import annotations

import errno
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

try:
    import fcntl
except ImportError:  # pragma: no cover - exercised only on non-POSIX hosts
    fcntl = None  # type: ignore[assignment]

try:
    from cargo_runtime import persistent_storage_error, workspace_cargo_target
except ModuleNotFoundError:
    from scripts.cargo_runtime import persistent_storage_error, workspace_cargo_target


ActivityMode = Literal["shared", "exclusive"]
ACTIVITY_LOCK_NAME = ".cargo-activity.lock"


class ActivityBusyError(RuntimeError):
    """Raised when a recipe cannot acquire the required activity lock."""


def _require_flock() -> None:
    if fcntl is None or os.name != "posix":
        raise RuntimeError(
            "Cargo activity coordination requires a POSIX host with advisory flock support"
        )


def activity_lock_path(repo: Path) -> Path:
    """Return the persistent workspace-wide Cargo activity lock path."""
    target = workspace_cargo_target(repo)
    lock_path = target / ACTIVITY_LOCK_NAME
    storage_error = persistent_storage_error(lock_path, label="Cargo activity lock")
    if storage_error:
        raise RuntimeError(storage_error)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    return lock_path


@dataclass
class ActivityLease:
    """An open descriptor holding a shared or exclusive activity lease."""

    mode: ActivityMode
    fd: int
    lock_path: Path

    def close(self) -> None:
        if self.fd < 0:
            return
        try:
            if fcntl is not None:
                fcntl.flock(self.fd, fcntl.LOCK_UN)
        finally:
            os.close(self.fd)
            self.fd = -1

    def __enter__(self) -> "ActivityLease":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def acquire_activity(repo: Path, mode: ActivityMode) -> ActivityLease:
    """Acquire a non-blocking shared/exclusive workspace activity lease."""
    _require_flock()
    if mode not in {"shared", "exclusive"}:
        raise ValueError(f"invalid Cargo activity mode: {mode!r}")
    lock_path = activity_lock_path(repo)
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    flags = fcntl.LOCK_SH if mode == "shared" else fcntl.LOCK_EX
    try:
        assert fcntl is not None
        fcntl.flock(fd, flags | fcntl.LOCK_NB)
    except OSError as error:
        os.close(fd)
        if isinstance(error, BlockingIOError) or error.errno in {
            errno.EACCES,
            errno.EAGAIN,
        }:
            raise ActivityBusyError(
                f"Cargo activity lock is busy ({mode} requested): {lock_path}"
            ) from error
        raise
    return ActivityLease(mode, fd, lock_path)
