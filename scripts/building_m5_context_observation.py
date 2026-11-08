"""Seeded M5 visual context only; never ordinary placement, power or art authority."""
import hashlib
import math
from pathlib import Path

if __package__:
    from . import building_foreground_observation as foreground
else:
    import building_foreground_observation as foreground

require, vector, canonical, close = foreground.require, foreground.vector, foreground.canonical, foreground.close
SCOPE = "seeded-paused-art-comparison"
LAMP_UI_SCOPE = "seeded-paused-lamp-supply-ui"


def verifier_sha256():
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def settings(value):
    lamp_ui = value.get("variant") == "lamp-supply-art-context"
    require(value.get("variant") in ("m5-art-context", "lamp-supply-art-context") and value.get("kind") in foreground.KINDS
            and value.get("identity", {}).get("kind") == value["kind"]
            and value["identity"].get("authority") == "art_preview"
            and value.get("mode") in ("feedback", "art-preview") and value.get("scope") == (LAMP_UI_SCOPE if lamp_ui else SCOPE)
            and value.get("view") in (("normal", "overview") if lamp_ui else ("normal",)) and value.get("lighting") == "standard"
            and value.get("spa_mask_page") is None and value.get("promotion_authority") is False
            and value.get("m5_context_verifier_sha256") == verifier_sha256(), "M5 context plan/authority differs")
    target = value.get("lamp_supply_target")
    require((type(target) is int and target in (0, 2) and value["kind"] == "OutdoorLamp" and value["mode"] == "feedback")
            if lamp_ui else target is None, "Lamp UI target/kind/mode differs")
    require(foreground.settings(value, context=True) is not None, "M5 context requires actual foreground observations")


def point(value):
    vector(value, 2)
    require(0 <= value[0] <= 1280 and 0 <= value[1] <= 720, "M5 context projected point outside window")


def layers(actual, expected):
    return isinstance(actual, list) and all(type(item) is int for item in actual) and actual == expected


def meshes(rows, owner, expected_pose, *, actor):
    require(isinstance(rows, list) and 0 < len(rows) <= 16 and (not actor or len(rows) == 1), "M5 context mesh inventory differs")
    identities = []
    for row in rows:
        require(type(row.get("entity")) is int and row["entity"] > 0 and type(row.get("root")) is int
                and row["root"] > 0 and type(row.get("owner")) is int and row["owner"] == owner
                and isinstance(row.get("mesh"), str) and bool(row["mesh"])
                and layers(row.get("layers"), [1] if actor else [1, 5]) and type(row.get("visible")) is bool,
                "M5 context mesh ownership/layer differs")
        local = foreground.matrix(row["pose"])
        root = foreground.matrix(row["root_pose"])
        require(close(row["root_pose"]["translation"], expected_pose["translation"])
                and close(row["root_pose"]["rotation"], expected_pose["rotation"])
                and close(row["root_pose"]["scale"], expected_pose["scale"]), "M5 renderer is detached from logical owner")
        expected_global = root if row["entity"] == row["root"] else foreground.product(root, local)
        require((row["entity"] != row["root"] or close(local, root))
                and close(row["global_matrix"], expected_global), "M5 renderer parent/global pose differs")
        material = row.get("material", {})
        require(material.get("kind") == ("standard" if actor else "structural")
                and isinstance(material.get("handle"), str) and bool(material["handle"])
                and isinstance(material.get("values"), str) and 0 < len(material["values"]) <= 16384,
                "M5 context actual material missing")
        vector(material.get("base_color"), 4)
        identities.append(row["entity"])
    require(len(set(identities)) == len(identities) and any(row["visible"] for row in rows), "M5 context mesh duplicate/invisible")
    return identities


