from __future__ import annotations

from .artifacts import *


SOURCE_FINGERPRINT_FILES = {
    ".cargo/config.toml",
    "Cargo.lock",
    "Cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "scripts/perf.py",
}
SOURCE_FINGERPRINT_PREFIXES = ("crates/", "scripts/perf_tool/")
SOURCE_FINGERPRINT_ASSET_PREFIX = "assets/"

def command_output(command: list[str], *, cwd: Path = REPO_ROOT) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        return "<unavailable>"
    return completed.stdout.strip()


def tracked_source_paths() -> list[str]:
    completed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError("git ls-files failed while fingerprinting the source")
    return sorted(filter(None, completed.stdout.splitlines()))


def source_fingerprint() -> str:
    """Match the native acceptance source boundary exactly.

    Rust/Python sources and build configuration are content-hashed. Assets use
    size and mtime so large trees remain cheap to sample at every session
    boundary while still detecting in-session mutation.
    """
    digest = hashlib.sha256()
    for relative in tracked_source_paths():
        source = REPO_ROOT / relative
        if not source.is_file():
            continue
        if relative in SOURCE_FINGERPRINT_FILES or relative.startswith(
            SOURCE_FINGERPRINT_PREFIXES
        ):
            digest.update(b"content\0")
            digest.update(relative.encode())
            digest.update(b"\0")
            with source.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif relative.startswith(SOURCE_FINGERPRINT_ASSET_PREFIX):
            stats = source.stat()
            digest.update(b"asset-stat\0")
            digest.update(relative.encode())
            digest.update(f"\0{stats.st_size}\0{stats.st_mtime_ns}\0".encode())
    return digest.hexdigest()


def finalize_session_source(manifest: dict[str, Any]) -> list[str]:
    source = manifest.get("source")
    if source is None:
        return []
    if not isinstance(source, dict):
        return ["manifest source provenance is not an object"]
    started = source.get("fingerprint_start")
    if not isinstance(started, str) or not re.fullmatch(r"[0-9a-f]{64}", started):
        return ["manifest source fingerprint_start is invalid"]
    ended = source_fingerprint()
    unchanged = ended == started
    source.update(
        {
            "fingerprint_end": ended,
            "unchanged": unchanged,
            "finished_at": datetime.now(UTC).isoformat(),
        }
    )
    return [] if unchanged else ["source fingerprint changed during the session"]


ENVIRONMENT_LOCK_WINDOW_FIELDS = (
    "logical_width",
    "logical_height",
    "physical_width",
    "physical_height",
    "scale_factor",
    "rtt_quality",
    "scene_target_width",
    "scene_target_height",
    "mask_target_width",
    "mask_target_height",
    "target_scale_factor",
)


def environment_lock_payload(
    *,
    manifest: dict[str, Any],
    validation: Validation,
    contract_id: str,
    stage_id: str,
) -> dict[str, Any]:
    if validation.window is None or validation.adapter is None:
        raise RuntimeError("environment lock requires validated window and adapter evidence")
    window = validation.window
    return {
        "schema_version": 1,
        "contract_id": contract_id,
        "stage_id": stage_id,
        "subject_commit": manifest["git"]["commit"],
        "source_fingerprint": manifest["source"]["fingerprint_start"],
        "host": manifest["host"],
        "adapter": validation.adapter,
        "resolved_window_backend": window["resolved_window_backend"],
        "adapter_backend": window["adapter_backend"],
        "requested_present_mode": window["requested_present_mode"],
        "effective_present_mode": window["effective_present_mode"],
        "window": {field: window[field] for field in ENVIRONMENT_LOCK_WINDOW_FIELDS},
        "capture_binary_sha256": manifest["binary"]["sha256"],
    }


