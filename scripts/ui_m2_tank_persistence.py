"""Ordinary adopted Tank Full -> Manual1 save/load; no release authority.

The construction/fill proof is an unchanged contiguous pre-F5 prefix. Every
remaining sample and input is checked across the actual durable-load boundary.
"""
import copy
import hashlib
import re
from pathlib import Path

from scripts import ui_m2_tank_completion as tank
from scripts import ui_m2_tank_inflow as inflow
from scripts import ui_m2_tank_supply as supply
from ui_progress_bars import catalog_control

native, pipeline, planning = tank.native, tank.pipeline, tank.planning
CASE, SCOPE = tank.PERSISTENCE_CASE, tank.PERSISTENCE_SCOPE
CHECKPOINTS = (*inflow.FULL_CHECKPOINTS, "m2-tank-persistence-saved", "m2-tank-persistence-loaded")
POLICY = "ordinary-full-returned-buckets-manual1-v1"
check_view, session_setting = tank.check_view, tank.session_setting
INPUT_PRODUCERS = {"KeyboardInput", "MouseButtonInput"}
PATTERNS = ("F5", r"menu:SelectSaveCatalogSlot \{ slot: Manual1, session: ([1-9][0-9]*) \}",
            "F9", r"menu:SelectLoadCatalogSlot \{ slot: Manual1, session: ([1-9][0-9]*) \}",
            r"menu:ConfirmLoadCatalogSlot \{ slot: Manual1, session: ([1-9][0-9]*) \}")


def source_contract(repo, source, codec):
    return dict(tank.source_contract(repo, source, codec, profile=(CASE, SCOPE)),
                persistence_policy=POLICY,
                persistence_verifier_sha256=pipeline.digest(Path(__file__).read_bytes()),
                inflow_verifier_sha256=pipeline.digest(Path(inflow.__file__).read_bytes()))


def planned_contract(args, repo):
    # Reuse admission checks, never its smaller contract's digest comparison.
    original = copy.copy(args)
    original.art_contract_sha256 = None
    inflow.planned_contract(original, repo, profile=(CASE, SCOPE))
    contract = source_contract(repo, Path(args.m2_art_source), Path(args.art_codec))
    if getattr(args, "art_contract_sha256", None):
        native.require(planning.art_contract_digest(contract) == args.art_contract_sha256,
                       "Tank persistence source changed")
    return contract


def prepare_view(repo, directory, contract):
    source, codec = Path(contract["source_root"]), Path(contract["codec"])
    native.require(pipeline.exact_json(source_contract(repo, source, codec), contract),
                   "Tank persistence contract changed")
    return tank.prepare_view(repo, directory, tank.source_contract(repo, source, codec, profile=(CASE, SCOPE)))


def returned(ctx):
    tanks = ctx.get("tanks", [])
    if len(tanks) != 1 or len(tanks[0].get("stored_items") or []) != 50:
        return False
    owner = tanks[0]["entity"]
    children = [s["entity"] for s in ctx.get("bucket_storages", []) if s.get("belongs_to") == owner]
    if not supply.full_setup_ready({"m2_context": ctx}, owner, children):
        return False
    buckets = {i["entity"] for i in ctx.get("items", []) if i.get("belongs_to") == owner}
    return not any(a.get("bucket") in buckets for a in ctx.get("bucket_actors", []))


