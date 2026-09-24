#!/usr/bin/env python3
"""Relay selected Cursor lifecycle hooks to one launcher-owned Orca bridge.

The Cursor agent never receives a Shell or MCP permission through this path.
This process is started by Cursor's controller-loaded hook mechanism and can
only address the private bridge endpoint/token inherited from the launcher.
"""

from __future__ import annotations

import json
import os
import socket
import sys
import uuid


MAX_INPUT = 512 * 1024
CURSOR_RESULT_FOLLOWUP = (
    "Return exactly one JSON object with only string keys outcome, subject, and body. "
    "Use outcome succeeded or failed, a short subject, and a three-sentence body. "
    "Do not invoke tools, mention lifecycle commands, use a code fence, or add any other text."
)
EVENT_FIELDS = {
    "beforeSubmitPrompt": {"prompt", "attachments"},
    "afterAgentResponse": {"text"},
    "stop": {"status", "loop_count"},
}
COMMON_FIELDS = {
    "conversation_id", "generation_id", "model", "model_id", "model_params",
    "hook_event_name", "cursor_version", "workspace_roots", "user_email",
    "transcript_path",
}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate JSON key")
        value[key] = item
    return value


def read_event() -> dict:
    raw = sys.stdin.buffer.read(MAX_INPUT + 1)
    if not raw or len(raw) > MAX_INPUT:
        raise ValueError("invalid Cursor hook input size")
    value = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(value, dict):
        raise ValueError("Cursor hook input must be an object")
    event = value.get("hook_event_name")
    if event not in EVENT_FIELDS:
        raise ValueError("unsupported Cursor hook event")
    # Cursor may add controller-owned metadata to hook payloads between CLI
    # releases. Keep the wire contract forward-compatible without forwarding
    # unknown values (which may contain data the bridge never requested).
    allowed = COMMON_FIELDS | EVENT_FIELDS[event]
    return {key: item for key, item in value.items() if key in allowed}


def exchange(event: dict) -> dict:
    endpoint = os.environ.get("ORCA_CURSOR_HOOK_ENDPOINT")
    token = os.environ.get("ORCA_CURSOR_HOOK_TOKEN")
    if not endpoint or not token:
        raise ValueError("Cursor hook bridge is unavailable")
    request = {"id": str(uuid.uuid4()), "authToken": token,
               "method": "cursor.hook", "params": event}
    payload = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
        client.settimeout(48)
        client.connect(endpoint)
        client.sendall(payload)
        chunks = bytearray()
        while b"\n" not in chunks:
            block = client.recv(65536)
            if not block or len(chunks) + len(block) > MAX_INPUT:
                raise ValueError("invalid Cursor hook bridge response")
            chunks.extend(block)
    response = json.loads(bytes(chunks).split(b"\n", 1)[0], object_pairs_hook=unique_object)
    if not isinstance(response, dict) or response.get("ok") is not True:
        raise ValueError("Cursor hook bridge refused the event")
    return response


def hook_output(event: dict, response: dict) -> dict:
    if event["hook_event_name"] == "beforeSubmitPrompt":
        result = response.get("result")
        if not isinstance(result, dict) or type(result.get("continue")) is not bool:
            raise ValueError("missing explicit Cursor admission decision")
        return {"continue": result["continue"]}
    if event["hook_event_name"] != "stop":
        return {}
    result = response.get("result")
    if not isinstance(result, dict):
        raise ValueError("invalid Cursor hook bridge result")
    followup = result.get("followup_message")
    if followup is None:
        return {}
    if followup != CURSOR_RESULT_FOLLOWUP:
        raise ValueError("invalid Cursor result follow-up")
    return {"followup_message": followup}


def main() -> int:
    try:
        event = read_event()
        response = exchange(event)
        output = hook_output(event, response)
        print(json.dumps(output, separators=(",", ":")))
        return 0
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        # Never print bridge data, hook input, tokens, or a potentially secret prompt.
        print('{"continue":false}')
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
