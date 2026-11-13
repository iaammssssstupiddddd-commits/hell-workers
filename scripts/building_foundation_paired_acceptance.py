"""Plan, guarded binary preparation and verifier for adjacent foundation pairs.

Main builds and snapshots both Capture binaries before Capture, then both Memory
binaries after all Capture cases close. Each case uses its own ordinary primary
validation registration, exact source repository and immutable --skip-build input.
No admission exception, nested batch, art or release authority is provided here.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import json
import math
import os
import re
from pathlib import Path
import statistics

if __package__:
    from . import building_art_static_acceptance as static
    from . import building_resource_inventory_acceptance as supplement
    from . import building_resource_inventory as inventory
    from . import building_native_executable as executable
    from . import validation_storage as storage
else:
    import building_art_static_acceptance as static
    import building_resource_inventory_acceptance as supplement
    import building_resource_inventory as inventory
    import building_native_executable as executable
    import validation_storage as storage

native = static.native
PROFILE = "building-foundation-adjacent-static-v1"
CUMULATIVE_PROFILE = "building-cumulative-adjacent-static-v1"
SOURCES = ("reference", "current")
RESOURCE_KEYS = inventory.TOTALS - {"material_shallow_size"}
METRICS = {"p95_ms", "p99_ms", "peak_live_bytes", "max_rss_kib", *RESOURCE_KEYS}


def matrix():
    return [(instrument, size, repeat, source)
            for instrument in ("capture", "memory") for size in ("small", "medium")
            for repeat in (1, 2, 3)
            for source in (SOURCES[::-1] if repeat == 2 else SOURCES)]


def label(case):
    return "-".join(map(str, case))


def limits(value):
    native.require(isinstance(value, dict) and set(value) == {"max_delta", "max_relative_mad", "max_gap_seconds", "reason"}
        and isinstance(value["max_delta"], dict) and set(value["max_delta"]) == METRICS
        and isinstance(value["reason"], str) and value["reason"].strip(), "complete foundation budget required")
    native.require(all(type(n) in (int, float) and math.isfinite(n) and n >= 0 for n in value["max_delta"].values())
        and type(value["max_relative_mad"]) is float and 0 < value["max_relative_mad"] <= .1
        and type(value["max_gap_seconds"]) in (int, float) and math.isfinite(value["max_gap_seconds"])
        and 0 < value["max_gap_seconds"] <= 300, "finite budget/noise/settlement gap required")
    return value


def cumulative(value):
    native.require(value.get("profile") in {PROFILE, CUMULATIVE_PROFILE}, "unknown adjacent comparison profile")
    return value["profile"] == CUMULATIVE_PROFILE


def cumulative_policy():
    # Closed repository-owned policy, never a spec-supplied module or callback.
    if __package__:
        from . import building_cumulative_acceptance
    else:
        import building_cumulative_acceptance
    return building_cumulative_acceptance


def entrypoint(planned):
    return Path(cumulative_policy().__file__).resolve() if cumulative(planned) else Path(__file__).resolve()


def drivers(profile=PROFILE):
    native.require(profile in {PROFILE, CUMULATIVE_PROFILE}, "unknown adjacent driver profile")
    result = {Path(module.__file__).name: static.sha256(Path(module.__file__))
              for module in (static, supplement, inventory, executable, storage)} | {
                  Path(__file__).name: static.sha256(Path(__file__)),
                  "primary_harness": native.native_harness_fingerprint(Path(__file__).resolve().parents[1])}
    return result | cumulative_policy().dependencies() if profile == CUMULATIVE_PROFILE else result


def identities(repo, reference, profile=PROFILE):
    native.require(repo != reference and not native.git_dirty_paths(repo), "distinct clean sources required")
    result = {"reference": supplement.local_identity(reference), "current": static.identity(repo)}
    native.require(profile in {PROFILE, CUMULATIVE_PROFILE}
        and result["reference"]["source_fingerprint"] != result["current"]["source_fingerprint"],
        "adjacent comparison requires distinct known sources")
    if profile == PROFILE:
        native.require(result["reference"]["asset_view_fingerprint"] == result["current"]["asset_view_fingerprint"],
                       "foundation requires different sources and identical baseline assets")
    return result


def check_plan(value):
    is_cumulative = cumulative(value)
    native.require(value.get("accepted") is False
        and value.get("promotion_authority") is False and value.get("drivers") == drivers(value["profile"])
        and value.get("matrix") == [list(case) for case in matrix()], "foundation plan contract/dependencies differ")
    limits(value["limits"])
    repo, reference = native.validate_repo(value["repos"]["current"]), native.validate_repo(value["repos"]["reference"])
    static.check_root(repo, Path(value["job_root"]))
    native.require(value["identities"] == identities(repo, reference, value["profile"]), "foundation source/assets changed")
    native.require(("release" in value) == is_cumulative, "release inventory/profile differs")
    native.require(("resource_basis_verifier" in value) == is_cumulative,
                   "historical resource verifier/profile differs")
    if is_cumulative:
        cumulative_policy().check_inputs(value)
    basis = Path(value["resource_basis"])
    native.require(static.sha256(basis / "manifest.json") == value["resource_basis_sha256"], "resource basis changed")
    verified = (cumulative_policy().verify_resource_basis(value)
                if is_cumulative else supplement.verify(basis))
    native.require(verified["source"] == value["identities"]["reference"], "resource basis/reference differs")


def plan(args, *, profile=PROFILE):
    repo, reference = native.validate_repo(args.repo), native.validate_repo(args.reference_repo)
    root = args.job_root.resolve()
    static.check_root(repo, root)
    native.require(not root.exists() and not args.output.resolve().is_relative_to(root), "fresh root and external plan required")
    native.require(profile in {PROFILE, CUMULATIVE_PROFILE}, "unknown adjacent plan profile")
    value = {"profile": profile, "repos": {"current": str(repo), "reference": str(reference)},
        "job_root": str(root), "identities": identities(repo, reference, profile), "limits": limits(native.read_json(args.budget)),
        "resource_basis": str(args.resource_basis.resolve()),
        "resource_basis_sha256": static.sha256(args.resource_basis / "manifest.json"),
        "drivers": drivers(profile), "matrix": [list(case) for case in matrix()], "adapter": args.adapter,
        "accepted": False, "promotion_authority": False,
        "adjacency": "only source/input hashing, raw verification and case seal/retain/finalize between paired runs; no build or other workload"}
    if profile == CUMULATIVE_PROFILE:
        value["release"] = cumulative_policy().plan_inputs(args, value)
        value["resource_basis_verifier"] = cumulative_policy().bind_resource_verifier(args, value)
    native.require(isinstance(value["adapter"], str) and value["adapter"].strip(), "adapter required")
    check_plan(value)
    native.atomic_write_json(args.output, value)
    return value


@dataclass(frozen=True)
class _CaptureProof:
    stage_path: Path
    stage_sha256: str
    plan_path: Path
    plan_sha256: str
    result: dict


def _verified_capture(path, plan_path):
    # Only constructed from a full verifier result in this call. Never read a
    # cached result from a document, CLI argument, previous process or next case.
    result = verify_stage(path)
    value = native.read_json(path)
    proof = _CaptureProof(path.resolve(), static.sha256(path), Path(value["plan"]).resolve(),
                          value["plan_sha256"], result)
    _capture_result(proof, path, plan_path)
    return proof


def _capture_result(proof, path, plan_path):
    native.require(isinstance(proof, _CaptureProof)
        and proof.stage_path == path.resolve() and proof.plan_path == plan_path.resolve()
        and proof.stage_sha256 == static.sha256(path) == proof.result["stage_sha256"]
        and proof.plan_sha256 == static.sha256(plan_path)
        and proof.result["instrumentation"] == "capture", "completed Capture proof path/plan/stage differs")
    return proof.result


def check_stage(path):
    value, planned, _capture = _check_stage(path)
    return value, planned


def _check_stage(path):
    value = native.read_json(path)
    native.require(value.get("stage_path") == str(path.resolve()), "stage path differs")
    plan_path = Path(value["plan"])
    native.require(static.sha256(plan_path) == value["plan_sha256"], "paired plan changed")
    planned = native.read_json(plan_path)
    check_plan(planned)
    native.require(value.get("profile") == planned["profile"] and value.get("instrumentation") in ("capture", "memory")
        and set(value["binaries"]) == set(SOURCES), "stage contract differs")
    prior, captured = None, None
    if value["instrumentation"] == "capture":
        native.require(value.get("prior_stage") is None, "Capture must be first")
    else:
        prior = Path(value["prior_stage"])
        native.require(static.sha256(prior) == value["prior_stage_sha256"], "Capture stage changed")
        captured = _verified_capture(prior, plan_path)
        native.require(time(value["created_at"]) > time(captured.result["rows"][-1]["finished_at"]),
                       "Memory stage was bound before Capture completed")
        previous = native.read_json(prior)
        native.require(all(value["binaries"][source]["sha256"] != previous["binaries"][source]["sha256"]
                           for source in SOURCES), "Capture/Memory executable not separated")
    root = Path(planned["job_root"])
    for source, binary in value["binaries"].items():
        executable.validate(Path(binary["path"]), root / f"{value['instrumentation']}-{source}", binary["sha256"])
        receipt = _verify_build(plan_path, source, value["instrumentation"], prior,
                                finalized=True, captured=captured)
        native.require(binary == receipt["binary"], "stage binary lacks its exact guarded source build")
        native.require(time(receipt["build_finished_at"]) < time(value["created_at"]),
                       "both source builds must complete before stage binding")
    native.require(len({binary["sha256"] for binary in value["binaries"].values()}) == 2,
                   "different source binaries must not be identical")
    return value, planned, captured


def bind(args):
    planned = native.read_json(args.plan)
    check_plan(planned)
    native.require(not args.output.exists(), "stage output must be fresh")
    value = {"profile": planned["profile"], "stage_path": str(args.output.resolve()),
        "plan": str(args.plan.resolve()), "plan_sha256": static.sha256(args.plan),
        "instrumentation": args.instrumentation, "created_at": native.utc_now(),
        "prior_stage": str(args.prior_stage.resolve()) if args.prior_stage else None,
        "prior_stage_sha256": static.sha256(args.prior_stage) if args.prior_stage else None,
        "binaries": {source: {"path": str(path.resolve()), "sha256": executable.digest(path)}
                     for source, path in (("reference", args.reference_binary), ("current", args.current_binary))}}
    # Check the in-memory stage via an exclusive private output; remove on refusal,
    # never leave a ready-looking rejected plan behind.
    native.atomic_write_json(args.output, value)
    try:
        check_stage(args.output)
    except Exception:
        args.output.unlink()
        raise
    return value


def command(planned, stage, case):
    if cumulative(planned):
        return cumulative_policy().case_command(planned, stage, case)
    instrument, size, _repeat, source = case
    parent = Path(planned["job_root"]) / label(case)
    argv = static.command(parent, instrument, size, 15 if size == "small" else 60, planned["adapter"])
    argv[argv.index("--repeat") + 1] = "1"
    return ["env", "RUST_LOG=info,wgpu=error", *argv, "--building-static-single-leg",
            "--skip-build", "--binary", stage["binaries"][source]["path"]]


def time(value):
    result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    native.require(result.utcoffset() is not None, "case timestamp has no timezone")
    return result


def case_verifier(stage, planned, case):
    cases = [item for item in matrix() if item[0] == stage["instrumentation"]]
    return ["python3", str(entrypoint(planned)), "verify-case", "--stage", stage["stage_path"],
            "--index", str(cases.index(case)), "--job-root", str(Path(planned["job_root"]) / label(case))]


def pinned_file(batch, path, sha):
    native.require({"path": str(path), "sha256": sha} in batch.get("command_files", []),
                   "registered command input changed: " + str(path))


def build_command(plan_path, planned, source, instrument, prior):
    argv = ["python3", str(entrypoint(planned)), "build", "--plan", str(plan_path.resolve()),
            "--source", source, "--instrumentation", instrument, "--repo", planned["repos"][source],
            "--job-root", str(Path(planned["job_root"]) / f"{instrument}-{source}")]
    if prior is not None:
        argv += ["--prior-stage", str(prior.resolve())]
    return argv


def prepared(repo, root, command, *, profile=PROFILE):
    native.require(not root.exists(), "prepared output must be fresh")
    resources = native.resource_snapshot(repo, require_launcher=True)
    native.require(not resources["failures"], str(resources["failures"]))
    native.require(profile in {PROFILE, CUMULATIVE_PROFILE}, "unknown prepared profile")
    return {"profile": profile, "status": "ready", "repo": str(repo), "job_root": str(root),
            "accepted": False, "promotion_authority": False, "resources": resources,
            "launcher_command": ["kitty", "--directory", str(repo), "--detach", *command]}


def cargo_command(instrument):
    native.require(instrument in ("capture", "memory"), "unsupported build instrument")
    return ["python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
            "-p", "bevy_app@0.1.0", "--no-default-features", "--features",
            "profiling" if instrument == "capture" else "profiling-memory", "--locked"]


def verify_build(plan_path, source, instrument, prior, *, finalized=False):
    captured = _verified_capture(prior, plan_path) if instrument == "memory" and prior is not None else None
    return _verify_build(plan_path, source, instrument, prior, finalized=finalized, captured=captured)


def _verify_build(plan_path, source, instrument, prior, *, finalized, captured):
    planned = native.read_json(plan_path)
    check_plan(planned)
    native.require(source in SOURCES and instrument in ("capture", "memory"), "build scope differs")
    root = Path(planned["job_root"]) / f"{instrument}-{source}"
    receipt = native.read_json(root / "manifest.json")
    native.require(receipt["plan_sha256"] == static.sha256(plan_path) and receipt["source"] == source
        and receipt["instrumentation"] == instrument and receipt["identity"] == planned["identities"][source]
        and receipt["cargo_command"] == cargo_command(instrument)
        and receipt["build_log_sha256"] == static.sha256(root / "build.log"), "build receipt differs")
    repo = Path(planned["repos"][source])
    ledger = native.read_json(storage.ledger_dir(storage.primary_for(repo)) / "ledger.json")
    batch_id = Path(planned["job_root"]).name + "-build-" + instrument + "-" + source
    batch = ledger["batches"][batch_id]
    expected = build_command(plan_path, planned, source, instrument, prior)
    verifier = expected.copy()
    verifier[2] = "verify-build"
    native.require(batch["repo"] == str(repo) and batch["roots"] == [str(root)]
        and batch["subject"] == planned["identities"][source]["subject_commit"]
        and batch["command"] == expected and batch["verify_command"] == verifier
        and type(batch.get("exit_code")) is int and batch["exit_code"] == 0
        and batch["phase"] in ({"finalized"} if finalized else {"needs-finalize", "sealing", "finalized"})
        and (not finalized or batch.get("result") == "pass"), "guarded build registration/execution differs")
    pinned_file(batch, plan_path.resolve(), static.sha256(plan_path))
    pinned_file(batch, entrypoint(planned), static.sha256(entrypoint(planned)))
    native.require(time(batch["started_at"]) < time(batch["finished_at"]), "build interval invalid")
    if instrument == "memory":
        native.require(prior is not None and receipt["prior_stage_sha256"] == static.sha256(prior),
                       "Memory build must bind completed Capture")
        capture_result = _capture_result(captured, prior, plan_path)
        native.require(capture_result["instrumentation"] == "capture"
            and native.read_json(prior)["plan_sha256"] == static.sha256(plan_path)
            and time(batch["started_at"]) > time(capture_result["rows"][-1]["finished_at"]),
            "Memory build started before exact Capture closure")
    else:
        native.require(prior is None and receipt["prior_stage_sha256"] is None, "Capture build must be first")
    executable.validate(Path(receipt["binary"]["path"]), root, receipt["binary"]["sha256"])
    return receipt | {"build_finished_at": batch["finished_at"]}


@native.activity_locked
def build(args):
    planned = native.read_json(args.plan)
    native.require(planned == args.frozen_plan, "build plan changed across admission")
    check_plan(planned)
    native.require(str(Path(args.repo).resolve()) == planned["repos"][args.source]
        and args.job_root.resolve() == Path(planned["job_root"]) / f"{args.instrumentation}-{args.source}",
        "build source/root differs")
    if args.instrumentation == "memory":
        native.require(args.prior_stage is not None, "Memory requires completed Capture before build")
        captured = verify_stage(args.prior_stage)
        prior = native.read_json(args.prior_stage)
        native.require(captured["instrumentation"] == "capture" and prior["plan_sha256"] == static.sha256(args.plan),
                       "Memory prior plan differs")
    else:
        native.require(args.prior_stage is None, "Capture build cannot have a prior stage")
    repo, root = Path(args.repo), args.job_root.resolve()
    native.require(not root.exists(), "build root must be fresh")
    root.mkdir(parents=True)
    job = root / "job.json"
    state = {"profile": planned["profile"], "status": "running", "pid": os.getpid(), "started_at": native.utc_now()}
    native.atomic_write_json(job, state)
    try:
        native.run_command("build", cargo_command(args.instrumentation), repo=repo,
            env=native.cargo_environment(repo), log_path=root / "build.log", job_file=job, state=state,
            timeout_seconds=7200)
        check_plan(planned)
        native.require(native.read_json(args.plan) == planned, "plan changed during build")
        binary = executable.freeze(repo / "target/profiling/bevy_app", root)
        receipt = {"profile": planned["profile"], "plan_sha256": static.sha256(args.plan), "source": args.source,
            "instrumentation": args.instrumentation, "identity": planned["identities"][args.source],
            "cargo_command": cargo_command(args.instrumentation), "build_log_sha256": static.sha256(root / "build.log"),
            "prior_stage_sha256": static.sha256(args.prior_stage) if args.prior_stage else None,
            "binary": {"path": str(binary), "sha256": executable.digest(binary)}}
        native.atomic_write_json(root / "manifest.json", receipt)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return receipt
    except Exception as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


def case_result(stage, planned, case, *, finalized=False):
    instrument, size, repeat, source = case
    parent = Path(planned["job_root"]) / label(case)
    repo = Path(planned["repos"][source])
    # Coordinator receipts bind actual once-only launch, exact source/argv/root,
    # exit and real host interval. They are read here, never rewritten.
    ledger = native.read_json(storage.ledger_dir(storage.primary_for(repo)) / "ledger.json")
    batch_id = Path(planned["job_root"]).name + "-" + label(case)
    batch = ledger["batches"][batch_id]
    native.require(batch["repo"] == str(repo) and batch["roots"] == [str(parent)]
        and batch["command"] == command(planned, stage, case)
        and batch["subject"] == planned["identities"][source]["subject_commit"]
        and batch.get("exit_code") == 0 and type(batch.get("exit_code")) is int
        and batch["phase"] in ({"finalized"} if finalized else {"needs-finalize", "sealing", "sealed", "finalized"}),
        "case registration/execution differs")
    native.require(not finalized or batch.get("result") == "pass", "case did not independently seal pass")
    native.require(batch.get("verify_command") == case_verifier(stage, planned, case),
                   "registered case verifier differs")
    pinned_file(batch, Path(stage["stage_path"]), static.sha256(Path(stage["stage_path"])))
    pinned_file(batch, entrypoint(planned), static.sha256(entrypoint(planned)))
    start, end = time(batch["started_at"]), time(batch["finished_at"])
    native.require(time(stage["created_at"]) < start < end, "case interval/stage order invalid")
    if cumulative(planned):
        checked, raw, observed = cumulative_policy().case_artifacts(planned, stage, case, parent)
    else:
        checked = static.verify_session(parent, instrument, size, 15 if size == "small" else 60,
                                        planned["identities"][source], planned["adapter"], repeats=1)
        case_id = static.Case("building-art-static", size, "gpu", static.SEED, 15 if size == "small" else 60, 0).identifier
        raw = parent / f"{instrument}-{size}/cases" / case_id / "run-001/data/building_resource_inventory.json"
        observed = inventory.boundaries(native.read_json(raw))
    native.require(checked["binary_sha256"] == stage["binaries"][source]["sha256"], "case binary differs")
    inventory.require_boundary_scale(observed, size)
    return {"instrumentation": instrument, "size": size, "repeat": repeat, "source": source,
        "batch": batch_id, "started_at": batch["started_at"], "finished_at": batch["finished_at"],
        "binary_sha256": checked["binary_sha256"], "session_sha256": checked["session_sha256"],
        "adapter": checked["observations"][0]["adapter"], "layout_sha256": checked["observations"][0]["layout_sha256"],
        "resource_sha256": static.sha256(raw), "inventory": observed,
        **({"presentation_sha256": checked["presentation_sha256"]} if cumulative(planned) else {}),
        "metrics": measured_metrics(checked["observations"][0]["metrics"], instrument)
                   | {k: observed[k] for k in RESOURCE_KEYS}}


def measured_metrics(raw, instrument):
    # frame_summary also returns diagnostic p50/max. The premeasurement policy
    # deliberately budgets p95/p99; never infer new budget keys from that API.
    native.require(instrument in ("capture", "memory"), "unknown metric instrumentation")
    keys = {"p95_ms", "p99_ms"} if instrument == "capture" else {"peak_live_bytes", "max_rss_kib"}
    native.require(isinstance(raw, dict) and keys <= raw.keys(), "required raw metrics missing")
    result = {key: raw[key] for key in keys}
    native.require(all(type(n) in (int, float) and math.isfinite(n) and n >= 0 for n in result.values()),
                   "required raw metrics must be finite nonnegative numbers")
    return result


def aggregate(rows, instrument, budget):
    expected = [case for case in matrix() if case[0] == instrument]
    required_metrics = RESOURCE_KEYS | ({"p95_ms", "p99_ms"} if instrument == "capture" else
                                       {"peak_live_bytes", "max_rss_kib"})
    native.require(instrument in ("capture", "memory")
        and all(set(row["metrics"]) == required_metrics for row in rows),
        "complete instrumentation-specific budget metrics required")
    native.require([(r["instrumentation"], r["size"], r["repeat"], r["source"]) for r in rows] == expected,
                   "complete ordered twelve-case stage required")
    native.require(all(r["adapter"] == rows[0]["adapter"] for r in rows), "adapter drift")
    native.require(all(time(row["started_at"]) < time(row["finished_at"]) for row in rows),
                   "case start/end interval invalid")
    for previous, current in zip(rows, rows[1:]):
        gap = (time(current["started_at"]) - time(previous["finished_at"])).total_seconds()
        # Size boundaries may include result checks, but still no build/workload.
        native.require(0 <= gap <= budget["max_gap_seconds"], "case overlap or excessive settlement gap")
    resource_assessments = {}
    for source in SOURCES:
        chosen = [r for r in rows if r["source"] == source]
        native.require(len({r["binary_sha256"] for r in chosen}) == 1, "source binary drift")
        for row in chosen[1:]:
            compare = inventory.legacy_boundary_growth if source == "reference" else inventory.plateau
            assessment = compare(chosen[0]["inventory"], row["inventory"])
            resource_assessments[source] = (assessment if source == "reference" else
                {"global_shared_mesh_plateau": True, "totals": assessment})
    result = []
    for size in ("small", "medium"):
        chosen = [r for r in rows if r["size"] == size]
        native.require(len({r["layout_sha256"] for r in chosen}) == 1, "fixture differs across sources/repeats")
        summary = {}
        for source in SOURCES:
            subset = [r for r in chosen if r["source"] == source]
            keys = set(subset[0]["metrics"])
            native.require(all(set(r["metrics"]) == keys for r in subset), "metric inventory differs")
            summary[source] = {}
            for key in keys:
                values = [r["metrics"][key] for r in subset]
                native.require(all(type(n) in (int, float) and math.isfinite(n) and n >= 0 for n in values), "invalid metric")
                median = statistics.median(values)
                mad = statistics.median(abs(n-median) for n in values)
                native.require(mad <= median * budget["max_relative_mad"], "foundation noise limit exceeded: " + key)
                summary[source][key] = {"median": median, "mad": mad}
        for key, after in summary["current"].items():
            native.require(after["median"] - summary["reference"][key]["median"] <= budget["max_delta"][key],
                           "foundation increment exceeded: " + key)
        result.append({"size": size, "sources": summary, "resource_assessments": resource_assessments})
    return result


def _boot_boundary():
    # Positive machine-lifetime evidence, not PID absence. Never supplied by a
    # CLI or saved document. Both reads must describe the same current boot.
    boot_path = Path("/proc/sys/kernel/random/boot_id")
    try:
        boot = boot_path.read_text().strip()
        btime = next(line.split()[1] for line in Path("/proc/stat").read_text().splitlines()
                     if line.startswith("btime "))
        native.require(boot == boot_path.read_text().strip(), "kernel boot changed during observation")
        require_boot = re.fullmatch(r"[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}", boot)
        native.require(require_boot is not None and btime.isdigit(), "kernel boot boundary unavailable")
        return {"boot_id": boot, "started_at": datetime.fromtimestamp(int(btime), timezone.utc).isoformat()}
    except (OSError, StopIteration, ValueError) as error:
        raise native.AcceptanceError("kernel boot boundary unavailable") from error


def _closed_registration_path(batch, history):
    """Prove the current coordinator's complete register/seal/close WAL path."""
    native.require(batch.get("coordinator_sha256") == storage.digest(Path(storage.__file__))
        and isinstance(history, list), "closed registration transition evidence unavailable")
    bid = batch["id"]
    pattern = re.compile(r"^(?:register|start|child started|execution ended|seal started|seal failed|recorded verification result|finalize|recover) "
                         + re.escape(bid) + r"(?:$|;|:)")
    events = [row for row in history if isinstance(row, dict) and isinstance(row.get("action"), str)
              and pattern.search(row["action"])]
    expected = [f"register {bid}", f"seal started {bid}",
                f"recorded verification result {bid}; no archive required", f"finalize {bid}: {batch['result']}"]
    native.require([row["action"] for row in events] == expected, "closed registration lifecycle differs")
    stamps = [time(row["at"]) for row in events]
    native.require(time(batch["created_at"]) <= stamps[0] <= stamps[1] <= time(batch["verified_at"])
        <= stamps[2] <= time(batch["finalized_at"]) <= stamps[3], "closed registration event order differs")
    return stamps[-1], events


