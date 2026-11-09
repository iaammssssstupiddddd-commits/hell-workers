"""Synthetic Tank inflow mutations; not actual River or native evidence."""
import copy
import unittest
from scripts import ui_m2_tank_inflow as inflow
from scripts.tests.test_ui_m2_tank_completion import fixture as construction_fixture


def fixture():
    raw, entries, client, inputs, contract = construction_fixture()
    for sample in raw["samples"]:
        for tank in sample.get("m2_context", {}).get("tanks", []):
            tank["pose"] = dict(translation=[0.,0.,.12], rotation=[0.,0.,0.,1.], scale=[1.,1.,1.])
    raw["scope"] = contract["scope"] = inflow.SCOPE
    contract["case"] = inflow.CASE
    contract["production_state"] = dict(kind="Tank", partial_y_wu=12., full_y_wu=22.)
    old = raw["samples"][-1]
    entries[-1]["value"]["m2_planning"]["consumers"] = copy.deepcopy(old["art_feedback"])
    entries[-1]["value"]["m2_planning"]["consumers"]["roots"][0]["tank_water_count"] = 0
    # Start a real-task-shaped synthetic Pouring interval, then its commit.
    for time in (34, 36, 40, 44, 47):
        sample = copy.deepcopy(old)
        index = len(raw["samples"])
        sample.update(sample_index=index, real_seconds=time + (.3 if time in (34, 44) else 0),
                      virtual_seconds=26. + min(10., time-34.), paused=time >= 44)
        typed = sample["m2_context"]
        typed.update(water_inventory_schema=1, tank_pour_channel=True, accepted=False,
                     performance_evidence=False, promotion_authority=False, bucket_actors=[])
        sample["art_feedback"]["roots"][0]["tank_water_count"] = 5 if time >= 40 else 0
        if 36 <= time < 40:
            bucket = typed["items"][0]
            bucket.update(resource="BucketWater", stored_in=None, delivering_to="30v1")
            typed["bucket_storages"][0]["stored_items"].remove("bucket-0")
            typed["bucket_actors"] = [dict(entity="worker", bucket="bucket-0", is_soul=True,
                phase="Pouring", carried_item="bucket-0", working_on="water-assignment", source=dict(kind="River"),
                destination=dict(kind="Tank", entity="30v1", exists=True), identity=dict(assignment="water-assignment",
                    target="water-assignment", work_type="GatherWater", matches_working_on=True))]
        if time >= 40:
            typed["items"][0].update(resource="BucketEmpty", stored_in=None, delivering_to=None)
            typed["bucket_storages"][0]["stored_items"].remove("bucket-0")
            typed["tanks"][0]["stored_items"] = [f"water-{n}" for n in range(5)]
            typed["items"] += [dict(entity=f"water-{n}", resource="Water", stored_in="30v1", loaded_in=None) for n in range(5)]
            part = sample["art_feedback"]["roots"][0]["parts"][1]
            part.update(visible=True)
            part["pose"]["translation"] = [0., 12., 0.]
            part["material_values"].update(white=False, base_color_srgba=[.58,.70,.73,1.])
            sample["art_feedback"]["roots"][0]["parts"][0]["material_values"]["base_color_srgba"] = [1.,1.,1.,1.]
        raw["samples"].append(sample)
        if time == 40:
            raw["events"] += [dict(producer="ordinary-tank-pour-branch-v1", operation="TankPour", sample_index=index,
                real_seconds=40., worker="worker", assignment="water-assignment", owner="30v1", bucket="bucket-0",
                world_epoch=0, bucket_before="BucketWater", river_source=True, target="water-assignment", gather_water=True,
                outputs=[f"water-{n}" for n in range(5)]),
                dict(producer="TaskCompletedVisualMessage", operation="TaskCompleted", sample_index=index, real_seconds=40.,
                     worker="worker", assignment="water-assignment", target="water-assignment", work_type="GatherWater")]
        if time in (34,44):
            key = "menu:SetTimeSpeed(Normal)" if time == 34 else "menu:SetTimeSpeed(Paused)"
            before = copy.deepcopy(entries[-1]["value"])
            before.update(frame=time*100, paused=time==34, cursor=[400.,400.])
            before["m2_planning"].update(real_seconds=float(time), virtual_seconds=sample["virtual_seconds"])
            before["controls"][key] = dict(fully_visible=True, interaction="Hovered")
            after = copy.deepcopy(before)
            after.update(frame=time*100+30, paused=time==44)
            after["m2_planning"]["real_seconds"] = time+.3
            first = len(inputs)+1
            for offset,pressed in enumerate((True,False)):
                inputs.append(dict(step=str(len(inputs)+1), nonce=client["nonce"], pid=client["pid"], window=client["window"],
                    world_epoch=0, pre_frame=before["frame"], post_frame=after["frame"], pressed=pressed, button=1,
                    observed_hover={key:"Pressed" if pressed else "Hovered"}))
                raw["events"].append(dict(producer="MouseButtonInput", operation="MouseButtonInput", state="Pressed" if pressed else "Released",
                    button="Left", window="99v1", sample_index=index, real_seconds=time+.1*(offset+1)))
            inputs[first-1]["tank_completion_action"] = dict(key=key, first_step=first, last_step=len(inputs), before=before, after=after)
    final = copy.deepcopy(entries[-1]["value"])
    final.update(frame=4700, paused=True)
    final["m2_planning"].update(real_seconds=47., virtual_seconds=36., consumers=copy.deepcopy(raw["samples"][-1]["art_feedback"]))
    entries.append(dict(case=inflow.CHECKPOINTS[-1], value=final, last_step=len(inputs)))
    for index,event in enumerate(raw["events"]):
        event["event_index"] = index
    return raw, entries, client, inputs, contract


