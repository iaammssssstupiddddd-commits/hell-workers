"""Synthetic geometry/input refusal fixtures, not native placement evidence."""
import copy
import unittest
from unittest.mock import Mock, patch

from scripts import building_production_yard_probe as probe


class YardProbeTests(unittest.TestCase):
    def setUp(self):
        self.yard = {"entity": "2v0", "min": [0, 0], "max": [608, 608],
                     "grid_min": [70, 40], "grid_max": [89, 59],
                     "grid_world_min": [0, 0], "grid_world_max": [608, 608],
                     "logical_viewport_corners": [[600, 600], [1208, 600], [1208, -8], [600, -8]]}
        self.sample = {"world_epoch": 0, "primary_window": "1v0", "dpi": 1,
            "menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None", "selected_building": None,
            "placement_context": {"camera": "3v0", "target_window": "1v0", "camera_position": [0, 0, 0],
                "logical_viewport_size": [1280, 720], "yards": [self.yard]}}

    def test_point_is_derived_from_actual_grid_projection_not_approval(self):
        yard, grid, pixel = probe.point(self.sample)
        self.assertEqual(yard, self.yard)
        self.assertEqual(grid, [73, 56])
        self.assertEqual(pixel, [696, 88])
        self.assertNotIn("accepted", yard)

    def test_foreign_missing_scaled_or_ambiguous_context_is_rejected(self):
        for change in ({"target_window": "foreign"}, {"camera": None}, {"yards": []},
                       {"yards": [self.yard, self.yard]}, {"logical_viewport_size": [640, 360]},
                       {"camera_position": [float("nan"), 0, 0]}):
            sample = copy.deepcopy(self.sample)
            sample["placement_context"].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.point(sample)
        for change in ({"dpi": True}, {"dpi": 2}, {"world_epoch": True}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.point(self.sample | change)

    def test_unaligned_nonfinite_rotated_or_invalid_grid_is_rejected(self):
        for change in ({"grid_world_min": [1, 0]}, {"grid_min": [True, 40]}, {"grid_max": [100, 59]},
                       {"logical_viewport_corners": None},
                       {"logical_viewport_corners": [[600, 600], [1208, 602], [1208, -8], [600, -8]]},
                       {"min": [float("inf"), 0]}):
            sample = copy.deepcopy(self.sample)
            sample["placement_context"]["yards"][0].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.point(sample)

    def test_pan_error_releases_original_key_once_without_click(self):
        raw = {"nonce": "n", "samples": [self.sample], "events": []}
        transport = Mock()
        with self.assertRaisesRegex(RuntimeError, "read failed"):
            probe.run(transport, Mock(side_effect=[raw, raw, raw, RuntimeError("read failed")]))
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["pan-1-press", "pan-1-release"])
        self.assertEqual(transport.send.call_args_list[0].kwargs, {"key": "w", "pressed": True})

    def test_initial_mode_change_sends_no_input(self):
        for change in ({"menu_state": "Architect"}, {"play_mode": "BuildingPlace"},
                       {"task_mode": "Other"}, {"selected_building": "Tank"}):
            transport = Mock()
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.run(transport, lambda: {"nonce": "n", "samples": [self.sample | change], "events": []})
            transport.send.assert_not_called()

    def test_pan_timeout_is_bounded_and_releases_held_input(self):
        raw = {"nonce": "n", "samples": [self.sample], "events": []}
        transport = Mock()
        with patch.object(probe.time, "monotonic", side_effect=[0, 5]), self.assertRaisesRegex(RuntimeError, "timeout"):
            probe.run(transport, lambda: raw)
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["pan-1-press", "pan-1-release"])

    def test_pan_overshoot_rejects_and_releases_without_correction(self):
        overshoot = copy.deepcopy(self.sample)
        overshoot["placement_context"]["camera_position"][1] = 260
        for corner in overshoot["placement_context"]["yards"][0]["logical_viewport_corners"]:
            corner[1] += 260
        before = {"nonce": "n", "samples": [self.sample], "events": []}
        after = {"nonce": "n", "samples": [self.sample, overshoot], "events": []}
        transport = Mock()
        with self.assertRaisesRegex(RuntimeError, "crossed endpoint"):
            probe.run(transport, Mock(side_effect=[before, before, before, after]))
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["pan-1-press", "pan-1-release"])

    def test_original_key_pan_and_stable_normal_endpoint(self):
        moved = copy.deepcopy(self.sample)
        moved["placement_context"]["camera_position"][1] = 64
        for corner in moved["placement_context"]["yards"][0]["logical_viewport_corners"]:
            corner[1] += 64
        samples = [copy.deepcopy(sample) for sample in (self.sample, self.sample, moved, moved)]
        events = [{"producer": "KeyboardInput", "key": "KeyW", "state": state,
                   "repeat": False, "window": "1v0", "sample_index": i + 1}
                  for i, state in enumerate(("Pressed", "Released"))]
        before = {"nonce": "n", "samples": samples[:1], "events": []}
        during = {"nonce": "n", "samples": samples[:3], "events": events[:1]}
        after = {"nonce": "n", "samples": samples, "events": events}
        transport = Mock()
        evidence = probe.run(transport, Mock(side_effect=[before, before, before, during, after, after]))
        probe.verify(after, evidence)
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["pan-1-press", "pan-1-release"])
        for change in ({"window": "foreign"}, {"sample_index": True}, {"state": "Pressed"}, {"repeat": True}):
            changed = copy.deepcopy(after)
            changed["events"][1].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.verify(changed, evidence)


if __name__ == "__main__":
    unittest.main()
