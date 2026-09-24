"""Host-only integration of immutable worker approvals into a dedicated checkout.

No push, reset, conflict resolution or inferred final approval. Prepare the entire
merge before touching the target; recover only exact before/after Git states.
"""

from __future__ import annotations

import hashlib
import os
import re
import subprocess
from contextlib import ExitStack
from pathlib import Path

if __package__:
    from . import orca_git_checkpoint as checkpoints, orca_role_state as bindings, orca_roles as roles
    from .host_coordination import acquire_host, host_pass_fds
else:
    import orca_git_checkpoint as checkpoints
    import orca_role_state as bindings
    import orca_roles as roles
    from host_coordination import acquire_host, host_pass_fds

git = checkpoints.git


def root() -> Path:
    return bindings.storage.checked_directory(bindings.state_path("worker-a").parent / "integrations")


def target_ticket(target: dict, head: str | None = None) -> dict:
    if not isinstance(target, dict) or set(target) != {"repo", "branch", "base"}:
        raise ValueError("integration target requires exact repo, branch and base")
    return {"schema": 1, "id": "integration-" + bindings.digest(target)[:40], **target,
            "base": head or target["base"], "read_only": True, "allowed_directories": [],
            "prompt": "Inspect the combined subject only; do not edit."}


def check_target(target: dict, head: str | None = None) -> Path:
    ticket = roles.validate_ticket(target_ticket(target, head))
    repo = Path(ticket["repo"])
    if git(repo, "status", "--porcelain"):
        raise ValueError("integration requires a clean target checkout")
    # Plumbing updates must not bypass an in-progress Git operation or sparse index.
    for name in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"):
        if Path(git(repo, "rev-parse", "--path-format=absolute", "--git-path", name)).exists():
            raise ValueError("integration target has an unfinished Git operation")
    if any(line and line[0] != "H" for line in git(repo, "ls-files", "-v").splitlines()):
        raise ValueError("integration requires ordinary tracked index entries")
    return repo


def check_approvals(target: dict, approvals: list[dict]) -> None:
    if not isinstance(approvals, list) or not 1 <= len(approvals) <= 2:
        raise ValueError("integration requires one or two worker approvals")
    common = checkpoints.subject(target_ticket(target))["common"]
    repos, slots = {target["repo"]}, set()
    for item in approvals:
        if (not isinstance(item, dict) or set(item) != {"slot", "ticket", "review"}
                or item["slot"] not in {"worker-a", "worker-b"} or item["slot"] in slots):
            raise ValueError("integration requires distinct worker approval subjects")
        ticket, record = item["ticket"], item["review"]
        if (ticket["repo"] in repos or checkpoints.subject(ticket)["common"] != common
                or ticket.get("review_base") != target["base"] or not record.get("receipt_id")):
            raise ValueError("approval requires a separate checkout, common repository and exact integration base")
        roles.verify_review(ticket, record)
        if git(Path(ticket["repo"]), "status", "--porcelain"):
            raise ValueError("worker approval must be a clean checkpoint")
        slots.add(item["slot"])
        repos.add(ticket["repo"])


