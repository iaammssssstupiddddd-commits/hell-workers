from __future__ import annotations

import copy
import unittest
import tempfile
from pathlib import Path
from unittest.mock import patch

from scripts import building_m2_candidate_storyboard as candidate
from scripts import building_m2_native_acceptance as legacy


def pose(rotation=None):
    return {"translation": [0., 0., 0.], "rotation": rotation or [0., 0., 0., 1.], "scale": [1., 1., 1.]}


class CandidateMotionTests(unittest.TestCase):
    def setUp(self):
        self.value = {"kind": "MudMixer", "leg": "refining", "parts": [
            {"name": name, "translation_wu": [0., 0., 0.], "rotation_xyzw": [0., 0., 0., 1.],
             "scale": [1., 1., 1.]} for name in ("body", "rotor")],
            "production_state": {"axis": [0., 1., 0.], "radians_per_second": 1.}}
        self.rows = []
        for index, (state, paused, clock, angle) in enumerate([
            ("Idle", False, 0., 0.), ("Active", False, .1, .1), ("Active", False, .2, .2),
            ("Active", True, .2, .2), ("Active", True, .2, .2),
            ("Active", False, .3, .3), ("Active", False, .4, .4),
            ("Idle", False, .5, .4), ("Idle", False, .6, .4)]):
            sample = {"paused": paused, "virtual_seconds": clock, "storyboard_stage": index, "storyboard_clock": 0.25}
            root = {"entity": "root", "state": f"Some(Mixer{state})", "parts": [
                {"name": "body", "entity": "body", "pose": pose()},
                {"name": "rotor", "entity": "rotor", "pose": pose(candidate.quaternion_step(
                    [0., 0., 0., 1.], [0., 1., 0.], angle))}]}
            self.rows.append((sample, root))

    def test_approved_axis_speed_pause_resume_and_idle_hold_pass(self):
        candidate.check_motion(self.rows, self.value)

    def test_body_motion_part_replacement_and_clock_disagreement_fail(self):
        for change in ("body", "part", "clock", "axis", "relative-speed"):
            rows, value = copy.deepcopy((self.rows, self.value))
            if change == "body":
                rows[2][1]["parts"][0]["pose"]["rotation"] = [0., 1., 0., 0.]
            elif change == "part":
                rows[5][1]["parts"][1]["entity"] = "replacement"
            elif change == "clock":
                rows[2][0]["virtual_seconds"] += .2
            elif change == "axis":
                value["production_state"]["axis"] = [1., 0., 0.]
            else:
                rows[2][0]["storyboard_clock"] = 1.0
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_motion(rows, value)

    def test_pause_and_idle_drift_fail(self):
        for index in (4, 8):
            rows = copy.deepcopy(self.rows)
            rows[index][1]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            with self.subTest(index=index), self.assertRaises(ValueError):
                candidate.check_motion(rows, self.value)

    def saved_rows(self):
        rows = copy.deepcopy(self.rows[:5])
        for sample, _ in rows:
            sample.update(world_epoch=0, fixture_owner="owner", lifecycle={"events": []})
        rows[4][0]["lifecycle"]["events"] = [{"operation": "Save", "result": "Succeeded"}]
        loaded = copy.deepcopy(rows[0])
        loaded[0].update(paused=True, world_epoch=1, fixture_owner="new-owner", virtual_seconds=0., lifecycle={"events": [
            {"operation": "Save", "result": "Succeeded"}, {"operation": "Load", "result": "Succeeded"}]})
        loaded[1]["entity"] = "new-root"
        for leaf in loaded[1]["parts"]:
            leaf["entity"] = "new-" + leaf["entity"]
        rows.extend([copy.deepcopy(rows[4]), loaded, copy.deepcopy(loaded)])
        return rows

    def test_load_requires_saved_nonzero_active_pause_and_new_idle_parts(self):
        value = {**self.value, "leg": "save-load"}
        rows = self.saved_rows()
        candidate.check_motion(rows, value)
        for change in ("pause", "root", "part", "rotation", "saved-idle", "saved-zero",
                       "save-outcome", "load-outcome", "epoch", "resume", "virtual", "clock"):
            invalid = copy.deepcopy(rows)
            if change == "pause":
                invalid[-1][0]["paused"] = False
            elif change == "root":
                invalid[-1][1]["entity"] = "root"
            elif change == "part":
                invalid[-1][1]["parts"][0]["entity"] = "body"
            elif change == "rotation":
                invalid[-1][1]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            elif change == "saved-idle":
                invalid[4][1]["state"] = "Some(MixerIdle)"
            elif change == "saved-zero":
                for row in invalid[:-1]:
                    row[1]["parts"][1]["pose"]["rotation"] = [0., 0., 0., 1.]
                # Exercise saved-state predicate independently of movement speed.
                with self.assertRaisesRegex(ValueError, "nonzero"):
                    candidate.check_saved_reset(invalid, {p["name"]: p for p in value["parts"]})
                continue
            elif change == "save-outcome":
                for row in invalid[:-1]:
                    row[0]["lifecycle"]["events"] = []
            elif change == "load-outcome":
                invalid[-1][0]["lifecycle"]["events"].pop()
            elif change == "epoch":
                invalid[-1][0]["world_epoch"] = 0
            elif change == "resume":
                invalid[5][0]["paused"] = False
            elif change == "virtual":
                invalid[5][0]["virtual_seconds"] += .1
            else:
                invalid[4][0]["storyboard_clock"] = 1.
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_motion(invalid, value)

    def test_saved_and_loaded_tail_ownership_epoch_and_pause_are_fixed(self):
        for change in ("saved-part", "saved-owner", "loaded-part", "loaded-root", "loaded-owner",
                       "loaded-epoch", "bool-epoch", "loaded-virtual", "skipped-epoch"):
            rows = self.saved_rows()
            if change == "saved-part":
                rows[5][1]["parts"][0]["entity"] = "transient-body"
            elif change == "saved-owner":
                rows[5][0]["fixture_owner"] = "transient-owner"
            elif change == "loaded-part":
                rows[-1][1]["parts"][0]["entity"] = "another-body"
            elif change == "loaded-root":
                rows[-1][1]["entity"] = "another-root"
            elif change == "loaded-owner":
                rows[-1][0]["fixture_owner"] = "another-owner"
            elif change == "loaded-epoch":
                rows[-1][0]["world_epoch"] = 2
            elif change == "bool-epoch":
                rows[3][0]["world_epoch"] = False
            elif change == "loaded-virtual":
                rows[-1][0]["virtual_seconds"] += .1
            else:
                for row in rows[-2:]:
                    row[0]["world_epoch"] = 2
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_motion(rows, {**self.value, "leg": "save-load"})

    def test_idle_to_idle_load_does_not_prove_reset(self):
        rows = self.saved_rows()
        for _, root in rows:
            root["state"] = "Some(MixerIdle)"
            root["parts"][1]["pose"]["rotation"] = [0., 0., 0., 1.]
        with self.assertRaisesRegex(ValueError, "nonzero"):
            candidate.check_motion(rows, {**self.value, "leg": "save-load"})

    def test_load_images_require_active_pause_then_new_epoch_idle_pause(self):
        def shot(state, epoch):
            return {"key": [True, 1., "Normal", [f"Some(Mixer{state})"], [], True, 3, None, epoch]}
        value = self.value | {"leg": "save-load", "identity": {"kind": "MudMixer"}}
        initial = {"key": [True, 1., "Normal", ["Some(MixerIdle)"], [], False, 1, None, 0]}
        shots = [initial, shot("Active", 0), shot("Idle", 1)]
        candidate.check_state_checkpoints(shots, value)
        for invalid in ([initial, shot("Idle", 1)], [initial, shot("Active", 0), shot("Idle", 0)],
                        [initial, shot("Idle", 1), shot("Active", 0)]):
            with self.assertRaises(ValueError):
                candidate.check_state_checkpoints(invalid, value)

    def test_refining_captures_must_show_resume_after_pause(self):
        def shot(state, paused):
            return {"key": [True, 1., "Normal", [f"Some(Mixer{state})"], [], paused]}
        shots = [shot("Idle", False), shot("Active", False), shot("Active", True),
                 shot("Active", False), shot("Idle", False)]
        candidate.check_state_checkpoints(shots, self.value | {"identity": {"kind": "MudMixer"}})
        with self.assertRaises(ValueError):
            candidate.check_state_checkpoints(shots[:3] + shots[4:], self.value | {"identity": {"kind": "MudMixer"}})


