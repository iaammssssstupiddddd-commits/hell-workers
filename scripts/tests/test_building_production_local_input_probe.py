"""Refusal tests for the technical-only ordinary-world input probe."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image, ImageDraw

from scripts import building_production_local_input_probe as input_probe


class LocalInputProbeTests(unittest.TestCase):
    def catalog_fixture(self, rendered):
        sample = {"dpi": 1, "menu_state": "Architect", "architect_panel_ready": True,
                  "play_mode": "Normal", "task_mode": "None", "selected_building": None,
                  "architect_cards": []}
        image = Image.new("RGB", input_probe.WINDOW, (120, 195, 75))
        draw = ImageDraw.Draw(image)
        for index, kind in enumerate(input_probe.selection_probe.KINDS):
            left, top = 10 + index % 6 * 144, 500 + index // 6 * 60
            rect = [left, top, left + 140, top + 56]
            sample["architect_cards"].append({"entity": str(index), "kind": kind,
                "physical_rect": rect, "physical_viewport": [1280, 720]})
            if index < rendered:
                draw.rectangle(rect, fill=(64, 64, 77))
        return sample, image

    def test_catalog_capture_requires_rendered_card_backgrounds(self):
        path = self.repo / "catalog.png"
        for count in (0, 3, 6, 7, 9):
            sample, image = self.catalog_fixture(count)
            image.save(path)
            if count < 7:
                with self.subTest(count=count), self.assertRaises(RuntimeError):
                    input_probe.check_rendered_catalog(path, sample)
            else:
                input_probe.check_rendered_catalog(path, sample)
        sample["architect_cards"].pop()
        with self.assertRaises(RuntimeError):
            input_probe.check_rendered_catalog(path, sample)

    def test_catalog_background_patches_reject_clipped_small_card(self):
        sample, image = self.catalog_fixture(9)
        path = self.repo / "catalog.png"
        image.save(path)
        sample["architect_cards"][0]["physical_rect"] = [10, 500, 24, 514]
        with self.assertRaises(RuntimeError):
            input_probe.check_rendered_catalog(path, sample)

    def test_catalog_capture_preserves_rejected_frames_without_input(self):
        self.root.mkdir()
        sample, rejected = self.catalog_fixture(0)
        _, accepted = self.catalog_fixture(9)
        frames = iter((rejected, accepted))

        def capture(_window, path, **_kwargs):
            next(frames).save(path)
            return True

        with patch.object(input_probe.native, "capture_client_png", side_effect=capture) as captured, \
                patch.object(input_probe, "check_capture_pixels"), patch.object(input_probe.time, "sleep"):
            input_probe.capture_catalog("0x7b", self.root, lambda: {"samples": [sample]})
            self.assertEqual(captured.call_count, 2)
        self.assertTrue((self.root / "catalog-capture-0.png").is_file())
        self.assertTrue((self.root / "catalog.png").is_file())

    def test_catalog_capture_failure_is_bounded_and_preserves_all_frames(self):
        self.root.mkdir()
        sample, rejected = self.catalog_fixture(0)

        def capture(_window, path, **_kwargs):
            rejected.save(path)
            return True

        with patch.object(input_probe.native, "capture_client_png", side_effect=capture) as captured, \
                patch.object(input_probe, "check_capture_pixels"), patch.object(input_probe.time, "sleep"), \
                self.assertRaises(RuntimeError):
            input_probe.capture_catalog("0x7b", self.root, lambda: {"samples": [sample]})
        self.assertEqual(captured.call_count, 8)
        self.assertEqual(len(list(self.root.glob("catalog-capture-*.png"))), 8)
        self.assertFalse((self.root / "catalog.png").exists())

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = Path(temporary.name).resolve()
        (self.repo / "target/native-acceptance").mkdir(parents=True)
        (self.repo / "target/profiling").mkdir(parents=True)
        consent_dir = self.repo / input_probe.CONSENT_RELATIVE.parent
        consent_dir.mkdir(parents=True, mode=0o700)
        (self.repo / input_probe.CONSENT_RELATIVE).write_text("")
        (self.repo / input_probe.CONSENT_RELATIVE).chmod(0o600)
        self.binary = self.repo / "target/profiling/bevy_app"
        self.binary.write_bytes(b"test binary")
        self.binary.chmod(0o755)
        self.root = self.repo / "target/native-acceptance/input-probe"
        self.plan_path = self.repo / "target/native-acceptance/input-probe-plan.json"
        self.value = {
            "schema": 1, "profile": input_probe.PROFILE, "repo": str(self.repo),
            "job_root": str(self.root), "head": "a" * 40, "source": "b" * 64,
            "harness": "c" * 64, "driver": input_probe.probe.digest(Path(input_probe.__file__)),
            "input_transport_sha256": input_probe.probe.digest(
                Path(input_probe.__file__).with_name("native_ui_input.py")),
            "portal_transport_sha256": input_probe.probe.digest(
                Path(input_probe.__file__).with_name("native_ui_portal.py")),
            "consent_file": str(self.repo / input_probe.CONSENT_RELATIVE),
            "binary": str(self.binary), "binary_sha256": input_probe.probe.digest(self.binary),
            "owner": input_probe.OWNER, "key": input_probe.KEY,
            "accepted": False, "performance_evidence": False,
            "art_approved": False, "promotion_authority": False,
        }
        for module, name, result in (
            (input_probe.native, "validate_repo", self.repo),
            (input_probe.native, "git_dirty_paths", []),
            (input_probe.native, "git_subject", "a" * 40),
            (input_probe.native, "source_fingerprint", "b" * 64),
            (input_probe.native, "native_harness_fingerprint", "c" * 64),
        ):
            mocked = patch.object(module, name, return_value=result)
            mocked.start()
            self.addCleanup(mocked.stop)

    def test_exact_plan_is_technical_only(self):
        self.plan_path.write_text("{}")
        self.assertEqual(input_probe.check_plan(self.value, self.plan_path),
                         (self.repo, self.root, self.binary))
        self.assertEqual(input_probe.command(self.plan_path, self.repo, self.root)[7],
                         str(self.plan_path))
        self.assertEqual(input_probe.verifier(self.root)[-1], str(self.root))

    def test_catalog_plan_is_fixed_not_an_arbitrary_key_or_command(self):
        self.plan_path.write_text("{}")
        value = self.value | {"profile": input_probe.CATALOG_PROFILE, "scenario": "architect-cancel", "key": "b"}
        self.assertEqual(input_probe.check_plan(value, self.plan_path), (self.repo, self.root, self.binary))
        for change in ({"scenario": "anything"}, {"key": "F12"}, {"profile": input_probe.PROFILE},
                       {"accepted": True}, {"command": ["arbitrary"]}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_plan(value | change, self.plan_path)

    def test_equipment_feedback_plan_is_kind_bounded_and_debug_binary_bound(self):
        self.plan_path.write_text("{}")
        binary = self.repo / "target/debug/bevy_app"
        binary.parent.mkdir()
        binary.write_bytes(b"feedback binary")
        binary.chmod(0o755)
        value = self.value | {"profile": input_probe.SELECTION_PROFILE, "scenario": "architect-select-cancel",
                              "key": "b", "selection_kind": "Tank", "binary": str(binary),
                              "binary_sha256": input_probe.probe.digest(binary),
                              "eis_transport_sha256": input_probe.probe.digest(
                                  Path(input_probe.__file__).with_name("native_ui_eis.py")),
                              "selection_driver_sha256": input_probe.probe.digest(
                                  Path(input_probe.selection_probe.__file__))}
        self.assertEqual(input_probe.check_plan(value, self.plan_path), (self.repo, self.root, binary))
        placement = value | {"profile": input_probe.PLACEMENT_PROFILE,
                             "scenario": "architect-select-place-cancel",
                             "placement_driver_sha256": input_probe.probe.digest(
                                 Path(input_probe.placement_probe.__file__))}
        self.assertEqual(input_probe.check_plan(placement, self.plan_path), (self.repo, self.root, binary))
        yard = placement | {"profile": input_probe.YARD_PROFILE, "scenario": "architect-select-place-yard-cancel",
                            "yard_driver_sha256": input_probe.probe.digest(Path(input_probe.yard_probe.__file__))}
        self.assertEqual(input_probe.check_plan(yard, self.plan_path), (self.repo, self.root, binary))
        companion = yard | {"profile": input_probe.COMPANION_PROFILE, "scenario": "architect-select-place-companion-cancel",
                            "companion_driver_sha256": input_probe.probe.digest(Path(input_probe.companion_probe.__file__))}
        self.assertEqual(input_probe.check_plan(companion, self.plan_path), (self.repo, self.root, binary))
        for change in ({"companion_driver_sha256": "0" * 64}, {"scenario": "architect-select-place-yard-cancel"},
                       {"accepted": True}, {"selection_kind": "MudMixer"}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_plan(companion | change, self.plan_path)
        for change in ({"selection_kind": "MudMixer"}, {"yard_driver_sha256": "0" * 64},
                       {"scenario": "architect-select-place-cancel"}, {"promotion_authority": True}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_plan(yard | change, self.plan_path)
        for change in ({"selection_kind": "MudMixer"}, {"selection_kind": "Bridge"},
                       {"placement_driver_sha256": "0" * 64}, {"scenario": "architect-select-cancel"},
                       {"accepted": True}, {"promotion_authority": True}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_plan(placement | change, self.plan_path)
        for change in ({"selection_kind": "Bridge"}, {"selection_driver_sha256": "0" * 64},
                       {"eis_transport_sha256": "0" * 64},
                       {"binary": str(self.binary)}, {"scenario": "architect-cancel"}, {"art_approved": True}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_plan(value | change, self.plan_path)

    def test_catalog_states_require_stable_raw_capture_and_world(self):
        raw = {"samples": [{"menu_state": menu, "play_mode": "Normal", "task_mode": "None", "world_epoch": 2,
                            "architect_panel_ready": menu == "Architect"}
                           for menu in ("Hidden", "Architect", "Architect", "Hidden", "Hidden", "Hidden")]}
        evidence = {"before": 0, "opened": 1, "opened_end": 2, "closed": 4, "closed_end": 5,
                    "close_event_count": 2, "catalog_sha256": "a" * 64}
        input_probe.check_catalog_states(raw, evidence)
        for change in ({"before": True}, {"opened_end": 1}, {"closed": 1}, {"closed_end": 4}, {"closed_end": True}, {"unknown": 0}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_catalog_states(raw, evidence | change)
        for key, value in (("menu_state", "Hidden"), ("world_epoch", 3), ("world_epoch", None),
                           ("play_mode", "BuildingPlace"), ("task_mode", "SelectBuildTarget"),
                           ("architect_panel_ready", False), ("architect_panel_ready", None)):
            changed = copy.deepcopy(raw)
            changed["samples"][2][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                input_probe.check_catalog_states(changed, evidence)

        for index in (3, 4, 5):
            for key, value in (("menu_state", "Architect"), ("architect_panel_ready", True),
                               ("architect_panel_ready", None), ("world_epoch", 3)):
                changed = copy.deepcopy(raw)
                changed["samples"][index][key] = value
                with self.subTest(index=index, key=key), self.assertRaises(RuntimeError):
                    input_probe.check_catalog_states(changed, evidence)

    def test_menu_wait_requires_two_fresh_matching_layout_samples(self):
        hidden = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
                  "selected_building": None, "architect_panel_ready": False}
        opened = hidden | {"menu_state": "Architect", "architect_panel_ready": True}
        for expected, before, after in (("Architect", hidden, opened), ("Hidden", opened, hidden)):
            snapshots = [{"samples": [before, before, after]}, {"samples": [before, before, after, after]}]
            with self.subTest(expected=expected), patch.object(input_probe, "raw_snapshot", side_effect=snapshots) as read, \
                    patch.object(input_probe.time, "sleep"):
                result = input_probe.wait_for_menu(Path("/unused"), "n", 1, expected, after=1)
                self.assertEqual(len(result["samples"]), 4)
                self.assertEqual(read.call_count, 2)

    def test_menu_wait_rejects_incomplete_preceding_mode_sample(self):
        ready = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
                 "selected_building": None, "architect_panel_ready": False}
        for change in ({"play_mode": "BuildingPlace"}, {"play_mode": "TaskDesignation"},
                       {"task_mode": "SoulSpaPlace"}, {"selected_building": "Tank"},
                       {"play_mode": None}, {"task_mode": None}):
            stale = ready | change
            snapshots = [{"samples": [stale, ready]}, {"samples": [stale, ready, ready]}]
            with self.subTest(change=change), patch.object(input_probe, "raw_snapshot", side_effect=snapshots) as read, \
                    patch.object(input_probe.time, "sleep"):
                result = input_probe.wait_for_menu(Path("/unused"), "n", 1, "Hidden", after=0)
                self.assertEqual(len(result["samples"]), 3)
                self.assertEqual(read.call_count, 2)

    def test_menu_wait_requires_explicit_cleared_selection(self):
        ready = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
                 "selected_building": None, "architect_panel_ready": False}
        missing = {key: value for key, value in ready.items() if key != "selected_building"}
        snapshots = [{"samples": [missing, ready]}, {"samples": [missing, ready, ready]}]
        with patch.object(input_probe, "raw_snapshot", side_effect=snapshots) as read, patch.object(input_probe.time, "sleep"):
            input_probe.wait_for_menu(Path("/unused"), "n", 1, "Hidden", after=0)
            self.assertEqual(read.call_count, 2)

    def test_key_witness_cannot_relabel_f12_as_architect(self):
        raw = {"events": [{"event_index": i, "sample_index": i + 1, "operation": operation, "key": "KeyB"}
                          for i, operation in enumerate(("KeyPress", "KeyRelease"))]}
        input_probe.witnessed_input(raw, before_event_count=0, key="KeyB")
        with self.assertRaises(RuntimeError):
            input_probe.witnessed_input(raw, before_event_count=0)

    def test_startup_capture_requires_fresh_normal_world_interval(self):
        row = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
               "architect_panel_ready": False, "world_epoch": 0}
        raw = {"samples": [row.copy(), row.copy()]}
        evidence = {"before": 0, "after": 1, "epoch": 0}
        input_probe.check_startup_capture(raw, evidence)
        for change in ({"before": True}, {"after": 0}, {"after": 2}, {"epoch": None}, {"epoch": True}, {"epoch": 1}):
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                input_probe.check_startup_capture(raw, evidence | change)
        for key, value in (("menu_state", "Architect"), ("task_mode", None),
                           ("architect_panel_ready", True), ("world_epoch", 1)):
            changed = copy.deepcopy(raw)
            changed["samples"][1][key] = value
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                input_probe.check_startup_capture(changed, evidence)

    def test_startup_blank_retry_preserves_rejected_capture(self):
        self.root.mkdir()
        row = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
               "architect_panel_ready": False, "world_epoch": 0}
        snapshots = [{"samples": [row.copy() for _ in range(n)]} for n in (1, 2, 2, 3)]

        def capture(_window, path, **_kwargs):
            Image.new("RGB", input_probe.WINDOW, (120, 195, 75)).save(path)
            return True

        with patch.object(input_probe, "raw_snapshot", side_effect=snapshots), \
                patch.object(input_probe.native, "capture_client_png", side_effect=capture), \
                patch.object(input_probe, "check_capture_pixels", side_effect=[ValueError("blank central viewport"), None]):
            evidence = input_probe.capture_startup("0x7b", self.root, Path("/unused"), "n", 1)
        self.assertEqual(evidence, {"before": 1, "after": 2, "epoch": 0})
        self.assertTrue((self.root / "startup-capture-0.png").is_file())
        self.assertTrue((self.root / "before.png").is_file())
        rejected = json.loads((self.root / "startup-rejected-0.json").read_text())
        self.assertEqual(rejected["reason"], "blank central viewport")
        self.assertEqual(rejected["interval"], {"before": 0, "after": 1, "epoch": 0})

    def test_actor_light_only_startup_is_not_a_rendered_world(self):
        path = self.repo / "startup.png"
        image = Image.new("RGB", input_probe.WINDOW, (25, 25, 25))
        ImageDraw.Draw(image).ellipse((500, 180, 700, 380), fill=(180, 90, 40))
        image.save(path)
        with self.assertRaisesRegex(ValueError, "majority clear"):
            input_probe.check_startup_background(path)
        Image.new("RGB", input_probe.WINDOW, (120, 195, 75)).save(path)
        input_probe.check_startup_background(path)

    def test_clear_background_retry_preserves_original_without_input(self):
        self.root.mkdir()
        row = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
               "architect_panel_ready": False, "world_epoch": 0}
        snapshots = [{"samples": [row.copy() for _ in range(n)]} for n in (1, 2, 2, 3)]
        colors = iter(((25, 25, 25), (120, 195, 75)))

        def capture(_window, path, **_kwargs):
            image = Image.new("RGB", input_probe.WINDOW, next(colors))
            ImageDraw.Draw(image).ellipse((500, 180, 700, 380), fill=(180, 90, 40))
            image.save(path)
            return True

        with patch.object(input_probe, "raw_snapshot", side_effect=snapshots), \
                patch.object(input_probe.native, "capture_client_png", side_effect=capture), \
                patch.object(input_probe, "check_capture_pixels"):
            evidence = input_probe.capture_startup("0x7b", self.root, Path("/unused"), "n", 1)
        self.assertEqual(evidence, {"before": 1, "after": 2, "epoch": 0})
        self.assertTrue((self.root / "startup-capture-0.png").is_file())
        self.assertEqual(json.loads((self.root / "startup-rejected-0.json").read_text())["reason"],
                         "startup viewport still majority clear background")

    def test_startup_capture_does_not_retry_nonblank_contract_failure(self):
        self.root.mkdir()
        row = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
               "architect_panel_ready": False, "world_epoch": 0}
        with patch.object(input_probe, "raw_snapshot", side_effect=[{"samples": [row]}, {"samples": [row, row]}]), \
                patch.object(input_probe.native, "capture_client_png", return_value=True) as capture, \
                patch.object(input_probe, "check_capture_pixels", side_effect=ValueError("actual client dimensions differ")):
            with self.assertRaisesRegex(ValueError, "dimensions differ"):
                input_probe.capture_startup("0x7b", self.root, Path("/unused"), "n", 1)
        self.assertEqual(capture.call_count, 1)

    def test_startup_blank_attempts_are_bounded_and_preserved(self):
        self.root.mkdir()
        row = {"menu_state": "Hidden", "play_mode": "Normal", "task_mode": "None",
               "architect_panel_ready": False, "world_epoch": 0}
        snapshots = [{"samples": [row.copy() for _ in range(n)]}
                     for attempt in range(16) for n in (attempt + 1, attempt + 2)]

        def capture(_window, path, **_kwargs):
            path.write_bytes(b"test PNG")
            return True

        with patch.object(input_probe, "raw_snapshot", side_effect=snapshots), \
                patch.object(input_probe.native, "capture_client_png", side_effect=capture) as captured, \
                patch.object(input_probe.time, "monotonic", return_value=0), \
                patch.object(input_probe, "check_capture_pixels", side_effect=ValueError("blank central viewport")):
            with self.assertRaisesRegex(RuntimeError, "attempt cap"):
                input_probe.capture_startup("0x7b", self.root, Path("/unused"), "n", 1)
        self.assertEqual(captured.call_count, 16)
        self.assertEqual(len(list(self.root.glob("startup-rejected-*.json"))), 16)
        self.assertFalse((self.root / "before.png").exists())

    def test_startup_timeout_does_not_capture_or_send_input(self):
        with patch.object(input_probe.time, "monotonic", side_effect=[0, 9]), \
                patch.object(input_probe.native, "capture_client_png") as capture:
            with self.assertRaisesRegex(RuntimeError, "timeout"):
                input_probe.capture_startup("0x7b", self.root, Path("/unused"), "n", 1)
        capture.assert_not_called()

    def test_rejects_authority_source_binary_transport_and_foreign_root(self):
        self.plan_path.write_text("{}")
        for key, replacement in (
            ("accepted", True), ("performance_evidence", True),
            ("art_approved", True), ("promotion_authority", True),
            ("head", "d" * 40), ("source", "d" * 64),
            ("driver", "d" * 64), ("input_transport_sha256", "d" * 64),
            ("portal_transport_sha256", "d" * 64),
            ("consent_file", str(self.repo / "other-consent.json")),
            ("binary_sha256", "d" * 64), ("key", "F11"),
            ("owner", "foreign"), ("job_root", str(self.repo)),
        ):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.value)
                changed[key] = replacement
                with self.assertRaises(RuntimeError):
                    input_probe.check_plan(changed, self.plan_path)
        self.root.mkdir()
        with self.assertRaises(RuntimeError):
            input_probe.check_plan(self.value, self.plan_path)

    def test_plan_rejection_does_not_publish_command_file(self):
        args = argparse.Namespace(repo=self.repo, job_root=self.root, output=self.plan_path)
        with patch.object(input_probe.native, "resource_snapshot", return_value={"failures": []}), \
             patch.object(input_probe.native, "save_catalog_window_capture_tool_error", return_value=None), \
             patch.object(input_probe.native, "git_dirty_paths", return_value=["dirty"]):
            with self.assertRaises(RuntimeError):
                input_probe.plan(args)
        self.assertFalse(self.plan_path.exists())

    def test_raw_witness_rejects_missing_duplicate_or_reordered_input(self):
        raw = {"events": [
            {"event_index": 0, "sample_index": 1, "operation": "PointerPress"},
            {"event_index": 1, "sample_index": 2, "operation": "KeyPress", "key": "F12"},
            {"event_index": 2, "sample_index": 3, "operation": "KeyRelease", "key": "F12"},
        ]}
        self.assertEqual(input_probe.witnessed_input(raw, before_event_count=1),
                         (raw["events"][1], raw["events"][2]))
        for boundary in (-1, 1.0, True, 4):
            with self.subTest(boundary=boundary), self.assertRaises(RuntimeError):
                input_probe.witnessed_input(raw, before_event_count=boundary)
        for rows in (raw["events"][:2], raw["events"] + [raw["events"][2]],
                     [raw["events"][0], raw["events"][2], raw["events"][1]],
                     [raw["events"][0], {**raw["events"][1], "key": "F11"}, raw["events"][2]]):
            with self.subTest(rows=rows), self.assertRaises(RuntimeError):
                input_probe.witnessed_input({"events": rows}, before_event_count=1)

    def test_portal_grant_is_exact_and_cannot_substitute_for_raw_witness(self):
        accepted = {"granted_devices": 3, "persistence": "saved",
                    "session": "/org/freedesktop/portal/desktop/session/test"}
        input_probe.check_portal_evidence(accepted)
        for key, replacement in (("granted_devices", 1), ("granted_devices", -1),
                                 ("granted_devices", True),
                                 ("persistence", "disabled"), ("session", "/foreign")):
            with self.subTest(key=key, replacement=replacement), self.assertRaises(RuntimeError):
                input_probe.check_portal_evidence({**accepted, key: replacement})
        with self.assertRaises(RuntimeError):
            input_probe.witnessed_input({"events": []}, before_event_count=0)

    def test_startup_waits_for_three_samples_without_weakening_raw_validation(self):
        raw_path = self.repo / "raw.json"
        for count, ready in ((1, False), (2, False), (3, True)):
            raw_path.write_text(json.dumps({"samples": [{}] * count}))
            self.assertIs(input_probe.raw_ready(raw_path), ready)
        for invalid in ({}, {"samples": None}, {"samples": [{}] * 1201}):
            raw_path.write_text(json.dumps(invalid))
            with self.assertRaises(RuntimeError):
                input_probe.raw_ready(raw_path)

    def test_owned_hex_window_id_is_numeric_only_at_input_boundary(self):
        self.assertEqual(input_probe.numeric_window_id("0x7b"), 123)
        for invalid in (123, "123", "0x0", "0x7g", "0x7b junk"):
            with self.subTest(invalid=invalid), self.assertRaises(RuntimeError):
                input_probe.numeric_window_id(invalid)

    def test_run_rejects_unplanned_direct_launch(self):
        args = argparse.Namespace(plan=self.plan_path, repo=self.repo, job_root=self.root)
        with patch.dict(input_probe.os.environ, {}, clear=True), self.assertRaises(RuntimeError):
            input_probe.run.__wrapped__(args)

    def test_sealing_requires_exact_independent_verifier_child(self):
        self.plan_path.write_text(json.dumps(self.value))
        context = {
            "schema_version": 1, "scope": "normal-validation-runner-ownership-only",
            "batch": "input-probe", "repo": str(self.repo), "owner": input_probe.OWNER,
            "root": str(self.root), "head": "a" * 40, "source": "b" * 64,
            "command": input_probe.command(self.plan_path, self.repo, self.root),
            "verifier": input_probe.verifier(self.root),
            "plan_sha256": input_probe.probe.digest(self.plan_path),
            "runner_pid": 9876, "runner_identity": "runner-birth",
            "coordinator_pid": os.getppid(), "coordinator_identity": "parent-birth",
            "accepted": False, "performance_evidence": False,
            "art_approved": False, "promotion_authority": False,
        }
        batch = {
            "phase": "sealing", "pid": os.getppid(), "process_identity": "parent-birth",
            "child_pid": os.getpid(), "child_identity": "current-birth", "repo": str(self.repo),
            "owner": input_probe.OWNER, "roots": [str(self.root)],
            "subject": context["head"], "subject_fingerprint": context["source"],
            "command": context["command"], "verify_command": context["verifier"],
            "policy_sha256": "policy", "coordinator_sha256": "controller",
            "command_files": [{"path": str(self.plan_path), "sha256": context["plan_sha256"]}],
            "exit_code": 0,
        }

        @contextmanager
        def locked(repo):
            self.assertEqual(repo, self.repo)
            yield {"batches": {"input-probe": batch}}

        identities = {os.getppid(): "parent-birth", os.getpid(): "current-birth"}
        with patch.object(input_probe.ownership.storage, "primary_for", return_value=self.repo), \
             patch.object(input_probe.ownership.storage, "locked", side_effect=locked), \
             patch.object(input_probe.ownership.storage, "live_identity", side_effect=identities.get), \
             patch.object(input_probe.ownership.storage, "policy_hash", return_value="policy"), \
             patch.object(input_probe.ownership.storage, "digest", return_value="controller"):
            input_probe.check_registered_context(self.repo, self.root, self.plan_path, context)
            batch["child_pid"] = 9
            batch["child_identity"] = "foreign-birth"
            with self.assertRaisesRegex(RuntimeError, "foreign or partial"):
                input_probe.check_registered_context(self.repo, self.root, self.plan_path, context)
            batch["child_pid"] = None
            batch["child_identity"] = None
            with self.assertRaisesRegex(RuntimeError, "foreign or partial"):
                input_probe.check_registered_context(self.repo, self.root, self.plan_path, context)

    def test_raw_timeout_releases_key_and_stops_owned_game(self):
        self.plan_path.write_text(json.dumps(self.value))
        args = argparse.Namespace(plan=self.plan_path, repo=self.repo, job_root=self.root)

        class Game:
            pid = 2345
            stopped = False

            def poll(self):
                return 0 if self.stopped else None

        class Transport:
            closed = False

            def __init__(self, window, owner, root, nonce, record, *, portal):
                self.record = record
                transport_instances.append(self)

            def activate(self):
                pass

            def send(self, step, nonce, **action):
                self.record({"step": step, "nonce": nonce, "window": 123,
                             "pid": 2345, **action})

            def close(self):
                self.closed = True

        class Portal:
            ready = True
            granted_devices = 3
            persistence = "saved"
            session = "/org/freedesktop/portal/desktop/session/test"
            closed = False

            def __init__(self, heartbeat, *, consent_file):
                self.assert_consent_file = consent_file
                portal_instances.append(self)

            def close(self):
                self.closed = True

        game = Game()
        transport = Transport
        transport_instances = []
        portal_instances = []

        def start_game(*args, **kwargs):
            (self.root / "raw.json").write_text(json.dumps({"samples": [{}, {}, {}]}))
            return game

        def stop_game(process):
            self.assertIs(process, game)
            game.stopped = True

        with patch.dict(input_probe.os.environ, {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
             patch.object(input_probe, "check_plan", return_value=(self.repo, self.root, self.binary)), \
             patch.object(input_probe.ownership, "running_context", return_value={}), \
             patch.object(input_probe.native, "cargo_environment", return_value={}), \
             patch.object(input_probe.subprocess, "Popen", side_effect=start_game), \
             patch.object(input_probe.native, "x11_client_windows_for_process_tree", return_value=[("0x7b", 2345)]), \
             patch.object(input_probe.probe, "process_metrics", return_value={"rss_kib": 1,
                                                                             "process_fd_count": 1}), \
             patch.object(input_probe, "raw_snapshot", return_value={"samples": [{}, {}, {}], "events": []}), \
             patch.object(input_probe, "capture_startup", return_value={"before": 0, "after": 1, "epoch": 0}), \
             patch.object(input_probe.native, "capture_client_png", return_value=True), \
             patch.object(input_probe, "PortalSession", Portal), \
             patch.object(input_probe, "PortalX11Input", transport), \
             patch.object(input_probe, "wait_for_raw_event", side_effect=RuntimeError("raw timeout")), \
             patch.object(input_probe.native, "stop_command_process", side_effect=stop_game):
            with self.assertRaisesRegex(RuntimeError, "raw timeout"):
                input_probe.run.__wrapped__(args)
        self.assertTrue(game.stopped)
        self.assertEqual(len(transport_instances), 1)
        self.assertTrue(transport_instances[0].closed)
        self.assertEqual(len(portal_instances), 1)
        self.assertTrue(portal_instances[0].closed)
        self.assertEqual(portal_instances[0].assert_consent_file, self.value["consent_file"])
        failure = json.loads((self.root / "failure.json").read_text())
        self.assertEqual((failure["status"], failure["stage"], failure["reason"]),
                         ("invalid", "F12 tap witness", "raw timeout"))
        self.assertEqual(len(failure["input_receipts"]), 2)
        self.assertFalse(failure["accepted"] or failure["promotion_authority"])


if __name__ == "__main__":
    unittest.main()
