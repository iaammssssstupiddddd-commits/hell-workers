"""Start one supervised editing worker from a reviewed Linear intake ticket.

The host controller owns Orca Run/Task/Dispatch creation and arms the private
Task bridge only after Orca has accepted the exact existing terminal.  It never
retries an unknown mutation, validates, commits, reviews, or updates Linear.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

try:
    import orca_coordinator as coordinator
    import orca_frontdesk as frontdesk
    import orca_issue_context as intake
    import orca_role_state as bindings
    import orca_roles as roles
    import orca_task_bridge as task_bridge
    import orca_ui_coordinator as ui_coordinator
    from host_coordination import acquire_host, state_root
except ModuleNotFoundError:
    from scripts import (orca_coordinator as coordinator, orca_frontdesk as frontdesk,
                         orca_issue_context as intake, orca_role_state as bindings,
                         orca_roles as roles, orca_task_bridge as task_bridge,
                         orca_ui_coordinator as ui_coordinator)
    from scripts.host_coordination import acquire_host, state_root


KEY = re.compile(r"[A-Za-z0-9_-]{1,128}")
BRIDGE_WAIT_SECONDS = 45


class DispatchError(RuntimeError):
    """A fail-closed dispatch error without raw provider or Linear content."""


def dispatch_path(request_id: str, ticket: dict) -> Path:
    request_id = coordinator.identity(request_id)
    name = f"{request_id}-{ticket['id']}.json"
    return frontdesk.checked_directory(state_root().parent / "dispatches") / name


def save(path: Path, data: dict) -> None:
    frontdesk.write_ledger(path, data)


def linear_record(request_id: str) -> dict:
    records = intake.read_ledger(intake.ledger_path())["imports"]
    matches = [item for item in records if item["request_id"] == request_id]
    if len(matches) != 1:
        raise DispatchError("dispatch requires exactly one immutable Linear intake record")
    return matches[0]


def checked_consultation(request_id: str) -> dict:
    data = coordinator.read_state(request_id)
    if not data["turns"] or data["turns"][-1].get("phase") != "succeeded":
        raise DispatchError("a successful coordinator consultation is required before dispatch")
    return data


def checked_coordinator(request_id: str, terminal: str) -> dict:
    """Accept the visible UI coordinator, retaining old saved consultations for recovery."""
    path = ui_coordinator.state_path(request_id)
    if path.exists() or path.is_symlink():
        try:
            return ui_coordinator.require_ready(request_id, terminal)
        except ui_coordinator.UiCoordinatorError as error:
            raise DispatchError(str(error)) from error
    return checked_consultation(request_id)


def decode_cli(completed: subprocess.CompletedProcess[str], operation: str) -> dict:
    if completed.returncode != 0 or completed.stderr.strip():
        raise DispatchError(f"Orca {operation} did not complete cleanly; preserve state and inspect")
    try:
        value = json.loads(completed.stdout)
    except (json.JSONDecodeError, UnicodeError) as error:
        raise DispatchError(f"Orca {operation} returned invalid JSON") from error
    if not isinstance(value, dict) or value.get("ok") is not True or not isinstance(value.get("result"), dict):
        raise DispatchError(f"Orca {operation} was not accepted")
    return value["result"]


def run_cli(executable: Path, argv: list[str], operation: str) -> dict:
    try:
        completed = subprocess.run(
            [str(executable), *argv, "--json"], stdin=subprocess.DEVNULL,
            capture_output=True, text=True, timeout=75, check=False,
        )
    except (OSError, UnicodeError, subprocess.TimeoutExpired) as error:
        raise DispatchError(f"Orca {operation} outcome is unknown; do not retry automatically") from error
    return decode_cli(completed, operation)


def lifecycle_key(value: object, prefix: str) -> str:
    if not isinstance(value, str) or not value.startswith(prefix) or not KEY.fullmatch(value):
        raise DispatchError(f"invalid {prefix.rstrip('_')} identity in Orca receipt")
    return value


def task_spec(ticket: dict, slot: str) -> str:
    reviewer = slot == "reviewer"
    scope = (", ".join(ticket["allowed_directories"])
             if not reviewer else "the exact read-only repository subject")
    acceptance = ticket.get(
        "acceptance", "Diff stays inside the assigned scope and worker_done reports the result."
    )
    return (
        f"Target: {scope}.\n"
        f"Change: {ticket['prompt']}\n"
        "Constraints: read AGENTS.md; no subagents, builds, tests, commit, push, PR, or Linear writes.\n"
        + ("Ownership: inspect only; do not edit. Report blocking findings to the coordinator.\n"
           if reviewer else
           f"Ownership: edit only {scope}; validation, review, commit, and integration remain coordinator-owned.\n")
        + f"Observable acceptance: {acceptance}"
    )


def wait_for_bridge(ticket: dict, slot: str, terminal: str) -> str:
    provider = roles.provider_for(ticket, slot)
    deadline = time.monotonic() + BRIDGE_WAIT_SECONDS
    while time.monotonic() < deadline:
        try:
            data = bindings.read_state(slot, provider, allow_pending=True)
        except (OSError, ValueError):
            time.sleep(0.1)
            continue
        last = data.get("last")
        if (isinstance(last, dict) and last.get("phase") == "starting"
                and last.get("terminal") == terminal and last.get("orca_bridge")):
            return bindings.identity(last["orca_bridge"])
        time.sleep(0.1)
    raise DispatchError("controlled role did not expose its bridge in time; preserve the terminal")


def start(request_id: str, ticket_path: Path, slot: str, coordinator_handle: str,
          *, orca_cli: Path | None = None, metadata: Path | None = None) -> dict:
    if slot not in {"worker-a", "worker-b", "reviewer"}:
        raise DispatchError("dispatch supports worker-a, worker-b, or the fixed reviewer")
    task_bridge.wire.identifier(coordinator_handle, prefix="term_")
    if not ticket_path.is_absolute():
        raise DispatchError("ticket path must be absolute")
    ticket = roles.load_ticket(ticket_path)
    if slot != "reviewer" and ticket.get("read_only") is True:
        raise DispatchError("editing dispatch requires a writable ticket")
    if slot == "reviewer" and (ticket.get("read_only") is not True or not ticket.get("source_sha256")):
        raise DispatchError("review dispatch requires a read-only ticket with source_sha256")
    roles.provider_for(ticket, slot)
    record = linear_record(request_id)
    checked_coordinator(request_id, coordinator_handle)
    executable = intake.checked_orca_cli(orca_cli or intake.default_orca_cli())
    metadata = metadata or Path.home() / ".config/orca"
    if not metadata.is_absolute() or not metadata.is_dir() or metadata.resolve() != metadata:
        raise DispatchError("Orca metadata directory is unavailable")
    path = dispatch_path(request_id, ticket)
    with acquire_host("dispatch-" + bindings.digest({"request": request_id, "ticket": ticket["id"]}),
                      inherit=False):
        if path.exists() or path.is_symlink():
            current = frontdesk.read_private_json(path, {})
            if current.get("phase") == "armed":
                return current
            raise DispatchError("a previous dispatch attempt exists; inspect it instead of starting a duplicate")
        data = {
            "schema": 1, "request_id": request_id, "linear_identifier": record["identifier"],
            "ticket_id": ticket["id"], "ticket_sha256": bindings.digest(ticket), "slot": slot,
            "repo": ticket["repo"], "phase": "starting", "run_id": None, "terminal": None,
            "bridge_id": None, "task_id": None, "dispatch_id": None,
        }
        save(path, data)
        try:
            run_result = run_cli(
                executable,
                ["orchestration", "run-create", "--objective",
                 f"{record['identifier']}: {ticket['id']}", "--from", coordinator_handle],
                "run-create",
            )
            run = run_result.get("run")
            if not isinstance(run, dict):
                raise DispatchError("Orca run-create receipt is incomplete")
            data["run_id"] = lifecycle_key(run.get("id"), "run_")
            save(path, data)

            launcher_argv = [
                sys.executable, str(Path(ticket["repo"]) / "scripts/orca_roles.py"), "launch",
                "--ticket", str(ticket_path), "--slot", slot,
                "--bridge-orca", str(executable), "--bridge-metadata", str(metadata),
            ]
            if slot == "reviewer":
                reviewer_state = bindings.read_state("reviewer", "codex")
                fixed = reviewer_state["tasks"].get("fixed-reviewer")
                if fixed:
                    launcher_argv.extend(["--resume-session", fixed["session_id"]])
            launcher = shlex.join(launcher_argv)
            titles = {
                "worker-a": "実装A（Codex）",
                "worker-b": "実装B（Cursor）",
                "reviewer": "レビュー（固定Codex）",
            }
            terminal_result = run_cli(
                executable,
                ["terminal", "create", "--worktree", f"path:{ticket['repo']}",
                 "--title", f"{titles[slot]} | {ticket['id']}", "--command", launcher],
                "terminal-create",
            )
            terminal_row = terminal_result.get("terminal")
            if not isinstance(terminal_row, dict):
                raise DispatchError("Orca terminal-create receipt is incomplete")
            data["terminal"] = task_bridge.wire.identifier(terminal_row.get("handle"), prefix="term_")
            save(path, data)

            data["bridge_id"] = wait_for_bridge(ticket, slot, data["terminal"])
            save(path, data)
            worker = run_cli(
                executable,
                ["orchestration", "worker-start", "--run", data["run_id"],
                 "--spec", task_spec(ticket, slot), "--task-title", ticket["id"],
                 "--worktree", f"path:{ticket['repo']}", "--terminal", data["terminal"],
                 "--from", coordinator_handle, "--timeout-ms", "60000"],
                "worker-start",
            )
            if worker.get("state") != "ready" or worker.get("stage") != "input_accepted":
                raise DispatchError("Orca worker-start did not reach ready/input_accepted")
            if worker.get("runId") != data["run_id"]:
                raise DispatchError("Orca worker-start returned a different Run")
            data["task_id"] = lifecycle_key(worker.get("taskId"), "task_")
            data["dispatch_id"] = lifecycle_key(worker.get("dispatchId"), "ctx_")
            save(path, data)
            task_bridge.arm(data["bridge_id"], {
                "run": data["run_id"], "task": data["task_id"],
                "dispatch": data["dispatch_id"], "coordinator": coordinator_handle,
            })
            data["phase"] = "armed"
            save(path, data)
            return data
        except BaseException:
            data["phase"] = "unknown"
            save(path, data)
            raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("start", "show"))
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--ticket", type=Path)
    parser.add_argument("--slot", choices=("worker-a", "worker-b", "reviewer"))
    parser.add_argument("--coordinator", default=os.environ.get("ORCA_TERMINAL_HANDLE"))
    parser.add_argument("--orca", type=Path)
    parser.add_argument("--metadata", type=Path)
    args = parser.parse_args()
    try:
        if args.action == "start":
            if args.ticket is None or args.slot is None or args.coordinator is None:
                raise DispatchError("start requires ticket, slot, and an Orca coordinator terminal")
            result = start(args.request_id, args.ticket.resolve(), args.slot, args.coordinator,
                           orca_cli=args.orca, metadata=args.metadata)
        else:
            if args.ticket is None:
                raise DispatchError("show requires the ticket path")
            ticket = roles.load_ticket(args.ticket.resolve())
            result = frontdesk.read_private_json(dispatch_path(args.request_id, ticket), {})
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except (DispatchError, OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"監督付き配車を停止しました: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
