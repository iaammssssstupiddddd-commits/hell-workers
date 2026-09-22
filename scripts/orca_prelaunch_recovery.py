"""Explicit host recovery for a refused launcher before any Task was dispatched.

This is not recovery of a running/unknown Orca Dispatch. It requires a stopped
coordinator, a positively closed refused terminal, an empty Run, unchanged source,
and the exact identity-less exited role barrier. Preserve all prior state before
re-enabling the same request, Run and workspaces. Never reset provider histories.
"""
from __future__ import annotations

import argparse
import copy
import json
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


def verify_external(cli: Path, data: dict, attempt: dict, closed: dict) -> None:
    """Close receipt is explicit positive host evidence, not missing-worker inference."""
    if (closed.get("ok") is not True
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
    if (fleet.get("scope", {}).get("run") != run["id"] or fleet.get("workers") != []
            or fleet.get("page", {}).get("total") != 0 or fleet.get("page", {}).get("hasMore") is not False):
        raise ValueError("prelaunch recovery requires a positively enumerated empty Run")
    terminals = loop.dispatch.run_cli(cli, ["terminal", "list"], "recovery-terminals")
    if (terminals.get("truncated") is not False or not isinstance(terminals.get("terminals"), list)
            or any(item.get("handle") == attempt["terminal"] for item in terminals["terminals"])):
        raise ValueError("closed refused terminal still exists or inventory is incomplete")


def recover(request_id: str, spec: dict, cli: Path) -> dict:
    required = {"slot", "loop_sha256", "role_sha256", "new_base", "reason", "terminal_close"}
    if (not isinstance(spec, dict) or set(spec) != required or spec["slot"] not in {"worker-a", "worker-b"}
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
                    or any(attempt.get(key) is not None for key in ("bridge_id", "task_id", "dispatch_id"))):
                raise ValueError("attempt may have reached dispatch; use Dispatch recovery instead")
            verify_external(cli, data, attempt, spec["terminal_close"])
            role_path = B.state_path(spec["slot"])
            role = S.read_private_json(role_path, {})
            if B.digest(role) != spec["role_sha256"]:
                raise ValueError("role changed; inspect before recovery")
            provider = loop.roles.provider_for(ticket, spec["slot"])
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
            S.write_ledger(receipt_path, journal)
        prior = journal["before"]["loop"]
        ticket = prior["lanes"][spec["slot"]]["ticket"]
        leases.enter_context(loop.acquire_host("dispatch-" + B.digest({"request": request_id, "ticket": ticket["id"]}), inherit=False))
        verify_external(cli, prior, journal["before"]["attempt"], spec["terminal_close"])
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
        S.write_ledger(receipt_path, journal)
        for key in ("role", "attempt"):
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
