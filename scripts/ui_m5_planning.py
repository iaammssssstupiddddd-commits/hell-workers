"""Ordinary M5 catalog/ghost/Blueprint feedback; never art or release authority."""
from __future__ import annotations

import copy
import json
import math
from pathlib import Path
import shutil

from scripts import building_art_acceptance as art

native, pipeline = art.native, art.pipeline
SCOPE = "ordinary-m5-art-planning-v1"
KINDS = {"m5-sand-planning": "SandPile", "m5-bone-planning": "BonePile",
         "m5-parking-planning": "WheelbarrowParking", "m5-lamp-planning": "OutdoorLamp"}
CHECKPOINTS = tuple("m5-" + phase for phase in ("empty", "catalog", "legal", "placed", "illegal", "selected"))


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def white_color(value):
    # Bevy 0.19 LinearRgba::WHITE -> sRGBA produces RGB 0.9999999403953552.
    # Admit its bounded f32 conversion error, never a gray tint or altered alpha.
    return (isinstance(value, list) and len(value) == 4 and all(finite(x) for x in value)
            and all(abs(x - 1.) <= 1e-6 for x in value[:3]) and value[3] == 1.)


def valid_pose(value):
    return (isinstance(value, dict) and all(isinstance(value.get(key), list) and len(value[key]) == size
            and all(finite(x) for x in value[key]) for key, size in (("translation", 3), ("rotation", 4), ("scale", 3))))


def source_contract(repo, source, codec, case):
    native.require(case in KINDS, "M5 dedicated case required")
    source, codec = pipeline.no_symlinks(source), pipeline.no_symlinks(codec)
    kind = KINDS[case]
    pipeline.verify_set(source, kind, codec)
    manifest, text, receipt = pipeline.load_set(source, kind, codec)
    native.require(manifest["identity"]["authority"] == "art_preview" and receipt is None,
                   "M5 planning admits ArtPreview only, not candidate or release")
    return {"scope": SCOPE, "case": case, "source_root": str(source), "codec": str(codec),
            "codec_sha256": pipeline.digest(codec.read_bytes()), "identity": manifest["identity"],
            "manifest_sha256": pipeline.digest(text.encode()), "artifacts": manifest["artifacts"],
            "world": manifest["world_preview"], "catalog": manifest["catalog_preview"],
            "base_assets_sha256": art.asset_view_fingerprint(repo),
            "verifier_sha256": pipeline.digest(Path(__file__).read_bytes())}


def planned_contract(args, repo):
    source, codec = getattr(args, "m5_art_source", None), getattr(args, "art_codec", None)
    native.require(source and codec and args.case in KINDS and args.smoke
                   and args.input_backend == "portal-eis" and not args.layout_menu
                   and not getattr(args, "rest_area_art_source", None)
                   and not getattr(args, "soul_spa_art_source", None)
                   and not getattr(args, "spa_resume_source", None), "M5 needs its exact bounded EIS source/codec case")
    contract = source_contract(repo, Path(source), Path(codec), args.case)
    if getattr(args, "art_contract_sha256", None):
        native.require(art_contract_digest(contract) == args.art_contract_sha256, "M5 contract changed")
    return contract


def art_contract_digest(contract):
    return pipeline.digest(pipeline.canonical(contract))


def prepare_view(repo, directory, contract):
    import ui_rest_area_art as shared
    native.require(pipeline.exact_json(source_contract(repo, Path(contract["source_root"]), Path(contract["codec"]),
                                                      contract["case"]), contract), "M5 source changed")
    view = shared.checked_view(repo, directory / "asset-view", prepared=False)
    native.require(not view.exists(), "M5 asset view must be fresh")
    shutil.copytree(repo / "assets", view / "assets")
    native.require(art.asset_view_fingerprint(view) == contract["base_assets_sha256"], "M5 base assets changed")
    source = Path(contract["source_root"])
    for entry in contract["artifacts"]:
        pipeline.put(pipeline.rooted(view / "assets", entry["path"]), pipeline.file_bytes(source, entry))
    locator = pipeline.locator(contract["identity"]["kind"])
    pipeline.put(pipeline.rooted(view / "assets", locator), pipeline.rooted(source, locator).read_bytes(), replace=True)
    return view


