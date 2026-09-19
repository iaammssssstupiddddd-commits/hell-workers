from __future__ import annotations

import importlib
import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch


HELPER_DIR = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPER_DIR))
acceptance = importlib.import_module("ui_usability_acceptance")


class KeyboardTapTests(unittest.TestCase):
    def test_release_precedes_slow_observer_acknowledgement(self):
        driver = acceptance.Driver.__new__(acceptance.Driver)
        driver.nonce, driver.events, driver.root = "owned", [], Path("unused")
        driver.last = {"frame": 10, "cursor": None, "mouse_left_pressed": False, "controls": {}}
        def send(step, nonce, **event):
            self.assertEqual(nonce, "owned")
            driver.events.append({"step": step, "pre_frame": 10, **event})
        driver.input = Mock()
        driver.input.send.side_effect = send
        def wait(predicate, *, after):
            self.assertEqual(after, 13)
            self.assertEqual([(event["key"], event["pressed"]) for event in driver.events],
                             [("d", True), ("d", False)])
            return {**driver.last, "frame": 100}
        driver.wait = Mock(side_effect=wait)
        with patch.object(acceptance.native, "atomic_write_json"):
            driver.key("d")
        self.assertEqual([event["step"] for event in driver.events], ["1", "2"])
        self.assertEqual([event["post_frame"] for event in driver.events], [100, 100])


class RefactorSuiteEvidenceTests(unittest.TestCase):
    def manifest(self):
        return {"case": "refactor-suite", "smoke": True, "input_backend": "portal",
                "portal_session": "/owned/session", "sessions": [
                    {"case": "refactor-rows", "directory": "viewport-0", "nonce": "rows"},
                    {"case": "progress-bars", "directory": "viewport-1", "nonce": "bars"}]}

    def test_suite_rejects_missing_reordered_or_reused_case_evidence(self):
        acceptance.check_session_inventory(self.manifest())
        for mutate in (lambda sessions: sessions.pop(), lambda sessions: sessions.reverse(),
                       lambda sessions: sessions[1].update(case="refactor-rows"),
                       lambda sessions: sessions[1].update(directory="viewport-0"),
                       lambda sessions: sessions[1].update(nonce="rows")):
            manifest = self.manifest()
            mutate(manifest["sessions"])
            with self.assertRaises(RuntimeError):
                acceptance.check_session_inventory(manifest)

    def test_suite_requires_shared_permission_and_keeps_row_xim_check(self):
        manifest = self.manifest()
        client = {"input_backend": "portal", "nonce": "rows", "portal_devices": 3,
                  "monitor_mapping": [1, 2.0], "xmodifiers": "@im=local", "portal_session": "/owned/session"}
        acceptance.check_input_client(manifest, manifest["sessions"][0], client)
        for field, replacement in (("portal_session", None), ("portal_session", "/other/session"),
                                   ("xmodifiers", "@im=ibus")):
            with self.subTest(field=field, value=replacement), self.assertRaises(RuntimeError):
                acceptance.check_input_client(manifest, manifest["sessions"][0], {**client, field: replacement})


class TerrainMaterialEvidenceTests(unittest.TestCase):
    def observation(self):
        return {"ready": True, "frame": 100, "task_rows": 0, "paused": True,
                "viewport": [1920, 1080], "dpi_mode": "native", "scale_factor": 1.0, "base_scale_factor": 1.0,
                "terrain_materials": {"scene": "new", "size": [1920, 1080],
                    "quality_scale": 1.0, "target_scale": 1.0, "camera_scale": 1.0,
                    "camera_bound": True, "composite_bound": True,
                    "gpu": {"scene": "new", "size": [1920, 1080], "resident": [True]*4,
                            "errors": [], "adapter": "Intel Arc", "backend": "Vulkan"},
                    "patches": [{"lod": index, "visible": True, "scene_rect": [100+index*200, 100, 200+index*200, 200],
                                 "client_rect": [100+index*200, 100, 200+index*200, 200]} for index in range(3)],
                    "readback": {"scene": "new", "size": [1920, 1080], "opaque_pixels": [64]*3}}}

    def test_terrain_requires_prepass_and_current_scene_readback(self):
        value = self.observation()
        acceptance.check_observation("terrain-initial", value)
        for key, replacement in (("resident", [True, True, True, False]), ("scene", "old")):
            invalid = copy.deepcopy(value)
            invalid["terrain_materials"]["gpu"][key] = replacement
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                acceptance.check_observation("terrain-initial", invalid)
        value["terrain_materials"]["readback"]["scene"] = "old"
        with self.assertRaisesRegex(RuntimeError, "stale Scene readback"):
            acceptance.check_observation("terrain-initial", value)

    def test_terrain_rejects_dpi_override_hidden_and_clear_patches(self):
        for mutate in (lambda value: value.update(dpi_mode="viewport-override"),
                       lambda value: value["terrain_materials"].update(quality_scale=0.5),
                       lambda value: value["terrain_materials"].update(camera_scale=2.0),
                       lambda value: value["terrain_materials"]["patches"][1].update(visible=False),
                       lambda value: value["terrain_materials"]["readback"].update(opaque_pixels=[64, 0, 64])):
            value = self.observation()
            mutate(value)
            with self.assertRaises(RuntimeError):
                acceptance.check_observation("terrain-initial", value)


