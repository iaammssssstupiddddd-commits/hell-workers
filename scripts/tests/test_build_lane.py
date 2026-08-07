from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path

from scripts import build_lane


class BuildLaneTests(unittest.TestCase):
    def test_two_leases_are_distinct_and_third_session_is_busy(self) -> None:
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parents[2] / "target"
        ) as directory:
            repo = Path(directory)
            first = build_lane.acquire_lane(repo)
            second = build_lane.acquire_lane(repo)
            try:
                self.assertEqual((first.lane, second.lane), ("a", "b"))
                states = {lane: state for lane, state, _ in build_lane.lane_states(repo)}
                self.assertEqual(states, {"a": "busy", "b": "busy"})
                with self.assertRaisesRegex(build_lane.LaneBusyError, "no fallback target"):
                    build_lane.acquire_lane(repo)
            finally:
                second.close()
                first.close()

            self.assertEqual(
                {lane: state for lane, state, _ in build_lane.lane_states(repo)},
                {"a": "free", "b": "free"},
            )

    def test_inherited_descriptor_must_refer_to_the_selected_lane_lock(self) -> None:
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parents[2] / "target"
        ) as directory:
            repo = Path(directory)
            with self.assertRaisesRegex(RuntimeError, "without an inherited"):
                build_lane.validate_inherited_lease(
                    repo, {build_lane.LANE_ENV: "a"}
                )
            with build_lane.acquire_lane(repo) as lease:
                environment = lease.child_environment({})
                self.assertEqual(
                    build_lane.validate_inherited_lease(repo, environment), "a"
                )

                wrong_path = build_lane.lane_lock_path(repo, "b")
                wrong_path.parent.mkdir(parents=True, exist_ok=True)
                wrong_fd = os.open(
                    wrong_path, os.O_CREAT | os.O_RDWR, 0o600
                )
                try:
                    os.set_inheritable(wrong_fd, True)
                    environment[build_lane.LANE_FD_ENV] = str(wrong_fd)
                    with self.assertRaisesRegex(RuntimeError, "does not refer"):
                        build_lane.validate_inherited_lease(repo, environment)
                finally:
                    os.close(wrong_fd)

    def test_lane_shell_releases_lease_after_child_exit(self) -> None:
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parents[2] / "target"
        ) as directory:
            repo = Path(directory)
            self.assertEqual(build_lane.run_lane_shell(repo, ["/usr/bin/true"]), 0)
            self.assertEqual(
                {lane: state for lane, state, _ in build_lane.lane_states(repo)},
                {"a": "free", "b": "free"},
            )

    def test_lane_shell_releases_lease_after_child_failure(self) -> None:
        with tempfile.TemporaryDirectory(
            dir=Path(__file__).resolve().parents[2] / "target"
        ) as directory:
            repo = Path(directory)
            self.assertEqual(
                build_lane.run_lane_shell(repo, ["/bin/sh", "-c", "exit 7"]), 7
            )
            self.assertEqual(
                {lane: state for lane, state, _ in build_lane.lane_states(repo)},
                {"a": "free", "b": "free"},
            )


if __name__ == "__main__":
    unittest.main()