def check_view(repo, view, contract):
    import ui_rest_area_art as shared
    view = shared.checked_view(repo, view, prepared=True)
    expected = {path.relative_to(repo / "assets").as_posix(): pipeline.digest(path.read_bytes())
                for path in sorted((repo / "assets").rglob("*")) if path.is_file()}
    expected.update({entry["path"]: entry["sha256"] for entry in contract["artifacts"]})
    expected[pipeline.locator(contract["identity"]["kind"])] = contract["manifest_sha256"]
    actual = {}
    for path in sorted((view / "assets").rglob("*")):
        native.require(not path.is_symlink(), "M5 asset view symlink")
        if path.is_file():
            actual[path.relative_to(view / "assets").as_posix()] = pipeline.digest(path.read_bytes())
    native.require(actual == expected, "M5 asset view inventory changed")


def session_setting(directory, nonce, contract):
    return json.dumps({"mode": "m5-feedback", "identity": contract["identity"],
                       "locator": pipeline.locator(contract["identity"]["kind"]), "nonce": nonce,
                       "output": str(directory / "production-cancellation.json")})


def context(value):
    ctx = value.get("m5_planning")
    native.require(isinstance(ctx, dict) and ctx.get("kind") in KINDS.values()
                   and ctx.get("inventory_overflow") is False and ctx.get("failure") is None
                   and ctx.get("primary_window") == value.get("primary_window")
                   and type(ctx.get("world_epoch")) is int and ctx["world_epoch"] == value.get("world_epoch")
                   and ctx.get("camera", {}).get("primary") is True
                   and ctx["camera"].get("viewport") == value.get("viewport"), "M5 context missing/overflow/camera differs")
    anchor = ctx.get("anchor")
    native.require(isinstance(anchor, list) and len(anchor) == 2 and all(type(x) is int for x in anchor), "M5 anchor differs")
    x, y = anchor
    expected = [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]] if ctx["kind"] == "WheelbarrowParking" else [[x, y]]
    native.require(ctx.get("footprint") == expected and [c["grid"] for c in ctx["cells"]] == expected,
                   "M5 actual footprint differs")
    initial = ctx.get("initial_building_inventory")
    native.require(isinstance(initial, list) and len(initial) <= 16
                   and all(type(row.get("entity")) is int for row in initial)
                   and len({row["entity"] for row in initial}) == len(initial)
                   and pipeline.exact_json(ctx.get("building_inventory"), initial)
                   and sorted(ctx["buildings"]) == sorted(row["entity"] for row in initial),
                   "M5 initial owners changed or an extra completed owner appeared")
    return ctx


def check_observation(case, value):
    native.require(case in CHECKPOINTS, "M5 checkpoint differs")
    ctx = context(value)
    native.require(value.get("paused") is (case in ("m5-empty", "m5-selected")), "M5 phase pause differs")
    if case in ("m5-legal", "m5-illegal"):
        native.require(ctx.get("play_mode") == "BuildingPlace" and ctx.get("build_kind") == ctx["kind"]
                       and ctx.get("cursor_grid") == ctx["anchor"]
                       and len([r for r in ctx["consumers"].get("sprites", []) if r.get("consumer") == "ghost"]) == 1,
                       "M5 ghost checkpoint lacks actual placement mode/consumer")
    if case in CHECKPOINTS[:3]:
        native.require(ctx.get("legal") is True and not ctx["blueprints"]
                       and all(c["owner"] is None and c["walkable"] is True for c in ctx["cells"]), "M5 empty/legal state differs")
    else:
        native.require(len(ctx["blueprints"]) == 1 and ctx.get("legal") is False
                       and ctx.get("reject_reason") == "OccupiedByBuilding", "M5 reservation/rejection differs")
        bp = ctx["blueprints"][0]
        native.require(type(bp.get("entity")) is int and bp["footprint"] == ctx["footprint"]
                       and finite(bp.get("progress")) and 0 <= bp["progress"] < 1
                       and all(c["owner"] == bp["entity"] for c in ctx["cells"]), "M5 owner/footprint/progress differs")
    if case == "m5-selected":
        header = value.get("controls", {}).get("text:inspection-header", {})
        native.require(value.get("selected") == ctx["blueprints"][0]["entity"]
                       and header.get("fully_visible") is True
                       and isinstance(header.get("text"), str) and header["text"].strip(), "M5 same Blueprint inspection missing/clipped")


