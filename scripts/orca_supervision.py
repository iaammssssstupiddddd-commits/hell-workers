"""Project-owned bridge between Orca's supervision panel and guarded intake."""

from __future__ import annotations

import argparse
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
    from . import host_coordination, orca_supervision_route as routing, orca_ui_coordinator as ui
    from .host_coordination import acquire_host
else:
    import orca_coordinator as coordinator
    import orca_frontdesk as frontdesk
    import orca_issue_context as intake
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
                                               "worktree_creating", "worktree_created", "ready"}):
        raise ValueError("invalid implementation route; preserve for reconciliation")
    return data


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
        state, detail, role_views = (route_view(root, item["id"])
                                     if receipt.get("action") == "implement"
                                     else consultation_view(root, item["id"]))
        entries.append({"id": item["id"], "revision": "1",
                        "title": item["request"].strip().splitlines()[0][:120],
                        "kind": "task", "state": state,
                        "detail": detail, "actions": [], "roles": role_views})
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
            or data.get("workflowId") != RECEPTION_ID or data.get("revision") != "1"
            or data.get("action") not in {"submit", "implement"} or not isinstance(data.get("text"), str)
            or not data["text"].strip() or len(data["text"]) > 8192):
        raise ValueError("supervision request is invalid")
    return data


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
        if request["runtimeId"] != current_runtime:
            receipt = frontdesk.read_private_json(root / "receipts" / name, {})
            expected = {"schema": 1, "runtimeId": request["runtimeId"],
                        "operationId": request["operationId"],
                        "workflowId": RECEPTION_ID, "revision": "1",
                        "action": request["action"], "text": request["text"],
                        "intakeId": request["operationId"], "phase": "accepted"}
            if receipt != expected or not any(
                item["id"] == request["operationId"]
                and item["request"] == request["text"].strip()
                for item in frontdesk.list_requests()
            ):
                raise ValueError("unsettled supervision request belongs to another runtime")
            if request["action"] == "submit":
                prepare_consult(root, request["operationId"])
        else:
            accept(root, request, current_runtime)
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
    parser.add_argument("action", choices=("once", "serve"))
    parser.add_argument("--state-dir", type=Path, required=True)
    parser.add_argument("--orca-cli", type=Path)
    parser.add_argument("--expected-runtime-id", required=True)
    args = parser.parse_args()
    configure_test_state(args.state_dir)
    expected = canonical_uuid(args.expected_runtime_id, "expected runtime ID")
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
