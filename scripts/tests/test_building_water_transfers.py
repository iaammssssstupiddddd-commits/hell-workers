"""Synthetic conversions only; neither ordinary-world nor native acceptance."""

import copy
import unittest
from scripts import building_water_transfers as water
from scripts import building_production_lifecycle as lifecycle


def fixture():
    def actor(phase, amount):
        return dict(
            entity="soul",
            is_soul=True,
            inventory_present=True,
            bucket="bucket",
            carried_item="bucket",
            working_on="request",
            phase=phase,
            amount=amount,
            identity=dict(
                assignment="request",
                target="request",
                work_type="HaulWaterToMixer",
                matches_working_on=True,
            ),
            source=dict(kind="Tank", entity="tank", exists=True, needs_fill=True),
            destination=dict(kind="Mixer", entity="mixer", exists=True),
        )

    def ctx(phase, resource, ids, delivering):
        return dict(
            water_inventory_schema=1,
            mixer_water_channel=True,
            accepted=False,
            performance_evidence=False,
            promotion_authority=False,
            tanks=[
                dict(
                    entity="tank",
                    provisional=False,
                    stockpile=dict(resource="Water", capacity=50),
                    stored_items=ids,
                )
            ],
            mixers=[
                dict(
                    entity="mixer",
                    building=dict(kind="MudMixer", provisional=False),
                    stockpile=dict(resource="Water", capacity=10),
                    stored_items=[],
                )
            ],
            bucket_actors=[actor(phase, 0 if phase == "Filling" else 5)],
            working_relations=[dict(entity="soul", owner="request")],
            items=[
                dict(
                    entity="bucket",
                    resource=resource,
                    belongs_to="tank",
                    stored_in=None,
                    loaded_in=None,
                    delivering_to=delivering,
                )
            ]
            + [
                dict(entity=i, resource="Water", stored_in="tank", loaded_in=None)
                for i in ids
            ],
        )

    consumed = [f"water-{i}" for i in range(5)]
    produced = [f"poured-{i}" for i in range(5)]
    samples = [
        dict(
            sample_index=i,
            real_seconds=float(i + 1),
            world_epoch=0,
            actors=[dict(entity="soul", task_identity=None)],
            **{key: [] for key in lifecycle.TRANSITION_CATEGORIES},
        )
        for i in range(4)
    ]
    samples[0]["m2_context"] = ctx("Filling", "BucketEmpty", consumed, None)
    samples[1]["m2_context"] = ctx("GoingToDestination", "BucketWater", [], "mixer")
    samples[2]["m2_context"] = ctx("Pouring", "BucketWater", [], "mixer")
    samples[3]["m2_context"] = ctx("ReturningBucket", "BucketEmpty", [], None)
    samples[3]["m2_context"]["mixers"][0]["stored_items"] = produced
    samples[3]["m2_context"]["items"] += [
        dict(entity=i, resource="Water", stored_in="mixer", loaded_in=None)
        for i in produced
    ]
    common = dict(
        worker="soul",
        assignment="request",
        target="request",
        owner="tank",
        mixer="mixer",
        bucket="bucket",
        carried_before="bucket",
        world_epoch=0,
        work_type="HaulWaterToMixer",
    )
    take = dict(
        common,
        producer="ordinary-tank-take-branch-v1",
        operation="TankTake",
        sample_index=1,
        event_index=0,
        real_seconds=2.0,
        bucket_before="BucketEmpty",
        inputs=consumed,
        inputs_before=[
            dict(entity=entity, resource="Water", stored_in="tank", loaded_in=None)
            for entity in consumed
        ],
        route_found=True,
    )
    pour = dict(
        common,
        producer="ordinary-mixer-pour-branch-v1",
        operation="MixerPour",
        sample_index=3,
        event_index=1,
        real_seconds=4.0,
        amount=5,
        bucket_before="BucketWater",
        before_count=0,
        capacity=10,
        outputs=produced,
        returning=True,
    )
    return dict(
        schema_version=2,
        transition_contract="indexed-state-delta-v1",
        samples=samples,
        events=[take, pour],
    )


