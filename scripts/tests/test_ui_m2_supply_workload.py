"""Synthetic prerequisite witnesses, never native acceptance evidence."""
import copy
import unittest
from scripts import ui_m2_supply_workload as workload


def inventory(*, area=False, floor=False):
    cells = [[40+x, 40+y] for y in range(4) for x in range(5)]
    bounds = dict(min=[0., 0.], max=[160., 128.])
    return dict(anchor=[40,40], cells=cells, **bounds, overflow=False,
                sites=[dict(entity="site", total=20, reinforced=0,poured=0,material_center=[80.,64.],phase="Reinforcing", **bounds)] if floor else [],
                tiles=[dict(entity=f"tile-{i}", site="site", grid=grid,bones=0,mud=0,state="WaitingBones") for i,grid in enumerate(cells)] if floor else [],
                completed=[], familiars=[dict(entity="familiar", bits=12, command="Patrol" if area else "Idle",
                                              area=bounds if area else None)])


def prepend_workload(raw, entries, client, inputs):
    """Insert actual typed controls and endpoint samples before Full setup."""
    for row in inputs:
        row["step"] = str(int(row["step"])+20)
        row["pre_frame"] += 2000
        row["post_frame"] += 2000
        if action := row.get("tank_completion_action"):
            action["first_step"] += 20
            action["last_step"] += 20
            for value in (action["before"],action["after"]):
                value["frame"] += 2000
                value["m2_planning"]["real_seconds"] += 20
    for entry in entries[3:]:
        entry["last_step"] += 20
        entry["value"]["frame"] += 2000
        entry["value"]["m2_planning"]["real_seconds"] += 20
    for sample in raw["samples"][1:]:
        sample["sample_index"] += 11
        sample["real_seconds"] += 20
    for event in raw["events"]:
        event["sample_index"] += 11
        event["real_seconds"] += 20
    prefix, samples, events = [], [], []
    for index,key in enumerate(workload.KEYS):
        time = float(index+2)
        before = copy.deepcopy(entries[2]["value"])
        ctx = inventory(area=index>2, floor=index>7)
        ctx.update(points=[[400.,400.],[500.,500.]],
                   task_mode="AreaSelection(None)" if index == 2 else "FloorPlace(None)")
        before.update(frame=int(time*100),paused=index not in (6,7,8),selected=12,cursor=[400.,400.],
                      pointer_over_ui=False,world_input_captured=False)
        before["m2_planning"].update(real_seconds=time,supply_workload=ctx)
        before["controls"][key] = dict(fully_visible=True,interaction="Hovered")
        after = copy.deepcopy(before)
        after.update(frame=before["frame"]+30,paused=index not in (5,6,7))
        after["m2_planning"]["real_seconds"] += .3
        if index in (2,7):
            after["m2_planning"]["supply_workload"] = dict(ctx, **{k:v for k,v in inventory(area=True,floor=index==7).items()})
        first = len(prefix)+1
        dragging = key.startswith("supply-")
        for offset,pressed in enumerate((True,False)):
            point = [500.,500.] if dragging and not pressed else [400.,400.]
            if dragging and not pressed:
                prefix.append(dict(step=str(len(prefix)+1),nonce=client["nonce"],pid=client["pid"],window=50,
                    world_epoch=0,pre_frame=before["frame"]+10,post_frame=before["frame"]+20,
                    point=[500,500],observed_cursor=point,observed_left_pressed=True))
            field = dict(key="Escape") if key == "Escape" else dict(button=1)
            prefix.append(dict(step=str(len(prefix)+1),nonce=client["nonce"],pid=client["pid"],window=50,
                world_epoch=0,pre_frame=before["frame"]+(20 if offset else 0),
                post_frame=before["frame"]+(30 if offset else 10),pressed=pressed,
                observed_hover={key:"Pressed" if pressed else "Hovered"},**field))
            sample = copy.deepcopy(raw["samples"][0])
            sample.update(sample_index=len(samples)+1,real_seconds=time+.1*(offset+1),
                          paused=after["paused"],relative_speed=1.,cursor_position=point,dpi=1.)
            # Menu pairs may share one sample; gestures retain both endpoints.
            if dragging or offset == 0:
                samples.append(sample)
            else:
                samples[-1]["real_seconds"] = time+.3
            events.append(dict(producer="KeyboardInput" if key=="Escape" else "MouseButtonInput",
                operation="KeyboardInput" if key=="Escape" else "MouseButtonInput",window="99v1",
                state="Pressed" if pressed else "Released",sample_index=len(samples),
                real_seconds=time+.1*(offset+1),**(dict(key="Escape",repeat=False) if key=="Escape" else dict(button="Left"))))
        prefix[first-1]["tank_completion_action"] = dict(key=key,first_step=first,last_step=len(prefix),before=before,after=after)
    inputs[:0] = prefix
    raw["samples"][1:1] = samples
    raw["events"][:0] = events
    for i,event in enumerate(raw["events"]):
        event["event_index"] = i
    return raw,entries,client,inputs


