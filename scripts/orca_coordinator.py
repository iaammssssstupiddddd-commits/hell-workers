"""On-demand, read-only Codex consultations with durable exact-session resume.

Control state is never writable in the agent namespace. A successful conversation
turn is not an Orca Task settlement, implementation, validation or review approval.
"""

from __future__ import annotations

import hashlib
import json
import os
import selectors
import shutil
import signal
import stat
import subprocess
import tempfile
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

try:
    import orca_frontdesk as desk
    import orca_roles as roles
    from host_coordination import acquire_host
except ModuleNotFoundError:
    from scripts import orca_frontdesk as desk, orca_roles as roles
    from scripts.host_coordination import acquire_host


REPO = Path(__file__).resolve().parents[1]
MAX_REPLY = 128_000
MAX_EVENT_BYTES = 4 * 1024 * 1024
TURN_TIMEOUT = 600


def identity(value: str) -> str:
    if not isinstance(value, str) or str(uuid.UUID(value)) != value:
        raise ValueError("expected a canonical UUID")
    return value


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def request(request_id: str) -> dict:
    identity(request_id)
    for item in desk.list_requests():
        if item["id"] == request_id:
            return item
    raise ValueError("unknown intake request")


def state_path(request_id: str) -> Path:
    identity(request_id)
    return desk.checked_directory(desk.ledger_path().parent / "consultations") / f"{request_id}.json"


def read_state(request_id: str) -> dict:
    request(request_id)
    data = desk.read_private_json(state_path(request_id), {
        "schema": 1, "request_id": request_id, "repo": str(REPO), "session_id": None, "turns": [],
    })
    if (not isinstance(data, dict) or data.get("schema") != 1 or data.get("request_id") != request_id
            or data.get("repo") != str(REPO) or not isinstance(data.get("turns"), list)):
        raise ValueError("invalid coordinator state or different repository; preserve for reconciliation")
    if data.get("session_id") is not None:
        identity(data["session_id"])
    seen = set()
    for turn in data["turns"]:
        turn_id = identity(turn["id"])
        if turn_id in seen or turn.get("phase") not in {"starting", "running", "succeeded", "failed", "unknown"}:
            raise ValueError("duplicate or unknown consultation turn")
        seen.add(turn_id)
        if not isinstance(turn.get("message"), str) or not turn["message"].strip():
            raise ValueError("invalid saved consultation input")
        if turn.get("phase") == "succeeded":
            if (turn.get("exit_code") != 0 or turn.get("completed") is not True
                    or turn.get("thread_seen") is not True or not data.get("session_id")
                    or turn.get("session_id") != data["session_id"]
                    or not isinstance(turn.get("response"), str) or not turn["response"]):
                raise ValueError("unproven consultation success")
    return data


def save_state(data: dict) -> None:
    desk.write_ledger(state_path(data["request_id"]), data)


def session_exists(runtime: Path, session_id: str, repo: Path) -> None:
    matches = list((runtime / "codex/sessions").glob(f"**/*-{identity(session_id)}.jsonl"))
    if len(matches) != 1:
        raise ValueError("exact coordinator session is missing or ambiguous; never start a replacement")
    path = matches[0]
    info = path.lstat()
    if (path.resolve() != path or not stat.S_ISREG(info.st_mode)
            or info.st_uid != os.getuid() or info.st_nlink != 1):
        raise ValueError("unsafe coordinator session file")
    with path.open(encoding="utf-8") as handle:
        meta = json.loads(handle.readline(MAX_EVENT_BYTES))
    if (meta.get("type") != "session_meta" or meta.get("payload", {}).get("id") != session_id
            or meta["payload"].get("cwd") != str(repo)):
        raise ValueError("coordinator session metadata differs from recorded identity/cwd")


def provider_command(session_id: str | None) -> list[str]:
    executable = shutil.which("codex")
    if executable is None:
        raise RuntimeError("Codex CLI is not installed")
    return [executable, "--sandbox", "read-only", "--ask-for-approval", "never",
            "--disable", "multi_agent", "exec", *(["resume", identity(session_id)] if session_id else []),
            "--json", "--ignore-user-config", "--ignore-rules", "-"]


