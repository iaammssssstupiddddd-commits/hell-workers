"""Observed Blender UV boundary stays exact; wrong UV/face mappings still reject."""
from copy import deepcopy
import json
from pathlib import Path
import sys
import struct
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from building_clay_geometry import Geometry
import validate_building_m2_indexed_faces as reader


class IndexedFloat32Tests(unittest.TestCase):
    def check(self, actual_uvs=None, indices=None, source_positions=None, actual_positions=None):
        geometry=Geometry(vertices=source_positions or [(-25,34,28),(-20,34,28),(-25,36,28)],faces=[(0,1,2)])
        # First pair is the source and actual export of failed Rest v3 vertex.
        # Remaining coordinates are exactly representable dyadic fixtures.
        face={'uv':[(.5202349856035131,.697265625),(.546875,.697265625),(.546875,.734375)]}
        uvs=actual_uvs if actual_uvs is not None else [(0.520235002040863,.302734375),(.546875,.302734375),(.546875,.265625)]
        document={'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':1},'indices':2}]}]}
        with patch.object(reader,'read_glb',return_value=(document,b'')), patch.object(reader,'accessor_values',side_effect=[actual_positions or geometry.vertices,uvs]), patch.object(reader,'index_values',return_value=indices if indices is not None else [0,1,2]):
            reader.check_m3_indexed_faces(Path('/unused.glb'),geometry,[face])

    def test_observed_export_boundary_matches_without_larger_tolerance(self):
        self.assertNotEqual(round(.5202349856035131,5),round(.520235002040863,5))
        self.check()

    def test_actual_clipped_roof_float32_position_matches_and_real_movement_rejects(self):
        source=[(23,34.241666667,22),(22,34,22),(23,36,22)]
        f32=lambda value:struct.unpack('<f',struct.pack('<f',value))[0]
        actual=[tuple(f32(n) for n in p) for p in source]
        self.assertEqual(actual[0][1],34.24166488647461)
        self.assertNotEqual(round(source[0][1],5),round(actual[0][1],5))
        self.check(source_positions=source,actual_positions=actual)
        shifted=deepcopy(actual);shifted[0]=(23,34.24169,22)
        with self.assertRaises(ValueError):self.check(source_positions=source,actual_positions=shifted)

    def test_real_glb_float32_accessors_preserve_the_observed_roof_boundary(self):
        source=[(23,34.241666667,22),(22,34,22),(23,36,22)]
        geometry=Geometry(vertices=source,faces=[(0,1,2)])
        face={'uv':[(.5202349856035131,.697265625),(.546875,.697265625),(.546875,.734375)]}
        uvs=[.520235002040863,.302734375,.546875,.302734375,.546875,.265625]
        def glb(points):
            binary=struct.pack('<9f',*(n for p in points for n in p))+struct.pack('<6f',*uvs)+struct.pack('<3H',0,1,2)
            doc={'asset':{'version':'2.0'},'buffers':[{'byteLength':len(binary)}],
                 'bufferViews':[{'buffer':0,'byteOffset':0,'byteLength':36},{'buffer':0,'byteOffset':36,'byteLength':24},{'buffer':0,'byteOffset':60,'byteLength':6}],
                 'accessors':[{'bufferView':0,'componentType':5126,'count':3,'type':'VEC3'}, {'bufferView':1,'componentType':5126,'count':3,'type':'VEC2'}, {'bufferView':2,'componentType':5123,'count':3,'type':'SCALAR'}],
                 'meshes':[{'primitives':[{'attributes':{'POSITION':0,'TEXCOORD_0':1},'indices':2}]}]}
            encoded=json.dumps(doc).encode();encoded+=b' '*((-len(encoded))%4);binary+=b'\0'*((-len(binary))%4)
            return struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))+struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
        with tempfile.TemporaryDirectory() as temp:
            p=Path(temp)/'roof.glb';p.write_bytes(glb(source))
            reader.check_m3_indexed_faces(p,geometry,[face])
            shifted=deepcopy(source);shifted[0]=(23,34.24169,22);p.write_bytes(glb(shifted))
            with self.assertRaises(ValueError):reader.check_m3_indexed_faces(p,geometry,[face])

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
