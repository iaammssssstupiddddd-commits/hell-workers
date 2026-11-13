"""Ordinary released Door planning oracle; no art/performance/release authority."""

from __future__ import annotations
import math

if __package__:
    from . import building_art_acceptance as art
else:
    import building_art_acceptance as art

pipeline, native = art.pipeline, art.native
require = pipeline.require
SCOPE = "ordinary-door-release-planning-v1"
CASES = {
    "door-release-east-west": "EastWest",
    "door-release-north-south": "NorthSouth",
    "door-release-tie": "EastWest",
}
CHECKPOINTS = ("empty", "catalog", "ghost", "blueprint")


def finite(v):
    return type(v) in (int, float) and math.isfinite(v)


def vector(v, n):
    require(
        isinstance(v, list) and len(v) == n and all(finite(x) for x in v),
        "Door nonfinite/bool vector",
    )
    return v


def context(value, case, identity):
    require(case in CASES, "unknown Door case")
    v = value.get("door_release_planning")
    require(
        isinstance(v, dict)
        and set(v)
        == {
            "scope",
            "ready",
            "failure",
            "fixture",
            "supports",
            "identity",
            "anchor",
            "point",
            "cursor_grid",
            "actual_axis",
            "anchor_owner",
            "walkable",
            "catalog",
            "ghosts",
            "blueprints",
            "building_inventory",
            "primary_window",
            "world_epoch",
            "play_mode",
            "build_kind",
            "real_seconds",
            "input",
            "promotion_authority",
            "performance_evidence",
            "natural_construction_evidence",
        },
        "Door context wire coverage differs",
    )
    require(
        v["scope"] == SCOPE
        and v["ready"] is True
        and v["failure"] is None
        and all(
            v[k] is False
            for k in (
                "promotion_authority",
                "performance_evidence",
                "natural_construction_evidence",
            )
        ),
        "Door context scope/readiness differs",
    )
    require(
        pipeline.exact_json(
            v["identity"],
            {
                k: identity[k]
                for k in ("asset_set_generation", "authority", "manifest_sha256")
            },
        )
        and identity["authority"] == "release_approved",
        "Door active identity differs",
    )
    require(
        type(v["primary_window"]) is int
        and v["primary_window"] == value["primary_window"]
        and type(value["primary_window"]) is int
        and type(value["world_epoch"]) is int
        and type(v["world_epoch"]) is int
        and v["world_epoch"] == value["world_epoch"]
        and finite(v["real_seconds"])
        and type(value["paused"]) is bool,
        "Door window/epoch/clock/pause differs",
    )
    anchor = v["anchor"]
    require(
        isinstance(anchor, list)
        and len(anchor) == 2
        and all(type(n) is int and 2 <= n < 98 for n in anchor),
        "Door anchor differs",
    )
    vector(v["point"], 2)
    require(
        all(0 < x < bound for x, bound in zip(v["point"], [1920, 1080], strict=True)),
        "Door anchor outside viewport",
    )
    x, y = anchor
    grids = (
        [[x - 1, y], [x + 1, y]]
        if case.endswith("east-west")
        else [[x, y - 1], [x, y + 1]]
        if case.endswith("north-south")
        else [[x - 1, y], [x + 1, y], [x, y - 1], [x, y + 1]]
    )
    fixture = v["fixture"]
    require(
        set(fixture)
        == {
            "scope",
            "anchor",
            "expected_axis",
            "supports",
            "initial_buildings",
            "initial_cells",
            "natural_construction_evidence",
        }
        and fixture["scope"] == "seeded-cardinal-supports-only"
        and fixture["natural_construction_evidence"] is False
        and pipeline.exact_json(fixture["anchor"], anchor)
        and fixture["expected_axis"] == CASES[case]
        and v["actual_axis"] == CASES[case],
        "Door support preparation scope/axis differs",
    )
    supports = v["supports"]
    require(
        isinstance(supports, list) and len(supports) == len(grids),
        "Door support coverage differs",
    )
    owners = []
    for row, initial, grid in zip(supports, fixture["supports"], grids, strict=True):
        require(
            set(row) == {"grid", "owner", "actual_owner", "kind", "provisional"}
            and set(initial) == {"grid", "owner"}
            and pipeline.exact_json(row["grid"], grid)
            and pipeline.exact_json(initial["grid"], grid)
            and all(
                type(n) is int
                for n in (row["owner"], row["actual_owner"], initial["owner"])
            )
            and row["owner"] == row["actual_owner"] == initial["owner"]
            and row["kind"] == "Wall"
            and row["provisional"] is False,
            "Door support ownership changed",
        )
        owners.append(row["owner"])
    require(len(set(owners)) == len(owners), "Door duplicate support owner")
    cells = fixture["initial_cells"]
    require(
        isinstance(cells, list)
        and len(cells) == 9
        and [r["grid"] for r in cells]
        == [[x + dx, y + dy] for dx in (-1, 0, 1) for dy in (-1, 0, 1)]
        and all(
            isinstance(r["grid"], list)
            and len(r["grid"]) == 2
            and all(type(n) is int for n in r["grid"])
            and r["walkable"] is True
            and r["owner"] is None
            and r["stockpile"] is False
            and r["floor"] is None
            for r in cells
        ),
        "Door fixture replaced an occupied or nonwalkable cell",
    )
    original = fixture["initial_buildings"]
    current = v["building_inventory"]
    require(
        isinstance(original, list)
        and isinstance(current, list)
        and len(original) <= 256
        and len(current) == len(original) + len(owners)
        and all(type(r["entity"]) is int for r in [*original, *current])
        and len({r["entity"] for r in current}) == len(current),
        "Door completed inventory changed",
    )
    by_id = {r["entity"]: r for r in current}
    require(
        all(pipeline.exact_json(by_id.get(r["entity"]), r) for r in original)
        and {r["entity"] for r in current} - {r["entity"] for r in original}
        == set(owners),
        "Door fixture changed an existing building or created an unexpected one",
    )
    require(
        v["input"]["failure"] is None
        and type(v["input"]["events"]) is int
        and 0 <= v["input"]["events"] <= 256,
        "Door input overflow/failure",
    )
    return v


