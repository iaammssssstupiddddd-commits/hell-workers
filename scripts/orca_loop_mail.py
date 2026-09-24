"""One Run and a fail-closed FIFO mailbox for the host-owned review loop.

Caller holds the loop scheduler lease. Agent text is data, never scheduling
authority. Only the UI coordinator decides answers and escalation dispositions.
"""
from __future__ import annotations

if __package__:
    from . import orca_dispatch as dispatch
else:
    import orca_dispatch as dispatch

storage = dispatch.frontdesk


def cli():
    return dispatch.intake.default_orca_cli()


def ensure_run(data: dict, save) -> dict:
    state = data["run"]
    if state["phase"] == "planned":
        current = dispatch.run_cli(cli(), ["orchestration", "run-current", "--from", data["terminal"]], "run-current")
        if "run" not in current or current["run"] is not None:
            raise ValueError("coordinator already owns a Run; reconcile before switching its inbox")
        state["phase"] = "creating"
        save(data)
        result = dispatch.run_cli(cli(), ["orchestration", "run-create", "--from", data["terminal"],
                                         "--objective", f"Review loop {data['request_id']}"], "run-create")
        run = result.get("run", {})
        if not isinstance(run, dict):
            raise ValueError("shared Run creation returned no Run")
        expected = {"id": run.get("id"), "consumer_generation": run.get("consumer_generation")}
        dispatch.checked_run(cli(), data["terminal"], expected)
        state.update(phase="ready", context=expected)
        save(data)
    elif state["phase"] != "ready":
        raise ValueError("shared Run creation outcome unknown; preserve and reconcile, never recreate")
    dispatch.checked_run(cli(), data["terminal"], state["context"])
    return state["context"]


def message_subject(data: dict, row: dict) -> dict | None:
    """Correlate with a host-confirmed bridge receipt, not only a copied ID."""
    if row.get("run_id") != data["run"]["context"]["id"] or row.get("to_handle") != "run:" + row["run_id"]:
        raise ValueError("message belongs to another Run or recipient")
    question = row.get("type") == "question"
    matches = [item for item in data["attempts"].values()
               if ("dispatch:" + item["dispatch_id"] if question else item["terminal"]) == row.get("from_handle")]
    if not matches:
        raise ValueError("message has no unique owned Dispatch; coordinator reconciliation required")
    confirmed, waiting = [], False
    for attempt in matches:
        result = attempt_message_subject(data, row, attempt)
        if result is True:
            confirmed.append(attempt)
        elif result is None:
            waiting = True
    if len(confirmed) > 1:
        raise ValueError("message has multiple confirmed Dispatch owners")
    if confirmed:
        return confirmed[0]
    if waiting:
        return None
    raise ValueError("message has no confirmed bridge receipt; do not infer lifecycle authority")


def attempt_message_subject(data: dict, row: dict, attempt: dict) -> bool | None:
    """A reused terminal is only a candidate; the immutable receipt owns the message."""
    question = row.get("type") == "question"
    if question:
        payload = dispatch.task_bridge.wire.decode(row.get("payload", "").encode())
        if (payload.get("taskId") != attempt["task_id"] or payload.get("dispatchId") != attempt["dispatch_id"]
                or payload.get("question") != row.get("body") or row.get("thread_id") != row["id"]):
            raise ValueError("question payload/thread differs from owned Dispatch")
    directory = dispatch.task_bridge.root() / dispatch.bindings.identity(attempt["bridge_id"])
    journal = storage.read_private_json(directory / "journal.json", {})
    expected = {"run": attempt["run_id"], "task": attempt["task_id"],
                "dispatch": attempt["dispatch_id"], "coordinator": data["terminal"]}
    if journal.get("authority") != expected:
        raise ValueError("inbox message bridge authority changed")
    operations = journal.get("operations", {})
    if not isinstance(operations, dict) or any(not isinstance(item, dict) for item in operations.values()):
        raise ValueError("invalid bridge journal operations")
    for operation in operations.values():
        if operation.get("phase") != "confirmed":
            continue
        result = operation.get("result", {})
        if not isinstance(result, dict):
            raise ValueError("invalid confirmed bridge receipt")
        message = result.get("message", {})
        if not isinstance(message, dict):
            raise ValueError("invalid confirmed bridge message")
        if message.get("id") == row["id"]:
            # Ignore runtime read flags/timestamps, but not body or routing changes.
            if any(row.get(key) != value for key, value in message.items()):
                raise ValueError("message differs from the confirmed bridge receipt")
            return True
        if row["type"] == "question" and result.get("messageId") == row["id"]:
            return True
    if journal.get("phase") == "active" and any(item.get("phase") == "pending" for item in operations.values()):
        return None  # ask may be in its bounded wait before saving its receipt.
    return False


