"""Synthetic group-gallery guards; no fixtures imply native/art approval."""
import copy
import io
import itertools
from contextlib import redirect_stdout
from unittest.mock import patch, MagicMock
from types import SimpleNamespace
import json
from pathlib import Path
import tempfile
import unittest

from scripts import building_group_art_acceptance as gallery
from scripts import building_group_art_observation as observation
from scripts.tests.test_building_m6_acceptance import group_sidecar, reseal
from perf_tool.artifact_readers import building_m6 as oracle
from perf_tool.artifact_readers.building_art_static import read_building_art_static
from perf_tool.model import Case


def gallery_sidecar(group="m2"):
    value = group_sidecar(group, True)
    expected = oracle.gallery_records(group)
    old = {(r["kind"], r["ordinal"]): r for r in value["initial"]["records"]}
    records = []
    for logical in expected:
        row = old[(logical["kind"], logical["ordinal"])]
        row.update(logical)
        if "root_translation_wu" in row:
            row["root_translation_wu"] = [row["center"][0], 0., -row["center"][1]]
        row["gallery_owner"] = row.get("owner", 2000 + len(records))
        if row["kind"] in {*oracle.PARTS, "Door"}:
            row["gallery_geometry"] = [{"entity": 10000 + len(records) * 8 + i, "mesh": "synthetic",
                "mesh_role": p["mesh_role"], "visible": p["visible"], "local_pose": {
                    "translation": p["translation_wu"], "rotation": p["rotation_xyzw"], "scale": p["scale"]},
                "aabb_min": [0, 0, 0], "aabb_max": [1, 1, 1]}
                for i, p in enumerate(row.get("parts", [{"mesh_role": "door", "visible": True,
                    "translation_wu": [row["center"][0], 16., -row["center"][1]], "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}]))]
        records.append(row)
    value["initial"]["records"] = records
    value["contract_id"] = oracle.GROUP_GALLERY_CONTRACT
    value["camera_scale"] = 1.
    return reseal(value)


def projected_m2(group="m2"):
    frozen = gallery_sidecar(group)["initial"]
    for row in frozen["records"]:
        if row["kind"] == "SoulSpa" and "parts" in row:
            for index, (part, resident) in enumerate(zip(row["parts"], row["gallery_geometry"], strict=True)):
                part["translation_wu"] = [index * 20., 0., 0.]
                resident["local_pose"]["translation"] = part["translation_wu"][:]
    session = {"bindings": frozen["m6"]["group"], "quality": "high", "scale_factor": 2., "view": "normal"}
    rtt = [.00125, 0, 0, 0, 0, 0, .001, 0, 0, .00125, 0, 0, 1, -1, .5, 1]
    overlay = [1/960, 0, 0, 0, 0, 1/840, 0, 0, 0, 0, .001, 0, 0, 0, .5, 1]
    composite = [1920, 0, 0, 0, 0, 1680, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    value = {"window": {"physical": [1920, 1680], "logical": [960., 840.], "dpi": 2.},
        "rtt": {"physical": [1920., 1680.], "logical": [960., 840.], "target_scale_factor": 2., "quality": "high"},
        "main_clip_from_world": rtt, "rtt_clip_from_world": rtt, "overlay_clip_from_world": overlay,
        "composite_world": composite, "visible_ui_nodes": 0, "records": []}
    for original in frozen["records"]:
        if original["kind"] not in (*oracle.GROUP_TARGETS[group], "Door"):
            continue
        root = observation.m6.art.foreground.matrix({"translation": [original["center"][0], 0., -original["center"][1]],
                                                   "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]})
        row = {"kind": original["kind"], "ordinal": original["ordinal"], "owner": original["gallery_owner"], "shapes": []}
        all_points = []
        for part in original["gallery_geometry"]:
            part["aabb_max"] = [10, 10, 10]
            if not part["visible"]:
                continue
            local_matrix = observation.m6.art.foreground.matrix(part["local_pose"])
            matrix = local_matrix if original["kind"] == "Door" else observation.m6.art.foreground.product(root, local_matrix)
            points, texels = [], []
            for corner in itertools.product((0, 10), repeat=3):
                world = observation.transform(matrix, [*corner, 1])
                logical = observation.project(rtt, world[:3], [960, 840])
                uv = [logical[0] / 960, logical[1] / 840]
                composed = observation.transform(composite, [uv[0] - .5, .5 - uv[1], 0, 1])
                points.append(observation.project(overlay, composed[:3], [1920, 1680]))
                texels.append([uv[0] * 1920, uv[1] * 1680])
            row["shapes"].append({"type": "mesh3d", "entity": part["entity"], "mesh": part["mesh"],
                "aabb_min": part["aabb_min"], "aabb_max": part["aabb_max"], "world_matrix": matrix,
                "physical_corners": points, "rtt_texel_corners": texels})
            all_points.extend(points)
        row["physical_bounds"] = observation.bounds(all_points)
        value["records"].append(row)
    return value, session, frozen


class GroupGalleryTests(unittest.TestCase):
    def test_exact_matrix_and_connection_only_are_distinct(self):
        self.assertEqual(len(gallery.matrix()), 18)
        self.assertEqual(len(set(gallery.matrix())), 18)
        self.assertEqual(gallery.matrix(True), [("high", 2., "normal")])
        self.assertEqual({dpi for _, dpi, _ in gallery.matrix()}, {1., 1.5, 2.})
        self.assertEqual({view for _, _, view in gallery.matrix()}, {"normal", "far"})

    def test_group_gallery_oracle_is_separate_from_every_performance_reader(self):
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        for group in ("m2", "m3", "m5"):
            value = gallery_sidecar(group)
            oracle.validate_group_gallery_sidecar(value, case)
            for validator in (oracle.validate_group_sidecar, oracle.validate_pre_release_sidecar, oracle.validate_sidecar):
                with self.assertRaises(ValueError):
                    validator(value, case)
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "building_art_static.json").write_text(json.dumps(value))
                self.assertIsNone(read_building_art_static(root, expected_case=case)[0])
            for mutate in (lambda v: v.update(camera_scale=5.),
                           lambda v: v["initial"]["records"][0]["center"].__setitem__(0, 0.),
                           lambda v: v["initial"]["m6"].update(mode="legacy-control"),
                           lambda v: v["initial"]["records"].pop()):
                bad = copy.deepcopy(value)
                mutate(bad)
                with self.assertRaises((ValueError, KeyError)):
                    oracle.validate_group_gallery_sidecar(reseal(bad), case)

    def test_content_addressed_role_aliases_keep_exact_artifact_count(self):
        declaration = group_sidecar("m2", True)["initial"]["m6"]["group"]
        binding = declaration["candidates"][0]
        images = [i for i, path in enumerate(binding["paths"]) if path.endswith(".png")]
        binding["paths"][images[2]] = binding["paths"][images[1]]
        oracle.validate_group_declaration(declaration)
        binding["paths"].pop()
        with self.assertRaises(ValueError):
            oracle.validate_group_declaration(declaration)

    def test_gallery_reordering_does_not_change_performance_positions(self):
        m2 = oracle.gallery_records("m2")
        self.assertEqual(next(r["anchor"] for r in m2 if r["kind"] == "Door"), [8, 18])
        self.assertEqual(next(r["anchor"] for r in oracle.expected_records(4) if r["kind"] == "Door"), [8, 43])
        self.assertEqual({tuple(r["anchor"]) for r in m2}, {tuple(r["anchor"]) for r in oracle.expected_records(4)})

    def test_ack_publish_never_replaces_previous_answer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            gallery.publish_ack(root, {"nonce": "first"})
            with self.assertRaises(FileExistsError):
                gallery.publish_ack(root, {"nonce": "second"})
            self.assertEqual(json.loads((root / "capture-ack.json").read_bytes()), {"nonce": "first"})

    def test_child_environment_removes_foreign_admission_scopes(self):
        parent = {"HW_BUILDING_M6_MODE": "candidate", "HW_BUILDING_ASSET_RELEASES": "[]",
                  "HW_NATIVE_UI_ROOT": "/foreign", "HW_BRIDGE_ACCEPTANCE_PROBE": "foreign", "DISPLAY": ":0"}
        env = gallery.environment(parent, {"job_root": "/frozen", "adapter": "Intel"}, {"nonce": "a"})
        self.assertNotIn("HW_BUILDING_M6_MODE", env)
        self.assertNotIn("HW_BUILDING_ASSET_RELEASES", env)
        self.assertNotIn("HW_NATIVE_UI_ROOT", env)
        self.assertNotIn("HW_BRIDGE_ACCEPTANCE_PROBE", env)
        self.assertEqual(env["DISPLAY"], ":0")
        self.assertEqual(json.loads(env[gallery.ENV]), {"nonce": "a"})
        self.assertEqual(parent["HW_BUILDING_M6_MODE"], "candidate")

    def test_plan_is_bound_before_admission_and_replacement_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "plan.json"
            value = {"repo": "/frozen", "job_root": "/frozen/target/native-acceptance/gallery"}
            path.write_bytes(gallery.pipeline.canonical(value))
            with patch("sys.argv", ["gallery", "run", "--plan", str(path)]), \
                 patch.object(gallery, "run", return_value={}) as run, redirect_stdout(io.StringIO()):
                gallery.main()
            args = run.call_args.args[0]
            self.assertEqual(args.frozen_plan, value)
            self.assertEqual(args.repo, Path("/frozen"))
            with patch.dict("os.environ", {"HW_NATIVE_ACCEPTANCE_LAUNCHED": "1"}), \
                 patch.object(gallery, "check") as check:
                args.frozen_plan = {**value, "connection_only": True}
                with self.assertRaisesRegex(ValueError, "plan changed across activity"):
                    gallery.run.__wrapped__(args)
                check.assert_not_called()

    def test_json_artifacts_reject_alias_keys_nonfinite_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "probe.json"
            for text in ('{"pid":1,"pid":2}', '{"pid":NaN}', '[]'):
                path.write_text(text)
                with self.assertRaises(ValueError):
                    gallery.read_artifact(path)
            path.write_text('{"pid":1}')
            alias = Path(directory) / "alias.json"
            alias.symlink_to(path)
            with self.assertRaises(ValueError):
                gallery.read_artifact(alias)

    def test_projection_independently_recomputes_column_major_matrices(self):
        matrix = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 10, 20, 30, 1]
        self.assertEqual(observation.transform(matrix, [1, 2, 3, 1]), [11, 22, 33, 1])
        identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
        self.assertEqual(observation.project(identity, [0, 0, .5], [1920, 1680]), [960, 840])
        with self.assertRaises(ValueError):
            observation.project(identity, [0, 0, -1], [1920, 1680])
        with self.assertRaises(ValueError):
            observation.transform([float("nan")] * 16, [1, 2, 3, 1])

    def test_projection_binds_all_group_consumers_and_rejects_consistent_foreign_shapes(self):
        value, session, frozen = projected_m2()
        observation.snapshot(value, session, frozen, Path("/unused"))
        mutations = (
            lambda v: v["records"][0].update(owner=1),
            lambda v: v["records"][0]["shapes"][0].update(entity=1),
            lambda v: v["records"][0]["shapes"][0].update(mesh="foreign"),
            lambda v: v["records"][0]["shapes"][0]["aabb_max"].__setitem__(0, 9),
            lambda v: v["records"][0]["shapes"][0]["world_matrix"].__setitem__(12, 0),
            lambda v: v["records"][0]["shapes"][0]["physical_corners"][0].__setitem__(0, 100),
            lambda v: v["records"].pop(),
            lambda v: v.update(visible_ui_nodes=1),
            lambda v: v["rtt"].update(quality="low"),
            lambda v: v["window"]["physical"].__setitem__(0, 1920.0),
            lambda v: v["window"].update(dpi=True),
        )
        for mutate in mutations:
            bad = copy.deepcopy(value)
            mutate(bad)
            with self.assertRaises(ValueError):
                observation.snapshot(bad, session, frozen, Path("/unused"))

    def test_self_consistent_spa_slot_duplication_cannot_consume_one_part_twice(self):
        value, session, frozen = projected_m2("m3")
        observation.snapshot(value, session, frozen, Path("/unused"))
        row = next(r for r in value["records"] if r["kind"] == "SoulSpa")
        source = next(r for r in frozen["records"] if r["kind"] == "SoulSpa")
        # Distinct entities and a self-consistently reprojected duplicate slot.
        source["gallery_geometry"][2]["local_pose"] = copy.deepcopy(source["gallery_geometry"][1]["local_pose"])
        replacement = copy.deepcopy(row["shapes"][1])
        replacement["entity"] = row["shapes"][2]["entity"]
        row["shapes"][2] = replacement
        row["physical_bounds"] = observation.bounds([p for shape in row["shapes"] for p in shape["physical_corners"]])
        with self.assertRaisesRegex(ValueError, "unconsumed authored"):
            observation.snapshot(value, session, frozen, Path("/unused"))

    def test_final_schema_and_pid_aliases_are_not_rust_integer_wire_values(self):
        value, settings, frozen = projected_m2()
        settings["nonce"] = "a" * 32
        probe = {"schema_version": 1, "profile": observation.PROFILE, "status": "ready", "nonce": settings["nonce"],
                 "pid": 123, "session": settings, "stable_frames": 60, "ready_at_unix_ms": 1000,
                 "fixture": frozen, "snapshot": value, "accepted": False, "performance_evidence": False, "promotion_authority": False}
        final = {"schema_version": 1, "profile": observation.PROFILE, "status": "complete", "nonce": settings["nonce"],
                 "pid": 123, "stable_frames": 61, "completed_at_unix_ms": 2000,
                 "probe_sha256": "b" * 64, "ack_sha256": "c" * 64, "ack": {},
                 "fixture": frozen, "snapshot": value, "accepted": False, "performance_evidence": False, "promotion_authority": False}
        # This test isolates the wire boundary, before native fixture validation.
        with patch.object(observation, "fixture", return_value=frozen), patch.object(observation.m6, "validate_authored_parts"):
            observation.validate(probe, final, settings, {"group_declaration": frozen["m6"]["group"]}, Path("/unused"))
            for field, alias in (("schema_version", True), ("schema_version", 1.), ("pid", 123.),
                                 ("stable_frames", 61.), ("completed_at_unix_ms", 2000.)):
                bad = copy.deepcopy(final)
                bad[field] = alias
                with self.assertRaises(ValueError):
                    observation.validate(probe, bad, settings, {"group_declaration": frozen["m6"]["group"]}, Path("/unused"))

    def test_clipped_or_tiny_geometry_cannot_pass_using_visibility(self):
        self.assertEqual(observation.area([[100, 100], [108, 108]], "normal"), [100, 100, 108, 108])
        for points, view in (([[15, 100], [100, 200]], "normal"),
                             ([[100, 100], [103, 200]], "normal"),
                             ([[100, 100], [104, 103]], "far"),
                             ([[100, 100], [1905, 200]], "normal")):
            with self.assertRaises(ValueError):
                observation.area(points, view)


class GroupWindowReadbackTests(unittest.TestCase):
    def test_owner_is_checked_before_opening_the_display(self):
        with patch.object(gallery.window_probe.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout="_NET_WM_PID(CARDINAL) = 999")), \
             patch.object(gallery.window_probe.ctypes, "CDLL") as library:
            with self.assertRaisesRegex(ValueError, "owner changed"):
                gallery.window_probe.client_geometry("0x123", 123)
            library.assert_not_called()
        for window, pid in (("0x0", 123), ("desktop", 123), ("0x123", True)):
            with self.assertRaises(ValueError):
                gallery.window_probe.client_geometry(window, pid)

    def test_readback_keeps_actual_size_instead_of_substituting_requested_size(self):
        library = MagicMock()
        library.XOpenDisplay.return_value = 42
        def geometry(*args):
            args[5]._obj.value = 1920
            args[6]._obj.value = 1624
            args[8]._obj.value = 24
            return 1
        library.XGetGeometry.side_effect = geometry
        with patch.object(gallery.window_probe.subprocess, "run", return_value=SimpleNamespace(returncode=0, stdout="_NET_WM_PID(CARDINAL) = 123")), \
             patch.object(gallery.window_probe.ctypes, "CDLL", return_value=library):
            value = gallery.window_probe.client_geometry("0x123", 123)
        self.assertEqual(value["physical"], [1920, 1624])
        library.XCloseDisplay.assert_called_once_with(42)
        for mutate in (lambda v: v.update(pid=123.), lambda v: v["physical"].__setitem__(0, True),
                       lambda v: v["physical"].__setitem__(1, 0), lambda v: v.update(window_id="0x999")):
            bad = copy.deepcopy(value)
            mutate(bad)
            with self.assertRaises(ValueError):
                gallery.window_probe.validate(bad, "0x123", 123)

    def test_geometry_history_is_bounded_and_pid_nonce_bound_without_input(self):
        geometry = {"window_id": "0x123", "pid": 123, "physical": [1920, 1624], "depth": 24, "wm_properties": "_NET_WM_PID(CARDINAL) = 123"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(gallery.native, "x11_client_windows_for_process_tree", return_value=[("0x123", 123)]), \
                 patch.object(gallery.native, "run_bounded_capture_tool", return_value=SimpleNamespace(returncode=0, stdout=json.dumps(geometry), stderr="")):
                gallery.observe_window(root, 123, "a" * 32)
                gallery.observe_window(root, 123, "a" * 32)
                value = gallery.read_artifact(root / "window-observations.json")
                self.assertEqual(len(value["observations"]), 1)
                self.assertEqual(value["observations"][0]["geometry"], geometry)
                with self.assertRaisesRegex(ValueError, "foreign window"):
                    gallery.observe_window(root, 123, "b" * 32)
            with patch.object(gallery.native, "x11_client_windows_for_process_tree", return_value=[("0x123", 999)]), \
                 patch.object(gallery.native, "run_bounded_capture_tool") as call:
                with self.assertRaises(ValueError):
                    gallery.observe_window(root, 123, "a" * 32)
                call.assert_not_called()


if __name__ == "__main__":
    unittest.main()
