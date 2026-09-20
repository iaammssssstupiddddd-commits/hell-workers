from __future__ import annotations

import argparse
import math
import os

from .model import (
    DEFAULT_SEED,
    PERF_DESCRIPTION,
)
from .rtt_light_contract import (
    CONTRACT_FILES,
    RTT_LIGHT_LANES,
    RTT_LIGHT_STAGES,
    load_rtt_light_contract,
    validate_stage_lane,
)


from .argument_validation.common import validate_common
from .argument_validation.density import validate_density
from .argument_validation.indoor_light import validate_indoor_light
from .argument_validation.specialized import (
    DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING as DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING,
    validate_specialized,
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
            "building-art-static",
        ],
    )
    parser.add_argument("--contract", choices=sorted(CONTRACT_FILES))
    parser.add_argument("--wall-phase", choices=["completed", "provisional", "mixed"])
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
        "--wall-formwork-acceptance",
        action="store_true",
        help=(
            "authorize the approved provisional Wall formwork gallery; requires "
            "--wall-actual-window and the paired native-acceptance environment"
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
    matrix = validate_common(args)
    if validate_density(args, matrix):
        return
    if validate_specialized(args, matrix):
        return
    validate_indoor_light(args, matrix)
