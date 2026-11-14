"""Loaded-case natural service release, bound to actual passive domain witnesses."""
from scripts import ui_spa_resumed_use as resumed

spa, native = resumed.spa, resumed.native


def verify(raw, worker, previous, before, after):
    service = previous.get("service")
    native.require(isinstance(service, dict) and isinstance(service.get("controller"), str)
                   and service["controller"] and type(service.get("controller_bits")) is int
                   and service["controller_bits"] >= 0 and service.get("controller_is_familiar") is True,
                   "Spa departure service owner missing")
    threshold = service.get("release_fatigue_threshold")
    native.require(spa.finite(threshold) and 0 <= threshold <= 1
                   and spa.finite(previous.get("fatigue")) and threshold < previous["fatigue"] <= 1,
                   "Spa departure did not exceed the observed service threshold")
    ending = next((s for s in after["spa_context"]["souls"] if s["entity"] == worker), None)
    native.require(ending is not None and "service" in ending and ending["service"] is None
                   and ending.get("task") == "None" and ending.get("generate_power") is None
                   and ending.get("working_on") is None,
                   "Spa departure retained service/task ownership")
    requests = [r for r in raw["events"] if r.get("operation") == "SoulServiceReleaseRequested"
                and r.get("worker") == worker]
    outcomes = [r for r in raw["events"] if r.get("operation") == "SoulServiceReleased"
                and r.get("worker") == worker]
    native.require(len(requests) == len(outcomes) == 1, "Spa departure release receipt missing/reused")
    request, outcome = requests[0], outcomes[0]
    native.require(request.get("producer") == "SquadManagementRequest" and request.get("reason") == "Fatigued"
                   and all(type(request.get(k)) is type(service[k]) and request[k] == service[k]
                           for k in ("controller", "controller_bits", "controller_is_familiar", "release_fatigue_threshold"))
                   and outcome.get("producer") == "OnReleasedFromService",
                   "Spa departure release controller/threshold/reason differs")
    native.require(type(before.get("world_epoch")) is int and before["world_epoch"] == after.get("world_epoch")
                   and type(before.get("sample_index")) is int
                   and after.get("sample_index") == before["sample_index"] + 1
                   and all(type(r.get("world_epoch")) is int and r["world_epoch"] == after["world_epoch"]
                           and type(r.get("sample_index")) is int and r["sample_index"] == after["sample_index"]
                           and spa.finite(r.get("real_seconds"))
                           and before["real_seconds"] < r["real_seconds"] <= after["real_seconds"]
                           for r in (request, outcome))
                   and type(request.get("event_index")) is int and type(outcome.get("event_index")) is int
                   and request["event_index"] < outcome["event_index"]
                   and request["real_seconds"] <= outcome["real_seconds"],
                   "Spa departure receipt epoch/sample/time differs")
