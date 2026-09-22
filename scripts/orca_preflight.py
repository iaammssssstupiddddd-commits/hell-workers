"""Read-only Orca transport/readiness probe; never creates or dispatches work.

Only the host holds Orca's real token. The stock CLI sees an ephemeral proxy
bootstrap bound to one runtime, terminal incarnation and canonical worktree.
This is a preflight boundary, NOT the separate Task lifecycle bridge.
"""

from __future__ import annotations

import argparse
import hmac
import json
import os
import secrets
import selectors
import shutil
import signal
import socket
import socketserver
import stat
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path

if __package__:
    from .host_coordination import state_root
    from .orca_frontdesk import checked_directory, write_ledger
else:
    from host_coordination import state_root
    from orca_frontdesk import checked_directory, write_ledger


VERSION = "1.4.205"
MAX_FRAME = 262_144
MAX_WAIT_MS = 15_000
# Installed 1.4.205 main runtime: G4i prompt detector and K3i.wait.
WAIT_STATUS = frozenset({"running", "unknown", "exited"})
BLOCKED_REASON = frozenset({
    "agent-update-prompt", "agent-cwd-prompt", "codex-model-migration-prompt",
    "agent-hooks-review-prompt", "agent-trust-workspace", "agent-interactive-prompt",
    "agent-approval-prompt",
})


class Refused(RuntimeError):
    """Fail-closed refusal with no raw upstream content or credentials."""


def mapping(value: object) -> dict:
    if not isinstance(value, dict):
        raise Refused("expected an object")
    return value


def object_pairs(pairs: list) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise Refused("duplicate JSON key")
        result[key] = value
    return result


def decode(raw: bytes) -> dict:
    def reject_constant(_):
        raise Refused("non-finite JSON value")
    if len(raw) > MAX_FRAME:
        raise Refused("oversized frame")
    value = json.loads(raw, object_pairs_hook=object_pairs, parse_constant=reject_constant)
    return mapping(value)


def identifier(value: object, *, prefix: str = "") -> str:
    if not isinstance(value, str) or not value.startswith(prefix):
        raise Refused("invalid identity")
    tail = value[len(prefix):]
    if str(uuid.UUID(tail)) != tail:
        raise Refused("invalid identity")
    return value


def receive(stream: socket.socket, deadline: float) -> dict:
    pending = b""
    total = 0
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise Refused("absolute response deadline exceeded")
        stream.settimeout(remaining)
        block = stream.recv(min(65536, MAX_FRAME + 1 - len(pending)))
        if not block:
            raise Refused("connection closed before response")
        pending += block
        total += len(block)
        if total > MAX_FRAME:
            raise Refused("oversized response stream")
        while b"\n" in pending:
            line, pending = pending.split(b"\n", 1)
            value = decode(line)
            if value == {"_keepalive": True}:
                continue
            if pending:
                raise Refused("multiple response frames")
            return value


