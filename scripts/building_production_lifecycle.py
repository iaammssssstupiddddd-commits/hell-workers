"""Independent raw-observation predicates for production lifecycle result legs.

Witnesses reference samples and recorded input/domain events; a leg name or an
accepted flag alone never passes. The host driver must provide these observations
from ordinary world operations. Missing driver coverage remains a hard failure.
"""
from __future__ import annotations

if __package__:
    from . import building_production_acceptance as evidence
    from . import building_bridge_acceptance as bridge
else:
    import building_production_acceptance as evidence
    import building_bridge_acceptance as bridge

require = evidence.require
TRANSITION_CATEGORIES = {"owners": "entity", "blueprints": "entity", "roots": "entity",
                         "items": "entity", "move_tasks": "entity", "generations": "kind", "owner_layers": "entity"}


def check_raw_transitions(raw):
    """Check indexed raw state deltas; they never imply an operation succeeded."""
    require(raw.get("schema_version") == 2 and raw.get("transition_contract") == "indexed-state-delta-v1",
            "raw transition contract missing or stale")
    samples, events_seen = raw["samples"], raw["events"]
    require(all(type(sample.get("sample_index")) is int and sample["sample_index"] == i
                for i, sample in enumerate(samples)), "raw sample ordinal changed")
    require(all(type(event.get("event_index")) is int and event["event_index"] == i
                for i, event in enumerate(events_seen)), "raw event ordinal changed")
    producers = {
        "hw_jobs::publish_building_completed": (("BuildingCompleted",), ("blueprint_owner", "owner", "kind")),
        "BuildingCompletedVisualMessage": (("BuildingCompletedVisualMessage",), ("blueprint_owner",)),
        "TaskCompletedVisualMessage": (("TaskCompleted",), ("worker", "assignment", "target", "work_type")),
        "DreamTransferredVisualMessage": (("DreamTransferred",), ("worker",)),
        "SaveLoadOutcome": (("Save", "Load"), ("target", "source", "result")),
        "DeconstructionCommitOutcome": (("Deconstruct",), ("worker", "order", "owner", "result")),
    }
    for event in events_seen:
        producer = event.get("producer")
        if producer in producers:
            operations, fields = producers[producer]
            require(event.get("operation") in operations
                    and all(isinstance(event.get(key), str) and event[key] for key in fields),
                    "domain message producer/payload mismatch")
        elif producer != "production_snapshot_delta":
            require(producer is None and event.get("operation") in {"PointerPress", "PointerRelease", "KeyPress", "KeyRelease"},
                    "unrecognized raw domain producer")
        else:
            require(event.get("operation") == "StateTransition", "state delta cannot declare a domain outcome")

    def indexed(sample, category, key):
        rows = sample.get(category)
        cap = {"items": 4096, "owner_layers": 2048, "generations": 9}.get(category, 512)
        require(isinstance(rows, list) and len(rows) <= cap, f"missing/overflow raw state category: {category}")
        result = {}
        for row in rows:
            require(isinstance(row, dict) and isinstance(row.get(key), str) and row[key]
                    and row[key] not in result, "missing/duplicate raw entity identity")
            result[row[key]] = row
        return result

    for sample in samples:
        for category, key in TRANSITION_CATEGORIES.items():
            indexed(sample, category, key)
    expected = []
    for i in range(1, len(samples)):
        for category, key in TRANSITION_CATEGORIES.items():
            before, after = (indexed(sample, category, key) for sample in samples[i - 1:i + 1])
            for entity in sorted(before.keys() | after.keys()):
                old, new = before.get(entity), after.get(entity)
                if evidence.pipeline.canonical(old) != evidence.pipeline.canonical(new):
                    expected.append({"producer": "production_snapshot_delta", "operation": "StateTransition",
                                     "sample_index": i, "before_sample_index": i - 1,
                                     "real_seconds": samples[i]["real_seconds"], "category": category,
                                     "entity": entity, "before": old, "after": new})
                    require(len(expected) < 4096, "raw transition cap exceeded")
    observed = [{k: v for k, v in event.items() if k != "event_index"}
                for event in events_seen if event.get("operation") == "StateTransition"]
    require(evidence.pipeline.canonical(observed) == evidence.pipeline.canonical(expected),
            "missing/remapped/synthetic raw state transition")


