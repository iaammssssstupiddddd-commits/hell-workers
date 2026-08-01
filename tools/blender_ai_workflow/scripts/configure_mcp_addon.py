"""Install and harden the Blender MCP bridge for this asset workspace."""

from __future__ import annotations

import argparse
import shutil
import sys
from datetime import UTC, datetime
from pathlib import Path

import addon_utils
import bpy


def script_arguments() -> list[str]:
    if "--" not in sys.argv:
        return []
    return sys.argv[sys.argv.index("--") + 1 :]


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zip", required=True)
    parser.add_argument("--asset-root", required=True)
    return parser


def snapshot_user_preferences() -> Path | None:
    source = Path(bpy.utils.user_resource("CONFIG", path="userpref.blend"))
    if not source.is_file():
        return None
    snapshot_root = source.parent / "backups" / "hell-workers"
    snapshot_root.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    destination = snapshot_root / f"blender-userpref-before-mcp-{timestamp}.blend"
    shutil.copy2(source, destination)
    destination.chmod(0o600)
    return destination


def main() -> None:
    args = build_parser().parse_args(script_arguments())
    archive = Path(args.zip).expanduser().resolve()
    asset_root = Path(args.asset_root).expanduser().resolve()
    if not archive.is_file():
        raise FileNotFoundError(archive)
    if not asset_root.is_dir():
        raise FileNotFoundError(asset_root)
    staging_root = asset_root / "staging"
    if not staging_root.is_dir():
        raise FileNotFoundError(staging_root)

    module_name = "blender_mcp_bridge"
    preference_snapshot = snapshot_user_preferences()
    addon_utils.disable(module_name, default_set=True)
    for loaded_name in tuple(sys.modules):
        if loaded_name == module_name or loaded_name.startswith(f"{module_name}."):
            del sys.modules[loaded_name]
    bpy.ops.preferences.addon_install(filepath=str(archive), overwrite=True)
    addon_utils.modules_refresh()
    addon_utils.enable(module_name, default_set=True, persistent=True)

    entry = bpy.context.preferences.addons.get(module_name)
    if entry is None:
        raise RuntimeError(f"add-on was not enabled: {module_name}")
    preferences = entry.preferences
    preferences.safe_mode = True
    preferences.allow_inline_code = False
    preferences.auto_start = False
    preferences.port = 9876
    preferences.approved_script_roots = str(staging_root)

    file_preferences = bpy.context.preferences.filepaths
    if hasattr(file_preferences, "use_scripts_auto_execute"):
        file_preferences.use_scripts_auto_execute = False

    bpy.ops.wm.save_userpref()
    print(
        "MCP_ADDON_CONFIGURED "
        f"module={module_name} safe_mode={preferences.safe_mode} "
        f"inline={preferences.allow_inline_code} port={preferences.port} "
        f"roots={preferences.approved_script_roots} "
        f"preference_snapshot={preference_snapshot or 'none'}"
    )


if __name__ == "__main__":
    main()