class RefactorRowEvidenceTests(unittest.TestCase):
    def test_simulation_requires_changed_source_matching_text_and_same_row(self):
        before = self.observation()
        for key, soul in before["refactor_rows"]["souls"].items():
            soul["fatigue"] = 0.25
            soul["row"].update(entity=key, fatigue={"text": "25%", "rect": [0, 0, 40, 20]})
        after = copy.deepcopy(before)
        after["refactor_rows"]["souls"]["deconstruct"]["fatigue"] = 0.23
        after["refactor_rows"]["souls"]["deconstruct"]["row"]["fatigue"]["text"] = "23%"
        acceptance.refactor_rows.check_value_update(before, after)
        for value in (before, copy.deepcopy(after)):
            if value is not before:
                row = value["refactor_rows"]["souls"]["deconstruct"]["row"]
                row["fatigue"]["text"] = "25%"
            with self.assertRaises(RuntimeError):
                acceptance.refactor_rows.check_value_update(before, value)
        after["refactor_rows"]["souls"]["deconstruct"]["row"]["entity"] = "replacement"
        with self.assertRaisesRegex(RuntimeError, "replaced"):
            acceptance.refactor_rows.check_value_update(before, after)

    def test_rows_require_observed_child_local_xim(self):
        manifest = {"input_backend": "portal", "case": "refactor-rows"}
        session = {"nonce": "owned"}
        client = {"input_backend": "portal", "nonce": "owned", "portal_devices": 3,
                  "monitor_mapping": [1, 2.0], "xmodifiers": "@im=local"}
        acceptance.check_input_client(manifest, session, client)
        for value in (None, "@im=ibus", ""):
            with self.subTest(value=value), self.assertRaisesRegex(RuntimeError, "child-local XIM"):
                acceptance.check_input_client(manifest, session, {**client, "xmodifiers": value})
        acceptance.check_input_client({**manifest, "case": "progress-bars"}, session,
                                      {**client, "xmodifiers": "@im=ibus"})

    def observation(self):
        souls = {}
        for index, (key, (label, _)) in enumerate(acceptance.refactor_rows.TASKS.items()):
            name = "renamed" if key == "deconstruct" else key
            souls[key] = {"entity": index + 1, "name": name, "row": {
                "rect": [0, 0, 100, 20],
                "name": {"text": name, "rect": [0, 0, 60, 20]},
                "label": {"text": label, "rect": [60, 0, 80, 20]},
                "icon": {"icon": "asset-id", "rect": [80, 0, 100, 20]},
            }}
        return {"paused": True, "selected": 1, "ready": True, "frame": 120, "task_rows": 0,
                "refactor_rows": {"souls": souls, "hovered": 1,
                                  "detail_task": {"text": "Task: Deconstruct (GoingToTarget)", "rect": [0, 0, 100, 20]}},
                "tooltip_alpha": 1.0,
                "tooltip_text": [{"text": "Task: Deconstruct (GoingToTarget)", "rect": [0, 0, 100, 20], "alpha": 1.0}]}

    def test_deconstruction_rejects_bucket_tooltip_and_wrong_hover(self):
        value = self.observation()
        acceptance.check_observation("tooltip-deconstruct", value)
        value["tooltip_text"][0]["text"] = "BucketTransport (GoingToBucket)"
        with self.assertRaisesRegex(RuntimeError, "Tooltip differs"):
            acceptance.check_observation("tooltip-deconstruct", value)
        value = self.observation()
        value["refactor_rows"]["hovered"] = 2
        with self.assertRaisesRegex(RuntimeError, "hover target"):
            acceptance.check_observation("tooltip-deconstruct", value)

    def test_rows_require_all_callers_current_name_and_visible_labels(self):
        value = self.observation()
        acceptance.check_observation("row-renamed", value)
        value["refactor_rows"]["souls"]["deconstruct"]["row"]["name"]["text"] = "old name"
        with self.assertRaisesRegex(RuntimeError, "stale"):
            acceptance.check_observation("row-renamed", value)
        value = self.observation()
        value["refactor_rows"]["souls"]["power"]["row"]["label"]["rect"] = None
        with self.assertRaisesRegex(RuntimeError, "list task label"):
            acceptance.check_observation("row-renamed", value)
        del value["refactor_rows"]["souls"]["bucket"]
        with self.assertRaisesRegex(RuntimeError, "coverage"):
            acceptance.check_observation("row-renamed", value)

    def test_search_clear_must_restore_folded_state(self):
        value = self.observation()
        for soul in value["refactor_rows"]["souls"].values():
            soul["row"]["rect"] = None
        acceptance.check_observation("row-search-cleared", value)
        value["refactor_rows"]["souls"]["deconstruct"]["row"]["rect"] = [0, 0, 100, 20]
        with self.assertRaisesRegex(RuntimeError, "visibility"):
            acceptance.check_observation("row-search-cleared", value)

    def test_duplicate_transition_without_input_is_rejected(self):
        value = self.observation()
        entries = [{"case": case, "value": value, "last_step": 1} for case in acceptance.refactor_rows.CHECKPOINTS]
        with self.assertRaisesRegex(RuntimeError, "fresh input"):
            acceptance.refactor_rows.check_sequence(entries)


