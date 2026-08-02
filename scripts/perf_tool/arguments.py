from __future__ import annotations

from .model import *

def add_run_arguments(parser: argparse.ArgumentParser, *, fixed_step_audit: bool = False) -> None:
    parser.add_argument(
        "--workload",
        default="gather",
        choices=["gather", "path-door", "construction", "ui-gpu", "task-dashboard"],
    )
    parser.add_argument("--sizes", default="medium", help="comma-separated: small,medium,large")
    parser.add_argument("--renders", default="cpu", help="comma-separated: cpu,gpu")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--preflight-runs", type=int, default=0)
    parser.add_argument("--souls", type=int)
    parser.add_argument("--familiars", type=int)
    parser.add_argument("--output", help="new artifact directory, relative to the repository when not absolute")
    parser.add_argument("--adapter", help="required substring of the actual WGPU adapter name")
    parser.add_argument("--backend", default="auto", choices=["auto", "vulkan", "gl", "dx12", "metal"])
    parser.add_argument(
        "--window-backend",
        default="auto",
        choices=["auto", "wayland", "x11", "headless"],
        help="window backend; headless omits Winit, the primary window, and display sockets for CPU-only audits",
    )
    parser.add_argument("--present-mode", default="novsync")
    parser.add_argument("--instrumentation", default="capture", choices=["capture", "tracy", "memory"])
    parser.add_argument(
        "--tracy-capture-binary",
        default=os.environ.get("TRACY_CAPTURE_BINARY"),
        help="Tracy 0.13.1 capture executable; required for Tracy runs",
    )
    parser.add_argument(
        "--tracy-csvexport-binary",
        default=os.environ.get("TRACY_CSVEXPORT_BINARY"),
        help="Tracy 0.13.1 csvexport executable; required for Tracy runs",
    )
    parser.add_argument(
        "--tracy-capture-secs",
        type=int,
        help=(
            "diagnostic-only fixed Tracy duration; validated runs omit it and stop after "
            "the game writes the complete measure artifacts"
        ),
    )
    parser.add_argument("--binary", help="prebuilt profiling binary path")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--timeout-secs", type=float, default=600.0)
    if fixed_step_audit:
        parser.add_argument("--fixed-hz", type=int, default=64)
        parser.add_argument("--warmup-ticks", type=int, default=1920)
        parser.add_argument("--audit-ticks", type=int, default=128)
        parser.add_argument(
            "--familiar-policies",
            default="baseline",
            help="comma-separated: baseline,default,disabled",
        )
        parser.add_argument(
            "--operation-dialog-modes",
            default="hidden",
            help="comma-separated: hidden,open",
        )
        parser.set_defaults(
            capture_kind="fixed-step-determinism",
            clock_mode="fixed",
        )
    else:
        parser.add_argument("--warmup-secs", type=float, default=30.0)
        parser.add_argument("--measure-secs", type=float, default=60.0)
        parser.add_argument("--warmup-checksum-policy", default="record", choices=["require", "record"])
        parser.add_argument(
            "--measure-end-checksum-policy",
            default="record",
            choices=["require", "record"],
        )
        parser.set_defaults(
            capture_kind="frame-time",
            clock_mode="realtime",
            familiar_policies="baseline",
            operation_dialog_modes="hidden",
        )
    parser.add_argument(
        "--allow-log-pattern",
        action="append",
        default=[],
        help="regular expression for a known, explicitly allowed pre-capture warning",
    )
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--dashboard-modes",
        default="hidden",
        help="comma-separated: hidden,visible,active-filter",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=PERF_DESCRIPTION)
    subparsers = parser.add_subparsers(dest="command", required=True)
    run_parser = subparsers.add_parser("run", help="build, run, validate, and summarize a matrix")
    add_run_arguments(run_parser)
    audit_parser = subparsers.add_parser(
        "audit",
        help="run a fixed-step determinism audit with all state checkpoints required",
    )
    add_run_arguments(audit_parser, fixed_step_audit=True)
    summarize_parser = subparsers.add_parser("summarize", help="rebuild aggregate.csv and report.md")
    summarize_parser.add_argument("session")
    summarize_parser.add_argument("--warmup-checksum-policy", choices=["require", "record"])
    summarize_parser.add_argument("--measure-end-checksum-policy", choices=["require", "record"])
    compare_parser = subparsers.add_parser("compare", help="compare compatible valid sessions")
    compare_parser.add_argument("--baseline", required=True)
    compare_parser.add_argument("--candidate", required=True)
    compare_parser.add_argument("--metric", default="p50", choices=["p50", "p95", "p99", "max"])
    compare_parser.add_argument("--max-regression-pct", type=float, default=5.0)
    compare_parser.add_argument("--min-runs", type=int, default=3)
    compare_parser.add_argument("--output")
    compare_parser.add_argument(
        "--allow-case-subset",
        action="store_true",
        help="allow a candidate that measures a size/render subset of the baseline; all other settings must match",
    )
    compare_dashboard_parser = subparsers.add_parser(
        "compare-dashboard-modes",
        help="validate and report the three dashboard modes within one session",
    )
    compare_dashboard_parser.add_argument("--session", required=True)
    compare_dashboard_parser.add_argument("--min-runs", type=int, default=3)
    compare_dashboard_parser.add_argument("--output")
    subparsers.add_parser("self-test", help="run stdlib-only validation fixtures")
    return parser


