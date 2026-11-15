"""Ordinary area/floor input prerequisite; never seeds work or materials."""
import copy
from scripts import ui_m2_tank_completion as tank

native, planning, pipeline = tank.native, tank.planning, tank.pipeline
KEYS = ["menu:ToggleOrders", "menu:SelectAreaTask", "supply-area-drag", "Escape",
        "menu:ToggleArchitect", "menu:SetTimeSpeed(Normal)", "menu:SelectFloorPlace",
        "supply-floor-drag", "menu:SetTimeSpeed(Paused)"]
CONTRACT = "ordinary-area-floor45-surplus-bound-v4"
COLUMNS, ROWS = 9, 5
TILES = COLUMNS * ROWS


def required_export(full_buckets):
    native.require(type(full_buckets) is int and 0 <= full_buckets <= 5, "Supply surplus bucket count differs")
    return max(0, full_buckets * 5 * 5 + 5 - 10)


def inventory(ctx):
    native.require(isinstance(ctx,dict) and ctx.get("overflow") is False, "Supply inventory overflow/type differs")
    for key in ("sites","tiles","familiars","completed"):
        rows = ctx.get(key)
        native.require(isinstance(rows,list) and len(rows) <= 128
                       and all(isinstance(r,dict) and isinstance(r.get("entity"),str) and r["entity"] for r in rows)
                       and len({r["entity"] for r in rows}) == len(rows), "Supply inventory rows/identity differ")
    for f in ctx["familiars"]:
        native.require(type(f.get("bits")) is int and f["bits"] >= 0 and isinstance(f.get("command"),str)
                       and (f.get("area") is None or isinstance(f["area"],dict)
                            and set(f["area"]) == {"min","max"}
                            and all(isinstance(f["area"][k],list) and len(f["area"][k]) == 2
                                    and all(planning.finite(v) for v in f["area"][k]) for k in ("min","max"))),
                       "Supply Familiar identity/area differs")
    native.require(len({f["bits"] for f in ctx["familiars"]}) == len(ctx["familiars"]), "Supply Familiar bits reused")
    for row in ctx["sites"]:
        native.require(all(type(row.get(k)) is int and 0 <= row[k] <= TILES for k in ("total","reinforced","poured"))
                       and all(isinstance(row.get(k),list) and len(row[k]) == 2
                               and all(planning.finite(v) for v in row[k]) for k in ("min","max","material_center"))
                       and isinstance(row.get("phase"),str), "Supply site counters/geometry differ")
    for row in ctx["tiles"]:
        native.require(isinstance(row.get("site"),str) and isinstance(row.get("grid"),list) and len(row["grid"]) == 2
                       and all(type(v) is int for v in row["grid"])
                       and type(row.get("bones")) is int and 0 <= row["bones"] <= 2
                       and type(row.get("mud")) is int and 0 <= row["mud"] <= 1
                       and isinstance(row.get("state"),str), "Supply tile counters/geometry differ")


def context(value):
    ctx = tank.context(value).get("supply_workload")
    native.require(isinstance(ctx, dict) and ctx.get("overflow") is False, "Supply floor inventory missing")
    anchor = ctx.get("anchor")
    native.require(isinstance(anchor, list) and len(anchor) == 2 and all(type(x) is int for x in anchor),
                   "Supply floor anchor missing")
    x, y = anchor
    native.require(ctx.get("cells") == [[x+dx, y+dy] for dy in range(ROWS) for dx in range(COLUMNS)],
                   "Supply floor footprint differs")
    native.require(all(isinstance(ctx.get(k), list) and len(ctx[k]) == 2
                       and all(planning.finite(v) for v in ctx[k]) for k in ("min", "max"))
                   and all(a < b for a,b in zip(ctx["min"],ctx["max"],strict=True))
                   and all(isinstance(ctx.get(k),list) and len(ctx[k]) <= 128
                           for k in ("sites","tiles","familiars","completed")),
                   "Supply workload geometry/inventory differs")
    inventory(ctx)
    return ctx


def active(ctx, selected):
    return [f for f in ctx["familiars"] if f["bits"] == selected and f["command"] == "Patrol"
            and f["area"] == {"min": ctx["min"], "max": ctx["max"]}]


def placed(ctx):
    return (len(ctx["sites"]) == 1 and ctx["sites"][0]["total"] == TILES
            and ctx["sites"][0]["min"] == ctx["min"] and ctx["sites"][0]["max"] == ctx["max"]
            and len(ctx["tiles"]) == TILES and {tuple(t["grid"]) for t in ctx["tiles"]} == {tuple(p) for p in ctx["cells"]}
            and len({t["entity"] for t in ctx["tiles"]}) == TILES
            and all(t["site"] == ctx["sites"][0]["entity"] for t in ctx["tiles"]))