def check_domain_witness(witness, trace):
    """A witness selects raw facts; no driver-supplied domain result is allowed."""
    require(type(witness.get("before")) is int and type(witness.get("after")) is int
            and 0 <= witness["before"] < witness["after"] < len(trace["samples"]),
            "invalid domain witness range")
    selected = witness.get("domain_event_indices")
    require(isinstance(selected, list) and selected and all(type(i) is int for i in selected)
            and selected == sorted(set(selected)), "missing raw domain/state witness")
    owner = witness.get("owner", trace.get("owner"))
    require(isinstance(owner, str) and owner, "domain witness lacks exact owner")
    bound_owner = False
    if "events" in witness:
        require(isinstance(witness["events"], list)
                and all(type(i) is int and 0 <= i < len(trace["events"]) for i in witness["events"]),
                "predicate event index outside raw trace")
        require(set(selected) <= set(witness["events"]), "domain witness is not among selected predicate events")
        require(all(i in selected for i in witness["events"] if "result" in trace["events"][i]),
                "operation outcome lacks a domain witness")
    for index in selected:
        require(0 <= index < len(trace["events"]), "domain event index outside raw trace")
        event = trace["events"][index]
        # SaveLoadOutcome.target is a display-safe save label, not an entity.
        # This global receipt cannot supply owner correlation, even when its
        # label happens to equal an entity ID; another selected event must do so.
        session_receipt = event.get("producer") == "SaveLoadOutcome"
        event_owners = [] if session_receipt else [
            event[key] for key in ("owner", "blueprint_owner", "target", "entity")
            if event.get(key) is not None]
        if not session_receipt and isinstance(event.get("source"), dict):
            if event["source"].get("owner") is not None:
                event_owners.append(event["source"]["owner"])
        require(not event_owners or owner in event_owners, "domain/state event belongs to another owner")
        bound_owner |= owner in event_owners
        require(witness["before"] < event["sample_index"] <= witness["after"], "domain event outside witness")
        require(event.get("producer") in {"production_snapshot_delta", "hw_jobs::publish_building_completed",
                                          "SaveLoadOutcome", "DeconstructionCommitOutcome",
                                          "TaskCompletedVisualMessage", "DreamTransferredVisualMessage"},
                "input alone is not an authoritative domain/state witness")
        if event.get("producer") == "production_snapshot_delta":
            require(event["operation"] == "StateTransition" and event["before_sample_index"] >= witness["before"],
                    "state transition starts outside witness")
    require(bound_owner, "domain/state witness belongs to another owner")


COMMON = ("gallery", "placement", "construction", "save-load", "deconstruct", "generation", "cleanup", "consumers")
LEGS = {
    "Tank": (*COMMON, "water", "move", "companion"),
    "MudMixer": (*COMMON, "refining", "move"),
    "RestArea": (*COMMON, "occupancy-dream"),
    "SoulSpa": (*COMMON, "all-masks", "construction-cancel", "operational-energy-walk"),
    "WheelbarrowParking": (*COMMON, "vehicle-anchor"),
    "SandPile": (*COMMON, "resource-image"),
    "BonePile": (*COMMON, "resource-image"),
    "OutdoorLamp": (*COMMON, "power"),
    "Door": ("dedicated-generation", "consumers", "states"),
    "Bridge": tuple(bridge.LEGS),
}
BRIDGE_CASES = {
    "placement": ("south-bank", "north-bank", "occupied-deck", "occupied-bank", "river-gap", "overlong-span"),
    "passage": ("lane0-forward", "lane0-backward", "lane1-forward", "lane1-backward"),
    "adjacent": ("two-owners", "remove-one"),
    "construction": ("delivered-and-built",),
    "instant-build": ("normal-input-debug-build",),
    "cancel": ("before-delivery", "partial"),
    "save-load": ("partial", "complete"),
    "deconstruct": ("river-and-dry-restoration",),
    "non-movable": ("ui-rejected",),
    "gallery": tuple(f"{quality}-{dpi}-{zoom}" for quality in ("High", "Medium", "Low")
                     for dpi in (1.0, 1.5, 2.0) for zoom in ("normal", "far")),
    "cleanup": tuple(f"round-{i}" for i in range(10)),
}


