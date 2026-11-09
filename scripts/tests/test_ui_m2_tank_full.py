"""Synthetic capacity campaign only; never native or River evidence."""
import copy
import unittest
from scripts import ui_m2_tank_full as full
from scripts.tests.test_ui_m2_tank_inflow import fixture as partial_fixture


def fixture():
    raw,entries,client,inputs,contract=partial_fixture()
    raw["scope"]=contract["scope"]=full.SCOPE
    contract["case"]=full.CASE
    entries[-1]["case"]=full.CHECKPOINTS[-1]
    original=raw["samples"]
    base=copy.deepcopy(next(s for s in original if s["real_seconds"]==40))
    actor=copy.deepcopy(next(s for s in original if s["real_seconds"]==36)["m2_context"]["bucket_actors"][0])
    actor["phase"]="GoingToDestination"
    produced=[f"water-{n}" for n in range(5)]
    extra=[]
    extra_events=[]
    for trip in range(1,10):
        prior=copy.deepcopy(base)
        prior.update(real_seconds=40.+trip*.2-.1, virtual_seconds=32.+trip*.2-.1)
        prior["m2_context"]["items"][0].update(resource="BucketWater",delivering_to="30v1")
        prior["m2_context"]["bucket_actors"]=[copy.deepcopy(actor)]
        extra.append(prior)
        outputs=[f"water-{trip*5+n}" for n in range(5)]
        produced+=outputs
        base=copy.deepcopy(base)
        base.update(real_seconds=40.+trip*.2,virtual_seconds=32.+trip*.2)
        base["m2_context"]["items"] += [dict(entity=entity,resource="Water",stored_in="30v1",loaded_in=None) for entity in outputs]
        base["m2_context"]["tanks"][0]["stored_items"]=list(produced)
        root=base["art_feedback"]["roots"][0]
        root["tank_water_count"]=len(produced)
        if trip==9:
            part=root["parts"][1]
            part["pose"]["translation"]=[0.,22.,0.]
            part["material_values"].update(white=True,base_color_srgba=[1.,1.,1.,1.])
        extra.append(base)
        extra_events.extend([
            dict(producer="ordinary-tank-pour-branch-v1",operation="TankPour",real_seconds=base["real_seconds"],
                 worker="worker",assignment="water-assignment",owner="30v1",bucket="bucket-0",world_epoch=0,
                 bucket_before="BucketWater",river_source=True,target="water-assignment",gather_water=True,outputs=outputs),
            dict(producer="TaskCompletedVisualMessage",operation="TaskCompleted",real_seconds=base["real_seconds"],
                 worker="worker",assignment="water-assignment",target="water-assignment",work_type="GatherWater")])
    for sample in original:
        if sample["real_seconds"]>=44:
            sample["m2_context"]=copy.deepcopy(base["m2_context"])
            sample["art_feedback"]=copy.deepcopy(base["art_feedback"])
    samples=sorted([*original,*extra],key=lambda s:s["real_seconds"])
    remap={s["sample_index"]:index for index,s in enumerate(samples) if s in original}
    for event in raw["events"]:
        event["sample_index"]=remap[event["sample_index"]]
        if "before_sample_index" in event:
            event["before_sample_index"]=remap[event["before_sample_index"]]
    for index,sample in enumerate(samples):
        sample["sample_index"]=index
    for event in extra_events:
        event["sample_index"]=next(s["sample_index"] for s in samples if s["real_seconds"]==event["real_seconds"])
    raw["samples"]=samples
    raw["events"]=sorted([*raw["events"],*extra_events],key=lambda e:e["sample_index"])
    fills=[]
    for pour in raw["events"]:
        if pour.get("operation") != "TankPour":
            continue
        index=pour["sample_index"]-1
        prior=samples[index]
        prior["m2_context"]["bucket_actors"][0].update(phase="GoingToDestination",inventory_present=True)
        fills.append(dict(producer="ordinary-river-fill-branch-v1",operation="RiverFill",sample_index=index,
                          real_seconds=prior["real_seconds"],bucket_before="BucketEmpty",
                          **{k:pour[k] for k in ("worker","assignment","target","owner","bucket","world_epoch")}))
    raw["events"]=sorted([*raw["events"],*fills],key=lambda e:e["sample_index"])
    for sample in samples:
        sample.get("m2_context",{})["river_fill_channel"]=True
    for index,event in enumerate(raw["events"]):
        event["event_index"]=index
    entries[-1]["value"]["m2_planning"]["consumers"]=copy.deepcopy(base["art_feedback"])
    return raw,entries,client,inputs,contract


