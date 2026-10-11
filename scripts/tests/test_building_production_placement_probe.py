"""Synthetic branch refusal fixtures, never native acceptance evidence."""
import copy
import unittest
from unittest.mock import Mock

from scripts import building_production_placement_probe as probe
from scripts import building_production_lifecycle as lifecycle


class PlacementProbeTests(unittest.TestCase):
    def setUp(self):
        self.raw = {"schema_version": 2, "transition_contract": "indexed-state-delta-v1",
                    "nonce": "test", "samples": [], "events": []}
        for index in range(4):
            self.raw["samples"].append({"sample_index": index, "real_seconds": index,
                "primary_window": "1v0", "world_epoch": 0, "dpi": 1, "blueprints": [],
                "menu_state": "Architect", "selected_building": "Tank", "play_mode": "BuildingPlace",
                "task_mode": "None", "cursor_position": probe.POINT if index else [10, 10]})
            for category in lifecycle.TRANSITION_CATEGORIES:
                self.raw["samples"][-1][category] = []
        for index, state in enumerate(("Pressed", "Released")):
            self.raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput",
                "state": state, "button": "Left", "window": "1v0", "sample_index": 2,
                "event_index": index, "real_seconds": 2})
        self.raw["events"].append({"producer": probe.PRODUCER, "operation": "PlacementOutcome",
            "sample_index": 2, "event_index": 2, "real_seconds": 2, "window": "1v0", "world_epoch": 0,
            "outcome": {"kind": "Tank", "grid": [0, 0], "status": "CompanionPending"}})
        self.evidence = {"before": 0, "pointer": 1, "after": 3, "event_boundary": 0,
                         "point": probe.POINT, "outcome_index": 2}

    def test_pending_and_rejected_are_only_branch_evidence(self):
        probe.verify(self.raw, self.evidence)
        self.raw["events"][2]["outcome"].update(status="Rejected", reason="outside world")
        probe.verify(self.raw, self.evidence)
        self.assertNotIn("accepted", self.evidence)

    def test_missing_changed_or_foreign_sample_is_rejected(self):
        for change in ({"world_epoch": True}, {"dpi": True}, {"primary_window": "other"},
                       {"blueprints": None}, {"blueprints": [{"entity": "2v0"}]},
                       {"cursor_position": [641, 300]}, {"play_mode": "Normal"}):
            raw = copy.deepcopy(self.raw)
            raw["samples"][3].update(change)
            with self.subTest(change=change), self.assertRaises((RuntimeError, ValueError)):
                probe.verify(raw, self.evidence)

    def test_foreign_synthetic_or_mistimed_input_is_rejected(self):
        for change in ({"window": "other"}, {"button": "Right"}, {"sample_index": True},
                       {"real_seconds": float("nan")}, {"real_seconds": 1},
                       {"event_index": 9}, {"state": "Released"}, {"operation": "Synthetic"}):
            raw = copy.deepcopy(self.raw)
            raw["events"][0].update(change)
            with self.subTest(change=change), self.assertRaises((RuntimeError, ValueError)):
                probe.verify(raw, self.evidence)

    def test_committed_owner_is_not_first_click_success(self):
        self.raw["events"][2]["outcome"].update(status="CompanionCommitted", owner="2v0",
                                                footprint=[[0, 0]], companion_owners=["3v0"])
        with self.assertRaises((RuntimeError, ValueError)):
            probe.verify(self.raw, self.evidence)

    def test_duplicate_branch_and_stale_evidence_are_rejected(self):
        for change in ({"before": True}, {"pointer": 0}, {"outcome_index": 1},
                       {"point": [641, 300]}, {"event_boundary": 3}):
            with self.subTest(change=change), self.assertRaises((RuntimeError, ValueError)):
                probe.verify(self.raw, self.evidence | change)
        self.raw["events"].append(self.raw["events"][2] | {"event_index": 3})
        with self.assertRaises((RuntimeError, ValueError)):
            probe.verify(self.raw, self.evidence)

    def test_one_motion_and_click_without_input_retry(self):
        before = copy.deepcopy(self.raw)
        before.update(samples=before["samples"][:1], events=[])
        pointer = copy.deepcopy(self.raw)
        pointer.update(samples=pointer["samples"][:2], events=[])
        transport = Mock()
        self.assertEqual(probe.run(transport, Mock(side_effect=[before, pointer, self.raw])), self.evidence)
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["world-motion", "world-press", "world-release"])

    def test_context_change_after_motion_never_sends_click(self):
        for change in ({"world_epoch": 1}, {"world_epoch": True}, {"primary_window": "other"},
                       {"dpi": 2}, {"selected_building": "MudMixer"}, {"play_mode": "Normal"},
                       {"task_mode": "Other"}):
            before, pointer = copy.deepcopy(self.raw), copy.deepcopy(self.raw)
            before.update(samples=before["samples"][:1], events=[])
            pointer.update(samples=pointer["samples"][:2], events=[])
            pointer["samples"][1].update(change)
            transport = Mock()
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.run(transport, Mock(side_effect=[before, pointer]))
            self.assertEqual([call.args[0] for call in transport.send.call_args_list], ["world-motion"])


if __name__ == "__main__":
    unittest.main()
