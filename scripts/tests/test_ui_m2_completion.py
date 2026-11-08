"""Synthetic campaign fixtures; never actual native/art/release evidence."""
import copy
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from scripts import ui_m2_completion as m2
from scripts.tests import test_ui_m2_planning as planning_tests


def fixture():
    def virtual(time):
        return max(0., min(time - 2, 10.)) + max(0., min(time - 14, 16.))
    contract = planning_tests.contract()
    contract.update(case=m2.CASE, scope=m2.SCOPE, parts=[dict(name=name, mesh_role=name, material_role="opaque_albedo",
        translation_wu=[0., 0., 0.], rotation_xyzw=[0., 0., 0., 1.], scale=[1., 1., 1.]) for name in ("body", "rotor")])
    values = [planning_tests.observation(contract, phase, time) for phase, time in (("m2-empty", 1), ("m2-placed", 13), ("m2-selected", 33))]
    for value in values:
        value["paused"] = True
        value["m2_planning"]["virtual_seconds"] = virtual(value["m2_planning"]["real_seconds"])
    completed = values[-1]["m2_planning"]
    completed.update(blueprints=[], buildings=[30], building_inventory=[dict(entity=30)])
    for cell in completed["cells"]:
        cell.update(owner=30, walkable=False)
    entries, inputs, samples, events = [], [], [], []
    checkpoints = {1: 0, 13: 1, 33: 2}
    actions = {2: "menu:SetTimeSpeed(Normal)", 4: "menu:ToggleArchitect", 6: "menu:SelectBuild(MudMixer)",
               8: "placement", 10: "Escape", 12: "menu:SetTimeSpeed(Paused)",
               14: "menu:SetTimeSpeed(Normal)", 30: "menu:SetTimeSpeed(Paused)"}
    footprint = values[0]["m2_planning"]["footprint"]
    for time in sorted({*checkpoints, *actions, 20, 21, 22, 23, 24, 25}):
        index = len(samples)
        owner = time >= 25
        placed = time >= 8 and not owner
        count = min(4, max(0, time - 19))
        real = time + (.3 if time in actions else 0)
        sample = dict(sample_index=index, real_seconds=real, virtual_seconds=virtual(real), world_epoch=0,
                      paused=time < 2 or 12 <= time < 14 or time >= 30, relative_speed=1.,
                      primary_window="99v1", primary_window_bits=99, cursor_position=[400., 400.], dpi=1.,
                      owners=[dict(entity="30v1", entity_bits=30, kind="MudMixer", provisional=False)] if owner else [],
                      blueprints=[dict(entity="20v1", entity_bits=20, kind="MudMixer", footprint=footprint,
                                       materials_complete=count == 4, delivered_materials={"Wood": count}, progress=.1)] if placed else [],
                      roots=[], items=[], move_tasks=[], generations=[],
                      actors=[dict(entity="worker", task_identity=dict(assignment="assignment", target="20v1", work_type="Build"))],
                      owner_layers=[])
        if time >= 8:
            sample["owner_layers"].append(dict(entity="20v1", exists=not owner, building_grids=[] if owner else footprint,
                                              floor_grids=[], door_grids=[], bridge_grids=[], stockpile_grids=[]))
        if owner:
            sample["owner_layers"].append(dict(entity="30v1", exists=True, building_grids=footprint,
                                              floor_grids=[], door_grids=[], bridge_grids=[], stockpile_grids=[]))
            sample["art_feedback"] = dict(requested=contract["identity"], active=contract["identity"], pending=None,
                descriptor=dict(identity=contract["identity"], parts=contract["parts"], meshes=dict(body="body-mesh", rotor="rotor-mesh"), albedo="albedo"),
                roots=[dict(entity="root", owner="30v1", identity=contract["identity"], owner_pose_matches=True,
                    parts=[dict(name=name, mesh=name + "-mesh", albedo="albedo", material="material", parent="root", visible=True,
                                pose=dict(translation=[0., 0., 0.], rotation=[0., 0., 0., 1.], scale=[1., 1., 1.]),
                                material_values=dict(white=True, opaque=True, lit=True, emissive_black=True, emissive_texture_absent=True,
                                                     roughness=1., reflectance=0.)) for name in ("body", "rotor")])])
        samples.append(sample)
        if time in checkpoints:
            slot = checkpoints[time]
            entries.append(dict(case=m2.CHECKPOINTS[slot], value=values[slot], last_step=len(inputs)))
        if time in actions:
            key = actions[time]
            value = copy.deepcopy(values[2 if owner else 1 if placed else 0])
            ctx = value["m2_planning"]
            ctx.update(real_seconds=time, virtual_seconds=virtual(time))
            value.update(frame=time * 100, cursor=[400., 400.])
            if key == "placement":
                value = planning_tests.observation(contract, "m2-legal", time)
            value["controls"][key] = dict(fully_visible=True, interaction="Hovered")
            value["paused"] = time <= 2 or 12 < time <= 14 or time > 30
            after = copy.deepcopy(value)
            after["m2_planning"]["real_seconds"] = time + .3
            after["m2_planning"]["virtual_seconds"] = virtual(time + .3)
            after["paused"] = sample["paused"]
            after["frame"] += 30
            first = len(inputs) + 1
            for offset, pressed in enumerate((True, False)):
                inputs.append(dict(step=str(len(inputs) + 1), nonce="b" * 32, pid=40, window=50, world_epoch=0,
                    pre_frame=value["frame"], post_frame=after["frame"], pressed=pressed,
                    observed_hover={key: "Pressed" if pressed else "Hovered"}, **({"key": "Escape"} if key == "Escape" else {"button": 1})))
                events.append(dict(producer="KeyboardInput" if key == "Escape" else "MouseButtonInput", sample_index=index,
                    real_seconds=time + .1 * (offset + 1), window="99v1", state="Pressed" if pressed else "Released",
                    **({"key": "Escape", "repeat": False, "operation": "KeyboardInput"} if key == "Escape" else {"button": "Left", "operation": "MouseButtonInput"})))
            inputs[first - 1]["m2_completion_action"] = dict(key=key, first_step=first, last_step=len(inputs), before=value, after=after)
            if key == "placement":
                events.append(dict(producer="ordinary-building-placement-branch-v1", operation="PlacementOutcome", sample_index=index,
                    real_seconds=time + .15, window="99v1", world_epoch=0,
                    outcome=dict(kind="MudMixer", grid=[5, 5], status="BlueprintCommitted", owner="20v1", footprint=footprint)))
        if 20 <= time <= 23:
            events.append(dict(producer="ordinary-blueprint-delivery-branch-v1", operation="MaterialDelivery", result="Committed",
                sample_index=index, real_seconds=time, worker="worker", assignment="assignment", owner="20v1", item=f"wood-{count}",
                world_epoch=0, kind="MudMixer", resource="Wood", before_count=count - 1, after_count=count, footprint=footprint, wheelbarrow=False))
        if time == 25:
            events.extend([dict(producer="hw_jobs::publish_building_completed", operation="BuildingCompleted", sample_index=index,
                real_seconds=time, blueprint_owner="20v1", owner="30v1", kind="MudMixer", footprint=footprint),
                dict(producer="TaskCompletedVisualMessage", operation="TaskCompleted", work_type="Build", sample_index=index,
                     real_seconds=time, worker="worker", assignment="assignment", target="20v1")])
    # Synthesize test-only indexed raw deltas, not native observations.
    from scripts import building_production_lifecycle as lifecycle
    for index in range(1, len(samples)):
        for category, key in lifecycle.TRANSITION_CATEGORIES.items():
            before, after = ({row[key]: row for row in sample[category]} for sample in samples[index - 1:index + 1])
            for entity in sorted(before.keys() | after.keys()):
                if before.get(entity) != after.get(entity):
                    events.append(dict(producer="production_snapshot_delta", operation="StateTransition", sample_index=index,
                        before_sample_index=index - 1, real_seconds=samples[index]["real_seconds"], category=category,
                        entity=entity, before=before.get(entity), after=after.get(entity)))
    # Keep the delta subsequence canonical while domain input/receipt order stays intact.
    for index, event in enumerate(events):
        event["event_index"] = index
    raw = dict(scope=m2.SCOPE, schema_version=2, transition_contract="indexed-state-delta-v1", failure=None,
               nonce="b" * 32, process_id=40, accepted=False, performance_evidence=False, promotion_authority=False,
               samples=samples, events=events)
    return raw, entries, dict(nonce="b" * 32, pid=40, window=50), inputs, contract


