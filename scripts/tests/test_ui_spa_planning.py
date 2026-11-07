"""Offline refusal tests, not proof of an executed native planning campaign."""
import copy
import hashlib
import importlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock

SKILL = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(SKILL))
spa = importlib.import_module("ui_spa_planning")
ui = importlib.import_module("ui_usability_acceptance")


class SpaPlanningTests(unittest.TestCase):
    def test_keyboard_waits_for_actual_delayed_pair_without_resending(self):
        before = {"nonce": "owned", "pid": 10, "spa_planning": {"real_seconds": 1.0}}
        value = {"spa_planning": {"real_seconds": 1.3}}
        raw = {"nonce": "owned", "process_id": 10, "failure": None, "events": [
            {"producer": "KeyboardInput", "state": state, "key": "F5", "repeat": False, "real_seconds": 1.25}
            for state in ("Pressed", "Released")]}
        self.assertTrue(spa.keyboard_recorded(raw, before, value, "F5"))
        self.assertFalse(spa.keyboard_recorded(raw, before, {"spa_planning": {"real_seconds": 1.2}}, "F5"))
        for change in ({"key": "F9"}, {"repeat": True}, {"state": "Pressed"}):
            changed = copy.deepcopy(raw)
            changed["events"][1].update(change)
            self.assertFalse(spa.keyboard_recorded(changed, before, value, "F5"))
        for change in ({"nonce": "foreign"}, {"process_id": 11}, {"failure": "overflow"}):
            with self.subTest(change=change), self.assertRaises(spa.native.AcceptanceError):
                spa.keyboard_recorded(dict(raw, **change), before, value, "F5")
        with tempfile.TemporaryDirectory() as directory:
            driver = Mock(root=Path(directory), last={"frame": 2}, events=[{}, {}])
            with unittest.mock.patch.object(spa, "input_context", return_value=before), \
                    unittest.mock.patch.object(spa, "record_action") as record:
                driver.wait.return_value = {"frame": 3}
                spa.tap(driver, "F5")
                driver.key.assert_called_once_with("F5")
                driver.wait.assert_called_once()
                self.assertEqual(driver.wait.call_args.kwargs["timeout"], 10)
                record.assert_called_once()

    def test_prepress_binds_latest_bounded_raw_not_post_commit_mode(self):
        before = {"spa_planning": {"real_seconds": 6.45}, "world_epoch": 0, "primary_window": 72}
        good = {"real_seconds": 6.21, "world_epoch": 0, "primary_window": "72v0", "primary_window_bits": 72,
                "menu_state": "Architect", "task_mode": "SoulSpaPlace(None)"}
        committed = dict(good, real_seconds=6.50, task_mode="None")
        spa.placement_prepress({"samples": [good, committed]}, before, "72v0")
        for change in ({"real_seconds": 5.44}, {"world_epoch": 1}, {"primary_window": "foreign"},
                       {"primary_window_bits": 99}, {"menu_state": "Hidden"}, {"task_mode": "None"}):
            with self.subTest(change=change), self.assertRaises(spa.native.AcceptanceError):
                spa.placement_prepress({"samples": [dict(good, **change), committed]}, before, "72v0")
        for samples in ([], [committed], [good, dict(good, real_seconds=6.40, task_mode="None")]):
            with self.subTest(samples=samples), self.assertRaises(spa.native.AcceptanceError):
                spa.placement_prepress({"samples": samples}, before, "72v0")

    def test_cancel_key_matches_actual_ui_intent_field_order(self):
        self.assertEqual(spa.cancel_key("865v1"),
                         "menu:CancelSoulSpaConstruction { source_task: None, target: 865v1 }")

    def test_world_press_keeps_actual_architect_menu_and_rejects_foreign_mode_before_press(self):
        for menu, mode, valid in (("Architect", "SoulSpaPlace(None)", True),
                                  ("Hidden", "SoulSpaPlace(None)", False), ("Architect", "None", False)):
            with tempfile.TemporaryDirectory() as directory:
                driver = Mock()
                driver.root, driver.events = Path(directory), []
                driver.last = {"frame": 100, "nonce": "nonce", "pid": 10, "world_epoch": 0,
                               "primary_window": 3, "paused": False, "cursor": [100, 100], "scale_factor": 1,
                               "mouse_left_pressed": False, "pointer_over_ui": False, "world_input_captured": False,
                               "viewport": [1920, 1080], "controls": {}, "menu_state": menu,
                               "spa_planning": {"point": [100, 100], "legal": True, "footprint": [[10, 10]],
                                                "real_seconds": 1., "task_mode": mode}}
                def send(**event):
                    driver.events.append(event)
                    driver.last["frame"] += 10
                    driver.last["spa_planning"]["real_seconds"] += .1
                driver.send.side_effect = send
                if valid:
                    spa.world_click(driver, "placement", "point")
                    self.assertEqual(driver.send.call_count, 3)
                    self.assertEqual(driver.send.call_args_list[1].kwargs, {"button": 1, "pressed": True})
                    self.assertEqual(driver.events[1]["spa_action"]["before"]["menu_state"], "Architect")
                else:
                    with self.assertRaises(spa.native.AcceptanceError):
                        spa.world_click(driver, "placement", "point")
                    driver.send.assert_called_once_with(point=(100, 100))

    def test_spa_helper_is_in_every_frozen_harness_inventory(self):
        from scripts.perf_tool import execution, renderdoc_capture
        relative = ".codex/skills/hell-workers-run-native-acceptance/scripts/ui_spa_planning.py"
        for inventory in (ui.native.NATIVE_HARNESS_FILES, execution.MEASUREMENT_HARNESS_FILES,
                          renderdoc_capture.MEASUREMENT_HARNESS_FILES):
            self.assertIn(relative, inventory)
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / relative
            helper.parent.mkdir(parents=True)
            helper.write_text("before", encoding="utf-8")
            with patch.object(ui.native, "NATIVE_HARNESS_FILES", (relative,)):
                before = ui.native.native_harness_fingerprint(root)
                helper.write_text("after", encoding="utf-8")
                self.assertNotEqual(before, ui.native.native_harness_fingerprint(root))

    def state(self, owner=None):
        footprint = [[10, 10], [11, 10], [10, 9], [11, 9]]
        state = {"sites": [], "tiles": [], "souls": [], "working_relations": [], "grids": [],
                 "outcome_channels": {"construction_cancel": True, "slots_change": True}}
        if owner is None:
            return state
        state["sites"] = [{"entity": f"{owner}v0", "entity_bits": owner, "phase": "Constructing",
                           "bones_required": 12, "bones_delivered": 0, "active_slots": 4,
                           "building_kind": "SoulSpa", "provisional": False, "pose": {"translation": [1, 2, 3]},
                           "generator": {"current_output": 0.0, "output_per_soul": 1.0},
                           "grid": f"{owner * 100}v0", "map_building_grids": footprint}]
        state["grids"] = [{"entity": f"{owner * 100}v0", "entity_bits": owner * 100,
                           "generation": 0.0, "consumption": 0.0, "powered": True,
                           "generators": [f"{owner}v0"]}]
        state["tiles"] = [{"entity": f"{owner * 10 + i}v0", "entity_bits": owner * 10 + i,
                           "parent_site": f"{owner}v0", "parent_exists": True,
                           "grid": grid, "pose": {"translation": [i, i, 0]}, "workers": None,
                           "designation": None, "task_slots": None,
                           "map": {"owner": f"{owner}v0", "walkable": True}} for i, grid in enumerate(footprint)]
        return state

    def fixtures(self, save_path):
        states = [self.state(n) for n in (None, 1, 1, 2, None)]
        entries, samples = [], []
        for i, (case, state) in enumerate(zip(spa.CHECKPOINTS, states, strict=True)):
            owner = state["sites"][0]["entity_bits"] if state["sites"] else None
            footprint = [[10, 10], [11, 10], [10, 9], [11, 9]]
            plan = {"anchor": [10, 10], "footprint": footprint, "draw_pos": [1, 2],
                    "real_seconds": (1, 10, 20, 40, 50)[i], "virtual_seconds": 0,
                    "legal": owner is None, "context": state,
                    "cells": [{"grid": grid, "owner": owner, "walkable": True} for grid in footprint]}
            value = {"nonce": "nonce", "pid": 10, "paused": True, "ready": True, "frame": int(plan["real_seconds"] * 100),
                     "task_rows": 0, "world_epoch": int(i >= 3), "primary_window": 3, "spa_planning": plan}
            entries.append({"case": case, "value": value, "last_step": i + 1})
            samples.append({"sample_index": i, "real_seconds": plan["real_seconds"], "virtual_seconds": 0,
                            "paused": True, "world_epoch": int(i >= 3), "spa_context": copy.deepcopy(state)})
        mapping = [{"saved_entity": a["entity"], "saved_bits": a["entity_bits"],
                    "loaded_entity": b["entity"], "loaded_bits": b["entity_bits"], "kind": kind}
                   for key, kind in (("sites", "SoulSpa"), ("tiles", "SoulSpaTile"), ("grids", "PowerGrid"))
                   for a, b in zip(states[2][key], states[3][key], strict=True)]
        payload = b"fixture save, not a real game execution"
        save_path.write_bytes(payload)
        events = []
        for i, op, source in ((2, "save", "Manual(ManualCatalog { dialog_session: 1 })"),
                              (3, "load", "Load(NormalCatalog { dialog_session: 2 })")):
            events.append({"operation": "DurableSave" if op == "save" else "DurableLoad",
                           "producer": f"ordinary-{op}-commit-v1", "result": "Committed",
                           "target": "Manual slot 1", "request_source": source,
                           "file_sha256": hashlib.sha256(payload).hexdigest(), "file_bytes": len(payload),
                           "world_epoch": int(i >= 3), "real_seconds": (19.15 if i == 2 else 39.15),
                           "sample_index": i, "event_index": len(events), "spa_context": copy.deepcopy(states[i])})
        events[1].update(previous_epoch=0, spa_entity_remap=mapping)
        events.append({"producer": "SoulSpaConstructionCancelOutcome", "operation": "SoulSpaConstructionCancel",
                       "owner": "2v0", "result": "Canceled { refunded_bones: 0 }", "refunded_bones": 0,
                       "sample_index": 4, "event_index": 2, "real_seconds": 49.15})
        inputs = []
        def sample(now, state, epoch, task="None"):
            return {"real_seconds": now, "virtual_seconds": 0, "paused": True, "world_epoch": epoch,
                    "spa_context": copy.deepcopy(state), "primary_window": "3v0", "cursor_position": [100, 100],
                    "dpi": 1, "task_mode": task, "menu_state": "Architect"}
        specs = [(2, "menu:ToggleArchitect"), (3, "menu:SelectArchitectCategory(Some(Plant))"),
                 (4, "menu:SelectTaskMode(SoulSpaPlace(None))"), (5, "placement"), (11, "F5"),
                 (19, "menu:SelectSaveCatalogSlot { slot: Manual1, session: 1 }"), (21, "F9"),
                 (25, "menu:SelectLoadCatalogSlot { slot: Manual1, session: 2 }"),
                 (39, "menu:ConfirmLoadCatalogSlot { slot: Manual1, session: 2 }"), (41, "select-loaded"),
                 (42, "menu:CancelSoulSpaConstruction { source_task: None, target: 2v0 }"),
                 (49, "menu:ConfirmSoulSpaConstructionCancel { target: 2v0, epoch: 1, ticket: 1 }")]
        for now, key in specs:
            before_state = self.state(None if now <= 5 else 1 if now <= 39 else 2)
            after_state = self.state(None if now == 49 or now < 5 else 1 if now < 39 else 2)
            before_epoch, after_epoch = int(now > 39), int(now >= 39)
            plan = copy.deepcopy(entries[0]["value"]["spa_planning"])
            plan.update(real_seconds=now, context=before_state, task_mode="SoulSpaPlace(None)" if key == "placement" else "None")
            before = {"nonce": "nonce", "pid": 10, "world_epoch": before_epoch, "primary_window": 3,
                      "frame": now * 100, "cursor": [100, 100], "spa_planning": plan,
                      "viewport": [1920, 1080], "scale_factor": 1,
                      "menu_state": "Architect",
                      "pointer_over_ui": False, "world_input_captured": False,
                      "control": {"fully_visible": True, "interaction": "Hovered", "rect": [90, 90, 110, 110]}}
            after = copy.deepcopy(before)
            after.update(frame=now * 100 + 30, world_epoch=after_epoch)
            after["spa_planning"].update(real_seconds=now + .3, context=after_state)
            first = len(inputs) + 1
            keyboard = key in {"F5", "F9"}
            for i, pressed in enumerate((True, False)):
                inputs.append({"step": str(len(inputs) + 1), "nonce": "nonce", "pid": 10, "window": 100,
                               "world_epoch": before_epoch if i == 0 else after_epoch,
                               "pre_frame": before["frame"] + i * 10, "post_frame": before["frame"] + i * 10 + 5,
                               "key": key if keyboard else None, "button": None if keyboard else 1, "pressed": pressed,
                               "observed_hover": {key: "Pressed"} if pressed else {}})
                when = now + .1 + i * .1
                samples.append(sample(when, before_state if i == 0 else after_state, before_epoch if i == 0 else after_epoch))
                event = {"producer": "KeyboardInput" if keyboard else "MouseButtonInput",
                         "operation": "KeyboardInput" if keyboard else "MouseButtonInput", "window": "3v0",
                         "state": "Pressed" if pressed else "Released", "real_seconds": when}
                event.update({"key": key, "repeat": False} if keyboard else {"button": "Left"})
                events.append(event)
            inputs[first - 1]["spa_action"] = {"key": key, "first_step": first, "last_step": len(inputs), "before": before, "after": after}
            samples.append(sample(now - .01, before_state, before_epoch, plan["task_mode"]))
            samples.append(sample(now + .01, before_state, before_epoch, plan["task_mode"]))
        events.append({"producer": "ordinary-building-placement-branch-v1", "operation": "PlacementOutcome",
                       "window": "3v0", "world_epoch": 0, "real_seconds": 5.15,
                       "outcome": {"kind": "SoulSpa", "grid": [10, 10], "status": "BuildingCommitted",
                                   "owner": "1v0", "footprint": footprint, "tile_owners": [f"{10+i}v0" for i in range(4)]}})
        samples.sort(key=lambda row: row["real_seconds"])
        for i, row in enumerate(samples):
            row.update(sample_index=i, primary_window="3v0", primary_window_bits=3)
        events.sort(key=lambda row: row["real_seconds"])
        for i, row in enumerate(events):
            row.update(event_index=i, sample_index=next(j for j, s in enumerate(samples) if s["real_seconds"] >= row["real_seconds"]))
        for entry in entries:
            entry["last_step"] = sum(row["post_frame"] < entry["value"]["frame"] for row in inputs)
        return ({"schema_version": 2, "scope": "production-raw-observations-v1", "failure": None,
                 "nonce": "nonce", "process_id": 10, "accepted": False, "performance_evidence": False,
                 "promotion_authority": False, "samples": samples, "events": events}, entries, {"nonce": "nonce", "pid": 10, "window": 100}, inputs)

    def test_exact_unfilled_fixture_and_five_checkpoint_recipe(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.fixtures(save)
            spa.verify(raw, entries, client, save, inputs)
            for entry in entries:
                ui.check_observation(entry["case"], entry["value"])
            self.assertIn("soul-spa-planning", ui.CASES)

    def completion_fixture(self, save_path):
        raw, old_entries, client, old_inputs = self.fixtures(save_path)
        entries = copy.deepcopy(old_entries[:2])
        for entry, name in zip(entries, spa.COMPLETION_CHECKPOINTS, strict=False):
            entry["case"] = name
        state = copy.deepcopy(entries[1]["value"]["spa_planning"]["context"])
        state["sites"][0].update(phase="Operational", bones_delivered=12)
        for tile in state["tiles"]:
            tile.update(designation="GeneratePower", task_slots=1)
        completed = copy.deepcopy(entries[1])
        completed.update(case=spa.COMPLETION_CHECKPOINTS[2], last_step=12)
        completed["value"].update(frame=3000)
        completed["value"]["spa_planning"].update(real_seconds=30., virtual_seconds=1., context=state)
        entries.append(completed)
        inputs = copy.deepcopy(old_inputs[:8])
        for now, paused in ((11, False), (29, True)):
            key = "menu:SetTimeSpeed(Paused)" if paused else "menu:SetTimeSpeed(Normal)"
            before = copy.deepcopy(inputs[0]["spa_action"]["before"])
            before.update(frame=now * 100, paused=not paused)
            before["spa_planning"].update(real_seconds=float(now), virtual_seconds=float(paused),
                                         context=state if paused else entries[1]["value"]["spa_planning"]["context"])
            after = copy.deepcopy(before)
            after.update(frame=now * 100 + 30, paused=paused)
            after["spa_planning"]["real_seconds"] = now + .3
            first = len(inputs)
            for i, pressed in enumerate((True, False)):
                row = copy.deepcopy(old_inputs[i])
                row.pop("spa_action", None)
                row.update(step=str(len(inputs) + 1), pre_frame=now * 100 + i * 10, post_frame=now * 100 + i * 10 + 5,
                           pressed=pressed, observed_hover={key: "Pressed"} if pressed else {})
                inputs.append(row)
            inputs[first]["spa_action"] = {"key": key, "first_step": first + 1, "last_step": first + 2,
                                          "before": before, "after": after}
        raw["samples"] = [s for s in raw["samples"] if s["real_seconds"] <= 10]
        raw["events"] = [e for e in raw["events"] if e["real_seconds"] <= 10]
        intermediate = copy.deepcopy(entries[1]["value"]["spa_planning"]["context"])
        intermediate["sites"][0]["bones_delivered"] = 6
        for now, pressed, virtual, paused, ctx in (
            (11.1, True, 0., True, entries[1]["value"]["spa_planning"]["context"]),
            (11.2, False, .1, False, entries[1]["value"]["spa_planning"]["context"]),
            (20., None, .5, False, intermediate),
            (29.1, True, 1., False, state), (29.2, False, 1., True, state), (30., None, 1., True, state)):
            sample = copy.deepcopy(raw["samples"][-1])
            sample.update(real_seconds=now, virtual_seconds=virtual, paused=paused, spa_context=copy.deepcopy(ctx),
                          cursor_position=[100, 100], dpi=1)
            raw["samples"].append(sample)
            if pressed is not None:
                raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput", "window": "3v0",
                                      "real_seconds": now, "state": "Pressed" if pressed else "Released", "button": "Left"})
        for i, sample in enumerate(raw["samples"]):
            sample["sample_index"] = i
        for i, event in enumerate(raw["events"]):
            event.update(event_index=i, sample_index=next(j for j, s in enumerate(raw["samples"])
                                                       if s["real_seconds"] >= event["real_seconds"]))
        return raw, entries, client, inputs

    def test_natural_completion_fixture_and_missing_evidence_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.completion_fixture(save)
            spa.verify_completion(raw, entries, client, inputs)
            for entry in entries:
                ui.check_observation(entry["case"], entry["value"])
            self.assertIn("soul-spa-completion", ui.CASES)
            for change in ("authority", "missing_input", "phase", "bones", "output", "designation", "slot",
                           "walkability", "foreign_parent", "foreign_grid", "ghost_worker", "epoch", "late"):
                raw, entries, client, inputs = self.completion_fixture(save)
                state = entries[-1]["value"]["spa_planning"]["context"]
                if change == "authority":
                    raw["promotion_authority"] = True
                elif change == "missing_input":
                    inputs.pop()
                elif change == "phase":
                    state["sites"][0]["phase"] = "Constructing"
                elif change == "bones":
                    state["sites"][0]["bones_delivered"] = 11
                elif change == "output":
                    state["sites"][0]["generator"]["current_output"] = 1
                elif change == "designation":
                    state["tiles"][0]["designation"] = None
                elif change == "slot":
                    state["tiles"][0]["task_slots"] = 2
                elif change == "walkability":
                    entries[-1]["value"]["spa_planning"]["cells"][0]["walkable"] = False
                elif change == "foreign_parent":
                    state["tiles"][0]["parent_site"] = "foreign"
                elif change == "foreign_grid":
                    state["grids"][0]["generators"] = ["foreign"]
                elif change == "ghost_worker":
                    state["tiles"][0]["workers"] = ["foreign"]
                elif change == "epoch":
                    entries[-1]["value"]["world_epoch"] = 1
                else:
                    entries[-1]["value"]["spa_planning"]["real_seconds"] = 300
                with self.subTest(change=change), self.assertRaises(spa.native.AcceptanceError):
                    spa.verify_completion(raw, entries, client, inputs)

    def test_completion_wait_is_bounded_and_never_assigns_or_completes_work(self):
        driver = Mock(last={"frame": 100})
        driver.wait.return_value = {"frame": 200}
        with unittest.mock.patch.object(spa, "exercise_placement") as place, \
                unittest.mock.patch.object(spa, "pause") as pause, unittest.mock.patch.object(spa, "wait_final_raw"):
            spa.exercise_completion(driver)
            place.assert_called_once_with(driver, spa.COMPLETION_CHECKPOINTS)
            self.assertEqual(pause.call_args_list, [unittest.mock.call(driver, False), unittest.mock.call(driver, True)])
            driver.wait.assert_called_once()
            self.assertEqual(driver.wait.call_args.kwargs["timeout"], 180)
            driver.capture.assert_called_once_with(spa.COMPLETION_CHECKPOINTS[-1])

    def test_completion_rejects_intermediate_subject_shape_and_clock_changes(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            for case in ("epoch", "window", "window_bits", "tile", "parent", "pose", "map", "grid",
                         "clock", "virtual_clock", "all_paused", "paused_progress", "counter", "missing_normal"):
                raw, entries, client, inputs = self.completion_fixture(save)
                middle = next(s for s in raw["samples"] if s["real_seconds"] == 20.)
                state = middle["spa_context"]
                if case == "epoch":
                    middle["world_epoch"] = 1
                elif case == "window":
                    middle["primary_window"] = "foreign"
                elif case == "window_bits":
                    middle["primary_window_bits"] = 99
                elif case == "tile":
                    state["tiles"][0]["entity_bits"] = 999
                elif case == "parent":
                    state["tiles"][0]["parent_site"] = "foreign"
                elif case == "pose":
                    state["tiles"][0]["pose"]["translation"][0] = 999
                elif case == "map":
                    state["tiles"][0]["map"]["walkable"] = False
                elif case == "grid":
                    state["sites"][0]["grid"] = "foreign"
                elif case == "clock":
                    middle["real_seconds"] = 1.
                elif case == "virtual_clock":
                    middle["virtual_seconds"] = -1.
                elif case == "all_paused":
                    for sample in raw["samples"]:
                        sample["paused"] = True
                elif case == "paused_progress":
                    for sample in raw["samples"]:
                        if 11.2 <= sample["real_seconds"] <= 29.1:
                            sample["paused"] = True
                            sample["virtual_seconds"] = 0.
                elif case == "counter":
                    state["sites"][0]["bones_delivered"] = 13
                else:
                    inputs[8]["spa_action"]["key"] = "menu:SetTimeSpeed(Paused)"
                with self.subTest(case=case), self.assertRaises(spa.native.AcceptanceError):
                    spa.verify_completion(raw, entries, client, inputs)

    def test_operational_worker_inverse_and_derived_output_are_exact(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.completion_fixture(save)
            value = entries[-1]["value"]
            state = value["spa_planning"]["context"]
            tile = state["tiles"][0]
            tile["workers"] = ["400v0"]
            state["souls"] = [{"entity": "400v0", "working_on": tile["entity"], "dream": 5., "fatigue": .1,
                               "stress": .2, "generate_power": {"tile": tile["entity"], "phase": "GoingToTile"}}]
            state["working_relations"] = [{"entity": "400v0", "target": tile["entity"], "is_soul": True}]
            state["sites"][0]["generator"]["current_output"] = 1.
            state["grids"][0]["generation"] = 1.
            spa.check_completion_observation(spa.COMPLETION_CHECKPOINTS[-1], value)
            state["souls"][0]["generate_power"]["phase"] = "Generating"
            spa.check_completion_observation(spa.COMPLETION_CHECKPOINTS[-1], value)
            for case in ("inverse", "foreign_task", "duplicate_worker", "non_soul", "output"):
                changed = copy.deepcopy(value)
                data = changed["spa_planning"]["context"]
                if case == "inverse":
                    data["working_relations"] = []
                elif case == "foreign_task":
                    data["souls"][0]["generate_power"]["tile"] = "foreign"
                elif case == "duplicate_worker":
                    data["tiles"][1]["workers"] = ["400v0"]
                elif case == "non_soul":
                    data["working_relations"][0]["is_soul"] = False
                else:
                    data["grids"][0]["generation"] = 0.
                with self.subTest(case=case), self.assertRaises(spa.native.AcceptanceError):
                    spa.check_completion_observation(spa.COMPLETION_CHECKPOINTS[-1], changed)

    def test_grid_numeric_identity_and_relationships_must_match_actual_remap(self):
        before, after = self.state(1), self.state(2)
        mapping = [{"saved_entity": a["entity"], "saved_bits": a["entity_bits"],
                    "loaded_entity": b["entity"], "loaded_bits": b["entity_bits"], "kind": kind}
                   for key, kind in (("sites", "SoulSpa"), ("tiles", "SoulSpaTile"), ("grids", "PowerGrid"))
                   for a, b in zip(before[key], after[key], strict=True)]
        spa.check_mapping(before, after, mapping)
        for change in ("missing_bits", "wrong_bits", "missing_mapping", "foreign_generator", "foreign_grid"):
            modified, remap = copy.deepcopy(after), copy.deepcopy(mapping)
            if change == "missing_bits":
                modified["grids"][0].pop("entity_bits")
            elif change == "wrong_bits":
                modified["grids"][0]["entity_bits"] = 999
            elif change == "missing_mapping":
                remap.pop()
            elif change == "foreign_generator":
                modified["grids"][0]["generators"] = ["foreign"]
            else:
                modified["sites"][0]["grid"] = "foreign"
            with self.subTest(change=change), self.assertRaises(spa.native.AcceptanceError):
                spa.check_mapping(before, modified, remap)

    def test_missing_foreign_or_changed_evidence_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            for case in range(18):
                raw, entries, client, inputs = self.fixtures(save)
                durable = [e for e in raw["events"] if e["operation"] in ("DurableSave", "DurableLoad")]
                cancel = next(e for e in raw["events"] if e["producer"] == "SoulSpaConstructionCancelOutcome")
                if case == 0:
                    raw["accepted"] = True
                elif case == 1:
                    raw["failure"] = "overflow"
                elif case == 2:
                    durable[1]["spa_entity_remap"].pop()
                elif case == 3:
                    durable[1]["spa_entity_remap"][0]["loaded_entity"] = "foreign"
                elif case == 4:
                    durable[1]["file_sha256"] = "0" * 64
                elif case == 5:
                    durable[0]["file_bytes"] = True
                elif case == 6:
                    cancel["result"] = "Paused"
                elif case == 7:
                    cancel["owner"] = "1v0"
                elif case == 8:
                    entries[3]["value"]["spa_planning"]["context"]["tiles"][0]["parent_site"] = "foreign"
                elif case == 9:
                    entries[1]["value"]["spa_planning"]["context"]["sites"][0]["bones_delivered"] = True
                elif case == 10:
                    for row in raw["samples"]:
                        if row["real_seconds"] >= 39:
                            row["paused"] = False
                elif case == 11:
                    durable[1]["previous_epoch"] = 1
                elif case == 12:
                    raw["events"].append(copy.deepcopy(raw["events"][-1]))
                elif case == 13:
                    durable[0]["real_seconds"] = 1
                elif case == 14:
                    durable[1]["spa_context"]["sites"][0]["active_slots"] = 0
                elif case == 15:
                    for row in raw["samples"]:
                        row["spa_context"]["outcome_channels"]["construction_cancel"] = False
                elif case == 16:
                    client["pid"] = 20
                else:
                    entries[4]["value"]["spa_planning"]["context"]["working_relations"] = [
                        {"entity": "worker", "target": "20v0"}]
                with self.subTest(case=case), self.assertRaises((spa.native.AcceptanceError, ValueError)):
                    spa.verify(raw, entries, client, save, inputs)

    def test_missing_replayed_foreign_or_unconfirmed_input_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            for case in range(17):
                raw, entries, client, inputs = self.fixtures(save)
                buttons = [e for e in raw["events"] if e["producer"] == "MouseButtonInput"]
                actions = [e["spa_action"] for e in inputs if "spa_action" in e]
                if case == 0:
                    inputs = []
                elif case == 1:
                    buttons[0]["window"] = "foreign"
                elif case == 2:
                    buttons[0]["state"] = "Released"
                elif case == 3:
                    raw["events"].remove(buttons[0])
                elif case == 4:
                    inputs[0]["observed_hover"] = {}
                elif case == 5:
                    inputs[0]["pressed"] = 1
                elif case == 6:
                    actions[-1]["key"] = actions[-1]["key"].replace("ticket: 1", "ticket: 0")
                elif case == 7:
                    actions[-1]["key"] = actions[-1]["key"].replace("epoch: 1", "epoch: 0")
                elif case == 8:
                    actions[0]["before"]["control"]["rect"] = [0, 0, 1, 1]
                elif case == 9:
                    actions[3]["before"]["spa_planning"]["task_mode"] = "None"
                elif case == 10:
                    actions[5]["key"] = actions[5]["key"].replace("session: 1", "session: 9")
                    inputs[actions[5]["first_step"] - 1]["observed_hover"] = {actions[5]["key"]: "Pressed"}
                elif case == 11:
                    inputs[0]["pid"] = 99
                elif case == 12:
                    buttons[0]["real_seconds"] = 0
                elif case == 13:
                    actions[-1]["first_step"] = 1
                elif case == 14:
                    # A consistent foreign UI identity must not match the raw primary window.
                    for action in actions:
                        for side in ("before", "after"):
                            action[side]["primary_window"] = 999
                elif case == 15:
                    # Move F5 and all its matching raw/frame facts before the
                    # placed capture; ordering alone would still accept it.
                    action = actions[4]
                    selected = inputs[action["first_step"] - 1:action["last_step"]]
                    sample_refs = {id(e): raw["samples"][e["sample_index"]] for e in raw["events"]}
                    for side in ("before", "after"):
                        action[side]["frame"] -= 500
                        action[side]["spa_planning"]["real_seconds"] -= 5
                    for row in selected:
                        row["pre_frame"] -= 500
                        row["post_frame"] -= 500
                    for row in raw["samples"]:
                        if 11 <= row["real_seconds"] < 12:
                            row["real_seconds"] -= 5
                    for row in raw["events"]:
                        if row.get("key") == "F5":
                            row["real_seconds"] -= 5
                    raw["samples"].sort(key=lambda row: row["real_seconds"])
                    for i, row in enumerate(raw["samples"]):
                        row["sample_index"] = i
                    raw["events"].sort(key=lambda row: row["real_seconds"])
                    for i, row in enumerate(raw["events"]):
                        row.update(event_index=i, sample_index=sample_refs[id(row)]["sample_index"])
                else:
                    # A selection is not permitted to change epoch, even with
                    # internally matching input/sample metadata.
                    action = actions[9]
                    action["after"]["world_epoch"] = 2
                    inputs[action["last_step"] - 1]["world_epoch"] = 2
                    for row in raw["events"]:
                        if row["producer"] == "MouseButtonInput" and 41.15 < row["real_seconds"] < 41.3:
                            raw["samples"][row["sample_index"]]["world_epoch"] = 2
                with self.subTest(case=case), self.assertRaises((spa.native.AcceptanceError, ValueError)):
                    spa.verify(raw, entries, client, save, inputs)

    def test_receipt_cannot_move_consistently_before_raw_press(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            for operation, start in (("PlacementOutcome", 5), ("DurableSave", 19), ("SoulSpaConstructionCancel", 49)):
                raw, entries, client, inputs = self.fixtures(save)
                receipt = next(e for e in raw["events"] if e["operation"] == operation)
                receipt.update(real_seconds=start + .05, sample_index=next(
                    i for i, s in enumerate(raw["samples"]) if s["real_seconds"] == start + .01))
                raw["samples"][receipt["sample_index"]]["real_seconds"] = start + .06
                raw["events"].sort(key=lambda e: e["real_seconds"])
                for i, row in enumerate(raw["events"]):
                    row["event_index"] = i
                with self.subTest(operation=operation), self.assertRaisesRegex(spa.native.AcceptanceError, "precedes"):
                    spa.verify(raw, entries, client, save, inputs)
            raw, entries, client, inputs = self.fixtures(save)
            receipt = next(e for e in raw["events"] if e["operation"] == "DurableSave")
            press = next(e for e in raw["events"] if e["producer"] == "MouseButtonInput"
                         and e["state"] == "Pressed" and 19 < e["real_seconds"] < 19.15)
            receipt.update(real_seconds=press["real_seconds"], sample_index=press["sample_index"])
            spa.verify(raw, entries, client, save, inputs)


if __name__ == "__main__":
    unittest.main()
