"""Open site chassis and closed draped shelter; these are not art approvals."""
from collections import Counter
from copy import deepcopy
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import building_clay_geometry as clay
import building_m2_art as m2
import building_m3_art as m3
import building_m3_numeric as numeric
import build_building_m2_candidate as mixer_builder
import build_building_m3_candidate as rest_builder
from building_mixer_uv_ink import local_point


def vertical_hits(mesh):
    """Actual intersections of the x=z=0 ray with triangle interiors."""
    hits=[]
    for face in mesh.faces:
        for i in range(1,len(face)-1):
            a,b,c=[mesh.vertices[j] for j in (face[0],face[i],face[i+1])]
            dx1,dz1,dx2,dz2=b[0]-a[0],b[2]-a[2],c[0]-a[0],c[2]-a[2]
            d=dx1*dz2-dx2*dz1
            if abs(d)<1e-9:
                continue
            u=(-a[0]*dz2+a[2]*dx2)/d
            v=(-dx1*a[2]+dz1*a[0])/d
            if u>=0 and v>=0 and u+v<=1:
                hits.append(a[1]+u*(b[1]-a[1])+v*(c[1]-a[1]))
    return hits


class ReadableGeometryTests(unittest.TestCase):
    def test_new_chassis_has_real_centre_gap_and_same_rotor_pivot_tilt_and_speed(self):
        old=clay.load_pilot('MudMixer','v3');new=clay.load_pilot('MudMixer','v4')
        self.assertTrue(vertical_hits(clay.build('MudMixer',old)['body']))
        self.assertFalse(vertical_hits(clay.build('MudMixer',new)['body']))
        self.assertEqual(new['roles']['rotor']['translation_wu'],old['roles']['rotor']['translation_wu'])
        self.assertEqual(new['construction']['drum_profile'],old['construction']['drum_profile'])
        self.assertEqual(new['construction']['tilt_radians'],old['construction']['tilt_radians'])
        self.assertEqual(new['states'],old['states'])
        path,_=m2.numeric_contract('MudMixer','v4')
        proposal=clay.read_json(path)
        self.assertEqual(proposal['kinds']['MudMixer']['motion']['seconds_per_revolution'],4)
        sweep=clay.drum_sweep(clay.build('MudMixer',new)['rotor'].vertices,new)
        self.assertTrue(sweep['continuous_full_turn'])
        self.assertGreater(min(sweep['static_component_clearances_wu']),0)
        broken=deepcopy(new);broken['construction']['body_boxes'][3]={'center':[0,25,0],'size':[3,4.5,36]}
        with self.assertRaises(ValueError):
            clay.drum_sweep(clay.build('MudMixer',new)['rotor'].vertices,broken)

    def test_draped_roof_and_folded_opaque_curtain_are_closed_meshes_with_safe_budget(self):
        mesh=m3.geometry('RestArea','v3')['body']
        self.assertEqual(mesh.bounds(),m3.geometry('RestArea','v2')['body'].bounds())
        self.assertLessEqual(mesh.triangles(),256)
        for surface in ('cloth:canopy','cloth:curtain'):
            edges=Counter()
            faces=[face for face,label in zip(mesh.faces,mesh.surfaces,strict=True) if label==surface]
            self.assertTrue(faces)
            for face in faces:
                for a,b in zip(face,face[1:]+face[:1],strict=True):
                    edges[tuple(sorted((a,b)))]+=1
            self.assertEqual(set(edges.values()),{2})
        layout=m3.face_layout('RestArea','v3')['body']
        self.assertLessEqual(len(layout),256)
        for face,indices in zip(layout,mesh.faces,strict=True):
            points=[mesh.vertices[i] for i in indices]
            for pixel,expected in zip(face['polygon_px'],points,strict=True):
                self.assertTrue(all(abs(a-b)<1e-8 for a,b in zip(local_point(pixel,face['polygon_px'],points),expected,strict=True)))

    def test_new_rest_painter_reaches_the_actual_v3_geometry_without_legacy_bypass(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            m3.paint('RestArea',root,'strokes',style='illustrated-v2',revision='v3')
            with Image.open(root/'albedo.png') as image:
                self.assertEqual(image.size,(512,512))
                self.assertEqual(image.getchannel('A').getextrema(),(255,255))
            self.assertFalse((root/'slot_emissive.png').exists())
            with self.assertRaises(ValueError):
                m3.paint('RestArea',root,'strokes',style='legacy',revision='v3')

    def test_revisions_do_not_silently_change_old_spa_or_grant_numeric_authority(self):
        payload=m3.CONTRACT_V3.read_bytes()
        self.assertEqual(numeric.numeric_contract(payload,'RestArea')['kind'],'RestArea')
        self.assertFalse(clay.read_json(m3.CONTRACT_V3)['numeric_freeze'])
        for kind,revision in [('SoulSpa','v3'),('SoulSpa','v4'),('SoulSpa','v5'),('RestArea','v6')]:
            with self.assertRaises(ValueError):
                m3.geometry(kind,revision)
        with self.assertRaises(ValueError):
            numeric.numeric_contract(payload,'SoulSpa')
        with patch.object(mixer_builder.subprocess,'run') as launch:
            with self.assertRaises(ValueError):
                mixer_builder.build_bundle('MudMixer','reject-old-style','strokes','v4','illustrated-v1')
            with self.assertRaises(ValueError):
                rest_builder.build_bundle('RestArea','reject-old-style','strokes','illustrated-v1','v3')
            launch.assert_not_called()


if __name__=='__main__':
    unittest.main()
