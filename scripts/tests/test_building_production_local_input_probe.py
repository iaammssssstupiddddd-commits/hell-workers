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

from scripts import building_production_local_input_probe as input_probe


class LocalInputProbeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.repo = Path(temporary.name).resolve()
        (self.repo / "target/native-acceptance").mkdir(parents=True)
        (self.repo / "target/profiling").mkdir(parents=True)
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

    def test_rejects_authority_source_binary_transport_and_foreign_root(self):
        self.plan_path.write_text("{}")
        for key, replacement in (
            ("accepted", True), ("performance_evidence", True),
            ("art_approved", True), ("promotion_authority", True),
            ("head", "d" * 40), ("source", "d" * 64),
            ("driver", "d" * 64), ("input_transport_sha256", "d" * 64),
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

    def test_startup_waits_for_three_samples_without_weakening_raw_validation(self):
        raw_path = self.repo / "raw.json"
        for count, ready in ((1, False), (2, False), (3, True)):
            raw_path.write_text(json.dumps({"samples": [{}] * count}))
            self.assertIs(input_probe.raw_ready(raw_path), ready)
        for invalid in ({}, {"samples": None}, {"samples": [{}] * 1201}):
            raw_path.write_text(json.dumps(invalid))
            with self.assertRaises(RuntimeError):
                input_probe.raw_ready(raw_path)

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

            def __init__(self, window, owner, root, nonce, record):
                self.record = record
                transport_instances.append(self)

            def activate(self):
                pass

            def send(self, step, nonce, **action):
                self.record({"step": step, "nonce": nonce, "window": 123,
                             "pid": 2345, **action})

            def close(self):
                self.closed = True

        game = Game()
        transport = Transport
        transport_instances = []

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
             patch.object(input_probe.native, "x11_client_windows_for_process_tree", return_value=[(123, 2345)]), \
             patch.object(input_probe.probe, "process_metrics", return_value={"rss_kib": 1,
                                                                             "process_fd_count": 1}), \
             patch.object(input_probe, "raw_snapshot", return_value={"samples": [{}, {}, {}], "events": []}), \
             patch.object(input_probe.native, "capture_client_png", return_value=True), \
             patch.object(input_probe, "X11Input", transport), \
             patch.object(input_probe, "wait_for_raw_event", side_effect=RuntimeError("raw timeout")), \
             patch.object(input_probe.native, "stop_command_process", side_effect=stop_game):
            with self.assertRaisesRegex(RuntimeError, "raw timeout"):
                input_probe.run.__wrapped__(args)
        self.assertTrue(game.stopped)
        self.assertEqual(len(transport_instances), 1)
        self.assertTrue(transport_instances[0].closed)
        failure = json.loads((self.root / "failure.json").read_text())
        self.assertEqual((failure["status"], failure["stage"], failure["reason"]),
                         ("invalid", "F12 press witness", "raw timeout"))
        self.assertFalse(failure["accepted"] or failure["promotion_authority"])


if __name__ == "__main__":
    unittest.main()
