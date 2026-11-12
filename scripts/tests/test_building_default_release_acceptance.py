from __future__ import annotations

import copy
import argparse
import tempfile
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from scripts import building_default_release_acceptance as audit


class DefaultStartupTests(unittest.TestCase):
    def setUp(self):
        self.prepare(("Tank", "MudMixer"))

    def prepare(self, kinds):
        self.bindings = [{"identity": {"kind": kind, "generation": 1, "authority": "release_approved",
            "manifest_sha256": "a"*64}, "locator": audit.pipeline.locator(kind)} for kind in kinds]
        self.value = {"bindings": self.bindings, "inventory_sha256": "b"*64}
        self.rows = [{"schema_version": 1, "source": "compiled_default", "status": status,
            "process_id": 42, "inventory_sha256": "b"*64, "bindings": copy.deepcopy(self.bindings),
            "detail": {}, "accepted": False, "promotion_authority": False, "performance_evidence": False}
            for status in ("selected", "active")]
        generations = []
        for binding in self.bindings:
            kind = binding["identity"]["kind"]
            generations.append({"expected": binding["identity"], "active": binding["identity"],
                "descriptor_identity": binding["identity"], "locator": binding["locator"],
                "pending": None, "failure": None,
                **{field: [{"role": role, "handle": kind+role, "resident": True}
                            for role in audit.pipeline.KINDS[kind][roles]]
                   for field, roles in [("meshes", "mesh_roles"), ("images", "image_roles")]}})
        self.rows[1]["detail"] = {"generations": generations, "consumers": {
            "owners": [], "roots": [], "sprites": [], "scope": "existing-normal-world-consumers-only", "spawned_for_audit": False}}

    def test_existing_pool_activation_does_not_invent_absent_consumers(self):
        result = audit.check_records(self.rows, self.value, 42)
        self.assertEqual(result["roots"], [])
        self.assertEqual(result["sprites"], [])
        self.assertFalse(result["spawned_for_audit"])

    def test_pending_failed_foreign_and_nonresident_activation_is_rejected(self):
        for field, value in [("active", None), ("pending", self.bindings[0]["identity"]),
                             ("failure", {"reason": "missing"}), ("locator", "other")]:
            rows = copy.deepcopy(self.rows)
            rows[1]["detail"]["generations"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)
        for field in ("active", "descriptor_identity", "expected"):
            rows = copy.deepcopy(self.rows)
            rows[1]["detail"]["generations"][0][field] = self.bindings[0]["identity"] | {"generation": True}
            with self.subTest(field=field), self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)
        for field in ("meshes", "images"):
            rows = copy.deepcopy(self.rows)
            rows[1]["detail"]["generations"][0][field][0]["resident"] = False
            with self.subTest(field=field), self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)

    def test_wrong_pid_source_inventory_schema_or_claim_is_rejected(self):
        for field, value in [("process_id", True), ("source", "explicit_override"), ("schema_version", True),
                             ("inventory_sha256", "c"*64), ("accepted", True), ("promotion_authority", True),
                             ("performance_evidence", True)]:
            rows = copy.deepcopy(self.rows)
            rows[1][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)
        for rows in [self.rows[:1], list(reversed(self.rows)), self.rows + self.rows[1:]]:
            with self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)

    def test_existing_root_must_use_exact_released_meshes_and_albedo(self):
        root = {"entity": "root", "owner": "owner", "kind": "Tank", "owner_kind": "Tank",
            "owner_exists": True, "identity": self.bindings[0]["identity"], "expected": self.bindings[0]["identity"],
            "parts": [{"entity": name, "name": name, "parent": "root", "mesh": "Tank"+name,
                       "albedo": "Tankalbedo", "mesh_resident": True, "albedo_resident": True} for name in ("body", "water")]}
        self.rows[1]["detail"]["consumers"]["owners"] = [{"entity": "owner", "kind": "Tank"}]
        self.rows[1]["detail"]["consumers"]["roots"] = [root]
        audit.check_records(self.rows, self.value, 42)
        for field, value in [("mesh", "fallback"), ("albedo", "foreign"), ("parent", "other"),
                             ("mesh_resident", False), ("name", "water")]:
            rows = copy.deepcopy(self.rows)
            rows[1]["detail"]["consumers"]["roots"][0]["parts"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                audit.check_records(rows, self.value, 42)
        rows = copy.deepcopy(self.rows)
        rows[1]["detail"]["consumers"]["roots"].append(copy.deepcopy(root))
        with self.assertRaises(ValueError):
            audit.check_records(rows, self.value, 42)

    def test_existing_owners_with_missing_root_or_sprite_are_not_absent_kinds(self):
        self.rows[1]["detail"]["consumers"]["owners"] = [{"entity": "owner", "kind": "Tank"}]
        with self.assertRaisesRegex(ValueError, "missing/duplicate presentation"):
            audit.check_records(self.rows, self.value, 42)
        self.prepare(("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"))
        self.rows[1]["detail"]["consumers"]["owners"] = [{"entity": "sand", "kind": "SandPile", "sprite_targets": [{"entity": "sand", "role": "owner"}]}]
        with self.assertRaisesRegex(ValueError, "missing/duplicate Sprite"):
            audit.check_records(self.rows, self.value, 42)
        self.rows[1]["detail"]["consumers"]["sprites"] = [{"entity": "sand", "owner": "sand", "target_role": "owner", "parent": None, "layer_kind": None,
            "kind": "SandPile", "image": "SandPileworld", "matches_released_world_image": True, "resident": True}]
        audit.check_records(self.rows, self.value, 42)

    def test_2d_owner_and_direct_layer_children_match_the_actual_publisher(self):
        self.prepare(("Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"))
        for roles in [("owner",), ("child",), ("owner", "child")]:
            rows = copy.deepcopy(self.rows)
            targets = [{"entity": "sand" if role == "owner" else "layer", "role": role} for role in roles]
            sprites = [{"entity": target["entity"], "owner": "sand", "kind": "SandPile", "target_role": target["role"],
                "parent": None if target["role"] == "owner" else "sand",
                "layer_kind": None if target["role"] == "owner" else "Struct", "image": "SandPileworld",
                "matches_released_world_image": True, "resident": True} for target in targets]
            rows[1]["detail"]["consumers"]["owners"] = [{"entity": "sand", "kind": "SandPile", "sprite_targets": targets}]
            rows[1]["detail"]["consumers"]["sprites"] = sprites
            with self.subTest(roles=roles):
                audit.check_records(rows, self.value, 42)
            for change in ("missing", "duplicate", "extra", "target0"):
                invalid = copy.deepcopy(rows)
                consumers = invalid[1]["detail"]["consumers"]
                if change == "missing":
                    consumers["sprites"].pop()
                elif change == "duplicate":
                    consumers["sprites"].append(copy.deepcopy(consumers["sprites"][0]))
                elif change == "extra":
                    consumers["sprites"].append(consumers["sprites"][0] | {"entity": "stray"})
                else:
                    consumers["owners"][0]["sprite_targets"] = []
                    consumers["sprites"] = []
                with self.subTest(roles=roles, change=change), self.assertRaises(ValueError):
                    audit.check_records(invalid, self.value, 42)
            if "child" in roles:
                for field, value in [("parent", "foreign"), ("layer_kind", None), ("target_role", "owner")]:
                    invalid = copy.deepcopy(rows)
                    child = next(row for row in invalid[1]["detail"]["consumers"]["sprites"] if row["entity"] == "layer")
                    child[field] = value
                    with self.subTest(roles=roles, field=field), self.assertRaises(ValueError):
                        audit.check_records(invalid, self.value, 42)

    def test_plan_change_around_admission_creates_no_root_build_or_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            path, root = repo / "plan.json", repo / "owned-job"
            original = {"repo": str(repo), "job_root": str(root), "plan_path": str(path)}
            args = argparse.Namespace(plan=path, frozen_plan=original, repo=str(repo), job_root=str(root))
            path.write_bytes(audit.pipeline.canonical(original | {"job_root": str(repo / "foreign-job")}))
            with patch.dict(audit.os.environ, {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
                 patch.object(audit.native, "run_command") as build, patch.object(audit.subprocess, "Popen") as launch:
                with self.assertRaisesRegex(ValueError, "plan changed"):
                    audit.run_frozen(args)
                build.assert_not_called()
                launch.assert_not_called()
            self.assertFalse(root.exists())
            self.assertFalse((repo / "foreign-job").exists())

    def test_plan_change_during_build_cannot_copy_binary_or_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            path, root = repo / "plan.json", repo / "owned-job"
            original = {"repo": str(repo), "job_root": str(root), "plan_path": str(path)}
            path.write_bytes(audit.pipeline.canonical(original))
            args = argparse.Namespace(plan=path, frozen_plan=original, repo=str(repo), job_root=str(root))
            def changed(*args, **kwargs):
                path.write_bytes(audit.pipeline.canonical(original | {"job_root": str(repo / "foreign-job")}))
            with patch.dict(audit.os.environ, {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
                 patch.object(audit, "check"), patch.object(audit.native, "cargo_environment", return_value={}), \
                 patch.object(audit.native, "run_command", side_effect=changed), \
                 patch.object(audit.executable, "freeze") as copy_binary, patch.object(audit.subprocess, "Popen") as launch:
                with self.assertRaisesRegex(ValueError, "plan changed"):
                    audit.run_frozen(args)
                copy_binary.assert_not_called()
                launch.assert_not_called()
            self.assertFalse((repo / "foreign-job").exists())
            self.assertEqual(json.loads((root / "job.json").read_text())["status"], "invalid")

    def test_normal_environment_removes_all_dedicated_overrides_but_keeps_activity_ownership(self):
        hostile = {prefix+"TEST": "1" for prefix in audit.EXCLUDED_ENV}
        hostile |= {"HW_BUILDING_ASSET_RELEASES": "candidate", "HW_PERF_SCENARIO": "1",
                    "HW_NATIVE_ACTIVITY_FD": "42", "BEVY_ASSET_ROOT": "wrong"}
        actual = audit.environment(hostile, Path("/private/view"), "Intel")
        self.assertFalse(any(key.startswith(audit.EXCLUDED_ENV) for key in actual))
        self.assertEqual(actual["HW_NATIVE_ACTIVITY_FD"], "42")
        self.assertEqual(actual["BEVY_ASSET_ROOT"], "/private/view")
        self.assertEqual(actual["WGPU_BACKEND"], "vulkan")

    def test_only_marked_json_records_are_consumed(self):
        log = "unrelated line\n" + "\n".join("timestamp INFO " + audit.MARKER + json.dumps(row) for row in self.rows)
        self.assertEqual(audit.records(log), self.rows)
        with self.assertRaises(ValueError):
            audit.records(audit.MARKER + "not json")

    def test_empty_compiled_inventory_cannot_plan_a_normal_release_probe(self):
        value = {"profile": audit.PROFILE, "status": "ready", "accepted": False, "performance_evidence": False,
                 "promotion_authority": False, "drivers": {}, "numeric_verifiers": {}, "repo": "/repo",
                 "codec": "/codec", "subject": {}, "codec_sha256": "a", "inventory_sha256": "a", "bindings": []}
        with patch.object(audit, "dependencies", return_value={}), \
             patch.object(audit.pipeline, "numeric_verifier_inventory", return_value={}), \
             patch.object(audit.art, "validate_subject"), patch.object(audit.art, "subject", return_value={}), \
             patch.object(audit.executable, "digest", return_value="a"), \
             patch.object(audit.pipeline, "read", return_value=[]), \
             patch.object(audit.pipeline, "release_bindings") as codec:
            with self.assertRaises(ValueError):
                audit.check(value)
            codec.assert_not_called()
