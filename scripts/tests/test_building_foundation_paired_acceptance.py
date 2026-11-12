from __future__ import annotations

import copy
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from scripts import building_foundation_paired_acceptance as paired
from scripts.tests.test_building_resource_inventory import boundary, owned_snapshot


def budget():
    return {"max_delta": {key: 0 for key in paired.METRICS},
            "max_relative_mad": .1, "max_gap_seconds": 180, "reason": "fixed before measurement"}


def rows(instrument="capture"):
    result = []
    epoch = datetime(2026, 11, 12, tzinfo=timezone.utc)
    for index, (mode, size, repeat, source) in enumerate(c for c in paired.matrix() if c[0] == instrument):
        resources = owned_snapshot(size, shared=source == "current")
        # Process-local handles differ and must not be compared across cases.
        for collection in ("meshes", "images", "materials"):
            resources[collection][0]["id"] += str(index)
        measured = {"p95_ms": 9., "p99_ms": 12.} if instrument == "capture" else {
            "peak_live_bytes": 1024, "max_rss_kib": 2048}
        result.append({"instrumentation": mode, "size": size, "repeat": repeat, "source": source,
            "adapter": {"name": "Intel", "driver": "same"}, "binary_sha256": source + mode,
            "layout_sha256": size, "inventory": resources,
            "started_at": (epoch + timedelta(seconds=index*100)).isoformat(),
            "finished_at": (epoch + timedelta(seconds=index*100+90)).isoformat(),
            "metrics": measured | {key: resources[key] for key in paired.RESOURCE_KEYS}})
    return result


