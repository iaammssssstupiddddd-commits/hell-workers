"""Resume an inspected pre-input retry on its existing controlled terminal.

Requires the preserved failed-input recovery receipt, original Task and same
Run. No provider reset, new terminal, new Task, or source deployment is allowed.
"""
from __future__ import annotations

import argparse
import json
import uuid
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop


def recover(request: str, cli: Path) -> dict:
    b, s, d = loop.bindings, loop.STORAGE, loop.dispatch
    with loop.acquire_host(loop.LOCK, inherit=False):
        data = loop.load(request)
        if len(data["lanes"]) != 1 or data["attempts"]:
            raise ValueError("only the first controlled input retry is supported")
        slot, lane = next(iter(data["lanes"].items()))
        ticket = lane["ticket"]
        path = d.dispatch_path(request, ticket)
        attempt = s.read_private_json(path, {})
        record = b.read_state(slot, loop.roles.provider_for(ticket, slot), allow_pending=True)
        prior = s.read_private_json(Path(record.get("recovery_receipt", "")), {})
        retry = prior.get("after", {}).get("attempt", {}).get("retry", {})
        if (data["phase"] != "paused" or lane["phase"] != "paused" or lane["session"] is not None
                or attempt.get("phase") not in {"unknown", "armed"} or not retry
                or prior.get("phase") != "complete" or prior.get("request_id") != request
                or prior["after"]["loop"]["lanes"][slot]["ticket"] != ticket
                or attempt.get("ticket_sha256") != b.digest(ticket)
                or attempt.get("shared_run") != data["run"]["context"]
                or record["last"].get("terminal") != attempt.get("terminal")
                or record["last"].get("orca_bridge") != attempt.get("bridge_id")
                or record["last"].get("phase") != "starting"
                or loop.roles.fingerprint(Path(ticket["repo"])) != lane["source"]):
            raise ValueError("current paused source/launcher does not match the inspected input retry")
        d.checked_run(cli, data["terminal"], data["run"]["context"])
        receipt_path = path.with_suffix(".input-recovery.json")
        receipt = s.read_private_json(receipt_path, {})
        if not receipt:
            latest = d.run_cli(cli, ["orchestration", "dispatch-show", "--task", retry["task"]], "input-latest")["dispatch"]
            if (latest.get("id") != retry["dispatch"] or latest.get("status") != "failed"
                    or latest.get("run_id") != data["run"]["context"]["id"]):
                raise ValueError("original failed Dispatch is no longer latest; inspect before retry")
            receipt = {"before": attempt, "retry": retry, "request": str(uuid.uuid4()), "phase": "prepared"}
            s.write_ledger(receipt_path, receipt)
        if receipt["retry"] != retry or receipt["before"]["terminal"] != attempt["terminal"]:
            raise ValueError("input recovery identity changed")
        if "result" not in receipt:
            result = d.run_cli(cli, ["orchestration", "worker-start", "--run", data["run"]["context"]["id"],
                "--task", retry["task"], "--retry-of", retry["dispatch"], "--worktree", "path:" + ticket["repo"],
                "--terminal", attempt["terminal"], "--from", data["terminal"], "--timeout-ms", "60000",
                "--retry-request", receipt["request"]], "input-retry")
            receipt["result"] = result
            s.write_ledger(receipt_path, receipt)
        result = receipt["result"]
        if (result.get("state") != "ready" or result.get("stage") != "input_accepted"
                or result.get("runId") != data["run"]["context"]["id"] or result.get("taskId") != retry["task"]):
            raise ValueError("input retry did not reach exact ready Task")
        dispatch_id = d.lifecycle_key(result.get("dispatchId"), "ctx_")
        d.task_bridge.arm(attempt["bridge_id"], {"run": result["runId"], "task": result["taskId"],
                                               "dispatch": dispatch_id, "coordinator": data["terminal"]})
        attempt.update(phase="armed", task_id=result["taskId"], dispatch_id=dispatch_id)
        d.save(path, attempt)
        attempt = {**attempt, "coordinator": data["terminal"]}
        data["attempts"][dispatch_id] = {**attempt, "slot": slot, "role": slot, "released": False}
        data.update(phase="active", reason=None)
        loop.transition(data, slot, "implementing", attempt=attempt, reason=None)
        receipt["phase"] = "complete"
        s.write_ledger(receipt_path, receipt)
        return {"recovered": True, "dispatch": dispatch_id, "receipt": str(receipt_path)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--orca", type=Path, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(recover(args.request_id, loop.dispatch.intake.checked_orca_cli(args.orca))))
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"Input recovery refused: {error}\n")
