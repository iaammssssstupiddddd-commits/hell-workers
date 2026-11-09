"""Synthetic supply witnesses, never actual native or release evidence."""
import copy
import unittest
from unittest import mock
from scripts import ui_m2_tank_supply as supply
from scripts import building_production_lifecycle as lifecycle
from scripts.tests import test_building_water_transfers as transfers
from scripts.tests import test_building_water_observations as water
from scripts.tests import test_building_mixer_materials as materials
from scripts.tests import test_building_m2_observations as refine_tests
from scripts.tests import test_ui_m2_tank_completion as construction
from scripts.tests import test_ui_m2_completion as mixer_tests


def fixture():
    samples, events = [], []

    def append(part_samples, part_events):
        offset = len(samples)
        for index, sample in enumerate(part_samples):
            sample.update(sample_index=offset+index, real_seconds=float(offset+index+1),
                          **{key: [] for key in lifecycle.TRANSITION_CATEGORIES})
            samples.append(sample)
        for event in part_events:
            event.update(sample_index=offset+event["sample_index"], event_index=len(events))
            event["real_seconds"] = samples[event["sample_index"]]["real_seconds"]
            events.append(event)

    river, river_samples = water.RiverFillTests.fixture()
    append(river_samples, [river])
    origin, origin_samples, origin_events = water.fixture()
    for event in origin_events:
        event.update(assignment="request", target="request")
    append(origin_samples, origin_events)
    transfer = transfers.fixture()
    append(transfer["samples"], transfer["events"])
    for resource, before, after in (("Sand",[0,0,0],[1,0,0]), ("Rock",[1,0,0],[1,1,0])):
        part = materials.fixture(resource=resource)
        part["events"][0].update(item="input-"+resource, carried_before="input-"+resource,
                                  before=before, after=after)
        append(part["samples"], part["events"])
    refine, refine_samples = refine_tests.fixture()
    refine.update(water="poured-0", before=[1,1,0], after=[0,0,5])
    append(refine_samples, [refine])
    return dict(schema_version=2, transition_contract="indexed-state-delta-v1", samples=samples, events=events)


def input_fixture():
    """A synthetic ordinary-control tail; not a complete campaign envelope."""
    raw, original, client, inputs, _ = mixer_tests.fixture()
    tank_context = construction.fixture()[1][-1]["value"]["m2_planning"]

    def wrap(value):
        nested = value["m2_planning"]
        value["m2_planning"] = dict(copy.deepcopy(tank_context), supply_mixer=nested,
                                  supply_worldgen=worldgen_fixture(),
                                  real_seconds=nested["real_seconds"],virtual_seconds=nested["virtual_seconds"])
        return value

    entries = [None,None,copy.deepcopy(original[0]),copy.deepcopy(original[1]),copy.deepcopy(original[2])]
    for entry in entries[2:]:
        wrap(entry["value"])
    entries[3]["case"],entries[4]["case"] = supply.CHECKPOINTS[3:5]
    for row in inputs:
        action = row.pop("m2_completion_action",None)
        if action:
            old = action["key"]
            action["key"] = "mixer-placement" if old == "placement" else "menu:SetTimeSpeed(Super)" if action["first_step"] == 13 else old
            for value in (action["before"],action["after"]):
                if old != action["key"] and old in value["controls"]:
                    value["controls"][action["key"]] = value["controls"].pop(old)
                wrap(value)
            row["tank_completion_action"] = action
    # The second Normal of the original Mixer-only fixture is this campaign's
    # first Super construction phase.
    for row in inputs:
        if row.get("observed_hover",{}).get("menu:SetTimeSpeed(Normal)") and int(row["step"]) in (13,14):
            row["observed_hover"]["menu:SetTimeSpeed(Super)"] = row["observed_hover"].pop("menu:SetTimeSpeed(Normal)")
    for sample in raw["samples"]:
        if sample["real_seconds"] >= 14:
            sample["relative_speed"] = 4.
    for time,key,paused in ((34,"menu:SetTimeSpeed(Super)",False),(36,"menu:SetTimeSpeed(Paused)",True)):
        before = copy.deepcopy(entries[4]["value"])
        before.update(frame=time*100,paused=not paused,cursor=[400.,400.])
        before["m2_planning"].update(real_seconds=float(time))
        before["controls"][key] = dict(fully_visible=True,interaction="Hovered")
        after = copy.deepcopy(before)
        after.update(frame=time*100+30,paused=paused)
        after["m2_planning"]["real_seconds"] += .3
        first = len(inputs)+1
        index = len(raw["samples"])
        sample = copy.deepcopy(raw["samples"][-1])
        sample.update(sample_index=index,real_seconds=time+.3,paused=paused,relative_speed=4.)
        raw["samples"].append(sample)
        for offset,pressed in enumerate((True,False)):
            inputs.append(dict(step=str(len(inputs)+1),nonce=client["nonce"],pid=client["pid"],window=50,world_epoch=0,
                pre_frame=before["frame"],post_frame=after["frame"],button=1,pressed=pressed,
                observed_hover={key:"Pressed" if pressed else "Hovered"}))
            raw["events"].append(dict(producer="MouseButtonInput",operation="MouseButtonInput",button="Left",window="99v1",
                state="Pressed" if pressed else "Released",sample_index=index,event_index=len(raw["events"]),real_seconds=time+.1*(offset+1)))
        inputs[first-1]["tank_completion_action"] = dict(key=key,first_step=first,last_step=len(inputs),before=before,after=after)
    final = copy.deepcopy(entries[4])
    final.update(case=supply.CHECKPOINTS[5],last_step=len(inputs))
    final["value"]["frame"] = 4000
    final["value"]["m2_planning"]["real_seconds"] = 40.
    entries.append(final)
    return raw,entries,client,inputs


