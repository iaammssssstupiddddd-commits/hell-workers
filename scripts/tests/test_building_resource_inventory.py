from __future__ import annotations

import copy
import unittest

from scripts import building_resource_inventory as inventory


def snapshot():
    return {
        "schema_version": 1,
        "contract_id": "building-resident-payload-inventory-v1",
        "mesh_count": 1,
        "image_count": 1,
        "material_count": 1,
        "mesh_vertex_index_payload_bytes": 42,
        "image_cpu_payload_bytes": 0,
        "images_without_cpu_backing": 1,
        "declared_texture_payload_bytes_estimate": 128,
        "material_shallow_size": 64,
        "material_shallow_bytes": 64,
        "meshes": [{"id": "mesh-1", "path": None, "vertex_index_payload_bytes": 42}],
        "images": [
            {
                "id": "image-1",
                "path": "textures/a.png",
                "cpu_payload_bytes": None,
                "declared_texture_payload_bytes_estimate": 128,
                "format": "Rgba8Unorm",
                "dimension": "D2",
                "extent": [8, 4, 1],
                "mip_levels": 1,
                "samples": 1,
            }
        ],
        "materials": [{"id": "material-1", "path": None}],
    }


def owned_snapshot(size="small", *, shared=False):
    value = snapshot()
    count = {"small": 7, "medium": 25}[size]
    value.update(schema_version=2, contract_id="building-resident-payload-inventory-v2", mesh_uses=[])
    for n in range(1 if shared else count):
        value["meshes"].append({"id": f"boundary-{n}", "path": None, "vertex_index_payload_bytes": 152})
    for n in range(count):
        value["mesh_uses"].append({"entity": n, "dimension": "2d", "mesh": f"boundary-{0 if shared else n}",
                                   "site_yard_boundary": True})
    value["mesh_count"] = len(value["meshes"])
    value["mesh_vertex_index_payload_bytes"] = sum(m["vertex_index_payload_bytes"] for m in value["meshes"])
    return value


def boundary():
    return {
        "schema_version": 1,
        "contract_id": "building-resource-boundaries-v1",
        "initial_boundary": "ready-before-warmup",
        "final_boundary": "post-measure-flush",
        "gpu_allocation_measurement": False,
        "initial": snapshot(),
        "final": snapshot(),
    }


