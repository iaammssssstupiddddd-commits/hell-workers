"""Own one reusable local Orca tab per request, checkout and role.

Never infer ownership or process exit from an agent's title/preview alone.
Unknown mutations remain journaled and cannot allocate a replacement tab.
"""
from __future__ import annotations

import os
import argparse
import json
import time
import uuid
from pathlib import Path

if __package__:
    from . import orca_frontdesk as storage, orca_role_state as bindings
    from .host_coordination import acquire_host, state_root
else:
    import orca_frontdesk as storage
    import orca_role_state as bindings
    from host_coordination import acquire_host, state_root


TITLES = {"worker-a": "実装A（Codex）", "worker-b": "実装B（Cursor）",
          "reviewer": "レビュー（固定Codex）"}


def root() -> Path:
    return storage.checked_directory(state_root().parent / "role-tabs")


def workspace_lock(repo: str) -> str:
    return "dispatch-" + bindings.digest({"role-tabs-workspace": repo})


def registry_path(request: str, repo: str, slot: str) -> Path:
    if slot not in TITLES:
        raise ValueError("unsupported role tab")
    key = bindings.digest({"request": request, "repo": repo, "slot": slot})
    return root() / f"{key}.json"


def idle_shell(handle: str, repo: str, proc: Path = Path("/proc")) -> dict:
    """Linux only: exactly one shell, foreground-owned tty, no session children."""
    processes, matches = [], []
    for path in proc.iterdir():
        if not path.name.isdecimal():
            continue
        try:
            fields = (path / "stat").read_text().rsplit(")", 1)[1].split()
            row = {"pid": int(path.name), "state": fields[0], "pgrp": int(fields[2]),
                   "session": int(fields[3]), "tty": int(fields[4]), "foreground": int(fields[5]),
                   "start": fields[19]}
            processes.append(row)
            if path.stat().st_uid != os.getuid():
                continue
            environment = (path / "environ").read_bytes().split(b"\0")
            if ("ORCA_TERMINAL_HANDLE=" + handle).encode() in environment:
                matches.append({**row, "comm": (path / "comm").read_text().strip(),
                                "cwd": str((path / "cwd").resolve())})
        except FileNotFoundError:
            continue
        except PermissionError:
            # A hidden same-session process still remains in the stat inventory.
            continue
    if len(matches) != 1:
        raise ValueError("role tab is busy or its local shell ownership is unknown")
    shell = matches[0]
    if (shell["comm"] not in {"bash", "zsh", "fish", "sh"} or shell["cwd"] != repo
            or shell["state"] != "S" or not shell["tty"]
            or shell["foreground"] != shell["pgrp"] or shell["pgrp"] != shell["pid"]
            or any(p["pid"] != shell["pid"] and
                   (p["session"] == shell["session"] or p["tty"] == shell["tty"])
                   for p in processes)):
        raise ValueError("role tab is not an idle foreground shell; preserve it")
    return shell


def shell_stopped(shell: dict, proc: Path = Path("/proc")) -> bool:
    """A reused PID is not the shell whose closure was journaled."""
    pid, start = shell.get("pid"), shell.get("start")
    if type(pid) is not int or pid <= 0 or not isinstance(start, str) or not start.isdecimal():
        raise ValueError("retired shell identity is incomplete")
    try:
        observed = (proc / str(pid) / "stat").read_text().rsplit(")", 1)[1].split()
    except FileNotFoundError:
        return True
    except (IndexError, OSError) as error:
        raise ValueError("retired shell process cannot be verified") from error
    if len(observed) <= 19 or not observed[19].isdecimal():
        raise ValueError("retired shell process cannot be verified")
    return observed[19] != start


def inventory(call, cli, repo: str) -> tuple[list, list]:
    result = call(cli, ["terminal", "list", "--worktree", f"path:{repo}",
                        "--include-visual-layouts"], "role-tab-list")
    rows, layouts = result.get("terminals"), result.get("visualLayouts")
    if rows == [] and layouts is None and result.get("totalCount") == 0:
        layouts = []
    if (not isinstance(rows, list) or not isinstance(layouts, list)
            or result.get("truncated") is not False
            or result.get("hostScope", {}).get("omittedHostIds") != []):
        raise ValueError("role tab inventory is incomplete")
    return rows, layouts


def identity(row: dict, repo: str) -> dict:
    if (row.get("worktreePath") != repo or row.get("executionHostId") != "local"
            or row.get("orphaned") is not False or row.get("connected") is not True
            or row.get("writable") is not True
            or not all(isinstance(row.get(k), str) and row[k]
                       for k in ("handle", "incarnationId", "worktreeId"))):
        raise ValueError("role tab identity is unproven")
    return {k: row[k] for k in ("handle", "incarnationId", "worktreeId")}


