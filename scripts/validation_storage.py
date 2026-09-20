#!/usr/bin/env python3
"""Register validation output, preserve review caches, and check finalization.

The ledger lives in the primary Git common directory. No command deletes data.
Frozen subjects run their own helpers under the primary coordinator.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import UTC, datetime
try:
    import fcntl
except ImportError:
    fcntl = None
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from typing import Any


REPO_ROOT = Path(__file__).resolve().parent.parent
POLICY = "docs/development-infra/validation-storage-workflow.md"
PRIMARY_ENV = "HW_VALIDATION_PRIMARY"
BATCH_ENV = "HW_VALIDATION_BATCH"
GIB = 1024**3
HOLD_KINDS = {"review", "comparison", "diagnostic", "evidence"}


def now() -> datetime:
    return datetime.now(UTC)


def stamp() -> str:
    return now().isoformat()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(f"Validation storage: {message}")


def nonempty(value: Any) -> bool:
    return isinstance(value, str) and bool(value.strip())


def instant(value: str) -> datetime:
    parsed = datetime.fromisoformat(value)
    require(parsed.tzinfo is not None, "timestamps require a timezone")
    return parsed


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(
        ["git", "-C", str(repo), *args], text=True, stderr=subprocess.PIPE
    ).strip()


def primary_for(repo: Path) -> Path:
    explicit = os.environ.get(PRIMARY_ENV)
    if explicit:
        primary = Path(explicit).resolve()
    else:
        common = Path(git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir"))
        primary = common.parent
    require((primary / POLICY).is_file(), f"primary policy is missing: {primary}; use the primary coordinator")
    return primary


def ledger_dir(primary: Path) -> Path:
    common = Path(git(primary, "rev-parse", "--path-format=absolute", "--git-common-dir"))
    require(common.parent == primary.resolve(), "the coordinator must use the primary checkout")
    return common / "validation-storage"


def atomic_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".ledger-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as output:
            json.dump(value, output, ensure_ascii=False, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


@contextmanager
def locked(primary: Path, *, initialize: bool = False):
    require(fcntl is not None, "local validation coordination requires POSIX file locks")
    directory = ledger_dir(primary)
    path = directory / "ledger.json"
    require(initialize or path.is_file(), "ledger is missing; run validation init in the primary checkout")
    directory.mkdir(parents=True, exist_ok=True)
    with (directory / "ledger.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        value = json.loads(path.read_text()) if path.exists() else None
        if value is not None:
            require(value.get("schema_version") == 1 and value.get("primary") == str(primary), "ledger identity/schema differs")
        yield value


def save(primary: Path, ledger: dict, action: str) -> None:
    ledger["history"].append({"at": stamp(), "action": action})
    atomic_json(ledger_dir(primary) / "ledger.json", ledger)


def overlap(a: Path, b: Path) -> bool:
    return a == b or a.is_relative_to(b) or b.is_relative_to(a)


def minimal_paths(paths) -> list[Path]:
    result: list[Path] = []
    for path in sorted(set(map(Path, paths)), key=lambda p: (len(p.parts), str(p))):
        if not any(path.is_relative_to(parent) for parent in result):
            result.append(path)
    return result


def allocated(path: Path) -> int:
    if not path.exists():
        return 0
    return int(subprocess.check_output(["du", "-sx", "-B1", "--", str(path)], text=True).split()[0])


def discover(primary: Path) -> dict:
    """Inspect worktrees/clones and output directories, never Cargo cache contents."""
    repos = {primary}
    pending = [primary]
    while pending:
        repo = pending.pop()
        for line in git(repo, "worktree", "list", "--porcelain").splitlines():
            if line.startswith("worktree "):
                found = Path(line[9:]).resolve()
                if found not in repos and found.exists():
                    repos.add(found)
                    pending.append(found)
        target = repo / "target"
        if target.is_dir():
            for entry in target.iterdir():
                if entry.is_dir() and not entry.is_symlink() and (entry / ".git").exists():
                    found = entry.resolve()
                    if found not in repos:
                        repos.add(found)
                        pending.append(found)
    footprints = {repo for repo in repos if repo != primary}
    units = set(footprints)
    for repo in repos:
        for name in ("native-acceptance", "perf-runs"):
            root = repo / "target" / name
            if not root.exists():
                continue
            footprints.add(root)
            for parent, dirs, files in os.walk(root, followlinks=False):
                # Every directory is inventoried so a new job inside an old group
                # cannot hide behind the group's pre-migration classification.
                units.update(Path(parent) / name for name in dirs)
                if files and Path(parent) != root:
                    units.add(Path(parent))
        target = repo / "target"
        if target.is_dir():
            for entry in target.glob("legacy-cargo-target-*"):
                footprints.add(entry)
                units.add(entry)
    return {
        "repos": sorted(map(str, repos)),
        "units": sorted(map(str, units)),
        "footprints": sorted(map(str, minimal_paths(footprints))),
    }


def initialize(primary: Path, budget: int | None = None) -> dict:
    with locked(primary, initialize=True) as existing:
        require(existing is None, "ledger already initialized; existing/new data cannot be relabelled as legacy")
        inventory = discover(primary)
        ledger = {
            "schema_version": 1, "primary": str(primary), "created_at": stamp(),
            "budget_bytes": budget, "legacy": inventory, "holds": {}, "batches": {}, "history": [],
        }
        require(budget is None or budget > 0, "an explicit budget must be positive")
        save(primary, ledger, "initialize legacy-untriaged inventory")
        return ledger


def validate_hold(hold: dict, previous: dict | None = None) -> None:
    for key in ("id", "path", "owner", "next_action", "release_when"):
        require(nonempty(hold.get(key)), f"hold requires {key}")
    require(hold.get("kind") in HOLD_KINDS, "unknown hold kind")
    require(isinstance(hold.get("consumers"), list) and hold["consumers"] and all(map(nonempty, hold["consumers"])), "hold requires concrete consumers")
    budget = hold.get("budget_bytes")
    require(budget is None or (isinstance(budget, int) and budget > 0), "an explicit hold budget must be positive")
    if hold.get("review_at"):
        instant(hold["review_at"])
    if hold["kind"] == "review":
        require(nonempty(hold.get("review_item_id")), "review hold requires a stable review_item_id")
        require(nonempty(hold.get("latest_feedback_at")), "review hold requires latest presentation/revision time")
        instant(hold["latest_feedback_at"])
    if hold["kind"] == "diagnostic":
        require(nonempty(hold.get("failure_cause")), "diagnostic requires failure_cause")
        require(nonempty(hold.get("origin_at")), "diagnostic requires original failure time")
        if previous:
            require(hold["origin_at"] == previous["origin_at"], "cannot reset original failure time")


def retain(primary: Path, hold: dict) -> dict:
    hold = dict(hold)
    hold["path"] = str(Path(hold["path"]).resolve())
    if hold.get("kind") == "comparison":
        if not hold.get("frozen_repo"):
            source = Path(hold["path"])
            hold["frozen_repo"] = next((str(parent) for parent in (source, *source.parents) if (parent / ".git").exists()), None)
        require(nonempty(hold["frozen_repo"]), "comparison requires the original frozen_repo")
        frozen = Path(hold["frozen_repo"]).resolve()
        require(git(frozen, "rev-parse", "--show-toplevel") == str(frozen), "frozen_repo must be a checkout root")
        hold["frozen_repo"] = str(frozen)
    with locked(primary) as ledger:
        previous = ledger["holds"].get(hold.get("id"))
        validate_hold(hold, previous)
        if previous:
            require(previous["path"] == hold["path"] and previous["kind"] == hold["kind"], "hold identity/path cannot be changed")
            require(previous.get("frozen_repo") == hold.get("frozen_repo"), "frozen repository dependency cannot be changed")
            require(set(previous["consumers"]) <= set(hold["consumers"]), "remove consumers with release and an explicit closure reason")
            hold["revisions"] = [*previous.get("revisions", []), {k: v for k, v in previous.items() if k != "revisions"}]
        for other in ledger["holds"].values():
            if other["id"] == hold["id"] or not other["consumers"]:
                continue
            if hold["kind"] == other["kind"] == "review":
                require(hold["review_item_id"] != other["review_item_id"], "review item already owns a candidate; reuse its existing workspace/target")
        require(Path(hold["path"]).exists(), "retained path does not exist")
        hold["observed_bytes"] = allocated(Path(hold["path"]))
        hold["observed_at"] = stamp()
        hold["known_units"] = previous.get("known_units", []) if previous else [
            path for path in discover(primary)["units"] if Path(path).is_relative_to(Path(hold["path"]))]
        require(hold.get("budget_bytes") is None or hold["observed_bytes"] <= hold["budget_bytes"], "retained path exceeds its explicit budget")
        ledger["holds"][hold["id"]] = hold
        save(primary, ledger, f"retain/review {hold['id']}")
        return hold


def release(primary: Path, identity: str, consumer: str, reason: str) -> None:
    require(nonempty(reason), "release requires final acceptance/closure/abandonment reason")
    with locked(primary) as ledger:
        hold = ledger["holds"][identity]
        require(consumer in hold["consumers"], "consumer is not registered")
        hold["consumers"].remove(consumer)
        if not hold["consumers"]:
            # Keep an auditable outstanding disposal, not a silently forgotten path.
            hold["closed_reason"] = reason
        save(primary, ledger, f"release {identity}/{consumer}: {reason}")


def policy_hash(primary: Path) -> str:
    return digest(primary / POLICY)


def subject_fingerprint(repo: Path) -> str:
    tracked = git(repo, "ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0")
    paths = {repo / name for name in tracked if name and (
        name.startswith(("crates/", "scripts/", ".cargo/", ".codex/skills/hell-workers-run-native-acceptance/scripts/"))
        or name in {"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml"}
    )}
    assets = repo / "assets"
    if assets.exists():
        for path in assets.rglob("*"):
            require(not (path.is_symlink() and path.is_dir()), "asset directory symlink cannot be frozen by this coordinator")
            if path.is_file():
                paths.add(path)
    hashed = hashlib.sha256()
    for path in sorted(paths):
        hashed.update(str(path.relative_to(repo)).encode() + b"\0")
        hashed.update((digest(path) if path.is_file() else "missing").encode())
    return hashed.hexdigest()


def require_mutable(repo: Path) -> None:
    primary = primary_for(repo)
    if not (ledger_dir(primary) / "ledger.json").exists():
        return
    with locked(primary) as ledger:
        for hold in ledger["holds"].values():
            require(not (hold["kind"] == "comparison" and hold["consumers"] and repo.resolve() == Path(hold["frozen_repo"])),
                    f"frozen comparison blocks Cargo writes: {hold['id']}")


def usage(ledger: dict, inventory: dict) -> int:
    paths = list(inventory["footprints"])
    paths.extend(hold["path"] for hold in ledger["holds"].values())
    paths.extend(path for batch in ledger["batches"].values() for path in batch["roots"])
    return sum(allocated(path) for path in minimal_paths(paths))


def live_identity(pid: int) -> str | None:
    try:
        # Field 22 follows the parenthesized process name, which may contain spaces.
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        if fields[0] == "Z":
            return None
        return Path("/proc/sys/kernel/random/boot_id").read_text().strip() + ":" + fields[19]
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None


def is_running(batch: dict) -> bool:
    if batch["phase"] == "finalized":
        return False
    return any(batch.get(identity) is not None and live_identity(batch[pid]) == batch[identity]
               for pid, identity in (("pid", "process_identity"), ("child_pid", "child_identity")))


def hold_errors(hold: dict, ledger: dict | None = None) -> list[str]:
    path = Path(hold["path"])
    if not hold["consumers"]:
        if ledger and any(other["consumers"] and path.is_relative_to(Path(other["path"])) for other in ledger["holds"].values()):
            return []
        return [f"released resource still exists: {path}"] if path.exists() else []
    errors = []
    if not path.exists():
        errors.append(f"active resource missing: {path}")
    if hold.get("budget_bytes") is not None and allocated(path) > hold["budget_bytes"]:
        errors.append(f"resource exceeds budget: {hold['id']}")
    return errors


def unregistered(ledger: dict, inventory: dict) -> list[str]:
    known = set(ledger["legacy"]["units"])
    roots = [Path(path) for batch in ledger["batches"].values() if batch["phase"] != "finalized" for path in batch["roots"]]
    known.update(path for batch in ledger["batches"].values() if batch["phase"] == "finalized" for path in batch.get("retained_units", []))
    known.update(path for hold in ledger["holds"].values() if hold["consumers"] and hold["kind"] != "review"
                 for path in hold.get("known_units", []))
    # A candidate covers its workspace identity, never all future jobs inside it.
    known.update(hold["path"] for hold in ledger["holds"].values())
    return sorted(path for path in inventory["units"] if path not in known and not any(overlap(Path(path), root) for root in roots))


def unresolved_legacy(ledger: dict) -> list[str]:
    return [old for old in ledger["legacy"]["units"] if Path(old).exists() and not any(
        hold["consumers"] and (old == hold["path"] if hold["kind"] == "review"
                               else Path(old).is_relative_to(Path(hold["path"])))
        for hold in ledger["holds"].values())]


def reconcile(primary: Path) -> dict:
    """Retire disposed/classified legacy entries without grandfathering new output."""
    with locked(primary) as ledger:
        ledger["legacy"]["units"] = unresolved_legacy(ledger)
        save(primary, ledger, "reconcile disposed legacy paths and explicit active uses")
        return {"legacy_untriaged_paths": len(ledger["legacy"]["units"])}


def report_errors(ledger: dict, batch: dict) -> list[str]:
    if is_running(batch):
        return []
    if batch["phase"] != "finalized":
        return [f"batch requires finalization: {batch['id']} ({batch['phase']})"]
    errors = []
    for root in batch["roots"]:
        if Path(root).exists() and not output_has_only_active_uses(Path(root), ledger):
            errors.append(f"job remains without its own retained consumer: {root}")
    return errors


def output_has_only_active_uses(root: Path, ledger: dict) -> bool:
    retained = [Path(hold["path"]) for hold in ledger["holds"].values()
                if hold["consumers"] and hold["kind"] != "review" and Path(hold["path"]).is_relative_to(root)]
    if not retained:
        return False
    if any(root.is_relative_to(path) for path in retained):
        return True
    return all(any(path.is_relative_to(keep) for keep in retained)
               for path in root.rglob("*") if path.is_file() or path.is_symlink())


def check(primary: Path, batch_id: str | None = None) -> dict:
    inventory = discover(primary)
    if not (ledger_dir(primary) / "ledger.json").exists():
        require(not inventory["units"], "unregistered validation data exists; initialize the legacy inventory")
        return {"status": "pass", "ledger": "not initialized; no validation data"}
    with locked(primary) as ledger:
        errors = [f"unregistered validation path: {path}" for path in minimal_paths(unregistered(ledger, inventory))]
        total = usage(ledger, inventory)
        if ledger["budget_bytes"] is not None and total > ledger["budget_bytes"]:
            errors.append("total validation storage exceeds budget; retain caches and stop new outputs")
        legacy = unresolved_legacy(ledger)
        if legacy:
            errors.append(f"legacy data requires disposal or a concrete active use: {len(legacy)} paths; first: {legacy[0]}")
        for hold in ledger["holds"].values():
            errors.extend(hold_errors(hold, ledger))
        batches = [ledger["batches"][batch_id]] if batch_id else ledger["batches"].values()
        for batch in batches:
            errors.extend(report_errors(ledger, batch))
        require(not errors, "\n".join(errors))
        return {"status": "pass", "allocated_bytes": total, "budget_bytes": ledger["budget_bytes"],
                "legacy_untriaged_paths": len(legacy), "batches": len(ledger["batches"]),
                "reviews_due": [hold["id"] for hold in ledger["holds"].values()
                                if hold["consumers"] and hold.get("review_at") and instant(hold["review_at"]) <= now()],
                "note": "closed outputs need no archive; active reviews keep their caches; review dates are advisory"}


def admit(primary: Path, ledger: dict, batch: dict) -> None:
    require(batch["policy_sha256"] == policy_hash(primary), "policy changed; create a fresh plan/registration")
    require(batch["coordinator_sha256"] == digest(Path(__file__)), "coordinator changed; create a fresh plan/registration")
    repo = Path(batch["repo"])
    require(git(repo, "rev-parse", "HEAD") == batch["subject"], "subject changed since registration")
    require(subject_fingerprint(repo) == batch["subject_fingerprint"], "source/assets changed since registration")
    inventory = discover(primary)
    require(not unregistered(ledger, inventory), "unregistered validation output exists; inspect validation check")
    total = usage(ledger, inventory)
    reservations = sum(other["reserve_bytes"] for other in ledger["batches"].values() if other["id"] != batch["id"] and is_running(other))
    require(ledger["budget_bytes"] is None or total + reservations + batch["reserve_bytes"] <= ledger["budget_bytes"], "capacity reservation exceeds total budget; preserve caches and reassess before new output")
    for hold in ledger["holds"].values():
        errors = hold_errors(hold, ledger)
        require(not errors, "; ".join(errors))
        if hold["consumers"] and hold["kind"] == "comparison" and repo == Path(hold["frozen_repo"]):
            raise RuntimeError(f"Validation storage: frozen dependency blocks source/assets/binary overwrite: {hold['id']}")
    for other in ledger["batches"].values():
        if other["id"] != batch["id"] and other["repo"] == batch["repo"]:
            require(other["phase"] == "finalized", f"previous batch must be finalized first: {other['id']}")
    # Legacy data in this subject must be classified before it can be mutated.
    for old in ledger["legacy"]["units"]:
        old_path = Path(old)
        owning_repos = [Path(path) for path in inventory["repos"] if old_path.is_relative_to(Path(path))]
        owning_repo = max(owning_repos, key=lambda path: len(path.parts)) if owning_repos else None
        if old_path.exists() and owning_repo == repo:
            classified = any(
                hold["consumers"] and (
                    old_path == Path(hold["path"]) if hold["kind"] == "review"
                    else old_path.is_relative_to(Path(hold["path"]))
                ) for hold in ledger["holds"].values()
            )
            require(classified, f"subject has legacy-untriaged data: {old}; register its actual consumers first")
    if repo != primary:
        require(any(hold["kind"] == "review" and hold["path"] == str(repo) and set(batch["consumers"]) <= set(hold["consumers"]) for hold in ledger["holds"].values()), "candidate workspace needs a review hold for every batch consumer")
    for item in batch.get("command_files", []):
        require(Path(item["path"]).is_file() and digest(Path(item["path"])) == item["sha256"], "planned helper changed")


def register(primary: Path, spec: dict, *, command: list[str] | None = None) -> dict:
    batch = dict(spec)
    for key in ("id", "repo", "owner"):
        require(nonempty(batch.get(key)), f"batch requires {key}")
    require(re.fullmatch(r"[A-Za-z0-9_.-]+", batch["id"]) is not None, "batch id has invalid characters")
    require(isinstance(batch.get("consumers"), list) and batch["consumers"] and all(map(nonempty, batch["consumers"])), "batch requires consumers")
    batch.setdefault("reserve_bytes", 0)
    require(isinstance(batch["reserve_bytes"], int) and batch["reserve_bytes"] >= 0, "reserve_bytes must be nonnegative")
    require(isinstance(batch.get("roots"), list) and batch["roots"], "batch requires exact output roots")
    repo = Path(batch["repo"]).resolve()
    batch["repo"] = str(repo)
    batch["roots"] = list(map(str, minimal_paths(Path(path).resolve() for path in batch["roots"])))
    for root in map(Path, batch["roots"]):
        require(not root.exists(), f"new output root already exists: {root}")
        require(root != repo and not repo.is_relative_to(root) and len(root.parts) > 2, "output root is too broad")
    batch.update({"phase": "registered", "created_at": stamp(), "subject": git(repo, "rev-parse", "HEAD"),
                  "subject_fingerprint": subject_fingerprint(repo),
                  "coordinator_sha256": digest(Path(__file__)),
                  "policy_sha256": policy_hash(primary), "command": command,
                  "command_files": [{"path": str(Path(arg).resolve()), "sha256": digest(Path(arg))}
                                    for arg in [*(command or []), *batch.get("verify_command", [])] if Path(arg).is_absolute() and Path(arg).is_file()]})
    with locked(primary) as ledger:
        require(batch["id"] not in ledger["batches"], "batch id already registered")
        for other in ledger["batches"].values():
            require(not any(overlap(Path(a), Path(b)) for a in batch["roots"] for b in other["roots"]), "output root overlaps another batch")
        admit(primary, ledger, batch)
        batch["before_bytes"] = usage(ledger, discover(primary))
        ledger["batches"][batch["id"]] = batch
        save(primary, ledger, f"register {batch['id']}")
        return batch


def require_admission(repo: Path, paths: list[Path]) -> None:
    primary = primary_for(repo)
    identity = os.environ.get(BATCH_ENV)
    require(nonempty(identity), "unregistered execution; use primary validation plan/execute")
    with locked(primary) as ledger:
        batch = ledger["batches"].get(identity)
        require(batch is not None and batch["phase"] == "running" and is_running(batch), "batch has no live coordinator")
        require(batch["repo"] == str(repo.resolve()), "batch subject repository differs")
        require(batch["policy_sha256"] == policy_hash(primary), "policy changed during execution")
        require(ledger["budget_bytes"] is None or usage(ledger, discover(primary)) <= ledger["budget_bytes"], "total capacity exceeded; stop before the next build/session and preserve current data")
        for path in paths:
            require(any(path.resolve().is_relative_to(Path(root)) for root in batch["roots"]), f"output is outside registered roots: {path}")


def tracked_command(primary: Path, identity: str, command: list[str], repo: str, env: dict, *, capture: bool = False):
    try:
        from host_coordination import acquire_host
    except ModuleNotFoundError:
        from scripts.host_coordination import acquire_host
    with acquire_host() as host:
        return _tracked_admitted_command(primary, identity, command, repo, host.environment(env),
                                         capture=capture, pass_fds=(host.fd,))


def _tracked_admitted_command(primary: Path, identity: str, command: list[str], repo: str,
                              env: dict, *, capture: bool, pass_fds: tuple[int, ...]):
    with subprocess.Popen(command, cwd=repo, env=env, text=True,
                          pass_fds=pass_fds,
                          stdout=subprocess.PIPE if capture else None,
                          stderr=subprocess.PIPE if capture else None) as child:
        with locked(primary) as ledger:
            ledger["batches"][identity].update({"child_pid": child.pid, "child_identity": live_identity(child.pid)})
            save(primary, ledger, f"child started {identity}")
        output, errors = child.communicate()
        return child.returncode, output or "", errors or ""


def execute(primary: Path, identity: str) -> int:
    with locked(primary) as ledger:
        batch = ledger["batches"][identity]
        require(batch["phase"] == "registered", "batch cannot be launched twice")
        require(batch["command"], "batch has no sealed command; register with plan or --command")
        require(not any(Path(root).exists() for root in batch["roots"]), "planned output appeared before execution")
        admit(primary, ledger, batch)
        batch.update({"phase": "running", "pid": os.getpid(), "process_identity": live_identity(os.getpid()), "started_at": stamp()})
        require(batch["process_identity"] is not None, "Linux process identity is unavailable")
        save(primary, ledger, f"start {identity}")
        command, repo = batch["command"], batch["repo"]
    result = 130
    try:
        env = {**os.environ, PRIMARY_ENV: str(primary), BATCH_ENV: identity, "PYTHONDONTWRITEBYTECODE": "1"}
        result, _, _ = tracked_command(primary, identity, command, repo, env)
        return result
    finally:
        with locked(primary) as ledger:
            batch = ledger["batches"][identity]
            batch.update({"phase": "needs-finalize", "exit_code": result, "finished_at": stamp()})
            batch["process_identity"] = None
            save(primary, ledger, f"execution ended {identity}; finalization required")


def seal(primary: Path, identity: str, spec: dict) -> dict:
    result, reason = spec["result"], spec["reason"]
    require(nonempty(reason), "record the verification command/result or failure stage/reason")
    require(not spec.get("files") and not spec.get("capsule"), "seal records results only; keep files only for an explicit active use or product release")
    with locked(primary) as ledger:
        batch = ledger["batches"][identity]
        require(not is_running(batch), "cannot finalize a live execution")
        require(batch["phase"] not in {"finalized", "sealing"} and "result" not in batch, "batch already sealed/finalized")
        require(result in {"pass", "invalid", "interrupted", "abandoned"}, "invalid result")
        if result == "pass":
            require(batch["phase"] == "needs-finalize" and batch.get("exit_code") == 0, "successful execution is required for a pass record")
            verifier = spec.get("verify_command")
            require(verifier and verifier == batch.get("verify_command"), "pass requires the verifier sealed at registration")
            native_verify = any(arg == "verify" or arg.startswith("verify-") for arg in verifier[1:])
            perf_verify = str(Path(batch["repo"]) / "scripts/perf.py") in verifier and "summarize" in verifier
            require(native_verify or perf_verify, "pass requires frozen verify or the subject perf.py summarize validator")
            require(any(arg in batch["roots"] or (Path(arg).is_absolute() and any(Path(arg).is_relative_to(Path(root)) for root in batch["roots"])) for arg in verifier), "verifier does not address this batch's output")
            require(subject_fingerprint(Path(batch["repo"])) == batch["subject_fingerprint"], "source/assets changed before sealing")
        snapshot = dict(batch)
        batch.update({"phase": "sealing", "pid": os.getpid(), "process_identity": live_identity(os.getpid())})
        save(primary, ledger, f"seal started {identity}")
    try:
        verifier = spec.get("verify_command")
        if verifier:
            code, output, errors = tracked_command(primary, identity, verifier, snapshot["repo"],
                {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}, capture=True)
            require(code == 0, f"frozen verifier failed: {output[-2000:]} {errors[-1000:]}")
        with locked(primary) as ledger:
            batch = ledger["batches"][identity]
            batch.update({"phase": "needs-finalize", "result": result, "verification_or_failure": reason,
                          "verified_at": stamp()})
            batch["process_identity"] = None
            save(primary, ledger, f"recorded verification result {identity}; no archive required")
            return batch
    except BaseException:
        with locked(primary) as ledger:
            ledger["batches"][identity].update({"phase": "needs-finalize", "process_identity": None})
            save(primary, ledger, f"seal failed {identity}; preserve originals")
        raise


def finalize(primary: Path, identity: str) -> dict:
    with locked(primary) as ledger:
        batch = ledger["batches"][identity]
        require(batch["phase"] == "needs-finalize" and "result" in batch, "seal the verification result before finalizing")
        batch.update({"phase": "finalized", "finalized_at": stamp()})
        require(not report_errors(ledger, batch), "\n".join(report_errors(ledger, batch)))
        inventory = discover(primary)
        batch["retained_units"] = [path for path in inventory["units"] if any(overlap(Path(path), Path(root)) for root in batch["roots"])]
        batch["after_bytes"] = usage(ledger, inventory)
        save(primary, ledger, f"finalize {identity}: {batch['result']}")
        return batch


def recover(primary: Path, identity: str, reason: str) -> None:
    require(nonempty(reason), "recovery requires observed process/failure details")
    with locked(primary) as ledger:
        batch = ledger["batches"][identity]
        require(not is_running(batch), "coordinator/helper still alive; preserve its data")
        require(batch["phase"] in {"running", "sealing", "needs-finalize"}, "batch does not need recovery")
        batch.update({"phase": "needs-finalize", "exit_code": batch.get("exit_code", 130), "process_identity": None, "child_identity": None})
        save(primary, ledger, f"recover {identity}: {reason}")


def plan(primary: Path, spec: dict, command: list[str]) -> dict:
    repo = Path(spec["repo"]).resolve()
    before = subject_fingerprint(repo)
    result = subprocess.run(command, cwd=repo, env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}, text=True, capture_output=True, check=False)
    require(result.returncode == 0, f"native plan failed: {result.stdout[-4000:]} {result.stderr[-1000:]}")
    payload = json.loads(result.stdout)
    require(before == subject_fingerprint(repo), "subject changed during plan generation")
    require(payload.get("status") == "ready", "native plan is not ready")
    launcher = payload["launcher_command"]
    require(launcher[:4] == ["kitty", "--directory", str(repo), "--detach"], "unsupported launcher; keep the established direct kitty boundary")
    roots = [payload[key] for key in ("job_root", "state_root", "output_root") if payload.get(key)]
    spec = dict(spec)
    if spec.get("verify_command"):
        expanded = []
        for arg in spec["verify_command"]:
            if arg.startswith("@"):
                key, _, suffix = arg[1:].partition("/")
                require(key in {"job_root", "state_root", "output_root"}, "unknown output placeholder")
                expanded.append(str(Path(payload[key]) / suffix))
            else:
                expanded.append(arg)
        spec["verify_command"] = expanded
    batch = register(primary, {**spec, "roots": roots}, command=launcher[4:])
    payload["launcher_command"] = [*launcher[:4], "env", "PYTHONDONTWRITEBYTECODE=1", "python3", str(primary / "scripts/validation_storage.py"), "--primary", str(primary), "execute", "--batch", batch["id"]]
    payload["storage_batch"] = batch["id"]
    payload["storage_report_command"] = ["python3", str(primary / "scripts/validation_storage.py"), "--primary", str(primary), "check", "--batch", batch["id"]]
    payload["storage_policy_sha256"] = batch["policy_sha256"]
    return payload


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--primary", type=Path)
    sub = parser.add_subparsers(dest="action", required=True)
    init = sub.add_parser("init")
    init.add_argument("--budget-gib", type=int)
    for name in ("retain", "register", "plan"):
        p = sub.add_parser(name)
        p.add_argument("--spec", type=Path, required=True)
        if name != "retain":
            p.add_argument("command", nargs=argparse.REMAINDER)
    p = sub.add_parser("release")
    p.add_argument("--hold", required=True)
    p.add_argument("--consumer", required=True)
    p.add_argument("--reason", required=True)
    for name in ("execute", "check", "seal", "finalize", "recover"):
        p = sub.add_parser(name)
        p.add_argument("--batch", required=name != "check")
        if name == "seal":
            p.add_argument("--spec", type=Path, required=True)
        if name == "recover":
            p.add_argument("--reason", required=True)
    p = sub.add_parser("budget")
    budget_option = p.add_mutually_exclusive_group(required=True)
    budget_option.add_argument("--gib", type=int)
    budget_option.add_argument("--none", action="store_true")
    p.add_argument("--reason", required=True)
    sub.add_parser("status")
    sub.add_parser("reconcile")
    args = parser.parse_args(argv)
    try:
        primary = args.primary.resolve() if args.primary else primary_for(REPO_ROOT)
        if args.action == "init":
            ledger = initialize(primary, args.budget_gib * GIB if args.budget_gib is not None else None)
            value = {"status": "initialized; classify existing data before checking", "budget_bytes": ledger["budget_bytes"]}
        elif args.action in {"retain", "register", "plan"}:
            spec = json.loads(args.spec.read_text())
            if args.action == "retain":
                value = retain(primary, spec)
            else:
                command = args.command[1:] if args.command[:1] == ["--"] else args.command
                value = plan(primary, spec, command) if args.action == "plan" else register(primary, spec, command=command)
        elif args.action == "release":
            release(primary, args.hold, args.consumer, args.reason)
            value = {"status": "consumer released; data preserved"}
        elif args.action == "execute":
            return execute(primary, args.batch)
        elif args.action == "finalize":
            value = finalize(primary, args.batch)
        elif args.action == "seal":
            value = seal(primary, args.batch, json.loads(args.spec.read_text()))
        elif args.action == "recover":
            recover(primary, args.batch, args.reason)
            value = {"status": "recovered; seal/finalize still required"}
        elif args.action == "budget":
            require((args.none or args.gib > 0) and nonempty(args.reason), "budget requires positive GiB or --none and a concrete reason")
            with locked(primary) as ledger:
                previous = ledger["budget_bytes"]
                ledger["budget_bytes"] = None if args.none else args.gib * GIB
                save(primary, ledger, f"budget {previous} -> {ledger['budget_bytes']}: {args.reason}")
                value = {"budget_bytes": ledger["budget_bytes"]}
        elif args.action == "check":
            value = check(primary, args.batch)
        elif args.action == "reconcile":
            value = reconcile(primary)
        else:
            with locked(primary) as ledger:
                value = {"primary": str(primary), "ledger": str(ledger_dir(primary) / "ledger.json"),
                         "budget_bytes": ledger["budget_bytes"],
                         "legacy_untriaged_paths": len(ledger["legacy"]["units"]),
                         "legacy_footprints": ledger["legacy"]["footprints"],
                         "holds": ledger["holds"],
                         "batches": {key: {name: batch.get(name) for name in ("phase", "repo", "roots", "owner", "consumers", "result")}
                                     for key, batch in ledger["batches"].items()}}
        print(json.dumps(value, ensure_ascii=False, indent=2))
        return 0
    except (RuntimeError, ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
