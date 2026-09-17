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
    def test_build_cards_must_be_fully_visible(self):
        cards = {f"menu:SelectBuild({kind})": {"fully_visible": True} for kind in range(12)}
        value = {"layout_scene": "build", "paused": True, "controls": cards,
                 "management_rect": None, "inspector_rect": None,
                 "catalog_rect": [0, 60, 100, 90],
                 "ui_rects": [[0, 60, 100, 90]], "viewport": [100, 100]}
        acceptance.check_layout_scene(value, "build")
        cards["menu:SelectBuild(0)"]["fully_visible"] = False
        with self.assertRaisesRegex(RuntimeError, "build card is clipped"):
            acceptance.check_layout_scene(value, "build")

    def test_pinned_selection_requires_distinct_target_and_visible_chip(self):
        chip = {"fully_visible": True}
        value = {"layout_scene": "pinned", "paused": True,
                 "controls": {"menu:Workspace(InspectSelection)": chip,
                              "menu:SelectAreaTaskFor(A)": {"fully_visible": True},
                              "label:menu:SelectAreaTaskFor(A)": {"fully_visible": True, "text": "作業範囲を変更"},
                              "text:inspection-header": {"fully_visible": True, "text": "固定: A"}},
                 "management_rect": None, "inspector_rect": [80, 10, 100, 80],
                 "pinned": "A", "selected": "B",
                 "ui_rects": [[80, 10, 100, 80]], "viewport": [100, 100]}
        acceptance.check_layout_scene(value, "pinned")
        value["controls"]["text:inspection-header"]["fully_visible"] = False
        with self.assertRaisesRegex(RuntimeError, "target heading is missing"):
            acceptance.check_layout_scene(value, "pinned")
        value["controls"]["text:inspection-header"]["fully_visible"] = True
        chip["fully_visible"] = False
        with self.assertRaisesRegex(RuntimeError, "chip is not fully visible"):
            acceptance.check_layout_scene(value, "pinned")
        chip["fully_visible"] = True
        value["selected"] = "A"
        with self.assertRaisesRegex(RuntimeError, "must be distinct"):
            acceptance.check_layout_scene(value, "pinned")

    def test_occlusion_union_does_not_double_count_nested_panels(self):
        self.assertEqual(acceptance.rectangle_union_area([[0, 0, 100, 20], [10, 5, 90, 15], [90, 0, 110, 20]]), 2200)

    def test_normal_scene_rejects_a_visible_panel_or_excess_occlusion(self):
        value = {"layout_scene": "normal", "paused": True, "controls": {},
                 "management_rect": None, "inspector_rect": None, "workspace_page": "World",
                 "mode_presentation": {"rect": None}, "ui_rects": [[0, 0, 100, 10]], "viewport": [100, 100]}
        self.assertEqual(acceptance.check_layout_scene(value, "normal"), 0.1)
        value["management_rect"] = [0, 0, 30, 50]
        with self.assertRaisesRegex(RuntimeError, "workspace open"):
            acceptance.check_layout_scene(value, "normal")
        value["management_rect"] = None
        value["ui_rects"] = [[0, 0, 100, 30]]
        with self.assertRaisesRegex(RuntimeError, "occlusion"):
            acceptance.check_layout_scene(value, "normal")

    def test_menu_must_not_cover_guidance(self):
        value = {"paused": True, "controls": {}, "list_text": [
            {"text": "Familiar", "rect": [40, 200, 200, 230]}],
            "menu_state": "Zones", "zones_rect": [200, 400, 350, 650],
            "mode_presentation": {"rect": [0, 600, 800, 660]}}
        with self.assertRaisesRegex(RuntimeError, "obscures mode guidance"):
            acceptance.check_entities_render(value)
        value["zones_rect"][3] = 596
        acceptance.check_entities_render(value)

    def test_zones_fixture_closes_management_and_preserves_guidance(self):
        value = {"layout_scene": "management", "paused": True, "controls": {},
                 "list_text": [], "menu_state": "Zones", "zones_rect": [0, 30, 30, 60],
                 "mode_presentation": {"rect": [0, 65, 80, 70]},
                 "management_rect": None, "inspector_rect": None,
                 "ui_rects": [[0, 30, 30, 60], [0, 65, 80, 70]], "viewport": [100, 100]}
        acceptance.check_layout_scene(value, "management")
        value["management_rect"] = [0, 10, 30, 25]
        with self.assertRaisesRegex(RuntimeError, "tool menu must close"):
            acceptance.check_layout_scene(value, "management")

    def test_area_disclosure_requires_all_controls_and_visible_exit(self):
        value = {"layout_scene": "area", "paused": True, "controls": {
            "menu:FinishAreaEdit": {"fully_visible": True},
            "label:menu:FinishAreaEdit": {"fully_visible": True, "text": "編集を終了"},
            "label:area-details-toggle": {"fully_visible": True, "text": "詳細操作を開く"},
            "area-details-toggle": {"fully_visible": True}},
            "management_rect": None, "inspector_rect": None,
            "area_edit_rect": [60, 10, 90, 30], "ui_rects": [[60, 10, 90, 30]],
            "viewport": [100, 100]}
        acceptance.check_layout_scene(value, "area")
        value["layout_scene"] = "area-details"
        with self.assertRaisesRegex(RuntimeError, "disclosure differs"):
            acceptance.check_layout_scene(value, "area-details")
        value["controls"].update({f"area-control:{i}": {"fully_visible": True} for i in range(10)})
        value["controls"].update({f"label:area-control:{i}": {"fully_visible": True, "text": "操作"} for i in range(10)})
        acceptance.check_layout_scene(value, "area-details")
        value["controls"]["menu:FinishAreaEdit"]["fully_visible"] = False
        with self.assertRaisesRegex(RuntimeError, "primary action is clipped"):
            acceptance.check_layout_scene(value, "area-details")

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
