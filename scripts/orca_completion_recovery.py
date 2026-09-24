"""Reconcile a preflight-refused completion from an exited controlled worker.

The exact recorded provider command is data, never executed as shell text. The
same terminal submits it with a durable retry identity after host scope checks.
No pending/ambiguous bridge mutation, replacement Task or new session is allowed.
"""
from __future__ import annotations

import argparse
import copy
import json
import re
import shlex
import uuid
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop


def recorded_command(path: Path, call_id: str, client: Path, authority: dict, terminal: str) -> tuple[list[str], dict]:
    matches = []
    with path.open(encoding="utf-8") as stream:
        for line in stream:
            record = json.loads(line)
            row = record.get("payload", {})
            if record.get("type") == "response_item" and row.get("type") == "custom_tool_call" and row.get("call_id") == call_id:
                matches.append(row)
    if len(matches) != 1:
        raise ValueError("exact recorded provider tool call required")
    raw = matches[0]["input"].split("tools.exec_command(", 1)[1]
    try:
        options, _ = json.JSONDecoder().raw_decode(raw)
        command = options["cmd"]
    except (json.JSONDecodeError, KeyError, TypeError):
        match = re.search(r'\bcmd\s*:\s*("(?:\\.|[^"\\])*")', raw)
        if match is None:
            raise ValueError("recorded completion command is unavailable")
        command = json.loads(match.group(1))
    argv = shlex.split(command)
    if argv[:3] != [str(client), "orchestration", "send"] or argv[-1] != "--json":
        raise ValueError("recorded command is not this bridge completion")
    flags = argv[3:-1]
    if len(flags) % 2:
        raise ValueError("invalid recorded completion flags")
    values = dict(zip(flags[::2], flags[1::2], strict=True))
    required = {"--from", "--dispatch-capability", "--type", "--subject", "--body", "--task-id", "--dispatch-id", "--outcome"}
    if (len(values) * 2 != len(flags) or not required <= set(values)
            or set(values) - required - {"--files-modified"} or values["--from"] != terminal
            or values["--type"] != "worker_done" or values["--outcome"] not in {"succeeded", "failed"}
            or values["--task-id"] != authority["task"] or values["--dispatch-id"] != authority["dispatch"]
            or not values["--dispatch-capability"].startswith("dcap_")):
        raise ValueError("recorded completion identity or scope differs")
    payload = {"taskId": authority["task"], "dispatchId": authority["dispatch"],
               "outcome": values["--outcome"]}
    if "--files-modified" in values:
        payload["filesModified"] = [name.strip() for name in values["--files-modified"].split(",") if name.strip()]
    return argv[1:-1], {"from": terminal, "devMode": False, "type": "worker_done", "subject": values["--subject"],
                       "body": values["--body"], "payload": json.dumps(payload), "waitForLifecycleSettlement": True}


