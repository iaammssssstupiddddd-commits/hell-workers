from __future__ import annotations

import importlib
from pathlib import Path
import sys
import unittest


HELPER_DIR = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPER_DIR))
acceptance = importlib.import_module("ui_usability_acceptance")


class TooltipEvidenceTests(unittest.TestCase):
    def observation(self):
        return {
            "ready": True, "frame": 120, "task_rows": 20, "controls": {},
            "paused": True, "tooltip_alpha": 1.0,
            "tooltip_presentation": {"rect": [550, 980, 750, 1020], "stack_index": 30},
            "mode_presentation": {"rect": [0, 970, 800, 1025], "stack_index": 20},
            "tooltip_text": [{"text": "メニュー", "rect": [560, 990, 740, 1010], "alpha": 1.0}],
        }

    def test_alpha_does_not_prove_visible_tooltip(self):
        value = self.observation()
        acceptance.check_observation("paused-tooltip", value)
        value["tooltip_presentation"]["rect"] = None
        with self.assertRaisesRegex(RuntimeError, "visible layout"):
            acceptance.check_observation("paused-tooltip", value)

    def test_mode_guidance_occlusion_is_rejected(self):
        value = self.observation()
        value["mode_presentation"]["stack_index"] = 40
        with self.assertRaisesRegex(RuntimeError, "behind the mode guidance"):
            acceptance.check_observation("paused-tooltip", value)

    def test_hidden_text_is_rejected(self):
        value = self.observation()
        value["tooltip_text"][0]["rect"] = None
        with self.assertRaisesRegex(RuntimeError, "visible text"):
            acceptance.check_observation("paused-tooltip", value)

    def test_readiness_waits_for_text_fade_after_container_fade(self):
        value = self.observation()
        value["tooltip_text"][0]["alpha"] = 0.9
        self.assertFalse(acceptance.paused_tooltip_ready(value))
        value["tooltip_text"][0]["alpha"] = 0.99
        self.assertTrue(acceptance.paused_tooltip_ready(value))

    def test_page_controls_hidden_by_mode_guidance_are_rejected(self):
        value = self.observation()
        value.update(left_panel="TaskList", minimized=False, page=9, task_total=200)
        value["controls"] = {"scroll:tasks": {}}
        for action in ("FirstPage", "PreviousPage", "NextPage", "LastPage"):
            value["controls"][f"task-control:{action}"] = {"rect": [40, 980, 80, 1020]}
        with self.assertRaisesRegex(RuntimeError, "overlaps task page controls"):
            acceptance.check_observation("last-page", value)
        for key, control in value["controls"].items():
            if key.startswith("task-control:"):
                control["rect"] = [40, 920, 80, 960]
        acceptance.check_observation("last-page", value)


class EntitiesRenderTests(unittest.TestCase):
    def test_menu_must_not_cover_guidance(self):
        value = {"paused": True, "controls": {}, "list_text": [
            {"text": "Familiar", "rect": [40, 200, 200, 230]}],
            "menu_state": "Zones", "zones_rect": [200, 400, 350, 650],
            "mode_presentation": {"rect": [0, 600, 800, 660]}}
        with self.assertRaisesRegex(RuntimeError, "obscures mode guidance"):
            acceptance.check_entities_render(value)
        value["zones_rect"][3] = 596
        acceptance.check_entities_render(value)

    def test_missing_header_is_rejected(self):
        value = {"paused": True, "controls": {}, "list_text": [
            {"text": "Familiar (2/2)", "rect": None}]}
        with self.assertRaisesRegex(RuntimeError, "header text"):
            acceptance.check_entities_render(value)
        value["list_text"][0]["rect"] = [40, 200, 200, 230]
        acceptance.check_entities_render(value)

    def test_unprepared_render_fixture_is_rejected(self):
        value = {"paused": False, "controls": {}, "list_text": [
            {"text": "Familiar (2/2)", "rect": [40, 200, 200, 230]}]}
        with self.assertRaisesRegex(RuntimeError, "render fixture"):
            acceptance.check_entities_render(value)


if __name__ == "__main__":
    unittest.main()
