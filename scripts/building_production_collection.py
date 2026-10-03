"""Bounded collector for an already host-admitted ordinary X11 world.

No build, launch, registry mutation, asset production or acceptance authority.
The host seals the resulting log/raw hashes separately; local hashes alone are
not a transport attestation. Unsupported lifecycle outcomes remain missing.
"""
from __future__ import annotations

import copy
import os
from pathlib import Path
import time

if __package__:
    from . import building_production_acceptance as evidence
    from .native_ui_input import X11Input
else:
    import building_production_acceptance as evidence
    from native_ui_input import X11Input

require = evidence.require
MAX_BYTES = 64 * 1024 * 1024
MAX_STEPS = 256
KEY_CODES = {**{f"F{i}": f"F{i}" for i in range(1, 13)},
             **{chr(i): f"Key{chr(i).upper()}" for i in range(ord("a"), ord("z") + 1)},
             **{str(i): f"Digit{i}" for i in range(10)},
             "Escape": "Escape", "Return": "Enter", "space": "Space",
             "Left": "ArrowLeft", "Right": "ArrowRight", "Up": "ArrowUp", "Down": "ArrowDown"}
SESSION_KEYS = ("subject", "binary_sha256", "codec_sha256", "driver_sha256",
                "input_transport_sha256", "plan_sha256", "campaign_nonce", "nonce",
                "pid", "root_pid", "window_id", "kind", "leg", "raw_observations_path")


def equal(a, b):
    return evidence.pipeline.canonical(a) == evidence.pipeline.canonical(b)


def bounded_read(path):
    with path.open("rb") as source:
        payload = source.read(MAX_BYTES + 1)
    require(len(payload) <= MAX_BYTES, "collection artifact exceeds byte cap")
    return payload


def write_once(path, payload):
    with path.open("xb") as destination:
        destination.write(payload)
        destination.flush()
        os.fsync(destination.fileno())


def session_binding(plan, plan_hash, binding, admission, steps):
    registry = evidence.registration_binding(plan)
    require(evidence.hashed(plan.get("registry_sha256"))
            and registry["registry_sha256"] == plan["registry_sha256"], "plan registry identity missing or differs")
    require(binding["subject"] == plan["subject"]
            and binding["plan_sha256"] == plan_hash
            and binding["campaign_nonce"] == plan["campaign_nonce"]
            and binding["binary_sha256"] == plan["binaries"]["Capture"]["sha256"]
            and binding["codec_sha256"] == plan["codec"]["sha256"]
            and binding["driver_sha256"] == plan["driver"]["sha256"]
            and binding["input_transport_sha256"] == plan["input_transport_sha256"],
            "collection subject/binary/codec/driver differs")
    require(isinstance(binding["nonce"], str) and evidence.NONCE.fullmatch(binding["nonce"])
            and all(type(binding[k]) is int and binding[k] > 0 for k in ("pid", "root_pid", "window_id")),
            "invalid owned process/window binding")
    if __package__:
        from .building_production_lifecycle import LEGS
    else:
        from building_production_lifecycle import LEGS
    require(binding["kind"] in evidence.GROUPS[plan["scope"]]
            and binding["leg"] in LEGS[binding["kind"]], "unsupported lifecycle leg")
    expected = {"authority": "host-admitted-production-session", "registry_sha256": registry["registry_sha256"],
                "session": {key: binding[key] for key in SESSION_KEYS},
                "steps_sha256": evidence.pipeline.digest(evidence.pipeline.canonical(steps)),
                "world": "normal-generated", "headless": False, "fixture_seeded_completion": False}
    require(equal(admission, expected), "host session admission differs")


