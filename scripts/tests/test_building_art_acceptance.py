"""Fail-closed recipe admission; no native processes are launched here."""
from __future__ import annotations

import copy
import csv
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import building_art_acceptance as art
from perf_tool.fixtures import write_fixture_run


class RecipeTests(unittest.TestCase):
    def test_m5_candidate_freezes_each_numeric_dependency_even_when_paths_are_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in art.pipeline.NUMERIC_VERIFIER_FILES:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"synthetic frozen dependency\n")
            with patch.object(art.pipeline, "__file__", str(root / "scripts/building_asset_pipeline.py")):
                inventory = art.pipeline.numeric_verifier_inventory()
                value = {"kind": "SandPile", "mode": "candidate", "numeric_verifiers": inventory}
                art.check_numeric_verifiers(value)
                for relative in inventory:
                    path = root / relative
                    original = path.read_bytes()
                    path.write_bytes(original + b"changed bytes, same path\n")
                    with self.subTest(relative=relative), self.assertRaisesRegex(ValueError, "inventory"):
                        # run and verify both enter this shared check before reading a candidate.
                        art.check_plan(value, Path("unused-codec"))
                    path.write_bytes(original)
                for kind in art.pipeline.building_m5_numeric.KINDS:
                    with self.assertRaisesRegex(ValueError, "inventory"):
                        art.check_numeric_verifiers({"kind": kind, "mode": "candidate"})
                for mode in ("feedback", "art-preview"):
                    art.check_numeric_verifiers({"kind": "SandPile", "mode": mode})
                    with self.assertRaises(ValueError):
                        art.check_numeric_verifiers({**value, "mode": mode})

    def context(self):
        descriptor = {"image_role": "world_preview", "canvas_px": [256, 256],
                      "canvas_wu": [128., 128.], "anchor_px": [128., 144.], "representative_state": "Empty"}
        artifact = {"role": "image:world_preview", "path": "preview.png", "bytes": 100, "sha256": "b" * 64}
        value = {"mode": "feedback", "kind": "RestArea", "identity": {"authority": "art_preview"},
                 "variant": "rest-area-art-context", "lighting": "standard", "view": "normal",
                 "scope": art.CONTEXT_SCOPE, "promotion_authority": False,
                 "context_preview": {"descriptor": descriptor, "artifact": artifact}}
        records = [{"ordinal": index, "occupants": (0, 1, 5, 0)[index], "owner": 100 + index,
                    "root": 200 + index, "tiles": [[index * 5 + x, y] for y in (18, 19) for x in (0, 1)],
                    "owner_screen": [200., 200.], "composite_screen": [200., 200.], "preview_offset_wu": [0., -160.],
                    "preview": {"entity": 300 + index, "descriptor": descriptor, "artifact": artifact,
                                "canvas_wu": [128., 128.], "anchor": [0., -.0625], "visible": True, "screen_anchor": [200., 360.]},
                    "parts": [{"visible": True, "material_values": dict.fromkeys(
                        ("white", "opaque", "lit", "emissive_black", "emissive_texture_absent"), True)}], "walls": [
                        {"owner": 400 + index * 2 + side, "grid": [index * 5, y], "visuals": [{"visible": True, "layers": [1, 5]}]}
                        for side, y in enumerate((17, 20))]} for index in range(4)]
        fixture = {"variant": value["variant"], "lighting": "standard", "scope": art.CONTEXT_SCOPE,
                   "identity": value["identity"], "target_count": 4, "wall_count": 8, "preview_count": 4,
                   "souls": 6, "view": "normal", "camera_scale": 1., "records": records,
                   "accepted": False, "performance_evidence": False, "promotion_authority": False,
                   "ambient": [{"brightness": 500.}], "directional": [
                       {"primary": True, "extra": False, "illuminance": 12000., "shadows": True},
                       {"primary": False, "extra": True, "illuminance": 0., "shadows": False}]}
        probe = {"profile": art.PROFILE, "status": "ready", "mode": "feedback", "identity": value["identity"],
                 "nonce": "a" * 32, "scope": art.CONTEXT_SCOPE, "stable_frames": 30, "view": "normal",
                 "camera_scale": 1., "variant": value["variant"], "lighting": "standard", "fixture": fixture}
        return value, probe

    def test_context_is_separate_and_exactly_admitted(self):
        value, probe = self.context()
        art.validate_probe(probe, value, probe["nonce"])
        command = art.game_command(Path("binary"), Path("job"), value)
        self.assertEqual(command[command.index("--spawn-souls") + 1], "6")
        old = art.game_command(Path("binary"), Path("job"), {})
        self.assertEqual(old[old.index("--spawn-souls") + 1], "15")
        environment = art.game_environment({}, Path("view"), {**value, "adapter": "Intel"}, "nonce", Path("job"))
        session = json.loads(environment["HW_BUILDING_ART_SESSION"])
        self.assertEqual(session["variant"], value["variant"])
        self.assertEqual(session["lighting"], value["lighting"])
        for key, changed in (("variant", "unknown"), ("lighting", "unknown"), ("kind", "Tank"),
                             ("mode", "candidate"), ("view", "overview"), ("scope", "ordinary-world"),
                             ("promotion_authority", True), ("identity", {"authority": "isolated_candidate"})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                art.context_settings({**value, key: changed})
        with self.assertRaises(ValueError):
            art.context_settings({"lighting": "dim"})
        # A context's four owners can never pass the old 36-owner gallery check.
        with self.assertRaises(ValueError):
            art.validate_probe(probe, {**value, "variant": "gallery", "lighting": "standard"}, probe["nonce"])

    def test_manifest_image_artifact_namespace_is_exact_and_unique(self):
        value, _ = self.context()
        expected = value["context_preview"]
        manifest = {"world_preview": expected["descriptor"], "artifacts": [expected["artifact"]]}
        self.assertEqual(art.context_preview(manifest), expected)
        for artifacts in ([], [{**expected["artifact"], "role": "world_preview"}],
                          [expected["artifact"], expected["artifact"]]):
            with self.subTest(artifacts=artifacts), self.assertRaisesRegex(ValueError, "image:"):
                art.context_preview({**manifest, "artifacts": artifacts})

    def test_context_rejects_wrong_inventory_identity_preview_walls_and_lights(self):
        value, original = self.context()
        changes = [
            ((), "lighting", "dim"), (("fixture",), "identity", {}),
            (("fixture",), "target_count", 36), (("fixture",), "wall_count", 7),
            (("fixture",), "preview_count", True), (("fixture",), "accepted", True),
            (("fixture",), "performance_evidence", True), (("fixture",), "promotion_authority", True),
            (("fixture", "records", 0), "owner", 101),
            (("fixture", "records", 0), "occupants", 5),
            (("fixture", "records", 0, "preview"), "anchor", [0, 0]),
            (("fixture", "records", 0, "preview"), "canvas_wu", [64, 64]),
            (("fixture", "records", 0, "preview"), "visible", False),
            (("fixture", "records", 0, "preview", "artifact"), "sha256", "f" * 64),
            (("fixture", "records", 0, "preview", "artifact"), "role", "world_preview"),
            (("fixture", "records", 0, "walls", 0), "grid", [0, 21]),
            (("fixture", "records", 0, "parts", 0), "visible", False),
            (("fixture", "records", 0, "parts", 0, "material_values"), "lit", False),
            (("fixture", "records", 0, "walls", 0, "visuals", 0), "visible", False),
            (("fixture", "records", 0, "walls", 0, "visuals", 0), "layers", [0]),
            (("fixture", "records", 0), "owner_screen", [float("nan"), 200.]),
            (("fixture", "records", 0), "composite_screen", [205., 200.]),
            (("fixture", "records", 0, "preview"), "screen_anchor", [200., 365.]),
            (("fixture", "records", 0, "preview"), "screen_anchor", [200., 800.]),
            (("fixture", "ambient", 0), "brightness", 100.),
            (("fixture", "directional", 0), "illuminance", 0.),
            (("fixture", "directional", 0), "shadows", False),
            (("fixture",), "directional", []),
        ]
        for path, key, changed in changes:
            bad = copy.deepcopy(original)
            target = bad
            for component in path:
                target = target[component]
            target[key] = changed
            with self.subTest(path=path, key=key), self.assertRaises(ValueError):
                art.validate_probe(bad, value, bad["nonce"])
        dim = copy.deepcopy(original)
        dim["lighting"] = dim["fixture"]["lighting"] = "dim"
        dim["fixture"]["ambient"][0]["brightness"] = 100.
        dim["fixture"]["directional"][0]["illuminance"] = 2400.
        art.validate_probe(dim, {**value, "lighting": "dim"}, dim["nonce"])

    def test_quality_is_bounded_and_legacy_plans_remain_high(self):
        self.assertEqual(art.quality({}), "high")
        for quality in art.QUALITIES:
            command = art.game_command(Path("binary"), Path("job"), {"quality": quality})
            self.assertEqual(command[command.index("--perf-rtt-quality") + 1], quality)
        for invalid in (None, "ultra", "Low", 1):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "quality"):
                art.game_command(Path("binary"), Path("job"), {"quality": invalid})

    def test_actual_window_quality_resolution_and_end_state_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_fixture_run(root)
            path = root / "data/window.csv"
            with path.open(newline="") as handle:
                original = next(csv.DictReader(handle))

            def write(row):
                with path.open("w", newline="") as handle:
                    writer = csv.DictWriter(handle, fieldnames=list(original))
                    writer.writeheader()
                    writer.writerow(row)

            for quality, factor in (("high", 1), ("medium", .75), ("low", .5)):
                row = dict(original)
                for prefix in ("", "end_"):
                    row.update({prefix + "rtt_quality": quality,
                                prefix + "scene_target_width": str(int(1280 * factor)),
                                prefix + "scene_target_height": str(int(720 * factor)),
                                prefix + "mask_target_width": str(int(1280 * factor)),
                                prefix + "mask_target_height": str(int(720 * factor)),
                                prefix + "target_scale_factor": str(factor)})
                write(row)
                value = {"quality": quality, "adapter": "Test GPU"}
                art.validate_window(root, value)
                for field, changed in (("rtt_quality", "invalid"),
                                       ("scene_target_width", "999"),
                                       ("end_rtt_quality", "invalid"),
                                       ("resolved_window_backend", "wayland")):
                    write({**row, field: changed})
                    with self.subTest(quality=quality, field=field), self.assertRaisesRegex(ValueError, "window"):
                        art.validate_window(root, value)
                write(row)
                with self.assertRaisesRegex(ValueError, "adapter"):
                    art.validate_window(root, {**value, "adapter": "foreign"})
            path.unlink()
            with self.assertRaisesRegex(ValueError, "missing"):
                art.validate_window(root, value)

    def test_camera_view_requires_matching_runtime_scale_and_inventory(self):
        value = {"mode": "feedback", "identity": {}, "scope": "paused-static-presentation", "view": "near"}
        probe = {"profile": art.PROFILE, "status": "ready", "mode": "feedback", "identity": {},
                 "nonce": "a" * 32, "scope": value["scope"], "stable_frames": 30,
                 "view": "near", "camera_scale": 0.5,
                 "fixture": {"target_count": 36, "view": "near", "camera_scale": 0.5}}
        art.validate_probe(probe, value, probe["nonce"])
        for key, changed in [("view", "far"), ("camera_scale", 5.0)]:
            with self.assertRaisesRegex(ValueError, "camera"):
                art.validate_probe({**probe, key: changed}, value, probe["nonce"])
        with self.assertRaisesRegex(ValueError, "inventory"):
            art.validate_probe({**probe, "fixture": {"target_count": 4}}, value, probe["nonce"])

    def test_child_logging_cannot_inherit_suppressed_adapter_evidence(self):
        original = {"RUST_LOG": "warn", "HW_WALL_MODE": "foreign", "KEEP": "unchanged"}
        value = {"kind": "RestArea", "mode": "feedback", "adapter": "Intel",
                 "identity": {"authority": "art_preview"}}
        result = art.game_environment(original, Path("view"), value, "nonce", Path("job"))
        self.assertEqual(result["RUST_LOG"], "info,wgpu=error")
        self.assertEqual(result["WGPU_BACKEND"], "vulkan")
        self.assertEqual(result["WGPU_ADAPTER_NAME"], "Intel")
        self.assertNotIn("HW_WALL_MODE", result)
        self.assertEqual(original["RUST_LOG"], "warn")
        self.assertEqual(result["KEEP"], original["KEEP"])

    def test_plan_reports_ready_only_after_resource_admission(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "codec"
            binary.write_bytes(b"codec fixture")
            args = SimpleNamespace(repo=root, mode="feedback", job_root=root / "job",
                                   source_root=root / "candidate", kind="RestArea", codec=binary,
                                   adapter="fixture", output=root / "plan.json")
            with patch.object(art.native, "validate_repo", return_value=root), \
                    patch.object(art, "subject", return_value=self.source()), \
                    patch.object(art.static, "check_root"), \
                    patch.object(art.pipeline, "load_set", return_value=(
                        {"identity": {"authority": "art_preview"}}, "{}", None)), \
                    patch.object(art, "check_plan"), \
                    patch.object(art.native, "resource_snapshot", return_value={"failures": []}) as resources:
                value = art.plan(args)
                self.assertEqual(value["status"], "ready")
                self.assertFalse(value["promotion_authority"])
                self.assertTrue(value["feedback_only"])
                args.output.unlink()
                resources.return_value = {"failures": ["insufficient memory"]}
                with self.assertRaisesRegex(ValueError, "insufficient memory"):
                    art.plan(args)
                self.assertFalse(args.output.exists())

    def source(self, digit="a"):
        return {"commit": digit * 40, "source": digit * 64, "harness": "b" * 64,
                "assets": "c" * 64, "driver": "d" * 64, "dirty": []}

    def test_feedback_cannot_become_clean_candidate_evidence(self):
        dirty = {**self.source(), "dirty": ["crates/bevy_app/src/main.rs"]}
        art.validate_subject(dirty, feedback=True)
        with self.assertRaisesRegex(ValueError, "clean"):
            art.validate_subject(dirty, feedback=False)
        missing = self.source()
        del missing["source"]
        with self.assertRaisesRegex(ValueError, "incomplete"):
            art.validate_subject(missing, feedback=False)
        value = {"mode": "candidate", "identity": {"authority": "isolated_candidate"}, "scope": "paused-static-presentation"}
        probe = {"profile": "building-art", "status": "ready", "mode": "art-preview", "identity": value["identity"],
                 "scope": value["scope"], "nonce": "a" * 32, "stable_frames": 30, "fixture": {"target_count": 36}}
        with self.assertRaisesRegex(ValueError, "acknowledgement"):
            art.validate_probe(probe, value, "a" * 32)

    def identities(self):
        left = {"subject_commit": "a" * 40, "source_fingerprint": "a" * 64,
                "harness_fingerprint": "b" * 64, "asset_view_fingerprint": "c" * 64, "driver_sha256": "d" * 64}
        right = {**left, "subject_commit": "e" * 40, "source_fingerprint": "e" * 64}
        return left, right

    def comparison(self, left_identity, right_identity, *, equal_binary=False, layout="f" * 64):
        left = {"identity": left_identity, "adapter": "fixture-adapter", "repo": "baseline"}
        right = {"identity": right_identity, "adapter": "fixture-adapter", "repo": "candidate"}
        before = {"sessions": [{"instrumentation": "capture", "size": "small", "binary_sha256": "1" * 64,
            "observations": [{"layout_sha256": "f" * 64, "adapter": "fixture-adapter"}], "aggregate": {"p95": {"median": 1.0}}}]}
        after = copy.deepcopy(before)
        after["sessions"][0]["binary_sha256"] = ("1" if equal_binary else "2") * 64
        after["sessions"][0]["observations"][0]["layout_sha256"] = layout
        with patch.object(art.native, "read_json", side_effect=[left, right]), \
                patch.object(art, "verify_frozen_static", side_effect=[before, after]) as verifier, \
                patch.object(art, "source_roles"):
            result = art.compare(Path("baseline"), Path("candidate"), left_identity, right_identity)
            self.assertEqual(verifier.call_count, 2)
            self.assertEqual(verifier.call_args_list[0].args, (Path("baseline"), left, left_identity))
            self.assertEqual(verifier.call_args_list[1].args, (Path("candidate"), right, right_identity))
            return result

    def test_separate_source_comparison_revalidates_both_raw_jobs_without_budget_claim(self):
        result = self.comparison(*self.identities())
        self.assertEqual(result["status"], "compared")
        self.assertEqual(result["budget_decision"], "not-evaluated")
        self.assertFalse(result["promotion_authority"])

    def test_same_source_same_binary_and_fabricated_fallback_are_rejected(self):
        left, right = self.identities()
        with self.assertRaisesRegex(ValueError, "distinct sources"):
            self.comparison(left, left)
        with self.assertRaisesRegex(ValueError, "binary pair"):
            self.comparison(left, right, equal_binary=True)
        with self.assertRaisesRegex(ValueError, "layout differs"):
            self.comparison(left, right, layout="0" * 64)
        with self.assertRaisesRegex(ValueError, "assets/adapter"):
            self.comparison(left, {**right, "asset_view_fingerprint": "0" * 64})

    def test_missing_or_malformed_full_identity_is_not_a_baseline(self):
        left, right = self.identities()
        for invalid in ({}, {**left, "source_fingerprint": "unknown"}):
            with self.assertRaises(ValueError):
                self.comparison(invalid, right)

    def test_two_post_m1_sources_do_not_replace_the_pre_m1_baseline(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline, candidate = Path(directory) / "baseline", Path(directory) / "candidate"
            marker = "crates/bevy_app/src/assets/building_asset_set/mod.rs"
            fixture = "crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/mod.rs"
            for root in (baseline, candidate):
                (root / fixture).parent.mkdir(parents=True)
                (root / fixture).write_text("fixture")
            (candidate / marker).parent.mkdir(parents=True)
            (candidate / marker).write_text("foundation")
            art.source_roles(baseline, candidate)
            (baseline / marker).parent.mkdir(parents=True)
            (baseline / marker).write_text("foundation")
            with self.assertRaisesRegex(ValueError, "pre-M1"):
                art.source_roles(baseline, candidate)


class FrozenVerifierTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def fixture(self, name):
        repo, job = self.root / name, self.root / (name + "-job")
        driver = repo / "scripts/building_art_static_acceptance.py"
        driver.parent.mkdir(parents=True)
        driver.write_text("# frozen driver " + name)
        job.mkdir()
        identity = {"subject_commit": name[0] * 40, "source_fingerprint": name[0] * 64,
                    "harness_fingerprint": art.pipeline.digest(name.encode()),
                    "asset_view_fingerprint": "c" * 64,
                    "driver_sha256": art.pipeline.digest(driver.read_bytes())}
        manifest = {"repo": str(repo), "profile": art.static.PROFILE, "identity": identity,
                    "sessions": [{"raw_inventory": name}]}
        (job / "manifest.json").write_text(json.dumps(manifest))
        result = {"status": "pass", "profile": art.static.PROFILE,
                  "subject_commit": identity["subject_commit"], "capture_runs": 6,
                  "memory_runs": 3, "active_simulation": "not-measured",
                  "sessions": manifest["sessions"]}
        return repo, job, driver, manifest, identity, result

    def invoke(self, fixture, *, result=None, returncode=0, dirty=None, mutate=None):
        repo, job, driver, manifest, identity, valid = fixture

        def child(command, **kwargs):
            self.assertEqual(command, [sys.executable, "-B", "-E", "-s", str(driver),
                                       "verify", "--job-root", str(job.resolve())])
            self.assertEqual(kwargs["cwd"], repo)
            if mutate:
                mutate()
            return subprocess.CompletedProcess(command, returncode,
                json.dumps(valid) if result is None else result, "raw job rejected")

        with patch.object(art.native, "validate_repo", return_value=repo), \
                patch.object(art.native, "git_subject", return_value=identity["subject_commit"]), \
                patch.object(art.native, "git_dirty_paths", return_value=dirty or []), \
                patch.object(art.subprocess, "run", side_effect=child) as process, \
                patch.object(art.static, "verify", side_effect=AssertionError("wrong source verifier")):
            try:
                return art.verify_frozen_static(job, manifest, identity)
            finally:
                self.launched = process.call_count

    def test_each_frozen_source_uses_its_own_driver_and_module_process(self):
        before, after = self.fixture("baseline"), self.fixture("candidate")
        self.assertNotEqual(before[4]["harness_fingerprint"], after[4]["harness_fingerprint"])
        self.assertNotEqual(before[4]["driver_sha256"], after[4]["driver_sha256"])
        for fixture in (before, after):
            self.assertEqual(self.invoke(fixture), fixture[5])
            self.assertEqual(self.launched, 1)

    def test_failed_raw_verification_never_falls_back_to_current_driver(self):
        with self.assertRaisesRegex(ValueError, "raw-job verification failed"):
            self.invoke(self.fixture("baseline"), returncode=1)

    def test_malformed_or_mismatched_verifier_result_is_rejected(self):
        fixture = self.fixture("baseline")
        for output in ("not json", "[]", json.dumps({**fixture[5], "status": "fail"}),
                       json.dumps({**fixture[5], "subject_commit": "f" * 40}),
                       json.dumps({**fixture[5], "sessions": []})):
            with self.subTest(output=output), self.assertRaises(ValueError):
                self.invoke(fixture, result=output)

    def test_dirty_or_modified_driver_is_rejected_before_launch(self):
        fixture = self.fixture("baseline")
        with self.assertRaisesRegex(ValueError, "dirty or changed"):
            self.invoke(fixture, dirty=["scripts/helper.py"])
        self.assertEqual(self.launched, 0)
        fixture[2].write_text("# changed driver")
        with self.assertRaisesRegex(ValueError, "driver identity differs"):
            self.invoke(fixture)
        self.assertEqual(self.launched, 0)

    def test_manifest_or_driver_changes_during_verification_are_rejected(self):
        for name in ("baseline", "candidate"):
            fixture = self.fixture(name)
            target = fixture[1] / "manifest.json" if name == "baseline" else fixture[2]
            with self.subTest(target=target), self.assertRaises(ValueError):
                self.invoke(fixture, mutate=lambda: target.write_text("{}"))
            self.assertEqual(self.launched, 1)


if __name__ == "__main__":
    unittest.main()
