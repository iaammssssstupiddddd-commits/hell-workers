"""Branch-local water conversions; no launch, art or release authority."""

import math
from scripts import building_production_acceptance as evidence

require, exact = evidence.require, evidence.pipeline.exact_json
COMMON = {
    "producer",
    "operation",
    "sample_index",
    "event_index",
    "real_seconds",
    "worker",
    "assignment",
    "target",
    "owner",
    "mixer",
    "bucket",
    "carried_before",
    "world_epoch",
    "work_type",
}


def one(rows, entity):
    found = [row for row in rows if row.get("entity") == entity]
    require(len(found) == 1, "Water transfer entity missing or duplicated")
    return found[0]


def active(ctx, event, phases, amount=None):
    actor = one(ctx.get("bucket_actors", []), event["worker"])
    require(
        actor.get("is_soul") is True
        and actor.get("inventory_present") is True
        and actor.get("bucket") == actor.get("carried_item") == event["bucket"]
        and actor.get("working_on") == event["target"]
        and actor.get("phase") in phases
        and exact(
            actor.get("identity"),
            dict(
                assignment=event["assignment"],
                target=event["target"],
                work_type="HaulWaterToMixer",
                matches_working_on=True,
            ),
        )
        and exact(
            actor.get("source"),
            dict(kind="Tank", entity=event["owner"], exists=True, needs_fill=True),
        )
        and exact(
            actor.get("destination"),
            dict(kind="Mixer", entity=event["mixer"], exists=True),
        )
        and (
            amount is None
            or type(actor.get("amount")) is int
            and actor["amount"] == amount
        ),
        "Water transfer live assignment differs",
    )


def check_inventory(ctx, owner):
    """Both relationship directions from the SAME post-commit snapshot."""
    require("stored_items" in owner, "Water owner inverse missing")
    inverse = owner["stored_items"]
    require(
        inverse is None
        or isinstance(inverse, list)
        and all(isinstance(entity, str) and entity for entity in inverse)
        and len(set(inverse)) == len(inverse),
        "Water owner inverse malformed",
    )
    forward = [
        item
        for item in ctx.get("items", [])
        if item.get("stored_in") == owner["entity"]
    ]
    require(
        all(
            isinstance(item.get("entity"), str)
            and item["entity"]
            and item.get("resource") == "Water"
            and "loaded_in" in item
            and item["loaded_in"] is None
            for item in forward
        ),
        "Water owner forward relation malformed",
    )
    ids = [item["entity"] for item in forward]
    require(
        len(set(ids)) == len(ids) and set(ids) == set(inverse or []),
        "Water owner forward/inverse inventory differs",
    )


