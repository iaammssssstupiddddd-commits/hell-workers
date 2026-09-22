from __future__ import annotations

import argparse

from .common import ParsedMatrix


# A CPU-only headless audit deliberately has no hardware renderer. Bevy emits
# this warning before the deterministic fixture begins; it is expected only
# for the fixed deconstruction workload and must not hide other log failures.
DECONSTRUCTION_HEADLESS_SOFTWARE_RENDERING_WARNING = (
    r"selected adapter is using a driver that only supports software rendering"
)


def validate_specialized(args: argparse.Namespace, matrix: ParsedMatrix) -> bool:
    if args.workload == "dream-ui-burst":
        if args.command not in {"run", "audit"}:
            raise ValueError("dream-ui-burst is available through perf.py run or audit")
        if matrix.selected_rtt_light:
            raise ValueError("dream-ui-burst does not accept an RtT-light contract selection")
        if matrix.sizes != ["small"] or matrix.renders != ["cpu"]:
            raise ValueError("dream-ui-burst requires --sizes small --renders cpu")
        if args.window_backend == "headless":
            raise ValueError("dream-ui-burst requires an actual window backend")
        if args.instrumentation not in {"capture", "memory"}:
            raise ValueError("dream-ui-burst requires --instrumentation capture or memory")
        if (
            matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"]
        ):
            raise ValueError(
                "dream-ui-burst requires familiar policy baseline, operation dialog hidden, and dashboard hidden"
            )
        if args.souls is not None or args.familiars is not None:
            raise ValueError("dream-ui-burst uses the default small population; overrides are forbidden")
        if args.save_runtime_root is not None:
            raise ValueError("--save-runtime-root is only valid for save-transaction")
        return True
    if args.workload == "save-transaction":
        if args.command != "run":
            raise ValueError("save-transaction is only available through perf.py run")
        if matrix.selected_rtt_light:
            raise ValueError("save-transaction does not accept an RtT-light contract selection")
        if args.window_backend != "headless":
            raise ValueError("save-transaction requires --window-backend headless")
        if args.instrumentation not in {"capture", "memory"}:
            raise ValueError("save-transaction requires --instrumentation capture or memory")
        if matrix.sizes != ["small", "medium", "large"] or matrix.renders != ["cpu"]:
            raise ValueError(
                "save-transaction requires --sizes small,medium,large --renders cpu"
            )
        if (
            matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"]
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
        return True
    if args.save_runtime_root is not None:
        raise ValueError("--save-runtime-root is only valid for save-transaction")
    if args.workload == "deconstruction":
        if args.command != "audit":
            raise ValueError("deconstruction is only available through the fixed-step audit")
        if matrix.selected_rtt_light:
            raise ValueError("deconstruction does not accept an RtT-light contract selection")
        if matrix.sizes != ["medium"] or matrix.renders != ["cpu"]:
            raise ValueError("deconstruction requires --sizes medium --renders cpu")
        if args.window_backend != "headless":
            raise ValueError("deconstruction requires --window-backend headless")
        if (
            matrix.familiar_policies != ["baseline"]
            or matrix.operation_dialog_modes != ["hidden"]
            or matrix.dashboard_modes != ["hidden"]
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
        return True
    return False