def check_consumers(feedback, contract, phase, owner=None, *, draw_pos=None, initial_inventory=(), initial_world=()):
    identity = contract["identity"]
    native.require(isinstance(feedback, dict) and feedback.get("failure") is None
                   and pipeline.exact_json(feedback.get("requested"), identity)
                   and pipeline.exact_json(feedback.get("active"), identity) and feedback.get("pending") is None
                   and all(feedback.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority")),
                   "M5 consumed generation/authority differs")
    descriptor = feedback.get("descriptor", {})
    native.require(pipeline.exact_json(descriptor.get("identity"), identity)
                   and pipeline.exact_json(descriptor.get("world"), contract["world"])
                   and pipeline.exact_json(descriptor.get("catalog"), contract["catalog"]), "M5 descriptor differs")
    images = descriptor.get("images", {})
    role = contract["world"]["image_role"]
    image = images.get(role)
    native.require(isinstance(image, str) and image, "M5 resident world image missing")
    rows = feedback.get("sprites")
    native.require(isinstance(rows, list) and len(rows) <= 256
                   and len({r.get("entity") for r in rows}) == len(rows), "M5 Sprite inventory differs")
    world_rows = [r for r in rows if r.get("consumer") == "world"]
    native.require(pipeline.exact_json(world_rows, list(initial_world))
                   and {r.get("owner") for r in world_rows} == {r["entity"] for r in initial_inventory},
                   "M5 initial world consumers changed or disappeared")
    for row in rows:
        native.require(row.get("consumer") in ("world", "ghost", "blueprint", "pulse")
                       and type(row.get("entity")) is int and type(row.get("owner")) is int
                       and row.get("image") == image and row.get("whole_image") is True and row.get("size") == contract["world"]["canvas_wu"]
                       and row.get("anchor") == [contract["world"]["anchor_px"][0] / contract["world"]["canvas_px"][0] - 0.5,
                                                 0.5 - contract["world"]["anchor_px"][1] / contract["world"]["canvas_px"][1]]
                       and isinstance(row.get("color"), list) and len(row["color"]) == 4 and all(finite(x) and 0 <= x <= 1 for x in row["color"])
                       and isinstance(row.get("global_matrix"), list) and len(row["global_matrix"]) == 16
                       and all(finite(x) for x in row["global_matrix"]), "M5 actual Sprite image/geometry differs")
        native.require(row.get("layers") == [0] and all(type(x) is int for x in row["layers"]), "M5 consumer render layer differs")
        pose = row.get("pose", {})
        role = row["consumer"]
        if role == "world":
            initial = next(r for r in initial_inventory if r["entity"] == row["owner"])
            owner_pose, owner_global = initial.get("pose"), initial.get("global_matrix")
            native.require(valid_pose(owner_pose) and valid_pose(pose) and owner_pose["rotation"] == [0, 0, 0, 1]
                           and all(x > 0 for x in owner_pose["scale"])
                           and isinstance(owner_global, list) and len(owner_global) == 16
                           and all(finite(x) for x in owner_global) and row.get("global_matrix") == owner_global
                           and (row.get("parent") == row["owner"] and pose == {"translation": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1]}
                                or row.get("parent") is None and row["entity"] == row["owner"] and pipeline.exact_json(pose, owner_pose))
                           and type(row.get("visible")) is bool and white_color(row["color"]),
                           "M5 initial world consumer is not bound to its original owner pose/parent")
            sx, sy, sz = owner_pose["scale"]
            expected_global = [sx, 0, 0, 0, 0, sy, 0, 0, 0, 0, sz, 0, *owner_pose["translation"], 1]
            native.require(all(abs(x - y) < 1e-5 for x, y in zip(owner_global, expected_global, strict=True)),
                           "M5 initial owner local/global binding differs")
            continue
        progress = row.get("blueprint_progress")
        if role != "ghost":
            native.require(finite(progress) and 0 <= progress < 1 and row.get("blueprint_state") in
                           ("NeedsMaterials", "Preparing", "ReadyToBuild", "Building"), "M5 actual visual mirror missing")
        scale = 1. if role == "ghost" else .9 + .1 * progress
        native.require(isinstance(draw_pos, list) and len(draw_pos) == 2 and all(finite(x) for x in draw_pos), "M5 consumer owner position missing")
        translation = [*draw_pos, 2. if role == "ghost" else .2]
        native.require(pose.get("rotation") == [0, 0, 0, 1]
                       and isinstance(pose.get("translation"), list) and len(pose["translation"]) == 3
                       and isinstance(pose.get("scale"), list) and len(pose["scale"]) == 3
                       and all(finite(x) for x in (*pose["translation"], *pose["scale"])), "M5 consumer pose missing")
        if role == "pulse":
            native.require(row.get("parent") == owner and pose["translation"] == [0, 0, 0]
                           and pose["scale"] == [1, 1, 1], "M5 pulse parent/local pose differs")
        else:
            native.require(row.get("parent") is None and (row["entity"] == row["owner"])
                           and all(abs(x - y) < 1e-5 for x, y in zip(pose["translation"], translation, strict=True))
                           and all(abs(x - scale) < 1e-5 for x in pose["scale"]), "M5 root owner/local pose differs")
        matrix = [scale, 0, 0, 0, 0, scale, 0, 0, 0, 0, scale, 0, *translation, 1]
        native.require(all(abs(x - y) < 1e-5 for x, y in zip(row["global_matrix"], matrix, strict=True)), "M5 actual global pose differs from owner")
        if row["consumer"] in ("blueprint", "pulse"):
            native.require(owner is not None and row["owner"] == owner, "M5 foreign Blueprint consumer")
    if phase in ("m5-legal", "m5-illegal"):
        ghosts = [r for r in rows if r["consumer"] == "ghost"]
        native.require(len(ghosts) == 1 and ghosts[0].get("visible") is True, "M5 actual visible ghost missing")
        expected = [0.5, 1, 0.5, 0.5] if phase == "m5-legal" else [1, 0.2, 0.2, 0.5]
        native.require(all(abs(x - y) < 1e-5 for x, y in zip(ghosts[0]["color"], expected, strict=True)),
                       "M5 validity color was changed or not observed")
    if phase in CHECKPOINTS[3:]:
        blueprints = [r for r in rows if r["consumer"] == "blueprint"]
        pulses = [r for r in rows if r["consumer"] == "pulse"]
        native.require(len(blueprints) == 1, "M5 Blueprint inventory incomplete")
        root = blueprints[0]
        building = root["blueprint_state"] == "Building"
        native.require(len(pulses) == int(building), "M5 pulse must match the actual construction state")
        displayed = pulses[0] if building else root
        native.require(displayed.get("visible") is True and displayed["color"][3] > 0,
                       "M5 actual Blueprint presentation hidden")
        if building:
            native.require(root["color"][3] == 0 and all(abs(x - 1) < 1e-5 for x in displayed["color"][:3]), "M5 building pulse color differs")
        else:
            native.require(all(abs(x - y) < 1e-5 for x, y in zip(root["color"][:3], [.1, .5, 1], strict=True)), "M5 Blueprint construction color differs")
    if phase == "m5-catalog":
        cards = feedback.get("catalog", [])
        native.require(len(cards) == 1 and cards[0].get("image") == images.get(contract["catalog"]["image_role"])
                       and cards[0].get("fully_visible") is True and cards[0].get("whole_image") is True
                       and white_color(cards[0].get("color"))
                       and isinstance(cards[0].get("physical_rect"), list) and len(cards[0]["physical_rect"]) == 4
                       and all(finite(x) for x in cards[0]["physical_rect"])
                       and 0 <= cards[0]["physical_rect"][0] < cards[0]["physical_rect"][2] <= 1920
                       and 0 <= cards[0]["physical_rect"][1] < cards[0]["physical_rect"][3] <= 1080,
                       "M5 actual catalog image is absent/hidden/clipped")