def _activity_end_upper_bound(batch, history=None):
    started = batch.get("started_at")
    native.require(started is not None or batch.get("phase") == "finalized",
                   "validation start boundary is unknown")
    lower = time(started if started is not None else batch["created_at"])
    if batch.get("finished_at") is not None:
        native.require(started is not None, "command finish has no start boundary")
        end = time(batch["finished_at"])
        native.require(end >= lower, "validation end precedes its start")
        return end, {"basis": "finished_at"}
    native.require(batch.get("phase") == "finalized" and batch.get("verified_at") and batch.get("finalized_at"),
                   "validation closure boundary is unknown")
    code = batch.get("exit_code")
    if started is None:
        native.require(batch.get("result") in {"invalid", "abandoned"} and code is None,
                       "missing-time history has inconsistent closure")
        basis = "closed_missing_execution_times_upper_bound"
    else:
        native.require(batch.get("result") in {"invalid", "interrupted", "abandoned"}
            and type(code) is int and -64 <= code <= 255 and code != 0,
            "closed history lacks a known non-success exit")
        basis = "finalized_at_upper_bound"
    verified, end = time(batch["verified_at"]), time(batch["finalized_at"])
    native.require(lower <= verified <= end, "closed validation timestamp order differs")
    native.require("process_identity" in batch, "closed history ownership is missing")
    audit = {"basis": basis, "start_lower_bound": lower.isoformat(),
             "start_lower_bound_basis": "started_at" if started is not None else "created_at"}
    if "child_identity" not in batch:
        native.require(started is None and "child_pid" not in batch and batch["process_identity"] is None,
                       "closed history child ownership is missing")
        if batch.get("coordinator_sha256") == storage.digest(Path(storage.__file__)):
            end, events = _closed_registration_path(batch, history)
            audit.update(basis="closed_registration_seal_path_upper_bound", transition_receipts=events)
        else:
            # A machine-local, terminal closure before the current boot cannot
            # authorize later execution; the reboot ends any unpublished orphan.
            boot = _boot_boundary()
            native.require(end <= time(boot["started_at"]), "missing-owner closure is not bounded by an older boot")
            end = time(boot["started_at"])
            audit.update(basis="closed_preboot_missing_child_upper_bound", kernel_boot=boot)
        return end, audit
    identities = [batch[key] for key in ("process_identity", "child_identity") if batch[key] is not None]
    if identities:
        boot = _boot_boundary()
        for identity in identities:
            native.require(isinstance(identity, str) and re.fullmatch(
                r"[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}:[1-9][0-9]*", identity) is not None
                and identity.split(":")[0] != boot["boot_id"], "closed history has an unresolved current owner")
        end = max(end, time(boot["started_at"]))
        audit.update(basis="different_boot_owner_upper_bound", kernel_boot=boot,
                     historical_identities=identities)
    return end, audit


