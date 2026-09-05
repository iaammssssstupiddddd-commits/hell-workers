from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

WORKFLOW_ROOT = Path(__file__).resolve().parents[1]
PROMOTION_SCRIPT = WORKFLOW_ROOT / "scripts/promote_asset_set.py"
ASSET_FIXTURE_SCRIPT = WORKFLOW_ROOT / "tests/test_asset_set_manifest.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PromotionFixture:
    def __init__(self, root: Path, promotion, fixture_module) -> None:
        self.root = root
        self.asset_root = root / "canonical-assets"
        staging_root = self.asset_root / "staging"
        staging_root.mkdir(parents=True)
        validator = fixture_module.load_validator()
        self.assets = fixture_module.AssetSetFixture(staging_root, validator)
        self.assets.licenses_root.rename(self.asset_root / "licenses")
        self.assets.licenses_root = self.asset_root / "licenses"
        self.assets.make_final(normal="rejected")
        self.snapshot = root / "snapshots/preimage.json"
        self.plan_path = root / "promotion-plan.json"
        self.evidence = self.assets.reports_root / "m5-evidence.json"
        self.approval = self.assets.reports_root / "release-approval.json"
        self.evidence.write_text('{"status":"pass"}\n', encoding="utf-8")
        self.approval.write_text('{"approved":true}\n', encoding="utf-8")
        self.promotion = promotion
        self.seal_plan()

    def seal_plan(self) -> dict[str, object]:
        plan = self.promotion.build_plan(
            self.assets.manifest_path,
            self.asset_root,
            self.snapshot,
            repo=None,
        )
        self.plan_path.write_bytes(self.promotion.canonical_json(plan))
        return plan

    def apply(self, receipt_id: str = "receipt-00000001", kill_hook=None):
        return self.promotion.apply_plan(
            self.plan_path,
            evidence_bundle=self.evidence,
            approval=self.approval,
            receipt_id=receipt_id,
            approved_at_utc="2026-09-01T00:02:00Z",
            repo=None,
            kill_hook=kill_hook,
        )

    def next_generation(self) -> dict[str, object]:
        self.assets.manifest["asset_set_generation"] += 1
        self.assets.write()
        self.snapshot = self.root / (
            f"snapshots/preimage-{self.assets.manifest['asset_set_generation']}.json"
        )
        self.plan_path = self.root / (
            f"promotion-plan-{self.assets.manifest['asset_set_generation']}.json"
        )
        return self.seal_plan()


class AssetSetPromotionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.promotion = load_module("promote_asset_set", PROMOTION_SCRIPT)
        cls.fixture_module = load_module(
            "test_asset_set_manifest_for_promotion", ASSET_FIXTURE_SCRIPT
        )

    def test_plan_is_read_only_and_binds_absent_preimage(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            plan = self.promotion.read_canonical_json(
                fixture.plan_path, "promotion plan"
            )
            self.assertEqual(plan["previous_active"], {"status": "absent"})
            self.assertEqual(plan["asset_set_generation"], 1)
            self.assertFalse((fixture.asset_root / "generations").exists())
            self.assertFalse(fixture.snapshot.exists())

    def test_apply_promotes_complete_generation_and_exact_allowlist(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            unlisted = fixture.assets.exports_root / "audio/unlisted.ogg"
            unlisted.parent.mkdir()
            unlisted.write_bytes(b"unlisted")
            result = fixture.apply()
            generation = fixture.asset_root / "generations/1"
            pointer = self.promotion.pointer_identity(fixture.asset_root)

            self.assertEqual(result["status"], "applied")
            self.assertEqual(pointer["asset_set_generation"], 1)
            self.assertTrue(fixture.snapshot.is_file())
            self.assertFalse((generation / "exports/audio/unlisted.ogg").exists())
            plan = self.promotion.read_canonical_json(
                fixture.plan_path, "promotion plan"
            )
            for entry in plan["payload"]:
                self.assertEqual(
                    self.promotion.sha256(generation / entry["destination"]),
                    entry["sha256"],
                )
            self.promotion.validate_active_generation(
                fixture.asset_root, plan, fixture.plan_path
            )

    def test_promoted_generation_validates_on_its_own(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply()
            generation = fixture.asset_root / "generations/1"
            validator = self.promotion.load_manifest_validator()
            # Every file the manifest points at has to travel with it, or the
            # canonical generation cannot be re-validated after promotion.
            validator.validate_manifest(
                generation / "manifest/wall-production-v1.asset-set.json",
                mode="final",
                blend_root=generation / "source/blender",
                exports_root=generation / "exports",
                reports_root=generation / "reports",
                licenses_root=generation / "licenses",
                repo=None,
            )

    def test_source_tamper_after_plan_is_rejected_before_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            relative = fixture.assets.manifest["production"]["core"][0]["path"]
            (fixture.assets.exports_root / relative).write_bytes(b"tampered")
            with self.assertRaisesRegex(RuntimeError, "sha256 differs|hash differs"):
                fixture.apply()
            self.assertFalse(fixture.snapshot.exists())
            self.assertFalse((fixture.asset_root / "generations").exists())

    def test_noncanonical_plan_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            plan = json.loads(fixture.plan_path.read_text())
            fixture.plan_path.write_text(json.dumps(plan, indent=2) + "\n")
            with self.assertRaisesRegex(self.promotion.PromotionError, "not canonical"):
                fixture.apply()

    def test_receipt_id_cannot_be_reused_for_next_generation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply(receipt_id="receipt-reused-01")
            fixture.next_generation()
            with self.assertRaisesRegex(self.promotion.PromotionError, "already used"):
                fixture.apply(receipt_id="receipt-reused-01")
            self.assertFalse(fixture.snapshot.exists())

    def test_matching_snapshot_allows_retry_after_temporary_recovery(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )

            def kill(stage: str) -> None:
                if stage == "after_temporary_create":
                    raise InterruptedError(stage)

            with self.assertRaises(InterruptedError):
                fixture.apply(kill_hook=kill)
            self.assertTrue(fixture.snapshot.is_file())
            recovered = self.promotion.recover_plan(fixture.plan_path, apply=True)
            self.assertEqual(len(recovered["quarantined"]), 1)
            result = fixture.apply(receipt_id="receipt-retry-0001")
            self.assertEqual(result["status"], "applied")

    def test_full_inert_generation_cannot_reuse_generation_number(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )

            def kill(stage: str) -> None:
                if stage == "after_generation_parent_fsync":
                    raise InterruptedError(stage)

            with self.assertRaises(InterruptedError):
                fixture.apply(kill_hook=kill)
            self.promotion.recover_plan(fixture.plan_path, apply=True)
            with self.assertRaisesRegex(
                self.promotion.PromotionError, "monotonically increasing"
            ):
                fixture.apply(receipt_id="receipt-retry-0002")

    def test_stale_active_pointer_after_plan_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            pointer_path = fixture.asset_root / self.promotion.ACTIVE_POINTER
            pointer_path.parent.mkdir()
            pointer_path.write_bytes(
                self.promotion.canonical_json(
                    {
                        "schema_version": 1,
                        "asset_set_id": "wall-production-v1",
                        "asset_set_generation": 99,
                        "manifest_sha256": "1" * 64,
                        "receipt_path": "generations/99/authority/promotion-receipt.json",
                        "receipt_sha256": "2" * 64,
                    }
                )
            )
            with self.assertRaisesRegex(
                self.promotion.PromotionError, "current inputs or active pointer"
            ):
                fixture.apply()
            self.assertFalse(fixture.snapshot.exists())

    def test_kill_points_leave_only_absent_or_complete_active_generation(self) -> None:
        with tempfile.TemporaryDirectory() as probe_directory:
            probe = PromotionFixture(
                Path(probe_directory), self.promotion, self.fixture_module
            )
            probe_plan = self.promotion.read_canonical_json(
                probe.plan_path, "promotion plan"
            )
            payload_stages = [
                f"after_copy:{entry['destination']}" for entry in probe_plan["payload"]
            ]
            directories = set()
            destinations = [entry["destination"] for entry in probe_plan["payload"]]
            destinations.extend(
                (
                    "evidence/m5-evidence-bundle.json",
                    "evidence/release-approval.json",
                    "authority/promotion-receipt.json",
                )
            )
            for destination in destinations:
                parent = Path(destination).parent
                while parent != Path("."):
                    directories.add(parent.as_posix())
                    parent = parent.parent
            directory_stages = [
                f"after_directory_fsync:{path}" for path in sorted(directories)
            ] + ["after_directory_fsync:."]
        stages = [
            "after_snapshot_fsync",
            "after_temporary_create",
            *payload_stages,
            "after_evidence_copy",
            "after_approval_copy",
            "after_receipt_fsync",
            *directory_stages,
            "after_generation_fsync",
            "after_generation_rename",
            "after_generation_parent_fsync",
            "after_pointer_temp_fsync",
            "after_pointer_rename",
            "after_pointer_parent_fsync",
        ]
        for ordinal, stage in enumerate(stages):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as directory:
                fixture = PromotionFixture(
                    Path(directory), self.promotion, self.fixture_module
                )

                def kill(current: str, target: str = stage) -> None:
                    if current == target:
                        raise InterruptedError(target)

                with self.assertRaises(InterruptedError):
                    fixture.apply(
                        receipt_id=f"receipt-kill-{ordinal:02d}", kill_hook=kill
                    )
                active = self.promotion.pointer_identity(fixture.asset_root)
                if active["status"] == "present":
                    plan = self.promotion.read_canonical_json(
                        fixture.plan_path, "promotion plan"
                    )
                    self.promotion.validate_active_generation(
                        fixture.asset_root, plan, fixture.plan_path
                    )
                else:
                    self.assertEqual(active, {"status": "absent"})

    def test_recover_quarantines_inert_generation_and_pointer_temp(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )

            def kill(stage: str) -> None:
                if stage == "after_pointer_temp_fsync":
                    raise InterruptedError(stage)

            with self.assertRaises(InterruptedError):
                fixture.apply(kill_hook=kill)
            inspection = self.promotion.recover_plan(fixture.plan_path, apply=False)
            self.assertEqual(inspection["status"], "inert")
            self.assertTrue(inspection["would_quarantine_generation"])
            self.assertEqual(len(inspection["temporary_pointers"]), 1)

            recovered = self.promotion.recover_plan(fixture.plan_path, apply=True)
            self.assertEqual(len(recovered["quarantined"]), 2)
            self.assertFalse((fixture.asset_root / "generations/1").exists())
            self.assertEqual(
                self.promotion.pointer_identity(fixture.asset_root),
                {"status": "absent"},
            )

    def test_rollback_restores_absent_preimage_without_deleting_generation(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply()
            preview = self.promotion.rollback_plan(
                fixture.plan_path, fixture.snapshot, apply=False
            )
            self.assertEqual(preview["status"], "planned")
            rolled_back = self.promotion.rollback_plan(
                fixture.plan_path, fixture.snapshot, apply=True
            )
            self.assertEqual(rolled_back["active"], {"status": "absent"})
            self.assertTrue((fixture.asset_root / "generations/1").is_dir())

    def test_rollback_restores_existing_preimage_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply(receipt_id="receipt-generation-01")
            first_identity = self.promotion.pointer_identity(fixture.asset_root)
            fixture.next_generation()
            fixture.apply(receipt_id="receipt-generation-02")
            self.assertEqual(
                self.promotion.pointer_identity(fixture.asset_root)[
                    "asset_set_generation"
                ],
                2,
            )
            result = self.promotion.rollback_plan(
                fixture.plan_path, fixture.snapshot, apply=True
            )
            self.assertEqual(result["active"], first_identity)
            self.assertTrue((fixture.asset_root / "generations/2").is_dir())

    def test_receipt_tamper_is_rejected_by_active_validation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply()
            plan = self.promotion.read_canonical_json(
                fixture.plan_path, "promotion plan"
            )
            receipt = (
                fixture.asset_root / "generations/1/authority/promotion-receipt.json"
            )
            receipt.chmod(0o644)
            receipt.write_bytes(receipt.read_bytes() + b" ")
            with self.assertRaisesRegex(
                self.promotion.PromotionError, "receipt hash differs"
            ):
                self.promotion.validate_active_generation(
                    fixture.asset_root, plan, fixture.plan_path
                )

    def test_evidence_tamper_is_rejected_by_receipt_validation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture = PromotionFixture(
                Path(directory), self.promotion, self.fixture_module
            )
            fixture.apply()
            plan = self.promotion.read_canonical_json(
                fixture.plan_path, "promotion plan"
            )
            evidence = (
                fixture.asset_root / "generations/1/evidence/m5-evidence-bundle.json"
            )
            evidence.chmod(0o644)
            evidence.write_bytes(b"tampered")
            with self.assertRaisesRegex(
                self.promotion.PromotionError, "evidence_bundle hash differs"
            ):
                self.promotion.validate_active_generation(
                    fixture.asset_root, plan, fixture.plan_path
                )

    def test_receipt_schema_and_all_authority_bindings_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = PromotionFixture(root, self.promotion, self.fixture_module)
            fixture.apply()
            plan = self.promotion.read_canonical_json(
                fixture.plan_path, "promotion plan"
            )
            generation = fixture.asset_root / "generations/1"
            receipt_path = generation / "authority/promotion-receipt.json"
            original = self.promotion.read_canonical_json(
                receipt_path, "promotion receipt"
            )
            mutations = {
                "fields differ": lambda value: value.pop("tool_tree"),
                "payload binding": lambda value: value.update(asset_set_generation=2),
                "plan binding": lambda value: value.update(
                    promotion_plan_sha256="0" * 64
                ),
                "preimage differs": lambda value: value.update(
                    previous_active={"status": "present"}
                ),
            }
            for ordinal, (message, mutate) in enumerate(mutations.items()):
                with self.subTest(message=message):
                    changed = json.loads(json.dumps(original))
                    mutate(changed)
                    candidate = root / f"receipt-negative-{ordinal}.json"
                    candidate.write_bytes(self.promotion.canonical_json(changed))
                    with self.assertRaisesRegex(self.promotion.PromotionError, message):
                        self.promotion.validate_receipt(
                            candidate,
                            plan=plan,
                            plan_hash=self.promotion.sha256(fixture.plan_path),
                            generation_root=generation,
                        )


if __name__ == "__main__":
    unittest.main()
