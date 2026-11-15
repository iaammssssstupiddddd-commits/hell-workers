"""Isolated MudMixer surface proposal; arithmetic pass is not numeric approval."""

import copy
import math
import struct
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

import validate_building_numeric_candidate as numeric
from building_clay_geometry import build, load_pilot
from prepare_building_clay_runtime import state_geometry
from building_m2_art import face_layout
if __package__:
    from .test_building_clay import encoded_geometry, write_glb
else:
    from test_building_clay import encoded_geometry, write_glb


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
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for role, mesh in meshes.items():
                document, binary = encoded_geometry(mesh)
                offset = document["bufferViews"][2]["byteOffset"]
                uv = [point for face in face_layout("MudMixer", "v2")[role]
                      for point in face["uv"]]
                for index, (u, v) in enumerate(uv):
                    struct.pack_into("<ff", binary, offset + index * 8, u, 1 - v)
                write_glb(root / f"{role}.glb", document, binary)
            measured = state_geometry("MudMixer", root, "v2")
            self.assertEqual(measured["roles"]["body"]["triangles"], 132)
            self.assertEqual(measured["rotor_sweep"]["clearance_mud_surface_wu"], 1.25)

            body = root / "body.glb"
            document, binary = encoded_geometry(meshes["body"])
            offset = document["bufferViews"][2]["byteOffset"]
            uv = [point for face in face_layout("MudMixer", "v2")["body"]
                  for point in face["uv"]]
            for index, (u, v) in enumerate(uv):
                struct.pack_into("<ff", binary, offset + index * 8, u, 1 - v)
            document["accessors"][3]["count"] -= 36  # keep all vertices, omit mud indices
            write_glb(body, document, binary)
            with self.assertRaisesRegex(ValueError, "indexed face is missing"):
                state_geometry("MudMixer", root, "v2")
            document["accessors"][3]["count"] += 36
            index_offset = document["bufferViews"][3]["byteOffset"]
            last = index_offset + (document["accessors"][3]["count"] - 3) * 4
            original = struct.unpack_from("<III", binary, last)
            struct.pack_into("<III", binary, last, 0, 1, 2)
            write_glb(body, document, binary)
            with self.assertRaisesRegex(ValueError, "indexed face"):
                state_geometry("MudMixer", root, "v2")
            struct.pack_into("<III", binary, last, original[0], original[2], original[1])
            write_glb(body, document, binary)
            with self.assertRaisesRegex(ValueError, "winding"):
                state_geometry("MudMixer", root, "v2")

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
