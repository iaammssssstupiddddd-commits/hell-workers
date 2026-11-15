"""Actual atlas compatibility, part-scoped pigment, and pre-launch rejection."""
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import build_building_m2_candidate as m2_builder
import build_building_m3_candidate as m3_builder
import building_handpaint_materials as hand
import building_m3_uv_ink as ink


def pixel_hash(path):
    with Image.open(path) as image:
        return hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest()


class WeatheredPigmentTests(unittest.TestCase):
    def test_mixer_changes_outer_drum_only_and_preserves_old_pixels(self):
        with tempfile.TemporaryDirectory() as directory:
            old, new = Path(directory)/'old.png', Path(directory)/'new.png'
            layout = hand.paint('MudMixer', old, 'strokes', 'v6')
            revised = hand.paint('MudMixer', new, 'strokes', 'v6', style='weathered-v1')
            self.assertEqual(layout, revised)
            self.assertEqual(pixel_hash(old), 'f026b6fa96ab55d07e4d5c562d8f6adcdb69fcb6ae8758db6163883851e8fbcb')
            cells = {f['cell'] for f in layout['rotor'] if f['surface'] in {f'drum:band{i}' for i in range(4)}}
            changes = 0
            with Image.open(old) as a, Image.open(new) as b:
                self.assertEqual(a.size, b.size)
                self.assertEqual(b.getchannel('A').getextrema(), (255,255))
                for y in range(512):
                    for x in range(512):
                        before, after = a.getpixel((x,y)), b.getpixel((x,y))
                        if before != after:
                            self.assertIn(y//32*16+x//32, cells)
                            self.assertLessEqual(max(after[:3])-min(after[:3]), 50)
                            changes += 1
            self.assertGreater(changes, 1000)

    def test_spa_old_atlas_and_all_emission_pixels_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            old, new = Path(directory)/'old', Path(directory)/'new'
            old.mkdir(); new.mkdir()
            ink.paint('SoulSpa', old, 'strokes')
            ink.paint('SoulSpa', new, 'strokes', style='weathered-v1')
            self.assertEqual(pixel_hash(old/'albedo.png'), '2591ecd3523b27c670e5d026efeb40bfd5328003bc1f8e3caf7967ff6bb9aeb9')
            self.assertEqual(pixel_hash(old/'slot_emissive.png'), '1d61b6e215bc9dddd25c41dae04cffcffaca4cb8f9015619fa85295a62f11c1d')
            self.assertEqual((old/'slot_emissive.png').read_bytes(), (new/'slot_emissive.png').read_bytes())
            self.assertNotEqual(pixel_hash(old/'albedo.png'), pixel_hash(new/'albedo.png'))
            with Image.open(new/'albedo.png') as image:
                self.assertEqual(image.size, (512,512))
                self.assertEqual(image.getchannel('A').getextrema(), (255,255))

    def test_spa_physical_bone_edge_joint_and_pad_have_distinct_pigment(self):
        from building_m3_art import contract
        _, spec = contract('SoulSpa')
        def body(point):return ink.weathered_spa_color('body','bone:shaft',point,spec)
        self.assertEqual(body((-10,5,-28.25)), (186,172,145,255))
        self.assertEqual(body((-10,5,-27.5)), (47,38,47,255))
        self.assertEqual(body((-20.3,5,-28.7)), (100,84,82,255))
        center=ink.weathered_spa_color('slot','slot:band2',(0,0,0),spec)
        edge=ink.weathered_spa_color('slot','slot:band2',(10.5,0,0),spec)
        self.assertLess(max(center[:3]),min(edge[:3]))
        with self.assertRaises(ValueError):ink.weathered_spa_color('body','cloth:canopy',(0,0,0),spec)

    def test_wrong_kind_revision_and_stage_reject_before_external_execution(self):
        with patch.object(m2_builder.subprocess, 'run') as launch:
            for kind,revision,stage in [('Tank','v1','strokes'),('MudMixer','v5','strokes'),('MudMixer','v6','planes')]:
                with self.assertRaises(ValueError):
                    m2_builder.build_bundle(kind,'rejected-weathered-pigment',stage,revision,'weathered-v1')
            for kind,revision,stage in [('RestArea','v2','strokes'),('SoulSpa','v3','strokes'),('SoulSpa','v2','planes')]:
                with self.assertRaises(ValueError):
                    m3_builder.build_bundle(kind,'rejected-weathered-pigment',stage,'weathered-v1',revision)
            with self.assertRaises(ValueError):
                hand.paint('RestArea',Path('/unused.png'),'strokes','v8',style='weathered-v1')
            launch.assert_not_called()


if __name__ == '__main__':unittest.main()
