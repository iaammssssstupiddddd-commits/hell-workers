"""Offline TAK-14 group/full evidence contract; never launches or promotes.

The coordinator supplies host-broker registration evidence and independently
collected artifacts. Offline equality checks do not authenticate that evidence.
No production lifecycle recipe is currently supported by the host boundary;
planning and result verification therefore remain unavailable.
Old static/M6 passes and self-declared pass booleans are not evidence.
"""
from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import re
import secrets
import statistics
import time
import uuid

if __package__:
    from . import building_art_acceptance as art
    from .perf_tool.execution import read_native_memory, read_resource_usage
else:
    import building_art_acceptance as art
    from perf_tool.execution import read_native_memory, read_resource_usage

pipeline = art.pipeline
PROFILE = "building-production-results-v1"
GROUPS = {
    "m2": ("Tank", "MudMixer"),
    "m3": ("RestArea", "SoulSpa"),
    "m5": ("WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"),
    "bridge": ("Bridge",),
}
GROUPS["full"] = (*GROUPS["m2"], *GROUPS["m3"], *GROUPS["m5"], "Bridge", "Door")
METRICS = {"p95_ms", "p99_ms", "rss_bytes", "native_peak_live_bytes",
           "gpu_measured_bytes", "gpu_estimated_bytes", "mesh_cpu_bytes", "image_cpu_bytes",
           "material_shallow_bytes", "mesh_count", "image_count", "material_count",
           "root_count", "part_count", "application_handle_count"}
COUNT_METRICS = {key for key in METRICS if key.endswith("_count")}
HASH = re.compile(r"[0-9a-f]{64}")
NONCE = re.compile(r"[0-9a-f]{32}")
SPEC_KEYS = {"scope", "repo", "subject", "codec", "driver", "binaries", "environment",
             "registration_context", "host_capabilities", "registration_request", "registration_intent",
             "gameplay_contract_sha256", "cases",
             "budget", "releases", "generation_trials", "baseline"}
GENERATED_KEYS = {"schema_version", "profile", "accepted", "promotion_authority", "launchable",
                  "created_at_ns", "campaign_nonce", "verifier_sha256", "lifecycle_verifier_sha256",
                  "input_transport_sha256", "registry_contract", "registry_sha256"}


def require(value, reason):
    pipeline.require(value, reason)


def number(value, *, positive=False):
    return type(value) in (int, float) and math.isfinite(value) and (value > 0 if positive else value >= 0)


def hashed(value):
    return isinstance(value, str) and HASH.fullmatch(value) is not None


def json_bytes(payload):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    def invalid(_):
        raise ValueError("nonfinite JSON")
    return json.loads(payload, object_pairs_hook=unique, parse_constant=invalid)


def artifact(root, record):
    require(isinstance(record, dict) and set(record) == {"path", "bytes", "sha256"}
            and hashed(record["sha256"]), "invalid artifact record")
    return pipeline.file_bytes(root, record)


def matrix():
    return [(instrument, size, repeat, mode)
            for instrument in ("Capture", "Memory") for size in ("N", "4N")
            for repeat in range(1, 4)
            for mode in (("candidate", "legacy-control") if repeat == 2
                         else ("legacy-control", "candidate"))]


def budget(value):
    require(isinstance(value, dict)
            and set(value) == {"max_delta", "max_relative_mad", "reason", "frozen_at_ns"}
            and isinstance(value["max_delta"], dict) and set(value["max_delta"]) == METRICS
            and all(number(v, positive=True) for v in value["max_delta"].values())
            and number(value["max_relative_mad"], positive=True)
            and isinstance(value["reason"], str) and value["reason"].strip()
            and type(value["frozen_at_ns"]) is int and value["frozen_at_ns"] > 0,
            "freeze a complete finite positive budget with rationale before any run")


def registry_contract(scope):
    return {"profile": PROFILE, "scope": scope, "promotion_authority": False,
            "launcher_registered": False, "launchable": False,
            "required_host_review": ["exact source/binary/codec/driver and asset inventory",
                "normal generated world and ordinary input driver; no seeded completion",
                "all lifecycle legs and owned X11 nonce/ACK capture",
                "one host heavy slot; sequential Capture then Memory",
                "native allocator, GPU measurement and distinct resource counters",
                "fresh campaign and per-run nonce; immutable artifacts and driver trace"],
            "required_collection_seal": ["host-sealed-production-collection", "session", "steps_sha256",
                "raw_observations_sha256", "action_log_sha256", "trace_sha256", "captures"],
            "available": False,
            "unavailable_reason": "production lifecycle recipe and required host instruments are not admitted",
            "authentication_authority": "host broker only; saved JSON is evidence, not authentication",
            "registration_actions": ["registration-context", "register", "registration-status"],
            "required_context_fields": ["controller_sha256", "recipe_revision", "bridge_revision", "subject", "owner"],
            "required_receipt_fields": ["batch", "phase", "sha256", "registration"],
            "required_plan_binding": "reviewed recipe must bind immutable plan path/hash and exact command",
            "host_admission": "controller-owned; never generated or inferred by product"}


