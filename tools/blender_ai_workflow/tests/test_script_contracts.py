from __future__ import annotations

import ast
import importlib.util
import json
import os
import subprocess
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

    def test_wall_post_export_wrapper_is_project_local_and_fail_closed(self) -> None:
        wrapper = (WORKFLOW_ROOT / "bin/validate-wall-glb").read_text(encoding="utf-8")
        self.assertIn("validate_wall_glb.py", wrapper)
        self.assertIn("wall-production-v1.geometry.json", wrapper)
        self.assertIn("staging/exports", wrapper)
        self.assertIn("staging/reports", wrapper)

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

    def test_exact_collection_export_is_opt_in_and_single_mesh(self) -> None:
        export_source = (SCRIPTS_ROOT / "export_glb.py").read_text(encoding="utf-8")
        validation_source = (SCRIPTS_ROOT / "validate_scene.py").read_text(
            encoding="utf-8"
        )
        self.assertIn('parser.add_argument("--collection")', export_source)
        self.assertIn("use_selection=use_selection", export_source)
        self.assertIn('"UNKNOWN_COLLECTION"', validation_source)
        self.assertIn('"EMPTY_COLLECTION"', validation_source)
        self.assertIn('"COLLECTION_NOT_IN_SCENE"', validation_source)
        self.assertIn('"MESH_COUNT"', validation_source)
        for name in ("validate-blend", "export-staging-glb"):
            wrapper = (WORKFLOW_ROOT / "bin" / name).read_text(encoding="utf-8")
            with self.subTest(wrapper=name):
                self.assertIn("--collection", wrapper)
                self.assertIn("--require-single-mesh", wrapper)

    def test_wall_export_scale_and_placeholder_material_are_opt_in(self) -> None:
        source = (SCRIPTS_ROOT / "export_glb.py").read_text(encoding="utf-8")
        wrapper = (WORKFLOW_ROOT / "bin/export-staging-glb").read_text(encoding="utf-8")
        self.assertIn('parser.add_argument("--geometry-scale"', source)
        self.assertIn('default="export"', source)
        self.assertIn("obj.data = obj.data.copy()", source)
        self.assertIn("vertex.co *= args.geometry_scale", source)
        self.assertIn("export_materials=args.materials_mode.upper()", source)
        self.assertIn("--geometry-scale", wrapper)
        self.assertIn("--materials-mode", wrapper)
        self.assertIn('OUTPUT_RELATIVE="$2"', wrapper)
        self.assertNotIn("reports/$2.khronos.json", wrapper)

    def test_wall_scene_generator_freezes_six_families_and_export_scale(self) -> None:
        source = (SCRIPTS_ROOT / "create_wall_production_scene.py").read_text(
            encoding="utf-8"
        )
        for collection in (
            "Wall_Isolated",
            "Wall_End",
            "Wall_Straight",
            "Wall_Corner",
            "Wall_TJunction",
            "Wall_Cross",
        ):
            self.assertIn(collection, source)
        self.assertIn('obj["hw_export_scale"] = 32.0', source)
        self.assertIn('obj["hw_nominal_thickness_wu"] = 9.6', source)
        self.assertIn('uv_layers.new(name="UVMap")', source)
        wrapper = (WORKFLOW_ROOT / "bin/create-wall-production-scene").read_text(
            encoding="utf-8"
        )
        self.assertIn("create_wall_production_scene.py", wrapper)
        self.assertIn("--factory-startup", wrapper)

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

    def test_wall_asset_set_template_is_a_separate_v2_schema(self) -> None:
        generic = json.loads(
            (WORKFLOW_ROOT / "templates/asset-manifest.template.json").read_text(
                encoding="utf-8"
            )
        )
        wall = json.loads(
            (WORKFLOW_ROOT / "templates/asset-set-manifest-v2.template.json").read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(generic["schema_version"], 1)
        self.assertEqual(wall["schema_version"], 2)
        self.assertEqual(wall["asset_set_id"], "wall-production-v1")
        self.assertEqual(len(wall["meshes"]), 6)
        self.assertEqual(wall["normal_decision"], "pending")

    def test_workspace_init_is_idempotent_and_preserves_generic_v1(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "assets"
            environment = {**os.environ, "HELL_WORKERS_ASSET_ROOT": str(root)}
            init = WORKFLOW_ROOT / "bin/init-asset-workspace"
            verify = WORKFLOW_ROOT / "bin/verify-asset-workspace"
            subprocess.run([init], check=True, env=environment, capture_output=True)
            generic = root / "manifests/asset-manifest.template.json"
            generic.write_text('{"schema_version":1,"preserved":true}\n')
            subprocess.run([init], check=True, env=environment, capture_output=True)
            subprocess.run([verify], check=True, env=environment, capture_output=True)
            self.assertTrue(json.loads(generic.read_text())["preserved"])
            wall = root / "manifests/wall-production-v1.asset-set.template.json"
            self.assertEqual(json.loads(wall.read_text())["schema_version"], 2)
            for relative in ("generations", "authority", "quarantine"):
                self.assertTrue((root / relative).is_dir())


if __name__ == "__main__":
    unittest.main()