class WorkloadTests(unittest.TestCase):
    def test_middle_drag_motion_requires_exact_endpoint_frame_order_and_held_button(self):
        from scripts.tests.test_ui_m2_tank_supply import input_fixture
        from scripts import ui_m2_tank_supply as supply
        for mutate in (lambda r:r.update(point=[501,500],observed_cursor=[501.,500.]),
                       lambda r:r.update(pre_frame=1,post_frame=2),
                       lambda r:r.update(observed_left_pressed=False)):
            raw,entries,client,inputs = input_fixture()
            action = next(r["tank_completion_action"] for r in inputs
                          if r.get("tank_completion_action",{}).get("key") == "supply-area-drag")
            mutate(inputs[action["first_step"]])
            with self.subTest(mutate=mutate), self.assertRaises(RuntimeError):
                supply.check_tail_inputs(raw,entries,client,inputs)

    def test_twenty_tile_commit_and_foreign_or_missing_inventory(self):
        self.assertTrue(workload.placed(inventory(area=True,floor=True)))
        for mutate in (lambda c:c["tiles"].pop(),lambda c:c["tiles"][0].update(site="foreign"),
                       lambda c:c["sites"][0].update(total=19),lambda c:c["sites"].append(c["sites"][0]),
                       lambda c:c["tiles"][0].update(grid=c["tiles"][1]["grid"])):
            ctx = inventory(area=True,floor=True)
            mutate(ctx)
            self.assertFalse(workload.placed(ctx))

    def test_area_requires_selected_patrolling_familiar_and_exact_bounds(self):
        ctx = inventory(area=True)
        self.assertTrue(workload.active(ctx,12))
        self.assertFalse(workload.active(ctx,13))
        ctx["familiars"][0]["command"] = "Idle"
        self.assertFalse(workload.active(ctx,12))

    @staticmethod
    def verification_fixture():
        from scripts.tests.test_ui_m2_tank_supply import input_fixture
        raw,entries,client,inputs = input_fixture()
        actions = [r["tank_completion_action"] for r in inputs if "tank_completion_action" in r]
        area_end = workload.tank.context(actions[2]["after"])["real_seconds"]
        floor_end = workload.tank.context(actions[7]["after"])["real_seconds"]
        # Include the exact after snapshots in the ordinary observer stream.
        raw = dict(events=[dict(producer="MouseButtonInput",state="Released",sample_index=i,
                               real_seconds=workload.tank.context(actions[a]["after"])["real_seconds"]-.05)
                           for a,i in ((2,3),(7,6))],
                   samples=[dict(real_seconds=t,m2_context=dict(supply_workload=inventory(
            area=t>=area_end,floor=t>=floor_end))) for t in (0.,2.,4.,area_end,7.,8.,floor_end,20.)])
        return raw,actions

    def test_commit_before_actual_release_is_rejected_without_changing_after(self):
        for action_index,raw_index in ((2,3),(7,6)):
            raw,actions = self.verification_fixture()
            # The state is still after UI before, but ahead of the actual input.
            raw["events"][0 if action_index==2 else 1]["sample_index"] = raw_index+1
            with self.subTest(action_index=action_index), self.assertRaises(RuntimeError):
                workload.verify(raw,actions)

    def test_raw_prerequisite_requires_actual_input_and_persistent_owner(self):
        raw,actions = self.verification_fixture()
        workload.verify(raw,actions)
        for index,mutate in (
            (0,lambda c:c.update(sites=inventory(floor=True)["sites"])),
            (4,lambda c:c.update(tiles=inventory(floor=True)["tiles"])),
            (-1,lambda c:c["familiars"][0].update(command="Idle")),
            (-1,lambda c:c["familiars"][0].update(bits=13)),
            (-1,lambda c:c["tiles"].pop()),
            (-1,lambda c:c["tiles"][0].update(entity=c["tiles"][1]["entity"])),
            (-1,lambda c:c["tiles"][0].update(entity="replacement")),
            (-1,lambda c:c["familiars"][0].update(bits=True)),
            (-1,lambda c:c["sites"][0].update(total=True)),
            (-1,lambda c:c.update(overflow=True)),
            (-1,lambda c:c.update(anchor=[41,40])),
        ):
            raw,actions = self.verification_fixture()
            mutate(raw["samples"][index]["m2_context"]["supply_workload"])
            with self.subTest(index=index,mutate=mutate),self.assertRaises(RuntimeError):
                workload.verify(raw,actions)

    def test_atomic_completed_floor_is_allowed_but_partial_or_replaced_is_not(self):
        raw,actions = self.verification_fixture()
        ctx = raw["samples"][-1]["m2_context"]["supply_workload"]
        ctx.update(sites=[],tiles=[],completed=[dict(grid=p,entity=f"floor-{i}",valid=True) for i,p in enumerate(ctx["cells"])])
        workload.verify(raw,actions)
        ctx["completed"].pop()
        with self.assertRaises(RuntimeError):
            workload.verify(raw,actions)


if __name__ == "__main__":
    unittest.main()
