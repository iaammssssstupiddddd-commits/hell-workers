from __future__ import annotations

import contextlib
import io
import json
import shutil
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKFLOW / "scripts"))
sys.path.insert(0, str(WORKFLOW / "tests"))
sys.path.insert(0, str(WORKFLOW.parents[1] / "scripts"))

import asset_release_manifest as release
import install_release_asset_set as installer
import promote_asset_set as promotion
import project_doorset
import project_wallset
import sync_external_assets as sync
import test_asset_set_manifest as legacy_fixture
from test_asset_set_promotion import PromotionFixture
import test_wall_formwork_contracts as formwork_fixture


def write_record(root: Path, relative: str, payload: bytes | dict) -> dict:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(promotion.canonical_json(payload) if isinstance(payload, dict) else payload)
    return {"path": relative, "sha256": promotion.sha256(path)}


def door_fixture(asset_root: Path, repo: Path, generation: int = 1) -> Path:
    staging = asset_root / "staging"
    exports, reports = staging / "exports", staging / "reports"
    core = []
    for relative, role in release.door.CORE:
        record = write_record(exports, relative, role.encode())
        core.append({**record, "bytes": len(role.encode()), "role": role})
    meshes = []
    for index, (state, triangles) in enumerate(release.door.STATES.items()):
        meshes.append({
            "state": state,
            "export": write_record(reports, f"door/{state}.export.json", {"status": "exported", "sha256": core[index]["sha256"]}),
            "khronos": write_record(reports, f"door/{state}.khronos.json", {"issues": {"numErrors": 0, "numWarnings": 0}}),
            "post_export": write_record(reports, f"door/{state}.post.json", {"status": "pass", "state": state, "triangles": triangles, "glb_sha256": core[index]["sha256"]}),
        })
    manifest = {
        "schema_version": 1, "asset_set_id": release.DOOR_ID,
        "asset_set_generation": generation, "manifest_mode": "final",
        "normal_decision": "not_used_by_design", "created_at_utc": "2026-09-10T00:00:00Z",
        "production": {"core": core, "optional": []}, "mesh_reports": meshes,
        "texture_report": write_record(reports, "door/texture.json", {"status": "pass", "albedo": {"opaque": True, "size": [512, 512]}}),
        "preview_report": write_record(reports, "door/preview.json", {
            "status": "rendered", "anchor_px": [128, 192],
            "ocio": {"fallback": False, "validation_status": "pass"},
            "previews": {axis: {"sha256": core[index]["sha256"]} for axis, index in (("ew", 4), ("ns", 5))},
        }),
        "art_review": {"status": "double_leaf_approved", "artifact": write_record(reports, "door/art.json", {"fixture": True})},
        "license": {"file": write_record(asset_root / "licenses", "door.md", b"fixture license")},
        "provenance": {},
        "source": {
            "blend": write_record(staging / "blend", "door.blend", b"blend fixture"),
            "geometry_contract": write_record(repo, "fixtures/door.json", {"fixture": True}),
            "scene_report": write_record(reports, "door/scene.json", {"status": "pass"}),
            "texture_prompt": write_record(reports, "door/prompt.txt", b"fixture prompt"),
            "runtime_subject": "a" * 40, "source_fingerprint": "b" * 64,
            "tool_commit": "c" * 40, "tool_tree": "d" * 40,
            "tool_versions": {}, "working_tree_diff_sha256": "0" * 64,
        },
    }
    relative = "door-final.json"
    write_record(reports, relative, manifest)
    return reports / relative