def durable(ctx):
    """Only actual persisted M2 entities, with complete forward/inverse relations."""
    native.require(isinstance(ctx, dict) and returned(ctx), "Tank persistence needs Full and five returned unheld buckets")
    categories = {"tanks": ctx["tanks"], "bucket_storages": ctx["bucket_storages"],
                  "items": [r for r in ctx["items"] if r.get("resource") in ("Water", "BucketEmpty", "BucketWater")]}
    native.require(len(categories["tanks"]) == 1 and len(categories["bucket_storages"]) == 2
                   and len(categories["items"]) == 55, "Tank persistence selected inventory differs")
    all_rows = [r for rows in categories.values() for r in rows]
    native.require(len({r.get("entity") for r in all_rows}) == 58
                   and all(isinstance(r.get("entity"), str) and r["entity"] and type(r.get("entity_bits")) is int
                           and r["entity_bits"] >= 0 and isinstance(r.get("pose"), dict)
                           and all(isinstance(r["pose"].get(k), list) and len(r["pose"][k]) == n
                                   and all(planning.finite(x) for x in r["pose"][k])
                                   for k,n in (("translation",3),("rotation",4),("scale",3))) for r in all_rows),
                   "Tank persistence identity/pose inventory differs")
    owner = categories["tanks"][0]["entity"]
    children = {s["entity"] for s in categories["bucket_storages"]}
    for item in categories["items"]:
        native.require(all(key in item and item[key] is None for key in ("loaded_in","delivering_to","stored_by_mixer"))
                       and (item.get("stored_in") == owner and item.get("belongs_to") is None
                            if item["resource"] == "Water" else item.get("stored_in") in children and item.get("belongs_to") == owner),
                       "Tank persistence item relation differs")
    return {key: sorted(copy.deepcopy(rows), key=lambda r:r["entity"]) for key,rows in categories.items()}


def mapped_durable(before, after, rows):
    old, new = durable(before), durable(after)
    a = {r["entity"]:r for group in old.values() for r in group}
    b = {r["entity"]:r for group in new.values() for r in group}
    native.require(isinstance(rows, list) and len(rows) == 58 and not set(a).intersection(b),
                   "Tank persistence map missing or stale")
    kinds = {r["entity"]: ("Tank" if key == "tanks" else "BucketStorage" if key == "bucket_storages" else r["resource"])
             for key,group in old.items() for r in group}
    lookup = {}
    for row in rows:
        saved, loaded = row.get("saved_entity"), row.get("loaded_entity")
        native.require(set(row) == {"saved_entity","loaded_entity","saved_bits","loaded_bits","kind"}
                       and saved in a and loaded in b and saved not in lookup and loaded not in lookup.values()
                       and type(row.get("saved_bits")) is int and type(row.get("loaded_bits")) is int
                       and row["saved_bits"] == a[saved]["entity_bits"] and row["loaded_bits"] == b[loaded]["entity_bits"]
                       and row.get("kind") == kinds[saved], "Tank persistence map duplicated/foreign/typed identity differs")
        lookup[saved] = loaded
    for key,group in old.items():
        for row in group:
            translated = copy.deepcopy(row)
            translated.pop("entity_bits")
            translated["entity"] = lookup[row["entity"]]
            for field in ("belongs_to", "stored_in", "pending_blueprint", "loaded_in", "delivering_to", "stored_by_mixer"):
                if translated.get(field) is not None:
                    native.require(translated[field] in lookup, "Tank persistence external durable relation")
                    translated[field] = lookup[translated[field]]
            if translated.get("stored_items") is not None:
                native.require(all(item in lookup for item in translated["stored_items"]), "Tank persistence foreign inverse")
                translated["stored_items"] = sorted(lookup[item] for item in translated["stored_items"])
            observed = copy.deepcopy(b[lookup[row["entity"]]])
            observed.pop("entity_bits")
            if observed.get("stored_items") is not None:
                observed["stored_items"] = sorted(observed["stored_items"])
            native.require(observed.get("entity") in {r["entity"] for r in new[key]},
                           "Tank persistence mapped category differs")
            native.require(pipeline.exact_json(translated, observed), "Tank persistence durable values differ")
    return lookup


def check_observation(case, value):
    native.require(case in CHECKPOINTS[-2:] and value.get("catalog") == "Closed"
                   and all(type(value.get(k)) is int for k in ("pid","frame","world_epoch","primary_window")),
                   "Tank persistence capture catalog differs")
    inflow.check_observation(inflow.FULL_CHECKPOINTS[-1], value)


