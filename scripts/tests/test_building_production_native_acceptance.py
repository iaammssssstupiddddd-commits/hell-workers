"""Fail-closed tests for the registered production runner; no native launch."""
from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]


def load_runner(evidence, collection, lifecycle):
    for name, module in {
        "building_production_acceptance": evidence,
        "building_production_collection": collection,
        "building_production_lifecycle": lifecycle,
    }.items():
        sys.modules[name] = module
    path = ROOT / "scripts/building_production_native_acceptance.py"
    spec = importlib.util.spec_from_file_location("production_native_test_subject", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Pipeline:
    @staticmethod
    def canonical(value):
        return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()

    @staticmethod
    def digest(value):
        import hashlib
        return hashlib.sha256(value).hexdigest()

    @staticmethod
    def read(path):
        return json.loads(Path(path).read_bytes())

    @staticmethod
    def put(path, value):
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        Path(path).write_bytes(value)

    @staticmethod
    def record(name, value):
        return {"path": name, "bytes": len(value), "sha256": Pipeline.digest(value)}


class ProductionNativeAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: __import__("shutil").rmtree(self.root))
        evidence = types.ModuleType("building_production_acceptance")
        evidence.PROFILE = "building-production-results-v1"
        evidence.pipeline = Pipeline
        evidence.NONCE = __import__("re").compile(r"[0-9a-f]{32}")
        evidence.GROUPS = {"m2": ("Tank",)}
        evidence.SPEC_KEYS = {"native_plan_path", "native_job_root"}
        evidence.GENERATED_KEYS = set()
        evidence.require = lambda value, reason: value or (_ for _ in ()).throw(ValueError(reason))
        evidence.hashed = lambda value: isinstance(value, str) and len(value) == 64
        evidence.json_bytes = lambda value: json.loads(value)
        evidence.matrix = lambda: [("Capture", "N", 1, "candidate"),
                                   ("Memory", "N", 1, "candidate")]
        evidence.check_registration_context = lambda value: value["context"]
        evidence.host_document = lambda value: value
        evidence.registration_binding = lambda value, receipt=None: {"registry_sha256": value["registry_sha256"]}
        evidence.require_production_registration = lambda value, receipt=None: evidence.registration_binding(value, receipt)
        evidence.check_registration_receipt = lambda value, receipt: receipt["registered_at_ns"]
        evidence.check_plan = lambda value, **kwargs: None
        evidence.plan = lambda value, **kwargs: value
        evidence.session = lambda *args, **kwargs: {}
        evidence.verify_results = lambda *args, **kwargs: {"promotion_authority": False,
            "art_approved": False, "release_approved": False}
        collection = types.ModuleType("building_production_collection")
        collection.SESSION_KEYS = ("subject", "binary_sha256", "codec_sha256", "driver_sha256",
            "input_transport_sha256", "plan_sha256", "campaign_nonce", "nonce", "pid", "root_pid",
            "window_id", "kind", "leg", "raw_observations_path")
        collection.validate_steps = lambda steps: evidence.require(steps == [], "steps")
        collection.session_binding = lambda *args: None
        collection.collect = lambda *args: None
        collection.write_once = lambda path, value: Path(path).write_bytes(value)
        lifecycle = types.ModuleType("building_production_lifecycle")
        lifecycle.LEGS = {"Tank": ("placement",)}
        lifecycle.verify = lambda *args, **kwargs: None
        self.evidence, self.collection, self.lifecycle = evidence, collection, lifecycle
        self.runner = load_runner(evidence, collection, lifecycle)
        self.subject = {"binding": {"scope_sha256": "a" * 64, "node_id": "full-acceptance"},
            "run": {"id": "run", "consumer_generation": 2}, "generation": 12,
            "repo": str(self.root), "head": "b" * 40, "source": "c" * 64}
        self.context = {"controller_sha256": "d" * 64, "recipe_revision": "e" * 64,
            "bridge_revision": "f" * 64, "subject": self.subject, "owner": "coordinator"}

    def value(self):
        runner = Path(self.runner.__file__).resolve()
        native_plan = str((self.root / "native-plan.json").resolve())
        job_root = str((self.root / "job").resolve())
        intent = {"action": "register", "spec": {"id": "batch", "repo": str(self.root),
            "owner": "coordinator", "consumers": [], "roots": [job_root],
            "verify_command": ["python3", str(runner), "verify", "--job-root", job_root]},
            "command": [*self.runner.PREFIX, str(runner), "run", "--plan", native_plan],
            "subject": self.subject, "controller_sha256": "d" * 64, "recipe_revision": "e" * 64}
        return {"context": self.context, "host_capabilities": {"ok": True, "revision": "e" * 64,
            "contracts": {"schema": 1, "recipes": [copy.deepcopy(self.runner.RECIPE_CONTRACT)]}},
            "registration_intent": intent, "registration_request": "00000000-0000-0000-0000-000000000001",
            "native_plan_path": native_plan, "native_job_root": job_root}

    def receipt(self, value):
        intent = value["registration_intent"]
        runner = Path(self.runner.__file__).resolve()
        return {"schema": 1, "authority": "host-admitted-native-registration",
            "request": value["registration_request"], "batch": "batch", "phase": "registered",
            "registered_at_ns": 3, "controller_sha256": "d" * 64, "recipe_revision": "e" * 64,
            "bridge_revision": "f" * 64, "subject": self.subject, "recipe_id": self.runner.RECIPE_ID,
            "intent_sha256": Pipeline.digest(json.dumps(intent, sort_keys=True, separators=(",", ":")).encode()),
            "plan_sha256": Pipeline.digest(Pipeline.canonical(value)),
            "runner_sha256": Pipeline.digest(runner.read_bytes()),
            "verifier_sha256": Pipeline.digest(runner.read_bytes()),
            "job_root": value["native_job_root"], "command": intent["command"],
            "verify_command": intent["spec"]["verify_command"], "promotion_authority": False}

    def test_recipe_adapter_requires_exact_live_contract_intent_and_receipt(self):
        value = self.value()
        binding = self.runner.recipe_binding(value)
        value["registry_sha256"] = binding["registry_sha256"]
        receipt = self.receipt(value)
        admitted = self.runner.recipe_binding(value, receipt)
        self.assertEqual(admitted["plan_sha256"], receipt["plan_sha256"])
        for case in ("recipe", "command", "subject", "authority", "runner", "root", "batch", "timestamp"):
            changed_value, changed_receipt = copy.deepcopy(value), copy.deepcopy(receipt)
            if case == "recipe":
                changed_value["host_capabilities"]["contracts"]["recipes"][0]["id"] = "foreign"
            elif case == "command":
                changed_value["registration_intent"]["command"][-1] += ".foreign"
            elif case == "subject":
                changed_receipt["subject"]["head"] = "0" * 40
            elif case == "authority":
                changed_receipt["promotion_authority"] = True
            elif case == "runner":
                changed_receipt["runner_sha256"] = "0" * 64
            elif case == "batch":
                changed_receipt["batch"] = "foreign"
            elif case == "timestamp":
                changed_receipt["registered_at_ns"] = 0
            else:
                changed_receipt["job_root"] += "-foreign"
            with self.subTest(case=case), self.assertRaises(ValueError):
                self.runner.recipe_binding(changed_value, changed_receipt)

    def test_local_capabilities_never_read_host_or_create_artifacts(self):
        before = list(self.root.iterdir())
        with patch.object(self.evidence, "check_registration_context", side_effect=AssertionError("old host read")), \
                patch.object(self.evidence, "host_document", side_effect=AssertionError("old host document")):
            result = self.runner.capabilities()
        self.assertFalse(result["available"])
        self.assertFalse(result["launchable"])
        for key in ("accepted", "promotion_authority", "art_approved", "release_approved"):
            self.assertIs(result[key], False)
        self.assertEqual(len(result["missing"]), 3)
        self.assertEqual(list(self.root.iterdir()), before)

    def sessions(self, runtime):
        root = Path(runtime["native_job_root"])
        result = []
        for row in self.runner.expected_sessions(runtime):
            result.append({**row, "nonce": f"{row['ordinal'] + 1:032x}",
                "descriptor": str(root / "host-sessions" / f"{row['ordinal']:04d}-ready.json"),
                "sealed_row": str(root / "host-sessions" / f"{row['ordinal']:04d}-sealed.json"),
                "steps": []})
        return result

    def test_sessions_are_capture_then_memory_complete_and_unique(self):
        runtime = {"scope": "m2", "native_job_root": str(self.root / "job")}
        sessions = self.sessions(runtime)
        self.runner.check_session_plan(sessions, runtime, Path(runtime["native_job_root"]))
        self.assertEqual([row["instrument"] for row in sessions],
                         ["Capture", "Capture", "Memory", "Memory"])
        for mutation in ("reorder", "missing", "duplicate", "escape"):
            changed = copy.deepcopy(sessions)
            if mutation == "reorder":
                changed[0], changed[-1] = changed[-1], changed[0]
            elif mutation == "missing":
                changed.pop()
            elif mutation == "duplicate":
                changed[-1]["nonce"] = changed[0]["nonce"]
            else:
                changed[0]["descriptor"] = str(self.root / "foreign.json")
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.runner.check_session_plan(changed, runtime, Path(runtime["native_job_root"]))

    def test_instrument_binding_never_interchanges_capture_and_memory(self):
        value = self.value()
        value["registry_sha256"] = self.runner.recipe_binding(value)["registry_sha256"]
        value.update(scope="m2", subject={"commit": "b" * 40}, campaign_nonce="1" * 32,
            binaries={"Capture": {"sha256": "2" * 64}, "Memory": {"sha256": "3" * 64}},
            codec={"sha256": "4" * 64}, driver={"sha256": "5" * 64},
            input_transport_sha256="6" * 64)
        binding = {key: None for key in self.collection.SESSION_KEYS}
        binding.update(subject=value["subject"], binary_sha256="3" * 64,
            codec_sha256="4" * 64, driver_sha256="5" * 64,
            input_transport_sha256="6" * 64, plan_sha256="7" * 64,
            campaign_nonce="1" * 32, nonce="8" * 32, pid=1, root_pid=1,
            window_id=1, kind="Tank", leg="placement", raw_observations_path="/raw")
        admission = {"authority": "host-admitted-production-session",
            "registry_sha256": value["registry_sha256"], "session": copy.deepcopy(binding),
            "steps_sha256": Pipeline.digest(Pipeline.canonical([])), "world": "normal-generated",
            "headless": False, "fixture_seeded_completion": False}
        # Exercise the real shared collector binding, not a parallel runner copy.
        from scripts import building_production_collection as real_collection
        with patch.object(real_collection, "evidence", self.evidence), \
                patch.object(self.evidence, "registration_binding", return_value={
                    "registry_sha256": value["registry_sha256"]}):
            real_collection.session_binding(value, "7" * 64, binding, admission, [], instrument="Memory")
            with self.assertRaisesRegex(ValueError, "binary"):
                real_collection.session_binding(value, "7" * 64, binding, admission, [], instrument="Capture")

    def test_run_requires_registered_launcher_and_verify_preserves_no_authority(self):
        with patch.dict("os.environ", {}, clear=True), self.assertRaisesRegex(ValueError, "registered"):
            self.runner.run(self.root / "missing.json")
        native_plan = {"runtime_plan_sha256": Pipeline.digest(b"{}")}
        with patch.object(self.runner, "check_native_plan", return_value=({}, self.root)), \
                patch.object(self.evidence.pipeline, "read", side_effect=[native_plan, {}, {
                    "accepted": False, "promotion_authority": False,
                    "memory_lifecycle": [{"kind": "Tank", "leg": "placement", "instrument": "Memory"}]}]), \
                patch.object(Path, "read_bytes", return_value=b"{}"), \
                patch.object(self.evidence, "check_registration_receipt"), \
                patch.object(self.lifecycle, "verify"), \
                patch.object(self.evidence, "verify_results", return_value={
                    "promotion_authority": False, "art_approved": False, "release_approved": False}):
            runtime = {"scope": "m2"}
            self.runner.check_native_plan.return_value = (runtime, self.root)
            result = self.runner.verify(self.root)
            self.assertIs(self.evidence.verify_results.call_args.kwargs["registration_adapter"],
                          self.runner.recipe_binding)
            self.assertIs(self.lifecycle.verify.call_args.kwargs["registration_adapter"],
                          self.runner.recipe_binding)
            self.assertEqual(self.lifecycle.verify.call_args.kwargs["instrument"], "Memory")
        self.assertFalse(result["accepted"])
        self.assertFalse(result["promotion_authority"])
        self.assertFalse(result["art_approved"])
        self.assertFalse(result["release_approved"])


if __name__ == "__main__":
    unittest.main()
