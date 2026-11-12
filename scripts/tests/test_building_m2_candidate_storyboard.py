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
            sample = {"paused": paused, "virtual_seconds": clock, "storyboard_stage": index}
            root = {"entity": "root", "state": f"Some(Mixer{state})", "parts": [
                {"name": "body", "entity": "body", "pose": pose()},
                {"name": "rotor", "entity": "rotor", "pose": pose(candidate.quaternion_step(
                    [0., 0., 0., 1.], [0., 1., 0.], angle))}]}
            self.rows.append((sample, root))

    def test_approved_axis_speed_pause_resume_and_idle_hold_pass(self):
        candidate.check_motion(self.rows, self.value)

    def test_body_motion_part_replacement_and_clock_disagreement_fail(self):
        for change in ("body", "part", "clock", "axis"):
            rows, value = copy.deepcopy((self.rows, self.value))
            if change == "body":
                rows[2][1]["parts"][0]["pose"]["rotation"] = [0., 1., 0., 0.]
            elif change == "part":
                rows[5][1]["parts"][1]["entity"] = "replacement"
            elif change == "clock":
                rows[2][0]["virtual_seconds"] += .2
            else:
                value["production_state"]["axis"] = [1., 0., 0.]
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_motion(rows, value)

    def test_pause_and_idle_drift_fail(self):
        for index in (4, 8):
            rows = copy.deepcopy(self.rows)
            rows[index][1]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            with self.subTest(index=index), self.assertRaises(ValueError):
                candidate.check_motion(rows, self.value)

    def test_load_requires_new_parts_paused_idle_and_reset_angle(self):
        value = {**self.value, "leg": "save-load"}
        rows = [copy.deepcopy(self.rows[0]), copy.deepcopy(self.rows[0])]
        rows[1][0]["paused"] = True
        rows[1][1]["entity"] = "new-root"
        for leaf in rows[1][1]["parts"]:
            leaf["entity"] = "new-" + leaf["entity"]
        candidate.check_motion(rows, value)
        for change in ("pause", "root", "part", "rotation"):
            invalid = copy.deepcopy(rows)
            if change == "pause":
                invalid[1][0]["paused"] = False
            elif change == "root":
                invalid[1][1]["entity"] = "root"
            elif change == "part":
                invalid[1][1]["parts"][0]["entity"] = "body"
            else:
                invalid[1][1]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            with self.subTest(change=change), self.assertRaises(ValueError):
                candidate.check_motion(invalid, value)

    def test_refining_captures_must_show_resume_after_pause(self):
        def shot(state, paused):
            return {"key": [True, 1., "Normal", [f"Some(Mixer{state})"], [], paused]}
        shots = [shot("Idle", False), shot("Active", False), shot("Active", True),
                 shot("Active", False), shot("Idle", False)]
        candidate.check_state_checkpoints(shots, self.value | {"identity": {"kind": "MudMixer"}})
        with self.assertRaises(ValueError):
            candidate.check_state_checkpoints(shots[:3] + shots[4:], self.value | {"identity": {"kind": "MudMixer"}})


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