def check_sprite(row, *, blueprint=False, pulse=False, anchor=None):
    require(
        set(row)
        == {
            "entity",
            "image",
            "expected_image",
            "matches",
            "image_size",
            "size",
            "anchor",
            "color",
            "visible",
            "position",
            "rotation",
            "scale",
            "global_matrix",
            "flip_x",
            "flip_y",
            "rect",
            "layers",
            "parent",
            "visibility",
            "inherited_visible",
        },
        "Door sprite wire differs",
    )
    require(
        type(row["entity"]) is int
        and row["matches"] is True
        and row["image"] == row["expected_image"]
        and isinstance(row["image"], str)
        and row["image"]
        and type(row["visible"]) is bool
        and (row["visible"] is True or pulse)
        and isinstance(row["layers"], list)
        and all(type(n) is int for n in row["layers"])
        and row["layers"] == [0]
        and row["visibility"] in ("Visible", "Inherited")
        and row["inherited_visible"] is True
        and pipeline.exact_json(row["size"], [64.0, 64.0])
        and pipeline.exact_json(row["anchor"], [0.0, -0.25])
        and row["flip_x"] is False
        and row["flip_y"] is False
        and row["rect"] is None,
        "Door preview descriptor/fallback differs",
    )
    require(
        isinstance(row["image_size"], list)
        and len(row["image_size"]) == 2
        and all(type(n) is int and n > 0 for n in row["image_size"]),
        "Door preview image not resident",
    )
    for key, n in (
        ("position", 3),
        ("rotation", 4),
        ("scale", 3),
        ("global_matrix", 16),
        ("color", 4),
    ):
        vector(row[key], n)
    require(
        0 <= row["color"][3] <= 1 and (row["color"][3] > 0 or pulse),
        "Door consumer transparent",
    )
    if not pulse:
        require(
            row["parent"] is None
            and row["rotation"] == [0.0, 0.0, 0.0, 1.0]
            # The ordinary Blueprint scale animation is 0.9 + 0.1*progress;
            # this audit requires progress exactly zero. Ghosts remain unit scale.
            and all(
                abs(axis - (0.9 if blueprint else 1.0)) <= 1e-6 for axis in row["scale"]
            )
            and abs(row["position"][2] - (0.2 if blueprint else 2.0)) <= 1e-6,
            "Door preview rotation/scale/depth differs",
        )
        require(
            anchor is not None
            and row["position"][:2] == [(n - 49.5) * 32 for n in anchor],
            "Door preview is not at the actual reserved cell",
        )
        pose = {
            "translation": row["position"],
            "rotation": row["rotation"],
            "scale": row["scale"],
        }
        require(
            art.foreground.close(row["global_matrix"], art.foreground.matrix(pose)),
            "Door preview world/local pose differs",
        )
    if not pulse:
        expected = [0.1, 0.5, 1.0, 0.4] if blueprint else [0.5, 1.0, 0.5, 0.5]
        require(
            all(
                abs(a - b) <= 1e-6 for a, b in zip(row["color"], expected, strict=True)
            ),
            "Door preview tint differs",
        )


