"""Synthetic branch fixtures only, not water lifecycle/native evidence."""
import copy
import unittest
from scripts import building_production_lifecycle as lifecycle


def fixture():
    event = dict(producer="ordinary-tank-pour-branch-v1", operation="TankPour", sample_index=0, event_index=0,
                 real_seconds=1., worker="worker", assignment="assignment", owner="tank", bucket="bucket", world_epoch=0,
                 bucket_before="BucketWater", river_source=True, target="assignment", gather_water=True, outputs=[f"water-{i}" for i in range(5)])
    context = dict(water_inventory_schema=1, tank_pour_channel=True,
                   accepted=False, performance_evidence=False, promotion_authority=False,
                   tanks=[dict(entity="tank", provisional=False, stockpile=dict(resource="Water", capacity=50),
                               stored_items=list(event["outputs"]))],
                   items=[dict(entity=item, resource="Water", stored_in="tank", loaded_in=None) for item in event["outputs"]]
                   + [dict(entity="bucket", resource="BucketEmpty", stored_in=None, loaded_in=None, delivering_to=None)])
    completed = dict(producer="TaskCompletedVisualMessage", operation="TaskCompleted", worker="worker", assignment="assignment",
                     target="assignment", work_type="GatherWater", sample_index=0, event_index=1, real_seconds=1.)
    return event, [dict(real_seconds=1., world_epoch=0, m2_context=context, actors=[dict(entity="worker")])], [event, completed]


class TankPourTests(unittest.TestCase):
    def test_exact_branch_receipt(self):
        lifecycle.check_tank_pour_receipt(*fixture())

    def test_missing_foreign_and_type_aliases_fail_closed(self):
        for mutate in (
            lambda e, s: e.update(world_epoch=True),
            lambda e, s: e.update(sample_index=True),
            lambda e, s: e.update(real_seconds=float("nan")),
            lambda e, s: e.update(river_source=1),
            lambda e, s: e.update(bucket_before="BucketEmpty"),
            lambda e, s: e.update(outputs=["water-0"] * 5),
            lambda e, s: e.update(owner="worker"),
            lambda e, s: e.update(worker="foreign"),
            lambda e, s: e.update(assignment="foreign"),
            lambda e, s: e.update(target="bucket"),
            lambda e, s: e.update(gather_water=1),
            lambda e, s: s[0].update(actors=[]),
            lambda e, s: s[0]["m2_context"].update(water_inventory_schema=True),
            lambda e, s: s[0]["m2_context"].update(tank_pour_channel=False),
            lambda e, s: s[0]["m2_context"]["tanks"][0].update(provisional=0),
            lambda e, s: s[0]["m2_context"]["tanks"][0]["stored_items"].pop(),
            lambda e, s: s[0]["m2_context"]["items"][0].update(stored_in="other"),
            lambda e, s: s[0]["m2_context"]["items"][-1].update(delivering_to="tank"),
            lambda e, s: s[0]["m2_context"]["items"].append(copy.deepcopy(s[0]["m2_context"]["items"][0])),
        ):
            event, samples, events = fixture()
            mutate(event, samples)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                lifecycle.check_tank_pour_receipt(event, samples, events)

    def test_raw_parser_rejects_reused_water_but_allows_reusable_bucket(self):
        event, samples, events = fixture()
        samples[0].update(sample_index=0, **{key: [] for key in lifecycle.TRANSITION_CATEGORIES})
        raw = dict(schema_version=2, transition_contract="indexed-state-delta-v1", samples=samples, events=events)
        lifecycle.check_raw_transitions(raw)
        duplicate = copy.deepcopy(event)
        duplicate["event_index"] = 2
        raw["events"].append(duplicate)
        with self.assertRaisesRegex(ValueError, "output receipt reused"):
            lifecycle.check_raw_transitions(raw)

    def test_completion_requires_real_same_sample_clock_and_unique_actor(self):
        for time in (None, True, float("nan"), float("inf"), -1., 2.):
            event, samples, events = fixture()
            events[1]["real_seconds"] = time
            with self.subTest(time=time), self.assertRaises(ValueError):
                lifecycle.check_tank_pour_receipt(event, samples, events)
        event, samples, events = fixture()
        samples[0]["actors"] *= 2
        with self.assertRaises(ValueError):
            lifecycle.check_tank_pour_receipt(event, samples, events)
