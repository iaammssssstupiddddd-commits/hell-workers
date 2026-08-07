from __future__ import annotations

from .fixtures import *
from .rtt_light_bundle import finalize_attempt, verify_attempt, verify_baseline

try:
    from scripts.build_coordination import acquire_activity
except ModuleNotFoundError:
    try:
        from build_coordination import acquire_activity
    except ModuleNotFoundError:
        from ..build_coordination import acquire_activity


def validate_rtt_light_contract_command(args: argparse.Namespace) -> int:
    contract = load_rtt_light_contract(args.contract)
    report = {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "lifecycle": contract["lifecycle"],
        **contract_fingerprints(contract),
        "stage": args.stage,
        "lane": args.lane,
        "fixtures": {
            size: {
                "layout_checksum": layout["layout_checksum"],
                "counts": layout["counts"],
            }
            for size in ("small", "medium", "large")
            for layout in [build_fixture_layout(contract, size)]
        },
    }
    if args.output:
        write_json(Path(args.output).resolve(), report)
    else:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0

def run_suite(args: argparse.Namespace) -> int:
    """Run a performance recipe under the workspace-wide exclusive lease."""
    if args.dry_run:
        return _run_suite(args)
    with acquire_activity(REPO_ROOT, "exclusive"):
        return _run_suite(args)


def _run_suite(args: argparse.Namespace) -> int:
    sizes = parse_csv_list(args.sizes, {"small", "medium", "large"}, "sizes")
    renders = parse_csv_list(args.renders, {"cpu", "gpu"}, "renders")
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
    behavior_cases: list[str | None] = (
        parse_csv_list(
            args.behavior_cases,
            {
                case["case_id"]
                for case in load_rtt_light_contract(args.contract)["behavior_cases"]
            },
            "behavior cases",
        )
        if args.command == "behavior"
        else [None]
    )
    if (args.souls is None) != (args.familiars is None):
        raise ValueError("--souls and --familiars must be provided together")
    cases = [
        Case(
            args.workload,
            size,
            render,
            args.seed,
            args.souls,
            args.familiars,
            familiar_policy,
            operation_dialog,
            dashboard_mode,
            behavior_case,
        )
        for size in sizes
        for render in renders
        for familiar_policy in familiar_policies
        for operation_dialog in operation_dialog_modes
        for dashboard_mode in dashboard_modes
        for behavior_case in behavior_cases
    ]
    if args.dry_run:
        binary = Path(args.binary or "target/profiling/bevy_app")
        rtt_selection = (
            f" --perf-contract {args.contract} --perf-stage {args.stage} --perf-lane {args.lane}"
            if args.contract is not None
            else ""
        )
        for case in cases:
            print(
                f"{case.identifier}: {binary} --perf-scenario --perf-seed {case.seed} "
                f"--perf-clock {args.clock_mode} --perf-familiar-policy "
                f"{case.familiar_policy} --perf-operation-dialog {case.operation_dialog} ..."
                f" --perf-dashboard {case.dashboard_mode}{rtt_selection}"
                + (
                    ""
                    if case.behavior_case is None
                    else f" --perf-behavior-case {case.behavior_case}"
                )
            )
        return 0

    validate_requested_output(args)
    require_cargo_memory()
    binary = build_binary(args)
    session_dir = prepare_session(args, binary, cases)
    print(f"Artifacts: {session_dir}", flush=True)
    for case in cases:
        for index in range(1, args.preflight_runs + 1):
            run_one(
                args=args,
                binary=binary,
                session_dir=session_dir,
                case=case,
                run_number=index,
                preflight=True,
            )
        for index in range(1, args.repeat + 1):
            run_one(
                args=args,
                binary=binary,
                session_dir=session_dir,
                case=case,
                run_number=index,
                preflight=False,
            )
    return 0 if summarize_session(session_dir) else 1

def main() -> int:
    args = build_parser().parse_args()
    try:
        validate_arguments(args)
        if args.command in {"run", "audit", "behavior"}:
            return run_suite(args)
        if args.command == "summarize":
            return 0 if summarize_session(
                Path(args.session).resolve(),
                args.warmup_checksum_policy,
                args.measure_end_checksum_policy,
            ) else 1
        if args.command == "compare":
            return compare_sessions(args)
        if args.command == "compare-dashboard-modes":
            return compare_dashboard_modes(args)
        if args.command == "validate-rtt-light-contract":
            return validate_rtt_light_contract_command(args)
        if args.command == "finalize-rtt-light-attempt":
            manifest = finalize_attempt(Path(args.attempt))
            print(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True))
            return 0
        if args.command == "verify-rtt-light-attempt":
            manifest = verify_attempt(Path(args.attempt))
            print(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True))
            return 0
        if args.command == "verify-rtt-light-baseline":
            report = verify_baseline(Path(args.baseline))
            print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
            return 0
        return self_test()
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"perf.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