def checkpoint(phase, value, case, identity):
    require(phase in CHECKPOINTS, "Door checkpoint unknown")
    v = context(value, case, identity)
    if phase != "blueprint":
        require(
            v["anchor_owner"] is None and v["walkable"] is True and not v["blueprints"],
            "Door anchor was not empty",
        )
    if phase in ("empty", "catalog", "blueprint"):
        require(not v["ghosts"], "Door ghost unexpectedly present")
    if phase == "catalog":
        require(len(v["catalog"]) == 1, "Door catalog not uniquely visible")
        row = v["catalog"][0]
        require(
            row["matches"] is True
            and row["image"] == row["expected_image"]
            and row["primary_camera"] is True,
            "Door catalog is not the normal EW preview",
        )
        rect = vector(row["rect"], 4)
        raw = vector(row["unclipped_rect"], 4)
        require(
            all(abs(a - b) <= 1 for a, b in zip(rect, raw, strict=True))
            and 0 <= rect[0] < rect[2] <= 1920
            and 0 <= rect[1] < rect[3] <= 1080,
            "Door catalog clipped",
        )
    require(
        value["paused"] is (phase != "ghost"), "Door checkpoint pause protocol differs"
    )
    if phase == "ghost":
        require(
            v["play_mode"] == "BuildingPlace"
            and v["build_kind"] == "Door"
            and pipeline.exact_json(v["cursor_grid"], v["anchor"])
            and len(v["ghosts"]) == 1
            and value["pointer_over_ui"] is False
            and value["world_input_captured"] is False,
            "Door ghost route differs",
        )
        check_sprite(v["ghosts"][0], anchor=v["anchor"])
    if phase == "blueprint":
        require(len(v["blueprints"]) == 1, "Door Blueprint not unique")
        bp = v["blueprints"][0]
        require(
            type(bp["entity"]) is int
            and bp["entity"] == v["anchor_owner"]
            and pipeline.exact_json(bp["footprint"], [v["anchor"]])
            and type(bp["progress"]) in (int, float)
            and bp["progress"] == 0
            and bp["materials_complete"] is False
            and type(bp["sprite"]["entity"]) is int
            and bp["sprite"]["entity"] == bp["entity"],
            "Door Blueprint owner/footprint/progress differs",
        )
        check_sprite(bp["sprite"], blueprint=True, anchor=v["anchor"])
        if bp["pulse"] is not None:
            check_sprite(bp["pulse"], pulse=True)
            pulse = bp["pulse"]
            require(
                type(pulse["parent"]) is int
                and pulse["parent"] == bp["entity"]
                and pulse["entity"] != bp["entity"],
                "Door pulse belongs to another Blueprint",
            )
            local = art.foreground.matrix(
                {
                    "translation": pulse["position"],
                    "rotation": pulse["rotation"],
                    "scale": pulse["scale"],
                }
            )
            require(
                art.foreground.close(
                    pulse["global_matrix"],
                    art.foreground.product(bp["sprite"]["global_matrix"], local),
                ),
                "Door pulse global/parent geometry differs",
            )
    return v