def recover(request: str, call_id: str, source: str, cli: Path, metadata: Path) -> dict:
    b, s, d, roles = loop.bindings, loop.STORAGE, loop.dispatch, loop.roles
    with loop.acquire_host(loop.LOCK, inherit=False), loop.acquire_host("worker-a", inherit=False):
        data = loop.load(request)
        lane = data["lanes"]["worker-a"]
        ticket, attempt = lane["ticket"], lane["attempt"]
        repo = Path(ticket["repo"])
        with loop.acquire_host(roles.workspace_slot(repo), inherit=False):
            roles.validate_ticket(ticket)
            roles.worker_scope(ticket, initial=False)
            if roles.fingerprint(repo) != source or lane["phase"] not in {"paused", "implementing"}:
                raise ValueError("inspected completed source changed")
            record = b.read_state("worker-a", "codex", allow_pending=True)
            last = record["last"]
            directory = d.task_bridge.root() / attempt["bridge_id"]
            receipt_path = directory / "completion-recovery.json"
            receipt = s.read_private_json(receipt_path, {})
            if ((last.get("phase") != "unknown" and record != receipt.get("after_role")) or last.get("process_exited") is not True
                    or type(last.get("exit_code")) is not int or last.get("terminal") != attempt["terminal"]
                    or last.get("orca_bridge") != attempt["bridge_id"]):
                raise ValueError("exact exited worker refusal required")
            bridge_path = directory / "journal.json"
            journal = s.read_private_json(bridge_path, {})
            authority = {"run": attempt["run_id"], "task": attempt["task_id"], "dispatch": attempt["dispatch_id"], "coordinator": data["terminal"]}
            key = last["key"]
            runtime = roles.prepare_runtime("worker-a/tasks/" + key)
            snapshot = b.session_snapshot(runtime, "codex", repo)
            if snapshot["session_id"] != record["tasks"][key]["session_id"]:
                raise ValueError("provider session changed")
            transcript = next((runtime / "codex/sessions").glob("**/*.jsonl"))
            argv, params = recorded_command(transcript, call_id, directory / "p/orca", authority, attempt["terminal"])
            upstream = d.task_bridge.Upstream.load(metadata)
            binding = d.task_bridge.wire.Binding.discover(upstream, attempt["terminal"], repo)
            policy = d.task_bridge.TaskPolicy(upstream, binding, directory, lambda: roles.worker_scope(ticket, initial=False))
            policy.authority = authority
            policy.parameters("orchestration.send", params)
            if not receipt:
                safe_communication = all(
                    isinstance(item, dict) and item.get("phase") == "confirmed"
                    and not item.get("result", {}).get("message")
                    and (item.get("result", {}).get("messages") == []
                         or (item.get("result", {}).get("answer") is None
                             and item.get("result", {}).get("timedOut") is True))
                    for item in journal.get("operations", {}).values()
                )
                if (journal.get("phase") != "unknown" or journal.get("revoked") is not True
                        or journal.get("authority") != authority or s.read_private_json(directory / "arm.json", {}) != authority
                        or not safe_communication):
                    raise ValueError("only a refused completion after confirmed empty communication can be reconciled")
                policy.current()
                receipt = {"phase": "prepared", "source": source, "call_id": call_id, "params": params,
                           "operation": str(uuid.uuid4()), "before": {"bridge": copy.deepcopy(journal), "role": copy.deepcopy(record), "loop": copy.deepcopy(data)}}
                s.write_ledger(receipt_path, receipt)
            if receipt["source"] != source or receipt["call_id"] != call_id or receipt["params"] != params:
                raise ValueError("completion recovery subject changed")
            if "result" not in receipt:
                result = d.run_cli(cli, [*argv, "--retry-request", receipt["operation"]], "reconciled-worker-done")
                receipt["result"] = result
                s.write_ledger(receipt_path, receipt)
            result = policy.project("orchestration.send", params, receipt["result"])
            if roles.fingerprint(repo) != source:
                raise ValueError("source changed during completion reconciliation")
            journal = copy.deepcopy(receipt["before"]["bridge"])
            outcome = json.loads(params["payload"])["outcome"]
            settled = "completed" if outcome == "succeeded" else "failed"
            journal.update(phase="settled", revoked=True, settled_status=settled, recovery_receipt=str(receipt_path))
            journal["operations"][receipt["operation"]] = {"signature": b.digest(params), "phase": "confirmed", "result": result}
            s.write_ledger(bridge_path, journal)
            record["tasks"][key].update(ticket_sha256=b.digest(ticket),
                                        subject=loop.checkpoints.subject(ticket),
                                        source_sha256=source, **snapshot)
            record["last"].update(phase="recorded", completion_recovery=str(receipt_path))
            receipt["after_role"] = copy.deepcopy(record)
            s.write_ledger(receipt_path, receipt)
            b.save_state(record)
            data.update(phase="active", reason=None)
            loop.transition(data, "worker-a", "implementing", reason=None)
            receipt["phase"] = "complete"
            s.write_ledger(receipt_path, receipt)
            return {"recovered": True, "receipt": str(receipt_path), "dispatch": authority["dispatch"],
                    "outcome": settled}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--call-id", required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--orca", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(recover(args.request_id, args.call_id, args.source,
                                 loop.dispatch.intake.checked_orca_cli(args.orca), args.metadata)))
    except (OSError, ValueError, RuntimeError, KeyError, IndexError) as error:
        parser.exit(1, f"Completion recovery refused: {error}\n")
