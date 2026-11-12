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