class CandidateMoveActorTests(unittest.TestCase):
    def rows(self):
        family = {"entity": "familiar", "name": "M2 lifecycle Familiar"}
        worker = {"entity": "worker", "commander": "familiar", "move_task": None,
                  "working_on": None, "identity": None}
        task = worker | {"move_task": {"assignment": "order", "building": "owner", "manager": "familiar"},
                         "working_on": "order", "identity": {"assignment": "order", "target": "order", "work_type": "Move"}}
        return [{"fixture_owner": "owner", "storyboard_move_actors": {"familiars": [], "workers": []}},
                {"fixture_owner": "owner", "storyboard_move_actors": {"familiars": [family], "workers": [task]}},
                {"fixture_owner": "owner", "storyboard_move_actors": {"familiars": [family], "workers": [worker]}}]

    def test_exact_move_fixture_has_no_startup_familiars_and_one_owned_assignment(self):
        candidate.check_move_actors(self.rows())
        self.assertEqual(candidate.probe_environment({"leg": "move-success", "actor_scope": candidate.MOVE_ACTORS}),
                         {"HW_M2_CANDIDATE_STORYBOARD": "1", "HW_SPAWN_FAMILIARS": "0"})
        for leg in ("refining", "save-load"):
            self.assertEqual(candidate.probe_environment({"leg": leg}), {"HW_M2_CANDIDATE_STORYBOARD": "1"})
        with self.assertRaises(ValueError):
            candidate.probe_environment({"leg": "move-success", "actor_scope": "ordinary-startup-actors"})

    def test_foreign_actor_assignment_and_unobserved_move_are_rejected(self):
        for change in ("startup", "familiar", "worker", "commander", "manager", "target", "assignment", "owner", "building-as-target", "no-move"):
            rows = copy.deepcopy(self.rows())
            actors = rows[1]["storyboard_move_actors"]
            worker = actors["workers"][0]
            if change == "startup":
                rows[0]["storyboard_move_actors"] = copy.deepcopy(actors)
            elif change == "familiar":
                actors["familiars"][0]["name"] = "foreign"
            elif change == "worker":
                rows[-1]["storyboard_move_actors"]["workers"][0]["entity"] = "replacement"
            elif change == "commander":
                worker["commander"] = "foreign"
            elif change == "manager":
                worker["move_task"]["manager"] = "foreign"
            elif change == "target":
                worker["identity"]["target"] = "foreign"
            elif change == "assignment":
                worker["identity"]["assignment"] = "foreign"
            elif change == "owner":
                worker["move_task"]["building"] = "foreign"
            elif change == "building-as-target":
                worker["working_on"] = worker["identity"]["target"] = "owner"
            else:
                worker["move_task"] = None
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_move_actors(rows)


