"""Synthetic receipt refusals, never native or art acceptance evidence."""
import copy
import unittest

from scripts import ui_spa_service_departure as service


def fixture():
    previous = {"entity": "soul", "fatigue": .71, "service": {
        "controller": "familiar", "controller_bits": 42, "controller_is_familiar": True,
        "release_fatigue_threshold": .7}}
    before = {"world_epoch": 3, "sample_index": 9, "real_seconds": 1.}
    after = {"world_epoch": 3, "sample_index": 10, "real_seconds": 1.1,
             "spa_context": {"souls": [{"entity": "soul", "service": None, "task": "None",
                                        "generate_power": None, "working_on": None}]}}
    raw = {"events": [
        {"producer": "SquadManagementRequest", "operation": "SoulServiceReleaseRequested",
         "worker": "soul", "reason": "Fatigued", **previous["service"],
         "world_epoch": 3, "sample_index": 10, "real_seconds": 1.05, "event_index": 20},
        {"producer": "OnReleasedFromService", "operation": "SoulServiceReleased", "worker": "soul",
         "world_epoch": 3, "sample_index": 10, "real_seconds": 1.05, "event_index": 21}]}
    return raw, previous, before, after


class ServiceDepartureTests(unittest.TestCase):
    def test_actual_nondefault_threshold_and_receipt_refusals(self):
        raw, previous, before, after = fixture()
        service.verify(raw, "soul", previous, before, after)
        for change in ("owner", "bits", "tag", "threshold", "bool", "nan", "at-threshold",
                       "missing-request", "missing-outcome", "duplicate", "worker", "reason", "epoch",
                       "early", "late", "index", "order", "task", "working", "service", "gap"):
            r, p, b, a = copy.deepcopy((raw, previous, before, after))
            if change == "owner":
                r["events"][0]["controller"] = "foreign"
            elif change == "bits":
                r["events"][0]["controller_bits"] += 1
            elif change == "tag":
                p["service"]["controller_is_familiar"] = False
            elif change == "threshold":
                r["events"][0]["release_fatigue_threshold"] = .6
            elif change in ("bool", "nan"):
                p["service"]["release_fatigue_threshold"] = True if change == "bool" else float("nan")
            elif change == "at-threshold":
                p["fatigue"] = .7
            elif change == "missing-request":
                r["events"].pop(0)
            elif change == "missing-outcome":
                r["events"].pop()
            elif change == "duplicate":
                r["events"].append(copy.deepcopy(r["events"][0]))
            elif change == "worker":
                r["events"][1]["worker"] = "foreign"
            elif change == "reason":
                r["events"][0]["reason"] = "Unknown"
            elif change == "epoch":
                r["events"][0]["world_epoch"] -= 1
            elif change in ("early", "late"):
                r["events"][0]["real_seconds"] = .9 if change == "early" else 1.2
            elif change == "index":
                r["events"][1]["sample_index"] -= 1
            elif change == "order":
                r["events"][1]["event_index"] = 19
            elif change == "task":
                a["spa_context"]["souls"][0]["task"] = "GeneratePower(...)"
            elif change == "working":
                a["spa_context"]["souls"][0]["working_on"] = "tile"
            elif change == "service":
                a["spa_context"]["souls"][0]["service"] = p["service"]
            else:
                a["sample_index"] += 1
            with self.subTest(change=change), self.assertRaises(RuntimeError):
                service.verify(r, "soul", p, b, a)
