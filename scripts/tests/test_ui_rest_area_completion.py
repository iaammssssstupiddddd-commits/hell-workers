"""Synthetic predicate tests; these fixtures are not native acceptance evidence."""
from __future__ import annotations

import copy
import importlib
from pathlib import Path
import sys
import unittest

HELPERS = Path(__file__).resolve().parents[2] / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPERS))
acceptance = importlib.import_module("ui_usability_acceptance")
completion = acceptance.rest_area_planning


class OrdinaryCompletionTests(unittest.TestCase):
    def fixture(self):
        footprint = [[40, 40], [41, 40], [40, 41], [41, 41]]
        entries = []
        for index, case in enumerate(completion.COMPLETION_CHECKPOINTS):
            owner = None if index == 0 else 7 if index == 1 else 8
            evidence = {"anchor": [40, 40], "footprint": footprint, "draw_pos": [20.0, 20.0],
                        "legal": index == 0, "buildings": [8] if index == 2 else [],
                        "blueprints": [{"entity": 7, "footprint": footprint, "progress": 0,
                                        "materials_complete": False}] if index == 1 else [],
                        "cells": [{"grid": grid, "owner": owner, "walkable": index == 0}
                                  for grid in footprint]}
            entries.append({"case": case, "last_step": index * 5,
                            "value": {"frame": 100 + index * 10, "world_epoch": 0,
                                      "nonce": "ordinary-completion", "pid": 123, "paused": True,
                                      "rest_area_planning": evidence}})
        samples = []
        for index in range(7):
            blueprint = {"entity": "blueprint", "entity_bits": 7, "kind": "RestArea",
                         "footprint": footprint, "progress": index / 10,
                         "materials_complete": index == 5, "delivered_materials": {"Wood": index}}
            layers = [{"entity": "blueprint", "exists": index < 6,
                       "building_grids": footprint if index < 6 else [],
                       "floor_grids": [], "door_grids": [], "bridge_grids": [], "stockpile_grids": []}]
            if index == 6:
                layers.append({**layers[0], "entity": "building", "exists": True, "building_grids": footprint})
            samples.append({"sample_index": index, "real_seconds": float(index), "world_epoch": 0,
                            "blueprints": [blueprint] if index < 6 else [],
                            "actors": [{"entity": "worker", "task_identity": {
                                "assignment": "blueprint", "target": "blueprint", "work_type": "Build"}}],
                            "items": [], "owner_layers": layers,
                            "owners": [{"entity": "building", "entity_bits": 8, "kind": "RestArea",
                                        "provisional": False}] if index == 6 else []})
        events = []
        for count in range(5):
            events.append({"producer": "ordinary-blueprint-delivery-branch-v1",
                           "operation": "MaterialDelivery", "result": "Committed",
                           "sample_index": count + 1, "event_index": count, "real_seconds": float(count + 1),
                           "worker": "worker", "assignment": "request", "owner": "blueprint",
                           "item": f"wood-{count}", "world_epoch": 0, "kind": "RestArea", "resource": "Wood",
                           "before_count": count, "after_count": count + 1, "footprint": footprint,
                           "wheelbarrow": False})
        events.append({"producer": "TaskCompletedVisualMessage", "operation": "TaskCompleted",
                       "sample_index": 5, "event_index": 5, "real_seconds": 5.0,
                       "worker": "worker", "assignment": "blueprint", "target": "blueprint", "work_type": "Build"})
        events.append({"producer": "hw_jobs::publish_building_completed", "operation": "BuildingCompleted",
                       "sample_index": 6, "event_index": 6, "real_seconds": 6.0,
                       "blueprint_owner": "blueprint", "owner": "building", "kind": "RestArea", "footprint": footprint})
        raw = {"scope": "production-raw-observations-v1", "schema_version": 2, "failure": None,
               "nonce": "ordinary-completion", "process_id": 123, "accepted": False,
               "performance_evidence": False, "promotion_authority": False, "samples": samples, "events": events}
        return raw, entries, {"nonce": "ordinary-completion", "pid": 123}

    def test_exact_real_branch_delivery_task_and_ownership_witnesses(self):
        raw, entries, client = self.fixture()
        original = copy.deepcopy(raw)
        completion.check_completion(raw, entries, client)
        self.assertEqual(raw, original)
        self.assertEqual(acceptance.session_matrix("rest-area-completion", True),
                         [("rest-area-completion", *acceptance.VIEWPORTS[0])])

    def test_missing_foreign_or_inferred_completion_is_rejected(self):
        mutations = (
            lambda raw, entries: raw.update(nonce="foreign"),
            lambda raw, entries: raw.update(accepted=True),
            lambda raw, entries: raw.update(performance_evidence=True),
            lambda raw, entries: raw.update(promotion_authority=True),
            lambda raw, entries: raw.update(failure="cap exceeded"),
            lambda raw, entries: raw["events"].pop(),
            lambda raw, entries: raw["events"].append(copy.deepcopy(raw["events"][-1])),
            lambda raw, entries: raw["events"][-1].update(producer="production_snapshot_delta"),
            lambda raw, entries: raw["events"][-1].update(blueprint_owner="foreign"),
            lambda raw, entries: raw["events"][-1].update(owner="blueprint"),
            lambda raw, entries: raw["events"][-1].update(real_seconds=float("nan")),
            lambda raw, entries: raw["events"][-1].update(sample_index=0),
            lambda raw, entries: raw["events"][-1].update(footprint=[[1, 2]]),
            lambda raw, entries: raw["samples"][6].update(world_epoch=1),
            lambda raw, entries: raw["samples"][5]["blueprints"][0].update(materials_complete=False),
            lambda raw, entries: raw["samples"][5]["blueprints"][0].update(entity_bits=99),
            lambda raw, entries: raw["samples"][6]["owners"][0].update(entity_bits=99),
            lambda raw, entries: raw["samples"][6]["owners"][0].update(provisional=True),
            lambda raw, entries: raw["samples"][6]["owner_layers"][0].update(exists=True),
            lambda raw, entries: raw["samples"][6]["owner_layers"][1].update(building_grids=[]),
            lambda raw, entries: raw["events"][5].update(work_type="Haul"),
            lambda raw, entries: raw["events"][5].update(target="foreign"),
            lambda raw, entries: raw["events"][5].update(worker="foreign"),
            lambda raw, entries: raw["events"][5].update(real_seconds=float("nan")),
            lambda raw, entries: raw["events"][5].update(assignment=""),
            lambda raw, entries: raw["events"][5].update(assignment="foreign"),
            lambda raw, entries: raw["events"][5].update(sample_index=0, real_seconds=0),
            lambda raw, entries: raw["events"][5].update(real_seconds=4.5),
            lambda raw, entries: raw["events"][4].update(item="wood-0"),
            lambda raw, entries: raw["events"][4].update(before_count=3),
            lambda raw, entries: entries[2]["value"].update(world_epoch=1),
            lambda raw, entries: entries[2]["value"].update(paused=False),
            lambda raw, entries: entries[2]["value"].update(pid=999),
            lambda raw, entries: entries[2]["value"]["rest_area_planning"].update(blueprints=[{}]),
            lambda raw, entries: entries[2]["value"]["rest_area_planning"]["cells"][0].update(owner=99),
            lambda raw, entries: entries[2]["value"]["rest_area_planning"]["cells"][0].update(walkable=True),
            lambda raw, entries: entries[2].update(last_step=0),
        )
        for mutate in mutations:
            raw, entries, client = self.fixture()
            mutate(raw, entries)
            with self.subTest(mutation=mutate), self.assertRaises((RuntimeError, ValueError)):
                completion.check_completion(raw, entries, client)

    def test_same_sample_domain_callback_precedes_later_task_message_drain(self):
        raw, entries, client = self.fixture()
        task = raw["events"].pop(5)
        domain = raw["events"][-1]
        domain["event_index"] = 5
        task.update(sample_index=6, event_index=6, real_seconds=6.0)
        raw["events"].append(task)
        completion.check_completion(raw, entries, client)
        task["real_seconds"] = 5.5
        with self.assertRaises(RuntimeError):
            completion.check_completion(raw, entries, client)

    def test_build_chain_keeps_the_real_originating_delivery_assignment(self):
        raw, entries, client = self.fixture()
        raw["events"][5]["assignment"] = "original-delivery-request"
        for sample in raw["samples"]:
            sample["actors"][0]["task_identity"]["assignment"] = "original-delivery-request"
        completion.check_completion(raw, entries, client)


if __name__ == "__main__":
    unittest.main()
