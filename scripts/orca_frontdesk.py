"""Durable local Orca intake. No agent is started by opening the front desk.

The request file records intake, not an alternative to Orca Task/Dispatch state.
Requests stay queued until an operator starts the guarded orchestration bridge.
An explicit consultation starts a read-only coordinator; supervised editing is a
separate confirmed action using a fixed ticket and worker slot.
"""

from __future__ import annotations

import argparse
import json
import os
import stat
import tempfile
import uuid
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

try:
    from host_coordination import acquire_host, state_root
except ModuleNotFoundError:
    from scripts.host_coordination import acquire_host, state_root


def checked_directory(path: Path) -> Path:
    if path.resolve() != path:
        raise RuntimeError(f"frontdesk path must not contain symlinks: {path}")
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    info = path.stat()
    if info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise RuntimeError(f"unsafe frontdesk directory: {path}")
    return path


def ledger_path() -> Path:
    return checked_directory(state_root().parent / "frontdesk") / "requests.json"


def read_private_json(path: Path, default: dict) -> dict:
    if not path.exists() and not path.is_symlink():
        return default
    info = path.lstat()
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
            or info.st_mode & 0o077 or info.st_nlink != 1):
        raise RuntimeError("unsafe intake ledger/state")
    return json.loads(path.read_text(encoding="utf-8"))


def read_ledger(path: Path) -> dict:
    data = read_private_json(path, {"schema": 1, "requests": []})
    if not isinstance(data, dict) or data.get("schema") != 1 or not isinstance(data.get("requests"), list):
        raise ValueError("invalid intake ledger; preserve it for recovery")
    identities = set()
    for item in data["requests"]:
        if not isinstance(item, dict) or str(uuid.UUID(item["id"])) != item["id"]:
            raise ValueError("invalid request identity")
        if item["id"] in identities or item.get("status") != "queued":
            raise ValueError("unknown/duplicate intake state; do not infer dispatch success")
        request = item.get("request")
        if not isinstance(request, str) or not request.strip() or len(request) > 32_000:
            raise ValueError("invalid request content; preserve it for recovery")
        created = item.get("created_at")
        if not isinstance(created, str) or datetime.fromisoformat(created).tzinfo is None:
            raise ValueError("invalid request timestamp; preserve it for recovery")
        if any(item.get(key) is not None for key in
               ("run_id", "task_id", "dispatch_id", "coordinator_session")):
            raise ValueError("queued intake has unexpected dispatch identity; reconcile before continuing")
        identities.add(item["id"])
    return data


def sync_directory(path: Path) -> None:
    directory = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