def default_shell(rows: list, layouts: list) -> dict | None:
    """Adopt only the single explicitly neutral default pane, never '統括'."""
    handles = []

    def visit(value):
        if isinstance(value, list):
            for item in value:
                visit(item)
        elif isinstance(value, dict):
            if value.get("title") == "作業シェル" and "panes" in value:
                pane = value["panes"]
                if pane.get("type") == "terminal":
                    handles.append(pane.get("handle"))
            else:
                for item in value.values():
                    if isinstance(item, (list, dict)):
                        visit(item)

    visit(layouts)
    matches = [row for row in rows if row.get("handle") in handles]
    return matches[0] if len(matches) == len(handles) == 1 else None


def default_path(repo: str, handle: str) -> Path:
    key = bindings.digest({"repo": repo, "handle": handle})
    return storage.checked_directory(root() / "defaults") / f"{key}.json"


def record_default(row: dict, repo: str, purpose: str = "shell") -> None:
    owned = identity(row, repo)
    storage.write_ledger(default_path(repo, owned["handle"]), {"identity": owned, "repo": repo, "purpose": purpose})


def adopt(call, cli, request: str, repo: str, slot: str, expected: dict) -> None:
    """Explicit migration only after the operator has selected the exact legacy tab."""
    path = registry_path(request, repo, slot)
    with acquire_host(workspace_lock(repo), inherit=False):
        if path.exists() or path.is_symlink():
            raise ValueError("role tab registry exists; do not overwrite")
        row = call(cli, ["terminal", "show", "--terminal", expected["handle"]], "role-tab-show")["terminal"]
        if identity(row, repo) != expected:
            raise ValueError("legacy role tab identity changed")
        idle_shell(expected["handle"], repo)
        storage.write_ledger(path, {"schema": 1, "request": request, "repo": repo,
                                   "slot": slot, "identity": expected, "phase": "known",
                                   "launch_id": None, "origin": "explicit-adoption"})


def retire(call, cli, repo: str, expected: dict) -> Path:
    """Explicitly selected obsolete shell only; preserve scrollback before closing."""
    handle = expected["handle"]
    key = bindings.digest(expected)
    directory = storage.checked_directory(root() / "retired")
    path = directory / f"{key}.json"
    with acquire_host(workspace_lock(repo), inherit=False):
        if path.exists() or path.is_symlink():
            raise ValueError("retirement already attempted; inspect receipt, do not replay")
        row = call(cli, ["terminal", "show", "--terminal", handle], "role-tab-show")["terminal"]
        if identity(row, repo) != expected:
            raise ValueError("obsolete tab identity changed")
        before = idle_shell(handle, repo)
        output = complete_output(call, cli, handle)
        storage.write_ledger(path, {"identity": expected, "repo": repo, "shell": before,
                                    "output": output, "phase": "prepared"})
        row = call(cli, ["terminal", "show", "--terminal", handle], "role-tab-show")["terminal"]
        if identity(row, repo) != expected or idle_shell(handle, repo) != before:
            raise ValueError("obsolete shell changed before closure")
        # Close only this pane, never another user's split in the same tab.
        receipt = call(cli, ["terminal", "close", "--terminal", handle], "role-tab-close")
        storage.write_ledger(path, {"identity": expected, "repo": repo, "shell": before, "output": output,
                                   "phase": "close-returned", "receipt": receipt})
        rows, _ = inventory(call, cli, repo)
        if any(row.get("handle") == handle for row in rows):
            raise ValueError("obsolete tab closure unconfirmed")
        for _ in range(20):
            if shell_stopped(before):
                break
            time.sleep(0.1)
        if not shell_stopped(before):
            raise ValueError("tab closed but shell exit unconfirmed; preserve receipt")
        return path