class CandidateBoundaryTests(unittest.TestCase):
    def test_legacy_scope_cannot_accept_candidate_or_unknown_scope(self):
        value = {"identity": {"authority": "isolated_candidate"}}
        for scope in ("m2-feedback-observations-only", "invented"):
            with self.subTest(scope=scope), self.assertRaisesRegex(ValueError, "scope/authority"):
                legacy.check_trace({}, value, "a"*32, 42, scope=scope)
        with self.assertRaisesRegex(ValueError, "scope/authority"):
            legacy.check_trace({}, {"identity": {"authority": "art_preview"}}, "a"*32, 42,
                               scope=candidate.SCOPE)

    def test_capture_prefix_keeps_scope_and_candidate_phase_separate(self):
        before = {"scope": candidate.SCOPE, "samples": []}
        with self.assertRaises(ValueError):
            legacy.check_capture_prefix(before, {"scope": "m2-feedback-observations-only", "samples": []})
        row = {"roots": [], "consumers": [], "paused": False}
        initial = legacy.checkpoint_key(row | {"storyboard_stage": 2})
        resumed = legacy.checkpoint_key(row | {"storyboard_stage": 4})
        moved = legacy.checkpoint_key(row | {"storyboard_stage": 4, "destination_owner": "owner"})
        self.assertNotEqual(initial, resumed)
        self.assertNotEqual(resumed, moved)
        self.assertEqual(len(legacy.checkpoint_key(row)), 6)

    def test_same_path_numeric_dependency_change_is_rejected_before_codec(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            (root / "fixtures").mkdir()
            contract = root / "fixtures/contract.json"
            contract.write_text("original")
            with patch.object(candidate.pipeline, "__file__", str(root / "scripts/pipeline.py")), \
                 patch.object(candidate.pipeline, "NUMERIC_VERIFIER_FILES", ("fixtures/contract.json",)):
                value = {"drivers": candidate.drivers(), "numeric_verifiers": candidate.pipeline.numeric_verifier_inventory()}
                candidate.check_dependencies(value)
                contract.write_text("changed")
                with patch.object(candidate.pipeline, "load_set") as codec:
                    with self.assertRaisesRegex(ValueError, "dependencies changed"):
                        candidate.check_dependencies(value)
                    codec.assert_not_called()

    def test_candidate_capture_rejects_hidden_and_foreign_layer_parts(self):
        sample = {"fixture_owner": "owner", "identity": {"kind": "Tank"}, "world_epoch": 1,
            "paused": False, "consumers": [], "roots": [{"owner": "owner", "owner_exists": True,
            "state": "Some(TankEmpty)", "parts": [
                {"name": "body", "visible": True, "visibility": "Some(Inherited)", "layer_3d": True},
                {"name": "water", "visible": False, "visibility": "Some(Hidden)", "layer_3d": True}]}]}
        samples = [copy.deepcopy(sample), copy.deepcopy(sample)]
        shot = {"sample_index": 0, "sample_end_index": 1, "key": legacy.checkpoint_key(sample)}
        candidate.check_capture_interval(samples, shot)
        for field, value in [("visible", False), ("layer_3d", False), ("visibility", "Some(Hidden)")]:
            changed = copy.deepcopy(samples)
            changed[1]["roots"][0]["parts"][0][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "hidden|render layer"):
                candidate.check_capture_interval(changed, shot)
        changed = copy.deepcopy(samples)
        for row in changed:
            row["roots"][0]["state"] = "Some(TankFull)"
        with self.assertRaisesRegex(ValueError, "hidden|render layer"):
            candidate.check_capture_interval(changed, shot | {"key": legacy.checkpoint_key(changed[0])})

    def test_candidate_overlay_copies_exact_approvals_and_rejects_missing_or_changed_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, repo, view = root / "source", root / "repo", root / "view"
            (repo / "assets").mkdir(parents=True)
            assets = view / "assets"
            for relative in candidate.PROVENANCE_FILES:
                path = source / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(relative.encode())
            locator = candidate.pipeline.locator("Tank")
            candidate.pipeline.put(assets / locator, b"{}")
            candidate.prepare_overlay(assets, source)
            self.assertEqual(candidate.provenance_inventory(assets), candidate.provenance_inventory(source))
            manifest = {"identity": {"kind": "Tank"}, "artifacts": []}
            candidate.verify_overlay(repo, view, manifest, "{}", source)
            approval = assets / "provenance/numeric-approval.json"
            original = approval.read_bytes()
            approval.write_bytes(b"changed approval")
            with self.assertRaises(ValueError):
                candidate.verify_overlay(repo, view, manifest, "{}", source)
            approval.unlink()
            with self.assertRaises(ValueError):
                candidate.verify_overlay(repo, view, manifest, "{}", source)
            approval.write_bytes(original)
            candidate.verify_overlay(repo, view, manifest, "{}", source)

    def test_candidate_copy_is_validated_before_codec_or_plan(self):
        with patch.object(candidate.shared.pipeline, "no_symlinks", side_effect=lambda p: p), \
             patch.object(candidate.shared.pipeline, "read", return_value={"plan": {}, "binary": {"path": "/foreign", "sha256": "a"*64}}), \
             patch.object(candidate, "check_plan") as plan:
            with self.assertRaises(ValueError):
                candidate.verify(candidate.Path("/nonexistent"))
            plan.assert_not_called()
