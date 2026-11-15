from __future__ import annotations

import unittest

from scripts.perf_tool.arguments import build_parser, validate_arguments


def arguments(*, repeat=1, single=True, instrument="capture", size="small", souls=15):
    argv = ["run", "--workload", "building-art-static", "--sizes", size, "--renders", "gpu",
            "--seed", "20260920", "--repeat", str(repeat), "--preflight-runs", "0",
            "--souls", str(souls), "--familiars", "0", "--instrumentation", instrument,
            "--warmup-secs", "30", "--measure-secs", "60", "--backend", "vulkan",
            "--window-backend", "x11", "--present-mode", "novsync", "--window-width", "1280",
            "--window-height", "720", "--window-scale-factor", "1", "--rtt-quality", "high",
            "--adapter", "Intel"]
    if single:
        argv.append("--building-static-single-leg")
    return build_parser().parse_args(argv)


class BuildingStaticSingleLegTests(unittest.TestCase):
    def test_explicit_leg_preserves_capture_memory_sizes_and_historical_three_run_default(self):
        for instrument in ("capture", "memory"):
            for size, souls in (("small", 15), ("medium", 60)):
                validate_arguments(arguments(instrument=instrument, size=size, souls=souls))
                validate_arguments(arguments(repeat=3, single=False, instrument=instrument, size=size, souls=souls))

    def test_single_run_is_not_implicitly_admitted_or_a_three_run_acceptance(self):
        for repeat, single in ((1, False), (3, True)):
            with self.subTest(repeat=repeat, single=single), self.assertRaises(ValueError):
                validate_arguments(arguments(repeat=repeat, single=single))

    def test_fixed_fixture_renderer_time_and_ui_conditions_cannot_be_relaxed(self):
        changes = {"seed": 1, "sizes": "small,medium", "renders": "cpu", "souls": 16,
                   "familiars": 1, "instrumentation": "renderdoc", "preflight_runs": 1,
                   "warmup_secs": 29., "measure_secs": 59., "backend": "gl",
                   "window_backend": "auto", "present_mode": "fifo", "window_width": 1279,
                   "window_height": 719, "window_scale_factor": 1.5, "rtt_quality": "medium",
                   "familiar_policies": "disabled", "operation_dialog_modes": "open",
                   "dashboard_modes": "visible", "contract": "rtt-light-v1"}
        for name, value in changes.items():
            args = arguments()
            setattr(args, name, value)
            with self.subTest(name=name), self.assertRaises(ValueError):
                validate_arguments(args)

    def test_flag_is_rejected_before_other_workload_returns_and_cannot_borrow_m6(self):
        for workload in ("wall-density", "door-density", "indoor-light", "task-dashboard"):
            args = arguments()
            args.workload = workload
            with self.subTest(workload=workload), self.assertRaises(ValueError):
                validate_arguments(args)
        for name, value in (("building_m6_mode", "legacy-control"),
                            ("building_m6_asset_root", "/private/assets")):
            args = arguments()
            setattr(args, name, value)
            with self.subTest(name=name), self.assertRaises(ValueError):
                validate_arguments(args)

    def test_non_run_command_cannot_use_the_leg(self):
        from scripts.perf_tool.argument_validation.common import validate_common
        from scripts.perf_tool.argument_validation.density import validate_density
        args = arguments()
        matrix = validate_common(args)
        args.command = "audit"
        with self.assertRaises(ValueError):
            validate_density(args, matrix)


if __name__ == "__main__":
    unittest.main()