def prompt_for(item: dict, message: str, subject: dict, *, initial: bool) -> str:
    primary = Path(roles.git(REPO, "rev-parse", "--path-format=absolute", "--git-common-dir")).parent
    return ("Hell Workersの統括（相談・計画段階）です。日本語で回答してください。\n"
            "今回の権限はread-only調査とtask分割だけです。編集、build/test、別agent、Orca操作、"
            "commit/push/PR、認証fileの読取りは禁止。実装AはCodex、BはCursor CLIの単純leaf作業、"
            "reviewerは固定Codexです。実装は未dispatchであり、完了/承認と報告しないこと。\n"
            f"正本文書は {primary}/docs/orca-quickstart.md と docs/development-infra/orca-development.md。"
            f"source作業場は {REPO}。primaryの別作業や別branchを同一対象とみなさない。\n"
            f"対象: {json.dumps(subject, ensure_ascii=False)}\n"
            "依頼は以下のJSON。相談の応答として、必要ならA/B分担・成立条件・未解決点を簡潔に提示。\n"
            + json.dumps({"intake_id": item["id"], "current_message": message,
                          **({"original_request": item["request"]} if initial else {})}, ensure_ascii=False))


def apply_event(data: dict, turn: dict, event: dict) -> None:
    kind = event.get("type")
    if turn.get("completed"):
        raise ValueError("event received after turn completion")
    if kind == "thread.started":
        if turn.get("thread_seen"):
            raise ValueError("duplicate thread identity event")
        session_id = identity(event["thread_id"])
        if data["session_id"] not in (None, session_id):
            raise ValueError("provider resumed a different session")
        data["session_id"] = session_id
        turn["thread_seen"] = True
        turn["session_id"] = session_id
        turn["phase"] = "running"
        save_state(data)
    elif kind == "turn.started":
        if not turn.get("thread_seen") or turn.get("started"):
            raise ValueError("turn started without this attempt's unique thread identity")
        turn["started"] = True
    elif kind == "turn.completed":
        if not turn.get("started") or turn.get("completed"):
            raise ValueError("unexpected completion event")
        turn["completed"] = True
    elif kind in {"turn.failed", "error"}:
        raise RuntimeError("Codex reported a failed turn; inspect the session before resuming")
    elif kind == "item.completed" and event.get("item", {}).get("type") == "agent_message":
        if not turn.get("started"):
            raise ValueError("message before turn start")
        reply = event["item"].get("text")
        if not isinstance(reply, str) or len(reply) > MAX_REPLY:
            raise ValueError("invalid or oversized coordinator reply")
        turn["response"] = reply


def stop_and_wait(child: subprocess.Popen) -> None:
    # The process group was created by this invocation. Never target a discovered
    # terminal, another agent, or a saved PID; PID reuse is not authority to kill.
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


def execute(command: list[str], prompt: str, data: dict, turn: dict) -> int:
    """Stream identities durably; bound time/line buffers; always reap our child."""
    with tempfile.TemporaryFile(dir=state_path(data["request_id"]).parent) as prompt_input:
        prompt_input.write(prompt.encode())
        prompt_input.seek(0)
        return execute_stream(command, prompt_input, data, turn)


def execute_stream(command: list[str], prompt_input, data: dict, turn: dict) -> int:
    # File-backed stdin prevents a large prompt from blocking the parent before
    # it can drain stdout. Stderr goes directly to the operator, never a full pipe.
    with subprocess.Popen(command, stdin=prompt_input, stdout=subprocess.PIPE,
                          start_new_session=True) as child:
        try:
            with selectors.DefaultSelector() as poller:
                poller.register(child.stdout, selectors.EVENT_READ)
                pending = b""
                deadline = time.monotonic() + TURN_TIMEOUT
                while True:
                    if time.monotonic() >= deadline:
                        raise TimeoutError("coordinator turn timed out; no automatic resend")
                    if not poller.select(timeout=1):
                        continue
                    block = os.read(child.stdout.fileno(), 65536)
                    if not block:
                        if pending:
                            raise ValueError("truncated Codex JSON stream")
                        break
                    pending += block
                    while b"\n" in pending:
                        line, pending = pending.split(b"\n", 1)
                        if len(line) > MAX_EVENT_BYTES:
                            raise ValueError("oversized Codex event")
                        event = json.loads(line)
                        if not isinstance(event, dict):
                            raise ValueError("invalid Codex event")
                        apply_event(data, turn, event)
                    if len(pending) > MAX_EVENT_BYTES:
                        raise ValueError("oversized Codex event buffer")
            return child.wait(timeout=max(1, deadline - time.monotonic()))
        finally:
            stop_and_wait(child)
            turn["process_exited"] = child.poll() is not None
            turn["exit_code"] = child.returncode