def power_context(fixture, value, logical_ids):
    lamp = value["kind"] == "OutdoorLamp"
    require(type(fixture.get("source_count")) is int and fixture["source_count"] == (2 if lamp else 0)
            and type(fixture.get("source_workers")) is int and fixture["source_workers"] == (6 if lamp else 0),
            "M5 source inventory differs")
    power = fixture.get("power")
    if not lamp:
        require(power is None, "Non-Lamp context cannot acquire a power fixture")
        return
    require(isinstance(power, dict) and type(power.get("source_count")) is int and power["source_count"] == 2
            and type(power.get("worker_count")) is int and power["worker_count"] == 6
            and len(power.get("records", [])) == 4, "Lamp source power inventory differs")
    used, grids, workers = set(logical_ids), set(), set()
    def entity(number):
        require(type(number) is int and number > 0 and number not in used, "Lamp source entity reused/invalid")
        used.add(number)
        return number
    for ordinal, (row, target) in enumerate(zip(power["records"], fixture["records"], strict=True)):
        grid = entity(row["grid"])
        grids.add(grid)
        entity(row["yard"])
        demand = row.get("demand")
        require(type(row.get("ordinal")) is int and row["ordinal"] == ordinal and type(row.get("lamp")) is int
                and row["lamp"] == target["owner"] and layers(row.get("consumers"), [target["owner"]])
                and type(demand) in (float, int) and abs(demand - .2) <= 1e-6
                and type(row.get("consumption")) in (float, int) and abs(row["consumption"] - demand) <= 1e-6
                and row.get("powered") is (ordinal >= 2), "Lamp actual allocation differs")
        low, high = vector(row["yard_min"], 2), vector(row["yard_max"], 2)
        require(all(a <= n <= b for a, n, b in zip(low, target["center"], high, strict=True)), "Lamp foreign Yard")
        if ordinal < 2:
            require(row.get("generators") == [] and row.get("source") is None
                    and type(row.get("generation")) in (float, int) and row["generation"] == 0, "Off Lamp has supply")
            continue
        source = row["source"]
        owner = entity(source["entity"])
        mask, count = (3, 2) if ordinal == 2 else (15, 4)
        require(type(source.get("ordinal")) is int and source["ordinal"] == ordinal
                and type(source.get("mask")) is int and source["mask"] == mask and source.get("phase") == "Operational"
                and type(source.get("grid")) is int and source["grid"] == grid and layers(row.get("generators"), [owner])
                and type(source.get("worker_count")) is int and source["worker_count"] == count
                and type(source.get("output_per_soul")) in (float, int) and source["output_per_soul"] == 1
                and type(source.get("output")) in (float, int) and source["output"] == count
                and type(row.get("generation")) in (float, int) and row["generation"] == count
                and len(source.get("tiles", [])) == 4, "Lamp real source differs")
        anchor = [8 + ordinal * 5, 23]
        for index, (tile, offset) in enumerate(zip(source["tiles"], ((0, 0), (1, 0), (0, -1), (1, -1)), strict=True)):
            tile_id = entity(tile["entity"])
            position = [anchor[0] + offset[0], anchor[1] + offset[1]]
            world_position = [(n + .5) * 32 - 1600 for n in position]
            require(type(tile.get("parent")) is int and tile["parent"] == owner and layers(tile.get("grid"), position)
                    and type(tile.get("map_owner")) is int and tile["map_owner"] == owner
                    and all(a <= n <= b for a, n, b in zip(low, world_position, high, strict=True)), "Lamp source tile/Yard differs")
            expected = int(bool(mask & (1 << index)))
            require(len(tile.get("workers", [])) == len(tile.get("worker_observations", [])) == expected,
                    "Lamp exact active slots differ")
            for worker, observation in zip(tile["workers"], tile["worker_observations"], strict=True):
                entity(worker)
                workers.add(worker)
                require(type(observation.get("entity")) is int and observation["entity"] == worker
                        and type(observation.get("working_on")) is int and observation["working_on"] == tile_id
                        and type(observation.get("task_tile")) is int and observation["task_tile"] == tile_id
                        and observation.get("task_phase") == "Generating" and close(observation["position"], world_position)
                        and type(observation.get("dream")) in (float, int) and observation["dream"] == 100,
                        "Lamp task/worker relationship differs")
    require(len(grids) == 4 and len(workers) == 6, "Lamp source graph differs")


