"""Synthetic Door route guards. No actual input/window or release authority."""

import copy
import json
import unittest

from scripts import door_release_ui_observation as oracle

IDENTITY = {
    "asset_set_generation": 7,
    "authority": "release_approved",
    "manifest_sha256": "4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449",
}


def fixture(case="door-release-east-west"):
    anchor = [50, 50]
    x, y = anchor
    grids = (
        [[49, 50], [51, 50]]
        if case.endswith("east-west")
        else [[50, 49], [50, 51]]
        if case.endswith("north-south")
        else [[49, 50], [51, 50], [50, 49], [50, 51]]
    )
    supports = [
        {
            "grid": grid,
            "owner": 100 + i,
            "actual_owner": 100 + i,
            "kind": "Wall",
            "provisional": False,
        }
        for i, grid in enumerate(grids)
    ]
    prep = {
        "scope": "seeded-cardinal-supports-only",
        "anchor": anchor,
        "expected_axis": oracle.CASES[case],
        "supports": [{"grid": r["grid"], "owner": r["owner"]} for r in supports],
        "initial_buildings": [],
        "natural_construction_evidence": False,
        "initial_cells": [
            {
                "grid": [x + dx, y + dy],
                "walkable": True,
                "owner": None,
                "floor": None,
                "stockpile": False,
            }
            for dx in (-1, 0, 1)
            for dy in (-1, 0, 1)
        ],
    }
    inventory = [
        {
            "entity": r["owner"],
            "kind": "Wall",
            "is_provisional": False,
            "position": [0.0, 0.0, 0.0],
        }
        for r in supports
    ]

    def sprite(blueprint):
        pose = {
            "translation": [16.0, 16.0, 0.2 if blueprint else 2.0],
            "rotation": [0.0, 0.0, 0.0, 1.0],
            "scale": [0.9, 0.9, 0.9] if blueprint else [1.0, 1.0, 1.0],
        }
        return {
            "entity": 999 if blueprint else 998,
            "image": "normal-" + oracle.CASES[case],
            "expected_image": "normal-" + oracle.CASES[case],
            "matches": True,
            "image_size": [256, 256],
            "size": [64.0, 64.0],
            "anchor": [0.0, -0.25],
            "color": [0.1, 0.5, 1.0, 0.4] if blueprint else [0.5, 1.0, 0.5, 0.5],
            "visible": True,
            "visibility": "Inherited",
            "inherited_visible": True,
            "layers": [0],
            "parent": None,
            "position": pose["translation"],
            "rotation": pose["rotation"],
            "scale": pose["scale"],
            "global_matrix": oracle.art.foreground.matrix(pose),
            "flip_x": False,
            "flip_y": False,
            "rect": None,
        }

    def observation(phase, now):
        v = {
            "scope": oracle.SCOPE,
            "ready": True,
            "failure": None,
            "fixture": copy.deepcopy(prep),
            "supports": copy.deepcopy(supports),
            "identity": copy.deepcopy(IDENTITY),
            "anchor": anchor[:],
            "point": [960.0, 540.0],
            "cursor_grid": anchor[:],
            "actual_axis": oracle.CASES[case],
            "anchor_owner": 999 if phase == "blueprint" else None,
            "walkable": True,
            "catalog": [],
            "ghosts": [sprite(False)] if phase == "ghost" else [],
            "blueprints": [
                {
                    "entity": 999,
                    "debug_entity": "999v0",
                    "footprint": [anchor[:]],
                    "progress": 0.0,
                    "materials_complete": False,
                    "sprite": sprite(True),
                    "pulse": None,
                }
            ]
            if phase == "blueprint"
            else [],
            "building_inventory": copy.deepcopy(inventory),
            "primary_window": 5,
            "world_epoch": 0,
            "play_mode": "BuildingPlace" if phase == "ghost" else "Normal",
            "build_kind": "Door" if phase == "ghost" else None,
            "real_seconds": now,
            "input": {"arm_frame": 90, "events": 0, "failure": None},
            "promotion_authority": False,
            "performance_evidence": False,
            "natural_construction_evidence": False,
        }
        if phase == "catalog":
            v["catalog"] = [
                {
                    "entity": 500,
                    "image": "normal-EastWest",
                    "expected_image": "normal-EastWest",
                    "matches": True,
                    "primary_camera": True,
                    "image_size": [256, 256],
                    "rect": [100.0, 100.0, 140.0, 140.0],
                    "unclipped_rect": [100.0, 100.0, 140.0, 140.0],
                }
            ]
        return {
            "door_release_planning": v,
            "primary_window": 5,
            "world_epoch": 0,
            "paused": phase != "ghost",
            "pointer_over_ui": False,
            "world_input_captured": False,
            "pid": 123,
            "nonce": "a" * 32,
            "viewport": [1920.0, 1080.0],
            "scale_factor": 1.0,
            "frame": int(now * 100),
        }

    entries = [
        {"case": phase, "value": observation(phase, now)}
        for phase, now in zip(oracle.CHECKPOINTS, [1.0, 2.5, 5.0, 8.0], strict=True)
    ]
    client = {"nonce": "a" * 32, "pid": 123, "window": 291}
    events = []
    inputs = []
    actions = []
    spec = [
        ("architect", "empty", 1.0, "catalog", 2.0),
        ("resume", "catalog", 2.5, "catalog", 3.0),
        ("select-door", "catalog", 3.0, "ghost", 4.0),
        ("placement", "ghost", 5.0, "blueprint", 6.0),
        ("escape", "blueprint", 6.0, "blueprint", 6.5),
        ("pause", "blueprint", 6.5, "blueprint", 7.0),
    ]
    for label, phase, lo, after, hi in spec:
        first = len(inputs) + 1
        common = {"nonce": client["nonce"], "pid": 123, "window": 291, "world_epoch": 0}
        if label != "escape":
            inputs.append(
                dict(
                    common,
                    step=str(len(inputs) + 1),
                    point=[960, 540],
                    pressed=None,
                    pre_frame=int(lo * 100) - (20 if label == "placement" else 0),
                    post_frame=int(lo * 100) + 1 - (20 if label == "placement" else 0),
                    observed_cursor=[960.0, 540.0],
                )
            )
        for i, pressed in enumerate((True, False)):
            gesture = dict(
                common,
                step=str(len(inputs) + 1),
                pressed=pressed,
                pre_frame=int(lo * 100) + 1 + i,
                post_frame=int(hi * 100) - 1 + i,
            )
            gesture["key" if label == "escape" else "button"] = (
                "Escape" if label == "escape" else 1
            )
            if pressed and label in ("architect", "select-door", "resume", "pause"):
                gesture["observed_hover"] = {
                    {
                        "architect": "menu:ToggleArchitect",
                        "select-door": "menu:SelectBuild(Door)",
                        "resume": "menu:SetTimeSpeed(Normal)",
                        "pause": "menu:SetTimeSpeed(Paused)",
                    }[label]: "Pressed"
                }
            inputs.append(gesture)
            event = {
                "producer": "KeyboardInput"
                if label == "escape"
                else "MouseButtonInput",
                "frame": int(lo * 100) + 20 + i,
                "real_seconds": lo + 0.2 + i * 0.01,
                "world_epoch": 0,
                "window": 5,
                "window_debug": "5v0",
                "state": "Pressed" if pressed else "Released",
            }
            event.update(
                {"key": "Escape", "repeat": False}
                if label == "escape"
                else {"button": "Left"}
            )
            events.append(event)
        before, after_value = observation(phase, lo), observation(after, hi)
        before["paused"], after_value["paused"] = {
            "architect": (True, True),
            "resume": (True, False),
            "select-door": (False, False),
            "placement": (False, False),
            "escape": (False, False),
            "pause": (False, True),
        }[label]
        actions.append(
            {
                "label": label,
                "first_step": first,
                "last_step": len(inputs),
                "before": before,
                "after": after_value,
            }
        )
    by_label = {action["label"]: action for action in actions}
    prefix = [
        0,
        by_label["architect"]["last_step"],
        by_label["placement"]["first_step"],
        by_label["pause"]["last_step"],
    ]
    for entry, step in zip(entries, prefix, strict=True):
        entry["last_step"] = step
    receipt = {
        "producer": "ordinary-building-placement-branch-v1",
        "operation": "PlacementOutcome",
        "real_seconds": 5.2,
        "window": "5v0",
        "world_epoch": 0,
        "outcome": {
            "kind": "Door",
            "status": "BlueprintCommitted",
            "owner": "999v0",
            "grid": anchor,
            "footprint": [anchor],
        },
    }
    raw = {
        "schema_version": 1,
        "scope": oracle.SCOPE,
        "nonce": client["nonce"],
        "pid": 123,
        "frame": 850,
        "real_seconds": 8.5,
        "world_epoch": 0,
        "arm_frame": 90,
        "events": events,
        "receipts": [receipt],
        "failure": None,
    }
    for value in [
        *(e["value"] for e in entries),
        *(a[k] for a in actions for k in ("before", "after")),
    ]:
        value["door_release_planning"]["input"]["events"] = sum(
            e["frame"] <= value["frame"] for e in events
        )
    return json.loads(
        json.dumps((raw, actions, inputs, client, entries, case, IDENTITY))
    )


