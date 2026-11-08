"""Synthetic M5 context, historical galleries do not gain new acceptance."""
import copy
from pathlib import Path
import unittest

from scripts import building_m5_context_observation as context
from scripts.tests import test_building_foreground_observation as foreground_tests
from scripts import building_art_acceptance as art


def pose(point):
    return {"translation": point, "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}


def mesh(entity, owner, actor, rendered):
    return {"entity": entity, "root": entity, "owner": owner, "mesh": "synthetic-mesh",
            "pose": rendered, "root_pose": copy.deepcopy(rendered), "global_matrix": context.foreground.matrix(rendered),
            "visible": True, "layers": [1] if actor else [1, 5],
            "material": {"kind": "standard" if actor else "structural", "handle": "synthetic-material",
                         "base_color": [1., 1., 1., 1.], "values": "synthetic material snapshot"}}


def fixture(kind="SandPile"):
    value, probe = foreground_tests.ForegroundTests().fixture(kind)
    value.update(variant="m5-art-context", mode="feedback", view="normal", lighting="standard", spa_mask_page=None,
                 scope=context.SCOPE, m5_context_verifier_sha256=context.verifier_sha256())
    probe.update(variant="m5-art-context", lighting="standard", scope=context.SCOPE)
    f = probe["fixture"]
    f.update(variant="m5-art-context", lighting="standard", scope=context.SCOPE, identity=value["identity"],
             view="normal", camera_scale=1., target_count=4, wall_count=8, preview_count=4, souls=8,
             accepted=False, performance_evidence=False, promotion_authority=False, idle_souls=8,
             source_count=0, source_workers=0, power=None)
    f["rtt_camera"] = {"entity": 9000, "active": True, "layers": [1], "pose": pose([0., 100., 0.])}
    f["rtt_camera"].update(scene_image="synthetic-scene", viewport=[1280., 720.], projection="Orthographic synthetic")
    compensation = context.math.hypot(150, 90) / 150
    composite_pose = pose([0., 0., 20.])
    composite_pose["scale"] = [1280., 720 * compensation, 1.]
    f["composite"] = {"entity": 9001, "visible": True, "layers": [2], "scene_image": "synthetic-scene",
                      "material": "synthetic-composite", "pose": composite_pose, "vertical_compensation": compensation}
    row = {"WheelbarrowParking": 28, "SandPile": 33, "BonePile": 38, "OutdoorLamp": 48}[kind]
    for index, record in enumerate(f["records"]):
        anchor = [8 + index * 5, row]
        extra = 16 if kind == "WheelbarrowParking" else 0
        center = [(n + .5) * 32 - 1600 + extra for n in anchor]
        record.update(anchor=anchor, center=center)
        fg = record["foreground"]
        fg.update(color_srgba=[1., 1., 1., 1.], owner_pose=pose([*center, 0.]), global_pose=pose([*center, 0.]))
        fg.update(global_matrix=context.foreground.matrix(fg["global_pose"]), parent_local_matrix=context.foreground.matrix(fg["global_pose"]))
        offsets = ((0, 0), (1, 0), (0, 1), (1, 1)) if kind == "WheelbarrowParking" else ((0, 0),)
        grids = [[anchor[0] + x, anchor[1] + y] for x, y in offsets]
        record["tiles"] = [{"grid": grid, "terrain": "Grass" if index % 2 else "Dirt"} for grid in grids]
        record["walls"] = [{"entity": 1000 + index * 2 + side, "grid": [anchor[0], y], "ground_projection": [640., 360.],
                            "pose": pose([(anchor[0] + .5) * 32 - 1600, (y + .5) * 32 - 1600, 0.]),
                            "meshes": [mesh(3000 + index * 2 + side, 1000 + index * 2 + side, False,
                                            pose([(anchor[0] + .5) * 32 - 1600, 16., -((y + .5) * 32 - 1600)]))]}
                           for side, y in enumerate((min(g[1] for g in grids) - 1, max(g[1] for g in grids) + 1))]
        record["souls"] = [{"entity": 2000 + index * 2 + side, "side": sign, "pose": pose([center[0], center[1] + sign * 12, 0.]),
                            "ground_projection": [640., 360.], "meshes": [mesh(4000 + index * 2 + side, 2000 + index * 2 + side, True,
                                                                              pose([center[0], 17.6, -center[1] - sign * 12]))]}
                           for side, sign in enumerate((-1, 1))]
        record["preview"] = {"entity": 5000 + index, "image_handle": f["records"][0]["foreground"]["image_handle"],
                             "canvas_wu": value["foreground_observation"]["descriptor"]["canvas_wu"], "anchor": [0., 0.],
                             "pose": pose([center[0], center[1] - 160, .2]), "ground_projection": [640., 360.], "visible": True}
    f["camera_center"] = [sum(r["center"][0] for r in f["records"]) / 4, f["records"][0]["center"][1] - 80]
    f["main_camera"] = {"entity": 9002, "target": "primary-window", "viewport": [1280, 720], "pose": pose([*f["camera_center"], 0.])}
    if kind == "OutdoorLamp":
        f.update(souls=14, source_count=2, source_workers=6, power={"records": [], "source_count": 2, "worker_count": 6})
        for ordinal, record in enumerate(f["records"]):
            grid, owner = 10000 + ordinal, 11000 + ordinal
            x = 8 + ordinal * 5
            low = [(x - 1 + .5) * 32 - 1600, (5 + .5) * 32 - 1600]
            high = [(x + 2 + .5) * 32 - 1600, (50 + .5) * 32 - 1600]
            row = {"ordinal": ordinal, "lamp": record["owner"], "grid": grid, "yard": 12000 + ordinal,
                   "yard_min": low, "yard_max": high, "consumers": [record["owner"]], "demand": .2,
                   "consumption": .2, "powered": ordinal >= 2, "generation": 0., "generators": [], "source": None}
            if ordinal >= 2:
                mask, count = (3, 2) if ordinal == 2 else (15, 4)
                source = {"entity": owner, "ordinal": ordinal, "mask": mask, "phase": "Operational", "grid": grid,
                          "worker_count": count, "output_per_soul": 1., "output": count, "tiles": []}
                row.update(generation=count, generators=[owner], source=source)
                for index, (dx, dy) in enumerate(((0, 0), (1, 0), (0, -1), (1, -1))):
                    tile, worker = 13000 + ordinal * 4 + index, 14000 + ordinal * 4 + index
                    position = [x + dx, 23 + dy]
                    active = bool(mask & (1 << index))
                    source["tiles"].append({"entity": tile, "parent": owner, "map_owner": owner, "grid": position,
                                            "workers": [worker] if active else [], "worker_observations": [{
                                                "entity": worker, "working_on": tile, "task_tile": tile,
                                                "task_phase": "Generating", "position": [(n + .5) * 32 - 1600 for n in position],
                                                "dream": 100.}] if active else []})
            f["power"]["records"].append(row)
    return value, probe


class M5ContextTests(unittest.TestCase):
    def test_lamp_source_graph_refuses_missing_foreign_and_changed_supply(self):
        value, probe = fixture("OutdoorLamp")
        for change in ("missing", "foreign-grid", "foreign-parent", "foreign-worker", "task", "output", "off-supply", "count", "tile", "yard"):
            bad = copy.deepcopy(probe)
            power = bad["fixture"]["power"]
            row = power["records"][2]
            source = row["source"]
            if change == "missing":
                power["records"].pop()
            elif change == "foreign-grid":
                source["grid"] += 1
            elif change == "foreign-parent":
                source["tiles"][0]["parent"] += 1
            elif change == "foreign-worker":
                source["tiles"][0]["worker_observations"][0]["entity"] += 1
            elif change == "task":
                source["tiles"][0]["worker_observations"][0]["task_phase"] = "GoingToTile"
            elif change == "output":
                source["output"] = 0.
            elif change == "off-supply":
                power["records"][0]["generators"] = [source["entity"]]
            elif change == "count":
                bad["fixture"]["source_workers"] = True
            elif change == "tile":
                source["tiles"][0]["grid"][1] += 1
            elif change == "yard":
                row["yard_min"] = [0., 0.]
            with self.subTest(change=change), self.assertRaises(ValueError):
                context.validate(bad, value)
        value, probe = fixture("SandPile")
        probe["fixture"]["power"] = {"source_count": 2}
        with self.assertRaises(ValueError):
            context.validate(probe, value)

    def test_all_four_explicit_scopes_without_mutation_or_gallery_authority(self):
        for kind in context.foreground.KINDS:
            value, probe = fixture(kind)
            original = copy.deepcopy((value, probe))
            context.validate(probe, value)
            self.assertEqual((value, probe), original)
            self.assertEqual(art.actor_count(value), 14 if kind == "OutdoorLamp" else 8)
            self.assertTrue(art.context_settings(value))
            with self.assertRaises(ValueError):
                context.foreground.validate(probe, value)
        self.assertEqual(art.actor_count({}), 15)

    def test_foreign_modes_kinds_lighting_views_and_verifier_are_rejected(self):
        value, _ = fixture()
        for field, changed in (("mode", "candidate"), ("kind", "Bridge"), ("view", "near"), ("view", "overview"),
                               ("lighting", "dim"), ("spa_mask_page", 0), ("promotion_authority", True),
                               ("scope", "paused-static-presentation"), ("m5_context_verifier_sha256", "0" * 64)):
            with self.subTest(field=field), self.assertRaises(ValueError):
                context.settings({**value, field: changed})
        for authority in ("isolated_candidate", "release_approved"):
            bad = copy.deepcopy(value)
            bad["identity"]["authority"] = authority
            with self.assertRaises(ValueError):
                context.settings(bad)

    def test_canonical_owner_ground_preview_and_real_scene_mesh_refusals(self):
        value, probe = fixture()
        for change in ("authority", "count", "terrain", "tile", "anchor", "center", "camera", "tint", "wall", "wall-mesh",
                       "actor", "actor-layer", "actor-material", "preview", "preview-image", "preview-canvas", "duplicate",
                       "wall-pose", "root-pose", "global-pose", "rtt-camera", "billboard-rotation", "mesh-layer-bool", "camera-layer-bool",
                       "main-target", "composite-image", "rtt-image", "composite-pose", "actor-layer-float", "wall-layer-float", "camera-layer-float"):
            bad = copy.deepcopy(probe)
            f = bad["fixture"]
            record = f["records"][0]
            if change == "authority":
                f["accepted"] = True
            elif change == "count":
                f["souls"] = 7
            elif change == "terrain":
                record["tiles"][0]["terrain"] = "River"
            elif change == "tile":
                record["tiles"][0]["grid"][0] += 1
            elif change == "anchor":
                record["anchor"][0] += 1
            elif change == "center":
                record["center"][0] += 1
            elif change == "camera":
                f["camera_center"][0] += 1
            elif change == "tint":
                record["foreground"]["color_srgba"][0] = .5
            elif change == "wall":
                record["walls"][0]["grid"] = record["tiles"][0]["grid"]
            elif change == "wall-mesh":
                record["walls"][0]["meshes"][0]["visible"] = False
            elif change == "actor":
                record["souls"][0]["pose"]["translation"][1] += 24
            elif change == "actor-layer":
                record["souls"][0]["meshes"][0]["layers"] = [0]
            elif change == "actor-material":
                record["souls"][0]["meshes"][0]["material"]["base_color"][0] = float("nan")
            elif change == "preview":
                record["preview"]["pose"]["translation"][1] += 1
            elif change == "preview-image":
                record["preview"]["image_handle"] = "foreign-image"
            elif change == "preview-canvas":
                record["preview"]["canvas_wu"] = [32., 32.]
            elif change == "duplicate":
                f["records"][1]["souls"][0]["entity"] = record["souls"][0]["entity"]
            elif change == "wall-pose":
                record["walls"][0]["pose"]["translation"][0] += 1
            elif change == "root-pose":
                record["walls"][0]["meshes"][0]["root_pose"]["translation"][0] += 1
            elif change == "global-pose":
                record["souls"][0]["meshes"][0]["global_matrix"][12] += 1
            elif change == "rtt-camera":
                f["rtt_camera"]["layers"] = [0]
            elif change == "billboard-rotation":
                f["rtt_camera"]["pose"]["rotation"] = [0., 1., 0., 0.]
            elif change == "mesh-layer-bool":
                record["walls"][0]["meshes"][0]["layers"] = [True, 5]
            elif change == "camera-layer-bool":
                f["rtt_camera"]["layers"] = [True]
            elif change == "main-target":
                f["main_camera"]["target"] = "foreign-window"
            elif change == "composite-image":
                f["composite"]["scene_image"] = "foreign-scene"
            elif change == "rtt-image":
                f["rtt_camera"]["scene_image"] = "foreign-scene"
            elif change == "composite-pose":
                f["composite"]["pose"]["translation"][2] += 1
            elif change == "actor-layer-float":
                record["souls"][0]["meshes"][0]["layers"] = [1.0]
            elif change == "wall-layer-float":
                record["walls"][0]["meshes"][0]["layers"] = [1.0, 5]
            elif change == "camera-layer-float":
                f["rtt_camera"]["layers"] = [1.0]
            with self.subTest(change=change), self.assertRaises(ValueError):
                context.validate(bad, value)

    def test_cli_and_historical_gallery_defaults_stay_separate(self):
        self.assertFalse(art.context_settings({}))
        with self.assertRaises(ValueError):
            art.actor_count({"variant": "m5-art-context"})
        value, _ = fixture("OutdoorLamp")
        command = art.game_command(Path("/synthetic-binary"), Path("/synthetic-root"), value)
        self.assertEqual(command[command.index("--spawn-souls") + 1], "14")
        legacy = art.game_command(Path("/synthetic-binary"), Path("/synthetic-root"), {})
        self.assertEqual(legacy[legacy.index("--spawn-souls") + 1], "15")


if __name__ == "__main__":
    unittest.main()
