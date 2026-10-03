"""Registered host adapter and producer for TAK-14 production evidence.

This helper never approves art, assets, release, installation or promotion.  A
literal infrastructure recipe must seal its plan before ``run`` is reachable.
The host owns process/window admission, captures and external instruments; this
driver only sequences those admitted sessions and the ordinary-input collector.
"""
from __future__ import annotations

import argparse
import contextlib
import copy
import json
import os
from pathlib import Path
import time

if __package__:
    from . import building_production_acceptance as evidence
    from . import building_production_collection as collection
    from . import building_production_lifecycle as lifecycle
else:
    import building_production_acceptance as evidence
    import building_production_collection as collection
    import building_production_lifecycle as lifecycle


PROFILE = "building-production-native-v1"
RECIPE_ID = "building-production-v1"
PREFIX = ["env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3"]
RECIPE_CONTRACT = {
    "id": RECIPE_ID,
    "helper": "building_production_native_acceptance.py",
    "directories": ["scripts"],
    "format": "plan",
    "flags": ["--plan"],
    "bindings": {"repo": "repo", "root": "job_root", "head": "subject.commit"},
    "values": {
        "schema_version": [1],
        "profile": [PROFILE],
        "accepted": [False],
        "promotion_authority": [False],
    },
    "documents": [{
        "field": "runtime_plan",
        "hash_field": "runtime_plan_sha256",
        "equal_fields": ["repo", "subject"],
        "values": {
            "schema_version": [1],
            "profile": [evidence.PROFILE],
            "accepted": [False],
            "promotion_authority": [False],
            "launchable": [False],
        },
        "executables": [
            {"field": "binaries.Capture.path", "hash_field": "binaries.Capture.sha256",
             "paths": ["target/profiling/bevy_app"], "retained_area": "target", "immutable": True},
            {"field": "binaries.Memory.path", "hash_field": "binaries.Memory.sha256",
             "paths": ["target/profiling/bevy_app"], "retained_area": "target", "immutable": True},
        ],
    }],
    "verifiers": [{"helper": "@helper", "args": ["verify", "--job-root", "@root"]}],
}


def require(value, reason):
    evidence.require(value, reason)


def canonical(value):
    return evidence.pipeline.canonical(value)


def digest(value):
    return evidence.pipeline.digest(value)


def record(path: Path) -> dict:
    payload = path.read_bytes()
    return {"path": str(path), "bytes": len(payload), "sha256": digest(payload)}


