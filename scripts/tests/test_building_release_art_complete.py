"""Whole released gallery wire/projection tests, not native or art approval."""

import copy
from pathlib import Path
import tempfile
import unittest

from scripts import building_release_art_observation as oracle
from scripts.tests.released_gallery_fixture import full_fixture


class CompleteReleasedGalleryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.values = full_fixture(self.root)

    def verify(self, values):
        oracle.validate(*values, self.root)

    def test_complete_released_36_owners_and_context_reach_actual_oracle(self):
        self.assertEqual(len(self.values[0]["fixture"]["records"]), 36)
        self.assertEqual(len(self.values[0]["snapshot"]["records"]), 36)
        self.verify(self.values)

    def test_runtime_envelope_must_match_exact_process_session_and_clock(self):
        for index, key, value in (
            (0, "pid", True),
            (1, "pid", 124),
            (1, "nonce", "b" * 32),
            (0, "stable_frames", 59),
            (1, "completed_at_unix_ms", 999),
            (1, "accepted", True),
            (0, "promotion_authority", True),
        ):
            with self.subTest(key=key, index=index):
                values = copy.deepcopy(self.values)
                values[index][key] = value
                with self.assertRaises(ValueError):
                    self.verify(values)

    def test_candidate_missing_identity_or_changed_door_cannot_enter_release_oracle(
        self,
    ):
        for mutation in (
            lambda v: v[0]["fixture"]["m6"]["identities"][0].update(
                authority="isolated_candidate"
            ),
            lambda v: v[0]["fixture"]["m6"]["identities"].pop(),
            lambda v: v[0]["fixture"]["m6"]["door_identity"].update(
                asset_set_generation=8
            ),
            lambda v: v[0]["fixture"]["records"].pop(),
        ):
            values = copy.deepcopy(self.values)
            mutation(values)
            # A consistent false final cannot legitimize a false initial fixture.
            values[1]["fixture"] = copy.deepcopy(values[0]["fixture"])
            with self.assertRaises(ValueError):
                self.verify(values)

    def test_actual_snapshot_drift_missing_owner_or_forged_projection_is_rejected(self):
        for mutation in (
            lambda v: v[1]["snapshot"]["records"].pop(),
            lambda v: v[0]["snapshot"]["records"].pop(),
            lambda v: v[0]["snapshot"]["records"][0]["shapes"][0]["physical_corners"][
                0
            ].__setitem__(0, -10.0),
            lambda v: v[0]["snapshot"]["records"][0].update(owner=99999),
            lambda v: v[0]["snapshot"]["context"]["structures"][0]["visible_meshes"][
                0
            ].update(entity=400),
        ):
            values = copy.deepcopy(self.values)
            mutation(values)
            # When testing both-side forgery, keep exact snapshots consistent.
            if len(values[1]["snapshot"]["records"]) == 36:
                values[1]["snapshot"] = copy.deepcopy(values[0]["snapshot"])
            with self.assertRaises(ValueError):
                self.verify(values)

    def test_visible_target_cannot_borrow_non_target_group_culling(self):
        values = copy.deepcopy(self.values)
        row = next(
            r for r in values[0]["fixture"]["records"] if r["kind"] == "MudMixer"
        )
        row["parts"][0]["visible"] = False
        row["gallery_geometry"][0]["visible"] = False
        values[1]["fixture"] = copy.deepcopy(values[0]["fixture"])
        with self.assertRaises(ValueError):
            self.verify(values)


class CompleteReleasedHostTests(unittest.TestCase):
    """Actual validate/projection oracle and actual transport verifier together."""

    def setUp(self):
        from scripts import building_release_art_acceptance as host
        from PIL import Image

        self.host = host
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.case = ("high", 1.0, "normal")
        self.path = self.root / "cases" / host.label(self.case)
        self.path.mkdir(parents=True)
        self.probe, self.final, _, self.value = full_fixture(self.root / "asset-view")
        self.value.update(job_root=str(self.root), adapter="Intel")
        self.probe["session"] = host.session(
            self.value, self.case, self.path, self.probe["nonce"]
        )
        self.sha = "b" * 64
        self.png = self.path / "actual-window.png"
        Image.new("RGBA", (1920, 1680), "white").save(self.png)
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
            "nonce": self.probe["nonce"],
            "pid": 123,
            "probe_sha256": self.write("probe.json", self.probe),
            "png_sha256": self.shot["sha256"],
            "window_id": "0x123",
            "width": 1920,
            "height": 1680,
            "captured_at_unix_ms": 1100,
        }
        self.final.update(
            probe_sha256=self.ack["probe_sha256"],
            ack_sha256=self.write("capture-ack.json", self.ack),
            ack=copy.deepcopy(self.ack),
        )
        self.receipt = {
            "nonce": self.probe["nonce"],
            "status": "pass",
            "exit_code": 0,
            "pid": 123,
            "case": list(self.case),
            "binary_sha256": self.sha,
            "screenshot": copy.deepcopy(self.shot),
        }
        self.write("final.json", self.final)
        self.write("case.json", self.receipt)
        (self.path / "actual-window.log").write_text(
            'AdapterInfo { name: "Intel GPU", backend: Vulkan }\n'
        )

    def write(self, name, value):
        payload = self.host.pipeline.canonical(value)
        (self.path / name).write_bytes(payload)
        return self.host.pipeline.digest(payload)

    def verify(self):
        return self.host.verify_case(self.value, self.case, self.sha)

    def test_actual_full_oracle_and_transport_agree_on_eight_sets_plus_door(self):
        self.assertEqual(self.verify()["case"], list(self.case))

    def test_recomputed_receipt_hashes_do_not_hide_false_fixture_or_snapshot(self):
        for mutation in (
            lambda v: v["fixture"]["m6"]["identities"][0].update(
                authority="isolated_candidate"
            ),
            lambda v: v["snapshot"]["records"].pop(),
            lambda v: v["snapshot"]["records"][0].update(owner=999999),
        ):
            probe = copy.deepcopy(self.probe)
            mutation(probe)
            final = copy.deepcopy(self.final)
            final["fixture"] = copy.deepcopy(probe["fixture"])
            final["snapshot"] = copy.deepcopy(probe["snapshot"])
            ack = copy.deepcopy(self.ack)
            ack["probe_sha256"] = self.write("probe.json", probe)
            final.update(
                probe_sha256=ack["probe_sha256"],
                ack=copy.deepcopy(ack),
                ack_sha256=self.write("capture-ack.json", ack),
            )
            self.write("final.json", final)
            with self.assertRaises(ValueError):
                self.verify()


if __name__ == "__main__":
    unittest.main()
