from __future__ import annotations

import argparse
import math
import os

from .model import (
    DEFAULT_SEED,
    DETERMINISM_EARLY_CHECKPOINTS,
    PERF_DESCRIPTION,
    parse_csv_list,
)
from .rtt_light_contract import (
    CONTRACT_FILES,
    RTT_LIGHT_LANES,
    RTT_LIGHT_STAGES,
    load_rtt_light_contract,
    validate_stage_lane,
)


# A CPU-only headless audit deliberately has no hardware renderer. Bevy emits
# this warning before the deterministic fixture begins; it is expected only
# for the fixed deconstruction workload and must not hide other log failures.
DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING = (
    r"selected adapter is using a driver that only supports software rendering"
)


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
            "dream-ui-burst",
            "indoor-light",
            "deconstruction",
            "save-transaction",
            "wall-density",
            "door-density",
        ],
    )
    parser.add_argument("--contract", choices=sorted(CONTRACT_FILES))
    parser.add_argument("--wall-phase", choices=["completed", "provisional"])
    parser.add_argument(
        "--wall-presentation",
        choices=["production", "fallback-control"],
        help=(
            "select the M5 formal Wall presentation; requires the paired "
            "HW_WALL_PERF_PRESENTATION environment owned by native acceptance"
        ),
    )
    parser.add_argument(
        "--door-presentation",
        choices=["production", "fallback-control"],
        help=(
            "select the formal Door-density presentation; requires the paired "
            "HW_DOOR_PERF_PRESENTATION environment owned by native acceptance"
        ),
    )
    parser.add_argument(
        "--wall-actual-window",
        action="store_true",
        help=(
            "run the single-case current-Wall actual-window calibration; reserved "
            "for the fail-closed native acceptance launcher"
        ),
    )
    parser.add_argument(
        "--wall-color-actual-window",
        action="store_true",
        help=(
            "run the single-case Wall five-patch color calibration; reserved "
            "for the fail-closed native acceptance launcher"
        ),
    )
    parser.add_argument(
        "--wall-art-matrix",
        action="store_true",
        help=(
            "authorize the Wall art High/Medium/Low x DPI 1.0/1.5/2.0 matrix; "
            "requires --wall-actual-window and the fail-closed native acceptance launcher"
        ),
    )
    parser.add_argument(
        "--wall-art-zoom",
        choices=("standard", "farthest"),
        default="standard",
        help=(
            "select the Wall art gallery zoom; farthest matches the player's "
            "widest zoom-out and requires --wall-art-matrix"
        ),
    )
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
    parser.add_argument("--instrumentation", default="capture", choices=["capture", "tracy", "memory", "renderdoc"])
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
    parser.add_argument(
        "--save-runtime-root",
        help=(
            "fresh disk-backed parent for save-transaction runtime data; "
            "each operation removes its exact body-containing child before its "
            "scalar evidence is retained"
        ),
    )
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
                default=None,
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
    field_core_parser = subparsers.add_parser(
        "field-core",
        help="run the exact P03 pure indoor-light field rebuild contract",
    )
    add_run_arguments(field_core_parser, fixed_step_audit=True)
    field_core_parser.set_defaults(
        workload="indoor-light",
        contract="rtt-light-v1",
        stage="p03",
        lane="field-core",
        sizes="large",
        renders="cpu",
        backend="vulkan",
        window_backend="headless",
        present_mode="novsync",
        seed=20_260_803,
        repeat=3,
        preflight_runs=0,
        instrumentation="capture",
        capture_kind="field-core",
        clock_mode="fixed",
        allow_log_pattern=[],
    )
    consumer_core_parser = subparsers.add_parser(
        "consumer-core",
        help="run the exact P07/P08 indoor-light gameplay and Room consumer contract",
    )
    add_run_arguments(consumer_core_parser, fixed_step_audit=True)
    consumer_core_parser.set_defaults(
        workload="indoor-light",
        contract="rtt-light-v1",
        stage="p07",
        lane="consumer-core",
        sizes="large",
        renders="cpu",
        backend="vulkan",
        window_backend="headless",
        present_mode="novsync",
        seed=20_260_803,
        repeat=3,
        preflight_runs=0,
        instrumentation="capture",
        capture_kind="consumer-core",
        clock_mode="fixed",
        allow_log_pattern=[],
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
    if args.command not in {"run", "audit", "behavior", "field-core", "consumer-core"}:
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
    if args.command in {"audit", "behavior", "field-core", "consumer-core"} and args.instrumentation != "capture":
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
    if args.command in {"audit", "behavior", "field-core", "consumer-core"}:
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
    if args.workload == "wall-density":
        if args.wall_actual_window and args.wall_color_actual_window:
            raise ValueError("Wall actual-window profiles are mutually exclusive")
        if args.wall_art_matrix and not args.wall_actual_window:
            raise ValueError("--wall-art-matrix requires --wall-actual-window")
        if args.wall_art_zoom != "standard" and not args.wall_art_matrix:
            raise ValueError("--wall-art-zoom farthest requires --wall-art-matrix")
        wall_actual_window = args.wall_actual_window or args.wall_color_actual_window
        if args.wall_presentation is not None and (
            wall_actual_window or args.wall_art_matrix
        ):
            raise ValueError(
                "--wall-presentation is reserved for formal wall-density runs"
            )
        if args.command != "run":
            raise ValueError("wall-density is only available through perf.py run")
        if selected_rtt_light:
            raise ValueError("wall-density does not accept an RtT-light contract selection")
        if args.wall_phase not in {"completed", "provisional"}:
            raise ValueError("wall-density requires --wall-phase completed|provisional")
        if wall_actual_window:
            if sizes != ["small"] or renders != ["gpu"]:
                raise ValueError(
                    "wall-density actual-window requires --sizes small --renders gpu"
                )
        elif sizes != ["small", "medium"] or renders != ["gpu"]:
            raise ValueError("wall-density requires --sizes small,medium --renders gpu")
        if args.seed != 20_260_901:
            raise ValueError("wall-density requires --seed 20260901")
        expected_repeat = 1 if wall_actual_window else 3
        if args.repeat != expected_repeat or args.preflight_runs != 0:
            raise ValueError(
                f"wall-density requires --repeat {expected_repeat} --preflight-runs 0"
            )
        expected_warmup = 10.0 if wall_actual_window else 30.0
        expected_measure = 10.0 if wall_actual_window else 60.0
        if args.warmup_secs != expected_warmup or args.measure_secs != expected_measure:
            raise ValueError(
                "wall-density requires "
                f"--warmup-secs {expected_warmup:g} "
                f"--measure-secs {expected_measure:g}"
            )
        if args.window_backend != "x11":
            raise ValueError("wall-density requires --window-backend x11")
        if args.backend != "vulkan" or args.present_mode != "novsync":
            raise ValueError("wall-density requires --backend vulkan --present-mode novsync")
        if args.window_width != 1280 or args.window_height != 720:
            raise ValueError(
                "wall-density requires a 1280x720 physical window"
            )
        if args.wall_art_matrix:
            if args.window_scale_factor not in {1.0, 1.5, 2.0} or args.rtt_quality not in {
                "high",
                "medium",
                "low",
            }:
                raise ValueError(
                    "wall-density Wall art matrix requires scale factor 1.0|1.5|2.0 "
                    "and RtT quality high|medium|low"
                )
        elif args.window_scale_factor != 1.0 or args.rtt_quality != "high":
            raise ValueError(
                "wall-density requires scale factor 1.0 and RtT quality high"
            )
        if args.instrumentation != "capture":
            raise ValueError("wall-density frame-time runs require --instrumentation capture")
        if (
            familiar_policies != ["baseline"]
            or operation_dialog_modes != ["hidden"]
            or dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "wall-density requires familiar policy baseline, operation dialog hidden, "
                "and dashboard hidden"
            )
        if args.souls != 0 or args.familiars != 0:
            raise ValueError("wall-density requires --souls 0 --familiars 0")
        return
    if args.wall_phase is not None:
        raise ValueError("--wall-phase is reserved for --workload wall-density")
    if args.wall_presentation is not None:
        raise ValueError("--wall-presentation is reserved for --workload wall-density")
    if args.wall_actual_window:
        raise ValueError("--wall-actual-window is reserved for --workload wall-density")
    if args.wall_color_actual_window:
        raise ValueError("--wall-color-actual-window is reserved for --workload wall-density")
    if args.wall_art_matrix:
        raise ValueError("--wall-art-matrix is reserved for --workload wall-density")
    if args.wall_art_zoom != "standard":
        raise ValueError("--wall-art-zoom is reserved for --workload wall-density")
    if args.workload == "door-density":
        if args.command != "run":
            raise ValueError("door-density is only available through perf.py run")
        if selected_rtt_light:
            raise ValueError("door-density does not accept an RtT-light contract selection")
        if args.door_presentation not in {"production", "fallback-control"}:
            raise ValueError("door-density requires --door-presentation")
        expected_sizes = ["small", "medium"] if args.instrumentation == "capture" else ["medium"]
        if sizes != expected_sizes or renders != ["gpu"]:
            raise ValueError(
                "door-density requires --sizes small,medium for Capture or medium for Memory, and --renders gpu"
            )
        if args.instrumentation not in {"capture", "memory"}:
            raise ValueError("door-density requires --instrumentation capture or memory")
        if args.seed != 20_260_906:
            raise ValueError("door-density requires --seed 20260906")
        if args.repeat != 3 or args.preflight_runs != 0:
            raise ValueError("door-density requires --repeat 3 --preflight-runs 0")
        if args.warmup_secs != 30.0 or args.measure_secs != 60.0:
            raise ValueError("door-density requires --warmup-secs 30 --measure-secs 60")
        if args.window_backend != "x11":
            raise ValueError("door-density requires --window-backend x11")
        if args.backend != "vulkan" or args.present_mode != "novsync":
            raise ValueError("door-density requires --backend vulkan --present-mode novsync")
        if args.window_width != 1280 or args.window_height != 720:
            raise ValueError("door-density requires a 1280x720 physical window")
        if args.window_scale_factor != 1.0 or args.rtt_quality != "high":
            raise ValueError("door-density requires scale factor 1.0 and RtT quality high")
        if (
            familiar_policies != ["baseline"]
            or operation_dialog_modes != ["hidden"]
            or dashboard_modes != ["hidden"]
            or args.souls != 0
            or args.familiars != 0
        ):
            raise ValueError("door-density requires zero actors and baseline hidden UI policies")
        return
    if args.door_presentation is not None:
        raise ValueError("--door-presentation is reserved for --workload door-density")
    if args.workload == "dream-ui-burst":
        if args.command not in {"run", "audit"}:
            raise ValueError("dream-ui-burst is available through perf.py run or audit")
        if selected_rtt_light:
            raise ValueError("dream-ui-burst does not accept an RtT-light contract selection")
        if sizes != ["small"] or renders != ["cpu"]:
            raise ValueError("dream-ui-burst requires --sizes small --renders cpu")
        if args.window_backend == "headless":
            raise ValueError("dream-ui-burst requires an actual window backend")
        if args.instrumentation not in {"capture", "memory"}:
            raise ValueError("dream-ui-burst requires --instrumentation capture or memory")
        if (
            familiar_policies != ["baseline"]
            or operation_dialog_modes != ["hidden"]
            or dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "dream-ui-burst requires familiar policy baseline, operation dialog hidden, and dashboard hidden"
            )
        if args.souls is not None or args.familiars is not None:
            raise ValueError("dream-ui-burst uses the default small population; overrides are forbidden")
        if args.save_runtime_root is not None:
            raise ValueError("--save-runtime-root is only valid for save-transaction")
        return
    if args.workload == "save-transaction":
        if args.command != "run":
            raise ValueError("save-transaction is only available through perf.py run")
        if selected_rtt_light:
            raise ValueError("save-transaction does not accept an RtT-light contract selection")
        if args.window_backend != "headless":
            raise ValueError("save-transaction requires --window-backend headless")
        if args.instrumentation not in {"capture", "memory"}:
            raise ValueError("save-transaction requires --instrumentation capture or memory")
        if sizes != ["small", "medium", "large"] or renders != ["cpu"]:
            raise ValueError(
                "save-transaction requires --sizes small,medium,large --renders cpu"
            )
        if (
            familiar_policies != ["baseline"]
            or operation_dialog_modes != ["hidden"]
            or dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "save-transaction requires familiar policy baseline, operation dialog hidden, and dashboard hidden"
            )
        if args.souls is not None or args.familiars is not None:
            raise ValueError(
                "save-transaction uses the gather population for the selected size; overrides are forbidden"
            )
        if args.repeat != 20 or args.preflight_runs != 3:
            raise ValueError(
                "save-transaction requires exactly --repeat 20 and --preflight-runs 3"
            )
        if args.warmup_secs != 1.0 or args.measure_secs != 2.0:
            raise ValueError(
                "save-transaction requires --warmup-secs 1 --measure-secs 2"
            )
        if args.binary is not None or args.skip_build:
            raise ValueError(
                "save-transaction must build and run its canonical profiling binary; "
                "--binary and --skip-build are forbidden"
            )
        return
    if args.save_runtime_root is not None:
        raise ValueError("--save-runtime-root is only valid for save-transaction")
    if args.workload == "deconstruction":
        if args.command != "audit":
            raise ValueError("deconstruction is only available through the fixed-step audit")
        if selected_rtt_light:
            raise ValueError("deconstruction does not accept an RtT-light contract selection")
        if sizes != ["medium"] or renders != ["cpu"]:
            raise ValueError("deconstruction requires --sizes medium --renders cpu")
        if args.window_backend != "headless":
            raise ValueError("deconstruction requires --window-backend headless")
        if (
            familiar_policies != ["baseline"]
            or operation_dialog_modes != ["hidden"]
            or dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "deconstruction requires familiar policy baseline, operation dialog hidden, and dashboard hidden"
            )
        if args.souls is not None or args.familiars is not None:
            raise ValueError("deconstruction uses the fixed medium population; overrides are forbidden")
        if args.allow_log_pattern not in (
            [],
            [DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING],
        ):
            raise ValueError(
                "deconstruction uses its fixed headless software-renderer allowance; "
                "custom --allow-log-pattern is forbidden"
            )
        args.allow_log_pattern = [DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING]
        return
    if args.workload != "indoor-light":
        if selected_rtt_light:
            raise ValueError(
                "--contract, --stage, and --lane are reserved for --workload indoor-light"
            )
        return

    expected_lane = (
        "behavior"
        if args.command == "behavior"
        else "field-core"
        if args.command == "field-core"
        else "consumer-core"
        if args.command == "consumer-core"
        else "static"
    )
    expected_stages = (
        {"p03", "p04", "p05", "p06", "p07", "p08"}
        if args.command == "field-core"
        else {"p07", "p08"}
        if args.command == "consumer-core"
        else {"current", "p01", "p02", "p03", "p04", "p05", "p06", "p07", "p08"}
    )
    if (
        args.contract != "rtt-light-v1"
        or args.stage not in expected_stages
        or args.lane != expected_lane
    ):
        raise ValueError(
            "--workload indoor-light currently requires --contract rtt-light-v1 "
            f"--stage {'p03|p04|p05|p06|p07|p08' if args.command == 'field-core' else 'p07|p08' if args.command == 'consumer-core' else 'current|p01|p02|p03|p04|p05|p06|p07|p08'} --lane {expected_lane}"
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
    if args.command in {"field-core", "consumer-core"}:
        command_name = args.command
        if sizes != ["large"] or renders != ["cpu"]:
            raise ValueError(f"{command_name} requires --sizes large --renders cpu")
        if args.window_backend != "headless":
            raise ValueError(f"{command_name} requires --window-backend headless")
        if args.backend != contract["formal_matrix"]["backend"]:
            raise ValueError(
                f"{command_name} requires --backend {contract['formal_matrix']['backend']}"
            )
        if args.present_mode != contract["formal_matrix"]["present_mode"]:
            raise ValueError(
                f"{command_name} requires --present-mode "
                + contract["formal_matrix"]["present_mode"]
            )
        if args.repeat != 3 or args.preflight_runs != 0:
            raise ValueError(f"{command_name} requires --repeat 3 --preflight-runs 0")
        if args.fixed_hz != contract["formal_matrix"]["fixed_hz"]:
            raise ValueError(
                f"{command_name} requires --fixed-hz {contract['formal_matrix']['fixed_hz']}"
            )
        expected_allow_patterns = contract["allow_log_patterns"]["headless_audit"]
        if args.allow_log_pattern not in ([], expected_allow_patterns):
            raise ValueError(
                f"{command_name} uses the exact contract headless log allowances; "
                "custom --allow-log-pattern is forbidden"
            )
        args.allow_log_pattern = list(expected_allow_patterns)
        return
    if args.command == "behavior":
        expected_cases = contract["stages"][args.stage]["required_behavior_cases"]
        if args.behavior_cases is None:
            args.behavior_cases = ",".join(expected_cases)
        behavior_cases = parse_csv_list(
            args.behavior_cases,
            {case["case_id"] for case in contract["behavior_cases"]},
            "behavior cases",
        )
        if behavior_cases != expected_cases:
            raise ValueError(
                f"{args.stage} behavior requires the exact ordered cases: "
                + ",".join(expected_cases)
            )
        if sizes != ["small"] or renders != ["cpu"]:
            raise ValueError(f"{args.stage} behavior requires --sizes small --renders cpu")
        if args.window_backend != "headless":
            raise ValueError(f"{args.stage} behavior requires --window-backend headless")
        if args.backend != contract["formal_matrix"]["backend"]:
            raise ValueError(
                f"{args.stage} behavior requires --backend {contract['formal_matrix']['backend']}"
            )
        if args.present_mode != contract["formal_matrix"]["present_mode"]:
            raise ValueError(
                f"{args.stage} behavior requires --present-mode "
                + contract["formal_matrix"]["present_mode"]
            )
        expected_allow_patterns = contract["allow_log_patterns"]["headless_audit"]
        if args.allow_log_pattern not in ([], expected_allow_patterns):
            raise ValueError(
                "behavior uses the exact contract headless log allowances; "
                "custom --allow-log-pattern is forbidden"
            )
        args.allow_log_pattern = list(expected_allow_patterns)
        if (
            args.repeat != contract["behavior_fixture"]["repeat"]
            or args.preflight_runs
            != contract["formal_matrix"]["behavior"]["preflight_runs"]
        ):
            raise ValueError(f"{args.stage} behavior requires --repeat 3 --preflight-runs 0")
        if args.fixed_hz != contract["formal_matrix"]["fixed_hz"]:
            raise ValueError(
                f"{args.stage} behavior requires --fixed-hz {contract['formal_matrix']['fixed_hz']}"
            )
