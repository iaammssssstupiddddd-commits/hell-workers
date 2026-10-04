"""Isolated MudMixer surface proposal; arithmetic pass is not numeric approval."""

import copy
import math
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

import validate_building_numeric_candidate as numeric
from building_clay_geometry import build, load_pilot


class MudMixerV2NumericTests(unittest.TestCase):
    def test_v2_adds_only_fixed_interior_surface_to_existing_roles(self):
        old = load_pilot("MudMixer")
        proposed = load_pilot("MudMixer", "v2")
        old_meshes = build("MudMixer", old)
        meshes = build("MudMixer", proposed)
        self.assertEqual(set(meshes), {"body", "rotor"})
        self.assertEqual(meshes["rotor"], old_meshes["rotor"])
        self.assertEqual(meshes["body"].triangles(), 132)
        self.assertEqual(meshes["body"].surfaces[-6:],
                         [f"mud0:{face}" for face in
                          ("bottom", "top", "front", "right", "back", "left")])
        self.assertEqual(meshes["body"].bounds(), old_meshes["body"].bounds())
        result = numeric.validate(Path(__file__).resolve().parents[1] / "fixtures" /
                                  "building-m2-v2.numeric-candidate.json")
        self.assertFalse(result["numeric_freeze"])
        self.assertFalse(result["runtime_published"])
        surface = numeric.expected_kind("MudMixer", "v2")["mud_surface"]
        self.assertEqual(surface["state"], "fixed_residue_not_fill_level")
        self.assertGreater(surface["floor_clearance_wu"], 0)
        self.assertGreater(surface["rotor_clearance_wu"], 0)

    def test_v2_rejects_intersections_state_and_geometry_changes(self):
        baseline = load_pilot("MudMixer", "v2")
        cases = {
            "floor": lambda c: c["construction"]["mud_box"].update(center=[0, 12.25, 0]),
            "rotor": lambda c: c["construction"]["mud_box"].update(center=[0, 13.75, 0]),
            "wall_x": lambda c: c["construction"]["mud_box"].update(center=[2, 12.5, 0]),
            "wall_z": lambda c: c["construction"]["mud_box"].update(center=[0, 12.5, 2]),
            "state": lambda c: c["states"].update(Active="Always spin"),
            "budget": lambda c: c["roles"]["body"].update(triangle_cap=120),
            "extra_box": lambda c: c["construction"]["body_boxes"].append(
                {"center": [0, 20, 0], "size": [1, 1, 1]}),
            "extra_role": lambda c: c["roles"].update(mud=copy.deepcopy(c["roles"]["body"])),
            "nonfinite": lambda c: c["construction"]["mud_box"].update(center=[0, math.inf, 0]),
            "bad_vector": lambda c: c["construction"]["mud_box"].update(size=[40, 0.5]),
        }
        for name, mutate in cases.items():
            with self.subTest(name=name):
                changed = copy.deepcopy(baseline)
                mutate(changed)
                original = numeric.load_pilot

                def contract(kind, revision="v1"):
                    return changed if (kind, revision) == ("MudMixer", "v2") else original(kind, revision)

                with patch.object(numeric, "load_pilot", side_effect=contract):
                    with self.assertRaises(ValueError):
                        numeric.expected_kind("MudMixer", "v2")


if __name__ == "__main__":
    unittest.main()
