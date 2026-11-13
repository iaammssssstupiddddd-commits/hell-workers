"""Synthetic whole Door input oracle plus on-disk transport, never native proof."""

import copy
from pathlib import Path
import tempfile
import unittest

from PIL import Image
from scripts import door_release_ui_acceptance as host
from scripts.tests.test_door_release_ui_observation import fixture, IDENTITY


class DoorHostTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.path = self.root / "case-0"
        self.path.mkdir()
        raw, actions, inputs, client, entries, case, _ = fixture()
        self.case = case
        self.value = {
            "job_root": str(self.root),
            "normal_inventory": {"door": copy.deepcopy(IDENTITY)},
        }
        self.sha = "b" * 64
        self.row = {
            "case": case,
            "directory": "case-0",
            "pid": 123,
            "nonce": "a" * 32,
            "binary_sha256": self.sha,
            "controlled_stop": True,
            "exit_code": -15,
        }
        self.arm = {
            "schema_version": 1,
            "nonce": "a" * 32,
            "pid": 123,
            "world_epoch": 0,
            "observed_frame": 86,
        }
        client.update(
            input_backend="portal-eis",
            input_transport="portal-eis-absolute-v1",
            geometry=[1920, 1080],
            observer_viewport=[1920, 1080],
            portal_devices=3,
            portal_session="/synthetic-session",
            monitor_mapping=[1, 1.0],
            eis_scroll=False,
            eis_region={
                "scale": 1.0,
                "root_size": [1920, 1080],
                "rect": [0.0, 0.0, 1920.0, 1080.0],
            },
        )
        self.raw = raw
        self.client = client
        self.entries = entries
        self.write("input-arm.json", self.arm)
        self.write("door-input.json", raw)
        self.write("actions.json", {"actions": actions})
        self.write("events.json", {"events": inputs})
        self.write("input-client.json", client)
        for entry in entries:
            path = self.path / (entry["case"] + ".png")
            Image.new("RGBA", (1920, 1080), "white").save(path)
            entry.update(
                screenshot=path.name,
                sha256=host.pipeline.digest(path.read_bytes()),
                capture_scope="x11-client-window",
                window=291,
                pid=123,
            )
        self.write("checkpoints.json", {"checkpoints": entries})
        (self.path / "game.log").write_text(
            'AdapterInfo { name: "Intel GPU", backend: Vulkan }\n'
        )

    def write(self, name, value):
        (self.path / name).write_bytes(host.pipeline.canonical(value))

    def verify(self):
        return host.verify_case(self.value, self.row, self.sha)

    def test_exact_synthetic_input_and_whole_oracle_are_bound_to_owned_pngs(self):
        self.assertEqual(self.verify()["case"], self.case)

    def test_arm_cannot_be_foreign_bool_premature_or_unbounded(self):
        for key, value in [
            ("pid", 124),
            ("pid", True),
            ("nonce", "b" * 32),
            ("world_epoch", False),
            ("observed_frame", 45),
            ("observed_frame", 91),
        ]:
            arm = copy.deepcopy(self.arm)
            arm[key] = value
            self.write("input-arm.json", arm)
            with self.assertRaises((ValueError, host.native.AcceptanceError)):
                self.verify()
        self.write("input-arm.json", self.arm)

    def test_unknown_child_exit_binary_and_bool_owner_are_rejected(self):
        for key, value in [
            ("exit_code", 0),
            ("controlled_stop", False),
            ("pid", True),
            ("binary_sha256", "c" * 64),
            ("nonce", "other"),
        ]:
            row = copy.deepcopy(self.row)
            row[key] = value
            with self.assertRaises((ValueError, host.native.AcceptanceError)):
                host.verify_case(self.value, row, self.sha)

    def test_wrong_portal_device_region_backend_or_owner_is_rejected(self):
        for key, value in [
            ("pid", 124),
            ("window", True),
            ("portal_devices", 1),
            ("portal_devices", True),
            ("input_backend", "xtest"),
            ("input_transport", "portal-notify-relative-v1"),
            ("geometry", [1280, 720]),
            (
                "eis_region",
                {
                    "scale": 1.0,
                    "root_size": [1920, 1080],
                    "rect": [1.0, 0.0, 1920.0, 1080.0],
                },
            ),
        ]:
            client = copy.deepcopy(self.client)
            client[key] = value
            self.write("input-client.json", client)
            with self.assertRaises((ValueError, host.native.AcceptanceError)):
                self.verify()
        self.write("input-client.json", self.client)

    def test_png_mutation_symlink_or_wrong_capture_owner_is_rejected(self):
        path = self.path / "ghost.png"
        old = path.read_bytes()
        path.write_bytes(old + b"changed")
        with self.assertRaises((ValueError, host.native.AcceptanceError)):
            self.verify()
        path.write_bytes(old)
        other = self.path / "other.png"
        path.rename(other)
        path.symlink_to(other)
        with self.assertRaises((ValueError, host.native.AcceptanceError)):
            self.verify()
        path.unlink()
        other.rename(path)
        entries = copy.deepcopy(self.entries)
        entries[2]["pid"] = 124
        self.write("checkpoints.json", {"checkpoints": entries})
        with self.assertRaises((ValueError, host.native.AcceptanceError)):
            self.verify()

    def test_renderer_warning_and_foreign_adapter_are_rejected(self):
        for text in (
            'AdapterInfo { name: "Other GPU", backend: Vulkan }',
            'AdapterInfo { name: "Intel GPU", backend: Gl }',
            'AdapterInfo { name: "Intel GPU", backend: Vulkan }\nWARN bad',
        ):
            (self.path / "game.log").write_text(text)
            with self.assertRaises((ValueError, host.native.AcceptanceError)):
                self.verify()


