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
ACTIVITY_LOCK_FD_ENV = "HELL_WORKERS_ACTIVITY_LOCK_FD"
ACTIVITY_LOCK_MODE_ENV = "HELL_WORKERS_ACTIVITY_LOCK_MODE"


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
    borrowed: bool = False

    def close(self) -> None:
        if self.fd < 0:
            return
        if self.borrowed:
            self.fd = -1
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
    inherited_fd_value = os.environ.get(ACTIVITY_LOCK_FD_ENV)
    inherited_mode = os.environ.get(ACTIVITY_LOCK_MODE_ENV)
    if inherited_fd_value is not None or inherited_mode is not None:
        if inherited_mode != "exclusive" or inherited_fd_value is None:
            raise RuntimeError("inherited Cargo activity lease metadata is invalid")
        try:
            inherited_fd = int(inherited_fd_value)
            inherited_stats = os.fstat(inherited_fd)
            lock_stats = lock_path.stat()
        except (OSError, ValueError) as error:
            raise RuntimeError("inherited Cargo activity lease fd is invalid") from error
        if (
            inherited_stats.st_dev != lock_stats.st_dev
            or inherited_stats.st_ino != lock_stats.st_ino
        ):
            raise RuntimeError(
                "inherited Cargo activity lease fd refers to a different lock"
            )
        probe_fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            assert fcntl is not None
            try:
                fcntl.flock(probe_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError as error:
                if not (
                    isinstance(error, BlockingIOError)
                    or error.errno in {errno.EACCES, errno.EAGAIN}
                ):
                    raise
            else:
                fcntl.flock(probe_fd, fcntl.LOCK_UN)
                raise RuntimeError("inherited Cargo activity lease is not locked")
        finally:
            os.close(probe_fd)
        return ActivityLease(
            mode="exclusive",
            fd=inherited_fd,
            lock_path=lock_path,
            borrowed=True,
        )
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


def activity_lease_environment(
    lease: ActivityLease, environment: dict[str, str] | None = None
) -> dict[str, str]:
    """Return an environment that can borrow an owned exclusive lease."""
    if lease.fd < 0 or lease.mode != "exclusive":
        raise RuntimeError("only an active exclusive Cargo lease can be inherited")
    inherited = dict(os.environ if environment is None else environment)
    inherited[ACTIVITY_LOCK_FD_ENV] = str(lease.fd)
    inherited[ACTIVITY_LOCK_MODE_ENV] = lease.mode
    return inherited


def activity_pass_fds(environment: dict[str, str]) -> tuple[int, ...]:
    """Resolve the exact lock descriptor a child process is allowed to inherit."""
    value = environment.get(ACTIVITY_LOCK_FD_ENV)
    mode = environment.get(ACTIVITY_LOCK_MODE_ENV)
    if value is None and mode is None:
        return ()
    if value is None or mode != "exclusive":
        raise RuntimeError("Cargo activity lease subprocess metadata is invalid")
    try:
        fd = int(value)
        os.fstat(fd)
    except (OSError, ValueError) as error:
        raise RuntimeError("Cargo activity lease subprocess fd is invalid") from error
    return (fd,)
