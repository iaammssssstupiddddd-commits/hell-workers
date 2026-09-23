"""Project-owned bridge between Orca's supervision panel and guarded intake."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path

if __package__:
    from . import orca_coordinator as coordinator, orca_frontdesk as frontdesk, orca_issue_context as intake
    from . import orca_role_tabs as role_tabs, orca_review_loop as review_loop
    from . import host_coordination, orca_supervision_route as routing, orca_ui_coordinator as ui
    from .host_coordination import acquire_host
else:
    import orca_coordinator as coordinator
    import orca_frontdesk as frontdesk
    import orca_issue_context as intake
    import orca_role_tabs as role_tabs
    import orca_review_loop as review_loop
    import host_coordination
    import orca_supervision_route as routing
    import orca_ui_coordinator as ui
    from host_coordination import acquire_host


RECEPTION_ID = str(uuid.uuid5(uuid.UUID("97673e6a-1227-4dc1-93c4-b4b79d4559aa"),
                              "hell-workers-reception"))
MAX_DOCUMENT_BYTES = 256 * 1024
REQUEST_KEYS = {"schema", "runtimeId", "expectedRuntimeId", "operationId",
                "workflowId", "revision", "action", "text"}
CONSULT_PHASES = {"ready", "launching", "settled", "unknown"}
LIFECYCLE_PHASES = {"active", "paused", "closing", "closed", "unknown"}
TEST_ISOLATED = False


class RuntimeChangedError(RuntimeError):
    """The controller must stop rather than bind commands to another Orca boot."""


def private_root(path: Path) -> Path:
    if not path.is_absolute() or path.resolve() != path:
        raise ValueError("supervision path must be absolute and contain no symlinks")
    return frontdesk.checked_directory(path)


def configure_test_state(panel_root: Path) -> None:
    """Keep hidden Electron acceptance entirely away from real host ledgers."""
    global TEST_ISOLATED
    override = os.environ.get("ORCA_SUPERVISION_TEST_STATE_ROOT")
    if not override:
        return
    root = Path(override)
    if (os.environ.get("ORCA_E2E_HEADLESS") != "1" or not root.is_absolute()
            or not root.is_relative_to(panel_root) or root == panel_root):
        raise RuntimeError("isolated supervision test state is not proven")
    private_root(root)
    def isolated() -> Path:
        return root / "coordination"
    host_coordination.state_root = isolated
    frontdesk.state_root = isolated
    TEST_ISOLATED = True


def directories(path: Path) -> tuple[Path, Path, Path]:
    root = private_root(path)
    return (root, frontdesk.checked_directory(root / "requests"),
            frontdesk.checked_directory(root / "receipts"))


def consult_dir(path: Path) -> Path:
    return frontdesk.checked_directory(private_root(path) / "consult-intents")


def consult_path(root: Path, request_id: str) -> Path:
    return consult_dir(root) / f"{canonical_uuid(request_id, 'request ID')}.json"


def consult_intent(root: Path, request_id: str) -> dict:
    path = consult_path(root, request_id)
    data = frontdesk.read_private_json(path, {})
    if data and (not isinstance(data, dict) or data.get("schema") != 1
                 or data.get("requestId") != request_id
                 or data.get("phase") not in CONSULT_PHASES):
        raise ValueError("invalid consultation intent; preserve for reconciliation")
    return data


def prepare_consult(root: Path, request_id: str) -> None:
    if consult_intent(root, request_id):
        return
    frontdesk.write_ledger(consult_path(root, request_id), {
        "schema": 1, "requestId": request_id, "phase": "ready"})


def canonical_uuid(value: object, name: str) -> str:
    try:
        parsed = uuid.UUID(value)
    except (TypeError, AttributeError, ValueError) as error:
        raise ValueError(f"invalid {name}") from error
    if str(parsed) != value:
        raise ValueError(f"invalid {name}")
    return value


def runtime_id(orca_cli: Path | None = None) -> str:
    executable = intake.checked_orca_cli(orca_cli or intake.default_orca_cli())
    try:
        completed = subprocess.run(
            [str(executable), "status", "--json"], capture_output=True,
            text=True, stdin=subprocess.DEVNULL, timeout=10, check=False,
        )
    except (OSError, UnicodeError, subprocess.TimeoutExpired) as error:
        raise RuntimeError("Orca runtime status is unavailable") from error
    if completed.returncode or completed.stderr.strip():
        raise RuntimeError("Orca runtime status is unverified")
    response = intake.decode_response(completed.stdout)
    result = response.get("result")
    runtime = result.get("runtime") if isinstance(result, dict) else None
    observed = runtime.get("runtimeId") if isinstance(runtime, dict) else None
    envelope = response.get("_meta")
    if (response.get("ok") is not True or not isinstance(runtime, dict)
            or runtime.get("reachable") is not True or runtime.get("state") != "ready"
            or not isinstance(envelope, dict) or envelope.get("runtimeId") != observed):
        raise RuntimeError("Orca runtime is not ready or changed during status read")
    return canonical_uuid(observed, "runtime ID")


def roles() -> list[dict]:
    return [{"role": role, "state": "unassigned", "detail": "未起動", "terminal": None}
            for role in ("coordinator", "worker-a", "worker-b", "reviewer")]


def reception() -> dict:
    return {"id": RECEPTION_ID, "revision": "1", "title": "受付・統括",
            "kind": "reception", "state": "ready",
            "detail": "依頼を受け付けます。受付後の状態を案件一覧で確認できます。",
            "actions": ["submit", "implement"], "roles": roles()}


def consultation_view(root: Path, request_id: str) -> tuple[str, str, list[dict]]:
    intent = consult_intent(root, request_id)
    if not intent:
        return ("queued", "受付済み。統括の相談はまだ開始していません。", roles())
    if intent["phase"] == "ready":
        return ("queued", "受付済み。統括への相談を開始します。", roles())
    if intent["phase"] == "unknown":
        return ("unknown", "統括の起動または終了が未確定です。自動再送はしていません。", roles())
    state = coordinator.read_state(request_id)
    turns = state["turns"]
    role_views = roles()
    if intent["phase"] == "launching":
        if turns and turns[-1]["phase"] in {"failed", "unknown"}:
            return ("unknown", "統括の相談結果を確定できません。自動再送はしていません。", role_views)
        role_views[0].update(state="running", detail="読取専用で相談中。実装・配車は未実行")
        return ("working", "統括へ相談中。実装・配車はまだ開始していません。", role_views)
    if turns and turns[-1]["phase"] == "succeeded":
        role_views[0].update(state="waiting", detail="相談回答済み。実装・配車は未実行")
        response = turns[-1]["response"].strip()
        return ("feedback", response[:2048], role_views)
    return ("unknown", "統括の相談が未確定です。自動再送はしていません。", role_views)


def route_record(root: Path, request_id: str) -> dict:
    data = frontdesk.read_private_json(routing.route_path(root, request_id), {})
    if data and (not isinstance(data, dict) or data.get("schema") != 1
                 or data.get("requestId") != request_id
                 or data.get("phase") not in {"prepared", "issue_creating", "issue_created",
                                               "worktree_creating", "worktree_created",
                                               "terminal_starting", "ready"}):
        raise ValueError("invalid implementation route; preserve for reconciliation")
    return data


def lifecycle_path(root: Path, request_id: str) -> Path:
    return frontdesk.checked_directory(private_root(root) / "lifecycle") / f"{canonical_uuid(request_id, 'workflow ID')}.json"


def lifecycle(root: Path, request_id: str) -> dict:
    data = frontdesk.read_private_json(lifecycle_path(root, request_id), {})
    if data and (not isinstance(data, dict) or data.get("schema") != 1
                 or data.get("requestId") != request_id
                 or data.get("phase") not in LIFECYCLE_PHASES
                 or not isinstance(data.get("revision"), int)
                 or data["revision"] < 2
                 or not isinstance(data.get("operationId"), str)
                 or data.get("outcome") not in {"accepted", "rejected"}
                 or (data.get("message") is not None and not isinstance(data["message"], str))):
        raise ValueError("invalid workflow lifecycle; preserve for reconciliation")
    if data:
        canonical_uuid(data["operationId"], "lifecycle operation ID")
        if "closeTargets" in data:
            checked_close_targets(data["closeTargets"])
    return data


def checked_close_targets(targets: object) -> list[dict]:
    if not isinstance(targets, list) or len(targets) > 20:
        raise ValueError("close target journal is invalid")
    seen: set[tuple[str, str]] = set()
    for target in targets:
        if not isinstance(target, dict) or set(target) != {"repo", "role", "identity"}:
            raise ValueError("close target journal is invalid")
        repo, role, identity = target["repo"], target["role"], target["identity"]
        if (not isinstance(repo, str) or not Path(repo).is_absolute()
                or Path(repo).resolve() != Path(repo) or role not in role_tabs.TITLES
                or not isinstance(identity, dict)
                or set(identity) != {"handle", "incarnationId", "worktreeId"}
                or not all(isinstance(value, str) and value for value in identity.values())
                or identity["worktreeId"].partition("::")[2] != repo
                or (repo, role) in seen):
            raise ValueError("close target journal is invalid")
        seen.add((repo, role))
    return targets


def task_revision(root: Path, request_id: str) -> str:
    current = lifecycle(root, request_id)
    receipt = frontdesk.read_private_json(root / "receipts" / f"{request_id}.json", {})
    material: dict = {"generation": current.get("revision", 1), "lifecycle": current}
    if receipt.get("action") == "implement":
        route = route_record(root, request_id)
        material["route"] = route
        child = route.get("childRequestId")
        if child:
            path = ui.state_path(child)
            if path.exists() or path.is_symlink():
                material["coordinator"] = ui.read_registered_state(child)
            loop_path = review_loop.state_path(child)
            if loop_path.exists() or loop_path.is_symlink():
                loop = review_loop.load(child)
                material["loop"] = {"phase": loop["phase"], "lanes": loop["lanes"],
                                    "attempts": loop.get("attempts", {})}
    else:
        material["consult"] = consult_intent(root, request_id)
    digest = hashlib.sha256(json.dumps(material, sort_keys=True, ensure_ascii=False).encode()).hexdigest()[:12]
    return f"{material['generation']}-{digest}"


def task_actions(root: Path, request_id: str, action: str) -> list[str]:
    state = lifecycle(root, request_id).get("phase")
    if state == "paused":
        return ["resume", "close"]
    if state in {"closing", "closed", "unknown"}:
        return []
    if action == "implement":
        route = route_record(root, request_id)
        if route.get("phase") != "ready":
            return []
        child = route.get("childRequestId")
        if child and review_loop.state_path(child).exists():
            phase = review_loop.load(child)["phase"]
            if phase != "approved":
                return []
        return ["pause", "close"]
    return ["close"] if consult_intent(root, request_id).get("phase") == "settled" else []


def lifecycle_view(root: Path, request_id: str, original: tuple[str, str, list[dict]]) -> tuple[str, str, list[dict]]:
    current = lifecycle(root, request_id)
    phase = current.get("phase")
    if not phase:
        return original
    _, _, role_views = original
    if phase == "active":
        if current.get("message"):
            return (original[0], current["message"], original[2])
        return original
    if phase == "paused":
        if original[0] == "unknown":
            return original
        for role in role_views:
            if role["state"] in {"running", "waiting"}:
                role["state"] = "paused"
        return ("paused", "中断中。作業場と会話・cacheを保持しています。", role_views)
    if phase == "closed":
        role_views[0].update(state="ended", detail="統括の処理は終了しました", terminal=None)
        return ("closed", "安全終了済み。成果と作業場は保持しています。", role_views)
    return ("unknown", current.get("message") or "終了の照合が未完了です。再起動・再送せず確認してください。", role_views)


def supervised_view(child_id: str, original: tuple[str, str, list[dict]]) -> tuple[str, str, list[dict]]:
    """Project the guarded loop without inventing tabs for unused roles."""
    if not review_loop.state_path(child_id).exists():
        return original
    data = review_loop.load(child_id)
    phase, detail, role_views = original
    attempts = list(data.get("attempts", {}).values())
    for index, role in enumerate(("worker-a", "worker-b", "reviewer"), start=1):
        matches = [attempt for attempt in attempts if attempt.get("role") == role]
        if not matches:
            continue
        attempt = matches[-1]
        handle = attempt.get("terminal")
        repo = attempt.get("repo")
        if not isinstance(handle, str) or not isinstance(repo, str):
            role_views[index].update(state="unknown", detail="担当の所在が未確定")
            phase = "unknown"
            continue
        try:
            code, response = ui.run_orca_response(["terminal", "show", "--terminal", handle])
        except (OSError, RuntimeError, ValueError):
            role_views[index].update(state="unknown", detail="担当タブの読取に失敗しました")
            phase = "unknown"
            continue
        terminal = response.get("result", {}).get("terminal", {})
        if (code or response.get("ok") is not True
                or terminal.get("handle") != handle
                or terminal.get("worktreePath") != repo
                or not isinstance(terminal.get("worktreeId"), str)
                or terminal["worktreeId"].partition("::")[2] != repo
                or terminal.get("executionHostId") != "local"
                or terminal.get("orphaned") is not False
                or terminal.get("connected") is not True
                or not isinstance(terminal.get("incarnationId"), str)
                or not terminal["incarnationId"]):
            role_views[index].update(state="unknown", detail="担当タブの所有を照合できません")
            phase = "unknown"
            continue
        role_views[index].update(
            state="waiting" if attempt.get("released") is True else "running",
            detail="前回の実行は終了" if attempt.get("released") is True else "監督下で実行中",
            terminal={key: terminal[key] for key in
                      ("handle", "incarnationId", "worktreeId", "executionHostId")})
    if data["phase"] == "paused":
        return ("paused", data.get("reason") or "監督ループを中断中", role_views)
    if data["phase"] == "approved":
        return ("feedback", "実装・固定レビューの結果を確認してください。", role_views)
    if phase == "unknown":
        return ("unknown", "担当の状態を照合できません。配車を増やさず確認してください。", role_views)
    reviewing = any(lane.get("phase") in review_loop.REVIEW_PHASES
                    for lane in data["lanes"].values())
    return ("review" if reviewing else "working",
            "固定レビュー中" if reviewing else "監督下で実装・検証中", role_views)


def route_view(root: Path, request_id: str) -> tuple[str, str, list[dict]]:
    data = route_record(root, request_id)
    role_views = roles()
    if not data:
        return ("queued", "実装依頼を受け付けました。作業場の準備待ちです。", role_views)
    phase = data["phase"]
    if phase in {"issue_creating", "worktree_creating"}:
        return ("unknown", "外部作成の結果を確認できません。二重作成せず照合待ちです。", role_views)
    if phase in {"prepared", "issue_created"}:
        return ("queued", "実装課題と分離作業場を準備中です。", role_views)
    child_id = data.get("childRequestId")
    if not child_id:
        return ("queued", "分離作業場を作成しました。統括タブの登録待ちです。", role_views)
    state_path = ui.state_path(child_id)
    if not state_path.exists() and not state_path.is_symlink():
        if phase == "terminal_starting":
            return ("unknown", "統括タブの起動結果を確認できません。二重起動せず照合待ちです。", role_views)
        return ("queued", "分離作業場を作成しました。統括タブの起動待ちです。", role_views)
    state = ui.read_registered_state(child_id)
    if (state["worktree_id"] != data.get("worktreeId")
            or state["linear_identifier"] != data.get("issueIdentifier")):
        raise ValueError("implementation route points to a different coordinator")
    if state["phase"] == "exited":
        role_views[0].update(state="ended", detail="統括タブのプロセスは終了しました")
        return ("unknown", "統括タブが終了しました。作業場は保持してあります。", role_views)
    code, response = ui.run_orca_response(["terminal", "show", "--terminal", state["terminal"]])
    terminal = response.get("result", {}).get("terminal", {})
    if (code or response.get("ok") is not True or terminal.get("handle") != state["terminal"]
            or terminal.get("worktreeId") != state["worktree_id"]
            or terminal.get("executionHostId") != "local"
            or terminal.get("orphaned") is not False
            or terminal.get("connected") is not True
            or not isinstance(terminal.get("incarnationId"), str)
            or not terminal["incarnationId"]):
        return ("unknown", "統括タブの所在を確認できません。作業場は保持してあります。", role_views)
    role_views[0].update(
        state="running" if state["phase"] == "ready" else "starting",
        detail="分離作業場で稼働中" if state["phase"] == "ready" else "統括タブを起動中",
        terminal={key: terminal[key] for key in
                  ("handle", "incarnationId", "worktreeId", "executionHostId")})
    return ("working" if state["phase"] == "ready" else "queued",
            "統括タブに移動できます。" if state["phase"] == "ready" else "統括タブを起動中です。",
            role_views)


def workflows(root: Path) -> list[dict]:
    entries = [reception()]
    routed_children: set[str] = set()
    for path in routing.route_path(root, RECEPTION_ID).parent.iterdir():
        if path.suffix != ".json":
            raise ValueError("unexpected implementation route entry")
        canonical_uuid(path.stem, "implementation route ID")
        child_id = route_record(root, path.stem).get("childRequestId")
        if child_id:
            canonical_uuid(child_id, "implementation child request ID")
            if child_id in routed_children:
                raise ValueError("implementation child request is not unique")
            routed_children.add(child_id)
    for item in frontdesk.list_requests():
        if item["id"] in routed_children:
            continue
        receipt = frontdesk.read_private_json(root / "receipts" / f"{item['id']}.json", {})
        if not receipt:
            # The global frontdesk also contains manually imported issues and
            # older CLI consultations. They are not owned by this fixed panel.
            continue
        if (receipt.get("schema") != 1 or receipt.get("intakeId") != item["id"]
                or receipt.get("operationId") != item["id"]
                or receipt.get("action") not in {"submit", "implement"}
                or receipt.get("phase") != "accepted"
                or receipt.get("text", "").strip() != item["request"]):
            raise ValueError("panel intake receipt does not match its workflow")
        current = lifecycle(root, item["id"])
        if current.get("phase") == "closed":
            state, detail, role_views = lifecycle_view(root, item["id"], ("closed", "", roles()))
        else:
            if receipt.get("action") == "implement":
                original = route_view(root, item["id"])
                child_id = route_record(root, item["id"]).get("childRequestId")
                if child_id and original[0] == "working":
                    original = supervised_view(child_id, original)
            else:
                original = consultation_view(root, item["id"])
            state, detail, role_views = lifecycle_view(root, item["id"], original)
        entries.append({"id": item["id"], "revision": task_revision(root, item["id"]),
                        "title": item["request"].strip().splitlines()[0][:120],
                        "kind": "task", "state": state,
                        "detail": detail, "actions": task_actions(root, item["id"], receipt.get("action")),
                        "roles": role_views})
    if len(entries) > 100:
        raise ValueError("supervision workflow count exceeds the UI bound")
    return entries


def publish(root: Path, current_runtime: str) -> dict:
    root, _, _ = directories(root)
    snapshot = {"schema": 1, "runtimeId": canonical_uuid(current_runtime, "runtime ID"),
                "publishedAt": int(time.time() * 1000), "workflows": workflows(root)}
    payload = json.dumps(snapshot, ensure_ascii=False).encode("utf-8")
    if len(payload) > MAX_DOCUMENT_BYTES:
        raise ValueError("supervision snapshot exceeds the UI bound")
    frontdesk.write_ledger(root / "snapshot.json", snapshot)
    return snapshot


def checked_request(path: Path) -> dict:
    if not re.fullmatch(r"[a-f0-9-]{36}\.json", path.name):
        raise ValueError("unexpected supervision request entry")
    canonical_uuid(path.stem, "operation ID")
    if path.lstat().st_size > MAX_DOCUMENT_BYTES:
        raise ValueError("supervision request exceeds its bound")
    data = frontdesk.read_private_json(path, {})
    if (not isinstance(data, dict) or set(data) != REQUEST_KEYS or data.get("schema") != 1
            or data.get("runtimeId") != data.get("expectedRuntimeId")
            or data.get("operationId") != path.stem
            or not isinstance(data.get("workflowId"), str)
            or not isinstance(data.get("revision"), str)
            or data.get("action") not in {"submit", "implement", "pause", "resume", "close"}
            or not isinstance(data.get("text"), str) or len(data["text"]) > 8192):
        raise ValueError("supervision request is invalid")
    reception_action = data["action"] in {"submit", "implement"}
    if reception_action:
        if (data["workflowId"] != RECEPTION_ID or data["revision"] != "1"
                or not data["text"].strip()):
            raise ValueError("supervision intake request is invalid")
    elif (data["workflowId"] == RECEPTION_ID or data["text"]
          or not re.fullmatch(r"[1-9][0-9]*-[0-9a-f]{12}", data["revision"])):
        raise ValueError("supervision lifecycle request is invalid")
    canonical_uuid(data["workflowId"], "workflow ID")
    return data


def lifecycle_receipt(root: Path, request: dict, current_runtime: str) -> dict:
    request_id = request["workflowId"]
    matches = [item for item in frontdesk.list_requests() if item["id"] == request_id]
    if len(matches) != 1:
        raise ValueError("lifecycle workflow does not exist")
    intake_receipt = frontdesk.read_private_json(root / "receipts" / f"{request_id}.json", {})
    if intake_receipt.get("phase") != "accepted":
        raise ValueError("workflow intake is unconfirmed")
    current = lifecycle(root, request_id)
    path = root / "receipts" / f"{request['operationId']}.json"
    existing = frontdesk.read_private_json(path, {})
    if existing:
        if (existing.get("schema") != 1 or existing.get("operationId") != request["operationId"]
                or existing.get("workflowId") != request_id or existing.get("action") != request["action"]):
            raise ValueError("lifecycle receipt conflicts with the request")
        return existing
    if (current.get("operationId") == request["operationId"]
            and current.get("requestRevision") == request["revision"]):
        # The prior controller may have committed the lifecycle transition but
        # lost its receipt. Never repeat a close or send a second stop.
        receipt = {"schema": 1, "runtimeId": current_runtime,
                   "operationId": request["operationId"], "workflowId": request_id,
                   "revision": request["revision"], "action": request["action"],
                   "text": "", "intakeId": request_id,
                   "phase": current.get("outcome", "accepted")}
        frontdesk.write_ledger(path, receipt)
        return receipt
    if request["revision"] != task_revision(root, request_id):
        rejected = {"schema": 1, "runtimeId": current_runtime,
                    "operationId": request["operationId"], "workflowId": request_id,
                    "revision": request["revision"], "action": request["action"],
                    "text": "", "intakeId": request_id, "phase": "rejected"}
        frontdesk.write_ledger(path, rejected)
        return rejected
    if request["action"] not in task_actions(root, request_id, intake_receipt.get("action")):
        frontdesk.write_ledger(lifecycle_path(root, request_id), {
            "schema": 1, "requestId": request_id, "revision": current.get("revision", 1) + 1,
            "requestRevision": request["revision"], "operationId": request["operationId"],
            "phase": current.get("phase", "active"), "outcome": "rejected",
            "message": "操作対象の工程が変わりました。状態を再確認してください。"})
        rejected = {"schema": 1, "runtimeId": current_runtime,
                    "operationId": request["operationId"], "workflowId": request_id,
                    "revision": request["revision"], "action": request["action"],
                    "text": "", "intakeId": request_id, "phase": "rejected"}
        frontdesk.write_ledger(path, rejected)
        return rejected
    phase = {"pause": "paused", "close": "closing", "resume": "active"}[request["action"]]
    expected = {"schema": 1, "requestId": request_id, "revision": current.get("revision", 1) + 1,
                "requestRevision": request["revision"], "operationId": request["operationId"],
                "phase": phase, "outcome": "accepted"}
    try:
        if request["action"] == "close":
            expected["closeTargets"] = preflight_close(root, request_id, intake_receipt)
        elif request["action"] == "pause":
            preflight_pause(root, request_id, intake_receipt)
        elif current.get("phase") != "paused":
            raise ValueError("only a paused workflow can resume")
        elif intake_receipt.get("action") == "implement":
            preflight_pause(root, request_id, intake_receipt)
    except ValueError as error:
        expected = {**expected, "phase": current.get("phase", "active"), "outcome": "rejected",
                    "message": f"操作を保留しました: {error}"[:2048]}
        frontdesk.write_ledger(lifecycle_path(root, request_id), expected)
        phase = "rejected"
    else:
        phase = "accepted"
    receipt = {"schema": 1, "runtimeId": current_runtime, "operationId": request["operationId"],
               "workflowId": request_id, "revision": request["revision"],
               "action": request["action"], "text": "", "intakeId": request_id,
               "phase": phase}
    if phase == "rejected":
        frontdesk.write_ledger(path, receipt)
        return receipt
    frontdesk.write_ledger(lifecycle_path(root, request_id), expected)
    frontdesk.write_ledger(path, receipt)
    if request["action"] == "close":
        try:
            finish_close(root, request_id, expected, intake_receipt)
        except Exception as error:
            frontdesk.write_ledger(lifecycle_path(root, request_id), {
                **expected, "phase": "unknown", "message": f"終了照合が未確定: {error}"[:2048]})
            raise
    return receipt


def preflight_pause(root: Path, request_id: str, intake_receipt: dict, *, allow_exited: bool = False) -> None:
    if intake_receipt.get("action") != "implement":
        raise ValueError("only a prepared implementation can be paused")
    data = route_record(root, request_id)
    if data.get("phase") != "ready":
        raise ValueError("implementation route is not ready for a pause")
    child = data.get("childRequestId")
    if not child:
        raise ValueError("coordinator ownership is missing")
    loop_path = review_loop.state_path(child)
    if (loop_path.exists() or loop_path.is_symlink()) and review_loop.load(child)["phase"] != "approved":
        raise ValueError("supervised loop is unresolved; do not pause its coordinator alone")
    state = ui.read_registered_state(child)
    phase_ok = state["phase"] == "ready" or (allow_exited and state["phase"] == "exited"
                                                and state.get("exit_code") == 0)
    if not phase_ok or state["worktree_id"] != data.get("worktreeId"):
        raise ValueError("coordinator ownership changed")
    worktree = Path(state["worktree_id"].partition("::")[2])
    code, response = ui.run_orca_response(["terminal", "show", "--terminal", state["terminal"]])
    if code or response.get("ok") is not True:
        raise ValueError("coordinator terminal is unconfirmed")
    owned = role_tabs.identity(response.get("result", {}).get("terminal", {}), str(worktree))
    if owned["handle"] != state["terminal"] or owned["worktreeId"] != state["worktree_id"]:
        raise ValueError("coordinator terminal ownership changed")
    role_tabs.idle_shell(state["terminal"], str(worktree))


def settled_role_targets(child: str, loop: dict | None) -> list[dict]:
    """Bind every settled attempt to its exact registered tab before closure."""
    expected: dict[tuple[str, str], str] = {}
    if loop is not None:
        for attempt in loop.get("attempts", {}).values():
            role, repo, handle = (attempt.get(key) for key in ("role", "repo", "terminal"))
            if (role not in role_tabs.TITLES or not isinstance(repo, str) or not repo
                    or not isinstance(handle, str) or not handle or attempt.get("released") is not True):
                raise ValueError("settled role attempt has incomplete terminal ownership")
            key = (repo, role)
            if key in expected and expected[key] != handle:
                raise ValueError("role terminal changed across attempts; reconcile before closure")
            expected[key] = handle
    targets = []
    seen: set[tuple[str, str]] = set()
    for path in sorted(role_tabs.root().glob("*.json")):
        record = frontdesk.read_private_json(path, {})
        if record.get("request") != child:
            continue
        repo, role = record.get("repo"), record.get("slot")
        key = (repo, role)
        if (record.get("schema") != 1 or key not in expected or key in seen
                or record.get("phase") != "known"
                or path != role_tabs.registry_path(child, repo, role)
                or not isinstance(record.get("identity"), dict)
                or record["identity"].get("handle") != expected[key]):
            raise ValueError("worker or reviewer tab registry is unresolved")
        seen.add(key)
        targets.append({"repo": repo, "role": role, "identity": record["identity"]})
    if seen != set(expected):
        raise ValueError("settled role tab registry is missing")
    return checked_close_targets(targets)


def checked_clean_worktree(worktree: Path) -> None:
    result = subprocess.run(["git", "-C", str(worktree), "status", "--porcelain", "--untracked-files=all"],
                            capture_output=True, text=True, check=False)
    if result.returncode or result.stdout or result.stderr:
        raise ValueError("worktree has unverified changes; preserve the workflow")


def preflight_close(root: Path, request_id: str, intake_receipt: dict) -> list[dict]:
    if intake_receipt.get("action") != "implement":
        if consult_intent(root, request_id).get("phase") != "settled":
            raise ValueError("consultation is not settled")
        return []
    preflight_pause(root, request_id, intake_receipt, allow_exited=True)
    data = route_record(root, request_id)
    child = data["childRequestId"]
    loop_path = review_loop.state_path(child)
    loop = None
    if loop_path.exists() or loop_path.is_symlink():
        loop = review_loop.load(child)
        if loop["phase"] != "approved" or any(
                attempt.get("released") is not True for attempt in loop.get("attempts", {}).values()):
            raise ValueError("supervised implementation or review is not fully settled")
        if (loop.get("integration") and loop["integration"].get("phase") != "approved"):
            raise ValueError("integration review is not approved")
        if loop.get("schema") == 2 and not review_loop.mail.drained(loop):
            raise ValueError("supervised notifications are not drained")
    targets = settled_role_targets(child, loop)
    state = ui.read_registered_state(child)
    worktree = Path(state["worktree_id"].partition("::")[2])
    handles: dict[str, set[str]] = {str(worktree): {state["terminal"]}}
    for target in targets:
        repo, identity = target["repo"], target["identity"]
        code, response = ui.run_orca_response(["terminal", "show", "--terminal", identity["handle"]])
        if (code or response.get("ok") is not True
                or role_tabs.identity(response.get("result", {}).get("terminal", {}), repo) != identity):
            raise ValueError("settled role tab identity changed")
        role_tabs.idle_shell(identity["handle"], repo)
        handles.setdefault(repo, set()).add(identity["handle"])
    for repo, owned in handles.items():
        code, inventory_response = ui.run_orca_response([
            "terminal", "list", "--worktree", f"path:{repo}", "--include-visual-layouts"])
        inventory = inventory_response.get("result", {})
        terminals = inventory.get("terminals")
        layouts = inventory.get("visualLayouts")
        if (code or inventory_response.get("ok") is not True
                or inventory.get("truncated") is not False
                or inventory.get("hostScope", {}).get("omittedHostIds") != []
                or not isinstance(terminals, list)
                or {row.get("handle") for row in terminals} != owned
                or len(terminals) != len(owned)
                or not isinstance(layouts, list) or len(layouts) != 1
                or len(layouts[0].get("root", {}).get("tabs", [])) != len(owned)):
            raise ValueError("unexpected tab or split remains; preserve the workflow")
        checked_clean_worktree(Path(repo))
    return targets


def verify_tab_free(worktree: Path) -> None:
    for attempt in range(2):
        if attempt:
            time.sleep(1)
        result_code, result_response = ui.run_orca_response([
            "terminal", "list", "--worktree", f"path:{worktree}"])
        inventory = result_response.get("result", {})
        if (result_code or result_response.get("ok") is not True
                or inventory.get("truncated") is not False
                or inventory.get("hostScope", {}).get("omittedHostIds") != []
                or inventory.get("terminals") != []):
            raise ValueError("terminal was closed but worktree is not tab-free")


def close_call(_cli: object, args: list[str], _purpose: str) -> dict:
    result_code, result_response = ui.run_orca_response(args)
    if result_code or result_response.get("ok") is not True:
        raise ValueError("terminal close result is unconfirmed")
    return result_response["result"]


def close_target(target: dict) -> None:
    repo, identity = target["repo"], target["identity"]
    retired = role_tabs.root() / "retired" / f"{role_tabs.bindings.digest(identity)}.json"
    if retired.exists() or retired.is_symlink():
        record = frontdesk.read_private_json(retired, {})
        closed = record.get("receipt", {}).get("close", {})
        if (record.get("identity") != identity or record.get("repo") != repo
                or record.get("phase") != "close-returned"
                or closed.get("handle") != identity["handle"]
                or closed.get("ptyKilled") is not True):
            raise ValueError("role tab close result is uncertain; do not replay")
        code, response = ui.run_orca_response(["terminal", "list", "--worktree", f"path:{repo}"])
        inventory = response.get("result", {})
        if (code or response.get("ok") is not True
                or inventory.get("truncated") is not False
                or inventory.get("hostScope", {}).get("omittedHostIds") != []
                or not isinstance(inventory.get("terminals"), list)
                or any(row.get("handle") == identity["handle"] for row in inventory["terminals"])):
            raise ValueError("role tab close receipt has no matching read-back")
        return
    role_tabs.retire(close_call, None, repo, identity)


def finish_close(root: Path, request_id: str, expected: dict, intake_receipt: dict) -> None:
    if intake_receipt.get("action") == "implement":
        targets = checked_close_targets(expected.get("closeTargets", []))
        for target in targets:
            close_target(target)
        data = route_record(root, request_id)
        state = ui.read_registered_state(data["childRequestId"])
        worktree = Path(state["worktree_id"].partition("::")[2])
        code, response = ui.run_orca_response(["terminal", "show", "--terminal", state["terminal"]])
        row = response.get("result", {}).get("terminal", {})
        if code or response.get("ok") is not True:
            raise ValueError("coordinator terminal identity is unconfirmed")
        identity = role_tabs.identity(row, str(worktree))
        if identity["handle"] != state["terminal"]:
            raise ValueError("coordinator terminal ownership changed")
        role_tabs.retire(close_call, None, str(worktree), identity)
        verify_tab_free(worktree)
        for repo in {target["repo"] for target in targets} - {str(worktree)}:
            verify_tab_free(Path(repo))
    completed = {key: value for key, value in expected.items() if key != "message"}
    frontdesk.write_ledger(lifecycle_path(root, request_id), {**completed, "phase": "closed"})


def reconcile_unfinished_close(root: Path, request_id: str) -> dict:
    """Resume only a close proven to have stopped before any terminal mutation."""
    with acquire_host("frontdesk-ui", inherit=False):
        current = lifecycle(root, request_id)
        if current.get("phase") != "unknown" or current.get("outcome") != "accepted":
            raise ValueError("no uncertain close is available for reconciliation")
        receipt = frontdesk.read_private_json(root / "receipts" / f"{current['operationId']}.json", {})
        if (receipt.get("operationId") != current["operationId"]
                or receipt.get("workflowId") != request_id
                or receipt.get("action") != "close" or receipt.get("phase") != "accepted"):
            raise ValueError("close receipt is not durable")
        intake_receipt = frontdesk.read_private_json(root / "receipts" / f"{request_id}.json", {})
        if intake_receipt.get("action") != "implement":
            raise ValueError("only an owned implementation close can be reconciled")
        route = route_record(root, request_id)
        state = ui.read_registered_state(route["childRequestId"])
        worktree = Path(state["worktree_id"].partition("::")[2])
        targets = checked_close_targets(current.get("closeTargets", []))
        for target in targets:
            close_target(target)
        retired_dir = frontdesk.checked_directory(role_tabs.root() / "retired")
        candidates = []
        for path in retired_dir.glob("*.json"):
            data = frontdesk.read_private_json(path, {})
            identity = data.get("identity", {})
            if (identity.get("handle") == state["terminal"]
                    and identity.get("worktreeId") == state["worktree_id"]):
                candidates.append(data)
        if len(candidates) > 1:
            raise ValueError("multiple retirement receipts match this coordinator")
        if candidates:
            retired = candidates[0]
            closed = retired.get("receipt", {}).get("close", {})
            if (retired.get("phase") != "close-returned"
                    or closed.get("handle") != state["terminal"]
                    or closed.get("ptyKilled") is not True):
                raise ValueError("terminal close result is uncertain; do not replay")
            verify_tab_free(worktree)
            for repo in {target["repo"] for target in targets} - {str(worktree)}:
                verify_tab_free(Path(repo))
            completed = {key: value for key, value in current.items() if key != "message"}
            frontdesk.write_ledger(lifecycle_path(root, request_id), {**completed, "phase": "closed"})
            return lifecycle(root, request_id)
        # A role tab may already be retired. The durable target list, not a new
        # preflight against a partly closed layout, owns this reconciliation.
        if not targets:
            preflight_close(root, request_id, intake_receipt)
        code, response = ui.run_orca_response(["terminal", "show", "--terminal", state["terminal"]])
        if code or response.get("ok") is not True:
            raise ValueError("coordinator terminal is unconfirmed")
        identity = role_tabs.identity(response.get("result", {}).get("terminal", {}), str(worktree))
        retired = retired_dir / f"{role_tabs.bindings.digest(identity)}.json"
        if retired.exists() or retired.is_symlink():
            raise ValueError("terminal closure may already have started; read back its receipt")
        try:
            finish_close(root, request_id, current, intake_receipt)
        except Exception as error:
            frontdesk.write_ledger(lifecycle_path(root, request_id), {
                **current, "message": f"終了照合が未確定: {error}"[:2048]})
            raise
        return lifecycle(root, request_id)


def accept(root: Path, request: dict, current_runtime: str) -> dict:
    _, _, receipts = directories(root)
    operation_id = request["operationId"]
    path = receipts / f"{operation_id}.json"
    expected = {"schema": 1, "runtimeId": current_runtime,
                "operationId": operation_id, "workflowId": RECEPTION_ID,
                "revision": "1", "action": request["action"], "text": request["text"],
                "intakeId": operation_id, "phase": "accepted"}
    existing = frontdesk.read_private_json(path, {})
    if existing:
        if existing != expected:
            raise ValueError("supervision receipt conflicts with its request")
        frontdesk.sync_directory(receipts)
        if request["action"] == "submit":
            prepare_consult(root, operation_id)
        return existing
    item = frontdesk.submit(request["text"], operation_id)
    if item["id"] != operation_id or item["request"] != request["text"].strip():
        raise ValueError("frontdesk intake does not match the operation")
    frontdesk.write_ledger(path, expected)
    if request["action"] == "submit":
        prepare_consult(root, operation_id)
    return expected


def tick_under_lock(root: Path, current_runtime: str) -> dict:
    root, requests, _ = directories(root)
    # A mismatched boot must not ingest a prior boot's queued commands.
    names = sorted(path.name for path in requests.iterdir())
    for name in names:
        if name.startswith(".") and name.endswith(".pending"):
            continue
        request = checked_request(requests / name)
        is_intake = request["action"] in {"submit", "implement"}
        if request["runtimeId"] != current_runtime:
            receipt = frontdesk.read_private_json(root / "receipts" / name, {})
            expected = {"schema": 1, "runtimeId": request["runtimeId"], "operationId": request["operationId"],
                        "workflowId": request["workflowId"], "revision": request["revision"],
                        "action": request["action"], "text": request["text"],
                        "intakeId": request["operationId"] if is_intake else request["workflowId"],
                        "phase": "accepted" if is_intake else receipt.get("phase")}
            if receipt != expected or (is_intake and not any(
                item["id"] == request["operationId"] and item["request"] == request["text"].strip()
                for item in frontdesk.list_requests())):
                raise ValueError("unsettled supervision request belongs to another runtime")
            if is_intake and request["action"] == "submit":
                prepare_consult(root, request["operationId"])
        else:
            if is_intake:
                accept(root, request, current_runtime)
            else:
                lifecycle_receipt(root, request, current_runtime)
        (requests / name).unlink()
        frontdesk.sync_directory(requests)
    return publish(root, current_runtime)


def tick(root: Path, current_runtime: str) -> dict:
    with acquire_host("frontdesk-ui", inherit=False):
        return tick_under_lock(root, current_runtime)


def write_consult_phase(root: Path, request_id: str, phase: str) -> None:
    if phase not in CONSULT_PHASES or not consult_intent(root, request_id):
        raise ValueError("cannot transition an unknown consultation")
    frontdesk.write_ledger(consult_path(root, request_id), {
        "schema": 1, "requestId": request_id, "phase": phase})


def reconcile_unowned_consults(root: Path) -> None:
    for path in consult_dir(root).iterdir():
        canonical_uuid(path.stem, "consultation ID")
        if path.suffix != ".json":
            raise ValueError("unexpected consultation intent entry")
        if consult_intent(root, path.stem)["phase"] == "launching":
            write_consult_phase(root, path.stem, "unknown")


def start_consult(root: Path) -> tuple[str, subprocess.Popen] | None:
    if TEST_ISOLATED:
        # Controller-connected UI tests assert intake durability, not a mock
        # coding provider. A real provider is exercised in a separate acceptance.
        return None
    for item in frontdesk.list_requests():
        request_id = item["id"]
        if consult_intent(root, request_id).get("phase") != "ready":
            continue
        # Durable launch intent precedes process creation. After a crash, the
        # provider may have started; never infer that retrying is safe.
        write_consult_phase(root, request_id, "launching")
        command = [sys.executable, str(Path(__file__).with_name("orca_frontdesk.py")),
                   "consult", "--request-id", request_id]
        try:
            child = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                     cwd=Path(__file__).resolve().parents[1], close_fds=True,
                                     start_new_session=True)
        except OSError:
            write_consult_phase(root, request_id, "unknown")
            raise
        return request_id, child
    return None


def settle_consult(root: Path, request_id: str, child: subprocess.Popen) -> None:
    if child.poll() is None:
        return
    state = coordinator.read_state(request_id)
    turns = state["turns"]
    if (child.returncode == 0 and turns and turns[-1]["phase"] == "succeeded"):
        write_consult_phase(root, request_id, "settled")
    else:
        write_consult_phase(root, request_id, "unknown")


def stop_owned_consult(root: Path, request_id: str, child: subprocess.Popen) -> None:
    if child.poll() is None:
        if os.getpgid(child.pid) != child.pid:
            raise RuntimeError("consultation process group is not owned by this controller")
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
    write_consult_phase(root, request_id, "unknown")


def start_route(root: Path, cli: Path | None = None) -> tuple[str, subprocess.Popen] | None:
    if TEST_ISOLATED:
        return None
    for item in frontdesk.list_requests():
        request_id = item["id"]
        receipt = frontdesk.read_private_json(root / "receipts" / f"{request_id}.json", {})
        if receipt.get("action") != "implement":
            continue
        phase = route_record(root, request_id).get("phase")
        if phase in {"issue_creating", "worktree_creating", "ready"}:
            continue
        environment = os.environ.copy()
        if cli:
            environment["HELL_WORKERS_ORCA_CLI"] = str(intake.checked_orca_cli(cli))
        command = [sys.executable, str(Path(__file__).with_name("orca_supervision_route.py")),
                   "--state-dir", str(root), "--request-id", request_id,
                   "--primary", str(ui.primary_repo())]
        child = subprocess.Popen(command, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                 cwd=Path(__file__).resolve().parents[1], env=environment,
                                 close_fds=True, start_new_session=True)
        return request_id, child
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("once", "serve", "reconcile-close"))
    parser.add_argument("--state-dir", type=Path, required=True)
    parser.add_argument("--orca-cli", type=Path)
    parser.add_argument("--expected-runtime-id", required=True)
    parser.add_argument("--request-id")
    args = parser.parse_args()
    configure_test_state(args.state_dir)
    expected = canonical_uuid(args.expected_runtime_id, "expected runtime ID")
    if args.action == "reconcile-close":
        if not args.request_id or runtime_id(args.orca_cli) != expected:
            raise RuntimeChangedError("close reconciliation requires the matching Orca runtime and request")
        result = reconcile_unfinished_close(args.state_dir, args.request_id)
        print(json.dumps({"requestId": args.request_id, "phase": result["phase"]}, ensure_ascii=False))
        return 0
    if args.action == "once":
        observed = runtime_id(args.orca_cli)
        if observed != expected:
            raise RuntimeChangedError("Orca runtime changed before publishing supervision")
        print(json.dumps(tick(args.state_dir, observed), ensure_ascii=False))
        return 0
    stopping = False

    def request_stop(_signal: int, _frame: object) -> None:
        nonlocal stopping
        stopping = True

    signal.signal(signal.SIGTERM, request_stop)
    signal.signal(signal.SIGINT, request_stop)
    active: tuple[str, subprocess.Popen] | None = None
    active_route: tuple[str, subprocess.Popen] | None = None
    with acquire_host("frontdesk-ui", inherit=False):
        reconcile_unowned_consults(args.state_dir)
        try:
            while not stopping:
                try:
                    observed = runtime_id(args.orca_cli)
                    if observed != expected:
                        raise RuntimeChangedError("Orca runtime changed; stop this controller")
                    if active and active[1].poll() is not None:
                        settle_consult(args.state_dir, *active)
                        active = None
                    if active_route and active_route[1].poll() is not None:
                        active_route = None
                    tick_under_lock(args.state_dir, observed)
                    if active is None:
                        active = start_consult(args.state_dir)
                    if active_route is None:
                        active_route = start_route(args.state_dir, args.orca_cli)
                except Exception as error:
                    print(f"Orca supervision paused: {type(error).__name__}: {error}", flush=True)
                    if isinstance(error, RuntimeChangedError):
                        return 1
                time.sleep(2)
        finally:
            if active:
                stop_owned_consult(args.state_dir, *active)
            # The router owns its durable journal and may be between an external
            # write and its response. Never kill or restart it blindly here.
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