class WaterTransferTests(unittest.TestCase):
    def test_exact_conversion_and_lossless_chain(self):
        raw = fixture()
        lifecycle.check_raw_transitions(raw)
        water.check_lossless_chain(raw)

    def test_partial_or_failed_branch_is_observed_but_not_lossless_supply(self):
        for field in ("partial", "route", "return"):
            raw = fixture()
            pour = raw["events"][1]
            ctx = raw["samples"][-1]["m2_context"]
            if field == "partial":
                pour.update(capacity=2, outputs=pour["outputs"][:2])
                ctx["mixers"][0]["stockpile"]["capacity"] = 2
                ctx["mixers"][0]["stored_items"] = pour["outputs"]
                ctx["items"] = ctx["items"][:3]
            elif field == "route":
                raw["events"][0]["route_found"] = False
            else:
                pour["returning"] = False
                ctx["bucket_actors"] = []
                ctx["working_relations"] = []
            lifecycle.check_raw_transitions(raw)
            with self.assertRaises(ValueError):
                water.check_lossless_chain(raw)

    def test_foreign_missing_type_alias_and_false_completion_rejected(self):
        for mutate in (
            lambda r: r["events"][0].update(world_epoch=True),
            lambda r: r["events"][0].update(route_found=1),
            lambda r: r["events"][0].update(target="bucket"),
            lambda r: r["events"][0].update(inputs=["water-0"] * 5),
            lambda r: r["samples"][1]["m2_context"]["items"].append(
                copy.deepcopy(r["samples"][0]["m2_context"]["items"][1])
            ),
            lambda r: r["events"][0]["inputs_before"][0].update(stored_in="foreign"),
            lambda r: r["samples"][1]["m2_context"]["bucket_actors"][0].update(
                amount=True
            ),
            lambda r: r["events"][1].update(returning=1),
            lambda r: r["events"][1].update(amount=True),
            lambda r: r["events"][1].update(before_count=-1),
            lambda r: r["events"][1].update(capacity=True),
            lambda r: r["samples"][3]["m2_context"]["items"][1].update(
                stored_in="foreign"
            ),
            lambda r: r["samples"][3]["m2_context"].update(promotion_authority=True),
            lambda r: r["events"].append(
                dict(
                    producer="TaskCompletedVisualMessage",
                    operation="TaskCompleted",
                    worker="soul",
                    assignment="request",
                    target="request",
                    work_type="HaulWaterToMixer",
                    sample_index=3,
                    event_index=2,
                    real_seconds=4.0,
                )
            ),
        ):
            raw = fixture()
            mutate(raw)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(raw)

    def test_duplicate_consumed_or_generated_id_is_not_a_second_transfer(self):
        for which in (0, 1):
            raw = fixture()
            duplicate = copy.deepcopy(raw["events"][which])
            duplicate["event_index"] = 2
            raw["events"].append(duplicate)
            with self.assertRaisesRegex(ValueError, "receipt reused"):
                lifecycle.check_raw_transitions(raw)

    def test_future_foreign_or_old_cycle_take_does_not_supply_a_pour(self):
        for mutate in (
            lambda r: r["events"][0].update(sample_index=3),
            lambda r: r["events"][0].update(worker="other"),
            lambda r: r["events"].append(dict(r["events"][1], event_index=2)),
        ):
            raw = fixture()
            mutate(raw)
            with self.assertRaises(ValueError):
                water.check_lossless_chain(raw)

    def test_successful_pair_cannot_hide_a_later_unfinished_take(self):
        for found in (True, False):
            raw = fixture()
            # A second real conversion, with distinct consumed water, but no
            # corresponding pour. Raw observation is valid, supply is not.
            for index, original in ((4, 0), (5, 1)):
                sample = copy.deepcopy(raw["samples"][original])
                sample.update(sample_index=index, real_seconds=float(index + 1))
                context = sample["m2_context"]
                for item in context["items"]:
                    if item["entity"].startswith("water-"):
                        item["entity"] = "next-" + item["entity"]
                context["tanks"][0]["stored_items"] = [
                    "next-" + entity for entity in context["tanks"][0]["stored_items"]
                ]
                raw["samples"].append(sample)
            take = copy.deepcopy(raw["events"][0])
            take.update(
                sample_index=5,
                event_index=2,
                real_seconds=6.0,
                inputs=["next-" + entity for entity in take["inputs"]],
                route_found=found,
            )
            for item in take["inputs_before"]:
                item["entity"] = "next-" + item["entity"]
            raw["events"].append(take)
            lifecycle.check_raw_transitions(raw)
            with self.assertRaisesRegex(ValueError, "unmatched Tank take"):
                water.check_lossless_chain(raw)

    def test_prior_periodic_sample_is_not_the_branch_before_state(self):
        raw = fixture()
        ctx = raw["samples"][0]["m2_context"]
        ctx["bucket_actors"][0].update(phase="GoingToBucket", carried_item=None)
        ctx["items"][0].update(stored_in="companion")
        # Inputs may themselves have arrived after the last periodic snapshot.
        ctx["items"] = ctx["items"][:1]
        ctx["tanks"][0]["stored_items"] = []
        raw["samples"][2]["m2_context"]["bucket_actors"] = []
        water.check_lossless_chain(raw)

    def test_post_take_inventory_requires_explicit_typed_absence(self):
        for value in (False, 0, "", ["water-x", "water-x"], [None]):
            raw = fixture()
            raw["samples"][1]["m2_context"]["tanks"][0]["stored_items"] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                water.check_lossless_chain(raw)
        raw = fixture()
        del raw["samples"][1]["m2_context"]["tanks"][0]["stored_items"]
        with self.assertRaises(ValueError):
            water.check_lossless_chain(raw)
        raw = fixture()
        raw["samples"][1]["m2_context"]["tanks"][0]["stored_items"] = None
        water.check_lossless_chain(raw)

    def test_same_snapshot_inventory_is_bijective_even_with_other_deliveries(self):
        for sample_index, owner_key, owner_id in (
            (1, "tanks", "tank"),
            (3, "mixers", "mixer"),
        ):
            for defect in ("absent", "missing", "extra", "wrong-type", "duplicate"):
                raw = fixture()
                ctx = raw["samples"][sample_index]["m2_context"]
                owner = ctx[owner_key][0]
                # Water not involved in this branch may be legitimately present.
                ctx["items"].append(
                    dict(
                        entity="other-water",
                        resource="Water",
                        stored_in=owner_id,
                        loaded_in=None,
                    )
                )
                owner["stored_items"] = [*owner["stored_items"], "other-water"]
                lifecycle.check_raw_transitions(raw)
                if defect == "absent":
                    owner["stored_items"] = None
                elif defect == "missing":
                    owner["stored_items"].remove("other-water")
                elif defect == "extra":
                    owner["stored_items"].append("foreign-water")
                elif defect == "wrong-type":
                    ctx["items"][-1]["resource"] = "Rock"
                else:
                    ctx["items"].append(copy.deepcopy(ctx["items"][-1]))
                with (
                    self.subTest(owner=owner_id, defect=defect),
                    self.assertRaises(ValueError),
                ):
                    lifecycle.check_raw_transitions(raw)

    def test_branch_before_fields_cannot_be_missing_or_reinterpreted(self):
        for mutate in (
            lambda r: r["events"][0].pop("carried_before"),
            lambda r: r["events"][0].update(carried_before="foreign"),
            lambda r: r["events"][0].pop("inputs_before"),
            lambda r: r["events"][0]["inputs_before"].pop(),
            lambda r: r["events"][0]["inputs_before"][0].update(resource="Rock"),
            lambda r: r["events"][0]["inputs_before"][0].update(loaded_in="vehicle"),
            lambda r: r["events"][1].update(bucket_before="BucketEmpty"),
        ):
            raw = fixture()
            mutate(raw)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                water.check_lossless_chain(raw)