class TankInflowTests(unittest.TestCase):
    def test_exact_synthetic_construction_then_pour(self):
        inflow.verify(*fixture())

    def test_missing_receipt_assignment_inventory_presentation_and_scope_rejected(self):
        for mutate in (
            lambda r,e,c,i,a: r.update(scope=inflow.construction.SCOPE),
            lambda r,e,c,i,a: a.update(case=inflow.construction.CASE),
            lambda r,e,c,i,a: r["events"].__setitem__(slice(None),[x for x in r["events"] if x.get("operation")!="TankPour"]),
            lambda r,e,c,i,a: next(s for s in r["samples"] if s["real_seconds"]==36)["m2_context"]["bucket_actors"][0]["identity"].update(assignment="foreign"),
            lambda r,e,c,i,a: r["samples"][-1]["m2_context"]["tanks"][0]["stored_items"].pop(),
            lambda r,e,c,i,a: r["samples"][-1]["m2_context"]["items"].pop(),
            lambda r,e,c,i,a: r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1].update(visible=False),
            lambda r,e,c,i,a: r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1]["pose"].update(translation=[0.,22.,0.]),
            lambda r,e,c,i,a: r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1]["material_values"].update(base_color_srgba=[1.,1.,1.,1.]),
            lambda r,e,c,i,a: e[-1]["value"]["m2_planning"]["consumers"]["roots"][0].update(tank_water_count=True),
            lambda r,e,c,i,a: r["samples"][-1].update(world_epoch=1),
            lambda r,e,c,i,a: r["samples"][-1].update(world_epoch=False),
            lambda r,e,c,i,a: r["samples"][-1].update(real_seconds=float("nan")),
            lambda r,e,c,i,a: r["samples"][-1].update(virtual_seconds=True),
            lambda r,e,c,i,a: r["samples"][-2].update(relative_speed=True),
            lambda r,e,c,i,a: e[-1]["value"].update(world_epoch=False),
            lambda r,e,c,i,a: e[-1]["value"].update(frame=4700.),
            lambda r,e,c,i,a: e[-1].update(last_step=float(e[-1]["last_step"])),
            lambda r,e,c,i,a: i[-2]["tank_completion_action"]["before"].update(frame=4400.),
            lambda r,e,c,i,a: r["samples"][-1]["owners"].clear(),
            lambda r,e,c,i,a: r["samples"][-1]["m2_context"]["tanks"][0].update(stored_items=False),
            lambda r,e,c,i,a: r["samples"][-1]["m2_context"]["tanks"][0]["pose"].update(translation=[32.,0.,.12]),
            lambda r,e,c,i,a: i[-2]["tank_completion_action"].update(key="menu:SetTimeSpeed(Super)"),
        ):
            changed=fixture()
            mutate(*changed)
            with self.subTest(mutate=mutate), self.assertRaises((ValueError,RuntimeError)):
                inflow.verify(*changed)

    def test_completion_default_still_rejects_inflow_scope(self):
        raw,entries,client,inputs,contract=fixture()
        with self.assertRaises((ValueError,RuntimeError)):
            inflow.construction.verify(raw,entries[:3],client,inputs[:entries[2]["last_step"]],contract)

    def test_empty_stored_items_type_aliases_still_rejected(self):
        for value in (False,0,""):
            raw,entries,client,inputs,contract=construction_fixture()
            raw["samples"][-1]["m2_context"]["tanks"][0]["stored_items"]=value
            with self.subTest(value=value), self.assertRaises((ValueError,RuntimeError)):
                inflow.construction.verify(raw,entries,client,inputs,contract)