class MixerCompletionTests(unittest.TestCase):
    def test_exact_synthetic_campaign(self):
        m2.verify(*fixture())

    def test_missing_foreign_or_inferred_evidence_rejected(self):
        for mutate in (
            lambda r, e, c, i, a: r.update(scope="ordinary-m2-candidate-planning-v1"),
            lambda r, e, c, i, a: r.update(accepted=True),
            lambda r, e, c, i, a: e[0]["value"]["m2_planning"].update(buildings=[30]),
            lambda r, e, c, i, a: r["events"].pop(),
            lambda r, e, c, i, a: i[0]["m2_completion_action"].update(key="F5"),
            lambda r, e, c, i, a: r["samples"][-1]["art_feedback"]["roots"][0]["parts"][0].update(mesh="fallback"),
            lambda r, e, c, i, a: r["samples"][-1]["art_feedback"]["roots"][0].update(owner="foreign"),
            lambda r, e, c, i, a: next(x for x in r["events"] if x["operation"] == "BuildingCompleted").update(owner="20v1"),
            lambda r, e, c, i, a: r["samples"][8].update(virtual_seconds=float("nan")),
            lambda r, e, c, i, a: next(x for x in r["events"] if x["operation"] == "PlacementOutcome").update(real_seconds=8.01),
            lambda r, e, c, i, a: e[-1]["value"].update(primary_window=100),
            lambda r, e, c, i, a: e[-1]["value"]["m2_planning"].update(primary_window=100),
            lambda r, e, c, i, a: e[-1]["value"]["m2_planning"].update(real_seconds=30.),
            lambda r, e, c, i, a: r["samples"][8].update(relative_speed=2.),
            lambda r, e, c, i, a: r["samples"][8].update(paused=True),
        ):
            changed = fixture()
            mutate(*changed)
            with self.subTest(mutate=mutate), self.assertRaises((RuntimeError, ValueError)):
                m2.verify(*changed)

    def test_zero_simulation_time_cannot_prove_natural_construction(self):
        raw, entries, client, inputs, contract = fixture()
        for sample in raw["samples"]:
            sample["virtual_seconds"] = 0.
        for entry in entries:
            entry["value"]["m2_planning"]["virtual_seconds"] = 0.
        with self.assertRaisesRegex(RuntimeError, "simulation advance"):
            m2.verify(raw, entries, client, inputs, contract)

    def test_foreign_checkpoint_window_cannot_be_hidden_inside_matching_context(self):
        raw, entries, client, inputs, contract = fixture()
        entries[-1]["value"]["primary_window"] = 100
        entries[-1]["value"]["m2_planning"]["primary_window"] = 100
        with self.assertRaisesRegex(RuntimeError, "checkpoint owner"):
            m2.verify(raw, entries, client, inputs, contract)

    def test_intermediate_map_change_is_rejected_even_with_consistent_deltas(self):
        from scripts import building_production_lifecycle as lifecycle
        raw, entries, client, inputs, contract = fixture()
        sample = next(row for row in raw["samples"] if row["real_seconds"] == 24)
        sample["owner_layers"][0]["building_grids"] = []
        raw["events"] = [event for event in raw["events"] if event["producer"] != "production_snapshot_delta"]
        for index in range(1, len(raw["samples"])):
            for category, key in lifecycle.TRANSITION_CATEGORIES.items():
                before, after = ({row[key]: row for row in sample[category]} for sample in raw["samples"][index - 1:index + 1])
                for entity in sorted(before.keys() | after.keys()):
                    if before.get(entity) != after.get(entity):
                        raw["events"].append(dict(producer="production_snapshot_delta", operation="StateTransition", sample_index=index,
                            before_sample_index=index - 1, real_seconds=raw["samples"][index]["real_seconds"], category=category,
                            entity=entity, before=before.get(entity), after=after.get(entity)))
        for index, event in enumerate(raw["events"]):
            event["event_index"] = index
        lifecycle.check_raw_transitions(raw)
        with self.assertRaisesRegex(RuntimeError, "intermediate reservation"):
            m2.verify(raw, entries, client, inputs, contract)

    def test_wrong_case_and_mixed_sources_cannot_plan(self):
        args = SimpleNamespace(case=m2.CASE, smoke=True, input_backend="portal-eis", m2_art_source="candidate", art_codec="codec",
                               layout_menu=False, m5_art_source=None)
        with patch.object(m2, "source_contract", return_value={}):
            self.assertEqual(m2.planned_contract(args, None), {})
            for key, value in (("case", "m2-mixer-candidate-planning"), ("input_backend", "xtest"), ("m5_art_source", "foreign")):
                changed = copy.copy(args)
                setattr(changed, key, value)
                with self.subTest(key=key), self.assertRaises(RuntimeError):
                    m2.planned_contract(changed, None)
