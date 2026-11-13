"""Synthetic transport/authority guards; never renderer or art evidence.

The per-case boundary uses a stubbed image oracle deliberately: independent
fixture/projection tests own that boundary. These tests exercise real on-disk
probe/ACK/PNG/process/log binding rather than infer it from mock return values.
"""

import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image
from scripts import building_release_art_acceptance as host


class ReleasedWallInventoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        for kind, generation, schema, roles in [
            ("wall", 13, 2, host.joint.wall_art.FORMWORK_PREVIEW_ROLES),
            (
                "door",
                7,
                1,
                (
                    "mesh:closed",
                    "mesh:open",
                    "mesh:locked",
                    "texture:albedo",
                    "preview:ew",
                    "preview:ns",
                ),
            ),
        ]:
            identity = {
                "asset_set_id": kind + "-production-v1",
                "asset_set_generation": generation,
                "manifest_sha256": ("a" if kind == "wall" else "b") * 64,
            }
            records = []
            for index, role in enumerate(roles):
                rel = f"{kind}_sets/{generation}/core/{index}.bin"
                payload = (kind + role).encode()
                self.put_asset(rel, payload)
                records.append(
                    {
                        "role": role,
                        "path": rel,
                        "bytes": len(payload),
                        "sha256": host.pipeline.digest(payload),
                    }
                )
            receipt_rel = f"{kind}_sets/{generation}/authority/promotion-receipt.json"
            receipt = host.joint.canonical_bytes(
                {"schema_version": 1, **identity, "new_active": identity}
            )
            self.put_asset(receipt_rel, receipt)
            manifest = {
                "schema_version": schema,
                **identity,
                "authority": "release_approved",
                "review_status": "art_approved",
                "core": records,
                "receipt": {
                    "path": receipt_rel,
                    "bytes": len(receipt),
                    "sha256": host.pipeline.digest(receipt),
                },
            }
            if kind == "wall":
                manifest.update(normal_decision="rejected", candidate_normal=None)
            self.put_asset(
                f"manifests/{kind}-production-v1.{kind}set",
                host.joint.canonical_bytes(manifest),
            )

    def put_asset(self, rel, payload):
        path = self.repo / "assets" / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)

    def test_normal_wall_v2_and_door_v1_reach_full_release_validator(self):
        self.assertEqual(
            host.wall_inventory(self.repo),
            {
                "asset_set_generation": 13,
                "authority": "release_approved",
                "manifest_sha256": "a" * 64,
            },
        )

    def test_wrong_wall_schema_authority_or_normal_contract_is_rejected(self):
        locator = self.repo / "assets/manifests/wall-production-v1.wallset"
        original = host.read_artifact(locator)
        for key, value in [
            ("schema_version", 1),
            ("authority", "isolated_candidate"),
            ("review_status", "pending"),
            ("asset_set_generation", True),
            ("normal_decision", "approved"),
        ]:
            changed = copy.deepcopy(original)
            changed[key] = value
            locator.write_bytes(host.joint.canonical_bytes(changed))
            with self.subTest(key=key), self.assertRaises(host.native.AcceptanceError):
                host.wall_inventory(self.repo)
        locator.write_bytes(host.joint.canonical_bytes(original))

    def test_full_validator_rejects_changed_core_and_receipt_bytes(self):
        locator = self.repo / "assets/manifests/wall-production-v1.wallset"
        manifest = host.read_artifact(locator)
        for record in [manifest["core"][0], manifest["receipt"]]:
            path = self.repo / "assets" / record["path"]
            original = path.read_bytes()
            path.write_bytes(b"changed")
            with (
                self.subTest(path=record["path"]),
                self.assertRaises(host.native.AcceptanceError),
            ):
                host.wall_inventory(self.repo)
            path.write_bytes(original)


class ReleasedTransportTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.case = ("high", 1.0, "normal")
        self.path = self.root / "cases" / host.label(self.case)
        self.path.mkdir(parents=True)
        self.value = {
            "job_root": str(self.root),
            "adapter": "Intel",
            "assets": {"bindings": []},
        }
        self.nonce = "a" * 32
        self.binary_sha = "b" * 64
        self.probe = {"pid": 123, "ready_at_unix_ms": 1000}
        self.png = self.path / "actual-window.png"
        Image.new("RGBA", (host.oracle.WIDTH, host.oracle.HEIGHT), "white").save(
            self.png
        )
        self.shot = {
            "path": "actual-window.png",
            "sha256": host.pipeline.digest(self.png.read_bytes()),
            "window_id": "0x123",
            "window_pid": 123,
            "capture_scope": "owned-x11-client",
            "captured_at_unix_ms": 1100,
        }
        self.ack = {
            "schema_version": 1,
            "nonce": self.nonce,
            "pid": 123,
            "probe_sha256": self.write("probe.json", self.probe),
            "png_sha256": self.shot["sha256"],
            "window_id": "0x123",
            "width": host.oracle.WIDTH,
            "height": host.oracle.HEIGHT,
            "captured_at_unix_ms": 1100,
        }
        self.final = {
            "pid": 123,
            "probe_sha256": self.ack["probe_sha256"],
            "ack_sha256": self.write("capture-ack.json", self.ack),
            "ack": copy.deepcopy(self.ack),
            "completed_at_unix_ms": 1200,
        }
        self.receipt = {
            "nonce": self.nonce,
            "status": "pass",
            "exit_code": 0,
            "pid": 123,
            "case": list(self.case),
            "binary_sha256": self.binary_sha,
            "screenshot": copy.deepcopy(self.shot),
        }
        self.write("final.json", self.final)
        self.write("case.json", self.receipt)
        self.log = self.path / "actual-window.log"
        self.log.write_text('AdapterInfo { name: "Intel GPU", backend: Vulkan }\n')
        self.addCleanup(patch.stopall)
        self.oracle = patch.object(host.oracle, "validate").start()

    def write(self, name, value):
        data = host.pipeline.canonical(value)
        (self.path / name).write_bytes(data)
        return host.pipeline.digest(data)

    def verify(self):
        return host.verify_case(self.value, self.case, self.binary_sha)

    def test_valid_transport_calls_full_image_oracle_with_exact_artifacts(self):
        self.assertEqual(self.verify()["case"], list(self.case))
        self.oracle.assert_called_once_with(
            self.probe,
            self.final,
            host.session(self.value, self.case, self.path, self.nonce),
            self.value,
            self.root / "asset-view",
        )

    def test_foreign_process_nonce_case_binary_or_exit_is_rejected(self):
        for key, changed in [
            ("pid", 124),
            ("pid", True),
            ("nonce", "z" * 32),
            ("case", ["high", 1.0, "far"]),
            ("case", ["high", True, "normal"]),
            ("binary_sha256", "c" * 64),
            ("exit_code", -15),
            ("exit_code", False),
        ]:
            with self.subTest(key=key, changed=changed):
                row = copy.deepcopy(self.receipt)
                row[key] = changed
                self.write("case.json", row)
                with self.assertRaises(ValueError):
                    self.verify()
        self.write("case.json", self.receipt)

    def test_changed_ack_or_probe_cannot_be_reinterpreted_as_verified(self):
        for name, original, key, changed in (
            ("probe.json", self.probe, "pid", 124),
            ("capture-ack.json", self.ack, "nonce", "c" * 32),
            ("capture-ack.json", self.ack, "pid", 124),
            ("capture-ack.json", self.ack, "width", True),
            ("capture-ack.json", self.ack, "window_id", "0x0"),
            ("capture-ack.json", self.ack, "png_sha256", "d" * 64),
        ):
            with self.subTest(name=name, key=key):
                row = copy.deepcopy(original)
                row[key] = changed
                self.write(name, row)
                with self.assertRaises(ValueError):
                    self.verify()
                self.write(name, original)

    def test_final_ack_identity_and_capture_time_are_checked(self):
        for mutate in (
            lambda v: v.update(ack_sha256="e" * 64),
            lambda v: v["ack"].update(pid=124),
            lambda v: v.update(completed_at_unix_ms=1099),
            lambda v: v.update(completed_at_unix_ms=31101),
        ):
            row = copy.deepcopy(self.final)
            mutate(row)
            self.write("final.json", row)
            with self.assertRaises(ValueError):
                self.verify()
        self.write("final.json", self.final)

    def test_changed_png_symlink_and_wrong_owner_are_rejected(self):
        original = self.png.read_bytes()
        self.png.write_bytes(original + b"changed")
        with self.assertRaises(ValueError):
            self.verify()
        self.png.write_bytes(original)
        target = self.path / "other.png"
        self.png.rename(target)
        self.png.symlink_to(target)
        with self.assertRaises(ValueError):
            self.verify()
        self.png.unlink()
        target.rename(self.png)
        row = copy.deepcopy(self.receipt)
        row["screenshot"]["window_pid"] = 124
        self.write("case.json", row)
        with self.assertRaises(ValueError):
            self.verify()

    def test_wrong_renderer_warning_or_duplicate_adapter_is_rejected(self):
        for text in (
            'AdapterInfo { name: "Intel GPU", backend: Gl }',
            'AdapterInfo { name: "Other GPU", backend: Vulkan }',
            'AdapterInfo { name: "Intel GPU", backend: Vulkan }\nWARN bad',
            'AdapterInfo { name: "Intel GPU", backend: Vulkan }\n' * 2,
        ):
            with self.subTest(text=text):
                self.log.write_text(text)
                with self.assertRaises(ValueError):
                    self.verify()

    def test_json_duplicate_nonfinite_oversized_and_symlink_rejected(self):
        path = self.path / "bad.json"
        for data in ('{"a":1,"a":2}', '{"a":NaN}', "[]"):
            path.write_text(data)
            with self.assertRaises(ValueError):
                host.read_artifact(path)
        with path.open("wb") as stream:
            stream.truncate(8 * 1024 * 1024 + 1)
        with self.assertRaises(ValueError):
            host.read_artifact(path)
        path.unlink()
        path.symlink_to(self.path / "probe.json")
        with self.assertRaises(ValueError):
            host.read_artifact(path)


if __name__ == "__main__":
    unittest.main()