def host_document(record):
    require(isinstance(record, dict) and set(record) == {"path", "sha256"}
            and hashed(record["sha256"]), "host registration context missing; registry exports are unsupported")
    payload = pipeline.no_symlinks(Path(record["path"])).read_bytes()
    require(pipeline.digest(payload) == record["sha256"], "host registration context changed")
    envelope = json_bytes(payload)
    require(isinstance(envelope, dict) and set(envelope) == {"ok", "result"}
            and envelope["ok"] is True and isinstance(envelope["result"], dict), "invalid saved host response")
    return envelope["result"]


def check_registration_context(value):
    """Check saved host evidence against the product subject, not its authenticity."""
    context = host_document(value.get("registration_context"))
    require(set(context) == {"controller_sha256", "recipe_revision", "bridge_revision", "subject", "owner"}
            and all(hashed(context.get(key)) for key in ("controller_sha256", "recipe_revision", "bridge_revision")),
            "host controller/recipe/bridge revision missing")
    subject = context.get("subject")
    require(isinstance(subject, dict) and set(subject) == {"binding", "run", "generation", "repo", "head", "source"},
            "host request subject missing")
    require(isinstance(subject["binding"], dict) and set(subject["binding"]) == {"scope_sha256", "node_id"}
            and hashed(subject["binding"]["scope_sha256"]) and subject["binding"]["node_id"] == "full-acceptance"
            and isinstance(subject["run"], dict) and set(subject["run"]) == {"id", "consumer_generation"}
            and isinstance(subject["run"]["id"], str) and subject["run"]["id"]
            and type(subject["run"]["consumer_generation"]) is int and subject["run"]["consumer_generation"] > 0
            and type(subject["generation"]) is int and subject["generation"] > 0, "invalid host request binding")
    request = value.get("registration_request")
    require(isinstance(request, str) and str(uuid.UUID(request)) == request, "registration request UUID required")
    require(subject["repo"] == value["repo"] and subject["head"] == value["subject"]["commit"]
            and subject["source"] == value["subject"]["source"], "host request repo/head/source differs")
    require(isinstance(context.get("owner"), str) and context["owner"].strip(), "host owner missing")
    intent = value.get("registration_intent")
    require(isinstance(intent, dict) and set(intent) == {
        "action", "spec", "command", "subject", "controller_sha256", "recipe_revision"}
        and intent["action"] == "register", "exact host register intent required")
    require(all(intent[key] == context[key] for key in ("subject", "controller_sha256", "recipe_revision")),
            "registration intent controller/revision/request differs")
    spec = intent["spec"]
    require(isinstance(spec, dict) and set(spec) in (
        {"id", "repo", "owner", "consumers", "roots", "verify_command"},
        {"id", "repo", "owner", "consumers", "roots", "verify_command", "reserve_bytes"})
        and spec["repo"] == subject["repo"] and spec["owner"] == context["owner"],
        "registration spec owner/repo differs")
    require(isinstance(intent["command"], list) and intent["command"]
            and all(isinstance(arg, str) and arg for arg in intent["command"]), "registration command missing")
    return context


def require_production_registration(value, receipt=None):
    """Reviewed host adapter boundary; public documents cannot admit a recipe.

    A future adapter must authenticate the exact request and return a registry
    binding; with a receipt it must also attest the plan/receipt hashes and time.
    Until that adapter exists every invocation remains unavailable.
    """
    context = check_registration_context(value)
    capabilities = host_document(value.get("host_capabilities"))
    require(capabilities.get("ok") is True and hashed(capabilities.get("revision")), "invalid host capabilities")
    registration = capabilities.get("registration")
    require(registration == {"schema": 1, "controller_sha256": context["controller_sha256"],
                             "actions": ["registration-context", "register", "registration-status"]},
            "host registration capabilities/controller mismatch")
    contracts = capabilities.get("contracts")
    require(isinstance(contracts, dict) and contracts.get("schema") == 1
            and isinstance(contracts.get("recipes"), list), "host recipe catalog missing")
    # No declared production recipe exists in the supplied live catalog. Do not
    # turn an arbitrary command or a locally edited capability JSON into one.
    # A reviewed adapter must supply command/plan/root/instrument/receipt binding
    # before this boundary can admit anything; this is intentionally not a flag.
    raise ValueError("production lifecycle recipe/instruments unavailable: host registration adapter is not admitted")


