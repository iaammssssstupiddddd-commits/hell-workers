"""Silhouette and actual solid coverage, without granting any art authority."""
from collections import Counter
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import building_clay_geometry as clay
import building_m2_art as m2
import building_m3_art as m3
import building_m3_numeric as numeric
import build_building_m2_candidate as mixer_builder
import build_building_m3_candidate as rest_builder
from building_mixer_uv_ink import local_point


def sub(a,b):return tuple(x-y for x,y in zip(a,b,strict=True))
def dot(a,b):return sum(x*y for x,y in zip(a,b,strict=True))
def cross(a,b):return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])


def ray(origin,direction,triangles):
    """Moller-Trumbore positive intersections of the actual owner triangles."""
    distances=[]
    for a,b,c in triangles:
        u,v=sub(b,a),sub(c,a);p=cross(direction,v);d=dot(u,p)
        if abs(d)<1e-9:continue
        t=sub(origin,a);alpha=dot(t,p)/d
        q=cross(t,u);beta=dot(direction,q)/d;distance=dot(v,q)/d
        if alpha>=-1e-8 and beta>=-1e-8 and alpha+beta<=1+1e-8 and distance>1e-8:
            distances.append(distance)
    return distances


def triangles(mesh,select):
    return [[mesh.vertices[j] for j in (face[0],face[i],face[i+1])]
            for face,label in zip(mesh.faces,mesh.surfaces,strict=True) if select(label)
            for i in range(1,len(face)-1)]


class HandpaintReadabilityTests(unittest.TestCase):
    def test_mixer_side_is_long_in_actual_projection_with_clear_sweep_and_unchanged_clock(self):
        spec=clay.load_pilot('MudMixer','v6');old=clay.load_pilot('MudMixer','v5');c=spec['construction']
        profile=c['drum_profile'];radius=max(r for r,_ in profile);length=max(y for _,y in profile)-min(y for _,y in profile)
        axis=(math.sin(c['yaw_radians'])*math.sin(c['tilt_radians']),math.cos(c['tilt_radians']),math.cos(c['yaw_radians'])*math.sin(c['tilt_radians']))
        self.assertGreater(length*math.hypot(axis[0],axis[2]-.6*axis[1]),radius*2*1.4)
        self.assertGreater(profile[3][1]-profile[2][1],radius)
        self.assertGreater(profile[5][0]/radius,.6)
        self.assertEqual(spec['states'],old['states'])
        self.assertEqual(spec['roles']['rotor']['translation_wu'],old['roles']['rotor']['translation_wu'])
        self.assertEqual(c['tilt_radians'],old['construction']['tilt_radians'])
        meshes=m2.candidate_geometry('MudMixer','v6');sweep=clay.drum_sweep(meshes['rotor'].vertices,spec)
        self.assertGreater(min(sweep['static_component_clearances_wu']),0)
        path,_=m2.numeric_contract('MudMixer','v6');self.assertFalse(clay.read_json(path)['numeric_freeze'])

    def test_shelter_has_closed_outward_roof_and_enclosure_inside_same_bounds_and_budget(self):
        mesh=m3.geometry('RestArea','v5')['body'];_,spec=m3.contract('RestArea','v5')
        self.assertEqual(mesh.bounds(),m3.geometry('RestArea','v4')['body'].bounds())
        self.assertLessEqual(mesh.triangles(),spec['roles']['body']['triangle_cap'])
        self.assertLessEqual(len(m3.face_layout('RestArea','v5')['body']),256)
        for select in (lambda s:s=='cloth:canopy',lambda s:s.startswith('cloth:') and s!='cloth:canopy'):
            edges=Counter()
            for face,label in zip(mesh.faces,mesh.surfaces,strict=True):
                if select(label):
                    for a,b in zip(face,face[1:]+face[:1],strict=True):edges[(a,b)]+=1
            self.assertTrue(edges)
            self.assertTrue(all(edges[(b,a)]==1 and count==1 for (a,b),count in edges.items()))
            volume=sum(dot(a,cross(b,c))/6 for a,b,c in triangles(mesh,select))
            self.assertGreater(volume,0)
        for face,indices in zip(m3.face_layout('RestArea','v5')['body'],mesh.faces,strict=True):
            points=[mesh.vertices[i] for i in indices]
            for pixel,expected in zip(face['polygon_px'],points,strict=True):
                self.assertTrue(all(abs(a-b)<1e-7 for a,b in zip(local_point(pixel,face['polygon_px'],points),expected,strict=True)))

    def test_shelter_overlaps_real_roof_planes_and_blocks_upward_and_side_rays(self):
        mesh=m3.geometry('RestArea','v5')['body'];_,spec=m3.contract('RestArea','v5')
        roof=triangles(mesh,lambda s:s=='cloth:canopy');shell=triangles(mesh,lambda s:s.startswith('cloth:') and s!='cloth:canopy')
        for face,label in zip(mesh.faces,mesh.surfaces,strict=True):
            if label!='cloth:ceiling':continue
            for i in face:
                x,y,z=mesh.vertices[i]
                levels=ray((x,-10,z),(0,1,0),roof)
                self.assertTrue(levels)
                self.assertAlmostEqual(y-(min(levels)-10),spec['enclosure']['roof_overlap_wu'],places=7)
        for x in (-16,0,16):
            for z in (-16,0,16):
                for direction in ((0,1,0),(1,0,0),(-1,0,0),(0,0,1),(0,0,-1),(1,0,1),(-1,0,-1)):
                    self.assertTrue(ray((x,12,z),direction,shell))
        # Actual entry frontage remains tall below the projected roof eave.
        eave=max(z-.6*y for triangle in roof for x,y,z in triangle if abs(x)<1 and z>25)
        entry_floor=spec['enclosure']['half_z_wu']-.6*spec['enclosure']['floor_y_wu']
        self.assertGreater(entry_floor-eave,16)
        self.assertFalse(clay.read_json(m3.CONTRACT_V5)['art_approved'])
        numeric.numeric_contract(m3.CONTRACT_V5.read_bytes(),'RestArea')
        with self.assertRaises(ValueError):numeric.numeric_contract(m3.CONTRACT_V5.read_bytes(),'SoulSpa')

    def test_explicit_new_proposals_paint_but_wrong_style_or_kind_cannot_launch(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            m2.paint('MudMixer',root/'mixer.png',revision='v6',style='handpaint-v1')
            m3.paint('RestArea',root,'strokes',revision='v5',style='handpaint-v1')
            for p in (root/'mixer.png',root/'albedo.png'):
                with Image.open(p) as image:self.assertEqual(image.getchannel('A').getextrema(),(255,255))
        with patch.object(mixer_builder.subprocess,'run') as launch:
            with self.assertRaises(ValueError):mixer_builder.build_bundle('MudMixer','refuse-new-style','strokes','v6','illustrated-v2')
            with self.assertRaises(ValueError):rest_builder.build_bundle('RestArea','refuse-new-style','strokes','illustrated-v2','v5')
            with self.assertRaises(ValueError):rest_builder.build_bundle('SoulSpa','refuse-new-kind','strokes','handpaint-v1','v5')
            launch.assert_not_called()


if __name__=='__main__':unittest.main()
