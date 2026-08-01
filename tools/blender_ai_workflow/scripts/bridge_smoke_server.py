"""Keep Blender's hardened MCP bridge alive briefly for an external query."""

from __future__ import annotations

import importlib
import time

import bpy


def main() -> None:
    module = importlib.import_module("blender_mcp_bridge")
    entry = bpy.context.preferences.addons.get("blender_mcp_bridge")
    if entry is None:
        raise RuntimeError("Blender MCP bridge is not enabled")
    preferences = entry.preferences
    if not preferences.safe_mode or preferences.allow_inline_code:
        raise RuntimeError("refusing to start smoke server with unsafe preferences")
    if module.TOOL_WHITELIST is None:
        raise RuntimeError("refusing to start smoke server without a command whitelist")
    if set(module.TOOL_WHITELIST) != set(module.SAFE_TOOL_WHITELIST):
        raise RuntimeError("refusing to start smoke server with a nonstandard whitelist")

    module._ensure_server_running()
    server = module._server
    if server is None or not server._running:
        raise RuntimeError("Blender MCP bridge did not start")

    print(f"MCP_SMOKE_READY host={server._host} port={server._port}", flush=True)
    deadline = time.monotonic() + 90.0
    while time.monotonic() < deadline:
        server._drain_request_queue()
        time.sleep(0.01)
    print("MCP_SMOKE_TIMEOUT", flush=True)


if __name__ == "__main__":
    main()