def registration_binding(value, receipt=None, *, registration_adapter=None):
    # Adapters are trusted in-process dependencies, never loaded from a plan,
    # environment variable or arbitrary command. Offline defaults fail closed.
    provider = require_production_registration if registration_adapter is None else registration_adapter
    binding = provider(value, receipt)
    keys = {"registry_sha256"}
    if receipt is not None:
        keys |= {"plan_sha256", "receipt_sha256", "admitted_at_ns"}
    require(isinstance(binding, dict) and set(binding) == keys
            and hashed(binding.get("registry_sha256")), "authenticated registry binding missing")
    if "registry_sha256" in value:
        require(binding["registry_sha256"] == value["registry_sha256"], "authenticated registry identity differs")
    if receipt is not None:
        require(hashed(value.get("registry_sha256"))
                and binding["registry_sha256"] == value["registry_sha256"], "plan registry identity missing or differs")
        require(binding["plan_sha256"] == pipeline.digest(pipeline.canonical(value))
                and binding["receipt_sha256"] == pipeline.digest(pipeline.canonical(receipt)),
                "authenticated plan/receipt binding differs")
        require(type(binding["admitted_at_ns"]) is int
                and binding["admitted_at_ns"] > value["created_at_ns"],
                "authenticated admission timestamp missing or predates plan")
    return binding


def check_registration_receipt(value, receipt, *, registration_adapter=None) -> int:
    """Check public consistency, then return only the host-attested time."""
    if registration_adapter is not None:
        return registration_binding(value, receipt, registration_adapter=registration_adapter)["admitted_at_ns"]
    context = check_registration_context(value)
    require(isinstance(receipt, dict) and set(receipt) == {"batch", "phase", "sha256", "registration"}
            and hashed(receipt["sha256"]), "invalid host registration receipt")
    intent = value["registration_intent"]
    # Host fingerprint differs from pipeline.canonical: ASCII escapes, no LF.
    fingerprint = pipeline.digest(json.dumps(intent, sort_keys=True, separators=(",", ":"),
                                             ensure_ascii=True, allow_nan=False).encode())
    require(receipt["registration"] == {
        "schema": 1, "request": value["registration_request"], "intent_sha256": fingerprint,
        "intent": intent, "subject": context["subject"],
        "controller_sha256": context["controller_sha256"], "recipe_revision": context["recipe_revision"]},
        "host registration receipt differs from exact request/intent")
    # The missing reviewed adapter must additionally bind batch, phase, command,
    # driver/verifier, immutable plan digest and output root. No value is admitted
    # based on these public-field comparisons alone.
    return registration_binding(value, receipt)["admitted_at_ns"]


def release_inventory(value):
    """Reuse the runtime codec for canonical manifest, dependency and receipt checks."""
    scope, codec = value["scope"], Path(value["codec"]["path"])
    require(set(value["releases"]) == set(GROUPS[scope]), "incomplete released target group")
    for kind, spec in value["releases"].items():
        if kind == "Door":
            # Existing Door verifier and g7 authority remain owned by the nine-kind path.
            if __package__:
                from . import building_m6_acceptance as m6
            else:
                import building_m6_acceptance as m6
            inventory = m6.inventory(Path(value["repo"]), codec)
            require(spec == {"inventory_sha256": pipeline.digest(pipeline.canonical(inventory))},
                    "dedicated Door inventory differs")
            continue
        manifest, text, _ = pipeline.load_set(Path(spec["root"]), kind, codec)
        require(manifest["identity"]["authority"] == "release_approved"
                and manifest["identity"] == spec["identity"]
                and pipeline.digest(text.encode()) == spec["manifest_file_sha256"],
                "unreleased or mismatched asset identity")


