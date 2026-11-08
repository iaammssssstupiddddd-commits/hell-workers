"""Synthetic observer fixtures only; these never mint native/art/release evidence."""
import copy
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from scripts import ui_m5_planning as m5


def contract(kind="SandPile"):
    preview = {"image_role": "world", "canvas_px": [256, 256], "canvas_wu": [64., 64.],
               "anchor_px": [128., 128.], "representative_state": "idle"}
    return {"scope": m5.SCOPE, "case": next(case for case, value in m5.KINDS.items() if value == kind),
            "identity": {"kind": kind, "generation": 1, "manifest_sha256": "a" * 64, "authority": "art_preview"},
            "world": preview, "catalog": {**preview, "image_role": "catalog"}}


def feedback(adopted, phase):
    rows = []
    for role, entity in (("ghost", 10), ("blueprint", 20), ("pulse", 21)):
        if role == "ghost" and phase not in ("m5-legal", "m5-illegal"):
            continue
        if role != "ghost" and phase not in m5.CHECKPOINTS[3:]:
            continue
        scale, translation = (1., [0., 0., 2.]) if role == "ghost" else (.91, [0., 0., .2])
        rows.append({"consumer": role, "entity": entity, "owner": 10 if role == "ghost" else 20,
                     "image": "world-image", "whole_image": True, "size": [64., 64.], "anchor": [0., 0.],
                     "color": ([1, .2, .2, .5] if phase == "m5-illegal" else [.5, 1, .5, .5]) if role == "ghost" else [1, 1, 1, 0 if role == "blueprint" else .5],
                     "global_matrix": [scale, 0, 0, 0, 0, scale, 0, 0, 0, 0, scale, 0, *translation, 1], "visible": True,
                     "layers": [0], "parent": 20 if role == "pulse" else None,
                     "pose": {"translation": [0, 0, 0] if role == "pulse" else translation,
                              "rotation": [0, 0, 0, 1], "scale": [1., 1., 1.] if role == "pulse" else [scale] * 3},
                     "blueprint_state": None if role == "ghost" else "Building", "blueprint_progress": None if role == "ghost" else .1})
    return {"requested": adopted["identity"], "active": adopted["identity"], "pending": None,
            "descriptor": {"identity": adopted["identity"], "world": adopted["world"], "catalog": adopted["catalog"],
                           "images": {"world": "world-image", "catalog": "catalog-image"}},
            "sprites": rows, "catalog": [{"image": "catalog-image", "physical_rect": [10, 20, 50, 60],
                                           "fully_visible": True, "whole_image": True, "color": [1, 1, 1, 1]}],
            "accepted": False, "performance_evidence": False, "promotion_authority": False}


def observation(adopted, phase, time):
    placed = phase in m5.CHECKPOINTS[3:]
    footprint = [[5, 5], [6, 5], [5, 6], [6, 6]] if adopted["identity"]["kind"] == "WheelbarrowParking" else [[5, 5]]
    ctx = {"kind": adopted["identity"]["kind"], "anchor": [5, 5], "footprint": footprint, "draw_pos": [0., 0.],
           "point": [400., 400.], "cursor_grid": [5, 5], "primary_window": 99,
           "world_epoch": 0,
           "play_mode": "BuildingPlace" if phase in ("m5-legal", "m5-placed", "m5-illegal") else "Normal",
           "build_kind": adopted["identity"]["kind"],
           "camera": {"entity": 98, "primary": True, "viewport": [1920, 1080]},
           "inventory_overflow": False, "legal": not placed,
           "reject_reason": "OccupiedByBuilding" if placed else None,
           "cells": [{"grid": cell, "walkable": True, "owner": 20 if placed else None} for cell in footprint],
           "blueprints": [{"entity": 20, "debug_entity": "20v1", "footprint": footprint, "progress": .1,
                           "materials_complete": False, "delivered_materials": {}}] if placed else [], "buildings": [],
           "initial_building_inventory": [], "building_inventory": [],
           "consumers": feedback(adopted, phase), "real_seconds": time, "virtual_seconds": 0.}
    return {"m5_planning": ctx, "nonce": "b" * 32, "pid": 40, "world_epoch": 0, "primary_window": 99,
            "frame": round(time * 100), "paused": phase in ("m5-empty", "m5-selected"),
            "scale_factor": 1., "cursor": [400., 400.], "viewport": [1920, 1080],
            "pointer_over_ui": False, "world_input_captured": False,
            "selected": 20 if phase == "m5-selected" else None, "pointer_target": 20 if placed else None,
            "controls": {"text:inspection-header": {"fully_visible": True, "text": "Blueprint"}}}