def vector(value, length):
    return isinstance(value, list) and len(value) == length and all(
        type(v) in (int, float) and evidence.math.isfinite(v) for v in value)


def sequence(values, expected):
    index = 0
    for value in values:
        if index < len(expected) and value == expected[index]:
            index += 1
    require(index == len(expected), "required ordered state sequence missing")


def events(trace, witness):
    selected = witness["events"]
    require(isinstance(selected, list) and selected and selected == sorted(set(selected)), "missing event witness")
    require(all(type(i) is int and 0 <= i < len(trace["events"]) for i in selected), "event index out of range")
    rows = [trace["events"][i] for i in selected]
    require(all(witness["before"] <= row["sample_index"] <= witness["after"] for row in rows),
            "event outside witnessed interval")
    return rows


def operation(rows, name, result=None):
    return any(row.get("operation") == name and (result is None or row.get("result") == result) for row in rows)


def owners(sample):
    return {row["entity"]: row for row in sample["owners"]}


def cleaned(sample, owner):
    require(owner not in owners(sample)
            and not any(row["owner"] == owner for row in sample["roots"])
            and not any(row.get("owner") == owner for row in sample["items"])
            and not any(row.get("blueprint_owner") == owner for row in sample["consumers"]),
            "owner cleanup incomplete")


def presentation(sample, owner, part_count):
    roots = [row for row in sample["roots"] if row["owner"] == owner]
    require(owner in owners(sample) and len(roots) == 1 and len(roots[0]["parts"]) == part_count,
            "owner/root/part cardinality differs")
    require(all(part["mesh"] in sample["resident_mesh_handles"] for part in roots[0]["parts"]),
            "nonresident displayed part")