def drag(driver, key):
    ctx = context(driver.last)
    points = ctx.get("points")
    native.require(isinstance(points, list) and len(points) == 2 and all(isinstance(p, list) and len(p) == 2
                   and all(planning.finite(x) and 0 < x < b for x,b in zip(p,driver.last["viewport"],strict=True))
                   for p in points), "Supply gesture is outside the owned client")
    dpi = driver.last["scale_factor"]
    driver.send(point=tuple(round(x*dpi) for x in points[0]))
    before, first = copy.deepcopy(driver.last), len(driver.events)
    native.require(before.get("pointer_over_ui") is False and before.get("world_input_captured") is False
                   and before.get("paused") is (key == "supply-area-drag"),
                   "Supply gesture capture/time precondition differs")
    mode = "AreaSelection(None)" if key == "supply-area-drag" else "FloorPlace(None)"
    native.require(context(before)["task_mode"] == mode, "Supply gesture mode differs")
    driver.send(button=1, pressed=True)
    driver.send(point=tuple(round(x*dpi) for x in points[1]))
    native.require(driver.last.get("pointer_over_ui") is False, "Supply gesture end is captured")
    driver.send(button=1, pressed=False)

    def flushed(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "Supply workload raw flush failed")
        rows = [e for e in raw.get("events", []) if e.get("producer") == "MouseButtonInput"
                and tank.context(before)["real_seconds"] < e.get("real_seconds", -1) <= tank.context(value)["real_seconds"]]
        done = active(context(value), before["selected"]) if key == "supply-area-drag" else placed(context(value))
        return len(rows) == 2 and [r["state"] for r in rows] == ["Pressed", "Released"] and bool(done)

    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["tank_completion_action"] = dict(key=key,first_step=first+1,last_step=len(driver.events),
        before=before,after=copy.deepcopy(driver.last))
    native.atomic_write_json(driver.root / "events.json", {"events":driver.events})


def exercise(driver):
    for key in KEYS:
        if key.startswith("supply-"):
            drag(driver,key)
        else:
            tank.capture_action(driver,key)


def check_gesture(action, rows, samples):
    before, after, key = action["before"], action["after"], action["key"]
    ctx = context(before)
    native.require(pipeline.exact_json({k:context(after)[k] for k in ("anchor","cells","min","max")},
                   {k:ctx[k] for k in ("anchor","cells","min","max")})
                   and before.get("paused") is after.get("paused") is (key == "supply-area-drag")
                   and not before.get("pointer_over_ui") and not before.get("world_input_captured"),
                   "Supply gesture subject/capture differs")
    native.require(ctx["task_mode"] == ("AreaSelection(None)" if key == "supply-area-drag" else "FloorPlace(None)"),
                   "Supply gesture began in wrong mode")
    for event, point in zip(rows, ctx["points"], strict=True):
        sample = samples[event["sample_index"]]
        cursor, dpi = sample.get("cursor_position"), sample.get("dpi")
        native.require(isinstance(cursor,list) and planning.finite(dpi) and dpi > 0
                       and all(abs(a-b) <= 2/dpi for a,b in zip(cursor,point,strict=True)),
                       "Supply gesture raw endpoint differs")
    if key == "supply-area-drag":
        native.require(active(context(after),before["selected"]), "Supply area did not commit to selected Familiar")
    else:
        native.require(placed(context(after)), "Supply floor did not commit exact forty-five tiles")


def verify(raw, actions):
    native.require([a["key"] for a in actions[:len(KEYS)]] == KEYS, "Supply workload inputs differ")
    area, floor = actions[2], actions[7]
    baseline = context(area["before"])
    selected = area["before"]["selected"]
    native.require(type(selected) is int and len([f for f in baseline["familiars"] if f["bits"] == selected
                   and f["area"] is None and f["command"] == "Idle"]) == 1, "Supply Familiar was preconfigured")
    cutoff = tank.context(floor["after"])["real_seconds"]
    def release(action):
        start,end = (tank.context(action[k])["real_seconds"] for k in ("before","after"))
        rows = [e for e in raw["events"] if e.get("producer") == "MouseButtonInput"
                and e.get("state") == "Released" and start < e.get("real_seconds",-1) <= end]
        native.require(len(rows) == 1 and type(rows[0].get("sample_index")) is int,
                       "Supply workload actual release missing or ambiguous")
        return rows[0]["sample_index"]
    area_release, floor_release = release(area), release(floor)
    site = context(floor["after"])["sites"][0]["entity"]
    tile_ids = {tuple(t["grid"]):t["entity"] for t in context(floor["after"])["tiles"]}
    completed_ids = None
    for index,sample in enumerate(raw["samples"]):
        ctx = sample.get("m2_context",{}).get("supply_workload")
        native.require(isinstance(ctx,dict) and ctx.get("overflow") is False
                       and all(pipeline.exact_json(ctx.get(k),baseline[k]) for k in ("anchor","cells","min","max")),
                       "Supply workload raw target changed")
        inventory(ctx)
        time = sample["real_seconds"]
        if index < floor_release:
            native.require(not ctx["sites"] and not ctx["tiles"] and not ctx["completed"],
                           "Supply floor existed before ordinary placement")
        if index < area_release:
            native.require(not ctx["sites"] and not ctx["tiles"] and not ctx["completed"]
                           and all(f["area"] is None and f["command"] == "Idle" for f in ctx["familiars"]),
                           "Supply workload or area existed before ordinary input")
        if time >= tank.context(area["after"])["real_seconds"]:
            native.require(active(ctx,selected), "Supply committed Familiar area/command changed")
        if time >= cutoff:
            if placed(ctx):
                native.require(completed_ids is None and ctx["sites"][0]["entity"] == site and not ctx["completed"]
                               and {tuple(t["grid"]):t["entity"] for t in ctx["tiles"]} == tile_ids,
                               "Supply floor site/tiles replaced or reverted")
            else:
                native.require(not ctx["sites"] and not ctx["tiles"] and len(ctx["completed"]) == TILES
                               and len({c["entity"] for c in ctx["completed"]}) == TILES
                               and all(c.get("valid") is True for c in ctx["completed"])
                               and {tuple(c["grid"]) for c in ctx["completed"]} == set(tile_ids),
                               "Supply floor site/tiles replaced or partially missing")
                current = {tuple(c["grid"]):c["entity"] for c in ctx["completed"]}
                native.require(completed_ids is None or current == completed_ids, "Supply completed floor replaced")
                completed_ids = current
