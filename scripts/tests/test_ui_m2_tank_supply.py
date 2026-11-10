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
    # Extra ordinary Full-setup controls precede the old Mixer-only tail.
    for row in inputs:
        row["step"] = str(int(row["step"]) + 4)
        if "tank_completion_action" in row:
            for key in ("first_step", "last_step"):
                row["tank_completion_action"][key] += 4
    for entry in entries[3:]:
        entry["last_step"] += 4
    for sample in raw["samples"][1:]:
        sample["sample_index"] += 2
    for event in raw["events"]:
        event["sample_index"] += 2 if event["sample_index"] else 0
    prefix_inputs, prefix_events, prefix_samples = [], [], []
    for index, (time, key, paused) in enumerate(((1.2, "menu:SetTimeSpeed(Super)", False),
                                               (1.6, "menu:SetTimeSpeed(Paused)", True))):
        before = copy.deepcopy(entries[2]["value"])
        before.update(frame=round(time*100), paused=not paused, cursor=[400.,400.])
        before["m2_planning"]["real_seconds"] = time
        before["controls"][key] = dict(fully_visible=True, interaction="Hovered")
        after = copy.deepcopy(before)
        after.update(frame=before["frame"]+20, paused=paused)
        after["m2_planning"]["real_seconds"] += .2
        sample = copy.deepcopy(raw["samples"][0])
        sample.update(sample_index=index+1, real_seconds=time+.2, paused=paused, relative_speed=4.)
        prefix_samples.append(sample)
        first = len(prefix_inputs)+1
        for offset, pressed in enumerate((True,False)):
            prefix_inputs.append(dict(step=str(len(prefix_inputs)+1), nonce=client["nonce"], pid=client["pid"],
                window=50, world_epoch=0, pre_frame=before["frame"], post_frame=after["frame"],
                button=1, pressed=pressed, observed_hover={key:"Pressed" if pressed else "Hovered"}))
            prefix_events.append(dict(producer="MouseButtonInput", operation="MouseButtonInput", button="Left",
                window="99v1", state="Pressed" if pressed else "Released", sample_index=index+1,
                real_seconds=time+.05*(offset+1)))
        prefix_inputs[first-1]["tank_completion_action"] = dict(key=key, first_step=first,
            last_step=len(prefix_inputs), before=before, after=after)
    inputs[:0] = prefix_inputs
    raw["events"][:0] = prefix_events
    raw["samples"][1:1] = prefix_samples
    for index,event in enumerate(raw["events"]):
        event["event_index"] = index
    from scripts.tests.test_ui_m2_supply_workload import prepend_workload
    return prepend_workload(raw,entries,client,inputs)


def worldgen_fixture():
    return dict(copy.deepcopy(supply.WORLDGEN), projected_routes=[
        dict(phase=phase, start=[80, 52], trees=15, rocks=12) for phase in range(3)])