def require_no_interleaving(rows, batches, *, history=None):
    start, end = time(rows[0]["started_at"]), time(rows[-1]["finished_at"])
    allowed = {r["batch"] for r in rows}
    closures = []
    for batch in batches:
        if batch["id"] in allowed:
            continue
        if batch.get("started_at") is not None and time(batch["started_at"]) > end:
            continue
        upper, audit = _activity_end_upper_bound(batch, history)
        lower = time(batch["started_at"] if batch.get("started_at") is not None else batch["created_at"])
        if lower > end:
            continue
        native.require(upper < start, "other validation interleaved: " + batch["id"])
        if audit["basis"] != "finished_at":
            closures.append({"batch": batch["id"], "end_upper_bound": upper.isoformat(),
                             "historical_result": batch["result"], **audit})
    return {"started_at": rows[0]["started_at"], "finished_at": rows[-1]["finished_at"],
            "closed_history_bounds": sorted(closures, key=lambda row: row["batch"])}


def verify_stage(path):
    stage, planned, captured = _check_stage(path)
    cases = [case for case in matrix() if case[0] == stage["instrumentation"]]
    rows = [case_result(stage, planned, case, finalized=True) for case in cases]
    cross_stage_resources = {}
    if stage["instrumentation"] == "memory":
        previous = _capture_result(captured, Path(stage["prior_stage"]), Path(stage["plan"]))
        native.require(time(rows[0]["started_at"]) > time(previous["rows"][-1]["finished_at"]),
                       "Memory ran before Capture completed")
        for source in SOURCES:
            before = next(r for r in previous["rows"] if r["source"] == source)
            after = next(r for r in rows if r["source"] == source)
            compare = inventory.legacy_boundary_growth if source == "reference" else inventory.plateau
            assessment = compare(before["inventory"], after["inventory"])
            cross_stage_resources[source] = assessment if source == "reference" else {"global_shared_mesh_plateau": True, "totals": assessment}
    ledger = native.read_json(storage.ledger_dir(storage.primary_for(Path(planned["repos"]["current"]))) / "ledger.json")
    interleaving = require_no_interleaving(rows, ledger["batches"].values(), history=ledger.get("history"))
    return {"profile": planned["profile"], "status": "pass", "instrumentation": stage["instrumentation"],
        "stage_sha256": static.sha256(path), "rows": rows,
        "aggregate": aggregate(rows, stage["instrumentation"], planned["limits"]),
        "cross_stage_resources": cross_stage_resources, "interleaving_audit": interleaving,
        "accepted": False, "promotion_authority": False,
        "unverified": ["active gameplay", "GPU allocation budget", "art", "release"]
                      + ([] if cumulative(planned) else ["final cumulative nine"])}