def check_receipt(event, samples, events):
    take = event.get("operation") == "TankTake"
    extra = (
        {"bucket_before", "inputs", "inputs_before", "route_found"}
        if take
        else {
            "bucket_before",
            "amount",
            "before_count",
            "capacity",
            "outputs",
            "returning",
        }
    )
    require(
        set(event) == COMMON | extra
        and event["operation"] == ("TankTake" if take else "MixerPour")
        and event["producer"]
        == ("ordinary-tank-take-branch-v1" if take else "ordinary-mixer-pour-branch-v1")
        and type(event["sample_index"]) is int
        and 0 < event["sample_index"] < len(samples)
        and type(event["event_index"]) is int
        and event["event_index"] >= 0,
        "Water transfer envelope differs",
    )
    index = event["sample_index"]
    before, after = samples[index - 1], samples[index]
    require(
        all(
            isinstance(event[k], str) and event[k]
            for k in ("worker", "assignment", "target", "owner", "mixer", "bucket")
        )
        and event["assignment"] == event["target"]
        and event["work_type"] == "HaulWaterToMixer"
        and event["carried_before"] == event["bucket"]
        and len({event[k] for k in ("worker", "target", "owner", "mixer", "bucket")})
        == 5
        and type(event["world_epoch"]) is int
        and event["world_epoch"] >= 0
        and all(
            type(s.get("world_epoch")) is int
            and s["world_epoch"] == event["world_epoch"]
            for s in (before, after)
        )
        and all(
            type(s.get("real_seconds")) in (int, float)
            and math.isfinite(s["real_seconds"])
            for s in (before, after)
        )
        and type(event["real_seconds"]) in (int, float)
        and math.isfinite(event["real_seconds"])
        and before["real_seconds"] < event["real_seconds"] <= after["real_seconds"],
        "Water transfer identity/clock differs",
    )
    old, ctx = before.get("m2_context", {}), after.get("m2_context", {})
    require(
        type(ctx.get("water_inventory_schema")) is int
        and ctx["water_inventory_schema"] == 1
        and ctx.get("mixer_water_channel") is True
        and all(
            ctx.get(k) is False
            for k in ("accepted", "performance_evidence", "promotion_authority")
        ),
        "Water transfer observer differs",
    )
    tank, mixer = (
        one(ctx.get("tanks", []), event["owner"]),
        one(ctx.get("mixers", []), event["mixer"]),
    )
    require(
        tank.get("provisional") is False
        and exact(tank.get("stockpile"), dict(resource="Water", capacity=50))
        and exact(mixer.get("building"), dict(kind="MudMixer", provisional=False))
        and isinstance(mixer.get("stockpile"), dict)
        and mixer["stockpile"].get("resource") == "Water"
        and type(mixer["stockpile"].get("capacity")) is int
        and mixer["stockpile"]["capacity"] > 0,
        "Water transfer buildings differ",
    )
    check_inventory(ctx, tank)
    check_inventory(ctx, mixer)
    bucket = one(ctx.get("items", []), event["bucket"])
    for row in (bucket,):
        require(
            row.get("belongs_to") == event["owner"]
            and {"loaded_in", "stored_in"} <= row.keys()
            and row["loaded_in"] is None
            and row["stored_in"] is None,
            "Water transfer bucket relation differs",
        )
    require(
        event["bucket_before"] == ("BucketEmpty" if take else "BucketWater"),
        "Water transfer prior bucket differs",
    )
    ids = event["inputs"] if take else event["outputs"]
    require(
        isinstance(ids, list)
        and 1 <= len(ids) <= 5
        and all(isinstance(i, str) and i for i in ids)
        and len(set(ids)) == len(ids)
        and not set(ids).intersection(
            event[k] for k in ("worker", "target", "owner", "mixer", "bucket")
        ),
        "Water transfer item identities differ",
    )
    if take:
        inverse = tank.get("stored_items")
        require(
            "stored_items" in tank
            and (
                inverse is None
                or isinstance(inverse, list)
                and all(isinstance(entity, str) and entity for entity in inverse)
                and len(set(inverse)) == len(inverse)
            ),
            "Tank take post inventory missing or malformed",
        )
        require(
            event["bucket_before"] == "BucketEmpty"
            and type(event["route_found"]) is bool
            and bucket.get("resource") == "BucketWater"
            and bucket.get("delivering_to") == event["mixer"],
            "Tank take result differs",
        )
        active(ctx, event, {"GoingToDestination"}, len(ids))
        require(
            isinstance(event["inputs_before"], list)
            and len(event["inputs_before"]) == len(ids),
            "Tank take branch inputs missing",
        )
        for entity, item in zip(ids, event["inputs_before"]):
            require(
                exact(
                    item,
                    dict(
                        entity=entity,
                        resource="Water",
                        stored_in=event["owner"],
                        loaded_in=None,
                    ),
                )
                and item.get("resource") == "Water"
                and item.get("stored_in") == event["owner"]
                and "loaded_in" in item
                and item["loaded_in"] is None
                and not any(i.get("entity") == entity for i in ctx.get("items", []))
                and (inverse is None or entity not in inverse),
                "Tank take consumed item differs",
            )
    else:
        require(
            all(type(event[k]) is int for k in ("amount", "before_count", "capacity"))
            and 1 <= event["amount"] <= 5
            and 0 <= event["before_count"] < event["capacity"]
            and len(ids)
            == min(event["amount"], event["capacity"] - event["before_count"])
            and event["capacity"] == mixer["stockpile"]["capacity"]
            and type(event["returning"]) is bool
            and bucket.get("resource") == "BucketEmpty"
            and "delivering_to" in bucket
            and bucket["delivering_to"] is None
            and isinstance(mixer.get("stored_items"), list),
            "Mixer pour count/cleanup differs",
        )
        for entity in ids:
            item = one(ctx.get("items", []), entity)
            require(
                item.get("resource") == "Water"
                and item.get("stored_in") == event["mixer"]
                and "loaded_in" in item
                and item["loaded_in"] is None
                and mixer["stored_items"].count(entity) == 1
                and not any(i.get("entity") == entity for i in old.get("items", [])),
                "Mixer pour output differs",
            )
        if event["returning"]:
            active(ctx, event, {"ReturningBucket"}, event["amount"])
        else:
            actor = one(after.get("actors", []), event["worker"])
            require(
                "task_identity" in actor
                and actor["task_identity"] is None
                and not any(
                    i.get("entity") == event["worker"]
                    for i in ctx.get("bucket_actors", [])
                )
                and not any(
                    i.get("entity") == event["worker"]
                    for i in ctx.get("working_relations", [])
                ),
                "Mixer return abort differs",
            )
        require(
            not any(
                e.get("producer") == "TaskCompletedVisualMessage"
                and e.get("worker") == event["worker"]
                and e.get("assignment") == event["assignment"]
                and e.get("sample_index") == index
                for e in events
            ),
            "Mixer pour is not transport completion",
        )


def check_lossless_chain(raw):
    """A dedicated supply leg, stricter than observing partial or aborted branches."""
    from scripts.building_production_lifecycle import check_raw_transitions

    check_raw_transitions(raw)
    takes = [e for e in raw["events"] if e.get("operation") == "TankTake"]
    pours = [e for e in raw["events"] if e.get("operation") == "MixerPour"]
    require(takes and pours, "Water supply lacks actual take/pour")
    used, last = set(), {}
    for pour in pours:
        prior = [
            t
            for t in takes
            if t["bucket"] == pour["bucket"]
            and t["sample_index"] < pour["sample_index"]
        ]
        require(prior, "Mixer pour lacks prior Tank take")
        take = max(prior, key=lambda event: event["sample_index"])
        require(
            take["event_index"] not in used
            and take["route_found"] is True
            and pour["returning"] is True
            and last.get(pour["bucket"], -1) < take["sample_index"]
            and all(
                take[k] == pour[k]
                for k in (
                    "world_epoch",
                    "worker",
                    "assignment",
                    "target",
                    "owner",
                    "mixer",
                    "bucket",
                )
            )
            and take["real_seconds"] < pour["real_seconds"]
            and raw["samples"][take["sample_index"]]["real_seconds"]
            < raw["samples"][pour["sample_index"]]["real_seconds"]
            and len(take["inputs"]) == pour["amount"] == len(pour["outputs"]),
            "Water supply is foreign, reused, lossy or failed",
        )
        used.add(take["event_index"])
        last[pour["bucket"]] = pour["sample_index"]
    require(len(used) == len(takes), "Water supply has an unmatched Tank take")