class SupplyTests(unittest.TestCase):
    @staticmethod
    def full_fixture():
        ctx = dict(tanks=[dict(entity="tank", provisional=False, stockpile=dict(resource="Water",capacity=50),
                              stored_items=[f"water-{i}" for i in range(50)])],
                   items=[dict(entity=f"water-{i}", resource="Water", stored_in="tank", loaded_in=None) for i in range(50)],
                   bucket_storages=[dict(entity="storage", belongs_to="tank", pending_blueprint=None,
                                         stockpile=dict(resource=None,capacity=10),
                                         stored_items=[f"bucket-{i}" for i in range(5)]),
                                    dict(entity="second", belongs_to="tank", pending_blueprint=None,
                                         stockpile=dict(resource=None,capacity=10),stored_items=None)], inventory_holders=[])
        ctx["bucket_actors"] = []
        ctx["items"] += [dict(entity=f"bucket-{i}", resource="BucketEmpty", belongs_to="tank", stored_in="storage",
                               loaded_in=None, delivering_to=None, stored_by_mixer=None) for i in range(5)]
        return dict(m2_context=ctx)

    def test_full_requires_five_returned_unheld_actual_buckets(self):
        children = ["storage", "second"]
        self.assertTrue(supply.full_setup_ready(self.full_fixture(), "tank", children))
        sample = self.full_fixture()
        sample["m2_context"]["items"][-1]["resource"] = "BucketWater"
        self.assertTrue(supply.full_setup_ready(sample,"tank",children))
        for mutate in (
            lambda c: c["tanks"][0]["stored_items"].pop(),
            lambda c: c["items"][-1].update(resource="Water"),
            lambda c: c["items"][-1].update(loaded_in="cart"),
            lambda c: c["items"][-1].update(delivering_to="tank"),
            lambda c: c["items"][-1].pop("loaded_in"),
            lambda c: c["inventory_holders"].append(dict(entity="ordinary-hauler",carried_item="bucket-4")),
        ):
            sample = self.full_fixture()
            mutate(sample["m2_context"])
            with self.subTest(mutate=mutate):
                self.assertFalse(supply.full_setup_ready(sample,"tank", children))
        sample = self.full_fixture()
        sample["m2_context"].pop("inventory_holders")
        with self.assertRaisesRegex(RuntimeError,"holders"):
            supply.full_setup_ready(sample,"tank", children)
        for mutate in (
            lambda c: c["items"][-1].update(stored_in="foreign"),
            lambda c: c["bucket_storages"][0]["stored_items"].pop(),
            lambda c: c["bucket_storages"][0].update(belongs_to="foreign"),
            lambda c: c["bucket_storages"].pop(),
            lambda c: c["bucket_storages"][1].update(entity="other-same-owner-storage"),
            lambda c: c["bucket_storages"][1].pop("pending_blueprint"),
            lambda c: c["bucket_storages"][1]["stockpile"].update(capacity=True),
            lambda c: c["bucket_storages"][1].update(stored_items=["foreign-item"]),
        ):
            sample = self.full_fixture()
            mutate(sample["m2_context"])
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                supply.full_setup_ready(sample,"tank",children)

    def test_full_setup_is_current_continuous_paused_not_historical(self):
        raw,entries,client,inputs = input_fixture()
        actions = supply.check_tail_inputs(raw,entries,client,inputs)
        for sample in raw["samples"]:
            sample.update(self.full_fixture())
        supply.check_full_setup(raw,actions[len(supply.workload.KEYS):],"tank",["storage","second"])
        raw["samples"][13]["m2_context"]["items"][-1]["resource"] = "Water"
        with self.assertRaisesRegex(RuntimeError,"not held"):
            supply.check_full_setup(raw,actions[len(supply.workload.KEYS):],"tank",["storage","second"])

    def test_motion_budget_is_supply_only_and_bounded(self):
        client = dict(nonce="b"*32,pid=40,window=50)
        def moves(count):
            return [dict(step=str(i+1),nonce=client["nonce"],pid=40,window=50,world_epoch=0,
                         pre_frame=2*i+1,post_frame=2*i+2,point=[1,1],observed_cursor=[1.,1.]) for i in range(count)]
        supply.planning.check_transport(moves(16),client,0)
        with self.assertRaises(RuntimeError):
            supply.planning.check_transport(moves(17),client,0)
        supply.planning.check_transport(moves(30),client,0,motion_cap=supply.MOTION_CAP)
        with self.assertRaises(RuntimeError):
            supply.planning.check_transport(moves(31),client,0,motion_cap=supply.MOTION_CAP)
        for cap in (True,0,161):
            with self.assertRaises(RuntimeError):
                supply.planning.check_transport(moves(1),client,0,motion_cap=cap)

    def test_actual_full_surplus_is_bound_to_finite_output_demand(self):
        self.assertEqual([supply.workload.required_export(n) for n in range(6)], [0,20,45,70,95,120])
        for count in range(6):
            sample = self.full_fixture()
            sample["paused"] = True
            buckets = [r for r in sample["m2_context"]["items"] if r.get("belongs_to") == "tank"]
            for row in buckets[:count]:
                row["resource"] = "BucketWater"
            if count <= 2:
                self.assertEqual(supply.check_output_budget([sample],"tank"),count)
            else:
                with self.assertRaisesRegex(RuntimeError,"insufficient"):
                    supply.check_output_budget([sample],"tank")
        for mutate in (
            lambda s:s["m2_context"]["bucket_actors"].append(dict(phase="Filling")),
            lambda s:s.update(paused=False),
            lambda s:s["m2_context"]["items"][-1].update(resource="BucketWater"),
        ):
            before = self.full_fixture()
            before["paused"] = True
            after = copy.deepcopy(before)
            mutate(after)
            with self.subTest(mutate=mutate),self.assertRaises(RuntimeError):
                supply.check_output_budget([before,after],"tank")
        for bad in (True,-1,6,1.0):
            with self.assertRaises(RuntimeError):
                supply.workload.required_export(bad)

    def test_exact_supply_event_subscription_and_rejected_mixing(self):
        raw = dict(scope=supply.SCOPE, event_contract=copy.deepcopy(supply.EVENT_CONTRACT), events=[])
        contract = dict(scope=supply.SCOPE, event_contract=copy.deepcopy(supply.EVENT_CONTRACT))
        supply.check_event_contract(raw, contract)
        for mutate in (
            lambda r, c: r.pop("event_contract"),
            lambda r, c: c.pop("event_contract"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v1"),
            lambda r, c: c.update(scope="ordinary-m2-tank-candidate-supply-v1"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v2"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v3"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v4"),
            lambda r, c: r.update(scope="ordinary-m2-tank-candidate-supply-v5"),
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
        self.assertEqual(set(proof), {"refine","tank_take","mixer_pour","materials","prefix"})
        self.assertEqual(len(proof["materials"]),2)

    @staticmethod
    def prefix_fixture():
        events = []
        def add(op, **fields):
            index = len(events)
            events.append(dict(operation=op, event_index=index, sample_index=index+1,
                               real_seconds=float(index+1), owner="mixer" if op in
                               ("MixerMaterial","RefineCommit") else "tank", **fields))
        shared = dict(worker="worker",assignment="request",target="request",world_epoch=0)
        add("RiverFill",bucket="fill",**shared)
        add("TankPour",bucket="fill",outputs=[f"stored-{i}" for i in range(5)],**shared)
        add("RiverFill",bucket="direct",**shared)
        add("MixerPour",bucket="direct",mixer="mixer",returning=True,amount=5,
            outputs=[f"direct-{i}" for i in range(5)],**shared)
        add("MixerMaterial",resource="Sand",before=[0,0,0],after=[1,0,0])
        add("MixerMaterial",resource="Rock",before=[1,0,0],after=[1,1,0])
        add("RefineCommit",water="direct-0",before=[1,1,0],after=[0,0,5])
        add("TankTake",bucket="empty",mixer="mixer",route_found=True,inputs=[f"stored-{i}" for i in range(5)],**shared)
        add("MixerPour",bucket="empty",mixer="mixer",returning=True,amount=5,
            outputs=[f"taken-{i}" for i in range(5)],**shared)
        # Actual mud haul-out is intentionally not inferred by this S/R ledger.
        add("MixerMaterial",resource="Sand",before=[0,0,0],after=[1,0,0])
        add("MixerMaterial",resource="Rock",before=[1,0,0],after=[1,1,0])
        add("RefineCommit",water="taken-0",before=[1,1,0],after=[0,0,5])
        return events

    def test_complete_prefix_keeps_direct_reuse_and_later_tank_origin(self):
        events = self.prefix_fixture()
        origins, classified = supply.check_complete_prefix(events,"tank","mixer")
        self.assertEqual(origins["direct-0"]["kind"],"RiverFill")
        self.assertEqual(origins["taken-0"]["kind"],"TankTake")
        self.assertEqual([e["origin"] for e in classified if e["operation"]=="RefineCommit"],
                         ["RiverFill","TankTake"])
        for mutate in (
            lambda e: e.pop(0),
            lambda e: e[3].update(amount=4),
            lambda e: e[6].update(water="unknown"),
            lambda e: e[11].update(water="direct-0"),
            lambda e: e[7].update(bucket="direct"),
            lambda e: e[7].update(inputs=["unproven"]),
            lambda e: e[8].update(worker="foreign"),
            lambda e: e[9].update(before=[1,0,0]),
            lambda e: e[11].update(before=[2,1,0]),
            lambda e: e[6].update(owner="foreign"),
        ):
            rows = self.prefix_fixture()
            mutate(rows)
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                supply.check_complete_prefix(rows,"tank","mixer")

    def test_old_filled_bucket_cycle_cannot_be_reused_after_tank_pour(self):
        events = self.prefix_fixture()
        events.insert(3,dict(operation="TankPour",event_index=99,sample_index=3,real_seconds=3.5,
                            owner="tank",bucket="direct",worker="worker",assignment="request",target="request",world_epoch=0,
                            outputs=["extra-water"]))
        with self.assertRaisesRegex(RuntimeError,"predecessor missing or reused"):
            supply.check_complete_prefix(events,"tank","mixer")

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
        # The actual observer drains all material receipts BEFORE Refine rows,
        # even when this branch executed after Refine in the same frame.
        raw["events"].remove(event)
        position = next(i for i,e in enumerate(raw["events"]) if e.get("operation")=="RefineCommit")
        raw["events"].insert(position,event)
        for i,row in enumerate(raw["events"]):
            row["event_index"] = i
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
