"""Synthetic refusal fixtures; never ordinary-world acceptance evidence."""
import copy
import unittest
from unittest.mock import Mock

from scripts import building_production_companion_probe as probe
from scripts.tests import test_building_production_placement_probe as placement_tests


class CompanionProbeTests(unittest.TestCase):
    def setUp(self):
        fixture = placement_tests.PlacementProbeTests()
        fixture.setUp()
        fixture.yard_fixture()
        self.raw, self.first = fixture.raw, fixture.evidence
        self.raw["samples"].extend(copy.deepcopy(self.raw["samples"][-1]) for _ in range(4))
        for i, sample in enumerate(self.raw["samples"]):
            sample.update(sample_index=i, real_seconds=i, companion_context={"available": True, "pending": None, "storages": []})
            if i >= 2:
                sample["companion_context"]["pending"] = {"parent_kind": "Tank", "parent_anchor": [73, 56],
                    "kind": "BucketStorage", "center": [112, 528], "radius": 160, "required": True}
            if i >= 5:
                sample["cursor_position"] = [760, 152]
        footprint = [[73, 56], [74, 56], [73, 57], [74, 57]]
        for sample in self.raw["samples"][6:]:
            sample["blueprints"] = [{"entity": "4v0", "kind": "Tank", "footprint": footprint}]
            sample["owner_layers"] = [{"entity": "4v0", "exists": True, "building_grids": footprint}]
            sample["companion_context"] = {"available": True, "pending": None, "storages": [
                {"entity": child, "position": [160 + 32 * i, 512, 0], "grid": [75 + i, 56],
                 "pending_owner": "4v0", "owner": None, "stockpile_grids": [[75 + i, 56]]}
                for i, child in enumerate(("5v0", "6v0"))]}
        for state in ("Pressed", "Released"):
            self.raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput",
                "state": state, "button": "Left", "window": "1v0", "sample_index": 6,
                "event_index": len(self.raw["events"]), "real_seconds": 6})
        self.raw["events"].append({"producer": probe.placement.PRODUCER, "operation": "PlacementOutcome",
            "sample_index": 6, "event_index": 5, "real_seconds": 6, "window": "1v0", "world_epoch": 0,
            "outcome": {"kind": "Tank", "grid": [75, 56], "status": "CompanionCommitted",
                "owner": "4v0", "footprint": footprint, "companion_owners": ["5v0", "6v0"]}})
        for category in ("blueprints", "owner_layers"):
            self.raw["events"].append({"producer": "production_snapshot_delta", "operation": "StateTransition",
                "sample_index": 6, "before_sample_index": 5, "real_seconds": 6, "category": category,
                "entity": "4v0", "before": None, "after": self.raw["samples"][6][category][0],
                "event_index": len(self.raw["events"])})
        self.evidence = {"before": 4, "pointer": 5, "after": 7, "event_boundary": 3,
                         "point": [760, 152], "outcome_index": 5}

    def test_actual_blueprint_and_two_owned_stockpiles_are_required(self):
        probe.verify(self.raw, self.evidence, self.first)
        self.assertNotIn("accepted", self.evidence)

    def test_changed_pending_geometry_is_rejected_before_input(self):
        for change in ({"parent_anchor": [74, 56]}, {"kind": "Other"}, {"required": False},
                       {"radius": True}, {"radius": float("nan")}, {"center": [113, 528]}):
            raw = copy.deepcopy(self.raw)
            raw["samples"] = raw["samples"][:5]
            raw["samples"][-1]["companion_context"]["pending"].update(change)
            transport = Mock()
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.run(transport, lambda: raw, self.first)
            transport.send.assert_not_called()

    def test_context_change_after_motion_never_sends_click(self):
        for key, value in (("primary_window", "foreign"), ("world_epoch", True), ("dpi", 2),
                           ("selected_building", "MudMixer"), ("companion_context", None), ("blueprints", [{"entity": "foreign"}])):
            start, pointer = copy.deepcopy(self.raw), copy.deepcopy(self.raw)
            start.update(samples=start["samples"][:5], events=start["events"][:3])
            pointer.update(samples=pointer["samples"][:6], events=pointer["events"][:3])
            pointer["samples"][-1][key] = value
            transport = Mock()
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                probe.run(transport, Mock(side_effect=[start, pointer]), self.first)
            self.assertEqual([call.args[0] for call in transport.send.call_args_list], ["companion-motion"])

    def test_false_commit_or_foreign_child_or_missing_map_ownership_rejected(self):
        for change in ({"pending_owner": "foreign"}, {"owner": "4v0"}, {"grid": [76, 56]},
                       {"stockpile_grids": []}, {"entity": "foreign"}, {"position": [161, 512, 0]},
                       {"position": [160, 512, float("nan")]}):
            raw = copy.deepcopy(self.raw)
            raw["samples"][-1]["companion_context"]["storages"][0].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.verify(raw, self.evidence, self.first)
        raw = copy.deepcopy(self.raw)
        raw["events"][5]["outcome"].update(status="CompanionRolledBack")
        with self.assertRaises((RuntimeError, ValueError)):
            probe.verify(raw, self.evidence, self.first)

    def test_one_original_click_and_no_retry(self):
        start, pointer = copy.deepcopy(self.raw), copy.deepcopy(self.raw)
        start.update(samples=start["samples"][:5], events=start["events"][:3])
        pointer.update(samples=pointer["samples"][:6], events=pointer["events"][:3])
        transport = Mock()
        self.assertEqual(probe.run(transport, Mock(side_effect=[start, pointer, self.raw]), self.first), self.evidence)
        self.assertEqual([call.args[0] for call in transport.send.call_args_list],
                         ["companion-motion", "companion-press", "companion-release"])

    def test_escape_closure_preserves_actual_parent_map_reservation(self):
        committed = self.raw["samples"][-1]
        probe.check_closed(committed, committed)
        for change in ({"exists": False}, {"building_grids": []}, {"entity": "foreign"}):
            closed = copy.deepcopy(committed)
            closed["owner_layers"][0].update(change)
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, "map ownership"):
                probe.check_closed(closed, committed)
        closed = copy.deepcopy(committed)
        closed["companion_context"]["storages"] = []
        with self.assertRaisesRegex(RuntimeError, "inventory"):
            probe.check_closed(closed, committed)


if __name__ == "__main__":
    unittest.main()
