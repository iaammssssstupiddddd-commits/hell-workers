"""Offline rejection contract tests; no native launch, asset approval or promotion."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_production_acceptance as production
from scripts import building_production_lifecycle as lifecycle


class ProductionAcceptanceTests(unittest.TestCase):
    @staticmethod
    def unavailable_host_fixture(root):
        # Public-schema rejection fixtures, never authenticated admissions or
        # successful production recipes. No host_admission marker is fabricated.
        subject = {"binding": {"scope_sha256": "a" * 64, "node_id": "full-acceptance"},
                   "run": {"id": "fixture-run", "consumer_generation": 1}, "generation": 12,
                   "repo": str(root), "head": "b" * 40, "source": "c" * 64}
        context = {"controller_sha256": "d" * 64, "recipe_revision": "e" * 64,
                   "bridge_revision": "f" * 64, "subject": subject, "owner": "fixture coordinator"}
        capabilities = {"ok": True, "revision": "e" * 64,
                        "contracts": {"schema": 1, "recipes": []},
                        "registration": {"schema": 1, "controller_sha256": "d" * 64,
                                         "actions": ["registration-context", "register", "registration-status"]}}

        def document(name, result):
            payload = production.pipeline.canonical({"ok": True, "result": result})
            path = root / name
            path.write_bytes(payload)
            return {"path": str(path), "sha256": production.pipeline.digest(payload)}

        intent = {"action": "register", "spec": {"id": "fixture", "repo": str(root),
                  "owner": context["owner"], "consumers": [], "roots": [], "verify_command": []},
                  "command": ["not-a-declared-production-recipe", "保存"], "subject": copy.deepcopy(subject),
                  "controller_sha256": context["controller_sha256"], "recipe_revision": context["recipe_revision"]}
        value = {"repo": str(root), "subject": {"commit": subject["head"], "source": subject["source"]},
                 "registration_request": "00000000-0000-0000-0000-000000000001",
                 "registration_context": document("context.json", context),
                 "host_capabilities": document("capabilities.json", capabilities), "registration_intent": intent}
        receipt = {"batch": "fixture", "phase": "fixture", "sha256": "a" * 64,
                   "registration": {"schema": 1, "request": value["registration_request"], "intent": intent,
                       "intent_sha256": production.pipeline.digest(json.dumps(
                           intent, sort_keys=True, separators=(",", ":")).encode()),
                       "subject": subject, "controller_sha256": context["controller_sha256"],
                       "recipe_revision": context["recipe_revision"]}}
        return value, receipt

    def test_host_public_records_never_admit_an_unavailable_production_recipe(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            value, receipt = self.unavailable_host_fixture(root)
            with self.assertRaisesRegex(ValueError, "recipe/instruments unavailable"):
                production.check_registration_receipt(value, receipt)
            with self.assertRaisesRegex(ValueError, "recipe/instruments unavailable"):
                production.plan(value)
            plan_path = root / "plan.json"
            plan_path.write_bytes(production.pipeline.canonical(value))
            with patch.object(production, "inspect_performance") as performance:
                with self.assertRaisesRegex(ValueError, "recipe/instruments unavailable"):
                    production.verify_results(plan_path, root / "nonexistent-result.json")
                performance.assert_not_called()
            self.assertFalse((root / "nonexistent-result.json").exists())
            for capabilities_change in ("controller", "actions", "revision", "invented-recipe"):
                altered = copy.deepcopy(value)
                path = Path(value["host_capabilities"]["path"])
                envelope = json.loads(path.read_bytes())
                if capabilities_change == "controller":
                    envelope["result"]["registration"]["controller_sha256"] = "0" * 64
                elif capabilities_change == "actions":
                    envelope["result"]["registration"]["actions"] = ["launch"]
                elif capabilities_change == "revision":
                    envelope["result"]["revision"] = None
                else:
                    envelope["result"]["contracts"]["recipes"] = [{"id": "invented-production", "authenticated": True}]
                payload = production.pipeline.canonical(envelope)
                changed_path = root / (capabilities_change + ".json")
                changed_path.write_bytes(payload)
                altered["host_capabilities"] = {"path": str(changed_path), "sha256": production.pipeline.digest(payload)}
                with self.subTest(capability=capabilities_change), self.assertRaises(ValueError):
                    production.plan(altered)

    def test_host_registration_context_and_receipt_mismatches_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            value, receipt = self.unavailable_host_fixture(Path(directory))
            for path, replacement in [
                (("registration_intent", "controller_sha256"), "0" * 64),
                (("registration_intent", "recipe_revision"), "0" * 64),
                (("registration_intent", "subject", "generation"), 13),
                (("registration_intent", "subject", "run", "id"), "foreign-run"),
                (("registration_intent", "subject", "binding", "scope_sha256"), "0" * 64),
                (("registration_intent", "action"), "launch"),
                (("registration_intent", "spec", "owner"), "foreign-owner"),
                (("subject", "commit"), "0" * 40), (("subject", "source"), "0" * 64),
                (("registration_intent", "command"), []),
                (("registration_context", "sha256"), "0" * 64),
                (("host_capabilities", "sha256"), "0" * 64),
            ]:
                altered = copy.deepcopy(value)
                target = altered
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = replacement
                with self.subTest(path=path), self.assertRaises(ValueError):
                    production.require_production_registration(altered)
            for field in receipt["registration"]:
                altered = copy.deepcopy(receipt)
                altered["registration"][field] = "foreign"
                with self.subTest(receipt_field=field), self.assertRaisesRegex(ValueError, "receipt differs"):
                    production.check_registration_receipt(value, altered)
            altered = copy.deepcopy(receipt)
            altered["registration"]["intent_sha256"] = production.pipeline.digest(
                production.pipeline.canonical(value["registration_intent"]))
            with self.assertRaisesRegex(ValueError, "receipt differs"):
                production.check_registration_receipt(value, altered)
            for forged in ({"registry_export": "old.json", "registry_sha256": "a" * 64},
                           {"authenticated": True, "launchable": True}):
                with self.assertRaises(ValueError):
                    production.plan(forged)

    @staticmethod
    def domain_verifier_fixture(kind, leg):
        # Transport is replaced below, but raw binding and every lifecycle
        # predicate on this leg run unchanged through verify(). No native claim.
        identity = {"manifest_sha256": "a" * 64}
        other = {"manifest_sha256": "b" * 64}
        phases = ["active-A", "failed-B-retains-A", "active-C", "fallback", "owner-removed"]
        count = 5 if leg == "cleanup" else 2
        samples = [{"real_seconds": float(i + 1), "sample_index": i,
                    **{category: [] for category in lifecycle.TRANSITION_CATEGORIES},
                    "identity": identity, "consumers": []} for i in range(count)]
        if leg == "cleanup":
            for sample, phase, ident in zip(samples, phases, [identity, identity, other, None, None]):
                sample.update(generation_phase=phase, identity=ident, resource_counts={"roots": 0})
        if kind == "Bridge":
            samples[-1].update(salvage={"Rock": 3}, restored_cells=[
                {"river": True, "walkable": False, "bridged": False, "owner": None} for _ in range(10)])
        events = [
            {"event_index": 0, "sample_index": 0, "producer": "TaskCompletedVisualMessage",
             "operation": "TaskCompleted", "target": "owner", "worker": "worker", "assignment": "task", "work_type": "Build"},
            {"event_index": 1, "sample_index": count - 1, "producer": "DeconstructionCommitOutcome",
             "operation": "Deconstruct", "result": "Committed", "owner": "owner", "worker": "worker", "order": "order"},
            {"event_index": 2, "sample_index": count - 1, "producer": "TaskCompletedVisualMessage",
             "operation": "TaskCompleted", "target": "foreign", "worker": "worker", "assignment": "task", "work_type": "Build"},
            {"event_index": 3, "sample_index": count - 1, "operation": "PointerPress"},
        ]
        witness = {"before": 0, "after": count - 1, "owner": "owner",
                   "events": [0, 1, 2, 3], "domain_event_indices": [1]}
        trace = {"promotion_authority": False, "schema_version": 2,
                 "transition_contract": "indexed-state-delta-v1", "samples": samples, "events": events,
                 "owner": "owner", "identity": identity, "leg": leg,
                 "scope": "bridge-normal-world-observations-only" if kind == "Bridge" else "production-normal-world-observations-v1",
                 "before_gameplay_contract_sha256": "c" * 64, "after_gameplay_contract_sha256": "c" * 64}
        if leg == "cleanup":
            witness.update(before_counts={"roots": 0}, after_counts={"roots": 0}, phases=phases,
                           retired_application_handles=0, orphan_parts=0)
            trace["cycles"] = [copy.deepcopy(witness) for _ in range(10)]
        else:
            if kind == "Bridge":
                witness["case"] = "river-and-dry-restoration"
            trace["witnesses"] = [witness]
        row = {"kind": kind, "leg": leg, "instrument": "Capture",
               "captures": [{"sample_index": 0}, {"sample_index": count - 1}]}
        value = {"scope": "fixture", "releases": {kind: {"identity": identity}},
                 "generation_trials": {kind: [{"identity": identity}, {"identity": other}]},
                 "gameplay_contract_sha256": "c" * 64}
        return trace, row, value

    def test_verify_requires_domain_binding_for_bridge_group_and_every_cycle(self):
        for kind, leg in (("Bridge", "deconstruct"), ("Tank", "deconstruct"), ("Tank", "cleanup")):
            raw, row, value = self.domain_verifier_fixture(kind, leg)

            def session(*args, **kwargs):
                lifecycle.check_raw_transitions(raw)
                normalized = copy.deepcopy(raw)
                for i, sample in enumerate(normalized["samples"]):
                    sample["raw_sample_index"] = i
                production.bind_lifecycle_observations(normalized, raw)
                return normalized

            # Isolate a single required leg, not its verifier or guard; session
            # supplies losslessly bound raw evidence instead of host IO/seals.
            with patch.dict(production.GROUPS, {"fixture": (kind,)}), \
                    patch.dict(lifecycle.LEGS, {kind: (leg,)}), \
                    patch.object(production, "session", side_effect=session):
                lifecycle.verify(None, [row], value, "plan", set())
                witnesses = raw["cycles" if leg == "cleanup" else "witnesses"]
                for ordinal, witness in enumerate(witnesses):
                    for invalid in (None, [], [0], [1, 1], [2], [1, 2], [3], [1, 3], [-1], [4]):
                        with self.subTest(kind=kind, leg=leg, cycle=ordinal, indices=invalid):
                            if invalid is None:
                                del witness["domain_event_indices"]
                            else:
                                witness["domain_event_indices"] = invalid
                            with self.assertRaises(ValueError):
                                lifecycle.verify(None, [row], value, "plan", set())
                            witness["domain_event_indices"] = [1]

    def test_verify_save_load_labels_require_separate_selected_owner_evidence(self):
        original, row, value = self.domain_verifier_fixture("Tank", "save-load")
        original["samples"][0]["owners"] = [{"entity": "owner"}]
        for i, sample in enumerate(original["samples"]):
            sample.update(world_epoch=i + 1, old_world_references=0,
                          durable_state={"building_kind": "Tank", "water": 3})
        original["events"] = [
            {"event_index": i, "sample_index": 1, "real_seconds": 2.0,
             "producer": "SaveLoadOutcome", "operation": operation,
             "result": "Succeeded", "target": "Quick Save", "source": "Quick"}
            for i, operation in enumerate(("Save", "Load"))]
        original["events"].append({
            "event_index": 2, "sample_index": 1, "before_sample_index": 0,
            "real_seconds": 2.0, "producer": "production_snapshot_delta",
            "operation": "StateTransition", "category": "owners", "entity": "owner",
            "before": {"entity": "owner"}, "after": None})
        original["witnesses"] = [{"before": 0, "after": 1, "owner": "owner",
                                   "events": [0, 1, 2], "domain_event_indices": [0, 1, 2]}]
        raw = copy.deepcopy(original)

        def session(*args, **kwargs):
            lifecycle.check_raw_transitions(raw)
            normalized = copy.deepcopy(raw)
            for i, sample in enumerate(normalized["samples"]):
                sample["raw_sample_index"] = i
            production.bind_lifecycle_observations(normalized, raw)
            return normalized

        # Exercise the public lifecycle entrypoint and real save/load predicates;
        # host IO/seals and the required leg set are the only replacements.
        with patch.dict(production.GROUPS, {"fixture": ("Tank",)}), \
                patch.dict(lifecycle.LEGS, {"Tank": ("save-load",)}), \
                patch.object(production, "session", side_effect=session):
            lifecycle.verify(None, [row], value, "plan", set())
            for invalid in ("unselected-owner", "foreign-owner", "label-equals-owner", "missing-save"):
                raw = copy.deepcopy(original)
                if invalid in {"unselected-owner", "label-equals-owner"}:
                    raw["witnesses"][0]["domain_event_indices"] = [0, 1]
                    if invalid == "label-equals-owner":
                        for event in raw["events"][:2]:
                            event["target"] = "owner"
                elif invalid == "foreign-owner":
                    # Remain consistent with raw state so rejection must come
                    # from owner correlation, not a malformed delta.
                    raw["samples"][0]["owners"][0]["entity"] = "foreign"
                    raw["events"][2]["entity"] = "foreign"
                    raw["events"][2]["before"]["entity"] = "foreign"
                else:
                    raw["witnesses"][0]["domain_event_indices"] = [1, 2]
                with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                    lifecycle.verify(None, [row], value, "plan", set())

    def test_authoritative_completion_witness_keeps_exact_blueprint_and_built_owner(self):
        samples = [{"sample_index": i, "real_seconds": float(i),
                    **{category: [] for category in lifecycle.TRANSITION_CATEGORIES}} for i in range(2)]
        event = {"producer": "hw_jobs::publish_building_completed", "operation": "BuildingCompleted",
                 "blueprint_owner": "blueprint", "owner": "built-owner", "kind": "Bridge",
                 "sample_index": 1, "event_index": 0}
        raw = {"schema_version": 2, "transition_contract": "indexed-state-delta-v1",
               "samples": samples, "events": [event]}
        lifecycle.check_raw_transitions(raw)
        witness = {"owner": "blueprint", "before": 0, "after": 1,
                   "events": [0], "domain_event_indices": [0]}
        lifecycle.check_domain_witness(witness, raw)
        with self.assertRaises(ValueError):
            lifecycle.check_domain_witness({**witness, "owner": "unrelated"}, raw)
        for change in ({"producer": "driver"}, {"operation": "Placement"}, {"owner": None}, {"event_index": 7}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                lifecycle.check_raw_transitions({**raw, "events": [{**event, **change}]})

    def test_external_gpu_and_handle_evidence_requires_exact_subject_and_measurements(self):
        row = dict.fromkeys(("subject", "plan_sha256", "binary_sha256", "codec_sha256", "driver_sha256",
                             "nonce", "pid", "environment"), "pinned")
        samples = [{"seconds": 1.0, "metrics": {"application_handle_count": 4,
                    "gpu_measured_bytes": 128, "gpu_estimated_bytes": 160},
                    "retired_application_handles": 0, "gpu_measurement_source": "external-profiler"}]
        gpu = {"session": row, "samples": [{"seconds": 1.0, "measured_bytes": 128,
                                           "estimated_bytes": 160, "source": "external-profiler"}]}
        handles = {"session": row, "samples": [{"seconds": 1.0, "application_handle_count": 4,
                                               "retired_application_handles": 0}]}
        production.bind_external_resources(row, samples, gpu, handles)
        for field in row:
            altered = copy.deepcopy(gpu)
            altered["session"][field] = "stale"
            with self.subTest(field=field), self.assertRaises(ValueError):
                production.bind_external_resources(row, samples, altered, handles)
        for absent in ({}, {"session": row}, {"session": row, "samples": []}):
            with self.assertRaises(ValueError):
                production.bind_external_resources(row, samples, absent, handles)
            with self.assertRaises(ValueError):
                production.bind_external_resources(row, samples, gpu, absent)
        for metric in samples[0]["metrics"]:
            for invalid in (None, False, float("inf")):
                altered = copy.deepcopy(samples)
                altered[0]["metrics"][metric] = invalid
                with self.subTest(metric=metric, invalid=invalid), self.assertRaises(ValueError):
                    production.bind_external_resources(row, altered, gpu, handles)

    @staticmethod
    def lifecycle_artifacts():
        # Synthetic complete observations exercise binding, not native acceptance.
        sample = {"real_seconds": 1.0, "owners": [{"entity": "owner", "movable": False}],
                  "roots": [{"owner": "owner", "parts": [{"mesh": "resident"}]}],
                  "identity": {"manifest_sha256": "a" * 64},
                  "water_state": "Partial", "water_y_wu": 1.0}
        raw = {"promotion_authority": False,
               "samples": [sample, {**copy.deepcopy(sample), "real_seconds": 2.0}],
               "events": [{"sample_index": 1, "operation": "Move", "result": "Rejected"}],
               "witnesses": [{"before": 0, "after": 1, "owner": "owner", "events": [0]}],
               "cycles": [{"before": 0, "after": 1, "orphan_parts": 0}],
               "owner": "owner", "production_state": {"partial_y_wu": 1.0},
               "before_gameplay_contract_sha256": "b" * 64,
               "baseline_item_image_sha256": "c" * 64}
        trace = copy.deepcopy(raw)
        for index, entry in enumerate(trace["samples"]):
            entry["raw_sample_index"] = index
        return trace, raw

    def test_lifecycle_lossless_observations_bind(self):
        trace, raw = self.lifecycle_artifacts()
        production.bind_lifecycle_observations(trace, raw)

    def test_lifecycle_normalized_predicate_mutations_are_rejected(self):
        mutations = [
            (("samples", 1, "owners", 0, "entity"), "invented"),
            (("samples", 1, "owners", 0, "movable"), True),
            (("samples", 1, "roots", 0, "owner"), "invented"),
            (("samples", 1, "roots", 0, "parts", 0, "mesh"), "invented"),
            (("samples", 1, "identity", "manifest_sha256"), "d" * 64),
            (("samples", 1, "water_state"), "Full"),
            (("samples", 1, "water_y_wu"), 2.0),
            (("events", 0, "operation"), "Navigation"),
            (("events", 0, "result"), "Arrived"),
            (("events", 0, "sample_index"), 0),
            (("witnesses", 0, "owner"), "invented"),
            (("cycles", 0, "orphan_parts"), False),
            (("owner",), "invented"),
            (("production_state", "partial_y_wu"), 2.0),
            (("before_gameplay_contract_sha256",), "d" * 64),
            (("baseline_item_image_sha256",), "d" * 64),
        ]
        for path, value in mutations:
            with self.subTest(path=path):
                trace, raw = self.lifecycle_artifacts()
                target = trace
                for key in path[:-1]:
                    target = target[key]
                target[path[-1]] = value
                with self.assertRaises(ValueError):
                    production.bind_lifecycle_observations(trace, raw)

    def test_lifecycle_missing_raw_fields_cannot_be_supplied_by_normalizer(self):
        for key in ("events", "witnesses", "cycles", "production_state", "owner"):
            trace, raw = self.lifecycle_artifacts()
            del raw[key]
            with self.subTest(key=key), self.assertRaises(ValueError):
                production.bind_lifecycle_observations(trace, raw)
        trace, raw = self.lifecycle_artifacts()
        trace["future_predicate"] = {"accepted": True}
        with self.assertRaises(ValueError):
            production.bind_lifecycle_observations(trace, raw)

    def test_lifecycle_cannot_drop_reorder_or_reindex_observations(self):
        for change in ("drop", "reorder", "reindex", "drop-event", "append-event"):
            trace, raw = self.lifecycle_artifacts()
            if change == "drop":
                trace["samples"].pop()
            elif change == "reorder":
                trace["samples"].reverse()
            elif change == "reindex":
                trace["samples"][1]["raw_sample_index"] = 0
            elif change == "drop-event":
                trace["events"].clear()
            else:
                trace["events"].append({"sample_index": 1, "operation": "BuildWork"})
            with self.subTest(change=change), self.assertRaises(ValueError):
                production.bind_lifecycle_observations(trace, raw)

    def test_budget_requires_every_distinct_metric_finite_positive_and_frozen(self):
        value = {"max_delta": dict.fromkeys(production.METRICS, 1.0),
                 "max_relative_mad": .05, "reason": "reviewed before comparison", "frozen_at_ns": 1}
        production.budget(value)
        for invalid in (None, {}, {**value, "reason": ""}, {**value, "frozen_at_ns": True}):
            with self.assertRaises(ValueError):
                production.budget(invalid)
        for metric in production.METRICS:
            for bad in (0, -1, True, float("nan"), float("inf")):
                invalid = copy.deepcopy(value)
                invalid["max_delta"][metric] = bad
                with self.assertRaises(ValueError, msg=f"{metric}={bad}"):
                    production.budget(invalid)
            missing = copy.deepcopy(value)
            del missing["max_delta"][metric]
            with self.assertRaises(ValueError):
                production.budget(missing)

    def test_capture_memory_are_sequential_and_pairs_reverse_in_middle_repeat(self):
        matrix = production.matrix()
        self.assertEqual(len(matrix), 24)
        self.assertTrue(all(row[0] == "Capture" for row in matrix[:12]))
        self.assertTrue(all(row[0] == "Memory" for row in matrix[12:]))
        for offset in (0, 6, 12, 18):
            self.assertEqual([row[3] for row in matrix[offset:offset + 6]],
                             ["legacy-control", "candidate", "candidate", "legacy-control", "legacy-control", "candidate"])

    def test_all_ten_and_independent_bridge_legs_are_required(self):
        self.assertEqual(len(production.GROUPS["full"]), 10)
        self.assertEqual(len(lifecycle.LEGS["Bridge"]), 11)
        self.assertEqual(set(lifecycle.LEGS["Bridge"]), set(lifecycle.BRIDGE_CASES))
        self.assertEqual(len(lifecycle.BRIDGE_CASES["passage"]), 4)
        self.assertEqual(len(lifecycle.BRIDGE_CASES["cleanup"]), 10)
        with self.assertRaises(ValueError):
            lifecycle.verify(None, [], {"scope": "bridge"}, "unused", set())

    def test_registry_contract_never_claims_launch_or_promotion_authority(self):
        for scope in production.GROUPS:
            contract = production.registry_contract(scope)
            for field in ("promotion_authority", "launcher_registered", "launchable"):
                self.assertIs(contract[field], False)
            self.assertIs(contract["available"], False)
            self.assertIn("registration", contract["required_receipt_fields"])
            self.assertIn("register", contract["registration_actions"])

    def test_ambiguous_and_nonfinite_json_is_rejected(self):
        for payload in (b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}'):
            with self.assertRaises(ValueError):
                production.json_bytes(payload)

    def test_bridge_recipe_names_and_pass_flags_are_not_results(self):
        for leg in lifecycle.LEGS["Bridge"]:
            with self.assertRaises(ValueError):
                lifecycle.check_bridge({"witnesses": []}, {"leg": leg}, {})

    def test_nonmovable_witness_requires_rejection_and_unchanged_owner(self):
        sample = {"owners": [{"entity": "bridge", "movable": False}], "move_tasks": []}
        trace = {"samples": [sample, copy.deepcopy(sample)],
                 "events": [{"sample_index": 1, "operation": "Move", "result": "Rejected"}]}
        witness = {"before": 0, "after": 1, "events": [0], "owner": "bridge"}
        lifecycle.bridge_witness("ui-rejected", "non-movable", witness, trace, {})
        trace["samples"][1]["owners"][0]["movable"] = True
        with self.assertRaises(ValueError):
            lifecycle.bridge_witness("ui-rejected", "non-movable", witness, trace, {})

    def test_bank_passage_does_not_accept_missing_lane_or_reversed_order(self):
        with self.assertRaises(ValueError):
            lifecycle.sequence([[0, 0], [0, 2]], [[0, 0], [0, 1], [0, 2]])
        with self.assertRaises(ValueError):
            lifecycle.sequence([[0, 2], [0, 1], [0, 0]], [[0, 0], [0, 1], [0, 2]])


if __name__ == "__main__":
    unittest.main()
