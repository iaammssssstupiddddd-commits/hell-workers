"""Synthetic rejection coverage only; never native/art/release evidence."""
import copy
import json
from pathlib import Path
import unittest
from contextlib import nullcontext
from unittest.mock import patch

from scripts import building_m2_ui_acceptance as recipe


def observed(case="m2-ui-tank-partial"):
    kind, count = recipe.CASES[case]
    identity = {"kind": kind, "generation": 1, "authority": "art_preview", "manifest_sha256": "a" * 64}
    owner, target = 40, 41 if kind == "MudMixer" else 40
    ctx = {"scope": recipe.SCOPE, "case": case, "ready": True, "identity": identity, "active": identity,
           "kind": kind, "owner": owner, "target": target, "inventory_overflow": False,
           "target_point": [900., 500.], "owner_point": [900., 510.], "virtual_seconds": 2.,
           "natural_lifecycle_evidence": False, "performance_evidence": False, "promotion_authority": False,
           "stockpile": {"capacity": 50, "resource": "Some(Water)"},
           "items": [{"entity": 100+i, "owner": owner, "resource": "Water"} for i in range(count or 0)],
           "stored_items": list(range(100, 100+(count or 0))), "workers": [], "working_on": [], "assigned": [], "worker": None}
    if kind == "MudMixer":
        ctx.update(workers=[target], working_on=[target], assigned=[target], worker={"entity": target, "soul": True,
            "mixer": owner, "working_on": owner, "assignment": owner, "target": owner, "work_type": "Refine",
            "phase": "Refining { progress: 0.0 }", "progress": 0.})
    text = f"Building: Tank: Water ({count}/50)" if kind == "Tank" else "Task: Refine (Refining { progress: 0.0 })"
    return {"m2_ui": ctx, "nonce": "b" * 32, "ready": True, "paused": True, "window_focused": True, "world_input_captured": False,
            "pointer_over_ui": False, "menu_state": "Hidden", "pid": 10, "frame": 100, "world_epoch": 0, "primary_window": 90,
            "hovered": target, "pointer_target": target, "tooltip_target": target, "tooltip_epoch": 0,
            "wheel_events": [], "wheel_overflow": False, "viewport": [1920., 1080.], "ui_scale": 1.,
            "camera_numeric": [{"entity": 91, "active": True, "target_window": 90, "scale": [5., 5., 5.], "orthographic_scale": 1.}],
            "cursor": [900., 500.], "scale_factor": 1., "tooltip_alpha": 1., "tooltip_roots": [80],
            "tooltip_presentation": {"entity": 80, "rect": [900., 510., 1200., 600.]},
            "tooltip_text": [{"text": text, "entity": 81, "root": 80, "rect": [910., 520., 1190., 540.],
                              "fully_visible": True, "primary_camera": True, "alpha": 1.}]}


