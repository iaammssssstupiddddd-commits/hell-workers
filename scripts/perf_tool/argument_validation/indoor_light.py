from __future__ import annotations

import argparse

from ..model import parse_csv_list
from ..rtt_light_contract import load_rtt_light_contract, validate_stage_lane
from .common import ParsedMatrix


def validate_indoor_light(args: argparse.Namespace, matrix: ParsedMatrix) -> None:
    if args.workload != "indoor-light":
        if matrix.selected_rtt_light:
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
        matrix.familiar_policies != ["baseline"]
        or matrix.operation_dialog_modes != ["hidden"]
        or matrix.dashboard_modes != ["hidden"]
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
        if matrix.sizes != ["large"] or matrix.renders != ["cpu"]:
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
        if matrix.sizes != ["small"] or matrix.renders != ["cpu"]:
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
