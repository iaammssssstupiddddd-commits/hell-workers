"""Host-only checkpoint commits and durable authorization for same-session resume.

Never relax the launcher's HEAD, ownership or dirty-source guards. A private
receipt explains precisely the one controller-owned transition they may accept.
The caller owns validation selection; this module records its execution against
the exact source. It does not dispatch agents, push or claim review approval.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from contextlib import ExitStack
from pathlib import Path

if __package__:
    from . import orca_role_state as bindings, orca_roles as roles
    from .host_coordination import HOST_FD_ENV, acquire_host, host_pass_fds
else:
    import orca_role_state as bindings
    import orca_roles as roles
    from host_coordination import HOST_FD_ENV, acquire_host, host_pass_fds


def root() -> Path:
    return bindings.storage.checked_directory(bindings.state_path("worker-a").parent / "checkpoints")


def git(repo: Path, *argv: str, env: dict | None = None, body: str | None = None) -> str:
    result = subprocess.run(["git", "--literal-pathspecs", "-C", str(repo), *argv], input=body, env=env,
                            capture_output=True, text=True, check=True, timeout=60)
    return result.stdout.strip()


def subject(ticket: dict) -> dict:
    repo = Path(ticket["repo"])
    return {"repo": str(repo), "branch": ticket["branch"], "base": ticket["base"],
            "common": roles.git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")}


def assignment(ticket: dict, slot: str) -> dict:
    return {"schema": 1, "slot": slot, "provider": roles.provider_for(ticket, slot),
            "ticket_sha256": bindings.digest(ticket), "subject": subject(ticket)}


def task_key(ticket: dict) -> str:
    return bindings.digest({"common": subject(ticket)["common"], "id": ticket["id"]})


def validation_execution(repo: Path, command: list[str]) -> tuple[list[str], dict | None]:
    """Route repository-local dev commands through the current host runner."""
    if len(command) < 3 or not Path(command[0]).name.startswith("python"):
        return command, None
    requested = Path(command[1])
    if not requested.is_absolute():
        requested = repo / requested
    try:
        requested = requested.resolve(strict=True)
    except OSError:
        return command, None
    candidate_dev = (repo / "scripts/dev.py").resolve()
    if requested != candidate_dev:
        return command, None
    runner = Path(__file__).with_name("orca_host_validation.py").resolve(strict=True)
    runner_sha256 = hashlib.sha256(runner.read_bytes()).hexdigest()
    execution = [sys.executable, str(runner), "--repo", str(repo), "--", *command[2:]]
    return execution, {"schema": 1, "kind": "host-dev-runner", "sha256": runner_sha256}


def recovered_failed_exit(previous: dict, last: dict, slot: str) -> bool:
    """Accept only a failed exit already sealed by coordinator validation recovery."""
    if last.get("exit_code") != 1 or last.get("settlement_exit", {}).get("outcome") != "failed":
        return False
    directory = bindings.storage.checked_directory(bindings.state_path(slot).parent / "loops" / "validation-resumes")
    marker = Path(last.get("coordinator_validation_recovery", ""))
    if marker.parent != directory:
        return False
    receipt = bindings.storage.read_private_json(marker, {})
    return (receipt.get("schema") == 1
            and receipt.get("subject") == slot
            and isinstance(receipt.get("request_id"), str)
            and isinstance(receipt.get("run_id"), str)
            and re.fullmatch(r"[a-f0-9]{64}", str(receipt.get("evidence", ""))) is not None
            and receipt.get("source_sha256") == previous.get("source_sha256")
            and last["settlement_exit"].get("source_sha256") == previous.get("source_sha256"))


def worker_exit(ticket: dict, slot: str) -> dict:
    if slot not in {"worker-a", "worker-b"} or ticket.get("read_only"):
        raise ValueError("checkpoint requires an editing worker")
    data = bindings.read_state(slot, roles.provider_for(ticket, slot))
    key = task_key(ticket)
    previous = data["tasks"].get(key)
    last = data.get("last") or {}
    if (not previous or last.get("key") != key or last.get("phase") != "recorded"
            or last.get("process_exited") is not True
            or (last.get("exit_code") != 0 and not recovered_failed_exit(previous, last, slot))
            or previous["ticket_sha256"] != bindings.digest(ticket)
            or previous["subject"] != subject(ticket)):
        raise ValueError("checkpoint requires exact successful worker exit")
    runtime = roles.prepare_runtime(f"{slot}/tasks/{key}")
    observed = bindings.session_snapshot(runtime, data["provider"], Path(previous["origin"]))
    if any(previous[name] != observed[name] for name in observed):
        raise ValueError("worker session changed after exit")
    roles.require_completed_bridge(last, Path(ticket["repo"]))
    return previous


def validate(ticket: dict, slot: str, command: list[str], help_reason: str,
             help_decision: str = "none") -> dict:
    """Run coordinator-selected argv, never command text obtained from an event."""
    if (not command or not all(isinstance(arg, str) and arg for arg in command)
            or not help_reason.strip() or len(help_reason) > 2000 or "\n" in help_reason
            or help_decision not in {"none", "updated"}):
        raise ValueError("validation requires explicit argv and a bounded Help decision")
    repo = Path(ticket["repo"])
    execution, executor = validation_execution(repo, command)
    with ExitStack() as leases:
        leases.enter_context(acquire_host(slot, inherit=False))
        leases.enter_context(acquire_host(roles.workspace_slot(repo), inherit=False))
        roles.validate_ticket(ticket)
        previous = worker_exit(ticket, slot)
        roles.worker_scope(ticket, initial=False)
        before = roles.fingerprint(repo)
        if previous["source_sha256"] != before:
            raise ValueError("source changed since worker exit")
        environment = dict(os.environ)
        if executor is None:
            heavy = leases.enter_context(acquire_host("heavy"))
            environment = heavy.environment(environment)
        else:
            # The current host runner acquires the heavy slot only around real
            # Cargo/audit work. Keeping an outer lease across a frozen
            # worktree's coordination unit tests makes those tests contend
            # with their own parent and masks the product diagnostics.
            environment.pop(HOST_FD_ENV, None)
        if help_decision == "none":
            environment["HELL_WORKERS_HELP_IMPACT_REASON"] = help_reason
        else:
            environment.pop("HELL_WORKERS_HELP_IMPACT_REASON", None)
        result = subprocess.run(execution, cwd=repo, env=environment, pass_fds=host_pass_fds(environment),
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=1800, check=False)
        if before != roles.fingerprint(repo):
            raise ValueError("validation modified source or index; no evidence accepted")
        evidence = {"schema": 1, "ticket_sha256": bindings.digest(ticket), "slot": slot,
                    "source_sha256": before, "command": command, "exit_code": result.returncode,
                    "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                    "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
                    "help_decision": help_decision, "help_reason": help_reason}
        if executor is not None:
            evidence["executor"] = executor
        if result.returncode:
            # Private diagnostics are data for a bounded follow-up, never argv.
            output = result.stdout + b"\n" + result.stderr
            evidence["diagnostic"] = output[-8000:].decode("utf-8", errors="replace")
            evidence["diagnostic_truncated"] = len(output) > 8000
        identifier = bindings.digest(evidence)
        bindings.storage.write_ledger(root() / f"validation-{identifier}.json", evidence)
        return {"id": identifier, **evidence}


def authorize_validation_retry(ticket: dict, slot: str, validation_id: str) -> dict:
    """Authorize another edit of the same dirty source without creating a commit."""
    if not isinstance(validation_id, str) or not re.fullmatch(r"[a-f0-9]{64}", validation_id):
        raise ValueError("invalid validation identity")
    repo = Path(ticket["repo"])
    with acquire_host(slot, inherit=False), acquire_host(roles.workspace_slot(repo), inherit=False):
        roles.validate_ticket(ticket)
        roles.worker_scope(ticket, initial=False)
        previous = worker_exit(ticket, slot)
        source = roles.fingerprint(repo)
        evidence = bindings.storage.read_private_json(root() / f"validation-{validation_id}.json", {})
        if (bindings.digest(evidence) != validation_id or evidence.get("ticket_sha256") != bindings.digest(ticket)
                or evidence.get("slot") != slot or evidence.get("source_sha256") != source
                or type(evidence.get("exit_code")) is not int or evidence["exit_code"] == 0
                or previous["source_sha256"] != source):
            raise ValueError("retry requires failed same-source validation")
        old_assignment = bindings.storage.read_private_json(
            bindings.state_path(slot).parent / "assignments" / f"{task_key(ticket)}.json", {})
        if old_assignment != assignment(ticket, slot):
            raise ValueError("retry assignment differs from worker ownership")
        successor = {**ticket, "generation": ticket.get("generation", 0) + 1}
        receipt = {"schema": 1, "phase": "validation_retry", "slot": slot,
                   "provider": roles.provider_for(ticket, slot), "ticket": ticket,
                   "next_ticket": successor, "previous_task": previous,
                   "previous_assignment": old_assignment, "next_assignment": assignment(successor, slot),
                   "source_after": source, "validation": evidence}
        path = bindings.generation_path(successor, slot)
        if bindings.storage.read_private_json(path, receipt) != receipt:
            raise ValueError("retry generation already has different authorization")
        bindings.storage.write_ledger(path, receipt)
        return receipt


def checkpoint(ticket: dict, slot: str, validation_id: str) -> dict:
    """Idempotent local commit; a durable prepared commit is recovered, not recreated."""
    if not isinstance(validation_id, str) or not re.fullmatch(r"[a-f0-9]{64}", validation_id):
        raise ValueError("invalid validation identity")
    repo = Path(ticket["repo"])
    operation = bindings.digest({"ticket": ticket, "slot": slot, "validation": validation_id})
    path = root() / f"{operation}.json"
    with ExitStack() as leases:
        for name in (slot, roles.workspace_slot(repo), "workspace-" + task_key(ticket)):
            leases.enter_context(acquire_host(name, inherit=False))
        saved = bindings.storage.read_private_json(path, {})
        if saved:
            if saved.get("operation") != operation or saved.get("ticket") != ticket or saved.get("slot") != slot:
                raise ValueError("invalid checkpoint journal")
            return finish(saved, path)
        roles.validate_ticket(ticket)
        roles.worker_scope(ticket, initial=False)
        previous = worker_exit(ticket, slot)
        source = roles.fingerprint(repo)
        evidence = bindings.storage.read_private_json(root() / f"validation-{validation_id}.json", {})
        if (bindings.digest(evidence) != validation_id or evidence.get("ticket_sha256") != bindings.digest(ticket)
                or evidence.get("slot") != slot or evidence.get("source_sha256") != source
                or evidence.get("exit_code") != 0 or previous["source_sha256"] != source):
            raise ValueError("checkpoint requires successful same-source validation")
        current_assignment = bindings.storage.read_private_json(
            bindings.state_path(slot).parent / "assignments" / f"{task_key(ticket)}.json", {})
        if current_assignment != assignment(ticket, slot):
            raise ValueError("checkpoint assignment differs from worker ownership")
        index = root() / f"{operation}.index"
        environment = {**os.environ, "GIT_INDEX_FILE": str(index)}
        # This is an isolated index; the worker's real index remains unchanged.
        try:
            git(repo, "read-tree", ticket["base"], env=environment)
            git(repo, "add", "-A", "--", *ticket["allowed_directories"], env=environment)
            tree = git(repo, "write-tree", env=environment)
        finally:
            index.unlink(missing_ok=True)
        if tree == git(repo, "rev-parse", f"{ticket['base']}^{{tree}}"):
            raise ValueError("empty checkpoint; no implementation change")
        message = (f"chore: checkpoint {ticket['id']}\n\nOrca-Operation: {operation}\n"
                   f"Orca-Validation: {validation_id}\nHelp-Impact: {evidence['help_decision']}\n"
                   f"Help-Impact-Reason: {evidence['help_reason']}\n")
        candidate = git(repo, "commit-tree", tree, "-p", ticket["base"], body=message)
        next_ticket = {**ticket, "base": candidate, "generation": ticket.get("generation", 0) + 1,
                       "assignment_base": ticket.get("assignment_base", ticket["base"])}
        saved = {"schema": 1, "operation": operation, "phase": "prepared", "ticket": ticket,
                 "slot": slot, "provider": roles.provider_for(ticket, slot), "previous_task": previous,
                 "previous_assignment": current_assignment, "source_before": source,
                 "candidate": candidate, "tree": tree, "validation": evidence, "next_ticket": next_ticket,
                 "next_assignment": assignment(next_ticket, slot)}
        bindings.storage.write_ledger(path, saved)
        return finish(saved, path)


def finish(saved: dict, path: Path) -> dict:
    ticket, candidate = saved["ticket"], saved["candidate"]
    repo = Path(ticket["repo"])
    if git(repo, "branch", "--show-current") != ticket["branch"]:
        raise ValueError("checkpoint branch changed; preserve and reconcile")
    if (git(repo, "show", "-s", "--format=%P", candidate) != ticket["base"]
            or git(repo, "rev-parse", f"{candidate}^{{tree}}") != saved["tree"]):
        raise ValueError("checkpoint commit no longer matches its intent")
    worker_exit(ticket, saved["slot"])
    head = git(repo, "rev-parse", "HEAD")
    if head == ticket["base"] and saved["phase"] == "prepared":
        roles.validate_ticket(ticket)
        if roles.fingerprint(repo) != saved["source_before"]:
            raise ValueError("checkpoint source changed before ref update")
        git(repo, "update-ref", f"refs/heads/{ticket['branch']}", candidate, head)
    elif head != candidate:
        raise ValueError("checkpoint HEAD is unknown; never rewrite history")
    # Only the old clean index or the already-completed index may be replaced.
    if git(repo, "write-tree") not in {saved["tree"], git(repo, "rev-parse", f"{ticket['base']}^{{tree}}") }:
        raise ValueError("checkpoint index changed; preserve and reconcile")
    index = root() / f"{saved['operation']}.verify-index"
    environment = {**os.environ, "GIT_INDEX_FILE": str(index)}
    try:
        git(repo, "read-tree", candidate, env=environment)
        git(repo, "diff", "--exit-code", candidate, "--", env=environment)
        if git(repo, "ls-files", "--others", "--exclude-standard", env=environment):
            raise ValueError("checkpoint has unexpected untracked source")
    finally:
        index.unlink(missing_ok=True)
    git(repo, "read-tree", candidate)
    roles.validate_ticket(saved["next_ticket"])
    saved.update(phase="committed", source_after=roles.fingerprint(repo))
    bindings.storage.write_ledger(path, saved)
    receipt_path = bindings.generation_path(saved["next_ticket"], saved["slot"])
    existing = bindings.storage.read_private_json(receipt_path, saved)
    if existing != saved:
        raise ValueError("generation receipt differs; never overwrite authorization")
    bindings.storage.write_ledger(receipt_path, saved)
    return saved


def review_ticket(receipt: dict) -> dict:
    """Keep launcher HEAD distinct from the actual comparison base."""
    worker = receipt["next_ticket"]
    verified = bindings.generation_receipt(worker, receipt["slot"], receipt["provider"])
    if verified != receipt:
        raise ValueError("checkpoint receipt differs from stored authorization")
    repo = Path(worker["repo"])
    if roles.fingerprint(repo) != receipt["source_after"] or git(repo, "status", "--porcelain"):
        raise ValueError("review requires the clean checkpoint subject")
    diff_base = receipt["ticket"].get("assignment_base", receipt["ticket"]["base"])
    ticket = {"schema": 1, "id": "review-" + task_key(worker)[:40], "repo": str(repo),
              "branch": worker["branch"], "base": receipt["candidate"], "review_base": diff_base,
              "generation": worker["generation"], "read_only": True, "allowed_directories": [],
              "source_sha256": receipt["source_after"],
              "validation_evidence": bindings.digest(receipt["validation"]),
              "prompt": (f"Review only {diff_base}..{receipt['candidate']}. "
                         f"Assigned scope: {receipt['ticket']['allowed_directories']}. "
                         "Do not edit. Return verdict approved or changes_requested and blocking_findings "
                         "as objects containing id, message, acceptance. Include exact base/head, "
                         "source_sha256 and validation_evidence from this ticket. "
                         "In your worker_done body, after the three-sentence summary, put one final line "
                         "ORCA_REVIEW_JSON: followed by a single-line JSON object with exactly "
                         "ticket, base, head, source_sha256, validation_evidence, verdict, blocking_findings. "
                         f"Use ticket={('review-' + task_key(worker)[:40])!r}. "
                         f"Exact source_sha256={receipt['source_after']}; "
                         f"validation_evidence={bindings.digest(receipt['validation'])}. "
                         "A completed review with changes_requested is a succeeded review Task, not a failed Task.")}
    return roles.validate_ticket(ticket)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("validate", "commit"))
    parser.add_argument("--ticket", type=Path, required=True)
    parser.add_argument("--slot", choices=("worker-a", "worker-b"), required=True)
    parser.add_argument("--validation-id")
    parser.add_argument("--help-reason", default="")
    parser.add_argument("--help-decision", choices=("none", "updated"), default="none")
    args, trailing = parser.parse_known_args()
    try:
        ticket = json.loads(args.ticket.read_text())
        if args.action == "validate":
            if trailing[:1] != ["--"]:
                raise ValueError("validation argv must follow --")
            result = validate(ticket, args.slot, trailing[1:], args.help_reason, args.help_decision)
        else:
            if trailing:
                raise ValueError("unexpected commit arguments")
            result = checkpoint(ticket, args.slot, args.validation_id)
            result = {**result, "review_ticket": review_ticket(result)}
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0 if result.get("exit_code", 0) == 0 else 1
    except (ValueError, OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Checkpoint stopped: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