def integrate(target: dict, approvals: list[dict], previous: dict | None = None) -> dict:
    """Idempotently combine a fixed, ordered set of approvals, never dirty work."""
    target_ticket(target)
    operation = bindings.digest({"target": target, "approvals": approvals, "previous": previous})
    path = root() / f"{operation}.json"
    repo = Path(target["repo"])
    with ExitStack() as leases:
        # Worker roles cannot resume while their approval is being consumed.
        for slot in ("worker-a", "worker-b", roles.workspace_slot(repo)):
            leases.enter_context(acquire_host(slot, inherit=False))
        check_approvals(target, approvals)
        saved = bindings.storage.read_private_json(path, {})
        if saved:
            if (saved.get("operation") != operation or saved.get("target") != target
                    or saved.get("approvals") != approvals or saved.get("previous") != previous):
                raise ValueError("integration journal identity changed")
            return finish(saved, path)
        if previous is not None:
            check_receipt(previous)
            if previous["target"] != target:
                raise ValueError("previous integration belongs to another target")
        start = previous["head"] if previous else target["base"]
        check_target(target, start)
        source = roles.fingerprint(repo)
        head, commits = start, []
        for item in approvals:
            worker = item["ticket"]["base"]
            git(repo, "merge-base", "--is-ancestor", target["base"], worker)
            if git(repo, "merge-base", head, worker) == worker:
                continue  # Already integrated unchanged lane; retain its graph.
            # No forced merge-base or conflict-favoring strategy; exit 1 rejects
            # the conflict tree without changing target index/worktree/ref.
            tree = git(repo, "merge-tree", "--write-tree", "--no-messages", head, worker)
            if not re.fullmatch(r"[a-f0-9]{40}", tree):
                raise ValueError("merge-tree did not return one clean tree")
            message = (f"chore: integrate {item['slot']} checkpoint\n\n"
                       f"Orca-Integration: {operation}\nOrca-Review: {item['review']['receipt_id']}\n")
            candidate = git(repo, "commit-tree", tree, "-p", head, "-p", worker, body=message)
            commits.append({"head": candidate, "parents": [head, worker], "tree": tree})
            head = candidate
        if not commits:
            raise ValueError("integration has no new approved checkpoint")
        saved = {"schema": 1, "operation": operation, "phase": "prepared", "target": target,
                 "approvals": approvals, "previous": previous, "start": start,
                 "generation": previous["generation"] + 1 if previous else 1,
                 "source_before": source, "commits": commits, "head": head}
        bindings.storage.write_ledger(path, saved)
        return finish(saved, path)


def finish(saved: dict, path: Path) -> dict:
    target, candidate, start = saved["target"], saved["head"], saved["start"]
    repo = Path(target["repo"])
    for commit in saved["commits"]:
        if (git(repo, "show", "-s", "--format=%P", commit["head"]) != " ".join(commit["parents"])
                or git(repo, "rev-parse", f"{commit['head']}^{{tree}}") != commit["tree"]):
            raise ValueError("integration commit differs from prepared intent")
    if git(repo, "branch", "--show-current") != target["branch"]:
        raise ValueError("integration branch changed; preserve and reconcile")
    head = git(repo, "rev-parse", "HEAD")
    if saved["phase"] == "committed":
        check_target(target, candidate)
        if roles.fingerprint(repo) != saved["source_after"]:
            raise ValueError("integrated source changed after commit")
        return saved
    if head not in {start, candidate}:
        raise ValueError("integration HEAD is unknown; never rewrite history")
    index = git(repo, "write-tree")
    old_tree = git(repo, "rev-parse", f"{start}^{{tree}}")
    new_tree = saved["commits"][-1]["tree"]
    if head == start and index == old_tree and saved["phase"] == "prepared":
        check_target(target, start)
        if roles.fingerprint(repo) != saved["source_before"]:
            raise ValueError("integration source changed before checkout")
        # Ignored local files are outside fingerprints too. Do not overwrite them.
        def paths(commit):
            raw = subprocess.check_output(["git", "-C", str(repo), "ls-tree", "-rz", "--name-only", commit])
            return {os.fsdecode(name) for name in raw.split(b"\0") if name}
        for name in paths(candidate) - paths(start):
            local = repo / name
            if os.path.lexists(local) or any(parent.is_symlink() for parent in local.parents if parent != repo):
                raise ValueError("integration would overwrite an existing local path")
        # Git's two-tree update refuses lost local changes. Never use --reset.
        git(repo, "read-tree", "-m", "-u", start, candidate)
    elif index != new_tree:
        raise ValueError("integration index is unknown; preserve and reconcile")
    git(repo, "diff-files", "--quiet")
    if git(repo, "ls-files", "--others", "--exclude-standard") or git(repo, "write-tree") != new_tree:
        raise ValueError("integration checkout incomplete or externally modified")
    check_approvals(target, saved["approvals"])
    if head == start:
        git(repo, "update-ref", f"refs/heads/{target['branch']}", candidate, head)
    check_target(target, candidate)
    saved.update(phase="committed", source_after=roles.fingerprint(repo))
    bindings.storage.write_ledger(path, saved)
    return saved


