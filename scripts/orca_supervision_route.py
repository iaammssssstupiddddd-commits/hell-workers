"""Fail-closed, one-issue/one-worktree routing for an explicit UI implementation request.

This is a project-owned journal. It does not make a consultation into an
implementation request and never retries an uncertain external write.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import uuid
from pathlib import Path

if __package__:
    from . import orca_frontdesk as desk, orca_issue_context as intake
    from . import orca_ui_coordinator as ui
    from .host_coordination import acquire_host
else:
    import orca_frontdesk as desk
    import orca_issue_context as intake
    import orca_ui_coordinator as ui
    from host_coordination import acquire_host


ISSUE = re.compile(r"[A-Z][A-Z0-9_-]{0,63}-[1-9][0-9]*")


def route_path(root: Path, request_id: str) -> Path:
    intake.canonical_uuid(request_id, "request id")
    return desk.checked_directory(root / "routes") / f"{request_id}.json"


def request(request_id: str) -> dict:
    matches = [item for item in desk.list_requests() if item["id"] == request_id]
    if len(matches) != 1:
        raise ValueError("implementation intake is not unique")
    return matches[0]


def save(path: Path, data: dict) -> None:
    desk.write_ledger(path, data)


def checked_result(call, args: list[str], purpose: str, input_text: str | None = None) -> dict:
    code, response = call(args, input_text)
    if code != 0 or response.get("ok") is not True:
        raise RuntimeError(f"{purpose} is unconfirmed; reconcile before another write")
    result = response.get("result")
    meta = response.get("_meta")
    if not isinstance(result, dict) or not isinstance(meta, dict):
        raise ValueError(f"{purpose} response lacks a verified result")
    intake.canonical_uuid(meta.get("runtimeId"), "Orca runtime id")
    return result


def route(root: Path, request_id: str, primary: Path, call) -> dict:
    """Execute only known phases; unknown writes require separate read-back recovery."""
    if not primary.is_absolute() or primary.resolve() != primary:
        raise ValueError("primary repository path is unverifiable")
    item = request(request_id)
    receipt = desk.read_private_json(root / "receipts" / f"{request_id}.json", {})
    if (receipt.get("schema") != 1 or receipt.get("operationId") != request_id
            or receipt.get("intakeId") != request_id or receipt.get("action") != "implement"
            or receipt.get("phase") != "accepted" or receipt.get("text", "").strip() != item["request"]):
        raise ValueError("explicit UI implementation intent is not durable")
    path = route_path(root, request_id)
    digest = hashlib.sha256(item["request"].encode()).hexdigest()
    with acquire_host("frontdesk-route", inherit=False):
        data = desk.read_private_json(path, {})
        if data:
            if (data.get("schema") != 1 or data.get("requestId") != request_id
                    or data.get("requestSha256") != digest
                    or data.get("primary") != str(primary)):
                raise ValueError("route journal does not match this intake")
        else:
            teams = checked_result(call, ["linear", "team", "list"], "Linear team discovery").get("teams")
            if not isinstance(teams, list) or len(teams) != 1:
                raise ValueError("exactly one connected Linear team is required")
            team = teams[0]
            workspace = intake.canonical_uuid(team.get("workspace", {}).get("id"), "workspace id")
            key = team.get("key")
            if not isinstance(key, str) or not re.fullmatch(r"[A-Z][A-Z0-9_-]{0,63}", key):
                raise ValueError("Linear team key is invalid")
            repos = checked_result(call, ["repo", "list"], "Orca repository discovery").get("repos")
            matches = [repo for repo in repos if repo.get("path") == str(primary)] if isinstance(repos, list) else []
            if len(matches) != 1:
                raise ValueError("primary Orca repository is not unique")
            repo_id = intake.canonical_uuid(matches[0].get("id"), "Orca repository id")
            data = {"schema": 1, "requestId": request_id, "requestSha256": digest,
                    "primary": str(primary), "phase": "prepared", "team": key,
                    "workspaceId": workspace, "repoId": repo_id,
                    "linearWriteId": str(uuid.uuid4()), "issueId": None,
                    "issueIdentifier": None, "worktreeId": None, "childRequestId": None}
            save(path, data)
        phase = data["phase"]
        if phase == "prepared":
            data["phase"] = "issue_creating"
            save(path, data)
            title = item["request"].strip().splitlines()[0][:120]
            result = checked_result(call, ["linear", "create", "--title", title,
                "--body-file", "-", "--team", data["team"],
                "--workspace", data["workspaceId"], "--write-id", data["linearWriteId"]],
                "Linear issue creation", item["request"])
            issue = result.get("issue")
            if (not isinstance(issue, dict) or not ISSUE.fullmatch(issue.get("identifier", ""))
                    or issue.get("title") != title or issue.get("team", {}).get("key") != data["team"]
                    or result.get("meta", {}).get("workspaceId") != data["workspaceId"]
                    or result.get("meta", {}).get("writeId") != data["linearWriteId"]):
                raise ValueError("Linear create response does not match the route intent")
            data["issueId"] = intake.canonical_uuid(issue.get("id"), "issue id")
            data["issueIdentifier"] = issue["identifier"]
            data["phase"] = "issue_created"
            save(path, data)
            phase = data["phase"]
        if phase == "issue_creating":
            raise RuntimeError("Linear creation is unresolved; do not create another issue")
        if phase == "issue_created":
            data["phase"] = "worktree_creating"
            save(path, data)
            result = checked_result(call, ["worktree", "create", "--repo", f"id:{data['repoId']}",
                "--name", data["issueIdentifier"].lower(),
                "--linear-issue", data["issueIdentifier"], "--no-parent",
                "--setup", "skip"], "Orca worktree creation")
            worktree = result.get("worktree")
            if not isinstance(worktree, dict) or worktree.get("linkedLinearIssue") != data["issueIdentifier"]:
                raise ValueError("created worktree has no matching Linear link")
            worktree_id = worktree.get("id")
            if not isinstance(worktree_id, str) or not worktree_id.startswith(data["repoId"] + "::"):
                raise ValueError("created worktree identity is invalid")
            worktree_path = Path(worktree_id.partition("::")[2])
            if (not worktree_path.is_absolute() or worktree_path.resolve() != worktree_path
                    or worktree_path == primary or worktree_path.is_relative_to(primary)):
                raise ValueError("created worktree path is unsafe")
            data["worktreeId"] = worktree_id
            data["phase"] = "worktree_created"
            save(path, data)
            phase = data["phase"]
        if phase == "worktree_creating":
            raise RuntimeError("worktree creation is unresolved; do not create another checkout")
        if phase not in {"worktree_created", "terminal_starting", "ready"}:
            raise ValueError("unknown route phase; preserve journal")
        if phase == "worktree_created":
            child_id = data["childRequestId"]
            if child_id is None:
                imported = intake.import_issue(data["issueIdentifier"], data["workspaceId"])
                child_id = intake.canonical_uuid(imported["request_id"], "child request id")
                if child_id == request_id:
                    raise ValueError("implementation child intake collides with its parent")
                data["childRequestId"] = child_id
                save(path, data)
            else:
                intake.canonical_uuid(child_id, "child request id")
            # This durable boundary precedes terminal creation. If the Orca
            # response is lost, a later tick may only read back the result;
            # it must not create a second coordinator tab.
            data["phase"] = "terminal_starting"
            save(path, data)
            coordinator_path = ui.state_path(child_id)
            if not coordinator_path.exists() and not coordinator_path.is_symlink():
                ui.ensure_coordinator_terminal(data["worktreeId"], focus=False)
            phase = data["phase"]
        if phase == "terminal_starting":
            child_id = intake.canonical_uuid(data.get("childRequestId"), "child request id")
            coordinator_path = ui.state_path(child_id)
            if coordinator_path.exists() or coordinator_path.is_symlink():
                state = ui.read_registered_state(child_id)
                if (state["worktree_id"] != data["worktreeId"]
                        or state["linear_identifier"] != data["issueIdentifier"]):
                    raise ValueError("visible coordinator belongs to another route")
                if state["phase"] == "exited":
                    raise RuntimeError("coordinator exited before acknowledging the route")
                if state["phase"] == "ready":
                    data["phase"] = "ready"
                    save(path, data)
        return data


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-dir", type=Path, required=True)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--primary", type=Path, required=True)
    args = parser.parse_args()
    result = route(args.state_dir, args.request_id, args.primary, ui.run_orca_response)
    print(json.dumps({"requestId": result["requestId"], "phase": result["phase"]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