class BuildingReleaseTests(unittest.TestCase):
    def plan(self, root: Path, manifest: Path, asset_root: Path, repo: Path | None) -> Path:
        name = manifest.stem
        plan = promotion.build_plan(manifest, asset_root, root / f"{name}.snapshot", repo=repo)
        path = root / f"{name}.plan.json"
        path.write_bytes(promotion.canonical_json(plan))
        return path

    def apply(self, root: Path, plan: Path, repo: Path | None, hook=None) -> dict:
        # Test fixtures only, never release evidence for real assets.
        evidence = root / "fixture-evidence.json"
        approval = root / "fixture-approval.json"
        evidence.write_text('{"fixture":true}\n')
        approval.write_text('{"fixture":true}\n')
        return promotion.apply_plan(plan, evidence_bundle=evidence, approval=approval,
            receipt_id="fixture-" + plan.stem, approved_at_utc="2026-09-10T00:00:00Z",
            repo=repo, kill_hook=hook)

    def sync_release(self, manifest: Path, destination: Path, *, dry_run: bool = False) -> int:
        generation = manifest.parent.parent
        with contextlib.redirect_stdout(io.StringIO()):
            return sync.sync_manifest_assets(source_root=generation / "exports",
                dest_root=destination, manifest_path=manifest, selection="core",
                receipt_path=generation / "authority/promotion-receipt.json",
                dry_run=dry_run, repo=None)

    def test_door_same_generation_as_wall_is_independent_and_rolls_back_to_absent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wall = PromotionFixture(root, promotion, legacy_fixture)
            wall.apply()
            pointer = (wall.asset_root / promotion.ACTIVE_POINTER).read_bytes()
            manifest = door_fixture(wall.asset_root, root / "repo")
            plan_path = self.plan(root, manifest, wall.asset_root, root / "repo")
            self.apply(root, plan_path, root / "repo")
            plan = promotion.read_canonical_json(plan_path, "plan")
            generation = wall.asset_root / plan["target_generation"]
            sealed = generation / "manifest/door-production-v1.asset-set.json"
            receipt = generation / "authority/promotion-receipt.json"
            projection = project_doorset.project_release(sealed, receipt)
            self.assertEqual(projection["core"][0]["path"], "door_sets/1/models/door_closed.glb")
            self.assertEqual(projection["core"][4]["path"], "door_sets/1/textures/buildings/door/door_preview_ew.png")
            self.assertEqual(projection["receipt"]["path"], "door_sets/1/authority/promotion-receipt.json")
            self.assertEqual(projection["authority"], "release_approved")
            self.assertNotIn("candidate_normal", projection)
            # Remove access to both staging and the source repo. The sealed
            # generation must carry everything its validator reads.
            (wall.asset_root / "staging").rename(root / "retired-staging")
            (root / "repo").rename(root / "retired-repo")
            self.assertEqual(release.validate(sealed, repo=None)["status"], "pass")
            destination = root / "runtime"
            self.assertEqual(self.sync_release(sealed, destination, dry_run=True), 6)
            self.assertFalse(destination.exists())
            self.assertEqual(self.sync_release(sealed, destination), 6)
            self.assertEqual(self.sync_release(sealed, destination), 0)
            for record in projection["core"]:
                self.assertEqual(promotion.sha256(destination / record["path"]), record["sha256"])
            snapshot = Path(plan["snapshot"])
            self.assertEqual(promotion.rollback_plan(plan_path, snapshot, apply=True)["status"], "rolled_back")
            self.assertEqual(promotion.pointer_identity(wall.asset_root, release.DOOR_ID), {"status": "absent"})
            self.assertEqual((wall.asset_root / promotion.ACTIVE_POINTER).read_bytes(), pointer)
            self.assertTrue(generation.is_dir())

    def test_door_interruption_recovery_and_generation_reuse_rejection(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan_path = self.plan(root, manifest, assets, repo)

            def interrupt(stage: str) -> None:
                if stage == "after_generation_rename":
                    raise RuntimeError("fixture interruption")

            with self.assertRaisesRegex(RuntimeError, "fixture interruption"):
                self.apply(root, plan_path, repo, interrupt)
            self.assertEqual(promotion.pointer_identity(assets, release.DOOR_ID), {"status": "absent"})
            self.assertEqual(promotion.recover_plan(plan_path, apply=False)["status"], "inert")
            result = promotion.recover_plan(plan_path, apply=True)
            self.assertEqual(len(result["quarantined"]), 1)
            with self.assertRaisesRegex(RuntimeError, "monotonically"):
                self.plan(root, manifest, assets, repo)

    def test_door_receipt_cannot_claim_wall_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan = self.plan(root, manifest, assets, repo)
            self.apply(root, plan, repo)
            receipt = assets / release.generation_directory(release.DOOR_ID) / "1/authority/promotion-receipt.json"
            value = promotion.read_canonical_json(receipt, "receipt")
            value["asset_set_id"] = release.WALL_ID
            wrong = root / "wrong-receipt.json"
            wrong.write_bytes(promotion.canonical_json(value))
            with self.assertRaisesRegex(RuntimeError, "receipt binding"):
                project_doorset.project_release(manifest, wrong)

    def test_install_switches_locator_last_and_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan = self.plan(root, manifest, assets, repo)
            self.apply(root, plan, repo)
            sealed = assets / release.generation_directory(release.DOOR_ID) / "1/manifest/door-production-v1.asset-set.json"
            destination = root / "runtime"
            real_write = promotion.write_atomic

            def assert_payload_ready(path: Path, payload: bytes, suffix: str) -> Path:
                projection = json.loads(payload)
                for record in [*projection["core"], projection["receipt"]]:
                    self.assertEqual(promotion.sha256(destination / record["path"]), record["sha256"])
                return real_write(path, payload, suffix)

            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(installer.install(sealed, destination, repo=None, apply=False)["status"], "planned")
                self.assertFalse(destination.exists())
                with mock.patch.object(promotion, "write_atomic", side_effect=assert_payload_ready) as write:
                    self.assertEqual(installer.install(sealed, destination, repo=None, apply=True)["status"], "installed")
                    self.assertFalse(installer.install(sealed, destination, repo=None, apply=True)["locator_changed"])
                    self.assertEqual(write.call_count, 1)
            self.assertFalse((destination / "manifests/wall-production-v1.wallset").exists())

    def test_install_rejects_extra_entry_and_locator_symlink_without_writes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan = self.plan(root, manifest, assets, repo)
            self.apply(root, plan, repo)
            sealed = assets / release.generation_directory(release.DOOR_ID) / "1/manifest/door-production-v1.asset-set.json"
            destination = root / "runtime"
            extra = destination / "door_sets/1/extra.txt"
            extra.parent.mkdir(parents=True)
            extra.write_text("keep")
            with self.assertRaisesRegex(RuntimeError, "unexpected runtime"):
                installer.install(sealed, destination, repo=None, apply=True)
            self.assertEqual(extra.read_text(), "keep")
            self.assertFalse((destination / "manifests").exists())
            other = root / "outside"
            other.mkdir()
            (destination / "manifests").symlink_to(other, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "symlink"):
                installer.install(sealed, destination, repo=None, apply=True)
            self.assertEqual(list(other.iterdir()), [])

    def test_install_failure_before_switch_preserves_locator_and_can_retry(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan = self.plan(root, manifest, assets, repo)
            self.apply(root, plan, repo)
            sealed = assets / release.generation_directory(release.DOOR_ID) / "1/manifest/door-production-v1.asset-set.json"
            destination = root / "runtime"
            locator = destination / installer.locator_path(release.DOOR_ID)
            locator.parent.mkdir(parents=True)
            previous = b"previous fixture locator\n"
            locator.write_bytes(previous)
            with contextlib.redirect_stdout(io.StringIO()):
                with mock.patch.object(promotion, "fsync_tree", side_effect=OSError("fixture interruption")):
                    with self.assertRaisesRegex(OSError, "fixture interruption"):
                        installer.install(sealed, destination, repo=None, apply=True)
                self.assertEqual(locator.read_bytes(), previous)
                self.assertEqual(installer.install(sealed, destination, repo=None, apply=True)["status"], "installed")
            self.assertEqual(json.loads(locator.read_bytes())["authority"], "release_approved")

    def test_door_tamper_after_plan_is_rejected_before_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets, repo = root / "assets", root / "repo"
            manifest = door_fixture(assets, repo)
            plan = self.plan(root, manifest, assets, repo)
            snapshot = Path(promotion.read_canonical_json(plan, "plan")["snapshot"])
            core = release.wall.read_json(manifest)["production"]["core"]
            (assets / "staging/exports" / core[-1]["path"]).write_bytes(b"tampered")
            with self.assertRaisesRegex(RuntimeError, "byte length differs|hash differs"):
                self.apply(root, plan, repo)
            self.assertFalse(snapshot.exists())
            self.assertFalse((assets / release.generation_directory(release.DOOR_ID)).exists())

    def test_formwork_carries_completed_source_and_rejects_completed_byte_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wall = PromotionFixture(root / "legacy", promotion, legacy_fixture)
            wall.apply()
            assets = root / "formwork"
            staging = assets / "staging"
            staging.mkdir(parents=True)
            manifest, payload, roots = formwork_fixture.WallFormworkManifestTests().fixture(staging)
            roots["licenses"].rename(assets / "licenses")
            shutil.copytree(wall.asset_root / "generations", assets / "generations")
            shutil.copytree(wall.asset_root / "authority", assets / "authority")
            completed = assets / "generations/1/manifest/wall-production-v1.asset-set.json"
            old = release.wall.read_json(completed)
            payload["production"]["core"][:8] = old["production"]["core"]
            for record in old["production"]["core"]:
                shutil.copyfile(assets / "generations/1/exports" / record["path"], roots["exports"] / record["path"])
            payload["source"]["completed"] = {"asset_set_generation": 1, "manifest_sha256": promotion.sha256(completed)}
            payload["source"]["runtime_subject"] = "a" * 40
            payload["source"]["working_tree_diff_sha256"] = "0" * 64
            payload["manifest_mode"] = "final"
            payload["art_review"] = {"status": "art_approved", "artifact": write_record(roots["reports"], "art.json", {"fixture": True})}
            manifest.write_bytes(promotion.canonical_json(payload))
            plan = self.plan(root, manifest, assets, None)
            self.apply(root, plan, None)
            sealed = assets / "generations/5/manifest/wall-production-v1.asset-set.json"
            receipt = sealed.parent.parent / "authority/promotion-receipt.json"
            projection = project_wallset.project_release(sealed, receipt)
            self.assertEqual(projection["schema_version"], 2)
            self.assertEqual(len(projection["core"]), 15)
            self.assertEqual(projection["core"][8]["path"], "wall_sets/5/models/wall_formwork_isolated.glb")
            staging.rename(root / "retired-staging")
            (assets / "generations/1").chmod(0o755)
            (assets / "generations/1").rename(root / "retired-completed")
            self.assertEqual(release.validate(sealed, repo=None)["status"], "pass")
            destination = root / "runtime"
            self.assertEqual(self.sync_release(sealed, destination), 15)
            changed = destination / projection["core"][-1]["path"]
            changed.chmod(0o644)
            changed.write_bytes(b"different generation bytes")
            with self.assertRaisesRegex(ValueError, "Immutable"):
                self.sync_release(sealed, destination)
            completed_mesh = sealed.parent.parent / "exports" / payload["production"]["core"][0]["path"]
            completed_mesh.chmod(0o644)
            completed_mesh.write_bytes(b"tampered")
            with self.assertRaisesRegex(RuntimeError, "completed core bytes"):
                release.validate(sealed, repo=None)


if __name__ == "__main__":
    unittest.main()
