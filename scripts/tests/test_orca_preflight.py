"""Read-only wire boundaries, real stock CLI and Linux mount acceptance."""

from __future__ import annotations

import copy
import json
import os
import shutil
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

from scripts import orca_preflight as probe


RUNTIME = "6b9dcc48-963a-402c-b979-e8b5e348b7a5"
HANDLE = "term_8a45da28-b2a2-48cf-8c8c-126f4cabdbd4"
INCARNATION = "04af48ef-120f-4672-b79f-dd3ee94f97ca"
SECRET = "fixture-upstream-secret-never-returned"
CLI = Path.home() / ".config/orca/linux-orca-cli-shim/orca"


class RuntimeHandler(socketserver.BaseRequestHandler):
    def handle(self):
        request = probe.receive(self.request, time.monotonic() + 2)
        self.server.requests.append(request)
        if request.get("authToken") != SECRET:
            raise AssertionError("proxy did not authenticate to fixture")
        method = request["method"]
        if method == "status.get":
            result = {"runtimeId": RUNTIME, "appVersion": probe.VERSION, "graphStatus": "ready",
                      "authToken": SECRET, "otherTerminal": "secret unrelated terminal"}
        elif method == "terminal.show":
            result = {"terminal": self.server.row}
        elif method == "terminal.wait":
            result = {"wait": {"handle": HANDLE, "condition": "tui-idle", "satisfied": False,
                               "blockedReason": "agent-interactive-prompt", "status": "running",
                               "preview": SECRET, "unknown": {"token": SECRET}}}
        else:
            raise AssertionError("unexpected upstream method")
        response = {"id": request["id"], "ok": True, "result": result, "_meta": {"runtimeId": RUNTIME}}
        if self.server.response_change:
            self.server.response_change(response)
        self.request.sendall(json.dumps(response).encode() + b"\n")