class DoorOracleTests(unittest.TestCase):
    def test_complete_closed_three_case_routes(self):
        for case in oracle.CASES:
            with self.subTest(case=case):
                self.assertEqual(
                    oracle.verify_inputs(*fixture(case))["axis"], oracle.CASES[case]
                )

    def test_rejects_missing_extra_reordered_or_foreign_input(self):
        for mutation in (
            lambda v: v[0]["events"].pop(),
            lambda v: v[0]["events"].append(copy.deepcopy(v[0]["events"][-1])),
            lambda v: v[0]["events"][0].update(window=6),
            lambda v: v[0]["events"][0].update(frame=89),
            lambda v: v[0]["events"][0].update(state="Released"),
            lambda v: v[0]["events"][-1].update(repeat=True),
            lambda v: v[2][0].update(nonce="b" * 32),
            lambda v: v[2][1].update(pressed=1),
            lambda v: v[2][1].update(post_frame=119),
            lambda v: v[2][0].update(point=[True, 540]),
            lambda v: v[1][0].update(last_step=1),
        ):
            values = fixture()
            mutation(values)
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)

    def test_checkpoint_transport_prefix_must_follow_its_causal_action(self):
        for index in range(4):
            for forged in [None, True, -1, 42]:
                values = fixture()
                values[4][index]["last_step"] = forged
                with (
                    self.subTest(index=index, forged=forged),
                    self.assertRaisesRegex(ValueError, "causal action"),
                ):
                    oracle.verify_inputs(*values)

    def test_pre_action_catalog_or_ghost_cannot_borrow_a_later_input_prefix(self):
        for index, now in [(1, 1.1), (2, 2.7)]:
            values = fixture()
            observation = values[4][index]["value"]
            observation["frame"] = int(now * 100)
            observation["door_release_planning"]["real_seconds"] = now
            observation["door_release_planning"]["input"]["events"] = sum(
                event["frame"] <= observation["frame"] for event in values[0]["events"]
            )
            with (
                self.subTest(index=index),
                self.assertRaisesRegex(ValueError, "causal action"),
            ):
                oracle.verify_inputs(*values)

    def test_paused_selection_cannot_borrow_a_pressed_ui_ack(self):
        values = fixture()
        action = next(row for row in values[1] if row["label"] == "select-door")
        action["before"]["paused"] = True
        action["after"]["paused"] = True
        with self.assertRaisesRegex(ValueError, "existing pause boundary"):
            oracle.verify_inputs(*values)

    def test_resume_pause_actions_cannot_be_omitted_or_use_foreign_controls(self):
        for label in ["resume", "pause"]:
            values = fixture()
            values[1] = [action for action in values[1] if action["label"] != label]
            with self.assertRaisesRegex(ValueError, "action sequence"):
                oracle.verify_inputs(*values)
            values = fixture()
            action = next(action for action in values[1] if action["label"] == label)
            values[2][action["first_step"]]["observed_hover"] = {
                "menu:SelectBuild(Door)": "Pressed"
            }
            with self.assertRaisesRegex(ValueError, "control press ACK"):
                oracle.verify_inputs(*values)

    def test_honest_but_overlong_running_interval_is_rejected_before_acceptance(self):
        values = fixture()

        def stretch(value):
            if isinstance(value, dict):
                for key, child in value.items():
                    if key in ("frame", "pre_frame", "post_frame", "real_seconds"):
                        value[key] = child * 10
                    else:
                        stretch(child)
            elif isinstance(value, list):
                for child in value:
                    stretch(child)

        stretch(values)
        with self.assertRaisesRegex(ValueError, "twenty-second"):
            oracle.verify_inputs(*values)

    def test_actual_receipt_must_bind_the_single_press_and_same_blueprint(self):
        for key, value in [
            ("world_epoch", False),
            ("real_seconds", 3.0),
            ("window", "6v0"),
            ("producer", "synthetic"),
        ]:
            values = fixture()
            values[0]["receipts"][0][key] = value
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)
        for key, value in [
            ("owner", "another"),
            ("kind", "Tank"),
            ("grid", [51, 50]),
            ("status", "Rejected"),
            ("footprint", [[51, 50]]),
        ]:
            values = fixture()
            values[0]["receipts"][0]["outcome"][key] = value
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)
        values = fixture()
        values[0]["receipts"] *= 2
        with self.assertRaises(ValueError):
            oracle.verify_inputs(*values)

    def test_context_rejects_wrong_generation_candidate_support_replacement_or_nonempty_floor(
        self,
    ):
        for mutation in (
            lambda v: v["identity"].update(authority="isolated_candidate"),
            lambda v: v["identity"].update(asset_set_generation=8),
            lambda v: v["supports"][0].update(actual_owner=111),
            lambda v: v["supports"][0].update(provisional=True),
            lambda v: v["fixture"]["initial_cells"][0].update(owner=400),
            lambda v: v["fixture"]["initial_cells"][0].update(floor=400),
            lambda v: v.update(actual_axis="NorthSouth"),
            lambda v: v["fixture"]["initial_cells"][0]["grid"].__setitem__(0, 49.0),
        ):
            values = fixture()
            mutation(values[4][0]["value"]["door_release_planning"])
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)

    def test_visible_consumer_rejects_wrong_handle_layer_parent_and_transform(self):
        for key, value in [
            ("matches", False),
            ("layers", [1]),
            ("layers", [False]),
            ("parent", 100),
            ("visible", False),
            ("inherited_visible", False),
            ("visibility", "Hidden"),
            ("size", [32.0, 32.0]),
            ("anchor", [0.0, 0.0]),
            ("color", [1.0, 1.0, 1.0, 0.5]),
            ("position", [48.0, 16.0, 0.5]),
        ]:
            values = fixture()
            values[4][2]["value"]["door_release_planning"]["ghosts"][0][key] = value
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)

    def test_ghost_and_zero_progress_blueprint_keep_their_distinct_scale_and_depth(
        self,
    ):
        for index, field, bad_value in [
            (2, "scale", [0.9, 0.9, 0.9]),
            (3, "scale", [1.0, 1.0, 1.0]),
            (3, "scale", [0.9, 0.9, 1.0]),
            (2, "position", [16.0, 16.0, 0.2]),
            (3, "position", [16.0, 16.0, 2.0]),
        ]:
            values = fixture()
            context = values[4][index]["value"]["door_release_planning"]
            sprite = (
                context["ghosts"][0]
                if index == 2
                else context["blueprints"][0]["sprite"]
            )
            sprite[field] = bad_value
            sprite["global_matrix"] = oracle.art.foreground.matrix(
                {
                    "translation": sprite["position"],
                    "rotation": sprite["rotation"],
                    "scale": sprite["scale"],
                }
            )
            with (
                self.subTest(index=index, field=field),
                self.assertRaisesRegex(ValueError, "scale/depth"),
            ):
                oracle.verify_inputs(*values)
        values = fixture()
        values[4][-1]["value"]["door_release_planning"]["blueprints"][0]["sprite"][
            "scale"
        ] = [0.8999999761581421] * 3
        pose = values[4][-1]["value"]["door_release_planning"]["blueprints"][0][
            "sprite"
        ]
        pose["global_matrix"] = oracle.art.foreground.matrix(
            {
                "translation": pose["position"],
                "rotation": pose["rotation"],
                "scale": pose["scale"],
            }
        )
        oracle.verify_inputs(*values)

    def test_blueprint_sprite_must_be_the_same_actual_blueprint_entity(self):
        values = fixture()
        values[4][-1]["value"]["door_release_planning"]["blueprints"][0]["sprite"][
            "entity"
        ] = 1001
        with self.assertRaises(ValueError):
            oracle.verify_inputs(*values)

    def test_snapshot_cannot_omit_or_rebind_the_actual_input_prefix(self):
        for key, value in [("events", 0), ("arm_frame", 91), ("arm_frame", True)]:
            values = fixture()
            values[4][-1]["value"]["door_release_planning"]["input"][key] = value
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)

    def test_vec2_viewport_requires_exact_finite_numeric_dimensions_without_bool(self):
        oracle.verify_inputs(*fixture())  # Real observer serializes Vec2 as floats.
        for viewport in [
            [1920.0, 1080.01],
            [1920.0, True],
            ["1920", 1080],
            [float("nan"), 1080.0],
            [1920.0, float("inf")],
        ]:
            values = fixture()
            values[4][0]["value"]["viewport"] = viewport
            with (
                self.subTest(viewport=viewport),
                self.assertRaisesRegex(ValueError, "viewport/DPI"),
            ):
                oracle.verify_inputs(*values)

    def test_snapshot_pid_nonce_dpi_and_fixture_drift_are_rejected(self):
        for key, value in [
            ("pid", True),
            ("nonce", "b" * 32),
            ("viewport", [1280, 720]),
            ("scale_factor", True),
        ]:
            values = fixture()
            values[4][0]["value"][key] = value
            with self.assertRaises(ValueError):
                oracle.verify_inputs(*values)


if __name__ == "__main__":
    unittest.main()