def check_world_action(value, key, contract, baseline):
    ctx = context(value)
    native.require(key in ("placement", "selection") and ctx["kind"] == contract["identity"]["kind"]
                   and all(ctx[k] == baseline[k] for k in ("anchor", "footprint", "draw_pos"))
                   and pipeline.exact_json(ctx["initial_building_inventory"], baseline["initial_building_inventory"])
                   and ctx["world_epoch"] == baseline["world_epoch"]
                   and ctx["primary_window"] == baseline["primary_window"]
                   and ctx.get("cursor_grid") == baseline["anchor"]
                   and value.get("pointer_over_ui") is False and value.get("world_input_captured") is False
                   and ctx.get("play_mode") == ("BuildingPlace" if key == "placement" else "Normal"),
                   "M5 world target/mode changed; no press permitted")
    if key == "placement":
        native.require(ctx.get("legal") is True and not ctx["blueprints"]
                       and ctx.get("build_kind") == contract["identity"]["kind"], "M5 placement is no longer legal")
    else:
        native.require(ctx.get("legal") is False and len(ctx["blueprints"]) == 1
                       and ctx["blueprints"][0]["entity"] == baseline.get("blueprint_owner")
                       and value.get("pointer_target") == ctx["blueprints"][0]["entity"],
                       "M5 actual selection resolver targets another object; no press permitted")