@dataclass(repr=False)
class Upstream:
    metadata_path: Path
    endpoint: Path
    runtime_id: str
    token: str = field(repr=False)

    @classmethod
    def load(cls, directory: Path) -> Upstream:
        path = directory / "orca-runtime.json"
        info = path.lstat()
        if (path.resolve() != path or not stat.S_ISREG(info.st_mode)
                or info.st_uid != os.getuid() or info.st_nlink != 1 or info.st_mode & 0o077):
            raise Refused("unsafe upstream bootstrap")
        with path.open("rb") as handle:
            data = decode(handle.read(MAX_FRAME + 1))
        identity = identifier(data.get("runtimeId"))
        token = data.get("authToken")
        if not isinstance(token, str) or not 16 <= len(token) <= 1024:
            raise Refused("invalid upstream authentication")
        transports = data.get("transports")
        if not isinstance(transports, list):
            raise Refused("unsupported upstream bootstrap")
        unix = [item for item in transports if isinstance(item, dict) and item.get("kind") == "unix"]
        if len(unix) != 1 or not isinstance(unix[0].get("endpoint"), str):
            raise Refused("ambiguous upstream transport")
        endpoint = Path(unix[0]["endpoint"])
        info = endpoint.lstat()
        if (not endpoint.is_absolute() or endpoint.resolve() != endpoint
                or not stat.S_ISSOCK(info.st_mode) or info.st_uid != os.getuid()):
            raise Refused("unsafe upstream socket")
        return cls(path, endpoint, identity, token)

    def call(self, method: str, params: dict | None = None) -> dict:
        # Even host-side callers cannot accidentally turn this into a mutation client.
        if method not in {"status.get", "terminal.show", "terminal.wait"}:
            raise Refused("upstream method denied")
        return self._exchange(method, params)

    def _exchange(self, method: str, params: dict | None, envelope: dict | None = None,
                  *, absolute_deadline: float | None = None) -> dict:
        # Task policies use a separate allowlist; the public preflight stays read-only.
        latest = self.load(self.metadata_path.parent)
        if (latest.runtime_id != self.runtime_id or latest.endpoint != self.endpoint
                or not hmac.compare_digest(latest.token, self.token)):
            raise Refused("upstream identity changed")
        request_id = str(uuid.uuid4())
        request = {"id": request_id, "authToken": self.token, "method": method}
        if envelope:
            if envelope.keys() - {"orchestrationCapability", "orchestrationContractVersion",
                                  "orchestrationRequestId", "compatibilityInvocationId"}:
                raise Refused("unsupported transport envelope")
            request.update(envelope)
        if params is not None:
            request["params"] = params
        timeout = 3 + (params.get("timeoutMs", 0) / 1000 if params else 0)
        if absolute_deadline is not None:
            timeout = min(timeout, absolute_deadline - time.monotonic())
        if timeout <= 0:
            raise Refused("absolute operation deadline exceeded")
        deadline = time.monotonic() + timeout
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(timeout)
            stream.connect(str(self.endpoint))
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Refused("absolute upstream deadline exceeded")
            stream.settimeout(remaining)
            stream.sendall(json.dumps(request).encode() + b"\n")
            response = receive(stream, deadline)
        if (response.get("id") != request_id
                or mapping(response.get("_meta")).get("runtimeId") != self.runtime_id):
            raise Refused("upstream response identity differs")
        if response.get("ok") is not True or not isinstance(response.get("result"), dict):
            # Raw errors may contain tokens, prompts or other terminal information.
            raise Refused("upstream did not confirm the observation")
        return response["result"]


@dataclass(frozen=True)
class Binding:
    terminal: str
    incarnation: str
    worktree: str
    repo: Path

    @classmethod
    def discover(cls, upstream: Upstream, terminal: str, repo: Path) -> Binding:
        identifier(terminal, prefix="term_")
        if not repo.is_dir() or repo.resolve() != repo:
            raise Refused("worktree must be a canonical absolute directory")
        row = mapping(upstream.call("terminal.show", {"terminal": terminal}).get("terminal"))
        binding = cls(terminal, identifier(row.get("incarnationId")), row.get("worktreeId"), repo)
        binding.validate(row)
        return binding

    def validate(self, row: dict) -> None:
        if (not isinstance(row, dict) or row.get("handle") != self.terminal
                or row.get("incarnationId") != self.incarnation
                or not isinstance(self.worktree, str) or not self.worktree
                or row.get("worktreeId") != self.worktree or row.get("worktreePath") != str(self.repo)
                or row.get("executionHostId") != "local" or row.get("connected") is not True
                or row.get("orphaned") is not False):
            raise Refused("terminal binding changed or is unproven")

    def project(self, row: dict) -> dict:
        self.validate(row)
        return {key: row[key] for key in ("handle", "incarnationId", "worktreeId", "worktreePath",
                                         "executionHostId", "connected", "orphaned")}


def project_wait(value: object, binding: Binding) -> dict:
    observed = mapping(value)
    if (type(observed.get("satisfied")) is not bool or observed.get("handle") != binding.terminal
            or observed.get("condition") != "tui-idle"
            or not isinstance(observed.get("status"), str) or observed["status"] not in WAIT_STATUS):
        raise Refused("unproven readiness result")
    result = {key: observed[key] for key in ("handle", "condition", "satisfied", "status")}
    if "blockedReason" in observed:
        reason = observed["blockedReason"]
        if not isinstance(reason, str) or reason not in BLOCKED_REASON or observed["satisfied"]:
            raise Refused("unknown or contradictory readiness reason")
        result["blockedReason"] = reason
    return result