def main(*, profile=PROFILE):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "prepare-build", "prepare-case", "build", "verify-build", "bind", "verify-case", "verify-stage"))
    for name in ("repo", "reference-repo", "budget", "resource-basis", "job-root", "output", "plan",
                 "stage", "reference-binary", "current-binary", "prior-stage"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--adapter")
    parser.add_argument("--instrumentation", choices=("capture", "memory"))
    parser.add_argument("--source", choices=SOURCES)
    parser.add_argument("--index", type=int)
    native.require(profile in {PROFILE, CUMULATIVE_PROFILE}, "unknown adjacent CLI profile")
    if profile == CUMULATIVE_PROFILE:
        parser.add_argument("--codec", type=Path)
        parser.add_argument("--resource-verifier-repo", type=Path)
    args = parser.parse_args()
    for path in (args.plan, args.stage):
        if path is not None:
            native.require(native.read_json(path).get("profile") == profile, "CLI/plan profile differs")
    if args.command == "plan":
        native.require(all((args.repo, args.reference_repo, args.budget, args.resource_basis, args.job_root, args.output)), "plan inputs missing")
        if profile == CUMULATIVE_PROFILE:
            native.require(args.codec is not None, "released inventory codec required")
            native.require(args.resource_verifier_repo is not None, "frozen resource verifier source required")
        result = plan(args, profile=profile)
    elif args.command == "prepare-build":
        native.require(all((args.plan, args.source, args.instrumentation)), "build preparation inputs missing")
        planned = native.read_json(args.plan)
        check_plan(planned)
        if args.instrumentation == "memory":
            native.require(args.prior_stage is not None, "Memory requires completed Capture")
            captured = verify_stage(args.prior_stage)
            native.require(captured["instrumentation"] == "capture"
                and native.read_json(args.prior_stage)["plan_sha256"] == static.sha256(args.plan), "Capture plan differs")
        else:
            native.require(args.prior_stage is None, "Capture build must be first")
        root = Path(planned["job_root"]) / f"{args.instrumentation}-{args.source}"
        result = prepared(Path(planned["repos"][args.source]), root,
                          build_command(args.plan, planned, args.source, args.instrumentation, args.prior_stage), profile=planned["profile"])
    elif args.command == "prepare-case":
        native.require(args.stage is not None and type(args.index) is int and 0 <= args.index < 12, "case stage/index required")
        stage, planned = check_stage(args.stage)
        case = [case for case in matrix() if case[0] == stage["instrumentation"]][args.index]
        result = prepared(Path(planned["repos"][case[3]]), Path(planned["job_root"]) / label(case),
                          command(planned, stage, case), profile=planned["profile"])
    elif args.command in ("build", "verify-build"):
        native.require(all((args.plan, args.repo, args.source, args.instrumentation, args.job_root)), "build inputs missing")
        if args.command == "build":
            args.frozen_plan = native.read_json(args.plan)
            result = build(args)
        else:
            result = verify_build(args.plan, args.source, args.instrumentation, args.prior_stage)
            native.require(args.repo.resolve() == Path(native.read_json(args.plan)["repos"][args.source])
                and args.job_root.resolve() == Path(result["binary"]["path"]).parent.parent,
                "verifier build source/root differs")
    elif args.command == "bind":
        native.require(all((args.plan, args.output, args.instrumentation, args.reference_binary, args.current_binary)), "binary stage inputs missing")
        result = bind(args)
    elif args.command == "verify-case":
        native.require(args.stage is not None and type(args.index) is int and 0 <= args.index < 12, "case stage/index required")
        stage, planned = check_stage(args.stage)
        cases = [case for case in matrix() if case[0] == stage["instrumentation"]]
        native.require(args.job_root is not None and args.job_root.resolve() ==
                       Path(planned["job_root"]) / label(cases[args.index]), "exact case output root required")
        result = case_result(stage, planned, cases[args.index])
    else:
        native.require(args.stage is not None, "stage required")
        result = verify_stage(args.stage)
    print(json.dumps(result, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
