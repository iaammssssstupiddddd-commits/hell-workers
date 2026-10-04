"""Shader-unused Light Field must be proved by actual bound Vulkan descriptors."""

from copy import deepcopy
from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from perf_tool import renderdoc_extract as extract
from perf_tool.renderdoc_foundation import validate_light_field_binding_evidence


class BoundLightFieldTests(unittest.TestCase):
    def setUp(self):
        self.rd = NS(
            ResourceId=NS(Null=lambda: "ResourceId::0"),
            DescriptorRange=NS,
            DescriptorCategory=NS(ReadOnlyResource=3),
            ShaderStageMask=NS(Fragment=16),
            CompType=NS(UNorm=1),
            Subresource=NS,
        )
        self.field = "ResourceId::415"
        self.label = "hell-workers-indoor-light-field"
        self.material = NS(descriptorSetResourceId="ResourceId::2191", descriptorBufferIndex=-1)
        self.store = NS(resourceId=self.material.descriptorSetResourceId,
                        firstDescriptorOffset=5, descriptorByteSize=2, descriptorCount=1)
        self.descriptors = [NS(resource=self.field, view="ResourceId::418")]
        self.locations = [NS(category=3, stageMask=17, fixedBindNumber=46)]
        self.resources = [NS(resourceId=self.field, name=self.label)]
        self.textures = [NS(resourceId=self.field, width=100, height=100)]
        self.pixel = [1.0, 1.0, 1.0, 1.0]
        self.queried = []
        self.controller = NS(
            GetVulkanPipelineState=lambda: NS(graphics=NS(descriptorSets=[None] * 3 + [self.material])),
            GetDescriptors=self.get_descriptors,
            GetDescriptorLocations=lambda *_: self.locations,
            GetResources=lambda: self.resources,
            GetTextures=lambda: self.textures,
            SetFrameEvent=lambda *_: None,
            PickPixel=lambda *_: NS(floatValue=self.pixel),
        )
        self.checkpoint = {"gpu_light_field": {
            "field_texture_label": self.label, "field_width": 100, "field_height": 100,
            "pixel_probe_x": 19, "pixel_probe_y": 25,
            "pixel_probe_expected_rgba": [255, 255, 255, 255],
        }}

    def get_descriptors(self, store_id, ranges):
        self.queried.append((store_id, vars(ranges[0])))
        return self.descriptors

    def bound(self):
        return extract._bound_light_field_descriptors(
            self.rd, self.controller, {self.store.resourceId: self.store}, self.field)

    def probe(self):
        bindings = [{"category": "bound:read-only", "resource_id": resource}
                    for _, _, resource in self.bound()]
        return extract._p06_gpu_light_field_pixel_probe(
            self.rd, self.controller, self.checkpoint, bindings, 500)

    def test_unused_texture_uses_bound_store_and_physical_binding(self):
        # No shader reflection API is provided: the variable was eliminated.
        self.assertEqual(self.bound(), [(3, 46, self.field)])
        self.assertEqual(self.queried[0], (self.store.resourceId,
                         {"offset": 5, "descriptorSize": 2, "count": 1}))
        self.assertTrue(self.probe()["passed"])
        self.assertEqual(self.probe()["captured_name"], self.label)

    def test_named_white_image_without_actual_binding_is_rejected(self):
        self.descriptors[0].resource = "ResourceId::295"
        with self.assertRaisesRegex(RuntimeError, "dimensions and pixel"):
            self.probe()

    def test_other_white_texture_cannot_replace_labelled_image(self):
        self.textures[0].resourceId = "ResourceId::295"
        with self.assertRaisesRegex(RuntimeError, "dimensions and pixel"):
            self.probe()

    def test_label_missing_or_ambiguous_is_rejected(self):
        for resources in ([], self.resources + [NS(resourceId="ResourceId::416", name=self.label)]):
            with self.subTest(resources=resources):
                self.resources = resources
                with self.assertRaisesRegex(RuntimeError, "exactly one resource"):
                    self.probe()

    def test_bound_image_pixel_mismatch_is_rejected(self):
        self.pixel = [0.0, 0.0, 0.0, 1.0]
        with self.assertRaisesRegex(RuntimeError, "dimensions and pixel"):
            self.probe()

    def test_invalid_material_descriptor_is_rejected(self):
        for field, value in (("category", 4), ("stageMask", 1),
                             ("fixedBindNumber", -1), ("fixedBindNumber", True)):
            with self.subTest(field=field, value=value):
                original = deepcopy(self.locations[0])
                setattr(self.locations[0], field, value)
                with self.assertRaisesRegex(RuntimeError, "descriptor is invalid"):
                    self.bound()
                self.locations[0] = original
        self.descriptors[0].view = "ResourceId::0"
        with self.assertRaisesRegex(RuntimeError, "descriptor is invalid"):
            self.bound()

    def test_missing_or_truncated_store_is_rejected(self):
        self.material.descriptorSetResourceId = "ResourceId::999"
        with self.assertRaisesRegex(RuntimeError, "supported descriptor store"):
            self.bound()
        self.material.descriptorSetResourceId = self.store.resourceId
        self.locations.clear()
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            self.bound()

    def test_duplicate_field_descriptors_are_rejected(self):
        self.store.descriptorCount = 2
        self.descriptors *= 2
        self.locations *= 2
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            self.bound()

    def test_offline_verifier_rejects_unrelated_or_changed_raw_binding_evidence(self):
        probe = {"resource_id": self.field, "binding_count": 1}
        row = {"category": "bound:read-only", "resource_id": self.field,
               "fixed_bind_set_or_space": 3}
        validate_light_field_binding_evidence(probe, [row])
        for changes in ({"resource_id": "ResourceId::999"},
                        {"category": "fragment:sampler"},
                        {"fixed_bind_set_or_space": 2}):
            with self.subTest(changes=changes), self.assertRaisesRegex(RuntimeError, "raw material bindings"):
                validate_light_field_binding_evidence(probe, [{**row, **changes}])
        with self.assertRaisesRegex(RuntimeError, "raw material bindings"):
            validate_light_field_binding_evidence({**probe, "binding_count": 2}, [row])


if __name__ == "__main__":
    unittest.main()
