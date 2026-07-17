from __future__ import annotations

from .fixtures import *

def run_suite(args: argparse.Namespace) -> int:
    sizes = parse_csv_list(args.sizes, {"small", "medium", "large"}, "sizes")
    renders = parse_csv_list(args.renders, {"cpu", "gpu"}, "renders")
    if (args.souls is None) != (args.familiars is None):
        raise ValueError("--souls and --familiars must be provided together")
    cases = [
        Case(args.workload, size, render, args.seed, args.souls, args.familiars)
        for size in sizes
        for render in renders
    ]
    if args.dry_run:
        binary = Path(args.binary or "target/profiling/bevy_app")
        for case in cases:
            print(
                f"{case.identifier}: {binary} --perf-scenario --perf-seed {case.seed} "
                f"--perf-clock {args.clock_mode} ..."
            )
        return 0

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
        if args.command in {"run", "audit"}:
            return run_suite(args)
        if args.command == "summarize":
            return 0 if summarize_session(
                Path(args.session).resolve(),
                args.warmup_checksum_policy,
                args.measure_end_checksum_policy,
            ) else 1
        if args.command == "compare":
            return compare_sessions(args)
        return self_test()
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(f"perf.py: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