def retire_settled(call, cli, repo: str, expected: dict) -> Path:
    """Close an exact released role tab after the owning loop proves settlement."""
    handle = expected["handle"]
    key = bindings.digest({"settled": expected})
    directory = storage.checked_directory(root() / "retired-settled")
    path = directory / f"{key}.json"
    with acquire_host(workspace_lock(repo), inherit=False):
        if path.exists() or path.is_symlink():
            raise ValueError("settled retirement already attempted; inspect receipt, do not replay")
        row = call(cli, ["terminal", "show", "--terminal", handle], "role-tab-show")["terminal"]
        if identity(row, repo) != expected:
            raise ValueError("settled role tab identity changed")
        wait = call(cli, ["terminal", "wait", "--terminal", handle, "--for", "tui-idle",
                          "--timeout-ms", "15000"], "role-tab-idle").get("wait", {})
        if (wait.get("handle") != handle or wait.get("condition") != "tui-idle"
                or wait.get("satisfied") is not True or wait.get("status") != "running"):
            raise ValueError("settled role tab is not idle; preserve it")
        output = complete_output(call, cli, handle)
        storage.write_ledger(path, {"identity": expected, "repo": repo, "output": output,
                                    "phase": "prepared", "settled": True})
        row = call(cli, ["terminal", "show", "--terminal", handle], "role-tab-show")["terminal"]
        if identity(row, repo) != expected:
            raise ValueError("settled role tab changed before closure")
        receipt = call(cli, ["terminal", "close", "--terminal", handle], "role-tab-close")
        storage.write_ledger(path, {"identity": expected, "repo": repo, "output": output,
                                   "phase": "close-returned", "settled": True, "receipt": receipt})
        rows, _ = inventory(call, cli, repo)
        if any(item.get("handle") == handle for item in rows):
            raise ValueError("settled tab closure unconfirmed")
        closed = receipt.get("close", {})
        if closed.get("handle") != handle or closed.get("ptyKilled") is not True:
            raise ValueError("settled tab PTY closure unconfirmed")
        return path


def complete_output(call, cli, handle: str) -> dict:
    """Preserve a capped preview or all retained completed lines by cursor."""
    preview = call(cli, ["terminal", "read", "--terminal", handle, "--limit", "2000"],
                   "role-tab-read")["terminal"]
    if preview.get("handle") != handle or type(preview.get("truncated")) is not bool:
        raise ValueError("scrollback preservation incomplete; do not close")
    if preview.get("limited") is False and preview.get("truncated") is False:
        return preview
    if (not str(preview.get("oldestCursor", "")).isdecimal()
            or not str(preview.get("latestCursor", "")).isdecimal()):
        raise ValueError("scrollback preservation incomplete; do not close")
    oldest = int(preview["oldestCursor"])
    latest = int(preview["latestCursor"])
    if oldest > latest or (preview['truncated'] and oldest == 0):
        raise ValueError("scrollback preservation incomplete; do not close")
    cursor = oldest
    lines: list[str] = []
    for _ in range(20):
        if cursor >= latest:
            break
        page = call(cli, ["terminal", "read", "--terminal", handle,
                          "--cursor", str(cursor), "--limit", "1000"], "role-tab-read")["terminal"]
        if (page.get("handle") != handle or page.get("truncated") is not False
                or page.get("oldestCursor", str(oldest)) != str(oldest)
                or page.get("latestCursor") != str(latest)
                or not isinstance(page.get("tail"), list)
                or page.get("returnedLineCount") != len(page["tail"])
                or not str(page.get("nextCursor", "")).isdecimal()):
            raise ValueError("scrollback preservation incomplete; do not close")
        next_cursor = int(page["nextCursor"])
        if next_cursor <= cursor or next_cursor > latest or next_cursor - cursor != len(page['tail']):
            raise ValueError("scrollback preservation incomplete; do not close")
        lines.extend(page["tail"])
        cursor = next_cursor
    if cursor != latest:
        raise ValueError("scrollback preservation exceeds the bounded archive; do not close")
    confirmed = call(cli, ["terminal", "read", "--terminal", handle, "--limit", "2000"],
                     "role-tab-read")["terminal"]
    if (confirmed.get("handle") != handle or confirmed.get("truncated") != preview['truncated']
            or confirmed.get("oldestCursor") != str(oldest)
            or confirmed.get("latestCursor") != str(latest)):
        raise ValueError("scrollback changed during preservation; do not close")
    return {"handle": handle, "tail": lines, "limited": False, "truncated": oldest > 0,
            "retainedRangeComplete": True, "droppedBeforeCursor": str(oldest),
            "oldestCursor": str(oldest), "nextCursor": str(latest), "latestCursor": str(latest),
            "returnedLineCount": len(lines), "preview": confirmed}