def validate(probe, value):
    settings(value)
    foreground.validate(probe, value, context=True)
    fixture = probe["fixture"]
    lamp_ui = value.get("variant") == "lamp-supply-art-context"
    camera_scale = 5 if value["view"] == "overview" else 1
    require(probe.get("variant") == fixture.get("variant") == value["variant"]
            and probe.get("lighting") == fixture.get("lighting") == "standard"
            and probe.get("scope") == fixture.get("scope") == value["scope"]
            and canonical(fixture.get("identity")) == canonical(value["identity"])
            and fixture.get("view") == value["view"] and type(fixture.get("camera_scale")) in (int, float)
            and fixture["camera_scale"] == camera_scale
            and all(fixture.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority")),
            "M5 context runtime/authority differs")
    require(all(type(fixture.get(key)) is int and fixture[key] == count for key, count in
                (("target_count", 4), ("wall_count", 8), ("preview_count", 4), ("idle_souls", 8),
                 ("souls", 14 if value["kind"] == "OutdoorLamp" else 8)))
            and len(fixture.get("records", [])) == 4, "M5 context inventory differs")
    descriptor = value["foreground_observation"]["descriptor"]
    camera = fixture.get("rtt_camera", {})
    require(type(camera.get("entity")) is int and camera["entity"] > 0
            and camera.get("active") is True and layers(camera.get("layers"), [1]), "M5 RTT camera differs")
    foreground.matrix(camera["pose"])
    main, composite = fixture.get("main_camera", {}), fixture.get("composite", {})
    require(type(main.get("entity")) is int and main["entity"] > 0 and main.get("target") == "primary-window"
            and layers(main.get("viewport"), [1280, 720]), "M5 main output window differs")
    foreground.matrix(main["pose"])
    vector(camera.get("viewport"), 2)
    require(all(n > 0 for n in camera["viewport"]) and isinstance(camera.get("projection"), str)
            and bool(camera["projection"]) and isinstance(camera.get("scene_image"), str) and bool(camera["scene_image"])
            and type(composite.get("entity")) is int and composite["entity"] > 0 and composite.get("visible") is True
            and layers(composite.get("layers"), [2]) and composite.get("scene_image") == camera["scene_image"]
            and isinstance(composite.get("material"), str) and bool(composite["material"]), "M5 RTT/composite output chain differs")
    compensation = math.hypot(150, 90) / 150
    require(type(composite.get("vertical_compensation")) in (float, int)
            and abs(composite["vertical_compensation"] - compensation) <= .001
            and close(composite["pose"]["translation"], [0., 0., 20.])
            and composite["pose"]["rotation"] == [0., 0., 0., 1.]
            and close(composite["pose"]["scale"], [1280., 720 * compensation, 1.]), "M5 composite geometry differs")
    foreground.matrix(composite["pose"])
    preview_image = fixture["records"][0]["foreground"]["image_handle"]
    identities, occupied, mesh_ids = [camera["entity"], main["entity"], composite["entity"]], set(), []
    row = {"WheelbarrowParking": 28, "SandPile": 33, "BonePile": 38, "OutdoorLamp": 48}[value["kind"]]
    centers = []
    for ordinal, record in enumerate(fixture["records"]):
        require(record.get("ordinal") == ordinal and record.get("kind") == value["kind"], "M5 context specimen order differs")
        owner = record["owner"]
        identities.extend((owner, record["foreground"]["entity"]))
        center = vector(record["center"], 2)
        anchor = record.get("anchor")
        require(isinstance(anchor, list) and len(anchor) == 2 and all(type(n) is int for n in anchor), "M5 context anchor differs")
        require(anchor == [8 + ordinal * 5, row], "M5 context canonical layout anchor differs")
        extra = 16 if value["kind"] == "WheelbarrowParking" else 0
        expected_center = [(n + .5) * 32 - 1600 + extra for n in anchor]
        require(close(center, expected_center), "M5 context world-grid center differs")
        centers.append(expected_center)
        offsets = ((0, 0), (1, 0), (0, 1), (1, 1)) if value["kind"] == "WheelbarrowParking" else ((0, 0),)
        grids = {(anchor[0] + x, anchor[1] + y) for x, y in offsets}
        tiles = record.get("tiles")
        require(isinstance(tiles, list) and len(tiles) == len(grids)
                and all(isinstance(tile.get("grid"), list) and len(tile["grid"]) == 2
                        and all(type(n) is int for n in tile["grid"]) and tile.get("terrain") in ("Grass", "Dirt", "Sand") for tile in tiles)
                and {tuple(tile["grid"]) for tile in tiles} == grids and not occupied.intersection(grids), "M5 actual terrain/footprint differs")
        occupied.update(grids)
        color = record["foreground"]["color_srgba"]
        require(all(abs(channel - 1) <= 1e-6 for channel in color), "M5 context foreground is not white opaque")
        walls = record.get("walls")
        expected = [(anchor[0], min(y for _, y in grids) - 1), (anchor[0], max(y for _, y in grids) + 1)]
        require(isinstance(walls, list) and len(walls) == 2, "M5 context wall count differs")
        for wall, grid in zip(walls, expected, strict=True):
            require(wall.get("grid") == list(grid) and grid not in occupied, "M5 front/back wall grid differs")
            occupied.add(grid)
            require(type(wall.get("entity")) is int and wall["entity"] > 0, "M5 context wall identity differs")
            identities.append(wall["entity"])
            point(wall["ground_projection"])
            logical = wall["pose"]
            foreground.matrix(logical)
            position = [(n + .5) * 32 - 1600 for n in grid]
            require(close(logical["translation"][:2], position) and logical["rotation"] == [0., 0., 0., 1.]
                    and logical["scale"] == [1., 1., 1.], "M5 wall logical pose differs")
            expected_pose = {"translation": [position[0], 16., -position[1]],
                             "rotation": [0., 0., 0., 1.], "scale": [1., 1., 1.]}
            mesh_ids.extend(meshes(wall["meshes"], wall["entity"], expected_pose, actor=False))
        souls = record.get("souls")
        require(isinstance(souls, list) and len(souls) == 2, "M5 context Soul count differs")
        for soul, side in zip(souls, (-1, 1), strict=True):
            require(type(soul.get("entity")) is int and soul["entity"] > 0 and type(soul.get("side")) in (int, float)
                    and soul["side"] == side and close(soul["pose"]["translation"][:2], [center[0], center[1] + side * 12]),
                    "M5 front/back Soul pose differs")
            foreground.matrix(soul["pose"])
            identities.append(soul["entity"])
            point(soul["ground_projection"])
            scale = soul["meshes"][0]["root_pose"]["scale"]
            vector(scale, 3)
            require(scale[0] in (-1., 1.) and scale[1:] == [1., 1.], "M5 Soul billboard scale differs")
            expected_pose = {"translation": [center[0], 17.6, -center[1] - side * 12],
                             "rotation": camera["pose"]["rotation"], "scale": scale}
            mesh_ids.extend(meshes(soul["meshes"], soul["entity"], expected_pose, actor=True))
        preview = record.get("preview", {})
        require(type(preview.get("entity")) is int and preview["entity"] > 0
                and preview.get("image_handle") == preview_image and preview.get("visible") is True
                and canonical(preview.get("canvas_wu")) == canonical(descriptor["canvas_wu"])
                and close(preview.get("anchor"), record["foreground"]["anchor"])
                and close(preview["pose"]["translation"][:2], [center[0], center[1] - 160])
                and type(preview["pose"]["translation"][2]) in (int, float) and abs(preview["pose"]["translation"][2] - .2) <= 1e-6
                and preview["pose"]["rotation"] == [0., 0., 0., 1.] and preview["pose"]["scale"] == [1., 1., 1.],
                "M5 context descriptor preview differs")
        identities.append(preview["entity"])
        point(preview["ground_projection"])
    require(close(fixture.get("camera_center"), [sum(center[0] for center in centers) / 4,
                                                   sum(center[1] for center in centers) / 4 - 80]),
            "M5 context camera layout differs")
    require(close(main["pose"]["translation"][:2], fixture["camera_center"])
            and main["pose"]["scale"] == [camera_scale]*3, "M5 actual main camera pose differs")
    require(len(set(identities)) == len(identities) and len(set(mesh_ids)) == len(mesh_ids)
            and not set(identities).intersection(mesh_ids), "M5 context identity reused")
    power_context(fixture, value, identities + mesh_ids)
    if lamp_ui:
        validate_lamp_ui(fixture, value, identities + mesh_ids)
    else:
        require("lamp_supply_ui" not in fixture, "Ordinary M5 context cannot acquire UI evidence")


def validate_lamp_ui(fixture, value, used):
    ui = fixture.get("lamp_supply_ui", {})
    ordinal = value["lamp_supply_target"]
    owner = fixture["records"][ordinal]["owner"]
    require(type(ui.get("schema")) is int and ui["schema"] == 1
            and ui.get("scope") == "seeded-no-input-rendering-only"
            and type(ui.get("ordinal")) is int and ui["ordinal"] == ordinal
            and all(type(ui.get(key)) is int and ui[key] == owner for key in ("owner", "selected", "inspected", "model_owner"))
            and ui.get("pinned") is None and ui.get("kind") == "OutdoorLamp"
            and type(ui.get("grid")) is int and ui["grid"] == fixture["power"]["records"][ordinal]["grid"]
            and ui.get("powered") is (ordinal == 2) and ui.get("ordinary_input_evidence") is False
            and ui.get("natural_supply_evidence") is False and layers(ui.get("viewport_physical"), [1280, 720])
            and type(ui.get("window")) is int and ui["window"] > 0
            and type(ui.get("scale_factor")) in (int, float) and ui["scale_factor"] == 1
            and type(ui.get("demand")) in (int, float) and abs(ui["demand"] - .2) <= 1e-6,
            "Lamp UI owner/authority/viewport differs")
    states = {"Supplied": "Supply: Supplied",
              "Shed { reason: InsufficientGeneration }": "Supply: Shed — insufficient generation",
              "Shed { reason: RestoreMargin }": "Supply: Shed — waiting for restore margin",
              "Shed { reason: LegacyGlobalDeficit }": "Supply: Shed — legacy grid deficit"}
    state = ui.get("supply_state")
    require(state in states and (state == "Supplied") is (ordinal == 2), "Lamp UI real supply differs")
    require(ui["window"] not in used, "Lamp UI window aliases a world object")
    seen = set(used) | {ui["window"]}
    for key, expected in (("header", "Building: OutdoorLamp"), ("power_text", states[state] + " / Demand: 0.2W")):
        widget = ui.get(key, {})
        entity = widget.get("entity")
        require(type(entity) is int and entity > 0 and entity not in seen and widget.get("text") == expected
                and widget.get("visible") is True and widget.get("fully_visible") is True,
                "Lamp UI actual text/visibility differs")
        seen.add(entity)
        camera = widget.get("camera", {})
        require(type(camera.get("entity")) is int and camera["entity"] > 0
                and type(camera.get("window")) is int and camera["window"] == ui["window"]
                and camera.get("target") == "owned-primary-window" and camera.get("active") is True
                and camera.get("full_viewport") is True and layers(camera.get("viewport_physical"), [1280, 720]),
                "Lamp UI actual render camera differs")
        color = vector(widget.get("color_srgba"), 4)
        require(all(0 <= channel <= 1 for channel in color) and color[3] > 0, "Lamp UI text is transparent/nonfinite")
        rect = vector(widget.get("rect_physical"), 4)
        require(close(rect, widget.get("rect_logical", [])) and 0 <= rect[0] < rect[2] <= 1280
                and 0 <= rect[1] < rect[3] <= 720 and min(rect[2]-rect[0], rect[3]-rect[1]) > 1,
                "Lamp UI actual rect differs")
