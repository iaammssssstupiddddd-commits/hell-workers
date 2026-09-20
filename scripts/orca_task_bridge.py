"""Single-Dispatch, host-owned Orca bridge for guarded read-only Codex roles.

This does not create a Run/Task, start a worker or approve a review. The host
arms one observed supervised Dispatch after readiness. Never reassign its
terminal until the bridge is closed and all in-flight requests have drained:
Orca 1.4.205 check has no atomic expected-Dispatch parameter.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
import re
import secrets
import socketserver
import threading
import time
import uuid
from pathlib import Path

try:
    import orca_preflight as wire
    from orca_frontdesk import read_private_json, write_ledger, checked_directory
    from host_coordination import acquire_host, state_root
except ModuleNotFoundError:
    from scripts import orca_preflight as wire
    from scripts.orca_frontdesk import read_private_json, write_ledger, checked_directory
    from scripts.host_coordination import acquire_host, state_root


CONTRACT = "orchestration.contract.v1"
ARM_SECONDS = 5
REQUEST_SECONDS = 45
KEEPALIVE_SECONDS = 1
METHODS = {"orchestration.send", "orchestration.check", "orchestration.ask"}
FIELDS = {"orchestrationContractVersion", "orchestrationRequestId", "compatibilityInvocationId"}


def key(value: object) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", value):
        raise wire.Refused("invalid lifecycle identity")
    return value


def digest(value: object) -> str:
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def root() -> Path:
    return checked_directory(state_root().parent / "task-bridges")


def lease_name(directory: Path) -> str:
    return "workspace-" + hashlib.sha256(str(directory).encode()).hexdigest()


class Upstream(wire.Upstream):
    deadline = None

    def call(self, method: str, params: dict | None = None, envelope: dict | None = None) -> dict:
        if method not in {"status.get", "terminal.show", "orchestration.workerShow",
                          "orchestration.dispatchShow", *METHODS}:
            raise wire.Refused("task bridge upstream method denied")
        return self._exchange(method, params, envelope, absolute_deadline=self.deadline)


class TaskPolicy:
    def __init__(self, upstream: Upstream, binding: wire.Binding, directory: Path, verify_subject):
        self.upstream, self.binding, self.directory = upstream, binding, directory
        self.verify_subject = verify_subject
        self.token = secrets.token_hex(32)
        self.revoked = False
        self.phase = "bootstrap"
        self.authority = None
        self.capability_hash = None
        self.remaining = 256
        self.delivery = None
        self.questions = set()
        self.pending_question = None
        self.operations = {}
        self.settled_status = None

    def text(self, value: object, *, optional: bool = False, limit: int = 32000) -> str | None:
        if value is None and optional:
            return None
        if (not isinstance(value, str) or not value.strip() or len(value.encode()) > limit
                or self.upstream.token in value or "dcap_" in value or self.token in value):
            raise wire.Refused("invalid or confidential message text")
        return value

    def save(self):
        # Neither the real transport token nor the Dispatch capability is persisted here.
        write_ledger(self.directory / "journal.json", {
            "schema": 1, "phase": self.phase, "revoked": self.revoked,
            "authority": self.authority, "capability_sha256": self.capability_hash,
            "operations": self.operations, "settled_status": self.settled_status,
        })

    def terminal_current(self):
        self.check_deadline()
        self.verify_subject()
        self.check_deadline()
        row = self.upstream.call("terminal.show", {"terminal": self.binding.terminal})
        self.binding.validate(row.get("terminal"))

    def check_deadline(self):
        if self.revoked:
            raise wire.Refused("bridge revoked")
        if self.upstream.deadline is not None and time.monotonic() >= self.upstream.deadline:
            raise wire.Refused("absolute operation deadline exceeded")

    def status(self) -> dict:
        self.terminal_current()
        value = self.upstream.call("status.get")
        capabilities = value.get("capabilities")
        if (value.get("runtimeId") != self.upstream.runtime_id or value.get("appVersion") != wire.VERSION
                or value.get("graphStatus") != "ready" or value.get("remoteControl") is not None
                or not isinstance(capabilities, list) or CONTRACT not in capabilities):
            raise wire.Refused("task runtime is not compatible")
        return {"runtimeId": value["runtimeId"], "appVersion": value["appVersion"],
                "graphStatus": value["graphStatus"], "capabilities": [CONTRACT]}

    def await_arm(self):
        deadline = time.monotonic() + ARM_SECONDS
        while self.authority is None:
            if self.revoked or time.monotonic() >= deadline:
                raise wire.Refused("host did not arm this bridge")
            path = self.directory / "arm.json"
            if path.exists() or path.is_symlink():
                with acquire_host(lease_name(self.directory), inherit=False):
                    value = wire.mapping(read_private_json(path, {}))
                    if set(value) != {"run", "task", "dispatch", "coordinator"}:
                        raise wire.Refused("invalid host arm instruction")
                    self.authority = {name: key(item) for name, item in value.items()}
                    wire.identifier(value["coordinator"], prefix="term_")
                    if value["coordinator"] == self.binding.terminal:
                        raise wire.Refused("worker cannot be its own coordinator")
                    self.current()
                    self.phase = "active"
                    self.save()
                return
            time.sleep(0.025)

    def current(self, *, settled: str | None = None):
        self.terminal_current()
        authority = self.authority
        latest = wire.mapping(self.upstream.call("orchestration.dispatchShow", {
            "task": authority["task"]}).get("dispatch"))
        if latest.get("id") != authority["dispatch"] or latest.get("task_id") != authority["task"]:
            raise wire.Refused("latest Dispatch changed")
        # workerShow can reconcile stale worker epochs. It is a host-only call,
        # after checking the original runtime bootstrap, never a worker capability.
        observed = self.upstream.call("orchestration.workerShow", {"dispatch": authority["dispatch"]})
        dispatch = wire.mapping(observed.get("dispatch"))
        worker = wire.mapping(observed.get("worker"))
        observation = wire.mapping(observed.get("observation"))
        expected = {"id": authority["dispatch"], "taskId": authority["task"], "runId": authority["run"],
                    "assigneeHandle": self.binding.terminal, "processIncarnation": self.binding.incarnation}
        if (any(dispatch.get(name) != value for name, value in expected.items())
                or worker.get("dispatchId") != authority["dispatch"]
                or worker.get("runtimeEpoch") != self.upstream.runtime_id
                or worker.get("agentTerminalHandle") != self.binding.terminal
                or worker.get("worktreeId") != self.binding.worktree
                or (settled is None and worker.get("state") != "ready")
                or observation.get("exactWorker") is not True or observation.get("status") != "live"):
            raise wire.Refused("supervised worker identity is unproven")
        expected_status = settled or "dispatched"
        if dispatch.get("status") != expected_status or latest.get("status") != expected_status:
            raise wire.Refused("Dispatch is not in the expected state")
        if settled is not None and (worker.get("state") != ("succeeded" if settled == "completed" else "failed")
                                    or worker.get("stage") != "settled"
                                    or dispatch.get("capabilityRevokedAt") is None):
            raise wire.Refused("worker settlement is unproven")
        if settled is None and dispatch.get("capabilityRevokedAt") is not None:
            raise wire.Refused("Dispatch capability was revoked")

    def parameters(self, method: str, params: object):
        params = wire.mapping(params)
        authority = self.authority
        if method == "orchestration.send":
            if (params.keys() - {"from", "subject", "body", "type", "payload", "devMode", "waitForLifecycleSettlement"}
                    or params.get("from") != self.binding.terminal or params.get("devMode") is not False
                    or params.get("type") not in {"heartbeat", "worker_done", "escalation"}):
                raise wire.Refused("send scope denied")
            self.text(params.get("subject"), limit=500)
            self.text(params.get("body"), optional=True)
            payload = wire.decode(self.text(params.get("payload")).encode())
            done = params["type"] == "worker_done"
            if (payload.get("taskId") != authority["task"] or payload.get("dispatchId") != authority["dispatch"]
                    or payload.keys() - {"taskId", "dispatchId", *( {"outcome"} if done else {"phase"})}):
                raise wire.Refused("message lifecycle identity denied")
            if done:
                if (payload.get("outcome") not in {"succeeded", "failed"}
                        or params.get("waitForLifecycleSettlement") is not True or self.pending_question
                        or self.delivery):
                    raise wire.Refused("completion is incomplete or has pending communication")
            elif "waitForLifecycleSettlement" in params:
                raise wire.Refused("non-completion settlement requested")
            if "phase" in payload:
                self.text(payload["phase"], limit=200)
        elif method == "orchestration.check":
            if (params.keys() - {"terminal", "compatibilityCliCommand", "ack", "wait", "timeoutMs"}
                    or params.get("terminal") != self.binding.terminal
                    or params.get("compatibilityCliCommand") != "orca-ide"):
                raise wire.Refused("check scope denied")
            if "ack" in params and (self.delivery is None or params["ack"] != self.delivery):
                raise wire.Refused("unobserved delivery acknowledgement")
            if "wait" in params and params["wait"] is not True:
                raise wire.Refused("invalid wait flag")
            if params.get("wait") is True or "timeoutMs" in params:
                self.timeout(params.get("timeoutMs"))
        else:
            if (params.keys() - {"from", "question", "resume", "options", "timeoutMs", "compatibilityCliCommand"}
                    or params.get("from") != self.binding.terminal
                    or params.get("compatibilityCliCommand") != "orca-ide"
                    or ("question" in params) == ("resume" in params)):
                raise wire.Refused("ask scope denied")
            self.timeout(params.get("timeoutMs"))
            if "resume" in params:
                if params["resume"] != self.pending_question or params["resume"] not in self.questions or "options" in params:
                    raise wire.Refused("unknown question resume")
            else:
                if self.pending_question:
                    raise wire.Refused("resume the outstanding question")
                self.text(params["question"])
                self.text(params.get("options"), optional=True, limit=2000)

    @staticmethod
    def timeout(value):
        if type(value) is not int or not 1 <= value <= wire.MAX_WAIT_MS:
            raise wire.Refused("explicit bounded wait required")

    def project_message(self, value: object, *, inbound: bool) -> dict:
        message = wire.mapping(value)
        authority = self.authority
        expected = {"run_id": authority["run"],
                    "from_handle": authority["coordinator"] if inbound else self.binding.terminal,
                    "to_handle": "dispatch:" + authority["dispatch"] if inbound else "run:" + authority["run"]}
        if any(message.get(name) != item for name, item in expected.items()):
            raise wire.Refused("message is outside this Dispatch")
        result = {"id": key(message.get("id")), **expected}
        result["type"] = key(message.get("type"))
        result["subject"] = self.text(message.get("subject"), limit=500)
        for name in ("body", "payload"):
            if name in message:
                result[name] = self.text(message[name], optional=True)
        return result

    def project(self, method: str, params: dict, value: dict) -> dict:
        if method == "orchestration.send":
            message = self.project_message(value.get("message"), inbound=False)
            if any(message.get(name) != params.get(name) for name in ("type", "subject", "body", "payload")):
                raise wire.Refused("send receipt differs from this request")
            result = {"message": message}
            if params["type"] == "worker_done":
                lifecycle = wire.mapping(value.get("lifecycle"))
                payload = wire.decode(params["payload"].encode())
                state = "completed" if payload["outcome"] == "succeeded" else "failed"
                expected = {"action": state, "taskId": self.authority["task"], "dispatchId": self.authority["dispatch"]}
                if any(lifecycle.get(name) != item for name, item in expected.items()):
                    raise wire.Refused("unproven settlement verdict")
                self.current(settled=state)
                result["lifecycle"] = expected
                self.phase = "settled"
                self.settled_status = state
            else:
                self.current()
            return result
        self.current()
        if method == "orchestration.check":
            if value.get("runId") != self.authority["run"] or value.get("dispatchId") != self.authority["dispatch"]:
                raise wire.Refused("delivery generation changed")
            messages = value.get("messages")
            if (not isinstance(messages, list) or len(messages) > 50
                    or type(value.get("count")) is not int or value["count"] != len(messages)):
                raise wire.Refused("invalid delivery batch")
            delivery = value.get("deliveryId")
            if delivery is not None:
                key(delivery)
            if bool(messages) != (delivery is not None):
                raise wire.Refused("delivery identity missing")
            if self.delivery and "ack" not in params and delivery != self.delivery:
                raise wire.Refused("unacknowledged batch changed")
            if value.get("acknowledged") != params.get("ack"):
                raise wire.Refused("acknowledgement not confirmed")
            result = {"runId": self.authority["run"], "dispatchId": self.authority["dispatch"],
                      "deliveryId": delivery, "messages": [self.project_message(m, inbound=True) for m in messages],
                      "count": len(messages), "acknowledged": value.get("acknowledged")}
            self.delivery = delivery
        else:
            question = key(value.get("messageId"))
            if value.get("threadId") != question or ("resume" in params and params["resume"] != question):
                raise wire.Refused("question identity changed")
            answer = self.text(value.get("answer"), optional=True)
            result = {"messageId": question, "threadId": question, "answer": answer}
            if value.get("timeoutMs") != params["timeoutMs"]:
                raise wire.Refused("question deadline differs")
            result["timeoutMs"] = value["timeoutMs"]
            if answer is not None:
                result["answerMessageId"] = key(value.get("answerMessageId"))
            if "accepted" in value:
                if type(value["accepted"]) is not bool:
                    raise wire.Refused("invalid question receipt")
                result["accepted"] = value["accepted"]
            self.questions.add(question)
            self.pending_question = question if answer is None else None
        flags = ("timedOut", "cancelled", "connectionLost")
        # Orca 1.4.205 $jn omits all three on the normal wait-arrival path,
        # including when the follow-up delivery lookup returns no messages.
        arrival = method == "orchestration.check" and params.get("wait") is True and not any(f in value for f in flags)
        for flag in flags:
            observed = False if arrival else value.get(flag)
            if type(observed) is not bool:
                raise wire.Refused("invalid communication receipt")
            result[flag] = observed
        if method == "orchestration.ask" and result["answer"] is not None and any(result[f] for f in ("timedOut", "cancelled", "connectionLost")):
            raise wire.Refused("contradictory question answer")
        return result

    def handle(self, request: dict, *, deadline: float | None = None) -> dict:
        request_id = "refused"
        self.upstream.deadline = deadline if deadline is not None else time.monotonic() + REQUEST_SECONDS
        try:
            request_id = wire.identifier(wire.mapping(request).get("id"))
            if self.revoked or self.remaining <= 0:
                raise wire.Refused("bridge revoked or exhausted")
            self.remaining -= 1
            if not isinstance(request.get("authToken"), str) or not hmac.compare_digest(request["authToken"], self.token):
                raise wire.Refused("proxy authentication denied")
            method = request.get("method")
            if method == "status.get":
                if request.keys() - {"id", "authToken", "method", "params"} or request.get("params") not in (None, {}):
                    raise wire.Refused("status envelope denied")
                result = self.status()
            else:
                if method not in METHODS:
                    raise wire.Refused("task method denied")
                self.await_arm()
                cap_fields = {"orchestrationCapability"} if method in {"orchestration.send", "orchestration.ask"} else set()
                if (method not in METHODS or set(request) != {"id", "authToken", "method", "params", *FIELDS, *cap_fields}
                        or type(request["orchestrationContractVersion"]) is not int or request["orchestrationContractVersion"] != 1):
                    raise wire.Refused("task envelope denied")
                operation = wire.identifier(request["orchestrationRequestId"])
                if request["compatibilityInvocationId"] != operation:
                    raise wire.Refused("mutation identities differ")
                envelope = {name: request[name] for name in FIELDS | cap_fields}
                if cap_fields:
                    cap = request["orchestrationCapability"]
                    if not isinstance(cap, str) or not cap.startswith("dcap_") or not 16 <= len(cap) <= 4096:
                        raise wire.Refused("missing live capability")
                    cap_hash = hashlib.sha256(cap.encode()).hexdigest()
                    if self.capability_hash not in (None, cap_hash):
                        raise wire.Refused("Dispatch capability changed")
                    self.capability_hash = cap_hash
                signature = digest({"method": method, "params": request["params"], "envelope": envelope})
                previous = self.operations.get(operation)
                if previous is not None:
                    if previous["signature"] != signature or previous["phase"] != "confirmed":
                        raise wire.Refused("unknown or changed mutation replay")
                    self.current(settled=self.settled_status)
                    result = previous["result"]
                else:
                    if self.phase != "active":
                        raise wire.Refused("no new mutation after settlement")
                    self.parameters(method, request["params"])
                    self.current()
                    self.operations[operation] = {"signature": signature, "phase": "pending"}
                    self.save()
                    self.check_deadline()
                    value = self.upstream.call(method, request["params"], envelope)
                    mutation = wire.mapping(value.get("mutation"))
                    if mutation.get("requestId") != operation or mutation.get("replayed") is not False:
                        raise wire.Refused("unexpected upstream mutation replay")
                    result = self.project(method, request["params"], value)
                    result["mutation"] = {"requestId": operation, "replayed": False}
                    self.operations[operation].update(phase="confirmed", result=result)
                    self.save()
            self.check_deadline()
            return {"id": request_id, "ok": True, "result": result, "_meta": {"runtimeId": self.upstream.runtime_id}}
        except (OSError, ValueError, TypeError, KeyError, RuntimeError):
            self.revoked = True
            self.phase = "unknown"
            try:
                self.save()
            except (OSError, ValueError, RuntimeError):
                pass
            return {"id": request_id, "ok": False, "error": {
                "code": "task_bridge_refused", "message": "Preserve this attempt and reconcile with the coordinator; do not resend."},
                "_meta": {"runtimeId": self.upstream.runtime_id}}
        finally:
            self.upstream.deadline = None


class Handler(socketserver.BaseRequestHandler):
    """Preserve the stock CLI inactivity budget without extending host deadlines."""

    def handle(self):
        policy = self.server.policy
        stop = threading.Event()
        thread = None
        try:
            request = wire.receive(self.request, time.monotonic() + 2)
            deadline = time.monotonic() + REQUEST_SECONDS

            def keepalive():
                while not stop.wait(KEEPALIVE_SECONDS):
                    if time.monotonic() >= deadline:
                        policy.revoked = True
                        return
                    try:
                        # Installed stock CLI recognizes precisely this framing.
                        self.request.sendall(b'{"_keepalive":true}\n')
                    except OSError:
                        policy.revoked = True
                        return

            if isinstance(request.get("authToken"), str) and hmac.compare_digest(request["authToken"], policy.token):
                thread = threading.Thread(target=keepalive)
                thread.start()
            result = policy.handle(request, deadline=deadline)
            stop.set()
            if thread:
                thread.join()
            self.request.sendall(json.dumps(result).encode() + b"\n")
        except (OSError, ValueError, TypeError, RuntimeError):
            policy.revoked = True
        finally:
            stop.set()
            if thread:
                thread.join()
            if policy.revoked:
                policy.phase = "unknown"
                try:
                    policy.save()
                except (OSError, ValueError, RuntimeError):
                    pass


class Proxy(socketserver.UnixStreamServer):
    request_queue_size = 2

    def __init__(self, path: Path, policy: TaskPolicy):
        self.policy = policy
        super().__init__(str(path), Handler)
        os.chmod(path, 0o600)


class Session:
    """One launcher-owned generation; source/slot leases must enclose its lifetime."""
    def __init__(self, executable: Path, metadata_dir: Path, terminal: str, repo: Path, verify_subject):
        if not executable.is_absolute() or not executable.is_file() or executable.is_symlink():
            raise wire.Refused("explicit installed CLI path required")
        self.executable = executable
        self.upstream = Upstream.load(metadata_dir)
        self.binding = wire.Binding.discover(self.upstream, terminal, repo)
        self.identifier = str(uuid.uuid4())
        self.directory = checked_directory(root() / self.identifier)
        # The account-root path plus a UUID must fit Linux sockaddr_un (107 bytes).
        self.public = checked_directory(self.directory / "p")
        self.policy = TaskPolicy(self.upstream, self.binding, self.directory, verify_subject)
        self.server = None
        self.thread = None
        self.terminal_lease = None

    def __enter__(self):
        try:
            # Cooperative launchers cannot open two channels for one incarnation.
            # Raw Orca UI/CLI reassignment must still be excluded by the coordinator.
            identity = self.upstream.runtime_id + self.binding.terminal + self.binding.incarnation
            self.terminal_lease = acquire_host("workspace-" + hashlib.sha256(identity.encode()).hexdigest(), inherit=False)
            return self.start()
        except BaseException:
            self.__exit__()
            raise

    def start(self):
        self.policy.status()
        write_ledger(self.directory / "identity.json", {
            "runtime": self.upstream.runtime_id, "terminal": self.binding.terminal,
            "incarnation": self.binding.incarnation, "repo": str(self.binding.repo), "pid": os.getpid(),
        })
        self.policy.save()
        endpoint = self.public / "rpc.sock"
        if len(os.fsencode(endpoint)) > 107:
            raise wire.Refused("account socket path is too long")
        write_ledger(self.public / "orca-runtime.json", {
            "runtimeId": self.upstream.runtime_id, "authToken": self.policy.token,
            "transports": [{"kind": "unix", "endpoint": str(endpoint)}],
        })
        self.server = Proxy(endpoint, self.policy)
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.05})
        self.thread.start()
        print(json.dumps({"orca_bridge": self.identifier, "phase": "bootstrap", "dispatch_allowed": False}), flush=True)
        return self

    def mounts(self) -> list[str]:
        return ["--tmpfs", str(self.upstream.metadata_path.parent),
                "--ro-bind", "/dev/null", str(self.upstream.endpoint),
                "--ro-bind", str(self.executable), str(self.executable),
                "--ro-bind", str(self.public), str(self.public),
                "--setenv", "ORCA_USER_DATA_PATH", str(self.public)]

    def __exit__(self, *_):
        self.policy.revoked = True
        if self.thread and self.thread.is_alive():
            self.server.shutdown()
            self.thread.join()
        if self.server:
            self.server.server_close()
        try:
            with acquire_host(lease_name(self.directory), inherit=False):
                if self.policy.phase not in {"settled", "unknown"}:
                    armed = (self.directory / "arm.json").exists() or (self.directory / "arm.json").is_symlink()
                    self.policy.phase = "unknown" if self.policy.authority or armed else "closed"
                self.policy.save()
        finally:
            try:
                # Even a failed journal lease/write must remove usable transport.
                for name in ("orca-runtime.json", "rpc.sock"):
                    (self.public / name).unlink(missing_ok=True)
            finally:
                if self.terminal_lease:
                    self.terminal_lease.close()


def arm(bridge_id: str, authority: dict) -> None:
    directory = root() / wire.identifier(bridge_id)
    with acquire_host(lease_name(directory), inherit=False):
        state = wire.mapping(read_private_json(directory / "journal.json", {}))
        identity = wire.mapping(read_private_json(directory / "identity.json", {}))
        if state.get("phase") != "bootstrap" or state.get("revoked") is not False:
            raise wire.Refused("bridge is not awaiting a first Dispatch")
        os.kill(identity["pid"], 0)
        if set(authority) != {"run", "task", "dispatch", "coordinator"}:
            raise wire.Refused("incomplete authority")
        for value in authority.values():
            key(value)
        wire.identifier(authority["coordinator"], prefix="term_")
        path = directory / "arm.json"
        if path.exists() or path.is_symlink():
            raise wire.Refused("bridge was already armed; never overwrite")
        write_ledger(path, authority)
        # This queues a host instruction. The proxy must still validate live
        # supervised ownership before its first mutation; this is not a receipt.


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("arm",))
    parser.add_argument("--bridge", required=True)
    for name in ("run", "task", "dispatch", "coordinator"):
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    try:
        arm(args.bridge, {name: getattr(args, name) for name in ("run", "task", "dispatch", "coordinator")})
        print("Host binding queued; live identity validation and worker settlement are still required.")
        return 0
    except (OSError, ValueError, TypeError, KeyError, RuntimeError):
        print("Task bridge binding refused; preserve the attempt and reconcile.")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