def campaign(adopted):
    raw = {"scope": m5.SCOPE, "schema_version": 2, "failure": None, "nonce": "b" * 32, "process_id": 40,
           "accepted": False, "performance_evidence": False, "promotion_authority": False, "samples": [], "events": []}
    entries, inputs = [], []
    select = f"menu:SelectBuild({adopted['identity']['kind']})"
    steps = [("m5-empty", 1, None), (None, 2, "menu:SetTimeSpeed(Normal)"), (None, 4, "menu:ToggleArchitect"),
             ("m5-catalog", 5, None), (None, 6, select), ("m5-legal", 7, None), (None, 8, "placement"),
             ("m5-placed", 9, None), (None, 10, "motion"), ("m5-illegal", 11, None),
             (None, 12, "Escape"), (None, 14, "menu:SetTimeSpeed(Paused)"), (None, 16, "selection"), ("m5-selected", 17, None)]
    phase = "m5-empty"
    for checkpoint, time, key in steps:
        if checkpoint:
            phase = checkpoint
        value = observation(adopted, phase, time)
        sample = {"sample_index": len(raw["samples"]), "real_seconds": time + (.3 if key else 0), "virtual_seconds": 0.,
                  "world_epoch": 0, "primary_window": "99v1", "primary_window_bits": 99, "dpi": 1.,
                  "owners": [],
                  "cursor_position": [400., 400.], "art_feedback": value["m5_planning"]["consumers"]}
        raw["samples"].append(sample)
        if checkpoint:
            entries.append({"case": checkpoint, "value": value, "last_step": len(inputs)})
            continue
        if key == "motion":
            inputs.append({"step": str(len(inputs) + 1), "nonce": "b" * 32, "pid": 40, "window": 50,
                           "world_epoch": 0, "pre_frame": value["frame"], "post_frame": value["frame"] + 30,
                           "point": [400, 400], "observed_cursor": [400., 400.], "pressed": None})
            continue
        before, after = copy.deepcopy(value), observation(adopted, phase, time + .3)
        if key == "selection":
            before["m5_planning"]["play_mode"] = after["m5_planning"]["play_mode"] = "Normal"
        before["controls"][key] = {"fully_visible": True, "interaction": "Hovered", "rect": [390, 390, 410, 410]}
        first = len(inputs)
        for i, pressed in enumerate((True, False)):
            inputs.append({"step": str(len(inputs) + 1), "nonce": "b" * 32, "pid": 40, "window": 50,
                           "world_epoch": 0, "pre_frame": before["frame"], "post_frame": after["frame"],
                           "pressed": pressed, **({"key": key} if key == "Escape" else {"button": 1}),
                           "observed_hover": {key: "Pressed" if pressed else "Hovered"}})
            raw["events"].append({"producer": "KeyboardInput" if key == "Escape" else "MouseButtonInput",
                "state": "Pressed" if pressed else "Released", "real_seconds": time + .1 * (i + 1),
                "sample_index": sample["sample_index"], "event_index": len(raw["events"]), "window": "99v1",
                **({"key": key, "repeat": False} if key == "Escape" else {"button": "Left"})})
        inputs[first]["m5_action"] = {"key": key, "first_step": first + 1, "last_step": len(inputs),
                                     "before": before, "after": after}
        if key == "placement":
            raw["events"].append({"producer": "ordinary-building-placement-branch-v1", "operation": "PlacementOutcome",
                "sample_index": sample["sample_index"], "event_index": len(raw["events"]), "real_seconds": time + .15,
                "window": "99v1", "world_epoch": 0, "outcome": {"kind": adopted["identity"]["kind"],
                "grid": [5, 5], "status": "BlueprintCommitted", "owner": "20v1", "footprint": value["m5_planning"]["footprint"]}})
    return raw, entries, {"nonce": "b" * 32, "pid": 40, "window": 50}, inputs


