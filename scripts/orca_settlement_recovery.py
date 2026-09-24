"""Reconcile an exited read-only reviewer after an accepted Orca settlement.

No message, Task, Dispatch or provider is created. Preserve the raw process exit
and before-state; normalize only the host result after exact read-back checks.
The existing coordinator remains the sole owner of release, verdict sealing and
inbox ACK. Frozen review subjects are never changed by this maintenance path.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import signal
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop

roles, bindings, storage = loop.roles, loop.bindings, loop.STORAGE


def recover_role(ticket: dict, expected: str, metadata: Path) -> dict:
    repo = Path(ticket["repo"])
    with loop.acquire_host("reviewer", inherit=False), loop.acquire_host(roles.workspace_slot(repo), inherit=False):
        roles.validate_ticket(ticket)
        if ticket.get("read_only") is not True or ticket.get("allowed_directories") != []:
            raise ValueError("only an unchanged read-only review can be reconciled")
        role = bindings.read_state("reviewer", "codex", allow_pending=True)
        last = role["last"]
        directory = roles.task_bridge.root() / bindings.identity(last["orca_bridge"])
        receipt_path = directory / "settlement-recovery.json"
        receipt = storage.read_private_json(receipt_path, {})
        before = receipt.get("before", role)
        if (bindings.digest(before) != expected or role not in (before, receipt.get("after"))
                or receipt and receipt.get("ticket_sha256") != bindings.digest(ticket)):
            raise ValueError("exact inspected reviewer state required")
        old = before["last"]
        if (old.get("phase") != "unknown" or old.get("process_exited") is not True
                or old.get("exit_code") not in (0, -signal.SIGTERM)
                or old.get("key") != "fixed-reviewer"
                or roles.fingerprint(repo) != old.get("source_before")
                or ticket.get("source_sha256") != old.get("source_before")):
            raise ValueError("exact exited reviewer and unchanged source required")
        settled = roles.settled_bridge(old, repo)
        if settled["outcome"] != "completed":
            raise ValueError("accepted successful settlement required")
        identity = storage.read_private_json(directory / "identity.json", {})
        authority = settled["authority"]
        upstream = roles.task_bridge.Upstream.load(metadata)
        if identity.get("runtime") != upstream.runtime_id:
            raise ValueError("Orca runtime changed")
        lease_key = identity["runtime"] + identity["terminal"] + identity["incarnation"]
        with loop.acquire_host("workspace-" + hashlib.sha256(lease_key.encode()).hexdigest(), inherit=False):
            observed = upstream.call("orchestration.workerShow", {"dispatch": authority["dispatch"]})
            dispatch, worker = observed["dispatch"], observed["worker"]
            expected_dispatch = {"id": authority["dispatch"], "taskId": authority["task"],
                                 "runId": authority["run"], "assigneeHandle": old["terminal"], "status": "completed"}
            if (any(dispatch.get(key) != value for key, value in expected_dispatch.items())
                    or dispatch.get("capabilityRevokedAt") is None
                    or worker.get("state") != "succeeded" or worker.get("runtimeEpoch") != upstream.runtime_id
                    or worker.get("dispatchId") != authority["dispatch"]
                    or observed.get("observation", {}).get("exactWorker") is not True
                    or not roles.task_bridge.exact_process(observed, terminal=old["terminal"],
                         incarnation=identity["incarnation"], worktree=worker.get("worktreeId"),
                         dispatch_id=authority["dispatch"])):
                raise ValueError("accepted Dispatch read-back does not match the exact attempt")
            latest = upstream.call("orchestration.dispatchShow", {"task": authority["task"]})["dispatch"]
            if (latest.get("id") != authority["dispatch"] or latest.get("status") != "completed"
                    or latest.get("task_id") != authority["task"]):
                raise ValueError("another Dispatch superseded this settlement")
            bound = before["tasks"]["fixed-reviewer"]
            snapshot = bindings.session_snapshot(roles.prepare_runtime("reviewer"), "codex", Path(bound["origin"]))
            if snapshot["session_id"] != bound["session_id"]:
                raise ValueError("fixed reviewer session changed")
            # Parsing is not approval; the normal driver must still seal/verify it.
            verdict = loop.parse_review(settled["message"]["body"], snapshot["session_id"])
            expected_verdict = {"ticket": ticket["id"], "base": ticket["review_base"], "head": ticket["base"],
                                "source_sha256": ticket["source_sha256"],
                                "validation_evidence": ticket["validation_evidence"]}
            if any(verdict.get(key) != value for key, value in expected_verdict.items()):
                raise ValueError("settled review belongs to another subject")
            after = copy.deepcopy(before)
            after["tasks"]["fixed-reviewer"] = {**bound, **snapshot, "ticket_sha256": bindings.digest(ticket),
                "subject": loop.checkpoints.subject(ticket), "source_sha256": ticket["source_sha256"]}
            after["last"].update(phase="recorded", provider_exit_code=old["exit_code"], exit_code=0,
                                 settlement_recovery=str(receipt_path))
            if receipt and receipt.get("after") != after:
                raise ValueError("provider history changed since reconciliation")
            if not receipt:
                receipt = {"schema": 1, "phase": "prepared", "before": before, "after": after,
                           "ticket_sha256": bindings.digest(ticket), "accepted_settlement": settled,
                           "read_back": expected_dispatch, "session": snapshot}
                storage.write_ledger(receipt_path, receipt)
            bindings.save_state(after)
            storage.write_ledger(receipt_path, {**receipt, "phase": "complete"})
            return {"receipt": str(receipt_path), "session": snapshot["session_id"],
                    "provider_exit_code": old["exit_code"], "outcome": "completed"}


def recover(request: str, expected_loop: str, expected_role: str, metadata: Path) -> dict:
    with loop.acquire_host(loop.LOCK, inherit=False):
        data = loop.load(request)
        if bindings.digest(data) != expected_loop or data["phase"] != "paused":
            raise ValueError("exact paused loop required")
        final = data.get("integration", {})
        if final.get("phase") != "reviewing":
            raise ValueError("only an existing final review can be resumed")
        registered = loop.dispatch.ui_coordinator.read_registered_state(request)
        if registered["phase"] != "ready" or registered["terminal"] != data["terminal"]:
            raise ValueError("original ready coordinator required")
        for lane in data["lanes"].values():
            if lane["phase"] == "approved":
                continue
            if (lane["phase"] != "paused" or not lane["history"]
                    or lane["history"][-1].get("from") != "approved"
                    or lane.get("reason") != "approval invalidated: unknown role attempt; reconcile before any new launch"):
                raise ValueError("unrelated paused lane cannot be resumed")
        # Old immutable approvals remain independently verifiable even while the
        # final review's process observation needs reconciliation.
        for lane in data["lanes"].values():
            roles.verify_review(lane["review_ticket"], lane["review"])
        loop.integration.check_receipt(final["receipt"])
        role = bindings.read_state("reviewer", "codex", allow_pending=True)
        if (role["last"].get("orca_bridge") != final["attempt"]["bridge_id"]
                or role["last"].get("terminal") != final["attempt"]["terminal"]):
            raise ValueError("reviewer belongs to another attempt")
        directory = storage.checked_directory(loop.root() / "settlement-recoveries")
        receipt_path = directory / f"{expected_loop}.json"
        receipt = storage.read_private_json(receipt_path, {})
        if receipt and receipt.get("before") != data:
            raise ValueError("recovery source ledger changed")
        if not receipt:
            storage.write_ledger(receipt_path, {"before": data, "expected_role": expected_role})
        result = recover_role(final["review_ticket"], expected_role, metadata)
        completion = loop.completion(final["review_ticket"], "reviewer", final["attempt"])
        if completion is None or completion["outcome"] != "completed":
            raise ValueError("reconciled completion not accepted by normal driver")
        for lane in data["lanes"].values():
            if lane["phase"] == "paused":
                lane["history"].append({"from": "paused", "to": "approved", "recovery": str(receipt_path)})
                lane.update(phase="approved", reason=None)
        data.update(phase="active", reason=None)
        # Save intent before enabling the existing host driver; no extra consumer.
        storage.write_ledger(receipt_path, {"before": receipt.get("before", loop.load(request)),
                                           "after": data, "role_recovery": result})
        loop.save(data)
        return {"phase": "active", "integration": "reviewing", **result}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--expected-loop", required=True)
    parser.add_argument("--expected-role", required=True)
    parser.add_argument("--metadata", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(recover(args.request_id, args.expected_loop, args.expected_role, args.metadata)))


if __name__ == "__main__":
    main()