def recipe_binding(value: dict, receipt: dict | None = None) -> dict:
    """Validate the exact live catalog/intent; never authenticate saved JSON.

    Authenticity remains with the broker that admits this exact plan and later
    seals the batch.  This adapter supplies product-side equality checks only.
    """
    context = evidence.check_registration_context(value)
    capabilities = evidence.host_document(value.get("host_capabilities"))
    require(capabilities.get("ok") is True
            and capabilities.get("revision") == context["recipe_revision"],
            "live recipe revision differs")
    contracts = capabilities.get("contracts")
    require(isinstance(contracts, dict) and contracts.get("schema") == 1,
            "host recipe catalog missing")
    rows = [row for row in contracts.get("recipes", []) if row.get("id") == RECIPE_ID]
    require(rows == [RECIPE_CONTRACT], "reviewed production recipe is unavailable or changed")
    runner = Path(__file__).resolve()
    intent = value["registration_intent"]
    require(isinstance(intent.get("command"), list) and len(intent["command"]) == 8
            and isinstance(intent.get("spec"), dict)
            and isinstance(intent["spec"].get("roots"), list)
            and len(intent["spec"]["roots"]) == 1,
            "host intent does not contain one exact plan and output root")
    native_plan_path = intent["command"][-1]
    native_job_root = intent["spec"]["roots"][0]
    expected_command = [*PREFIX, str(runner), "run", "--plan", native_plan_path]
    expected_verify = ["python3", str(runner), "verify", "--job-root", native_job_root]
    require(intent["command"] == expected_command
            and Path(native_plan_path).is_absolute()
            and Path(native_plan_path).resolve() == Path(native_plan_path)
            and Path(native_job_root).is_absolute()
            and Path(native_job_root).resolve() == Path(native_job_root)
            and intent["spec"]["verify_command"] == expected_verify,
            "host intent does not bind the exact runner plan/root/verifier")
    registry = digest(canonical({"revision": capabilities["revision"], "recipe": RECIPE_CONTRACT}))
    result = {"registry_sha256": registry}
    if receipt is None:
        return result
    required = {"schema", "authority", "request", "batch", "phase", "registered_at_ns",
                "controller_sha256", "recipe_revision", "bridge_revision", "subject",
                "recipe_id", "intent_sha256", "plan_sha256", "runner_sha256",
                "verifier_sha256", "job_root", "command", "verify_command",
                "promotion_authority"}
    require(isinstance(receipt, dict) and set(receipt) == required
            and receipt["schema"] == 1
            and receipt["authority"] == "host-admitted-native-registration"
            and receipt["phase"] == "registered"
            and receipt["request"] == value["registration_request"]
            and receipt["controller_sha256"] == context["controller_sha256"]
            and receipt["recipe_revision"] == context["recipe_revision"]
            and receipt["bridge_revision"] == context["bridge_revision"]
            and receipt["subject"] == context["subject"]
            and receipt["recipe_id"] == RECIPE_ID
            and receipt["intent_sha256"] == digest(json.dumps(
                intent, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
                allow_nan=False).encode())
            and receipt["runner_sha256"] == digest(runner.read_bytes())
            and receipt["verifier_sha256"] == receipt["runner_sha256"]
            and receipt["job_root"] == native_job_root
            and receipt["command"] == expected_command
            and receipt["verify_command"] == expected_verify
            and receipt["promotion_authority"] is False
            and type(receipt["registered_at_ns"]) is int,
            "host registration adapter receipt differs")
    result.update(plan_sha256=receipt["plan_sha256"],
                  receipt_sha256=digest(canonical(receipt)),
                  admitted_at_ns=receipt["registered_at_ns"])
    return result


@contextlib.contextmanager
def adapter_scope():
    original_binding = evidence.registration_binding
    original_required = evidence.require_production_registration
    original_receipt = evidence.check_registration_receipt

    def binding(value, receipt=None):
        result = recipe_binding(value, receipt)
        require(evidence.hashed(value.get("registry_sha256"))
                and value["registry_sha256"] == result["registry_sha256"],
                "authenticated registry identity differs")
        return result

    def admitted(value, receipt):
        result = binding(value, receipt)
        require(result["plan_sha256"] == digest(canonical(value))
                and result["receipt_sha256"] == digest(canonical(receipt))
                and result["admitted_at_ns"] > value["created_at_ns"],
                "host receipt does not bind this exact runtime plan")
        return result["admitted_at_ns"]

    evidence.require_production_registration = recipe_binding
    evidence.registration_binding = binding
    evidence.check_registration_receipt = admitted
    try:
        yield
    finally:
        evidence.registration_binding = original_binding
        evidence.require_production_registration = original_required
        evidence.check_registration_receipt = original_receipt


def expected_sessions(runtime: dict) -> list[dict]:
    performance = [{"purpose": "performance", "instrument": instrument,
                    "size": size, "repeat": repeat, "mode": mode}
                   for instrument, size, repeat, mode in evidence.matrix()]
    lifecycle_rows = [{"purpose": "lifecycle", "instrument": instrument,
                       "kind": kind, "leg": leg}
                      for instrument in ("Capture", "Memory")
                      for kind in evidence.GROUPS[runtime["scope"]]
                      for leg in lifecycle.LEGS[kind]]
    rows = [row for instrument in ("Capture", "Memory")
            for row in [*performance, *lifecycle_rows] if row["instrument"] == instrument]
    return [{"ordinal": index, **row} for index, row in enumerate(rows)]


