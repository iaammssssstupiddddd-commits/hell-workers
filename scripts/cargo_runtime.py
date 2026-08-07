"""Keep repository Cargo work on persistent storage with a bounded build fan-out."""

from __future__ import annotations

import os
import platform
import re
from pathlib import Path
from typing import Mapping


GIB = 1024**3
MIN_CARGO_MEMORY_GIB = 8
TWO_JOB_MEMORY_GIB = 16
MEMORY_FILESYSTEM_TYPES = frozenset({"tmpfs", "ramfs", "devtmpfs"})
TMP_ROOT = Path("/tmp")
INTERACTIVE_BUILD_LANES = ("a", "b")


def decode_mount_path(value: str) -> str:
    """Decode the octal escapes used in Linux mountinfo paths."""
    return re.sub(
        r"\\([0-7]{3})",
        lambda match: chr(int(match.group(1), 8)),
        value,
    )


def path_is_within(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def filesystem_type(
    path: Path, *, mountinfo_path: Path = Path("/proc/self/mountinfo")
) -> str:
    """Return the Linux filesystem type for ``path`` without shelling out."""
    target = path.resolve()
    matched: tuple[int, str] | None = None
    try:
        lines = mountinfo_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise RuntimeError(f"cannot inspect mount table for {target}: {error}") from error

    for line in lines:
        left, separator, right = line.partition(" - ")
        if not separator:
            continue
        left_fields = left.split()
        right_fields = right.split()
        if len(left_fields) < 5 or not right_fields:
            continue
        mount_point = Path(decode_mount_path(left_fields[4])).resolve()
        if not path_is_within(target, mount_point):
            continue
        candidate = (len(str(mount_point)), right_fields[0])
        if matched is None or candidate[0] > matched[0]:
            matched = candidate

    if matched is None:
        raise RuntimeError(f"cannot resolve filesystem type for {target}")
    return matched[1]


def workspace_cargo_target(repo: Path) -> Path:
    return (repo / "target").resolve()


def validate_build_lane(lane: str | None) -> str | None:
    """Validate the small, fixed set of interactive build lanes."""
    if lane is None:
        return None
    if lane not in INTERACTIVE_BUILD_LANES:
        allowed = ", ".join(INTERACTIVE_BUILD_LANES)
        raise ValueError(f"invalid build lane {lane!r}; expected one of: {allowed}")
    return lane


def workspace_lane_target(repo: Path, lane: str) -> Path:
    """Return the persistent Cargo target/build root for an interactive lane."""
    validated_lane = validate_build_lane(lane)
    assert validated_lane is not None
    return workspace_cargo_target(repo) / "lanes" / validated_lane


def workspace_lane_temp_dir(repo: Path, lane: str, namespace: str) -> Path:
    """Return a lane-local process temporary directory."""
    return workspace_lane_target(repo, lane) / workspace_temp_dir_name(namespace)


def workspace_temp_dir_name(namespace: str) -> str:
    if not re.fullmatch(r"\.?[a-z0-9][a-z0-9._-]*", namespace):
        raise ValueError(f"invalid workspace temporary-directory namespace: {namespace!r}")
    return namespace


def workspace_temp_dir(repo: Path, namespace: str) -> Path:
    return workspace_cargo_target(repo) / workspace_temp_dir_name(namespace)


def account_home() -> Path:
    """Return the account home without trusting a caller-provided ``HOME``."""
    if os.name == "posix":
        try:
            import pwd

            return Path(pwd.getpwuid(os.getuid()).pw_dir).resolve()
        except (ImportError, KeyError, OSError):
            pass
    return Path.home().resolve()


def environment_path(repo: Path, value: str) -> Path:
    """Resolve an environment path as Cargo will see it from the repository."""
    candidate = Path(value).expanduser()
    if not candidate.is_absolute():
        candidate = repo / candidate
    return candidate.resolve()


def persistent_toolchain_home(
    repo: Path,
    environment: Mapping[str, str],
    *,
    variable: str,
    default_name: str,
    label: str,
) -> Path:
    """Keep Cargo and rustup caches off tmpfs without discarding safe overrides."""
    inherited = environment.get(variable)
    if inherited:
        candidate = environment_path(repo, inherited)
        if persistent_storage_error(candidate, label=label) is None:
            return candidate

    fallback = account_home() / default_name
    storage_error = persistent_storage_error(fallback, label=label)
    if storage_error:
        raise RuntimeError(storage_error)
    return fallback


def meminfo_bytes() -> dict[str, int] | None:
    try:
        lines = Path("/proc/meminfo").read_text(encoding="utf-8").splitlines()
    except OSError:
        return None
    values: dict[str, int] = {}
    for line in lines:
        key, separator, remainder = line.partition(":")
        fields = remainder.split()
        if not separator or not fields or not fields[0].isdigit():
            continue
        values[key] = int(fields[0]) * 1024
    return values


def mem_available_bytes() -> int | None:
    values = meminfo_bytes()
    return values.get("MemAvailable") if values is not None else None


def swap_memory_bytes() -> tuple[int | None, int | None]:
    """Return total/free swap, or ``None`` values when Linux counters are absent."""
    values = meminfo_bytes()
    if values is None:
        return None, None
    return values.get("SwapTotal"), values.get("SwapFree")


def cargo_build_jobs(memory_available: int | None = None) -> int:
    available = mem_available_bytes() if memory_available is None else memory_available
    return 2 if available is not None and available >= TWO_JOB_MEMORY_GIB * GIB else 1


def cargo_memory_error(
    memory_available: int | None = None,
    *,
    swap_total: int | None = None,
    swap_free: int | None = None,
) -> str | None:
    is_linux = platform.system() == "Linux"
    counters: dict[str, int] | None = None
    if memory_available is None:
        counters = meminfo_bytes()

    if memory_available is None:
        if counters is None:
            return (
                "cannot read /proc/meminfo for the Cargo safety guard"
                if is_linux
                else None
            )
        available = counters.get("MemAvailable")
        if available is None:
            return (
                "MemAvailable is unavailable for the Cargo safety guard"
                if is_linux
                else None
            )
    else:
        available = memory_available
    if available < MIN_CARGO_MEMORY_GIB * GIB:
        return (
            f"MemAvailable {available / GIB:.2f} GiB is below the "
            f"{MIN_CARGO_MEMORY_GIB} GiB Cargo safety floor"
        )

    # MemAvailable is the admission signal. Swap counters are retained in
    # resource manifests for diagnosis, but low/unknown swap is not a hard
    # failure while the RAM floor is satisfied.
    return None


def require_cargo_memory(
    memory_available: int | None = None,
    *,
    swap_total: int | None = None,
    swap_free: int | None = None,
) -> None:
    error = cargo_memory_error(
        memory_available,
        swap_total=swap_total,
        swap_free=swap_free,
    )
    if error:
        raise RuntimeError(error)


def cargo_environment(
    repo: Path,
    *,
    namespace: str,
    environment: Mapping[str, str] | None = None,
    incremental: bool | None,
    lane: str | None = None,
    create_temp_dir: bool = True,
) -> dict[str, str]:
    """Normalize Cargo output, compiler temp files, and build parallelism.

    ``CARGO_TARGET_DIR``, Cargo/rustup cache homes, and the three conventional
    temporary-directory variables are deliberately normalized. This makes an
    inherited shell setting unable to redirect Rust/Bevy output into a tmpfs.
    """
    validated_lane = validate_build_lane(lane)
    if validated_lane is None:
        target = workspace_cargo_target(repo)
        temporary = workspace_temp_dir(repo, namespace)
        jobs = cargo_build_jobs()
    else:
        target = workspace_lane_target(repo, validated_lane)
        temporary = workspace_lane_temp_dir(repo, validated_lane, namespace)
        jobs = 1
    for path, label in (
        (target, "workspace Cargo target"),
        (temporary, "workspace Cargo temporary directory"),
    ):
        storage_error = persistent_storage_error(path, label=label)
        if storage_error:
            raise RuntimeError(storage_error)
    if create_temp_dir:
        temporary.mkdir(parents=True, exist_ok=True)

    values = os.environ.copy() if environment is None else dict(environment)
    cargo_home = persistent_toolchain_home(
        repo,
        values,
        variable="CARGO_HOME",
        default_name=".cargo",
        label="Cargo home",
    )
    rustup_home = persistent_toolchain_home(
        repo,
        values,
        variable="RUSTUP_HOME",
        default_name=".rustup",
        label="rustup home",
    )
    values.update(
        {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_BUILD_TARGET_DIR": str(target),
            "CARGO_BUILD_BUILD_DIR": str(target),
            "CARGO_HOME": str(cargo_home),
            "RUSTUP_HOME": str(rustup_home),
            "TMPDIR": str(temporary),
            "TMP": str(temporary),
            "TEMP": str(temporary),
            "CARGO_BUILD_JOBS": str(jobs),
        }
    )
    if incremental is not None:
        values["CARGO_INCREMENTAL"] = "1" if incremental else "0"
    return values


def persistent_storage_error(path: Path, *, label: str) -> str | None:
    """Describe why a build, binary, or artifact path is unsafe for native work."""
    resolved = path.resolve()
    if path_is_within(resolved, TMP_ROOT):
        return f"{label} must not be placed under /tmp: {resolved}"
    if platform.system() != "Linux":
        return None
    filesystem = filesystem_type(resolved)
    if filesystem in MEMORY_FILESYSTEM_TYPES:
        return (
            f"{label} must use persistent storage, not {filesystem}: {resolved}"
        )
    return None


def resource_policy(
    repo: Path,
    *,
    namespace: str,
    incremental: bool | None,
    lane: str | None = None,
) -> dict[str, object]:
    """Return compact provenance suitable for a run manifest."""
    environment = cargo_environment(
        repo,
        namespace=namespace,
        incremental=incremental,
        lane=lane,
        create_temp_dir=False,
    )
    target = Path(environment["CARGO_TARGET_DIR"])
    temporary = Path(environment["TMPDIR"])
    cargo_home = Path(environment["CARGO_HOME"])
    rustup_home = Path(environment["RUSTUP_HOME"])
    memory_available = mem_available_bytes()
    swap_total, swap_free = swap_memory_bytes()
    return {
        "cargo_target": str(target),
        "cargo_build_dir": environment.get("CARGO_BUILD_BUILD_DIR"),
        "cargo_target_filesystem": filesystem_type(target)
        if platform.system() == "Linux"
        else None,
        "process_temp_dir": str(temporary),
        "process_temp_filesystem": filesystem_type(temporary)
        if platform.system() == "Linux"
        else None,
        "cargo_home": str(cargo_home),
        "cargo_home_filesystem": filesystem_type(cargo_home)
        if platform.system() == "Linux"
        else None,
        "rustup_home": str(rustup_home),
        "rustup_home_filesystem": filesystem_type(rustup_home)
        if platform.system() == "Linux"
        else None,
        "cargo_incremental": environment.get("CARGO_INCREMENTAL"),
        "cargo_build_jobs": int(environment["CARGO_BUILD_JOBS"]),
        "memory_guard": {
            "mem_available_gib": (
                round(memory_available / GIB, 2)
                if memory_available is not None
                else None
            ),
            "swap_total_gib": (
                round(swap_total / GIB, 2) if swap_total is not None else None
            ),
            "swap_free_gib": (
                round(swap_free / GIB, 2) if swap_free is not None else None
            ),
            "minimum_mem_available_gib": MIN_CARGO_MEMORY_GIB,
            "swap_is_telemetry_only": True,
        },
        "enforced": True,
    }
