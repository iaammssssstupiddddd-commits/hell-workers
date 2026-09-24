"""Durable Linear/GitHub sync intents; external events never drive the case directly."""

from __future__ import annotations

import hashlib
import json
import re
import uuid
from pathlib import Path

if __package__:
    from . import orca_frontdesk as storage, orca_issue_context as intake
    from .host_coordination import acquire_host
else:
    import orca_frontdesk as storage
    import orca_issue_context as intake
    from host_coordination import acquire_host


SCHEMA = 1
NAMESPACE = uuid.UUID("6598f23d-5741-5aaf-988f-a13d7f9c6559")
PHASES = {"queued", "sending", "confirmed", "unknown", "blocked_authority"}
LINEAR_STAGES = {"started": 1, "review": 2, "completed": 3, "stopped": 4}
SHA = re.compile(r"[0-9a-f]{40}")


def root(state_dir: Path) -> Path:
    if not state_dir.is_absolute() or state_dir.resolve() != state_dir:
        raise ValueError("external sync root must be absolute and normalized")
    return storage.checked_directory(state_dir / "external-sync")


def path(state_dir: Path, request_id: str) -> Path:
    intake.canonical_uuid(request_id, "request id")
    return root(state_dir) / f"{request_id}.json"


def empty(request_id: str) -> dict:
    return {"schema": SCHEMA, "requestId": request_id, "sequence": 0,
            "operations": [], "proposals": []}


