"""Independent raw rejection tests; synthetic data grants no native authority."""
import copy
from dataclasses import replace
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from scripts import building_resident_observation as resident
from scripts import building_m6_acceptance as m6
from scripts import building_cumulative_acceptance as cumulative
from scripts.tests.test_building_m6_acceptance import sidecar, reseal, group_sidecar
from scripts.tests.test_building_group_art_acceptance import gallery_sidecar
from perf_tool.artifact_readers import building_m6 as reader
from perf_tool.model import Case


def fixture(kind, ordinal=0, illustrated=True):
    row = next(r for r in sidecar(True)["initial"]["records"] if r["kind"] == kind and r["ordinal"] == ordinal)
    parts = [{"name": p["resident"]["name"], **{k: copy.deepcopy(p[k]) for k in
              ("mesh_role", "translation_wu", "rotation_xyzw", "scale")}} for p in row["parts"]]
    manifest = {"identity": copy.deepcopy(row["parts"][0]["resident"]["root_identity"]), "parts": parts}
    if illustrated and kind == "Tank":
        manifest.update(body_paint_projection={"method": "world_preview_v1", "matte_linear_rgb": [.1, .2, .3]},
                        world_preview={"canvas_wu": [64., 64.], "anchor_px": [128., 208.],
                                       "canvas_px": [256, 256], "image_role": "world_preview"})
        facts = row["parts"][0]["resident"]
        facts["body_paint"].update(mode=3., canvas=[64., 64., .5, .8125], matte=[.1, .2, .3, 1.])
        facts["texture"] = facts["images"]["world_preview"]
    elif illustrated:
        manifest["surface_paint"] = {"method": "uv_ink_v1"}
        for part in row["parts"]:
            part["resident"]["body_paint"]["mode"] = 4.
    return manifest, row