def accept_batch(data: dict, result: dict, expected_ack: str | None) -> None:
    rows, delivery = result.get("messages"), result.get("deliveryId")
    if (result.get("runId") != data["run"]["context"]["id"]
            or result.get("acknowledged") != expected_ack or result.get("cancelled") is True
            or result.get("connectionLost") is True or not isinstance(rows, list) or len(rows) > 50
            or type(result.get("count")) is not int or result["count"] != len(rows)
            or bool(rows) != bool(delivery)):
        raise ValueError("invalid Run Delivery or unconfirmed acknowledgement")
    inbox = data["inbox"]
    if inbox["delivery"] is not None and expected_ack is None:
        raise ValueError("cannot replace an unacknowledged local Delivery")
    if delivery is not None:
        dispatch.lifecycle_key(delivery, "delivery_")
    seen, messages = set(), {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("invalid message row")
        key = dispatch.lifecycle_key(row.get("id"), "msg_")
        if key in seen or key in inbox["handled"]:
            raise ValueError("duplicate message or already acknowledged message reappeared")
        if (row.get("type") not in {"heartbeat", "worker_done", "question", "escalation"}
                or any(not isinstance(row.get(field, ""), str) or len(row.get(field, "")) > limit
                       for field, limit in (("subject", 500), ("body", 32000), ("payload", 32000)))):
            raise ValueError("unsupported or oversized inbox message; preserve for reconciliation")
        seen.add(key)
        messages[key] = {"row": row, "phase": "received"}
    inbox.update(delivery=delivery, messages=messages, operation=None)


def pending(data: dict) -> list[dict]:
    return [{"id": key, "slot": item["slot"], "type": item["row"]["type"],
             "subject": item["row"].get("subject", ""), "body": item["row"].get("body", ""),
             "payload": item["row"].get("payload", ""), "untrusted": True}
            for key, item in data["inbox"]["messages"].items() if item["phase"] == "decision"]


def poll(data: dict, save) -> bool:
    """Returns whether new work must wait. Settlement/release may still progress."""
    if data["run"]["phase"] == "planned":
        return False
    ensure_run(data, save)
    inbox = data["inbox"]
    if inbox["operation"] is not None:
        raise ValueError("inbox mutation outcome unknown; preserve pending operation without replay")
    for item in inbox["messages"].values():
        if item["phase"] == "processed":
            continue
        attempt = message_subject(data, item["row"])
        if attempt is None:
            continue
        item.update(dispatch_id=attempt["dispatch_id"], slot=attempt["slot"])
        kind = item["row"]["type"]
        if kind == "heartbeat" or kind == "worker_done" and attempt.get("released") is True:
            item["phase"] = "processed"
        elif kind in {"question", "escalation"}:
            item["phase"] = "decision"
    save(data)
    if inbox["delivery"] and any(item["phase"] != "processed" for item in inbox["messages"].values()):
        return any(item["phase"] != "processed" and item["row"]["type"] in {"question", "escalation"}
                   for item in inbox["messages"].values())
    ack = inbox["delivery"]
    if len(inbox["handled"]) + len(inbox["messages"]) > 4096:
        raise ValueError("bounded inbox history exhausted; coordinator decision required")
    inbox["operation"] = {"kind": "check", "ack": ack}
    save(data)
    result = dispatch.run_cli(cli(), ["orchestration", "check", "--terminal", data["terminal"],
        "--run", data["run"]["context"]["id"], *(["--ack", ack] if ack else [])], "loop-check")
    if result.get("acknowledged") != ack:
        raise ValueError("inbox acknowledgement not confirmed")
    for key, item in inbox["messages"].items():
        inbox["handled"][key] = dispatch.bindings.digest(item)
        if item["row"]["type"] == "worker_done":
            data["attempts"][item["dispatch_id"]]["completion_acknowledged"] = True
    accept_batch(data, result, ack)
    save(data)
    # Newly received questions become actionable on the next tick, after host proof.
    return any(item["row"]["type"] in {"question", "escalation"} for item in inbox["messages"].values())


def drained(data: dict) -> bool:
    return (data["inbox"]["delivery"] is None and data["inbox"]["operation"] is None
            and all(item.get("completion_acknowledged") is True for item in data["attempts"].values()))


def decide(data: dict, key: str, body: str, disposition: str, save) -> None:
    if not isinstance(body, str) or not body.strip() or len(body) > 32000 or "\0" in body:
        raise ValueError("decision needs a bounded, nonempty coordinator explanation")
    ensure_run(data, save)
    inbox = data["inbox"]
    item = inbox["messages"].get(key)
    if not item or item["phase"] not in {"decision", "processed"}:
        raise ValueError("message is not awaiting a verified coordinator decision")
    decision = {"body": body, "disposition": disposition}
    if item["phase"] == "processed":
        if item.get("decision") != decision:
            raise ValueError("message already processed with a different decision")
        return
    if inbox["operation"] is not None:
        raise ValueError("previous answer outcome unknown; do not resend")
    attempt = message_subject(data, item["row"])
    if attempt is None:
        raise ValueError("message authority is not confirmed")
    kind = item["row"]["type"]
    if kind == "question":
        if disposition != "reply":
            raise ValueError("questions require an explicit reply")
        inbox["operation"] = {"kind": "reply", "id": key, **decision}
        save(data)
        result = dispatch.run_cli(cli(), ["orchestration", "reply", "--id", key, "--body", body,
            "--run", attempt["run_id"], "--from", data["terminal"]], "loop-reply")
        question, message = result.get("question", {}), result.get("message", {})
        if not isinstance(question, dict) or not isinstance(message, dict):
            raise ValueError("answer receipt is incomplete")
        if (question.get("message_id") != key or question.get("run_id") != attempt["run_id"]
                or question.get("dispatch_id") != attempt["dispatch_id"] or question.get("status") != "answered"
                or question.get("answered_by_generation") != data["run"]["context"]["consumer_generation"]
                or question.get("answer_body") != body or question.get("answer_message_id") != message.get("id")
                or message.get("thread_id") != key or message.get("body") != body
                or message.get("run_id") != attempt["run_id"]
                or message.get("from_handle") != "run:" + attempt["run_id"]
                or message.get("to_handle") != "dispatch:" + attempt["dispatch_id"]):
            raise ValueError("answer receipt does not prove this question was answered")
        dispatch.lifecycle_key(message.get("id"), "msg_")
        item["answer_id"] = message["id"]
    elif kind == "escalation" and disposition in {"continue", "pause"}:
        if disposition == "pause":
            data.update(phase="paused", reason="coordinator escalation decision: " + body)
    else:
        raise ValueError("unsupported coordinator disposition")
    item.update(phase="processed", decision=decision)
    inbox["operation"] = None
    save(data)
