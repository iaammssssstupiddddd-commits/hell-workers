import copy
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from perf_tool.artifact_readers.building_art_active import canonical, read_building_art_active
from perf_tool.artifact_readers.building_art_static import expected_records
from perf_tool.artifact_readers.workload import read_workload_sidecars
from perf_tool.artifacts import measurement_duration_clock
from perf_tool.arguments import build_parser, validate_arguments
from perf_tool.model import Case
import building_art_active_acceptance as acceptance


def sidecar(copies=4):
    initial = {"records": expected_records(copies), "target_count": copies * 9,
               "target_structural_roots": copies * 5, "target_foreground_owners": copies * 4,
               "target_active_unique_meshes": 2, "souls": copies // 4 * 29, "completion_effects": 0}
    return {"schema_version": 1, "contract_id": "building-art-active-nine-v1", "initialized": True,
            "failed": False, "frames": 3600, "seconds": 60.01, "particles_max": 10, "particles_sum": 100,
            "ui_particles_max": 10, "ui_particles_sum": 100,
            "initial": initial, "layout_sha256": hashlib.sha256(canonical(initial)).hexdigest(),
            "lanes": [{"ordinal": i, "produced": 15, "delivered": 5, "sand_in": 3, "rock_in": 3,
                       "water_in": 5, "frames": 3600, "active_frames": 360,
                       "produced_per_20s": [5, 5, 5]} for i in range(copies) if i % 4 >= 2]}


class BuildingArtActiveTests(unittest.TestCase):
    def check_value(self, value, size="small"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "building_art_active.json").write_text(json.dumps(value))
            return read_building_art_active(root, expected_case=Case("building-art-active", size, "gpu", 20260920,
                29 if size == "small" else 116, 2 if size == "small" else 8))

    def test_exact_inventory_and_virtual_clock(self):
        for size, copies in (("small", 4), ("medium", 16)):
            self.assertEqual(self.check_value(sidecar(copies), size)[1], [])
        self.assertEqual(measurement_duration_clock("building-art-active"), ("virtual", False))
        self.assertEqual(measurement_duration_clock("building-art-static"), ("real", True))

    def test_idle_one_shot_starved_and_missing_output_rejected(self):
        for field, value in (("produced", 5), ("delivered", 0), ("sand_in", 0), ("rock_in", 0),
                             ("water_in", 0), ("active_frames", 0), ("active_frames", 3600),
                             ("produced_per_20s", [15, 0, 0]), ("produced_per_20s", [5, 5, 10])):
            with self.subTest(field=field, value=value):
                data = sidecar()
                data["lanes"][0][field] = value
                self.assertTrue(self.check_value(data)[1])

    def test_lost_coverage_paused_short_and_particle_free_rejected(self):
        for field, value in (("frames", 0), ("seconds", 0), ("seconds", 59.9), ("seconds", float("nan")),
                             ("failed", True), ("initialized", False), ("particles_max", 0),
                             ("particles_sum", 0), ("particles_max", 97), ("ui_particles_max", 0),
                             ("ui_particles_max", 129), ("ui_particles_sum", 0), ("schema_version", True)):
            data = sidecar()
            data[field] = value
            self.assertTrue(self.check_value(data)[1])
        for field, value in (("ordinal", True), ("frames", 3599), ("delivered", True)):
            data = sidecar()
            data["lanes"][0][field] = value
            self.assertTrue(self.check_value(data)[1])

    def test_owner_inventory_and_rehashed_wrong_layout_rejected(self):
        for change in (lambda rows: rows.pop(), lambda rows: rows.reverse(),
                       lambda rows: rows.append(copy.deepcopy(rows[0]))):
            data = sidecar()
            change(data["lanes"])
            self.assertTrue(self.check_value(data)[1])
        data = sidecar()
        data["initial"]["records"][0]["kind"] = "Bridge"
        data["layout_sha256"] = hashlib.sha256(canonical(data["initial"])).hexdigest()
        self.assertTrue(self.check_value(data)[1])

    def test_missing_malformed_and_cross_workload_rejected(self):
        case = Case("building-art-active", "small", "gpu", 20260920, 29, 2)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertTrue(read_building_art_active(root, expected_case=case)[1])
            for data in ('{"frames":1,"frames":2}', '{"seconds":NaN}', '[]'):
                (root / "building_art_active.json").write_text(data)
                self.assertTrue(read_building_art_active(root, expected_case=case)[1])
            result = read_workload_sidecars(root, expected_case=Case("gather", "small", "gpu", 1, 1, 1),
                capture_kind="frame-time", expected_contract=None, expected_stage=None, expected_lane=None)
            self.assertTrue(any("forbidden" in reason for reason in result.reasons))

    def test_formal_helper_matrix_is_valid_and_population_changes_rejected(self):
        for instrumentation, size, souls in acceptance.MATRIX:
            command = acceptance.command(Path("/unused"), instrumentation, size, souls, "Intel")
            args = build_parser().parse_args(command[2:])
            validate_arguments(args)
            args.familiars = 0
            with self.assertRaises(ValueError):
                validate_arguments(args)