def validate_arguments(args: argparse.Namespace) -> None:
    if args.command not in {"run", "audit"}:
        return
    if args.repeat < 1:
        raise ValueError("--repeat must be at least 1")
    if args.preflight_runs < 0:
        raise ValueError("--preflight-runs cannot be negative")
    if args.seed < 0:
        raise ValueError("--seed cannot be negative")
    if args.command == "run" and (args.warmup_secs < 0 or args.measure_secs <= 0):
        raise ValueError("--warmup-secs must be nonnegative and --measure-secs must be positive")
    if args.timeout_secs <= 0:
        raise ValueError("--timeout-secs must be positive")
    if args.tracy_capture_secs is not None and args.tracy_capture_secs <= 0:
        raise ValueError("--tracy-capture-secs must be positive")
    if args.tracy_capture_secs is not None and args.instrumentation != "tracy":
        raise ValueError("--tracy-capture-secs is only valid with --instrumentation tracy")
    if (
        args.tracy_capture_secs is not None
        and args.instrumentation == "tracy"
        and not args.dry_run
    ):
        raise ValueError(
            "validated Tracy runs must omit --tracy-capture-secs so the runner can "
            "disconnect Tracy at the measure-artifact boundary"
        )
    if args.command == "audit" and args.instrumentation != "capture":
        raise ValueError("fixed-step audit only supports --instrumentation capture")
    if args.instrumentation == "tracy" and not args.dry_run:
        missing_tools = [
            option
            for option, value in (
                ("--tracy-capture-binary", args.tracy_capture_binary),
                ("--tracy-csvexport-binary", args.tracy_csvexport_binary),
            )
            if not value
        ]
        if missing_tools:
            raise ValueError(
                f"--instrumentation {args.instrumentation} requires " + ", ".join(missing_tools)
            )
    if args.command == "audit":
        if args.fixed_hz <= 0:
            raise ValueError("--fixed-hz must be positive")
        if args.warmup_ticks <= DETERMINISM_EARLY_CHECKPOINTS[-1][1]:
            raise ValueError("--warmup-ticks must be greater than 128")
        if args.audit_ticks <= 0:
            raise ValueError("--audit-ticks must be positive")
    familiar_policies = parse_csv_list(
        args.familiar_policies,
        {"baseline", "default", "disabled"},
        "familiar policies",
    )
    operation_dialog_modes = parse_csv_list(
        args.operation_dialog_modes,
        {"hidden", "open"},
        "operation dialog modes",
    )
    dashboard_modes = parse_csv_list(
        args.dashboard_modes,
        {"hidden", "visible", "active-filter"},
        "dashboard modes",
    )
    renders = parse_csv_list(args.renders, {"cpu", "gpu"}, "renders")
    if args.window_backend == "headless" and any(render != "cpu" for render in renders):
        raise ValueError("--window-backend headless only supports --renders cpu")
    uses_controlled_b2_mode = any(
        mode != "baseline" for mode in familiar_policies
    ) or any(mode != "hidden" for mode in operation_dialog_modes)
    if uses_controlled_b2_mode and (
        args.command != "audit" or args.workload != "gather"
    ):
        raise ValueError(
            "controlled familiar policy and operation dialog modes require `audit --workload gather`"
        )
    if any(mode in {"default", "disabled"} for mode in familiar_policies) and (
        args.souls == 0 or args.familiars == 0
    ):
        raise ValueError(
            "controlled familiar policy modes require at least one Soul and one Familiar"
        )
    if any(mode != "hidden" for mode in dashboard_modes) and args.workload != "task-dashboard":
        raise ValueError(
            "visible and active-filter dashboard modes require --workload task-dashboard"
        )
    if args.workload == "task-dashboard" and (
        familiar_policies != ["baseline"] or operation_dialog_modes != ["hidden"]
    ):
        raise ValueError(
            "task-dashboard requires familiar policy baseline and operation dialog hidden"
        )