def write_ledger(path: Path, data: dict) -> None:
    checked_directory(path.parent)
    fd, temporary = tempfile.mkstemp(prefix=".requests-", suffix=".json", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(data, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        sync_directory(path.parent)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


@contextmanager
def ledger():
    with acquire_host("frontdesk-state", inherit=False):
        path = ledger_path()
        data = read_ledger(path)
        yield path, data


def submit(text: str, request_id: str | None = None) -> dict:
    text = text.strip()
    if not text or len(text) > 32_000:
        raise ValueError("request must contain 1..32000 characters")
    identity = request_id or str(uuid.uuid4())
    if str(uuid.UUID(identity)) != identity:
        raise ValueError("request id must be a canonical UUID")
    with ledger() as (path, data):
        for item in data["requests"]:
            if item["id"] == identity:
                if item["request"] != text:
                    raise ValueError("request id already belongs to a different request")
                # The previous response may have failed after replace but before
                # directory fsync. Do not report durable success until it succeeds.
                sync_directory(path.parent)
                return item
        item = {"id": identity, "request": text, "status": "queued",
                "created_at": datetime.now(timezone.utc).isoformat(),
                "run_id": None, "task_id": None, "dispatch_id": None,
                "coordinator_session": None,
                "routing": {"worker-a": "codex", "worker-b": "cursor-simple-only", "reviewer": "codex-fixed"}}
        data["requests"].append(item)
        write_ledger(path, data)
        return item


def list_requests() -> list[dict]:
    with ledger() as (_, data):
        return data["requests"]


def coordinator_module():
    try:
        import orca_coordinator
    except ModuleNotFoundError:
        from scripts import orca_coordinator
    return orca_coordinator


def linear_module():
    try:
        import orca_issue_context
    except ModuleNotFoundError:
        from scripts import orca_issue_context
    return orca_issue_context


def dispatch_module():
    try:
        import orca_dispatch
    except ModuleNotFoundError:
        from scripts import orca_dispatch
    return orca_dispatch


def multiline() -> str:
    print("単独の . で確定、空なら取消。", flush=True)
    lines = []
    while True:
        line = input()
        if line == ".":
            return "\n".join(lines).strip()
        lines.append(line)
        if sum(map(len, lines)) + len(lines) > 32_000:
            raise ValueError("入力は32000文字以内にしてください")


def select_request() -> str:
    items = list_requests()
    for number, item in enumerate(items, 1):
        print(f"{number}: {item['request'][:80]}  [{item['id']}]", flush=True)
    value = input("受付番号またはUUID（空で取消）: ").strip()
    if not value:
        return ""
    if value.isdecimal():
        index = int(value) - 1
        if not 0 <= index < len(items):
            raise ValueError("受付番号が範囲外です")
        return items[index]["id"]
    return value


def show_reply(turn: dict) -> None:
    print(turn["response"], flush=True)
    if turn.get("source_changed"):
        print("注意: 相談中にsourceが変化しました。最新状態を再確認してください。", flush=True)
    print("相談ターン終了。依頼は未dispatch・未承認です。", flush=True)


def menu_action(choice: str) -> None:
    if choice == "1":
        issue = input("Linear課題IDまたはURL（空で取消）: ").strip()
        if not issue:
            return
        workspace = input("Linear workspace UUID: ").strip()
        result = linear_module().import_issue(issue, workspace)
        print(f"Linear受付済み（未dispatch）: {result['request_id']}", flush=True)
    elif choice == "7":
        print("目的・完了条件・変更禁止事項を入力。", flush=True)
        text = multiline()
        if text:
            item = submit(text)
            print(f"手入力受付済み（未dispatch）: {item['id']}", flush=True)
    elif choice == "2":
        print(json.dumps(list_requests(), ensure_ascii=False, indent=2), flush=True)
    elif choice in {"3", "4", "5", "6", "8"}:
        request_id = select_request()
        if not request_id:
            return
        if choice == "8":
            ticket = input("固定ticketの絶対path（空で取消）: ").strip()
            if not ticket:
                return
            slot = input("担当（worker-a / worker-b / reviewer、空で取消）: ").strip()
            if not slot:
                return
            if input("監督付きTaskを開始します。開始する場合 yes: ").strip() != "yes":
                return
            handle = os.environ.get("ORCA_TERMINAL_HANDLE")
            if not handle:
                raise RuntimeError("Orca管理terminalから実行してください")
            result = dispatch_module().start(request_id, Path(ticket).resolve(), slot, handle)
            print(json.dumps(result, ensure_ascii=False, indent=2), flush=True)
            return
        coordinator = coordinator_module()
        if choice == "5":
            print(json.dumps(coordinator.read_state(request_id), ensure_ascii=False, indent=2), flush=True)
            return
        message = None
        turn_id = None
        if choice in {"4", "6"}:
            if choice == "6":
                print("異常終了後の照合。子process終了が証明できない場合は拒否します。元依頼は再送しません。", flush=True)
            print("同じ会話へ追加する指示:", flush=True)
            message = multiline()
            if not message:
                return
            turn_id = str(uuid.uuid4())
        if input("Codexを1ターン起動します（read-only・モデル利用あり）。開始する場合 yes: ").strip() != "yes":
            return
        show_reply(coordinator.consult(request_id, message, turn_id, recover=choice == "6"))


def menu() -> None:
    with acquire_host("frontdesk-ui", inherit=False):
        print("Hell Workers | 開発受付・統括相談", flush=True)
        print("統括は選択時だけ起動。相談後、固定ticketから監督付き編集Taskを開始できます。", flush=True)
        while True:
            print("\n1: Linear課題を受付  2: 一覧  3: 統括へ相談  4: 追記  5: 状態/回答  "
                  "6: 異常終了後の照合  7: 手入力fallback  8: 実装/レビューTask開始  q: 閉じる", flush=True)
            try:
                choice = input("> ").strip()
                if choice == "q":
                    return
                menu_action(choice)
            except (EOFError, KeyboardInterrupt):
                return
            except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
                print(f"操作を停止しました（状態は保全）: {error}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("menu", "list", "submit", "consult", "follow-up", "recover", "consultation"))
    parser.add_argument("--request-file", type=Path)
    parser.add_argument("--request-id")
    parser.add_argument("--turn-id")
    args = parser.parse_args()
    try:
        if args.action == "menu":
            menu()
        elif args.action == "list":
            print(json.dumps(list_requests(), ensure_ascii=False, indent=2))
        elif args.action in {"consult", "follow-up", "recover", "consultation"}:
            if not args.request_id:
                raise ValueError("request-id is required")
            coordinator = coordinator_module()
            if args.action == "consultation":
                print(json.dumps(coordinator.read_state(args.request_id), ensure_ascii=False, indent=2))
            else:
                if args.action in {"follow-up", "recover"} and not args.request_file:
                    raise ValueError("follow-up requires request-file and turn-id")
                message = args.request_file.read_text(encoding="utf-8") if args.action in {"follow-up", "recover"} else None
                show_reply(coordinator.consult(args.request_id, message, args.turn_id, recover=args.action == "recover"))
        else:
            if not args.request_file:
                raise ValueError("submit requires --request-file")
            print(json.dumps(submit(args.request_file.read_text(encoding="utf-8"), args.request_id),
                             ensure_ascii=False, indent=2))
        return 0
    except (OSError, ValueError, RuntimeError, KeyError, TypeError) as error:
        print(f"受付を停止しました: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
