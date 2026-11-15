"""Synthetic refusal fixtures only; these do not constitute native evidence."""
import copy
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

from scripts import building_production_selection_probe as selection


class SelectionProbeTests(unittest.TestCase):
    def setUp(self):
        self.card = {"entity": "4v0", "kind": "Tank", "physical_rect": [50, 60, 250, 120],
                     "physical_viewport": [1280, 720]}
        row = {"menu_state": "Architect", "play_mode": "Normal", "task_mode": "None",
               "selected_building": None, "dpi": 1, "world_epoch": 2, "primary_window": "1v0",
               "architect_panel_ready": True,
               "architect_cards": [self.card], "cursor_position": [150, 90]}
        selected = row | {"selected_building": "Tank", "play_mode": "BuildingPlace"}
        self.raw = {"samples": [copy.deepcopy(s) for s in [row, row, row, selected, selected, selected]],
                    "events": [{"producer": "MouseButtonInput", "operation": "MouseButtonInput",
                                "button": "Left", "state": state, "window": "1v0",
                                "sample_index": 3, "event_index": index}
                               for index, state in enumerate(("Pressed", "Released"))]}
        self.evidence = {"before": 1, "pointer": 2, "selected": 4, "end": 5,
                         "event_boundary": 0, "card": self.card, "point": [150, 90]}

    def test_exact_card_and_real_selection_are_distinct(self):
        self.assertEqual(selection.card(self.raw["samples"][0], "Tank"), (self.card, (150, 90)))
        selection.verify(self.raw, self.evidence, "Tank")
        self.assertFalse(selection.selected(self.raw["samples"][0], "Tank"))
        self.assertNotIn("accepted", self.evidence)

    def test_no_bridge_arbitrary_kind_hidden_or_scaled_input(self):
        row = self.raw["samples"][0]
        for kind in ("Bridge", "Wall", "anything"):
            with self.subTest(kind=kind), self.assertRaises(RuntimeError):
                selection.card(row, kind)
        for change in ({"dpi": 2}, {"dpi": True}, {"menu_state": "Hidden"}, {"play_mode": "BuildingPlace"},
                       {"architect_panel_ready": False},
                       {"architect_cards": []}, {"architect_cards": [self.card, self.card]}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                selection.card(row | change, "Tank")

    def test_nonfinite_wrong_viewport_and_degenerate_rect_rejected(self):
        for change in ({"physical_rect": [True, 60, 250, 120]},
                       {"physical_rect": [float("nan"), 60, 250, 120]},
                       {"physical_rect": [-1, 60, 250, 120]},
                       {"physical_rect": [50, 60, 50, 120]}, {"physical_viewport": [720, 1280]}):
            row = self.raw["samples"][0] | {"architect_cards": [self.card | change]}
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                selection.card(row, "Tank")

    def test_stale_changed_card_world_pointer_or_selection_rejected(self):
        for index, change in ((0, {"world_epoch": 3}), (0, {"world_epoch": True}),
                              (1, {"primary_window": "other"}), (2, {"cursor_position": [151, 90]}),
                              (5, {"selected_building": "MudMixer"}), (5, {"play_mode": "Normal"}),
                              (5, {"dpi": 2}),
                              (1, {"architect_cards": [self.card | {"entity": "5v0"}]})):
            raw = copy.deepcopy(self.raw)
            raw["samples"][index].update(change)
            with self.subTest(index=index, change=change), self.assertRaises(RuntimeError):
                selection.verify(raw, self.evidence, "Tank")
        for change in ({"before": True}, {"pointer": 1}, {"selected": 5}, {"end": 4},
                       {"event_boundary": True}, {"point": [151, 90]}, {"point": [150.0, 90]}, {"unknown": 1}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                selection.verify(self.raw, self.evidence | change, "Tank")

    def test_mouse_raw_cannot_be_replaced_by_state_or_wrong_window(self):
        for change in ({"producer": "ButtonInput"}, {"operation": "PointerPress"},
                       {"button": "Right"}, {"window": "other"}, {"sample_index": 1},
                       {"sample_index": True}, {"event_index": True}, {"state": "Released"}):
            raw = copy.deepcopy(self.raw)
            raw["events"][0].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                selection.verify(raw, self.evidence, "Tank")
        raw = copy.deepcopy(self.raw)
        raw["events"].append(raw["events"][1].copy())
        with self.assertRaises(RuntimeError):
            selection.verify(raw, self.evidence, "Tank")

    def test_soul_spa_uses_authoritative_task_mode_not_build_context(self):
        row = self.raw["samples"][0] | {"play_mode": "TaskDesignation", "task_mode": "SoulSpaPlace(None)"}
        self.assertTrue(selection.selected(row, "SoulSpa"))
        self.assertFalse(selection.selected(row | {"selected_building": "SoulSpa"}, "SoulSpa"))
        self.assertFalse(selection.selected(row | {"task_mode": "None"}, "SoulSpa"))

    def test_timeout_has_no_input_retry(self):
        with patch.object(selection.time, "monotonic", side_effect=[0, 9]):
            with self.assertRaisesRegex(RuntimeError, "timeout"):
                selection.wait(lambda: self.raw, lambda _: False)

    def snapshots(self):
        return [{"nonce": "n", "samples": copy.deepcopy(self.raw["samples"][:n]),
                 "events": copy.deepcopy(self.raw["events"] if n >= 5 else [])}
                for n in (2, 3, 5, 6)]

    def test_run_sends_exactly_one_motion_press_release_and_captures_selected(self):
        transport, capture = Mock(), Mock()
        evidence = selection.run(transport, Mock(side_effect=self.snapshots()), "Tank", Path("/unused"), capture)
        self.assertEqual(evidence, self.evidence)
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["select-motion", "select-press", "select-release"])
        self.assertEqual(transport.send.call_args_list[0].kwargs, {"point": (150, 90)})
        capture.assert_called_once_with(Path("/unused/selected.png"))

    def test_changed_pointer_target_does_not_click_or_capture(self):
        snapshots = self.snapshots()
        snapshots[1]["samples"][-1]["architect_cards"] = [self.card | {"entity": "other"}]
        transport, capture = Mock(), Mock()
        with self.assertRaisesRegex(RuntimeError, "target changed"):
            selection.run(transport, Mock(side_effect=snapshots), "Tank", Path("/unused"), capture)
        self.assertEqual(transport.send.call_count, 1)
        capture.assert_not_called()

    def test_unsettled_initial_window_or_epoch_sends_nothing(self):
        for change in ({"primary_window": "other"}, {"world_epoch": 3}):
            snapshots = self.snapshots()
            snapshots[0]["samples"][0].update(change)
            transport = Mock()
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, "unsettled"):
                selection.run(transport, Mock(side_effect=snapshots), "Tank", Path("/unused"), Mock())
            transport.send.assert_not_called()

    def test_missing_pointer_ack_never_clicks_or_retries_motion(self):
        transport = Mock()
        with patch.object(selection.time, "monotonic", side_effect=[0, 3]):
            with self.assertRaisesRegex(RuntimeError, "timeout"):
                selection.run(transport, Mock(return_value=self.snapshots()[0]), "Tank", Path("/unused"), Mock())
        self.assertEqual(transport.send.call_count, 1)
