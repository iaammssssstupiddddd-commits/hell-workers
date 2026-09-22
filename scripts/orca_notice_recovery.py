"""Explicitly acknowledge inspected non-lifecycle notices in a paused Run.

Only a check that sent no ACK can be replayed here. Lifecycle messages cannot
be adopted or discarded. A durable retry identity protects the subsequent ACK.
"""
from __future__ import annotations

import argparse
import copy
import json
import uuid
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop


def notice_hash(row: dict) -> str:
    return loop.bindings.digest({key: row.get(key) for key in
        ("id", "run_id", "from_handle", "to_handle", "type", "subject", "body", "payload", "thread_id")})


def recover(request_id: str, expected: dict, reason: str, cli: Path) -> dict:
    if (not isinstance(expected, dict) or not expected or len(expected) > 50
            or any(not isinstance(key, str) or not key.startswith("msg_")
                   or not isinstance(value, str) or len(value) != 64 for key, value in expected.items())
            or not isinstance(reason, str) or not reason.strip() or len(reason) > 32000):
        raise ValueError("exact inspected notice hashes and disposition reason required")
    b, store = loop.bindings, loop.STORAGE
    with loop.acquire_host(loop.LOCK, inherit=False):
        data = loop.load(request_id)
        identity = b.digest({"request": request_id, "expected": expected, "reason": reason})
        path = store.checked_directory(loop.root() / "notice-recoveries") / f"{identity}.json"
        journal = store.read_private_json(path, {})
        loop.dispatch.checked_run(cli, data["terminal"], data["run"]["context"])
        if not journal:
            if (data["phase"] != "paused" or not str(data["reason"]).startswith("inbox:")
                    or data["inbox"]["operation"] != {"kind": "check", "ack": None}
                    or data["inbox"]["delivery"] is not None or data["inbox"]["messages"]):
                raise ValueError("only an unacknowledged first-check decoder refusal is supported")
            response = loop.dispatch.run_cli(cli, ["orchestration", "check", "--terminal", data["terminal"],
                                                  "--run", data["run"]["context"]["id"]], "notice-replay")
            rows = response.get("messages")
            if (response.get("runId") != data["run"]["context"]["id"] or response.get("acknowledged") is not None
                    or response.get("cancelled") is True or response.get("connectionLost") is True
                    or not isinstance(rows, list) or response.get("count") != len(rows)
                    or len(rows) != len(expected) or any(not isinstance(row, dict) for row in rows)
                    or {row.get("id"): notice_hash(row) for row in rows} != expected
                    or any(row.get("type") != "status" or row.get("run_id") != response["runId"]
                           or row.get("to_handle") != "run:" + response["runId"] for row in rows)):
                raise ValueError("Delivery differs from explicitly inspected non-lifecycle notices")
            delivery = loop.dispatch.lifecycle_key(response.get("deliveryId"), "delivery_")
            journal = {"schema": 1, "before": copy.deepcopy(data), "expected": expected, "reason": reason,
                       "delivery": delivery, "messages": rows, "ack_request": str(uuid.uuid4()), "phase": "prepared"}
            store.write_ledger(path, journal)
        elif journal["expected"] != expected or journal["reason"] != reason:
            raise ValueError("notice recovery journal mismatch")
        if journal["phase"] == "complete" or data == journal.get("after"):
            if data != journal["after"]:
                raise ValueError("loop progressed since recovery; do not replay")
            journal["phase"] = "complete"
            store.write_ledger(path, journal)
            return {"recovered": True, "receipt": str(path)}
        if data != journal["before"]:
            raise ValueError("loop changed during notice recovery")
        if "ack_result" not in journal:
            result = loop.dispatch.run_cli(cli, ["orchestration", "check", "--terminal", data["terminal"],
                "--run", data["run"]["context"]["id"], "--ack", journal["delivery"],
                "--retry-request", journal["ack_request"]], "notice-ack")
            journal["ack_result"] = result
            store.write_ledger(path, journal)
        # Reuse the normal decoder for any subsequent worker messages. A second
        # unsupported batch stays paused and is never silently thrown away.
        loop.mail.accept_batch(data, journal["ack_result"], journal["delivery"])
        for row in journal["messages"]:
            data["inbox"]["handled"][row["id"]] = b.digest({"row": row, "reason": reason, "receipt": str(path)})
        data.update(phase="active", reason=None)
        journal["after"] = data
        store.write_ledger(path, journal)
        loop.save(data)
        journal["phase"] = "complete"
        store.write_ledger(path, journal)
        return {"recovered": True, "receipt": str(path)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--orca", type=Path, required=True)
    args = parser.parse_args()
    spec = loop.STORAGE.read_private_json(args.spec, {})
    try:
        print(json.dumps(recover(args.request_id, spec["expected"], spec["reason"],
                                 loop.dispatch.intake.checked_orca_cli(args.orca))))
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"Notice recovery refused: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