def validate_steps(steps):
    require(isinstance(steps, list) and 1 <= len(steps) <= MAX_STEPS, "invalid input step count")
    seen, held = set(), set()
    for step in steps:
        require(set(step) == {"id", "input", "until", "timeout_seconds"}, "unknown input step fields")
        require(isinstance(step["id"], str) and 0 < len(step["id"]) <= 128 and step["id"] not in seen,
                "missing/repeated input step")
        seen.add(step["id"])
        require(evidence.number(step["timeout_seconds"], positive=True) and step["timeout_seconds"] <= 30,
                "unbounded input wait")
        action = step["input"]
        require(isinstance(action, dict) and set(action) <= {"point", "button", "key", "pressed"}
                and bool(action) and not ("button" in action and "key" in action), "unsupported ordinary input")
        if "point" in action:
            require(isinstance(action["point"], list) and len(action["point"]) == 2
                    and all(type(v) is int and v >= 0 for v in action["point"]), "invalid pointer point")
        token = None
        if "key" in action:
            require(isinstance(action["key"], str) and action["key"] in KEY_CODES, "unobserved key")
            token = ("key", action["key"])
        elif "button" in action:
            require(type(action["button"]) is int and action["button"] == 1, "only observed left pointer supported")
            token = ("button", 1)
        if token is not None:
            require(type(action.get("pressed")) is bool and (token in held) is not action["pressed"],
                    "repeated press or unmatched release")
            (held.add if action["pressed"] else held.remove)(token)
        else:
            require("point" in action and "pressed" not in action, "empty ordinary input")
        condition = step["until"]
        require(isinstance(condition, dict) and set(condition) == {"path", "equals"}
                and isinstance(condition["path"], list) and 1 <= len(condition["path"]) <= 16
                and all(type(k) in (str, int) for k in condition["path"]), "missing bounded raw predicate")
    require(not held, "plan leaves input held")


def at(value, path):
    for key in path:
        if isinstance(value, list):
            require(type(key) is int and 0 <= key < len(value), "raw reference outside sequence")
        else:
            require(isinstance(value, dict) and isinstance(key, str) and key in value, "raw field unavailable")
        value = value[key]
    return value


def raw_record(payload, binding, previous=None):
    raw = evidence.json_bytes(payload)
    require(raw.get("scope") == "production-raw-observations-v1"
            and raw.get("nonce") == binding["nonce"] and raw.get("process_id") == binding["pid"]
            and raw.get("failure") is None and raw.get("accepted") is False
            and raw.get("performance_evidence") is False and raw.get("promotion_authority") is False,
            "raw observation failed or belongs to another process")
    samples, events = raw["samples"], raw["events"]
    require(isinstance(samples, list) and 1 <= len(samples) <= 1200
            and isinstance(events, list) and len(events) < 4096, "raw collection cap/coverage failure")
    require(all(evidence.number(s["real_seconds"]) for s in samples)
            and all(a["real_seconds"] < b["real_seconds"] for a, b in zip(samples, samples[1:])),
            "raw sample order differs")
    require(all(type(e["sample_index"]) is int and 0 <= e["sample_index"] < len(samples) for e in events)
            and [e["sample_index"] for e in events] == sorted(e["sample_index"] for e in events),
            "raw event index/order differs")
    if __package__:
        from .building_production_lifecycle import check_raw_transitions
    else:
        from building_production_lifecycle import check_raw_transitions
    check_raw_transitions(raw)
    if previous is not None:
        for key in ("samples", "events"):
            require(equal(raw[key][:len(previous[key])], previous[key]), "raw prefix was rewritten")
    return raw


def normalize(raw, binding, raw_hash):
    # Copy evidence, never fill missing states/outcomes/witnesses/counters.
    raw_record(evidence.pipeline.canonical(raw), binding)
    trace = copy.deepcopy(raw)
    for index, sample in enumerate(trace["samples"]):
        sample["raw_sample_index"] = index
    trace.update({"scope": "bridge-normal-world-observations-only" if binding["kind"] == "Bridge"
                  else "production-normal-world-observations-v1", "leg": binding["leg"],
                  "subject": binding["subject"], "plan_sha256": binding["plan_sha256"],
                  "binary_sha256": binding["binary_sha256"], "driver_sha256": binding["driver_sha256"],
                  "raw_observations_sha256": raw_hash, "fixture_seeded_completion": False})
    evidence.bind_lifecycle_observations(trace, raw)
    return trace


def check_process(binding):
    os.kill(binding["pid"], 0)
    require(evidence.pipeline.digest(Path(f'/proc/{binding["pid"]}/exe').read_bytes()) == binding["binary_sha256"],
            "live executable differs")


