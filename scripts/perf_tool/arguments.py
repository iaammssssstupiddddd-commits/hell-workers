from __future__ import annotations

from .model import *
from .rtt_light_contract import *

def add_run_arguments(
    parser: argparse.ArgumentParser,
    *,
    fixed_step_audit: bool = False,
    fixed_step_behavior: bool = False,
) -> None:
    if fixed_step_audit and fixed_step_behavior:
        raise ValueError("a runner cannot be both audit and behavior")
    parser.add_argument(
        "--workload",
        default="gather",
        choices=[
            "gather",
            "path-door",
            "construction",
            "ui-gpu",
            "task-dashboard",
            "indoor-light",
        ],
    )
    parser.add_argument("--contract", choices=sorted(CONTRACT_FILES))
    parser.add_argument("--stage", choices=RTT_LIGHT_STAGES)
    parser.add_argument("--lane", choices=RTT_LIGHT_LANES)
    parser.add_argument("--sizes", default="medium", help="comma-separated: small,medium,large")
    parser.add_argument("--renders", default="cpu", help="comma-separated: cpu,gpu")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--preflight-runs", type=int, default=0)
    parser.add_argument("--souls", type=int)
    parser.add_argument("--familiars", type=int)
    parser.add_argument("--output", help="new artifact directory, relative to the repository when not absolute")
    parser.add_argument(
        "--environment-lock",
        help=(
            "generation-level RtT-light environment lock; Capture creates it from "
            "the first valid preflight and Memory requires an exact match"
        ),
    )
    parser.add_argument("--adapter", help="required substring of the actual WGPU adapter name")
    parser.add_argument("--backend", default="auto", choices=["auto", "vulkan", "gl", "dx12", "metal"])
    parser.add_argument(
        "--window-backend",
        default="auto",
        choices=["auto", "wayland", "x11", "headless"],
        help="window backend; headless omits Winit, the primary window, and display sockets for CPU-only audits",
    )
    parser.add_argument(
        "--present-mode",
        default="novsync",
        choices=["novsync", "fifo", "auto_vsync", "mailbox", "immediate"],
    )
    parser.add_argument("--window-width", type=int, help="requested physical primary-window width")
    parser.add_argument("--window-height", type=int, help="requested physical primary-window height")
    parser.add_argument(
        "--window-scale-factor",
        type=float,
        help="requested primary-window scale-factor override",
    )
    parser.add_argument("--rtt-quality", choices=["high", "medium", "low"])
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
    if fixed_step_audit or fixed_step_behavior:
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
        if fixed_step_behavior:
            parser.add_argument(
                "--behavior-cases",
                default="door-state-v1,load-normal-v1",
                help="comma-separated contract behavior cases",
            )
            parser.set_defaults(
                capture_kind="fixed-step-behavior",
                clock_mode="fixed-behavior",
            )
        else:
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
    behavior_parser = subparsers.add_parser(
        "behavior",
        help="run the exact fixed-step RtT-light current behavior contract",
    )
    add_run_arguments(behavior_parser, fixed_step_behavior=True)
    behavior_parser.set_defaults(
        workload="indoor-light",
        contract="rtt-light-v1",
        stage="current",
        lane="behavior",
        sizes="small",
        renders="cpu",
        backend="vulkan",
        window_backend="headless",
        present_mode="novsync",
        seed=20_260_803,
        repeat=3,
        preflight_runs=0,
        instrumentation="capture",
    )
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
    contract_parser = subparsers.add_parser(
        "validate-rtt-light-contract",
        help="validate a canonical RtT-light contract and print its stable fingerprints",
    )
    contract_parser.add_argument("--contract", required=True, choices=sorted(CONTRACT_FILES))
    contract_parser.add_argument("--stage", choices=RTT_LIGHT_STAGES)
    contract_parser.add_argument("--lane", choices=RTT_LIGHT_LANES)
    contract_parser.add_argument("--output")
    finalize_attempt_parser = subparsers.add_parser(
        "finalize-rtt-light-attempt",
        help="revalidate, assemble, and register one completed formal RtT-light attempt",
    )
    finalize_attempt_parser.add_argument("--attempt", required=True)
    verify_attempt_parser = subparsers.add_parser(
        "verify-rtt-light-attempt",
        help="revalidate one registered formal RtT-light attempt without changing it",
    )
    verify_attempt_parser.add_argument("--attempt", required=True)
    verify_baseline_parser = subparsers.add_parser(
        "verify-rtt-light-baseline",
        help="revalidate every registered RtT-light baseline stage and locator",
    )
    verify_baseline_parser.add_argument("--baseline", required=True)
    subparsers.add_parser("self-test", help="run stdlib-only validation fixtures")
    return parser


