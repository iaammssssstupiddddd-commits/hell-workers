from __future__ import annotations

import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from scripts import cargo_runtime
from scripts import dev
from scripts.perf_tool import execution


class CargoRuntimeTests(unittest.TestCase):
    def test_cargo_environment_overrides_tmpfs_parent_settings(self) -> None:
        repo = execution.REPO_ROOT
        with patch.object(cargo_runtime, "account_home", return_value=repo):
            environment = cargo_runtime.cargo_environment(
                repo,
                namespace=".test-tmp",
                environment={
                    "CARGO_TARGET_DIR": "/tmp/unsafe-target",
                    "CARGO_HOME": "/tmp/unsafe-cargo-home",
                    "RUSTUP_HOME": "/tmp/unsafe-rustup-home",
                    "TMPDIR": "/tmp/unsafe-temp",
                    "CARGO_INCREMENTAL": "1",
                    "CARGO_BUILD_JOBS": "128",
                },
                incremental=False,
                create_temp_dir=False,
            )

        target = cargo_runtime.workspace_cargo_target(repo)
        temporary = cargo_runtime.workspace_temp_dir(repo, ".test-tmp")
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(target))
        self.assertEqual(environment["CARGO_BUILD_TARGET_DIR"], str(target))
        self.assertEqual(environment["CARGO_BUILD_BUILD_DIR"], str(target))
        self.assertEqual(environment["TMPDIR"], str(temporary))
        self.assertEqual(environment["TMP"], str(temporary))
        self.assertEqual(environment["TEMP"], str(temporary))
        self.assertEqual(environment["CARGO_HOME"], str(repo / ".cargo"))
        self.assertEqual(environment["RUSTUP_HOME"], str(repo / ".rustup"))
        self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
        self.assertIn(environment["CARGO_BUILD_JOBS"], {"1", "2"})
        self.assertEqual(
            cargo_runtime.cargo_build_jobs(15 * cargo_runtime.GIB),
            1,
        )
        self.assertEqual(
            cargo_runtime.cargo_build_jobs(16 * cargo_runtime.GIB),
            2,
        )
        self.assertIsNotNone(
            cargo_runtime.cargo_memory_error(
                7 * cargo_runtime.GIB,
                swap_total=8 * cargo_runtime.GIB,
                swap_free=2 * cargo_runtime.GIB,
            )
        )
        self.assertIsNone(
            cargo_runtime.cargo_memory_error(
                8 * cargo_runtime.GIB,
                swap_total=8 * cargo_runtime.GIB,
                swap_free=2 * cargo_runtime.GIB,
            )
        )

    def test_named_lanes_isolate_target_build_and_temp_paths(self) -> None:
        repo = execution.REPO_ROOT
        environments = {}
        with patch.object(cargo_runtime, "account_home", return_value=repo):
            for lane in cargo_runtime.INTERACTIVE_BUILD_LANES:
                environments[lane] = cargo_runtime.cargo_environment(
                    repo,
                    namespace=".test-tmp",
                    environment={
                        "CARGO_TARGET_DIR": "/tmp/unsafe-target",
                        "CARGO_BUILD_BUILD_DIR": "/tmp/unsafe-build",
                        "TMPDIR": "/tmp/unsafe-temp",
                        "CARGO_BUILD_JOBS": "128",
                    },
                    incremental=None,
                    lane=lane,
                    create_temp_dir=False,
                )

        targets = {
            lane: cargo_runtime.workspace_lane_target(repo, lane)
            for lane in cargo_runtime.INTERACTIVE_BUILD_LANES
        }
        temporary = {
            lane: cargo_runtime.workspace_lane_temp_dir(repo, lane, ".test-tmp")
            for lane in cargo_runtime.INTERACTIVE_BUILD_LANES
        }
        self.assertNotEqual(targets["a"], targets["b"])
        self.assertNotEqual(temporary["a"], temporary["b"])
        for lane, environment in environments.items():
            self.assertEqual(environment["CARGO_TARGET_DIR"], str(targets[lane]))
            self.assertEqual(environment["CARGO_BUILD_TARGET_DIR"], str(targets[lane]))
            self.assertEqual(environment["CARGO_BUILD_BUILD_DIR"], str(targets[lane]))
            self.assertEqual(environment["TMPDIR"], str(temporary[lane]))
            self.assertEqual(environment["TMP"], str(temporary[lane]))
            self.assertEqual(environment["TEMP"], str(temporary[lane]))
            self.assertEqual(environment["CARGO_BUILD_JOBS"], "1")

    def test_invalid_lane_is_rejected_before_path_resolution(self) -> None:
        with self.assertRaisesRegex(ValueError, "invalid build lane"):
            cargo_runtime.workspace_lane_target(execution.REPO_ROOT, "c")
        with self.assertRaisesRegex(ValueError, "invalid build lane"):
            cargo_runtime.cargo_environment(
                execution.REPO_ROOT,
                namespace=".test-tmp",
                incremental=None,
                lane="../target",
                create_temp_dir=False,
            )
        self.assertIsNone(
            cargo_runtime.cargo_memory_error(
                9 * cargo_runtime.GIB,
                swap_total=8 * cargo_runtime.GIB,
                swap_free=0,
            )
        )
        self.assertIsNone(
            cargo_runtime.cargo_memory_error(
                8 * cargo_runtime.GIB,
                swap_total=0,
                swap_free=0,
            )
        )

    def test_linux_memory_guard_fails_closed_when_counters_are_unavailable(self) -> None:
        with patch.object(cargo_runtime.platform, "system", return_value="Linux"), patch.object(
            cargo_runtime,
            "meminfo_bytes",
            return_value=None,
        ):
            self.assertIn("cannot read /proc/meminfo", cargo_runtime.cargo_memory_error())

        with patch.object(cargo_runtime.platform, "system", return_value="Linux"), patch.object(
            cargo_runtime,
            "meminfo_bytes",
            return_value={"SwapTotal": 0, "SwapFree": 0},
        ):
            self.assertIn("MemAvailable", cargo_runtime.cargo_memory_error())

        with patch.object(cargo_runtime.platform, "system", return_value="Linux"), patch.object(
            cargo_runtime,
            "meminfo_bytes",
            return_value={
                "MemAvailable": 8 * cargo_runtime.GIB,
                "SwapTotal": 8 * cargo_runtime.GIB,
            },
        ):
            self.assertIsNone(cargo_runtime.cargo_memory_error())

    def test_non_linux_memory_guard_does_not_require_procfs_counters(self) -> None:
        with patch.object(cargo_runtime.platform, "system", return_value="Darwin"), patch.object(
            cargo_runtime,
            "meminfo_bytes",
            return_value=None,
        ):
            self.assertIsNone(cargo_runtime.cargo_memory_error())

    def test_mount_parser_identifies_memory_backed_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = root / "workspace"
            temporary = root / "tmp"
            workspace.mkdir()
            temporary.mkdir()
            mountinfo = root / "mountinfo"
            mountinfo.write_text(
                f"36 25 0:32 / {workspace} rw - ext4 /dev/fake rw\n"
                f"37 36 0:33 / {temporary} rw - tmpfs tmpfs rw\n",
                encoding="utf-8",
            )

            self.assertEqual(
                cargo_runtime.filesystem_type(
                    workspace / "target",
                    mountinfo_path=mountinfo,
                ),
                "ext4",
            )
            self.assertEqual(
                cargo_runtime.filesystem_type(
                    temporary / "target",
                    mountinfo_path=mountinfo,
                ),
                "tmpfs",
            )

    def test_cargo_environment_rejects_a_tmpfs_workspace_target(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "must not be placed under /tmp"):
                cargo_runtime.cargo_environment(
                    Path(directory),
                    namespace=".test-tmp",
                    incremental=False,
                    create_temp_dir=False,
                )

    def test_non_linux_storage_guard_does_not_read_linux_mountinfo(self) -> None:
        with patch.object(cargo_runtime.platform, "system", return_value="Darwin"):
            self.assertIsNone(
                cargo_runtime.persistent_storage_error(
                    execution.REPO_ROOT / "target",
                    label="workspace Cargo target",
                )
            )

    def test_cargo_environment_preserves_safe_toolchain_cache_overrides(self) -> None:
        repo = execution.REPO_ROOT
        cargo_home = repo / "target" / ".test-cargo-home"
        rustup_home = repo / "target" / ".test-rustup-home"
        environment = cargo_runtime.cargo_environment(
            repo,
            namespace=".test-tmp",
            environment={
                "CARGO_HOME": str(cargo_home),
                "RUSTUP_HOME": str(rustup_home),
            },
            incremental=False,
            create_temp_dir=False,
        )

        self.assertEqual(environment["CARGO_HOME"], str(cargo_home))
        self.assertEqual(environment["RUSTUP_HOME"], str(rustup_home))

    def test_perf_runner_uses_the_controlled_environment(self) -> None:
        environment = execution.performance_environment()
        self.assertEqual(
            environment["CARGO_TARGET_DIR"],
            str(cargo_runtime.workspace_cargo_target(execution.REPO_ROOT)),
        )
        self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
        self.assertEqual(
            environment["TMPDIR"],
            str(cargo_runtime.workspace_temp_dir(execution.REPO_ROOT, ".perf-tmp")),
        )

    def test_resource_policy_records_memory_guard_and_swap_telemetry_contract(self) -> None:
        policy = cargo_runtime.resource_policy(
            execution.REPO_ROOT,
            namespace=".test-tmp",
            incremental=False,
        )

        guard = policy["memory_guard"]
        self.assertEqual(
            guard["minimum_mem_available_gib"],
            cargo_runtime.MIN_CARGO_MEMORY_GIB,
        )
        self.assertTrue(guard["swap_is_telemetry_only"])

    def test_resource_policy_records_lane_build_root(self) -> None:
        policy = cargo_runtime.resource_policy(
            execution.REPO_ROOT,
            namespace=".test-tmp",
            incremental=None,
            lane="a",
        )
        lane_target = cargo_runtime.workspace_lane_target(execution.REPO_ROOT, "a")
        self.assertEqual(policy["cargo_target"], str(lane_target))
        self.assertEqual(policy["cargo_build_dir"], str(lane_target))
        self.assertEqual(policy["cargo_build_jobs"], 1)

    def test_tmp_output_is_rejected_before_a_perf_run(self) -> None:
        message = cargo_runtime.persistent_storage_error(
            Path("/tmp/hell-workers-perf-output"),
            label="performance artifact output",
        )
        self.assertIsNotNone(message)
        self.assertIn("must not be placed under /tmp", message)

        with self.assertRaisesRegex(RuntimeError, "must not be placed under /tmp"):
            execution.validate_requested_output(
                Namespace(output="/tmp/hell-workers-perf-output")
            )

    def test_symlinked_artifact_root_is_rejected_after_resolution(self) -> None:
        target = cargo_runtime.workspace_cargo_target(execution.REPO_ROOT)
        with tempfile.TemporaryDirectory(dir=target) as directory:
            redirect = Path(directory) / "perf-runs"
            redirect.symlink_to("/tmp", target_is_directory=True)
            message = cargo_runtime.persistent_storage_error(
                redirect / "session",
                label="performance artifact output",
            )

        self.assertIsNotNone(message)
        self.assertIn("must not be placed under /tmp", message)

    def test_default_perf_output_is_storage_validated_before_build(self) -> None:
        with patch.object(
            execution,
            "default_output_root",
            return_value=Path("/tmp/hell-workers-perf-output"),
        ):
            with self.assertRaisesRegex(RuntimeError, "must not be placed under /tmp"):
                execution.validate_requested_output(Namespace(output=None))

    def test_tracy_csvexport_receives_the_controlled_environment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = {"TMPDIR": "/persistent/tmp", "CARGO_HOME": "/persistent/cargo"}
            with patch.object(
                execution.subprocess,
                "run",
                return_value=SimpleNamespace(returncode=0),
            ) as run:
                returncode, timeout_error = execution.run_csvexport(
                    Path("/usr/bin/true"),
                    root / "trace.tracy",
                    root / "zones.csv",
                    root / "zones.log",
                    ["-f", "zone"],
                    5.0,
                    environment,
                )

        self.assertEqual((returncode, timeout_error), (0, None))
        self.assertEqual(run.call_args.kwargs["env"], environment)

    def test_dev_cargo_commands_use_the_controlled_environment(self) -> None:
        with patch.object(dev, "require_cargo_memory") as require_memory, patch.object(
            dev.subprocess,
            "run",
        ) as run:
            dev.run_command(["cargo", "check", "--workspace"])

        environment = run.call_args.kwargs["env"]
        require_memory.assert_called_once_with()
        self.assertEqual(
            environment["CARGO_TARGET_DIR"],
            str(cargo_runtime.workspace_cargo_target(dev.REPO_ROOT)),
        )
        self.assertEqual(
            environment["TMPDIR"],
            str(cargo_runtime.workspace_temp_dir(dev.REPO_ROOT, ".dev-tmp")),
        )
        self.assertIn(environment["CARGO_BUILD_JOBS"], {"1", "2"})

    def test_dev_cargo_commands_keep_an_inherited_lane(self) -> None:
        with patch.object(dev, "validate_inherited_lease", return_value="b"):
            with patch.object(dev, "require_cargo_memory"), patch.object(
                dev.subprocess, "run"
            ) as run:
                dev.run_command(["cargo", "check", "--workspace"])

        environment = run.call_args.kwargs["env"]
        lane_target = cargo_runtime.workspace_lane_target(dev.REPO_ROOT, "b")
        self.assertEqual(environment["CARGO_TARGET_DIR"], str(lane_target))
        self.assertEqual(environment["CARGO_BUILD_TARGET_DIR"], str(lane_target))
        self.assertEqual(environment["CARGO_BUILD_BUILD_DIR"], str(lane_target))
        self.assertEqual(
            environment["TMPDIR"],
            str(cargo_runtime.workspace_lane_temp_dir(dev.REPO_ROOT, "b", ".dev-tmp")),
        )
        self.assertEqual(environment["CARGO_BUILD_JOBS"], "1")

    def test_dev_rejects_output_root_overrides(self) -> None:
        for arguments in (
            ["--target-dir", "/tmp/target"],
            ["--target-dir=/tmp/target"],
            ["--config", "build.target-dir=/tmp/target"],
            ["--config=build.build-dir=/tmp/build"],
            ["--config", 'build = { target-dir = "/tmp/target" }'],
        ):
            with self.subTest(arguments=arguments):
                with patch.object(dev.subprocess, "run") as run:
                    with self.assertRaisesRegex(RuntimeError, "overrides are not supported"):
                        dev.run_command(["cargo", "check", *arguments])
                run.assert_not_called()

    def test_dev_does_not_spawn_a_compile_below_the_memory_floor(self) -> None:
        with patch.object(
            dev,
            "require_cargo_memory",
            side_effect=RuntimeError("memory floor"),
        ), patch.object(dev.subprocess, "run") as run:
            with self.assertRaisesRegex(RuntimeError, "memory floor"):
                dev.run_command(["cargo", "check", "--workspace"])

        run.assert_not_called()

    def test_dev_reports_a_guard_refusal_without_a_traceback(self) -> None:
        with patch.object(
            dev,
            "require_cargo_memory",
            side_effect=RuntimeError("memory floor"),
        ):
            self.assertEqual(dev.main(["cargo", "--", "check", "--workspace"]), 1)


if __name__ == "__main__":
    unittest.main()
