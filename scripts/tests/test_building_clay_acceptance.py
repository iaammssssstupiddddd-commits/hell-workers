"""M2 observed-state rejection cases; does not start a game or accept art."""

import copy
import unittest

from scripts import building_clay_acceptance as clay


def records(kind):
    contract = clay.load_pilot(kind)
    result = []
    for ordinal in range(4):
        state = ({"stored_water": [0, 1, 50, 50][ordinal], "capacity": 50}
                 if kind == "Tank" else {"refining": ordinal >= 2})
        row = {"kind": kind, "ordinal": ordinal, "center": [100, 200],
               "root_translation_wu": [100, 0, -200], "state": state, "parts": []}
        for role, spec in contract["roles"].items():
            position, visible = list(spec["translation_wu"]), True
            if role == "water":
                water = contract["states"][["Empty", "Partial", "Full", "Full"][ordinal]]
                position[1], visible = water["water_y_wu"], water["water_visible"]
            row["parts"].append({"mesh_role": role, "translation_wu": position,
                                 "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1], "visible": visible})
        result.append(row)
    return result


class ClayAcceptanceTests(unittest.TestCase):
    def test_all_paused_clay_states(self):
        for kind in clay.PILOTS:
            result = clay.inspect_records(kind, records(kind), clay.load_pilot(kind))
            self.assertEqual(result["specimens"], 4)

    def test_missing_duplicate_or_incorrect_pose_fails_closed(self):
        for kind in clay.PILOTS:
            mutations = [
                lambda rows: rows.pop(),
                lambda rows: rows[0].update(ordinal=1),
                lambda rows: rows[0].pop("parts"),
                lambda rows: rows[0]["parts"].pop(),
                lambda rows: rows[0]["root_translation_wu"].__setitem__(1, 12.8),
                lambda rows: rows[1]["parts"][1].update(translation_wu=[0, 0, 0]),
                lambda rows: rows[1]["parts"][1].update(rotation_xyzw=[0, 1, 0, 0]),
                lambda rows: rows[1]["parts"][1].update(scale=[32, 32, 32]),
                lambda rows: rows[1]["parts"][1].update(visible=False),
                lambda rows: rows[1]["parts"][1].update(translation_wu=[0, float("nan"), 0]),
            ]
            for index, mutate in enumerate(mutations):
                with self.subTest(kind=kind, mutation=index):
                    rows = copy.deepcopy(records(kind))
                    mutate(rows)
                    with self.assertRaises(ValueError):
                        clay.inspect_records(kind, rows, clay.load_pilot(kind))

    def test_empty_water_must_be_hidden_and_full_must_be_higher(self):
        for index, field, value in ((0, "visible", True), (2, "translation_wu", [0, 12, 0])):
            rows = records("Tank")
            rows[index]["parts"][1][field] = value
            with self.assertRaises(ValueError):
                clay.inspect_records("Tank", rows, clay.load_pilot("Tank"))


if __name__ == "__main__":
    unittest.main()
