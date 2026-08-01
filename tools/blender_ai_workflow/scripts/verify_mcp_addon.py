"""Verify the installed Blender MCP bridge without mutating a scene."""

from __future__ import annotations

import importlib
import os

import bpy


def main() -> None:
    module_name = "blender_mcp_bridge"
    entry = bpy.context.preferences.addons.get(module_name)
    if entry is None:
        raise RuntimeError(f"add-on is not enabled: {module_name}")
    preferences = entry.preferences
    expected = {
        "safe_mode": True,
        "allow_inline_code": False,
        "auto_start": False,
        "port": 9876,
    }
    actual = {name: getattr(preferences, name) for name in expected}
    if actual != expected:
        raise RuntimeError(f"unsafe MCP preferences: expected={expected} actual={actual}")
    if getattr(bpy.context.preferences.filepaths, "use_scripts_auto_execute", False):
        raise RuntimeError("Blender Python auto-execution is enabled")
    if not preferences.approved_script_roots:
        raise RuntimeError("approved_script_roots is empty")
    expected_root = os.path.realpath(
        os.path.join(os.environ["HELL_WORKERS_ASSET_ROOT"], "staging")
    )
    actual_root = os.path.realpath(preferences.approved_script_roots)
    if actual_root != expected_root:
        raise RuntimeError(
            f"unsafe MCP path root: expected={expected_root} actual={actual_root}"
        )
    module = importlib.import_module(module_name)
    if module.HOST != "127.0.0.1":
        raise RuntimeError(f"MCP bridge is not loopback-only: {module.HOST}")
    blocked_commands = {
        "python.execute",
        "python.execute_async",
        "job.status",
        "job.cancel",
        "job.list",
        "export.obj",
        "export.fbx",
        "export.gltf",
    }
    if module.TOOL_WHITELIST is None:
        raise RuntimeError("MCP command whitelist is disabled")
    if set(module.TOOL_WHITELIST) != set(module.SAFE_TOOL_WHITELIST):
        raise RuntimeError(
            "MCP command whitelist differs from the hardened safe command set"
        )
    exposed = blocked_commands.intersection(module.TOOL_WHITELIST)
    if exposed:
        raise RuntimeError(f"unsafe MCP commands are enabled: {sorted(exposed)}")
    print(
        "MCP_ADDON_VERIFIED "
        f"blender={bpy.app.version_string} module={module_name} "
        f"safe_mode={actual['safe_mode']} inline={actual['allow_inline_code']} "
        f"auto_start={actual['auto_start']} port={actual['port']} "
        f"roots={preferences.approved_script_roots} "
        f"blocked={','.join(sorted(blocked_commands))}"
    )


if __name__ == "__main__":
    main()