def verify_inputs(raw, actions, inputs, client, entries, case, identity):
    require(
        set(raw)
        == {
            "schema_version",
            "scope",
            "nonce",
            "pid",
            "frame",
            "real_seconds",
            "world_epoch",
            "arm_frame",
            "events",
            "receipts",
            "failure",
        }
        and type(raw["schema_version"]) is int
        and raw["schema_version"] == 1
        and raw["scope"] == SCOPE
        and raw["nonce"] == client["nonce"]
        and type(raw["pid"]) is int
        and raw["pid"] == client["pid"]
        and raw["failure"] is None
        and type(raw["arm_frame"]) is int
        and 46 <= raw["arm_frame"] <= raw["frame"]
        and finite(raw["real_seconds"]),
        "Door raw identity/arm/failure differs",
    )
    require(
        isinstance(entries, list) and [e["case"] for e in entries] == list(CHECKPOINTS),
        "Door checkpoints differ",
    )
    contexts = [checkpoint(e["case"], e["value"], case, identity) for e in entries]
    for value in [
        *(e["value"] for e in entries),
        *(a[k] for a in actions for k in ("before", "after")),
    ]:
        require(
            type(value.get("pid")) is int
            and value["pid"] == client["pid"]
            and value.get("nonce") == client["nonce"]
            and isinstance(value.get("viewport"), list)
            and all(type(n) is int for n in value["viewport"])
            and value["viewport"] == [1920, 1080]
            and type(value.get("scale_factor")) in (int, float)
            and value["scale_factor"] == 1.0
            and type(value.get("frame")) is int
            and raw["arm_frame"] <= value["frame"] <= raw["frame"],
            "Door snapshot PID/nonce/viewport/DPI differs",
        )
    epoch = contexts[0]["world_epoch"]
    fixture = contexts[0]["fixture"]
    require(
        type(raw["world_epoch"]) is int
        and raw["world_epoch"] == epoch
        and all(pipeline.exact_json(v["fixture"], fixture) for v in contexts)
        and all(
            a["real_seconds"] < b["real_seconds"]
            for a, b in zip(contexts, contexts[1:])
        ),
        "Door fixture/epoch/checkpoint clock drift",
    )
    require(
        isinstance(inputs, list)
        and 0 < len(inputs) <= 64
        and [row["step"] for row in inputs] == [str(i + 1) for i in range(len(inputs))],
        "Door transport steps differ",
    )
    for row in inputs:
        require(
            row["nonce"] == client["nonce"]
            and all(
                type(row[k]) is int
                for k in ("pid", "window", "world_epoch", "pre_frame", "post_frame")
            )
            and row["pid"] == client["pid"]
            and row["window"] == client["window"]
            and row["world_epoch"] == epoch
            and raw["arm_frame"] <= row["pre_frame"] < row["post_frame"] <= raw["frame"]
            and sum(row.get(k) is not None for k in ("point", "key", "button")) == 1,
            "Door transport ownership/order differs",
        )
        if row.get("point") is not None:
            require(
                isinstance(row["point"], list)
                and len(row["point"]) == 2
                and all(
                    type(n) is int and 0 < n < bound
                    for n, bound in zip(row["point"], [1920, 1080], strict=True)
                )
                and row.get("pressed") is None,
                "Door pointer coordinate differs",
            )
            cursor = vector(row["observed_cursor"], 2)
            require(
                all(abs(a - b) <= 2 for a, b in zip(cursor, row["point"], strict=True)),
                "Door pointer ACK differs",
            )
        else:
            require(
                type(row["pressed"]) is bool
                and (
                    type(row.get("button")) is int and row["button"] == 1
                    if row.get("button") is not None
                    else row.get("key") == "Escape"
                ),
                "Door unsupported input command",
            )
    require(
        all(
            a["pre_frame"] <= b["pre_frame"] and a["post_frame"] <= b["post_frame"]
            for a, b in zip(inputs, inputs[1:])
        ),
        "Door input transport reordered",
    )
    labels = [a["label"] for a in actions]
    require(
        labels
        in (
            ["architect", "resume", "select-door", "placement", "escape", "pause"],
            [
                "dev-minimize",
                "architect",
                "resume",
                "select-door",
                "placement",
                "escape",
                "pause",
            ],
        ),
        "Door action sequence differs",
    )
    events = raw["events"]
    require(
        isinstance(events, list) and 0 < len(events) <= 256,
        "Door raw input cap differs",
    )
    require(
        all(
            set(e)
            == {
                "producer",
                "frame",
                "real_seconds",
                "world_epoch",
                "window",
                "window_debug",
                "state",
            }
            | (
                {"key", "repeat"}
                if e.get("producer") == "KeyboardInput"
                else {"button"}
            )
            and type(e.get("frame")) is int
            and raw["arm_frame"] <= e["frame"] <= raw["frame"]
            and type(e.get("world_epoch")) is int
            and e["world_epoch"] == epoch
            and finite(e.get("real_seconds"))
            and e["real_seconds"] <= raw["real_seconds"]
            and e["window"] == contexts[0]["primary_window"]
            for e in events
        ),
        "Door raw event owner/clock differs",
    )
    require(
        all(
            a["frame"] <= b["frame"] and a["real_seconds"] <= b["real_seconds"]
            for a, b in zip(events, events[1:])
        ),
        "Door raw event order differs",
    )
    for value in [
        *(e["value"] for e in entries),
        *(a[k] for a in actions for k in ("before", "after")),
    ]:
        observed = value["door_release_planning"]["input"]
        require(
            type(observed["arm_frame"]) is int
            and observed["arm_frame"] == raw["arm_frame"]
            and observed["events"] == sum(e["frame"] <= value["frame"] for e in events),
            "Door snapshot differs from the complete every-frame input prefix",
        )
    used = []
    used_transport = []
    placement = None
    previous = contexts[0]["real_seconds"]
    for action in actions:
        before = context(action["before"], case, identity)
        after = context(action["after"], case, identity)
        require(
            pipeline.exact_json(before["fixture"], fixture)
            and pipeline.exact_json(after["fixture"], fixture)
            and previous <= before["real_seconds"] < after["real_seconds"],
            "Door action state/clock differs",
        )
        previous = after["real_seconds"]
        expected_pause = {
            "dev-minimize": (True, True),
            "architect": (True, True),
            "resume": (True, False),
            "select-door": (False, False),
            "placement": (False, False),
            "escape": (False, False),
            "pause": (False, True),
        }[action["label"]]
        require(
            (action["before"]["paused"], action["after"]["paused"]) == expected_pause,
            "Door action violates the existing pause boundary",
        )
        require(
            type(action["first_step"]) is int
            and type(action["last_step"]) is int
            and 1 <= action["first_step"] <= action["last_step"] <= len(inputs),
            "Door action transport interval differs",
        )
        steps = inputs[action["first_step"] - 1 : action["last_step"]]
        used_transport.extend(range(action["first_step"], action["last_step"] + 1))
        gestures = [r for r in steps if r.get("point") is None]
        require(
            len(gestures) == 2 and [r["pressed"] for r in gestures] == [True, False],
            "Door action requires one press/release only",
        )
        selected = [
            (i, e)
            for i, e in enumerate(events)
            if before["real_seconds"] < e["real_seconds"] <= after["real_seconds"]
        ]
        require(
            len(selected) == 2
            and [e["state"] for _, e in selected] == ["Pressed", "Released"],
            "Door actual press/release pair missing/extra",
        )
        for (_, event), gesture in zip(selected, gestures, strict=True):
            require(
                gesture["pre_frame"] < event["frame"] <= gesture["post_frame"],
                "Door actual input frame differs from its exact transport acknowledgement",
            )
        used.extend(i for i, _ in selected)
        escape = action["label"] == "escape"
        for _, event in selected:
            require(
                event["producer"] == ("KeyboardInput" if escape else "MouseButtonInput")
                and (
                    event.get("key") == "Escape" and event.get("repeat") is False
                    if escape
                    else event.get("button") == "Left"
                ),
                "Door action actual input kind/repeat differs",
            )
        require(
            all(
                r.get("key") == "Escape" if escape else r.get("button") == 1
                for r in gestures
            ),
            "Door gesture transport differs",
        )
        if action["label"] in (
            "dev-minimize",
            "architect",
            "select-door",
            "resume",
            "pause",
        ):
            key = {
                "dev-minimize": "dev-minimize",
                "architect": "menu:ToggleArchitect",
                "select-door": "menu:SelectBuild(Door)",
                "resume": "menu:SetTimeSpeed(Normal)",
                "pause": "menu:SetTimeSpeed(Paused)",
            }[action["label"]]
            require(
                gestures[0].get("observed_hover", {}).get(key) == "Pressed",
                "Door actual UI control press ACK differs",
            )
        if action["label"] == "placement":
            require(
                before["play_mode"] == "BuildingPlace"
                and before["build_kind"] == "Door"
                and before["cursor_grid"] == before["anchor"]
                and action["before"]["pointer_over_ui"] is False
                and action["before"]["world_input_captured"] is False,
                "Door placement did not use the actual legal ghost",
            )
            placement = (before, after, selected[0][1])
    require(
        sorted(used) == list(range(len(events)))
        and sorted(used_transport) == list(range(1, len(inputs) + 1)),
        "Door extra, omitted or multiply consumed input",
    )
    # Captured appearance must follow the action that caused it. Matching
    # input counts and ordered screenshots alone do not prove that causality.
    by_label = {action["label"]: action for action in actions}
    architect, select = by_label["architect"], by_label["select-door"]
    place, pause_action = by_label["placement"], by_label["pause"]
    resume_action = by_label["resume"]
    require(
        0
        < pause_action["after"]["door_release_planning"]["real_seconds"]
        - resume_action["before"]["door_release_planning"]["real_seconds"]
        <= 20.0,
        "Door ordinary running interval exceeds the declared twenty-second cap",
    )
    first_press = next(
        step
        for step in range(place["first_step"], place["last_step"] + 1)
        if inputs[step - 1].get("point") is None
    )
    expected_steps = [
        0,
        architect["last_step"],
        first_press - 1,
        pause_action["last_step"],
    ]
    bounds = [
        (None, actions[0]["before"]),
        (architect["after"], resume_action["before"]),
        (select["after"], place["before"]),
        (pause_action["after"], None),
    ]
    for entry, last_step, (lower, upper) in zip(
        entries, expected_steps, bounds, strict=True
    ):
        value = entry["value"]
        observed_time = value["door_release_planning"]["real_seconds"]
        require(
            type(entry.get("last_step")) is int and entry["last_step"] == last_step,
            "Door checkpoint transport prefix does not match its causal action",
        )
        for bound, is_lower in [(lower, True), (upper, False)]:
            if bound is None:
                continue
            bound_time = bound["door_release_planning"]["real_seconds"]
            require(
                (value["frame"] >= bound["frame"] and observed_time >= bound_time)
                if is_lower
                else (value["frame"] <= bound["frame"] and observed_time <= bound_time),
                "Door checkpoint precedes or outlives its causal action interval",
            )
        if last_step:
            require(
                inputs[last_step - 1]["post_frame"] <= value["frame"],
                "Door checkpoint precedes its acknowledged transport prefix",
            )
    require(
        placement is not None and len(raw["receipts"]) == 1,
        "Door ordinary placement receipt must be unique",
    )
    before, after, press = placement
    receipt = raw["receipts"][0]
    outcome = receipt["outcome"]
    bp = contexts[-1]["blueprints"][0]
    require(
        receipt["producer"] == "ordinary-building-placement-branch-v1"
        and receipt["operation"] == "PlacementOutcome"
        and type(receipt["world_epoch"]) is int
        and receipt["world_epoch"] == epoch
        and receipt["window"] == press["window_debug"]
        and finite(receipt["real_seconds"])
        and before["real_seconds"] < receipt["real_seconds"] <= after["real_seconds"]
        and receipt["real_seconds"] >= press["real_seconds"]
        and outcome["kind"] == "Door"
        and outcome["status"] == "BlueprintCommitted"
        and outcome["owner"] == bp["debug_entity"]
        and pipeline.exact_json(outcome["grid"], before["anchor"])
        and pipeline.exact_json(outcome["footprint"], [before["anchor"]]),
        "Door actual domain placement owner/footprint/input binding differs",
    )
    require(
        raw["frame"] >= entries[-1]["value"]["frame"] >= actions[-1]["after"]["frame"]
        and raw["real_seconds"]
        >= contexts[-1]["real_seconds"]
        >= actions[-1]["after"]["door_release_planning"]["real_seconds"],
        "Door journal missing final owned tail",
    )
    return {
        "case": case,
        "axis": CASES[case],
        "events": len(events),
        "input_steps": len(inputs),
        "scope": SCOPE,
        "natural_construction_evidence": False,
        "performance_evidence": False,
        "promotion_authority": False,
    }
