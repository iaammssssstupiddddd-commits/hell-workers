"""Synthetic campaign fixtures; never actual native/art/release evidence."""
import copy
import unittest
from scripts import ui_m2_tank_completion as m2
from scripts.tests import test_ui_m2_planning as planning_tests


def fixture():
    def virtual(time):
        return max(0., min(time - 2, 10.)) + max(0., min(time - 14, 16.))
    contract = planning_tests.contract()
    contract["identity"]["kind"] = "Tank"
    contract.update(case=m2.CASE, scope=m2.SCOPE, parts=[dict(name=name, mesh_role=name, material_role="opaque_albedo",
        translation_wu=[0., 0., 0.], rotation_xyzw=[0., 0., 0., 1.], scale=[1., 1., 1.]) for name in ("body", "water")])
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
    actions = {2: "menu:SetTimeSpeed(Normal)", 4: "menu:ToggleArchitect", 6: "menu:SelectBuild(Tank)",
               8: "placement", 9: "companion-placement", 10: "Escape", 12: "menu:SetTimeSpeed(Paused)",
               14: "menu:SetTimeSpeed(Normal)", 30: "menu:SetTimeSpeed(Paused)"}
    footprint = values[0]["m2_planning"]["footprint"]
    for time in sorted({*checkpoints, *actions, 20, 21, 22, 23, 24, 25}):
        index = len(samples)
        owner = time >= 25
        placed = time >= 9 and not owner
        count = min(2, max(0, time - 19))
        real = time + (.3 if time in actions else 0)
        sample = dict(sample_index=index, real_seconds=real, virtual_seconds=virtual(real), world_epoch=0,
                      paused=time < 2 or 12 <= time < 14 or time >= 30, relative_speed=1.,
                      primary_window="99v1", primary_window_bits=99, cursor_position=[400., 400.], dpi=1.,
                      owners=[dict(entity="30v1", entity_bits=30, kind="Tank", provisional=False)] if owner else [],
                      blueprints=[dict(entity="20v1", entity_bits=20, kind="Tank", footprint=footprint,
                                       materials_complete=count == 2, delivered_materials={"Wood": count}, progress=.1)] if placed else [],
                      roots=[], items=[], move_tasks=[], generations=[],
                      actors=[dict(entity="worker", task_identity=dict(assignment="assignment", target="20v1", work_type="Build"))],
                      owner_layers=[])
        if time >= 9:
            sample["owner_layers"].append(dict(entity="20v1", exists=not owner, building_grids=[] if owner else footprint,
                                              floor_grids=[], door_grids=[], bridge_grids=[], stockpile_grids=[]))
        if owner:
            sample["owner_layers"].append(dict(entity="30v1", exists=True, building_grids=footprint,
                                              floor_grids=[], door_grids=[], bridge_grids=[], stockpile_grids=[]))
            sample["art_feedback"] = dict(requested=contract["identity"], active=contract["identity"], pending=None,
                descriptor=dict(identity=contract["identity"], parts=contract["parts"], meshes=dict(body="body-mesh", water="water-mesh"), albedo="albedo"),
                roots=[dict(entity="root", owner="30v1", identity=contract["identity"], owner_pose_matches=True,
                    parts=[dict(name=name, mesh=name + "-mesh", albedo="albedo", material="material", parent="root", visible=name != "water",
                                pose=dict(translation=[0., 0., 0.], rotation=[0., 0., 0., 1.], scale=[1., 1., 1.]),
                                material_values=dict(white=True, opaque=True, lit=True, emissive_black=True, emissive_texture_absent=True,
                                                     roughness=1., reflectance=0.)) for name in ("body", "water")])])
        pending = dict(parent_kind="Tank", parent_anchor=[5, 5], kind="BucketStorage", center=[0., 0.], radius=160., required=True)
        sample["companion_context"] = dict(available=True, pending=pending if time == 8 else None, storages=[])
        if time >= 9:
            sample["companion_context"]["storages"] = [dict(entity=f"child-{n}", grid=[7 + n, 5],
                position=[(7 + n - 49.5) * 32., (5 - 49.5) * 32., .01], stockpile_grids=[[7 + n, 5]], pending_owner=None if owner else "20v1",
                owner="30v1" if owner else None) for n in range(2)]
        if owner:
            sample["m2_context"] = dict(tanks=[dict(entity="30v1", provisional=False, stockpile=dict(resource="Water", capacity=50), stored_items=None)],
                items=[dict(entity=f"bucket-{n}", resource="BucketEmpty", belongs_to="30v1", stored_in=f"child-{n % 2}",
                            loaded_in=None, delivering_to=None, stored_by_mixer=None) for n in range(5)])
        if time >= 9:
            typed = sample.setdefault("m2_context", dict(items=[]))
            typed["bucket_storages"] = [dict(entity=row["entity"], pose=dict(translation=row["position"]),
                belongs_to=row["owner"], pending_blueprint=row["pending_owner"], stockpile=dict(resource=None, capacity=10),
                stored_items=[item["entity"] for item in typed["items"] if item["stored_in"] == row["entity"]] or None)
                for row in sample["companion_context"]["storages"]]
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
            ctx = value["m2_planning"]
            ctx.update(companion_pending=pending if key == "companion-placement" else None,
                       companion_grid=[7, 5], companion_point=[450., 400.], companion_legal=True)
            if key == "companion-placement":
                ctx.update(cursor_grid=[7, 5], play_mode="BuildingPlace", legal=True, blueprints=[], buildings=[])
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
            inputs[first - 1]["tank_completion_action"] = dict(key=key, first_step=first, last_step=len(inputs), before=value, after=after)
            if key == "placement":
                events.append(dict(producer="ordinary-building-placement-branch-v1", operation="PlacementOutcome", sample_index=index,
                    real_seconds=time + .15, window="99v1", world_epoch=0,
                    outcome=dict(kind="Tank", grid=[5, 5], status="CompanionPending")))
            if key == "companion-placement":
                events.append(dict(producer="ordinary-building-placement-branch-v1", operation="PlacementOutcome", sample_index=index,
                    real_seconds=time + .15, window="99v1", world_epoch=0,
                    outcome=dict(kind="Tank", grid=[7, 5], status="CompanionCommitted", owner="20v1", footprint=footprint, companion_owners=["child-0", "child-1"])))
        if 20 <= time <= 21:
            events.append(dict(producer="ordinary-blueprint-delivery-branch-v1", operation="MaterialDelivery", result="Committed",
                sample_index=index, real_seconds=time, worker="worker", assignment="assignment", owner="20v1", item=f"wood-{count}",
                world_epoch=0, kind="Tank", resource="Wood", before_count=count - 1, after_count=count, footprint=footprint, wheelbarrow=False))
        if time == 25:
            events.extend([dict(producer="hw_jobs::publish_building_completed", operation="BuildingCompleted", sample_index=index,
                real_seconds=time, blueprint_owner="20v1", owner="30v1", kind="Tank", footprint=footprint),
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


class PlacementGhostObservationTests(unittest.TestCase):
    def observation(self, case):
        _, entries, _, _, _ = fixture()
        value = copy.deepcopy(entries[0]["value"])
        ctx = value["m2_planning"]
        value["ready"], value["paused"] = True, case == "refactor-ghost-empty"
        ctx["anchor_world"] = [0.0, 0.0]
        ctx["point"], ctx["companion_point"] = [400.0, 400.0], [464.0, 400.0]
        ctx["play_mode"] = ("Normal" if case in ("refactor-ghost-empty", "refactor-ghost-escaped") else
                            "FloorPlace" if case == "refactor-ghost-wall" else "BuildingPlace")
        value["cursor"] = [464.0, 400.0] if case == "refactor-ghost-companion" else [400.0, 400.0]
        def sprite(entity, path, size, anchor, color, position):
            return dict(entity=entity, path=path, size=size, anchor=anchor, resident=True, visible=True,
                        view_visible=True, color=color, position=[*position, 1.0])
        ghost = dict(active=m2.GHOST_TANK_ID, pending=None, failure=None,
                     door=m2.GHOST_DOOR_ID, ghosts=[], partners=[])
        ctx["placement_ghosts"] = ghost
        ctx["companion_pending"] = None
        if case == "refactor-ghost-door":
            ctx["build_kind"] = "Door"
            ghost["ghosts"] = [sprite(10, next(iter(m2.GHOST_DOOR_PATHS)), [64.0, 64.0], [0.0, -0.25],
                                      [1.0, 0.2, 0.2, 0.5], [0.0, 0.0])]
        elif case == "refactor-ghost-tank":
            ctx["build_kind"], ctx["legal"] = "Tank", True
            ghost["ghosts"] = [sprite(10, m2.GHOST_TANK_PATH, [96.0, 96.0], [0.0, -0.125],
                                      [0.5, 1.0, 0.5, 0.5], [0.0, 0.0])]
        elif case == "refactor-ghost-companion":
            ctx["companion_pending"] = dict(parent_anchor=[5, 5], parent_kind="Tank", kind="BucketStorage",
                                            center=[0.0, 0.0], radius=160.0, required=True)
            ctx["cursor_grid"] = ctx["companion_grid"] = [7, 5]
            ctx["companion_legal"] = True
            ghost["ghosts"] = [sprite(10, "textures/items/bucket/bucket_empty.png", [64.0, 32.0], [0.0, 0.0],
                                      [0.5, 1.0, 0.5, 0.5], [80.0, 0.0])]
            ghost["partners"] = [sprite(11, m2.GHOST_TANK_PATH, [96.0, 96.0], [0.0, -0.125],
                                        [0.8, 0.9, 1.0, 0.35], [0.0, 0.0])]
        elif case == "refactor-ghost-wall":
            ctx["task_mode"] = "WallPlace(None)"
        return value

    def test_each_stage_accepts_its_actual_sprite_contract(self):
        for case in m2.GHOST_CHECKPOINTS:
            with self.subTest(case=case):
                m2.check_ghost_observation(case, self.observation(case))

    def test_wrong_generation_anchor_or_pending_sprite_is_rejected(self):
        for mutation in ("generation", "anchor", "sprite", "wall", "color", "visibility", "pending", "partner"):
            with self.subTest(mutation=mutation):
                case = "refactor-ghost-wall" if mutation == "wall" else "refactor-ghost-companion"
                value = self.observation(case)
                state = value["m2_planning"]["placement_ghosts"]
                if mutation == "generation":
                    state["active"] = {**m2.GHOST_TANK_ID, "generation": 3}
                elif mutation == "anchor":
                    state["partners"][0]["anchor"] = [0.0, 0.0]
                elif mutation == "sprite":
                    state["ghosts"][0]["path"] = m2.GHOST_TANK_PATH
                elif mutation == "color":
                    state["ghosts"][0]["color"] = [1.0, 0.2, 0.2, 0.5]
                elif mutation == "visibility":
                    state["partners"][0]["view_visible"] = False
                elif mutation == "pending":
                    value["m2_planning"]["companion_pending"]["parent_anchor"] = [8, 8]
                elif mutation == "partner":
                    state["partners"][0]["entity"] = state["ghosts"][0]["entity"]
                else:
                    value["m2_planning"]["task_mode"] = "None"
                with self.assertRaises(m2.native.AcceptanceError):
                    m2.check_ghost_observation(case, value)

    def evidence(self):
        client = {"nonce": "b" * 32, "pid": 40, "window": 50}
        entries = []
        for index, case in enumerate(m2.GHOST_CHECKPOINTS):
            value = self.observation(case)
            value["frame"] = 100 + 100 * index
            entries.append(dict(case=case, value=value, last_step=0))
        groups = [
            ["menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", "menu:SelectBuild(Door)"],
            ["menu:SelectBuild(Tank)"], ["placement"], ["Escape"],
            ["menu:ToggleArchitect", "menu:SelectBuild(Wall)"],
        ]
        events, raw_events, samples = [], [], []
        for lower, group in enumerate(groups):
            for action_index, key in enumerate(group):
                a, b = copy.deepcopy(entries[lower]["value"]), copy.deepcopy(entries[lower + 1]["value"])
                a["frame"] = entries[lower]["value"]["frame"] + 10 + action_index * 10
                b["frame"] = a["frame"] + 4
                start = a["frame"] / 100.0
                a["m2_planning"]["real_seconds"], b["m2_planning"]["real_seconds"] = start, start + 0.1
                a["controls"] = {key: {"fully_visible": True, "interaction": "Hovered"}}
                a["pointer_over_ui"], a["world_input_captured"] = False, False
                if key == "placement":
                    a["m2_planning"].update(play_mode="BuildingPlace", build_kind="Tank", legal=True,
                                            companion_legal=True, cursor_grid=[5, 5], companion_pending=None)
                first = len(events) + 1
                for i, pressed in enumerate((True, False)):
                    event = dict(step=str(len(events) + 1), nonce=client["nonce"], pid=40, window=50,
                                 world_epoch=0, pre_frame=a["frame"] + i, post_frame=a["frame"] + i + 1,
                                 point=None, button=None if key == "Escape" else 1,
                                 key="Escape" if key == "Escape" else None, pressed=pressed,
                                 observed_cursor=[400.0, 400.0],
                                 observed_hover={key: "Pressed"} if pressed else {})
                    events.append(event)
                    sample_index = len(samples)
                    time = start + (i + 1) * 0.02
                    samples.append(dict(world_epoch=0, primary_window_bits=99, primary_window="window", real_seconds=time + 0.005))
                    raw_events.append(dict(producer="KeyboardInput" if key == "Escape" else "MouseButtonInput",
                                           state="Pressed" if pressed else "Released", event_index=len(raw_events),
                                           sample_index=sample_index, real_seconds=time, window="window",
                                           **({"key": "Escape", "repeat": False} if key == "Escape" else {"button": "Left"})))
                    if key == "placement" and pressed:
                        samples.append(dict(world_epoch=0, primary_window_bits=99, primary_window="window", real_seconds=start + 0.035))
                        raw_events.append(dict(producer="ordinary-building-placement-branch-v1", operation="PlacementOutcome",
                                               event_index=len(raw_events), sample_index=len(samples) - 1, real_seconds=start + 0.03,
                                               window="window", world_epoch=0,
                                               outcome=dict(kind="Tank", grid=[5, 5], status="CompanionPending")))
                events[first - 1]["tank_completion_action"] = dict(key=key, first_step=first, last_step=len(events), before=a, after=b)
            if lower in (0, 1, 2, 4):
                point = [464, 400] if lower == 2 else [400, 400]
                frame = entries[lower]["value"]["frame"] + 80
                events.append(dict(step=str(len(events) + 1), nonce=client["nonce"], pid=40, window=50,
                                   world_epoch=0, pre_frame=frame, post_frame=frame + 1, point=point,
                                   button=None, key=None, pressed=None, observed_cursor=point))
            entries[lower + 1]["last_step"] = len(events)
        raw = dict(failure=None, nonce=client["nonce"], process_id=40, samples=samples, events=raw_events)
        return raw, entries, client, events

    def test_complete_ghost_input_and_pending_receipt_is_accepted(self):
        m2.verify_ghosts(*self.evidence())

    def test_invalid_ghost_sequence_or_receipt_is_rejected(self):
        for mutation in ("checkpoint", "receipt_missing", "receipt_duplicate", "epoch", "pid", "anchor", "second_click", "not_cancelled", "missing_motion", "early_motion", "wrong_mode", "click_cursor"):
            with self.subTest(mutation=mutation):
                raw, entries, client, events = self.evidence()
                receipt = next(r for r in raw["events"] if r["producer"] == "ordinary-building-placement-branch-v1")
                if mutation == "checkpoint":
                    entries[2]["last_step"] = entries[1]["last_step"]
                elif mutation == "receipt_missing":
                    raw["events"].remove(receipt)
                elif mutation == "receipt_duplicate":
                    raw["events"].append(copy.deepcopy(receipt))
                elif mutation == "epoch":
                    entries[3]["value"]["world_epoch"] = 1
                elif mutation == "pid":
                    events[0]["pid"] = 999
                elif mutation == "anchor":
                    receipt["outcome"]["grid"] = [6, 5]
                elif mutation == "second_click":
                    next(e for e in reversed(events) if "tank_completion_action" in e)["tank_completion_action"]["key"] = "placement"
                elif mutation == "missing_motion":
                    next(e for e in events if e.get("point") == [464, 400])["point"] = [465, 400]
                elif mutation == "early_motion":
                    action = next(e["tank_completion_action"] for e in events if e.get("tank_completion_action", {}).get("key") == "placement")
                    next(e for e in events if e.get("point") == [464, 400])["pre_frame"] = action["after"]["frame"] - 1
                elif mutation == "wrong_mode":
                    entries[5]["value"]["m2_planning"]["play_mode"] = "Normal"
                elif mutation == "click_cursor":
                    action = next(e["tank_completion_action"] for e in events if e.get("tank_completion_action", {}).get("key") == "placement")
                    events[action["first_step"] - 1]["observed_cursor"] = [900, 900]
                else:
                    entries[4]["value"]["m2_planning"]["placement_ghosts"]["ghosts"] = [None]
                with self.assertRaises(m2.native.AcceptanceError):
                    m2.verify_ghosts(raw, entries, client, events)


class TankCompletionTests(unittest.TestCase):
    def test_exact_synthetic_companion_construction(self):
        m2.verify(*fixture())

    def test_missing_or_foreign_companion_bucket_and_water_fail_closed(self):
        for mutate in (
            lambda r, e, c, i, a: a["identity"].update(kind="MudMixer"),
            lambda r, e, c, i, a: r.update(scope="ordinary-m2-candidate-completion-v1"),
            lambda r, e, c, i, a: r["events"][6].update(real_seconds=0.),
            lambda r, e, c, i, a: next(x for x in r["events"] if x.get("outcome", {}).get("status") == "CompanionCommitted")["outcome"].update(grid=[5, 5]),
            lambda r, e, c, i, a: next(x for x in r["events"] if x.get("outcome", {}).get("status") == "CompanionCommitted")["outcome"].update(companion_owners=["child-0", "child-0"]),
            lambda r, e, c, i, a: r["samples"][-1]["companion_context"]["storages"][0].update(owner="foreign"),
            lambda r, e, c, i, a: r["samples"][-1]["companion_context"]["storages"].pop(),
            lambda r, e, c, i, a: r["samples"][-1]["art_feedback"]["roots"][0]["parts"][1].update(visible=True),
            lambda r, e, c, i, a: r["samples"][-1]["m2_context"]["tanks"][0].update(stored_items=["water"]),
            lambda r, e, c, i, a: next(s for s in r["samples"] if s["real_seconds"] == 25)["m2_context"]["items"].pop(),
            lambda r, e, c, i, a: next(s for s in r["samples"] if s["real_seconds"] == 25)["m2_context"]["items"][0].update(belongs_to="foreign"),
            lambda r, e, c, i, a: r["samples"][-1]["companion_context"]["storages"][0].update(position=[0., 0., 0.]),
            lambda r, e, c, i, a: r["samples"][-1]["m2_context"]["bucket_storages"][0]["stockpile"].update(capacity=1),
            lambda r, e, c, i, a: r["samples"][-1]["m2_context"]["bucket_storages"][0].update(stored_items=["foreign"]),
            lambda r, e, c, i, a: r["samples"][-1]["m2_context"]["tanks"][0].pop("stored_items"),
            lambda r, e, c, i, a: next(s for s in r["samples"] if s["real_seconds"] == 25)["m2_context"]["items"][0].update(loaded_in="cart"),
            lambda r, e, c, i, a: next(s for s in r["samples"] if s["real_seconds"] == 8.3)["companion_context"]["pending"].update(required=1),
        ):
            changed = fixture()
            mutate(*changed)
            with self.subTest(mutate=mutate), self.assertRaises((RuntimeError, ValueError)):
                m2.verify(*changed)

    def test_world_target_is_checked_before_press(self):
        raw, entries, client, inputs, contract = fixture()
        baseline = m2.context(entries[0]["value"])
        action = next(row["tank_completion_action"] for row in inputs
                      if row.get("tank_completion_action", {}).get("key") == "companion-placement")
        m2.check_world_action(action["before"], "companion-placement", baseline)
        for key, value in (("cursor_grid", [5, 5]), ("companion_pending", None), ("companion_legal", False)):
            before = copy.deepcopy(action["before"])
            before["m2_planning"][key] = value
            with self.assertRaises((RuntimeError, ValueError)):
                m2.check_world_action(before, "companion-placement", baseline)