class Policy:
    def __init__(self, upstream: Upstream, binding: Binding):
        self.upstream = upstream
        self.binding = binding
        self.token = secrets.token_hex(32)
        self.revoked = False
        self.remaining = 16

    def handle(self, request: dict) -> dict:
        request_id = "refused"
        try:
            request_id = identifier(mapping(request).get("id"))
            if self.revoked or self.remaining <= 0:
                raise Refused("proxy no longer active")
            self.remaining -= 1
            if (not {"id", "authToken", "method"} <= request.keys()
                    or request.keys() - {"id", "authToken", "method", "params"}
                    or not isinstance(request.get("authToken"), str)
                    or not hmac.compare_digest(request["authToken"], self.token)):
                raise Refused("request envelope denied")
            method, params = request["method"], request.get("params")
            if method == "status.get":
                if params not in (None, {}):
                    raise Refused("status parameters denied")
            elif method == "terminal.show":
                if params != {"terminal": self.binding.terminal}:
                    raise Refused("terminal parameters denied")
            elif method == "terminal.wait":
                if (not isinstance(params, dict) or set(params) != {"terminal", "for", "timeoutMs"}
                        or params["terminal"] != self.binding.terminal or params["for"] != "tui-idle"
                        or type(params["timeoutMs"]) is not int or not 1 <= params["timeoutMs"] <= MAX_WAIT_MS):
                    raise Refused("wait parameters denied")
            else:
                raise Refused("method denied")
            # Every request verifies current terminal ownership before accessing state.
            current = self.upstream.call("terminal.show", {"terminal": self.binding.terminal})
            self.binding.validate(current.get("terminal", {}))
            if method == "terminal.show":
                result = {"terminal": self.binding.project(current["terminal"])}
            elif method == "status.get":
                status = self.upstream.call(method)
                if (status.get("runtimeId") != self.upstream.runtime_id
                        or status.get("appVersion") != VERSION or status.get("graphStatus") != "ready"
                        or status.get("remoteControl") is not None):
                    raise Refused("unsupported or unready runtime")
                result = {key: status[key] for key in ("runtimeId", "appVersion", "graphStatus")}
            else:
                observed = self.upstream.call(method, params).get("wait")
                self.binding.validate(self.upstream.call("terminal.show", {
                    "terminal": self.binding.terminal})["terminal"])
                result = {"wait": project_wait(observed, self.binding)}
            return {"id": request_id, "ok": True, "result": result,
                    "_meta": {"runtimeId": self.upstream.runtime_id}}
        except (OSError, ValueError, TypeError, KeyError, RuntimeError):
            # All violations revoke this short-lived probe, including partial transport results.
            self.revoked = True
            return {"id": request_id, "ok": False,
                    "error": {"code": "preflight_refused", "message": "Observation refused; no dispatch authorized."},
                    "_meta": {"runtimeId": self.upstream.runtime_id}}


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            request = receive(self.request, time.monotonic() + 2)
            result = self.server.policy.handle(request)
            self.request.sendall(json.dumps(result).encode() + b"\n")
        except (OSError, ValueError, RuntimeError):
            self.server.policy.revoked = True


class Proxy(socketserver.UnixStreamServer):
    # Sequential handling bounds threads and simultaneous upstream waits to one.
    request_queue_size = 2

    def __init__(self, path: Path, policy: Policy):
        self.policy = policy
        super().__init__(str(path), Handler)
        os.chmod(path, 0o600)


def sandbox(executable: Path, directory: Path, upstream: Upstream, repo: Path, argv: list[str]) -> list[str]:
    bwrap = shutil.which("bwrap")
    if sys.platform != "linux" or not bwrap:
        raise Refused("Linux bubblewrap is required")
    if not executable.is_file() or not executable.is_absolute():
        raise Refused("an explicit installed Orca executable is required")
    # No account auth or project config is needed by this non-LLM probe.
    result = [bwrap, "--die-with-parent", "--new-session", "--unshare-pid", "--unshare-net",
              "--ro-bind", "/", "/", "--proc", "/proc", "--dev", "/dev",
              "--tmpfs", "/tmp", "--tmpfs", "/run", "--clearenv",
              "--tmpfs", str(upstream.metadata_path.parent),
              "--ro-bind", "/dev/null", str(upstream.endpoint),
              "--ro-bind", str(executable), str(executable),
              "--ro-bind", str(directory), str(directory),
              "--setenv", "ORCA_USER_DATA_PATH", str(directory),
              "--setenv", "PATH", os.defpath,
              "--setenv", "HOME", "/tmp", "--setenv", "TMPDIR", "/tmp",
              "--setenv", "LANG", "C.UTF-8", "--chdir", str(repo), "--", str(executable), *argv]
    return result