def validate(receipt: dict, config: dict) -> dict:
    """Fresh combined-head evidence; worker validation is never reused."""
    target, head = receipt["target"], receipt["head"]
    repo = Path(target["repo"])
    with acquire_host(roles.workspace_slot(repo), inherit=False), acquire_host("heavy") as heavy:
        check_receipt(receipt)
        source = roles.fingerprint(repo)
        environment = heavy.environment(dict(os.environ))
        if config["help_decision"] == "none":
            environment["HELL_WORKERS_HELP_IMPACT_REASON"] = config["help_reason"]
        else:
            environment.pop("HELL_WORKERS_HELP_IMPACT_REASON", None)
        result = subprocess.run(config["argv"], cwd=repo, env=environment, pass_fds=host_pass_fds(environment),
                                stdin=subprocess.DEVNULL, capture_output=True, timeout=1800, check=False)
        if source != roles.fingerprint(repo):
            raise ValueError("combined validation modified source or index")
        evidence = {"schema": 1, "integration": receipt["operation"], "head": head, "source_sha256": source,
                    "command": config["argv"], "exit_code": result.returncode,
                    "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                    "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
                    "help_reason": config["help_reason"], "help_decision": config["help_decision"]}
        if result.returncode:
            evidence["diagnostic"] = (result.stdout + b"\n" + result.stderr)[-8000:].decode("utf-8", errors="replace")
        identifier = bindings.digest(evidence)
        bindings.storage.write_ledger(root() / f"validation-{identifier}.json", evidence)
        return {"id": identifier, **evidence}


def check_receipt(receipt: dict) -> None:
    stored = bindings.storage.read_private_json(root() / f"{receipt['operation']}.json", {})
    if stored != receipt or receipt.get("phase") != "committed":
        raise ValueError("integration receipt is not the committed host record")
    repo = check_target(receipt["target"], receipt["head"])
    if roles.fingerprint(repo) != receipt["source_after"]:
        raise ValueError("integrated source changed")


def review_ticket(receipt: dict, evidence: dict) -> dict:
    check_receipt(receipt)
    value = {key: val for key, val in evidence.items() if key != "id"}
    if (bindings.digest(value) != evidence.get("id") or evidence.get("exit_code") != 0
            or evidence.get("integration") != receipt["operation"]
            or evidence.get("head") != receipt["head"] or evidence.get("source_sha256") != receipt["source_after"]
            or bindings.storage.read_private_json(root() / f"validation-{evidence['id']}.json", {}) != value):
        raise ValueError("final review requires successful same-integration validation")
    ticket = target_ticket(receipt["target"], receipt["head"])
    ticket.update(review_base=receipt["target"]["base"], source_sha256=receipt["source_after"],
                  validation_evidence=evidence["id"], generation=receipt["generation"])
    ticket["prompt"] = (f"Review the combined changes {ticket['review_base']}..{ticket['base']}, including interactions. "
                        "Do not reuse individual worker approvals. Do not edit. A completed review, even with "
                        "changes_requested, is a succeeded Task. After the three-sentence worker_done summary, "
                        "include exactly one final ORCA_REVIEW_JSON: line with JSON keys ticket, base, head, "
                        "source_sha256, validation_evidence, verdict, blocking_findings. "
                        "verdict must be approved or changes_requested; findings contain id, message, acceptance. "
                        f"ticket={ticket['id']}; base={ticket['review_base']}; head={ticket['base']}; "
                        f"source_sha256={ticket['source_sha256']}; validation_evidence={evidence['id']}.")
    return roles.validate_ticket(ticket)