class FoundationPairedTests(unittest.TestCase):
    def test_complete_reversed_middle_pairs_use_same_source_binaries_and_cross_process_payloads(self):
        for mode in ("capture", "memory"):
            result = paired.aggregate(rows(mode), mode, budget())
            self.assertEqual(len(result), 2)
            self.assertFalse(result[0]["resource_assessments"]["reference"]["global_shared_mesh_plateau"])
            self.assertTrue(result[0]["resource_assessments"]["current"]["global_shared_mesh_plateau"])
        self.assertEqual([c[3] for c in paired.matrix()[:6]],
                         ["reference", "current", "current", "reference", "reference", "current"])

    def test_incomplete_duplicate_reordered_or_overlapping_cases_cannot_pass(self):
        for change in ("missing", "duplicate", "order", "overlap", "long-gap", "naive-time"):
            data = rows()
            if change == "missing":
                data.pop()
            elif change == "duplicate":
                data[-1] = copy.deepcopy(data[-2])
            elif change == "order":
                data[2], data[3] = data[3], data[2]
            elif change == "overlap":
                data[1]["started_at"] = data[0]["started_at"]
            elif change == "long-gap":
                data[1]["started_at"] = "2026-11-12T00:10:00+00:00"
            else:
                data[0]["started_at"] = "2026-11-12T00:00:00"
            with self.subTest(change=change), self.assertRaises((paired.native.AcceptanceError, TypeError)):
                paired.aggregate(data, "capture", budget())

    def test_foreign_environment_binary_fixture_resource_noise_and_excess_increment_fail(self):
        for change in ("adapter", "binary", "fixture", "resource", "noise", "increment", "bool", "nan"):
            data = rows()
            if change == "adapter":
                data[-1]["adapter"] = {"name": "other"}
            elif change == "binary":
                data[-1]["binary_sha256"] = "changed"
            elif change == "fixture":
                data[-1]["layout_sha256"] = "changed"
            elif change == "resource":
                data[-1]["inventory"]["images"][0]["path"] = "other.png"
            elif change == "noise":
                for index, number in zip((1, 2, 5), (1, 20, 40)):
                    data[index]["metrics"]["p95_ms"] = number
            elif change == "increment":
                for row in data:
                    if row["source"] == "current":
                        row["metrics"]["p95_ms"] += 1
            else:
                data[0]["metrics"]["p95_ms"] = True if change == "bool" else float("nan")
            with self.subTest(change=change), self.assertRaises((paired.native.AcceptanceError, ValueError)):
                paired.aggregate(data, "capture", budget())

    def test_budget_requires_all_metrics_finite_limits_noise_and_settlement_bound_before_runs(self):
        paired.limits(budget())
        for change in ("missing", "bool", "nan", "negative", "noise", "gap", "reason"):
            value = budget()
            if change == "missing":
                value["max_delta"].pop("mesh_count")
            elif change in ("bool", "nan", "negative"):
                value["max_delta"]["p95_ms"] = {"bool": True, "nan": float("inf"), "negative": -1}[change]
            elif change == "noise":
                value["max_relative_mad"] = .11
            elif change == "gap":
                value["max_gap_seconds"] = 301
            else:
                value["reason"] = ""
            with self.subTest(change=change), self.assertRaises(paired.native.AcceptanceError):
                paired.limits(value)

    def test_command_uses_exact_one_case_prebuilt_binary_without_changing_perf_admission(self):
        value = {"job_root": "/primary/target/native-acceptance/paired", "adapter": "Intel"}
        stage = {"binaries": {"current": {"path": "/primary/target/native-acceptance/paired/capture-current/binary/bevy_app"}}}
        argv = paired.command(value, stage, ("capture", "small", 2, "current"))
        self.assertEqual(argv[:5], ["env", "RUST_LOG=info,wgpu=error", "python3", "scripts/perf.py", "run"])
        self.assertEqual(argv[argv.index("--repeat")+1], "1")
        self.assertEqual(argv[-3:], ["--skip-build", "--binary", stage["binaries"]["current"]["path"]])
        self.assertNotIn("--building-m6-mode", argv)

    def test_preparation_returns_the_standard_coordinator_wrappable_launcher_without_running_it(self):
        root = Path("/not-created/foundation-case")
        argv = ["python3", "scripts/perf.py", "run", "--skip-build", "--binary", "/frozen/bevy_app"]
        with patch.object(Path, "exists", return_value=False), \
             patch.object(paired.native, "resource_snapshot", return_value={"failures": []}), \
             patch.object(paired.native, "run_command") as run:
            result = paired.prepared(Path("/reference"), root, argv)
            self.assertEqual(result["launcher_command"], ["kitty", "--directory", "/reference", "--detach", *argv])
            self.assertEqual(result["job_root"], str(root))
            self.assertFalse(result["accepted"])
            run.assert_not_called()
        with patch.object(Path, "exists", return_value=True), self.assertRaises(paired.native.AcceptanceError):
            paired.prepared(Path("/reference"), root, argv)

    def test_stage_rejects_changed_plan_before_reading_any_raw_or_registering(self):
        stage = {"plan": "/private/plan", "plan_sha256": "before", "stage_path": "/private/stage"}
        with patch.object(paired.native, "read_json", return_value=stage), \
             patch.object(paired.static, "sha256", return_value="changed"), \
             patch.object(paired, "check_plan") as check:
            with self.assertRaisesRegex(paired.native.AcceptanceError, "plan changed"):
                paired.check_stage(Path("/private/stage"))
            check.assert_not_called()

    def test_case_joins_actual_raw_to_exact_registered_source_command_root_and_exit(self):
        case = ("capture", "small", 1, "current")
        stage = {"created_at": "2026-11-11T00:00:00+00:00", "instrumentation": "capture", "stage_path": "/private/stage",
                 "binaries": {"current": {"path": "/snapshot/binary/bevy_app", "sha256": "a"*64}}}
        planned = {"job_root": "/primary/target/native-acceptance/comparison", "adapter": "Intel",
                   "repos": {"current": "/primary"}, "identities": {"current": {"subject_commit": "c"*40}}}
        identifier = "comparison-" + paired.label(case)
        batch = {"id": identifier, "repo": "/primary", "roots": [planned["job_root"]+"/"+paired.label(case)],
                 "command": paired.command(planned, stage, case), "subject": "c"*40, "exit_code": 0,
                 "phase": "finalized", "result": "pass", "started_at": "2026-11-12T00:00:00+00:00",
                 "finished_at": "2026-11-12T00:01:30+00:00"}
        batch["verify_command"] = paired.case_verifier(stage, planned, case)
        batch["command_files"] = [{"path": str(path), "sha256": "d"*64}
                                  for path in (Path(stage["stage_path"]), Path(paired.__file__).resolve())]
        observed = {"binary_sha256": "a"*64, "session_sha256": "b"*64,
                    "observations": [{"adapter": {"name": "Intel"}, "layout_sha256": "layout",
                                      "metrics": {"p95_ms": 9., "p99_ms": 12.}}]}
        for change in (None, "repo", "command", "roots", "subject", "exit_code", "bool-exit", "phase", "result",
                       "binary", "verify-command", "stage-hash", "helper-hash"):
            actual, raw = copy.deepcopy(batch), copy.deepcopy(observed)
            if change in ("repo", "subject"):
                actual[change] = "foreign"
            elif change == "command":
                actual[change] = ["arbitrary", "command"]
            elif change == "roots":
                actual[change].append("/foreign")
            elif change == "exit_code":
                actual[change] = 1
            elif change == "bool-exit":
                actual["exit_code"] = False
            elif change == "phase":
                actual[change] = "running"
            elif change == "result":
                actual[change] = "invalid"
            elif change == "binary":
                raw["binary_sha256"] = "changed"
            elif change == "verify-command":
                actual["verify_command"] = ["python3", "/fake", "verify"]
            elif change == "stage-hash":
                actual["command_files"][0]["sha256"] = "prior-budget-stage"
            elif change == "helper-hash":
                actual["command_files"][1]["sha256"] = "changed"
            with self.subTest(change=change), \
                 patch.object(paired.storage, "primary_for", return_value=Path("/primary")), \
                 patch.object(paired.storage, "ledger_dir", return_value=Path("/ledger")), \
                 patch.object(paired.native, "read_json", side_effect=[{"batches": {identifier: actual}}, boundary() | {"initial": owned_snapshot(shared=True), "final": owned_snapshot(shared=True)}]), \
                 patch.object(paired.static, "verify_session", return_value=raw), \
                 patch.object(paired.static, "sha256", return_value="d"*64):
                if change is None:
                    result = paired.case_result(stage, planned, case, finalized=True)
                    self.assertEqual(result["batch"], identifier)
                    self.assertEqual(result["metrics"]["mesh_vertex_index_payload_bytes"], 194)
                else:
                    with self.assertRaises(paired.native.AcceptanceError):
                        paired.case_result(stage, planned, case, finalized=True)

    def test_overlap_includes_other_validation_started_before_stage_and_still_running(self):
        data = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00",
                 "finished_at": "2026-11-12T02:00:00+00:00"}]
        before = {"id": "other", "started_at": "2026-11-12T00:00:00+00:00",
                  "finished_at": "2026-11-12T00:59:59+00:00"}
        paired.require_no_interleaving(data, [before])
        for end in (None, "2026-11-12T01:01:00+00:00", "2026-11-12T03:00:00+00:00"):
            with self.subTest(end=end), self.assertRaisesRegex(paired.native.AcceptanceError, "interleaved"):
                paired.require_no_interleaving(data, [before | {"finished_at": end}])

    def test_stage_requires_guarded_build_receipts_and_distinct_source_binaries(self):
        planned = {"job_root": "/root"}
        def binary(source, sha):
            return {"path": f"/root/capture-{source}/binary/bevy_app", "sha256": sha}
        stage = {"profile": paired.PROFILE, "stage_path": "/private/stage", "plan": "/private/plan",
                 "plan_sha256": "p"*64, "instrumentation": "capture", "prior_stage": None,
                 "created_at": "2026-11-12T01:00:00+00:00",
                 "binaries": {"reference": binary("reference", "a"*64), "current": binary("current", "b"*64)}}
        for change in (None, "foreign-receipt", "same-binary", "late-build"):
            actual = copy.deepcopy(stage)
            if change == "same-binary":
                actual["binaries"]["current"]["sha256"] = "a"*64
            receipts = [{"binary": copy.deepcopy(value), "build_finished_at": "2026-11-12T00:00:00+00:00"}
                        for value in actual["binaries"].values()]
            if change == "foreign-receipt":
                receipts[0]["binary"]["sha256"] = "foreign"
            elif change == "late-build":
                receipts[0]["build_finished_at"] = "2026-11-12T02:00:00+00:00"
            with self.subTest(change=change), \
                 patch.object(paired.native, "read_json", side_effect=[actual, planned]), \
                 patch.object(paired.static, "sha256", return_value="p"*64), \
                 patch.object(paired, "check_plan"), patch.object(paired.executable, "validate"), \
                 patch.object(paired, "verify_build", side_effect=receipts):
                if change is None:
                    paired.check_stage(Path("/private/stage"))
                else:
                    with self.assertRaises(paired.native.AcceptanceError):
                        paired.check_stage(Path("/private/stage"))

    def test_memory_build_cannot_create_output_without_completed_capture(self):
        planned = {"repos": {"current": "/primary"}, "job_root": "/job"}
        args = SimpleNamespace(plan=Path("/private/plan"), frozen_plan=planned,
                               repo=Path("/primary"), source="current", instrumentation="memory",
                               job_root=Path("/job/memory-current"), prior_stage=None)
        with patch.object(paired.native, "read_json", return_value=planned), \
             patch.object(paired, "check_plan"), patch.object(paired.native, "run_command") as run:
            with self.assertRaisesRegex(paired.native.AcceptanceError, "completed Capture"):
                paired.build.__wrapped__(args)
            run.assert_not_called()
        for instrument, feature in (("capture", "profiling"), ("memory", "profiling-memory")):
            argv = paired.cargo_command(instrument)
            self.assertEqual(argv[argv.index("--features")+1], feature)
            self.assertEqual(argv[argv.index("--profile")+1], "profiling")
            self.assertIn("--no-default-features", argv)

    def test_guarded_build_receipt_requires_exact_source_recipe_plan_and_successful_registration(self):
        plan_path = Path("/private/plan")
        planned = {"repos": {"current": "/primary"}, "job_root": "/root",
                   "identities": {"current": {"subject_commit": "a"*40}}}
        command = paired.build_command(plan_path, planned, "current", "capture", None)
        verifier = command.copy()
        verifier[2] = "verify-build"
        receipt = {"plan_sha256": "p"*64, "source": "current", "instrumentation": "capture",
                   "identity": planned["identities"]["current"], "cargo_command": paired.cargo_command("capture"),
                   "build_log_sha256": "p"*64, "prior_stage_sha256": None,
                   "binary": {"path": "/root/capture-current/binary/bevy_app", "sha256": "b"*64}}
        batch = {"id": "root-build-capture-current", "repo": "/primary", "roots": ["/root/capture-current"],
                 "subject": "a"*40, "command": command, "verify_command": verifier, "exit_code": 0,
                 "phase": "finalized", "result": "pass", "started_at": "2026-11-12T00:00:00+00:00",
                 "finished_at": "2026-11-12T00:01:00+00:00",
                 "command_files": [{"path": str(path), "sha256": "p"*64}
                                   for path in (plan_path, Path(paired.__file__).resolve())]}
        for change in (None, "source", "identity", "feature", "recipe", "subject", "plan", "exit", "unsealed"):
            actual, evidence = copy.deepcopy(batch), copy.deepcopy(receipt)
            if change == "source":
                evidence["source"] = "reference"
            elif change == "identity":
                evidence["identity"]["subject_commit"] = "foreign"
            elif change == "feature":
                evidence["cargo_command"] = paired.cargo_command("memory")
            elif change == "recipe":
                actual["command"] = ["arbitrary"]
            elif change == "subject":
                actual["subject"] = "foreign"
            elif change == "plan":
                actual["command_files"][0]["sha256"] = "changed-after-build"
            elif change == "exit":
                actual["exit_code"] = 1
            elif change == "unsealed":
                actual["phase"] = "needs-finalize"
            with self.subTest(change=change), \
                 patch.object(paired.native, "read_json", side_effect=[planned, evidence, {"batches": {actual["id"]: actual}}]), \
                 patch.object(paired, "check_plan"), \
                 patch.object(paired.static, "sha256", return_value="p"*64), \
                 patch.object(paired.storage, "primary_for", return_value=Path("/primary")), \
                 patch.object(paired.storage, "ledger_dir", return_value=Path("/ledger")), \
                 patch.object(paired.executable, "validate"):
                if change is None:
                    self.assertEqual(paired.verify_build(plan_path, "current", "capture", None, finalized=True)["binary"],
                                     evidence["binary"])
                else:
                    with self.assertRaises(paired.native.AcceptanceError):
                        paired.verify_build(plan_path, "current", "capture", None, finalized=True)


if __name__ == "__main__":
    unittest.main()