def run_cli(command: list[str], timeout: float) -> subprocess.CompletedProcess:
    """Bound both pipes while reading, and reap the CLI process group on every exit."""
    deadline = time.monotonic() + timeout
    with subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, start_new_session=True, umask=0o077) as process:
        output = {process.stdout: bytearray(), process.stderr: bytearray()}
        try:
            with selectors.DefaultSelector() as selector:
                for stream in output:
                    selector.register(stream, selectors.EVENT_READ)
                while selector.get_map():
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise Refused("absolute CLI deadline exceeded")
                    for key, _ in selector.select(remaining):
                        stream = key.fileobj
                        block = os.read(stream.fileno(), min(65536, MAX_FRAME + 1 - len(output[stream])))
                        if not block:
                            selector.unregister(stream)
                        else:
                            output[stream].extend(block)
                            if len(output[stream]) > MAX_FRAME:
                                raise Refused("oversized CLI output")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise Refused("absolute CLI deadline exceeded")
            code = process.wait(timeout=remaining)
            return subprocess.CompletedProcess(command, code, bytes(output[process.stdout]),
                                               bytes(output[process.stderr]))
        finally:
            # Also stop descendants holding a pipe open after their parent exits.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()


def cli_result(output: dict, kind: str, code: int, upstream: Upstream, binding: Binding) -> dict:
    if output.get("ok") is not True:
        raise Refused("isolated CLI could not confirm preflight")
    result = mapping(output.get("result"))
    if kind == "status":
        runtime, graph = mapping(result.get("runtime")), mapping(result.get("graph"))
        if (code or runtime.get("reachable") is not True or runtime.get("runtimeId") != upstream.runtime_id
                or graph.get("state") != "ready"):
            raise Refused("runtime status is not ready")
        return {}
    if kind == "show":
        if code:
            raise Refused("terminal observation failed")
        return {"terminal": binding.project(result.get("terminal"))}
    if kind != "wait":
        raise Refused("unexpected CLI command")
    observed = mapping(result.get("wait"))
    satisfied = observed.get("satisfied")
    if type(satisfied) is not bool or code != (0 if satisfied else 1):
        raise Refused("readiness receipt contradicts exit")
    return {"wait": project_wait(observed, binding)}


def probe(executable: Path, metadata_dir: Path, terminal: str, repo: Path, wait_ms: int = 0) -> dict:
    if type(wait_ms) is not int or not 0 <= wait_ms <= MAX_WAIT_MS:
        raise Refused("wait must be between 0 and 15000 milliseconds")
    upstream = Upstream.load(metadata_dir)
    binding = Binding.discover(upstream, terminal, repo)
    policy = Policy(upstream, binding)
    root = checked_directory(state_root().parent / "preflight")
    # Only our temporary proxy files are removed, after CLI and server both exit.
    with tempfile.TemporaryDirectory(prefix="p-", dir=root) as temporary:
        directory = Path(temporary)
        endpoint = directory / "rpc.sock"
        metadata = {"runtimeId": upstream.runtime_id, "authToken": policy.token,
                    "transports": [{"kind": "unix", "endpoint": str(endpoint)}]}
        write_ledger(directory / "orca-runtime.json", metadata)
        with Proxy(endpoint, policy) as server:
            thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.05})
            thread.start()
            outputs = []
            try:
                commands = [["status", "--json"], ["terminal", "show", "--terminal", terminal, "--json"]]
                if wait_ms:
                    commands.append(["terminal", "wait", "--terminal", terminal, "--for", "tui-idle",
                                     "--timeout-ms", str(wait_ms), "--json"])
                for argv in commands:
                    completed = run_cli(sandbox(executable, directory, upstream, repo, argv),
                                        timeout=wait_ms / 1000 + 8)
                    output = decode(completed.stdout)
                    if policy.revoked:
                        raise Refused("isolated CLI could not confirm preflight")
                    kind = "status" if argv[0] == "status" else argv[1]
                    outputs.append(cli_result(output, kind, completed.returncode, upstream, binding))
            finally:
                policy.revoked = True
                # shutdown waits for the current handler's bounded receive/upstream
                # deadlines (at most 2 + 3 + 18 + 3 seconds), then stops accepting.
                server.shutdown()
                thread.join()
    return {"transport": "verified", "runtime_id": upstream.runtime_id, "terminal": binding.terminal,
            "incarnation": binding.incarnation, "repo": str(repo), "dispatch_allowed": False,
            "readiness": outputs[-1]["wait"] if wait_ms else "not-checked"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--orca", type=Path, required=True)
    parser.add_argument("--metadata-dir", type=Path, required=True)
    parser.add_argument("--terminal", required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--wait-ms", type=int, default=0)
    args = parser.parse_args()
    try:
        print(json.dumps(probe(args.orca, args.metadata_dir, args.terminal, args.repo, args.wait_ms)))
        return 0
    except (OSError, ValueError, TypeError, KeyError, RuntimeError, subprocess.SubprocessError):
        # Never echo CLI stderr, raw RPCs, bootstrap content or exception reprs.
        print("Orca preflight refused; no task was created and no dispatch is authorized.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
