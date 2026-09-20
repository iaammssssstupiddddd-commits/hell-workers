import copy
import hashlib
import json
import sys
import tempfile
import unittest
from contextlib import ExitStack
from dataclasses import asdict
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from perf_tool.artifact_readers.building_art_static import expected_records, read_building_art_static
from perf_tool.artifact_readers.workload import read_workload_sidecars
from perf_tool.artifacts import measurement_duration_clock
from perf_tool.arguments import build_parser, validate_arguments
from perf_tool.model import Case
from perf_tool.model import Validation
import building_art_static_acceptance as acceptance


def sidecar(copies):
    evidence = {"records": expected_records(copies), "target_count": copies * 9,
                "target_structural_roots": copies * 5, "target_foreground_owners": copies * 4,
                "target_active_unique_meshes": 2, "souls": copies // 4 * 15, "completion_effects": 0}
    digest = hashlib.sha256(json.dumps(evidence, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"schema_version": 1, "contract_id": "building-art-static-nine-v2", "evidence_kind": "paused-static-only",
            "active_simulation_evidence": False, "camera_scale": 5.0, "stable_frames": 100,
            "initial": evidence, "final": copy.deepcopy(evidence), "layout_sha256": digest}


class BuildingArtStaticTests(unittest.TestCase):
    def check_value(self, value, *, size="small"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "building_art_static.json").write_text(json.dumps(value))
            return read_building_art_static(root, expected_case=Case("building-art-static", size, "gpu", 20260920, 15 if size == "small" else 60, 0))

    def test_exact_small_and_medium(self):
        for size, copies in (("small", 4), ("medium", 16)):
            self.assertEqual(self.check_value(sidecar(copies), size=size)[1], [])
            records = expected_records(copies)
            self.assertEqual(len(records), copies * 9)
            self.assertEqual(len({row["kind"] for row in records}), 9)
            self.assertNotIn("Bridge", {row["kind"] for row in records})

    def test_old_ten_kind_contract_and_bridge_injection_are_rejected(self):
        value = sidecar(4)
        value["contract_id"] = "building-art-static-v1"
        self.assertTrue(self.check_value(value)[1])
        value = sidecar(4)
        value["initial"]["records"][0]["kind"] = "Bridge"
        value["final"] = copy.deepcopy(value["initial"])
        value["layout_sha256"] = hashlib.sha256(json.dumps(value["initial"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        self.assertTrue(self.check_value(value)[1])

    def test_corruption_and_false_active_claim_are_rejected(self):
        for key, invalid in (("schema_version", True), ("active_simulation_evidence", True), ("stable_frames", True),
                             ("stable_frames", 0), ("camera_scale", 4), ("layout_sha256", "0" * 64),
                             ("contract_id", "door-density-v1"), ("initial", None)):
            with self.subTest(key=key, invalid=invalid):
                value = sidecar(4)
                value[key] = invalid
                self.assertTrue(self.check_value(value)[1])

    def test_rehashed_self_consistent_wrong_state_is_not_accepted(self):
        value = sidecar(4)
        value["initial"]["records"][1]["state"]["stored_water"] = 50
        value["final"] = copy.deepcopy(value["initial"])
        value["layout_sha256"] = hashlib.sha256(json.dumps(value["initial"], sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        self.assertTrue(self.check_value(value)[1])

    def test_duplicate_missing_and_wrong_owner_records(self):
        for change in (lambda records: records.pop(), lambda records: records.append(records[0]),
                       lambda records: records[0].update(kind="Wall"),
                       lambda records: records[0].update(ordinal=True)):
            value = sidecar(4)
            change(value["initial"]["records"])
            self.assertTrue(self.check_value(value)[1])

    def test_malformed_json_missing_file_and_cross_workload_are_rejected(self):
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertTrue(read_building_art_static(root, expected_case=case)[1])
            for text in ('{"schema_version":1,"schema_version":1}', '{"camera_scale":NaN}', '[]'):
                (root / "building_art_static.json").write_text(text)
                self.assertTrue(read_building_art_static(root, expected_case=case)[1])
            result = read_workload_sidecars(root, expected_case=Case("gather", "small", "gpu", 1, 1, 1),
                capture_kind="frame-time", expected_contract=None, expected_stage=None, expected_lane=None)
            self.assertTrue(any("forbidden" in reason for reason in result.reasons))

    def test_static_uses_real_clock_only(self):
        self.assertEqual(measurement_duration_clock("building-art-static"), ("real", True))
        self.assertEqual(measurement_duration_clock("gather"), ("virtual", False))

    def test_cli_requires_exact_formal_contract(self):
        argv = ["run", "--workload", "building-art-static", "--sizes", "small", "--renders", "gpu",
                "--seed", "20260920", "--repeat", "3", "--preflight-runs", "0", "--souls", "15",
                "--familiars", "0", "--warmup-secs", "30", "--measure-secs", "60", "--backend", "vulkan",
                "--window-backend", "x11", "--present-mode", "novsync", "--window-width", "1280",
                "--window-height", "720", "--window-scale-factor", "1", "--rtt-quality", "high"]
        validate_arguments(build_parser().parse_args(argv))
        for flag, bad in (("--souls", "0"), ("--sizes", "small,medium"), ("--repeat", "1"),
                          ("--warmup-secs", "1"), ("--seed", "20260906")):
            invalid = argv.copy()
            invalid[invalid.index(flag) + 1] = bad
            with self.subTest(flag=flag), self.assertRaises(ValueError):
                validate_arguments(build_parser().parse_args(invalid))


class NativeStaticReferenceTests(unittest.TestCase):
    def test_launcher_matrix_uses_public_guarded_runner(self):
        for instrumentation, size, souls in acceptance.MATRIX:
            argv = acceptance.command(Path("/reference"), instrumentation, size, souls, "Intel")
            self.assertEqual(argv[:3], ["python3", "scripts/perf.py", "run"])
            args = build_parser().parse_args(argv[2:])
            validate_arguments(args)
            self.assertEqual((args.instrumentation, args.souls), (instrumentation, souls))
            self.assertNotIn("--skip-build", argv)

    def test_verifier_recomputes_memory_and_rejects_tampered_artifacts(self):
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            root = Path(directory)
            session = root / "memory-medium"
            case = Case("building-art-static", "medium", "gpu", 20260920, 60, 0)
            frozen = {"subject_commit": "c" * 40, "source_fingerprint": "s" * 64}
            matrix = {"workload": case.workload, "sizes": ["medium"], "renders": ["gpu"], "seed": 20260920,
                      "repeat": 3, "preflight_runs": 0, "warmup_secs": 30.0, "measure_secs": 60.0,
                      "souls": 60, "familiars": 0, "familiar_policies": ["baseline"],
                      "operation_dialog_modes": ["hidden"], "dashboard_modes": ["hidden"],
                      "capture_kind": "frame-time", "clock_mode": "realtime", "window_width": 1280,
                      "window_height": 720, "window_scale_factor": 1.0, "rtt_quality": "high"}
            session.mkdir()
            (session / "matrix.json").write_text(json.dumps(matrix))
            manifest = {"status": "valid", "git": {"commit": frozen["subject_commit"], "dirty_paths": []},
                        "source": {"fingerprint_start": frozen["source_fingerprint"],
                                   "fingerprint_end": frozen["source_fingerprint"], "unchanged": True},
                        "binary": {"instrumentation": "memory", "sha256": "b" * 64}}
            (session / "manifest.json").write_text(json.dumps(manifest))
            allocation = {"peak_live_bytes": 100}
            process = {"max_rss_kib": 200}
            artifact = {"instrumentation": "memory", "allocation_memory": allocation, "process_memory": process}
            validation = Validation(True, [], {"samples": "3"}, {"name": "Intel"}, [], [])
            validation.window = {"backend": "x11"}
            validation.profile_artifact = artifact
            for number in range(1, 4):
                run = session / "cases" / case.identifier / f"run-{number:03d}"
                (run / "data").mkdir(parents=True)
                (run / "validation.json").write_text(json.dumps(validation.to_json()))
                (run / "profile-artifact.json").write_text(json.dumps(artifact))
                (run / "run-metadata.json").write_text(json.dumps({
                    "case": asdict(case), "returncode": 0, "runtime_data_cleaned": True,
                    "actual_adapter": validation.adapter, "actual_window": validation.window}))
                (run / "data/building_art_static.json").write_text(json.dumps(sidecar(16)))
            validation.profile_artifact = None
            stack.enter_context(patch.object(acceptance, "validate_run", side_effect=lambda *a, **k: copy.deepcopy(validation)))
            stack.enter_context(patch.object(acceptance, "read_frames", return_value=([1., 2., 3.], [])))
            stack.enter_context(patch.object(acceptance, "read_native_memory", return_value=(allocation, [])))
            stack.enter_context(patch.object(acceptance, "read_resource_usage", return_value=(process, [])))
            result = acceptance.verify_session(root, "memory", "medium", 60, frozen, "Intel")
            self.assertEqual(result["aggregate"]["peak_live_bytes"], {"median": 100, "mad": 0})
            self.assertEqual(len(result["observations"]), 3)
            (run / "profile-artifact.json").write_text(json.dumps({**artifact, "process_memory": {"max_rss_kib": 1}}))
            with self.assertRaisesRegex(acceptance.native.AcceptanceError, "raw counters"):
                acceptance.verify_session(root, "memory", "medium", 60, frozen, "Intel")
            (run / "profile-artifact.json").write_text(json.dumps(artifact))
            (run / "validation.json").write_text("{}")
            with self.assertRaisesRegex(acceptance.native.AcceptanceError, "stored validation"):
                acceptance.verify_session(root, "memory", "medium", 60, frozen, "Intel")
            manifest["git"]["dirty_paths"] = ["dirty.rs"]
            (session / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(acceptance.native.AcceptanceError, "subject differs"):
                acceptance.verify_session(root, "memory", "medium", 60, frozen, "Intel")
