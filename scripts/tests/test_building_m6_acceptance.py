"""Synthetic tests only; these identities do not represent approved assets."""

import copy
import hashlib
import io
import unittest
import tempfile
import os
import struct
import importlib
from dataclasses import asdict
from types import SimpleNamespace
from unittest.mock import patch
from contextlib import redirect_stdout
from pathlib import Path

from scripts import building_m6_acceptance as m6
from perf_tool.arguments import build_parser, validate_arguments
from perf_tool.artifact_readers import building_m6 as oracle
from perf_tool.artifact_readers.building_art_static import expected_records
from perf_tool.model import Case
from perf_tool.execution import fixed_environment


def sidecar(candidate=False, copies=4):
    bindings = {kind: {"kind": kind, "authority": "release_approved", "generation": 1,
                       "manifest_sha256": "a" * 64} for kind in oracle.SETS}
    records = expected_records(copies)
    if candidate:
        for index, row in enumerate(records):
            if row["kind"] == "Door":
                continue
            row["parts"] = [
                {
                    "mesh_role": role,
                    "translation_wu": [0.0, 0.0, 0.0],
                    "rotation_xyzw": [0.0, 0.0, 0.0, 1.0],
                    "scale": [1.0, 1.0, 1.0],
                    "visible": not (
                        row["kind"] == "Tank"
                        and role == "water"
                        and row["state"]["stored_water"] == 0
                    ),
                }
                for role in oracle.PARTS.get(row["kind"], [])
            ]
            if row["kind"] in {"Tank", "MudMixer"}:
                row["root_translation_wu"] = [row["center"][0], 0.0, -row["center"][1]]
            if row["kind"] in oracle.foreground.KINDS:
                owner = 1000 + index * 2
                row["owner"] = owner
                kind = row["kind"]
                preview = {"image_role": "world_off" if kind == "OutdoorLamp" else "world",
                           "canvas_px": [256, 256], "canvas_wu": [64., 64.] if kind == "WheelbarrowParking" else [32., 32.],
                           "anchor_px": [128., 128.], "representative_state": "Off" if kind == "OutdoorLamp" else "Empty"}
                role = "world_on" if kind == "OutdoorLamp" and row["ordinal"] % 4 >= 2 else preview["image_role"]
                owner_z = .05 if kind in {"SandPile", "BonePile"} else .12
                pose = {"translation": [*row["center"], struct.unpack("<f", struct.pack("<f", owner_z))[0]],
                        "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
                local = {"translation": [0., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
                row["foreground"] = {"schema": 1, "identity": bindings[kind], "descriptor": preview,
                    "owner": owner, "entity": owner + 1, "parent": owner, "image_role": role,
                    "artifact": {"role": "image:" + role, "path": role + ".png", "bytes": 100, "sha256": "b" * 64},
                    "image_handle": "synthetic-image-" + kind + role, "image_matches": True,
                    "canvas_wu": preview["canvas_wu"], "anchor": [0., 0.], "visible": True, "layers": [0],
                    "color_srgba": [1., 1., 1., 1.], "owner_pose": pose, "local_pose": local,
                    "global_pose": copy.deepcopy(pose), "global_matrix": oracle.foreground.matrix(pose),
                    "parent_local_matrix": oracle.foreground.matrix(pose)}
    evidence = {
        "records": records,
        "target_count": copies * 9,
        "target_structural_roots": copies * 5,
        "target_foreground_owners": copies * 4,
        "target_active_unique_meshes": 8 if candidate else 2,
        "souls": copies // 4 * 15,
        "completion_effects": 0,
        "m6": {
            "mode": "candidate" if candidate else "legacy-control",
            "identities": list(bindings.values()),
            "door_identity": {
                "asset_set_generation": 7,
                "authority": "release_approved",
                "manifest_sha256": oracle.DOOR_HASH,
            },
            "target_mesh_entities": copies * (11 if candidate else 5),
            "pool_active": 8 if candidate else 0,
            "pool_pending": 0,
            "resources": dict.fromkeys(oracle.RESOURCE_KEYS, 100),
            "gpu_allocation_bytes": None,
            "active_state_acceptance": False,
        },
    }
    value = {
        "schema_version": 1,
        "contract_id": oracle.CONTRACT,
        "evidence_kind": "paused-static-only",
        "active_simulation_evidence": False,
        "camera_scale": 5.0,
        "stable_frames": 200,
        "initial": evidence,
    }
    return reseal(value)


def reseal(value):
    value["final"] = copy.deepcopy(value["initial"])
    value["layout_sha256"] = hashlib.sha256(
        oracle.canonical(value["initial"])
    ).hexdigest()
    return value


class M6Tests(unittest.TestCase):
    def test_candidate_foreground_keeps_session_binary_identity(self):
        """Exercise the whole case verifier after an M5 asset manifest is inspected."""
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        observed = sidecar(candidate=True)["initial"]
        row = next(r for r in observed["records"] if r["kind"] == "OutdoorLamp")
        observed["records"] = [row]
        asset = {"identity": row["foreground"]["identity"], "parts": [],
                 "world_preview": row["foreground"]["descriptor"],
                 "artifacts": [row["foreground"]["artifact"]]}
        frozen = {"subject_commit": "a" * 40, "source_fingerprint": "b" * 64}
        session_manifest = {"status": "valid", "git": {"commit": frozen["subject_commit"], "dirty_paths": []},
                            "source": {"fingerprint_start": frozen["source_fingerprint"],
                                       "fingerprint_end": frozen["source_fingerprint"], "unchanged": True},
                            "binary": {"instrumentation": "capture", "sha256": "c" * 64}}
        validation = SimpleNamespace(valid=True, reasons=[], summary={"samples": 1},
                                     adapter={"name": "synthetic"}, window={}, to_json=lambda: {})
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            session = root / "capture-small-1-candidate"
            run = session / "cases" / case.identifier / "run-001"
            run.mkdir(parents=True)
            value = {"job_root": str(root), "profile": m6.PRE_RELEASE_PROFILE, "identity": frozen,
                     "adapter": "synthetic", "assets": {"bindings": [{"identity": asset["identity"]}],
                     "manifests": [asset], "files": {}}}
            observed["m6"]["identities"] = [asset["identity"]]
            requested = {"HW_BUILDING_M6_MODE": "candidate", "RUST_LOG": "info,wgpu=error",
                         "BEVY_ASSET_ROOT": str(root / "asset-view"),
                         "HW_BUILDING_M6_CANDIDATES": m6.json.dumps(value["assets"]["bindings"])}
            payloads = {"manifest.json": session_manifest,
                        "matrix.json": {"repeat": 1, "preflight_runs": 0, "warmup_secs": 30.,
                                        "measure_secs": 60., "sizes": ["small"], "renders": ["gpu"]},
                        "run-metadata.json": {"case": asdict(case), "returncode": 0,
                                              "actual_adapter": validation.adapter, "actual_window": {},
                                              "runtime_data_cleaned": True},
                        "requested-environment.json": requested,
                        "building_art_static.json": {"stable_frames": 1}, "validation.json": {}}
            with patch.object(m6.native, "read_json", side_effect=lambda p: payloads[p.name]), \
                 patch.object(m6, "validate_run", return_value=validation), \
                 patch.object(importlib.import_module("scripts.perf_tool.artifact_readers.building_m6"),
                              "validate_pre_release_sidecar", return_value=observed), \
                 patch.object(m6, "read_frames", return_value=([1], [])), \
                 patch.object(m6, "frame_summary", return_value={"p95_ms": 1., "p99_ms": 2.}), \
                 patch.object(m6.static, "session_digest", return_value="d" * 64):
                result = m6.verify_case(value, ("capture", "small", 1, "candidate"))
            self.assertEqual(result["binary_sha256"], session_manifest["binary"]["sha256"])

    def test_foreground_dependency_is_frozen_and_missing_binding_is_rejected(self):
        value = {"foreground_verifier_sha256": m6.art.foreground.verifier_sha256()}
        m6.check_foreground_verifier(value)
        for wrong in ({}, {"foreground_verifier_sha256": "a" * 64}):
            with self.assertRaisesRegex(ValueError, "foreground verifier changed"):
                m6.check_foreground_verifier(wrong)

    def test_m6_foreground_rejects_unknown_fields_cross_owner_and_medium_lamp_role(self):
        case = Case("building-art-static", "medium", "gpu", 20260920, 60, 0)
        value = sidecar(True, 16)
        oracle.validate_sidecar(value, case)
        def target(document, kind="OutdoorLamp", ordinal=4):
            return next(row for row in document["initial"]["records"] if row["kind"] == kind and row["ordinal"] == ordinal)
        mutations = (
            lambda v: target(v).update(unknown=True),
            lambda v: target(v)["foreground"].update(unknown=True),
            lambda v: target(v).pop("foreground"),
            lambda v: target(v)["foreground"].update(owner=1),
            lambda v: target(v)["foreground"].update(parent=1),
            lambda v: target(v)["foreground"].update(layers=[1]),
            lambda v: target(v)["foreground"].update(image_matches=False),
            lambda v: target(v)["foreground"].update(image_role="world_on"),
            lambda v: target(v)["foreground"].update(identity={**target(v)["foreground"]["identity"], "manifest_sha256": "b" * 64}),
            lambda v: target(v)["foreground"]["owner_pose"]["translation"].__setitem__(2, 0.0),
        )
        for mutate in mutations:
            bad = copy.deepcopy(value)
            mutate(bad)
            with self.assertRaises((ValueError, KeyError)):
                oracle.validate_sidecar(reseal(bad), case)
        bad = copy.deepcopy(value)
        target(bad)["foreground"]["entity"] = target(bad, ordinal=0)["foreground"]["entity"]
        with self.assertRaisesRegex(ValueError, "duplicate foreground"):
            oracle.validate_sidecar(reseal(bad), case)
        bad = copy.deepcopy(value)
        bad["final"]["records"][-1]["state"]["powered"] = False
        with self.assertRaisesRegex(ValueError, "checkpoint drift"):
            oracle.validate_sidecar(bad, case)
    def test_cli_binds_frozen_repository_and_root_before_the_activity_guard(self):
        with tempfile.TemporaryDirectory() as directory:
            plan = Path(directory) / "plan.json"
            m6.pipeline.put(plan, m6.pipeline.canonical({"repo": "/frozen/repo", "job_root": "/frozen/repo/target/native-acceptance/job"}))
            with patch("sys.argv", ["building_m6_acceptance.py", "run", "--plan", str(plan)]), \
                 patch.object(m6, "run", return_value={"status": "synthetic"}) as guarded, \
                 redirect_stdout(io.StringIO()):
                m6.main()
            guarded.assert_called_once()
            args = guarded.call_args.args[0]
            self.assertEqual(args.repo, "/frozen/repo")
            self.assertEqual(args.job_root, "/frozen/repo/target/native-acceptance/job")
            self.assertEqual(args.plan, plan)
            self.assertEqual(args.frozen_plan, m6.pipeline.read(plan))

    def test_changed_plan_is_rejected_before_run_inputs_or_outputs_are_touched(self):
        from types import SimpleNamespace
        args = SimpleNamespace(plan=Path("/private/plan"), frozen_plan={"job_root": "/admitted"})
        with patch.dict(os.environ, {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
             patch.object(m6.pipeline, "read", return_value={"job_root": "/changed"}), \
             patch.object(m6, "check_inputs") as check:
            with self.assertRaisesRegex(ValueError, "plan changed across activity admission"):
                m6.run.__wrapped__(args)
            check.assert_not_called()

    def test_pre_release_freezes_provenance_verifier_code_not_only_candidate_bytes(self):
        value = {"profile": m6.PRE_RELEASE_PROFILE,
                 "numeric_verifiers": m6.pipeline.numeric_verifier_inventory(),
                 "art_driver_sha256": m6.pipeline.digest(Path(m6.art.__file__).read_bytes())}
        m6.check_candidate_verifiers(value)
        with patch.object(m6.pipeline, "numeric_verifier_inventory", return_value={"changed.py": "a" * 64}):
            with self.assertRaises(ValueError):
                m6.check_candidate_verifiers(value)
        bad = dict(value, art_driver_sha256="0" * 64)
        with self.assertRaises(ValueError):
            m6.check_candidate_verifiers(bad)
        with self.assertRaises(ValueError):
            m6.check_candidate_verifiers(dict(value, profile=m6.PROFILE))

    def test_paused_parts_use_exact_f32_and_approved_tank_state(self):
        part = {"mesh_role": "water", "translation_wu": [0, 0.6, 0],
                "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}
        spa = m6.projected_parts({"parts": [part]}, {"kind": "SoulSpa"})[0]
        expected = struct.unpack("<f", struct.pack("<f", 0.6))[0]
        self.assertEqual(spa["translation_wu"][1], expected)
        self.assertNotEqual(spa["translation_wu"][1], 0.6)
        neighbor = struct.unpack("<f", struct.pack("<I", struct.unpack("<I", struct.pack("<f", 0.6))[0] + 1))[0]
        self.assertNotEqual(spa["translation_wu"][1], neighbor)
        manifest = {"parts": [part], "production_state": {"kind": "Tank", "partial_y_wu": 12, "full_y_wu": 22}}
        for water, y in ((0, 12), (25, 12), (50, 22)):
            actual = m6.projected_parts(manifest, {"kind": "Tank", "state": {"stored_water": water}})
            self.assertEqual(actual[0]["translation_wu"], [0, y, 0])
        self.assertEqual(part["translation_wu"], [0, 0.6, 0])
        for number in (float("nan"), float("inf"), True):
            invalid = copy.deepcopy(part)
            invalid["translation_wu"][1] = number
            with self.assertRaises(ValueError):
                m6.projected_parts({"parts": [invalid]}, {"kind": "SoulSpa"})

    def test_pre_release_view_requires_mode_and_records_explicit_candidates(self):
        argv = m6.command(Path("/job"), m6.matrix()[0], "Intel", pre_release=True)
        args = build_parser().parse_args(argv[2:])
        validate_arguments(args)
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaises(ValueError):
                fixed_environment(args)
        with patch.dict(os.environ, {"HW_BUILDING_M6_CANDIDATES": "exact-bindings", "RUST_LOG": "error"}):
            env = fixed_environment(args)
            self.assertEqual(env["BEVY_ASSET_ROOT"], "/job/asset-view")
            self.assertEqual(env["HW_BUILDING_M6_CANDIDATES"], "exact-bindings")
            self.assertEqual(env["RUST_LOG"], "info,wgpu=error")
        args.building_m6_mode = None
        with self.assertRaises(ValueError):
            validate_arguments(args)

    def test_candidate_view_preserves_door_and_rejects_artifact_collision_or_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, candidate, job = root / "repo", root / "candidate", root / "job"
            (repo / "assets/manifests").mkdir(parents=True)
            (repo / "assets/manifests/door").write_bytes(b"unchanged-door")
            (candidate / "manifests").mkdir(parents=True)
            locator = m6.pipeline.locator("Tank")
            (candidate / locator).write_bytes(b"candidate-manifest")
            payload = b"candidate-mesh"
            (candidate / "mesh").write_bytes(payload)
            record = {"path": "mesh", "bytes": len(payload), "sha256": m6.pipeline.digest(payload)}
            value = {"repo": str(repo), "job_root": str(job), "profile": m6.PRE_RELEASE_PROFILE,
                     "assets": {"sources": [{"root": str(candidate), "kind": "Tank"}],
                                "manifests": [{"artifacts": [record]}]}}
            m6.prepare_view(value)
            m6.check_view(value)
            self.assertEqual((job / "asset-view/assets/manifests/door").read_bytes(), b"unchanged-door")
            (job / "asset-view/assets/mesh").write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                m6.check_view(value)
            (repo / "assets/mesh").write_bytes(b"different-primary")
            with self.assertRaises(ValueError):
                m6.expected_view(value)

    def test_pre_release_contract_is_exact_and_rejected_by_release_oracle(self):
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        for candidate in (False, True):
            value = sidecar(candidate)
            value["contract_id"] = oracle.PRE_RELEASE_CONTRACT
            value["initial"]["pre_release_comparison"] = True
            for identity in value["initial"]["m6"]["identities"]:
                identity["authority"] = "isolated_candidate"
            reseal(value)
            oracle.validate_pre_release_sidecar(value, case)
            with self.assertRaises(ValueError):
                oracle.validate_sidecar(value, case)
            for mutation in ("release", "scope", "pool", "bridge"):
                bad = copy.deepcopy(value)
                if mutation == "release":
                    bad["initial"]["m6"]["identities"][0]["authority"] = "release_approved"
                elif mutation == "scope":
                    bad["initial"]["pre_release_comparison"] = False
                elif mutation == "bridge":
                    bad["initial"]["m6"]["identities"][0]["kind"] = "Bridge"
                else:
                    bad["initial"]["m6"]["pool_active"] += 1
                with self.assertRaises(ValueError):
                    oracle.validate_pre_release_sidecar(reseal(bad), case)
        with self.assertRaises(ValueError):
            oracle.validate_pre_release_sidecar(sidecar(), case)

    def test_candidate_inventory_rejects_incomplete_kind_map_before_codec(self):
        for roots in ({}, {"Tank": "/candidate"}, {"Bridge": "/candidate"}):
            with self.assertRaises(ValueError):
                m6.inventory(Path("/repo"), Path("/codec"), roots)

    def test_file_inventory_rejects_symlinks_and_tracks_changed_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "asset").write_bytes(b"before")
            before = m6.file_inventory(root)
            (root / "asset").write_bytes(b"after")
            self.assertNotEqual(before, m6.file_inventory(root))
            (root / "alias").symlink_to(root / "asset")
            with self.assertRaises(ValueError):
                m6.file_inventory(root)

    def test_independent_logical_oracle_accepts_both_modes_and_densities(self):
        for candidate in (False, True):
            for size, copies in (("small", 4), ("medium", 16)):
                oracle.validate_sidecar(
                    sidecar(candidate, copies),
                    Case(
                        "building-art-static",
                        size,
                        "gpu",
                        20260920,
                        copies // 4 * 15,
                        0,
                    ),
                )

    def test_rehashed_cross_kind_and_false_acceptance_records_are_rejected(self):
        mutations = [
            lambda v: v["initial"]["m6"]["identities"][0].update(kind="Bridge"),
            lambda v: v["initial"]["m6"]["identities"][0].update(
                authority="isolated_candidate"
            ),
            lambda v: v["initial"]["m6"]["door_identity"].update(
                manifest_sha256="b" * 64
            ),
            lambda v: v["initial"]["m6"].update(pool_pending=1),
            lambda v: v["initial"]["m6"].update(active_state_acceptance=True),
            lambda v: v["initial"]["records"][0]["state"].update(stored_water=1),
            lambda v: v["initial"]["records"][0]["parts"][0].update(mesh_role="rotor"),
            lambda v: v["initial"]["records"][0].update(root_translation_wu=[0, 0, 0]),
            lambda v: v["initial"]["m6"]["resources"].update(resident_mesh_count=True),
        ]
        for mutate in mutations:
            value = sidecar(True)
            mutate(value)
            with self.assertRaises(ValueError):
                oracle.validate_sidecar(
                    reseal(value),
                    Case("building-art-static", "small", "gpu", 20260920, 15, 0),
                )

    def test_matrix_has_adjacent_reversed_pairs_and_sequential_instrumentation(self):
        cases = m6.matrix()
        self.assertEqual(len(cases), 24)
        self.assertEqual(
            [case[0] for case in cases], ["capture"] * 12 + ["memory"] * 12
        )
        for index in range(0, len(cases), 2):
            left, right = cases[index : index + 2]
            self.assertEqual(left[:3], right[:3])
            self.assertEqual(left[3], "candidate" if left[2] == 2 else "legacy-control")
            self.assertNotEqual(left[3], right[3])
            for case in (left, right):
                argv = m6.command(Path("/registered-job"), case, "Intel")
                validate_arguments(build_parser().parse_args(argv[2:]))
                self.assertNotIn("--skip-build", argv)
                self.assertEqual(argv[argv.index("--repeat") + 1], "1")

    def test_aggregate_refuses_missing_repeat_binary_environment_noise_and_growth(self):
        limits = {
            "max_delta": dict.fromkeys(m6.METRICS, 100),
            "max_relative_mad": 0.1,
            "reason": "synthetic",
        }
        rows = [
            {
                "instrumentation": instrument,
                "size": size,
                "repeat": repeat,
                "mode": mode,
                "binary_sha256": instrument,
                "adapter": {"name": "test"},
                "window": {"width": 1280},
                "metrics": dict.fromkeys(m6.METRICS, 100),
            }
            for instrument, size, repeat, mode in m6.matrix()
        ]
        self.assertEqual(len(m6.aggregate(rows, m6.budget(limits))), 4)
        invalid = copy.deepcopy(rows)
        invalid[0]["binary_sha256"] = "different"
        with self.assertRaises(ValueError):
            m6.aggregate(invalid, limits)
        with self.assertRaises(ValueError):
            m6.aggregate(rows[:-1], limits)
        invalid = copy.deepcopy(rows)
        invalid[0]["window"] = {"width": 1920}
        with self.assertRaises(ValueError):
            m6.aggregate(invalid, limits)
        invalid = copy.deepcopy(rows)
        for row in invalid:
            if row["size"] == "medium":
                row["metrics"]["resident_mesh_count"] = 101
        with self.assertRaises(ValueError):
            m6.aggregate(invalid, limits)
        invalid = copy.deepcopy(rows)
        for row in invalid:
            row["metrics"]["p95_ms"] = (1, 100, 1000)[row["repeat"] - 1]
        with self.assertRaises(ValueError):
            m6.aggregate(invalid, limits)

    def test_budget_requires_all_independent_metrics_and_finite_limits(self):
        for value in (
            {},
            {"max_delta": {}, "max_relative_mad": 0.1, "reason": "missing"},
            {
                "max_delta": dict.fromkeys(m6.METRICS, float("nan")),
                "max_relative_mad": 0.1,
                "reason": "nonfinite",
            },
        ):
            with self.assertRaises(ValueError):
                m6.budget(value)


if __name__ == "__main__":
    unittest.main()