def action(driver, key):
    keyboard = key in ("F5", "F9")
    if not keyboard:
        driver.send(point=driver.point(key))
        native.require(driver.last["controls"].get(key, {}).get("interaction") == "Hovered", "Tank catalog not hovered; no press")
    before, first = copy.deepcopy(driver.last), len(driver.events)
    if keyboard:
        driver.key(key)
    else:
        driver.send(button=1, pressed=True)
        native.require(driver.last["controls"].get(key, {}).get("interaction") == "Pressed", "Tank catalog press missing")
        driver.send(button=1, pressed=False)
    def flushed(value):
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None and raw.get("nonce") == before["nonce"]
                       and raw.get("process_id") == before["pid"], "Tank persistence input flush owner differs")
        found = [e for e in raw["events"] if e.get("producer") in INPUT_PRODUCERS
                 and tank.context(before)["real_seconds"] < e.get("real_seconds", -1) <= tank.context(value)["real_seconds"]]
        return len(found) == 2 and [e.get("state") for e in found] == ["Pressed", "Released"]
    driver.last = driver.wait(flushed, after=driver.last["frame"], timeout=10)
    driver.events[first]["tank_persistence_action"] = dict(key=key, first_step=first+1, last_step=len(driver.events),
                                                         before=before, after=copy.deepcopy(driver.last))
    native.atomic_write_json(driver.root / "events.json", {"events": driver.events})


