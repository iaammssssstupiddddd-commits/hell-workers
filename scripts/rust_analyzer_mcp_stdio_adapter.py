#!/usr/bin/env python3
"""Share one rust-analyzer-mcp backend between stdio MCP clients.

``rust-analyzer-mcp`` only supports stdio and creates one rust-analyzer child
per invocation.  Editors and agent CLIs therefore duplicate a large analysis
database when they open the same Cargo workspace concurrently.  This adapter
keeps the public stdio contract, but forwards requests to an owner-only Unix
socket daemon keyed by the canonical workspace and relevant Cargo environment.

The daemon serializes requests because rust-analyzer-mcp 0.2.0 processes them
sequentially.  It releases the heavyweight backend after an idle timeout and
exits shortly after its last client disconnects.  A later request recreates the
backend transparently.
"""

from __future__ import annotations

import argparse
import contextlib
import errno
import fcntl
import hashlib
import json
import os
import signal
import socket
import socketserver
import stat
import subprocess
import sys
import tempfile
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import BinaryIO, Iterator, Optional, Sequence


DEFAULT_BACKEND_IDLE_SECONDS = 300.0
DEFAULT_DAEMON_IDLE_SECONDS = 30.0
RUNTIME_DIRECTORY_NAME = "hell-workers-rust-analyzer-mcp"
SOCKET_PATH_LIMIT = 100
ENVIRONMENT_IDENTITY_KEYS = (
    "CARGO_TARGET_DIR",
    "RUSTUP_TOOLCHAIN",
    "RUSTUP_HOME",
    "CARGO_HOME",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
)

_LOG_PATH = os.environ.get("RA_MCP_ADAPTER_LOG")


def _log(prefix: str, text: str) -> None:
    if not _LOG_PATH:
        return
    try:
        timestamp = datetime.now(timezone.utc).isoformat()
        with open(_LOG_PATH, "a", encoding="utf-8") as file:
            file.write(f"{timestamp} {prefix} {text}\n")
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
    """Read one NDJSON or Content-Length framed MCP message."""
    first_line = stream.readline()
    if first_line == b"":
        return None

    if first_line in (b"\r\n", b"\n"):
        return b""

    if not _is_content_length_header(first_line):
        return first_line.strip()

    headers: dict[str, str] = {}
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
        _log("C->D", "missing content-length header")
        return b""

    try:
        length = int(content_length)
    except ValueError:
        _log("C->D", f"invalid content-length={content_length!r}")
        return b""

    return _read_exact(stream, length)