def check_plan(value, *, registration_adapter=None):
    registration_binding(value, registration_adapter=registration_adapter)
    require(set(value) == SPEC_KEYS | GENERATED_KEYS, "unknown/missing plan fields")
    require(type(value["created_at_ns"]) is int and value["created_at_ns"] > 0, "invalid plan freeze timestamp")
    require(value["schema_version"] == 1 and value["profile"] == PROFILE
            and value["scope"] in GROUPS and value["promotion_authority"] is False
            and value["accepted"] is False and value["launchable"] is False,
            "unsupported plan or authority claim")
    art.validate_subject(value["subject"], feedback=False)
    require(art.subject(Path(value["repo"])) == value["subject"], "head/source/assets changed")
    require(value["verifier_sha256"] == pipeline.digest(Path(__file__).read_bytes()), "verifier changed")
    require(value["input_transport_sha256"] == pipeline.digest(
        Path(__file__).with_name("native_ui_input.py").read_bytes()), "input transport changed")
    require(NONCE.fullmatch(value["campaign_nonce"]) is not None, "invalid campaign nonce")
    budget(value["budget"])
    require(value["budget"]["frozen_at_ns"] <= value["created_at_ns"], "budget was not frozen")
    require(set(value["binaries"]) == {"Capture", "Memory"}, "sequential instruments required")
    baseline_binaries = []
    if value["scope"] == "full":
        baseline = value["baseline"]
        art.validate_subject(baseline["subject"], feedback=False)
        require(art.subject(Path(baseline["repo"])) == baseline["subject"]
                and baseline["subject"]["source"] != value["subject"]["source"]
                and set(baseline["binaries"]) == {"Capture", "Memory"},
                "cumulative comparison requires an exact distinct-source baseline with the same normal ten-kind case")
        baseline_binaries = list(baseline["binaries"].values())
    else:
        require(value.get("baseline") is None, "group comparison must use the same binary/source")
    for spec in [value["codec"], value["driver"], *value["binaries"].values(), *baseline_binaries]:
        require(set(spec) == {"path", "sha256"} and hashed(spec["sha256"])
                and pipeline.digest(pipeline.no_symlinks(Path(spec["path"])).read_bytes()) == spec["sha256"],
                "binary/codec/driver changed")
    require(value["binaries"]["Capture"]["sha256"] != value["binaries"]["Memory"]["sha256"],
            "Capture and Memory must be separate builds")
    require(value["environment"]["backend"] == "Vulkan"
            and value["environment"]["window_backend"] == "x11"
            and all(isinstance(value["environment"].get(k), str) and value["environment"][k].strip()
                    for k in ("adapter", "driver", "present_mode")), "actual Vulkan/X11 environment required")
    require(value["registry_contract"] == registry_contract(value["scope"]), "registry requirements changed")
    require(hashed(value["gameplay_contract_sha256"]), "gameplay contract hash missing")
    require(set(value["cases"]) == {"N", "4N"}, "N/4N cases missing")
    for size, case in value["cases"].items():
        require(set(case) == {"copies", "world_seed", "terrain_sha256", "layout_sha256", "logical_state_sha256", "other_groups_sha256",
                             "camera_sha256", "population_sha256", "activity_sha256"}
                and type(case["copies"]) is int and case["copies"] == (4 if size == "N" else 16)
                and type(case["world_seed"]) is int and case["world_seed"] == 20260920
                and all(hashed(v) for key, v in case.items() if key not in {"copies", "world_seed"}), "invalid frozen comparison case")
    release_inventory(value)
    trials = value["generation_trials"]
    require(set(trials) == set(GROUPS[value["scope"]]) - {"Door"}, "generation trial inventory missing")
    for kind, records in trials.items():
        require(isinstance(records, list) and len(records) == 2
                and records[0] == value["releases"][kind], "generation trials require released A and newer C")
        previous = 0
        for record in records:
            manifest, text, _ = pipeline.load_set(Path(record["root"]), kind, Path(value["codec"]["path"]))
            require(manifest["identity"] == record["identity"]
                    and record["identity"]["authority"] == "release_approved"
                    and record["identity"]["generation"] > previous
                    and pipeline.digest(text.encode()) == record["manifest_file_sha256"], "unreleased/mismatched generation trial")
            previous = record["identity"]["generation"]
    require(value["lifecycle_verifier_sha256"] == pipeline.digest(
        Path(__file__).with_name("building_production_lifecycle.py").read_bytes()), "lifecycle verifier changed")


