"""Read-only ownership check for a normal-development production runner.

Only a running primary ``dev.py validation execute`` child can obtain this
context. Environment variables and saved receipts alone do not authenticate a
session. This does not launch a game, enable the retired recipe, supply missing
measurements, approve an asset, or seal acceptance. Callers must still implement
and independently review the ordinary-world producer and its exact recipe.
"""
from __future__ import annotations

import copy
import os
from pathlib import Path
import time

if __package__:
    from . import validation_storage as storage
else:
    import validation_storage as storage


def exact_path(path: Path) -> Path:
    storage.require(path.is_absolute() and path.resolve() == path,
                    "production context requires a canonical absolute path")
    # resolve() alone does not reject a symlink whose target names this path.
    storage.require(not any(part.is_symlink() for part in (path, *path.parents)),
                    "production context paths must not contain symlinks")
    return path


def wait_for_child(primary: Path, identity: str, pid: int) -> None:
    """Allow only the parent's initial child publication, never an execution retry."""
    deadline = time.monotonic() + 2
    while True:
        with storage.locked(primary) as ledger:
            batch = ledger["batches"].get(identity)
            storage.require(batch is not None and batch["phase"] == "running",
                            "production batch is not running")
            controller = batch.get("pid")
            storage.require(type(controller) is int and controller == os.getppid()
                            and batch.get("process_identity") is not None
                            and storage.live_identity(controller) == batch["process_identity"],
                            "production coordinator identity differs")
            child, birth = batch.get("child_pid"), batch.get("child_identity")
            if child is not None or birth is not None:
                storage.require(child == pid and birth is not None
                                and storage.live_identity(pid) == birth,
                                "production runner child identity differs")
                return
        storage.require(time.monotonic() < deadline,
                        "validation parent did not publish its child within the bounded wait")
        # Popen returns before the parent publishes its child identity. Never
        # hold the ledger lock while waiting for that exact publication.
        time.sleep(0.02)


def running_context(*, repo: Path, plan: Path, root: Path, owner: str,
                    command: list[str], verifier: list[str]) -> dict:
    """Match the current runner against live ledger ownership and frozen inputs.

    ``command``/``verifier`` are the reviewed caller's literal recipe argv, not
    values authenticated by a public plan. The plan must be a hashed command
    file. No arbitrary provider is loaded, and this function never mutates the
    ledger. A wrapper/grandchild is deliberately unsupported: the direct child
    and coordinator birth identities must both match.
    """
    repo, plan, root = map(exact_path, (repo, plan, root))
    storage.require(root.parent == repo / "target/native-acceptance",
                    "production root must be a direct native-acceptance child")
    storage.require(plan.is_file() and storage.nonempty(owner),
                    "production plan/owner is missing")
    storage.require(all(isinstance(argv, list) and argv
                        and all(storage.nonempty(arg) for arg in argv)
                        for argv in (command, verifier)), "literal recipe argv is missing")
    storage.require(str(plan) in command, "recipe command does not bind its plan")
    primary = storage.primary_for(repo)
    storage.require(primary == repo, "normal production context requires the primary checkout")
    identity = os.environ.get(storage.BATCH_ENV)
    storage.require(storage.nonempty(identity), "production runner is not a validation child")
    pid = os.getpid()
    wait_for_child(primary, identity, pid)
    with storage.locked(primary) as ledger:
        batch = ledger["batches"].get(identity)
        storage.require(batch is not None and batch["phase"] == "running",
                        "production batch is not running")
        storage.require(batch.get("child_pid") == pid
                        and batch.get("child_identity") is not None
                        and storage.live_identity(pid) == batch["child_identity"],
                        "production runner child identity differs")
        controller = batch.get("pid")
        storage.require(type(controller) is int and controller == os.getppid()
                        and batch.get("process_identity") is not None
                        and storage.live_identity(controller) == batch["process_identity"],
                        "production coordinator identity differs")
        storage.require(batch["repo"] == str(repo) and batch["owner"] == owner
                        and batch["roots"] == [str(root)]
                        and batch["command"] == command
                        and batch.get("verify_command") == verifier,
                        "production recipe/owner/root differs")
        storage.require(batch["subject"] == storage.git(repo, "rev-parse", "HEAD")
                        and batch["subject_fingerprint"] == storage.subject_fingerprint(repo),
                        "production source changed")
        storage.require(batch["policy_sha256"] == storage.policy_hash(primary)
                        and batch["coordinator_sha256"] == storage.digest(Path(storage.__file__)),
                        "validation policy/controller changed")
        files = batch["command_files"]
        storage.require(any(row["path"] == str(plan) for row in files),
                        "production plan was not sealed during registration")
        for row in files:
            path = exact_path(Path(row["path"]))
            storage.require(path.is_file() and storage.digest(path) == row["sha256"],
                            "registered command file changed")
        plan_hashes = {row["sha256"] for row in files if row["path"] == str(plan)}
        storage.require(len(plan_hashes) == 1, "registered plan hashes disagree")
        result = copy.deepcopy({
            "schema_version": 1, "scope": "normal-validation-runner-ownership-only",
            "batch": identity, "repo": str(repo), "owner": owner, "root": str(root),
            "head": batch["subject"], "source": batch["subject_fingerprint"],
            "command": command, "verifier": verifier,
            "plan_sha256": next(iter(plan_hashes)),
            "runner_pid": pid, "runner_identity": batch["child_identity"],
            "coordinator_pid": controller, "coordinator_identity": batch["process_identity"],
            "accepted": False, "performance_evidence": False,
            "art_approved": False, "promotion_authority": False,
        })
    return result
