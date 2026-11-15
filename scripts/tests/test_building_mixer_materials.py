"""Synthetic branch records only, never native or whole supply acceptance."""

import copy
import unittest
from scripts import building_production_lifecycle as lifecycle


def fixture(wheel=False, resource="Sand"):
    event = dict(
        producer="ordinary-mixer-material-branch-v1",
        operation="MixerMaterial",
        sample_index=0,
        event_index=0,
        real_seconds=1.0,
        world_epoch=0,
        worker="soul",
        assignment="request",
        target="mixer" if wheel else "input",
        work_type="WheelbarrowHaul" if wheel else "HaulToMixer",
        owner="mixer",
        item="input",
        resource=resource,
        actual_resource=resource,
        stored_before=None,
        loaded_before="wheel" if wheel else None,
        carried_before="wheel" if wheel else "input",
        wheelbarrow="wheel" if wheel else None,
        counter_order=["Sand", "Rock", "StasisMud"],
        before=[1, 2, 3],
        after=[2, 2, 3] if resource == "Sand" else [1, 3, 3],
    )
    sample = dict(
        sample_index=0,
        real_seconds=1.0,
        world_epoch=0,
        actors=[dict(entity="soul")],
        m2_context=dict(
            schema=1,
            mixer_material_channel=True,
            accepted=False,
            performance_evidence=False,
            promotion_authority=False,
            mixers=[
                dict(
                    entity="mixer",
                    building=dict(kind="MudMixer", provisional=False),
                    sand=4,
                    rock=1,
                    mud=8,
                )
            ],
            items=[],
        ),
        **{key: [] for key in lifecycle.TRANSITION_CATEGORIES},
    )
    return dict(
        schema_version=2,
        transition_contract="indexed-state-delta-v1",
        samples=[sample],
        events=[event],
    )


class MixerMaterialTests(unittest.TestCase):
    def test_hand_and_wheelbarrow_record_branch_not_later_counter_totals(self):
        for wheel in (False, True):
            for resource in ("Sand", "Rock"):
                lifecycle.check_raw_transitions(fixture(wheel, resource))

    def test_foreign_malformed_unapplied_and_authoritative_records_rejected(self):
        for mutate in (
            lambda r: r["events"][0].update(world_epoch=True),
            lambda r: r["events"][0].update(sample_index=True),
            lambda r: r["events"][0].update(real_seconds=float("nan")),
            lambda r: r["events"][0].update(real_seconds=2.0),
            lambda r: r["events"][0].update(resource="Water"),
            lambda r: r["events"][0].update(actual_resource=None),
            lambda r: r["events"][0].update(stored_before="warehouse"),
            lambda r: r["events"][0].update(carried_before=None),
            lambda r: r["events"][0].update(loaded_before="wheel"),
            lambda r: r["events"][0].update(work_type="Refine"),
            lambda r: r["events"][0].update(before=[True, 2, 3]),
            lambda r: r["events"][0].update(after=[3, 2, 3]),
            lambda r: r["events"][0].update(before=[5, 2, 3], after=[6, 2, 3]),
            lambda r: r["events"][0].update(after=[2, 2, 4]),
            lambda r: r["events"][0].pop("loaded_before"),
            lambda r: r["samples"][0]["m2_context"]["items"].append(
                dict(entity="input")
            ),
            lambda r: r["samples"][0]["m2_context"]["mixers"][0]["building"].update(
                provisional=True
            ),
            lambda r: r["samples"][0]["m2_context"].update(
                mixer_material_channel=False
            ),
            lambda r: r["samples"][0]["m2_context"].update(accepted=True),
            lambda r: r["samples"][0].update(actors=[]),
        ):
            raw = fixture()
            mutate(raw)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(raw)

    def test_wheelbarrow_requires_exact_loaded_and_carried_identity(self):
        for field, value in (
            ("loaded_before", None),
            ("carried_before", "other"),
            ("wheelbarrow", False),
            ("wheelbarrow", "soul"),
            ("work_type", "HaulToMixer"),
        ):
            raw = fixture(True)
            raw["events"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions(raw)

    def test_reused_item_cannot_count_as_a_second_delivery(self):
        raw = fixture()
        duplicate = copy.deepcopy(raw["events"][0])
        duplicate["event_index"] = 1
        raw["events"].append(duplicate)
        with self.assertRaisesRegex(ValueError, "receipt reused"):
            lifecycle.check_raw_transitions(raw)
