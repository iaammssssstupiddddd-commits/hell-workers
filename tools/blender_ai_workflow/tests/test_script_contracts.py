from __future__ import annotations

import ast
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = WORKFLOW_ROOT / "scripts"
PROJECT_ROOT = WORKFLOW_ROOT.parents[1]


class ScriptContractTests(unittest.TestCase):
    def test_all_blender_scripts_parse(self) -> None:
        for path in sorted(SCRIPTS_ROOT.glob("*.py")):
            with self.subTest(path=path.name):
                ast.parse(path.read_text(encoding="utf-8"), filename=str(path))

    def test_common_path_guard_rejects_shared_prefix_sibling(self) -> None:
        spec = importlib.util.spec_from_file_location(
            "workflow_common",
            SCRIPTS_ROOT / "workflow_common.py",
        )
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            allowed = parent / "assets"
            sibling = parent / "assets-escape"
            allowed.mkdir()
            sibling.mkdir()
            self.assertTrue(module.is_within(allowed / "model.glb", allowed))
            self.assertFalse(module.is_within(sibling / "model.glb", allowed))

    def test_export_script_targets_staging_only(self) -> None:
        source = (SCRIPTS_ROOT / "export_glb.py").read_text(encoding="utf-8")
        self.assertIn('staging_path(args.output, "exports")', source)
        self.assertIn("use_renderable=True", source)
        self.assertNotIn("sync_external_assets", source)

    def test_codex_mcp_surface_excludes_execution_and_direct_export(self) -> None:
        config = (PROJECT_ROOT / ".codex/config.toml").read_text(encoding="utf-8")
        self.assertIn('"blender_scene_save_as"', config)
        self.assertIn('BLENDER_MCP_ALLOW_HEADLESS = "0"', config)
        self.assertIn('BLENDER_MCP_ALLOW_PYTHON_EXEC = "0"', config)
        self.assertNotIn('"blender_python_exec"', config)
        self.assertNotIn('"blender_export_gltf"', config)

    def test_batch_wrappers_isolate_network_and_user_addons(self) -> None:
        for name in ("validate-blend", "export-staging-glb", "workflow-smoke"):
            source = (WORKFLOW_ROOT / "bin" / name).read_text(encoding="utf-8")
            with self.subTest(wrapper=name):
                self.assertIn("BLENDER_SAFE_NO_NETWORK=1", source)
                self.assertIn("--factory-startup", source)

    def test_manifest_template_is_valid_json(self) -> None:
        path = WORKFLOW_ROOT / "templates/asset-manifest.template.json"
        payload = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(payload["schema_version"], 1)
        self.assertIn("approval", payload)


if __name__ == "__main__":
    unittest.main()
