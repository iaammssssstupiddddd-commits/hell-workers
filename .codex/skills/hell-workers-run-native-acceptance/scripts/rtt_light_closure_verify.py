"""Read-only revalidation of the bounded P08 closure and its frozen raw inputs."""
from copy import deepcopy
from pathlib import Path
import sys

import native_acceptance as native


def verify_renderer_gates(contract, cases):
    from perf_tool.rtt_light_contract import expected_gate_result_rows
    from perf_tool.rtt_light_bundle import _gate_observed, _comparison_passes

    required = {"RLV1-P01-RTT", "RLV1-P02-PRESENT", "RLV1-P06-RENDER",
                "RLV1-P06-COLOR", "RLV1-P08-CROSS-CONSUMER", "RLV1-P06-UPLOAD"}
    rows = [row for row in expected_gate_result_rows(contract, "p08")
            if row["case_id"] == "renderdoc-medium-gpu" and row["gate_id"] in required
            and (row["gate_id"] != "RLV1-P06-UPLOAD"
                 or row["metric_id"] in {"field_image_count", "field_handle_count"})]
    native.require({row["gate_id"] for row in rows} == required, "closure renderer gate coverage differs")
    native.require({row["metric_id"] for row in rows if row["gate_id"] == "RLV1-P06-UPLOAD"}
                   == {"field_image_count", "field_handle_count"}, "closure field image/handle gate coverage differs")
    for row in rows:
        native.require(row.get("reference_stage") is None, "closure renderer gate requires a reference stage")
        observed = _gate_observed(row, cases, {}, {})
        native.require(_comparison_passes(observed, row["threshold"], row["comparator"], row["value_type"]),
                       f"closure renderer gate failed: {row['gate_id']}/{row['metric_id']}={observed}")


def verify(args):
    repo = native.validate_repo(args.repo)
    root = Path(args.job_root).resolve()
    native.require(root.is_relative_to(repo / "target/native-acceptance"), "closure job is outside its subject")
    job = native.read_json(root / "job.json")
    native.require(job.get("profile") == "rtt-light" and job.get("measurement_kind") == "p08-closure"
                   and job.get("status") == "valid", "expected a valid bounded closure")
    native.require(job.get("repo") == str(repo) and job.get("job_root") == str(root), "closure job location differs")
    native.require(job.get("window_backend") == args.window_backend, "closure window backend differs")
    native.assert_clean_subject(repo, job["subject_commit"])
    native.assert_source_unchanged(repo, job["source_fingerprint"])
    native.assert_native_harness_unchanged(repo, job["harness_fingerprint"])
    attempt = root / "artifacts"
    native.require(job["paths"]["attempt"] == str(attempt)
                   and job["paths"]["environment_lock"] == str(attempt / "environment-lock.json"), "closure artifact paths differ")
    sys.path.insert(0, str(repo / "scripts"))
    from perf_tool.model import Case
    from perf_tool.policy import validate_session_artifact_set
    from perf_tool.renderdoc_foundation import DIAGNOSTIC_NAMESPACE, verify_capsule_hash
    from perf_tool.renderdoc_capture import _validate_environment_lock, _validate_capture_session
    from perf_tool.rtt_light_bundle import (
        directory_inventory, directory_digest, _revalidate_run, _load_renderdoc_evidence,
    )
    capture = attempt / "capture"
    inventory = directory_inventory(capture, relative_to=capture)
    native.require(job.get("capture_inventory") == inventory
                   and job.get("capture_digest") == directory_digest(inventory), "closure Capture files changed or were never sealed")
    manifest = native.read_json(capture / "manifest.json")
    native.require(manifest.get("schema_version") == 2 and manifest.get("binary", {}).get("instrumentation") == "capture", "closure Capture schema or instrumentation differs")
    errors = validate_session_artifact_set(capture, manifest)
    native.require(not errors, f"closure Capture artifact set: {errors}")
    lock = native.read_json(attempt / "environment-lock.json")
    _validate_capture_session(manifest, commit=job["subject_commit"], fingerprint=job["source_fingerprint"], binary_hash=lock["capture_binary_sha256"])
    native.require(native.manifest_binary_hash(capture, repo) == lock["capture_binary_sha256"], "closure Capture binary differs from environment lock")
    contract = native.rtt_light_contract(repo, "p08")
    # Reuse the raw run verifier with the closure's fixed 3/5-second timings.
    # This local view is never written back or used as a formal baseline.
    closure_contract = deepcopy(contract)
    closure_contract["formal_matrix"]["capture"].update(warmup_secs=3.0, measure_secs=5.0)
    case = Case("indoor-light", "medium", "gpu", contract["formal_matrix"]["seed"], None, None)
    for preflight, name in ((True, "preflight-001"), (False, "run-001")):
        _revalidate_run(run_dir=capture / "cases" / case.identifier / name, expected_case=case,
                        contract=closure_contract, stage="p08", leg_id="capture", job=job,
                        environment_lock=lock, preflight=preflight)
    renderdoc = Path(job["paths"]["renderdoc"])
    foundation = renderdoc.parent.parent
    native.require(renderdoc.is_absolute() and renderdoc.resolve() == renderdoc
                   and foundation.parent == repo / DIAGNOSTIC_NAMESPACE
                   and renderdoc.relative_to(foundation).as_posix() == "closure/renderdoc", "closure RenderDoc path differs")
    rd_manifest = native.read_json(renderdoc / "manifest.json")
    binary = Path(rd_manifest["binary"]["path"])
    native.require(binary == foundation / "capsule/renderdoc/bevy_app" and binary.resolve() == binary, "RenderDoc binary is outside the sealed foundation")
    capsule = verify_capsule_hash(binary.parent)
    native.require(capsule.leg == "renderdoc" and capsule.profile == "profiling-renderdoc"
                   and capsule.features == "profiling-renderdoc"
                   and capsule.cargo_lock_sha256 == native.sha256(repo / "Cargo.lock"), "closure capsule build contract differs")
    _validate_environment_lock(lock, contract=contract, commit=job["subject_commit"],
                               fingerprint=job["source_fingerprint"], adapter_filter=args.adapter,
                               window_backend=args.window_backend, binary_sha256=capsule.binary_sha256, stage="p08")
    _, cases = _load_renderdoc_evidence(attempt=renderdoc.parent, contract=contract, stage="p08", job=job, environment_lock=lock)
    verify_renderer_gates(contract, cases)
    result = native.verify_rtt_light_closure(capture=capture, renderdoc=renderdoc,
        adapter=args.adapter, window_backend=args.window_backend, subject_commit=job["subject_commit"],
        source_fingerprint_value=job["source_fingerprint"], renderdoc_binary_sha256=capsule.binary_sha256)
    native.require(result == job["verification"], "closure result differs from the sealed outcome")
    return result
