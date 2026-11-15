"""Synthetic receipt rejection regressions, never native/lifecycle acceptance."""
import copy
import unittest
from scripts import building_production_lifecycle as lifecycle


def fixture():
    event = dict(producer="ordinary-mixer-refine-branch-v1", operation="RefineCommit", sample_index=0, event_index=0,
                 real_seconds=1., worker="worker", assignment="assignment", owner="mixer", world_epoch=0,
                 water="water", water_count=1, outputs=[f"mud-{i}" for i in range(5)],
                 counter_order=["Sand", "Rock", "StasisMud"], before=[2, 3, 0], after=[1, 2, 5])
    context = dict(schema=1, refine_commit_channel=True, accepted=False, performance_evidence=False, promotion_authority=False,
                   mixers=[dict(entity="mixer", building=dict(kind="MudMixer", provisional=False), sand=9, rock=9, mud=5)],
                   actors=[dict(entity="worker", mixer="mixer", is_soul=True, phase="Done", owner_exists=True, working_on=None,
                                identity=dict(assignment="assignment", target="mixer", work_type="Refine", matches_working_on=True))],
                   items=[dict(entity=item, resource="StasisMud", stored_by_mixer="mixer", stored_in=None, loaded_in=None)
                          for item in event["outputs"]], working_relations=[])
    sample = dict(real_seconds=1., world_epoch=0, m2_context=context)
    return event, [sample]


class RefineReceiptTests(unittest.TestCase):
    def test_raw_parser_admits_exact_receipt_but_never_reuses_its_items(self):
        event, samples = fixture()
        samples[0].update(sample_index=0, **{key: [] for key in lifecycle.TRANSITION_CATEGORIES})
        raw = dict(schema_version=2, transition_contract="indexed-state-delta-v1", samples=samples, events=[event])
        lifecycle.check_raw_transitions(raw)
        duplicate = copy.deepcopy(event)
        duplicate["event_index"] = 1
        raw["events"].append(duplicate)
        with self.assertRaisesRegex(ValueError, "receipt reused"):
            lifecycle.check_raw_transitions(raw)

    def test_exact_receipt_uses_branch_delta_not_later_delivery_counters(self):
        event, samples = fixture()
        lifecycle.check_refine_receipt(event, samples)
        self.assertNotEqual(samples[0]["m2_context"]["mixers"][0]["sand"], event["after"][0])

    def test_foreign_missing_and_fabricated_commit_is_rejected(self):
        for mutate in (
            lambda e, s: e.update(world_epoch=False),
            lambda e, s: e.update(real_seconds=float("nan")),
            lambda e, s: e.update(sample_index=True),
            lambda e, s: e.update(event_index=True),
            lambda e, s: e.update(water_count=True),
            lambda e, s: e.update(water=None),
            lambda e, s: e.update(outputs=["mud-0"] * 5),
            lambda e, s: e["after"].__setitem__(0, 2),
            lambda e, s: e["after"].__setitem__(2, 6),
            lambda e, s: e["before"].__setitem__(0, True),
            lambda e, s: s[0]["m2_context"].update(refine_commit_channel=False),
            lambda e, s: s[0]["m2_context"].update(schema=True),
            lambda e, s: s[0]["m2_context"].update(promotion_authority=True),
            lambda e, s: s[0]["m2_context"]["actors"][0].update(working_on="mixer"),
            lambda e, s: s[0]["m2_context"]["actors"][0].pop("working_on"),
            lambda e, s: s[0]["m2_context"]["mixers"][0]["building"].update(provisional=0),
            lambda e, s: s[0]["m2_context"]["actors"][0].update(phase="Refining"),
            lambda e, s: s[0]["m2_context"]["actors"][0]["identity"].update(assignment="foreign"),
            lambda e, s: s[0]["m2_context"]["actors"][0]["identity"].update(matches_working_on=1),
            lambda e, s: s[0]["m2_context"]["items"].append(dict(entity="water")),
            lambda e, s: s[0]["m2_context"]["items"].pop(),
            lambda e, s: s[0]["m2_context"]["items"][0].update(resource="Water"),
            lambda e, s: s[0]["m2_context"]["items"][0].update(stored_by_mixer="foreign"),
            lambda e, s: s[0]["m2_context"]["items"][0].update(stored_in="mixer"),
            lambda e, s: s[0]["m2_context"]["items"][0].pop("stored_in"),
            lambda e, s: s[0]["m2_context"]["items"].append(copy.deepcopy(s[0]["m2_context"]["items"][0])),
        ):
            event, samples = fixture()
            mutate(event, samples)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                lifecycle.check_refine_receipt(event, samples)


if __name__ == "__main__":
    unittest.main()
