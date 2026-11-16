"""Generation observations must bind stable ownership and actual resident parts."""

import copy
import unittest

from scripts import building_tank_generation_feedback as generation


def snapshot(phase):
    identity = [None, generation.IDENTITIES[0], generation.IDENTITIES[0], generation.IDENTITIES[1], None][phase]
    part_generation = 1 if phase == 2 else phase
    handles = {
        "body": f"body-mesh-{part_generation}",
        "water": f"water-mesh-{part_generation}",
        "albedo": "atlas",
        "world_preview": "preview",
    }
    names = ("fallback",) if phase in (0, 4) else ("body", "water")
    return {
        "schema_version": 1,
        "nonce": "a" * 32,
        "pid": 123,
        "phase": phase,
        "stable": 30,
        "failure": None,
        "pending": generation.IDENTITIES[1] if phase == 2 else None,
        "retired_gone": True,
        "state": "Some(TankPartial)",
        "active": identity,
        "root_identity": identity,
        "owner": "owner",
        "root": "root",
        "owner_pos": [1.0, 2.0, 0.0],
        "root_pos": [1.0, 12.8 if phase in (0, 4) else 0.0, -2.0],
        "handles": handles if identity else None,
        "camera": {
            "position": [1.0, 2.0, 100.0],
            "scale": [1.0, 1.0, 1.0],
            "zoom": 1.0,
        },
        "parts": [
            {
                "name": name,
                "entity": f"{part_generation}-{name}",
                "visible": True,
                "layer_3d": True,
                "mesh_resident": True,
                "mesh": handles.get(name, "fallback-mesh"),
                "albedo_resident": True,
                "albedo": "preview" if phase == 3 and name == "body" else "atlas",
                "opaque": True,
                "lit": True,
                "body_paint": {"mode": 3.0 if phase == 3 and name == "body" else 0.82},
                "translation": [0.0, 12.0, 0.0] if name == "water" else [0.0, 0.0, 0.0],
                "rotation": [0.0, 0.0, 0.0, 1.0],
                "scale": [1.0, 1.0, 1.0],
            }
            for name in names
        ],
    }


class GenerationFeedbackTests(unittest.TestCase):
    def setUp(self):
        self.shots = [
            {"before": snapshot(i), "after": {**snapshot(i), "stable": 35}}
            for i in range(5)
        ]

    def test_valid_same_root_replacement_sequence(self):
        generation.check_sequence(self.shots, "a" * 32, 123)

    def test_rejects_foreign_or_unstable_and_old_parts(self):
        for change in (
            "nonce",
            "phase",
            "stable",
            "retired",
            "old-part",
            "root",
            "mesh",
            "identity",
            "clock",
            "pending-handle",
            "pending-role",
        ):
            shots = copy.deepcopy(self.shots)
            row = shots[2]["before"]
            if change == "nonce":
                row["nonce"] = "b" * 32
            elif change == "phase":
                row["phase"] = 1
            elif change == "stable":
                row["stable"] = 29
            elif change == "retired":
                row["retired_gone"] = False
            elif change == "old-part":
                shots[3]["before"]["parts"][0]["entity"] = shots[2]["before"]["parts"][
                    0
                ]["entity"]
                shots[3]["after"]["parts"][0]["entity"] = shots[2]["before"]["parts"][
                    0
                ]["entity"]
            elif change == "root":
                row["root"] = "new-root"
            elif change == "mesh":
                row["parts"][0]["mesh"] = "stale"
            elif change == "identity":
                row["active"] = generation.IDENTITIES[1]
            elif change == "pending-handle":
                for side in ("before", "after"):
                    shots[2][side]["handles"]["body"] = "other-body"
                    shots[2][side]["parts"][0]["mesh"] = "other-body"
            elif change == "pending-role":
                for side in ("before", "after"):
                    parts = shots[2][side]["parts"]
                    parts[0]["entity"], parts[1]["entity"] = parts[1]["entity"], parts[0]["entity"]
            else:
                shots[2]["after"]["stable"] = 30
            with self.subTest(change=change), self.assertRaises(ValueError):
                generation.check_sequence(shots, "a" * 32, 123)


if __name__ == "__main__":
    unittest.main()
