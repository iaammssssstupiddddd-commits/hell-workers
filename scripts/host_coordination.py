"""One cooperative heavy-work slot per OS account, across clones and worktrees.

The fixed account-owned path deliberately ignores HOME/XDG overrides. Locks are
descriptor-owned (never unlink lock files); child inheritance extends their life.
This is admission control for project entrypoints, not a security sandbox.
"""

from __future__ import annotations

import errno
import os
import re
import stat
from dataclasses import dataclass
from pathlib import Path

try:
    import fcntl
except ImportError:
    fcntl = None  # type: ignore[assignment]

try:
    from cargo_runtime import account_home, persistent_storage_error
except ModuleNotFoundError:
    from scripts.cargo_runtime import account_home, persistent_storage_error


HOST_FD_ENV = "HELL_WORKERS_HOST_LOCK_FD"


class HostBusyError(RuntimeError):
    """A different process tree owns the host's heavy-work slot."""


def state_root() -> Path:
    return account_home() / ".local/state/hell-workers/coordination"


def lock_path(name: str = "heavy") -> Path:
    if (name not in {"heavy", "reviewer", "worker-a", "worker-b", "coordinator", "frontdesk-state",
                     "frontdesk-ui", "linear-intake-state"}
            and not re.fullmatch(r"(?:workspace|dispatch)-[a-f0-9]{64}", name)):
        raise ValueError(f"unknown host slot: {name}")
    root = state_root()
    if root.resolve() != root:
        raise RuntimeError(f"coordination directory must not contain symlinks: {root}")
    error = persistent_storage_error(root, label="host coordination")
    if error:
        raise RuntimeError(error)
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    info = root.stat()
    if info.st_uid != os.getuid() or info.st_mode & 0o022:
        raise RuntimeError(f"coordination directory has unsafe owner/permissions: {root}")
    return root / f"{name}.lock"


def open_lock(path: Path) -> int:
    if fcntl is None or os.name != "posix":
        raise RuntimeError("host coordination requires POSIX flock")
    fd = os.open(path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
    info = os.fstat(fd)
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        os.close(fd)
        raise RuntimeError(f"unsafe coordination lock: {path}")
    return fd


@dataclass
class HostLease:
    fd: int
    path: Path
    borrowed: bool = False

    def close(self) -> None:
        if self.fd >= 0 and not self.borrowed:
            # LOCK_UN would also unlock inherited child descriptors.
            os.close(self.fd)
        self.fd = -1

    def environment(self, environment: dict[str, str]) -> dict[str, str]:
        if self.fd < 0:
            raise RuntimeError("host lease is closed")
        return {**environment, HOST_FD_ENV: str(self.fd)}

    def __enter__(self) -> HostLease:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def validate_inherited(fd: int, path: Path) -> None:
    """Prove this exact open description owns the lock, not just the inode."""
    assert fcntl is not None
    info, expected = os.fstat(fd), path.stat(follow_symlinks=False)
    if (info.st_dev, info.st_ino) != (expected.st_dev, expected.st_ino):
        raise RuntimeError("inherited host descriptor refers to a different lock")
    probe = open_lock(path)
    try:
        try:
            fcntl.flock(probe, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            pass
        else:
            raise RuntimeError("inherited host descriptor is not locked")
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("inherited host descriptor does not own the lock") from error
    finally:
        os.close(probe)


def acquire_host(name: str = "heavy", *, inherit: bool = True) -> HostLease:
    path = lock_path(name)
    inherited = os.environ.get(HOST_FD_ENV) if inherit and name == "heavy" else None
    if inherited is not None:
        try:
            fd = int(inherited)
            validate_inherited(fd, path)
        except (OSError, ValueError) as error:
            raise RuntimeError("invalid inherited host lease") from error
        return HostLease(fd, path, borrowed=True)
    fd = open_lock(path)
    try:
        assert fcntl is not None
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        os.close(fd)
        if error.errno in {errno.EACCES, errno.EAGAIN}:
            raise HostBusyError(f"host slot busy ({name}): {path}; retry after its owner exits") from error
        raise
    return HostLease(fd, path)


def host_pass_fds(environment: dict[str, str] | None = None) -> tuple[int, ...]:
    environment = os.environ if environment is None else environment
    raw = environment.get(HOST_FD_ENV)
    if raw is None:
        return ()
    try:
        fd = int(raw)
        validate_inherited(fd, lock_path())
    except (OSError, ValueError) as error:
        raise RuntimeError("invalid child host lease") from error
    return (fd,)