class M2UiTests(unittest.TestCase):
    def test_mixer_task_can_wrap_only_across_two_adjacent_owned_visible_rows(self):
        value = observed("m2-ui-mixer-refining")
        first = value["tooltip_text"][0]
        first.update(text="Task: Refine (Refining { progress:", rect=[1005., 686., 1216., 703.])
        second = copy.deepcopy(first)
        second.update(entity=82, text="0.0 })", rect=[1005., 703., 1216., 720.])
        value["tooltip_text"].append(second)
        recipe.check_observation("m2-ui-mixer-refining", value, value["m2_ui"]["identity"])
        for mutate in (
            lambda v: v["tooltip_text"][1].update(root=99),
            lambda v: v["tooltip_text"][1].update(entity=81),
            lambda v: v["tooltip_text"][1].update(text="1.0 })"),
            lambda v: v["tooltip_text"][1].update(fully_visible=False),
            lambda v: v["tooltip_text"][1].update(primary_camera=False),
            lambda v: v["tooltip_text"][1].update(alpha=.9999),
            lambda v: v["tooltip_text"][1].update(rect=[1005., 900., 1216., 917.]),
            lambda v: v["tooltip_text"][1].update(rect=[969., 703., 1216., 720.]),
            lambda v: v["tooltip_text"][1].update(rect=[1005., 600., 1216., 617.]),
            lambda v: v["tooltip_text"][1].update(rect=[1005., 1078., 1216., 1095.]),
        ):
            changed = copy.deepcopy(value)
            mutate(changed)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-mixer-refining", changed, changed["m2_ui"]["identity"])

    def test_capture_readiness_waits_for_all_real_tooltip_fades_to_finish(self):
        value = observed()
        identity = value["m2_ui"]["identity"]
        recipe.check_observation("m2-ui-tank-partial", value, identity)
        for alpha in (.95, .9999507069587708, .9999998211860657, float("nan"), 1.001, True):
            for root in (False, True):
                changed = copy.deepcopy(value)
                if root:
                    changed["tooltip_alpha"] = alpha
                else:
                    changed["tooltip_text"][0]["alpha"] = alpha
                with self.subTest(alpha=alpha, root=root), self.assertRaises(ValueError):
                    recipe.check_observation("m2-ui-tank-partial", changed, identity)
        changed = copy.deepcopy(value)
        extra = copy.deepcopy(changed["tooltip_text"][0])
        extra.update(entity=82, text="Building: Tank", alpha=.9999507069587708)
        changed["tooltip_text"].append(extra)
        with self.assertRaises(ValueError):
            recipe.check_observation("m2-ui-tank-partial", changed, identity)

    def test_cli_passes_the_frozen_namespace_through_actual_activity_guard(self):
        @recipe.native.activity_locked
        def guarded(args):
            self.assertEqual(args.repo, "/unused/repo")
            self.assertEqual(args.job_root, "/unused/job")
            self.assertEqual(args.plan, Path("/unused/plan.json"))
            return {"status": "valid"}
        lease_environment = {recipe.native.ACTIVITY_LOCK_FD_ENV: "4", recipe.native.ACTIVITY_LOCK_MODE_ENV: "exclusive"}
        with patch("sys.argv", ["recipe", "run", "--plan", "/unused/plan.json"]), \
             patch.object(recipe.native, "read_json", return_value={"repo": "/unused/repo", "job_root": "/unused/job"}), \
             patch.object(recipe.native, "require_admission") as admission, \
             patch.object(recipe.native, "acquire_activity", return_value=nullcontext(None)), \
             patch.object(recipe.native, "activity_lease_environment", return_value=lease_environment), \
             patch.object(recipe.native, "print_json"), patch.object(recipe, "run", guarded):
            self.assertEqual(recipe.main(), 0)
            admission.assert_called_once_with(Path("/unused/repo"), [Path("/unused/job")])

    def test_actual_session_setting_pins_normal_gallery_admission(self):
        for case in recipe.CASES:
            value = observed(case)
            session = json.loads(recipe.session_setting({"case": case, "input": {"identity": value["m2_ui"]["identity"]}}, Path("/unused/job"), "b" * 32))
            self.assertEqual((session["view"], session["variant"], session["lighting"], session["mode"]),
                             ("normal", "gallery", "standard", "feedback"))
            self.assertEqual(session["identity"], value["m2_ui"]["identity"])
            self.assertEqual(session["status_path"], "/unused/job/probe.json")

    def trace_fixture(self):
        value = observed()
        parts = [{"name": name, "translation_wu": [0., 0., 0.], "rotation_xyzw": [0., 0., 0., 1.], "scale": [1.] * 3}
                 for name in ("body", "water")]
        plan = {"case": "m2-ui-tank-partial", "input": {"identity": value["m2_ui"]["identity"], "parts": parts, "geometry_sha256": recipe.m2.NUMERIC_SHA256}}
        rows = []
        root_pose = {"translation": [0., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1.] * 3}
        root_matrix = recipe.art.foreground.matrix(root_pose)
        for time, frame in enumerate((90, 100, 110), start=1):
            ui_value = copy.deepcopy(value)
            ui_value["frame"] = frame
            rendered = [{"name": part["name"], "mesh_resident": True, "layer_3d": True, "mesh_handle": part["name"], "visible": True,
                         "entity": str(200+i), "parent": "root",
                         "material": {"albedo": "image", "albedo_resident": True, "opaque": True, "lit": True,
                                      "color": [.58, .70, .73, 1.] if part["name"] == "water" else [1.] * 4},
                         "pose": {"translation": [0., 12., 0.] if part["name"] == "water" else [0.] * 3,
                                  "rotation": [0., 0., 0., 1.], "scale": [1.] * 3}} for i, part in enumerate(parts)]
            for part in rendered:
                part["global_matrix"] = recipe.art.foreground.product(root_matrix, recipe.art.foreground.matrix(part["pose"]))
            rows.append({"real_seconds": float(time), "ui_art_observed": ui_value, "paused": True,
                         "identity": value["m2_ui"]["identity"], "world_epoch": 0, "virtual_seconds": 2.,
                         "fixture_owner": "40v1", "fixture_owner_bits": 40,
                         "owners": [{"entity": "40v1", "pose": {"translation": [0.] * 3, "rotation": [0., 0., 0., 1.], "scale": [1.] * 3}}],
                         "roots": [{"owner": "40v1", "entity": "root", "parent": None, "identity": value["m2_ui"]["identity"],
                                    "pose": root_pose, "global_matrix": root_matrix, "owner_exists": True, "state": "Some(TankPartial)", "parts": rendered}],
                         "descriptor_handles": {"meshes": {"body": "body", "water": "water"}, "albedo": "image"}})
        raw = {"scope": "m2-feedback-observations-only", "leg": "ui-art", "failure": None, "accepted": False,
               "numeric_candidate_sha256": recipe.m2.NUMERIC_SHA256,
               "performance_evidence": False, "identity": value["m2_ui"]["identity"], "nonce": value["nonce"], "process_id": 10, "samples": rows}
        before, after = copy.deepcopy(value), copy.deepcopy(value)
        before["frame"], after["frame"] = 95, 105
        return raw, plan, before, after

    def test_ui_revision_is_explicit_kind_bounded_and_not_inferred_from_catalog_order(self):
        sha=recipe.m2.numeric_contract('MudMixer','v6')[1]
        value={'case':'m2-ui-mixer-refining','input':{'revision':'v6','geometry_sha256':sha}}
        self.assertEqual(recipe.acceptance_revision(value),'v6')
        for revision,digest,case in ((None,sha,value['case']),('v6','0'*64,value['case']),
                                    ('v6',sha,'m2-ui-tank-full'),(True,sha,value['case']),
                                    ('v7',sha,value['case'])):
            changed=copy.deepcopy(value)
            changed['case']=case
            changed['input']={'geometry_sha256':digest}
            if revision is not None:
                changed['input']['revision']=revision
            with self.assertRaises(ValueError):
                recipe.acceptance_revision(changed)
        self.assertEqual(recipe.acceptance_revision({'case':'m2-ui-tank-empty','input':{'geometry_sha256':recipe.m2.NUMERIC_SHA256}}),'v1')

    def test_last_trace_binds_actual_production_parts_and_never_mutates(self):
        raw, plan, before, after = self.trace_fixture()
        old = copy.deepcopy(raw)
        recipe.check_trace(raw, plan, before, after)
        self.assertEqual(raw, old)

    def test_production_root_tracks_owner_ground_plane_and_rejects_fallback_elevation(self):
        raw, plan, before, after = self.trace_fixture()
        for row in raw["samples"]:
            row["owners"][0]["pose"]["translation"] = [-1312., -1312., .12]
            root = row["roots"][0]
            root["pose"]["translation"] = [-1312., 0., 1312.]
            root["global_matrix"] = recipe.art.foreground.matrix(root["pose"])
            for part in root["parts"]:
                part["global_matrix"] = recipe.art.foreground.product(root["global_matrix"], recipe.art.foreground.matrix(part["pose"]))
        recipe.check_trace(raw, plan, before, after)
        for y in (12.8, .12, 1.):
            wrong = copy.deepcopy(raw)
            for row in wrong["samples"]:
                root = row["roots"][0]
                root["pose"]["translation"][1] = y
                root["global_matrix"] = recipe.art.foreground.matrix(root["pose"])
                for part in root["parts"]:
                    part["global_matrix"] = recipe.art.foreground.product(root["global_matrix"], recipe.art.foreground.matrix(part["pose"]))
            with self.subTest(y=y), self.assertRaises(ValueError):
                recipe.check_trace(wrong, plan, before, after)

    def test_last_trace_rejects_fallback_foreign_missing_parts_and_drift(self):
        for mutate in (
            lambda r: r["samples"][1].update(roots=[]),
            lambda r: r["samples"][1].update(fixture_owner_bits=99),
            lambda r: r["samples"][1].update(paused=False),
            lambda r: r["samples"][1].update(virtual_seconds=3.),
            lambda r: r["samples"][1]["roots"][0].update(state="Fallback"),
            lambda r: r["samples"][1]["roots"][0].update(entity="foreign-root"),
            lambda r: r["samples"][1]["roots"][0].update(parent="foreign-parent"),
            lambda r: r["samples"][1]["roots"][0].update(pose={"translation": [1, 2, 3], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1]}),
            lambda r: r["samples"][1]["roots"][0]["parts"][0].update(mesh_handle="foreign"),
            lambda r: r["samples"][1]["roots"][0]["parts"][0].update(mesh_resident=False),
            lambda r: r["samples"][1]["roots"][0]["parts"][0].update(visible=False),
            lambda r: r["samples"][1]["roots"][0]["parts"][0]["material"].update(albedo="foreign"),
            lambda r: r["samples"][1]["roots"][0]["parts"][0]["material"].update(albedo_resident=False),
            lambda r: r["samples"][1]["roots"][0]["parts"][0]["pose"].update(translation=[1, 2, 3]),
            lambda r: r["samples"].pop(),
            lambda r: r["samples"][1].update(real_seconds=0.),
        ):
            raw, plan, before, after = self.trace_fixture()
            mutate(raw)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                recipe.check_trace(raw, plan, before, after)

    def test_consistently_missing_or_boolean_paused_clock_cannot_be_accepted(self):
        for missing in (None, True, float("inf"), -1.):
            raw, plan, before, after = self.trace_fixture()
            before["m2_ui"]["virtual_seconds"] = after["m2_ui"]["virtual_seconds"] = missing
            for row in raw["samples"]:
                row["virtual_seconds"] = missing
                row["ui_art_observed"]["m2_ui"]["virtual_seconds"] = missing
            with self.subTest(missing=missing), self.assertRaises(ValueError):
                recipe.check_trace(raw, plan, before, after)

    def test_exact_cases_do_not_mutate_or_grant_authority(self):
        for case in recipe.CASES:
            value = observed(case)
            original = copy.deepcopy(value)
            recipe.check_observation(case, value, value["m2_ui"]["identity"])
            self.assertEqual(value, original)
            self.assertFalse(value["m2_ui"]["promotion_authority"])

    def test_tank_rejects_same_count_non_water_and_inverse_mismatch(self):
        for mutate in (
            lambda v: v["m2_ui"]["items"][0].update(resource="Rock"),
            lambda v: v["m2_ui"]["items"][0].update(owner=99),
            lambda v: v["m2_ui"]["items"][0].update(entity=101),
            lambda v: v["m2_ui"]["stored_items"].pop(),
            lambda v: v["m2_ui"]["stockpile"].update(capacity=50.0),
            lambda v: v["m2_ui"]["active"].update(generation=True),
        ):
            value = observed()
            identity = copy.deepcopy(value["m2_ui"]["identity"])
            mutate(value)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-tank-partial", value, identity)

    def test_refining_requires_actual_exact_worker_and_all_inverse_sets(self):
        for key, changed in [("mixer", 99), ("working_on", 99), ("target", 99), ("assignment", 99),
                             ("soul", False), ("work_type", "GeneratePower"), ("phase", "GoingToMixer"),
                             ("progress", False), ("progress", .1)]:
            value = observed("m2-ui-mixer-refining")
            value["m2_ui"]["worker"][key] = changed
            with self.subTest(key=key, changed=changed), self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-mixer-refining", value, value["m2_ui"]["identity"])
        for key in ("workers", "working_on", "assigned"):
            value = observed("m2-ui-mixer-refining")
            value["m2_ui"][key] = [99]
            with self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-mixer-refining", value, value["m2_ui"]["identity"])

    def test_foreign_transparent_clipped_or_wrong_camera_text_is_rejected(self):
        for key, changed in [("root", 99), ("alpha", .5), ("fully_visible", False), ("primary_camera", False),
                             ("rect", [-1, 20, 100, 40]), ("text", "Building: Tank: Water (50/50)")]:
            value = observed()
            value["tooltip_text"][0][key] = changed
            with self.subTest(key=key), self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-tank-partial", value, value["m2_ui"]["identity"])

    def test_stale_hover_changed_projection_and_unclaimed_wheel_are_rejected(self):
        mutations = [lambda v: v.update(hovered=99), lambda v: v.update(pointer_target=99),
                     lambda v: v.update(tooltip_epoch=1), lambda v: v.update(paused=False),
                     lambda v: v.update(cursor=[1., 2.]), lambda v: v.update(wheel_events=[{}]),
                     lambda v: v.update(tooltip_roots=[80, 99]),
                     lambda v: v["camera_numeric"][0].update(orthographic_scale=2.),
                     lambda v: v["m2_ui"].update(promotion_authority=True)]
        for mutate in mutations:
            value = observed()
            mutate(value)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                recipe.check_observation("m2-ui-tank-partial", value, value["m2_ui"]["identity"])


if __name__ == "__main__":
    unittest.main()
