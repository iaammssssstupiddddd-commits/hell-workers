"""The preview-only motion contract cannot be promoted or accept corrupt observations."""
import copy
import math
import unittest
from unittest.mock import patch

from scripts import building_m2_dirty_feedback_motion as motion
from scripts.tests.test_building_m2_candidate_storyboard import CandidateMotionTests


class DirtyMotionTests(unittest.TestCase):
    def setUp(self):
        fixture = CandidateMotionTests()
        fixture.setUp()
        self.value = fixture.value
        self.value.update(scope=motion.SCOPE, dirty_feedback=True, identity={
            "kind": "MudMixer", "generation": 1, "authority": "art_preview", "manifest_sha256": "a" * 64},
            production_state=motion.MOTION)
        self.nonce = "b" * 32
        self.rows = fixture.rows
        for index, (sample, root) in enumerate(self.rows):
            sample.update(identity=self.value["identity"], real_seconds=float(index + 1),
                fixture_owner="owner", owners=[{"entity": "owner", "pose": {"translation": [1, 2, 3]}}],
                roots=[root], items=[{"owner": "owner"}], consumers=[{"role": "world", "image": "albedo"}],
                expected_images={"world": "albedo"}, descriptor_handles={
                    "albedo": "albedo", "meshes": {"body": "body-mesh", "rotor": "rotor-mesh"}})
            root.update(owner="owner", owner_exists=True, identity=self.value["identity"],
                        pose={"translation": [1, 0, -2]})
            rotor = root["parts"][1]
            angle = (0., .1, .2, .2, .2, .3, .4, .4, .4)[index] * math.pi / 2
            rotor["pose"]["rotation"] = motion.storyboard.quaternion_step([0., 0., 0., 1.], [0., 1., 0.], angle)
            for leaf in root["parts"]:
                leaf.update(parent="root", mesh_resident=True, layer_3d=True,
                    mesh_handle=leaf["name"] + "-mesh", material={"albedo_resident": True,
                    "albedo": "albedo", "opaque": True, "lit": True})
        self.trace = {"schema_version": 1, "scope": motion.TRACE_SCOPE, "leg": "refining",
            "identity": self.value["identity"], "nonce": self.nonce, "process_id": 42,
            "failure": None, "accepted": False, "performance_evidence": False,
            "samples": [sample for sample, _ in self.rows]}

    def test_motion_and_scope_are_bound_without_approval(self):
        motion.check_trace(self.trace, self.value, self.nonce, 42)
        for field, replacement in (("scope", motion.shared.DIRTY_SCOPE), ("dirty_feedback", False),
                                   ("kind", "Tank"), ("leg", "save-load")):
            invalid = copy.deepcopy(self.value)
            invalid[field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                motion.check_trace(self.trace, invalid, self.nonce, 42)
        for field, replacement in (("authority", "isolated_candidate"), ("kind", "Tank")):
            invalid = copy.deepcopy(self.value)
            invalid["identity"][field] = replacement
            with self.subTest(field=field), self.assertRaises(ValueError):
                motion.check_trace(self.trace, invalid, self.nonce, 42)

    def test_changed_clock_pause_and_residency_fail(self):
        for change in ("clock", "pause", "mesh"):
            trace = copy.deepcopy(self.trace)
            if change == "clock":
                trace["samples"][2]["storyboard_clock"] = 1.
            elif change == "pause":
                trace["samples"][4]["roots"][0]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            else:
                trace["samples"][2]["roots"][0]["parts"][0]["mesh_resident"] = False
            with self.subTest(change=change), self.assertRaises(ValueError):
                motion.check_trace(trace, self.value, self.nonce, 42)

    def test_save_load_requires_paused_active_then_fresh_idle(self):
        trace, value = copy.deepcopy((self.trace, self.value))
        value["leg"] = trace["leg"] = "save-load"
        rows = trace["samples"][:5]
        for sample in rows:
            sample.update(world_epoch=0, lifecycle={"leg": "save-load", "events": []})
        rows[4]["lifecycle"]["events"] = [{"operation": "Save", "result": "Succeeded"}]
        rows.append(copy.deepcopy(rows[4]))
        loaded = copy.deepcopy(rows[0])
        loaded.update(paused=True, world_epoch=1, fixture_owner="new-owner", fixture_owner_exists=True, virtual_seconds=0.,
            lifecycle={"leg": "save-load", "events": [{"operation": "Save", "result": "Succeeded"},
                       {"operation": "Load", "result": "Succeeded"}]})
        loaded["owners"][0]["entity"] = "new-owner"
        root = loaded["roots"][0]
        root.update(owner="new-owner", entity="new-root")
        for part in root["parts"]:
            part.update(parent="new-root", entity="new-" + part["entity"])
        rows.extend([loaded, copy.deepcopy(loaded)])
        for index, sample in enumerate(rows):
            sample["real_seconds"] = float(index + 1)
        trace["samples"] = rows
        motion.check_trace(trace, value, self.nonce, 42)
        for change in ("owner", "rotation", "epoch", "pause"):
            damaged = copy.deepcopy(trace)
            last = damaged["samples"][-1]
            if change == "owner":
                last["roots"][0]["parts"][0]["entity"] = "body"
            elif change == "rotation":
                last["roots"][0]["parts"][1]["pose"]["rotation"] = [0., 1., 0., 0.]
            elif change == "epoch":
                last["world_epoch"] = 2
            else:
                last["paused"] = False
            with self.subTest(change=change), self.assertRaises(ValueError):
                motion.check_trace(damaged, value, self.nonce, 42)

    def test_plan_rejects_changed_contract_before_trusting_inputs(self):
        value = {**self.value, "input_scope": "seeded-domain-operations",
            "clock_scope": "preview-motion-quarter-speed-save-hold-v1", "drivers": motion.drivers(),
            "numeric_verifiers": motion.pipeline.numeric_verifier_inventory()}
        for key, changed in (("scope", motion.shared.SCOPE), ("dirty_feedback", False),
                             ("production_state", {"axis": [1, 0, 0]}), ("drivers", {})):
            invalid = copy.deepcopy(value)
            invalid[key] = changed
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "dirty motion contract"):
                motion.check_plan(invalid)
        with patch.object(motion.shared, "check_plan", side_effect=ValueError("bound inputs")):
            with self.assertRaisesRegex(ValueError, "bound inputs"):
                motion.check_plan(value)


if __name__ == "__main__":
    unittest.main()