def bridge_witness(case, leg, witness, trace, value):
    before, after = trace["samples"][witness["before"]], trace["samples"][witness["after"]]
    rows, owner = events(trace, witness), witness["owner"]
    samples = trace["samples"][witness["before"]:witness["after"] + 1]
    if leg == "placement":
        require(operation(rows, "PointerPress") and operation(rows, "PointerRelease"), "ordinary pointer placement absent")
        if case in {"south-bank", "north-bank"}:
            require(operation(rows, "Placement", "Committed"), "placement did not commit")
            bp = next((bp for bp in after["blueprints"] if bp["entity"] == owner), None)
            require(bp is not None and len(bp["footprint"]) == 10
                    and bp["footprint"] in [c["footprint"] for c in before["legal_crossings"]],
                    "placement footprint differs from normal crossing resolver")
        else:
            require(operation(rows, "Placement", "Rejected")
                    and before["owners"] == after["owners"] and before["blueprints"] == after["blueprints"]
                    and before["bridge_cells"] == after["bridge_cells"], "invalid placement mutated world")
    elif leg == "passage":
        require(operation(rows, "Navigation", "Arrived"), "actual navigation did not finish")
        actor = witness["actor"]
        path = witness["expected_grid_path"]
        require(len(path) == 7 and all(vector(cell, 2) for cell in path)
                and len({p[0] for p in path}) == 1
                and all(abs(a[1] - b[1]) == 1 for a, b in zip(path, path[1:])), "bank/deck traversal path differs")
        lane = int(case[4])
        crossing = witness["crossing"]
        require(crossing in before["legal_crossings"] or crossing in before["occupied_crossings"], "unobserved crossing")
        deck = crossing["footprint"]
        require(len(deck) == 10 and all(cell in deck for cell in path[1:-1])
                and path[0] in crossing["banks"] and path[-1] in crossing["banks"]
                and path[0][0] == crossing["anchor"][0] + lane
                and (path[-1][1] > path[0][1]) == case.endswith("forward"), "wrong banks/lane/direction")
        observed = [a["grid"] for sample in samples for a in sample["actors"] if a["entity"] == actor]
        sequence(observed, path)
        require(all(a["actor_height_wu"] == witness["baseline_actor_height_wu"]
                    for sample in samples for a in sample["actors"] if a["entity"] == actor), "actor height changed")
    elif leg == "adjacent":
        neighbor = witness["neighbor"]
        require(owner != neighbor and neighbor in owners(after), "adjacent owner missing")
        presentation(after, neighbor, 1)
        if case == "two-owners":
            presentation(after, owner, 1)
            left, right = (owners(after)[entity]["footprint"] for entity in (owner, neighbor))
            require(not set(map(tuple, left)) & set(map(tuple, right))
                    and abs(min(x for x, _ in left) - min(x for x, _ in right)) == 2,
                    "bridges overlap or are not adjacent")
        else:
            cleaned(after, owner)
            require(operation(rows, "Deconstruct", "Committed") and operation(rows, "Navigation", "Arrived"),
                    "neighbor passage after deconstruction missing")
    elif leg in {"construction", "instant-build"}:
        require(operation(rows, "PointerRelease") and operation(rows, "Placement", "Committed"), "ordinary placement missing")
        presentation(after, owner, 1)
        if leg == "construction":
            progress = [bp["progress"] for sample in samples for bp in sample["blueprints"] if bp["entity"] == owner]
            require(any(p == 0 for p in progress) and any(0 < p < 1 for p in progress)
                    and operation(rows, "MaterialDelivery", "Committed") and operation(rows, "BuildWork", "Committed")
                    and all(sample["instant_build"] is False for sample in samples), "normal construction missing")
        else:
            require(after["instant_build"] is True, "Instant Build mode not observed")
    elif leg == "cancel":
        bp = next((bp for bp in before["blueprints"] if bp["entity"] == owner), None)
        require(bp is not None and (bp["progress"] == 0 if case == "before-delivery" else 0 < bp["progress"] < 1),
                "wrong cancellation stage")
        require(operation(rows, "CancelConstruction", "Committed"), "cancel outcome missing")
        cleaned(after, owner)
        require(after["bridge_cells"] == before["bridge_cells"]
                and witness["initial_walkability"] == after["footprint_walkability"]
                and after["pending_owner_tasks"] == 0
                and after["refund_items"] == before["delivered_items"], "cancel restoration/refund differs")
    elif leg == "save-load":
        require(operation(rows, "Save", "Succeeded") and operation(rows, "Load", "Succeeded")
                and after["world_epoch"] > before["world_epoch"], "real save/load replacement missing")
        loaded = witness["loaded_owner"]
        require(owner != loaded and after["paused"] is True
                and before["durable_state"] == after["durable_state"], "paused loaded durable state differs")
        cleaned(after, owner)
        if case == "complete":
            presentation(after, loaded, 1)
        else:
            require(any(bp["entity"] == loaded and 0 < bp["progress"] < 1 for bp in after["blueprints"]),
                    "partial construction not rehydrated")
    elif leg == "deconstruct":
        require(operation(rows, "Deconstruct", "Committed") and after["salvage"] == {"Rock": 3}, "deconstruction/salvage differs")
        cleaned(after, owner)
        cells = after["restored_cells"]
        require(len(cells) == 10 and any(c["river"] for c in cells)
                and all(c["walkable"] is not c["river"] and c["bridged"] is False and c["owner"] is None for c in cells),
                "river/dry walkability not restored")
    elif leg == "non-movable":
        require(operation(rows, "Move", "Rejected") and before["owners"] == after["owners"]
                and after["move_tasks"] == [] and owners(after)[owner]["movable"] is False,
                "Bridge move rejection differs")
    elif leg == "gallery":
        quality, dpi, zoom = case.split("-")
        require(after["quality"] == quality and after["dpi"] == float(dpi) and after["zoom"] == zoom,
                "quality/DPI/zoom case missing")
        require({"world", "ghost", "blueprint", "pulse", "catalog"} <= {
            c["role"] for c in after["consumers"] if c["visible"] is True
            and c["identity"] == after["identity"]}, "same-generation consumer views missing")
    elif leg == "cleanup":
        admitted = [spec["identity"] for spec in value["generation_trials"]["Bridge"]]
        require(all(sample["identity"] is None or sample["identity"] in admitted for sample in samples),
                "unadmitted Bridge generation")
        sequence([sample["identity"] for sample in samples], [None, admitted[0], admitted[1], None])
        sequence([sample["generation_phase"] for sample in samples],
                 ["cold-fallback", "active-A", "pending-B", "failed-B-retains-A", "active-C", "invalidated-fallback", "owner-removed"])
        require(all(sample["pool_active"] <= 1 and sample["pool_pending"] <= 1 for sample in samples), "unbounded Bridge pool")
        cleaned(after, owner)
        require(after["retired_application_handles"] == 0 and after["orphan_parts"] == 0, "retired ownership leaked")


