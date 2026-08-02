from __future__ import annotations

from .artifacts import *

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

    matrix = {
        "workload": args.workload,
        "sizes": [case.size for case in cases],
        "renders": [case.render for case in cases],
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
        "capture_kind": args.capture_kind,
        "clock_mode": args.clock_mode,
        "warmup_checksum_policy": getattr(args, "warmup_checksum_policy", None),
        "measure_end_checksum_policy": getattr(args, "measure_end_checksum_policy", None),
        "tracy_capture_secs": args.tracy_capture_secs,
    }
    write_json(session_dir / "matrix.json", matrix)
    manifest = {
        "schema_version": 1,
        "created_at": datetime.now(UTC).isoformat(),
        "repo_root": str(REPO_ROOT),
        "git": git_metadata(),
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
    if args.capture_kind == "frame-time":
        command.extend(
            [
                "--perf-warmup-secs",
                str(args.warmup_secs),
                "--perf-measure-secs",
                str(args.measure_secs),
            ]
        )
    else:
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
    if case.souls is not None:
        command.extend(["--spawn-souls", str(case.souls)])
        command.extend(["--spawn-familiars", str(case.familiars)])
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
    write_json(temporary_dir / "validation.json", validation.to_json())
    write_json(
        temporary_dir / "run-metadata.json",
        {
            "case": asdict(case),
            "preflight": preflight,
            "returncode": returncode,
            "trace_returncode": trace_returncode,
            "started_by": "scripts/perf.py",
            "actual_adapter": validation.adapter,
        },
    )
    temporary_dir.replace(final_dir)
    return validation
