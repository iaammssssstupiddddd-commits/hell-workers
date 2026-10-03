"""Independent native-recipe rejection coverage without starting a process."""

import copy
from argparse import Namespace
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_m2_native_acceptance as recipe


class NativeRecipeTests(unittest.TestCase):
    def setUp(self):
        self.identity = {"kind": "Tank", "generation": 1, "authority": "art_preview",
                         "manifest_sha256": "a" * 64}
        self.value = {"identity": self.identity, "leg": "water"}
        self.nonce = "b" * 32
        self.trace = {"schema_version": 1, "scope": "m2-feedback-observations-only",
                      "leg": "water",
                      "identity": self.identity, "nonce": self.nonce, "process_id": 42,
                      "failure": None, "accepted": False, "performance_evidence": False,
                      "samples": [{"identity": self.identity, "real_seconds": time,
                                   "virtual_seconds": time, "paused": False,
                                   "owners": [], "roots": [], "items": [],
                                   "expected_images": {"catalog": "Some(candidate)"},
                                   "consumers": [{"role": "catalog", "image": "Some(candidate)"}]}
                                  for time in (1.0, 1.2, 1.4)]}
        for sample, state in zip(self.trace["samples"], ("TankEmpty", "TankPartial", "TankFull"), strict=True):
            sample.update(fixture_owner="42v1", owners=[{"entity": "42v1", "pose": {"translation": [1, 2, 3]}}], roots=[{
                "owner": "42v1", "owner_exists": True, "state": f"Some({state})",
                "pose": {"translation": [1, 0, -2]},
                "parts": [{"name": "body"}, {"name": "water"}]}])

    def test_catalog_only_wrong_owner_missing_and_reordered_states_are_rejected(self):
        mutations = [
            lambda rows: [row.update(owners=[], roots=[]) for row in rows],
            lambda rows: rows[1]["roots"][0].update(owner="other"),
            lambda rows: rows[1]["roots"][0].update(state="Some(TankFull)"),
            lambda rows: rows[2]["roots"][0].update(owner_exists=False),
            lambda rows: rows[1].update(fixture_owner=None),
            lambda rows: rows[1]["roots"][0].update(pose={"translation": [9, 0, 9]}),
        ]
        for mutation in mutations:
            trace = copy.deepcopy(self.trace)
            mutation(trace["samples"])
            with self.assertRaises(ValueError):
                recipe.check_trace(trace, self.value, self.nonce, 42)

    def test_active_trace_binding_does_not_mutate_or_approve(self):
        before = copy.deepcopy(self.trace)
        recipe.check_trace(self.trace, self.value, self.nonce, 42)
        self.assertEqual(self.trace, before)
        self.assertFalse(self.trace["accepted"])

    def test_mixer_requires_owned_pause_resume_motion_and_overlap(self):
        value = {"identity": {**self.identity, "kind": "MudMixer"}, "leg": "refining"}
        states = [("MixerIdle", False, 0), ("MixerActive", False, 1),
                  ("MixerActive", True, 1), ("MixerActive", True, 1),
                  ("MixerActive", True, 1), ("MixerActive", False, 2), ("MixerIdle", False, 2)]
        owned = [({"paused": paused, "items": [{"owner": "owner"}]}, {
            "owner": "owner", "state": f"Some({state})", "parts": [{"name": "body"},
            {"name": "rotor", "pose": {"rotation": [0, rotation, 0, 1]}}]})
            for state, paused, rotation in states]
        recipe.check_owned_states(owned, value)
        for mutation in (
            lambda rows: [sample.update(paused=False) for sample, _ in rows],
            lambda rows: [sample.update(items=[]) for sample, _ in rows],
            lambda rows: rows[4][1]["parts"][1]["pose"].update(rotation=[0, 3, 0, 1]),
            lambda rows: [root["parts"][1]["pose"].update(rotation=[0, 0, 0, 1]) for _, root in rows],
            lambda rows: rows.pop(),
        ):
            changed = copy.deepcopy(owned)
            mutation(changed)
            with self.assertRaises(ValueError):
                recipe.check_owned_states(changed, value)

    def test_deconstruction_needs_commit_and_actual_cleanup(self):
        first = self.trace["samples"][0]
        owned = [(first, first["roots"][0])]
        final = {"fixture_owner": "42v1", "fixture_owner_exists": False, "roots": [], "items": [],
                 "lifecycle": {"leg": "deconstruct", "events": [
                     {"operation": "Deconstruct", "result": "Committed", "owner": "42v1"}]}}
        value = {**self.value, "leg": "deconstruct"}
        recipe.check_lifecycle([first, final], owned, value)
        for mutation in (lambda row: row.update(fixture_owner_exists=True),
                         lambda row: row.update(items=[{"owner": "42v1"}]),
                         lambda row: row["lifecycle"].update(events=[])):
            changed = copy.deepcopy(final)
            mutation(changed)
            with self.assertRaises(ValueError):
                recipe.check_lifecycle([first, changed], owned, value)

    def mixer_idle_trace(self, leg):
        trace = copy.deepcopy(self.trace)
        identity = {**self.identity, "kind": "MudMixer"}
        trace.update(identity=identity, leg=leg)
        for sample in trace["samples"]:
            sample.update(identity=identity, fixture_owner_exists=True, world_epoch=1,
                          lifecycle={"leg": leg, "events": []})
            sample["roots"][0].update(state="Some(MixerIdle)", parts=[{"name": "body"}, {"name": "rotor"}])
            sample["consumers"][0]["visible"] = True
        return trace, {"identity": identity, "leg": leg}

    def test_non_refining_mixer_legs_use_idle_states_and_keep_lifecycle_validation(self):
        for leg in recipe.LIFECYCLE_LEGS - {"companion"}:
            with self.subTest(leg=leg):
                trace, value = self.mixer_idle_trace(leg)
                # Lifecycle outcomes have separate behavior coverage; this checks routing
                # never silently skips that verifier when selecting idle state requirements.
                with patch.object(recipe, "check_lifecycle") as lifecycle:
                    recipe.check_trace(trace, value, self.nonce, 42)
                    lifecycle.assert_called_once()
                    self.assertEqual(lifecycle.call_args.args[2], value)
                checkpoints = [{"key": recipe.checkpoint_key(sample)} for sample in trace["samples"]]
                recipe.check_state_checkpoints(checkpoints, value)
                with self.assertRaises(ValueError):
                    recipe.check_state_checkpoints([], value)
                with self.assertRaises(ValueError):
                    recipe.check_state_checkpoints(checkpoints, {**value, "leg": "refining"})
                for sample in trace["samples"]:
                    sample["roots"][0]["state"] = "Some(MixerActive)"
                with patch.object(recipe, "check_lifecycle"), self.assertRaises(ValueError):
                    recipe.check_trace(trace, value, self.nonce, 42)

    def test_idle_mixer_trace_validates_real_catalog_save_load_and_deconstruct_outcomes(self):
        for leg in ("catalog", "save-load", "deconstruct"):
            with self.subTest(leg=leg):
                trace, value = self.mixer_idle_trace(leg)
                final = trace["samples"][-1]
                if leg == "save-load":
                    final.update(fixture_owner="43v1", world_epoch=2)
                    final["owners"][0]["entity"] = "43v1"
                    final["roots"][0]["owner"] = "43v1"
                    final["lifecycle"]["events"] = [{"operation": operation, "result": "Succeeded"}
                                                    for operation in ("Save", "Load")]
                elif leg == "deconstruct":
                    final.update(fixture_owner_exists=False, roots=[], owners=[])
                    final["lifecycle"]["events"] = [{"operation": "Deconstruct", "result": "Committed", "owner": "42v1"}]
                recipe.check_trace(trace, value, self.nonce, 42)
                if leg == "catalog":
                    for sample in trace["samples"]:
                        sample["consumers"][0]["visible"] = False
                else:
                    final["lifecycle"]["events"] = []
                with self.assertRaises(ValueError):
                    recipe.check_trace(trace, value, self.nonce, 42)

    def test_refining_checkpoint_contract_still_requires_active_and_paused_captures(self):
        value = {"identity": {**self.identity, "kind": "MudMixer"}, "leg": "refining"}
        shots = [{"key": [True, 1.0, "Normal", [state], [], paused]}
                 for state, paused in [("Some(MixerIdle)", False), ("Some(MixerActive)", False),
                                       ("Some(MixerActive)", True)]]
        recipe.check_state_checkpoints(shots, value)
        for index in range(len(shots)):
            with self.subTest(missing=index), self.assertRaises(ValueError):
                recipe.check_state_checkpoints(shots[:index] + shots[index + 1:], value)

    def test_save_load_needs_replacement_outcomes_and_same_feet(self):
        first = copy.deepcopy(self.trace["samples"][0])
        first["world_epoch"] = 1
        first["roots"][0]["pose"] = {"translation": [1, 12.8, 3]}
        final = {"fixture_owner": "43v1", "fixture_owner_exists": True, "world_epoch": 2,
                 "roots": [{**first["roots"][0], "owner": "43v1"}],
                 "lifecycle": {"leg": "save-load", "events": [
                     {"operation": operation, "result": "Succeeded"} for operation in ("Save", "Load")]}}
        owned = [(first, first["roots"][0]), (final, final["roots"][0])]
        value = {**self.value, "leg": "save-load"}
        recipe.check_lifecycle([first, final], owned, value)
        for mutation in (lambda row: row.update(world_epoch=1),
                         lambda row: row["roots"][0].update(pose={"translation": [9, 9, 9]}),
                         lambda row: row["lifecycle"].update(events=[])):
            changed = copy.deepcopy(final)
            mutation(changed)
            with self.assertRaises(ValueError):
                recipe.check_lifecycle([first, changed], owned, value)

    def test_hidden_catalog_is_not_visual_evidence(self):
        rows = copy.deepcopy(self.trace["samples"])
        owned = [(row, row["roots"][0]) for row in rows]
        value = {**self.value, "leg": "catalog"}
        with self.assertRaises(ValueError):
            recipe.check_interaction(rows, owned, value)
        rows[-1]["consumers"][0]["visible"] = True
        recipe.check_interaction(rows, owned, value)

    def test_placement_requires_separate_owner_bound_blueprint_and_pulse_hold(self):
        for kind in ("Tank", "MudMixer"):
            with self.subTest(kind=kind):
                rows = copy.deepcopy(self.trace["samples"])
                for index, row in enumerate(rows):
                    row["real_seconds"] = float(index * 2)
                    row["destination_owner"] = "43v1"
                    row["expected_images"]["world"] = "Some(candidate)"
                    row["consumers"] = [{"role": role, "entity": entity, "visible": True,
                                         "image": "Some(candidate)", "blueprint_owner": "43v1",
                                         "blueprint_kind": kind, "blueprint_state": "Building",
                                         "blueprint_progress": 0.25}
                                        for role, entity in (("blueprint", "43v1"), ("pulse", "44v1"),
                                                             ("ghost", "45v1"), ("companion", "46v1"))]
                final = copy.deepcopy(rows[-1])
                final.update(real_seconds=5.0, destination_owner="47v1", destination_anchor=[12, 15], companions=[{}, {}],
                             consumers=[], placement_rejection={"header": "Cannot place"},
                             lifecycle={"pulse_observation_complete": True, "placement_blueprint": "43v1", "events": [
                                 {"operation": "PlacementPress", "target": [-10, -10], "projected": [-10, -10]},
                                 {"operation": "PlacementRejected", "target": [-10, -10]},
                                 {"operation": "PlacementPress", "target": [12, 15], "projected": [12, 15]}]})
                final["roots"].append({"owner": "47v1", "owner_exists": True, "parts": [{}, {}]})
                rows.append(final)
                value = {"leg": "placement", "identity": {**self.identity, "kind": kind}}
                owned = [(rows[0], rows[0]["roots"][0])]
                recipe.check_interaction(rows, owned, value)
                for role_index in (0, 1):
                    for field, replacement in (("visible", False), ("image", "Some(fallback)"),
                                               ("blueprint_owner", "unrelated"), ("blueprint_kind", "Door"),
                                               ("blueprint_state", "ReadyToBuild"), ("blueprint_progress", 1.0),
                                               ("entity", None)):
                        changed = copy.deepcopy(rows)
                        for row in changed[:-1]:
                            row["consumers"][role_index][field] = replacement
                        with self.subTest(role=role_index, field=field), self.assertRaises(ValueError):
                            recipe.check_interaction(changed, owned, value)
                for mutation in (lambda data: data[-1]["lifecycle"].update(pulse_observation_complete=False),
                                 lambda data: data[-1]["lifecycle"]["events"][2].update(projected=[-5, -5]),
                                 lambda data: data[-1].update(destination_anchor=[1, 1]),
                                 lambda data: data[-1]["lifecycle"]["events"].reverse(),
                                 lambda data: data[1]["consumers"].clear(),
                                 lambda data: data[2].update(real_seconds=2.5)):
                    changed = copy.deepcopy(rows)
                    mutation(changed)
                    with self.assertRaises(ValueError):
                        recipe.check_interaction(changed, owned, value)

    def test_common_legs_cannot_pass_on_owned_world_and_catalog_alone(self):
        rows = copy.deepcopy(self.trace["samples"])
        owned = [(row, row["roots"][0]) for row in rows]
        for leg in recipe.LIFECYCLE_LEGS - {"save-load", "deconstruct"}:
            with self.subTest(leg=leg), self.assertRaises(ValueError):
                recipe.check_interaction(rows, owned, {**self.value, "leg": leg})

    def test_wrong_nonce_pid_generation_and_static_clock_are_rejected(self):
        mutations = [
            lambda trace: trace.update(nonce="c" * 32),
            lambda trace: trace.update(leg="placement"),
            lambda trace: trace.update(process_id=43),
            lambda trace: trace.update(identity={**self.identity, "generation": 2}),
            lambda trace: [row.update(paused=True) for row in trace["samples"]],
            lambda trace: [row.update(virtual_seconds=1.0) for row in trace["samples"]],
            lambda trace: trace["samples"][1].update(real_seconds=0.5),
            lambda trace: trace["samples"][1].update(virtual_seconds=float("nan")),
            lambda trace: trace.update(performance_evidence=True),
            lambda trace: trace.update(failure="sample cap exceeded"),
            lambda trace: [row.update(consumers=[]) for row in trace["samples"]],
        ]
        for index, mutation in enumerate(mutations):
            trace = copy.deepcopy(self.trace)
            mutation(trace)
            with self.subTest(index=index), self.assertRaises(ValueError):
                recipe.check_trace(trace, self.value, self.nonce, 42)

    def test_renderer_must_match_selector_on_vulkan_without_warnings(self):
        log = 'AdapterInfo { name: "Exact GPU", backend: Vulkan }'
        recipe.check_renderer(log, "Exact GPU")
        for changed in (log.replace("Vulkan", "Gl"), log.replace("Exact GPU", "Other GPU"),
                        log + "\nWARN renderer fallback", log + "\n" + log):
            with self.subTest(log=changed), self.assertRaises(ValueError):
                recipe.check_renderer(changed, "Exact GPU")

    def test_intel_selector_launch_and_record_preserve_observed_full_name(self):
        name = "Intel(R) Arc(tm) Graphics (MTL)"
        log = f'AdapterInfo {{ name: "{name}", vendor: 32902, backend: Vulkan }}'
        for selector in ("Intel", "iNtEl", name):
            with self.subTest(selector=selector):
                self.assertEqual(recipe.adapter_environment(selector),
                                 {"WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": selector})
                record = recipe.check_renderer(log, selector)
                self.assertEqual(record, {"selector": selector, "name": name, "backend": "Vulkan"})
                recipe.check_renderer_record(log, selector, record)
                for bad in (None, {}, {**record, "name": "Intel"},
                            {**record, "selector": "Other"}, {**record, "backend": "Gl"}):
                    with self.assertRaises(ValueError):
                        recipe.check_renderer_record(log, selector, bad)

    def test_selector_contract_rejects_missing_duplicate_foreign_and_warning_logs(self):
        log = 'AdapterInfo { name: "Intel(R) Arc(tm) Graphics (MTL)", backend: Vulkan }'
        for bad in ("", log.replace("Intel(R)", "AMD"), log.replace("Vulkan", "Gl"),
                    log.replace("Vulkan", "Dx12"), log + "\n" + log,
                    log + '\nAdapterInfo { name: "unparsed" }',
                    log + "\nWARN fallback", log + "\nERROR device", log + "\nbevy_ecs::error::handler"):
            with self.subTest(log=bad), self.assertRaises(ValueError):
                recipe.check_renderer(bad, "Intel")
        for selector in (None, "", "  ", 42):
            with self.subTest(selector=selector), self.assertRaises(ValueError):
                recipe.adapter_environment(selector)
            with self.assertRaises(ValueError):
                recipe.check_renderer(log, selector)

    def test_both_runtime_verification_is_required_by_native_plan(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime_path = root / "runtime.json"
            runtime = {"repo": str(root), "subject": {"source": "d" * 64},
                       "kinds": {"Tank": {"identity": self.identity}}}
            runtime_path.write_bytes(recipe.pipeline.canonical(runtime))
            value = {"status": "ready", "profile": recipe.PROFILE, "scope": recipe.SCOPE, "kind": "Tank", "leg": "water",
                     "accepted": False, "promotion_authority": False, "duration_seconds": 30,
                     "adapter": "Intel", "runtime_plan": str(runtime_path),
                     "runtime_plan_sha256": recipe.pipeline.digest(runtime_path.read_bytes()),
                     "repo": str(root), "job_root": str(root / "job"),
                     "subject": runtime["subject"], "identity": self.identity}
            with patch.object(recipe.m2, "verify", return_value={"subject": runtime["subject"]}) as verify, \
                    patch.object(recipe.art.static, "check_root"):
                self.assertEqual(recipe.check_plan(value), runtime)
                verify.assert_called_once_with(runtime_path)
                for selector in (None, "", "  "):
                    with self.subTest(selector=selector), self.assertRaisesRegex(ValueError, "selector"):
                        recipe.check_plan({**value, "adapter": selector})
                for status in (None, "pass", "blocked", True):
                    changed = {**value, "status": status}
                    if status is None:
                        del changed["status"]
                    with self.subTest(status=status), self.assertRaisesRegex(ValueError, "not ready"):
                        recipe.check_plan(changed)
                for approval in ("accepted", "promotion_authority"):
                    with self.subTest(approval=approval), self.assertRaisesRegex(ValueError, "wrong interactive plan"):
                        recipe.check_plan({**value, approval: True})
                with self.assertRaisesRegex(ValueError, "subject"):
                    recipe.check_plan({**value, "subject": {"source": "e" * 64}})
                with self.assertRaisesRegex(ValueError, "leg"):
                    recipe.check_plan({**value, "duration_seconds": 180})
                for leg in recipe.m2.COMMON_LEGS.keys() - recipe.LIFECYCLE_LEGS:
                    with self.subTest(leg=leg), self.assertRaisesRegex(ValueError, "leg"):
                        recipe.check_plan({**value, "leg": leg})
            runtime_path.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "runtime plan changed"):
                recipe.check_plan(value)

    def test_generated_plan_is_ready_only_after_prerequisites_without_approval(self):
        for kind, leg in recipe.DRIVEN_LEGS.items():
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                identity = {**self.identity, "kind": kind}
                runtime = {"repo": str(root), "subject": {"source": "d" * 64},
                           "kinds": {kind: {"identity": identity}}}
                runtime_path = root / "runtime.json"
                runtime_path.write_bytes(recipe.pipeline.canonical(runtime))
                args = Namespace(runtime_plan=runtime_path, job_root=root / "job",
                                 output=root / "plan.json", kind=kind, leg=leg,
                                 duration_seconds=30, adapter="Intel")
                with patch.object(recipe.m2, "verify", return_value={"subject": runtime["subject"]}), \
                        patch.object(recipe.art.static, "check_root"), \
                        patch.object(recipe.native, "resource_snapshot", return_value={"failures": []}) as resources:
                    result = recipe.plan(args)
                    self.assertEqual(result["status"], "ready")
                    self.assertEqual(result["adapter"], "Intel")
                    self.assertEqual(recipe.adapter_environment(result["adapter"])["WGPU_ADAPTER_NAME"], "Intel")
                    observed = recipe.check_renderer('AdapterInfo { name: "Intel(R) Arc(tm) Graphics (MTL)", backend: Vulkan }',
                                                     result["adapter"])
                    recipe.check_renderer_record('AdapterInfo { name: "Intel(R) Arc(tm) Graphics (MTL)", backend: Vulkan }',
                                                 result["adapter"], observed)
                    self.assertIs(result["accepted"], False)
                    self.assertIs(result["promotion_authority"], False)
                    self.assertEqual(recipe.pipeline.read(args.output), result)
                    self.assertEqual(result["launcher_command"][:4], ["kitty", "--directory", str(root), "--detach"])
                    resources.assert_called_once_with(root, require_launcher=True)
                    args.output = root / "blocked.json"
                    resources.return_value = {"failures": ["launcher unavailable"]}
                    with self.assertRaisesRegex(ValueError, "launcher unavailable"):
                        recipe.plan(args)
                    self.assertFalse(args.output.exists())

    def test_private_fallback_assets_are_revalidated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, view = root / "repo", root / "view"
            for base in (repo, view):
                (base / "assets").mkdir(parents=True)
                (base / "assets/base.png").write_bytes(b"base")
            manifest = {"identity": self.identity, "artifacts": [
                {"path": "candidate.png", "sha256": recipe.pipeline.digest(b"candidate")}]}
            (view / "assets/candidate.png").write_bytes(b"candidate")
            text = "manifest fixture"
            recipe.pipeline.put(view / "assets" / recipe.pipeline.locator("Tank"), text.encode())
            recipe.check_asset_view(repo, view, manifest, text)
            (view / "assets/base.png").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "asset view"):
                recipe.check_asset_view(repo, view, manifest, text)


if __name__ == "__main__":
    unittest.main()