def plan(spec, *, registration_adapter=None):
    binding = registration_binding(spec, registration_adapter=registration_adapter)
    require(set(spec) == SPEC_KEYS, "unknown/missing specification fields; baseline is null for groups")
    value = {**spec, "schema_version": 1, "profile": PROFILE,
             "accepted": False, "promotion_authority": False, "launchable": False,
             "created_at_ns": time.time_ns(), "campaign_nonce": secrets.token_hex(16),
             "verifier_sha256": pipeline.digest(Path(__file__).read_bytes()),
             "input_transport_sha256": pipeline.digest(Path(__file__).with_name("native_ui_input.py").read_bytes()),
             "lifecycle_verifier_sha256": pipeline.digest(Path(__file__).with_name("building_production_lifecycle.py").read_bytes()),
             "registry_contract": registry_contract(spec["scope"]),
             "registry_sha256": binding["registry_sha256"]}
    check_plan(value, registration_adapter=registration_adapter)
    return value


def bind_lifecycle_observations(trace, raw):
    """Require lossless raw evidence, not driver-authored lifecycle outcomes.

    Only session metadata and the leg selector may be added by the driver.
    Every other field (including future predicate inputs) must already exist in
    the hashed runtime artifact. No sidecar witness authority is inferred here.
    Keep the complete sample/event order so witness indices cannot be remapped.
    """
    envelope = {"schema_version", "scope", "leg", "nonce", "process_id",
                "failure", "accepted", "fixture_seeded_completion", "subject",
                "plan_sha256", "binary_sha256", "driver_sha256",
                "raw_observations_sha256", "performance_evidence"}
    require(raw.get("promotion_authority") is False, "raw observation authority differs")
    samples = trace.get("samples")
    raw_samples = raw.get("samples")
    require(isinstance(samples, list) and isinstance(raw_samples, list)
            and len(samples) == len(raw_samples) and len(samples) >= 2,
            "lifecycle requires the complete raw sample sequence")
    for index, (sample, observed) in enumerate(zip(samples, raw_samples)):
        require(isinstance(sample, dict) and isinstance(observed, dict)
                and type(sample.get("raw_sample_index")) is int
                and sample["raw_sample_index"] == index,
                "normalized sample index differs")
        payload = {key: value for key, value in sample.items() if key != "raw_sample_index"}
        require(pipeline.canonical(payload) == pipeline.canonical(observed),
                "normalized sample differs from raw runtime observation")
    require(isinstance(trace.get("events"), list) and isinstance(raw.get("events"), list),
            "raw domain events missing")
    for key in set(trace) - envelope - {"samples"}:
        require(key in raw and pipeline.canonical(trace[key]) == pipeline.canonical(raw[key]),
                f"lifecycle field has no matching raw witness: {key}")


