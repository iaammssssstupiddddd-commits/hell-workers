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
    def m2_context(self, kind="Tank"):
        value, probe = self.context()
        value.update(kind=kind, variant="m2-art-context")
        value["context_geometry_sha256"] = art.M2_NUMERIC[kind][-1]
        value["context_mesh_verifier_sha256"] = art.m5_context.verifier_sha256()
        value["identity"] = {"authority": "art_preview", "kind": kind}
        names = ("body", "water" if kind == "Tank" else "rotor")
        value["context_parts"] = [{"name": name, "mesh_role": name, "translation_wu": [0., 0., 0.],
            "rotation_xyzw": [0., 0., 0., 1.], "scale": [1., 1., 1.]} for name in names]
        probe.update(variant=value["variant"], identity=value["identity"])
        fixture = probe["fixture"]
        fixture.update(variant=value["variant"], identity=value["identity"], souls=8 if kind == "Tank" else 10)
        fixture["geometry_contract_sha256"] = value["context_geometry_sha256"]
        fixture["rtt_camera"] = {"transform": {"rotation": [0., 0., 0., 1.]}}
        fixture["resources"] = {"scope": "exact-m2-resident-dependencies", "global_pool_evidence": False,
            "meshes": [{"role": name, "handle": "mesh:" + name} for name in names],
            "images": [{"role": role, "handle": "image:" + role} for role in ("albedo", "world_preview", "catalog")]}
        states = []
        for ordinal, record in enumerate(fixture["records"]):
            record["root_transform"] = {"translation": [ordinal * 160., 0., 0.], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            record["parts"] = []
            record["preview"]["image"] = "image:world_preview"
            for index, original in enumerate(value["context_parts"]):
                water = original["name"] == "water"
                local = {"translation": [0., 22. if ordinal >= 2 else 12., 0.] if water else [0., 0., 0.],
                    "rotation": original["rotation_xyzw"], "scale": original["scale"]}
                material = dict.fromkeys(("opaque", "lit", "emissive_black", "emissive_texture_absent"), True)
                material.update(color_srgba=[.58, .70, .73, 1.] if water and ordinal == 1 else [1., 1., 1., 1.],
                    roughness=1., reflectance=0., metallic=0.)
                record["parts"].append({"entity": 500 + ordinal * 2 + index, "role": original["mesh_role"],
                    "mesh": "mesh:" + original["mesh_role"],
                    "visible": not water or ordinal > 0, "transform": local, "material_values": material,
                    "global_matrix": art.foreground.product(art.foreground.matrix(record["root_transform"]), art.foreground.matrix(local))})
            souls = []
            for index, side in enumerate((-1., 1.)):
                entity = 600 + ordinal * 2 + index
                ground = [ordinal * 160., side * 12.]
                pose = {"translation": [ground[0], 17.6, -ground[1]], "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
                souls.append({"entity": entity, "side": side, "ground": ground, "meshes": [{
                    "entity": 800 + ordinal * 2 + index, "root": 800 + ordinal * 2 + index,
                    "visible": True, "owner": entity, "layers": [1], "global_matrix": art.foreground.matrix(pose),
                    "mesh": "Mesh(1)", "pose": pose, "root_pose": pose,
                    "material": {"kind": "standard", "handle": "Material(1)", "values": "actual material", "base_color": [1., 1., 1., 1.]}}]})
            states.append({"ordinal": ordinal, "owner": record["owner"], "souls": souls,
                "state": {"stored_water": (0, 25, 50, 50)[ordinal], "capacity": 50,
                    "water_items": list(range(1000 + ordinal * 100, 1000 + ordinal * 100 + (0, 25, 50, 50)[ordinal]))} if kind == "Tank"
                else {"refining": ordinal >= 2, "workers": [{"entity": 700 + ordinal, "mixer": record["owner"],
                    "working_on": record["owner"], "assignment": record["owner"], "target": record["owner"],
                    "work_type": "Refine", "phase": "Refining", "progress": 0.}] if ordinal >= 2 else []}})
        fixture["m2_state"] = {"idle_souls": 8, "source_workers": 0 if kind == "Tank" else 2,
            "ordinary_input_evidence": False, "lifecycle_evidence": False, "records": states}
        return value, probe

    def test_m2_context_has_separate_kind_mode_population_and_view_admission(self):
        for kind, population in (("Tank", 8), ("MudMixer", 10)):
            value, probe = self.m2_context(kind)
            art.validate_probe(probe, value, probe["nonce"])
            self.assertEqual(art.actor_count(value), population)
            for key, changed in (("kind", "RestArea"), ("mode", "candidate"), ("view", "overview"),
                ("view", "near"), ("spa_mask_page", 0), ("lamp_supply_target", 0)):
                with self.subTest(kind=kind, key=key), self.assertRaises(ValueError):
                    art.context_settings({**value, key: changed})

    def test_m2_context_rejects_wrong_water_state_pose_visibility_and_material(self):
        value, original = self.m2_context()
        for ordinal in range(4):
            for key, changed in (("visible", ordinal == 0), ("transform", {"translation": [0., 99., 0.]}),
                ("role", "body"), ("material_values", {"color_srgba": [1., 0., 0., 1.]})):
                probe = copy.deepcopy(original)
                probe["fixture"]["records"][ordinal]["parts"][1][key] = changed
                with self.subTest(ordinal=ordinal, key=key), self.assertRaises(ValueError):
                    art.validate_probe(probe, value, probe["nonce"])

    def test_m2_context_rejects_foreign_worker_soul_and_authority_claims(self):
        for kind in ("Tank", "MudMixer"):
            value, original = self.m2_context(kind)
            for key, changed in (("idle_souls", 6), ("source_workers", 8), ("ordinary_input_evidence", True),
                ("lifecycle_evidence", True)):
                probe = copy.deepcopy(original)
                probe["fixture"]["m2_state"][key] = changed
                with self.subTest(kind=kind, key=key), self.assertRaises(ValueError):
                    art.validate_probe(probe, value, probe["nonce"])
            for changed in (None, False, float("nan"), 100):
                probe = copy.deepcopy(original)
                probe["fixture"]["m2_state"]["records"][0]["souls"][0]["ground"][0] = changed
                with self.subTest(kind=kind, changed=changed), self.assertRaises(ValueError):
                    art.validate_probe(probe, value, probe["nonce"])

    def test_m2_context_rejects_corrupt_render_matrices_layers_and_receipt_bindings(self):
        for kind in ("Tank", "MudMixer"):
            value, original = self.m2_context(kind)
            for key, changed in (("global_matrix", [0.] * 16), ("global_matrix", [float("nan")] * 16),
                                 ("layers", [True]), ("material", {})):
                probe = copy.deepcopy(original)
                probe["fixture"]["m2_state"]["records"][0]["souls"][0]["meshes"][0][key] = changed
                with self.subTest(kind=kind, key=key), self.assertRaises(ValueError):
                    art.validate_probe(probe, value, probe["nonce"])
            probe = copy.deepcopy(original)
            probe["fixture"]["records"][0]["parts"][0]["global_matrix"] = [0.] * 16
            with self.assertRaises(ValueError):
                art.validate_probe(probe, value, probe["nonce"])
            probe = copy.deepcopy(original)
            probe["fixture"]["geometry_contract_sha256"] = "f" * 64
            with self.assertRaises(ValueError):
                art.validate_probe(probe, value, probe["nonce"])
        value, probe = self.m2_context("MudMixer")
        probe["fixture"]["m2_state"]["records"][2]["state"]["workers"][0]["target"] = 999
        with self.assertRaises(ValueError):
            art.validate_probe(probe, value, probe["nonce"])

    def test_m2_context_requires_complete_exact_resident_dependencies_not_global_counts(self):
        value, original = self.m2_context()
        for change in ("missing-image", "missing-mesh", "foreign-handle", "global-claim", "global-counts"):
            probe = copy.deepcopy(original)
            resources = probe["fixture"]["resources"]
            if change == "missing-image":
                resources["images"].pop()
            elif change == "missing-mesh":
                resources["meshes"].pop()
            elif change == "foreign-handle":
                resources["meshes"][0]["handle"] = "foreign"
            elif change == "global-claim":
                resources["global_pool_evidence"] = True
            else:
                probe["fixture"]["resources"] = {"images": 159, "meshes": 20, "materials": 10}
            with self.subTest(change=change), self.assertRaises(ValueError):
                art.validate_probe(probe, value, probe["nonce"])
        value, probe = self.m2_context("Tank")
        probe["fixture"]["m2_state"]["records"][2]["state"]["water_items"][0] = 100
        with self.assertRaises(ValueError):
            art.validate_probe(probe, value, probe["nonce"])

    def test_historical_gallery_without_variant_still_enters_exact_plan_check(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            frozen = self.source()
            manifest = {"identity": {"kind": "Tank", "authority": "art_preview"}}
            value = {"kind": "Tank", "mode": "feedback", "profile": art.PROFILE, "repo": str(root),
                "source_root": str(root / "candidate"), "job_root": str(root / "job"), "subject": frozen,
                "identity": manifest["identity"], "manifest_sha256": art.pipeline.digest(b"fixture")}
            with patch.object(art.native, "validate_repo", return_value=root), \
                    patch.object(art, "subject", return_value=frozen), \
                    patch.object(art.static, "check_root"), \
                    patch.object(art.pipeline, "load_set", return_value=(manifest, "fixture", None)):
                self.assertEqual(art.check_plan(value, root / "codec"), manifest)

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

    def test_gallery_dpi_contract_rejects_partial_invalid_and_context_values(self):
        self.assertEqual(art.scale_factor({}), 1.0)
        for dpi in art.GALLERY_SCALE_FACTORS:
            value = {"window_contract": art.WINDOW_CONTRACT, "scale_factor": dpi}
            command = art.game_command(Path("binary"), Path("job"), value)
            self.assertEqual(float(command[command.index("--perf-window-scale-factor") + 1]), dpi)
        for value in ({"scale_factor": 1}, {"window_contract": art.WINDOW_CONTRACT},
                      {"window_contract": "foreign", "scale_factor": 1},
                      *({"window_contract": art.WINDOW_CONTRACT, "scale_factor": n}
                        for n in (None, True, False, "1", 0, 1.25, 3, float("nan"), float("inf")))):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "window contract"):
                art.game_command(Path("binary"), Path("job"), value)
        for variant in ("rest-area-art-context", "m2-art-context", *art.M5_CONTEXT_VARIANTS):
            for dpi in (1.5, 2.0):
                with self.subTest(variant=variant, dpi=dpi), self.assertRaisesRegex(ValueError, "context"):
                    art.scale_factor({"window_contract": art.WINDOW_CONTRACT,
                                      "scale_factor": dpi, "variant": variant})

    def test_gallery_dpi_is_bound_into_the_game_session_with_legacy_compatibility(self):
        base = {"kind": "Tank", "mode": "candidate", "adapter": "Intel", "identity": {}}
        for dpi in art.GALLERY_SCALE_FACTORS:
            value = {**base, "window_contract": art.WINDOW_CONTRACT, "scale_factor": dpi}
            env = art.game_environment({}, Path("view"), value, "nonce", Path("job"))
            session = json.loads(env["HW_BUILDING_ART_SESSION"])
            self.assertEqual(session["window_contract"], art.WINDOW_CONTRACT)
            self.assertEqual(session["scale_factor"], dpi)
        old = json.loads(art.game_environment({}, Path("view"), base, "nonce", Path("job"))["HW_BUILDING_ART_SESSION"])
        self.assertNotIn("window_contract", old)
        self.assertNotIn("scale_factor", old)

    def test_gallery_quality_dpi_checks_actual_logical_physical_rtt_and_end_state(self):
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

            for dpi in art.GALLERY_SCALE_FACTORS:
                for quality, factor in (("high", 1), ("medium", .75), ("low", .5)):
                    row = dict(original)
                    for prefix in ("", "end_"):
                        row.update({prefix + "scale_factor": str(dpi),
                            prefix + "logical_width": str(1280 / dpi),
                            prefix + "logical_height": str(720 / dpi),
                            prefix + "rtt_quality": quality,
                            prefix + "scene_target_width": str(int(1280 * factor)),
                            prefix + "scene_target_height": str(int(720 * factor)),
                            prefix + "mask_target_width": str(int(1280 * factor)),
                            prefix + "mask_target_height": str(int(720 * factor)),
                            prefix + "target_scale_factor": str(dpi * factor)})
                    value = {"window_contract": art.WINDOW_CONTRACT, "scale_factor": dpi,
                             "quality": quality, "adapter": "Test GPU"}
                    write(row)
                    art.validate_window(root, value)
                    for field, bad in (("scale_factor", "1.25"), ("end_scale_factor", "1.25"),
                            ("logical_width", "999"), ("physical_width", "999"),
                            ("scene_target_width", "999"), ("mask_target_height", "999"),
                            ("target_scale_factor", "999")):
                        write({**row, field: bad})
                        with self.subTest(dpi=dpi, quality=quality, field=field), \
                                self.assertRaisesRegex(ValueError, "window"):
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
