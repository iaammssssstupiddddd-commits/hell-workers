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
    def test_real_frame_summary_diagnostics_do_not_extend_the_fixed_budget(self):
        raw = paired.static.frame_summary([float(n) for n in range(1, 101)])
        self.assertEqual(set(raw), {"p50_ms", "p95_ms", "p99_ms", "max_ms"})
        data = rows()
        for row in data:
            row["metrics"] = paired.measured_metrics(raw, "capture") | {
                key: row["inventory"][key] for key in paired.RESOURCE_KEYS}
        self.assertEqual(len(paired.aggregate(data, "capture", budget())), 2)
        self.assertEqual(paired.measured_metrics({"peak_live_bytes": 12, "max_rss_kib": 34}, "memory"),
                         {"peak_live_bytes": 12, "max_rss_kib": 34})

    def test_metric_selection_rejects_missing_invalid_and_foreign_instrumentation(self):
        for instrument in ("capture", "memory"):
            keys = ("p95_ms", "p99_ms") if instrument == "capture" else ("peak_live_bytes", "max_rss_kib")
            good = dict.fromkeys(keys, 12)
            for invalid in (None, {}, {keys[0]: 12}, {**good, keys[0]: True},
                            {**good, keys[0]: -1}, {**good, keys[0]: float("nan")}):
                with self.subTest(instrument=instrument, raw=invalid), self.assertRaises(paired.native.AcceptanceError):
                    paired.measured_metrics(invalid, instrument)
        with self.assertRaises(paired.native.AcceptanceError):
            paired.measured_metrics({}, "foreign")

    def test_aggregate_rejects_missing_or_unbudgeted_raw_metric_before_comparison(self):
        for instrument in ("capture", "memory"):
            for extra in (False, True):
                data = rows(instrument)
                if extra:
                    data[0]["metrics"]["p50_ms"] = 1.
                else:
                    data[0]["metrics"].pop("p95_ms" if instrument == "capture" else "peak_live_bytes")
                with self.subTest(instrument=instrument, extra=extra), self.assertRaises(paired.native.AcceptanceError):
                    paired.aggregate(data, instrument, budget())

    def test_capture_proof_is_local_and_rejects_foreign_paths_plans_and_changed_hashes(self):
        proof = paired._CaptureProof(Path("/capture"), "c"*64, Path("/plan"), "p"*64,
                                     {"instrumentation": "capture", "stage_sha256": "c"*64})
        def sha(path):
            return "c"*64 if path == Path("/capture") else "p"*64
        with patch.object(paired.static, "sha256", side_effect=sha):
            self.assertEqual(paired._capture_result(proof, Path("/capture"), Path("/plan")), proof.result)
            for cached, stage, plan in [(None, "/capture", "/plan"), (proof.__dict__, "/capture", "/plan"),
                                        (proof, "/foreign", "/plan"), (proof, "/capture", "/foreign")]:
                with self.subTest(stage=stage, plan=plan), self.assertRaises(paired.native.AcceptanceError):
                    paired._capture_result(cached, Path(stage), Path(plan))
        with patch.object(paired.static, "sha256", return_value="changed"), self.assertRaises(paired.native.AcceptanceError):
            paired._capture_result(proof, Path("/capture"), Path("/plan"))

    def test_proof_is_created_only_from_full_verification_each_time(self):
        result = {"instrumentation": "capture", "stage_sha256": "c"*64}
        with patch.object(paired, "verify_stage", return_value=result) as verify, \
             patch.object(paired.native, "read_json", return_value={"plan": "/plan", "plan_sha256": "p"*64}), \
             patch.object(paired.static, "sha256", side_effect=lambda path: "c"*64 if path == Path("/capture") else "p"*64):
            first = paired._verified_capture(Path("/capture"), Path("/plan"))
            second = paired._verified_capture(Path("/capture"), Path("/plan"))
            self.assertIsNot(first, second)
            self.assertEqual(verify.call_count, 2)
        with patch.object(paired, "verify_stage", side_effect=ValueError("raw failed")), self.assertRaises(ValueError):
            paired._verified_capture(Path("/capture"), Path("/plan"))

    def test_memory_stage_verifies_capture_once_then_checks_both_build_receipts(self):
        stage = {"profile": paired.PROFILE, "stage_path": "/memory", "plan": "/plan", "plan_sha256": "p"*64,
                 "instrumentation": "memory", "prior_stage": "/capture", "prior_stage_sha256": "c"*64,
                 "created_at": "2026-11-12T02:00:00+00:00",
                 "binaries": {source: {"path": "/root/"+source, "sha256": letter*64}
                              for source, letter in (("reference", "r"), ("current", "s"))}}
        old = {"binaries": {source: {"sha256": letter*64} for source, letter in (("reference", "a"), ("current", "b"))}}
        planned = {"job_root": "/root"}
        proof = paired._CaptureProof(Path("/capture"), "c"*64, Path("/plan"), "p"*64,
            {"instrumentation": "capture", "stage_sha256": "c"*64,
             "rows": [{"finished_at": "2026-11-12T00:00:00+00:00"}]})
        documents = {"/memory": stage, "/plan": planned, "/capture": old}
        def receipt(_plan, source, *_args, **kwargs):
            self.assertIs(kwargs["captured"], proof)
            self.assertTrue(kwargs["finalized"])
            return {"binary": stage["binaries"][source], "build_finished_at": "2026-11-12T01:00:00+00:00"}
        with patch.object(paired.native, "read_json", side_effect=lambda path: documents[str(path)]), \
             patch.object(paired.static, "sha256", side_effect=lambda path: "c"*64 if path == Path("/capture") else "p"*64), \
             patch.object(paired, "check_plan"), patch.object(paired.executable, "validate"), \
             patch.object(paired, "_verified_capture", return_value=proof) as verified, \
             patch.object(paired, "_verify_build", side_effect=receipt) as builds:
            paired.check_stage(Path("/memory"))
            verified.assert_called_once_with(Path("/capture"), Path("/plan"))
            self.assertEqual(builds.call_count, 2)

    def test_standalone_memory_build_verifier_still_fully_checks_capture(self):
        proof = object()
        with patch.object(paired, "_verified_capture", return_value=proof) as verify, \
             patch.object(paired, "_verify_build", return_value={"checked": True}) as receipt:
            self.assertEqual(paired.verify_build(Path("/plan"), "current", "memory", Path("/capture")), {"checked": True})
            verify.assert_called_once_with(Path("/capture"), Path("/plan"))
            self.assertIs(receipt.call_args.kwargs["captured"], proof)

    def test_memory_result_reuses_same_call_proof_without_dropping_cross_stage_checks(self):
        data = rows("memory")
        previous = {"instrumentation": "capture", "stage_sha256": "c"*64,
                    "rows": [{"source": source, "finished_at": "2026-11-11T23:00:00+00:00",
                              "inventory": owned_snapshot(shared=source=="current")} for source in paired.SOURCES]}
        proof = paired._CaptureProof(Path("/capture"), "c"*64, Path("/plan"), "p"*64, previous)
        stage = {"instrumentation": "memory", "prior_stage": "/capture", "plan": "/plan"}
        planned = {"repos": {"current": "/primary"}, "limits": budget()}
        with patch.object(paired, "_check_stage", return_value=(stage, planned, proof)), \
             patch.object(paired, "case_result", side_effect=data), \
             patch.object(paired.static, "sha256", side_effect=lambda path: "c"*64 if path == Path("/capture") else "p"*64), \
             patch.object(paired.storage, "primary_for", return_value=Path("/primary")), \
             patch.object(paired.storage, "ledger_dir", return_value=Path("/ledger")), \
             patch.object(paired.native, "read_json", return_value={"batches": {}}), \
             patch.object(paired, "require_no_interleaving"):
            result = paired.verify_stage(Path("/memory"))
            self.assertFalse(result["cross_stage_resources"]["reference"]["global_shared_mesh_plateau"])
            self.assertTrue(result["cross_stage_resources"]["current"]["global_shared_mesh_plateau"])
            data[0]["inventory"]["images"][0]["path"] = "changed"
            with patch.object(paired, "case_result", side_effect=data), self.assertRaises(ValueError):
                paired.verify_stage(Path("/memory"))

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
                                      "metrics": paired.static.frame_summary([9., 10., 11., 12.])}]}
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
                    self.assertEqual(set(result["metrics"]), paired.RESOURCE_KEYS | {"p95_ms", "p99_ms"})
                else:
                    with self.assertRaises(paired.native.AcceptanceError):
                        paired.case_result(stage, planned, case, finalized=True)

    def test_formal_closed_history_is_a_conservative_upper_bound_never_a_fake_finish(self):
        rows = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00",
                 "finished_at": "2026-11-12T02:00:00+00:00"}]
        closed = {"id": "historical-interrupted", "phase": "finalized", "result": "interrupted", "exit_code": 130,
                  "started_at": "2026-11-10T00:00:00+00:00", "finished_at": None,
                  "verified_at": "2026-11-11T00:00:00+00:00", "finalized_at": "2026-11-11T01:00:00+00:00",
                  "process_identity": None, "child_identity": None}
        original = copy.deepcopy(closed)
        result = paired.require_no_interleaving(rows, [closed])
        self.assertEqual(closed, original)
        self.assertEqual(result["closed_history_bounds"][0], {
            "batch": closed["id"], "end_upper_bound": closed["finalized_at"],
            "basis": "finalized_at_upper_bound", "historical_result": "interrupted",
            "start_lower_bound": closed["started_at"], "start_lower_bound_basis": "started_at"})
        for change in ({"phase": "running"}, {"phase": "sealing"}, {"phase": "needs-finalize"},
                       {"result": "unknown"}, {"result": "pass"}, {"exit_code": None}, {"exit_code": False},
                       {"exit_code": 0}, {"exit_code": 999},
                       {"process_identity": "live-owner"}, {"child_identity": "live-child"},
                       {"verified_at": None}, {"finalized_at": None},
                       {"finalized_at": "2026-11-12T01:30:00+00:00"},
                       {"finalized_at": "2026-11-10T01:00:00+00:00"},
                       {"finished_at": "2026-11-09T00:00:00+00:00"}):
            with self.subTest(change=change), patch.object(paired, "_boot_boundary", return_value={"boot_id": "a"*36, "started_at": "2026-11-11T00:00:00+00:00"}), self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [closed | change])

        for key in ("process_identity", "child_identity"):
            missing = {k: v for k, v in closed.items() if k != key}
            with self.subTest(missing=key), self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [missing])
        with self.assertRaises(ValueError):
            paired.require_no_interleaving(rows, [closed | {"finished_at": ""}])

    def test_missing_start_only_allows_formally_closed_missing_time_history(self):
        rows = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00", "finished_at": "2026-11-12T02:00:00+00:00"}]
        closed = {"id": "unstarted", "phase": "finalized", "result": "abandoned", "exit_code": None,
                  "created_at": "2026-11-10T00:00:00+00:00", "verified_at": "2026-11-10T01:00:00+00:00",
                  "finalized_at": "2026-11-10T02:00:00+00:00", "process_identity": None, "child_identity": None}
        result = paired.require_no_interleaving(rows, [closed])
        self.assertEqual(result["closed_history_bounds"][0]["basis"], "closed_missing_execution_times_upper_bound")
        for change in ({"phase": "registered"}, {"phase": "running"}, {"phase": "needs-finalize"}, {"phase": "unknown"},
                       {"result": "pass"}, {"exit_code": 130}, {"verified_at": None}, {"finalized_at": None}):
            with self.subTest(change=change), self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [closed | change])

    def test_old_boot_is_positive_owner_end_bound_and_current_or_unknown_owner_still_fails(self):
        rows = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00", "finished_at": "2026-11-12T02:00:00+00:00"}]
        old = "11111111-1111-1111-1111-111111111111"
        current = "22222222-2222-2222-2222-222222222222"
        closed = {"id": "old-owner", "phase": "finalized", "result": "abandoned", "exit_code": None,
                  "created_at": "2026-09-27T00:00:00+00:00", "verified_at": "2026-09-27T01:00:00+00:00",
                  "finalized_at": "2026-09-27T02:00:00+00:00", "process_identity": None, "child_pid": 123, "child_identity": old+":123"}
        boot = {"boot_id": current, "started_at": "2026-11-11T00:00:00+00:00"}
        original = copy.deepcopy(closed)
        with patch.object(paired, "_boot_boundary", return_value=boot):
            audit = paired.require_no_interleaving(rows, [closed])["closed_history_bounds"][0]
            self.assertEqual(audit["basis"], "different_boot_owner_upper_bound")
            self.assertEqual(audit["end_upper_bound"], boot["started_at"])
            self.assertEqual(closed, original)
            for identity in (current+":123", "not-a-birth-identity", old+":0"):
                with self.subTest(identity=identity), self.assertRaises(paired.native.AcceptanceError):
                    paired.require_no_interleaving(rows, [closed | {"child_identity": identity}])
        for missing in ("child_identity", "process_identity"):
            with self.subTest(missing=missing), self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [{k:v for k,v in closed.items() if k!=missing}])
        with patch.object(paired, "_boot_boundary", return_value=boot | {"started_at": "2026-11-12T01:30:00+00:00"}), self.assertRaises(paired.native.AcceptanceError):
            paired.require_no_interleaving(rows, [closed])
        with patch.object(paired, "_boot_boundary", side_effect=paired.native.AcceptanceError("unavailable")), self.assertRaises(paired.native.AcceptanceError):
            paired.require_no_interleaving(rows, [closed])

    def test_missing_child_record_needs_current_closed_wal_or_positive_preboot_boundary(self):
        rows = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00", "finished_at": "2026-11-12T02:00:00+00:00"}]
        closed = {"id": "no-child-record", "phase": "finalized", "result": "abandoned", "exit_code": None,
                  "created_at": "2026-11-10T00:00:00+00:00", "verified_at": "2026-11-10T01:00:00+00:00",
                  "finalized_at": "2026-11-10T02:00:00+00:00", "process_identity": None, "coordinator_sha256": "current"}
        events = [{"action": action, "at": at} for action, at in [
            ("register no-child-record", "2026-11-10T00:01:00+00:00"),
            ("seal started no-child-record", "2026-11-10T00:59:00+00:00"),
            ("recorded verification result no-child-record; no archive required", "2026-11-10T01:01:00+00:00"),
            ("finalize no-child-record: abandoned", "2026-11-10T02:01:00+00:00")]]
        with patch.object(paired.storage, "digest", return_value="current"):
            audit = paired.require_no_interleaving(rows, [closed], history=events)["closed_history_bounds"][0]
            self.assertEqual(audit["basis"], "closed_registration_seal_path_upper_bound")
            self.assertEqual(audit["end_upper_bound"], events[-1]["at"])
            for history in (None, events[:-1], events[::-1], events+[{'action':'start no-child-record','at':'2026-11-10T00:30:00+00:00'}]):
                with self.subTest(history=history), self.assertRaises(paired.native.AcceptanceError):
                    paired.require_no_interleaving(rows, [closed], history=history)
            with self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [closed | {"child_pid":123}], history=events)
            legacy = closed | {"coordinator_sha256":"old"}
            boot = {"boot_id":"22222222-2222-2222-2222-222222222222", "started_at":"2026-11-11T00:00:00+00:00"}
            with patch.object(paired, "_boot_boundary", return_value=boot):
                audit = paired.require_no_interleaving(rows, [legacy])["closed_history_bounds"][0]
                self.assertEqual(audit["basis"], "closed_preboot_missing_child_upper_bound")
                self.assertEqual(audit["end_upper_bound"], boot["started_at"])
            with patch.object(paired, "_boot_boundary", return_value=boot | {"started_at":"2026-11-09T00:00:00+00:00"}), self.assertRaises(paired.native.AcceptanceError):
                paired.require_no_interleaving(rows, [legacy])

    def test_overlap_includes_other_validation_started_before_stage_and_still_running(self):
        data = [{"batch": "owned", "started_at": "2026-11-12T01:00:00+00:00",
                 "finished_at": "2026-11-12T02:00:00+00:00"}]
        before = {"id": "other", "started_at": "2026-11-12T00:00:00+00:00",
                  "finished_at": "2026-11-12T00:59:59+00:00"}
        paired.require_no_interleaving(data, [before])
        for end in (None, "2026-11-12T01:01:00+00:00", "2026-11-12T03:00:00+00:00"):
            with self.subTest(end=end), self.assertRaises(paired.native.AcceptanceError):
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
                 patch.object(paired, "_verify_build", side_effect=receipts):
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