def payload_digest(payload: dict) -> str:
    return hashlib.sha256(json.dumps(
        payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()).hexdigest()


def validate(data: dict, request_id: str) -> dict:
    if (not isinstance(data, dict) or set(data) != {"schema", "requestId", "sequence", "operations", "proposals"}
            or data.get("schema") != SCHEMA or data.get("requestId") != request_id
            or type(data.get("sequence")) is not int or data["sequence"] < 0
            or not isinstance(data.get("operations"), list) or len(data["operations"]) > 100
            or not isinstance(data.get("proposals"), list) or len(data["proposals"]) > 100):
        raise ValueError("invalid external sync ledger; preserve for reconciliation")
    seen: set[str] = set()
    for index, operation in enumerate(data["operations"], start=1):
        required = {"id", "sequence", "kind", "payload", "payloadSha256", "phase", "result"}
        if (not isinstance(operation, dict) or set(operation) != required
                or operation.get("sequence") != index or operation.get("phase") not in PHASES
                or not isinstance(operation.get("payload"), dict)
                or operation.get("payloadSha256") != payload_digest(operation["payload"])
                or operation.get("kind") not in {"linear_status", "linear_comment", "github_draft_pr"}
                or operation.get("id") in seen):
            raise ValueError("invalid external sync operation")
        intake.canonical_uuid(operation["id"], "external operation id")
        seen.add(operation["id"])
    if data["sequence"] != len(data["operations"]):
        raise ValueError("external sync sequence is discontinuous")
    for proposal in data["proposals"]:
        if (not isinstance(proposal, dict)
                or set(proposal) != {"id", "source", "event", "payloadSha256", "phase"}
                or proposal.get("source") not in {"linear", "github"}
                or proposal.get("phase") != "proposed"
                or not isinstance(proposal.get("event"), str)
                or not re.fullmatch(r"[0-9a-f]{64}", proposal.get("payloadSha256", ""))):
            raise ValueError("invalid untrusted external proposal")
        intake.canonical_uuid(proposal["id"], "external proposal id")
    return data


def read(state_dir: Path, request_id: str) -> dict:
    data = storage.read_private_json(path(state_dir, request_id), {})
    return validate(data, request_id) if data else empty(request_id)


def save(state_dir: Path, data: dict) -> None:
    validate(data, data["requestId"])
    storage.write_ledger(path(state_dir, data["requestId"]), data)


def enqueue(state_dir: Path, request_id: str, kind: str, payload: dict,
            *, authorized: bool = True) -> dict:
    if kind not in {"linear_status", "linear_comment", "github_draft_pr"}:
        raise ValueError("unsupported external sync kind")
    digest = payload_digest(payload)
    operation_id = str(uuid.uuid5(NAMESPACE, f"{request_id}\n{kind}\n{digest}"))
    with acquire_host("external-sync", inherit=False):
        data = read(state_dir, request_id)
        matches = [item for item in data["operations"] if item["id"] == operation_id]
        if matches:
            return matches[0]
        operation = {"id": operation_id, "sequence": data["sequence"] + 1, "kind": kind,
                     "payload": payload, "payloadSha256": digest,
                     "phase": "queued" if authorized else "blocked_authority", "result": None}
        data["operations"].append(operation)
        data["sequence"] += 1
        save(state_dir, data)
        return operation


def queue_linear_status(state_dir: Path, request_id: str, issue: str, stage: str,
                        summary: str) -> dict:
    if stage not in LINEAR_STAGES or not isinstance(issue, str) or not issue:
        raise ValueError("invalid Linear sync target")
    current = read(state_dir, request_id)
    prior = [item for item in current["operations"] if item["kind"] == "linear_status"
             and item["phase"] != "unknown"]
    if prior and LINEAR_STAGES[prior[-1]["payload"]["stage"]] > LINEAR_STAGES[stage]:
        raise ValueError("stale Linear status cannot overwrite a newer case stage")
    return enqueue(state_dir, request_id, "linear_status",
                   {"issue": issue, "stage": stage, "summary": summary})


def queue_github_draft(state_dir: Path, request_id: str, *, branch: str, base: str,
                       head: str, tested_sha: str, review_sha: str,
                       publication_authorized: bool) -> dict:
    for value, label in ((base, "base"), (head, "head"), (tested_sha, "tested SHA"),
                         (review_sha, "review SHA")):
        if not isinstance(value, str) or not SHA.fullmatch(value):
            raise ValueError(f"invalid {label}")
    if head != tested_sha or head != review_sha:
        raise ValueError("Draft PR gate requires the same tested and fixed-reviewed head")
    if not isinstance(branch, str) or not branch.strip():
        raise ValueError("invalid publication branch")
    return enqueue(
        state_dir, request_id, "github_draft_pr",
        {"branch": branch, "base": base, "head": head, "testedSha": tested_sha,
         "reviewSha": review_sha}, authorized=publication_authorized,
    )


def transition(state_dir: Path, request_id: str, operation_id: str, phase: str,
               result: dict | None = None) -> dict:
    intake.canonical_uuid(operation_id, "external operation id")
    if phase not in {"sending", "confirmed", "unknown"}:
        raise ValueError("invalid external operation transition")
    with acquire_host("external-sync", inherit=False):
        data = read(state_dir, request_id)
        matches = [item for item in data["operations"] if item["id"] == operation_id]
        if len(matches) != 1:
            raise ValueError("external operation is not unique")
        operation = matches[0]
        allowed = {"queued": {"sending"}, "sending": {"confirmed", "unknown"},
                   "confirmed": {"confirmed"}, "unknown": {"unknown"},
                   "blocked_authority": set()}
        if phase not in allowed[operation["phase"]]:
            raise ValueError("external operation transition would replay or bypass authority")
        if operation["phase"] == phase:
            if operation["result"] != result:
                raise ValueError("external operation result changed")
            return operation
        operation.update(phase=phase, result=result)
        save(state_dir, data)
        return operation


def propose(state_dir: Path, request_id: str, source: str, event: str, payload: dict) -> dict:
    if source not in {"linear", "github"} or not isinstance(event, str) or not event.strip():
        raise ValueError("invalid external event proposal")
    digest = payload_digest(payload)
    proposal_id = str(uuid.uuid5(NAMESPACE, f"proposal\n{request_id}\n{source}\n{event}\n{digest}"))
    with acquire_host("external-sync", inherit=False):
        data = read(state_dir, request_id)
        matches = [item for item in data["proposals"] if item["id"] == proposal_id]
        if matches:
            return matches[0]
        proposal = {"id": proposal_id, "source": source, "event": event,
                    "payloadSha256": digest, "phase": "proposed"}
        data["proposals"].append(proposal)
        save(state_dir, data)
        return proposal


def projection(state_dir: Path, request_id: str) -> dict:
    data = read(state_dir, request_id)
    if not data["operations"]:
        return {"state": "not_required", "detail": "外部同期対象はありません"}
    phases = {item["phase"] for item in data["operations"]}
    if "unknown" in phases:
        return {"state": "unknown", "detail": "外部書き込み結果を再照合中です"}
    if "blocked_authority" in phases:
        return {"state": "blocked_authority", "detail": "公開許可待ち。ローカル成果は保全済みです"}
    if "sending" in phases:
        return {"state": "syncing", "detail": "外部サービスへ同期中です"}
    if "queued" in phases:
        return {"state": "pending", "detail": "外部同期キュー待ちです"}
    return {"state": "synced", "detail": "外部同期を確認済みです"}