def capture_action(driver, key, *, world=False, contract=None, baseline=None):
    if not world and key != "Escape":
        driver.send(point=driver.point(key))
        native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered", "M5 control not hovered; no press")
    before, first = copy.deepcopy(driver.last), len(driver.events)
    if world:
        check_world_action(before, key, contract, baseline)
    if key == "Escape":
        driver.key(key)
    else:
        native.require(not world or not before["pointer_over_ui"] and not before["world_input_captured"], "M5 world input captured")
        driver.send(button=1, pressed=True)
        if not world:
            native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed", "M5 control press missing")
        driver.send(button=1, pressed=False)
    # Last-stage raw may trail the UI ACK. Wait for the original pair to flush;
    # never resend an event or widen an execution deadline.
    def flushed(value):
        path = driver.root / "production-cancellation.json"
        if not path.is_file():
            return False
        raw = native.read_json(path)
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "M5 raw flush owner/failure differs")
        rows = [r for r in raw.get("events", []) if r.get("producer") == ("KeyboardInput" if key == "Escape" else "MouseButtonInput")
                and context(before)["real_seconds"] < r.get("real_seconds", -1) <= context(value)["real_seconds"]]
        return len(rows) == 2 and [r.get("state") for r in rows] == ["Pressed", "Released"]
    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["m5_action"] = {"key": key, "first_step": first + 1,
        "last_step": len(driver.events), "before": before, "after": copy.deepcopy(driver.last)}
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def check_transport(inputs, client, epoch):
    native.require(isinstance(inputs, list) and 0 < len(inputs) <= 160
                   and [r.get("step") for r in inputs] == [str(i + 1) for i in range(len(inputs))], "M5 input bounds/steps differ")
    for row in inputs:
        native.require(row.get("nonce") == client["nonce"]
                       and all(type(row.get(key)) is int for key in ("pid", "window", "world_epoch", "pre_frame", "post_frame"))
                       and row["pid"] == client["pid"] and row["window"] == client["window"]
                       and row["world_epoch"] == epoch and 0 <= row["pre_frame"] < row["post_frame"]
                       and sum(row.get(key) is not None for key in ("point", "key", "button")) == 1,
                       "M5 transport receipt identity/type differs")
        if row.get("point") is not None:
            point, cursor = row["point"], row.get("observed_cursor")
            native.require(isinstance(point, (list, tuple)) and len(point) == 2
                           and all(type(axis) is int and 0 < axis < limit for axis, limit in zip(point, [1920, 1080], strict=True))
                           and row.get("pressed") is None and "m5_action" not in row
                           and isinstance(cursor, list) and len(cursor) == 2 and all(finite(x) for x in cursor)
                           and all(abs(x - y) <= 2 for x, y in zip(point, cursor, strict=True)), "M5 motion ACK/coordinate differs")
    native.require(sum(r.get("point") is not None for r in inputs) <= 16
                   and all(a["pre_frame"] <= b["pre_frame"] and a["post_frame"] <= b["post_frame"]
                           for a, b in zip(inputs, inputs[1:])), "M5 motion cap/transport order differs")