def consult(request_id: str, message: str | None = None, turn_id: str | None = None,
            *, recover: bool = False) -> dict:
    item = request(request_id)
    initial = message is None
    if recover and initial:
        raise ValueError("recovery requires an explicit new message and turn UUID")
    if initial:
        message = item["request"]
        turn_id = str(uuid.uuid5(uuid.UUID(request_id), "initial-consultation"))
    elif not turn_id:
        raise ValueError("follow-up requires an explicit turn UUID for idempotent retry")
    identity(turn_id)
    if not isinstance(message, str) or not message.strip() or len(message) > 32_000:
        raise ValueError("consultation input must contain 1..32000 characters")
    with acquire_host("coordinator", inherit=False):
        data = read_state(request_id)
        request_hash = hashlib.sha256(item["request"].encode()).hexdigest()
        if data.get("request_sha256", request_hash) != request_hash:
            raise ValueError("intake content changed since this conversation began")
        data["request_sha256"] = request_hash
        for turn in data["turns"]:
            if turn["id"] == turn_id:
                if turn["message"] != message:
                    raise ValueError("turn UUID already belongs to a different message")
                if turn["phase"] != "succeeded":
                    raise RuntimeError("previous attempt is failed/unknown; never automatically resend")
                desk.sync_directory(state_path(request_id).parent)
                print("保存済み回答を再表示します（Codexは再起動しません）。", flush=True)
                return {**turn, "source_changed": turn.get("source_changed", False)
                        or roles.fingerprint(REPO) != turn["subject"]["source_sha256"]}
        unsettled = data["turns"] and data["turns"][-1]["phase"] != "succeeded"
        if unsettled:
            last = data["turns"][-1]
            if not recover or last.get("process_exited") is not True or not data["session_id"]:
                raise RuntimeError("unsettled consultation; explicit recovery needs proven child exit and session ID")
        elif recover:
            raise ValueError("no unsettled turn to recover; use follow-up")
        if not initial and not data["session_id"]:
            raise ValueError("consult first before sending a follow-up")
        branch = roles.git(REPO, "branch", "--show-current")
        common = roles.git(REPO, "rev-parse", "--path-format=absolute", "--git-common-dir")
        if data["turns"] and (data["turns"][0]["subject"].get("branch") != branch
                              or data["turns"][0]["subject"].get("git_common_dir") != common):
            raise ValueError("checkout branch/common directory changed; use a separate intake request")
        runtime = roles.prepare_runtime(f"coordinator/{request_id}")
        if data["session_id"]:
            session_exists(runtime, data["session_id"], REPO)
        elif any((runtime / "codex/sessions").glob("**/*.jsonl")):
            raise ValueError("unrecorded session exists; reconcile instead of replacing it")
        command = provider_command(data["session_id"])
        subject = {"repo": str(REPO), "head": roles.git(REPO, "rev-parse", "HEAD"),
                   "branch": branch, "git_common_dir": common,
                   "source_sha256": roles.fingerprint(REPO)}
        prompt = prompt_for(item, message, subject, initial=initial)
        if recover:
            prompt = ("復旧照合の新しいターンです。直前の処理結果は不明です。元の指示を再実行せず、"
                      "会話履歴から前回の到達点と未完事項を確認して返答してください。\n" + prompt)
        command = roles.sandbox_command({"repo": str(REPO), "allowed_directories": []},
                                        "reviewer", runtime, command)
        turn = {"id": turn_id, "message": message, "phase": "starting", "created_at": now(),
                "subject": subject, "terminal": os.environ.get("ORCA_TERMINAL_HANDLE"),
                "workspace": os.environ.get("ORCA_WORKTREE_ID"),
                "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
                "completed": False, "exit_code": None, "response": ""}
        if recover:
            turn["recovery_of"] = data["turns"][-1]["id"]
        data["turns"].append(turn)
        save_state(data)  # durable intent precedes spawn
        try:
            print(f"統括へ相談中: {request_id} / turn {turn_id}（read-only、未dispatch）", flush=True)
            code = execute(command, prompt, data, turn)
            turn["exit_code"] = code
            if code != 0 or not turn["completed"] or not turn["response"] or not data["session_id"]:
                raise RuntimeError("consultation lacks successful completion and a final response")
            session_exists(runtime, data["session_id"], REPO)
            turn["source_changed"] = roles.fingerprint(REPO) != subject["source_sha256"]
            turn["phase"] = "succeeded"
        except BaseException as error:
            turn["phase"] = "unknown"
            turn["error"] = type(error).__name__ + ": " + str(error)
            turn["ended_at"] = now()
            save_state(data)
            raise
        turn["ended_at"] = now()
        save_state(data)
        return turn
