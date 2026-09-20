"""Linux-only, opt-in Orca worker/reviewer launcher with mount write boundaries.

Workers edit existing, explicitly assigned directories; Git and authoritative
docs stay read-only. The coordinator builds, reviews evidence and commits.
No model/effort choice, publishing, agent nesting or automatic task dispatch.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import uuid
from contextlib import ExitStack
from pathlib import Path

try:
    import orca_role_state as bindings
    import orca_task_bridge as task_bridge
    from host_coordination import acquire_host, state_root
    from orca_providers import command_for, cursor_permissions, provider_for, write_cursor_policy
except ModuleNotFoundError:
    from scripts import orca_role_state as bindings
    from scripts import orca_task_bridge as task_bridge
    from scripts.host_coordination import acquire_host, state_root
    from scripts.orca_providers import command_for, cursor_permissions, provider_for, write_cursor_policy


def git(repo: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()


def load_ticket(path: Path) -> dict:
    ticket = json.loads(path.read_text(encoding="utf-8"))
    return validate_ticket(ticket)


def validate_ticket(ticket: dict) -> dict:
    if not isinstance(ticket, dict) or ticket.get("schema") != 1:
        raise ValueError("ticket schema must be 1")
    for key in ("id", "branch", "base", "prompt"):
        if not isinstance(ticket.get(key), str) or not ticket[key].strip():
            raise ValueError(f"ticket requires {key}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,63}", ticket["id"]):
        raise ValueError("invalid ticket id")
    repo = Path(ticket["repo"])
    if not repo.is_absolute() or repo.resolve() != repo:
        raise ValueError("ticket repo must be a canonical absolute path")
    if git(repo, "rev-parse", "--show-toplevel") != str(repo):
        raise ValueError("ticket must name the checkout root")
    common = Path(git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir"))
    if common.parent == repo:
        raise ValueError("agents must use a linked worktree, never the primary checkout")
    if git(repo, "branch", "--show-current") != ticket["branch"]:
        raise ValueError("ticket branch differs from checkout")
    if not re.fullmatch(r"[a-f0-9]{40}", ticket["base"]):
        raise ValueError("ticket base must be a full SHA")
    if git(repo, "rev-parse", "HEAD") != ticket["base"]:
        raise ValueError("ticket base changed; reissue the ticket and review")
    allowed = ticket.get("allowed_directories", [])
    if not isinstance(allowed, list):
        raise ValueError("allowed_directories must be a list")
    if type(ticket.get("read_only", False)) is not bool:
        raise ValueError("read_only must be a boolean")
    if ticket.get("read_only") and allowed:
        raise ValueError("read-only ticket must have no writable directories")
    paths: list[Path] = []
    for value in allowed:
        if not isinstance(value, str):
            raise ValueError("allowed directory must be a string")
        relative = Path(value)
        if (relative.is_absolute() or not relative.parts or
                any(part in {"..", ".git", ".codex", ".agents"} for part in relative.parts)
                or relative.parts[0] in {"docs", "target", ".cursor", ".github"}):
            raise ValueError(f"protected/non-relative allowed directory: {value}")
        candidate = repo / relative
        if candidate.resolve() != candidate or not candidate.is_dir():
            raise ValueError(f"allow only existing non-symlink directories: {value}")
        for child in candidate.rglob("*"):
            if child.is_symlink() or (child.is_file() and child.stat().st_nlink != 1):
                raise ValueError(f"symlink/hardlink in writable scope: {child}")
        if any(candidate.is_relative_to(other) or other.is_relative_to(candidate) for other in paths):
            raise ValueError("overlapping allowed directories")
        paths.append(candidate)
    return ticket


def fingerprint(repo: Path) -> str:
    """Bind review to HEAD and exact tracked/untracked non-ignored source bytes."""
    digest = hashlib.sha256(git(repo, "rev-parse", "HEAD").encode())
    names = subprocess.check_output([
        "git", "-C", str(repo), "ls-files", "-z", "--cached", "--others", "--exclude-standard"
    ]).split(b"\0")
    for name in sorted(set(names) - {b""}):
        path = repo / os.fsdecode(name)
        digest.update(name + b"\0")
        if path.is_symlink():
            digest.update(b"link\0" + os.fsencode(os.readlink(path)))
        elif path.is_file():
            digest.update(str(path.stat().st_mode & 0o777).encode() + b"\0")
            with path.open("rb") as handle:
                for block in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(block)
        else:
            digest.update(b"deleted")
    digest.update(subprocess.check_output(["git", "-C", str(repo), "diff", "--cached", "--binary"]))
    return digest.hexdigest()


def sandbox_command(ticket: dict, role: str, runtime: Path, command: list[str],
                    *, provider: str = "codex", policy: Path | None = None,
                    bridge: task_bridge.Session | None = None) -> list[str]:
    """Outer mount namespace constrains every process, including MCPs and hooks."""
    bwrap = shutil.which("bwrap")
    if not bwrap or sys.platform != "linux":
        raise RuntimeError("role isolation requires Linux bubblewrap; no unsafe fallback")
    repo = Path(ticket["repo"])
    result = [bwrap, "--die-with-parent", "--new-session", "--unshare-pid",
              "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev",
              "--tmpfs", "/tmp", "--tmpfs", "/run", "--clearenv"]
    resolv = Path("/etc/resolv.conf").resolve(strict=True)
    # systemd-resolved often stores this under /run, which we hide above.
    result.extend(["--ro-bind", str(resolv), str(resolv)])
    for key in ("PATH", "HOME", "USER", "LOGNAME", "TERM", "LANG", "COLORTERM"):
        if key in os.environ:
            result.extend(["--setenv", key, os.environ[key]])
    # Same-UID role processes must not read each other's proxy tokens, controller
    # journals or live capability-bearing transcripts. Re-expose only this runtime.
    private_state = state_root().parent
    if private_state.is_dir():
        result.extend(["--tmpfs", str(private_state)])
    result.extend(["--bind", str(runtime), str(runtime),
                   "--setenv", "CODEX_HOME", str(runtime / "codex"),
                   "--setenv", "TMPDIR", str(runtime / "tmp"),
                   "--setenv", "PYTHONDONTWRITEBYTECODE", "1"])
    # Account credentials are mounted read-only, never copied or printed.
    if provider == "codex":
        auth = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "auth.json"
        if auth.is_file():
            result.extend(["--ro-bind", str(auth), str(runtime / "codex/auth.json")])
    else:
        if policy is None or not policy.is_file():
            raise RuntimeError("Cursor isolation requires a read-only permission policy")
        bindings.safe_file(policy)
        if (policy.name != "cli.json" or set(policy.parent.iterdir()) != {policy}
                or json.loads(policy.read_text()) != {"permissions": cursor_permissions(ticket)["permissions"]}
                or not (repo / ".cursor").is_dir() or (repo / ".cursor").is_symlink()):
            raise RuntimeError("Cursor requires an exact sanitized project permission directory")
        result.extend(["--setenv", "CURSOR_CONFIG_DIR", str(runtime / "cursor"),
                       "--setenv", "CURSOR_DATA_DIR", str(runtime / "cursor-data"),
                       "--setenv", "XDG_CONFIG_HOME", str(runtime / "xdg"),
                       "--setenv", "XDG_CACHE_HOME", str(runtime / "cache"),
                       "--setenv", "AGENT_CLI_CREDENTIAL_STORE", "file"])
        auth = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config"))) / "cursor/auth.json"
        if auth.is_file():
            result.extend(["--ro-bind", str(auth), str(runtime / "xdg/cursor/auth.json")])
    # Hide inherited config/MCP transports; the agent gets a clean Codex home.
    primary = Path(git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")).parent
    config_paths = [root / name for root in dict.fromkeys((repo, primary))
                    for name in (".codex", ".cursor", ".claude")]
    for path in (Path.home() / ".codex", Path.home() / ".orca", Path.home() / ".cursor",
                 Path.home() / ".claude",
                 Path.home() / ".config/cursor", Path.home() / ".config/orca",
                 *config_paths):
        if path.is_dir() and not runtime.is_relative_to(path):
            result.extend(["--tmpfs", str(path)])
    if provider == "cursor":
        # Global CLI metadata must remain writable (model/cache startup uses
        # atomic rename). Project permissions override it and cannot be changed.
        result.extend(["--ro-bind", str(policy.parent), str(repo / ".cursor")])
    if role == "worker" and not ticket.get("read_only"):
        for relative in ticket["allowed_directories"]:
            path = repo / relative
            result.extend(["--bind", str(path), str(path)])
    if bridge is not None:
        if provider != "codex" or (role != "reviewer" and not ticket.get("read_only")):
            raise ValueError("Task bridge currently supports read-only Codex only")
        result.extend(bridge.mounts())
    result.extend(["--chdir", str(repo), "--", *command])
    return result


def prepare_runtime(slot: str) -> Path:
    runtime = state_root().parent / "agents" / slot
    if runtime.resolve() != runtime:
        raise RuntimeError("agent runtime must not contain symlinks")
    for path in (runtime, runtime / "codex", runtime / "tmp", runtime / "cursor",
                 runtime / "xdg", runtime / "xdg/cursor", runtime / "cache", runtime / "cursor-data"):
        path.mkdir(parents=True, exist_ok=True, mode=0o700)
        if path.stat().st_uid != os.getuid() or path.stat().st_mode & 0o077:
            raise RuntimeError(f"unsafe agent runtime permissions: {path}")
    return runtime


def verify_review(ticket: dict, record: dict) -> None:
    repo = Path(ticket["repo"])
    expected = {"ticket": ticket["id"], "base": ticket["base"],
                "head": git(repo, "rev-parse", "HEAD"),
                "source_sha256": fingerprint(repo), "verdict": "approved"}
    if any(record.get(key) != value for key, value in expected.items()):
        raise ValueError("review is stale or not approved for this exact subject")
    if not record.get("reviewer_session") or not record.get("validation_evidence"):
        raise ValueError("review requires fixed reviewer session and same-subject validation evidence")
    if record.get("blocking_findings") != []:
        raise ValueError("review has unresolved or unrecorded blocking findings")
    with acquire_host("reviewer", inherit=False), acquire_host(workspace_slot(repo), inherit=False):
        validate_ticket(ticket)
        data = bindings.read_state("reviewer", "codex")
        bound = data["tasks"].get("fixed-reviewer")
        if (not bound or record["reviewer_session"] != bound["session_id"]
                or bound["ticket_sha256"] != bindings.digest(ticket)
                or bound["source_sha256"] != fingerprint(repo)
                or data["last"]["exit_code"] != 0):
            raise ValueError("review does not match the fixed reviewer's latest observed subject")
        snapshot = bindings.session_snapshot(prepare_runtime("reviewer"), "codex", Path(bound["origin"]))
        if any(snapshot[key] != bound[key] for key in snapshot):
            raise ValueError("fixed reviewer history changed; reconcile before accepting review")


def workspace_slot(repo: Path) -> str:
    return "workspace-" + hashlib.sha256(str(repo).encode()).hexdigest()


def worker_scope(ticket: dict, *, initial: bool) -> None:
    repo = Path(ticket["repo"])
    if initial and git(repo, "status", "--porcelain"):
        raise ValueError("worker launch requires a clean checkout; preserve existing changes")
    if git(repo, "diff", "--cached", "--name-only"):
        raise ValueError("worker index changed; coordinator must reconcile")
    names = set()
    for args in (("diff", "HEAD", "--no-renames", "--name-only", "-z"),
                 ("ls-files", "--others", "--exclude-standard", "-z")):
        names.update(subprocess.check_output(["git", "-C", str(repo), *args]).split(b"\0"))
    for name in names - {b""}:
        path = Path(os.fsdecode(name))
        if not any(path.is_relative_to(scope) for scope in map(Path, ticket["allowed_directories"])):
            raise ValueError("worker has changes outside the assigned scope; preserve and reconcile")


def run_provider(command: list[str], data: dict) -> int:
    child = None
    try:
        child = subprocess.Popen(command, start_new_session=True, umask=0o077)
        return child.wait()
    finally:
        if child is not None:
            if child.poll() is None:
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=5)
            data["last"].update(process_exited=True, exit_code=child.returncode)
        # Until all postconditions pass this stays unknown, even after exit 0.
        data["last"]["phase"] = "unknown"
        bindings.save_state(data)


def launch(ticket: dict, slot: str, *, dry_run: bool, resume_session: str | None = None,
           follow_up: str | None = None, bridge_settings: tuple[Path, Path] | None = None) -> int:
    role = "reviewer" if slot == "reviewer" else "worker"
    read_only = role == "reviewer" or ticket.get("read_only") is True
    provider = provider_for(ticket, slot)
    if bridge_settings and (provider != "codex" or not read_only or dry_run):
        raise ValueError("Task bridge requires a real read-only Codex launch; Cursor remains denied")
    if resume_session:
        bindings.identity(resume_session)
    if follow_up is not None and (not follow_up.strip() or len(follow_up) > 32_000):
        raise ValueError("follow-up must contain 1..32000 characters")
    if role == "worker" and not read_only and not ticket.get("allowed_directories"):
        raise ValueError("worker needs a nonempty writable directory scope")
    repo = Path(ticket["repo"])
    prompt = (
        f"Role: {role}. Ticket: {ticket['id']}. Read AGENTS.md. No subagents, no commit, "
        "no push, no changes outside assigned directories. Do not start builds/tests/analysis "
        "servers; ask the coordinator for validation. Report findings and stop. "
        f"Source access: {'read-only; never edit files' if read_only else 'assigned directories only'}. "
        f"Allowed directories: {[] if read_only else ticket['allowed_directories']}.\n"
        + (follow_up if follow_up is not None else ticket["prompt"])
    )
    if bridge_settings:
        prompt += ("\nTask bridge bootstrap only: do not invent lifecycle IDs or send any Orca RPC until a live "
                   "Orca preamble arrives. Do not create runs, tasks, workers or gates. When dispatched, copy "
                   "its executable, terminal, capability and IDs exactly; use --json. Ask/check waits require "
                   "--timeout-ms 10000. Process all delivered messages before explicit check --ack. "
                   "A bridge refusal means stop and ask the host coordinator to reconcile, never resend. "
                   "worker_done is not review approval. No edits or builds are allowed.")
    if dry_run:
        command = command_for(provider, repo, role, prompt, resume_session, read_only=read_only)
        print(json.dumps({"slot": slot, "role": role, "provider": provider, "repo": str(repo),
                          "read_only": read_only,
                          "allowed_directories": [] if read_only else ticket["allowed_directories"],
                          "command": command}, ensure_ascii=False, indent=2))
        return 0
    with ExitStack() as leases:
        leases.enter_context(acquire_host(slot, inherit=False))
        leases.enter_context(acquire_host(workspace_slot(repo), inherit=False))
        validate_ticket(ticket)
        before = fingerprint(repo)
        subject = {"repo": str(repo), "branch": ticket["branch"], "base": ticket["base"],
                   "common": git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")}
        data = bindings.read_state(slot, provider)
        key, previous = bindings.admit(data, ticket, subject, before, resume_session, follow_up)
        if read_only and ticket.get("source_sha256") != before:
            raise ValueError("read-only ticket needs the current source_sha256; regenerate after changes")
        if role == "worker":
            if not read_only:
                worker_scope(ticket, initial=previous is None)
            task_key = bindings.digest({"common": subject["common"], "id": ticket["id"]})
            leases.enter_context(acquire_host("workspace-" + task_key, inherit=False))
            bindings.claim_task(task_key, slot, provider, ticket, subject)
            # Do not silently adopt pre-ledger slot history when moving to task runtimes.
            if bindings.history_exists(prepare_runtime(slot)):
                raise ValueError("unbound legacy worker history; reconcile before launch")
        runtime = prepare_runtime(slot if role == "reviewer" else f"{slot}/tasks/{key}")
        origin = Path(previous["origin"]) if previous else repo
        if previous:
            snapshot = bindings.session_snapshot(runtime, provider, origin)
            if any(snapshot[name] != previous[name] for name in snapshot):
                raise ValueError("provider session changed since observed exit; reconcile before resume")
        elif bindings.history_exists(runtime):
            raise ValueError("unbound provider history; never start a replacement session")
        policy = None
        if provider == "cursor":
            policy = bindings.storage.checked_directory(state_root() / "worker-b-cursor-policy") / "cli.json"
            write_cursor_policy(ticket, policy, project=True)
            config = runtime / "cursor/cli-config.json"
            if not config.exists() and not config.is_symlink():
                write_cursor_policy(ticket, config)
            else:
                current = bindings.storage.read_private_json(config, {})
                if (not isinstance(current, dict) or current.get("version") != 1
                        or not isinstance(current.get("editor"), dict)
                        or not isinstance(current["editor"].get("vimMode"), bool)):
                    raise ValueError("invalid Cursor runtime configuration; preserve for reconciliation")
        with ExitStack() as channels:
            bridge = None
            if bridge_settings:
                def verify_subject():
                    validate_ticket(ticket)
                    if fingerprint(repo) != before:
                        raise ValueError("bridge source changed; preserve and reconcile")
                bridge = channels.enter_context(task_bridge.Session(
                    *bridge_settings, os.environ.get("ORCA_TERMINAL_HANDLE"), repo, verify_subject))
                prompt += (f"\nFor every Orca CLI invocation use exactly {bridge.client}; "
                           "never use bare `orca` or the installed client directly. Do not read or print "
                           "the bridge metadata; the wrapper supplies its private transport path.")
            command = command_for(provider, repo, role, prompt, resume_session, read_only=read_only,
                                  externally_sandboxed=bridge is not None)
            command = sandbox_command(ticket, role, runtime, command, provider=provider, policy=policy, bridge=bridge)
            data["last"] = {"attempt_id": str(uuid.uuid4()), "key": key, "phase": "starting",
                            "process_exited": False, "exit_code": None, "source_before": before,
                            "input_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
                            "terminal": os.environ.get("ORCA_TERMINAL_HANDLE"),
                            **({"orca_bridge": bridge.identifier} if bridge else {})}
            bindings.save_state(data)
            code = run_provider(command, data)
        if bridge and bridge.policy.phase == "unknown":
            raise RuntimeError("Task bridge outcome unknown; preserve role and reconcile before resuming")
        validate_ticket(ticket)
        after = fingerprint(repo)
        if read_only and after != before:
            raise RuntimeError("source changed during read-only observation; result is invalid")
        if role == "worker" and not read_only:
            worker_scope(ticket, initial=False)
        snapshot = bindings.session_snapshot(runtime, provider, origin)
        if previous and snapshot["session_id"] != previous["session_id"]:
            raise ValueError("provider switched session; preserve for reconciliation")
        data["tasks"][key] = {"key": key, "ticket_sha256": bindings.digest(ticket), "subject": subject,
                              "origin": str(origin), "source_sha256": after, **snapshot}
        data["last"]["phase"] = "recorded"
        bindings.save_state(data)
        print(json.dumps({"ticket": ticket["id"], "role": role, "provider": provider,
                          "exit_code": code, "session_id": snapshot["session_id"],
                          "source_sha256": after, "approved": False}))
        return code


def abandon_start(ticket: dict, slot: str, attempt_id: str, observed_source: str, reason: str) -> None:
    """Explicitly close a failed read-only first start; never retry or erase it."""
    if slot == "reviewer" or ticket.get("read_only") is not True:
        raise ValueError("only a read-only worker's empty first start can be abandoned")
    bindings.identity(attempt_id)
    if not isinstance(reason, str) or not reason.strip() or len(reason) > 2000:
        raise ValueError("abandonment requires a bounded reconciliation reason")
    provider = provider_for(ticket, slot)
    repo = Path(ticket["repo"])
    with acquire_host(slot, inherit=False), acquire_host(workspace_slot(repo), inherit=False):
        validate_ticket(ticket)
        if fingerprint(repo) != observed_source:
            raise ValueError("observed source changed; inspect the current diff before reconciliation")
        data = bindings.read_state(slot, provider, allow_pending=True)
        last = data["last"]
        subject = {"repo": str(repo), "branch": ticket["branch"], "base": ticket["base"],
                   "common": git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")}
        key = bindings.digest({"common": subject["common"], "id": ticket["id"]})
        if (not isinstance(last, dict) or last.get("attempt_id") != attempt_id or last.get("key") != key
                or last.get("phase") != "unknown" or last.get("process_exited") is not True
                or type(last.get("exit_code")) is not int or last["exit_code"] == 0 or key in data["tasks"]):
            raise ValueError("requires exact failed first attempt and positive process-exit evidence")
        if "orca_bridge" in last:
            raise ValueError("Task bridge attempt needs supervised lifecycle reconciliation; abandon-start is not sufficient")
        with acquire_host("workspace-" + key, inherit=False):
            assignment = bindings.state_path(slot).parent / "assignments" / f"{key}.json"
            expected = {"schema": 1, "slot": slot, "provider": provider,
                        "ticket_sha256": bindings.digest(ticket), "subject": subject}
            if bindings.storage.read_private_json(assignment, {}) != expected:
                raise ValueError("failed start ownership differs from the supplied ticket")
            runtime = prepare_runtime(f"{slot}/tasks/{key}")
            if bindings.history_exists(runtime) or bindings.history_exists(prepare_runtime(slot)):
                raise ValueError("provider history exists; preserve and reconcile the existing session")
            last.update(phase="abandoned", ticket_sha256=bindings.digest(ticket),
                        observed_source_sha256=observed_source, reason=reason)
            data.setdefault("abandoned", {})[key] = dict(last)
            bindings.save_state(data)


def reconcile_bridge(ticket: dict, slot: str, attempt_id: str, observed_source: str,
                     reason: str, metadata_dir: Path) -> None:
    """Record one proven failed Dispatch without accepting its task result."""
    bindings.identity(attempt_id)
    if (not isinstance(observed_source, str)
            or not re.fullmatch(r"[a-f0-9]{64}", observed_source)):
        raise ValueError("reconciliation requires the observed source fingerprint")
    if not isinstance(reason, str) or not reason.strip() or len(reason) > 2000:
        raise ValueError("reconciliation requires a bounded reason")
    if not metadata_dir.is_absolute() or not metadata_dir.is_dir():
        raise ValueError("reconciliation requires an absolute Orca metadata directory")
    provider = provider_for(ticket, slot)
    if provider != "codex" or (slot != "reviewer" and ticket.get("read_only") is not True):
        raise ValueError("only a failed read-only Codex bridge can be reconciled")
    repo = Path(ticket["repo"])
    with acquire_host(slot, inherit=False), acquire_host(workspace_slot(repo), inherit=False):
        validate_ticket(ticket)
        data = bindings.read_state(slot, provider, allow_pending=True)
        last = data["last"]
        subject = {"repo": str(repo), "branch": ticket["branch"], "base": ticket["base"],
                   "common": git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")}
        key = "fixed-reviewer" if slot == "reviewer" else bindings.digest({
            "common": subject["common"], "id": ticket["id"]})
        recorded_exit = (last.get("phase") == "unknown"
                         and last.get("process_exited") is True
                         and type(last.get("exit_code")) is int) if isinstance(last, dict) else False
        interrupted_exit = (last.get("phase") == "starting"
                            and last.get("process_exited") is False
                            and last.get("exit_code") is None) if isinstance(last, dict) else False
        if (not isinstance(last, dict) or last.get("attempt_id") != attempt_id
                or last.get("key") != key or not (recorded_exit or interrupted_exit)
                or last.get("source_before") != observed_source or last.get("orca_bridge") is None):
            raise ValueError("requires the exact exited unknown bridge attempt")
        bridge_id = bindings.identity(last["orca_bridge"])
        directory = task_bridge.root() / bridge_id
        journal = bindings.storage.read_private_json(directory / "journal.json", {})
        identity = bindings.storage.read_private_json(directory / "identity.json", {})
        authority = bindings.storage.read_private_json(directory / "arm.json", {})
        operations = journal.get("operations") if isinstance(journal, dict) else None
        mutation_free = journal.get("authority") is None and operations == {} if isinstance(journal, dict) else False
        ambiguous = (journal.get("authority") == authority and isinstance(operations, dict)
                     and bool(operations) and all(
                         isinstance(operation, str) and isinstance(row, dict)
                         and set(row) == {"signature", "phase"}
                         and isinstance(row["signature"], str)
                         and re.fullmatch(r"[a-f0-9]{64}", row["signature"])
                         and row["phase"] == "pending"
                         for operation, row in operations.items())) if isinstance(journal, dict) else False
        if (not isinstance(journal, dict) or journal.get("phase") != "unknown"
                or journal.get("revoked") is not True or not (mutation_free or ambiguous)
                or journal.get("settled_status") is not None):
            raise ValueError("bridge journal is not a recoverable unknown attempt")
        ambiguous_operations = []
        if ambiguous:
            ambiguous_operations = sorted(bindings.identity(operation) for operation in operations)
        if (not isinstance(identity, dict) or identity.get("terminal") != last.get("terminal")
                or identity.get("repo") != str(repo) or type(identity.get("pid")) is not int
                or identity["pid"] <= 0):
            raise ValueError("bridge launcher identity differs from the role attempt")
        try:
            os.kill(identity["pid"], 0)
        except ProcessLookupError:
            pass
        else:
            raise ValueError("bridge launcher is still live")
        if (not isinstance(authority, dict)
                or set(authority) != {"run", "task", "dispatch", "coordinator"}):
            raise ValueError("bridge authority is missing or incomplete")
        for value in authority.values():
            task_bridge.key(value)
        upstream = task_bridge.Upstream.load(metadata_dir)
        if identity.get("runtime") != upstream.runtime_id:
            raise ValueError("bridge belongs to a different Orca runtime")
        observed = upstream.call("orchestration.workerShow", {"dispatch": authority["dispatch"]})
        dispatch = task_bridge.wire.mapping(observed.get("dispatch"))
        worker = task_bridge.wire.mapping(observed.get("worker"))
        observation = task_bridge.wire.mapping(observed.get("observation"))
        expected = {"id": authority["dispatch"], "taskId": authority["task"],
                    "runId": authority["run"], "assigneeHandle": identity["terminal"],
                    "status": "failed"}
        if (any(dispatch.get(name) != value for name, value in expected.items())
                or not task_bridge.exact_process(
                    observed, terminal=identity["terminal"], incarnation=identity.get("incarnation"),
                    worktree=worker.get("worktreeId"), dispatch_id=authority["dispatch"])
                or dispatch.get("capabilityRevokedAt") is None
                or worker.get("dispatchId") != authority["dispatch"]
                or worker.get("runtimeEpoch") != upstream.runtime_id
                or worker.get("agentTerminalHandle") != identity["terminal"]
                or worker.get("state") not in {"abandoned", "failed", "stopped"}
                or observation.get("exactWorker") is not True):
            raise ValueError("failed Dispatch identity or settlement is unproven")
        if interrupted_exit and observation.get("status") != "exited":
            raise ValueError("interrupted bridge launcher exit is unproven")
        latest = task_bridge.wire.mapping(upstream.call(
            "orchestration.dispatchShow", {"task": authority["task"]}).get("dispatch"))
        same_attempt = latest.get("id") == authority["dispatch"] and latest.get("status") == "failed"
        directly_retried = (latest.get("retry_of_dispatch_id") == authority["dispatch"]
                            and latest.get("status") in {"failed", "completed"})
        if latest.get("task_id") != authority["task"] or not (same_attempt or directly_retried):
            raise ValueError("failed Dispatch is not the latest attempt or its direct predecessor")
        runtime = prepare_runtime(slot if slot == "reviewer" else f"{slot}/tasks/{key}")
        previous = data["tasks"].get(key)
        origin = Path(previous["origin"]) if previous else repo
        snapshot = bindings.session_snapshot(runtime, provider, origin)
        if previous and snapshot["session_id"] != previous["session_id"]:
            raise ValueError("provider switched session during the failed bridge attempt")
        evidence = {"run": authority["run"], "task": authority["task"],
                    "dispatch": authority["dispatch"], "terminal": identity["terminal"],
                    "worker_state": worker["state"], **snapshot,
                    "current_source_sha256": fingerprint(repo), "reason": reason.strip(),
                    "exit_observation": ("external_terminal_exited" if interrupted_exit
                                         else "launcher_recorded"),
                    "ambiguous_operations": ambiguous_operations}
        if interrupted_exit:
            last.update(process_exited=True)
        if previous:
            previous["session_sha256"] = snapshot["session_sha256"]
            last.update(phase="recorded", bridge_reconciliation=evidence)
        else:
            last.update(phase="abandoned", ticket_sha256=bindings.digest(ticket),
                        observed_source_sha256=observed_source, reason=reason.strip(),
                        bridge_reconciliation=evidence)
            data.setdefault("abandoned", {})[key] = dict(last)
        bindings.save_state(data)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("launch", "fingerprint", "verify-review", "abandon-start",
                                           "reconcile-bridge"))
    parser.add_argument("--ticket", type=Path, required=True)
    parser.add_argument("--slot", choices=("worker-a", "worker-b", "reviewer"), default="reviewer")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--resume-session")
    parser.add_argument("--follow-up-file", type=Path)
    parser.add_argument("--review-record", type=Path)
    parser.add_argument("--attempt-id")
    parser.add_argument("--observed-source")
    parser.add_argument("--reason")
    parser.add_argument("--bridge-orca", type=Path)
    parser.add_argument("--bridge-metadata", type=Path)
    args = parser.parse_args()
    try:
        bridge_settings = None
        if args.action == "launch" and (args.bridge_orca or args.bridge_metadata):
            if not (args.bridge_orca and args.bridge_metadata):
                raise ValueError("launch needs both --bridge-orca and --bridge-metadata")
            bridge_settings = (args.bridge_orca, args.bridge_metadata)
        elif args.action != "reconcile-bridge" and (args.bridge_orca or args.bridge_metadata):
            raise ValueError("bridge options are not valid for this action")
        ticket = load_ticket(args.ticket)
        if args.action == "fingerprint":
            print(fingerprint(Path(ticket["repo"])))
            return 0
        if args.action == "verify-review":
            if args.review_record is None:
                raise ValueError("--review-record is required")
            verify_review(ticket, json.loads(args.review_record.read_text()))
            print("Review matches the exact source; this command does not integrate or publish.")
            return 0
        if args.action == "abandon-start":
            if args.dry_run:
                raise ValueError("abandon-start does not support --dry-run; no state changed")
            abandon_start(ticket, args.slot, args.attempt_id, args.observed_source, args.reason)
            print("Failed read-only start preserved as abandoned; no agent started and no task approved.")
            return 0
        if args.action == "reconcile-bridge":
            if args.dry_run or args.bridge_orca or args.bridge_metadata is None:
                raise ValueError("reconcile-bridge needs --bridge-metadata and does not support dry-run/--bridge-orca")
            reconcile_bridge(ticket, args.slot, args.attempt_id, args.observed_source,
                             args.reason, args.bridge_metadata)
            print("Failed bridge reconciled; no task result was accepted or approved.")
            return 0
        return launch(ticket, args.slot, dry_run=args.dry_run, resume_session=args.resume_session,
                      follow_up=args.follow_up_file.read_text() if args.follow_up_file else None,
                      bridge_settings=bridge_settings)
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Orca role refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