def check_session_plan(sessions: list[dict], runtime: dict, root: Path) -> None:
    expected = expected_sessions(runtime)
    require(isinstance(sessions, list) and len(sessions) == len(expected),
            "missing production sessions")
    seen = set()
    for planned, wanted in zip(sessions, expected, strict=True):
        fixed = {key: planned.get(key) for key in wanted}
        require(fixed == wanted and set(planned) == set(wanted) | {
            "nonce", "descriptor", "sealed_row", "steps"},
            "production session order/shape differs")
        require(isinstance(planned["nonce"], str)
                and evidence.NONCE.fullmatch(planned["nonce"])
                and planned["nonce"] not in seen, "missing/reused session nonce")
        seen.add(planned["nonce"])
        collection.validate_steps(planned["steps"])
        for key in ("descriptor", "sealed_row"):
            path = Path(planned[key])
            require(path.is_absolute() and path.resolve() == path
                    and path.is_relative_to(root / "host-sessions"),
                    "host session path escapes the registered output root")


def check_native_plan(value: dict) -> tuple[dict, Path]:
    required = {"schema_version", "profile", "repo", "job_root", "subject",
                "native_plan_path", "runtime_plan", "runtime_plan_sha256", "registration_receipt_path",
                "sessions", "accepted", "promotion_authority", "runner_sha256"}
    require(isinstance(value, dict) and set(value) == required
            and value["schema_version"] == 1 and value["profile"] == PROFILE
            and value["accepted"] is False and value["promotion_authority"] is False,
            "invalid native production plan")
    root = Path(value["job_root"])
    native_path = Path(value["native_plan_path"])
    runtime_path = Path(value["runtime_plan"])
    runner = Path(__file__).resolve()
    require(root.is_absolute() and root.resolve() == root
            and native_path.is_absolute() and native_path.resolve() == native_path
            and runtime_path.is_absolute() and runtime_path.resolve(strict=True) == runtime_path
            and digest(runtime_path.read_bytes()) == value["runtime_plan_sha256"]
            and value["runner_sha256"] == digest(runner.read_bytes()),
            "native plan path/helper changed")
    runtime = evidence.pipeline.read(runtime_path)
    intent = runtime["registration_intent"]
    require(runtime["repo"] == value["repo"] and runtime["subject"] == value["subject"]
            and intent["command"][-1] == str(native_path)
            and intent["spec"]["roots"] == [str(root)],
            "runtime/native subject, plan, or output root differs")
    receipt_path = Path(value["registration_receipt_path"])
    require(receipt_path.is_absolute() and receipt_path.resolve() == receipt_path
            and receipt_path.is_relative_to(root / "host-registration"),
            "registration receipt path escapes output root")
    with adapter_scope():
        evidence.check_plan(runtime)
    check_session_plan(value["sessions"], runtime, root)
    return runtime, root


def instrument_session_binding(plan, plan_hash, binding, admission, steps, instrument):
    registry = evidence.registration_binding(plan)
    require(instrument in {"Capture", "Memory"}
            and binding["binary_sha256"] == plan["binaries"][instrument]["sha256"],
            "session instrument/binary differs")
    require(isinstance(binding, dict) and set(binding) == set(collection.SESSION_KEYS)
            and binding["subject"] == plan["subject"]
            and binding["plan_sha256"] == plan_hash
            and binding["campaign_nonce"] == plan["campaign_nonce"]
            and binding["codec_sha256"] == plan["codec"]["sha256"]
            and binding["driver_sha256"] == plan["driver"]["sha256"]
            and binding["input_transport_sha256"] == plan["input_transport_sha256"]
            and isinstance(binding["nonce"], str) and evidence.NONCE.fullmatch(binding["nonce"])
            and all(type(binding[key]) is int and binding[key] > 0
                    for key in ("pid", "root_pid", "window_id"))
            and binding["kind"] in evidence.GROUPS[plan["scope"]]
            and binding["leg"] in lifecycle.LEGS[binding["kind"]],
            "session subject/driver/kind/leg differs")
    expected_admission = {
        "authority": "host-admitted-production-session",
        "registry_sha256": registry["registry_sha256"],
        "session": {key: binding[key] for key in collection.SESSION_KEYS},
        "steps_sha256": digest(canonical(steps)),
        "world": "normal-generated", "headless": False,
        "fixture_seeded_completion": False,
    }
    require(canonical(admission) == canonical(expected_admission),
            "host session admission differs")