def launch(call, cli, request: str, repo: str, slot: str, command: str,
           *, closed_receipt: dict | None = None) -> dict:
    path = registry_path(request, repo, slot)
    with acquire_host(workspace_lock(repo), inherit=False):
        current = storage.read_private_json(path, {})
        if current and (current.get("request") != request or current.get("repo") != repo
                        or current.get("slot") != slot or current.get("phase") != "known"):
            raise ValueError("role tab mutation unresolved; reconcile without creating another tab")
        rows, layouts = inventory(call, cli, repo)
        row = None
        if current:
            matches = [item for item in rows if item.get("handle") == current["identity"]["handle"]]
            if len(matches) != 1:
                closed = (closed_receipt or {}).get("result", {}).get("close", {})
                if (matches or (closed_receipt or {}).get("ok") is not True
                        or closed.get("handle") != current["identity"]["handle"]
                        or closed.get("ptyKilled") is not True):
                    raise ValueError("registered role tab missing; explicit reconciliation required")
            else:
                row = matches[0]
                if identity(row, repo) != current["identity"]:
                    raise ValueError("role tab incarnation changed; do not reuse")
        else:
            # Old unmanaged role tabs require explicit migration, not a new copy.
            if any(TITLES[slot] in str(layout) for layout in layouts):
                raise ValueError("unregistered role tab exists; reconcile before creating another")
            row = default_shell(rows, layouts)
            if row is not None:
                marker = storage.read_private_json(default_path(repo, row["handle"]), {})
                if marker != {"identity": identity(row, repo), "repo": repo, "purpose": "shell"}:
                    row = None
        if row is not None:
            owned = identity(row, repo)
            idle_shell(owned["handle"], repo)
        else:
            owned = None
        state = {"schema": 1, "request": request, "repo": repo, "slot": slot,
                 "identity": owned, "phase": "unknown", "launch_id": str(uuid.uuid4())}
        storage.write_ledger(path, state)
        title = TITLES[slot] + "・起動中"
        if owned is None:
            result = call(cli, ["terminal", "create", "--worktree", f"path:{repo}",
                               "--title", title, "--command", command], "terminal-create")
            handle = result.get("terminal", {}).get("handle")
            row = call(cli, ["terminal", "show", "--terminal", handle], "role-tab-show").get("terminal", {})
            owned = identity(row, repo)
            if owned["handle"] != handle:
                raise ValueError("created role tab identity mismatch")
        else:
            call(cli, ["terminal", "rename", "--terminal", owned["handle"], "--title", title], "role-tab-rename")
            shown = call(cli, ["terminal", "show", "--terminal", owned["handle"]], "role-tab-show")["terminal"]
            if identity(shown, repo) != owned:
                raise ValueError("role tab changed before launch")
            idle_shell(owned["handle"], repo)
            sent = call(cli, ["terminal", "send", "--terminal", owned["handle"],
                              "--text", command, "--enter"], "role-tab-send").get("send", {})
            if sent.get("accepted") is not True or sent.get("handle") != owned["handle"]:
                raise ValueError("role tab launch input unknown; do not resend")
        storage.write_ledger(path, {**state, "identity": owned, "phase": "known"})
        return {**owned, "launch_id": state["launch_id"]}


def label(call, cli, attempt: dict, status: str) -> None:
    """Never rename legacy/unregistered or subsequently reused terminals."""
    if not all(attempt.get(k) for k in ("request_id", "repo", "slot", "terminal")):
        return
    path = registry_path(attempt["request_id"], attempt["repo"], attempt["slot"])
    with acquire_host(workspace_lock(attempt["repo"]), inherit=False):
        state = storage.read_private_json(path, {})
        if (state.get("phase") != "known" or not attempt.get("tab_launch_id")
                or state.get("launch_id") != attempt["tab_launch_id"]
                or state.get("identity", {}).get("handle") != attempt["terminal"]):
            return
        row = call(cli, ["terminal", "show", "--terminal", attempt["terminal"]], "role-tab-show")["terminal"]
        if identity(row, attempt["repo"]) != state["identity"]:
            raise ValueError("role tab incarnation changed before status update")
        call(cli, ["terminal", "rename", "--terminal", attempt["terminal"],
                   "--title", TITLES[attempt["slot"]] + "・" + status], "role-tab-rename")


def main() -> int:
    """Coordinator maintenance only; never ask operators to enter these IDs."""
    if __package__:
        from . import orca_dispatch as dispatch
    else:
        import orca_dispatch as dispatch
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("adopt", "retire"))
    parser.add_argument("--repo", required=True)
    parser.add_argument("--terminal", required=True)
    parser.add_argument("--incarnation", required=True)
    parser.add_argument("--worktree-id", required=True)
    parser.add_argument("--request")
    parser.add_argument("--slot", choices=tuple(TITLES))
    args = parser.parse_args()
    expected = {"handle": args.terminal, "incarnationId": args.incarnation, "worktreeId": args.worktree_id}
    try:
        cli = dispatch.intake.default_orca_cli()
        if args.action == "adopt":
            if not args.request or not args.slot:
                raise ValueError("adoption requires request and role")
            adopt(dispatch.run_cli, cli, args.request, args.repo, args.slot, expected)
            print(json.dumps({"adopted": args.terminal}))
        else:
            path = retire(dispatch.run_cli, cli, args.repo, expected)
            print(json.dumps({"retired": args.terminal, "receipt": str(path)}))
        return 0
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Role tab maintenance refused: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
