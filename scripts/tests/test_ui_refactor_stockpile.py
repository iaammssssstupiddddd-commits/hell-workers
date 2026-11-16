"""Target changes require the matching real pointer and button receipts."""

import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(
    0,
    str(
        Path(__file__).resolve().parents[2]
        / ".codex/skills/hell-workers-run-native-acceptance/scripts"
    ),
)
from scripts import ui_refactor_stockpile as stockpile


def values():
    result = []
    for i, case in enumerate(stockpile.CHECKPOINTS):
        selected = [None, 22, 22, 11, 11][i]
        cells = [
            {
                "index": k,
                "entity": entity,
                "map_owner": entity,
                "priority": "Normal",
                "target_amount": 10,
                "allow_export": True,
                "is_any": not (i >= (4 if k == 0 else 2)),
                "is_none": i >= (4 if k == 0 else 2),
                "point": {"rank": 0, "screen": [100.0 + 100 * k, 100.0]},
            }
            for k, entity in enumerate((11, 22))
        ]
        button = (
            None
            if i == 0
            else {
                "entity": 55,
                "key": "clear",
                "rect": [10, 10, 30, 30],
                "target": selected,
                "clear": True,
                "other_empty": True,
            }
        )
        value = {
            "paused": True,
            "world_epoch": 7,
            "frame": 100 + i * 100,
            "nonce": "owned",
            "pid": 123,
            "selected": selected,
            "inspection_target": selected,
            "pointer_over_ui": False,
            "scale_factor": 1.0,
            "world_input_captured": False,
            "controls": {}
            if button is None
            else {"clear": {"entity": 55, "fully_visible": True}},
            "refactor_stockpile": {"cells": cells, "button": button},
        }
        result.append({"case": case, "last_step": i * 3, "value": value})
    return result


def evidence():
    entries, events = values(), []
    for i, name in enumerate(("select-b", "clear-b", "select-a", "clear-a")):
        before = copy.deepcopy(entries[i]["value"])
        after = copy.deepcopy(entries[i + 1]["value"])
        before["frame"] += 2
        after["frame"] -= 2
        target = 22 if i < 2 else 11
        if i % 2 == 0:
            before["pointer_target"] = target
            cursor = [200.0, 100.0] if i == 0 else [100.0, 100.0]
        else:
            cursor = [20.0, 20.0]
        first = len(events) + 1
        for j, kind in enumerate(("move", "press", "release")):
            event = {
                "step": str(len(events) + 1),
                "nonce": "owned",
                "pid": 123,
                "window": 99,
                "world_epoch": 7,
                "pre_frame": 100 + i * 100 + j + 1,
                "post_frame": 100 + i * 100 + j + 2,
                "observed_cursor": cursor,
            }
            if kind == "move":
                event["point"] = cursor
                if i % 2:
                    event.update(
                        observed_hover={"clear": "Hovered"},
                        observed_hover_entities={"clear": 55},
                    )
            else:
                event.update(
                    button=1,
                    pressed=kind == "press",
                    observed_left_pressed=kind == "press",
                )
                if i % 2 and kind == "press":
                    event.update(
                        observed_hover={"clear": "Pressed"},
                        observed_hover_entities={"clear": 55},
                    )
            events.append(event)
        events[-1]["stockpile_action"] = {
            "name": name,
            "first": first,
            "last": len(events),
            "before": before,
            "after": after,
        }
    return entries, events, {"nonce": "owned", "pid": 123, "window": 99}


class StockpileTargetTests(unittest.TestCase):
    def test_valid_target_sequence(self):
        stockpile.check_sequence(*evidence())

    def test_wrong_target_or_input_is_rejected(self):
        for defect in (
            "button",
            "mouse",
            "selection",
            "cursor",
            "other",
            "state",
            "identity",
        ):
            entries, events, client = evidence()
            if defect == "button":
                events[4]["observed_hover_entities"]["clear"] = 999
            elif defect == "mouse":
                events[4]["pressed"] = False
            elif defect == "selection":
                events[2]["stockpile_action"]["before"]["selected"] = 22
            elif defect == "cursor":
                events[3]["observed_cursor"] = [80, 80]
            elif defect == "other":
                events[3]["key"] = "x"
            elif defect == "state":
                entries[2]["value"]["refactor_stockpile"]["cells"][0]["is_any"] = False
            else:
                events[0]["nonce"] = "foreign"
            with (
                self.subTest(defect=defect),
                self.assertRaises(stockpile.native.AcceptanceError),
            ):
                stockpile.check_sequence(entries, events, client)


if __name__ == "__main__":
    unittest.main()