@contextlib.contextmanager
def collection_scope(instrument):
    original = collection.session_binding
    collection.session_binding = lambda plan, plan_hash, binding, admission, steps: (
        instrument_session_binding(plan, plan_hash, binding, admission, steps, instrument))
    try:
        yield
    finally:
        collection.session_binding = original


def read_when_available(path: Path, deadline: float) -> dict:
    while not path.exists():
        require(time.monotonic() < deadline, f"host artifact unavailable: {path.name}")
        time.sleep(.05)
    return evidence.pipeline.read(path)


def check_sealed_row(row: dict, planned: dict, binding: dict, runtime: dict, plan_hash: str) -> None:
    require(row.get("instrument") == planned["instrument"]
            and row.get("nonce") == planned["nonce"]
            and row.get("subject") == runtime["subject"]
            and row.get("plan_sha256") == plan_hash
            and row.get("binary_sha256") == runtime["binaries"][planned["instrument"]]["sha256"]
            and row.get("pid") == binding["pid"]
            and row.get("world") == "normal-generated"
            and row.get("headless") is False
            and row.get("static_gallery") is False
            and row.get("fixture_seeded_completion") is False
            and row.get("promotion_authority") is False,
            "host-sealed row differs from admitted session")
    if planned["purpose"] == "performance":
        require(all(row.get(key) == planned[key] for key in ("size", "repeat", "mode")),
                "performance session selector differs")
    else:
        require(row.get("kind") == planned["kind"] and row.get("leg") == planned["leg"],
                "lifecycle session selector differs")


def run(plan_path: Path) -> dict:
    require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
            "use the registered no-prompt launcher")
    source_bytes = plan_path.read_bytes()
    value = evidence.json_bytes(source_bytes)
    runtime, root = check_native_plan(value)
    require(not root.exists(), "native production root already exists")
    root.mkdir(parents=True)
    (root / "host-registration").mkdir()
    (root / "host-sessions").mkdir()
    collection.write_once(root / "plan.json", source_bytes)
    runtime_bytes = Path(value["runtime_plan"]).read_bytes()
    collection.write_once(root / "runtime-plan.json", runtime_bytes)
    plan_hash = digest(runtime_bytes)
    receipt_path = Path(value["registration_receipt_path"])
    deadline = time.monotonic() + 300
    receipt = read_when_available(receipt_path, deadline)
    with adapter_scope():
        admitted = evidence.check_registration_receipt(runtime, receipt)
        require(admitted < time.time_ns(), "registration admission time is in the future")
        performance, lifecycle_capture, lifecycle_memory = [], [], []
        for planned in value["sessions"]:
            descriptor = read_when_available(Path(planned["descriptor"]), deadline)
            require(isinstance(descriptor, dict)
                    and set(descriptor) == {"binding", "admission", "raw_path"},
                    "invalid host session descriptor")
            binding = descriptor["binding"]
            require(binding.get("nonce") == planned["nonce"], "host session nonce differs")
            output = root / f"collection-{planned['ordinal']:04d}"
            with collection_scope(planned["instrument"]):
                collection.collect(root / "runtime-plan.json", binding, descriptor["admission"],
                                   planned["steps"], Path(descriptor["raw_path"]), output)
            row = read_when_available(Path(planned["sealed_row"]), deadline)
            check_sealed_row(row, planned, binding, runtime, plan_hash)
            if planned["purpose"] == "performance":
                performance.append(row)
            elif planned["instrument"] == "Capture":
                lifecycle_capture.append(row)
            else:
                lifecycle_memory.append(row)
        receipt_copy = root / "registration-receipt.json"
        collection.write_once(receipt_copy, canonical(receipt))
        result = {
            "profile": evidence.PROFILE,
            "promotion_authority": False,
            "accepted": False,
            "plan_sha256": plan_hash,
            "registration_receipt": evidence.pipeline.record(
                receipt_copy.name, receipt_copy.read_bytes()),
            "performance": performance,
            "lifecycle": lifecycle_capture,
            "memory_lifecycle": lifecycle_memory,
        }
        collection.write_once(root / "result.json", canonical(result))
    verify(root)
    return result


