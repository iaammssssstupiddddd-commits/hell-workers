from __future__ import annotations

import argparse
from datetime import timedelta
import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from scripts import check_agent_rules, validation_storage as storage
from scripts.perf_tool import cli, execution


class ValidationStorageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / "primary"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        (self.repo / storage.POLICY).parent.mkdir(parents=True)
        (self.repo / storage.POLICY).write_text("Keep feedback caches.\n")
        (self.repo / ".gitignore").write_text("target/\n")
        self.verifier = self.repo / "fixture_verifier.py"
        self.verifier.write_text("import json, sys\nfrom pathlib import Path\nassert sys.argv[1] == 'verify'\nassert json.loads((Path(sys.argv[2])/'manifest.json').read_text())['status']=='pass'\n")
        self.commit()
        self.env = patch.dict(os.environ, {storage.PRIMARY_ENV: str(self.repo), storage.BATCH_ENV: ""})
        self.env.start()
        self.addCleanup(self.env.stop)
        storage.initialize(self.repo)
        self.output = self.repo / "target/native-acceptance/first"

    def commit(self):
        subprocess.run(["git", "-C", str(self.repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(self.repo), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], check=True)

    def spec(self, identity="first", output=None, repo=None):
        return {"id": identity, "repo": str(repo or self.repo), "owner": "fixture",
                "consumers": ["wall-review"], "reserve_bytes": 1024**2,
                "roots": [str(output or self.output)],
                "verify_command": [sys.executable, str(self.verifier), "verify", str(output or self.output)]}

    def hold(self, path, *, kind="review", identity="candidate", **extra):
        value = {"id": identity, "path": str(path), "kind": kind, "owner": "fixture",
                 "consumers": ["wall-review"], "budget_bytes": 100 * 1024**2,
                 "review_at": (storage.now() + timedelta(days=6)).isoformat(),
                 "next_action": "apply requested wall changes",
                 "release_when": "wall review explicitly accepted or closed",
                 "review_item_id": "wall", "latest_feedback_at": storage.stamp(),
                 "expires_at": (storage.now() + timedelta(days=6)).isoformat()}
        return {**value, **extra}

    def run_batch(self, identity="first", output=None, repo=None):
        output = output or self.output
        program = "from pathlib import Path; p=Path(" + repr(str(output)) + "); p.mkdir(parents=True); (p/'manifest.json').write_text('{\"status\":\"pass\"}')"
        storage.register(self.repo, self.spec(identity, output, repo), command=[sys.executable, "-c", program])
        self.assertEqual(storage.execute(self.repo, identity), 0)

    def seal_batch(self, identity="first", output=None, result="pass"):
        output = output or self.output
        verifier = [sys.executable, str(self.verifier), "verify", str(output)]
        return storage.seal(self.repo, identity, {"result": result,
            "reason": "frozen fixture verifier passed" if result == "pass" else "cancelled before output",
            "verify_command": verifier if result == "pass" else None})

    def test_unregistered_native_and_perf_stop_before_build_or_job_creation(self):
        with patch.object(cli, "REPO_ROOT", self.repo), patch.object(cli, "_run_suite") as run:
            with self.assertRaisesRegex(RuntimeError, "unregistered execution"):
                cli.run_suite(argparse.Namespace(dry_run=False))
            run.assert_not_called()
        with patch.object(execution, "REPO_ROOT", self.repo):
            with self.assertRaisesRegex(RuntimeError, "unregistered execution"):
                execution.build_binary(argparse.Namespace())
        path = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts/native_acceptance.py"
        spec = importlib.util.spec_from_file_location("storage_test_native", path)
        native = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(native)
        invoked = []
        wrapped = native.activity_locked(lambda args: invoked.append(True))
        with self.assertRaisesRegex(RuntimeError, "unregistered execution"):
            wrapped(argparse.Namespace(repo=str(self.repo), job_root=str(self.output)))
        self.assertFalse(invoked)
        self.assertFalse(self.output.exists())

    def test_feedback_reuses_target_after_finalized_batch_and_new_subject(self):
        candidate = Path(self.temp.name) / "candidate"
        subprocess.run(["git", "-C", str(self.repo), "worktree", "add", "-q", "--detach", str(candidate)], check=True)
        cache = candidate / "target/profiling/deps/warm"
        cache.parent.mkdir(parents=True)
        cache.write_bytes(b"unchanged dependency")
        storage.retain(self.repo, self.hold(candidate))
        output = candidate / "target/native-acceptance/first"
        self.run_batch(output=output, repo=candidate)
        with self.assertRaisesRegex(RuntimeError, "requires finalization"):
            storage.check(self.repo)
        self.seal_batch(output=output)
        with self.assertRaisesRegex(RuntimeError, "job remains"):
            storage.finalize(self.repo, "first")
        shutil.rmtree(output)
        storage.finalize(self.repo, "first")
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        subprocess.run(["git", "-C", str(candidate), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "--allow-empty", "-qm", "feedback revision"], check=True)
        second = candidate / "target/native-acceptance/second"
        self.run_batch("second", second, candidate)
        self.seal_batch("second", second)
        shutil.rmtree(second)
        storage.finalize(self.repo, "second")
        self.assertEqual(cache.read_bytes(), b"unchanged dependency")
        self.assertEqual(storage.check(self.repo)["status"], "pass")

    def test_active_raw_consumer_allows_report_but_comparison_blocks_rebuild(self):
        self.run_batch()
        self.seal_batch()
        storage.retain(self.repo, self.hold(self.output, kind="comparison", identity="before-after"))
        storage.finalize(self.repo, "first")
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        with self.assertRaisesRegex(RuntimeError, "frozen dependency"):
            storage.register(self.repo, self.spec("second", self.output.with_name("second")))

    def test_closed_batch_needs_no_capsule_and_does_not_keep_old_archive_alive(self):
        self.run_batch()
        with self.assertRaisesRegex(RuntimeError, "seal the verification result"):
            storage.finalize(self.repo, "first")
        record = self.seal_batch()
        self.assertNotIn("capsule", record)
        self.assertFalse((Path(self.temp.name) / "record-first").exists())
        shutil.rmtree(self.output)
        storage.finalize(self.repo, "first")
        with storage.locked(self.repo) as ledger:
            ledger["batches"]["first"]["capsule"] = {"path": "/removed/historical/record", "files": [{"path": "/removed/manifest"}]}
            storage.save(self.repo, ledger, "simulate prior coordinator record whose archive was disposed")
        self.assertEqual(storage.check(self.repo)["status"], "pass")

    def test_failed_verifier_preserves_originals(self):
        self.run_batch()
        (self.output / "manifest.json").write_text('{"status":"invalid"}')
        with self.assertRaisesRegex(RuntimeError, "frozen verifier failed"):
            storage.seal(self.repo, "first", {
                "result": "pass", "reason": "verify failed", "verify_command": self.spec()["verify_command"]})
        self.assertTrue((self.output / "manifest.json").exists())

    def test_abandoned_before_manifest_can_close_with_reason(self):
        storage.register(self.repo, self.spec(), command=[sys.executable, "-c", "raise SystemExit(1)"])
        self.seal_batch(result="abandoned")
        storage.finalize(self.repo, "first")
        self.assertEqual(storage.check(self.repo)["status"], "pass")

    def test_shared_review_release_does_not_remove_other_consumer_or_cache(self):
        cache = self.repo / "target/profiling"
        cache.mkdir(parents=True)
        storage.retain(self.repo, self.hold(cache, consumers=["wall-review", "door-review"]))
        storage.release(self.repo, "candidate", "wall-review", "wall accepted; door still active")
        self.assertTrue(cache.exists())
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        with storage.locked(self.repo) as ledger:
            self.assertEqual(ledger["holds"]["candidate"]["consumers"], ["door-review"])

    def test_month_of_silence_keeps_feedback_builds_enabled_and_budget_is_opt_in(self):
        cache = self.repo / "target/profiling"
        cache.mkdir(parents=True)
        storage.retain(self.repo, self.hold(cache))
        with patch.object(storage, "now", return_value=storage.now() + timedelta(days=30)):
            self.assertEqual(storage.check(self.repo)["reviews_due"], ["candidate"])
            storage.register(self.repo, self.spec())
        with storage.locked(self.repo) as ledger:
            ledger["budget_bytes"] = 1
            storage.save(self.repo, ledger, "simulate budget pressure")
        with self.assertRaisesRegex(RuntimeError, "capacity reservation"):
            storage.register(self.repo, self.spec("second", self.output.with_name("second")))
        self.assertTrue(cache.exists())
        self.assertFalse(self.output.exists())

    def test_retention_requires_actual_end_condition_without_a_fixed_day_limit(self):
        raw = self.repo / "raw"
        raw.mkdir()
        origin = storage.now() - timedelta(days=8)
        value = self.hold(raw, kind="diagnostic", failure_cause="same failure", origin_at=origin.isoformat())
        value.pop("budget_bytes")
        value.pop("expires_at")
        value["review_at"] = (storage.now() + timedelta(days=45)).isoformat()
        with self.assertRaisesRegex(RuntimeError, "release_when"):
            storage.retain(self.repo, {**value, "release_when": ""})
        storage.retain(self.repo, value)
        with self.assertRaisesRegex(RuntimeError, "original failure time"):
            storage.retain(self.repo, {**value, "origin_at": storage.stamp()})

    def test_reconcile_removes_disposed_legacy_without_hiding_new_jobs(self):
        old = self.repo / "target/native-acceptance/old-job"
        old.mkdir(parents=True)
        with storage.locked(self.repo) as ledger:
            ledger["legacy"] = storage.discover(self.repo)
            storage.save(self.repo, ledger, "fixture legacy")
        with self.assertRaisesRegex(RuntimeError, "legacy data requires disposal"):
            storage.check(self.repo)
        shutil.rmtree(old)
        self.assertEqual(storage.reconcile(self.repo)["legacy_untriaged_paths"], 0)
        old.mkdir()
        with self.assertRaisesRegex(RuntimeError, "unregistered validation path"):
            storage.check(self.repo)

    def test_explicit_budget_can_be_removed_without_new_default(self):
        self.assertEqual(storage.main(["budget", "--gib", "1", "--reason", "fixture constraint"]), 0)
        self.assertEqual(storage.main(["budget", "--none", "--reason", "constraint no longer applies"]), 0)
        with storage.locked(self.repo) as ledger:
            self.assertIsNone(ledger["budget_bytes"])

    def test_native_stage_admission_accepts_no_budget(self):
        storage.register(self.repo, self.spec())
        with storage.locked(self.repo) as ledger:
            ledger["batches"]["first"].update(phase="running", pid=os.getpid(), process_identity=storage.live_identity(os.getpid()))
            storage.save(self.repo, ledger, "fixture live coordinator")
        with patch.dict(os.environ, {storage.BATCH_ENV: "first"}):
            storage.require_admission(self.repo, [self.output])

    def test_parent_hold_does_not_hide_new_unregistered_jobs(self):
        parent = self.repo / "target/native-acceptance/diagnosis"
        parent.mkdir(parents=True)
        storage.retain(self.repo, self.hold(parent, kind="evidence"))
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        (parent / "new-job").mkdir()
        storage.retain(self.repo, self.hold(parent, kind="evidence", next_action="updated description"))
        with self.assertRaisesRegex(RuntimeError, "unregistered validation path"):
            storage.check(self.repo)

    def test_parent_source_hold_cannot_retain_unneeded_job_outputs(self):
        storage.retain(self.repo, self.hold(self.repo, kind="evidence"))
        self.run_batch()
        self.seal_batch()
        with self.assertRaisesRegex(RuntimeError, "job remains"):
            storage.finalize(self.repo, "first")
        shutil.rmtree(self.output)
        storage.finalize(self.repo, "first")
        self.assertEqual(storage.check(self.repo)["status"], "pass")

    def test_only_latest_image_can_remain_without_preserving_whole_job(self):
        self.run_batch()
        image = self.output / "latest.png"
        image.write_bytes(b"fixture image")
        self.seal_batch()
        storage.retain(self.repo, self.hold(image, kind="evidence"))
        with self.assertRaisesRegex(RuntimeError, "job remains"):
            storage.finalize(self.repo, "first")
        (self.output / "manifest.json").unlink()
        storage.finalize(self.repo, "first")
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        (self.output / "new-job").mkdir()
        with self.assertRaisesRegex(RuntimeError, "unregistered validation path"):
            storage.check(self.repo)

    def test_new_output_cannot_be_hidden_in_legacy_group_or_reinitialized(self):
        old = self.repo / "target/native-acceptance/old-group/old-job"
        old.mkdir(parents=True)
        with storage.locked(self.repo) as ledger:
            ledger["legacy"] = storage.discover(self.repo)
            storage.save(self.repo, ledger, "fixture migration snapshot")
        (old.parent / "unregistered-job").mkdir()
        with self.assertRaisesRegex(RuntimeError, "unregistered validation path"):
            storage.check(self.repo)
        with self.assertRaisesRegex(RuntimeError, "already initialized"):
            storage.initialize(self.repo, storage.GIB)

    def test_policy_or_helper_change_rejects_planned_execution(self):
        script = self.repo / "runner.py"
        script.write_text("print('fixture')\n")
        storage.register(self.repo, self.spec(), command=[sys.executable, str(script)])
        script.write_text("raise SystemExit('changed')\n")
        with self.assertRaisesRegex(RuntimeError, "helper changed"):
            storage.execute(self.repo, "first")
        (self.repo / storage.POLICY).write_text("New policy")
        with self.assertRaisesRegex(RuntimeError, "policy changed"):
            storage.execute(self.repo, "first")

    def test_frozen_plan_uses_primary_coordinator_without_modifying_helper(self):
        helper = self.repo / "frozen.py"
        payload = {"status": "ready", "job_root": str(self.output),
            "launcher_command": ["kitty", "--directory", str(self.repo), "--detach", "env", "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", sys.executable, "-c", "print('frozen')"],
            "status_command": ["echo", "status"]}
        helper.write_text("import json; print(json.dumps(" + repr(payload) + "))\n")
        before = helper.read_bytes()
        planned = storage.plan(self.repo, self.spec(), [sys.executable, str(helper)])
        self.assertEqual(planned["launcher_command"][:4], payload["launcher_command"][:4])
        self.assertIn(str(self.repo / "scripts/validation_storage.py"), planned["launcher_command"])
        self.assertEqual(helper.read_bytes(), before)

    def test_linked_worktree_resolves_primary_common_directory(self):
        candidate = Path(self.temp.name) / "linked"
        subprocess.run(["git", "-C", str(self.repo), "worktree", "add", "-q", "--detach", str(candidate)], check=True)
        with patch.dict(os.environ):
            os.environ.pop(storage.PRIMARY_ENV, None)
            self.assertEqual(storage.primary_for(candidate), self.repo)

    def test_storage_rule_deletion_is_detected(self):
        rule = self.repo / "AGENTS.md"
        rule.write_text(check_agent_rules.MANDATORY_STORAGE_RULE)
        self.assertEqual(check_agent_rules.missing_storage_rules([rule]), ())
        rule.write_text("Run normal tests only")
        self.assertEqual(check_agent_rules.missing_storage_rules([rule]), (rule,))

    def test_deep_attempt_ancestors_are_allowed_but_unknown_sibling_is_not(self):
        output = self.repo / "target/perf-runs/rtt-light/contract/subject/attempts/one"
        self.run_batch(output=output)
        with storage.locked(self.repo) as ledger:
            self.assertEqual(storage.unregistered(ledger, storage.discover(self.repo)), [])
        (output.parent / "unknown").mkdir()
        with self.assertRaisesRegex(RuntimeError, "unregistered validation path"):
            storage.check(self.repo)

    def test_candidate_can_move_when_original_path_has_comparison_consumer(self):
        old = self.repo / "old-candidate"
        old.mkdir()
        new = self.repo / "new-candidate"
        new.mkdir()
        storage.retain(self.repo, self.hold(old))
        storage.retain(self.repo, self.hold(old, kind="comparison", identity="frozen"))
        storage.release(self.repo, "candidate", "wall-review", "advance editable candidate; original frozen for comparison")
        storage.retain(self.repo, self.hold(new, identity="next-candidate"))
        self.assertEqual(storage.check(self.repo)["status"], "pass")
        self.assertTrue(old.exists())

    def test_same_commit_source_change_and_wrong_verifier_are_rejected(self):
        code = self.repo / "crates/fixture/src/lib.rs"
        code.parent.mkdir(parents=True)
        code.write_text("before")
        storage.register(self.repo, self.spec(), command=[sys.executable, "-c", "pass"])
        code.write_text("after")
        with self.assertRaisesRegex(RuntimeError, "source/assets changed"):
            storage.execute(self.repo, "first")
        code.write_text("before")
        storage.execute(self.repo, "first")
        with self.assertRaisesRegex(RuntimeError, "verifier sealed"):
            storage.seal(self.repo, "first", {"result": "pass", "reason": "wrong target",
                "verify_command": [sys.executable, str(self.verifier), "verify", str(self.output.with_name("other"))]})

    def test_stale_verification_is_recoverable_without_creating_an_archive(self):
        storage.register(self.repo, self.spec(), command=[sys.executable, "-c", "pass"])
        with storage.locked(self.repo) as ledger:
            ledger["batches"]["first"].update(phase="sealing", pid=999999999, process_identity="old")
            storage.save(self.repo, ledger, "simulate interrupted seal")
        storage.recover(self.repo, "first", "helper stopped")
        self.seal_batch(result="abandoned")
        storage.finalize(self.repo, "first")

    def test_live_child_prevents_recovery_after_coordinator_dies(self):
        storage.register(self.repo, self.spec(), command=[sys.executable, "-c", "pass"])
        with storage.locked(self.repo) as ledger:
            ledger["batches"]["first"].update(phase="running", pid=999999999, process_identity="old",
                child_pid=os.getpid(), child_identity=storage.live_identity(os.getpid()))
            storage.save(self.repo, ledger, "simulate orphaned helper")
        with self.assertRaisesRegex(RuntimeError, "still alive"):
            storage.recover(self.repo, "first", "coordinator gone")
        with self.assertRaisesRegex(RuntimeError, "live execution"):
            self.seal_batch(result="abandoned")

    def test_regular_cargo_protects_frozen_view(self):
        raw = self.repo / "target/profiling"
        raw.mkdir(parents=True)
        storage.retain(self.repo, self.hold(raw, kind="comparison"))
        with self.assertRaisesRegex(RuntimeError, "frozen comparison blocks Cargo"):
            storage.require_mutable(self.repo)

    def test_cli_uses_primary_ledger_from_linked_checkout(self):
        candidate = Path(self.temp.name) / "linked-cli"
        subprocess.run(["git", "-C", str(self.repo), "worktree", "add", "-q", "--detach", str(candidate)], check=True)
        storage.retain(self.repo, self.hold(candidate))
        with patch.dict(os.environ), patch.object(storage, "REPO_ROOT", candidate):
            os.environ.pop(storage.PRIMARY_ENV, None)
            self.assertEqual(storage.main(["check"]), 0)

    def test_nested_frozen_checkout_does_not_block_unrelated_primary_build(self):
        candidate = self.repo / "target/historical-subject"
        subprocess.run(["git", "-C", str(self.repo), "worktree", "add", "-q", "--detach", str(candidate)], check=True)
        storage.retain(self.repo, self.hold(candidate, kind="comparison"))
        storage.require_mutable(self.repo)
        with self.assertRaisesRegex(RuntimeError, "frozen comparison"):
            storage.require_mutable(candidate)

    def test_seal_registers_live_verifier_child_before_accepting_its_result(self):
        self.verifier.write_text(self.verifier.read_text() +
            "import os, time\n" +
            "ledger = Path(" + repr(str(self.repo / ".git/validation-storage/ledger.json")) + ")\n" +
            "for attempt in range(100):\n" +
            "    batch = json.loads(ledger.read_text())['batches']['first']\n" +
            "    if batch.get('child_pid') == os.getpid(): break\n" +
            "    time.sleep(0.01)\n" +
            "else: raise AssertionError('unregistered sealing child')\n" +
            "assert batch['phase'] == 'sealing' and batch['child_identity']\n")
        self.run_batch()
        self.seal_batch()


if __name__ == "__main__":
    unittest.main()
