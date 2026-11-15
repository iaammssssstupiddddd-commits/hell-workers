"""Synthetic predicate tests, not ordinary-world acceptance evidence."""
from __future__ import annotations

import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

from scripts.tests import test_ui_rest_area_completion as base

usage = base.acceptance.rest_area_usage


class OrdinaryRestUsageTests(unittest.TestCase):
    def test_runner_pins_ready_soul_across_real_pause_drain_without_retry(self):
        _, entries, _ = self.fixture()
        for attack in ("none", "exited", "replacement", "insufficient"):
            ready = copy.deepcopy(entries[3]["value"])
            ready["paused"] = False
            ready["rest_area_planning"]["rest_area_usage"]["souls"][0]["dream"] = 2.05
            occupied = copy.deepcopy(ready)
            occupied["paused"] = True
            state = occupied["rest_area_planning"]["rest_area_usage"]
            state["souls"][0]["dream"] = 1.9
            if attack == "exited":
                state["owners"][0]["occupants"] = []
                state["souls"][0]["resting_in"] = None
            elif attack == "replacement":
                state["owners"][0]["occupants"] = ["foreign"]
                state["souls"][0]["entity"] = "foreign"
            elif attack == "insufficient":
                state["souls"][0]["dream"] = 0.5
            drained = copy.deepcopy(occupied)
            drained["rest_area_planning"]["rest_area_usage"]["souls"][0]["dream"] = 1.3
            driver = SimpleNamespace(last=copy.deepcopy(entries[2]["value"]), checkpoints=[])
            waits, inputs = [], []

            def wait(predicate, *, after, timeout):
                value = ready if not waits else drained
                waits.append(timeout)
                self.assertTrue(predicate(value))
                return value

            def pause(target, paused):
                inputs.append(paused)
                if paused:
                    target.last = copy.deepcopy(occupied if inputs.count(True) == 1 else drained)
                else:
                    target.last["paused"] = False

            def capture(case):
                driver.checkpoints.append({"case": case, "value": copy.deepcopy(driver.last)})

            driver.wait, driver.capture = wait, capture
            with self.subTest(attack=attack), patch.object(usage.planning, "exercise"), \
                    patch.object(usage.planning, "set_refund_paused", side_effect=pause), \
                    patch.object(usage, "wait_for_raw_checkpoint") as raw:
                if attack == "none":
                    usage.exercise(driver)
                    self.assertEqual(waits, [240, 15])
                    self.assertEqual(inputs, [False, True, False, True])
                    self.assertEqual([c["case"] for c in driver.checkpoints], list(usage.CHECKPOINTS[-2:]))
                    self.assertEqual([call.args[2] for call in raw.call_args_list], ["worker", "worker"])
                else:
                    with self.assertRaises(usage.native.AcceptanceError):
                        usage.exercise(driver)
                    self.assertEqual(waits, [240])
                    self.assertEqual(inputs, [False, True])
                    raw.assert_not_called()

    def test_capture_waits_for_raw_writer_without_relaxing_binding_or_sending_input(self):
        raw, entries, _ = self.fixture()
        stale = copy.deepcopy(raw)
        stale["samples"].pop()
        value = entries[-1]["value"]
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "production-cancellation.json").touch()

            def wait(predicate, *, after, timeout):
                self.assertEqual((after, timeout), (value["frame"], 15))
                self.assertFalse(predicate(value))
                self.assertTrue(predicate(value))

            driver = SimpleNamespace(root=root, checkpoints=entries, wait=wait)
            with patch.object(usage.native, "read_json", side_effect=[stale, raw]):
                usage.wait_for_raw_checkpoint(driver, 8, "worker")

            def timeout(predicate, **kwargs):
                self.assertFalse(predicate(value))
                raise usage.native.AcceptanceError("UI checkpoint timed out; input will not be resent")

            driver.wait = timeout
            with patch.object(usage.native, "read_json", return_value=stale), self.assertRaises(RuntimeError):
                usage.wait_for_raw_checkpoint(driver, 8, "worker")
            for field, invalid in (("nonce", "foreign"), ("process_id", -1), ("failure", "cap reached")):
                bad = copy.deepcopy(raw)
                bad[field] = invalid
                driver.wait = lambda predicate, **kwargs: predicate(value)
                with patch.object(usage.native, "read_json", return_value=bad), self.assertRaises(RuntimeError):
                    usage.wait_for_raw_checkpoint(driver, 8, "worker")
            for corrupt in ("timestamp", "relationship"):
                bad = copy.deepcopy(raw)
                if corrupt == "timestamp":
                    bad["samples"][-1]["real_seconds"] += 0.1
                else:
                    bad["samples"][-1]["rest_area_context"]["owners"][0]["occupants"] = []
                with patch.object(usage.native, "read_json", side_effect=[bad, raw]) as read:
                    with self.subTest(corrupt=corrupt), self.assertRaises(RuntimeError):
                        usage.wait_for_raw_checkpoint(driver, 8, "worker")
                    self.assertEqual(read.call_count, 1)

    def test_actual_usage_helper_content_changes_the_bound_harness_hash(self):
        from scripts.perf_tool import execution, renderdoc_capture
        relative = ".codex/skills/hell-workers-run-native-acceptance/scripts/ui_rest_area_usage.py"
        for inventory in (base.acceptance.native.NATIVE_HARNESS_FILES,
                          execution.MEASUREMENT_HARNESS_FILES,
                          renderdoc_capture.MEASUREMENT_HARNESS_FILES):
            self.assertEqual(inventory.count(relative), 1)
        original = (Path(__file__).resolve().parents[2] / relative).read_bytes()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            helper = root / relative
            helper.parent.mkdir(parents=True)
            helper.write_bytes(original)
            with patch.object(base.acceptance.native, "NATIVE_HARNESS_FILES", (relative,)):
                before = base.acceptance.native.native_harness_fingerprint(root)
                helper.write_bytes(original + b"\n# deliberate test-only content mutation\n")
                self.assertNotEqual(before, base.acceptance.native.native_harness_fingerprint(root))

    def fixture(self):
        raw, entries, client = base.OrdinaryCompletionTests().fixture()
        for index in range(7, 10):
            step = index - 7
            state = {"real_seconds": float(index), "virtual_seconds": float(index),
                     "dream_pool": 5.0 + step * 0.25,
                     "owners": [{"entity": "building", "entity_bits": 8,
                                 "building_kind": "RestArea", "provisional": False, "capacity": 4,
                                 "occupants": ["worker"], "reservations": None}],
                     "souls": [{"entity": "worker", "entity_bits": 99, "dream": 10.0 - step * 0.25,
                                "fatigue": 0.4, "stress": 0.2, "position": [1.0, 2.0, 0.0],
                                "visibility": "Hidden", "walkable": True,
                                "resting_in": "building", "reserved_for": None, "cooldown": None,
                                "idle": {"behavior": "Resting", "timer": float(step), "total_idle_time": 2.0}}]}
            sample = copy.deepcopy(raw["samples"][6])
            sample.update(sample_index=index, real_seconds=float(index), virtual_seconds=float(index),
                          paused=index != 8, rest_area_context=state)
            sample["owners"][0]["position"] = [20.0, 20.0, 0.0]
            sample["actors"][0]["task_identity"] = None
            raw["samples"].append(sample)
            if index in (7, 9):
                entry = copy.deepcopy(entries[2])
                entry.update(case=usage.CHECKPOINTS[-2 if index == 7 else -1], last_step=15 if index == 7 else 20)
                entry["value"]["frame"] = 130 if index == 7 else 140
                entry["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(state)
                entries.append(entry)
            if index > 7:
                raw["events"].append({"event_index": len(raw["events"]), "sample_index": index,
                                      "real_seconds": float(index), "producer": "DreamTransferredVisualMessage",
                                      "operation": "DreamTransferred", "worker": "worker", "amount": 0.25,
                                      "is_final": False,
                                      "source": {"kind": "RestArea", "owner": "building", "origin": [20.0, 20.0]}})
        return raw, entries, client

    def test_same_soul_rest_admission_drain_and_pool_witnesses_are_read_only(self):
        raw, entries, client = self.fixture()
        original = copy.deepcopy((raw, entries, client))
        usage.check_usage(raw, entries, client)
        self.assertEqual((raw, entries, client), original)
        self.assertEqual(base.acceptance.session_matrix("rest-area-usage", True),
                         [("rest-area-usage", *base.acceptance.VIEWPORTS[0])])

    def test_missing_foreign_stale_or_unreconciled_usage_is_rejected(self):
        mutations = (
            lambda r, e: r.update(nonce="foreign"),
            lambda r, e: r.update(accepted=True),
            lambda r, e: r.update(promotion_authority=True),
            lambda r, e: e.pop(),
            lambda r, e: e[3]["value"].update(pid=321),
            lambda r, e: e[3]["value"].update(world_epoch=1),
            lambda r, e: e[3]["value"].update(paused=False),
            lambda r, e: e[4].update(last_step=10),
            lambda r, e: e[4]["value"].update(frame=120),
            lambda r, e: e[4]["value"]["rest_area_planning"].update(anchor=[1, 2]),
            lambda r, e: e[3]["value"]["rest_area_planning"]["rest_area_usage"].update(failure="cap reached"),
            lambda r, e: e[3]["value"]["rest_area_planning"]["rest_area_usage"].update(real_seconds=float("nan")),
            lambda r, e: e[3]["value"]["rest_area_planning"]["rest_area_usage"].update(dream_pool=None),
            lambda r, e: r["samples"][7].update(paused=False),
            lambda r, e: r["samples"][8].update(world_epoch=1),
            lambda r, e: r["samples"][8].update(virtual_seconds=float("nan")),
            lambda r, e: r["samples"][7]["rest_area_context"].update(real_seconds=700),
            lambda r, e: r["samples"][7]["rest_area_context"].update(virtual_seconds=700),
            lambda r, e: r["samples"][8]["rest_area_context"]["owners"][0].update(occupants=[]),
            lambda r, e: r["samples"][8]["rest_area_context"]["owners"][0].update(occupants=["worker", "worker"]),
            lambda r, e: r["samples"][8]["rest_area_context"]["owners"][0].update(reservations=["worker"]),
            lambda r, e: r["samples"][8]["rest_area_context"]["owners"][0].update(provisional=True),
            lambda r, e: r["samples"][8]["rest_area_context"]["owners"][0].update(capacity=0),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(resting_in="foreign"),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(visibility="Visible"),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(walkable=False),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(entity_bits=100),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(position=[3.0, 4.0, 0.0]),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(dream=float("nan")),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0].update(cooldown=15.0),
            lambda r, e: r["samples"][8]["rest_area_context"]["souls"][0]["idle"].update(behavior="Sleeping"),
            lambda r, e: r["samples"][8]["rest_area_context"].update(dream_pool=float("nan")),
            lambda r, e: r["samples"][8]["rest_area_context"].update(real_seconds=float("nan")),
            lambda r, e: r["samples"][8]["rest_area_context"].update(virtual_seconds=7.0),
            lambda r, e: r["events"].pop(),
            lambda r, e: r["events"][-1].update(worker="foreign"),
            lambda r, e: r["events"][-1].update(operation="inferred"),
            lambda r, e: r["events"][-1].update(amount=0),
            lambda r, e: r["events"][-1].update(amount=float("nan")),
            lambda r, e: r["events"][-1].update(is_final=True),
            lambda r, e: r["events"][-1].update(real_seconds=10.0),
            lambda r, e: r["events"][-1].update(sample_index=8),
            lambda r, e: r["events"][-1]["source"].update(kind="Sleeping"),
            lambda r, e: r["events"][-1]["source"].update(owner="foreign"),
            lambda r, e: r["events"][-1]["source"].update(origin=[21.0, 20.0]),
        )
        for mutate in mutations:
            raw, entries, client = self.fixture()
            mutate(raw, entries)
            with self.subTest(mutation=mutate), self.assertRaises((RuntimeError, ValueError)):
                usage.check_usage(raw, entries, client)

    def test_rebound_checkpoints_cannot_hide_wrong_amount_pool_or_generation(self):
        mutations = (
            lambda r: r["samples"][9]["rest_area_context"].update(dream_pool=5.1),
            lambda r: r["samples"][9]["rest_area_context"]["souls"][0].update(dream=9.0),
            lambda r: r["samples"][9]["rest_area_context"]["souls"][0].update(dream=9.9),
            lambda r: r["samples"][9]["rest_area_context"]["souls"][0].update(entity_bits=100),
            lambda r: r["samples"][9]["rest_area_context"]["souls"][0].update(position=[3.0, 4.0, 0.0]),
        )
        for mutate in mutations:
            raw, entries, client = self.fixture()
            mutate(raw)
            entries[4]["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(
                raw["samples"][9]["rest_area_context"])
            with self.subTest(mutation=mutate), self.assertRaises((RuntimeError, ValueError)):
                usage.check_usage(raw, entries, client)

    def test_soul_only_rest_and_reservation_references_are_rejected(self):
        for relationship in ("resting_in", "reserved_for"):
            raw, entries, client = self.fixture()
            extra = copy.deepcopy(raw["samples"][8]["rest_area_context"]["souls"][0])
            extra.update(entity="extra-soul", entity_bits=100, resting_in=None, reserved_for=None)
            extra[relationship] = "building"
            raw["samples"][8]["rest_area_context"]["souls"].append(extra)
            with self.subTest(relationship=relationship), self.assertRaises(RuntimeError):
                usage.check_usage(raw, entries, client)

    def exit_fixture(self):
        raw, entries, client = self.fixture()
        for index in range(10, 13):
            sample = copy.deepcopy(raw["samples"][9])
            sample.update(sample_index=index, real_seconds=float(index), virtual_seconds=float(index), paused=index == 12)
            state = sample["rest_area_context"]
            state.update(real_seconds=float(index), virtual_seconds=float(index), dream_pool=5.5 + min(index - 9, 2) * 4.75)
            row = state["souls"][0]
            row["dream"] = 4.75 if index == 10 else 0.0
            if index >= 11:
                row.update(resting_in=None, visibility="Visible", cooldown=15.0 - (index - 10) * 0.25)
                row["idle"]["behavior"] = "Wandering"
                state["owners"][0]["occupants"] = None
            raw["samples"].append(sample)
            if index < 12:
                event = copy.deepcopy(raw["events"][-1])
                event.update(event_index=len(raw["events"]), sample_index=index, real_seconds=float(index),
                             amount=4.75, is_final=index == 11)
                raw["events"].append(event)
        entry = copy.deepcopy(entries[-1])
        entry.update(case=usage.EXIT_CHECKPOINTS[-1], last_step=25)
        entry["value"]["frame"] = 150
        entry["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(raw["samples"][-1]["rest_area_context"])
        entries.append(entry)
        return raw, entries, client

    def test_natural_exhaustion_exit_and_active_cooldown_are_read_only(self):
        raw, entries, client = self.exit_fixture()
        original = copy.deepcopy((raw, entries, client))
        usage.check_exit(raw, entries, client)
        self.assertEqual((raw, entries, client), original)
        self.assertEqual(base.acceptance.session_matrix("rest-area-exit", True),
                         [("rest-area-exit", *base.acceptance.VIEWPORTS[0])])

    def test_missing_foreign_final_or_broken_exit_is_rejected(self):
        mutations = (
            lambda r, e: e.pop(),
            lambda r, e: e[-1]["value"].update(pid=-1),
            lambda r, e: e[-1]["value"].update(world_epoch=1),
            lambda r, e: e[-1]["value"].update(paused=False),
            lambda r, e: e[-1].update(last_step=20),
            lambda r, e: r["samples"][10]["rest_area_context"]["souls"][0].update(position=[7, 7, 0]),
            lambda r, e: r["samples"][11]["rest_area_context"]["souls"][0].update(entity_bits=100),
            lambda r, e: r["samples"][11]["rest_area_context"]["souls"][0].update(cooldown=None),
            lambda r, e: r["samples"][11]["rest_area_context"]["souls"][0].update(visibility="Hidden"),
            lambda r, e: r["samples"][11]["rest_area_context"]["souls"][0].update(walkable=False),
            lambda r, e: r["samples"][11]["rest_area_context"]["owners"][0].update(occupants=["worker"]),
            lambda r, e: r["samples"][11].update(world_epoch=1),
            lambda r, e: r["samples"][11].update(virtual_seconds=float("nan")),
            lambda r, e: r["samples"][11]["rest_area_context"]["souls"][0].update(cooldown=10),
            lambda r, e: r["events"][-1].update(is_final=False),
            lambda r, e: r["events"][-1].update(amount=1),
            lambda r, e: r["events"][-1].update(worker="foreign"),
            lambda r, e: r["events"][-1].update(real_seconds=13),
            lambda r, e: r["events"][-1].update(sample_index=12, real_seconds=12),
            lambda r, e: r["events"][-1]["source"].update(kind="Sleeping"),
            lambda r, e: r["events"][-1]["source"].update(owner="foreign"),
            lambda r, e: r["events"][-1]["source"].update(origin=[21, 20]),
        )
        for mutate in mutations:
            raw, entries, client = self.exit_fixture()
            mutate(raw, entries)
            with self.subTest(mutation=mutate), self.assertRaises((RuntimeError, ValueError)):
                usage.check_exit(raw, entries, client)

    def test_rebound_exit_cannot_hide_retained_dream_or_missing_pool_growth(self):
        for corrupt in ("dream", "dream_pool"):
            raw, entries, client = self.exit_fixture()
            for sample in raw["samples"][10:]:
                if corrupt == "dream_pool":
                    sample["rest_area_context"]["dream_pool"] = 5.5
                elif sample["sample_index"] >= 11:
                    sample["rest_area_context"]["souls"][0]["dream"] = 5.0
            entries[-1]["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(
                raw["samples"][-1]["rest_area_context"])
            with self.subTest(corrupt=corrupt), self.assertRaises(RuntimeError):
                usage.check_exit(raw, entries, client)


if __name__ == "__main__":
    unittest.main()