def exercise(driver, contract):
    inflow.exercise(driver, contract)
    action(driver, "F5")
    driver.last = driver.wait(lambda v:any(k.startswith("menu:SelectSaveCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    action(driver, catalog_control(driver, "SelectSaveCatalogSlot"))
    driver.last = driver.wait(lambda v:v["catalog"] == "Closed" and "Save:Succeeded" in v["save_outcomes"], after=driver.last["frame"])
    driver.capture(CHECKPOINTS[-2])
    action(driver, "F9")
    driver.last = driver.wait(lambda v:any(k.startswith("menu:SelectLoadCatalogSlot") for k in v["controls"]), after=driver.last["frame"])
    action(driver, catalog_control(driver, "SelectLoadCatalogSlot"))
    epoch = driver.last["world_epoch"]
    action(driver, catalog_control(driver, "ConfirmLoadCatalogSlot"))
    def ready(value):
        if value["world_epoch"] != epoch+1 or value["catalog"] != "Closed":
            return False
        raw = native.read_json(driver.root / "production-cancellation.json")
        native.require(raw.get("failure") is None, "Tank persistence collector failed")
        try:
            check_observation(CHECKPOINTS[-1], value)
            tank.check_completed_consumers(raw, value, contract, water_count=50)
        except (ValueError, RuntimeError):
            return False
        return True
    driver.last = driver.wait(ready, after=driver.last["frame"], timeout=60)
    driver.capture(CHECKPOINTS[-1])
    def flushed(_):
        raw = native.read_json(driver.root / "production-cancellation.json")
        return raw.get("samples") and raw["samples"][-1]["real_seconds"] >= tank.context(driver.checkpoints[-1]["value"])["real_seconds"]
    driver.wait(flushed, after=driver.last["frame"], timeout=15)


def check_inputs(raw, entries, client, inputs):
    lower = entries[3]
    tail = inputs[lower["last_step"]:]
    actions = [r["tank_persistence_action"] for r in tail if "tank_persistence_action" in r]
    native.require(len(actions) == 5 and all(re.fullmatch(pattern, a.get("key", ""))
                   for pattern,a in zip(PATTERNS, actions, strict=True)), "Tank persistence catalog sequence differs")
    selected, confirmed = (re.fullmatch(PATTERNS[n], actions[n]["key"])[1] for n in (3,4))
    native.require(selected == confirmed, "Tank persistence load session differs")
    claimed, observed, presses = set(), set(), []
    epoch, window = lower["value"]["world_epoch"], lower["value"]["primary_window"]
    for n,a in enumerate(actions):
        first,last = a.get("first_step"),a.get("last_step")
        before,after,key = a["before"],a["after"],a["key"]
        native.require(type(first) is int and type(last) is int and last == first+1
                       and lower["last_step"] < first < last <= len(inputs) and not claimed.intersection((first,last))
                       and inputs[first-1].get("tank_persistence_action") == a, "Tank persistence action ordinals differ")
        claimed.update((first,last))
        start,end = (tank.context(v)["real_seconds"] for v in (before,after))
        native.require(planning.finite(start) and planning.finite(end) and start < end
                       and all(type(v.get(k)) is int for v in (before,after) for k in ("pid","frame","world_epoch","primary_window"))
                       and before["frame"] < after["frame"] and before["world_epoch"] == epoch
                       and after["world_epoch"] in ((epoch,epoch+1) if n == 4 else (epoch,))
                       and all(v.get("nonce") == client["nonce"] and v.get("pid") == client["pid"]
                               and v.get("primary_window") == window and v.get("paused") is True for v in (before,after)),
                       "Tank persistence action owner/clock/epoch differs")
        sent = inputs[first-1:last]
        keyboard = n in (0,2)
        native.require([r.get("pressed") for r in sent] == [True,False]
                       and all(type(r.get("pressed")) is bool and before["frame"] <= r["pre_frame"] < r["post_frame"] <= after["frame"]
                               and r["world_epoch"] in (before["world_epoch"],after["world_epoch"])
                               and (r.get("key") == key and r.get("button") is None if keyboard
                                    else type(r.get("button")) is int and r["button"] == 1 and r.get("key") is None) for r in sent),
                       "Tank persistence sent pair differs")
        rows = [r for r in raw["events"] if r.get("producer") in INPUT_PRODUCERS and start < r["real_seconds"] <= end]
        native.require(len(rows) == 2 and [r.get("state") for r in rows] == ["Pressed","Released"], "Tank persistence raw pair differs")
        for r in rows:
            sample = raw["samples"][r["sample_index"]]
            native.require(r["event_index"] not in observed and r.get("window") == sample.get("primary_window")
                           and r["real_seconds"] <= sample["real_seconds"]
                           and (r.get("producer") == "KeyboardInput" and r.get("key") == key and r.get("repeat") is False if keyboard
                                else r.get("producer") == "MouseButtonInput" and r.get("button") == "Left"),
                           "Tank persistence raw pair owner/key differs")
            observed.add(r["event_index"])
            if not keyboard:
                cursor,dpi = sample.get("cursor_position"),sample.get("dpi")
                native.require(isinstance(cursor,list) and len(cursor) == 2 and planning.finite(dpi) and dpi > 0
                               and dpi == before.get("scale_factor") and all(planning.finite(x) and abs(x*dpi-y) <= 2
                                   for x,y in zip(cursor,before["cursor"],strict=True)), "Tank persistence raw cursor differs")
        if not keyboard:
            target = before["controls"].get(key,{})
            native.require(target.get("fully_visible") is True and target.get("interaction") == "Hovered"
                           and sent[0].get("observed_hover",{}).get(key) == "Pressed", "Tank persistence catalog target differs")
        presses.append(rows[0])
    native.require(all(int(r["step"]) in claimed for r in tail if r.get("key") is not None or r.get("button") is not None)
                   and observed == {r["event_index"] for r in raw["events"] if r.get("producer") in INPUT_PRODUCERS
                                    and r["real_seconds"] > tank.context(lower["value"])["real_seconds"]}, "Tank persistence extra input")
    # Validate complete transport without changing its epoch values or ordinals.
    native.require(len(inputs) <= 160 and [r.get("step") for r in inputs] == [str(i+1) for i in range(len(inputs))]
                   and all(r.get("nonce") == client["nonce"] and type(r.get("pid")) is int and r["pid"] == client["pid"]
                           and type(r.get("window")) is int and r["window"] == client["window"]
                           and type(r.get("world_epoch")) is int and r["world_epoch"] in (epoch,epoch+1)
                           and type(r.get("pre_frame")) is int and type(r.get("post_frame")) is int
                           and 0 <= r["pre_frame"] < r["post_frame"]
                           and sum(r.get(k) is not None for k in ("key","button","point")) == 1 for r in inputs)
                   and all(a["pre_frame"] <= b["pre_frame"] and a["post_frame"] <= b["post_frame"] for a,b in zip(inputs,inputs[1:])),
                   "Tank persistence transport owner differs")
    motion_actions = set()
    for r in tail:
        if r.get("point") is not None:
            point,cursor = r["point"],r.get("observed_cursor")
            next_actions = [a for a in actions if int(r["step"]) < a["first_step"]]
            native.require(next_actions, "Tank persistence motion after final input")
            following = next_actions[0]
            ordinal = following["first_step"]
            native.require(isinstance(point,(tuple,list)) and len(point) == 2 and isinstance(cursor,list) and len(cursor) == 2
                           and r.get("pressed") is None and r["world_epoch"] == epoch
                           and following["key"] not in ("F5","F9") and ordinal not in motion_actions
                           and r["post_frame"] <= following["before"]["frame"]
                           and all(type(x) is int and 0 < x < limit and planning.finite(y) and abs(x-y) <= 2
                                   for x,y,limit in zip(point,cursor,lower["value"]["viewport"],strict=True))
                           and all(abs(x-y) <= 2 for x,y in zip(point,following["before"]["cursor"],strict=True)),
                           "Tank persistence motion ACK differs")
            motion_actions.add(ordinal)
    native.require(sum(r.get("point") is not None for r in tail) <= 3, "Tank persistence extra motion")
    native.require(all(a["after"]["frame"] <= b["before"]["frame"]
                       and tank.context(a["after"])["real_seconds"] <= tank.context(b["before"])["real_seconds"] for a,b in zip(actions,actions[1:])),
                   "Tank persistence action order differs")
    for lower,upper,group in ((entries[3],entries[4],actions[:2]),(entries[4],entries[5],actions[2:])):
        native.require(all(lower["last_step"] < a["first_step"] < a["last_step"] <= upper["last_step"]
                           and lower["value"]["frame"] <= a["before"]["frame"] < a["after"]["frame"] < upper["value"]["frame"]
                           and tank.context(lower["value"])["real_seconds"] <= tank.context(a["before"])["real_seconds"]
                               < tank.context(a["after"])["real_seconds"] < tank.context(upper["value"])["real_seconds"] for a in group),
                       "Tank persistence action crossed capture interval")
    return actions,presses


def check_tail_events(raw, cutoff):
    """Legacy button edges are paired observations, not extra domain authority."""
    events,samples = raw["events"],raw["samples"]
    tail = [e for e in events if e["sample_index"] >= cutoff]
    allowed = INPUT_PRODUCERS | {"ordinary-save-commit-v1","ordinary-load-commit-v1","SaveLoadOutcome"}
    paired = set()
    operations = {"KeyPress":("KeyboardInput","Pressed"),"KeyRelease":("KeyboardInput","Released"),
                  "PointerPress":("MouseButtonInput","Pressed"),"PointerRelease":("MouseButtonInput","Released")}
    for e in tail:
        producer = e.get("producer")
        if producer in allowed:
            continue
        if producer == "production_snapshot_delta":
            index,category,entity = e["sample_index"],e.get("category"),e.get("entity")
            native.require(set(e) == {"producer","operation","sample_index","event_index","before_sample_index","real_seconds","category","entity","before","after"}
                           and e.get("operation") == "StateTransition" and type(e.get("before_sample_index")) is int
                           and index > 0 and e["before_sample_index"] == index-1
                           and category in {"owners","blueprints","roots","items","move_tasks","generations","owner_layers"}
                           and isinstance(entity,str) and entity and e["real_seconds"] == samples[index]["real_seconds"],
                           "Tank persistence state delta schema differs")
            key = "kind" if category == "generations" else "entity"
            for field,ordinal in (("before",index-1),("after",index)):
                observed = [r for r in samples[ordinal][category] if r.get(key) == entity]
                native.require(len(observed) <= 1 and pipeline.exact_json(e[field],observed[0] if observed else None),
                               "Tank persistence state delta payload differs")
            native.require(not pipeline.exact_json(e["before"],e["after"]), "Tank persistence empty state delta")
            continue
        op = e.get("operation")
        native.require(producer is None and op in operations, "Tank persistence unexpected tail producer")
        keyboard = op.startswith("Key")
        native.require(set(e) == {"sample_index","event_index","real_seconds","operation"} | ({"key"} if keyboard else set()),
                       "Tank persistence legacy edge schema differs")
        wanted,state = operations[op]
        matches = [r for r in tail if r.get("producer") == wanted and r.get("state") == state
                   and r["sample_index"] == e["sample_index"] and r["real_seconds"] == e["real_seconds"]
                   and (r.get("key") == e.get("key") if keyboard else r.get("button") == "Left")]
        native.require(len(matches) == 1 and matches[0]["event_index"] not in paired, "Tank persistence legacy edge has no unique raw pair")
        paired.add(matches[0]["event_index"])
    native.require(paired == {e["event_index"] for e in tail if e.get("producer") in INPUT_PRODUCERS},
                   "Tank persistence raw input lacks its legacy edge")


def verify(raw, entries, client, inputs, contract, *, save_path):
    native.require(contract.get("case") == CASE and contract.get("scope") == raw.get("scope") == SCOPE
                   and contract.get("persistence_policy") == POLICY
                   and [e["case"] for e in entries] == list(CHECKPOINTS), "Tank persistence profile differs")
    samples,events = raw["samples"],raw["events"]
    native.require(0 < len(samples) <= 1200 and len(events) < 4096
                   and all(type(s.get("sample_index")) is int and s["sample_index"] == n
                           and planning.finite(s.get("real_seconds")) and s["real_seconds"] >= 0
                           and planning.finite(s.get("virtual_seconds")) and s["virtual_seconds"] >= 0 for n,s in enumerate(samples))
                   and all(a["real_seconds"] < b["real_seconds"] for a,b in zip(samples,samples[1:]))
                   and all(type(e.get("event_index")) is int and e["event_index"] == n
                           and type(e.get("sample_index")) is int and 0 <= e["sample_index"] < len(samples)
                           and planning.finite(e.get("real_seconds")) and 0 <= e["real_seconds"] <= samples[e["sample_index"]]["real_seconds"]
                           for n,e in enumerate(events)), "Tank persistence raw envelope differs")
    actions,presses = check_inputs(raw,entries,client,inputs)
    cutoff = presses[0]["sample_index"]
    native.require(cutoff > 0, "Tank persistence pre-save prefix missing")
    prefix = dict(raw, samples=samples[:cutoff], events=[e for e in events if e["sample_index"] < cutoff])
    inflow.verify(prefix,entries[:4],client,inputs[:entries[3]["last_step"]],contract,profile=(CASE,SCOPE))
    receipts = [e for e in events if e.get("operation") in ("DurableSave","DurableLoad")]
    native.require([e.get("operation") for e in receipts] == ["DurableSave","DurableLoad"], "Tank persistence durable pair missing")
    native.require(save_path.name == "manual-1.scn.ron" and save_path.parent.name == "saves"
                   and not save_path.is_symlink() and not save_path.parent.is_symlink() and save_path.is_file()
                   and 0 < save_path.stat().st_size <= 64*1024*1024, "Tank persistence save path/size differs")
    with save_path.open("rb") as stream:
        data = stream.read(64*1024*1024+1)
    native.require(0 < len(data) <= 64*1024*1024, "Tank persistence save capped")
    before,after = (e.get("m2_context") for e in receipts)
    mapped_durable(before,after,receipts[1].get("m2_entity_remap"))
    committed = [e for e in prefix["events"] if isinstance(e.get("outcome"),dict) and "companion_owners" in e["outcome"]]
    native.require(len(committed) == 1, "Tank persistence actual companion placement missing")
    children = committed[0]["outcome"]["companion_owners"]
    original_owner = tank.context(entries[3]["value"])["consumers"]["roots"][0]["owner"]
    native.require(before["tanks"][0]["entity"] == original_owner
                   and {s["entity"] for s in before["bucket_storages"]} == set(children),
                   "Tank persistence original committed companions replaced")
    epoch = entries[3]["value"]["world_epoch"]
    boundary = receipts[1]["sample_index"]
    native.require(type(receipts[1].get("previous_epoch")) is int and receipts[1]["previous_epoch"] == epoch
                   and cutoff <= receipts[0]["sample_index"] < boundary < len(samples), "Tank persistence load boundary differs")
    for n,(row,op,action_index) in enumerate(zip(receipts,("save","load"),(1,4),strict=True)):
        value = entries[4+n]["value"]
        check_observation(CHECKPOINTS[4+n],value)
        a,p = actions[action_index],presses[action_index]
        session = re.fullmatch(PATTERNS[action_index],a["key"])[1]
        request = f"Manual(ManualCatalog {{ dialog_session: {session} }})" if n == 0 else f"Load(NormalCatalog {{ dialog_session: {session} }})"
        native.require(row.get("producer") == f"ordinary-{op}-commit-v1" and row.get("result") == "Committed"
                       and row.get("target") == "Manual slot 1" and row.get("request_source") == request
                       and row.get("file_sha256") == hashlib.sha256(data).hexdigest()
                       and type(row.get("file_bytes")) is int and row["file_bytes"] == len(data)
                       and type(row.get("world_epoch")) is int and row["world_epoch"] == epoch+n
                       and p["sample_index"] <= row["sample_index"] and p["real_seconds"] <= row["real_seconds"]
                       and tank.context(a["before"])["real_seconds"] < row["real_seconds"] <= tank.context(value)["real_seconds"]
                       and value["world_epoch"] == epoch+n and value.get("nonce") == client["nonce"] and value.get("pid") == client["pid"],
                       "Tank persistence durable commit/input binding differs")
        tank.check_completed_consumers(raw,value,contract,water_count=50)
    native.require(tank.context(entries[4]["value"])["real_seconds"] < receipts[1]["real_seconds"], "Tank load precedes saved capture")
    check_tail_events(raw,cutoff)
    outcomes = [e for e in events if e.get("producer") == "SaveLoadOutcome"]
    native.require(len(outcomes) == 2 and all(e.get("operation") == op and e.get("result") == "Succeeded"
                   and e.get("target") == "Manual slot 1" and e.get("source") == receipt["request_source"]
                   and e["sample_index"] == receipt["sample_index"]
                   for e,op,receipt in zip(outcomes,("Save","Load"),receipts,strict=True)), "Tank persistence outcome pair differs")
    clock = tank.context(entries[3]["value"])["virtual_seconds"]
    window = entries[3]["value"]["primary_window"]
    for sample in samples[cutoff-1:]:
        post = sample["sample_index"] >= boundary
        state = after if post else before
        owner = state["tanks"][0]["entity"]
        owners = [r for r in sample.get("owners",[]) if r.get("kind") == "Tank"]
        layers = [r for r in sample.get("owner_layers",[]) if r.get("entity") == owner]
        native.require(type(sample.get("world_epoch")) is int and sample["world_epoch"] == epoch+int(post)
                       and type(sample.get("primary_window_bits")) is int and sample["primary_window_bits"] == window
                       and sample.get("primary_window") == samples[0].get("primary_window")
                       and sample.get("paused") is True and type(sample.get("relative_speed")) in (int,float)
                       and sample["relative_speed"] == 4 and sample["virtual_seconds"] == clock
                       and not sample.get("blueprints") and len(owners) == len(layers) == 1
                       and owners[0].get("entity") == owner and owners[0].get("provisional") is False
                       and owners[0].get("entity_bits") == state["tanks"][0]["entity_bits"]
                       and layers[0].get("exists") is True
                       and pipeline.exact_json(layers[0].get("building_grids"),tank.context(entries[0]["value"])["footprint"])
                       and all(layers[0].get(k) == [] for k in ("floor_grids","door_grids","bridge_grids","stockpile_grids"))
                       and pipeline.exact_json(durable(sample.get("m2_context")),durable(state)),
                       "Tank persistence transient inventory/epoch/clock changed")
    old_root = tank.context(entries[3]["value"])["consumers"]["roots"][0]["entity"]
    new_root = tank.context(entries[5]["value"])["consumers"]["roots"][0]["entity"]
    native.require(old_root != new_root, "Tank persistence reused pre-load rendering root")
    for entry in entries:
        native.require(type(entry.get("last_step")) is int and entry["last_step"] == sum(r["post_frame"] < entry["value"]["frame"] for r in inputs)
                       and all(pipeline.exact_json(tank.context(entry["value"])[k],tank.context(entries[0]["value"])[k]) for k in ("anchor","footprint","draw_pos")),
                       "Tank persistence checkpoint/input geometry differs")
    native.require(entries[-1]["last_step"] == len(inputs)
                   and samples[-1]["real_seconds"] >= tank.context(entries[-1]["value"])["real_seconds"], "Tank persistence final raw tail missing")