def write_json_exclusive(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def enforce_environment_lock(
    *,
    args: argparse.Namespace,
    session_dir: Path,
    validation: Validation,
    preflight: bool,
) -> list[str]:
    if args.environment_lock is None:
        return []
    lock_path = Path(args.environment_lock).resolve()
    manifest = json.loads((session_dir / "manifest.json").read_text(encoding="utf-8"))
    try:
        observed = environment_lock_payload(
            manifest=manifest,
            validation=validation,
            contract_id=args.contract,
            stage_id=args.stage,
        )
    except (KeyError, RuntimeError) as error:
        return [str(error)]
    if not lock_path.exists():
        if args.instrumentation != "capture" or not preflight:
            return ["environment lock is missing before a non-Capture-preflight run"]
        if not validation.valid:
            return ["invalid Capture preflight cannot create the environment lock"]
        try:
            write_json_exclusive(lock_path, observed)
        except FileExistsError:
            pass
        except OSError as error:
            return [f"cannot create environment lock: {error}"]
    try:
        expected = json.loads(lock_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read environment lock: {error}"]
    comparable = dict(observed)
    if args.instrumentation == "memory":
        comparable["capture_binary_sha256"] = expected.get("capture_binary_sha256")
    if expected != comparable:
        return ["run environment differs from the generation environment lock"]
    return []


def git_metadata() -> dict[str, Any]:
    status = command_output(["git", "status", "--short"])
    return {
        "commit": command_output(["git", "rev-parse", "HEAD"]),
        "short_commit": command_output(["git", "rev-parse", "--short", "HEAD"]),
        "dirty_paths": [] if status == "" else status.splitlines(),
    }


def cpu_model() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "<unknown>"


def host_metadata() -> dict[str, str]:
    return {
        "platform": platform.platform(),
        "python": sys.version.split()[0],
        "cpu": cpu_model(),
        "hostname": platform.node(),
        "cargo": command_output(["cargo", "--version"]),
        "rustc": command_output(["rustc", "--version"]),
    }


def fixed_environment(args: argparse.Namespace) -> dict[str, str]:
    values = {
        "BEVY_ASSET_ROOT": str(REPO_ROOT),
        "HW_PRESENT_MODE": args.present_mode,
        "HW_WINDOW_BACKEND": args.window_backend,
    }
    if args.backend != "auto":
        values["WGPU_BACKEND"] = args.backend
    if args.adapter:
        values["WGPU_ADAPTER_NAME"] = args.adapter
    return values


def cargo_features(instrumentation: str) -> str:
    return {
        "capture": "profiling",
        "tracy": "profiling-tracy",
        "memory": "profiling-memory",
    }[instrumentation]


def build_binary(args: argparse.Namespace) -> Path:
    binary = Path(args.binary).resolve() if args.binary else REPO_ROOT / "target/profiling/bevy_app"
    if args.skip_build:
        if not binary.is_file():
            raise RuntimeError(f"profiling binary does not exist: {binary}")
        return binary

    command = [
        "cargo",
        "build",
        "--profile",
        "profiling",
        "-p",
        "bevy_app@0.1.0",
        "--no-default-features",
        "--features",
        cargo_features(args.instrumentation),
    ]
    print("+", " ".join(command), flush=True)
    completed = subprocess.run(command, cwd=REPO_ROOT, check=False)
    if completed.returncode != 0:
        raise RuntimeError("profiling binary build failed")
    if not binary.is_file():
        raise RuntimeError(f"Cargo succeeded but profiling binary is missing: {binary}")
    return binary


def executable_path(value: str | None, label: str) -> Path:
    if not value:
        raise RuntimeError(f"{label} was not provided")
    path = Path(value).resolve()
    if not path.is_file() or not os.access(path, os.X_OK):
        raise RuntimeError(f"{label} is not an executable file: {path}")
    return path


def profiling_tool_metadata(args: argparse.Namespace) -> dict[str, Any]:
    if args.instrumentation == "capture":
        return {}
    metadata: dict[str, Any] = {}
    if args.instrumentation == "tracy":
        capture = executable_path(args.tracy_capture_binary, "Tracy capture binary")
        csvexport = executable_path(args.tracy_csvexport_binary, "Tracy csvexport binary")
        metadata.update(
            {
                "tracy_version": "0.13.1",
                "capture": {"path": str(capture), "sha256": sha256(capture)},
                "csvexport": {"path": str(csvexport), "sha256": sha256(csvexport)},
            }
        )
    if args.instrumentation == "memory":
        timer = shutil.which("time")
        if not timer:
            raise RuntimeError("GNU time is required for --instrumentation memory")
        timer_path = executable_path(timer, "GNU time binary")
        metadata["time"] = {"path": str(timer_path), "sha256": sha256(timer_path)}
    return metadata


def prepare_session(args: argparse.Namespace, binary: Path, cases: list[Case]) -> Path:
    if args.output:
        output = Path(args.output)
        session_dir = output if output.is_absolute() else REPO_ROOT / output
    else:
        timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
        session_dir = REPO_ROOT / "target/perf-runs" / f"{timestamp}-{git_metadata()['short_commit']}"
    session_dir = session_dir.resolve()
    if session_dir.exists():
        raise RuntimeError(f"output directory already exists: {session_dir}")
    session_dir.mkdir(parents=True)
    (session_dir / "cases").mkdir()

    rtt_light_contract = None
    if args.contract is not None:
        contract = load_rtt_light_contract(args.contract)
        selected_sizes = list(dict.fromkeys(case.size for case in cases))
        fixture_layouts = {
            size: build_fixture_layout(contract, size) for size in selected_sizes
        }
        rtt_light_contract = {
            "contract_id": args.contract,
            "stage_id": args.stage,
            "lane": args.lane,
            **contract_fingerprints(contract),
            "fixture_id": contract["fixture"]["fixture_id"],
            "layout_checksums": {
                size: layout["layout_checksum"]
                for size, layout in fixture_layouts.items()
            },
            "lifecycle": contract["lifecycle"],
        }

    matrix = {
        "workload": args.workload,
        "sizes": list(dict.fromkeys(case.size for case in cases)),
        "renders": list(dict.fromkeys(case.render for case in cases)),
        "seed": args.seed,
        "repeat": args.repeat,
        "warmup_secs": getattr(args, "warmup_secs", None),
        "measure_secs": getattr(args, "measure_secs", None),
        "fixed_hz": getattr(args, "fixed_hz", None),
        "warmup_ticks": getattr(args, "warmup_ticks", None),
        "audit_ticks": getattr(args, "audit_ticks", None),
        "preflight_runs": args.preflight_runs,
        "souls": args.souls,
        "familiars": args.familiars,
        "familiar_policies": sorted({case.familiar_policy for case in cases}),
        "operation_dialog_modes": sorted({case.operation_dialog for case in cases}),
        "dashboard_modes": sorted({case.dashboard_mode for case in cases}),
        "behavior_cases": list(
            dict.fromkeys(
                case.behavior_case
                for case in cases
                if case.behavior_case is not None
            )
        ),
        "capture_kind": args.capture_kind,
        "clock_mode": args.clock_mode,
        "warmup_checksum_policy": getattr(args, "warmup_checksum_policy", None),
        "measure_end_checksum_policy": getattr(args, "measure_end_checksum_policy", None),
        "allow_log_patterns": list(args.allow_log_pattern),
        "tracy_capture_secs": args.tracy_capture_secs,
        "window_width": args.window_width,
        "window_height": args.window_height,
        "window_scale_factor": args.window_scale_factor,
        "rtt_quality": args.rtt_quality,
        "environment_lock": (
            str(Path(args.environment_lock).resolve())
            if args.environment_lock is not None
            else None
        ),
        "rtt_light_contract": rtt_light_contract,
    }
    write_json(session_dir / "matrix.json", matrix)
    source_start = source_fingerprint()
    manifest = {
        "schema_version": SESSION_MANIFEST_SCHEMA_VERSION,
        "created_at": datetime.now(UTC).isoformat(),
        "repo_root": str(REPO_ROOT),
        "git": git_metadata(),
        "source": {
            "algorithm": "hell-workers-source-v1",
            "fingerprint_start": source_start,
            "fingerprint_end": None,
            "unchanged": None,
            "started_at": datetime.now(UTC).isoformat(),
            "finished_at": None,
        },
        "host": host_metadata(),
        "binary": {
            "path": str(binary),
            "sha256": sha256(binary),
            "instrumentation": args.instrumentation,
        },
        "profiling_tools": profiling_tool_metadata(args),
        "requested_environment": fixed_environment(args),
        "matrix": matrix,
        "cases": [asdict(case) | {"id": case.identifier} for case in cases],
        "actual_adapters": [],
        "status": "running",
    }
    write_json(session_dir / "manifest.json", manifest)
    return session_dir


def run_csvexport(
    csvexport: Path,
    trace_path: Path,
    output_path: Path,
    log_path: Path,
    arguments: list[str],
    timeout_secs: float,
) -> tuple[int, str | None]:
    with output_path.open("w", encoding="utf-8") as output_handle, log_path.open(
        "w", encoding="utf-8"
    ) as log_handle:
        try:
            completed = subprocess.run(
                [str(csvexport), *arguments, str(trace_path)],
                cwd=REPO_ROOT,
                stdout=output_handle,
                stderr=log_handle,
                check=False,
                timeout=timeout_secs,
            )
            return completed.returncode, None
        except subprocess.TimeoutExpired:
            return 124, f"Tracy csvexport timed out after {timeout_secs} seconds"


def read_tracy_zone_summary(path: Path) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse Tracy Task Dashboard zones: {error}"]
    required = {"name", "total_ns", "counts", "mean_ns", "min_ns", "max_ns"}
    if not required <= headers:
        errors.append("Tracy Task Dashboard zone CSV is missing required columns")
        return {}, errors
    zones: list[dict[str, Any]] = []
    total_ns = 0
    invocations = 0
    for index, row in enumerate(rows):
        try:
            zone_total = int(row["total_ns"])
            zone_count = int(row["counts"])
            zone_mean = int(row["mean_ns"])
            zone_min = int(row["min_ns"])
            zone_max = int(row["max_ns"])
            if min(zone_total, zone_count, zone_mean, zone_min, zone_max) < 0:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            errors.append(f"Tracy Task Dashboard zone row {index} has invalid numeric data")
            continue
        zones.append(
            {
                "name": row["name"],
                "source": row.get("src_file", ""),
                "line": row.get("src_line", ""),
                "total_ns": zone_total,
                "count": zone_count,
                "mean_ns": zone_mean,
                "min_ns": zone_min,
                "max_ns": zone_max,
            }
        )
        total_ns += zone_total
        invocations += zone_count
    if not zones:
        errors.append(
            f"Tracy trace has no zones matching {TRACY_DASHBOARD_ZONE_FILTER!r}"
        )
    return {
        "zone_count": len(zones),
        "total_ns": total_ns,
        "invocations": invocations,
        "mean_ns_per_invocation": total_ns / invocations if invocations else 0.0,
        "zones": zones,
    }, errors


def read_native_memory(
    path: Path, *, frame_samples: int | None
) -> tuple[dict[str, Any], list[str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse native memory artifact: {error}"]
    required = {
        "schema_version",
        "baseline_live_bytes",
        "peak_live_bytes",
        "final_live_bytes",
        "allocated_bytes",
        "deallocated_bytes",
        "allocation_calls",
        "deallocation_calls",
        "reallocation_calls",
        "accounting_errors",
    }
    if headers != required:
        return {}, ["native memory artifact has unexpected columns"]
    if len(rows) != 1:
        return {}, ["native memory artifact must contain exactly one row"]
    try:
        if rows[0]["schema_version"] != "1":
            raise ValueError
        values = {name: int(rows[0][name]) for name in required - {"schema_version"}}
        if min(values.values()) < 0:
            raise ValueError
        if values["accounting_errors"] != 0:
            raise ValueError
        if values["peak_live_bytes"] < max(
            values["baseline_live_bytes"], values["final_live_bytes"]
        ):
            raise ValueError
        if values["baseline_live_bytes"] + values["allocated_bytes"] != (
            values["final_live_bytes"] + values["deallocated_bytes"]
        ):
            raise ValueError
        if (values["allocation_calls"] == 0) != (values["allocated_bytes"] == 0):
            raise ValueError
        if (values["deallocation_calls"] == 0) != (
            values["deallocated_bytes"] == 0
        ):
            raise ValueError
        if values["reallocation_calls"] > min(
            values["allocation_calls"], values["deallocation_calls"]
        ):
            raise ValueError
        if frame_samples is None or frame_samples <= 0:
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["native memory artifact contains invalid values"]
    return {
        "source": "profiling-memory global allocator counters",
        **values,
        "peak_growth_bytes": values["peak_live_bytes"]
        - values["baseline_live_bytes"],
        "net_live_growth_bytes": values["final_live_bytes"]
        - values["baseline_live_bytes"],
        "allocated_bytes_per_frame": values["allocated_bytes"] / frame_samples,
        "deallocated_bytes_per_frame": values["deallocated_bytes"] / frame_samples,
        "allocation_calls_per_frame": values["allocation_calls"] / frame_samples,
        "deallocation_calls_per_frame": values["deallocation_calls"] / frame_samples,
    }, []


def read_resource_usage(path: Path) -> tuple[dict[str, Any], list[str]]:
    errors: list[str] = []
    if not path.is_file():
        return {}, ["missing GNU time resource-usage.txt"]
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        key, separator, value = line.partition("=")
        if separator:
            values[key] = value
    try:
        result = {
            "max_rss_kib": int(values["max_rss_kib"]),
            "user_cpu_secs": float(values["user_cpu_secs"]),
            "system_cpu_secs": float(values["system_cpu_secs"]),
            "exit_status": int(values["exit_status"]),
        }
        if (
            result["max_rss_kib"] <= 0
            or result["user_cpu_secs"] < 0
            or result["system_cpu_secs"] < 0
            or not math.isfinite(result["user_cpu_secs"])
            or not math.isfinite(result["system_cpu_secs"])
        ):
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["GNU time resource usage contains invalid values"]
    return result, errors


def read_task_dashboard_cpu(path: Path) -> tuple[dict[str, Any], list[str]]:
    try:
        with path.open(newline="", encoding="utf-8") as handle:
            reader = csv.DictReader(handle)
            rows = list(reader)
            headers = set(reader.fieldnames or [])
    except (csv.Error, OSError, UnicodeError) as error:
        return {}, [f"cannot parse Task Dashboard CPU artifact: {error}"]
    required = {"schema_version", "system_invocations", "total_elapsed_ns"}
    if headers != required:
        return {}, ["Task Dashboard CPU artifact has unexpected columns"]
    if len(rows) != 1:
        return {}, ["Task Dashboard CPU artifact must contain exactly one row"]
    try:
        if rows[0]["schema_version"] != "1":
            raise ValueError
        invocations = int(rows[0]["system_invocations"])
        total_ns = int(rows[0]["total_elapsed_ns"])
        if invocations <= 0 or total_ns <= 0:
            raise ValueError
    except (KeyError, TypeError, ValueError):
        return {}, ["Task Dashboard CPU artifact contains invalid values"]
    return {
        "source": "profiling-only Instant timer",
        "invocations": invocations,
        "total_ns": total_ns,
        "mean_ns_per_invocation": total_ns / invocations,
    }, []


def collect_profile_artifact(
    *,
    args: argparse.Namespace,
    case: Case,
    run_dir: Path,
    trace_returncode: int | None,
    frame_samples: int | None,
) -> tuple[dict[str, Any] | None, list[str]]:
    if args.instrumentation == "capture":
        if args.capture_kind != "frame-time" or case.workload != "task-dashboard":
            return None, []
        cpu_summary, errors = read_task_dashboard_cpu(
            run_dir / "data" / "task_dashboard_cpu.csv"
        )
        artifact = {
            "instrumentation": "capture",
            "task_dashboard_cpu": cpu_summary,
        }
        write_json(run_dir / "profile-artifact.json", artifact)
        return artifact, errors
    if args.instrumentation == "memory":
        memory_summary, memory_errors = read_native_memory(
            run_dir / "data" / "memory.csv", frame_samples=frame_samples
        )
        resource_usage, resource_errors = read_resource_usage(
            run_dir / "resource-usage.txt"
        )
        artifact = {
            "instrumentation": "memory",
            "allocation_memory": memory_summary,
            "process_memory": resource_usage,
        }
        write_json(run_dir / "profile-artifact.json", artifact)
        return artifact, memory_errors + resource_errors
    errors: list[str] = []
    trace_path = run_dir / "trace.tracy"
    if trace_returncode != 0:
        errors.append(f"Tracy capture exited with status {trace_returncode}")
    if not trace_path.is_file() or trace_path.stat().st_size <= 0:
        errors.append("missing or empty trace.tracy")
        return None, errors
    artifact: dict[str, Any] = {
        "instrumentation": args.instrumentation,
        "trace": {
            "path": "trace.tracy",
            "bytes": trace_path.stat().st_size,
            "sha256": sha256(trace_path),
        },
    }
    csvexport = executable_path(args.tracy_csvexport_binary, "Tracy csvexport binary")
    if args.instrumentation == "tracy" and case.workload == "task-dashboard":
        zones_path = run_dir / "tracy-task-dashboard-zones.csv"
        returncode, timeout_error = run_csvexport(
            csvexport,
            trace_path,
            zones_path,
            run_dir / "tracy-task-dashboard-zones.log",
            ["-f", TRACY_DASHBOARD_ZONE_FILTER],
            min(args.timeout_secs, 120.0),
        )
        if timeout_error:
            errors.append(timeout_error)
        if returncode != 0:
            errors.append(f"Tracy Task Dashboard csvexport exited with status {returncode}")
        else:
            cpu_summary, cpu_errors = read_tracy_zone_summary(zones_path)
            errors.extend(cpu_errors)
            artifact["task_dashboard_cpu"] = cpu_summary
    write_json(run_dir / "profile-artifact.json", artifact)
    return artifact, errors


def run_one(
    *,
    args: argparse.Namespace,
    binary: Path,
    session_dir: Path,
    case: Case,
    run_number: int,
    preflight: bool,
) -> Validation:
    case_dir = session_dir / "cases" / case.identifier
    case_dir.mkdir(exist_ok=True)
    label = ("preflight-" if preflight else "run-") + f"{run_number:03d}"
    final_dir = case_dir / label
    temporary_dir = case_dir / f".{label}.tmp"
    if final_dir.exists() or temporary_dir.exists():
        raise RuntimeError(f"run directory collision: {final_dir}")
    temporary_dir.mkdir()
    data_dir = temporary_dir / "data"
    data_dir.mkdir()

    command = [
        str(binary),
        "--perf-scenario",
        "--perf-seed",
        str(case.seed),
        "--perf-size",
        case.size,
        "--perf-workload",
        case.workload,
        "--perf-render",
        case.render,
        "--perf-clock",
        args.clock_mode,
        "--perf-familiar-policy",
        case.familiar_policy,
        "--perf-operation-dialog",
        case.operation_dialog,
        "--perf-dashboard",
        case.dashboard_mode,
        "--perf-output-dir",
        str(data_dir),
    ]
    if args.contract is not None:
        command.extend(
            [
                "--perf-contract",
                args.contract,
                "--perf-stage",
                args.stage,
                "--perf-lane",
                args.lane,
            ]
        )
    if case.behavior_case is not None:
        command.extend(["--perf-behavior-case", case.behavior_case])
    if args.capture_kind == "frame-time":
        command.extend(
            [
                "--perf-warmup-secs",
                str(args.warmup_secs),
                "--perf-measure-secs",
                str(args.measure_secs),
            ]
        )
    elif args.capture_kind == "fixed-step-determinism":
        command.extend(
            [
                "--perf-fixed-hz",
                str(args.fixed_hz),
                "--perf-warmup-ticks",
                str(args.warmup_ticks),
                "--perf-audit-ticks",
                str(args.audit_ticks),
            ]
        )
    else:
        command.extend(["--perf-fixed-hz", str(args.fixed_hz)])
    if case.souls is not None:
        command.extend(["--spawn-souls", str(case.souls)])
        command.extend(["--spawn-familiars", str(case.familiars)])
    if args.window_width is not None:
        command.extend(["--perf-window-width", str(args.window_width)])
        command.extend(["--perf-window-height", str(args.window_height)])
    if args.window_scale_factor is not None:
        command.extend(
            ["--perf-window-scale-factor", str(args.window_scale_factor)]
        )
    if args.rtt_quality is not None:
        command.extend(["--perf-rtt-quality", args.rtt_quality])
    env = os.environ.copy()
    env.update(fixed_environment(args))
    launch_command = command
    if args.instrumentation == "memory":
        timer = executable_path(shutil.which("time"), "GNU time binary")
        launch_command = [
            str(timer),
            "-f",
            "max_rss_kib=%M\nuser_cpu_secs=%U\nsystem_cpu_secs=%S\nexit_status=%x",
            "-o",
            str(temporary_dir / "resource-usage.txt"),
            *command,
        ]
    (temporary_dir / "command.txt").write_text(
        " ".join(launch_command) + "\n", encoding="utf-8"
    )
    write_json(
        temporary_dir / "requested-environment.json",
        {key: env[key] for key in sorted(fixed_environment(args))},
    )

    trace_process: subprocess.Popen[bytes] | None = None
    trace_log_handle = None
    trace_returncode: int | None = None
    if args.instrumentation == "tracy":
        capture = executable_path(args.tracy_capture_binary, "Tracy capture binary")
        trace_command = [
            str(capture),
            "-o",
            str(temporary_dir / "trace.tracy"),
            "-f",
        ]
        if args.tracy_capture_secs is not None:
            trace_command.extend(["-s", str(args.tracy_capture_secs)])
        (temporary_dir / "tracy-capture-command.txt").write_text(
            " ".join(trace_command) + "\n", encoding="utf-8"
        )
        trace_log_handle = (temporary_dir / "tracy-capture.log").open("wb")
        trace_process = subprocess.Popen(
            trace_command,
            cwd=REPO_ROOT,
            stdout=trace_log_handle,
            stderr=subprocess.STDOUT,
            creationflags=(
                subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
            ),
        )

    print(f"[{case.identifier} {label}]", flush=True)
    try:
        with (temporary_dir / "run.log").open("w", encoding="utf-8") as log_handle:
            if trace_process is None:
                try:
                    completed = subprocess.run(
                        launch_command,
                        cwd=REPO_ROOT,
                        env=env,
                        stdout=log_handle,
                        stderr=subprocess.STDOUT,
                        check=False,
                        timeout=args.timeout_secs,
                    )
                    returncode = completed.returncode
                except subprocess.TimeoutExpired:
                    returncode = 124
                    log_handle.write(
                        f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n"
                    )
            else:
                game_process = subprocess.Popen(
                    launch_command,
                    cwd=REPO_ROOT,
                    env=env,
                    stdout=log_handle,
                    stderr=subprocess.STDOUT,
                )
                deadline = time.monotonic() + args.timeout_secs
                trace_disconnect_requested = False
                while game_process.poll() is None:
                    if (
                        not trace_disconnect_requested
                        and (data_dir / "summary.csv").is_file()
                    ):
                        if trace_process.poll() is None:
                            trace_process.send_signal(
                                signal.CTRL_BREAK_EVENT
                                if os.name == "nt"
                                else signal.SIGINT
                            )
                        trace_disconnect_requested = True
                    if time.monotonic() >= deadline:
                        game_process.terminate()
                        try:
                            game_process.wait(timeout=5.0)
                        except subprocess.TimeoutExpired:
                            game_process.kill()
                            game_process.wait()
                        returncode = 124
                        log_handle.write(
                            f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n"
                        )
                        break
                    time.sleep(0.05)
                else:
                    returncode = game_process.returncode
                if not trace_disconnect_requested and trace_process.poll() is None:
                    trace_process.send_signal(
                        signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT
                    )
    finally:
        if trace_process is not None:
            try:
                trace_returncode = trace_process.wait(timeout=min(args.timeout_secs, 120.0))
            except subprocess.TimeoutExpired:
                trace_process.terminate()
                try:
                    trace_process.wait(timeout=5.0)
                except subprocess.TimeoutExpired:
                    trace_process.kill()
                    trace_process.wait()
                trace_returncode = 124
            if trace_log_handle is not None:
                trace_log_handle.close()

    validation = validate_run(
        temporary_dir,
        returncode=returncode,
        expected_case=case,
        expected_adapter=args.adapter,
        expected_backend=args.backend,
        allow_log_patterns=args.allow_log_pattern,
        capture_kind=args.capture_kind,
        expected_warmup_secs=getattr(args, "warmup_secs", None),
        expected_measure_secs=getattr(args, "measure_secs", None),
        expected_fixed_hz=getattr(args, "fixed_hz", None),
        expected_warmup_ticks=getattr(args, "warmup_ticks", None),
        expected_audit_ticks=getattr(args, "audit_ticks", None),
        expected_window_backend=args.window_backend,
        expected_present_mode=args.present_mode,
        expected_window_width=args.window_width,
        expected_window_height=args.window_height,
        expected_window_scale_factor=args.window_scale_factor,
        expected_rtt_quality=args.rtt_quality,
        expected_contract=args.contract,
        expected_stage=args.stage,
        expected_lane=args.lane,
    )
    frame_samples = None
    if validation.summary is not None:
        try:
            frame_samples = int(validation.summary["samples"])
        except (KeyError, TypeError, ValueError):
            pass
    profile_artifact, profile_errors = collect_profile_artifact(
        args=args,
        case=case,
        run_dir=temporary_dir,
        trace_returncode=trace_returncode,
        frame_samples=frame_samples,
    )
    validation.profile_artifact = profile_artifact
    validation.reasons.extend(profile_errors)
    validation.valid = not validation.reasons
    validation.reasons.extend(
        enforce_environment_lock(
            args=args,
            session_dir=session_dir,
            validation=validation,
            preflight=preflight,
        )
    )
    validation.valid = not validation.reasons
    write_json(temporary_dir / "validation.json", validation.to_json())
    write_json(
        temporary_dir / "run-metadata.json",
        {
            "case": asdict(case),
            "rtt_light_contract": (
                {
                    "contract_id": args.contract,
                    "stage_id": args.stage,
                    "lane": args.lane,
                    "layout_checksum": build_fixture_layout(
                        load_rtt_light_contract(args.contract), case.size
                    )["layout_checksum"],
                }
                if args.contract is not None
                else None
            ),
            "preflight": preflight,
            "returncode": returncode,
            "trace_returncode": trace_returncode,
            "started_by": "scripts/perf.py",
            "actual_adapter": validation.adapter,
            "actual_window": validation.window,
            "actual_render_inventory": validation.render_inventory,
        },
    )
    temporary_dir.replace(final_dir)
    return validation