def check_bridge(trace, row, value):
    leg = row["leg"]
    witnesses = trace["witnesses"]
    require([w["case"] for w in witnesses] == list(BRIDGE_CASES[leg]), "missing/reordered Bridge subcase")
    for witness in witnesses:
        require(type(witness["before"]) is int and type(witness["after"]) is int
                and 0 <= witness["before"] < witness["after"] < len(trace["samples"]), "invalid witness range")
        require({witness["before"], witness["after"]} <= {shot["sample_index"] for shot in row["captures"]},
                "witness endpoints lack owned captures")
        if leg != "gallery":
            check_domain_witness(witness, trace)
        bridge_witness(witness["case"], leg, witness, trace, value)


def check_group(trace, row, value):
    kind, leg = row["kind"], row["leg"]
    samples, events_seen = trace["samples"], trace["events"]
    if leg not in {"gallery", "consumers", "generation", "cleanup", "dedicated-generation"}:
        witnesses = trace.get("witnesses")
        require(isinstance(witnesses, list) and witnesses, "raw group domain witnesses unavailable")
        covered = set()
        for witness in witnesses:
            check_domain_witness(witness, trace)
            covered.update(witness["domain_event_indices"])
        require(all(index in covered for index, event in enumerate(events_seen)
                    if "result" in event), "group operation outcome lacks a domain witness")
    require(all(sample["identity"] == value["releases"][kind].get("identity") for sample in samples)
            if leg not in {"generation", "cleanup", "dedicated-generation"} and kind != "Door" else True,
            "group runtime identity differs")
    if leg == "gallery":
        captured = {shot["sample_index"] for shot in row["captures"]}
        actual = {(s["quality"], s["dpi"], s["zoom"]) for index, s in enumerate(samples) if index in captured}
        expected = {(quality, dpi, zoom) for quality in ("High", "Medium", "Low")
                    for dpi in (1.0, 1.5, 2.0) for zoom in ("normal", "far")}
        require(expected <= actual, "owned group quality/DPI/zoom captures missing")
    elif leg == "water":
        sequence([s["water_state"] for s in samples], ["Empty", "Partial", "Full", "Empty"])
        contract = trace["production_state"]
        manifest, _, _ = evidence.pipeline.load_set(
            evidence.Path(value["releases"][kind]["root"]), kind, evidence.Path(value["codec"]["path"]))
        require(contract == manifest["production_state"] and contract["kind"] == "Tank", "wrong motion contract")
        for s in samples:
            require(s["water_visible"] is (s["water_state"] != "Empty")
                    and s["water_y_wu"] == contract["full_y_wu" if s["water_state"] == "Full" else "partial_y_wu"],
                    "water state/position differs")
    elif leg == "refining":
        manifest, _, _ = evidence.pipeline.load_set(
            evidence.Path(value["releases"][kind]["root"]), kind, evidence.Path(value["codec"]["path"]))
        contract = manifest["production_state"]
        require(trace["production_state"] == contract and contract["kind"] == "MudMixer",
                "rotor contract does not match admitted manifest")
        require(all(evidence.number(s["rotor_angle"]) and s["rotor_angle"] < evidence.math.tau
                    and s["rotor_axis"] == contract["axis"]
                    and s["body_local_pose"] == samples[0]["body_local_pose"] for s in samples),
                "rotor axis/angle or fixed body differs")
        sequence([(s["refining"], s["paused"]) for s in samples],
                 [(False, False), (True, False), (True, True), (True, False), (False, False)])
        moved = False
        for a, b in zip(samples, samples[1:]):
            if b["paused"] or not b["refining"]:
                require(a["rotor_angle"] == b["rotor_angle"], "paused/idle rotor advanced")
            else:
                if a["refining"] and not a["paused"]:
                    expected = (a["rotor_angle"] + (b["virtual_seconds"] - a["virtual_seconds"])
                                * contract["radians_per_second"]) % evidence.math.tau
                    require(abs(evidence.math.remainder(b["rotor_angle"] - expected, evidence.math.tau)) < .001,
                            "rotor rate differs from admitted contract")
                moved |= a["rotor_angle"] != b["rotor_angle"]
        require(moved, "refining rotor never moved")
    elif leg == "all-masks":
        require({s["worker_mask"] for s in samples} == set(range(16)), "all Spa masks missing")
        require(all(s["lit_mask"] == s["worker_mask"] and s["part_count"] == 5 for s in samples), "Spa slot mapping differs")
    elif leg in {"power", "states"}:
        expected = {False, True} if kind == "OutdoorLamp" else {"EWClosed", "EWOpen", "EWLocked", "NSClosed", "NSOpen", "NSLocked"}
        require({s["state"] for s in samples} == expected
                and all(s["rendered_state"] == s["state"] for s in samples), "state/rendered state differs")
    elif leg in {"generation", "cleanup", "dedicated-generation"}:
        require(len(trace["cycles"]) == 10, "ten lifecycle cycles required")
        for cycle in trace["cycles"]:
            check_domain_witness(cycle, trace)
            require(type(cycle["before"]) is int and type(cycle["after"]) is int
                    and 0 <= cycle["before"] < cycle["after"] < len(samples), "invalid cleanup interval")
            interval = samples[cycle["before"]:cycle["after"] + 1]
            if kind != "Door":
                admitted = [spec["identity"] for spec in value["generation_trials"][kind]]
                require(all(s["identity"] is None or s["identity"] in admitted for s in interval), "unadmitted generation")
                sequence([s["identity"] for s in interval], [admitted[0], admitted[1], None])
            require(cycle["before_counts"] == interval[0]["resource_counts"]
                    and cycle["after_counts"] == interval[-1]["resource_counts"]
                    and cycle["phases"] == [s["generation_phase"] for s in interval], "cleanup witnesses differ from samples")
            require(cycle["before_counts"] == cycle["after_counts"]
                    and cycle["retired_application_handles"] == 0 and cycle["orphan_parts"] == 0,
                    "generation/owner cleanup leaked")
            sequence(cycle["phases"], ["active-A", "failed-B-retains-A", "active-C", "fallback", "owner-removed"])
    elif leg == "consumers":
        required = {"world", "ghost", "blueprint", "pulse", "catalog"}
        if kind in {"Tank", "MudMixer"}:
            required |= {"move", "destination"}
        require(required <= {c["role"] for s in samples for c in s["consumers"] if c["visible"] is True
                             and c["identity"] == s["identity"]}, "consumer coverage missing")
    else:
        expected = {
            "placement": [("Placement", "Rejected"), ("Placement", "Committed")],
            "construction": [("MaterialDelivery", "Committed"), ("BuildWork", "Committed")],
            "save-load": [("Save", "Succeeded"), ("Load", "Succeeded")],
            "deconstruct": [("Deconstruct", "Committed")],
            "move": [("Move", "Rejected"), ("Move", "Cancelled"), ("Move", "Committed")],
            "companion": [("CompanionPlacement", "Rejected"), ("CompanionPlacement", "Committed")],
            "occupancy-dream": [("Rest", "Entered"), ("Dream", "Emitted"), ("Rest", "Exited")],
            "construction-cancel": [("CancelConstruction", "Committed")],
            "operational-energy-walk": [("BuildWork", "Committed"), ("EnergyDistribution", "Committed"), ("Navigation", "Arrived")],
            "vehicle-anchor": [("VehicleParking", "Committed"), ("VehicleDeparture", "Committed")],
            "resource-image": [("Gather", "Committed"), ("Transport", "Committed")],
        }[leg]
        sequence([(e["operation"], e.get("result")) for e in events_seen], expected)
        require(trace["before_gameplay_contract_sha256"] == trace["after_gameplay_contract_sha256"]
                == value["gameplay_contract_sha256"], "gameplay rules changed")
        if leg in {"resource-image", "vehicle-anchor"}:
            require(evidence.hashed(trace["baseline_item_image_sha256"])
                    and all(s["item_image_sha256"] == trace["baseline_item_image_sha256"] for s in samples),
                    "resource/vehicle image changed")
        if leg == "save-load":
            require(samples[-1]["world_epoch"] > samples[0]["world_epoch"]
                    and samples[-1]["old_world_references"] == 0
                    and samples[-1]["durable_state"] == samples[0]["durable_state"], "load reconstruction differs")
        if leg in {"deconstruct", "construction-cancel"}:
            cleaned(samples[-1], trace["owner"])


