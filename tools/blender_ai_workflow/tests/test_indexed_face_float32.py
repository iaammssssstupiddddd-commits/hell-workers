"""Observed Blender UV boundary stays exact; wrong UV/face mappings still reject."""
from copy import deepcopy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from building_clay_geometry import Geometry
import validate_building_m2_indexed_faces as reader


class IndexedFloat32Tests(unittest.TestCase):
    def check(self, actual_uvs=None, indices=None):
        geometry=Geometry(vertices=[(-25,34,28),(-20,34,28),(-25,36,28)],faces=[(0,1,2)])
        # First pair is the source and actual export of failed Rest v3 vertex.
        # Remaining coordinates are exactly representable dyadic fixtures.
        face={'uv':[(.5202349856035131,.697265625),(.546875,.697265625),(.546875,.734375)]}
        uvs=actual_uvs if actual_uvs is not None else [(0.520235002040863,.302734375),(.546875,.302734375),(.546875,.265625)]
        document={'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':1},'indices':2}]}]}
        with patch.object(reader,'read_glb',return_value=(document,b'')), patch.object(reader,'accessor_values',side_effect=[geometry.vertices,uvs]), patch.object(reader,'index_values',return_value=indices if indices is not None else [0,1,2]):
            reader.check_m3_indexed_faces(Path('/unused.glb'),geometry,[face])

    def test_observed_export_boundary_matches_without_larger_tolerance(self):
        self.assertNotEqual(round(.5202349856035131,5),round(.520235002040863,5))
        self.check()

    def test_real_uv_change_unflipped_v_and_wrong_winding_still_reject(self):
        original=[(0.520235002040863,.302734375),(.546875,.302734375),(.546875,.265625)]
        for attack in ('different-u','unflipped-v','winding'):
            with self.subTest(attack=attack):
                uvs=deepcopy(original);indices=[0,1,2]
                if attack=='different-u':uvs[0]=(.52025,uvs[0][1])
                if attack=='unflipped-v':uvs[0]=(uvs[0][0],.697265625)
                if attack=='winding':indices=[0,2,1]
                with self.assertRaises(ValueError):self.check(uvs,indices)

    def test_all_expected_vertices_do_not_allow_a_cross_face_triangle(self):
        points=[(-25,34,28),(-20,34,28),(-25,36,28)]
        geometry=Geometry(vertices=points+points,faces=[(0,1,2),(3,4,5)])
        faces=[{'uv':[(.5202349856035131,.697265625),(.546875,.697265625),(.546875,.734375)]},
               {'uv':[(.75,.75),(.78125,.75),(.75,.78125)]}]
        uvs=[(.520235002040863,.302734375),(.546875,.302734375),(.546875,.265625),
             (.75,.25),(.78125,.25),(.75,.21875)]
        document={'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':1},'indices':2}]}]}
        for indices, rejected in [([0,1,2,3,4,5],False),([0,4,2,3,1,5],True)]:
            with patch.object(reader,'read_glb',return_value=(document,b'')), patch.object(reader,'accessor_values',side_effect=[geometry.vertices,uvs]), patch.object(reader,'index_values',return_value=indices):
                if rejected:
                    with self.assertRaises(ValueError):reader.check_m3_indexed_faces(Path('/unused.glb'),geometry,faces)
                else:
                    reader.check_m3_indexed_faces(Path('/unused.glb'),geometry,faces)


if __name__=='__main__':
    unittest.main()