def validate_arguments(args: argparse.Namespace) -> None:
    if args.command == "validate-rtt-light-contract":
        if (args.stage is None) != (args.lane is None):
            raise ValueError("--stage and --lane must be provided together")
        contract = load_rtt_light_contract(args.contract)
        if args.stage is not None:
            validate_stage_lane(contract, args.stage, args.lane)
        return
    if args.command == "compare":
        if args.min_runs < 1:
            raise ValueError("--min-runs must be at least 1")
        if not math.isfinite(args.max_regression_pct) or args.max_regression_pct < 0:
            raise ValueError("--max-regression-pct must be finite and nonnegative")
        return
    if args.command == "compare-dashboard-modes":
        if args.min_runs < 1:
            raise ValueError("--min-runs must be at least 1")
        return
    if args.command not in {"run", "audit", "behavior"}:
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
    if (args.window_width is None) != (args.window_height is None):
        raise ValueError("--window-width and --window-height must be provided together")
    if args.window_width is not None and (args.window_width <= 0 or args.window_height <= 0):
        raise ValueError("--window-width and --window-height must be positive")
    if args.window_scale_factor is not None and (
        not math.isfinite(args.window_scale_factor) or args.window_scale_factor <= 0
    ):
        raise ValueError("--window-scale-factor must be finite and positive")
    if args.window_backend == "headless" and (
        args.window_width is not None or args.window_scale_factor is not None
    ):
        raise ValueError(
            "--window-width/--window-height and --window-scale-factor are not applicable "
            "with --window-backend headless"
        )
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
    if args.command in {"audit", "behavior"} and args.instrumentation != "capture":
        raise ValueError("fixed-step audit and behavior only support --instrumentation capture")
    if args.environment_lock is not None and not (
        args.command == "run"
        and args.workload == "indoor-light"
        and args.contract == "rtt-light-v1"
        and args.lane == "static"
        and args.instrumentation in {"capture", "memory"}
    ):
        raise ValueError(
            "--environment-lock requires an indoor-light static Capture or Memory run"
        )
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
    if args.command in {"audit", "behavior"}:
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
    sizes = parse_csv_list(args.sizes, {"small", "medium", "large"}, "sizes")
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
    selected_rtt_light = args.contract is not None or args.stage is not None or args.lane is not None
    if args.workload != "indoor-light":
        if selected_rtt_light:
            raise ValueError(
                "--contract, --stage, and --lane are reserved for --workload indoor-light"
            )
        return

    expected_lane = "behavior" if args.command == "behavior" else "static"
    if (args.contract, args.stage, args.lane) != (
        "rtt-light-v1",
        "current",
        expected_lane,
    ):
        raise ValueError(
            "--workload indoor-light currently requires --contract rtt-light-v1 "
            f"--stage current --lane {expected_lane}"
        )
    contract = load_rtt_light_contract(args.contract)
    validate_stage_lane(contract, args.stage, args.lane)
    if args.souls is not None or args.familiars is not None:
        raise ValueError("indoor-light uses the exact contract population; overrides are forbidden")
    if (
        familiar_policies != ["baseline"]
        or operation_dialog_modes != ["hidden"]
        or dashboard_modes != ["hidden"]
    ):
        raise ValueError(
            "indoor-light requires familiar policy baseline, operation dialog hidden, "
            "and dashboard hidden"
        )
    if args.seed != contract["formal_matrix"]["seed"]:
        raise ValueError(
            f"indoor-light rtt-light-v1 requires --seed {contract['formal_matrix']['seed']}"
        )
    if args.command == "behavior":
        behavior_cases = parse_csv_list(
            args.behavior_cases,
            {case["case_id"] for case in contract["behavior_cases"]},
            "behavior cases",
        )
        expected_cases = contract["stages"][args.stage]["required_behavior_cases"]
        if behavior_cases != expected_cases:
            raise ValueError(
                "current behavior requires the exact ordered cases: "
                + ",".join(expected_cases)
            )
        if sizes != ["small"] or renders != ["cpu"]:
            raise ValueError("current behavior requires --sizes small --renders cpu")
        if args.window_backend != "headless":
            raise ValueError("current behavior requires --window-backend headless")
        if args.backend != contract["formal_matrix"]["backend"]:
            raise ValueError(
                f"current behavior requires --backend {contract['formal_matrix']['backend']}"
            )
        if args.present_mode != contract["formal_matrix"]["present_mode"]:
            raise ValueError(
                "current behavior requires --present-mode "
                + contract["formal_matrix"]["present_mode"]
            )
        if (
            args.repeat != contract["behavior_fixture"]["repeat"]
            or args.preflight_runs
            != contract["formal_matrix"]["behavior"]["preflight_runs"]
        ):
            raise ValueError("current behavior requires --repeat 3 --preflight-runs 0")
        if args.fixed_hz != contract["formal_matrix"]["fixed_hz"]:
            raise ValueError(
                f"current behavior requires --fixed-hz {contract['formal_matrix']['fixed_hz']}"
            )
