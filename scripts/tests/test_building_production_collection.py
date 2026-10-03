"""Collection contract tests; all transport/process calls are mocked, no native launch."""
import copy
from contextlib import ExitStack
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

from scripts import building_production_collection as collection
from scripts import building_production_acceptance as evidence
from scripts import building_production_lifecycle as lifecycle


class CollectionTests(unittest.TestCase):
    def test_session_claim_and_artifacts_cannot_be_rewritten(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "raw.collection-claim"
            collection.write_once(path, b"first")
            with self.assertRaises(FileExistsError):
                collection.write_once(path, b"retry")
            self.assertEqual(path.read_bytes(), b"first")

    def fixture(self):
        binding = dict.fromkeys(collection.SESSION_KEYS, "a" * 64)
        binding.update(nonce="b" * 32, pid=42, root_pid=40, window_id=64,
                       kind="Tank", leg="placement", subject={"source": "exact"})
        steps = [{"id": name, "input": {"button": 1, "pressed": pressed},
                  "until": {"path": ["paused"], "equals": False}, "timeout_seconds": 1}
                 for name, pressed in (("press", True), ("release", False))]
        raw = {"scope": "production-raw-observations-v1", "nonce": binding["nonce"],
               "schema_version": 2, "transition_contract": "indexed-state-delta-v1",
               "process_id": binding["pid"], "failure": None, "accepted": False,
               "performance_evidence": False, "promotion_authority": False,
               "samples": [{"real_seconds": float(i), "paused": False, "owners": []} for i in range(3)],
               "events": [{"sample_index": i, "operation": operation}
                          for i, operation in ((1, "PointerPress"), (2, "PointerRelease"))]}
        for i, sample in enumerate(raw["samples"]):
            sample["sample_index"] = i
            for category in lifecycle.TRANSITION_CATEGORIES:
                sample.setdefault(category, [])
        for i, event in enumerate(raw["events"]):
            event["event_index"] = i
        records = []
        for i, step in enumerate(steps):
            records.extend([{"type": "intent", "step": step["id"], "before": i, "input": step["input"]},
                            {"type": "sent", "step": step["id"], "nonce": binding["nonce"],
                             "pid": 42, "window": 64, **step["input"]},
                            {"type": "observed", "step": step["id"], "after": i + 1}])
        for i, row in enumerate(records):
            row.update(ordinal=i, at_ns=i + 1)
        log = {"session": binding, "steps": steps, "records": records, "failure": None, "promotion_authority": False}
        return binding, steps, raw, log

    def transition_fixture(self):
        binding, steps, raw, log = self.fixture()
        owner = {"entity": "owner", "kind": "Tank", "position": [0, 0, 0]}
        raw["samples"][2]["owners"] = [owner]
        raw["events"].append({"event_index": 2, "producer": "production_snapshot_delta",
                              "operation": "StateTransition", "category": "owners", "entity": "owner",
                              "before_sample_index": 1, "sample_index": 2, "real_seconds": 2.0,
                              "before": None, "after": copy.deepcopy(owner)})
        return binding, steps, raw, log

    def test_raw_state_transition_is_lossless_and_not_an_inferred_commit(self):
        binding, _, raw, _ = self.transition_fixture()
        collection.raw_record(evidence.pipeline.canonical(raw), binding)
        trace = collection.normalize(raw, binding, "a" * 64)
        self.assertEqual(trace["events"], raw["events"])
        self.assertNotIn("result", trace["events"][2])
        lifecycle.check_domain_witness({"owner": "owner", "before": 1, "after": 2, "domain_event_indices": [2]}, trace)
        for selected in ([0], [1], [], [3], [2, 2]):
            with self.subTest(selected=selected), self.assertRaises(ValueError):
                lifecycle.check_domain_witness({"owner": "owner", "before": 1, "after": 2, "domain_event_indices": selected}, trace)
        with self.assertRaises(ValueError):
            lifecycle.check_domain_witness({"owner": "foreign", "before": 1, "after": 2, "domain_event_indices": [2]}, trace)

    def test_raw_transition_remapping_missing_delta_and_synthetic_values_fail(self):
        for change in ("missing", "owner", "before", "after", "category", "index", "sample-index", "duplicate-owner", "old-schema"):
            binding, _, raw, _ = self.transition_fixture()
            if change == "missing":
                raw["events"].pop()
            elif change == "owner":
                raw["events"][2]["entity"] = "foreign"
            elif change == "before":
                raw["events"][2]["before_sample_index"] = 0
            elif change == "after":
                raw["events"][2]["after"]["position"] = [8, 8, 8]
            elif change == "category":
                raw["events"][2]["category"] = "roots"
            elif change == "index":
                raw["events"][2]["event_index"] = 0
            elif change == "sample-index":
                raw["samples"][2]["sample_index"] = 0
            elif change == "duplicate-owner":
                raw["samples"][2]["owners"] *= 2
            else:
                raw["schema_version"] = 1
            with self.subTest(change=change), self.assertRaises(ValueError):
                collection.raw_record(evidence.pipeline.canonical(raw), binding)

    def test_lossless_normalization_and_actual_input_ranges(self):
        binding, _, raw, log = self.fixture()
        trace = collection.normalize(raw, binding, "a" * 64)
        self.assertEqual(trace["events"], raw["events"])
        self.assertEqual(collection.verify_action_records(log, raw), [(0, 1), (1, 2)])
        evidence.bind_lifecycle_observations(trace, raw)
        self.assertNotIn("witnesses", trace)
        self.assertNotIn("identity", trace)
        self.assertIs(trace["accepted"], False)
        trace["samples"][0]["owners"].append("synthetic")
        with self.assertRaises(ValueError):
            evidence.bind_lifecycle_observations(trace, raw)

    def test_generated_plan_session_and_result_admission_contract(self):
        from scripts.tests.test_building_production_acceptance import ProductionAcceptanceTests

        # No recipe is inserted into a capability catalog. Only the unavailable
        # host adapter is replaced at the admission boundary. Separate source,
        # asset and measurement providers are fixtures, not acceptance evidence.
        with tempfile.TemporaryDirectory() as temporary, ExitStack() as stack:
            root = Path(temporary)
            spec, receipt = ProductionAcceptanceTests.unavailable_host_fixture(root)
            spec["subject"].update(harness="a" * 64, assets="b" * 64, driver="c" * 64, dirty=[])

            def binary(name):
                path = root / name
                path.write_bytes(name.encode())
                return {"path": str(path), "sha256": evidence.pipeline.digest(path.read_bytes())}

            inventories = {}
            trials = {}
            for kind in evidence.GROUPS["m2"]:
                trials[kind] = []
                for generation in (1, 2):
                    path = root / f"{kind}-{generation}"
                    identity = {"kind": kind, "generation": generation, "authority": "release_approved"}
                    text = evidence.pipeline.canonical({"identity": identity}).decode()
                    inventories[str(path)] = ({"identity": identity}, text, None)
                    trials[kind].append({"root": str(path), "identity": identity,
                                        "manifest_file_sha256": evidence.pipeline.digest(text.encode())})
            spec.update(scope="m2", codec=binary("codec"), driver=binary("driver"),
                binaries={name: binary(name) for name in ("Capture", "Memory")}, baseline=None,
                environment={"backend": "Vulkan", "window_backend": "x11", "adapter": "fixture",
                             "driver": "fixture", "present_mode": "fixture"},
                gameplay_contract_sha256="a" * 64,
                cases={size: {"copies": copies, "world_seed": 20260920,
                              **dict.fromkeys(("terrain_sha256", "layout_sha256", "logical_state_sha256",
                                  "other_groups_sha256", "camera_sha256", "population_sha256", "activity_sha256"), "a" * 64)}
                       for size, copies in (("N", 4), ("4N", 16))},
                budget={"max_delta": dict.fromkeys(evidence.METRICS, 1), "max_relative_mad": .05,
                        "reason": "fixture only", "frozen_at_ns": 1},
                releases={kind: records[0] for kind, records in trials.items()}, generation_trials=trials)

            def adapter(value, registered_receipt=None):
                result = {"registry_sha256": "d" * 64}
                if registered_receipt is not None:
                    result.update(plan_sha256=evidence.pipeline.digest(evidence.pipeline.canonical(value)),
                                  receipt_sha256=evidence.pipeline.digest(evidence.pipeline.canonical(registered_receipt)),
                                  admitted_at_ns=value["created_at_ns"] + 1)
                return result

            host = stack.enter_context(patch.object(evidence, "require_production_registration", side_effect=adapter))
            stack.enter_context(patch.object(evidence.art, "subject", return_value=spec["subject"]))
            stack.enter_context(patch.object(evidence.pipeline, "load_set",
                                             side_effect=lambda path, *_: inventories[str(path)]))
            # All plan schema checks, generation checks and admission consumers
            # run for real; never append a registry field to a hand-built plan.
            plan = evidence.plan(spec)
            self.assertEqual(set(plan), evidence.SPEC_KEYS | evidence.GENERATED_KEYS)
            self.assertFalse(plan["launchable"])
            self.assertFalse(plan["promotion_authority"])
            plan_path = root / "plan.json"
            plan_path.write_bytes(evidence.pipeline.canonical(plan))
            plan_hash = evidence.pipeline.digest(plan_path.read_bytes())
            binding, steps, _, _ = self.fixture()
            binding.update(subject=plan["subject"], plan_sha256=plan_hash, campaign_nonce=plan["campaign_nonce"],
                           binary_sha256=plan["binaries"]["Capture"]["sha256"], codec_sha256=plan["codec"]["sha256"],
                           driver_sha256=plan["driver"]["sha256"], input_transport_sha256=plan["input_transport_sha256"])
            admission = {"authority": "host-admitted-production-session", "registry_sha256": plan["registry_sha256"],
                         "session": copy.deepcopy(binding), "steps_sha256": evidence.pipeline.digest(evidence.pipeline.canonical(steps)),
                         "world": "normal-generated", "headless": False, "fixture_seeded_completion": False}
            collection.session_binding(plan, plan_hash, binding, admission, steps)
            for key in collection.SESSION_KEYS:
                bad = copy.deepcopy(binding)
                bad[key] = "different"
                with self.subTest(key=key), self.assertRaises(ValueError):
                    collection.session_binding(plan, plan_hash, bad, admission, steps)
            with self.assertRaises(ValueError):
                collection.session_binding(plan, plan_hash, binding, {**admission, "registry_sha256": "e" * 64}, steps)

            admitted = evidence.check_registration_receipt(plan, receipt)
            self.assertEqual(admitted, plan["created_at_ns"] + 1)
            payload = evidence.pipeline.canonical(receipt)
            (root / "receipt.json").write_bytes(payload)
            rows = [{"instrument": instrument, "size": size, "repeat": repeat, "mode": mode,
                     "started_at_ns": admitted + 2 * i + 1, "finished_at_ns": admitted + 2 * i + 2}
                    for i, (instrument, size, repeat, mode) in enumerate(evidence.matrix())]
            result = {"profile": evidence.PROFILE, "promotion_authority": False, "plan_sha256": plan_hash,
                      "registration_receipt": evidence.pipeline.record("receipt.json", payload),
                      "performance": rows, "lifecycle": {}}
            result_path = root / "result.json"
            result_path.write_bytes(evidence.pipeline.canonical(result))
            performance = stack.enter_context(patch.object(evidence, "inspect_performance",
                                                          return_value=dict.fromkeys(evidence.METRICS, 1)))
            stack.enter_context(patch.object(lifecycle, "verify"))
            verified = evidence.verify_results(plan_path, result_path)
            self.assertFalse(verified["promotion_authority"])
            self.assertFalse(verified["art_approved"])
            self.assertFalse(verified["release_approved"])
            self.assertEqual(performance.call_count, len(rows))

            for field, bad in (("registry_sha256", "e" * 64), ("plan_sha256", "f" * 64),
                               ("receipt_sha256", "f" * 64), ("admitted_at_ns", None),
                               ("admitted_at_ns", True), ("admitted_at_ns", plan["created_at_ns"])):
                def altered(value, registered_receipt=None):
                    bound = adapter(value, registered_receipt)
                    if registered_receipt is not None:
                        bound[field] = bad
                    return bound
                host.side_effect = altered
                performance.reset_mock()
                with self.subTest(field=field, bad=bad), self.assertRaises(ValueError):
                    evidence.verify_results(plan_path, result_path)
                performance.assert_not_called()
            for absent in (None, {"registry_sha256": plan["registry_sha256"]}):
                host.side_effect = lambda value, received=None: adapter(value) if received is None else absent
                with self.subTest(absent=absent), self.assertRaisesRegex(ValueError, "binding missing"):
                    evidence.verify_results(plan_path, result_path)
            host.side_effect = lambda *_: None
            with self.assertRaisesRegex(ValueError, "binding missing"):
                evidence.check_plan(plan)
            host.side_effect = lambda *_: {"registry_sha256": "e" * 64}
            with self.assertRaisesRegex(ValueError, "identity differs"):
                evidence.check_plan(plan)
            with self.assertRaisesRegex(ValueError, "identity differs"):
                collection.session_binding(plan, plan_hash, binding, admission, steps)
            host.side_effect = adapter
            rows[0]["started_at_ns"] = admitted
            result_path.write_bytes(evidence.pipeline.canonical(result))
            with self.assertRaisesRegex(ValueError, "out-of-order"):
                evidence.verify_results(plan_path, result_path)

    def test_repeated_synthetic_missing_and_remapped_input_refused(self):
        for change in ("repeat", "synthetic", "missing", "remap", "event", "failed", "foreign"):
            _, _, raw, log = self.fixture()
            if change == "repeat":
                log["steps"][1]["id"] = "press"
            elif change == "synthetic":
                log["records"][1]["button"] = 2
            elif change == "missing":
                log["records"].pop()
            elif change == "remap":
                log["records"][2]["after"] = 2
            elif change == "event":
                raw["events"][0]["operation"] = "Placement"
            elif change == "failed":
                log["failure"] = "lost focus"
            else:
                log["records"][1]["pid"] = 100
            with self.subTest(change=change), self.assertRaises(ValueError):
                collection.verify_action_records(log, raw)

    def test_prefix_rewrite_missing_domain_and_invalid_event_indices_refused(self):
        binding, _, raw, _ = self.fixture()
        for change in ("owner", "event", "drop", "reorder", "index", "nonce"):
            bad = copy.deepcopy(raw)
            if change == "owner":
                bad["samples"][0]["owners"] = ["fabricated"]
            elif change == "event":
                bad["events"][0]["operation"] = "Committed"
            elif change == "drop":
                bad["samples"].pop()
            elif change == "reorder":
                bad["samples"].reverse()
            elif change == "index":
                bad["events"][0]["sample_index"] = 99
            else:
                bad["nonce"] = "c" * 32
            with self.subTest(change=change), self.assertRaises(ValueError):
                collection.raw_record(evidence.pipeline.canonical(bad), binding, raw)
        with self.assertRaises(ValueError):
            collection.at(raw["samples"][0], ["Navigation", "Arrived"])

    def test_unbalanced_unobserved_or_unbounded_inputs_refused(self):
        _, steps, _, _ = self.fixture()
        for bad in (steps[:1], steps + steps, [{**steps[0], "timeout_seconds": float("inf")}],
                    [{**steps[0], "input": {"key": "unobserved", "pressed": True}}]):
            with self.assertRaises(ValueError):
                collection.validate_steps(bad)

    def test_host_seal_binds_log_raw_trace_and_owned_capture_ranges(self):
        binding, steps, raw, log = self.fixture()
        # This unit covers sealing/ranges only. The actual plan/session contract
        # is exercised by test_generated_plan_session_and_result_admission_contract.
        plan = {}
        admission = {"steps_sha256": evidence.pipeline.digest(evidence.pipeline.canonical(steps))}
        row = {**binding, "started_at_ns": 0, "finished_at_ns": 10,
               "captures": [{"window_id": "64", "sample_index": i} for i in (0, 2)]}
        for name in ("action_log", "collection_admission", "collection_seal", "raw_observations", "trace"):
            row[name] = {"path": name, "sha256": "e" * 64}
        seal = {"authority": "host-sealed-production-collection", "session": binding,
                "steps_sha256": admission["steps_sha256"], "raw_observations_sha256": "e" * 64,
                "action_log_sha256": "e" * 64, "trace_sha256": "e" * 64, "captures": row["captures"]}
        artifacts = {"action_log": log, "collection_admission": admission, "collection_seal": seal}
        with patch.object(collection, "session_binding"), patch.object(
                evidence, "artifact", side_effect=lambda root, spec: evidence.pipeline.canonical(artifacts[spec["path"]])):
            collection.verify_log(None, row, plan, raw, {})
            for key in ("raw_observations_sha256", "action_log_sha256", "trace_sha256", "captures"):
                original = seal[key]
                seal[key] = None
                with self.subTest(key=key), self.assertRaises(ValueError):
                    collection.verify_log(None, row, plan, raw, {})
                seal[key] = original
            with self.assertRaises(ValueError):
                collection.verify_log(None, row, plan, raw, {"witnesses": [{"before": 1, "after": 1}]})

    def test_process_failure_never_constructs_input_transport(self):
        binding, steps, _, _ = self.fixture()
        plan = {"driver": {"sha256": evidence.pipeline.digest(Path(collection.__file__).read_bytes())},
                "input_transport_sha256": evidence.pipeline.digest(Path(collection.__file__).with_name("native_ui_input.py").read_bytes())}
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            plan_path = root / "plan.json"
            plan_path.write_bytes(evidence.pipeline.canonical(plan))
            binding["raw_observations_path"] = str(root / "missing")
            with patch.object(evidence, "check_plan"), patch.object(collection, "session_binding"), \
                 patch.object(collection, "check_process", side_effect=ProcessLookupError("gone")), \
                 patch.object(collection, "X11Input") as transport:
                with self.assertRaises(ProcessLookupError):
                    collection.collect(plan_path, binding, {}, steps, root / "missing", root / "result")
                transport.assert_not_called()
            log = evidence.json_bytes((root / "result/action-log.json").read_bytes())
            self.assertEqual(log["failure"], "gone")
            self.assertFalse((root / "result/trace.json").exists())

    def test_focus_loss_and_timeout_close_transport_without_resending(self):
        for reason in ("focus", "timeout"):
            binding, steps, raw, _ = self.fixture()
            raw["samples"] = raw["samples"][:1]
            raw["events"] = []
            plan = {"driver": {"sha256": evidence.pipeline.digest(Path(collection.__file__).read_bytes())},
                    "input_transport_sha256": evidence.pipeline.digest(Path(collection.__file__).with_name("native_ui_input.py").read_bytes())}
            transport = Mock(held_buttons={1}, held_keys=set())
            if reason == "focus":
                transport._check_focus.side_effect = [None, RuntimeError("lost focus")]
            with self.subTest(reason=reason), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                (root / "plan.json").write_bytes(evidence.pipeline.canonical(plan))
                (root / "raw.json").write_bytes(evidence.pipeline.canonical(raw))
                binding["raw_observations_path"] = str(root / "raw.json")
                with patch.object(evidence, "check_plan"), patch.object(collection, "session_binding"), \
                     patch.object(collection, "check_process"), patch.object(collection, "X11Input", return_value=transport), \
                     patch.object(collection.time, "monotonic", side_effect=[0, 0, 0, 0, 2]):
                    with self.assertRaises((ValueError, RuntimeError)):
                        collection.collect(root / "plan.json", binding, {}, steps, root / "raw.json", root / "result")
                transport.send.assert_called_once()
                transport.close.assert_called_once()
                self.assertFalse((root / "result/trace.json").exists())
                self.assertIsNotNone(evidence.json_bytes((root / "result/action-log.json").read_bytes())["failure"])