class ResourceInventoryTests(unittest.TestCase):
    def test_historical_duplicate_geometry_is_explicit_failure_not_current_plateau(self):
        small, medium = owned_snapshot(), owned_snapshot("medium")
        for data, size in [(small, "small"), (medium, "medium")]:
            inventory.require_boundary_scale(data, size)
        with self.assertRaises(ValueError):
            inventory.plateau(small, medium)
        result = inventory.legacy_boundary_growth(small, medium)
        self.assertFalse(result["global_shared_mesh_plateau"])
        self.assertFalse(result["accepted"])
        self.assertEqual(result["boundary_mesh_bytes"], [1064, 3800])
        inventory.plateau(owned_snapshot(shared=True), owned_snapshot("medium", shared=True))
        with self.assertRaises(ValueError):
            inventory.legacy_boundary_growth(owned_snapshot(shared=True), medium)

    def test_unknown_boundary_owners_foreign_consumers_and_payload_growth_fail(self):
        for change in ("missing", "duplicate", "foreign", "payload", "shared-growth", "scale"):
            small, medium = owned_snapshot(), owned_snapshot("medium")
            if change == "missing":
                medium["mesh_uses"].pop()
            elif change == "duplicate":
                medium["mesh_uses"].append(medium["mesh_uses"][0])
            elif change == "foreign":
                medium["mesh_uses"].append({"entity": 100, "dimension": "3d", "mesh": "boundary-0", "site_yard_boundary": False})
            elif change == "payload":
                medium["meshes"][-1]["vertex_index_payload_bytes"] += 1
                medium["mesh_vertex_index_payload_bytes"] += 1
            elif change == "shared-growth":
                medium["meshes"][0]["vertex_index_payload_bytes"] += 1
                medium["mesh_vertex_index_payload_bytes"] += 1
            else:
                medium = owned_snapshot()
            with self.subTest(change=change), self.assertRaises(ValueError):
                inventory.require_boundary_scale(medium, "medium")
                inventory.legacy_boundary_growth(small, medium)

    def test_packed_rgb9e5_uses_the_wgpu_four_byte_texel_for_a_real_3d_lut(self):
        image = snapshot()["images"][0]
        image.update(format="Rgb9e5Ufloat", dimension="D3", extent=[48, 48, 48],
                     cpu_payload_bytes=442368, declared_texture_payload_bytes_estimate=442368)
        self.assertEqual(inventory.texture_estimate(image), 442368)
        image["mip_levels"] = 2
        self.assertEqual(inventory.texture_estimate(image), (48**3 + 24**3)*4)
        image["format"] = "UnknownPackedFormat"
        with self.assertRaises(ValueError):
            inventory.texture_estimate(image)
    def test_cpu_none_is_explicit_and_only_descriptor_is_estimated(self):
        value = inventory.boundaries(boundary())
        self.assertEqual(value["image_cpu_payload_bytes"], 0)
        self.assertEqual(value["images_without_cpu_backing"], 1)
        self.assertEqual(value["declared_texture_payload_bytes_estimate"], 128)

    def test_boundary_drift_unknown_gpu_claim_or_missing_final_is_rejected(self):
        for change in (
            "missing-final",
            "gpu",
            "boundary",
            "id",
            "bytes",
            "bool-schema",
        ):
            data = boundary()
            if change == "missing-final":
                del data["final"]
            elif change == "gpu":
                data["gpu_allocation_measurement"] = True
            elif change == "boundary":
                data["final_boundary"] = "initial-clone"
            elif change == "id":
                data["final"]["meshes"][0]["id"] = "replacement"
            elif change == "bool-schema":
                data["schema_version"] = True
            else:
                data["final"]["mesh_vertex_index_payload_bytes"] += 1
                data["final"]["meshes"][0]["vertex_index_payload_bytes"] += 1
            with self.subTest(change=change), self.assertRaises(ValueError):
                inventory.boundaries(data)

    def test_raw_counts_types_none_totals_duplicates_and_unknown_formats_fail(self):
        for change in (
            "bool",
            "negative",
            "count",
            "duplicate",
            "none",
            "format",
            "estimate",
            "extent",
            "extra",
        ):
            data = snapshot()
            if change == "bool":
                data["mesh_count"] = True
            elif change == "negative":
                data["meshes"][0]["vertex_index_payload_bytes"] = -1
            elif change == "count":
                data["image_count"] = 2
            elif change == "duplicate":
                data["meshes"] *= 2
                data["mesh_count"] = 2
            elif change == "none":
                data["images_without_cpu_backing"] = 0
            elif change == "format":
                data["images"][0]["format"] = "Depth24Plus"
            elif change == "estimate":
                data["images"][0]["declared_texture_payload_bytes_estimate"] += 1
            elif change == "extent":
                data["images"][0]["extent"][0] = False
            else:
                data["images"][0]["gpu_allocated_bytes"] = 128
            with self.subTest(change=change), self.assertRaises(ValueError):
                inventory.validate(data)

    def test_layers_mips_msaa_and_3d_shrink_are_independently_derived(self):
        image = snapshot()["images"][0]
        image.update(extent=[8, 4, 2], mip_levels=2, samples=4)
        self.assertEqual(inventory.texture_estimate(image), (8 * 4 + 4 * 2) * 2 * 4 * 4)
        image.update(dimension="D3", samples=1)
        self.assertEqual(inventory.texture_estimate(image), (8 * 4 * 2 + 4 * 2) * 4)
        for key, value in [("mip_levels", 5), ("samples", 3), ("samples", 4)]:
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                inventory.texture_estimate(image | {key: value})

    def test_cross_process_handle_ids_are_not_semantic_resource_identity(self):
        small = snapshot()
        medium = copy.deepcopy(small)
        for kind in ("meshes", "images", "materials"):
            medium[kind][0]["id"] += "-other-process"
        inventory.plateau(small, medium)
        medium["images"][0]["path"] = "textures/different.png"
        with self.assertRaises(ValueError):
            inventory.plateau(small, medium)
        medium = copy.deepcopy(small)
        medium["meshes"][0]["vertex_index_payload_bytes"] += 1
        medium["mesh_vertex_index_payload_bytes"] += 1
        with self.assertRaises(ValueError):
            inventory.plateau(small, medium)


if __name__ == "__main__":
    unittest.main()