class ProgressBarEvidenceTests(unittest.TestCase):
    def initial(self):
        owners, bars = {}, []
        for index, (kind, ratio) in enumerate(acceptance.progress_bars.INITIAL_RATIOS.items(), 1):
            owners[kind] = {"entity": index, "global": [100, 100, 0],
                            "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 100, 100, 0, 1],
                            "task": "Gather" if kind == "soul" else None}
            for fill in (False, True):
                x = (40 * ratio - 40) / 2 if fill else 0
                bars.append({"entity": 10 + index * 2 + int(fill), "kind": kind,
                             "owner_key": kind, "owner": index, "background": not fill, "fill": fill,
                             "width": 40, "size": [40 * ratio if fill else 40, 4],
                             "local": [x, 20, 0], "global": [100+x, 120, 0],
                             "point": [100+x, 120], "visible": True, "color": [0, 1, 0, 1]})
        return {"ready": True, "frame": 120, "task_rows": 0, "viewport": [1920, 1080], "world_epoch": 1,
                "save_outcomes": [], "task_outcomes": [],
                "progress_bars": {"owners": owners, "bars": bars, "wall_target": 8,
                                  "floor_complete": False, "source_present": True}}

    def sequence(self):
        entries = []
        for index, case in enumerate(acceptance.progress_bars.CHECKPOINTS):
            value = self.initial()
            value["frame"] += index * 30
            evidence = value["progress_bars"]
            if index >= 1:
                value["save_outcomes"] = ["Save:Succeeded"]
            if index >= 3:
                value["world_epoch"] = 2
                value["save_outcomes"].append("Load:Succeeded")
                for owner in evidence["owners"].values():
                    owner["entity"] += 100
                for bar in evidence["bars"]:
                    bar["entity"] += 100
                    bar["owner"] += 100
                evidence["wall_target"] += 100
                evidence["owners"]["soul"]["task"] = "None"
            removed = {"soul", "floor"} if index == 2 else {"soul"} if index == 3 else set(evidence["owners"]) if index == 4 else set()
            evidence["bars"] = [bar for bar in evidence["bars"] if bar["kind"] not in removed]
            for kind in removed - {"soul"}:
                del evidence["owners"][kind]
            if index in (2, 4):
                evidence["floor_complete"] = True
            if index == 2:
                evidence["source_present"] = False
            if index == 4:
                value["task_outcomes"] = [{"entity": target, "action": "Cancel", "result": "CancellationRequested", "epoch": 2, "frame": 230} for target in (102, 108)]
            entries.append({"case": case, "value": value, "last_step": index + 1})
        return entries

    def test_four_callers_ownership_and_on_screen_geometry_are_required(self):
        value = self.initial()
        acceptance.check_observation("bars-initial", value)
        for field, replacement, reason in (("owner", 999, "owner"), ("point", [-1, 100], "viewport"), ("global", [500, 500, 0], "follow")):
            changed = copy.deepcopy(value)
            changed["progress_bars"]["bars"][0][field] = replacement
            with self.assertRaisesRegex(RuntimeError, reason):
                acceptance.check_observation("bars-initial", changed)
        value["progress_bars"]["bars"].pop()
        with self.assertRaisesRegex(RuntimeError, "pair"):
            acceptance.check_observation("bars-initial", value)

    def test_parent_scale_and_rotation_are_composed_in_all_axes(self):
        for matrix, transform in (
            ([0.94, 0, 0, 0, 0, 0.94, 0, 0, 0, 0, 0.94, 0, -144, 48, 0, 1],
             lambda x, y, z: [-144 + 0.94*x, 48 + 0.94*y, 0.94*z]),
            ([0, 2, 0, 0, -3, 0, 0, 0, 0, 0, 4, 0, 100, 200, 10, 1],
             lambda x, y, z: [100 - 3*y, 200 + 2*x, 10 + 4*z]),
        ):
            with self.subTest(matrix=matrix):
                value = self.initial()
                evidence = value["progress_bars"]
                evidence["owners"]["blueprint"]["matrix"] = matrix
                evidence["owners"]["blueprint"]["global"] = matrix[12:15]
                pair = [bar for bar in evidence["bars"] if bar["kind"] == "blueprint"]
                for bar in pair:
                    bar["local"][1:] = [-18, 4.1 if bar["fill"] else 4]
                    bar["global"] = transform(*bar["local"])
                acceptance.check_observation("bars-initial", value)
                pair[0]["global"][2] += 0.1
                with self.assertRaisesRegex(RuntimeError, "follow"):
                    acceptance.check_observation("bars-initial", value)
                pair[0]["global"] = [matrix[12 + axis] + pair[0]["local"][axis] for axis in range(3)]
                with self.assertRaisesRegex(RuntimeError, "follow"):
                    acceptance.check_observation("bars-initial", value)

    def test_invalid_parent_matrix_and_coordinates_are_rejected(self):
        for matrix in (None, [1] * 15, [float("nan")] * 16, [float("inf")] * 16):
            value = self.initial()
            value["progress_bars"]["owners"]["soul"]["matrix"] = matrix
            with self.assertRaisesRegex(RuntimeError, "matrix"):
                acceptance.check_observation("bars-initial", value)
        for coordinates in ([1, 2], [1, float("nan"), 3], [1, 2, float("inf")]):
            value = self.initial()
            value["progress_bars"]["bars"][0]["global"] = coordinates
            with self.assertRaisesRegex(RuntimeError, "coordinates"):
                acceptance.check_observation("bars-initial", value)

    def test_load_replaces_bars_and_cancel_requires_domain_receipts(self):
        entries = self.sequence()
        acceptance.progress_bars.check_sequence(entries)
        entries[-1]["value"]["task_outcomes"] = []
        with self.assertRaisesRegex(RuntimeError, "cancellation receipts"):
            acceptance.progress_bars.check_sequence(entries)
        entries = self.sequence()
        entries[3]["value"]["progress_bars"]["bars"][0]["entity"] = entries[2]["value"]["progress_bars"]["bars"][0]["entity"]
        with self.assertRaisesRegex(RuntimeError, "old progress"):
            acceptance.progress_bars.check_sequence(entries)

    def test_disappearance_without_simulation_completion_is_rejected(self):
        value = self.sequence()[2]["value"]
        value["progress_bars"]["source_present"] = True
        with self.assertRaisesRegex(RuntimeError, "before gathering completed"):
            acceptance.check_observation("bars-completed", value)


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
