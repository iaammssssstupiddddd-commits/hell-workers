#!/usr/bin/env python3
"""
Compatibility bridge for rust-analyzer-mcp v0.2.0.

The server is line-delimited JSON and does not fully follow rmcp's startup
notification handling. This adapter normalizes framing and filters problematic
startup notifications/responses for rmcp-based clients.
"""

from __future__ import annotations

import subprocess
import sys
import threading
import json
import os
from datetime import datetime, timezone
from typing import BinaryIO, Dict, Optional


_LOG_PATH = os.environ.get("RA_MCP_ADAPTER_LOG")


def _log(prefix: str, text: str) -> None:
    if not _LOG_PATH:
        return
    try:
        ts = datetime.now(timezone.utc).isoformat()
        with open(_LOG_PATH, "a", encoding="utf-8") as f:
            f.write(f"{ts} {prefix} {text}\n")
    except OSError:
        pass


def _read_exact(stream: BinaryIO, length: int) -> Optional[bytes]:
    chunks = bytearray()
    while len(chunks) < length:
        piece = stream.read(length - len(chunks))
        if not piece:
            return None
        chunks.extend(piece)
    return bytes(chunks)


def _is_content_length_header(line: bytes) -> bool:
    return line.lstrip().lower().startswith(b"content-length:")


def _decode_header_line(line: bytes) -> Optional[tuple[str, str]]:
    decoded = line.decode("ascii", errors="replace").strip()
    if ":" not in decoded:
        return None
    key, value = decoded.split(":", 1)
    return key.strip().lower(), value.strip()


def _read_mcp_payload(stream: BinaryIO) -> Optional[bytes]:
    """
    Read one MCP message payload.

    Supports both:
    - NDJSON framing (rmcp stdio): one JSON object per line
    - Content-Length framing (legacy adapters/clients)
    """
    first_line = stream.readline()
    if first_line == b"":
        return None

    if first_line in (b"\r\n", b"\n"):
        return b""

    if _is_content_length_header(first_line):
        headers: Dict[str, str] = {}

        parsed = _decode_header_line(first_line)
        if parsed:
            headers[parsed[0]] = parsed[1]

        while True:
            line = stream.readline()
            if line == b"":
                return None
            if line in (b"\r\n", b"\n"):
                break
            parsed = _decode_header_line(line)
            if parsed:
                headers[parsed[0]] = parsed[1]

        content_length = headers.get("content-length")
        if not content_length:
            _log("C->A", "missing content-length header")
            return b""

        try:
            length = int(content_length)
        except ValueError:
            _log("C->A", f"invalid content-length={content_length!r}")
            return b""

        payload = _read_exact(stream, length)
        return payload

    # NDJSON framing
    return first_line.strip()


def _pump_client_to_child(client_in: BinaryIO, child_in: BinaryIO) -> None:
    try:
        while True:
            payload = _read_mcp_payload(client_in)
            if payload is None:
                break

            if not payload:
                continue

            # rust-analyzer-mcp v0.2.0 reads requests line-by-line.
            # Normalize any pretty-printed JSON into compact one-line JSON.
            try:
                message = json.loads(payload.decode("utf-8"))

                # rmcp clients send notifications/initialized during startup.
                # rust-analyzer-mcp replies with an invalid JSON-RPC error
                # (id: null), which breaks rmcp's strict decoder.
                method = message.get("method")
                has_id = "id" in message and message["id"] is not None
                if isinstance(method, str) and method.startswith("notifications/") and not has_id:
                    _log("C->A", f"dropped notification: {method}")
                    continue

                compact = json.dumps(
                    message,
                    separators=(",", ":"),
                    ensure_ascii=False,
                ).encode("utf-8")
            except Exception:
                compact = payload.replace(b"\r", b" ").replace(b"\n", b" ")

            _log("C->A", compact.decode("utf-8", errors="replace"))

            child_in.write(compact)
            child_in.write(b"\n")
            child_in.flush()
    except (BrokenPipeError, OSError):
        pass
    finally:
        try:
            child_in.close()
        except OSError:
            pass


def _pump_child_to_client(child_out: BinaryIO, client_out: BinaryIO) -> None:
    while True:
        line = child_out.readline()
        if line == b"":
            break

        payload = line.strip()
        if not payload:
            continue

        # Drop invalid JSON-RPC error responses with id=null.
        # rmcp expects response ids to be non-null.
        try:
            decoded = json.loads(payload.decode("utf-8"))
            if (
                isinstance(decoded, dict)
                and decoded.get("id") is None
                and "error" in decoded
                and "method" not in decoded
            ):
                _log("A->C", "dropped invalid error response with id=null")
                continue
        except Exception:
            pass

        _log("A->C", payload.decode("utf-8", errors="replace"))

        # Codex rmcp stdio transport uses newline-delimited JSON framing.
        client_out.write(payload)
        client_out.write(b"\n")
        client_out.flush()


def _pump_stderr(child_err: BinaryIO, stderr_out: BinaryIO) -> None:
    try:
        while True:
            chunk = child_err.read(8192)
            if not chunk:
                break
            stderr_out.write(chunk)
            stderr_out.flush()
            _log("A[stderr]", chunk.decode("utf-8", errors="replace").rstrip())
    except OSError:
        pass


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "Usage: rust_analyzer_mcp_stdio_adapter.py <command> [args...]",
            file=sys.stderr,
        )
        return 2

    child = subprocess.Popen(
        sys.argv[1:],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=0,
    )

    assert child.stdin is not None
    assert child.stdout is not None
    assert child.stderr is not None

    stderr_thread = threading.Thread(
        target=_pump_stderr,
        args=(child.stderr, sys.stderr.buffer),
        daemon=True,
    )
    stderr_thread.start()

    input_thread = threading.Thread(
        target=_pump_client_to_child,
        args=(sys.stdin.buffer, child.stdin),
        daemon=True,
    )
    input_thread.start()

    try:
        _pump_child_to_client(child.stdout, sys.stdout.buffer)
    except (BrokenPipeError, OSError):
        pass
    finally:
        try:
            child.terminate()
        except OSError:
            pass

    input_thread.join(timeout=1.0)
    stderr_thread.join(timeout=1.0)

    try:
        return child.wait(timeout=2.0)
    except subprocess.TimeoutExpired:
        child.kill()
        return child.wait(timeout=2.0)


if __name__ == "__main__":
    raise SystemExit(main())