class ResidentTests(unittest.TestCase):
    def test_old_and_new_manifests_bind_actual_named_parts_for_all_four_kinds(self):
        for kind in resident.ROLES:
            for illustrated in (False, True):
                for ordinal in range(4):
                    manifest, row = fixture(kind, ordinal, illustrated)
                    resident.validate_record(manifest, row, manifest["parts"])

    def test_missing_foreign_duplicate_or_detached_residents_fail(self):
        manifest, row = fixture("SoulSpa", 3)
        mutations = (
            lambda r: r["parts"][0].pop("resident"),
            lambda r: r["parts"][0]["resident"].update(extra=True),
            lambda r: r["parts"][0]["resident"].update(owner=1),
            lambda r: r["parts"][0]["resident"].update(parent=1),
            lambda r: r["parts"][0]["resident"].update(entity=r["owner"]),
            lambda r: r["parts"][0]["resident"]["root_identity"].update(generation=2),
            lambda r: r["parts"][0]["resident"]["root_pose"]["translation"].__setitem__(0, 0.),
            lambda r: r["parts"][1]["resident"].update(name="slot1"),
            lambda r: r["parts"][1]["resident"].update(entity=r["parts"][0]["resident"]["entity"]),
            lambda r: r["parts"][0]["resident"].update(mesh="foreign"),
            lambda r: r["parts"][0]["resident"].update(texture="foreign"),
            lambda r: r["parts"][0]["resident"]["images"].update(albedo="foreign"),
            lambda r: r["parts"][0]["resident"]["body_paint"].update(mode=True),
            lambda r: r["parts"][0]["resident"]["body_paint"]["canvas"].__setitem__(0, float("nan")),
        )
        for mutate in mutations:
            bad = copy.deepcopy(row)
            mutate(bad)
            with self.assertRaises(ValueError):
                resident.validate_record(manifest, bad, manifest["parts"])

    def test_paint_contract_rejects_missing_modes_atlas_and_projection_uniforms(self):
        for kind in resident.ROLES:
            manifest, row = fixture(kind)
            for field, value in (("mode", 0.), ("ink_base", False)) if kind != "Tank" else (("mode", 0.82), ("standard", False), ("canvas", [1.] * 4), ("matte", [1.] * 4)):
                with self.subTest(kind=kind, field=field):
                    bad = copy.deepcopy(row)
                    bad["parts"][0]["resident"]["body_paint"][field] = value
                    with self.assertRaises(ValueError):
                        resident.validate_record(manifest, bad, manifest["parts"])
            bad = copy.deepcopy(row)
            bad["parts"][0]["resident"]["texture"] = "foreign"
            with self.assertRaises(ValueError):
                resident.validate_record(manifest, bad, manifest["parts"])
            with self.assertRaises(ValueError):
                resident.validate_record({k: v for k, v in manifest.items() if k not in {"body_paint_projection", "surface_paint"}}, row, manifest["parts"])
        manifest, row = fixture("Tank")
        row["parts"][1]["resident"]["body_paint"]["mode"] = 3.
        with self.assertRaisesRegex(ValueError, "water material"):
            resident.validate_record(manifest, row, manifest["parts"])

    def test_spa_emission_is_worker_mask_bound_for_each_named_slot_and_body(self):
        for mask in range(16):
            manifest, row = fixture("SoulSpa")
            row["state"]["mask"] = mask
            for part in row["parts"]:
                actual = part["resident"]
                active = actual["name"] != "body" and bool(mask & (1 << int(actual["name"][-1])))
                actual["spa"].update(slot_active=active, emissive_white=active, emissive_black=not active,
                                     texture=actual["images"]["slot_emissive"] if active else None)
            resident.validate_record(manifest, row, manifest["parts"])
            for i in range(5):
                for key in ("slot_active", "emissive_white", "emissive_black", "texture"):
                    bad = copy.deepcopy(row)
                    facts = bad["parts"][i]["resident"]["spa"]
                    facts[key] = not facts[key] if key != "texture" else "foreign"
                    with self.assertRaisesRegex(ValueError, "emission"):
                        resident.validate_record(manifest, bad, manifest["parts"])

    def test_named_pose_prevents_slot_swaps_that_keep_shared_mesh_and_emission(self):
        manifest, row = fixture("SoulSpa")
        manifest["parts"][1]["translation_wu"][0] = 12.
        row["parts"][1]["translation_wu"][0] = 12.
        resident.validate_record(manifest, row, manifest["parts"])
        row["parts"][1]["resident"]["name"], row["parts"][2]["resident"]["name"] = "slot1", "slot0"
        with self.assertRaisesRegex(ValueError, "named part pose"):
            resident.validate_record(manifest, row, manifest["parts"])

    def test_all_loaded_modes_and_densities_require_residents_only_for_loaded_kinds(self):
        for copies in (4, 16):
            case = Case("building-art-static", "small" if copies == 4 else "medium", "gpu", 20260920, 15 if copies == 4 else 60, 0)
            for group in ("m2", "m3", "m5"):
                for candidate in (False, True):
                    value = group_sidecar(group, candidate, copies)
                    reader.validate_group_sidecar(value, case)
                    rows = [r for r in value["initial"]["records"] if "parts" in r and r["kind"] in resident.ROLES]
                    if rows:
                        rows[0]["parts"][0].pop("resident")
                        with self.assertRaises(ValueError):
                            reader.validate_group_sidecar(reseal(value), case)

    def test_coordinated_descriptor_substitution_and_cross_foreground_alias_fail(self):
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        for whole_owner in (False, True):
            value = sidecar(True)
            row = next(r for r in value["initial"]["records"] if r["kind"] == "SoulSpa")
            for part in row["parts"] if whole_owner else row["parts"][:1]:
                facts = part["resident"]
                facts["images"]["albedo"] = facts["texture"] = "coordinated-foreign"
                facts["meshes"][part["mesh_role"]] = facts["mesh"] = "coordinated-foreign"
            with self.assertRaises(ValueError):
                reader.validate_sidecar(reseal(value), case)
        value = sidecar(True)
        structural = next(r for r in value["initial"]["records"] if r["kind"] == "Tank")
        foreground = next(r for r in value["initial"]["records"] if "foreground" in r)
        structural["parts"][0]["resident"]["entity"] = foreground["foreground"]["entity"]
        with self.assertRaisesRegex(ValueError, "shared structural/foreground"):
            reader.validate_sidecar(reseal(value), case)

    def test_fixed_spa_culling_keeps_real_flags_while_target_and_full_nine_remain_strict(self):
        case = Case("building-art-static", "small", "gpu", 20260920, 15, 0)
        value = gallery_sidecar("m5")
        for row in value["initial"]["records"]:
            if row["kind"] == "SoulSpa":
                for part, geometry in zip(row["parts"], row["gallery_geometry"], strict=True):
                    part["visible"] = geometry["visible"] = False
        reader.validate_group_gallery_sidecar(reseal(value), case)
        for group in (None, "m3"):
            value = sidecar(True) if group is None else gallery_sidecar(group)
            row = next(r for r in value["initial"]["records"] if r["kind"] == "SoulSpa")
            row["parts"][0]["visible"] = False
            if group:
                row["gallery_geometry"][0]["visible"] = False
            with self.assertRaisesRegex(ValueError, "visibility"):
                (reader.validate_sidecar if group is None else reader.validate_group_gallery_sidecar)(reseal(value), case)

    def test_plan_and_public_context_freeze_both_new_oracles_before_raw_verification(self):
        dependencies = resident.dependencies()
        self.assertEqual(cumulative.dependencies()["resident_verifiers"], dependencies)
        value = {"foreground_verifier_sha256": m6.art.foreground.verifier_sha256(), "resident_verifiers": dependencies}
        for wrong in (None, {}, {**dependencies, "resident": "0" * 64}, {**dependencies, "body_paint": "0" * 64}):
            with self.assertRaisesRegex(ValueError, "resident/paint"):
                m6.check_foreground_verifier({**value, "resident_verifiers": wrong})
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            codec = repo / "codec"
            codec.write_bytes(b"synthetic codec")
            with patch.object(m6.native, "git_dirty_paths", return_value=[]), patch.object(m6.static, "identity", return_value={}), patch.object(m6, "inventory", return_value={}):
                context = m6.released_case_context(repo, codec, "Intel")
                self.assertEqual(context.resident_verifiers, dependencies)
                with patch.object(m6, "_verify_case_artifacts") as oracle:
                    with self.assertRaisesRegex(ValueError, "context changed or forged"):
                        m6.verify_released_case(replace(context, resident_verifiers={}), ("capture", "small", 1, "candidate"), repo, "a" * 64)
                    oracle.assert_not_called()
                with patch.object(resident, "dependencies", return_value={}), patch.object(m6, "_verify_case_artifacts") as oracle:
                    with self.assertRaisesRegex(ValueError, "before raw oracle"):
                        m6._verify_released_case_artifacts(context, ("capture", "small", 1, "candidate"), repo, "a" * 64)
                    oracle.assert_not_called()


if __name__ == "__main__":
    unittest.main()
