"""Synthetic transport/domain refusals; no real native or approval evidence."""
import copy
from pathlib import Path
import tempfile
import unittest

from scripts import ui_spa_resumed_use as resumed
from scripts.tests import test_ui_spa_candidate_art as candidates
from scripts.tests import test_ui_spa_occupied_save as occupied


def campaign(save):
    raw, entries, client, inputs = occupied.campaign_fixture(save)
    raw["scope"] = resumed.SCOPE
    contract = candidates.contract()
    contract.update(case=resumed.CASE, scope=resumed.SCOPE)
    loaded = copy.deepcopy(resumed.spa.context(entries[6]["value"]))

    def state(active, dream):
        result = copy.deepcopy(loaded)
        result["relative_speed"] = 4.
        soul, tile = result["souls"][0], result["tiles"][0]
        soul.update(dream=dream, task="GeneratePower(...)" if active else "None",
                    working_on=tile["entity"] if active else None,
                    generate_power={"tile": tile["entity"], "phase": "Generating"} if active else None)
        tile["workers"] = [soul["entity"]] if active else None
        result["working_relations"] = [{"entity": soul["entity"], "target": tile["entity"],
                                        "is_soul": True, "target_is_spa_tile": True}] if active else []
        result["sites"][0]["generator"]["current_output"] = float(active)
        result["grids"][0].update(generation=float(active), powered=active)
        return result

    def sample(now, virtual, paused, context):
        row = copy.deepcopy(raw["samples"][-1])
        row.update(real_seconds=now, virtual_seconds=virtual, paused=paused,
                   world_epoch=1, spa_context=copy.deepcopy(context))
        row.pop("art_feedback", None)
        raw["samples"].append(row)

    specs = [(74, True, 8., state(False, 38.)), (76, False, 10., state(True, 38.)),
             (80, True, 10., state(True, 38.)), (82, False, 14., state(True, 36.)),
             (86, True, 14., state(True, 36.)), (104, False, 66.2, state(False, 9.8))]
    for now, play, virtual, context in specs:
        key = "menu:SetTimeSpeed(Super)" if play else "menu:SetTimeSpeed(Paused)"
        a = copy.deepcopy(inputs[0]["spa_action"]["before"])
        a.update(frame=now * 100, world_epoch=1, paused=play)
        a["spa_planning"].update(real_seconds=float(now), virtual_seconds=virtual, context=copy.deepcopy(context))
        b = copy.deepcopy(a)
        b.update(frame=now * 100 + 30, paused=not play)
        b["spa_planning"]["real_seconds"] = now + .3
        first = len(inputs)
        for i, pressed in enumerate((True, False)):
            row = copy.deepcopy(inputs[i])
            row.pop("spa_action", None)
            row.update(step=str(len(inputs) + 1), pre_frame=now * 100 + i * 10,
                       post_frame=now * 100 + i * 10 + 5, world_epoch=1,
                       key=None, button=1, pressed=pressed, observed_hover={key: "Pressed"} if pressed else {})
            inputs.append(row)
            when = now + .1 + i * .1
            sample(when, virtual, play if pressed else not play, context)
            raw["events"].append({"producer": "MouseButtonInput", "real_seconds": when,
                                   "window": "3v0", "state": "Pressed" if pressed else "Released", "button": "Left"})
        inputs[first]["spa_action"] = {"key": key, "first_step": first + 1, "last_step": first + 2,
                                       "before": a, "after": b}
    sample(102., 66., False, state(True, 9.9))
    sample(102.1, 66.2, False, state(False, 9.8))
    for now, virtual, context, name, step in ((77., 10., state(True, 38.), resumed.CHECKPOINTS[7], 34),
                                           (83., 14., state(True, 36.), resumed.CHECKPOINTS[8], 38),
                                           (105., 66.2, state(False, 9.8), resumed.CHECKPOINTS[9], 42)):
        sample(now, virtual, True, context)
        entry = copy.deepcopy(entries[6])
        entry.update(case=name, last_step=step)
        entry["value"].update(frame=int(now * 100))
        entry["value"]["spa_planning"].update(real_seconds=now, virtual_seconds=virtual, context=context)
        entries.append(entry)
    raw["samples"].sort(key=lambda row: row["real_seconds"])
    raw["events"].sort(key=lambda row: row["real_seconds"])
    for i, row in enumerate(raw["samples"]):
        row["sample_index"] = i
    for i, row in enumerate(raw["events"]):
        row.update(event_index=i, sample_index=next(j for j, sample in enumerate(raw["samples"])
                                                   if sample["real_seconds"] >= row["real_seconds"]))
    for entry in entries[2:]:
        candidates.attach(resumed.spa.raw_checkpoint(raw, entry["value"]), entry["value"], contract)
    return raw, entries, client, inputs, contract


