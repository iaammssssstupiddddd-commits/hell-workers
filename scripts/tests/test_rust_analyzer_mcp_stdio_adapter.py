from __future__ import annotations

import json
import os
import select
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from io import BytesIO
from pathlib import Path

from scripts import rust_analyzer_mcp_stdio_adapter as adapter


FAKE_MCP_SERVER = r'''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

counter = Path(os.environ["FAKE_MCP_COUNTER"])
with counter.open("a", encoding="utf-8") as file:
    file.write("started\n")
    file.flush()

for line in sys.stdin.buffer:
    request = json.loads(line)
    response = {
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {
            "pid": os.getpid(),
            "method": request.get("method"),
            "params": request.get("params"),
        },
    }
    sys.stdout.buffer.write(json.dumps(response, separators=(",", ":")).encode("utf-8"))
    sys.stdout.buffer.write(b"\n")
    sys.stdout.buffer.flush()
'''


class RustAnalyzerMcpStdioAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.workspace = self.root / "workspace"
        self.workspace.mkdir()
        (self.workspace / "Cargo.toml").write_text(
            "[workspace]\nmembers = []\n",
            encoding="utf-8",
        )
        self.counter = self.root / "backend-starts.log"
        self.fake_server = self.root / "fake_mcp_server.py"
        self.fake_server.write_text(FAKE_MCP_SERVER, encoding="utf-8")
        self.environment = os.environ.copy()
        self.environment.update(
            {
                "HELL_WORKERS_RA_MCP_RUNTIME_DIR": str(self.root / "runtime"),
                "HELL_WORKERS_RA_MCP_BACKEND_IDLE_SECONDS": "0.25",
                "HELL_WORKERS_RA_MCP_DAEMON_IDLE_SECONDS": "0.25",
                "FAKE_MCP_COUNTER": str(self.counter),
            }
        )
        self.processes: list[subprocess.Popen[bytes]] = []

    def tearDown(self) -> None:
        for process in self.processes:
            if process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
            try:
                process.wait(timeout=3.0)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=3.0)
            for stream in (process.stdin, process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()

        endpoint = adapter.socket_path(self.workspace, self.environment)
        self._wait_until(lambda: not endpoint.exists(), timeout=3.0)
        self.temporary_directory.cleanup()

    def _start_client(self) -> subprocess.Popen[bytes]:
        process = subprocess.Popen(
            [
                sys.executable,
                str(Path(adapter.__file__).resolve()),
                sys.executable,
                str(self.fake_server),
            ],
            cwd=self.workspace,
            env=self.environment,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.processes.append(process)
        return process

    def _request(self, process: subprocess.Popen[bytes], request: dict[str, object]) -> dict[str, object]:
        assert process.stdin is not None
        assert process.stdout is not None
        process.stdin.write(json.dumps(request).encode("utf-8") + b"\n")
        process.stdin.flush()
        ready, _, _ = select.select([process.stdout], [], [], 5.0)
        self.assertTrue(ready, "shared MCP proxy did not return a response within five seconds")
        line = process.stdout.readline()
        self.assertTrue(line, "shared MCP proxy closed its response stream")
        return json.loads(line)

    def _start_count(self) -> int:
        if not self.counter.exists():
            return 0
        return len(self.counter.read_text(encoding="utf-8").splitlines())

    def _wait_until(self, condition: object, *, timeout: float) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if condition():  # type: ignore[operator]
                return
            time.sleep(0.03)
        self.fail("condition was not satisfied before timeout")

    def test_normalizes_content_length_framing_and_drops_startup_notifications(self) -> None:
        request = b'{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
        framed = BytesIO(
            b"Content-Length: "
            + str(len(request)).encode("ascii")
            + b"\r\n\r\n"
            + request
        )

        self.assertEqual(adapter._read_mcp_payload(framed), request)
        self.assertEqual(
            adapter._normalize_client_payload(
                b'{"jsonrpc":"2.0","method":"notifications/initialized"}'
            ),
            None,
        )
        tool_list = json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "result": {
                    "tools": [
                        {"name": "rust_analyzer_hover"},
                        {"name": "rust_analyzer_set_workspace"},
                    ]
                },
            }
        ).encode("utf-8")
        visible_tools = json.loads(adapter._hide_shared_only_tools(tool_list))
        self.assertEqual(
            [tool["name"] for tool in visible_tools["result"]["tools"]],
            ["rust_analyzer_hover"],
        )

    def test_workspace_identity_keeps_incompatible_cargo_environments_separate(self) -> None:
        default_identity = adapter.workspace_identity(self.workspace, {})
        alternate_identity = adapter.workspace_identity(
            self.workspace,
            {"CARGO_TARGET_DIR": str(self.workspace / "alternate-target")},
        )

        self.assertNotEqual(default_identity, alternate_identity)

    def test_concurrent_clients_share_one_backend_then_restart_after_idle(self) -> None:
        first = self._start_client()
        second = self._start_client()
        responses: dict[str, dict[str, object]] = {}
        barrier = threading.Barrier(3)

        def send(process: subprocess.Popen[bytes], label: str) -> None:
            barrier.wait()
            responses[label] = self._request(
                process,
                {
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/list",
                    "params": {"client": label},
                },
            )

        first_thread = threading.Thread(target=send, args=(first, "first"))
        second_thread = threading.Thread(target=send, args=(second, "second"))
        first_thread.start()
        second_thread.start()
        barrier.wait()
        first_thread.join(timeout=6.0)
        second_thread.join(timeout=6.0)

        self.assertFalse(first_thread.is_alive())
        self.assertFalse(second_thread.is_alive())
        self.assertEqual(responses["first"]["result"]["params"]["client"], "first")
        self.assertEqual(responses["second"]["result"]["params"]["client"], "second")
        self.assertEqual(responses["first"]["result"]["pid"], responses["second"]["result"]["pid"])
        self.assertEqual(self._start_count(), 1)

        rejected = self._request(
            second,
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {"name": "rust_analyzer_set_workspace", "arguments": {}},
            },
        )
        self.assertEqual(rejected["error"]["code"], -32001)
        self.assertEqual(self._start_count(), 1)

        self._wait_until(lambda: self._start_count() == 1, timeout=0.5)
        time.sleep(0.35)
        restarted = self._request(
            first,
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/list",
                "params": {"client": "restart"},
            },
        )
        self.assertEqual(restarted["result"]["params"]["client"], "restart")
        self._wait_until(lambda: self._start_count() == 2, timeout=3.0)

        assert first.stdin is not None
        assert second.stdin is not None
        first.stdin.close()
        second.stdin.close()
        self._wait_until(
            lambda: not adapter.socket_path(self.workspace, self.environment).exists(),
            timeout=3.0,
        )


if __name__ == "__main__":
    unittest.main()
