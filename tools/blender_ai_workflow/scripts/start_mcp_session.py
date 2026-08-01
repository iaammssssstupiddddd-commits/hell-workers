"""Start the hardened Blender bridge for an explicit interactive AI session."""

from __future__ import annotations

import importlib

import bpy


def main() -> None:
    module = importlib.import_module("blender_mcp_bridge")
    entry = bpy.context.preferences.addons.get("blender_mcp_bridge")
    if entry is None:
        raise RuntimeError("Blender MCP bridge is not enabled")
    preferences = entry.preferences
    if (
        not preferences.safe_mode
        or preferences.allow_inline_code
        or preferences.auto_start
    ):
        raise RuntimeError("refusing to start MCP with unsafe preferences")
    if module.TOOL_WHITELIST is None:
        raise RuntimeError("refusing to start MCP without a command whitelist")
    if set(module.TOOL_WHITELIST) != set(module.SAFE_TOOL_WHITELIST):
        raise RuntimeError("refusing to start MCP with a nonstandard whitelist")

    module._ensure_server_running()
    if not module._server_healthy():
        raise RuntimeError("Blender MCP bridge did not start")
    print(
        "MCP_AI_SESSION_READY "
        f"host={module.HOST} port={module.PORT} blender={bpy.app.version_string}"
    )


if __name__ == "__main__":
    main()
