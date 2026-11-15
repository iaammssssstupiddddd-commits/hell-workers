"""Real source pixels, closed material inputs and visible construction geometry."""
import hashlib
import json
import math
from collections import Counter
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import building_handpaint_materials as hand
import building_m2_art as m2
import building_m3_art as m3
import building_m3_numeric as numeric
import build_building_m2_candidate as mixer_builder
import build_building_m3_candidate as rest_builder
from building_clay_geometry import load_pilot,drum_sweep
from building_mixer_uv_ink import local_point


class HandpaintTests(unittest.TestCase):
    def test_sources_are_closed_opaque_and_have_no_approval_authority(self):
        images,snapshot=hand.load_sources()
        self.assertEqual(set(images),{'drum','iron','canvas'})
        self.assertEqual(set(snapshot['images']),{'drum','iron','canvas'})
        self.assertEqual(set(hand.source_paths()),{'drum.png','iron.png','canvas.png','materials.json'})
        for image in images.values():image.close()
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);shutil.copytree(hand.SOURCE_ROOT,root/'inputs');root=root/'inputs'
            with (root/'iron.png').open('ab') as f:f.write(b'changed')
            with patch.object(hand,'SOURCE_ROOT',root):
                with self.assertRaises(ValueError):hand.source_snapshot()

    def test_alpha_holes_are_rejected_even_after_a_new_manifest_is_hash_bound(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)/'sources';shutil.copytree(hand.SOURCE_ROOT,root)
            p=root/'drum.png';Image.new('RGBA',(16,16),(0,0,0,0)).save(p)
            value=json.loads((root/'materials.json').read_bytes());row=value['images'][0]
            self.assertEqual(row['name'],'drum')
            row.update(bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),size=[16,16])
            p=root/'materials.json';p.write_text(json.dumps(value))
            with patch.object(hand,'SOURCE_ROOT',root),patch.object(hand,'SOURCE_MANIFEST_SHA256',hashlib.sha256(p.read_bytes()).hexdigest()):
                with self.assertRaisesRegex(ValueError,'alpha holes'):hand.load_sources()

    def test_new_pigment_reaches_every_actual_face_and_keeps_all_gutters_opaque(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            m2.paint('MudMixer',root/'mixer.png',revision='v5',style='handpaint-v1')
            m3.paint('RestArea',root,'strokes',revision='v4',style='handpaint-v1')
            for p in (root/'mixer.png',root/'albedo.png'):
                with Image.open(p) as image:
                    self.assertEqual(image.mode,'RGBA');self.assertEqual(image.size,(512,512))
                    self.assertEqual(image.getchannel('A').getextrema(),(255,255))
            mesh=m3.geometry('RestArea','v4')['body']
            for face,indices in zip(m3.face_layout('RestArea','v4')['body'],mesh.faces,strict=True):
                points=[mesh.vertices[i] for i in indices]
                for pixel,expected in zip(face['polygon_px'],points,strict=True):
                    self.assertTrue(all(abs(a-b)<1e-8 for a,b in zip(local_point(pixel,face['polygon_px'],points),expected,strict=True)))

    def test_new_canopy_is_low_in_the_centre_and_mixer_has_positive_continuous_clearance(self):
        _,spec=m3.contract('RestArea','v4')
        self.assertLess(spec['roof']['height_grid'][2][3],min(spec['roof']['height_grid'][1][1],spec['roof']['height_grid'][1][5]))
        mesh=m3.geometry('RestArea','v4')['body'];self.assertEqual(mesh.triangles(),304)
        self.assertEqual(mesh.bounds(),m3.geometry('RestArea','v2')['body'].bounds())
        for surface in ('cloth:canopy','cloth:curtain'):
            edges=Counter()
            for face,label in zip(mesh.faces,mesh.surfaces,strict=True):
                if label==surface:
                    for a,b in zip(face,face[1:]+face[:1],strict=True):
                        edges[tuple(sorted((a,b)))]+=1
            self.assertTrue(edges)
            self.assertEqual(set(edges.values()),{2})
        proposal=numeric.numeric_contract(m3.CONTRACT_V4.read_bytes(),'RestArea')
        self.assertEqual(proposal['geometry']['roles']['body']['triangle_cap'],320)
        spec=load_pilot('MudMixer','v5');meshes=m2.candidate_geometry('MudMixer','v5')
        sweep=drum_sweep(meshes['rotor'].vertices,spec)
        self.assertGreater(min(sweep['static_component_clearances_wu']),0)
        images,_=hand.load_sources()
        def color(angle):
            return hand.mixer_color('rotor','drum:band2',(14*math.cos(angle),1,14*math.sin(angle)),spec,images)
        self.assertEqual(color(-math.pi+1e-9),color(math.pi-1e-9))
        self.assertNotEqual(color(.6),color(.6+math.pi))
        for image in images.values():image.close()

    def test_wrong_kind_revision_and_nonstroke_stages_reject_without_blender(self):
        with patch.object(mixer_builder.subprocess,'run') as launch:
            for kind,revision,stage in [('Tank','v1','strokes'),('MudMixer','v3','strokes'),('MudMixer','v5','planes')]:
                with self.assertRaises(ValueError):mixer_builder.build_bundle(kind,'rejected-handpaint',stage,revision,'handpaint-v1')
            for kind,revision,stage in [('SoulSpa','v4','strokes'),('RestArea','v2','strokes'),('RestArea','v4','planes')]:
                with self.assertRaises(ValueError):rest_builder.build_bundle(kind,'rejected-handpaint',stage,'handpaint-v1',revision)
            launch.assert_not_called()


if __name__=='__main__':unittest.main()