def session(root, row, plan_value, plan_hash, seen, *, performance, registration_adapter=None):
    """Bind raw trace, owned client capture, ACK, renderer and instrument to one run."""
    baseline = performance and plan_value["scope"] == "full" and row.get("mode") == "legacy-control"
    expected_subject = plan_value["baseline"]["subject"] if baseline else plan_value["subject"]
    binaries = plan_value["baseline"]["binaries"] if baseline else plan_value["binaries"]
    require(row["plan_sha256"] == plan_hash and row["subject"] == expected_subject
            and row["campaign_nonce"] == plan_value["campaign_nonce"]
            and row["codec_sha256"] == plan_value["codec"]["sha256"]
            and row["driver_sha256"] == plan_value["driver"]["sha256"]
            and row["releases"] == plan_value["releases"]
            and row["generation_trials"] == plan_value["generation_trials"]
            and row["environment"] == plan_value["environment"], "session subject differs")
    nonce = row["nonce"]
    require(isinstance(nonce, str) and NONCE.fullmatch(nonce) is not None and nonce not in seen,
            "missing/reused session nonce")
    seen.add(nonce)
    require(row["instrument"] in {"Capture", "Memory"}
            and row["binary_sha256"] == binaries[row["instrument"]]["sha256"],
            "instrument binary differs")
    require(type(row["pid"]) is int and row["pid"] > 0
            and type(row["started_at_ns"]) is int and type(row["finished_at_ns"]) is int
            and plan_value["created_at_ns"] < row["started_at_ns"] < row["finished_at_ns"],
            "stale or invalid run interval")
    require(row["world"] == "normal-generated" and row["fixture_seeded_completion"] is False
            and row["headless"] is False and row["static_gallery"] is False
            and row["promotion_authority"] is False, "non-production-world evidence")
    log = artifact(root, row["log"]).decode()
    require(not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log), "native warnings/errors")
    adapters = re.findall(r'AdapterInfo \{ name: "([^\"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    require(adapters == [(plan_value["environment"]["adapter"], "Vulkan")], "actual adapter differs")
    trace = json_bytes(artifact(root, row["trace"]))
    require(trace["nonce"] == nonce and trace["process_id"] == row["pid"]
            and trace["failure"] is None and trace["accepted"] is False
            and trace["fixture_seeded_completion"] is False
            and trace["subject"] == expected_subject
            and trace["plan_sha256"] == plan_hash
            and trace["binary_sha256"] == row["binary_sha256"]
            and trace["driver_sha256"] == row["driver_sha256"], "trace session differs")
    if not performance:
        raw = json_bytes(artifact(root, row["raw_observations"]))
        require(raw["scope"] == "production-raw-observations-v1" and raw["nonce"] == nonce
                and raw["process_id"] == row["pid"] and raw["failure"] is None
                and raw["accepted"] is False and raw["performance_evidence"] is False
                and trace["raw_observations_sha256"] == row["raw_observations"]["sha256"],
                "missing or mismatched underlying runtime observations")
        bind_lifecycle_observations(trace, raw)
        if __package__:
            from . import building_production_collection as collection
        else:
            import building_production_collection as collection
        collection.verify_log(root, row, plan_value, raw, trace, registration_adapter=registration_adapter)
    shots = row["captures"]
    require(isinstance(shots, list) and 2 <= len(shots) <= 128, "actual-window captures missing")
    for shot in shots:
        require(shot["nonce"] == nonce and shot["pid"] == row["pid"]
                and shot["capture_scope"] == "owned-x11-client"
                and isinstance(shot["window_id"], str) and shot["window_id"], "unowned window capture")
        require(type(shot["captured_at_ns"]) is int
                and row["started_at_ns"] <= shot["captured_at_ns"] <= row["finished_at_ns"], "stale capture")
        width, height = art.native.validate_png_structure(artifact(root, shot["image"]))
        require(width >= 640 and height >= 360, "capture dimensions too small")
        ack = json_bytes(artifact(root, shot["ack"]))
        require(ack == {"nonce": nonce, "pid": row["pid"], "window_id": shot["window_id"],
                        "image_sha256": shot["image"]["sha256"], "trace_sha256": row["trace"]["sha256"],
                        "sample_index": shot["sample_index"], "captured_at_ns": shot["captured_at_ns"]}, "capture ACK binding differs")
        require(type(shot["sample_index"]) is int and 0 <= shot["sample_index"] < len(trace["samples"]),
                "capture sample missing")
    require(len({shot["sample_index"] for shot in shots}) == len(shots), "duplicate capture samples")
    require(trace["performance_evidence"] is performance, "trace measurement scope differs")
    return trace


def bind_external_resources(row, samples, gpu, handles):
    binding = {key: row[key] for key in ("subject", "plan_sha256", "binary_sha256", "codec_sha256",
                                       "driver_sha256", "nonce", "pid", "environment")}
    require(pipeline.canonical(handles.get("session")) == pipeline.canonical(binding)
            and pipeline.canonical(gpu.get("session")) == pipeline.canonical(binding),
            "external GPU/application-handle subject differs")
    expected_handles = [{"seconds": s["seconds"], "application_handle_count": s["metrics"]["application_handle_count"],
                         "retired_application_handles": s["retired_application_handles"]} for s in samples]
    expected_gpu = [{"seconds": s["seconds"], "measured_bytes": s["metrics"]["gpu_measured_bytes"],
                     "estimated_bytes": s["metrics"]["gpu_estimated_bytes"], "source": s["gpu_measurement_source"]}
                    for s in samples]
    require(all(type(s["application_handle_count"]) is int and s["application_handle_count"] >= 0
                for s in expected_handles)
            and all(number(s["measured_bytes"]) and number(s["estimated_bytes"]) for s in expected_gpu),
            "null/nonfinite observer fields are not external measurements")
    require(pipeline.canonical(handles.get("samples")) == pipeline.canonical(expected_handles)
            and pipeline.canonical(gpu.get("samples")) == pipeline.canonical(expected_gpu),
            "external GPU/application-handle samples differ")


def inspect_performance(root, row, value, plan_hash, seen, *, registration_adapter=None):
    trace = session(root, row, value, plan_hash, seen, performance=True,
                    registration_adapter=registration_adapter)
    require(row["case"] == value["cases"][row["size"]], "layout/state/camera/population/activity differs")
    require(number(row["warmup_seconds"]) and number(row["measure_seconds"])
            and row["warmup_seconds"] >= 30 and row["measure_seconds"] >= 60
            and row["diagnostic_probe_enabled"] is False, "measurement protocol differs")
    samples = trace["samples"]
    require(len(samples) >= 60, "insufficient raw measurement samples")
    observed_metrics = METRICS - {"p95_ms", "p99_ms"}
    if row["instrument"] == "Capture":
        observed_metrics = observed_metrics - {"native_peak_live_bytes"}
    previous = -1
    for sample in samples:
        require(number(sample["seconds"]) and sample["seconds"] > previous
                and sample["paused"] is False and sample["case"] == row["case"],
                "static/nonmonotonic/different workload")
        previous = sample["seconds"]
        require(set(sample["metrics"]) == observed_metrics
                and all(number(v) for v in sample["metrics"].values())
                and all(type(sample["metrics"][k]) is int for k in COUNT_METRICS)
                and sample["gpu_measurement_source"] in {"vulkan-memory-budget", "external-profiler"},
                "missing/nonfinite/mislabelled resource measurement")
        require(sample["retired_application_handles"] == 0 and sample["orphan_parts"] == 0
                and sample["pool_pending"] == 0, "unsettled pool/owner cleanup")
    require(samples[-1]["seconds"] - samples[0]["seconds"] >= 59, "truncated measurement interval")
    frames = json_bytes(artifact(root, row["frames"]))
    require(frames["nonce"] == row["nonce"] and frames["pid"] == row["pid"]
            and frames["measurement_start_seconds"] >= row["warmup_seconds"]
            and frames["measurement_end_seconds"] - frames["measurement_start_seconds"] >= row["measure_seconds"],
            "frame session/window differs")
    times = frames["frame_ms"]
    require(len(times) >= 120 and all(number(v, positive=True) for v in times)
            and sum(times) >= 59000, "invalid frame window")
    ordered = sorted(times)
    metrics = {"p95_ms": ordered[math.ceil(len(times) * .95) - 1],
               "p99_ms": ordered[math.ceil(len(times) * .99) - 1]}
    metrics.update({key: max(sample["metrics"][key] for sample in samples)
                    for key in observed_metrics})
    artifact(root, row["resource_usage"])
    usage, errors = read_resource_usage(pipeline.rooted(root, row["resource_usage"]["path"]))
    require(not errors and usage["exit_status"] == 0 and metrics["rss_bytes"] == usage["max_rss_kib"] * 1024,
            "RSS units/process resource evidence differs")
    if row["instrument"] == "Memory":
        artifact(root, row["native_allocator"])
        memory, errors = read_native_memory(pipeline.rooted(root, row["native_allocator"]["path"]), frame_samples=len(times))
        require(not errors and metrics["native_peak_live_bytes"] == memory["peak_live_bytes"], "native allocator evidence differs")
    else:
        require(row.get("native_allocator") is None, "Capture cannot claim native instrumentation")
    gpu = json_bytes(artifact(root, row["gpu_observations"]))
    handles = json_bytes(artifact(root, row["application_handles"]))
    bind_external_resources(row, samples, gpu, handles)
    require(gpu["nonce"] == row["nonce"] and gpu["pid"] == row["pid"]
            and gpu["adapter"] == row["environment"]["adapter"]
            and gpu["driver"] == row["environment"]["driver"]
            and gpu["measured_peak_bytes"] == metrics["gpu_measured_bytes"]
            and gpu["estimated_peak_bytes"] == metrics["gpu_estimated_bytes"], "GPU measurement/estimate differs")
    require(row["mode"] in {"candidate", "legacy-control"}, "invalid comparison mode")
    inventory = json_bytes(artifact(root, row["resident_inventory"]))
    require(inventory["nonce"] == row["nonce"] and inventory["pid"] == row["pid"]
            and inventory["other_groups_sha256"] == row["case"]["other_groups_sha256"], "resident session differs")
    expected = [] if row["mode"] == "legacy-control" else [
        spec["identity"] for kind, spec in value["releases"].items() if kind != "Door"]
    require(inventory["target_identities"] == expected, "control preloaded candidate or candidate not resident")
    copies = row["case"]["copies"]
    roots, candidate_parts = {"m2": (2, 4), "m3": (2, 6), "m5": (0, 0),
                              "bridge": (1, 1), "full": (6, 12)}[value["scope"]]
    require(metrics["root_count"] == roots * copies
            and metrics["part_count"] == (candidate_parts if row["mode"] == "candidate" else roots) * copies,
            "target root/mesh-part structure differs")
    return metrics


def verify_results(plan_path, result_path, *, registration_adapter=None):
    value = pipeline.read(plan_path)
    check_plan(value, registration_adapter=registration_adapter)
    plan_hash = pipeline.digest(plan_path.read_bytes())
    result = pipeline.read(result_path)
    root = result_path.parent
    require(result["profile"] == PROFILE and result["promotion_authority"] is False
            and result["plan_sha256"] == plan_hash, "wrong result scope")
    receipt = json_bytes(artifact(root, result["registration_receipt"]))
    admitted_at_ns = check_registration_receipt(value, receipt, registration_adapter=registration_adapter)
    rows = result["performance"]
    require([(r["instrument"], r["size"], r["repeat"], r["mode"]) for r in rows] == matrix(),
            "missing/reordered adjacent reversed three-repeat Capture/Memory matrix")
    seen, metrics, end = set(), {}, admitted_at_ns
    for row in rows:
        require(row["started_at_ns"] > end, "overlapping/out-of-order instrumentation")
        observed = inspect_performance(root, row, value, plan_hash, seen,
                                       registration_adapter=registration_adapter)
        end = row["finished_at_ns"]
        metrics[(row["instrument"], row["size"], row["repeat"], row["mode"])] = observed
    summaries = {}
    for instrument in ("Capture", "Memory"):
        for size in ("N", "4N"):
            for key in METRICS:
                # Native allocations are judged only in Memory, timing only in Capture.
                if (key.startswith("p9") and instrument != "Capture") or (key == "native_peak_live_bytes" and instrument != "Memory"):
                    continue
                series = {mode: [metrics[(instrument, size, rep, mode)][key] for rep in range(1, 4)]
                          for mode in ("legacy-control", "candidate")}
                stats = {}
                for mode, numbers in series.items():
                    median = statistics.median(numbers)
                    mad = statistics.median(abs(n - median) for n in numbers)
                    require(mad / max(median, 1) <= value["budget"]["max_relative_mad"], "measurement noise exceeds frozen budget")
                    stats[mode] = {"median": median, "mad": mad}
                require(stats["candidate"]["median"] - stats["legacy-control"]["median"] <= value["budget"]["max_delta"][key],
                        f"budget exceeded: {instrument}/{size}/{key}")
                summaries[f"{instrument}/{size}/{key}"] = stats
        for repeat in range(1, 4):
            # Instance/entity-owned references can grow; shared asset counts cannot.
            for key in ("mesh_count", "image_count", "material_count"):
                require(metrics[(instrument, "N", repeat, "candidate")][key]
                        == metrics[(instrument, "4N", repeat, "candidate")][key], "shared asset pool scales with owners")
    if __package__:
        from . import building_production_lifecycle as lifecycle
    else:
        import building_production_lifecycle as lifecycle
    lifecycle.verify(root, result["lifecycle"], value, plan_hash, seen,
                     registration_adapter=registration_adapter)
    check_plan(value, registration_adapter=registration_adapter)
    require(result == pipeline.read(result_path) and pipeline.digest(plan_path.read_bytes()) == plan_hash,
            "inputs changed during verification")
    return {"profile": PROFILE, "scope": value["scope"], "evidence_verified": True,
            "promotion_authority": False, "art_approved": False, "release_approved": False,
            "plan_sha256": plan_hash, "result_sha256": pipeline.digest(result_path.read_bytes()),
            "subject": value["subject"], "metrics": summaries,
            "comparison": "distinct-source-cumulative" if value["scope"] == "full" else "same-binary-group"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    registry = commands.add_parser("registry-contract")
    registry.add_argument("--scope", choices=GROUPS, required=True)
    create = commands.add_parser("plan")
    create.add_argument("--spec", type=Path, required=True)
    create.add_argument("--output", type=Path, required=True)
    verify = commands.add_parser("verify-results")
    verify.add_argument("--plan", type=Path, required=True)
    verify.add_argument("--result", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "registry-contract":
        result = registry_contract(args.scope)
    elif args.command == "plan":
        result = plan(pipeline.read(args.spec))
        pipeline.put(args.output, pipeline.canonical(result))
    else:
        result = verify_results(args.plan, args.result)
    print(json.dumps(result, indent=2, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
