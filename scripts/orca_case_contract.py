"""Versioned, read-only case projection shared by the controller and Orca UI."""

from __future__ import annotations

import re
from copy import deepcopy


SCHEMA = 2
SHA256 = re.compile(r"[0-9a-f]{64}")
STATES = {"queued", "ready", "working", "review", "feedback", "paused", "closing", "closed", "unknown"}
STAGES = {"intake", "planning", "implementation", "validation", "review", "integration",
          "external_sync", "feedback", "cleanup", "complete", "attention"}
SYNC_STATES = {"not_required", "pending", "syncing", "synced", "offline",
               "blocked_authority", "blocked_policy", "unknown"}
DISPOSITIONS = {"none", "continue_existing", "create_child", "create_standalone"}


def bounded(value: object, label: str, maximum: int = 2048, *, optional: bool = False) -> str | None:
    if value is None and optional:
        return None
    if not isinstance(value, str) or not value.strip() or len(value) > maximum:
        raise ValueError(f"invalid {label}")
    return value


def stage_for(state: str, detail: str) -> str:
    if state not in STATES:
        raise ValueError("unknown workflow state")
    if state in {"queued", "ready"}:
        return "planning" if "実装" in detail or "作業場" in detail else "intake"
    if state == "working":
        return "implementation"
    if state == "review":
        return "review"
    if state == "feedback":
        return "feedback"
    if state in {"paused", "unknown"}:
        return "attention"
    if state == "closing":
        return "cleanup"
    return "complete"


def next_action_for(state: str) -> str:
    return {
        "queued": "統括が準備を続けます",
        "ready": "自然文で相談または実装を依頼できます",
        "working": "担当完了後に統括検証へ進みます",
        "review": "固定レビュー結果を待ちます",
        "feedback": "結果を確認し、必要なら同じ案件へ追加依頼してください",
        "paused": "保全状態を確認して再開または終了を選べます",
        "closing": "終了照合と資源整理を続けます",
        "closed": "追加操作はありません",
        "unknown": "再照合で安全な再開条件を確認します",
    }[state]


def decision_view(decision: dict | None) -> dict | None:
    if not decision:
        return None
    disposition = decision.get("disposition")
    if disposition not in DISPOSITIONS or not SHA256.fullmatch(decision.get("sha256", "")):
        raise ValueError("invalid intake decision projection")
    issue = decision.get("existingIssue") or decision.get("parentIssue")
    identifier = issue.get("identifier") if isinstance(issue, dict) else None
    if identifier is not None:
        bounded(identifier, "Linear identifier", 128)
    return {
        "disposition": disposition,
        "rationale": bounded(decision.get("rationale"), "intake rationale"),
        "issueIdentifier": identifier,
        "sha256": decision["sha256"],
    }


def sync_view(sync: dict | None) -> dict:
    if not sync:
        return {"state": "not_required", "detail": "外部同期対象はありません"}
    state = sync.get("state")
    if state not in SYNC_STATES:
        raise ValueError("unknown external sync state")
    return {"state": state, "detail": bounded(sync.get("detail"), "external sync detail")}


def enrich(workflow: dict, *, observed_at: int, decision: dict | None = None,
           sync: dict | None = None) -> dict:
    if type(observed_at) is not int or observed_at < 0:
        raise ValueError("invalid workflow observation time")
    state = workflow.get("state")
    detail = bounded(workflow.get("detail"), "workflow detail")
    stage = stage_for(state, detail)
    waiting = detail if state in {"queued", "review", "paused", "unknown"} else None
    result = {
        **workflow,
        "stage": stage,
        "waitReason": waiting,
        "nextAction": next_action_for(state),
        "observedAt": observed_at,
        "intakeDecision": decision_view(decision),
        "externalSync": sync_view(sync),
    }
    validate_workflow(result)
    return result


def validate_workflow(value: dict) -> dict:
    required = {"id", "revision", "title", "kind", "state", "detail", "actions", "roles",
                "stage", "waitReason", "nextAction", "observedAt", "intakeDecision", "externalSync"}
    if not isinstance(value, dict) or set(value) != required or value.get("state") not in STATES:
        raise ValueError("invalid case workflow projection")
    if value.get("stage") not in STAGES:
        raise ValueError("invalid case stage")
    bounded(value.get("nextAction"), "next action")
    bounded(value.get("waitReason"), "wait reason", optional=True)
    if type(value.get("observedAt")) is not int or value["observedAt"] < 0:
        raise ValueError("invalid workflow observation time")
    decision_view(value.get("intakeDecision"))
    sync_view(value.get("externalSync"))
    return value


def snapshot(runtime_id: str, published_at: int, workflows: list[dict]) -> dict:
    if not isinstance(runtime_id, str) or not runtime_id or type(published_at) is not int or published_at < 0:
        raise ValueError("invalid case snapshot identity")
    if not isinstance(workflows, list) or len(workflows) > 100:
        raise ValueError("invalid case workflow count")
    for workflow in workflows:
        validate_workflow(workflow)
    if len({item["id"] for item in workflows}) != len(workflows):
        raise ValueError("duplicate case workflow identity")
    return {"schema": SCHEMA, "runtimeId": runtime_id, "publishedAt": published_at,
            "workflows": workflows}


def migrate_preview(value: dict) -> dict:
    """Upgrade a schema-1 UI snapshot in memory; never mutate its source ledger."""
    if not isinstance(value, dict):
        raise ValueError("invalid legacy snapshot")
    if value.get("schema") == SCHEMA:
        result = deepcopy(value)
        snapshot(result.get("runtimeId"), result.get("publishedAt"), result.get("workflows"))
        return result
    if value.get("schema") != 1 or set(value) != {"schema", "runtimeId", "publishedAt", "workflows"}:
        raise ValueError("unsupported case snapshot schema; use read-only rollback")
    observed = value.get("publishedAt")
    upgraded = [enrich(deepcopy(item), observed_at=observed) for item in value.get("workflows", [])]
    return snapshot(value.get("runtimeId"), observed, upgraded)