def verify(root, rows, value, plan_hash, seen):
    expected = {(kind, leg) for kind in evidence.GROUPS[value["scope"]] for leg in LEGS[kind]}
    require(len(rows) == len(expected) and {(r["kind"], r["leg"]) for r in rows} == expected,
            "missing/duplicate lifecycle legs")
    for row in rows:
        trace = evidence.session(root, row, value, plan_hash, seen, performance=False)
        require(all("identity" in sample for sample in trace["samples"])
                if row["kind"] != "Door" else all("door_identity" in sample for sample in trace["samples"]),
                "raw collector lacks per-kind lifecycle identity; no synthesized projection is permitted")
        if row["kind"] == "Bridge":
            require(isinstance(trace.get("witnesses"), list), "raw Bridge witnesses unavailable")
        elif row["leg"] in {"generation", "cleanup", "dedicated-generation"}:
            require(isinstance(trace.get("cycles"), list), "raw generation/cleanup cycles unavailable")
        require(row["instrument"] == "Capture" and trace["leg"] == row["leg"]
                and trace["scope"] == ("bridge-normal-world-observations-only" if row["kind"] == "Bridge"
                                       else "production-normal-world-observations-v1"), "legacy or wrong lifecycle trace")
        previous = -1
        for sample in trace["samples"]:
            require(evidence.number(sample["real_seconds"]) and sample["real_seconds"] > previous, "nonmonotonic observation")
            previous = sample["real_seconds"]
        require(len(trace["samples"]) >= 2, "no lifecycle observations")
        if row["kind"] == "Bridge":
            require(trace["identity"] == value["releases"]["Bridge"]["identity"], "Bridge identity differs")
            require(all(sample["identity"] in (None, trace["identity"]) for sample in trace["samples"])
                    or row["leg"] == "cleanup", "Bridge sample identity differs")
            check_bridge(trace, row, value)
        else:
            if row["kind"] == "Door":
                if __package__:
                    from .perf_tool.artifact_readers.building_m6 import DOOR_HASH
                else:
                    from perf_tool.artifact_readers.building_m6 import DOOR_HASH
                expected_door = {"asset_set_generation": 7, "authority": "release_approved", "manifest_sha256": DOOR_HASH}
                require(trace["door_identity"] == expected_door
                        and all(s["door_identity"] == expected_door for s in trace["samples"]), "dedicated Door identity differs")
            check_group(trace, row, value)
