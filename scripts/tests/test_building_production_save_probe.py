"""Synthetic refusal tests, not native or asset acceptance evidence."""
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock

from scripts import building_production_save_probe as probe


class SaveProbeTests(unittest.TestCase):
    def test_load_capture_requires_two_later_closed_samples_and_settle_time(self):
        samples = []
        for i, seconds in enumerate((1.0, 1.2, 1.6)):
            sample = self.sample("Closed")
            sample.update(sample_index=i, real_seconds=seconds, world_epoch=1, menu_state="Hidden")
            samples.append(sample)
        self.assertFalse(probe.loaded_capture_ready(samples[:1], 0, "1v0", 1))
        self.assertFalse(probe.loaded_capture_ready(samples[:2], 0, "1v0", 1))
        self.assertTrue(probe.loaded_capture_ready(samples, 0, "1v0", 1))
        early = copy.deepcopy(samples)
        early[-1]["real_seconds"] = 1.4
        self.assertFalse(probe.loaded_capture_ready(early, 0, "1v0", 1))
        for key, value in (("world_epoch", 2), ("primary_window", "foreign"), ("menu_state", "Architect")):
            changed = copy.deepcopy(samples)
            changed[-1][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                probe.loaded_capture_ready(changed, 0, "1v0", 1)
        changed = copy.deepcopy(samples)
        changed[-1]["save_catalog_context"]["mode"] = "LoadConfirmManual1"
        with self.assertRaises(RuntimeError):
            probe.loaded_capture_ready(changed, 0, "1v0", 1)

    def test_failed_terminal_save_load_is_reported_without_timeout_or_retry(self):
        for operation in ("Save", "Load"):
            read = Mock(return_value={"events": [{"producer": "SaveLoadOutcome", "operation": operation,
                                                   "result": "Failed(InvalidData)"}], "samples": [{}]})
            with self.subTest(operation=operation), self.assertRaisesRegex(RuntimeError, "terminal rejection"):
                probe.wait_success(read, operation, 0, lambda _s: False)
            self.assertEqual(read.call_count, 1)

    def test_passive_layout_wait_requires_two_actual_stable_samples_and_no_input(self):
        empty = self.sample()
        empty["save_catalog_context"]["controls"] = []
        raw = {"nonce": "nonce", "samples": [empty, self.sample()]}
        ready = {"nonce": "nonce", "samples": [self.sample(), self.sample()]}
        self.assertEqual(probe.wait_control(Mock(side_effect=[raw, ready]), "Save", "nonce", "1v0", 0), ready)
        for key, value in (("primary_window", "foreign"), ("world_epoch", 1)):
            changed = copy.deepcopy(ready)
            changed["samples"][-1][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                probe.wait_control(lambda: changed, "Save", "nonce", "1v0", 0)

    def sample(self, mode="SaveCatalog", session=7):
        action = {"SaveCatalog": "Save", "LoadCatalog": "Load", "LoadConfirmManual1": "ConfirmLoad"}.get(mode)
        return {"dpi": 1, "play_mode": "Normal", "task_mode": "None", "selected_building": None,
                "primary_window": "1v0", "world_epoch": 0,
                "save_catalog_context": {"available": True, "mode": mode, "session": session, "storage_root": "saves",
                "manual1": {"exists": mode != "SaveCatalog", "length": 0 if mode == "SaveCatalog" else 4,
                            "can_save": True, "can_load": mode != "SaveCatalog"},
                "controls": [{"action": action, "session": session, "slot": "Manual1", "entity": "button", "camera": "camera",
                              "window": "1v0", "physical_rect": [20, 40, 120, 80], "physical_viewport": [1280, 720]}] if action else []}}

    def test_control_requires_exact_actual_session_camera_window_and_fresh_slot(self):
        self.assertEqual(probe.control(self.sample(), "Save")[1], [70, 60])
        for change in ({"session": 6}, {"window": "foreign"}, {"camera": None}, {"slot": "Manual2"},
                       {"physical_rect": [0, 0, float("nan"), 1]}, {"physical_viewport": [640, 360]}):
            sample = self.sample()
            sample["save_catalog_context"]["controls"][0].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.control(sample, "Save")
        for change in ({"exists": True}, {"can_save": False}, {"length": 4}):
            sample = self.sample()
            sample["save_catalog_context"]["manual1"].update(change)
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                probe.control(sample, "Save")

    def test_original_click_not_sent_after_control_or_epoch_changes(self):
        initial = {"nonce": "nonce", "samples": [self.sample()], "events": []}
        for key, value in (("world_epoch", 1), ("primary_window", "foreign"), ("dpi", 2)):
            pointer = copy.deepcopy(initial)
            pointer["samples"].append(self.sample())
            pointer["samples"][-1].update(cursor_position=[70, 60])
            pointer["samples"][-1][key] = value
            transport = Mock()
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                probe.click(transport, Mock(side_effect=[initial, pointer]), "Save", "nonce", initial["samples"][-1])
            self.assertEqual([c.args[0] for c in transport.send.call_args_list], ["save-motion"])

    def test_no_input_when_context_changes_after_ready_wait(self):
        expected = self.sample()
        for key, value in (("world_epoch", 1), ("primary_window", "foreign"), ("session", 8), ("nonce", "foreign")):
            raw = {"nonce": "nonce", "samples": [self.sample()], "events": []}
            if key == "nonce":
                raw[key] = value
            elif key == "session":
                raw["samples"][-1]["save_catalog_context"][key] = value
                raw["samples"][-1]["save_catalog_context"]["controls"][0][key] = value
            else:
                raw["samples"][-1][key] = value
            transport = Mock()
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                probe.click(transport, lambda: raw, "Save", "nonce", expected)
            transport.send.assert_not_called()

    def test_file_hash_is_actual_unique_job_local_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "runtime/saves"
            directory.mkdir(parents=True)
            path = directory / "manual-1.scn.ron"
            path.write_bytes(b"real test bytes")
            result = probe.file_identity(root)
            self.assertEqual(result["bytes"], 15)
            self.assertEqual(result["path"], "runtime/saves/manual-1.scn.ron")
            self.assertEqual(path.read_bytes(), b"real test bytes")
            link = directory / "hardlink"
            link.hardlink_to(path)
            with self.assertRaises(RuntimeError):
                probe.file_identity(root)
            link.unlink()
            path.unlink()
            path.symlink_to(directory / "hardlink")
            with self.assertRaises(OSError):
                probe.file_identity(root)

    def test_planning_requires_actual_map_and_two_pending_children(self):
        footprint = [[1, 1], [2, 1], [1, 2], [2, 2]]
        sample = {"blueprints": [{"entity": "parent", "kind": "Tank", "footprint": footprint, "progress": 0,
                                  "delivered_materials": {}, "flexible_material_requirement": None}],
                  "owner_layers": [{"entity": "parent", "exists": True, "building_grids": footprint}],
                  "companion_context": {"available": True, "pending": None, "storages": [
                      {"entity": child, "pending_owner": "parent", "owner": None, "grid": [i, 1],
                       "stockpile_grids": [[i, 1]], "position": [i * 32, 32, 0]} for i, child in enumerate(("a", "b"))]}}
        self.assertEqual(probe.planning(sample)[:2], ("parent", {"a", "b"}))
        for bad in ("map", "child", "pending", "owner"):
            changed = copy.deepcopy(sample)
            if bad == "map":
                changed["owner_layers"][0]["building_grids"] = []
            elif bad == "child":
                changed["companion_context"]["storages"].pop()
            elif bad == "pending":
                changed["companion_context"]["pending"] = {"kind": "Tank"}
            else:
                changed["companion_context"]["storages"][0]["pending_owner"] = "foreign"
            with self.subTest(bad=bad), self.assertRaises(RuntimeError):
                probe.planning(changed)

    def test_exact_save_load_binding_and_negative_outcomes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "runtime/saves").mkdir(parents=True)
            (root / "runtime/saves/manual-1.scn.ron").write_bytes(b"file")
            samples = []
            footprint = [[1, 1], [2, 1], [1, 2], [2, 2]]
            for i in range(15):
                mode = "SaveCatalog" if i in (1, 2) else "LoadCatalog" if i in (6, 7) else "LoadConfirmManual1" if i in (9, 10) else "Closed"
                sample = self.sample(mode, 7 if i < 6 else 8)
                sample.update(sample_index=i, real_seconds=i, cursor_position=[70, 60], world_epoch=1 if i >= 12 else 0, menu_state="Hidden")
                sample.update({category: [] for category in probe.lifecycle.TRANSITION_CATEGORIES})
                owner = "new" if i >= 12 else "old"
                sample["blueprints"] = [{"entity": owner, "kind": "Tank", "footprint": footprint, "progress": 0,
                                         "delivered_materials": {}, "flexible_material_requirement": None}]
                sample["owner_layers"] = [{"entity": owner, "exists": True, "building_grids": footprint}]
                sample["companion_context"] = {"available": True, "pending": None, "storages": [
                    {"entity": owner + str(j), "pending_owner": owner, "owner": None, "grid": [j, 1],
                     "stockpile_grids": [[j, 1]], "position": [j * 32, 32, 0]} for j in range(2)]}
                samples.append(sample)
            events = []
            def event(i, producer, operation, **fields):
                events.append(dict(sample_index=i, real_seconds=i, producer=producer, operation=operation, **fields))
            for state in ("Pressed", "Released"):
                event(1, "KeyboardInput", "KeyboardInput", key="F5", state=state, repeat=False, window="1v0")
            save_boundary = len(events)
            for state in ("Pressed", "Released"):
                event(3, "MouseButtonInput", "MouseButtonInput", button="Left", state=state, window="1v0")
            event(3, "SaveLoadOutcome", "Save", result="Succeeded", target="Manual slot 1", source="Manual(ManualCatalog { dialog_session: 7 })")
            for state in ("Pressed", "Released"):
                event(6, "KeyboardInput", "KeyboardInput", key="F9", state=state, repeat=False, window="1v0")
            load_boundary = len(events)
            for state in ("Pressed", "Released"):
                event(8, "MouseButtonInput", "MouseButtonInput", button="Left", state=state, window="1v0")
            confirm_boundary = len(events)
            for state in ("Pressed", "Released"):
                event(11, "MouseButtonInput", "MouseButtonInput", button="Left", state=state, window="1v0")
            event(12, "SaveLoadOutcome", "Load", result="Succeeded", target="Manual slot 1", source="Load(NormalCatalog { dialog_session: 8 })")
            for category, key in probe.lifecycle.TRANSITION_CATEGORIES.items():
                for row in samples[11][category]:
                    event(12, "production_snapshot_delta", "StateTransition", before_sample_index=11,
                          category=category, entity=row[key], before=row, after=None)
                for row in samples[12][category]:
                    event(12, "production_snapshot_delta", "StateTransition", before_sample_index=11,
                          category=category, entity=row[key], before=None, after=row)
            # State transition inventory is canonical entity order, not message order.
            deltas = sorted([e for e in events if e["producer"] == "production_snapshot_delta"],
                            key=lambda e: (list(probe.lifecycle.TRANSITION_CATEGORIES).index(e["category"]), e["entity"]))
            events = [e for e in events if e["producer"] != "production_snapshot_delta"] + deltas
            for i, row in enumerate(events):
                row["event_index"] = i
            raw = dict(schema_version=2, transition_contract="indexed-state-delta-v1", samples=samples, events=events)
            evidence = {"before": 0, "save": dict(before=1, pointer=2, after=4, boundary=save_boundary, point=[70, 60]),
                        "load": dict(before=6, pointer=7, boundary=load_boundary, point=[70, 60]),
                        "confirm": dict(before=9, pointer=10, after=14, boundary=confirm_boundary, point=[70, 60]),
                        "file": probe.file_identity(root)}
            probe.verify(raw, evidence, root)
            changed = copy.deepcopy(raw)
            changed["samples"][-1]["companion_context"]["storages"].append(copy.deepcopy(samples[0]["companion_context"]["storages"][0]))
            with self.assertRaises(RuntimeError):
                probe.verify(changed, evidence, root)
            # Tombstones are observations, not surviving entities or reservations.
            tombstone = dict(entity="old", exists=False, building_grids=[], floor_grids=[], door_grids=[], bridge_grids=[], stockpile_grids=[])
            # Test planning restoration directly; raw delta verifier separately
            # rejects adding an unrecorded owner-layer row to an existing trace.
            for key, value in (("exists", True), ("building_grids", footprint), ("stockpile_grids", [[1, 1]])):
                changed = copy.deepcopy(raw)
                row = tombstone | {key: value}
                changed["samples"][-1]["owner_layers"].append(row)
                changed["events"].append(dict(event_index=len(changed["events"]), sample_index=14, before_sample_index=13,
                    real_seconds=14, producer="production_snapshot_delta", operation="StateTransition",
                    category="owner_layers", entity="old", before=None, after=row))
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    probe.verify(changed, evidence, root)
            early = copy.deepcopy(evidence)
            early["confirm"]["after"] = 13
            with self.assertRaisesRegex(RuntimeError, "later settled"):
                probe.verify(raw, early, root)
            for key, value in (("result", "Failed(LoadNotFound)"), ("source", "Load(NormalCatalog { dialog_session: 7 })"),
                               ("target", "Manual slot 2")):
                changed = copy.deepcopy(raw)
                next(e for e in changed["events"] if e["producer"] == "SaveLoadOutcome" and e["operation"] == "Load")[key] = value
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    probe.verify(changed, evidence, root)
            (root / "runtime/saves/manual-1.scn.ron").write_bytes(b"changed")
            with self.assertRaises(RuntimeError):
                probe.verify(raw, evidence, root)


if __name__ == "__main__":
    unittest.main()