def collect(plan_path, binding, admission, steps, raw_path, output):
    """Collect only; output existence forbids retries, including failed attempts.

    The host owns launch, capture/ACK, shutdown and sealing. No leg is marked
    verified here; independent lifecycle verification may reject missing signals.
    """
    plan_bytes = bounded_read(plan_path)
    plan = evidence.json_bytes(plan_bytes)
    evidence.check_plan(plan)
    validate_steps(steps)
    session_binding(plan, evidence.pipeline.digest(plan_bytes), binding, admission, steps)
    require(plan["driver"]["sha256"] == evidence.pipeline.digest(Path(__file__).read_bytes()), "wrong collector driver")
    require(plan["input_transport_sha256"] == evidence.pipeline.digest(Path(__file__).with_name("native_ui_input.py").read_bytes()),
            "input transport changed")
    require(str(raw_path.resolve()) == binding["raw_observations_path"] and raw_path.is_absolute(),
            "raw path differs from host admission")
    output.mkdir(parents=False, exist_ok=False)
    # A different output directory must not replay a previously attempted session.
    write_once(raw_path.with_name(raw_path.name + ".collection-claim"),
               evidence.pipeline.canonical({"session": binding, "output": str(output)}))
    records = []
    raw = None
    raw_payload = None
    transport = None
    failure = None
    started = time.monotonic()
    # Append-only intent/receipt journal survives a failed input or host interruption.
    with (output / "actions.jsonl").open("xb") as journal:
        def record(event):
            entry = {"ordinal": len(records), "at_ns": time.time_ns(), **event}
            journal.write(evidence.pipeline.canonical(entry))
            journal.flush()
            os.fsync(journal.fileno())
            records.append(entry)
        try:
            check_process(binding)
            raw_payload = bounded_read(raw_path)
            raw = raw_record(raw_payload, binding)
            transport = X11Input(binding["window_id"], binding["pid"], binding["root_pid"], binding["nonce"],
                                 lambda event: record({"type": "sent", **event}))
            transport.activate()
            for step in steps:
                require(time.monotonic() - started < 600, "collection deadline exceeded")
                transport._check_owner()
                transport._check_focus()
                raw_payload = bounded_read(raw_path)
                raw = raw_record(raw_payload, binding, raw)
                before = len(raw["samples"]) - 1
                record({"type": "intent", "step": step["id"], "before": before, "input": step["input"]})
                transport.send(step["id"], binding["nonce"], **step["input"])
                deadline = time.monotonic() + step["timeout_seconds"]
                while True:
                    require(time.monotonic() - started < 600, "collection deadline exceeded")
                    transport._check_owner()
                    transport._check_focus()
                    raw_payload = bounded_read(raw_path)
                    raw = raw_record(raw_payload, binding, raw)
                    try:
                        matched = equal(at(raw["samples"][-1], step["until"]["path"]), step["until"]["equals"])
                    except ValueError:
                        matched = False
                    if len(raw["samples"]) - 1 > before and matched:
                        record({"type": "observed", "step": step["id"], "after": len(raw["samples"]) - 1})
                        break
                    require(time.monotonic() < deadline, "ordinary input/raw predicate timeout")
                    time.sleep(.05)
            require(not transport.held_buttons and not transport.held_keys, "input still held")
            verify_action_records({"session": binding, "steps": steps, "records": records,
                                   "failure": None, "promotion_authority": False}, raw)
            check_process(binding)
            evidence.check_plan(plan)
        except BaseException as error:
            failure = str(error)
            raise
        finally:
            try:
                if transport is not None:
                    transport.close()
            except Exception as error:
                failure = failure or f"input cleanup failed: {error}"
                raise
            finally:
                result = {"session": {k: binding[k] for k in SESSION_KEYS}, "steps": steps,
                          "records": records, "failure": failure, "promotion_authority": False}
                write_once(output / "action-log.json", evidence.pipeline.canonical(result))
                if raw is not None:
                    payload = raw_payload
                    write_once(output / "raw.json", payload)
                    if failure is None:
                        trace = normalize(raw, binding, evidence.pipeline.digest(payload))
                        write_once(output / "trace.json", evidence.pipeline.canonical(trace))
    return output