def verify(raw, entries, client, inputs, contract):
    from scripts import building_production_lifecycle as lifecycle
    native.require(contract.get("scope") == SCOPE and contract.get("case") in KINDS
                   and contract["identity"].get("kind") == KINDS[contract["case"]]
                   and contract["identity"].get("authority") == "art_preview", "M5 admission differs")
    native.require(raw.get("scope") == SCOPE and raw.get("schema_version") == 2 and raw.get("failure") is None
                   and raw.get("nonce") == client["nonce"] and raw.get("process_id") == client["pid"]
                   and all(raw.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority")), "M5 raw owner/scope/authority differs")
    samples = raw.get("samples", [])
    native.require(0 < len(samples) <= 1200 and len(raw.get("events", [])) < 4096
                   and all(type(s.get("sample_index")) is int and s["sample_index"] == i
                           and finite(s.get("real_seconds")) for i, s in enumerate(samples))
                   and all(a["real_seconds"] < b["real_seconds"] for a, b in zip(samples, samples[1:])), "M5 raw inventory/clock differs")
    for i, event in enumerate(raw["events"]):
        index = event.get("sample_index")
        native.require(type(event.get("event_index")) is int and event["event_index"] == i
                       and type(index) is int and 0 <= index < len(samples)
                       and finite(event.get("real_seconds")) and 0 <= event["real_seconds"] <= samples[index]["real_seconds"]
                       and event.get("producer") != "MouseWheel", "M5 raw event ordinal/sample/unexpected wheel differs")
    native.require([e.get("case") for e in entries] == list(CHECKPOINTS), "M5 checkpoint sequence differs")
    first, owner = copy.deepcopy(context(entries[0]["value"])), context(entries[3]["value"])["blueprints"][0]["entity"]
    first["blueprint_owner"] = owner
    initial_inventory = first["initial_building_inventory"]
    initial_world = [r for r in first["consumers"]["sprites"] if r.get("consumer") == "world"]
    for entry in entries:
        value, phase = entry["value"], entry["case"]
        ctx = context(value)
        check_observation(phase, value)
        native.require(value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"]
                       and all(type(value.get(key)) is int for key in ("pid", "world_epoch", "primary_window", "frame"))
                       and value.get("primary_window") == entries[0]["value"]["primary_window"]
                       and value.get("world_epoch") == entries[0]["value"]["world_epoch"]
                       and ctx["kind"] == contract["identity"]["kind"]
                       and pipeline.exact_json(ctx["initial_building_inventory"], initial_inventory)
                       and all(ctx[key] == first[key] for key in ("anchor", "footprint", "draw_pos")), "M5 checkpoint subject/position differs")
        if phase in CHECKPOINTS[3:]:
            native.require(ctx["blueprints"][0]["entity"] == owner, "M5 Blueprint owner changed")
        if phase != "m5-empty" or initial_inventory:
            if phase in ("m5-legal", "m5-illegal"):
                native.require(ctx.get("cursor_grid") == ctx["anchor"] and ctx.get("play_mode") == "BuildingPlace"
                               and ctx.get("build_kind") == contract["identity"]["kind"], "M5 cursor/mode is not on the target anchor")
            check_consumers(ctx["consumers"], contract, phase, owner if phase in CHECKPOINTS[3:] else None,
                            draw_pos=ctx["draw_pos"], initial_inventory=initial_inventory, initial_world=initial_world)
            near = [s for s in samples if s.get("world_epoch") == value["world_epoch"]
                    and abs(s["real_seconds"] - ctx["real_seconds"]) <= 1
                    and finite(s.get("virtual_seconds"))
                    and abs(s["virtual_seconds"] - ctx["virtual_seconds"]) <= (0 if value["paused"] else 1)]
            native.require(near, "M5 checkpoint lacks same-clock raw sample")
            # Validate an independent raw view as well as the UI view. Pulse
            # alpha is real-time animated; it must not be copied or whitened.
            nearest = min(near, key=lambda s: abs(s["real_seconds"] - ctx["real_seconds"]))
            native.require(nearest.get("primary_window_bits") == value.get("primary_window"), "M5 raw primary window differs")
            check_consumers(nearest.get("art_feedback"), contract, phase, owner if phase in CHECKPOINTS[3:] else None,
                            draw_pos=ctx["draw_pos"], initial_inventory=initial_inventory, initial_world=initial_world)
    native.require(all(b["value"]["frame"] > a["value"]["frame"] and b["last_step"] > a["last_step"]
                       for a, b in zip(entries, entries[1:])), "M5 checkpoint lacks new input")
    start, end = first["real_seconds"], context(entries[-1]["value"])["real_seconds"]
    tail = [s for s in samples if start <= s["real_seconds"] <= end]
    native.require(tail and tail[0]["real_seconds"] <= start + 1 and tail[-1]["real_seconds"] >= end - 1,
                   "M5 initial-owner raw interval is missing")
    for sample in tail:
        feedback = sample.get("art_feedback", {})
        native.require(sample.get("world_epoch") == first["world_epoch"]
                       and sample.get("primary_window_bits") == first["primary_window"]
                       and pipeline.exact_json(feedback.get("active"), contract["identity"])
                       and pipeline.exact_json(feedback.get("requested"), contract["identity"])
                       and feedback.get("pending") is None
                       and all(feedback.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority"))
                       and pipeline.exact_json([r for r in feedback.get("sprites", []) if r.get("consumer") == "world"], initial_world),
                       "M5 initial world consumer changed in an intermediate raw sample")
        owners = sample.get("owners")
        native.require(isinstance(owners, list), "M5 raw owner inventory missing")
        current = [r for r in owners if r.get("kind") == contract["identity"]["kind"]]
        native.require(len(current) == len(initial_inventory)
                       and sorted(r.get("entity_bits", -1) for r in current) == sorted(r["entity"] for r in initial_inventory),
                       "M5 intermediate raw owner added/removed/replaced")
        for row in current:
            initial = next(r for r in initial_inventory if r["entity"] == row["entity_bits"])
            native.require(type(row.get("entity_bits")) is int
                           and pipeline.exact_json({"translation": row.get("position"), "rotation": row.get("rotation"), "scale": row.get("scale")}, initial["pose"]),
                           "M5 intermediate raw owner pose changed")
    check_transport(inputs, client, entries[0]["value"]["world_epoch"])
    actions = [r["m5_action"] for r in inputs if "m5_action" in r]
    keys = [a.get("key") for a in actions]
    select = f"menu:SelectBuild({contract['identity']['kind']})"
    expected_prefix = ["menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect", select, "placement"]
    expected_tail = ["Escape", "menu:SetTimeSpeed(Paused)", "selection"]
    native.require(keys in [optional + expected_prefix + reselect + expected_tail
                           for optional in ([], ["dev-minimize"]) for reselect in ([], [select], ["menu:ToggleArchitect", select])],
                   "M5 input action sequence differs")
    claimed, raw_claimed = set(), set()
    phase_actions = [["menu:SetTimeSpeed(Normal)", "menu:ToggleArchitect"], [select],
                     ["placement"], [], ["Escape", "menu:SetTimeSpeed(Paused)", "selection"]]
    for phase_index, (lower, upper) in enumerate(zip(entries, entries[1:])):
        group = [a for a in actions if lower["last_step"] < a["first_step"] <= upper["last_step"]]
        expected = phase_actions[phase_index]
        native.require([a["key"] for a in group] == expected or phase_index == 3
                       and [a["key"] for a in group] in ([select], ["menu:ToggleArchitect", select]), "M5 action belongs to wrong checkpoint interval")
        for action in group:
            native.require(action["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= action["before"]["frame"] < action["after"]["frame"] < upper["value"]["frame"]
                           and context(lower["value"])["real_seconds"] <= context(action["before"])["real_seconds"]
                           < context(action["after"])["real_seconds"] < context(upper["value"])["real_seconds"], "M5 action crosses phase boundary")
    for action in actions:
        a, b, key = action["before"], action["after"], action["key"]
        first_step, last_step = action["first_step"], action["last_step"]
        native.require(type(first_step) is int and type(last_step) is int and last_step == first_step + 1
                       and 1 <= first_step < last_step <= len(inputs)
                       and not claimed.intersection({first_step, last_step})
                       and inputs[first_step - 1].get("m5_action") == action, "M5 input action reused/unbounded")
        claimed.update((first_step, last_step))
        start, end = context(a)["real_seconds"], context(b)["real_seconds"]
        native.require(finite(start) and finite(end) and start < end and a["frame"] < b["frame"]
                       and all(v.get("nonce") == client["nonce"] and v.get("pid") == client["pid"]
                               and v.get("world_epoch") == entries[0]["value"]["world_epoch"]
                               and v.get("primary_window") == entries[0]["value"]["primary_window"] for v in (a, b)), "M5 action clock/owner differs")
        sent = inputs[first_step - 1:last_step]
        keyboard = key == "Escape"
        native.require([r.get("pressed") for r in sent] == [True, False]
                       and all(type(r.get("pressed")) is bool and r.get("nonce") == client["nonce"]
                               and r.get("pid") == client["pid"] and r.get("window") == client["window"]
                               and r.get("world_epoch") == a["world_epoch"]
                               and a["frame"] <= r.get("pre_frame", -1) < r.get("post_frame", -1) <= b["frame"]
                               and (r.get("key") == key if keyboard else type(r.get("button")) is int and r["button"] == 1)
                               for r in sent), "M5 actual sent pair differs")
        rows = [r for r in raw["events"] if r.get("producer") == ("KeyboardInput" if keyboard else "MouseButtonInput")
                and start < r.get("real_seconds", -1) <= end]
        native.require(len(rows) == 2 and [r.get("state") for r in rows] == ["Pressed", "Released"], "M5 raw input pair missing")
        for row in rows:
            index = row.get("sample_index")
            native.require(type(index) is int and 0 <= index < len(samples) and row.get("event_index") not in raw_claimed,
                           "M5 raw input reused/outside samples")
            raw_claimed.add(row["event_index"])
            sample = samples[index]
            native.require(sample.get("world_epoch") == a["world_epoch"]
                           and sample.get("primary_window_bits") == a["primary_window"]
                           and row.get("window") == sample.get("primary_window")
                           and sample["real_seconds"] >= row["real_seconds"]
                           and (row.get("key") == "Escape" and row.get("repeat") is False if keyboard else row.get("button") == "Left"), "M5 raw event identity differs")
            if not keyboard:
                cursor, dpi = sample.get("cursor_position"), sample.get("dpi")
                native.require(isinstance(cursor, list) and len(cursor) == 2 and all(finite(x) for x in cursor)
                               and finite(dpi) and dpi == a.get("scale_factor") and dpi > 0
                               and all(abs(x * dpi - y) <= 2 for x, y in zip(cursor, a["cursor"], strict=True)), "M5 raw cursor differs")
        if key.startswith("menu:") or key == "dev-minimize":
            control = a["controls"].get(key, {})
            native.require(control.get("fully_visible") is True and control.get("interaction") == "Hovered"
                           and sent[0].get("observed_hover", {}).get(key) == "Pressed", "M5 control was not the actual pressed target")
        if key in ("placement", "selection"):
            check_world_action(a, key, contract, first)
    native.require(all(i + 1 in claimed for i, r in enumerate(inputs) if r.get("button") is not None or r.get("key") is not None)
                   and raw_claimed == {r["event_index"] for r in raw["events"] if r.get("producer") in ("KeyboardInput", "MouseButtonInput")}, "M5 extraneous/unclaimed input")
    receipts = [r for r in raw["events"] if r.get("producer") == "ordinary-building-placement-branch-v1"]
    native.require(len(receipts) == 1, "M5 placement receipt missing/duplicate")
    receipt = receipts[0]
    lifecycle.check_placement_receipt(receipt, samples)
    outcome = receipt["outcome"]
    placement = next(a for a in actions if a["key"] == "placement")
    presses = [r for r in raw["events"] if r.get("producer") == "MouseButtonInput" and r.get("state") == "Pressed"
               and context(placement["before"])["real_seconds"] < r.get("real_seconds", -1) <= context(placement["after"])["real_seconds"]]
    native.require(len(presses) == 1 and receipt["real_seconds"] >= presses[0]["real_seconds"]
                   and receipt["sample_index"] >= presses[0]["sample_index"], "M5 receipt precedes actual placement press")
    native.require(outcome.get("kind") == contract["identity"]["kind"] and outcome.get("status") == "BlueprintCommitted"
                   and outcome.get("owner") == context(entries[3]["value"])["blueprints"][0]["debug_entity"] and outcome.get("grid") == first["anchor"]
                   and outcome.get("footprint") == first["footprint"]
                   and context(placement["before"])["real_seconds"] < receipt["real_seconds"] <= context(placement["after"])["real_seconds"], "M5 actual placement outcome differs")


def exercise(driver, contract):
    baseline = copy.deepcopy(context(driver.last))
    if "text-field:DevPoc" in driver.last["controls"]:
        capture_action(driver, "dev-minimize")
    driver.capture("m5-empty")
    capture_action(driver, "menu:SetTimeSpeed(Normal)")
    capture_action(driver, "menu:ToggleArchitect")
    driver.capture("m5-catalog")
    select = f"menu:SelectBuild({contract['identity']['kind']})"
    capture_action(driver, select)
    def point():
        projected = context(driver.last)["point"]
        native.require(isinstance(projected, list) and len(projected) == 2
                       and all(finite(x) and 0 < x < bound for x, bound in zip(projected, driver.last["viewport"], strict=True)), "M5 point outside window")
        factor = driver.last["scale_factor"]
        native.require(finite(factor) and factor > 0, "M5 physical scale missing")
        driver.send(point=tuple(round(x * factor) for x in projected))
        native.require(context(driver.last).get("cursor_grid") == context(driver.last)["anchor"], "M5 actual cursor/grid differs")
        native.require(not driver.last["pointer_over_ui"] and not driver.last["world_input_captured"], "M5 point covered")
    point()
    driver.capture("m5-legal")
    capture_action(driver, "placement", world=True, contract=contract, baseline=baseline)
    driver.last = driver.wait(lambda v: len(context(v)["blueprints"]) == 1, after=driver.last["frame"])
    driver.capture("m5-placed")
    baseline["blueprint_owner"] = context(driver.last)["blueprints"][0]["entity"]
    # Explicit real selection, never assign BuildContext or spawn a ghost.
    if context(driver.last).get("play_mode") != "BuildingPlace" or context(driver.last).get("build_kind") != contract["identity"]["kind"]:
        if select not in driver.last["controls"]:
            capture_action(driver, "menu:ToggleArchitect")
        capture_action(driver, select)
    point()
    driver.capture("m5-illegal")
    capture_action(driver, "Escape")
    capture_action(driver, "menu:SetTimeSpeed(Paused)")
    point()
    capture_action(driver, "selection", world=True, contract=contract, baseline=baseline)
    owner = context(driver.last)["blueprints"][0]["entity"]
    driver.last = driver.wait(lambda v: v.get("selected") == owner
                              and v.get("controls", {}).get("text:inspection-header", {}).get("fully_visible") is True,
                              after=driver.last["frame"])
    driver.capture("m5-selected")