class TankFullTests(unittest.TestCase):
    def test_ten_exact_pours_reach_fifty_with_full_water_material(self):
        full.verify(*fixture())

    def test_partial_count_and_wrong_full_pose_material_or_profile_rejected(self):
        for mutate in (
            lambda r,e,c,i,a:e[-1]["value"]["m2_planning"]["consumers"]["roots"][0].update(tank_water_count=49),
            lambda r,e,c,i,a:r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1]["pose"].update(translation=[0.,12.,0.]),
            lambda r,e,c,i,a:r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1]["material_values"].update(white=False),
            lambda r,e,c,i,a:r.update(scope=full.inflow.SCOPE),
            lambda r,e,c,i,a:a.update(case=full.inflow.CASE),
        ):
            changed=fixture()
            mutate(*changed)
            with self.assertRaises((ValueError,RuntimeError)):
                full.verify(*changed)

    def test_partial_default_does_not_borrow_full_scope(self):
        with self.assertRaises((ValueError,RuntimeError)):
            full.inflow.verify(*fixture())

    def test_full_requires_fresh_same_bucket_worker_request_river_fill(self):
        for mutate in (
            lambda r:r.update(events=[e for e in r["events"] if e.get("operation")!="RiverFill"]),
            lambda r:next(e for e in r["events"] if e.get("operation")=="RiverFill").update(worker="foreign"),
            lambda r:next(e for e in r["events"] if e.get("operation")=="RiverFill").update(bucket="foreign"),
        ):
            args=fixture()
            mutate(args[0])
            for index,event in enumerate(args[0]["events"]):
                event["event_index"]=index
            with self.assertRaises((ValueError,RuntimeError)):
                full.verify(*args)

    def test_fill_observation_cannot_be_future_or_reused_from_a_previous_cycle(self):
        raw=fixture()[0]
        full.inflow.check_river_fill_chain(raw)
        fills=[e for e in raw["events"] if e.get("operation")=="RiverFill"]
        pours=[e for e in raw["events"] if e.get("operation")=="TankPour"]
        # Keep event ordinals and old event clocks: neither proves chronology.
        future=copy.deepcopy(raw)
        future_fill=next(e for e in future["events"] if e.get("operation")=="RiverFill")
        future_fill["sample_index"]=len(future["samples"])-1
        with self.assertRaisesRegex((ValueError,RuntimeError),"stale or foreign"):
            full.inflow.check_river_fill_chain(future)
        same_sample=copy.deepcopy(raw)
        next(e for e in same_sample["events"] if e.get("operation")=="RiverFill")["sample_index"]=pours[0]["sample_index"]
        with self.assertRaisesRegex((ValueError,RuntimeError),"stale or foreign"):
            full.inflow.check_river_fill_chain(same_sample)
        old_cycle=copy.deepcopy(raw)
        second=[e for e in old_cycle["events"] if e.get("operation")=="RiverFill"][1]
        second.update(sample_index=fills[0]["sample_index"],real_seconds=fills[0]["real_seconds"])
        with self.assertRaisesRegex((ValueError,RuntimeError),"stale or foreign"):
            full.inflow.check_river_fill_chain(old_cycle)