class PreflightTests(unittest.TestCase):
    def setUp(self):
        # Socket paths must fit Linux's sockaddr_un; fixtures need no Git checkout.
        self.temp = tempfile.TemporaryDirectory(prefix="hwp-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        target = Path(__file__).resolve().parents[2] / "target"
        target.mkdir(exist_ok=True)
        repo_temp = tempfile.TemporaryDirectory(prefix="preflight-", dir=target)
        self.addCleanup(repo_temp.cleanup)
        self.repo = Path(repo_temp.name)
        self.config = self.root / "upstream"
        self.config.mkdir(mode=0o700)
        self.server = socketserver.UnixStreamServer(str(self.root / "upstream.sock"), RuntimeHandler)
        self.server.requests = []
        self.server.response_change = None
        self.server.row = {"handle": HANDLE, "incarnationId": INCARNATION,
                           "worktreeId": "fixture::" + str(self.repo), "worktreePath": str(self.repo),
                           "executionHostId": "local", "connected": True, "orphaned": False,
                           "preview": SECRET, "title": SECRET, "agentWait": {"token": SECRET}}
        self.metadata = {"runtimeId": RUNTIME, "authToken": SECRET,
                         "transports": [{"kind": "unix", "endpoint": str(self.root / "upstream.sock")}]}
        self.metadata_path = self.config / "orca-runtime.json"
        probe.write_ledger(self.metadata_path, self.metadata)
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={"poll_interval": 0.01})
        self.thread.start()
        self.addCleanup(self.stop)
        self.upstream = probe.Upstream.load(self.config)
        self.binding = probe.Binding.discover(self.upstream, HANDLE, self.repo)
        self.server.requests.clear()

    def stop(self):
        self.server.shutdown()
        self.thread.join(timeout=3)
        self.server.server_close()

    def policy(self):
        return probe.Policy(self.upstream, self.binding)

    @staticmethod
    def request(policy, method="status.get", params=None):
        result = {"id": str(uuid.uuid4()), "authToken": policy.token, "method": method}
        if params is not None:
            result["params"] = params
        return result

    def test_only_projected_status_and_terminal_fields_escape(self):
        policy = self.policy()
        for method, params in (("status.get", None), ("terminal.show", {"terminal": HANDLE}),
                               ("terminal.wait", {"terminal": HANDLE, "for": "tui-idle", "timeoutMs": 100})):
            with self.subTest(method=method):
                reply = policy.handle(self.request(policy, method, params))
                self.assertTrue(reply["ok"])
                self.assertNotIn(SECRET, json.dumps(reply))
                self.assertNotIn("preview", json.dumps(reply))
                self.assertNotIn("otherTerminal", json.dumps(reply))
        self.assertFalse(reply["result"]["wait"]["satisfied"])
        self.assertEqual(reply["result"]["wait"]["blockedReason"], "agent-interactive-prompt")

    def test_unknown_rpc_identity_flags_and_mutations_never_reach_host(self):
        mutations = [
            {"method": "terminal.send"}, {"method": "terminal.resolveActive"},
            {"method": "orchestration.send"}, {"method": "orchestration.check"},
            {"method": "status.get", "params": {"extra": True}},
            {"method": "terminal.show", "params": {"terminal": HANDLE, "extra": True}},
            {"method": "terminal.show", "params": {"terminal": "other"}},
            {"method": "terminal.wait", "params": {"terminal": HANDLE, "for": "exit", "timeoutMs": 1}},
            {"method": "terminal.wait", "params": {"terminal": HANDLE, "for": "tui-idle", "timeoutMs": True}},
            {"method": "terminal.wait", "params": {"terminal": HANDLE, "for": "tui-idle", "timeoutMs": 15001}},
            {"authToken": "wrong"}, {"orchestrationCapability": "never-forward"},
            {"compatibilityInvocationId": "unexpected"}, {"id": "not-a-uuid"},
        ]
        for change in mutations:
            with self.subTest(change=change):
                policy = self.policy()
                reply = policy.handle({**self.request(policy), **change})
                self.assertFalse(reply["ok"])
                self.assertTrue(policy.revoked)
                self.assertFalse(policy.handle(self.request(policy))["ok"])
                self.assertEqual(self.server.requests, [])

    def test_changed_terminal_or_runtime_revokes_without_sanitized_success(self):
        for key, value in (("incarnationId", str(uuid.uuid4())), ("worktreePath", "/other"),
                           ("connected", False), ("orphaned", True), ("executionHostId", "remote")):
            original = copy.deepcopy(self.server.row)
            with self.subTest(key=key):
                self.server.row[key] = value
                policy = self.policy()
                self.assertFalse(policy.handle(self.request(policy))["ok"])
                self.assertTrue(policy.revoked)
            self.server.row = original
        self.metadata["runtimeId"] = str(uuid.uuid4())
        probe.write_ledger(self.metadata_path, self.metadata)
        policy = self.policy()
        self.assertFalse(policy.handle(self.request(policy))["ok"])

    def test_wait_rechecks_binding_after_wait_and_checks_response_handle(self):
        def changed(response):
            if "wait" in response["result"]:
                self.server.row["incarnationId"] = str(uuid.uuid4())
        self.server.response_change = changed
        policy = self.policy()
        request = self.request(policy, "terminal.wait", {"terminal": HANDLE, "for": "tui-idle", "timeoutMs": 1})
        self.assertFalse(policy.handle(request)["ok"])
        self.server.row["incarnationId"] = INCARNATION
        def wrong_handle(response):
            if "wait" in response["result"]:
                response["result"]["wait"]["handle"] = "other"
        self.server.response_change = wrong_handle
        policy = self.policy()
        self.assertFalse(policy.handle({**request, "authToken": policy.token})["ok"])

    def test_response_identity_and_raw_error_never_escape(self):
        for change in ({"id": "wrong"}, {"_meta": {"runtimeId": "other"}},
                       {"ok": False, "error": {"code": SECRET, "message": SECRET, "data": SECRET}}):
            with self.subTest(change=change):
                self.server.response_change = lambda response: response.update(change)
                policy = self.policy()
                response = policy.handle(self.request(policy))
                self.assertFalse(response["ok"])
                self.assertNotIn(SECRET, json.dumps(response))

    def test_unconfirmed_read_is_distinct_from_identity_corruption(self):
        self.server.response_change = lambda response: response.update(ok=False, error={"message": SECRET})
        with self.assertRaises(probe.ObservationUnavailable) as failure:
            self.upstream.call("terminal.show", {"terminal": HANDLE})
        self.assertNotIn(SECRET, str(failure.exception))
        self.server.response_change = lambda response: response.update(id="wrong", ok=False)
        with self.assertRaises(probe.Refused) as failure:
            self.upstream.call("terminal.show", {"terminal": HANDLE})
        self.assertNotIsInstance(failure.exception, probe.ObservationUnavailable)

    def test_malformed_nested_upstream_objects_revoke_without_leaking(self):
        for key in ("_meta", "result", "terminal", "wait"):
            for value in (None, [SECRET], SECRET):
                with self.subTest(key=key, value=value):
                    def change(response):
                        if key in ("_meta", "result"):
                            response[key] = value
                        elif key in response["result"]:
                            response["result"][key] = value
                    self.server.response_change = change
                    policy = self.policy()
                    response = policy.handle(self.request(policy, "terminal.wait", {
                        "terminal": HANDLE, "for": "tui-idle", "timeoutMs": 1}))
                    self.assertFalse(response["ok"])
                    self.assertTrue(policy.revoked)
                    self.assertNotIn(SECRET, json.dumps(response))
                    if key == "terminal":
                        with self.assertRaises(probe.Refused):
                            probe.Binding.discover(self.upstream, HANDLE, self.repo)

    def test_unknown_wait_enum_cannot_leak_arbitrary_text(self):
        for key in ("blockedReason", "status"):
            for value in (SECRET, [SECRET], {"secret": SECRET}, "", None):
                with self.subTest(key=key, value=value):
                    def change(response):
                        if "wait" in response["result"]:
                            response["result"]["wait"][key] = value
                    self.server.response_change = change
                    policy = self.policy()
                    response = policy.handle(self.request(policy, "terminal.wait", {
                        "terminal": HANDLE, "for": "tui-idle", "timeoutMs": 1}))
                    self.assertFalse(response["ok"])
                    self.assertTrue(policy.revoked)
                    self.assertNotIn(SECRET, json.dumps(response))

    def test_wait_projection_matches_installed_wire_enums(self):
        wait = {"handle": HANDLE, "condition": "tui-idle", "satisfied": False, "status": "running"}
        for status in probe.WAIT_STATUS:
            for reason in probe.BLOCKED_REASON:
                observed = {**wait, "status": status, "blockedReason": reason}
                self.assertEqual(probe.project_wait(observed, self.binding), observed)
        ready = {**wait, "satisfied": True}
        self.assertEqual(probe.project_wait(ready, self.binding), ready)
        with self.assertRaises(probe.Refused):
            probe.project_wait({**ready, "blockedReason": "agent-interactive-prompt"}, self.binding)
        del wait["status"]
        with self.assertRaises(probe.Refused):
            probe.project_wait(wait, self.binding)

    def test_cli_nested_objects_and_exit_codes_are_checked(self):
        status = {"runtime": {"runtimeId": RUNTIME, "reachable": True}, "graph": {"state": "ready"}}
        wait = {"wait": {"handle": HANDLE, "condition": "tui-idle", "satisfied": True, "status": "running"}}
        fixtures = (("status", status), ("show", {"terminal": self.server.row}), ("wait", wait))
        for kind, result in fixtures:
            for key in ("result", *result):
                for value in (None, [SECRET], SECRET):
                    with self.subTest(kind=kind, key=key, value=value):
                        output = {"ok": True, "result": copy.deepcopy(result)}
                        if key == "result":
                            output[key] = value
                        else:
                            output["result"][key] = value
                        with self.assertRaises(probe.Refused):
                            probe.cli_result(output, kind, 0, self.upstream, self.binding)
        for result, code in ((status, 1), ({"runtime": {"reachable": False}, "graph": {}}, 0)):
            with self.assertRaises(probe.Refused):
                probe.cli_result({"ok": True, "result": result}, "status", code, self.upstream, self.binding)
        with self.assertRaises(probe.Refused):
            probe.cli_result({"ok": True, "result": wait}, "wait", 1, self.upstream, self.binding)

    def test_proxy_cleanup_after_cli_failure(self):
        root = self.root / "state/coordination"
        with patch.object(probe, "state_root", return_value=root), \
                patch.object(probe, "sandbox", return_value=["not-run"]), \
                patch.object(probe, "run_cli", side_effect=probe.Refused("fixture failure")):
            with self.assertRaises(probe.Refused):
                probe.probe(CLI, self.config, HANDLE, self.repo)
        self.assertEqual(list((root.parent / "preflight").iterdir()), [])

    def test_shutdown_finishes_inflight_handler_before_socket_cleanup(self):
        policy = self.policy()
        entered, release = threading.Event(), threading.Event()
        socket_path = self.root / "delayed.sock"
        def delayed(request):
            entered.set()
            release.wait(timeout=1)
            return {"ok": False}
        with probe.Proxy(socket_path, policy) as server, patch.object(policy, "handle", side_effect=delayed):
            thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
            thread.start()
            with socket.socket(socket.AF_UNIX) as client:
                try:
                    client.connect(str(socket_path))
                    client.sendall(json.dumps(self.request(policy)).encode() + b"\n")
                    self.assertTrue(entered.wait(timeout=1))
                    shutdown = threading.Thread(target=server.shutdown)
                    shutdown.start()
                    self.assertTrue(shutdown.is_alive())
                    self.assertTrue(socket_path.exists())
                finally:
                    release.set()
                    server.shutdown()
                    thread.join(timeout=2)
            shutdown.join(timeout=2)
            self.assertFalse(thread.is_alive())
            self.assertFalse(shutdown.is_alive())

    def test_remote_control_version_and_graph_are_not_silently_ignored(self):
        for key, value in (("remoteControl", {"state": "blocked"}), ("appVersion", "future"),
                           ("graphStatus", "starting"), ("runtimeId", "different")):
            with self.subTest(key=key):
                def change(response):
                    if "runtimeId" in response["result"]:
                        response["result"][key] = value
                self.server.response_change = change
                policy = self.policy()
                self.assertFalse(policy.handle(self.request(policy))["ok"])

    def test_metadata_permissions_symlink_and_auth_rotation_fail_closed(self):
        self.metadata_path.chmod(0o644)
        with self.assertRaises(probe.Refused):
            probe.Upstream.load(self.config)
        self.metadata_path.chmod(0o600)
        alias = self.root / "alias"
        alias.symlink_to(self.config, target_is_directory=True)
        with self.assertRaises(probe.Refused):
            probe.Upstream.load(alias)
        self.metadata["authToken"] = "rotated-fixture-token"
        probe.write_ledger(self.metadata_path, self.metadata)
        with self.assertRaises(probe.Refused):
            self.upstream.call("status.get")
        self.assertEqual(self.server.requests, [])

    def test_proxy_budget_and_revocation_are_one_way(self):
        policy = self.policy()
        policy.remaining = 1
        self.assertTrue(policy.handle(self.request(policy))["ok"])
        self.assertFalse(policy.handle(self.request(policy))["ok"])
        before = len(self.server.requests)
        self.assertFalse(policy.handle(self.request(policy))["ok"])
        self.assertEqual(len(self.server.requests), before)

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap") and CLI.is_file(),
                         "installed Orca CLI and Linux bubblewrap required")
    def test_stock_cli_through_readonly_proxy_and_unsatisfied_wait(self):
        # No real Orca state/auth/LLM: exact stock CLI talks only to this fake runtime.
        root = self.root / "state/coordination"
        with patch.object(probe, "state_root", return_value=root):
            result = probe.probe(CLI, self.config, HANDLE, self.repo, wait_ms=100)
        self.assertEqual(result["transport"], "verified")
        self.assertFalse(result["dispatch_allowed"])
        self.assertFalse(result["readiness"]["satisfied"])
        self.assertEqual(list((root.parent / "preflight").iterdir()), [])

    @unittest.skipUnless(sys.platform == "linux" and shutil.which("bwrap"), "Linux bubblewrap required")
    def test_mount_denies_bootstrap_direct_socket_and_metadata_writes(self):
        policy_dir = self.root / "proxy"
        policy_dir.mkdir(mode=0o700)
        policy_file = policy_dir / "orca-runtime.json"
        probe.write_ledger(policy_file, {"authToken": "limited"})
        script = """import json, pathlib, socket, sys
original, direct, limited = map(pathlib.Path, sys.argv[1:])
assert not original.exists()
try:
    socket.socket(socket.AF_UNIX).connect(str(direct))
except OSError:
    pass
else:
    raise AssertionError('direct transport visible')
assert json.loads(limited.read_text())['authToken'] == 'limited'
try:
    limited.write_text('{}')
except OSError:
    pass
else:
    raise AssertionError('writable proxy metadata')
print('mount boundaries verified')
"""
        command = probe.sandbox(Path(sys.executable).resolve(), policy_dir, self.upstream, self.repo,
                                ["-c", script, str(self.metadata_path), str(self.upstream.endpoint), str(policy_file)])
        result = subprocess.run(command, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("mount boundaries verified", result.stdout)


class FrameTests(unittest.TestCase):
    def test_duplicate_nonfinite_and_nonobject_frames_rejected(self):
        for raw in (b'{"id":1,"id":2}', b'{"value":NaN}', b'[]', b'x' * (probe.MAX_FRAME + 1)):
            with self.subTest(raw=raw[:40]), self.assertRaises((ValueError, probe.Refused)):
                probe.decode(raw)

    def test_truncated_multiple_and_oversized_frames_rejected(self):
        for raw in (b'{', b'{}\n{}\n', b'x' * 100):
            left, right = socket.socketpair()
            try:
                right.sendall(raw)
                right.shutdown(socket.SHUT_WR)
                with self.subTest(raw=raw[:40]), patch.object(probe, "MAX_FRAME", 64), \
                        self.assertRaises(probe.Refused):
                    probe.receive(left, time.monotonic() + 1)
            finally:
                left.close()
                right.close()

    def test_keepalive_does_not_extend_absolute_deadline(self):
        left, right = socket.socketpair()
        try:
            right.sendall(b'{"_keepalive":true}\n')
            with self.assertRaises((OSError, probe.Refused)):
                probe.receive(left, time.monotonic() + 0.03)
        finally:
            left.close()
            right.close()


class CliCaptureTests(unittest.TestCase):
    def test_bounded_capture_preserves_small_outputs_and_exit(self):
        result = probe.run_cli([sys.executable, "-c", "import sys;print('ok');print('notice',file=sys.stderr);sys.exit(1)"], 2)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (1, b"ok\n", b"notice\n"))

    def test_both_output_limits_kill_and_reap_the_cli(self):
        for fd in (1, 2):
            with self.subTest(fd=fd), tempfile.TemporaryDirectory() as directory:
                pidfile = Path(directory) / "pid"
                script = ("import os,pathlib,sys;pathlib.Path(sys.argv[1]).write_text(str(os.getpid()));"
                          f"os.write({fd}, b'x' * 8192);os.read(0,1);import time;time.sleep(10)")
                with patch.object(probe, "MAX_FRAME", 64), self.assertRaises(probe.Refused):
                    probe.run_cli([sys.executable, "-c", script, str(pidfile)], 2)
                pid = int(pidfile.read_text())
                with self.assertRaises(ProcessLookupError):
                    os.kill(pid, 0)
                with self.assertRaises(ChildProcessError):
                    os.waitpid(pid, os.WNOHANG)

    def test_deadline_kills_a_silent_cli(self):
        start = time.monotonic()
        with self.assertRaises(probe.Refused):
            probe.run_cli([sys.executable, "-c", "import time;time.sleep(10)"], 0.1)
        self.assertLess(time.monotonic() - start, 2)

    @unittest.skipUnless(sys.platform == "linux", "Linux process group evidence required")
    def test_deadline_stops_descendant_holding_pipes_after_parent_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / "child"
            script = """import os, pathlib, sys, time
child = os.fork()
if child:
    pathlib.Path(sys.argv[1]).write_text(str(child))
    os._exit(0)
time.sleep(10)
"""
            with self.assertRaises(probe.Refused):
                probe.run_cli([sys.executable, "-c", script, str(pidfile)], 0.5)
            child = int(pidfile.read_text())
            # Grandchildren are reaped by init; only require no living descendant.
            for _ in range(100):
                try:
                    state = Path(f"/proc/{child}/stat").read_text().rsplit(")", 1)[1].split()[0]
                except FileNotFoundError:
                    break
                if state == "Z":
                    break
                time.sleep(0.01)
            else:
                self.fail("CLI descendant survived group cleanup")


if __name__ == "__main__":
    unittest.main()
