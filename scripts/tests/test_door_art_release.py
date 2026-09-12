from __future__ import annotations

import copy
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

HELPER = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts/door_art_acceptance.py"
spec = importlib.util.spec_from_file_location("door_art_release_test_subject", HELPER)
assert spec is not None and spec.loader is not None
art = importlib.util.module_from_spec(spec)
spec.loader.exec_module(art)
behavior = art.behavior


class DoorReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="door-release-test-")
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        self.locator = self.repo / behavior.DOORSET_RELATIVE
        self.locator.parent.mkdir(parents=True)
        identity = {"asset_set_id": "door-production-v1", "asset_set_generation": 6,
                    "manifest_sha256": "a" * 64}
        receipt = {"schema_version": 1, **identity, "new_active": identity}
        self.receipt_path = self.repo / "assets/door_sets/6/authority/promotion-receipt.json"
        self.receipt_path.parent.mkdir(parents=True)
        self.receipt_path.write_bytes(behavior.canonical_bytes(receipt))
        core = []
        for index, role in enumerate(("mesh:closed", "mesh:open", "mesh:locked", "texture:albedo", "preview:ew", "preview:ns")):
            path = self.repo / f"assets/door_sets/6/core-{index}"
            path.write_bytes(role.encode())
            core.append({"role": role, **self.record(path)})
        self.payload = {"schema_version": 1, **identity, "authority": "release_approved",
                        "review_status": "art_approved", "normal_decision": "not_used_by_design",
                        "core": core, "receipt": self.record(self.receipt_path)}
        self.write_locator()

    def record(self, path):
        return {"path": path.relative_to(self.repo / "assets").as_posix(),
                "bytes": path.stat().st_size, "sha256": behavior.sha256(path)}

    def write_locator(self):
        self.locator.write_bytes(behavior.canonical_bytes(self.payload))

    def test_release_requires_explicit_selection(self):
        identity = behavior.candidate_identity(self.repo, release=True)
        self.assertEqual(identity["authority"], "release_approved")
        self.assertEqual(identity["receipt_sha256"], behavior.sha256(self.receipt_path))
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo)
        self.payload["authority"] = "isolated_candidate"
        self.payload["receipt"] = None
        self.write_locator()
        self.assertNotIn("authority", behavior.candidate_identity(self.repo))
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo, release=True)

    def test_corrupt_or_missing_receipt_is_rejected(self):
        self.receipt_path.write_bytes(b"changed")
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo, release=True)
        self.receipt_path.unlink()
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo, release=True)

    def test_rehashed_receipt_for_another_asset_is_rejected(self):
        receipt = art.native.read_json(self.receipt_path)
        receipt["asset_set_id"] = "wall-production-v1"
        self.receipt_path.write_bytes(behavior.canonical_bytes(receipt))
        self.payload["receipt"] = self.record(self.receipt_path)
        self.write_locator()
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo, release=True)

    def test_core_identity_paths_and_encoding_fail_closed(self):
        original = copy.deepcopy(self.payload)
        for mutation in ("hash", "duplicate", "escape", "normal", "review", "generation"):
            with self.subTest(mutation=mutation):
                self.payload = copy.deepcopy(original)
                if mutation == "hash":
                    self.payload["core"][0]["sha256"] = "b" * 64
                elif mutation == "duplicate":
                    self.payload["core"][1] = {**self.payload["core"][0], "role": "mesh:open"}
                elif mutation == "escape":
                    self.payload["core"][0]["path"] = "door_sets/7/absent"
                elif mutation == "normal":
                    self.payload["normal_decision"] = "adopted"
                elif mutation == "review":
                    self.payload["review_status"] = "pending"
                else:
                    self.payload["asset_set_generation"] = True
                self.write_locator()
                with self.assertRaises(art.native.AcceptanceError):
                    behavior.candidate_identity(self.repo, release=True)
        self.payload = original
        self.write_locator()
        self.locator.write_bytes(self.locator.read_bytes() + b" ")
        with self.assertRaises(art.native.AcceptanceError):
            behavior.candidate_identity(self.repo, release=True)

    def test_release_environment_removes_candidate_and_preview_opt_ins(self):
        identity = behavior.candidate_identity(self.repo, release=True)
        forbidden = {"HW_DOOR_ART_PREVIEW": "1", "HW_WALL_ART_PREVIEW": "1",
                     "HW_DOOR_CANDIDATE": "1", "HW_WALL_CANDIDATE": "1",
                     "HW_DOOR_CANDIDATE_GENERATION": "99", "HW_WALL_CANDIDATE_GENERATION": "99",
                     "HW_DOOR_CANDIDATE_MANIFEST_SHA256": "b" * 64,
                     "HW_WALL_CANDIDATE_MANIFEST_SHA256": "b" * 64}
        with mock.patch.dict(os.environ, forbidden):
            env = art.run_environment(self.repo, self.repo / "job", "Intel", "nonce", identity, release=True)
            self.assertTrue(set(forbidden).isdisjoint(env))
            self.assertEqual(env["HW_DOOR_ART_ACTUAL_WINDOW"], "1")
            env = art.run_environment(self.repo, self.repo / "job", "Intel", "nonce", identity, release=False)
            self.assertEqual(env["HW_DOOR_CANDIDATE_GENERATION"], "6")
            self.assertNotIn("HW_DOOR_ART_PREVIEW", env)

    def test_profile_and_cli_selection_are_explicit(self):
        self.assertFalse(art.parser().parse_args(["plan", "--repo", str(self.repo)]).release)
        self.assertTrue(art.parser().parse_args(["plan", "--repo", str(self.repo), "--release"]).release)
        self.assertNotEqual(art.profile_name(False), art.profile_name(True))
        for value in (None, "false", 1):
            with self.assertRaises(art.native.AcceptanceError):
                art.profile_name(value)


if __name__ == "__main__":
    unittest.main()
