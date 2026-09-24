"""Durable coordinator-owned Linear intake decisions.

The operator never has to pre-create or select a Linear issue.  The coordinator
records exactly one policy decision for an immutable intake before any external
write.  External adapters consume this receipt but cannot reinterpret it.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import uuid
from datetime import datetime, timezone
from pathlib import Path

if __package__:
    from . import orca_frontdesk as desk, orca_issue_context as intake
    from .host_coordination import acquire_host
else:
    import orca_frontdesk as desk
    import orca_issue_context as intake
    from host_coordination import acquire_host


SCHEMA = 1
DISPOSITIONS = {"none", "continue_existing", "create_child", "create_standalone"}
ISSUE = re.compile(r"[A-Z][A-Z0-9_-]{0,63}-[1-9][0-9]*")
WRITE_NAMESPACE = uuid.UUID("c90a9c8f-c27b-5ecb-8462-070fc478e600")


def decision_root(root: Path) -> Path:
    if not root.is_absolute() or root.resolve() != root:
        raise ValueError("intake decision root must be an absolute normalized path")
    return desk.checked_directory(root / "intake-decisions")


def decision_path(root: Path, request_id: str) -> Path:
    intake.canonical_uuid(request_id, "request id")
    return decision_root(root) / f"{request_id}.json"


def request_sha256(request_id: str) -> str:
    matches = [item for item in desk.list_requests() if item["id"] == request_id]
    if len(matches) != 1:
        raise ValueError("intake request is not unique")
    return hashlib.sha256(matches[0]["request"].encode("utf-8")).hexdigest()


def checked_issue(value: object, label: str) -> dict | None:
    if value is None:
        return None
    if not isinstance(value, dict) or set(value) != {"workspaceId", "issueId", "identifier"}:
        raise ValueError(f"invalid {label}")
    intake.canonical_uuid(value["workspaceId"], f"{label} workspace id")
    intake.canonical_uuid(value["issueId"], f"{label} issue id")
    if not isinstance(value["identifier"], str) or not ISSUE.fullmatch(value["identifier"]):
        raise ValueError(f"invalid {label} identifier")
    return value


def content(data: dict) -> dict:
    return {key: value for key, value in data.items() if key != "sha256"}


def digest(data: dict) -> str:
    return hashlib.sha256(
        json.dumps(content(data), ensure_ascii=False, sort_keys=True, separators=(",", ":"),
                   allow_nan=False).encode("utf-8")
    ).hexdigest()


def validate(data: dict, request_id: str, expected_request_sha256: str | None = None) -> dict:
    required = {
        "schema", "requestId", "requestSha256", "disposition", "rationale", "decidedBy",
        "existingIssue", "parentIssue", "writeId", "createdAt", "sha256",
    }
    if not isinstance(data, dict) or set(data) != required or data.get("schema") != SCHEMA:
        raise ValueError("invalid intake decision; preserve it for reconciliation")
    if data.get("requestId") != request_id:
        raise ValueError("intake decision belongs to another request")
    intake.canonical_uuid(request_id, "request id")
    if (not isinstance(data.get("requestSha256"), str)
            or not re.fullmatch(r"[0-9a-f]{64}", data["requestSha256"])
            or (expected_request_sha256 is not None
                and data["requestSha256"] != expected_request_sha256)):
        raise ValueError("intake decision request digest changed")
    disposition = data.get("disposition")
    if disposition not in DISPOSITIONS:
        raise ValueError("unknown intake decision disposition")
    if (not isinstance(data.get("rationale"), str) or not data["rationale"].strip()
            or len(data["rationale"]) > 2048
            or not isinstance(data.get("decidedBy"), str) or not data["decidedBy"].strip()
            or len(data["decidedBy"]) > 256):
        raise ValueError("intake decision requires bounded coordinator rationale")
    existing = checked_issue(data.get("existingIssue"), "existing issue")
    parent = checked_issue(data.get("parentIssue"), "parent issue")
    if disposition == "continue_existing" and (existing is None or parent is not None):
        raise ValueError("existing continuation requires exactly one existing issue")
    if disposition == "create_child" and (parent is None or existing is not None):
        raise ValueError("child creation requires exactly one parent issue")
    if disposition in {"none", "create_standalone"} and (existing is not None or parent is not None):
        raise ValueError("standalone/no-issue decision cannot bind another issue")
    write_id = data.get("writeId")
    if disposition in {"create_child", "create_standalone"}:
        intake.canonical_uuid(write_id, "Linear write id")
        expected_write = str(uuid.uuid5(
            WRITE_NAMESPACE,
            f"{request_id}\n{data['requestSha256']}\n{disposition}\n"
            f"{parent['issueId'] if parent else ''}",
        ))
        if write_id != expected_write:
            raise ValueError("Linear write id is not bound to this decision")
    elif write_id is not None:
        raise ValueError("non-creating decision cannot carry a Linear write id")
    try:
        created = datetime.fromisoformat(data["createdAt"])
    except (TypeError, ValueError) as error:
        raise ValueError("invalid intake decision timestamp") from error
    if created.tzinfo is None or data.get("sha256") != digest(data):
        raise ValueError("intake decision digest or timestamp is invalid")
    return data


def read(root: Path, request_id: str) -> dict:
    expected = request_sha256(request_id)
    data = desk.read_private_json(decision_path(root, request_id), {})
    if not data:
        return {}
    return validate(data, request_id, expected)


def record(
    root: Path,
    request_id: str,
    disposition: str,
    rationale: str,
    decided_by: str,
    *,
    existing_issue: dict | None = None,
    parent_issue: dict | None = None,
) -> dict:
    request_digest = request_sha256(request_id)
    if disposition not in DISPOSITIONS:
        raise ValueError("unknown intake decision disposition")
    write_id = None
    if disposition in {"create_child", "create_standalone"}:
        parent = checked_issue(parent_issue, "parent issue")
        write_id = str(uuid.uuid5(
            WRITE_NAMESPACE,
            f"{request_id}\n{request_digest}\n{disposition}\n"
            f"{parent['issueId'] if parent else ''}",
        ))
    data = {
        "schema": SCHEMA,
        "requestId": request_id,
        "requestSha256": request_digest,
        "disposition": disposition,
        "rationale": rationale.strip(),
        "decidedBy": decided_by.strip(),
        "existingIssue": existing_issue,
        "parentIssue": parent_issue,
        "writeId": write_id,
        "createdAt": datetime.now(timezone.utc).isoformat(),
    }
    data["sha256"] = digest(data)
    validate(data, request_id, request_digest)
    with acquire_host("intake-decision", inherit=False):
        path = decision_path(root, request_id)
        existing = desk.read_private_json(path, {})
        if existing:
            current = validate(existing, request_id, request_digest)
            comparable = {key: value for key, value in data.items() if key != "createdAt"}
            old = {key: value for key, value in current.items() if key != "createdAt"}
            # sha256 contains createdAt, so compare the semantic fields directly.
            comparable.pop("sha256")
            old.pop("sha256")
            if old != comparable:
                raise ValueError("intake decision is immutable; reconcile instead of replacing it")
            desk.sync_directory(path.parent)
            return current
        desk.write_ledger(path, data)
    return data


def fixed_reception_decision(root: Path, request_id: str, action: str) -> dict:
    """Apply the fixed reception coordinator policy before any route write."""
    if action == "submit":
        return record(
            root, request_id, "none",
            "相談・調査の受付であり、実装課題やworktreeを作成しない",
            "fixed-reception-coordinator-policy-v1",
        )
    if action == "implement":
        return record(
            root, request_id, "create_standalone",
            "固定受付からの明示実装依頼で、継続可能なlinked issueがないため独立課題にする",
            "fixed-reception-coordinator-policy-v1",
        )
    raise ValueError("unsupported fixed reception action")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("show", "decide"))
    parser.add_argument("--state-dir", type=Path, required=True)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--disposition", choices=sorted(DISPOSITIONS))
    parser.add_argument("--rationale")
    parser.add_argument("--decided-by")
    parser.add_argument("--existing-issue", type=Path)
    parser.add_argument("--parent-issue", type=Path)
    args = parser.parse_args()
    if args.action == "show":
        result = read(args.state_dir, args.request_id)
    else:
        if not args.disposition or not args.rationale or not args.decided_by:
            raise ValueError("decide requires disposition, rationale, and decided-by")
        def load(path: Path | None) -> dict | None:
            return json.loads(path.read_text(encoding="utf-8")) if path else None

        result = record(
            args.state_dir, args.request_id, args.disposition, args.rationale, args.decided_by,
            existing_issue=load(args.existing_issue), parent_issue=load(args.parent_issue),
        )
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
