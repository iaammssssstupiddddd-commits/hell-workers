"""Exercise the hardened Blender bridge through the MCP stdio server."""

from __future__ import annotations

import asyncio
import json
import os
from pathlib import Path
from typing import Any

from mcp import ClientSession, McpError, StdioServerParameters
from mcp.client.stdio import stdio_client

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SERVER = WORKFLOW_ROOT / "bin" / "blender-mcp-server"


def content_text(result: Any) -> str:
    return "\n".join(
        str(getattr(item, "text", ""))
        for item in getattr(result, "content", [])
        if getattr(item, "text", None) is not None
    )


async def expect_blocked(
    session: ClientSession,
    tool: str,
    arguments: dict[str, Any],
    marker: str,
) -> None:
    try:
        result = await session.call_tool(tool, arguments)
    except McpError as exc:
        message = str(exc)
    else:
        message = content_text(result)
        if not getattr(result, "isError", False):
            raise RuntimeError(f"unsafe tool unexpectedly succeeded: {tool}")
    if marker.lower() not in message.lower():
        raise RuntimeError(
            f"{tool} was blocked without the expected marker {marker!r}: {message}"
        )


async def run() -> None:
    asset_root = Path(
        os.environ.get(
            "HELL_WORKERS_ASSET_ROOT",
            str(Path.home() / "Sync" / "hell-workers-assets"),
        )
    ).resolve()
    save_path = asset_root / "staging" / "blend" / "mcp_bridge_smoke.blend"
    parameters = StdioServerParameters(
        command=str(SERVER),
        env={
            **os.environ,
            "HELL_WORKERS_ASSET_ROOT": str(asset_root),
            "BLENDER_MCP_ALLOW_HEADLESS": "0",
            "BLENDER_MCP_ALLOW_PYTHON_EXEC": "0",
        },
    )

    async with (
        stdio_client(parameters) as (reader, writer),
        ClientSession(reader, writer) as session,
    ):
        await session.initialize()
        tools = await session.list_tools()
        names = {tool.name for tool in tools.tools}
        required = {
            "blender_scene_get_info",
            "blender_scene_save_as",
            "blender_object_create",
        }
        missing = sorted(required - names)
        if missing:
            raise RuntimeError(f"MCP tools are missing: {missing}")

        scene_result = await session.call_tool("blender_scene_get_info", {})
        if getattr(scene_result, "isError", False):
            raise RuntimeError(content_text(scene_result))
        scene = json.loads(content_text(scene_result))
        if scene.get("render_engine") != "BLENDER_EEVEE":
            raise RuntimeError(f"unexpected scene response: {scene}")

        save_result = await session.call_tool(
            "blender_scene_save_as",
            {"filepath": str(save_path)},
        )
        if getattr(save_result, "isError", False):
            raise RuntimeError(content_text(save_result))
        if not save_path.is_file():
            raise RuntimeError(f"MCP save did not create: {save_path}")

        await expect_blocked(
            session,
            "blender_python_exec",
            {"code": "__result__ = 1"},
            "disabled",
        )
        await expect_blocked(
            session,
            "blender_render_still",
            {"transport": "headless"},
            "disabled",
        )
        await expect_blocked(
            session,
            "blender_export_gltf",
            {"filepath": str(asset_root / "staging" / "exports" / "bypass.glb")},
            "whitelist",
        )

    print(
        "MCP_STDIO_SMOKE_OK "
        f"tools={len(names)} scene={scene['name']} "
        f"render_engine={scene['render_engine']} saved={save_path}"
    )


def main() -> None:
    asyncio.run(run())


if __name__ == "__main__":
    main()