def verify_action_records(log, raw):
    """Validate every input receipt against immutable instructions and raw events."""
    require(log["failure"] is None and log["promotion_authority"] is False, "failed collection")
    steps, records, binding = log["steps"], log["records"], log["session"]
    validate_steps(steps)
    require(len(records) == 3 * len(steps), "incomplete/extra collection records")
    require(all(type(r["ordinal"]) is int and r["ordinal"] == i for i, r in enumerate(records))
            and all(type(r["at_ns"]) is int and r["at_ns"] > 0 for r in records)
            and all(a["at_ns"] <= b["at_ns"] for a, b in zip(records, records[1:])), "action log order differs")
    ranges, used_events, end = [], set(), -1
    for offset, step in enumerate(steps):
        intent, sent, observed = records[3 * offset:3 * offset + 3]
        require([r["type"] for r in (intent, sent, observed)] == ["intent", "sent", "observed"]
                and all(r["step"] == step["id"] for r in (intent, sent, observed))
                and equal(intent["input"], step["input"]), "input intent/receipt differs")
        require(sent["nonce"] == binding["nonce"] and sent["pid"] == binding["pid"]
                and sent["window"] == binding["window_id"], "foreign input receipt")
        require(all(equal(sent.get(k), step["input"].get(k)) for k in ("point", "button", "key", "pressed")),
                "synthetic/different input receipt")
        before, after = intent["before"], observed["after"]
        require(type(before) is int and type(after) is int and 0 <= before < after < len(raw["samples"])
                and before >= end, "remapped/overlapping input range")
        require(equal(at(raw["samples"][after], step["until"]["path"]), step["until"]["equals"]),
                "input predicate has no raw witness")
        action = step["input"]
        if "button" in action or "key" in action:
            operation = ("Pointer" if "button" in action else "Key") + ("Press" if action["pressed"] else "Release")
            candidates = [index for index, event in enumerate(raw["events"])
                          if index not in used_events and before < event["sample_index"] <= after
                          and event.get("operation") == operation
                          and ("key" not in action or event.get("key") == KEY_CODES[action["key"]])]
            require(len(candidates) == 1, "input lacks a unique actual runtime event")
            used_events.add(candidates[0])
        else:
            require(equal(raw["samples"][after].get("cursor_position"), [float(v) for v in action["point"]]),
                    "pointer motion not observed")
        ranges.append((before, after))
        end = after
    return ranges


def verify_log(root, row, plan, raw, trace):
    log = evidence.json_bytes(evidence.artifact(root, row["action_log"]))
    binding = log["session"]
    raw_record(evidence.pipeline.canonical(raw), binding)
    admission = evidence.json_bytes(evidence.artifact(root, row["collection_admission"]))
    session_binding(plan, row["plan_sha256"], binding, admission, log["steps"])
    require(all(equal(binding[k], row[k]) for k in ("subject", "binary_sha256", "codec_sha256", "driver_sha256",
                                                   "plan_sha256", "campaign_nonce", "nonce", "pid", "kind", "leg")),
            "action log belongs to another session")
    seal = evidence.json_bytes(evidence.artifact(root, row["collection_seal"]))
    require(equal(seal, {"authority": "host-sealed-production-collection", "session": binding,
                        "steps_sha256": admission["steps_sha256"],
                        "raw_observations_sha256": row["raw_observations"]["sha256"],
                        "action_log_sha256": row["action_log"]["sha256"],
                        "trace_sha256": row["trace"]["sha256"], "captures": row["captures"]}),
            "host collection seal missing or differs")
    ranges = verify_action_records(log, raw)
    require(all(row["started_at_ns"] <= r["at_ns"] <= row["finished_at_ns"] for r in log["records"]),
            "input outside process interval")
    for shot in row["captures"]:
        require(str(binding["window_id"]) == shot["window_id"]
                and any(before <= shot["sample_index"] <= after for before, after in ranges),
                "capture not bound to observed ordinary input")
    for witness in [*trace.get("witnesses", []), *trace.get("cycles", [])]:
        require(type(witness["before"]) is int and type(witness["after"]) is int
                and 0 <= witness["before"] < witness["after"] < len(raw["samples"])
                and any(witness["before"] <= before < after <= witness["after"] for before, after in ranges),
                "witness/cycle lacks ordinary input and raw range")
        if __package__:
            from .building_production_lifecycle import check_domain_witness
        else:
            from building_production_lifecycle import check_domain_witness
        check_domain_witness(witness, trace)
