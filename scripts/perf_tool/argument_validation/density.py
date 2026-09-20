from __future__ import annotations

import argparse

from .common import ParsedMatrix


def validate_density(args: argparse.Namespace, matrix: ParsedMatrix) -> bool:
    if args.workload == "wall-density":
        if args.wall_actual_window and args.wall_color_actual_window:
            raise ValueError("Wall actual-window profiles are mutually exclusive")
        if args.wall_art_matrix and not args.wall_actual_window:
            raise ValueError("--wall-art-matrix requires --wall-actual-window")
        if args.wall_formwork_acceptance and (
            not args.wall_actual_window or not args.wall_art_matrix
        ):
            raise ValueError(
                "--wall-formwork-acceptance requires --wall-actual-window and --wall-art-matrix"
            )
        if args.wall_formwork_acceptance and args.wall_phase != "mixed":
            raise ValueError(
                "--wall-formwork-acceptance requires --wall-phase mixed"
            )
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
        if matrix.selected_rtt_light:
            raise ValueError("wall-density does not accept an RtT-light contract selection")
        if args.wall_phase not in {"completed", "provisional", "mixed"}:
            raise ValueError(
                "wall-density requires --wall-phase completed|provisional|mixed"
            )
        if args.wall_phase == "mixed" and (
            (wall_actual_window and not args.wall_formwork_acceptance)
            or (not wall_actual_window and args.wall_presentation is None)
        ):
            raise ValueError(
                "wall-density mixed requires the formal --wall-presentation path"
            )
        if wall_actual_window:
            expected_actual_size = (
                ["medium"] if args.wall_formwork_acceptance else ["small"]
            )
            if matrix.sizes != expected_actual_size or matrix.renders != ["gpu"]:
                raise ValueError(
                    "wall-density actual-window requires its profile size and --renders gpu"
                )
        elif args.wall_phase == "mixed" and (
            matrix.sizes != ["medium"] or matrix.renders != ["gpu"]
        ):
            raise ValueError(
                "wall-density mixed requires --sizes medium --renders gpu"
            )
        elif args.wall_phase != "mixed" and (
            matrix.sizes != ["small", "medium"] or matrix.renders != ["gpu"]
        ):
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
        if args.instrumentation != "capture" and not (
            args.instrumentation == "memory"
            and args.wall_phase == "mixed"
            and args.wall_presentation is not None
        ):
            raise ValueError(
                "wall-density requires Capture, except formal mixed Memory runs"
            )
        if (
            matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "wall-density requires familiar policy baseline, operation dialog hidden, "
                "and dashboard hidden"
            )
        expected_souls = 2 if args.wall_formwork_acceptance else 0
        if args.souls != expected_souls or args.familiars != 0:
            raise ValueError(
                f"wall-density requires --souls {expected_souls} --familiars 0"
            )
        return True
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
    if args.wall_formwork_acceptance:
        raise ValueError(
            "--wall-formwork-acceptance is reserved for --workload wall-density"
        )
    if args.wall_art_zoom != "standard":
        raise ValueError("--wall-art-zoom is reserved for --workload wall-density")
    if args.workload == "door-density":
        if args.command != "run":
            raise ValueError("door-density is only available through perf.py run")
        if matrix.selected_rtt_light:
            raise ValueError("door-density does not accept an RtT-light contract selection")
        if args.door_presentation not in {"production", "fallback-control"}:
            raise ValueError("door-density requires --door-presentation")
        expected_sizes = ["small", "medium"] if args.instrumentation == "capture" else ["medium"]
        if matrix.sizes != expected_sizes or matrix.renders != ["gpu"]:
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
            matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"]
            or args.souls != 0
            or args.familiars != 0
        ):
            raise ValueError("door-density requires zero actors and baseline hidden UI policies")
        return True
    if args.door_presentation is not None:
        raise ValueError("--door-presentation is reserved for --workload door-density")
    if args.workload == "building-art-static":
        if (
            args.command != "run" or matrix.selected_rtt_light
            or matrix.sizes not in (["small"], ["medium"])
            or matrix.renders != ["gpu"]
            or args.instrumentation not in {"capture", "memory"}
            or args.seed != 20_260_920 or args.repeat != 3 or args.preflight_runs != 0
            or args.warmup_secs != 30.0 or args.measure_secs != 60.0
            or args.window_backend != "x11" or args.backend != "vulkan"
            or args.present_mode != "novsync"
            or args.window_width != 1280 or args.window_height != 720
            or args.window_scale_factor != 1.0 or args.rtt_quality != "high"
            or matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"] or args.familiars != 0
            or args.souls != (15 if matrix.sizes == ["small"] else 60)
        ):
            raise ValueError("building-art-static requires one size (small/15 Souls or medium/60 Souls), zero Familiars, GPU Capture|Memory, seed 20260920, three 30/60s runs without preflight, X11/Vulkan/novsync/1280x720/high/DPI-1 and baseline hidden UI")
        return True
    return False
