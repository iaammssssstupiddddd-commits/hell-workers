"""Revised local ink preserves old pixels, parts and closed/rotating surfaces."""
import hashlib
import math
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import building_m2_art as m2
import building_m3_art as m3
import building_mixer_uv_ink as mixer
import building_m3_uv_ink as rest
import build_building_m2_candidate as m2_builder
import build_building_m3_candidate as m3_builder


class ReadableInkTests(unittest.TestCase):
    def test_prior_adopted_illustrated_pixels_stay_reproducible(self):
        # SHA of decoded adopted g4 authoring originals, not generated from the
        # revised painter. Revisions must not silently repaint the prior style.
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            m2.paint('MudMixer', root/'mixer.png', revision='v3', style='illustrated-v1')
            m3.paint('RestArea', root, 'strokes', style='illustrated-v1')
            for name, expected in (
                ('mixer.png', 'a04d9c4390283f2be37f386fd43e4b33f44eb71d311e8586bcd044290faf107f'),
                ('albedo.png', 'eafe08d2c4649b5570f44c9f68803c5143df399bf3b156a1b726ecdf68155710'),
            ):
                with Image.open(root/name) as image:
                    self.assertEqual(hashlib.sha256(image.tobytes()).hexdigest(), expected)

    def test_revised_atlases_keep_opaque_gutters_and_unchanged_layout(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            before = m2.face_layout('MudMixer', 'v3')
            self.assertEqual(mixer.paint(root/'mixer.png', style='illustrated-v2'), before)
            rest.paint('RestArea', root, 'strokes', style='illustrated-v2')
            for name in ('mixer.png', 'albedo.png'):
                with Image.open(root/name) as image:
                    self.assertEqual(image.mode, 'RGBA')
                    self.assertEqual(image.size, (512, 512))
                    self.assertEqual(image.getchannel('A').getextrema(), (255, 255))
            first = (root/'albedo.png').read_bytes()
            rest.paint('RestArea', root, 'strokes', style='illustrated-v2')
            self.assertEqual((root/'albedo.png').read_bytes(), first)

    def test_entry_is_closed_dark_curtain_separate_from_wall_roof_and_frame(self):
        def at(surface, point):
            return rest.color('RestArea', 'body', surface, point, style='illustrated-v2')
        entry = at('cloth:front', (1, 16, 23.5))
        wall = at('cloth:front', (16, 16, 26))
        roof = at('cloth:canopy', (20, 35, 0))
        frame = at('cloth:front', (6, 16, 23.5))
        self.assertLess(sum(entry[:3]), sum(wall[:3]))
        self.assertLess(sum(wall[:3]), sum(roof[:3]))
        self.assertGreater(sum(frame[:3]), sum(entry[:3]))
        self.assertTrue(all(c[3] == 255 for c in (entry, wall, roof, frame)))

    def test_drum_drawing_has_no_cylindrical_uv_seam_and_rotates_asymmetrically(self):
        def at(angle):
            return mixer.color('rotor', 'drum:band2',
                               (14*math.cos(angle), 1, 14*math.sin(angle)), style='illustrated-v2')
        self.assertEqual(at(-math.pi+1e-9), at(math.pi-1e-9))
        self.assertNotEqual(at(.55), at(.55+math.pi))
        samples=[mixer.color('rotor', 'drum:band4',
                            (6.5*math.cos(a), 14, 6.5*math.sin(a)), style='illustrated-v2')
                 for a in (0, math.pi/2, math.pi, 3*math.pi/2)]
        self.assertEqual(len(set(samples)), 2)  # worn lip never becomes a uniform bright ring

    def test_revised_style_rejects_spa_tank_old_rotor_and_unknown_style_before_launch(self):
        with patch.object(m2_builder.subprocess, 'run') as launch:
            for kind, revision in [('Tank', 'v1'), ('MudMixer', 'v1')]:
                with self.assertRaises(ValueError):
                    m2_builder.build_bundle(kind, 'readable-rejected', 'strokes', revision, 'illustrated-v2')
            launch.assert_not_called()
        with self.assertRaises(ValueError):
            m3_builder.selected_style('SoulSpa', 'illustrated-v2')
        with self.assertRaises(ValueError):
            rest.color('SoulSpa', 'body', 'bone:shaft', (0, 0, 0), style='illustrated-v2')
        with self.assertRaises(ValueError):
            mixer.color('rotor', 'drum:band2', (0, 0, 0), style='unknown')


if __name__ == '__main__':
    unittest.main()