class M5PlanningTests(unittest.TestCase):
    def test_existing_parking_is_preserved_and_not_confused_with_the_new_target(self):
        adopted = contract("WheelbarrowParking")
        original = campaign(adopted)
        owner_pose = {"translation": [100., 100., .5], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
        matrix = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 100., 100., .5, 1.]
        inventory = [{"entity": 90, "pose": owner_pose, "global_matrix": matrix}]
        row = feedback(adopted, "m5-legal")["sprites"][0]
        row.update(consumer="world", entity=91, owner=90, parent=90, color=[.9999999403953552] * 3 + [1.],
                   global_matrix=matrix, pose={"translation": [0., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]})
        raw, entries, client, inputs = original
        values = [e["value"] for e in entries] + [a["m5_action"][side] for a in inputs if "m5_action" in a for side in ("before", "after")]
        for value in values:
            ctx = value["m5_planning"]
            ctx.update(buildings=[90], initial_building_inventory=copy.deepcopy(inventory), building_inventory=copy.deepcopy(inventory))
            if not any(r["consumer"] == "world" for r in ctx["consumers"]["sprites"]):
                ctx["consumers"]["sprites"].append(copy.deepcopy(row))
        for sample in raw["samples"]:
            sample["owners"] = [{"entity_bits": 90, "kind": "WheelbarrowParking", "position": owner_pose["translation"],
                                 "rotation": owner_pose["rotation"], "scale": owner_pose["scale"]}]
            if not any(r["consumer"] == "world" for r in sample["art_feedback"]["sprites"]):
                sample["art_feedback"]["sprites"].append(copy.deepcopy(row))
        m5.verify(*original, adopted)
        for mutate in (
            lambda r, e, c, i: e[2]["value"]["m5_planning"].update(buildings=[]),
            lambda r, e, c, i: e[2]["value"]["m5_planning"].update(buildings=[90, 92]),
            lambda r, e, c, i: e[2]["value"]["m5_planning"]["building_inventory"][0]["pose"].update(translation=[200., 100., .5]),
            lambda r, e, c, i: e[2]["value"]["m5_planning"]["consumers"]["sprites"][-1].update(owner=92),
            lambda r, e, c, i: e[2]["value"]["m5_planning"]["consumers"]["sprites"][-1].update(parent=92),
            lambda r, e, c, i: e[2]["value"]["m5_planning"]["consumers"]["sprites"][-1].update(image="fallback"),
            lambda r, e, c, i: r["samples"][1]["art_feedback"]["sprites"][-1].update(owner=92),
            lambda r, e, c, i: r["samples"][1]["art_feedback"].update(sprites=[]),
            lambda r, e, c, i: r["samples"][1].update(owners=[]),
            lambda r, e, c, i: r["samples"][1]["owners"][0].update(position=[200., 100., .5]),
        ):
            changed = copy.deepcopy(original)
            mutate(*changed)
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                m5.verify(*changed, adopted)

    def test_real_bevy_white_f32_conversion_is_not_gray_tint_authority(self):
        for value in ([1., 1., 1., 1.], [0.9999999403953552] * 3 + [1.]):
            self.assertTrue(m5.white_color(value))
        for value in ([.999, 1., 1., 1.], [1., 1., 1., .99999994], [True] * 4,
                      [float("nan"), 1., 1., 1.], [1., 1., 1.], None):
            self.assertFalse(m5.white_color(value))

    def test_changed_world_target_stops_before_any_press(self):
        adopted = contract()
        for phase, key in (("m5-legal", "placement"), ("m5-selected", "selection")):
            original = observation(adopted, phase, 1)
            baseline = copy.deepcopy(original["m5_planning"])
            baseline["blueprint_owner"] = 20
            m5.check_world_action(original, key, adopted, baseline)
            for mutate in (
                lambda v: v.update(pointer_over_ui=True),
                lambda v: v["m5_planning"].update(play_mode="TaskDesignation"),
                lambda v: v["m5_planning"].update(cursor_grid=[6, 5]),
                lambda v: v["m5_planning"].update(draw_pos=[1, 0]),
                lambda v: v["m5_planning"].update(legal=(key != "placement")),
            ):
                value = copy.deepcopy(original)
                mutate(value)
                driver = SimpleNamespace(last=value, events=[])
                with patch.object(m5.copy, "deepcopy", wraps=copy.deepcopy), \
                     patch.object(m5.native, "atomic_write_json"):
                    with self.subTest(phase=phase), self.assertRaises(RuntimeError):
                        m5.capture_action(driver, key, world=True, contract=adopted, baseline=baseline)
                self.assertEqual(driver.events, [])

    def test_four_positive_synthetic_campaigns(self):
        for kind in m5.KINDS.values():
            adopted = contract(kind)
            m5.verify(*campaign(adopted), adopted)

    def test_missing_reordered_and_foreign_current_evidence_is_rejected(self):
        adopted = contract()
        original = campaign(adopted)
        for mutate in (
            lambda r, e, c, i: r.update(scope="ordinary-rest-art-feedback-v1"),
            lambda r, e, c, i: r.update(promotion_authority=True),
            lambda r, e, c, i: e.reverse(),
            lambda r, e, c, i: e[3]["value"]["m5_planning"]["blueprints"][0].update(entity=200),
            lambda r, e, c, i: e[4]["value"]["m5_planning"].update(reject_reason="NotInYard"),
            lambda r, e, c, i: e[-1]["value"].update(selected=200),
            lambda r, e, c, i: e[-1]["value"]["controls"]["text:inspection-header"].update(fully_visible=False),
            lambda r, e, c, i: r["events"].pop(),
            lambda r, e, c, i: i[0]["m5_action"].update(key="F5"),
            lambda r, e, c, i: r["events"][0].update(window="foreign"),
            lambda r, e, c, i: r["events"][0].update(state="Released"),
            lambda r, e, c, i: r["events"][0].update(event_index=True),
            lambda r, e, c, i: r["samples"][0].update(real_seconds=99),
            lambda r, e, c, i: c.update(pid=41),
            lambda r, e, c, i: e[2]["value"]["m5_planning"].update(cursor_grid=[6, 5]),
            lambda r, e, c, i: e[2]["value"]["m5_planning"]["camera"].update(primary=False),
            lambda r, e, c, i: e[2].update(last_step=2),
            lambda r, e, c, i: next(row for row in r["events"] if row["producer"] == "ordinary-building-placement-branch-v1").update(real_seconds=8.01),
        ):
            changed = copy.deepcopy(original)
            mutate(*changed)
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                m5.verify(*changed, adopted)

    def test_foreign_motion_cannot_hide_outside_button_action_claims(self):
        for field, value in (("pid", 999), ("nonce", "foreign"), ("window", 51), ("world_epoch", 1),
                             ("point", [-1, 50]), ("observed_cursor", [900, 900]), ("pressed", True),
                             ("key", "F5"), ("pre_frame", True)):
            client = {"pid": 40, "nonce": "b" * 32, "window": 50}
            row = {"step": "1", "point": [400, 400], "pressed": None, "observed_cursor": [400., 400.],
                   "pid": 40, "nonce": client["nonce"], "window": 50, "world_epoch": 0,
                   "pre_frame": 10, "post_frame": 20}
            m5.check_transport([row], client, 0)
            row[field] = value
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                m5.check_transport([row], client, 0)

    def test_actual_2d_consumers_reject_geometry_and_tint_substitution(self):
        adopted = contract()
        for phase in ("m5-legal", "m5-illegal", "m5-placed", "m5-selected"):
            original = feedback(adopted, phase)
            m5.check_consumers(original, adopted, phase, 20 if phase in m5.CHECKPOINTS[3:] else None, draw_pos=[0., 0.])
            for field, value in (("image", "fallback"), ("size", [32, 32]), ("anchor", [1, 1]),
                                 ("owner", 200), ("global_matrix", [float("nan")] * 16), ("color", [True] * 4),
                                 ("layers", [1]), ("parent", 200), ("pose", {})):
                changed = copy.deepcopy(original)
                changed["sprites"][0][field] = value
                with self.subTest(phase=phase, field=field), self.assertRaises(RuntimeError):
                    m5.check_consumers(changed, adopted, phase, 20 if phase in m5.CHECKPOINTS[3:] else None, draw_pos=[0., 0.])

    def test_unstarted_blueprint_has_no_fabricated_pulse(self):
        adopted = contract()
        evidence = feedback(adopted, "m5-placed")
        evidence["sprites"] = [r for r in evidence["sprites"] if r["consumer"] == "blueprint"]
        root = evidence["sprites"][0]
        root.update(blueprint_state="NeedsMaterials", blueprint_progress=0., color=[.1, .5, 1., .4])
        root["pose"]["scale"] = [.9] * 3
        root["global_matrix"] = [.9, 0, 0, 0, 0, .9, 0, 0, 0, 0, .9, 0, 0, 0, .2, 1]
        m5.check_consumers(evidence, adopted, "m5-placed", 20, draw_pos=[0., 0.])
        root["visible"] = False
        with self.assertRaises(RuntimeError):
            m5.check_consumers(evidence, adopted, "m5-placed", 20, draw_pos=[0., 0.])

    def test_case_scope_backend_and_foreign_source_rejected_before_planning(self):
        args = SimpleNamespace(case="m5-sand-planning", m5_art_source="/source", art_codec="/codec",
                               smoke=True, input_backend="portal-eis", layout_menu=False)
        with patch.object(m5, "source_contract", return_value=contract()) as source:
            m5.planned_contract(args, Path("/repo"))
            self.assertEqual(source.call_count, 1)
            for key, value in (("case", "bridge-planning"), ("smoke", False), ("input_backend", "none"),
                               ("input_backend", "portal"), ("rest_area_art_source", "/foreign"), ("spa_resume_source", "/old")):
                changed = copy.copy(args)
                setattr(changed, key, value)
                with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                    m5.planned_contract(changed, Path("/repo"))


if __name__ == "__main__":
    unittest.main()
