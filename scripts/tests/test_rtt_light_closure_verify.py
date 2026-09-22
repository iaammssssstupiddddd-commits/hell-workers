from contextlib import ExitStack
from copy import deepcopy
import importlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(REPO / "scripts"), str(REPO / ".codex/skills/hell-workers-run-native-acceptance/scripts")]
closure = importlib.import_module("rtt_light_closure_verify")
bundle = importlib.import_module("perf_tool.rtt_light_bundle")


class ClosureStorageTests(unittest.TestCase):
    def test_closure_root_contains_all_output_paths_and_rejects_another_namespace(self):
        root = closure.native.closure_state_root(REPO, None)
        namespace = REPO / "target/native-acceptance/renderdoc-foundation"
        self.assertEqual(root.parent, namespace)
        self.assertEqual(closure.native.closure_state_root(REPO, str(root)), root)
        for suffix in ("artifacts/capture", "capsule/renderdoc", "closure/renderdoc"):
            self.assertTrue((root / suffix).is_relative_to(root))
        for invalid in (namespace, REPO / "target/native-acceptance/elsewhere", root / "nested"):
            with self.subTest(path=invalid), self.assertRaisesRegex(RuntimeError, "namespace"):
                closure.native.closure_state_root(REPO, str(invalid))


class RendererGateTests(unittest.TestCase):
    def setUp(self):
        self.contract = closure.native.rtt_light_contract(REPO, "p08")
        self.metrics = {
            "scene_target_count": 1, "mask_target_count": 0,
            "camera_3d_rtt_count": 1, "mask_camera_count": 0,
            "mask_pass_count": 0, "mask_binding_count": 0,
            "mask_sample_count": 0, "mask_proxy_count": 0,
            "explicit_color_bytes": 8294400,
            "layer_2d_camera_count": 1, "layer_2d_pass_count": 1,
            "duplicate_presentation_count": 0, "building_exactly_one_presentation": True,
            "soul_billboard_per_soul": 1.0, "familiar_3d_count": 0,
            "state_and_bounce_probes_pass": True,
            "field_image_count": 1, "field_handle_count": 1,
            "point_light_count_increment": 0, "spot_light_count_increment": 0,
            "shadow_map_count_increment": 0, "local_light_pass_increment": 0,
            "receiver_binding_count": 1, "shared_field_image": True,
            "duplicate_2d_pass_count": 0,
            "cpu_golden_vectors_pass": True, "pixel_probes_pass": True,
            "revision_epoch_consistency": True,
        }

    def verify(self, metrics):
        closure.verify_renderer_gates(self.contract, {
            "renderdoc-medium-gpu": {"validations": [], "gate_metrics": metrics},
        })

    def test_bounded_renderer_needs_no_performance_or_historical_baseline(self):
        self.verify(self.metrics)

    def test_inconsistent_presentation_upload_and_epoch_are_rejected(self):
        for key, value in (("building_exactly_one_presentation", False),
                           ("duplicate_presentation_count", 1),
                           ("shared_field_image", False), ("field_image_count", 2),
                           ("revision_epoch_consistency", False)):
            with self.subTest(metric=key), self.assertRaisesRegex(RuntimeError, key):
                self.verify({**self.metrics, key: value})

    def test_missing_receiver_evidence_is_rejected(self):
        del self.metrics["receiver_binding_count"]
        with self.assertRaisesRegex(RuntimeError, "receiver_binding_count"):
            self.verify(self.metrics)


class ClosureSealTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = Path(temporary.name)
        self.root = self.repo / "target/native-acceptance/closure"
        self.capture = self.root / "artifacts/capture"
        self.capture.mkdir(parents=True)
        self.manifest = {"schema_version": 2, "binary": {"instrumentation": "capture"}}
        self.write(self.capture / "manifest.json", self.manifest)
        (self.capture / "frames.csv").write_text("frame,value\n1,12\n")
        inventory = bundle.directory_inventory(self.capture, relative_to=self.capture)
        self.job = {"profile": "rtt-light", "measurement_kind": "p08-closure", "status": "valid",
                    "repo": str(self.repo), "job_root": str(self.root), "window_backend": "x11",
                    "subject_commit": "subject", "source_fingerprint": "source", "harness_fingerprint": "harness",
                    "paths": {"attempt": str(self.root / "artifacts"),
                              "environment_lock": str(self.root / "artifacts/environment-lock.json")},
                    "capture_inventory": inventory, "capture_digest": bundle.directory_digest(inventory)}
        self.args = SimpleNamespace(repo=str(self.repo), job_root=str(self.root), window_backend="x11", adapter="Intel")
        stack = self.enterContext(ExitStack())
        stack.enter_context(patch.object(closure.native, "validate_repo", return_value=self.repo))
        for method in ("assert_clean_subject", "assert_source_unchanged", "assert_native_harness_unchanged"):
            stack.enter_context(patch.object(closure.native, method))
        self.raw_gate = stack.enter_context(patch("perf_tool.policy.validate_session_artifact_set",
                                                  side_effect=RuntimeError("raw verifier reached")))

    @staticmethod
    def write(path, value):
        path.write_text(json.dumps(value))

    def verify(self):
        self.write(self.root / "job.json", self.job)
        return closure.verify(self.args)

    def test_matching_inventory_reaches_independent_raw_verifier(self):
        with self.assertRaisesRegex(RuntimeError, "raw verifier reached"):
            self.verify()
        self.raw_gate.assert_called_once()

    def test_missing_seal_or_changed_raw_fails_before_raw_validation(self):
        original = deepcopy(self.job)
        del self.job["capture_inventory"]
        with self.assertRaisesRegex(RuntimeError, "Capture files changed"):
            self.verify()
        self.job = original
        (self.capture / "frames.csv").write_text("frame,value\n1,99\n")
        with self.assertRaisesRegex(RuntimeError, "Capture files changed"):
            self.verify()
        self.raw_gate.assert_not_called()

    def test_resealed_wrong_schema_still_fails(self):
        self.manifest["schema_version"] = 1
        self.write(self.capture / "manifest.json", self.manifest)
        inventory = bundle.directory_inventory(self.capture, relative_to=self.capture)
        self.job.update(capture_inventory=inventory, capture_digest=bundle.directory_digest(inventory))
        with self.assertRaisesRegex(RuntimeError, "schema or instrumentation"):
            self.verify()
        self.raw_gate.assert_not_called()

    def test_mixed_job_location_and_frozen_input_drift_fail(self):
        self.job["paths"]["attempt"] = str(self.root / "other")
        with self.assertRaisesRegex(RuntimeError, "artifact paths differ"):
            self.verify()
        self.job["paths"]["attempt"] = str(self.root / "artifacts")
        for method in ("assert_source_unchanged", "assert_native_harness_unchanged"):
            with self.subTest(method=method), patch.object(closure.native, method, side_effect=RuntimeError("input drift")):
                with self.assertRaisesRegex(RuntimeError, "input drift"):
                    self.verify()
        self.raw_gate.assert_not_called()