def worldgen_fixture():
    return dict(copy.deepcopy(supply.WORLDGEN), projected_routes=[
        dict(phase=phase, start=[80, 52], trees=15, rocks=12) for phase in range(3)])


class SupplyTests(unittest.TestCase):
    def test_exact_supply_event_subscription_and_rejected_mixing(self):
        raw = dict(scope=supply.SCOPE, event_contract=copy.deepcopy(supply.EVENT_CONTRACT), events=[])
        contract = dict(scope=supply.SCOPE, event_contract=copy.deepcopy(supply.EVENT_CONTRACT))
        supply.check_event_contract(raw, contract)
        for mutate in (
            lambda r, c: r.pop("event_contract"),
            lambda r, c: c.pop("event_contract"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v1"),
            lambda r, c: c.update(scope="ordinary-m2-tank-candidate-supply-v1"),
            lambda r, c: r["event_contract"].update(id="foreign"),
            lambda r, c: c["event_contract"]["excluded_producers"].append("ordinary-tank-take-branch-v1"),
            lambda r, c: r["events"].append(dict(producer="DreamTransferredVisualMessage", operation="other")),
            lambda r, c: r["events"].append(dict(producer="unknown", operation="DreamTransferred")),
        ):
            r, c = copy.deepcopy(raw), copy.deepcopy(contract)
            mutate(r, c)
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                supply.check_event_contract(r, c)

    def test_exact_natural_world_and_missing_or_foreign_prerequisites(self):
        for field, replacement in (("seed", 20260914), ("seed", True), ("attempt", True),
                                   ("attempt", 1), ("fallback", 0), ("fallback", True),
                                   ("layout_sha256", "0" * 64)):
            value = construction.fixture()[1][-1]["value"]
            value["m2_planning"]["supply_worldgen"] = worldgen_fixture()
            supply.check_worldgen(value)
            value["m2_planning"]["supply_worldgen"][field] = replacement
            with self.subTest(field=field, replacement=replacement), self.assertRaises(RuntimeError):
                supply.check_worldgen(value)
        for mutate in (
            lambda w: w.pop("seed"),
            lambda w: w.update(extra="unbound"),
            lambda w: w["projected_routes"].pop(),
            lambda w: w["projected_routes"][1].update(phase=0),
            lambda w: w["projected_routes"][1].update(phase=True),
            lambda w: w["projected_routes"][2].update(trees=0),
            lambda w: w["projected_routes"][0].update(rocks=True),
            lambda w: w["projected_routes"][0].update(start=[True, 52]),
        ):
            value = construction.fixture()[1][-1]["value"]
            observed = worldgen_fixture()
            value["m2_planning"]["supply_worldgen"] = observed
            mutate(observed)
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                supply.check_worldgen(value)

    def test_exact_control_tail_and_missing_reused_or_foreign_inputs(self):
        supply.check_tail_inputs(*input_fixture())
        for mutate in (
            lambda r,e,c,i: i[-1].update(pressed=True),
            lambda r,e,c,i: i[-2]["tank_completion_action"]["after"].update(world_epoch=True),
            lambda r,e,c,i: i[-2]["tank_completion_action"]["before"]["controls"]["menu:SetTimeSpeed(Paused)"].update(fully_visible=False),
            lambda r,e,c,i: r["samples"][-1].update(relative_speed=1),
            lambda r,e,c,i: r["events"][-1].update(window="foreign"),
            lambda r,e,c,i: e[-1].update(last_step=e[-1]["last_step"]-2),
            lambda r,e,c,i: i[-2]["tank_completion_action"].update(first_step=1),
            lambda r,e,c,i: r["samples"][-1].update(cursor_position=[1.,1.]),
        ):
            args = input_fixture()
            mutate(*args)
            with self.subTest(mutate=mutate), self.assertRaises((ValueError,RuntimeError)):
                supply.check_tail_inputs(*args)

    def test_one_selected_exact_leg_and_material_counter_chain(self):
        raw = fixture()
        proof = supply.check_supply_chain(raw, "tank", "mixer", capture_time=20.)
        self.assertEqual(set(proof), {"refine","tank_take","mixer_pour","materials"})
        self.assertEqual(len(proof["materials"]),2)

    def test_missing_foreign_lossy_and_unproven_solids_fail(self):
        for defect in ("no-river","no-take","foreign-tank","foreign-mixer","lossy","route","return",
                       "foreign-refine-water","solid-gap","preseed","duplicate-solid","raw-invalid"):
            raw = fixture()
            by_op = {e["operation"]:e for e in raw["events"]}
            if defect == "no-river":
                raw["events"].remove(by_op["RiverFill"])
            elif defect == "no-take":
                raw["events"].remove(by_op["TankTake"])
            elif defect == "foreign-tank":
                by_op["TankTake"]["owner"] = "other"
            elif defect == "foreign-mixer":
                by_op["MixerPour"]["mixer"] = "other"
            elif defect == "lossy":
                by_op["MixerPour"]["outputs"].pop()
            elif defect == "route":
                by_op["TankTake"]["route_found"] = False
            elif defect == "return":
                by_op["MixerPour"]["returning"] = False
            elif defect == "foreign-refine-water":
                by_op["RefineCommit"]["water"] = "unproven"
            elif defect == "solid-gap":
                by_op["MixerMaterial"].update(before=[2,0,0],after=[2,1,0])
            elif defect == "preseed":
                by_op["RefineCommit"].update(before=[2,1,0],after=[1,0,5])
            elif defect == "duplicate-solid":
                raw["events"].append(copy.deepcopy(by_op["MixerMaterial"]))
            else:
                by_op["RefineCommit"]["world_epoch"] = True
            for index,event in enumerate(raw["events"]):
                event["event_index"] = index
            with self.subTest(defect=defect), self.assertRaises((ValueError, RuntimeError)):
                supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)

    def test_all_raw_branches_still_validated_before_selection(self):
        raw = fixture()
        with mock.patch.object(supply.lifecycle,"check_raw_transitions",side_effect=ValueError("raw failed")) as check:
            with self.assertRaisesRegex(ValueError,"raw failed"):
                supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)
            check.assert_called_once_with(raw)

    def test_future_refinement_cannot_explain_an_earlier_capture(self):
        raw = fixture()
        with self.assertRaisesRegex(RuntimeError,"after its capture"):
            supply.check_supply_chain(raw,"tank","mixer",capture_time=raw["samples"][-1]["real_seconds"]-.1)

    def test_same_frame_post_refine_material_is_not_consumed_before_refine(self):
        raw = fixture()
        event = copy.deepcopy(next(e for e in raw["events"] if e.get("operation") == "MixerMaterial"))
        event.update(event_index=len(raw["events"]),sample_index=len(raw["samples"])-1,
                     real_seconds=raw["samples"][-1]["real_seconds"],before=[0,0,5],after=[1,0,5],
                     item="post-input",carried_before="post-input")
        raw["events"].append(event)
        sample = raw["samples"][-1]
        sample["actors"] = [dict(entity="soul")]
        sample["m2_context"]["mixer_material_channel"] = True
        proof = supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)
        self.assertNotIn(event["event_index"],proof["materials"])

    def test_counter_path_cannot_reorder_material_history(self):
        raw = fixture()
        solids = [e for e in raw["events"] if e.get("operation") == "MixerMaterial"]
        for event,index in zip(solids,(7,6),strict=True):
            event.update(sample_index=index,real_seconds=raw["samples"][index]["real_seconds"])
        lifecycle.check_raw_transitions(raw)
        with self.assertRaisesRegex(RuntimeError,"chronology"):
            supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)
        raw = fixture()
        solids = [e for e in raw["events"] if e.get("operation") == "MixerMaterial"]
        solids[1].update(sample_index=solids[0]["sample_index"],real_seconds=solids[0]["real_seconds"])
        supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)
        raw["events"][solids[0]["event_index"]],raw["events"][solids[1]["event_index"]] = solids[1],solids[0]
        for index,event in enumerate(raw["events"]):
            event["event_index"] = index
        lifecycle.check_raw_transitions(raw)
        with self.assertRaisesRegex(RuntimeError,"chronology"):
            supply.check_supply_chain(raw,"tank","mixer",capture_time=20.)

    def test_mixer_domain_helper_keeps_original_single_completion_guard(self):
        raw, entries, client, _, _ = mixer_tests.fixture()
        mixer_tests.m2.check_completion(raw,entries,client)
        raw["events"].append(dict(operation="BuildingCompleted",kind="Tank",event_index=len(raw["events"])))
        with self.assertRaisesRegex(RuntimeError,"completion domain"):
            mixer_tests.m2.check_completion(raw,entries,client)
        mixer_tests.m2.check_mixer_domain_completion(raw,entries[1]["value"],entries[2]["value"],
            entries[1]["value"]["m2_planning"],entries[2]["value"]["m2_planning"])

    def test_supply_admission_is_explicit_not_inferred_from_raw_scope(self):
        raw, entries, client, inputs, contract = construction.fixture()
        raw["scope"] = supply.SCOPE
        with self.assertRaisesRegex(RuntimeError,"scope"):
            supply.verify(raw,entries,client,inputs,contract)


if __name__ == "__main__":
    unittest.main()
