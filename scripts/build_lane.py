"""Session leases for the fixed interactive Cargo build lanes."""

from __future__ import annotations

import errno
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence

try:
    import fcntl
except ImportError:  # pragma: no cover - exercised only on non-POSIX hosts
    fcntl = None  # type: ignore[assignment]

try:
    from cargo_runtime import (
        INTERACTIVE_BUILD_LANES,
        persistent_storage_error,
        validate_build_lane,
        workspace_lane_target,
    )
except ModuleNotFoundError:
    from scripts.cargo_runtime import (
        INTERACTIVE_BUILD_LANES,
        persistent_storage_error,
        validate_build_lane,
        workspace_lane_target,
    )


LANE_LOCK_NAME = ".session.lock"
LANE_ENV = "HW_BUILD_LANE"
LANE_FD_ENV = "HW_BUILD_LANE_FD"


class LaneBusyError(RuntimeError):
    """Raised when every fixed interactive lane is currently leased."""


def _require_flock() -> None:
    if fcntl is None or os.name != "posix":
        raise RuntimeError("lane shell requires a POSIX host with advisory flock support")


def lane_lock_path(repo: Path, lane: str) -> Path:
    """Return the lock path for a validated lane."""
    validated_lane = validate_build_lane(lane)
    assert validated_lane is not None
    return workspace_lane_target(repo, validated_lane) / LANE_LOCK_NAME


def _prepare_lock_path(repo: Path, lane: str) -> Path:
    lock_path = lane_lock_path(repo, lane)
    storage_error = persistent_storage_error(lock_path, label="interactive Cargo lane")
    if storage_error:
        raise RuntimeError(storage_error)
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    return lock_path


@dataclass
class BuildLaneLease:
    """An open, held lane descriptor that remains valid for a child shell."""

    repo: Path
    lane: str
    fd: int
    lock_path: Path

    def child_environment(
        self, environment: Mapping[str, str] | None = None
    ) -> dict[str, str]:
        values = os.environ.copy() if environment is None else dict(environment)
        values[LANE_ENV] = self.lane
        values[LANE_FD_ENV] = str(self.fd)
        return values

    def close(self) -> None:
        if self.fd < 0:
            return
        try:
            if fcntl is not None:
                fcntl.flock(self.fd, fcntl.LOCK_UN)
        finally:
            os.close(self.fd)
            self.fd = -1

    def __enter__(self) -> "BuildLaneLease":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


def _try_lock(lock_path: Path) -> int | None:
    _require_flock()
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
    try:
        assert fcntl is not None
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError as error:
        os.close(fd)
        if isinstance(error, BlockingIOError) or error.errno in {
            errno.EACCES,
            errno.EAGAIN,
        }:
            return None
        raise
    try:
        os.set_inheritable(fd, True)
    except OSError:
        assert fcntl is not None
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)
        raise
    return fd


def acquire_lane(
    repo: Path, lanes: Sequence[str] = INTERACTIVE_BUILD_LANES
) -> BuildLaneLease:
    """Acquire the first free lane in deterministic order."""
    _require_flock()
    for lane in lanes:
        lock_path = _prepare_lock_path(repo, lane)
        fd = _try_lock(lock_path)
        if fd is not None:
            return BuildLaneLease(repo.resolve(), lane, fd, lock_path)
    names = ", ".join(lanes)
    raise LaneBusyError(f"all interactive Cargo lanes are busy ({names}); no fallback target is used")


def lane_states(repo: Path) -> list[tuple[str, str, Path]]:
    """Report each lane as free or busy without taking ownership."""
    _require_flock()
    states: list[tuple[str, str, Path]] = []
    for lane in INTERACTIVE_BUILD_LANES:
        lock_path = _prepare_lock_path(repo, lane)
        fd = os.open(lock_path, os.O_CREAT | os.O_RDWR, 0o600)
        try:
            assert fcntl is not None
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError as error:
                if isinstance(error, BlockingIOError) or error.errno in {
                    errno.EACCES,
                    errno.EAGAIN,
                }:
                    states.append((lane, "busy", lock_path))
                    continue
                raise
            else:
                states.append((lane, "free", lock_path))
                fcntl.flock(fd, fcntl.LOCK_UN)
        finally:
            os.close(fd)
    return states


def validate_inherited_lease(repo: Path, environment: Mapping[str, str]) -> str | None:
    """Validate the lane descriptor inherited by a session child process."""
    lane_value = environment.get(LANE_ENV)
    if not lane_value:
        return None
    lane = validate_build_lane(lane_value)
    assert lane is not None
    descriptor = environment.get(LANE_FD_ENV)
    if descriptor is None:
        raise RuntimeError(
            f"{LANE_ENV} is set without an inherited {LANE_FD_ENV}; refusing an unleased lane"
        )
    try:
        fd = int(descriptor)
    except ValueError as error:
        raise RuntimeError(f"invalid inherited lane descriptor: {descriptor!r}") from error
    if fd < 0:
        raise RuntimeError(f"invalid inherited lane descriptor: {descriptor!r}")

    lock_path = lane_lock_path(repo, lane)
    try:
        descriptor_stat = os.fstat(fd)
        lock_stat = os.stat(lock_path)
        inheritable = os.get_inheritable(fd)
    except OSError as error:
        raise RuntimeError(
            f"cannot validate inherited lane {lane!r} descriptor {fd}: {error}"
        ) from error
    if not inheritable:
        raise RuntimeError(f"inherited lane descriptor {fd} is not inheritable")
    if (descriptor_stat.st_dev, descriptor_stat.st_ino) != (
        lock_stat.st_dev,
        lock_stat.st_ino,
    ):
        raise RuntimeError(
            f"inherited lane descriptor does not refer to {lock_path}; refusing unleased lane"
        )
    return lane


def run_lane_shell(
    repo: Path,
    command: Sequence[str] | None = None,
    *,
    environment: Mapping[str, str] | None = None,
) -> int:
    """Run a shell/command while keeping its lane lease open."""
    with acquire_lane(repo) as lease:
        values = lease.child_environment(environment)
        child = list(command) if command else [values.get("SHELL") or "/bin/bash"]
        print(f"Build lane {lease.lane}: {workspace_lane_target(repo, lease.lane)}", flush=True)
        print(f"+ {' '.join(child)}", flush=True)
        completed = subprocess.run(
            child,
            cwd=repo,
            env=values,
            check=False,
            pass_fds=(lease.fd,),
        )
        return completed.returncode
