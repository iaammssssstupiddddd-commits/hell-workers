"""Synthetic rejection tests, not ordinary-world or save-format acceptance."""
import copy
import hashlib
from pathlib import Path
import tempfile
import unittest

from scripts.tests.test_ui_rest_area_usage import OrdinaryRestUsageTests, usage


class OrdinaryRestSaveTests(unittest.TestCase):
    def fixture(self):
        raw, entries, client = OrdinaryRestUsageTests().fixture()
        payload = b"synthetic durable bytes, not a game save"
        before = copy.deepcopy(usage.context(entries[-1]["value"]))
        mapping = [{"kind": kind, "saved_entity": old, "saved_bits": bits,
                    "loaded_entity": new, "loaded_bits": new_bits}
                   for kind, old, bits, new, new_bits in (
                       ("RestArea", "building", 8, "new-building", 88),
                       ("Soul", "worker", 99, "new-worker", 199))]
        after = copy.deepcopy(before)
        after["owners"][0].update(entity="new-building", entity_bits=88, occupants=["new-worker"])
        after["souls"][0].update(entity="new-worker", entity_bits=199, resting_in="new-building")
        for index, phase, state in ((10, "saved", before), (11, "loaded", after)):
            state["real_seconds"] = float(index)
            sample = copy.deepcopy(raw["samples"][-1])
            sample.update(sample_index=index, real_seconds=float(index), world_epoch=0 if index == 10 else 1,
                          paused=True, rest_area_context=state)
            raw["samples"].append(sample)
            entry = copy.deepcopy(entries[-1])
            entry.update(case=f"rest-area-usage-{phase}", last_step=index * 5)
            value = entry["value"]
            value.update(frame=index * 10 + 100, world_epoch=sample["world_epoch"], catalog="Closed",
                         save_outcomes=["Save:Succeeded", "Load:Succeeded"])
            value["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(state)
            if phase == "loaded":
                value["rest_area_planning"]["buildings"] = [88]
                for cell in value["rest_area_planning"]["cells"]:
                    cell["owner"] = 88
            entries.append(entry)
            event = {"event_index": len(raw["events"]), "sample_index": index, "real_seconds": float(index),
                     "producer": f"ordinary-{'save' if phase == 'saved' else 'load'}-commit-v1",
                     "operation": "DurableSave" if phase == "saved" else "DurableLoad", "result": "Committed",
                     "target": "Manual slot 1", "world_epoch": sample["world_epoch"],
                     "request_source": "Manual(ManualCatalog { dialog_session: 1 })" if phase == "saved"
                     else "Load(NormalCatalog { dialog_session: 2 })", "file_bytes": len(payload),
                     "file_sha256": hashlib.sha256(payload).hexdigest(), "rest_area_context": copy.deepcopy(state)}
            if phase == "loaded":
                event.update(previous_epoch=0, entity_remap=mapping)
            raw["events"].append(event)
        return raw, entries, client, payload

    def verify(self, raw, entries, client, payload):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "saves" / "manual-1.scn.ron"
            path.parent.mkdir()
            path.write_bytes(payload)
            usage.check_save(raw, entries, client, path)

    def test_exact_durable_bytes_map_and_occupied_pair(self):
        self.verify(*self.fixture())

    def test_real_producer_field_contract_is_pinned_and_old_alias_rejected(self):
        repo = Path(__file__).resolve().parents[2]
        producer = (repo / "crates/bevy_app/src/systems/save/observations.rs").read_text()
        self.assertIn('"entity_remap": mapping', producer)
        raw, entries, client, payload = self.fixture()
        row = raw["events"][-1]
        row["mapping"] = row.pop("entity_remap")
        with self.assertRaises(RuntimeError):
            self.verify(raw, entries, client, payload)

    def test_case_is_bound_to_the_same_session_and_observation_pipeline(self):
        from scripts.tests import test_ui_rest_area_completion as base
        self.assertEqual(base.acceptance.session_matrix("rest-area-save", True),
                         [("rest-area-save", *base.acceptance.VIEWPORTS[0])])
        _, entries, _, _ = self.fixture()
        for entry in entries[-2:]:
            entry["value"].update(ready=True, task_rows=0)
            base.acceptance.check_observation(entry["case"], entry["value"])

    def test_changed_bytes_are_not_accepted_from_a_receipt(self):
        raw, entries, client, payload = self.fixture()
        with self.assertRaises(RuntimeError):
            self.verify(raw, entries, client, payload + b"changed")

    def test_rendered_frame_wait_is_not_a_license_to_skip_the_paused_interval(self):
        raw, entries, _, _ = self.fixture()
        receipt = raw["events"][-1]
        captured = copy.deepcopy(entries[-1]["value"])
        captured["rest_area_planning"]["rest_area_usage"]["real_seconds"] = 51.0
        for index in (12, 13):
            sample = copy.deepcopy(raw["samples"][-1])
            sample.update(sample_index=index, real_seconds=31.0 if index == 12 else 51.0)
            sample["rest_area_context"]["real_seconds"] = sample["real_seconds"]
            raw["samples"].append(sample)
        usage.check_receipt_interval(raw, receipt, captured, 88, "new-worker")
        for mode in ("pause", "epoch", "virtual", "state", "index", "clock"):
            bad = copy.deepcopy(raw)
            sample = bad["samples"][12]
            if mode == "state":
                sample["rest_area_context"]["souls"][0]["dream"] += 1
            elif mode == "clock":
                sample["rest_area_context"]["real_seconds"] -= 1
            else:
                key, invalid = {"pause": ("paused", False), "epoch": ("world_epoch", 2),
                                "virtual": ("virtual_seconds", 10), "index": ("sample_index", 999)}[mode]
                sample[key] = invalid
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                usage.check_receipt_interval(bad, receipt, captured, 88, "new-worker")

    def test_missing_duplicate_foreign_recovery_or_bad_receipt(self):
        for field, bad in (("producer", "invented"), ("result", "Failed"), ("target", "Manual slot 2"),
                           ("request_source", "Load(RecoveryCatalog { dialog_session: 2 })"),
                           ("file_sha256", "0" * 64), ("file_bytes", -1), ("world_epoch", 0),
                           ("previous_epoch", 99), ("sample_index", -1), ("real_seconds", float("nan"))):
            raw, entries, client, payload = self.fixture()
            raw["events"][-1][field] = bad
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.verify(raw, entries, client, payload)
        for mode in ("missing", "duplicate"):
            raw, entries, client, payload = self.fixture()
            if mode == "missing":
                raw["events"].pop()
            else:
                raw["events"].append(copy.deepcopy(raw["events"][-1]))
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.verify(raw, entries, client, payload)

    def test_actual_complete_map_not_names_or_coordinates(self):
        for mode in ("missing", "duplicate", "foreign", "kind", "bits", "same-id", "alias"):
            raw, entries, client, payload = self.fixture()
            mapping = raw["events"][-1]["entity_remap"]
            if mode == "missing":
                mapping.pop()
            elif mode == "duplicate":
                mapping.append(copy.deepcopy(mapping[0]))
            else:
                key, bad = {"foreign": ("saved_entity", "foreign"), "kind": ("kind", "Soul"),
                            "bits": ("loaded_bits", 999), "same-id": ("loaded_entity", "building"),
                            "alias": ("loaded_entity", "new-worker")}[mode]
                mapping[0][key] = bad
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.verify(raw, entries, client, payload)

    def test_durable_values_and_relationships_must_survive(self):
        raw, entries, _, _ = self.fixture()
        before = usage.context(entries[-2]["value"])
        after = usage.context(entries[-1]["value"])
        mapping = raw["events"][-1]["entity_remap"]
        for field, bad in (("dream", 0), ("fatigue", 0), ("stress", 0), ("idle", None),
                           ("visibility", "Visible"), ("position", [3, 2, 0]),
                           ("resting_in", "foreign"), ("reserved_for", "new-building"), ("cooldown", 15)):
            changed = copy.deepcopy(after)
            changed["souls"][0][field] = bad
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                usage.check_durable_mapping(before, changed, mapping)
        changed = copy.deepcopy(after)
        changed["owners"][0]["occupants"] = []
        with self.assertRaises(RuntimeError):
            usage.check_durable_mapping(before, changed, mapping)

    def test_only_non_resting_shell_visibility_is_projected_to_inherited(self):
        before = {"dream_pool": 1, "owners": [], "souls": [{"entity": "old", "entity_bits": 1,
                  "resting_in": None, "reserved_for": None, "visibility": "Visible", "dream": 2}]}
        after = copy.deepcopy(before)
        after["souls"][0].update(entity="new", entity_bits=2, visibility="Inherited")
        mapping = [{"kind": "Soul", "saved_entity": "old", "saved_bits": 1,
                    "loaded_entity": "new", "loaded_bits": 2}]
        usage.check_durable_mapping(before, after, mapping)
        for visibility in ("Visible", "Hidden", None):
            bad = copy.deepcopy(after)
            bad["souls"][0]["visibility"] = visibility
            with self.subTest(visibility=visibility), self.assertRaises(RuntimeError):
                usage.check_durable_mapping(before, bad, mapping)
        bad = copy.deepcopy(before)
        bad["souls"][0]["visibility"] = "Hidden"
        with self.assertRaises(RuntimeError):
            usage.check_durable_mapping(bad, after, mapping)
        changed = copy.deepcopy(after)
        changed["dream_pool"] += 1
        with self.assertRaises(RuntimeError):
            usage.check_durable_mapping(before, changed, mapping)

    def test_pause_request_epoch_outcome_and_target_are_required(self):
        for field, bad in (("paused", False), ("world_epoch", 0), ("catalog", "Confirm"),
                           ("save_outcomes", []), ("nonce", "foreign"), ("pid", -1), ("frame", 0)):
            raw, entries, client, payload = self.fixture()
            entries[-1]["value"][field] = bad
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.verify(raw, entries, client, payload)
        raw, entries, client, payload = self.fixture()
        entries[-1]["value"]["rest_area_planning"]["anchor"] = [0, 0]
        with self.assertRaises(RuntimeError):
            self.verify(raw, entries, client, payload)

    def test_receipt_and_raw_must_match_checkpoint_not_merely_contain_an_occupant(self):
        for mode in ("receipt", "raw", "time", "sample-epoch", "context-clock"):
            raw, entries, client, payload = self.fixture()
            if mode == "receipt":
                raw["events"][-1]["rest_area_context"]["souls"][0]["dream"] += 1
            elif mode == "raw":
                raw["samples"][-1]["rest_area_context"]["souls"][0]["dream"] += 1
            elif mode == "time":
                raw["events"][-1]["real_seconds"] -= 500
            elif mode == "context-clock":
                raw["events"][-1]["rest_area_context"]["real_seconds"] -= 1
            else:
                raw["samples"][-1]["world_epoch"] = 99
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.verify(raw, entries, client, payload)

    def test_symlink_save_is_rejected(self):
        raw, entries, client, payload = self.fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "saves" / "manual-1.scn.ron"
            path.parent.mkdir()
            target = root / "original"
            target.write_bytes(payload)
            path.symlink_to(target)
            with self.assertRaises(RuntimeError):
                usage.check_save(raw, entries, client, path)

    def lifecycle_fixture(self):
        raw, entries, client, payload = self.fixture()
        for index, virtual, dream, cooldown in ((12, 10.0, 4.75, None), (13, 11.0, 0.0, 14.75),
                                                (14, 12.0, 0.0, 14.5), (15, 19.0, 0.0, 7.5),
                                                (16, 27.0, 0.0, None)):
            sample = copy.deepcopy(raw["samples"][11])
            sample.update(sample_index=index, real_seconds=float(index + 10), virtual_seconds=virtual,
                          paused=index in (14, 16))
            sample["owners"][0]["entity"] = "new-building"
            state = sample["rest_area_context"]
            state.update(real_seconds=sample["real_seconds"], virtual_seconds=virtual,
                         dream_pool=5.5 + (9.5 - dream))
            row = state["souls"][0]
            row["dream"] = dream
            if index >= 13:
                row.update(resting_in=None, visibility="Visible", cooldown=cooldown)
                row["idle"]["behavior"] = "Wandering"
                state["owners"][0]["occupants"] = None
            raw["samples"].append(sample)
            if index in (12, 13):
                raw["events"].append({"event_index": len(raw["events"]), "sample_index": index,
                                      "real_seconds": sample["real_seconds"],
                                      "producer": "DreamTransferredVisualMessage", "operation": "DreamTransferred",
                                      "worker": "new-worker", "amount": 4.75, "is_final": index == 13,
                                      "source": {"kind": "RestArea", "owner": "new-building", "origin": [20.0, 20.0]}})
            if index in (14, 16):
                entry = copy.deepcopy(entries[-1])
                entry.update(case=usage.LIFECYCLE_CHECKPOINTS[-2 if index == 14 else -1], last_step=index * 5)
                entry["value"]["frame"] = index * 10 + 100
                entry["value"]["rest_area_planning"]["rest_area_usage"] = copy.deepcopy(state)
                entries.append(entry)
        return raw, entries, client, payload

    def verify_lifecycle(self, raw, entries, client, payload):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "saves" / "manual-1.scn.ron"
            path.parent.mkdir()
            path.write_bytes(payload)
            usage.check_lifecycle(raw, entries, client, path)

    def test_post_load_natural_exit_and_actual_cooldown_removal(self):
        original = self.lifecycle_fixture()
        snapshot = copy.deepcopy(original)
        self.verify_lifecycle(*original)
        self.assertEqual(original, snapshot)
        from scripts.tests import test_ui_rest_area_completion as base
        self.assertEqual(base.acceptance.session_matrix("rest-area-lifecycle", True),
                         [("rest-area-lifecycle", *base.acceptance.VIEWPORTS[0])])
        for entry in original[1][-2:]:
            value = copy.deepcopy(entry["value"])
            value.update(ready=True, task_rows=0)
            base.acceptance.check_observation(entry["case"], value)

    def test_cooldown_early_removal_entry_reset_and_foreign_time_are_rejected(self):
        for mode in ("early", "entry", "reset", "clock", "epoch", "generation", "index"):
            raw, entries, client, payload = self.lifecycle_fixture()
            sample = raw["samples"][15]
            row = sample["rest_area_context"]["souls"][0]
            if mode == "early":
                row["cooldown"] = None
            elif mode == "entry":
                row["resting_in"] = "new-building"
            elif mode == "reset":
                row["cooldown"] = 15.0
            elif mode == "generation":
                row["entity_bits"] += 1
            else:
                key, bad = {"clock": ("virtual_seconds", 5), "epoch": ("world_epoch", 2),
                            "index": ("sample_index", 1000)}[mode]
                sample[key] = bad
            with self.subTest(mode=mode), self.assertRaises(RuntimeError):
                self.verify_lifecycle(raw, entries, client, payload)

    def test_expiry_requires_a_full_paused_raw_context_not_just_a_missing_component(self):
        raw, entries, client, payload = self.lifecycle_fixture()
        raw["samples"][-1]["rest_area_context"]["dream_pool"] += 1
        with self.assertRaises(RuntimeError):
            self.verify_lifecycle(raw, entries, client, payload)

    def multiple_occupants_fixture(self):
        raw, entries, client, payload = self.lifecycle_fixture()
        original = copy.deepcopy(usage.context(entries[5]["value"])["souls"][0])
        for loaded, states in (
            (False, [sample["rest_area_context"] for sample in raw["samples"][7:11]]
             + [usage.context(entry["value"]) for entry in entries[3:6]]
             + [row["rest_area_context"] for row in raw["events"] if row.get("operation") == "DurableSave"]),
            (True, [sample["rest_area_context"] for sample in raw["samples"][11:]]
             + [usage.context(entry["value"]) for entry in entries[6:]]
             + [row["rest_area_context"] for row in raw["events"] if row.get("operation") == "DurableLoad"]),
        ):
            for state in states:
                other = copy.deepcopy(original)
                other.update(entity="other-new" if loaded else "other-old", entity_bits=200 if loaded else 100,
                             resting_in="new-building" if loaded else "building")
                state["souls"].append(other)
                occupants = state["owners"][0]["occupants"] or []
                state["owners"][0]["occupants"] = ([other["entity"]] + occupants if loaded
                                                    else occupants + [other["entity"]])
        next(row for row in raw["events"] if row.get("operation") == "DurableLoad")["entity_remap"].append(
            {"kind": "Soul", "saved_entity": "other-old", "saved_bits": 100,
             "loaded_entity": "other-new", "loaded_bits": 200})
        return raw, entries, client, payload

    def test_saved_occupant_remap_survives_loaded_occupant_order_changes(self):
        raw, entries, client, payload = self.multiple_occupants_fixture()
        self.assertEqual(usage.candidates(usage.context(entries[6]["value"]), 88)[0][1]["entity"], "other-new")
        self.verify_lifecycle(raw, entries, client, payload)

    def test_another_loaded_occupants_exit_cannot_replace_the_saved_selection(self):
        raw, entries, client, payload = self.multiple_occupants_fixture()
        swapped = {"new-worker": "other-new", "other-new": "new-worker"}
        bits = {"new-worker": 199, "other-new": 200}
        states = [sample["rest_area_context"] for sample in raw["samples"][12:]]
        states += [usage.context(entry["value"]) for entry in entries[7:]]
        for state in states:
            for row in state["souls"]:
                row["entity"] = swapped[row["entity"]]
                row["entity_bits"] = bits[row["entity"]]
            for owner in state["owners"]:
                if owner["occupants"] is not None:
                    owner["occupants"] = [swapped[entity] for entity in owner["occupants"]]
        for event in raw["events"]:
            if event.get("worker") == "new-worker":
                event["worker"] = "other-new"
        with self.assertRaises(RuntimeError):
            self.verify_lifecycle(raw, entries, client, payload)


if __name__ == "__main__":
    unittest.main()