class SpaResumedUseTests(unittest.TestCase):
    def test_same_virtual_requires_exact_dream_and_running_step_uses_consumption_rate(self):
        self.assertTrue(resumed.dream_step(38., 38., 10., 10.))
        self.assertFalse(resumed.dream_step(38., 37.99999, 10., 10.))
        self.assertTrue(resumed.dream_step(38., 37., 10., 12.))
        for args in ((38., 38., 10., 12.), (38., 36., 10., 12.),
                     (38., 39., 10., 12.), (38., 37., 12., 10.),
                     (True, 0., 10., 12.), (38., float("nan"), 10., 12.)):
            with self.subTest(args=args):
                self.assertFalse(resumed.dream_step(*args))

    def test_full_prefix_and_remapped_resumed_departure(self):
        with tempfile.TemporaryDirectory() as directory:
            save = Path(directory) / "saves/manual-1.scn.ron"
            save.parent.mkdir()
            raw, entries, client, inputs, contract = campaign(save)
            resumed.verify(raw, entries, client, save, inputs, contract)
            for change in ("scope", "case", "prefix-task", "clock", "epoch", "owner", "relation", "emission", "dream", "departure", "extra-input", "raw-cap", "event-index", "foreign-domain", "paused-consumption", "running-transfer"):
                r, e, c, i, adopted = copy.deepcopy((raw, entries, client, inputs, contract))
                final = resumed.spa.raw_checkpoint(r, e[-1]["value"])
                if change == "scope":
                    r["scope"] = resumed.art.SCOPE
                elif change == "case":
                    adopted["case"] = resumed.art.CASE
                elif change == "prefix-task":
                    resumed.spa.raw_checkpoint(r, e[6]["value"])["spa_context"]["souls"][0]["task"] = "GeneratePower(...)"
                elif change == "clock":
                    r["samples"][-2]["virtual_seconds"] += 1
                elif change == "epoch":
                    final["world_epoch"] = 2
                elif change == "owner":
                    final["spa_context"]["sites"][0]["entity"] = "foreign"
                elif change == "relation":
                    final["spa_context"]["working_relations"] = [{"entity": "400v1", "target": "10v1"}]
                elif change == "emission":
                    final["art_feedback"]["roots"][0]["parts"][1]["material_values"]["emissive_white"] = True
                elif change == "dream":
                    e[8]["value"]["spa_planning"]["context"]["souls"][0]["dream"] = 40.
                elif change == "departure":
                    next(s for s in r["samples"] if s["real_seconds"] == 102.)["spa_context"]["souls"][0]["dream"] = 30.
                elif change == "extra-input":
                    i[-2]["spa_action"]["key"] = "menu:SetTimeSpeed(Fast)"
                elif change == "raw-cap":
                    r["samples"].extend([copy.deepcopy(r["samples"][-1])] * 1200)
                elif change == "event-index":
                    r["events"][-1]["event_index"] += 1
                elif change == "foreign-domain":
                    r["events"][-1]["operation"] = "SoulSpaSlotsChange"
                elif change == "paused-consumption":
                    for sample in r["samples"]:
                        if sample["real_seconds"] in (80.1, 80.2):
                            sample["spa_context"]["souls"][0]["dream"] = 37.
                    for event in i:
                        action = event.get("spa_action")
                        if action and action["before"]["spa_planning"]["real_seconds"] == 80.:
                            for side in ("before", "after"):
                                action[side]["spa_planning"]["context"]["souls"][0]["dream"] = 37.
                elif change == "running-transfer":
                    middle = copy.deepcopy(resumed.spa.raw_checkpoint(r, e[7]["value"]))
                    middle.update(real_seconds=81., virtual_seconds=12., paused=False)
                    middle["spa_context"]["souls"][0]["dream"] = 36.
                    r["samples"].append(middle)
                    r["samples"].sort(key=lambda sample: sample["real_seconds"])
                    for n, sample in enumerate(r["samples"]):
                        sample["sample_index"] = n
                    for event in r["events"]:
                        event["sample_index"] = next(n for n, sample in enumerate(r["samples"])
                                                     if sample["real_seconds"] >= event["real_seconds"])
                expected = "per-sample Dream" if change in ("paused-consumption", "running-transfer") else ".*"
                with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, expected):
                    resumed.verify(r, e, c, save, i, adopted)


if __name__ == "__main__":
    unittest.main()
