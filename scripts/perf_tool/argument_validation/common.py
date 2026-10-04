from __future__ import annotations

import argparse
import math
from dataclasses import dataclass

from ..model import DETERMINISM_EARLY_CHECKPOINTS, parse_csv_list


@dataclass(frozen=True)
class ParsedMatrix:
    familiar_policies: list[str]
    operation_dialog_modes: list[str]
    dashboard_modes: list[str]
    renders: list[str]
    sizes: list[str]
    selected_rtt_light: bool


def validate_common(args: argparse.Namespace) -> ParsedMatrix:
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
    return ParsedMatrix(familiar_policies, operation_dialog_modes, dashboard_modes, renders, sizes, selected_rtt_light)
