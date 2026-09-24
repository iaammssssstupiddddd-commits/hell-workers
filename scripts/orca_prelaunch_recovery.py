"""Explicit host recovery for a refused launcher before any implementation input.

This is not recovery of a running/unknown Orca Dispatch. It requires a stopped
coordinator, a positively closed refused terminal, an empty Run, unchanged source,
and the exact identity-less exited role barrier. Alternatively, a positively
failed input Dispatch with a closed unarmed bridge and no provider history can
retry its original Task. Preserve all prior state before
re-enabling the same request, Run and workspaces. Never reset provider histories.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
import re
from contextlib import ExitStack
from pathlib import Path

if __package__:
    from . import orca_review_loop as loop
else:
    import orca_review_loop as loop

B = loop.bindings
S = loop.STORAGE


def empty_role(slot: str, provider: str, recovery: str) -> dict:
    return {"schema": 1, "slot": slot, "provider": provider, "tasks": {}, "last": None,
            "recovery_receipt": recovery}


def verify_barrier(data: dict, slot: str, provider: str) -> None:
    if (not isinstance(data, dict) or set(data) != {"schema", "slot", "provider", "tasks", "last"}
            or data["schema"] != 1 or data["slot"] != slot or data["provider"] != provider
            or data["tasks"] != {} or not isinstance(data["last"], dict)
            or set(data["last"]) != {"phase", "process_exited", "exit_code"}
            or data["last"]["phase"] != "unknown" or data["last"]["process_exited"] is not True
            or type(data["last"]["exit_code"]) is not int or data["last"]["exit_code"] == 0):
        raise ValueError("only an identity-less exited empty role barrier can be quarantined")


def verify_input_failure(cli: Path, data: dict, attempt: dict, dispatch_id: str, bootstrap: bool = False) -> dict:
    """Only a revoked, positively failed input attempt can retain its Task for retry."""
    loop.dispatch.lifecycle_key(dispatch_id, "ctx_")
    result = loop.dispatch.run_cli(cli, ["orchestration", "worker-show", "--dispatch", dispatch_id], "recovery-failed-input")
    row, worker = result.get("dispatch", {}), result.get("worker", {})
    if (row.get("id") != dispatch_id or row.get("runId") != data["run"]["context"]["id"]
            or row.get("assigneeHandle") != attempt["terminal"] or row.get("status") != "failed"
            or (not bootstrap and row.get("lastFailure") != "agent_prompt_blocked") or not row.get("capabilityRevokedAt")
            or worker.get("dispatchId") != dispatch_id or worker.get("state") not in ({"abandoned", "stopped"} if bootstrap else {"failed"})
            or (not bootstrap and worker.get("stage") != "dispatch_input") or worker.get("agentTerminalHandle") != attempt["terminal"]
            or result.get("observation", {}).get("exactWorker") is not True):
        raise ValueError("exact revoked failed input Dispatch is required")
    loop.dispatch.lifecycle_key(row.get("taskId"), "task_")
    return result


def verify_unarmed_role(role: dict, attempt: dict, ticket: dict, bootstrap: bool = False) -> dict:
    last = role.get("last", {})
    if (role.get("tasks") != {} or last.get("phase") != "unknown" or last.get("process_exited") is not True
            or type(last.get("exit_code")) is not int or last.get("terminal") != attempt["terminal"]
            or last.get("orca_bridge") != attempt["bridge_id"]
            or last.get("source_before") != loop.roles.fingerprint(Path(ticket["repo"]))):
        raise ValueError("only an exited source-unchanged first input attempt is recoverable")
    B.identity(last.get("attempt_id"))
    directory = loop.dispatch.task_bridge.root() / B.identity(last.get("orca_bridge"))
    bridge = S.read_private_json(directory / "journal.json", {})
    identity = S.read_private_json(directory / "identity.json", {})
    if (bridge.get("phase") != ("unknown" if bootstrap else "closed") or bridge.get("revoked") is not True
            or bridge.get("authority") is not None or bridge.get("operations") != {}
            or (directory / "arm.json").exists() or (directory / "arm.json").is_symlink()
            or identity.get("terminal") != attempt["terminal"] or identity.get("repo") != ticket["repo"]
            or type(identity.get("pid")) is not int or identity["pid"] <= 0):
        raise ValueError("input attempt bridge must be closed, unarmed and mutation-free")
    try:
        os.kill(identity["pid"], 0)
    except ProcessLookupError:
        pass
    else:
        raise ValueError("old launcher remains alive")
    for slot in (role["slot"], *([] if bootstrap else [f'{role["slot"]}/tasks/{last["key"]}'])):
        if B.history_exists(loop.roles.prepare_runtime(slot)):
            raise ValueError("provider history exists; preserve its session")
    result = {"bridge": bridge, "identity": identity}
    if bootstrap:
        result["session"] = B.session_snapshot(loop.roles.prepare_runtime(f'{role["slot"]}/tasks/{last["key"]}'),
                                               role["provider"], Path(ticket["repo"]))
    return result


def verify_external(cli: Path, data: dict, attempt: dict, closed: dict, failed: str | None = None, bootstrap: bool = False) -> None:
    """Close receipt is explicit positive host evidence, not missing-worker inference."""
    if not failed and (closed.get("ok") is not True
            or closed.get("result", {}).get("close", {}).get("handle") != attempt["terminal"]
            or closed.get("result", {}).get("close", {}).get("ptyKilled") is not True):
        raise ValueError("exact refused terminal needs a positive close receipt")
    status = loop.dispatch.run_cli(cli, ["status"], "recovery-status")
    # Runtime identity is also returned in the top-level response metadata; pin it
    # via run/terminal identities below rather than trusting a stale process PID.
    if not status:
        raise ValueError("Orca runtime status is unavailable")
    run = data["run"]["context"]
    loop.dispatch.checked_run(cli, data["terminal"], run)
    fleet = loop.dispatch.run_cli(cli, ["orchestration", "worker-list", "--run", run["id"]], "recovery-workers")
    if failed:
        observed = verify_input_failure(cli, data, attempt, failed, bootstrap)
        if bootstrap:
            workers = fleet.get("workers", [])
            if (fleet.get("scope", {}).get("run") != run["id"] or fleet.get("page", {}).get("hasMore") is not False
                    or fleet.get("page", {}).get("total") != len(workers)
                    or failed not in [item.get("dispatchId") for item in workers]
                    or any(item.get("taskId") != observed["dispatch"]["taskId"]
                           or item.get("workerState") not in {"failed", "stopped", "abandoned"} for item in workers)):
                raise ValueError("bootstrap recovery requires only settled attempts of its original Task")
            return
        if (fleet.get("scope", {}).get("run") != run["id"] or fleet.get("page", {}).get("hasMore") is not False
                or fleet.get("page", {}).get("total") != 1
                or [item.get("dispatchId") for item in fleet.get("workers", [])] != [failed]):
            raise ValueError("failed input recovery requires exactly its single Dispatch")
        return
    if (fleet.get("scope", {}).get("run") != run["id"] or fleet.get("workers") != []
            or fleet.get("page", {}).get("total") != 0 or fleet.get("page", {}).get("hasMore") is not False):
        raise ValueError("prelaunch recovery requires a positively enumerated empty Run")
    terminals = loop.dispatch.run_cli(cli, ["terminal", "list"], "recovery-terminals")
    if (terminals.get("truncated") is not False or not isinstance(terminals.get("terminals"), list)
            or any(item.get("handle") == attempt["terminal"] for item in terminals["terminals"])):
        raise ValueError("closed refused terminal still exists or inventory is incomplete")


def recover(request_id: str, spec: dict, cli: Path) -> dict:
    required = {"slot", "loop_sha256", "role_sha256", "new_base", "reason", "terminal_close"}
    if (not isinstance(spec, dict) or set(spec) not in (required, required | {"failed_dispatch"}, required | {"failed_dispatch", "bootstrap_session"}) or spec["slot"] not in {"worker-a", "worker-b"}
            or ("bootstrap_session" in spec and spec["bootstrap_session"] is not True)
            or not isinstance(spec["reason"], str) or not spec["reason"].strip()
            or not isinstance(spec["new_base"], str) or not re.fullmatch(r"[a-f0-9]{40}", spec["new_base"])):
        raise ValueError("explicit recovery specification required")
    identity = B.digest({"request": request_id, "spec": spec})
    receipt_path = S.checked_directory(loop.root() / "recoveries") / f"{identity}.json"
    with ExitStack() as leases:
        for name in ("ui-coordinator", "workspace-" + B.digest({"driver": request_id}), loop.LOCK, spec["slot"]):
            leases.enter_context(loop.acquire_host(name, inherit=False))
        saved = S.read_private_json(receipt_path, {})
        if saved:
            if saved.get("spec") != spec or saved.get("request_id") != request_id:
                raise ValueError("recovery receipt identity mismatch")
            journal = saved
        else:
            data = loop.load(request_id)
            if B.digest(data) != spec["loop_sha256"] or data["schema"] != 2:
                raise ValueError("loop changed; inspect before recovery")
            lane = data["lanes"][spec["slot"]]
            if (len(data["lanes"]) != 1 or lane["phase"] not in {"dispatching", "paused"}
                    or lane["session"] is not None or lane["revisions"] != 0 or data["attempts"]
                    or lane["ticket"].get("generation", 0) != 0
                    or data["inbox"]["delivery"] is not None or data["inbox"]["operation"] is not None
                    or data.get("integration", {}).get("phase") != "planned"):
                raise ValueError("only untouched first prelaunch attempts can be recovered")
            ticket = lane["ticket"]
            attempt_path = loop.dispatch.dispatch_path(request_id, ticket)
            attempt = S.read_private_json(attempt_path, {})
            if (attempt.get("phase") != "unknown" or attempt.get("slot") != spec["slot"]
                    or attempt.get("ticket_sha256") != B.digest(ticket)
                    or attempt.get("shared_run") != data["run"]["context"]
                    or any(attempt.get(key) is not None for key in ("task_id", "dispatch_id"))
                    or (not spec.get("failed_dispatch") and attempt.get("bridge_id") is not None)):
                raise ValueError("attempt may have reached dispatch; use Dispatch recovery instead")
            verify_external(cli, data, attempt, spec["terminal_close"], spec.get("failed_dispatch"), spec.get("bootstrap_session", False))
            role_path = B.state_path(spec["slot"])
            role = S.read_private_json(role_path, {})
            if B.digest(role) != spec["role_sha256"]:
                raise ValueError("role changed; inspect before recovery")
            provider = loop.roles.provider_for(ticket, spec["slot"])
            failure = None
            if spec.get("failed_dispatch"):
                proof = verify_unarmed_role(role, attempt, ticket, spec.get("bootstrap_session", False))
                failure = verify_input_failure(cli, data, attempt, spec["failed_dispatch"], spec.get("bootstrap_session", False))
                if spec.get("bootstrap_session"):
                    input_receipt = S.read_private_json(attempt_path.with_suffix(".input-recovery.json"), {})
                    result = input_receipt.get("result", {})
                    if (input_receipt.get("before") != attempt or result.get("state") != "ready"
                            or result.get("stage") != "input_accepted" or result.get("dispatchId") != spec["failed_dispatch"]
                            or result.get("taskId") != failure["dispatch"]["taskId"] or result.get("runId") != data["run"]["context"]["id"]):
                        raise ValueError("bootstrap session needs its exact accepted input receipt")
                    proof["input_receipt"] = input_receipt
            else:
                verify_barrier(role, spec["slot"], provider)
            updated = copy.deepcopy(data)
            target = updated["integration"]["target"]
            old_base = ticket["base"]
            workspaces = [ticket["repo"], target["repo"]]
            for value in workspaces:
                repo = Path(value)
                if (loop.roles.git(repo, "status", "--porcelain")
                        or loop.roles.git(repo, "rev-parse", "HEAD") != old_base
                        or loop.roles.fingerprint(repo) != (lane["source"] if value == ticket["repo"] else data["integration"]["source"])):
                    raise ValueError("prelaunch workspace changed")
            repo = Path(ticket["repo"])
            new_base = loop.roles.git(repo, "rev-parse", "--verify", f"{spec['new_base']}^{{commit}}")
            if new_base != spec["new_base"] or loop.roles.git(repo, "merge-base", old_base, new_base) != old_base:
                raise ValueError("recovery deployment must be a full-SHA fast-forward")
            names = loop.roles.git(repo, "diff", "--name-only", old_base, new_base).splitlines()
            if not names or any(not name.startswith("scripts/") for name in names):
                raise ValueError("prelaunch recovery deployment is restricted to tooling changes")
            updated["lanes"][spec["slot"]]["ticket"]["base"] = new_base
            updated["lanes"][spec["slot"]]["phase"] = "planned"
            updated["lanes"][spec["slot"]]["history"].append({"from": lane["phase"], "to": "planned", "recovery": identity})
            target["base"] = new_base
            updated.update(phase="active", reason=None)
            updated["spec_sha256"] = B.digest({"lanes": [{"slot": spec["slot"], "ticket": updated["lanes"][spec["slot"]]["ticket"], "validation": lane["validation"]}],
                                                 "integration": {"target": target, "validation": updated["integration"]["validation"]}})
            replacement = {"phase": "retry_ready", "recovery": str(receipt_path)}
            journal = {"schema": 1, "request_id": request_id, "spec": spec, "phase": "prepared",
                       "before": {"loop": data, "role": role, "attempt": attempt},
                       "after": {"loop": updated, "role": empty_role(spec["slot"], provider, str(receipt_path)), "attempt": replacement},
                       "paths": {"attempt": str(attempt_path), "role": str(role_path)}, "workspaces": workspaces}
            if failure:
                journal["failed_input"] = {"observed": failure, **proof}
                replacement["retry"] = {"task": failure["dispatch"]["taskId"], "dispatch": spec["failed_dispatch"]}
                key = role["last"]["key"]
                leases.enter_context(loop.acquire_host("workspace-" + key, inherit=False))
                assignment = role_path.parent / "assignments" / f"{key}.json"
                original = S.read_private_json(assignment, {})
                common = loop.roles.git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")
                subject = {"repo": ticket["repo"], "common": common, "branch": ticket["branch"], "base": old_base}
                if original != {"schema": 1, "slot": spec["slot"], "provider": provider,
                                "ticket_sha256": B.digest(ticket), "subject": subject}:
                    raise ValueError("original failed assignment differs")
                journal["paths"]["assignment"] = str(assignment)
                journal["before"]["assignment"] = original
                journal["after"]["assignment"] = {**original, "ticket_sha256": B.digest(updated["lanes"][spec["slot"]]["ticket"]),
                                                   "subject": {**subject, "base": new_base}}
                if spec.get("bootstrap_session"):
                    snapshot = proof["session"]
                    repaired = copy.deepcopy(role)
                    repaired["tasks"][key] = {"key": key, "ticket_sha256": journal["after"]["assignment"]["ticket_sha256"],
                        "subject": journal["after"]["assignment"]["subject"], "origin": ticket["repo"],
                        "source_sha256": lane["source"], **snapshot}
                    repaired["last"].update(phase="recorded")
                    repaired["recovery_receipt"] = str(receipt_path)
                    journal["after"]["role"] = repaired
                    updated["lanes"][spec["slot"]].update(session=snapshot["session_id"],
                        follow_up="Continue the same assigned regression after the reconciled unarmed bridge failure. No source was changed. Wait for the new live Orca preamble before editing, and report completion through its bridge.")
            S.write_ledger(receipt_path, journal)
        prior = journal["before"]["loop"]
        if saved and "assignment" in journal["paths"]:
            leases.enter_context(loop.acquire_host("workspace-" + journal["before"]["role"]["last"]["key"], inherit=False))
        ticket = prior["lanes"][spec["slot"]]["ticket"]
        leases.enter_context(loop.acquire_host("dispatch-" + B.digest({"request": request_id, "ticket": ticket["id"]}), inherit=False))
        verify_external(cli, prior, journal["before"]["attempt"], spec["terminal_close"], spec.get("failed_dispatch"), spec.get("bootstrap_session", False))
        for value in journal["workspaces"]:
            leases.enter_context(loop.acquire_host(loop.roles.workspace_slot(Path(value)), inherit=False))
        # Prepared journal makes interrupted deployment/replacement replayable, with
        # exact before/after checks. No completed worker source can enter this path.
        for value in journal["workspaces"]:
            repo = Path(value)
            if loop.roles.git(repo, "status", "--porcelain"):
                raise ValueError("workspace became dirty during recovery")
            head = loop.roles.git(repo, "rev-parse", "HEAD")
            old = journal["before"]["loop"]["lanes"][spec["slot"]]["ticket"]["base"]
            if head not in {old, spec["new_base"]}:
                raise ValueError("workspace HEAD changed during recovery")
            if head == old:
                expected = (journal["before"]["loop"]["lanes"][spec["slot"]]["source"]
                            if value == journal["workspaces"][0] else journal["before"]["loop"]["integration"]["source"])
                if loop.roles.fingerprint(repo) != expected:
                    raise ValueError("workspace source changed during recovery")
                loop.roles.git(repo, "merge", "--ff-only", spec["new_base"])
        updated = journal["after"]["loop"]
        updated["lanes"][spec["slot"]]["source"] = loop.roles.fingerprint(Path(journal["workspaces"][0]))
        updated["integration"]["source"] = loop.roles.fingerprint(Path(journal["workspaces"][1]))
        if spec.get("bootstrap_session"):
            journal["after"]["role"]["tasks"][journal["before"]["role"]["last"]["key"]]["source_sha256"] = updated["lanes"][spec["slot"]]["source"]
        S.write_ledger(receipt_path, journal)
        for key in ("role", "attempt", *(["assignment"] if "assignment" in journal["paths"] else [])):
            path = Path(journal["paths"][key])
            current = S.read_private_json(path, {})
            if current not in (journal["before"][key], journal["after"][key]):
                raise ValueError(f"{key} changed during recovery")
            S.write_ledger(path, journal["after"][key])
        current = loop.load(request_id)
        if current not in (journal["before"]["loop"], updated):
            raise ValueError("loop changed during recovery")
        loop.save(updated)
        journal["phase"] = "complete"
        S.write_ledger(receipt_path, journal)
        return {"recovered": True, "receipt": str(receipt_path), "run": updated["run"]["context"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--orca", type=Path, required=True)
    args = parser.parse_args()
    try:
        cli = loop.dispatch.intake.checked_orca_cli(args.orca)
        print(json.dumps(recover(args.request_id, S.read_private_json(args.spec, {}), cli)))
    except (OSError, ValueError, RuntimeError) as error:
        parser.exit(1, f"Recovery refused: {error}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