class DoorIndependentBatchGroupTests(unittest.TestCase):
    """Group boundary only; full per-case disk/raw oracle is tested above."""

    def setUp(self):
        from unittest.mock import patch

        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.roots = [Path(self.temp.name) / f"world-{i}" for i in range(3)]
        self.manifests = {}
        for i, (root, case) in enumerate(
            zip(self.roots, host.oracle.CASES, strict=True)
        ):
            root.mkdir()
            (root / "manifest.json").write_bytes(b"synthetic group boundary fixture\n")
            self.manifests[root] = {
                "case": case,
                "plan": {
                    "identity": {"source": "a" * 64},
                    "normal_inventory": {"door": IDENTITY},
                    "asset_fingerprint": "b" * 64,
                    "drivers": {"driver": "c" * 64},
                },
                "binary": {"sha256": "d" * 64},
                "portal_session": f"/independent/{i}",
                "sessions": [{"pid": 100 + i, "nonce": f"{i:032x}"}],
                "observations": [case],
            }
        self.verifier = patch.object(
            host, "verify", side_effect=lambda root: self.manifests[root]
        ).start()
        self.addCleanup(patch.stopall)

    def test_group_recomputes_every_case_and_reports_three_independent_worlds(self):
        result = host.verify_group(self.roots)
        self.assertEqual(self.verifier.call_count, 3)
        self.assertEqual([c["case"] for c in result["cases"]], list(host.oracle.CASES))
        self.assertEqual(result["world_scope"], "three-independent-native-worlds")
        self.assertIs(result["accepted"], False)
        self.assertIs(result["promotion_authority"], False)

    def test_group_rejects_duplicate_missing_or_reordered_cases(self):
        for roots in [self.roots[:2], [self.roots[0]] * 3, list(reversed(self.roots))]:
            with self.subTest(roots=roots), self.assertRaises(ValueError):
                host.verify_group(roots)
        self.manifests[self.roots[1]]["case"] = list(host.oracle.CASES)[0]
        with self.assertRaisesRegex(ValueError, "exactly once"):
            host.verify_group(self.roots)

    def test_group_rejects_foreign_subject_inventory_driver_or_binary(self):
        for field in ["identity", "normal_inventory", "asset_fingerprint", "drivers"]:
            old = self.manifests[self.roots[1]]["plan"][field]
            self.manifests[self.roots[1]]["plan"][field] = {"foreign": True}
            with (
                self.subTest(field=field),
                self.assertRaisesRegex(ValueError, "source, normal inventory"),
            ):
                host.verify_group(self.roots)
            self.manifests[self.roots[1]]["plan"][field] = old
        self.manifests[self.roots[1]]["binary"]["sha256"] = "e" * 64
        with self.assertRaisesRegex(ValueError, "binary differs"):
            host.verify_group(self.roots)

    def test_group_rejects_reused_process_nonce_or_portal_session(self):
        for field in ["pid", "nonce"]:
            old = self.manifests[self.roots[1]]["sessions"][0][field]
            self.manifests[self.roots[1]]["sessions"][0][field] = self.manifests[
                self.roots[0]
            ]["sessions"][0][field]
            with (
                self.subTest(field=field),
                self.assertRaisesRegex(ValueError, "must be distinct"),
            ):
                host.verify_group(self.roots)
            self.manifests[self.roots[1]]["sessions"][0][field] = old
        self.manifests[self.roots[1]]["portal_session"] = self.manifests[self.roots[0]][
            "portal_session"
        ]
        with self.assertRaisesRegex(ValueError, "must be distinct"):
            host.verify_group(self.roots)

    def test_closed_case_plan_rejects_legacy_whole_matrix_and_unknown_case(self):
        for case, cases in [
            ("door-release-east-west", list(host.oracle.CASES)),
            ("arbitrary-command", ["arbitrary-command"]),
        ]:
            value = {
                "profile": host.PROFILE,
                "schema_version": 1,
                "status": "ready",
                "case": case,
                "cases": cases,
                "accepted": False,
                "promotion_authority": False,
                "performance_evidence": False,
            }
            with (
                self.subTest(case=case),
                self.assertRaisesRegex(ValueError, "scope differs"),
            ):
                host.check(value)


if __name__ == "__main__":
    unittest.main()