def _normalize_client_payload(payload: bytes) -> Optional[bytes]:
    """Return one compact JSON line, or drop an MCP notification safely."""
    try:
        message = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return payload.replace(b"\r", b" ").replace(b"\n", b" ")

    if isinstance(message, dict):
        method = message.get("method")
        has_id = "id" in message and message["id"] is not None
        if isinstance(method, str) and method.startswith("notifications/") and not has_id:
            # rust-analyzer-mcp 0.2.0 emits an invalid id:null response for
            # these startup notifications, which rmcp rejects.
            _log("C->D", f"dropped notification: {method}")
            return None

    return json.dumps(message, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def _request_id(payload: bytes) -> object | None:
    try:
        message = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    if isinstance(message, dict):
        return message.get("id")
    return None


def _error_response(payload: bytes, message: str, code: int = -32000) -> bytes:
    return json.dumps(
        {
            "jsonrpc": "2.0",
            "id": _request_id(payload),
            "error": {"code": code, "message": message},
        },
        separators=(",", ":"),
    ).encode("utf-8")


def _is_workspace_switch(payload: bytes) -> bool:
    try:
        message = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return False
    if not isinstance(message, dict) or message.get("method") != "tools/call":
        return False
    params = message.get("params")
    return isinstance(params, dict) and params.get("name") == "rust_analyzer_set_workspace"


def _hide_shared_only_tools(response: bytes) -> bytes:
    """Do not advertise a tool that would mutate every connected client's root."""
    try:
        message = json.loads(response.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return response
    if not isinstance(message, dict):
        return response
    result = message.get("result")
    if not isinstance(result, dict) or not isinstance(result.get("tools"), list):
        return response

    tools = result["tools"]
    filtered = [
        tool
        for tool in tools
        if not isinstance(tool, dict) or tool.get("name") != "rust_analyzer_set_workspace"
    ]
    if len(filtered) == len(tools):
        return response
    result["tools"] = filtered
    return json.dumps(message, separators=(",", ":"), ensure_ascii=False).encode("utf-8")


def _parse_positive_seconds(value: str, *, variable: str) -> float:
    try:
        seconds = float(value)
    except ValueError as error:
        raise RuntimeError(f"{variable} must be a positive number of seconds") from error
    if seconds <= 0:
        raise RuntimeError(f"{variable} must be greater than zero")
    return seconds


def backend_idle_seconds(environment: dict[str, str] | None = None) -> float:
    environment = os.environ if environment is None else environment
    value = environment.get("HELL_WORKERS_RA_MCP_BACKEND_IDLE_SECONDS")
    if value is None:
        return DEFAULT_BACKEND_IDLE_SECONDS
    return _parse_positive_seconds(value, variable="HELL_WORKERS_RA_MCP_BACKEND_IDLE_SECONDS")


def daemon_idle_seconds(environment: dict[str, str] | None = None) -> float:
    environment = os.environ if environment is None else environment
    value = environment.get("HELL_WORKERS_RA_MCP_DAEMON_IDLE_SECONDS")
    if value is None:
        return DEFAULT_DAEMON_IDLE_SECONDS
    return _parse_positive_seconds(value, variable="HELL_WORKERS_RA_MCP_DAEMON_IDLE_SECONDS")


def _private_runtime_directory(path: Path) -> Path:
    path.mkdir(mode=0o700, parents=True, exist_ok=True)
    try:
        path.chmod(0o700)
    except OSError:
        pass
    return path


def runtime_directory(environment: dict[str, str] | None = None) -> Path:
    environment = os.environ if environment is None else environment
    configured = environment.get("HELL_WORKERS_RA_MCP_RUNTIME_DIR")
    if configured:
        return _private_runtime_directory(Path(configured).expanduser())

    runtime_root = environment.get("XDG_RUNTIME_DIR")
    if runtime_root:
        return _private_runtime_directory(Path(runtime_root) / RUNTIME_DIRECTORY_NAME)

    user_id = getattr(os, "getuid", lambda: 0)()
    return _private_runtime_directory(
        Path(tempfile.gettempdir()) / f"{RUNTIME_DIRECTORY_NAME}-{user_id}"
    )


def _git_workspace_root(start: Path) -> Optional[Path]:
    try:
        result = subprocess.run(
            ["git", "-C", str(start), "rev-parse", "--show-toplevel"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=2.0,
        )
    except (FileNotFoundError, subprocess.SubprocessError, OSError):
        return None

    root = result.stdout.strip()
    return Path(root).resolve() if root else None


def workspace_root(
    cwd: Path | None = None,
    environment: dict[str, str] | None = None,
) -> Path:
    """Resolve a stable root so subdirectories share the same backend."""
    environment = os.environ if environment is None else environment
    configured = environment.get("HELL_WORKERS_RA_MCP_WORKSPACE")
    start = Path(configured).expanduser() if configured else (cwd or Path.cwd())
    start = start.resolve()

    git_root = _git_workspace_root(start)
    if git_root is not None:
        return git_root

    for candidate in (start, *start.parents):
        if (candidate / "Cargo.toml").is_file():
            return candidate
    return start


def workspace_identity(root: Path, environment: dict[str, str] | None = None) -> str:
    environment = os.environ if environment is None else environment
    relevant_environment = {
        key: environment[key] for key in ENVIRONMENT_IDENTITY_KEYS if key in environment
    }
    encoded = json.dumps(
        {"workspace": str(root.resolve()), "environment": relevant_environment},
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()[:20]


def _fallback_runtime_directory() -> Path:
    user_id = getattr(os, "getuid", lambda: 0)()
    return _private_runtime_directory(
        Path(tempfile.gettempdir()) / f"{RUNTIME_DIRECTORY_NAME}-{user_id}"
    )


def socket_path(root: Path, environment: dict[str, str] | None = None) -> Path:
    identity = workspace_identity(root, environment)
    candidate = runtime_directory(environment) / f"ra-{identity}.sock"
    if len(os.fsencode(str(candidate))) < SOCKET_PATH_LIMIT:
        return candidate
    return _fallback_runtime_directory() / f"ra-{identity}.sock"


def _lock_path(path: Path) -> Path:
    return path.with_suffix(".lock")


@contextlib.contextmanager
def _startup_lock(path: Path) -> Iterator[None]:
    descriptor = os.open(path, os.O_CREAT | os.O_RDWR, 0o600)
    try:
        fcntl.flock(descriptor, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        os.close(descriptor)


def _connect(path: Path) -> socket.socket:
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.settimeout(1.0)
    try:
        client.connect(str(path))
    except BaseException:
        client.close()
        raise
    client.settimeout(None)
    return client


def _remove_stale_socket(path: Path) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if not stat.S_ISSOCK(mode):
        raise RuntimeError(f"refusing to remove non-socket shared MCP path: {path}")
    path.unlink()


def _connection_is_unavailable(error: OSError) -> bool:
    return error.errno in {errno.ENOENT, errno.ECONNREFUSED, errno.ECONNRESET}


def _daemon_command(
    workspace: Path,
    endpoint: Path,
    command: Sequence[str],
    backend_idle: float,
    daemon_idle: float,
) -> list[str]:
    return [
        sys.executable,
        str(Path(__file__).resolve()),
        "--daemon",
        "--workspace",
        str(workspace),
        "--socket",
        str(endpoint),
        "--backend-idle-seconds",
        str(backend_idle),
        "--daemon-idle-seconds",
        str(daemon_idle),
        "--command-json",
        json.dumps(list(command)),
    ]


def _start_daemon(
    workspace: Path,
    endpoint: Path,
    command: Sequence[str],
    backend_idle: float,
    daemon_idle: float,
) -> None:
    subprocess.Popen(
        _daemon_command(workspace, endpoint, command, backend_idle, daemon_idle),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        close_fds=True,
        start_new_session=True,
    )


def connect_shared_backend(
    workspace: Path,
    command: Sequence[str],
    *,
    environment: dict[str, str] | None = None,
) -> socket.socket:
    """Connect to, or safely create, the singleton backend for ``workspace``."""
    endpoint = socket_path(workspace, environment)
    try:
        return _connect(endpoint)
    except OSError as error:
        if not _connection_is_unavailable(error):
            raise RuntimeError(f"cannot connect to shared rust-analyzer MCP: {error}") from error

    with _startup_lock(_lock_path(endpoint)):
        try:
            return _connect(endpoint)
        except OSError as error:
            if not _connection_is_unavailable(error):
                raise RuntimeError(f"cannot connect to shared rust-analyzer MCP: {error}") from error

        _remove_stale_socket(endpoint)
        _start_daemon(
            workspace,
            endpoint,
            command,
            backend_idle_seconds(environment),
            daemon_idle_seconds(environment),
        )

        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            try:
                return _connect(endpoint)
            except OSError as error:
                if not _connection_is_unavailable(error):
                    raise RuntimeError(f"cannot connect to shared rust-analyzer MCP: {error}") from error
                time.sleep(0.05)

    raise RuntimeError("shared rust-analyzer MCP daemon did not start within 10 seconds")


def _drain_backend_stderr(stream: BinaryIO) -> None:
    try:
        while True:
            line = stream.readline()
            if not line:
                return
            _log("backend[stderr]", line.decode("utf-8", errors="replace").rstrip())
    except OSError:
        return


class Backend:
    """A restartable, serialized stdio connection to rust-analyzer-mcp."""

    def __init__(
        self,
        workspace: Path,
        command: Sequence[str],
        idle_seconds: float,
    ) -> None:
        self.workspace = workspace
        self.command = list(command)
        self.idle_seconds = idle_seconds
        self._lock = threading.Lock()
        self._process: subprocess.Popen[bytes] | None = None
        self._last_request: float | None = None

    def _start_locked(self) -> None:
        if not self.command:
            raise RuntimeError("rust-analyzer-mcp command is empty")
        self._process = subprocess.Popen(
            [*self.command, str(self.workspace)],
            cwd=self.workspace,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
            start_new_session=True,
        )
        assert self._process.stderr is not None
        threading.Thread(
            target=_drain_backend_stderr,
            args=(self._process.stderr,),
            daemon=True,
        ).start()

    def _stop_locked(self) -> None:
        process = self._process
        self._process = None
        self._last_request = None
        if process is None:
            return
        try:
            if process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
        except OSError:
            pass
        try:
            process.wait(timeout=3.0)
        except subprocess.TimeoutExpired:
            self._terminate_process_group(process, signal.SIGTERM)
            try:
                process.wait(timeout=2.0)
            except subprocess.TimeoutExpired:
                self._terminate_process_group(process, signal.SIGKILL)
                process.wait(timeout=2.0)
        finally:
            # rust-analyzer-mcp normally shuts its rust-analyzer child down on
            # EOF.  Retire the dedicated process group as well so a crashed
            # MCP wrapper cannot leave a heavyweight child behind.
            self._terminate_process_group(process, signal.SIGTERM)
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()

    @staticmethod
    def _terminate_process_group(process: subprocess.Popen[bytes], signal_number: int) -> None:
        try:
            os.killpg(process.pid, signal_number)
        except (OSError, ProcessLookupError):
            try:
                if signal_number == signal.SIGKILL:
                    process.kill()
                else:
                    process.terminate()
            except OSError:
                pass

    def request(self, payload: bytes) -> bytes:
        with self._lock:
            if self._process is None or self._process.poll() is not None:
                self._stop_locked()
                self._start_locked()

            assert self._process is not None
            assert self._process.stdin is not None
            assert self._process.stdout is not None
            try:
                self._process.stdin.write(payload)
                self._process.stdin.write(b"\n")
                self._process.stdin.flush()
                response = self._process.stdout.readline()
            except (BrokenPipeError, OSError) as error:
                self._stop_locked()
                raise RuntimeError("shared rust-analyzer MCP backend stopped unexpectedly") from error

            if not response:
                self._stop_locked()
                raise RuntimeError("shared rust-analyzer MCP backend closed its response stream")

            self._last_request = time.monotonic()
            return response.strip()

    def stop_if_idle(self, now: float) -> None:
        with self._lock:
            if (
                self._process is not None
                and self._last_request is not None
                and now - self._last_request >= self.idle_seconds
            ):
                _log("daemon", "stopping idle rust-analyzer MCP backend")
                self._stop_locked()

    def close(self) -> None:
        with self._lock:
            self._stop_locked()


class SharedBackendState:
    def __init__(self, backend: Backend) -> None:
        self.backend = backend
        self._clients_lock = threading.Lock()
        self._clients = 0
        self._last_client_disconnect = time.monotonic()

    def client_connected(self) -> None:
        with self._clients_lock:
            self._clients += 1

    def client_disconnected(self) -> None:
        with self._clients_lock:
            self._clients = max(0, self._clients - 1)
            if self._clients == 0:
                self._last_client_disconnect = time.monotonic()

    def daemon_is_idle(self, now: float, idle_seconds: float) -> bool:
        with self._clients_lock:
            return self._clients == 0 and now - self._last_client_disconnect >= idle_seconds

    def handle(self, payload: bytes) -> bytes:
        if _is_workspace_switch(payload):
            return _error_response(
                payload,
                "rust_analyzer_set_workspace is unavailable while the backend is shared",
                code=-32001,
            )
        try:
            response = self.backend.request(payload)
        except (OSError, RuntimeError) as error:
            return _error_response(payload, str(error))
        response = _hide_shared_only_tools(response)
        _log("D->C", response.decode("utf-8", errors="replace"))
        return response


class SharedUnixServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True

    def __init__(self, endpoint: Path, state: SharedBackendState) -> None:
        self.state = state
        super().__init__(str(endpoint), SharedClientHandler)


class SharedClientHandler(socketserver.StreamRequestHandler):
    _registered = False

    def setup(self) -> None:
        super().setup()
        self.server.state.client_connected()  # type: ignore[attr-defined]
        self._registered = True

    def handle(self) -> None:
        while True:
            payload = self.rfile.readline()
            if not payload:
                return
            compact = payload.strip()
            if not compact:
                continue
            response = self.server.state.handle(compact)  # type: ignore[attr-defined]
            self.wfile.write(response)
            self.wfile.write(b"\n")
            self.wfile.flush()

    def finish(self) -> None:
        if self._registered:
            self.server.state.client_disconnected()  # type: ignore[attr-defined]
        super().finish()


def run_daemon(
    workspace: Path,
    endpoint: Path,
    command: Sequence[str],
    *,
    backend_idle: float,
    daemon_idle: float,
) -> int:
    if endpoint.exists():
        raise RuntimeError(f"shared rust-analyzer MCP socket already exists: {endpoint}")

    backend = Backend(workspace, command, backend_idle)
    state = SharedBackendState(backend)
    server = SharedUnixServer(endpoint, state)
    try:
        endpoint.chmod(0o600)
    except OSError:
        pass

    shutdown_requested = threading.Event()
    previous_handlers: dict[int, object] = {}

    def request_shutdown(_signal_number: int, _frame: object) -> None:
        shutdown_requested.set()

    for signal_number in (signal.SIGINT, signal.SIGTERM):
        try:
            previous_handlers[signal_number] = signal.signal(signal_number, request_shutdown)
        except ValueError:
            # run_daemon is normally the daemon process main thread. Keeping
            # this callable in tests is useful even when that is not true.
            pass

    serving = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.1})
    serving.start()
    _log("daemon", f"started shared backend for {workspace}")
    try:
        while True:
            time.sleep(0.1)
            now = time.monotonic()
            backend.stop_if_idle(now)
            if shutdown_requested.is_set() or state.daemon_is_idle(now, daemon_idle):
                return 0
    finally:
        # Keep a new proxy from mistaking this endpoint for a stale socket
        # between server_close() and unlink().
        with _startup_lock(_lock_path(endpoint)):
            server.shutdown()
            serving.join(timeout=2.0)
            server.server_close()
            backend.close()
            try:
                _remove_stale_socket(endpoint)
            except RuntimeError:
                pass
        for signal_number, previous_handler in previous_handlers.items():
            signal.signal(signal_number, previous_handler)
        _log("daemon", f"stopped shared backend for {workspace}")


def _daemon_main(argv: Sequence[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--socket", required=True)
    parser.add_argument("--backend-idle-seconds", type=float, required=True)
    parser.add_argument("--daemon-idle-seconds", type=float, required=True)
    parser.add_argument("--command-json", required=True)
    args = parser.parse_args(argv)

    try:
        command = json.loads(args.command_json)
    except json.JSONDecodeError as error:
        raise RuntimeError("daemon command JSON is invalid") from error
    if not isinstance(command, list) or not all(isinstance(item, str) for item in command):
        raise RuntimeError("daemon command JSON must be an array of strings")
    if args.backend_idle_seconds <= 0 or args.daemon_idle_seconds <= 0:
        raise RuntimeError("daemon idle timeouts must be greater than zero")

    return run_daemon(
        Path(args.workspace).resolve(),
        Path(args.socket),
        command,
        backend_idle=args.backend_idle_seconds,
        daemon_idle=args.daemon_idle_seconds,
    )


def run_proxy(command: Sequence[str]) -> int:
    if not command:
        print(
            "Usage: rust_analyzer_mcp_stdio_adapter.py <command> [args...]",
            file=sys.stderr,
        )
        return 2

    workspace = workspace_root()
    try:
        client = connect_shared_backend(workspace, command)
    except RuntimeError as error:
        print(f"rust-analyzer MCP shared backend unavailable: {error}", file=sys.stderr)
        return 1

    try:
        with client, client.makefile("rwb") as daemon_stream:
            while True:
                payload = _read_mcp_payload(sys.stdin.buffer)
                if payload is None:
                    return 0
                if not payload:
                    continue

                compact = _normalize_client_payload(payload)
                if compact is None:
                    continue
                _log("C->D", compact.decode("utf-8", errors="replace"))
                try:
                    daemon_stream.write(compact)
                    daemon_stream.write(b"\n")
                    daemon_stream.flush()
                    response = daemon_stream.readline().strip()
                except (BrokenPipeError, OSError) as error:
                    response = _error_response(
                        compact,
                        f"shared rust-analyzer MCP connection closed: {error}",
                    )

                if not response:
                    response = _error_response(
                        compact,
                        "shared rust-analyzer MCP backend returned no response",
                    )
                sys.stdout.buffer.write(response)
                sys.stdout.buffer.write(b"\n")
                sys.stdout.buffer.flush()
    except OSError:
        return 0


def main(argv: Sequence[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv[:1] == ["--daemon"]:
        return _daemon_main(argv[1:])
    return run_proxy(argv)


if __name__ == "__main__":
    raise SystemExit(main())