def verify(root: Path) -> dict:
    native_plan = evidence.pipeline.read(root / "plan.json")
    runtime, expected_root = check_native_plan(native_plan)
    require(root.resolve() == expected_root and digest((root / "runtime-plan.json").read_bytes())
            == native_plan["runtime_plan_sha256"], "verified job root/runtime plan differs")
    receipt = evidence.pipeline.read(root / "registration-receipt.json")
    result = evidence.pipeline.read(root / "result.json")
    require(result.get("accepted") is False and result.get("promotion_authority") is False,
            "native result gained authority")
    expected_memory = {(kind, leg) for kind in evidence.GROUPS[runtime["scope"]]
                       for leg in lifecycle.LEGS[kind]}
    memory = result.get("memory_lifecycle")
    require(isinstance(memory, list) and len(memory) == len(expected_memory)
            and {(row.get("kind"), row.get("leg")) for row in memory} == expected_memory
            and all(row.get("instrument") == "Memory" for row in memory),
            "missing/duplicate Memory lifecycle sessions")
    with adapter_scope():
        evidence.check_registration_receipt(runtime, receipt)
        seen = set()
        plan_hash = digest((root / "runtime-plan.json").read_bytes())
        for row in memory:
            with collection_scope("Memory"):
                evidence.session(root, row, runtime, plan_hash, seen, performance=False)
        verified = evidence.verify_results(root / "runtime-plan.json", root / "result.json")
    require(verified["promotion_authority"] is False
            and verified["art_approved"] is False
            and verified["release_approved"] is False,
            "technical verifier granted product authority")
    return {**verified, "profile": PROFILE, "memory_lifecycle_verified": True,
            "accepted": False, "promotion_authority": False,
            "art_approved": False, "release_approved": False}


def create_plan(spec_path: Path, output: Path) -> dict:
    envelope = evidence.pipeline.read(spec_path)
    require(isinstance(envelope, dict)
            and set(envelope) == {"runtime_spec", "job_root", "registration_receipt_path", "sessions"},
            "invalid native planning envelope")
    runtime_spec = copy.deepcopy(envelope["runtime_spec"])
    job_root = str(Path(envelope["job_root"]).resolve())
    require(runtime_spec["registration_intent"]["command"][-1] == str(output.resolve())
            and runtime_spec["registration_intent"]["spec"]["roots"] == [job_root],
            "planning envelope differs from registered plan/root")
    with adapter_scope():
        runtime = evidence.plan(runtime_spec)
    runtime_path = output.with_name(output.stem + "-runtime.json").resolve()
    evidence.pipeline.put(runtime_path, canonical(runtime))
    value = {
        "schema_version": 1, "profile": PROFILE,
        "repo": runtime["repo"], "job_root": job_root,
        "subject": runtime["subject"], "native_plan_path": str(output.resolve()),
        "runtime_plan": str(runtime_path),
        "runtime_plan_sha256": digest(runtime_path.read_bytes()),
        "registration_receipt_path": str(Path(envelope["registration_receipt_path"]).resolve()),
        "sessions": envelope["sessions"], "accepted": False,
        "promotion_authority": False, "runner_sha256": digest(Path(__file__).read_bytes()),
    }
    check_native_plan(value)
    evidence.pipeline.put(output, canonical(value))
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    planning = commands.add_parser("plan")
    planning.add_argument("--spec", type=Path, required=True)
    planning.add_argument("--output", type=Path, required=True)
    execute = commands.add_parser("run")
    execute.add_argument("--plan", type=Path, required=True)
    checking = commands.add_parser("verify")
    checking.add_argument("--job-root", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "plan":
        result = create_plan(args.spec, args.output)
    elif args.command == "run":
        result = run(args.plan)
    else:
        result = verify(args.job_root)
    print(json.dumps(result, indent=2, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
