from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
SYNC_SCRIPT = PROJECT_ROOT / "scripts/sync_external_assets.py"
ASSET_FIXTURE_SCRIPT = (
    PROJECT_ROOT / "tools/blender_ai_workflow/tests/test_asset_set_manifest.py"
)
PROMOTION_TEST_SCRIPT = (
    PROJECT_ROOT / "tools/blender_ai_workflow/tests/test_asset_set_promotion.py"
)


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ManifestAssetSyncTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sync = load_module("sync_external_assets", SYNC_SCRIPT)
        cls.fixture_module = load_module(
            "test_asset_set_manifest_fixture", ASSET_FIXTURE_SCRIPT
        )
        cls.validator = cls.fixture_module.load_validator()

    def make_fixture(self, directory: str):
        external_root = Path(directory) / "external"
        staging_root = external_root / "staging"
        staging_root.mkdir(parents=True)
        fixture = self.fixture_module.AssetSetFixture(staging_root, self.validator)
        fixture.licenses_root.rename(external_root / "licenses")
        fixture.licenses_root = external_root / "licenses"
        fixture.make_final(normal="rejected")
        return fixture, external_root

    def promoted_generation(self, directory: str):
        promotion_tests = load_module("promotion_fixture", PROMOTION_TEST_SCRIPT)
        promotion = load_module("promote_asset_set", promotion_tests.PROMOTION_SCRIPT)
        fixture = promotion_tests.PromotionFixture(
            Path(directory) / "promotion", promotion, self.fixture_module
        )
        fixture.apply()
        generation = fixture.asset_root / "generations/1"
        return generation, fixture

    def test_released_generation_lands_where_the_projection_names_it(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            generation, _ = self.promoted_generation(directory)
            dest_root = Path(directory) / "repo-assets"

            copied = self.sync.sync_manifest_assets(
                source_root=generation / "exports",
                dest_root=dest_root,
                manifest_path=generation / "manifest/wall-production-v1.asset-set.json",
                selection="core",
                dry_run=False,
                repo=None,
                receipt_path=generation / "authority/promotion-receipt.json",
            )

            self.assertEqual(copied, 8)
            meshes = sorted(
                str(path.relative_to(dest_root))
                for path in dest_root.rglob("*.glb")
            )
            self.assertEqual(
                meshes,
                [
                    f"wall_sets/1/models/wall_{name}.glb"
                    for name in (
                        "corner",
                        "cross",
                        "end",
                        "isolated",
                        "straight",
                        "t_junction",
                    )
                ],
            )
            self.assertTrue(
                (dest_root / "wall_sets/1/textures/buildings/wall/wall_albedo.png").is_file()
            )
            self.assertFalse((dest_root / "models").exists())

    def test_released_generation_requires_its_own_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            generation, fixture = self.promoted_generation(directory)
            dest_root = Path(directory) / "repo-assets"
            manifest_path = generation / "manifest/wall-production-v1.asset-set.json"

            with self.assertRaises(ValueError):
                self.sync.sync_manifest_assets(
                    source_root=generation / "exports",
                    dest_root=dest_root,
                    manifest_path=manifest_path,
                    selection="core",
                    dry_run=True,
                    repo=None,
                )

            foreign = Path(directory) / "foreign-receipt.json"
            foreign.write_text('{"manifest_sha256":"0"}\n', encoding="utf-8")
            with self.assertRaises(ValueError):
                self.sync.sync_manifest_assets(
                    source_root=generation / "exports",
                    dest_root=dest_root,
                    manifest_path=manifest_path,
                    selection="core",
                    dry_run=True,
                    repo=None,
                    receipt_path=foreign,
                )

    def test_core_sync_copies_only_eight_manifest_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, external_root = self.make_fixture(directory)
            unrelated_source = fixture.exports_root / "audio/unlisted.ogg"
            unrelated_source.parent.mkdir()
            unrelated_source.write_bytes(b"unlisted")
            dest_root = Path(directory) / "repo-assets"
            unrelated_dest = dest_root / "textures/keep.png"
            unrelated_dest.parent.mkdir(parents=True)
            unrelated_dest.write_bytes(b"keep")

            copied = self.sync.sync_manifest_assets(
                source_root=fixture.exports_root,
                dest_root=dest_root,
                manifest_path=fixture.manifest_path,
                selection="core",
                dry_run=False,
                repo=None,
            )

            self.assertEqual(copied, 8)
            self.assertTrue(unrelated_dest.is_file())
            self.assertFalse((dest_root / "audio/unlisted.ogg").exists())
            self.assertFalse((dest_root / self.validator.TEXTURES["normal"]).exists())
            for record in fixture.manifest["production"]["core"]:
                self.assertEqual(
                    self.sync.sha256(dest_root / record["path"]), record["sha256"]
                )
            self.assertEqual(external_root.name, "external")

    def test_optional_normal_selection_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, _ = self.make_fixture(directory)
            with self.assertRaisesRegex(ValueError, "approved core"):
                self.sync.selected_manifest_records(
                    fixture.manifest, "optional:normal"
                )

    def test_dry_run_does_not_create_destination(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, _ = self.make_fixture(directory)
            dest_root = Path(directory) / "absent-assets"
            copied = self.sync.sync_manifest_assets(
                source_root=fixture.exports_root,
                dest_root=dest_root,
                manifest_path=fixture.manifest_path,
                selection="core",
                dry_run=True,
                repo=None,
            )
            self.assertEqual(copied, 8)
            self.assertFalse(dest_root.exists())

    def test_tampered_source_is_rejected_before_copy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, _ = self.make_fixture(directory)
            relative = fixture.manifest["production"]["core"][0]["path"]
            (fixture.exports_root / relative).write_bytes(b"tampered")
            dest_root = Path(directory) / "repo-assets"
            with self.assertRaisesRegex(RuntimeError, "sha256 differs"):
                self.sync.sync_manifest_assets(
                    source_root=fixture.exports_root,
                    dest_root=dest_root,
                    manifest_path=fixture.manifest_path,
                    selection="core",
                    dry_run=False,
                    repo=None,
                )
            self.assertFalse(dest_root.exists())

    def test_destination_symlink_escape_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, _ = self.make_fixture(directory)
            dest_root = Path(directory) / "repo-assets"
            outside = Path(directory) / "outside"
            dest_root.mkdir()
            outside.mkdir()
            (dest_root / "models").symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "escapes|symlink"):
                self.sync.sync_manifest_assets(
                    source_root=fixture.exports_root,
                    dest_root=dest_root,
                    manifest_path=fixture.manifest_path,
                    selection="core",
                    dry_run=False,
                    repo=None,
                )
            self.assertEqual(list(outside.iterdir()), [])

    def test_legacy_copy_helper_keeps_relative_layout(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_top = root / "source/textures"
            dest_top = root / "dest/textures"
            source_file = source_top / "ui/icon.png"
            source_file.parent.mkdir(parents=True)
            source_file.write_bytes(b"legacy")
            self.assertTrue(
                self.sync.copy_if_needed(
                    source_file, source_top, dest_top, dry_run=False
                )
            )
            self.assertEqual((dest_top / "ui/icon.png").read_bytes(), b"legacy")


if __name__ == "__main__":
    unittest.main()
