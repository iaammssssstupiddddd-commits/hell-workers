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
        "capture_kind": args.capture_kind,
        "clock_mode": args.clock_mode,
        "warmup_checksum_policy": getattr(args, "warmup_checksum_policy", None),
        "measure_end_checksum_policy": getattr(args, "measure_end_checksum_policy", None),
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
        "requested_environment": fixed_environment(args),
        "matrix": matrix,
        "cases": [asdict(case) | {"id": case.identifier} for case in cases],
        "actual_adapters": [],
        "status": "running",
    }
    write_json(session_dir / "manifest.json", manifest)
    return session_dir


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
    (temporary_dir / "command.txt").write_text(" ".join(command) + "\n", encoding="utf-8")
    write_json(
        temporary_dir / "requested-environment.json",
        {key: env[key] for key in sorted(fixed_environment(args))},
    )

    print(f"[{case.identifier} {label}]", flush=True)
    with (temporary_dir / "run.log").open("w", encoding="utf-8") as log_handle:
        try:
            completed = subprocess.run(
                command,
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
            log_handle.write(f"PERF_RUNNER: timeout after {args.timeout_secs} seconds\n")

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
    write_json(temporary_dir / "validation.json", validation.to_json())
    write_json(
        temporary_dir / "run-metadata.json",
        {
            "case": asdict(case),
            "preflight": preflight,
            "returncode": returncode,
            "started_by": "scripts/perf.py",
            "actual_adapter": validation.adapter,
        },
    )
    temporary_dir.replace(final_dir)
    return validation
