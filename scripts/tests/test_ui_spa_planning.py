"""Offline refusal tests, not proof of an executed native planning campaign."""
import copy
import hashlib
import importlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

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
        state = {"sites": [], "tiles": [], "souls": [], "working_relations": [], "grids": [], "relative_speed": 1.,
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

    def persistence_fixture(self, save):
        raw, entries, client, inputs = self.completion_fixture(save)
        old_raw, old_entries, _, old_inputs = self.fixtures(save)
        def operational(state):
            for site in state["sites"]:
                site.update(phase="Operational", bones_delivered=12)
            for tile in state["tiles"]:
                tile.update(designation="GeneratePower", task_slots=1)
        for entry, name in zip(copy.deepcopy(old_entries[2:4]), spa.PERSISTENCE_CHECKPOINTS[3:], strict=True):
            entry.update(case=name, last_step=entry["last_step"] + 4)
            entry["value"]["frame"] += 3000
            entry["value"]["spa_planning"]["real_seconds"] += 30
            entry["value"]["spa_planning"]["virtual_seconds"] = 1.
            operational(entry["value"]["spa_planning"]["context"])
            entries.append(entry)
        for row in copy.deepcopy(old_inputs[8:18]):
            row.update(step=str(int(row["step"]) + 4), pre_frame=row["pre_frame"] + 3000, post_frame=row["post_frame"] + 3000)
            if "spa_action" in row:
                action = row["spa_action"]
                action.update(first_step=action["first_step"] + 4, last_step=action["last_step"] + 4)
                for side in ("before", "after"):
                    value = action[side]
                    value.update(frame=value["frame"] + 3000, paused=True)
                    value["spa_planning"]["real_seconds"] += 30
                    value["spa_planning"]["virtual_seconds"] = 1.
                    operational(value["spa_planning"]["context"])
            inputs.append(row)
        for sample in copy.deepcopy(old_raw["samples"]):
            if 10 < sample["real_seconds"] <= 40:
                sample.update(real_seconds=sample["real_seconds"] + 30, virtual_seconds=1.)
                operational(sample["spa_context"])
                raw["samples"].append(sample)
        for event in copy.deepcopy(old_raw["events"]):
            if 10 < event["real_seconds"] <= 40:
                event["real_seconds"] += 30
                if "spa_context" in event:
                    operational(event["spa_context"])
                raw["events"].append(event)
        for i, sample in enumerate(raw["samples"]):
            sample["sample_index"] = i
        for i, event in enumerate(raw["events"]):
            event.update(event_index=i, sample_index=next(j for j, s in enumerate(raw["samples"])
                                                       if s["real_seconds"] >= event["real_seconds"]))
        return raw, entries, client, inputs

    def test_empty_operational_persistence_and_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.persistence_fixture(save)
            spa.verify(raw, entries, client, save, inputs, persistence=True)
            for entry in entries:
                ui.check_observation(entry["case"], entry["value"])
            self.assertIn("soul-spa-persistence", ui.CASES)
            for change in ("remap", "hash", "epoch", "phase", "occupant", "paused", "input", "foreign_operation",
                           "pre_save_drift", "post_save_drift", "post_save_task", "early_epoch", "bool_epoch"):
                raw, entries, client, inputs = self.persistence_fixture(save)
                if change == "remap":
                    next(e for e in raw["events"] if e.get("operation") == "DurableLoad")["spa_entity_remap"].pop()
                elif change == "hash":
                    next(e for e in raw["events"] if e.get("operation") == "DurableSave")["file_sha256"] = "0" * 64
                elif change == "epoch":
                    entries[-1]["value"]["world_epoch"] = 0
                elif change == "phase":
                    spa.context(entries[-1]["value"])["sites"][0]["phase"] = "Constructing"
                elif change == "occupant":
                    spa.context(entries[-1]["value"])["tiles"][0]["workers"] = ["foreign"]
                elif change == "paused":
                    entries[-1]["value"]["paused"] = False
                elif change == "input":
                    inputs.pop()
                elif change == "foreign_operation":
                    raw["events"].append(dict(raw["events"][-1], event_index=len(raw["events"]), operation="SoulSpaSlotsChange"))
                elif change == "pre_save_drift":
                    sample = next(s for s in raw["samples"] if 30 < s["real_seconds"] < 49)
                    sample["spa_context"]["sites"][0]["bones_delivered"] = 11
                else:
                    sample = next(s for s in raw["samples"] if 50 < s["real_seconds"] < 69)
                    if change == "post_save_drift":
                        sample["spa_context"]["sites"][0].update(phase="Constructing", bones_delivered=11)
                    elif change == "post_save_task":
                        sample["spa_context"]["souls"] = [{"entity": "ghost", "working_on": None,
                                                         "generate_power": {"tile": "10v0", "phase": "GoingToTile"}}]
                    elif change == "bool_epoch":
                        sample["world_epoch"] = False
                    else:
                        sample["world_epoch"] = 1
                with self.subTest(change=change), self.assertRaises(spa.native.AcceptanceError):
                    spa.verify(raw, entries, client, save, inputs, persistence=True)

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

    def test_persistence_construction_has_one_separate_bounded_wait(self):
        driver = Mock(last={"frame": 100})
        driver.wait.return_value = {"frame": 200}
        with patch.object(spa, "exercise_placement"), patch.object(spa, "pause"), patch.object(spa, "wait_final_raw"):
            spa.exercise_completion(driver, persistence=True)
        driver.wait.assert_called_once()
        self.assertEqual(driver.wait.call_args.kwargs["timeout"], 300)
        driver.send.assert_not_called()
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            for factor in (12, 16):
                raw, entries, client, inputs = self.completion_fixture(save)
                def retime(row):
                    if row["real_seconds"] > 10:
                        row["real_seconds"] = 10 + factor * (row["real_seconds"] - 10)
                for row in raw["samples"] + raw["events"]:
                    retime(row)
                for entry in entries:
                    retime(entry["value"]["spa_planning"])
                for row in inputs:
                    if "spa_action" in row:
                        for side in ("before", "after"):
                            retime(row["spa_action"][side]["spa_planning"])
                with self.assertRaises(spa.native.AcceptanceError):
                    spa.verify_completion(raw, entries, client, inputs)
                if factor == 12:
                    spa.verify_completion(raw, entries, client, inputs, persistence=True)
                else:
                    with self.assertRaises(spa.native.AcceptanceError):
                        spa.verify_completion(raw, entries, client, inputs, persistence=True)

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

    def usage_fixture(self, save_path):
        raw, entries, client, inputs = self.completion_fixture(save_path)
        empty = copy.deepcopy(entries[-1]["value"]["spa_planning"]["context"])
        def occupied(dream, fatigue):
            state = copy.deepcopy(empty)
            tile = state["tiles"][0]
            tile["workers"] = ["400v0"]
            state["souls"] = [{"entity": "400v0", "entity_bits": 400, "working_on": tile["entity"],
                               "dream": dream, "fatigue": fatigue, "stress": .1,
                               "generate_power": {"tile": tile["entity"], "phase": "Generating"}}]
            state["working_relations"] = [{"entity": "400v0", "target": tile["entity"], "is_soul": True}]
            state["sites"][0]["generator"]["current_output"] = 1.
            state["grids"][0]["generation"] = 1.
            return state
        for now, paused in ((31, False), (49, True), (51, False), (54, True)):
            key = "menu:SetTimeSpeed(Fast)" if now == 31 else "menu:SetTimeSpeed(Paused)" if paused else "menu:SetTimeSpeed(Normal)"
            before = copy.deepcopy(inputs[8]["spa_action"]["before"])
            before.update(frame=now * 100, paused=not paused)
            before["spa_planning"]["real_seconds"] = float(now)
            before["spa_planning"]["context"]["relative_speed"] = 1. if now in (31, 54) else 2.
            after = copy.deepcopy(before)
            after.update(frame=now * 100 + 30, paused=paused)
            after["spa_planning"]["real_seconds"] = now + .3
            after["spa_planning"]["context"]["relative_speed"] = 2. if now in (31, 49) else 1.
            first = len(inputs)
            for i, pressed in enumerate((True, False)):
                row = copy.deepcopy(inputs[i])
                row.pop("spa_action", None)
                row.update(step=str(len(inputs) + 1), pre_frame=now * 100 + i * 10, post_frame=now * 100 + i * 10 + 5,
                           pressed=pressed, observed_hover={key: "Pressed"} if pressed else {})
                inputs.append(row)
            inputs[first]["spa_action"] = {"key": key, "first_step": first + 1, "last_step": first + 2,
                                          "before": before, "after": after}
        timeline = [(31.1, True, 1., True, empty), (31.2, False, 1.1, False, empty),
                    (40., None, 2.5, False, empty), (49.1, True, 4., False, occupied(40., .1)),
                    (49.2, False, 4., True, occupied(40., .1)), (50., None, 4., True, occupied(40., .1)),
                    (51.1, True, 4., True, occupied(40., .1)), (51.2, False, 4.1, False, occupied(39.95, .1005)),
                    (53., None, 6., False, occupied(39., .11)), (54.1, True, 8., False, occupied(38., .12)),
                    (54.2, False, 8., True, occupied(38., .12)), (55., None, 8., True, occupied(38., .12))]
        for now, pressed, virtual, paused, state in timeline:
            state = copy.deepcopy(state)
            state["relative_speed"] = 2. if 31.2 <= now < 51.2 else 1.
            sample = copy.deepcopy(raw["samples"][-1])
            sample.update(real_seconds=now, virtual_seconds=virtual, paused=paused, spa_context=state)
            raw["samples"].append(sample)
            if pressed is not None:
                raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput", "window": "3v0",
                                      "real_seconds": now, "state": "Pressed" if pressed else "Released", "button": "Left"})
            if now in (50., 55.):
                entry = copy.deepcopy(entries[2])
                entry.update(case=spa.USAGE_CHECKPOINTS[len(entries)], last_step=16 if now == 50 else 20)
                entry["value"].update(frame=int(now * 100))
                entry["value"]["spa_planning"].update(real_seconds=now, virtual_seconds=virtual, context=copy.deepcopy(state))
                entries.append(entry)
        for i, sample in enumerate(raw["samples"]):
            sample["sample_index"] = i
        for i, event in enumerate(raw["events"]):
            event.update(event_index=i, sample_index=next(j for j, s in enumerate(raw["samples"])
                                                       if s["real_seconds"] >= event["real_seconds"]))
        return raw, entries, client, inputs

    def slots_fixture(self, save):
        raw, entries, client, inputs = self.usage_fixture(save)
        state = copy.deepcopy(entries[-1]["value"]["spa_planning"]["context"])
        paused, virtual, selected = True, 8., None
        def sample(now):
            value = copy.deepcopy(raw["samples"][-1])
            value.update(sample_index=len(raw["samples"]), real_seconds=now, virtual_seconds=virtual,
                         paused=paused, spa_context=copy.deepcopy(state))
            raw["samples"].append(value)
        for now, key in ((56, "select-operational"), (57, "menu:SetTimeSpeed(Normal)"),
                         *((58 + i, spa.slots_key("1v0", n)) for i, n in enumerate((3, 2, 1, 0))),
                         (62, "menu:SetTimeSpeed(Paused)"), (64, "menu:SetTimeSpeed(Normal)"),
                         (68, "menu:SetTimeSpeed(Paused)")):
            before = copy.deepcopy(inputs[0]["spa_action"]["before"])
            before.update(frame=now * 100, paused=paused, selected=selected, menu_state="Hidden")
            before["spa_planning"].update(real_seconds=float(now), virtual_seconds=virtual,
                                         context=copy.deepcopy(state), selection_point=[100, 100],
                                         selection_target={"owner": 1, "world": [0, 0], "point": [100, 100],
                                                           "hit": "Direct", "class": "Object", "rank": 0}, task_mode="None")
            first = len(inputs)
            for i, pressed in enumerate((True, False)):
                if not pressed:
                    if key == "select-operational":
                        selected = 1
                    elif key.startswith("menu:SetSoulSpaActiveSlots"):
                        state["sites"][0]["active_slots"] -= 1
                    else:
                        paused = key == "menu:SetTimeSpeed(Paused)"
                if now == 62:
                    virtual = 9.225
                elif now == 68:
                    virtual = 13.125
                elif not paused and now >= 58:
                    virtual = 8. + (now + .1 + i * .1 - 57.2) * .25
                sample(now + .1 + i * .1)
                row = copy.deepcopy(inputs[i])
                row.pop("spa_action", None)
                row.update(step=str(len(inputs) + 1), pre_frame=now * 100 + i * 10,
                           post_frame=now * 100 + i * 10 + 5, pressed=pressed,
                           observed_hover={key: "Pressed"} if pressed else {})
                inputs.append(row)
                raw["events"].append({"producer": "MouseButtonInput", "operation": "MouseButtonInput", "window": "3v0",
                                      "real_seconds": now + .1 + i * .1, "state": "Pressed" if pressed else "Released",
                                      "button": "Left", "sample_index": len(raw["samples"]) - 1})
            after = copy.deepcopy(before)
            after.update(frame=now * 100 + 30, paused=paused, selected=selected)
            after["spa_planning"].update(real_seconds=now + .3, virtual_seconds=virtual, context=copy.deepcopy(state))
            inputs[first]["spa_action"] = {"key": key, "first_step": first + 1, "last_step": first + 2,
                                          "before": before, "after": after}
            if key.startswith("menu:SetSoulSpaActiveSlots"):
                count = state["sites"][0]["active_slots"]
                raw["events"].append({"producer": "SoulSpaSlotsChangeOutcome", "operation": "SoulSpaSlotsChange",
                                      "owner": "1v0", "result": f"Applied {{ requested: {count}, applied: {count}, clamped: false }}",
                                      "applied": {"requested": count, "applied": count, "clamped": False},
                                      "sample_index": len(raw["samples"]) - 1, "real_seconds": now + .15})
            if now in (62, 68):
                sample(float(now + 1))
                entry = copy.deepcopy(entries[-1])
                entry.update(case=spa.SLOTS_CHECKPOINTS[len(entries)], last_step=len(inputs))
                entry["value"].update(frame=(now + 1) * 100, selected=selected)
                entry["value"]["spa_planning"].update(real_seconds=float(now + 1), virtual_seconds=virtual,
                                                       context=copy.deepcopy(state))
                entries.append(entry)
        raw["events"].sort(key=lambda e: e["real_seconds"])
        for i, event in enumerate(raw["events"]):
            event["event_index"] = i
        return raw, entries, client, inputs

    def test_slots_require_exact_inputs_outcomes_and_non_eviction(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.slots_fixture(save)
            client["input_ack_policy"] = "next-exact-observation-v1"
            spa.verify_slots(raw, entries, client, inputs)
            self.assertIn("soul-spa-slots", ui.CASES)
            for entry in entries:
                ui.check_observation(entry["case"], entry["value"])
            for mutation in ("missing-receipt", "foreign-owner", "boolean-count", "reordered-receipt", "early-epoch",
                             "eviction", "new-worker", "unpaused-capture", "wrong-selection", "extra-action", "short-dwell",
                             "paused-outcome", "boolean-speed", "boolean-epoch", "boolean-selection", "wrong-projection",
                             "orphan-assignment", "duplicate-receipt", "duplicate-sample", "sample-cap", "event-cap",
                             "early-selection", "early-slot"):
                raw, entries, client, inputs = self.slots_fixture(save)
                client["input_ack_policy"] = "next-exact-observation-v1"
                receipts = [e for e in raw["events"] if e.get("operation") == "SoulSpaSlotsChange"]
                if mutation == "missing-receipt":
                    raw["events"].remove(receipts[0])
                elif mutation == "foreign-owner":
                    receipts[0]["owner"] = "foreign"
                elif mutation == "boolean-count":
                    receipts[-1]["applied"]["requested"] = False
                elif mutation == "reordered-receipt":
                    receipts[-1]["sample_index"] = receipts[0]["sample_index"]
                elif mutation == "early-epoch":
                    raw["samples"][receipts[-1]["sample_index"]]["world_epoch"] = 1
                elif mutation in ("eviction", "new-worker"):
                    sample = raw["samples"][receipts[-1]["sample_index"] + 1]
                    state = sample["spa_context"]
                    if mutation == "eviction":
                        state["souls"][0]["generate_power"]["phase"] = "GoingToTile"
                    else:
                        tile = state["tiles"][1]
                        tile["workers"] = ["401v0"]
                        soul = copy.deepcopy(state["souls"][0])
                        soul.update(entity="401v0", entity_bits=401, working_on=tile["entity"])
                        soul["generate_power"]["tile"] = tile["entity"]
                        state["souls"].append(soul)
                        state["working_relations"].append({"entity": "401v0", "target": tile["entity"], "is_soul": True})
                        state["sites"][0]["generator"]["current_output"] = 2.
                        state["grids"][0]["generation"] = 2.
                elif mutation == "unpaused-capture":
                    entries[-1]["value"]["paused"] = False
                elif mutation == "wrong-selection":
                    inputs[20]["spa_action"]["after"]["selected"] = 2
                elif mutation == "extra-action":
                    inputs[22]["spa_action"]["key"] = "menu:SetTimeSpeed(Fast)"
                elif mutation == "paused-outcome":
                    raw["samples"][receipts[-1]["sample_index"]]["paused"] = True
                elif mutation == "boolean-speed":
                    raw["samples"][receipts[-1]["sample_index"]]["spa_context"]["relative_speed"] = True
                elif mutation == "boolean-epoch":
                    entries[-1]["value"]["world_epoch"] = False
                elif mutation == "boolean-selection":
                    inputs[20]["spa_action"]["after"]["selected"] = True
                elif mutation == "wrong-projection":
                    inputs[20]["spa_action"]["before"]["spa_planning"]["selection_point"] = []
                elif mutation == "orphan-assignment":
                    state = raw["samples"][receipts[-1]["sample_index"] + 1]["spa_context"]
                    state["souls"].append({"entity": "401v0", "generate_power": {
                        "tile": state["tiles"][1]["entity"], "phase": "GoingToTile"}})
                elif mutation == "duplicate-receipt":
                    raw["events"].append(copy.deepcopy(receipts[-1]))
                elif mutation == "duplicate-sample":
                    raw["samples"][-1]["sample_index"] = raw["samples"][-2]["sample_index"]
                elif mutation == "sample-cap":
                    raw["samples"] += [copy.deepcopy(raw["samples"][-1])] * 1201
                elif mutation == "event-cap":
                    raw["events"] += [copy.deepcopy(raw["events"][-1])] * 4097
                elif mutation in ("early-selection", "early-slot"):
                    action = inputs[20 if mutation == "early-selection" else 24]["spa_action"]
                    for side in ("before", "after"):
                        action[side]["frame"] -= 500
                        action[side]["spa_planning"]["real_seconds"] -= 5
                else:
                    entries[-1]["value"]["spa_planning"]["virtual_seconds"] = 10.
                with self.subTest(mutation=mutation), self.assertRaises(spa.native.AcceptanceError):
                    spa.verify_slots(raw, entries, client, inputs)

    def test_slot_admission_inventory_includes_inflight_tasks_before_arrival(self):
        state = self.state(1)
        tile = state["tiles"][0]
        state["souls"] = [{"entity": "400v0", "generate_power": {"tile": tile["entity"], "phase": "GoingToTile"}}]
        before = spa.spa_assignment_keys(state)
        self.assertEqual(before, {("400v0", tile["entity"])})
        tile["workers"] = ["400v0"]
        state["souls"][0]["working_on"] = tile["entity"]
        state["souls"][0]["generate_power"]["phase"] = "Generating"
        self.assertEqual(spa.spa_assignment_keys(state), before)

    def test_slots_reject_self_consistent_input_and_receipt_times_before_used_checkpoint(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.slots_fixture(save)
            client["input_ack_policy"] = "next-exact-observation-v1"
            actions = [row["spa_action"] for row in inputs if "spa_action" in row
                       and row["spa_action"]["first_step"] > entries[4]["last_step"]]
            for i, action in enumerate(actions):
                start, end = (action[side]["spa_planning"]["real_seconds"] for side in ("before", "after"))
                now = 54.4 + i * .04
                frame = 5440 + i * 4
                for side, offset in (("before", 0), ("after", 3)):
                    action[side]["frame"] = frame + offset
                    action[side]["spa_planning"]["real_seconds"] = now + offset * .01
                for j, row in enumerate(inputs[action["first_step"] - 1:action["last_step"]]):
                    row.update(pre_frame=frame + j, post_frame=frame + j + 1)
                for event in raw["events"]:
                    if start < event["real_seconds"] <= end:
                        if event.get("producer") == "MouseButtonInput":
                            event["real_seconds"] = now + (.01 if event["state"] == "Pressed" else .02)
                        elif event.get("operation") == "SoulSpaSlotsChange":
                            event["real_seconds"] = now + .015
            raw["events"].sort(key=lambda e: e["real_seconds"])
            for i, event in enumerate(raw["events"]):
                event["event_index"] = i
            # Ownership, exact physical pairs and annotations agree; only the
            # claimed phase is wrong. Do not accept late sample indices as proof.
            spa.check_raw_session(raw, client)
            spa.check_inputs(raw, entries, client, inputs, completion=True, usage=True, slots=True)
            with self.assertRaisesRegex(spa.native.AcceptanceError, "checkpoint phase"):
                spa.verify_slots(raw, entries, client, inputs)

    def test_slot_driver_does_not_retry_unknown_world_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.usage_fixture(save)
            driver = Mock(last=copy.deepcopy(entries[-1]["value"]))
            driver.last.update(selected=999, controls={})
            driver.last["spa_planning"].update(selection_point=[100, 100], selection_target={
                "owner": 1, "world": [0, 0], "point": [100, 100], "hit": "Direct", "class": "Object", "rank": 0})
            with patch.object(spa, "exercise_usage") as prefix, patch.object(spa, "world_click") as world:
                with self.assertRaises(spa.native.AcceptanceError):
                    spa.exercise_slots(driver)
                prefix.assert_called_once_with(driver)
                world.assert_called_once_with(driver, "select-operational", "selection_point")
                driver.send.assert_not_called()

    def test_selection_probe_is_rechecked_after_motion_before_press(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, _, _, inputs = self.slots_fixture(save)
            driver = Mock(last=copy.deepcopy(inputs[20]["spa_action"]["before"]), events=[])
            driver.last.update(mouse_left_pressed=False, controls={})
            def motion(**event):
                driver.last["frame"] += 1
                driver.last["spa_planning"]["selection_target"]["owner"] = 2
            driver.send.side_effect = motion
            with self.assertRaises(spa.native.AcceptanceError):
                spa.world_click(driver, "select-operational", "selection_point")
            driver.send.assert_called_once_with(point=(100, 100))

    def test_selection_probe_is_exact_owned_finite_and_on_existing_footprint(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, _, _, inputs = self.slots_fixture(save)
            baseline = inputs[20]["spa_action"]["before"]
            self.assertEqual(spa.check_selection_target(baseline), [100, 100])
            for patch_value in (None, {"owner": 2}, {"owner": True}, {"world": [500, 500]},
                                {"world": [float("nan"), 0]}, {"point": [float("inf"), 100]},
                                {"hit": "Snapped"}, {"class": "Soul"}, {"rank": True}, {"rank": -1}, {"rank": 256}):
                value = copy.deepcopy(baseline)
                if patch_value is None:
                    value["spa_planning"]["selection_target"] = None
                else:
                    value["spa_planning"]["selection_target"].update(patch_value)
                with self.subTest(patch_value=patch_value), self.assertRaises(spa.native.AcceptanceError):
                    spa.check_selection_target(value)
            missing_pose = copy.deepcopy(baseline)
            spa.context(missing_pose)["tiles"][0]["pose"] = None
            with self.assertRaisesRegex(spa.native.AcceptanceError, "pose"):
                spa.check_selection_target(missing_pose)
            driver = Mock(last=copy.deepcopy(baseline))
            driver.last["spa_planning"]["selection_target"] = None
            with patch.object(spa, "exercise_usage"), patch.object(spa, "world_click") as world:
                with self.assertRaises(spa.native.AcceptanceError):
                    spa.exercise_slots(driver)
                world.assert_not_called()
                driver.send.assert_not_called()

    def picker_scroll_fixture(self):
        key = "scroll:world-candidates:7:0"
        state = self.state(1)
        before = {"nonce": "n", "pid": 10, "world_epoch": 0, "primary_window": 3, "camera": ["fixed"],
                  "selected": 400, "paused": True, "pointer_over_ui": True, "world_input_captured": False,
                  "wheel_overflow": False, "wheel_events": [], "cursor": [150, 120], "frame": 10,
                  "spa_planning": {"context": state, "virtual_seconds": 8., "picker": {
                      "revision": 7, "epoch": 0, "anchor": [0, 0], "entries": [
                          {"entity": 400, "class": "Soul"}, {"entity": 1, "class": "Object"}]}},
                  "controls": {key: {"entity": 55, "fully_visible": True, "rect": [100, 100, 200, 200], "scroll": [0, 0]}}}
        after = copy.deepcopy(before)
        after["frame"] = 20
        after["controls"][key]["scroll"][1] = 24.
        after["wheel_events"] = [{"index": 0, "frame": 18, "epoch": 0, "window": 3,
                                  "phase": "Moved", "unit": "Line", "delta": [0, -1]}]
        motion = {"point": [150, 120], "button": None, "key": None, "nonce": "n", "pid": 10,
                  "window": 100, "world_epoch": 0, "pre_frame": 5, "post_frame": 10}
        sent = [{"step": str(i + 2), "button": 5, "pressed": pressed, "point": None, "key": None,
                 "nonce": "n", "pid": 10, "window": 100, "world_epoch": 0, "pre_frame": 10 + i * 3, "post_frame": 13 + i * 3}
                for i, pressed in enumerate((True, False))]
        action = {"key": key, "window": 100, "first_step": 2, "last_step": 3, "before": before, "after": after}
        return key, action, [motion, *sent]

    def test_slot_measurement_requires_every_initial_worker_to_be_safe(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.usage_fixture(save)
            state = spa.context(entries[-1]["value"])
            self.assertEqual(spa.safe_slot_workers(state).keys(), spa.generating(state).keys())
            unsafe = copy.deepcopy(state["souls"][0])
            unsafe.update(entity="unsafe")
            unsafe["fatigue"] = .733
            state["souls"].append(unsafe)
            state["tiles"][0]["workers"].append("unsafe")
            with self.assertRaisesRegex(spa.native.AcceptanceError, "unsafe"):
                spa.safe_slot_workers(state)

    def test_picker_wheel_requires_real_receipt_progress_and_unchanged_context(self):
        _, action, inputs = self.picker_scroll_fixture()
        spa.check_picker_scroll(action, inputs, action["after"]["wheel_events"])
        self.assertEqual(spa.picker_target(action["before"], 1), "world-candidate:1:7:0")
        for mutation in ("no-wheel", "extra-wheel", "wrong-window", "wrong-epoch", "wrong-direction", "overflow",
                         "no-progress", "list-rebuilt", "revision", "owner", "pointer", "camera", "world-time", "transport",
                         "foreign-motion", "foreign-pair", "early-wheel"):
            key, action, inputs = self.picker_scroll_fixture()
            b = action["after"]
            if mutation == "no-wheel":
                b["wheel_events"] = []
            elif mutation == "extra-wheel":
                b["wheel_events"] *= 2
            elif mutation == "wrong-window":
                b["wheel_events"][0]["window"] = 999
            elif mutation == "wrong-epoch":
                b["wheel_events"][0]["epoch"] = 1
            elif mutation == "wrong-direction":
                b["wheel_events"][0]["delta"][1] = 1
            elif mutation == "overflow":
                b["wheel_overflow"] = True
            elif mutation == "no-progress":
                b["controls"][key]["scroll"][1] = 0
            elif mutation == "list-rebuilt":
                b["controls"][key]["entity"] = 56
            elif mutation == "revision":
                b["spa_planning"]["picker"]["revision"] = 8
            elif mutation == "owner":
                b["spa_planning"]["picker"]["entries"][-1]["entity"] = 2
            elif mutation == "pointer":
                b["cursor"] = [201, 120]
            elif mutation == "camera":
                b["camera"] = ["changed"]
            elif mutation == "world-time":
                b["spa_planning"]["virtual_seconds"] += 1
            elif mutation == "foreign-motion":
                inputs[0]["window"] = 999
            elif mutation == "foreign-pair":
                inputs[1]["window"] = inputs[2]["window"] = 999
            elif mutation == "early-wheel":
                b["wheel_events"][0]["frame"] = 12
            else:
                inputs[2]["button"] = 1
            with self.subTest(mutation=mutation), self.assertRaises(spa.native.AcceptanceError):
                spa.check_picker_scroll(action, inputs, b["wheel_events"])

    def test_picker_inventory_rejects_absent_duplicate_foreign_and_boolean_owner(self):
        _, action, _ = self.picker_scroll_fixture()
        baseline = action["before"]
        for patch_value in ({"entries": []}, {"epoch": 1}, {"revision": True},
                            {"entries": [{"entity": 1, "class": "Object"}] * 2},
                            {"entries": [{"entity": True, "class": "Object"}, {"entity": 400, "class": "Soul"}]},
                            {"entries": [{"entity": 1, "class": "Resource"}, {"entity": 400, "class": "Soul"}]}):
            value = copy.deepcopy(baseline)
            value["spa_planning"]["picker"].update(patch_value)
            with self.subTest(patch_value=patch_value), self.assertRaises(spa.native.AcceptanceError):
                spa.picker_target(value, 1)

    def test_picker_wheel_phase_and_entire_history_are_bound_to_exact_client(self):
        _, action, inputs = self.picker_scroll_fixture()
        inputs[1]["spa_scroll"] = action
        lower = copy.deepcopy(action["before"])
        lower["frame"] = 1
        upper = copy.deepcopy(action["after"])
        upper["frame"] = 30
        entries = [{"value": lower, "last_step": 0}] * 5 + [
            {"value": upper, "last_step": 3}, {"value": upper, "last_step": 3}]
        client = {"nonce": "n", "pid": 10, "window": 100}
        self.assertEqual(spa.check_picker_scrolls(entries, client, inputs), {2, 3})
        for mutation in ("foreign-client", "foreign-window", "early-phase", "extra-wheel", "missing-annotation"):
            changed_entries, changed_inputs, changed_client = copy.deepcopy((entries, inputs, client))
            if mutation == "foreign-client":
                changed_client["nonce"] = "foreign"
            elif mutation == "foreign-window":
                changed_client["window"] = 999
            elif mutation == "early-phase":
                changed_entries[4]["value"]["frame"] = 11
            elif mutation == "extra-wheel":
                changed_entries[-1]["value"]["wheel_events"] *= 2
            else:
                changed_inputs[1].pop("spa_scroll")
            with self.subTest(mutation=mutation), self.assertRaises(spa.native.AcceptanceError):
                spa.check_picker_scrolls(changed_entries, changed_client, changed_inputs)

    def test_operational_selection_uses_one_world_click_and_scrolls_only_exact_picker(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, _, _, inputs = self.slots_fixture(save)
            baseline = copy.deepcopy(inputs[20]["spa_action"]["before"])
            key, action, _ = self.picker_scroll_fixture()
            baseline["spa_planning"]["picker"] = action["before"]["spa_planning"]["picker"]
            baseline["spa_planning"]["selection_target"]["rank"] = 1
            baseline["controls"] = {key: action["before"]["controls"][key]}
            driver = Mock(last=baseline)
            driver.wait.side_effect = lambda predicate, **kwargs: driver.last if predicate(driver.last) else self.fail("missing picker")
            def clicked(_, target):
                if target == "world-candidate:1:7:0":
                    driver.last["selected"] = 1
            def scrolled(_, target):
                self.assertEqual(target, key)
                driver.last["controls"]["world-candidate:1:7:0"] = {"fully_visible": True}
            with patch.object(spa, "world_click") as world, patch.object(spa, "click", side_effect=clicked) as click, \
                    patch.object(spa, "picker_scroll", side_effect=scrolled) as wheel:
                spa.select_operational(driver, {"entity_bits": 1})
                world.assert_called_once_with(driver, "select-operational", "selection_point")
                wheel.assert_called_once_with(driver, key)
                self.assertEqual([call.args[1] for call in click.call_args_list],
                                 ["world-candidate:toggle:7:0", "world-candidate:1:7:0"])
                driver.send.assert_not_called()

    def test_usage_requires_actual_worker_consumption_and_continuity(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs = self.usage_fixture(save)
            spa.verify_usage(raw, entries, client, inputs)
            for case in ("window", "epoch", "tile", "clock", "worker", "dream", "speed", "speed_action", "speed_bool", "paused_consumption",
                         "shifted_consumption", "output", "input", "authority"):
                raw, entries, client, inputs = self.usage_fixture(save)
                middle = next(s for s in raw["samples"] if s["real_seconds"] == 53.)
                if case == "window":
                    middle["primary_window"] = "foreign"
                elif case == "epoch":
                    middle["world_epoch"] = 1
                elif case == "tile":
                    middle["spa_context"]["tiles"][0]["parent_site"] = "foreign"
                elif case == "clock":
                    middle["virtual_seconds"] = 0.
                elif case == "worker":
                    middle["spa_context"]["souls"][0]["generate_power"]["phase"] = "GoingToTile"
                elif case == "dream":
                    middle["spa_context"]["souls"][0]["dream"] = 45.
                elif case == "speed":
                    middle["spa_context"]["relative_speed"] = 2.
                elif case in ("speed_action", "speed_bool"):
                    inputs[12]["spa_action"]["after"]["spa_planning"]["context"]["relative_speed"] = True if case == "speed_bool" else 1.
                elif case in ("paused_consumption", "shifted_consumption"):
                    # Preserve both checkpoint endpoints and the total .5/s decrease,
                    # but move consumption into a paused or a wrong running interval.
                    for sample in raw["samples"]:
                        if 51.1 <= sample["real_seconds"] < 55.:
                            sample["spa_context"]["souls"][0]["dream"] = (
                                38. if case == "paused_consumption" else 40. if sample["real_seconds"] < 54.1 else 38.)
                elif case == "output":
                    middle["spa_context"]["sites"][0]["generator"]["current_output"] = 0.
                elif case == "input":
                    inputs.pop()
                else:
                    raw["accepted"] = True
                with self.subTest(case=case), self.assertRaises(spa.native.AcceptanceError):
                    spa.verify_usage(raw, entries, client, inputs)

    def test_usage_wait_is_bounded_without_seed_or_retry(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.usage_fixture(save)
            driver = Mock()
            driver.last = entries[2]["value"]
            driver.wait.side_effect = [entries[3]["value"], entries[3]["value"], entries[4]["value"]]
            with patch.object(spa, "exercise_completion") as completion, patch.object(spa, "pause") as pause, \
                    patch.object(spa, "click") as click, \
                    patch.object(spa, "wait_final_raw") as finish:
                spa.exercise_usage(driver)
            completion.assert_called_once_with(driver)
            self.assertEqual([c.kwargs.get("timeout") for c in driver.wait.call_args_list], [None, 300, 15])
            click.assert_called_once_with(driver, "menu:SetTimeSpeed(Fast)")
            self.assertEqual([c.args[1] for c in pause.call_args_list], [True, False, True])
            self.assertEqual([c.args[0] for c in driver.capture.call_args_list], list(spa.USAGE_CHECKPOINTS[3:]))
            finish.assert_called_once_with(driver)
            driver.send.assert_not_called()

    def test_measurement_window_requires_real_worker_and_vital_headroom(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.usage_fixture(save)
            state = spa.context(entries[3]["value"])
            before = copy.deepcopy(state)
            self.assertEqual(set(spa.measurement_workers(state)), {"400v0"})
            self.assertEqual(state, before)
            for key, value in (("fatigue", .74), ("stress", .9), ("dream", 12), ("fatigue", None), ("fatigue", True)):
                changed = copy.deepcopy(state)
                changed["souls"][0][key] = value
                self.assertFalse(spa.measurement_workers(changed))
            changed = copy.deepcopy(state)
            changed["souls"][0]["generate_power"]["phase"] = "GoingToTile"
            self.assertFalse(spa.measurement_workers(changed))

    def test_collector_does_not_switch_to_first_unhealthy_worker_after_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            _, entries, _, _ = self.usage_fixture(save)
            using, used = (copy.deepcopy(e["value"]) for e in entries[3:])
            for value in (using, used):
                state = spa.context(value)
                healthy = copy.deepcopy(state["souls"][0])
                healthy.update(entity="999v0", entity_bits=999, fatigue=.2)
                healthy["generate_power"]["tile"] = state["tiles"][1]["entity"]
                healthy["working_on"] = state["tiles"][1]["entity"]
                state["tiles"][1]["workers"] = ["999v0"]
                state["souls"][0]["fatigue"] = .75
                state["souls"].append(healthy)
            # The fatigued ID-first worker leaves, while the selected healthy
            # worker remains. The measured predicate must follow the latter.
            spa.context(used)["souls"][0]["generate_power"] = None
            driver = Mock()
            driver.last = entries[2]["value"]
            driver.wait.side_effect = [using, using, used]
            with patch.object(spa, "exercise_completion"), patch.object(spa, "pause"), \
                    patch.object(spa, "click"), patch.object(spa, "wait_final_raw"):
                spa.exercise_usage(driver)
            self.assertTrue(driver.wait.call_args_list[-1].args[0](used))

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
