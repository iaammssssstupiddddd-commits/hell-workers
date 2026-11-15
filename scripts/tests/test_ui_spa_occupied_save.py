"""Synthetic offline mapping/refusal fixtures, never native acceptance evidence."""
import copy
import hashlib
import importlib
from pathlib import Path
import sys
import tempfile
import unittest

from scripts.tests import test_ui_spa_planning as planning_fixtures

SKILL = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(SKILL))
occupied = importlib.import_module("ui_spa_occupied_save")


def fixture():
    pose = {"translation": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}
    before = {
        "sites": [{"entity": "site-old", "entity_bits": 1, "grid": "grid-old",
                   "phase": "Operational", "bones_required": 12, "bones_delivered": 12,
                   "active_slots": 4, "generator": {"current_output": 2.0, "output_per_soul": 1.0}}],
        "tiles": [{"entity": f"tile{i}-old", "entity_bits": 10 + i, "parent_site": "site-old",
                   "map": {"owner": "site-old", "walkable": True}, "grid": [i % 2, -i // 2],
                   "workers": [f"soul{i}-old"] if i < 2 else None,
                   "designation": "GeneratePower", "task_slots": 1} for i in range(4)],
        "grids": [{"entity": "grid-old", "entity_bits": 20, "generation": 2.0,
                   "generators": ["site-old"], "powered": True}],
        "souls": [{"entity": f"soul{i}-old", "entity_bits": 30 + i, "dream": 25.0,
                   "fatigue": 0.1, "stress": 0.2, "pose": copy.deepcopy(pose),
                   "working_on": f"tile{i}-old", "task": "GeneratePower(...)",
                   "generate_power": {"tile": f"tile{i}-old", "phase": "Generating"}} for i in range(2)],
        "working_relations": [{"entity": f"soul{i}-old", "target": f"tile{i}-old"} for i in range(2)],
    }
    after = copy.deepcopy(before)
    spa_map, soul_map = [], []
    for category, kind in (("sites", "SoulSpa"), ("tiles", "SoulSpaTile"),
                           ("grids", "PowerGrid"), ("souls", "Soul")):
        for old, new in zip(before[category], after[category], strict=True):
            new["entity"] = old["entity"].replace("-old", "-new")
            new["entity_bits"] += 100
            row = {"kind": kind, "saved_entity": old["entity"], "saved_bits": old["entity_bits"],
                   "loaded_entity": new["entity"], "loaded_bits": new["entity_bits"]}
            (soul_map if category == "souls" else spa_map).append(row)
    after["sites"][0]["grid"] = "grid-new"
    after["grids"][0]["generators"] = ["site-new"]
    for tile in after["tiles"]:
        tile["parent_site"] = tile["map"]["owner"] = "site-new"
        tile["workers"] = None
    for soul in after["souls"]:
        soul.update(task="None", working_on=None, generate_power=None)
    after["working_relations"] = []
    return before, after, spa_map, soul_map


def campaign_fixture(save):
    raw, entries, client, inputs = planning_fixtures.SpaPlanningTests().usage_fixture(save)
    contexts = [sample["spa_context"] for sample in raw["samples"]]
    contexts += [entry["value"]["spa_planning"]["context"] for entry in entries]
    contexts += [row["spa_action"][side]["spa_planning"]["context"]
                 for row in inputs if "spa_action" in row for side in ("before", "after")]
    for state in contexts:
        for soul in state["souls"]:
            soul.update(task="GeneratePower(...)", pose={"translation": [1.0, 2.0, 3.0]})
    before = copy.deepcopy(occupied.spa.context(entries[-1]["value"]))
    after = copy.deepcopy(before)
    spa_map, soul_map = [], []
    lookup = {}
    for category, kind in (("sites", "SoulSpa"), ("tiles", "SoulSpaTile"),
                           ("grids", "PowerGrid"), ("souls", "Soul")):
        for old, new in zip(before[category], after[category], strict=True):
            new.update(entity=old["entity"].replace("v0", "v1"), entity_bits=old["entity_bits"] + 1000)
            lookup[old["entity"]] = new["entity"]
            (soul_map if category == "souls" else spa_map).append({
                "kind": kind, "saved_entity": old["entity"], "saved_bits": old["entity_bits"],
                "loaded_entity": new["entity"], "loaded_bits": new["entity_bits"]})
    after["sites"][0]["grid"] = lookup[before["sites"][0]["grid"]]
    after["grids"][0]["generators"] = [after["sites"][0]["entity"]]
    for tile in after["tiles"]:
        tile.update(parent_site=after["sites"][0]["entity"], workers=None)
        tile["map"]["owner"] = after["sites"][0]["entity"]
    for soul in after["souls"]:
        soul.update(task="None", working_on=None, generate_power=None)
    after["working_relations"] = []
    def sample(now, state, epoch):
        value = copy.deepcopy(raw["samples"][-1])
        value.update(real_seconds=now, virtual_seconds=8.0, paused=True, world_epoch=epoch,
                     spa_context=copy.deepcopy(state))
        raw["samples"].append(value)
    specs = [(60, "F5"), (62, "menu:SelectSaveCatalogSlot { slot: Manual1, session: 1 }"),
             (65, "F9"), (67, "menu:SelectLoadCatalogSlot { slot: Manual1, session: 2 }"),
             (69, "menu:ConfirmLoadCatalogSlot { slot: Manual1, session: 2 }")]
    for now, key in specs:
        a = copy.deepcopy(inputs[0]["spa_action"]["before"])
        a.update(frame=now * 100, paused=True, world_epoch=0)
        a["spa_planning"].update(real_seconds=float(now), virtual_seconds=8.0, context=copy.deepcopy(before))
        b = copy.deepcopy(a)
        b.update(frame=now * 100 + 30, world_epoch=int(now == 69))
        b["spa_planning"].update(real_seconds=now + .3, context=copy.deepcopy(after if now == 69 else before))
        first = len(inputs)
        keyboard = key in ("F5", "F9")
        for i, pressed in enumerate((True, False)):
            row = copy.deepcopy(inputs[i])
            row.pop("spa_action", None)
            row.update(step=str(len(inputs) + 1), pre_frame=now * 100 + i * 10,
                       post_frame=now * 100 + i * 10 + 5, world_epoch=int(now == 69 and not pressed),
                       key=key if keyboard else None, button=None if keyboard else 1, pressed=pressed,
                       observed_hover={key: "Pressed"} if pressed else {})
            inputs.append(row)
            when, epoch = now + .1 + i * .1, int(now == 69 and not pressed)
            sample(when, after if epoch else before, epoch)
            event = {"producer": "KeyboardInput" if keyboard else "MouseButtonInput",
                     "real_seconds": when, "window": "3v0", "state": "Pressed" if pressed else "Released"}
            event.update({"key": key, "repeat": False} if keyboard else {"button": "Left"})
            raw["events"].append(event)
        inputs[first]["spa_action"] = {"key": key, "first_step": first + 1, "last_step": first + 2,
                                      "before": a, "after": b}
    for now, state, epoch, name, last_step in (
            (64., before, 0, occupied.CHECKPOINTS[-2], 24),
            (72., after, 1, occupied.CHECKPOINTS[-1], 30)):
        sample(now, state, epoch)
        entry = copy.deepcopy(entries[-1])
        entry.update(case=name, last_step=last_step)
        entry["value"].update(frame=int(now * 100), world_epoch=epoch)
        entry["value"]["spa_planning"].update(real_seconds=now, context=copy.deepcopy(state))
        for cell in entry["value"]["spa_planning"]["cells"]:
            cell["owner"] = state["sites"][0]["entity_bits"]
        entries.append(entry)
    sample(71., after, 1)
    for op, now, state, epoch, session in (("save", 62.15, before, 0, 1), ("load", 69.15, after, 1, 2)):
        source = f"Manual(ManualCatalog {{ dialog_session: {session} }})" if op == "save" else f"Load(NormalCatalog {{ dialog_session: {session} }})"
        row = {"operation": "DurableSave" if op == "save" else "DurableLoad",
               "producer": f"ordinary-{op}-commit-v1", "result": "Committed", "target": "Manual slot 1",
               "request_source": source, "file_sha256": hashlib.sha256(save.read_bytes()).hexdigest(),
               "file_bytes": save.stat().st_size, "world_epoch": epoch, "real_seconds": now,
               "spa_context": copy.deepcopy(state)}
        if op == "load":
            row.update(previous_epoch=0, spa_entity_remap=spa_map, entity_remap=soul_map)
        raw["events"].append(row)
    raw["samples"].sort(key=lambda row: row["real_seconds"])
    raw["events"].sort(key=lambda row: row["real_seconds"])
    for i, row in enumerate(raw["samples"]):
        row["sample_index"] = i
    for i, row in enumerate(raw["events"]):
        row.update(event_index=i, sample_index=next(j for j, s in enumerate(raw["samples"])
                                                   if s["real_seconds"] >= row["real_seconds"]))
    return raw, entries, client, inputs


class OccupiedSpaSaveTests(unittest.TestCase):
    def test_campaign_and_transient_refusals(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            args = campaign_fixture(save)
            occupied.verify(args[0], args[1], args[2], save, args[3])
            for change in ("unpause", "time", "epoch", "soul", "task", "output", "hash", "session", "missing_map", "input",
                           "late_commit_time", "late_save_index", "premature_load"):
                raw, entries, client, inputs = campaign_fixture(save)
                load = next(e for e in raw["events"] if e.get("operation") == "DurableLoad")
                middle = next(s for s in raw["samples"] if s["real_seconds"] == 71.)
                if change == "unpause":
                    middle["paused"] = False
                elif change == "time":
                    middle["virtual_seconds"] += 1
                elif change == "epoch":
                    middle["world_epoch"] = 0
                elif change == "soul":
                    middle["spa_context"]["souls"][0]["dream"] += 1
                elif change == "task":
                    middle["spa_context"]["souls"][0]["task"] = "GeneratePower(...)"
                elif change == "output":
                    middle["spa_context"]["sites"][0]["generator"]["current_output"] = 0.
                elif change == "hash":
                    load["file_sha256"] = "0" * 64
                elif change == "session":
                    load["request_source"] = "Load(RecoveryCatalog { dialog_session: 2 })"
                elif change == "missing_map":
                    load["entity_remap"] = []
                elif change == "late_commit_time":
                    load["real_seconds"] = 71.5
                elif change == "late_save_index":
                    receipt = next(e for e in raw["events"] if e.get("operation") == "DurableSave")
                    receipt["sample_index"] = next(i for i, sample in enumerate(raw["samples"])
                                                   if sample["real_seconds"] == 69.1)
                elif change == "premature_load":
                    # Move the complete F9 pair before saved capture, including
                    # raw rows, frame ACKs and checkpoint ordinal: self-consistent
                    # transport is still the wrong declared phase.
                    action = next(e["spa_action"] for e in inputs if e.get("spa_action", {}).get("key") == "F9")
                    for value in (action["before"], action["after"]):
                        value["frame"] -= 200
                        value["spa_planning"]["real_seconds"] -= 2
                    for row in inputs:
                        if row.get("key") == "F9":
                            row["pre_frame"] -= 200
                            row["post_frame"] -= 200
                    for row in raw["events"]:
                        if row.get("key") == "F9":
                            row["real_seconds"] -= 2
                    for sample in raw["samples"]:
                        if sample["real_seconds"] in (65.1, 65.2):
                            sample["real_seconds"] -= 2
                    raw["samples"].sort(key=lambda sample: sample["real_seconds"])
                    raw["events"].sort(key=lambda event: event["real_seconds"])
                    for i, row in enumerate(raw["samples"]):
                        row["sample_index"] = i
                    for i, row in enumerate(raw["events"]):
                        row.update(event_index=i, sample_index=next(j for j, sample in enumerate(raw["samples"])
                                                                   if sample["real_seconds"] >= row["real_seconds"]))
                    entries[5]["last_step"] = 26
                else:
                    inputs[-2]["spa_action"]["key"] = "menu:SetTimeSpeed(Normal)"
                with self.subTest(change=change), self.assertRaises(occupied.native.AcceptanceError):
                    occupied.verify(raw, entries, client, save, inputs)

    def test_real_map_order_is_irrelevant_but_runtime_tasks_are_reset(self):
        before, after, spa_map, soul_map = fixture()
        original = copy.deepcopy((before, after, spa_map, soul_map))
        occupied.mapped_durable(before, after, list(reversed(spa_map)))
        lookup = occupied.mapped_souls(before, after, list(reversed(soul_map)))
        self.assertEqual(lookup, {"soul0-old": "soul0-new", "soul1-old": "soul1-new"})
        # Paused pending rebuild may retain saved output, not a Generating proof.
        self.assertEqual(after["sites"][0]["generator"]["current_output"], 2.0)
        self.assertFalse(occupied.spa.generating(after))
        self.assertEqual((before, after, spa_map, soul_map), original)

    def test_durable_mapping_rejects_omission_type_substitution_and_changes(self):
        for change in ("missing_map", "duplicate_map", "foreign_parent", "phase", "slots", "grid",
                       "generator", "typed_slot", "walkable", "foreign_owner"):
            with self.subTest(change=change), self.assertRaises(occupied.native.AcceptanceError):
                before, after, spa_map, _ = fixture()
                if change == "missing_map":
                    spa_map.pop()
                elif change == "duplicate_map":
                    spa_map[-1] = spa_map[0]
                elif change == "foreign_parent":
                    after["tiles"][0]["parent_site"] = "foreign"
                elif change == "phase":
                    after["sites"][0]["phase"] = "Constructing"
                elif change == "slots":
                    after["sites"][0]["active_slots"] = 0
                elif change == "grid":
                    after["grids"][0]["generation"] = 0.0
                elif change == "generator":
                    after["sites"][0]["generator"]["current_output"] = 0.0
                elif change == "typed_slot":
                    after["tiles"][0]["task_slots"] = True
                elif change == "walkable":
                    after["tiles"][0]["map"]["walkable"] = False
                else:
                    after["tiles"][0]["map"]["owner"] = "foreign"
                occupied.mapped_durable(before, after, spa_map)

    def test_soul_mapping_rejects_foreign_duplicate_stale_or_changed_workers(self):
        for change in ("missing", "duplicate", "foreign", "bits", "stale", "dream", "fatigue",
                       "stress", "pose", "task", "working_on", "tile_workers", "relation", "assignment"):
            with self.subTest(change=change), self.assertRaises(occupied.native.AcceptanceError):
                before, after, _, mapping = fixture()
                soul = after["souls"][0]
                if change == "missing":
                    mapping.pop()
                elif change == "duplicate":
                    mapping[1] = mapping[0]
                elif change == "foreign":
                    mapping[0]["kind"] = "Foreign"
                elif change == "bits":
                    mapping[0]["loaded_bits"] = True
                elif change == "stale":
                    soul["entity"] = mapping[0]["loaded_entity"] = "soul0-old"
                elif change in ("dream", "fatigue", "stress"):
                    soul[change] += 0.1
                elif change == "pose":
                    soul["pose"]["translation"][0] = 1.0
                elif change == "task":
                    soul["task"] = "GeneratePower(...)"
                elif change == "working_on":
                    soul["working_on"] = "tile0-new"
                elif change == "tile_workers":
                    after["tiles"][0]["workers"] = ["soul0-new"]
                elif change == "relation":
                    after["working_relations"] = [{"entity": "soul0-new", "target": "tile0-new"}]
                else:
                    soul["generate_power"] = {"tile": "tile0-new", "phase": "Generating"}
                occupied.mapped_souls(before, after, mapping)
